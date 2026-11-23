// Trasea desktop app: a thin Tauri shell around `trasea-core`. Everything runs on this computer; the app makes no network request.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use trasea_core::batch::{is_supported_image, run_batch as core_run_batch, scan_image_files, BatchConfig, BatchSummary};
use trasea_core::export::{export_options_for_production, palette_of, to_ase, to_separation_svgs, to_svg, Doc, ExportOptions, LinesAs};
use trasea_core::{export, hex, load, prepare, vectorize_prepared, BrushStroke, NodeHeatmap, Params, PreparedImage, ProductionMode, ProductionOptions, SymmetryMode, TextRegion};

/// The expensive first half of the conversion (loading, thin lines, clean / Real-ESRGAN) for the picture that is open.
/// Changing the palette, the brush strokes, production profile or the node target runs only the second half again.
struct CachedPrep {
    path: String,
    ai_clean: bool,
    keep_lines: bool,
    prep: Arc<PreparedImage>,
    /// The loaded picture as a PNG data URL (encoded once).
    original: String,
}

#[derive(Default)]
struct Last {
    doc: Mutex<Option<Doc>>,
    prepared: Mutex<Option<CachedPrep>>,
}

struct BatchControl {
    cancel: Arc<AtomicBool>,
    running: AtomicBool,
}

impl Default for BatchControl {
    fn default() -> Self {
        Self {
            cancel: Arc::new(AtomicBool::new(false)),
            running: AtomicBool::new(false),
        }
    }
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiParams {
    colors: usize,
    /// 0..100 from the slider
    smooth: f32,
    keep_lines: bool,
    ai_clean: bool,
    /// Write circles, ellipses and rectangles as pure shapes.
    detect_primitives: bool,
    /// "off", "auto", "vertical" or "horizontal"
    symmetry: String,
    /// Warn when the picture seems to hold text.
    detect_text: bool,
    /// The designer's own palette as `#RRGGBB` strings (empty / absent = automatic).
    #[serde(default)]
    custom_palette: Option<Vec<String>>,
    /// Explicit colour merges `[[fromHex, toHex], ...]`.
    #[serde(default)]
    palette_merges: Vec<(String, String)>,
    /// Node budget (absent = automatic).
    #[serde(default)]
    target_nodes: Option<usize>,
    /// Correction brush strokes in source pixels.
    #[serde(default)]
    brush_strokes: Vec<BrushStroke>,
    /// "standard", "screenPrint", or "cutEmbroidery"
    #[serde(default)]
    production_mode: String,
    /// Minimum stroke width in points (pt).
    #[serde(default)]
    min_stroke_pt: Option<f64>,
    /// Omit background layer (CutEmbroidery).
    #[serde(default)]
    strip_background: Option<bool>,
    /// Add 4 registration marks in outer margin (ScreenPrint).
    #[serde(default)]
    registration_marks: Option<bool>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Processed {
    svg: String,
    /// The loaded image (what the vector is compared with) as a PNG data URL.
    original: String,
    width: u32,
    height: u32,
    paths: usize,
    nodes: usize,
    bytes: usize,
    mae: Option<f64>,
    notes: Vec<String>,
    palette: Vec<String>,
    used_ai: bool,
    secs: f64,
    thin_pixels: usize,
    /// Circles, ellipses and rectangles written as pure shapes.
    primitives: usize,
    /// "vertical" / "horizontal" when the symmetry enforcer changed the picture.
    symmetry_applied: Option<String>,
    /// Suspected text areas in source pixels. Only for the preview; never exported.
    text_regions: Vec<TextRegion>,
    /// Node density for the preview overlay. Never exported.
    heatmap: NodeHeatmap,
    /// Smoothness (0..100) that was really used (differs from the slider when a node target was applied).
    effective_smooth: f32,
    /// `true` / `false` when a node target was given and met / not met.
    budget_met: Option<bool>,
    /// The picture was already prepared: loading and cleaning were not run again.
    reused: bool,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    stage: String,
    fraction: f32,
}

/// Export options chosen in the UI.
#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiExport {
    /// "stroke" or "shape"
    lines_as: String,
    /// 0, 0.25, 0.5 or 1 (points)
    trap_pt: f64,
    #[serde(default)]
    registration_marks: bool,
    #[serde(default)]
    strip_background: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiBatchConfig {
    inputs: Vec<String>,
    output_dir: String,
    params: UiParams,
    export: UiExport,
    write_pdf: bool,
    write_ase: bool,
    write_separations: bool,
}

/// Folder with the Real-ESRGAN sidecar: bundled next to the app, or TRASEA_SIDECAR for development.
fn sidecar_dir(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("realesrgan"));
    }
    if let Ok(dev) = std::env::var("TRASEA_SIDECAR") {
        candidates.push(PathBuf::from(dev));
    }
    candidates.into_iter().find(|d| d.join(trasea_core::clean::SIDECAR_EXE).is_file())
}

fn png_data_url(img: &image::RgbImage) -> Result<String, String> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}

/// `#RRGGBB` to a colour.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn to_core_params(params: &UiParams, sidecar: Option<PathBuf>) -> Params {
    let custom_palette = params.custom_palette.as_ref().and_then(|list| {
        let parsed: Vec<[u8; 3]> = list.iter().filter_map(|s| parse_hex(s)).collect();
        (parsed.len() >= 2).then_some(parsed)
    });
    let palette_merges: Vec<([u8; 3], [u8; 3])> = params
        .palette_merges
        .iter()
        .filter_map(|(a, b)| Some((parse_hex(a)?, parse_hex(b)?)))
        .collect();
    let mode = ProductionMode::parse(&params.production_mode).unwrap_or(ProductionMode::Standard);
    let mut prod = match mode {
        ProductionMode::Standard => ProductionOptions::standard(),
        ProductionMode::ScreenPrint => ProductionOptions::screen_print(),
        ProductionMode::CutEmbroidery => ProductionOptions::cut_embroidery(),
    };
    if let Some(ms) = params.min_stroke_pt {
        prod.min_stroke_pt = ms.max(0.0);
    }
    if let Some(sb) = params.strip_background {
        prod.strip_background = sb;
    }
    if let Some(rm) = params.registration_marks {
        prod.registration_marks = rm;
    }

    Params {
        colors: params.colors.clamp(2, 16),
        smooth: (params.smooth / 100.0).clamp(0.0, 1.0),
        keep_lines: params.keep_lines,
        ai_clean: params.ai_clean,
        sidecar_dir: sidecar,
        detect_primitives: params.detect_primitives,
        symmetry: SymmetryMode::parse(&params.symmetry).unwrap_or(SymmetryMode::Off),
        detect_text: params.detect_text,
        custom_palette,
        palette_merges,
        target_nodes: params.target_nodes.filter(|&n| n > 0),
        brush_strokes: params.brush_strokes.clone(),
        production: prod,
        ..Params::default()
    }
}

fn to_core_export(options: &UiExport) -> ExportOptions {
    ExportOptions {
        lines_as: if options.lines_as == "shape" { LinesAs::Shape } else { LinesAs::Stroke },
        trap_pt: options.trap_pt.clamp(0.0, 1.0),
        registration_marks: options.registration_marks,
        strip_background: options.strip_background,
    }
}

#[tauri::command]
async fn process(app: AppHandle, last: State<'_, Last>, path: String, params: UiParams) -> Result<Processed, String> {
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();
    // take what is cached for this picture (if anything) out of the lock before the long work starts
    let cached: Option<(Arc<PreparedImage>, String)> = {
        let guard = last.prepared.lock().map_err(|_| "internal error".to_string())?;
        guard
            .as_ref()
            .filter(|c| c.path == path && c.ai_clean == params.ai_clean && c.keep_lines == params.keep_lines)
            .map(|c| (c.prep.clone(), c.original.clone()))
    };
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(Processed, Doc, Option<CachedPrep>), String> {
        let t0 = Instant::now();
        let emit = |stage: &str, fraction: f32| {
            let _ = handle.emit("progress", ProgressEvent { stage: stage.to_string(), fraction });
        };
        let progress = |s: &str, f: f32| emit(s, f);
        let p = to_core_params(&params, sidecar);
        let (prep, original, reused, fresh) = match cached {
            Some((prep, original)) => (prep, original, true, None),
            None => {
                emit("Memuat gambar", 0.02);
                let img = load::load_path(Path::new(&path), load::DEFAULT_MAX_SIDE).map_err(|e| e.to_string())?;
                let original = png_data_url(&img)?;
                let prep = Arc::new(prepare(img, &p, &progress).map_err(|e| e.to_string())?);
                let entry = CachedPrep {
                    path: path.clone(),
                    ai_clean: params.ai_clean,
                    keep_lines: params.keep_lines,
                    prep: prep.clone(),
                    original: original.clone(),
                };
                (prep, original, false, Some(entry))
            }
        };
        let out = vectorize_prepared(&prep, &p, &progress).map_err(|e| e.to_string())?;
        let processed = Processed {
            svg: out.svg.clone(),
            original,
            width: prep.src.width(),
            height: prep.src.height(),
            paths: out.stats.paths,
            nodes: out.stats.nodes,
            bytes: out.stats.bytes,
            mae: out.stats.mae,
            notes: out.notes.clone(),
            palette: out.palette.iter().map(|c| hex(*c)).collect(),
            used_ai: out.used_ai,
            secs: t0.elapsed().as_secs_f64(),
            thin_pixels: out.thin_pixels,
            primitives: out.stats.primitives,
            symmetry_applied: out.symmetry_applied.clone(),
            text_regions: out.text_regions.clone(),
            heatmap: out.heatmap.clone(),
            effective_smooth: out.effective_smooth * 100.0,
            budget_met: out.budget_met,
            reused,
        };
        Ok((processed, out.doc, fresh))
    })
    .await
    .map_err(|e| format!("the conversion stopped unexpectedly: {e}"))??;
    *last.doc.lock().map_err(|_| "internal error".to_string())? = Some(result.1);
    if let Some(entry) = result.2 {
        *last.prepared.lock().map_err(|_| "internal error".to_string())? = Some(entry);
    }
    Ok(result.0)
}

/// Colour under the eyedropper: the median of the 5x5 pixels around the click, from the loaded image in memory.
#[tauri::command]
fn pick_color(last: State<'_, Last>, x: u32, y: u32) -> Result<String, String> {
    let guard = last.prepared.lock().map_err(|_| "internal error".to_string())?;
    let c = guard.as_ref().ok_or("Open a picture first.")?;
    let img = &c.prep.src;
    let (cx, cy) = (x.min(img.width() - 1) as i64, y.min(img.height() - 1) as i64);
    let mut ch: [Vec<u8>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for dy in -2..=2i64 {
        for dx in -2..=2i64 {
            let (px, py) = ((cx + dx).clamp(0, img.width() as i64 - 1) as u32, (cy + dy).clamp(0, img.height() as i64 - 1) as u32);
            let p = img.get_pixel(px, py);
            for k in 0..3 {
                ch[k].push(p[k]);
            }
        }
    }
    let med = |v: &mut Vec<u8>| {
        v.sort_unstable();
        v[v.len() / 2]
    };
    let [mut r, mut g, mut b] = ch;
    Ok(hex([med(&mut r), med(&mut g), med(&mut b)]))
}

/// Write the last result. `kind` is "svg", "pdf" or "ase".
#[tauri::command]
fn save_file(last: State<'_, Last>, kind: String, path: String, options: UiExport) -> Result<(), String> {
    let doc = last.doc.lock().map_err(|_| "internal error".to_string())?.clone().ok_or("There is nothing to export yet. Convert an image first.")?;
    let opts = to_core_export(&options);
    let write = |bytes: Vec<u8>| std::fs::write(&path, bytes).map_err(|e| format!("Cannot write {path}: {e}"));
    match kind.as_str() {
        "svg" => write(to_svg(&doc, &opts).into_bytes()),
        "pdf" => write(export::svg_to_pdf(&to_svg(&doc, &opts)).map_err(|e| e.to_string())?),
        "ase" => write(to_ase(&palette_of(&doc))),
        _ => Err("unknown export format".into()),
    }
}

/// Export 100%-black per-colour separation films for the currently open drawing (`kind`: "svg", "pdf", or "both").
#[tauri::command]
fn save_separations(last: State<'_, Last>, dir: String, base_name: String, kind: String, options: UiExport) -> Result<Vec<String>, String> {
    let doc = last.doc.lock().map_err(|_| "internal error".to_string())?.clone().ok_or("Belum ada hasil untuk diekspor.")?;
    let out_dir = PathBuf::from(&dir);
    std::fs::create_dir_all(&out_dir).map_err(|e| format!("Gagal membuat folder {dir}: {e}"))?;
    let opts = to_core_export(&options);
    let films = to_separation_svgs(&doc, &opts);
    if films.is_empty() {
        return Err("Tidak ada warna spot yang dapat dipisahkan.".into());
    }
    let stem = if base_name.trim().is_empty() { "gambar" } else { base_name.trim() };
    let write_svg = kind == "svg" || kind == "both";
    let write_pdf = kind == "pdf" || kind == "both";
    let mut written = Vec::new();
    for film in films {
        if write_svg {
            let p = out_dir.join(format!("{stem}_{}.svg", film.suffix));
            std::fs::write(&p, &film.svg).map_err(|e| format!("Gagal menulis {}: {e}", p.display()))?;
            written.push(p.to_string_lossy().to_string());
        }
        if write_pdf {
            let p = out_dir.join(format!("{stem}_{}.pdf", film.suffix));
            let bytes = export::svg_to_pdf(&film.svg).map_err(|e| e.to_string())?;
            std::fs::write(&p, bytes).map_err(|e| format!("Gagal menulis {}: {e}", p.display()))?;
            written.push(p.to_string_lossy().to_string());
        }
    }
    Ok(written)
}

#[tauri::command]
fn scan_folder(dir: String, include_subfolders: bool) -> Result<Vec<String>, String> {
    let files = scan_image_files(Path::new(&dir), include_subfolders).map_err(|e| e.to_string())?;
    Ok(files.into_iter().map(|p| p.to_string_lossy().to_string()).collect())
}

#[tauri::command]
fn collect_dropped_images(paths: Vec<String>, include_subfolders: bool) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for raw in paths {
        let p = PathBuf::from(&raw);
        if p.is_dir() {
            if let Ok(found) = scan_image_files(&p, include_subfolders) {
                for f in found {
                    out.push(f.to_string_lossy().to_string());
                }
            }
        } else if is_supported_image(&p) {
            out.push(raw);
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

#[tauri::command]
async fn run_batch(app: AppHandle, batch_ctrl: State<'_, BatchControl>, config: UiBatchConfig) -> Result<BatchSummary, String> {
    if batch_ctrl.running.swap(true, Ordering::SeqCst) {
        return Err("Proses batch lain sedang berjalan.".into());
    }
    batch_ctrl.cancel.store(false, Ordering::Relaxed);
    let cancel = batch_ctrl.cancel.clone();
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();

    let res = tauri::async_runtime::spawn_blocking(move || -> Result<BatchSummary, String> {
        let p = to_core_params(&config.params, sidecar);
        let mut eo = to_core_export(&config.export);
        let prod_eo = export_options_for_production(&p.production);
        if p.production.mode == ProductionMode::CutEmbroidery {
            eo = prod_eo;
        }
        let cfg = BatchConfig {
            inputs: config.inputs.into_iter().map(PathBuf::from).collect(),
            output_dir: PathBuf::from(config.output_dir),
            params: p,
            export: eo,
            write_pdf: config.write_pdf,
            write_ase: config.write_ase,
            write_separations: config.write_separations,
            write_report_csv: true,
            write_report_md: true,
            max_workers: None,
        };
        core_run_batch(&cfg, cancel, |prog| {
            let _ = handle.emit("batch-progress", prog);
        })
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("batch berhenti tak terduga: {e}"));

    batch_ctrl.running.store(false, Ordering::SeqCst);
    res?
}

#[tauri::command]
fn cancel_batch(batch_ctrl: State<'_, BatchControl>) -> Result<(), String> {
    batch_ctrl.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
fn open_folder(dir: String) -> Result<(), String> {
    let p = Path::new(&dir);
    if !p.exists() {
        return Err(format!("Folder tidak ditemukan: {dir}"));
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(p)
            .spawn()
            .map_err(|e| format!("Gagal membuka folder: {e}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("Gagal membuka folder: {e}"))?;
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::process::Command::new("xdg-open")
            .arg(p)
            .spawn()
            .map_err(|e| format!("Gagal membuka folder: {e}"))?;
    }
    Ok(())
}

#[tauri::command]
fn sidecar_available(app: AppHandle) -> bool {
    sidecar_dir(&app).is_some()
}

#[tauri::command]
fn third_party_licenses(app: AppHandle) -> String {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("THIRD_PARTY_LICENSES.md"));
    }
    candidates.push(PathBuf::from("../THIRD_PARTY_LICENSES.md"));
    candidates.push(PathBuf::from("THIRD_PARTY_LICENSES.md"));
    candidates.into_iter().find_map(|p| std::fs::read_to_string(p).ok()).unwrap_or_else(|| "THIRD_PARTY_LICENSES.md was not found.".to_string())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Last::default())
        .manage(BatchControl::default())
        .invoke_handler(tauri::generate_handler![
            process,
            pick_color,
            save_file,
            save_separations,
            scan_folder,
            collect_dropped_images,
            run_batch,
            cancel_batch,
            open_folder,
            sidecar_available,
            third_party_licenses
        ])
        .run(tauri::generate_context!())
        .expect("error while running Trasea");
}