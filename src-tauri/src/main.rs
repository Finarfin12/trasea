// Trasea desktop app: a thin Tauri shell around `trasea-core`. Everything runs on this computer; the app makes no network request.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use trasea_core::export::{palette_of, to_ase, to_svg, Doc, ExportOptions, LinesAs};
use trasea_core::{export, hex, load, prepare, vectorize_prepared, BrushStroke, NodeHeatmap, Params, PreparedImage, SymmetryMode, TextRegion};

/// The expensive first half of the conversion (loading, thin lines, clean / Real-ESRGAN) for the picture that is open.
/// Changing the palette, the brush strokes or the node target runs only the second half again.
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

#[derive(Deserialize)]
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
    /// Node budget (absent = automatic).
    #[serde(default)]
    target_nodes: Option<usize>,
    /// Correction brush strokes in source pixels.
    #[serde(default)]
    brush_strokes: Vec<BrushStroke>,
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

#[tauri::command]
async fn process(app: AppHandle, last: State<'_, Last>, path: String, params: UiParams) -> Result<Processed, String> {
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();
    // take what is cached for this picture (if anything) out of the lock before the long work starts
    let cached: Option<(Arc<PreparedImage>, String)> = {
        let guard = last.prepared.lock().map_err(|_| "internal error".to_string())?;
        guard.as_ref().filter(|c| c.path == path && c.ai_clean == params.ai_clean && c.keep_lines == params.keep_lines).map(|c| (c.prep.clone(), c.original.clone()))
    };
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(Processed, Doc, Option<CachedPrep>), String> {
        let t0 = Instant::now();
        let progress = |stage: &str, fraction: f32| {
            let _ = handle.emit("progress", ProgressEvent { stage: stage.to_string(), fraction });
        };
        let p = Params {
            colors: params.colors.clamp(2, 16),
            smooth: (params.smooth / 100.0).clamp(0.0, 1.0),
            keep_lines: params.keep_lines,
            ai_clean: params.ai_clean,
            sidecar_dir: sidecar,
            up: 2,
            measure: true,
            detect_primitives: params.detect_primitives,
            symmetry: SymmetryMode::parse(&params.symmetry).unwrap_or_default(),
            detect_text: params.detect_text,
            custom_palette: params.custom_palette.as_ref().map(|v| v.iter().filter_map(|s| parse_hex(s)).collect::<Vec<_>>()).filter(|v| v.len() >= 2),
            target_nodes: params.target_nodes.filter(|n| *n > 0),
            brush_strokes: params.brush_strokes.clone(),
        };
        let (prep, original, reused, fresh) = match cached {
            Some((prep, original)) => (prep, original, true, None),
            None => {
                let img = load::load_path(Path::new(&path), load::DEFAULT_MAX_SIDE).map_err(|e| e.to_string())?;
                let original = png_data_url(&img)?;
                let prep = Arc::new(prepare(img, &p, &progress).map_err(|e| e.to_string())?);
                let entry = CachedPrep { path: path.clone(), ai_clean: params.ai_clean, keep_lines: params.keep_lines, prep: prep.clone(), original: original.clone() };
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

/// Colour of one pixel of the open picture (the eyedropper): exact, from the loaded image in memory.
#[tauri::command]
fn pick_color(last: State<'_, Last>, x: u32, y: u32) -> Result<String, String> {
    let guard = last.prepared.lock().map_err(|_| "internal error".to_string())?;
    let c = guard.as_ref().ok_or("Open a picture first.")?;
    let img = &c.prep.src;
    let p = img.get_pixel(x.min(img.width() - 1), y.min(img.height() - 1));
    Ok(hex([p[0], p[1], p[2]]))
}

/// Export options chosen in the UI.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiExport {
    /// "stroke" or "shape"
    lines_as: String,
    /// 0, 0.25, 0.5 or 1 (points)
    trap_pt: f64,
}

/// Write the last result. `kind` is "svg", "pdf" or "ase".
#[tauri::command]
fn save_file(last: State<'_, Last>, kind: String, path: String, options: UiExport) -> Result<(), String> {
    let doc = last.doc.lock().map_err(|_| "internal error".to_string())?.clone().ok_or("There is nothing to export yet. Convert an image first.")?;
    let opts = ExportOptions { lines_as: if options.lines_as == "shape" { LinesAs::Shape } else { LinesAs::Stroke }, trap_pt: options.trap_pt.clamp(0.0, 1.0) };
    let write = |bytes: Vec<u8>| std::fs::write(&path, bytes).map_err(|e| format!("Cannot write {path}: {e}"));
    match kind.as_str() {
        "svg" => write(to_svg(&doc, &opts).into_bytes()),
        "pdf" => write(export::svg_to_pdf(&to_svg(&doc, &opts)).map_err(|e| e.to_string())?),
        "ase" => write(to_ase(&palette_of(&doc))),
        _ => Err("unknown export format".into()),
    }
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
        .invoke_handler(tauri::generate_handler![process, pick_color, save_file, sidecar_available, third_party_licenses])
        .run(tauri::generate_context!())
        .expect("error while running Trasea");
}
