// Trasea desktop app: a thin Tauri shell around `trasea-core`. Everything runs on this computer; the app makes no network request.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod updater;

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, Manager, State};
use trasea_core::batch::{is_supported_image, run_batch as core_run_batch, scan_image_files, BatchConfig, BatchSummary};
use trasea_core::clean::plain_path;
use trasea_core::color::{is_out_of_cmyk_gamut, rgb_to_soft_proof_rgb, ColorSpace, SpotColorMapping};
use trasea_core::dxf::DxfUnit;
use trasea_core::export::{
    export_options_for_production, palette_of, to_ase_with_spots, to_dxf, to_eps, to_pdf,
    to_separation_pdfs, to_separation_svgs, to_svg, Doc, ExportOptions, LinesAs,
};
use trasea_core::node_edit::{inspect_path_nodes as core_inspect_path_nodes, NodeEdit, PathNodeGeometry};
use trasea_core::path_edit::PathEdit;
use trasea_core::preprocess::PreprocessOptions;
use trasea_core::project::{
    load_project as core_load_project, save_project as core_save_project, sha256_file, LoadedProject,
    ProjectExportOptions, ProjectFile, ProjectOptions, PROJECT_SCHEMA_VERSION,
};
use trasea_core::svg_import::load_svg_doc;
use trasea_core::watch::{watch_folder, WatchConfig};
use trasea_core::{
    hex, load, prepare, vectorize_prepared, BrushStroke, NodeHeatmap, Params, PreparedImage,
    ProductionMode, ProductionOptions, SymmetryMode, TextRegion,
};

/// The expensive first half of the conversion (loading, preprocess, thin lines, clean / Real-ESRGAN) for the picture that is open.
struct CachedPrep {
    path: String,
    ai_clean: bool,
    keep_lines: bool,
    preprocess: PreprocessOptions,
    prep: Arc<PreparedImage>,
    /// The (optionally preprocessed) picture as a PNG data URL (encoded once).
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

struct WatchControl {
    stop: Mutex<Option<Arc<AtomicBool>>>,
    running: Arc<AtomicBool>,
}

impl Default for WatchControl {
    fn default() -> Self {
        Self {
            stop: Mutex::new(None),
            running: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn default_centerline_stroke_pt() -> f64 {
    1.0
}

fn default_die_cut_mm() -> f64 {
    2.0
}

fn default_die_cut_spot_name() -> String {
    "CutContour".to_string()
}

fn default_true() -> bool {
    true
}

fn default_sticker_backing_color() -> String {
    "#FFFFFF".to_string()
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiParams {
    colors: usize,
    /// 0..100 from the slider
    smooth: f32,
    keep_lines: bool,
    ai_clean: bool,
    detect_primitives: bool,
    symmetry: String,
    detect_text: bool,
    #[serde(default)]
    custom_palette: Option<Vec<String>>,
    #[serde(default)]
    palette_merges: Vec<(String, String)>,
    #[serde(default)]
    target_nodes: Option<usize>,
    #[serde(default)]
    brush_strokes: Vec<BrushStroke>,
    #[serde(default)]
    production_mode: String,
    #[serde(default)]
    min_stroke_pt: Option<f64>,
    #[serde(default)]
    strip_background: Option<bool>,
    #[serde(default)]
    registration_marks: Option<bool>,
    #[serde(default)]
    sablon_knockout: bool,
    #[serde(default)]
    preprocess: PreprocessOptions,
    #[serde(default)]
    centerline_invert: bool,
    #[serde(default = "default_centerline_stroke_pt")]
    centerline_stroke_pt: f64,
    #[serde(default)]
    path_edits: Vec<PathEdit>,
    /// 1.3.0 node edits (move node / handle, delete node, toggle smooth, simplify), in source pixels.
    #[serde(default)]
    node_edits: Vec<NodeEdit>,
    /// Fingerprint of the unedited trace the edits were made on; edits for another trace are skipped (1.4.0).
    #[serde(default)]
    edit_fingerprint: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PaletteProofItem {
    hex: String,
    proof_hex: String,
    out_of_gamut: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Processed {
    svg: String,
    original: String,
    width: u32,
    height: u32,
    paths: usize,
    nodes: usize,
    bytes: usize,
    mae: Option<f64>,
    notes: Vec<String>,
    palette: Vec<String>,
    palette_proof: Vec<PaletteProofItem>,
    used_ai: bool,
    secs: f64,
    thin_pixels: usize,
    primitives: usize,
    symmetry_applied: Option<String>,
    text_regions: Vec<TextRegion>,
    heatmap: NodeHeatmap,
    effective_smooth: f32,
    budget_met: Option<bool>,
    reused: bool,
    /// Fingerprint of the unedited trace (path/node edits are tied to it).
    trace_fingerprint: String,
    /// True when the sent edits were skipped because they were made on a different trace.
    edits_stale: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RawImagePreview {
    data_url: String,
    width: u32,
    height: u32,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    stage: String,
    fraction: f32,
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct UiExport {
    lines_as: String,
    trap_pt: f64,
    #[serde(default)]
    registration_marks: bool,
    #[serde(default)]
    strip_background: bool,
    #[serde(default)]
    color_space: ColorSpace,
    #[serde(default)]
    dxf_unit: DxfUnit,
    #[serde(default)]
    dxf_target_width_mm: Option<f32>,
    #[serde(default)]
    die_cut_contour: bool,
    #[serde(default = "default_die_cut_mm")]
    die_cut_offset_mm: f64,
    #[serde(default = "default_die_cut_spot_name")]
    die_cut_spot_name: String,
    #[serde(default = "default_true")]
    die_cut_bridge_islands: bool,
    #[serde(default)]
    spot_colors: Vec<SpotColorMapping>,
    #[serde(default)]
    sticker_backing: bool,
    #[serde(default = "default_sticker_backing_color")]
    sticker_backing_color: String,
    #[serde(default)]
    sticker_bleed_mm: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiBatchConfig {
    inputs: Vec<String>,
    output_dir: String,
    params: UiParams,
    export: UiExport,
    write_pdf: bool,
    #[serde(default)]
    write_eps: bool,
    #[serde(default)]
    write_dxf: bool,
    write_ase: bool,
    write_separations: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct UiWatchConfig {
    in_dir: String,
    out_dir: String,
    #[serde(default)]
    include_subfolders: bool,
    params: UiParams,
    export: UiExport,
    #[serde(default)]
    write_pdf: bool,
    #[serde(default)]
    write_eps: bool,
    #[serde(default)]
    write_dxf: bool,
    #[serde(default)]
    write_ase: bool,
    #[serde(default)]
    write_separations: bool,
    #[serde(default)]
    poll_ms: Option<u64>,
    #[serde(default)]
    debounce_ms: Option<u64>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WatchStoppedEvent {
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserPreset {
    id: String,
    name: String,
    options: ProjectOptions,
    export_options: ProjectExportOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPaletteColor {
    hex: String,
    #[serde(default)]
    spot_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SavedPalette {
    id: String,
    name: String,
    colors: Vec<SavedPaletteColor>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettings {
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_theme")]
    theme: String,
    #[serde(default = "default_canvas_bg")]
    canvas_bg: String,
    #[serde(default)]
    open_folder_after_export: bool,
    #[serde(default)]
    default_color_space: ColorSpace,
    #[serde(default)]
    default_dxf_unit: DxfUnit,
    #[serde(default = "default_dpi")]
    default_dpi: u32,
    #[serde(default = "default_die_cut_mm")]
    default_die_cut_offset_mm: f64,
    #[serde(default = "default_die_cut_spot_name")]
    default_die_cut_spot_name: String,
    #[serde(default = "default_true")]
    default_die_cut_bridge: bool,
    #[serde(default)]
    prefer_median_only: bool,
    #[serde(default = "default_undo_limit")]
    undo_limit: usize,
    #[serde(default)]
    shortcuts: HashMap<String, String>,
    #[serde(default)]
    auto_check_update: bool,
}

fn default_language() -> String {
    "id".to_string()
}

fn default_theme() -> String {
    "dark".to_string()
}

fn default_canvas_bg() -> String {
    "checkerDark".to_string()
}

fn default_dpi() -> u32 {
    96
}

fn default_undo_limit() -> usize {
    50
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: default_language(),
            theme: default_theme(),
            canvas_bg: default_canvas_bg(),
            open_folder_after_export: false,
            default_color_space: ColorSpace::Rgb,
            default_dxf_unit: DxfUnit::Mm,
            default_dpi: default_dpi(),
            default_die_cut_offset_mm: default_die_cut_mm(),
            default_die_cut_spot_name: default_die_cut_spot_name(),
            default_die_cut_bridge: true,
            prefer_median_only: false,
            undo_limit: default_undo_limit(),
            shortcuts: HashMap::new(),
            auto_check_update: false,
        }
    }
}

fn sanitize_settings(s: &mut AppSettings) {
    if s.language != "en" && s.language != "id" {
        s.language = default_language();
    }
    if !matches!(s.theme.as_str(), "dark" | "light" | "system") {
        s.theme = default_theme();
    }
    if !matches!(s.canvas_bg.as_str(), "checkerLight" | "checkerDark" | "solid") {
        s.canvas_bg = default_canvas_bg();
    }
    if !matches!(s.default_dpi, 72 | 96 | 300) {
        s.default_dpi = default_dpi();
    }
    if !s.default_die_cut_offset_mm.is_finite() {
        s.default_die_cut_offset_mm = default_die_cut_mm();
    } else {
        s.default_die_cut_offset_mm = s.default_die_cut_offset_mm.clamp(0.5, 10.0);
    }
    if s.default_die_cut_spot_name.trim().is_empty() {
        s.default_die_cut_spot_name = default_die_cut_spot_name();
    }
    if !matches!(s.undo_limit, 20 | 50 | 100) {
        s.undo_limit = default_undo_limit();
    }
}

fn config_dir(app: &AppHandle) -> Result<PathBuf, String> {
    if updater::is_portable_mode() {
        if let Ok(exe) = std::env::current_exe() {
            if let Some(parent) = exe.parent() {
                let data_dir = parent.join("data");
                if std::fs::create_dir_all(&data_dir).is_ok() {
                    let probe = data_dir.join(".trasea_write_test");
                    if std::fs::write(&probe, b"ok").is_ok() {
                        let _ = std::fs::remove_file(&probe);
                        return Ok(data_dir);
                    }
                }
                eprintln!("Trasea portable: folder data read-only, fallback ke APPDATA.");
            }
        }
    }
    let dir = app.path().app_config_dir().map_err(|e| format!("Gagal menemukan folder konfigurasi: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Gagal membuat folder konfigurasi: {e}"))?;
    Ok(dir)
}

fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_cache_dir().map_err(|e| format!("Gagal menemukan folder cache: {e}"))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("Gagal membuat folder cache: {e}"))?;
    Ok(dir)
}

fn presets_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("presets.json"))
}

fn palettes_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("palettes.json"))
}

fn settings_file(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(config_dir(app)?.join("settings.json"))
}

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
        ProductionMode::Shading => ProductionOptions::shading(),
        ProductionMode::Centerline => ProductionOptions::centerline(),
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
    prod.sablon_knockout = params.sablon_knockout;
    prod.centerline_invert = params.centerline_invert;
    prod.centerline_stroke_pt = if params.centerline_stroke_pt.is_finite() {
        params.centerline_stroke_pt.clamp(0.25, 12.0)
    } else {
        default_centerline_stroke_pt()
    };

    let colors = match mode {
        ProductionMode::Shading => params.colors.clamp(2, 32),
        ProductionMode::ScreenPrint => params.colors.clamp(2, 8),
        ProductionMode::Standard | ProductionMode::CutEmbroidery | ProductionMode::Centerline => params.colors.clamp(2, 16),
    };

    Params {
        colors,
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
        preprocess: params.preprocess.clone(),
        path_edits: params.path_edits.clone(),
        node_edits: params.node_edits.clone(),
        edit_fingerprint: params.edit_fingerprint.clone(),
        ..Params::default()
    }
}

fn to_core_export(options: &UiExport) -> ExportOptions {
    ExportOptions {
        lines_as: if options.lines_as == "shape" { LinesAs::Shape } else { LinesAs::Stroke },
        trap_pt: options.trap_pt.clamp(0.0, 2.0),
        registration_marks: options.registration_marks,
        strip_background: options.strip_background,
        color_space: options.color_space,
        dxf_unit: options.dxf_unit,
        dxf_target_width_mm: options.dxf_target_width_mm.filter(|v| v.is_finite() && *v > 0.0),
        die_cut_contour: options.die_cut_contour,
        die_cut_offset_mm: if options.die_cut_offset_mm.is_finite() {
            options.die_cut_offset_mm.clamp(0.5, 10.0)
        } else {
            default_die_cut_mm()
        },
        die_cut_spot_name: if options.die_cut_spot_name.trim().is_empty() {
            default_die_cut_spot_name()
        } else {
            options.die_cut_spot_name.trim().to_string()
        },
        die_cut_bridge_islands: options.die_cut_bridge_islands,
        spot_colors: options.spot_colors.clone(),
        sticker_backing: options.sticker_backing,
        sticker_backing_color: if options.sticker_backing_color.trim().is_empty() {
            default_sticker_backing_color()
        } else {
            options.sticker_backing_color.trim().to_string()
        },
        sticker_bleed_mm: if options.sticker_bleed_mm.is_finite() {
            options.sticker_bleed_mm.clamp(0.0, 3.0)
        } else {
            0.0
        },
    }
}

fn merge_batch_export(p: &Params, options: &UiExport) -> ExportOptions {
    let mut eo = to_core_export(options);
    if matches!(p.production.mode, ProductionMode::CutEmbroidery | ProductionMode::Centerline) {
        let prod_eo = export_options_for_production(&p.production);
        eo.lines_as = prod_eo.lines_as;
        eo.trap_pt = prod_eo.trap_pt;
        eo.registration_marks = prod_eo.registration_marks;
        eo.strip_background = prod_eo.strip_background;
    }
    eo
}

#[tauri::command]
async fn process(app: AppHandle, last: State<'_, Last>, path: String, params: UiParams) -> Result<Processed, String> {
    if path.to_ascii_lowercase().ends_with(".svg") {
        return process_svg(&last, path, params).await;
    }
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();
    let cached: Option<(Arc<PreparedImage>, String)> = {
        let guard = last.prepared.lock().map_err(|_| "internal error".to_string())?;
        guard
            .as_ref()
            .filter(|c| {
                c.path == path
                    && c.ai_clean == params.ai_clean
                    && c.keep_lines == params.keep_lines
                    && c.preprocess == params.preprocess
            })
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
                let prep = Arc::new(prepare(img, &p, &progress).map_err(|e| e.to_string())?);
                let original = png_data_url(&prep.src)?;
                let entry = CachedPrep {
                    path: path.clone(),
                    ai_clean: params.ai_clean,
                    keep_lines: params.keep_lines,
                    preprocess: params.preprocess.clone(),
                    prep: prep.clone(),
                    original: original.clone(),
                };
                (prep, original, false, Some(entry))
            }
        };
        let out = vectorize_prepared(&prep, &p, &progress).map_err(|e| e.to_string())?;
        let palette_proof: Vec<PaletteProofItem> = out
            .palette
            .iter()
            .map(|&c| PaletteProofItem {
                hex: hex(c),
                proof_hex: hex(rgb_to_soft_proof_rgb(c[0], c[1], c[2])),
                out_of_gamut: is_out_of_cmyk_gamut(c[0], c[1], c[2]),
            })
            .collect();
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
            palette_proof,
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
            trace_fingerprint: out.trace_fingerprint.clone(),
            edits_stale: out.edits_stale,
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

/// Open an .svg as vectors (no tracing): path edits and node edits are applied, the original is rendered for the compare view.
async fn process_svg(last: &State<'_, Last>, path: String, params: UiParams) -> Result<Processed, String> {
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(Processed, Doc), String> {
        let t0 = Instant::now();
        let p = to_core_params(&params, None);
        let eo = export_options_for_production(&p.production);
        let import = load_svg_doc(Path::new(&path)).map_err(|e| e.to_string())?;
        let (w, h) = (import.doc.width, import.doc.height);
        let out = trasea_core::vectorize_svg(import, &p, &eo);
        let original_svg = std::fs::read_to_string(&path).unwrap_or_else(|_| out.svg.clone());
        let original_img = trasea_core::stats::render_svg(&original_svg, w, h)
            .or_else(|_| trasea_core::stats::render_svg(&out.svg, w, h))
            .map_err(|e| e.to_string())?;
        let original = png_data_url(&original_img)?;
        let palette_proof: Vec<PaletteProofItem> = out
            .palette
            .iter()
            .map(|&c| PaletteProofItem {
                hex: hex(c),
                proof_hex: hex(rgb_to_soft_proof_rgb(c[0], c[1], c[2])),
                out_of_gamut: is_out_of_cmyk_gamut(c[0], c[1], c[2]),
            })
            .collect();
        let processed = Processed {
            svg: out.svg.clone(),
            original,
            width: w,
            height: h,
            paths: out.stats.paths,
            nodes: out.stats.nodes,
            bytes: out.stats.bytes,
            mae: out.stats.mae,
            notes: out.notes.clone(),
            palette: out.palette.iter().map(|c| hex(*c)).collect(),
            palette_proof,
            used_ai: false,
            secs: t0.elapsed().as_secs_f64(),
            thin_pixels: 0,
            primitives: out.stats.primitives,
            symmetry_applied: None,
            text_regions: Vec::new(),
            heatmap: out.heatmap.clone(),
            effective_smooth: p.smooth * 100.0,
            budget_met: None,
            reused: false,
            trace_fingerprint: out.trace_fingerprint.clone(),
            edits_stale: out.edits_stale,
        };
        Ok((processed, out.doc))
    })
    .await
    .map_err(|e| format!("the SVG import stopped unexpectedly: {e}"))??;
    *last.doc.lock().map_err(|_| "internal error".to_string())? = Some(result.1);
    // there is no raster to sample colours from
    *last.prepared.lock().map_err(|_| "internal error".to_string())? = None;
    Ok(result.0)
}

/// Anchors and Bezier handles of one path of the last result (source pixels), for the Node tool overlay.
#[tauri::command]
fn inspect_path_nodes(last: State<'_, Last>, target_id: String) -> Result<Option<PathNodeGeometry>, String> {
    let guard = last.doc.lock().map_err(|_| "internal error".to_string())?;
    Ok(guard.as_ref().and_then(|doc| core_inspect_path_nodes(doc, &target_id)))
}

#[tauri::command]
async fn load_raw_image_preview(path: String) -> Result<RawImagePreview, String> {
    tauri::async_runtime::spawn_blocking(move || -> Result<RawImagePreview, String> {
        let img = load::load_path(Path::new(&path), load::DEFAULT_MAX_SIDE).map_err(|e| e.to_string())?;
        let (width, height) = img.dimensions();
        let data_url = png_data_url(&img)?;
        Ok(RawImagePreview { data_url, width, height })
    })
    .await
    .map_err(|e| format!("Gagal memuat pratinjau gambar: {e}"))?
}

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

/// Render a fast preview SVG from the last vectorized `Doc` with current export/contour options
/// (e.g. live `CutContour` preview without re-running vectorization).
#[tauri::command]
fn render_preview_svg(last: State<'_, Last>, options: UiExport) -> Result<String, String> {
    let doc = last
        .doc
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or("Belum ada hasil vektor untuk dipratinjau.")?;
    let opts = to_core_export(&options);
    Ok(to_svg(&doc, &opts))
}

/// Write arbitrary UTF-8 text (e.g. `.gpl`, `.json`, or `.css` palette export).
#[tauri::command]
fn save_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content.into_bytes()).map_err(|e| format!("Gagal menulis {path}: {e}"))
}

/// Write the last result. `kind` is "svg", "pdf", "eps", "dxf", or "ase".
/// Per QT-1=A, all 1.1.0 & 1.2.0 features (DXF, CMYK PDF/ASE, Die-Cut Contour, Named Spot, Centerline) are open to everyone (Trasea is free; there is no license system).
#[tauri::command]
fn save_file(last: State<'_, Last>, kind: String, path: String, options: UiExport) -> Result<(), String> {
    let doc = last
        .doc
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or("Belum ada hasil untuk diekspor. Konversi gambar dulu.")?;
    let opts = to_core_export(&options);
    let write = |bytes: Vec<u8>| std::fs::write(&path, bytes).map_err(|e| format!("Gagal menulis {path}: {e}"));
    match kind.as_str() {
        "svg" => write(to_svg(&doc, &opts).into_bytes()),
        "pdf" => write(to_pdf(&doc, &opts).map_err(|e| e.to_string())?),
        "eps" => write(to_eps(&doc, &opts).into_bytes()),
        "dxf" => write(to_dxf(&doc, &opts).into_bytes()),
        "ase" => {
            let cut_spot = opts.die_cut_contour.then_some(opts.die_cut_spot_name.as_str());
            write(to_ase_with_spots(&palette_of(&doc), opts.color_space, &opts.spot_colors, cut_spot))
        }
        _ => Err("unknown export format".into()),
    }
}

/// Dedicated DXF export command (open without Pro key per QT-1=B).
#[tauri::command]
fn export_dxf(last: State<'_, Last>, path: String, options: UiExport) -> Result<(), String> {
    let doc = last
        .doc
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or("Belum ada hasil untuk diekspor. Konversi gambar dulu.")?;
    let opts = to_core_export(&options);
    let dxf = to_dxf(&doc, &opts);
    std::fs::write(&path, dxf.into_bytes()).map_err(|e| format!("Gagal menulis {path}: {e}"))
}

/// Paste a raster image from the system clipboard into a local cache PNG and return its path (open, no Pro key).
#[tauri::command]
fn paste_clipboard_image(app: AppHandle) -> Result<String, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Gagal mengakses clipboard sistem: {e}"))?;
    let img_data = cb
        .get_image()
        .map_err(|_| "Clipboard tidak berisi gambar. Salin gambar atau tangkapan layar terlebih dahulu.".to_string())?;
    let w = img_data.width as u32;
    let h = img_data.height as u32;
    if w == 0 || h == 0 {
        return Err("Gambar di clipboard kosong.".into());
    }
    let rgba = image::RgbaImage::from_raw(w, h, img_data.bytes.into_owned())
        .ok_or_else(|| "Format piksel gambar clipboard tidak valid.".to_string())?;
    let dir = cache_dir(&app)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let dest = dir.join(format!("clipboard-{ts}.png"));
    rgba.save(&dest)
        .map_err(|e| format!("Gagal menyimpan gambar clipboard ke cache: {e}"))?;

    // Best-effort cleanup of older clipboard PNGs (keep the newest 20).
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut old: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("clipboard-") && n.ends_with(".png"))
                    .unwrap_or(false)
            })
            .collect();
        old.sort();
        if old.len() > 20 {
            for p in &old[..old.len() - 20] {
                let _ = std::fs::remove_file(p);
            }
        }
    }

    Ok(plain_path(&dest).to_string_lossy().to_string())
}

/// Copy clean SVG markup of the last vectorized document to the system clipboard (open, no Pro key).
#[tauri::command]
fn copy_svg_to_clipboard(last: State<'_, Last>, options: UiExport) -> Result<usize, String> {
    let doc = last
        .doc
        .lock()
        .map_err(|_| "internal error".to_string())?
        .clone()
        .ok_or("Belum ada hasil vektor untuk disalin. Konversi gambar dulu.")?;
    let opts = to_core_export(&options);
    let svg = to_svg(&doc, &opts);
    let len = svg.len();
    let mut cb = arboard::Clipboard::new().map_err(|e| format!("Gagal mengakses clipboard sistem: {e}"))?;
    cb.set_text(svg)
        .map_err(|e| format!("Gagal menyalin SVG ke clipboard: {e}"))?;
    Ok(len)
}

/// Export 100%-black per-colour separation films (supports RGB and pure CMYK K=100% PDFs).
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
    if write_svg {
        for film in &films {
            let p = out_dir.join(format!("{stem}_{}.svg", film.suffix));
            std::fs::write(&p, &film.svg).map_err(|e| format!("Gagal menulis {}: {e}", p.display()))?;
            written.push(p.to_string_lossy().to_string());
        }
    }
    if write_pdf {
        let pdfs = to_separation_pdfs(&doc, &opts).map_err(|e| e.to_string())?;
        for (film, bytes) in pdfs {
            let p = out_dir.join(format!("{stem}_{}.pdf", film.suffix));
            std::fs::write(&p, bytes).map_err(|e| format!("Gagal menulis {}: {e}", p.display()))?;
            written.push(p.to_string_lossy().to_string());
        }
    }
    Ok(written)
}

#[tauri::command]
fn get_app_settings(app: AppHandle) -> AppSettings {
    let Ok(p) = settings_file(&app) else {
        return AppSettings::default();
    };
    let Ok(raw) = std::fs::read_to_string(p) else {
        return AppSettings::default();
    };
    let mut s: AppSettings = serde_json::from_str(&raw).unwrap_or_default();
    sanitize_settings(&mut s);
    s
}

#[tauri::command]
fn save_app_settings(app: AppHandle, mut settings: AppSettings) -> Result<AppSettings, String> {
    sanitize_settings(&mut settings);
    let p = settings_file(&app)?;
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("Gagal menyimpan pengaturan aplikasi: {e}"))?;
    Ok(settings)
}

#[tauri::command]
fn get_settings_path(app: AppHandle) -> Result<String, String> {
    let p = settings_file(&app)?;
    Ok(plain_path(&p).to_string_lossy().to_string())
}

#[tauri::command]
fn list_saved_palettes(app: AppHandle) -> Result<Vec<SavedPalette>, String> {
    let p = palettes_file(&app)?;
    if !p.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&p).map_err(|e| format!("Gagal membaca palettes.json: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Format palettes.json rusak: {e}"))
}

#[tauri::command]
fn save_palette_library_item(app: AppHandle, palette: SavedPalette) -> Result<Vec<SavedPalette>, String> {
    let name = palette.name.trim();
    if name.is_empty() {
        return Err("Nama palet tidak boleh kosong.".into());
    }
    if palette.colors.is_empty() {
        return Err("Palet harus memiliki minimal 1 warna.".into());
    }
    let mut list = list_saved_palettes(app.clone()).unwrap_or_default();
    let item = SavedPalette {
        id: if palette.id.trim().is_empty() {
            format!(
                "pal-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0)
            )
        } else {
            palette.id
        },
        name: name.to_string(),
        colors: palette.colors,
    };
    if let Some(pos) = list.iter().position(|x| x.id == item.id || x.name.eq_ignore_ascii_case(name)) {
        list[pos] = item;
    } else {
        list.push(item);
    }
    let p = palettes_file(&app)?;
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("Gagal menyimpan palet: {e}"))?;
    Ok(list)
}

#[tauri::command]
fn delete_saved_palette(app: AppHandle, id: String) -> Result<Vec<SavedPalette>, String> {
    let mut list = list_saved_palettes(app.clone()).unwrap_or_default();
    list.retain(|x| x.id != id);
    let p = palettes_file(&app)?;
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("Gagal memperbarui palettes.json: {e}"))?;
    Ok(list)
}

#[tauri::command]
fn save_project(path: String, source_path: String, options: ProjectOptions, export_options: ProjectExportOptions) -> Result<(), String> {
    let sha = if !source_path.is_empty() && Path::new(&source_path).is_file() {
        sha256_file(Path::new(&source_path)).unwrap_or_default()
    } else {
        String::new()
    };
    let proj = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        source_path,
        source_sha256: sha,
        options,
        export_options,
    };
    core_save_project(Path::new(&path), &proj).map_err(|e| e.to_string())
}

#[tauri::command]
fn load_project(path: String) -> Result<LoadedProject, String> {
    core_load_project(Path::new(&path)).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_presets(app: AppHandle) -> Result<Vec<UserPreset>, String> {
    let p = presets_file(&app)?;
    if !p.is_file() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&p).map_err(|e| format!("Gagal membaca presets.json: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("Format presets.json rusak: {e}"))
}

#[tauri::command]
fn save_preset(app: AppHandle, preset: UserPreset) -> Result<Vec<UserPreset>, String> {
    let name = preset.name.trim();
    if name.is_empty() {
        return Err("Nama preset tidak boleh kosong.".into());
    }
    let mut list = list_presets(app.clone()).unwrap_or_default();
    if let Some(pos) = list.iter().position(|x| x.id == preset.id || x.name.eq_ignore_ascii_case(name)) {
        list[pos] = UserPreset { name: name.to_string(), ..preset };
    } else {
        list.push(UserPreset { name: name.to_string(), ..preset });
    }
    let p = presets_file(&app)?;
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("Gagal menyimpan preset: {e}"))?;
    Ok(list)
}

#[tauri::command]
fn delete_preset(app: AppHandle, id: String) -> Result<Vec<UserPreset>, String> {
    let mut list = list_presets(app.clone()).unwrap_or_default();
    list.retain(|x| x.id != id);
    let p = presets_file(&app)?;
    let json = serde_json::to_string_pretty(&list).map_err(|e| e.to_string())?;
    std::fs::write(&p, json).map_err(|e| format!("Gagal memperbarui preset: {e}"))?;
    Ok(list)
}

#[tauri::command]
fn sample_image_path(app: AppHandle) -> Result<String, String> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("sample").join("sample-illustration.png"));
    }
    candidates.push(PathBuf::from("src-tauri/resources/sample/sample-illustration.png"));
    candidates.push(PathBuf::from("resources/sample/sample-illustration.png"));
    let found = candidates
        .into_iter()
        .find(|p| p.is_file())
        .ok_or_else(|| "Gambar contoh bawaan tidak ditemukan.".to_string())?;
    let abs = std::fs::canonicalize(&found).unwrap_or(found);
    Ok(plain_path(&abs).to_string_lossy().to_string())
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
        } else if p.is_file() && p.extension().and_then(|e| e.to_str()).map_or(false, |e| e.eq_ignore_ascii_case("svg")) {
            // an .svg opens as vectors in the editor (it is not a batch input)
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
        let eo = merge_batch_export(&p, &config.export);
        let cfg = BatchConfig {
            inputs: config.inputs.into_iter().map(PathBuf::from).collect(),
            output_dir: PathBuf::from(config.output_dir),
            params: p,
            export: eo.clone(),
            write_pdf: config.write_pdf,
            write_eps: config.write_eps,
            write_dxf: config.write_dxf,
            write_ase: config.write_ase,
            write_separations: config.write_separations,
            write_report_csv: true,
            write_report_md: true,
            color_space: eo.color_space,
            dxf_unit: eo.dxf_unit,
            dxf_target_width_mm: eo.dxf_target_width_mm,
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

/// Start Watch Folder background polling (open without Pro key per QT-1=B).
#[tauri::command]
fn start_watch_folder(app: AppHandle, watch_ctrl: State<'_, WatchControl>, config: UiWatchConfig) -> Result<(), String> {
    let in_path = PathBuf::from(&config.in_dir);
    let out_path = PathBuf::from(&config.out_dir);
    if !in_path.is_dir() {
        return Err(format!("Folder pantauan tidak ditemukan: {}", config.in_dir));
    }
    if config.out_dir.trim().is_empty() {
        return Err("Folder output belum dipilih.".into());
    }
    if watch_ctrl.running.swap(true, Ordering::SeqCst) {
        return Err("Pemantauan folder lain sedang berjalan.".into());
    }
    let stop = Arc::new(AtomicBool::new(false));
    {
        let mut slot = watch_ctrl.stop.lock().map_err(|_| "internal error".to_string())?;
        *slot = Some(stop.clone());
    }
    let running_flag = watch_ctrl.running.clone();
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();

    let p = to_core_params(&config.params, sidecar);
    let eo = merge_batch_export(&p, &config.export);
    let batch = BatchConfig {
        inputs: Vec::new(),
        output_dir: out_path.clone(),
        params: p,
        export: eo.clone(),
        write_pdf: config.write_pdf,
        write_eps: config.write_eps,
        write_dxf: config.write_dxf,
        write_ase: config.write_ase,
        write_separations: config.write_separations,
        write_report_csv: true,
        write_report_md: true,
        color_space: eo.color_space,
        dxf_unit: eo.dxf_unit,
        dxf_target_width_mm: eo.dxf_target_width_mm,
        max_workers: None,
    };
    let mut wcfg = WatchConfig::new(in_path, out_path, batch);
    wcfg.include_subfolders = config.include_subfolders;
    if let Some(ms) = config.poll_ms {
        wcfg.poll_ms = ms.max(100);
    }
    if let Some(ms) = config.debounce_ms {
        wcfg.debounce_ms = ms;
    }

    std::thread::spawn(move || {
        let res = watch_folder(&wcfg, stop, |ev| {
            let _ = handle.emit("watch-event", ev);
        });
        running_flag.store(false, Ordering::SeqCst);
        let error = res.err().map(|e| e.to_string());
        let _ = handle.emit("watch-stopped", WatchStoppedEvent { error });
    });

    Ok(())
}

#[tauri::command]
fn stop_watch_folder(watch_ctrl: State<'_, WatchControl>) -> Result<(), String> {
    let slot = watch_ctrl.stop.lock().map_err(|_| "internal error".to_string())?;
    if let Some(flag) = slot.as_ref() {
        flag.store(true, Ordering::Relaxed);
    }
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
async fn check_for_update() -> Result<updater::UpdateInfo, String> {
    tauri::async_runtime::spawn_blocking(updater::check_latest_release)
        .await
        .map_err(|e| format!("Gagal memeriksa pembaruan: {e}"))?
}

#[tauri::command]
fn get_app_mode() -> updater::AppModeInfo {
    updater::AppModeInfo {
        is_portable: updater::is_portable_mode(),
        current_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}

#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    updater::open_download_url(&url)
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
        .manage(WatchControl::default())
        .invoke_handler(tauri::generate_handler![
            process,
            render_preview_svg,
            load_raw_image_preview,
            pick_color,
            inspect_path_nodes,
            save_file,
            save_text_file,
            export_dxf,
            paste_clipboard_image,
            copy_svg_to_clipboard,
            save_separations,
            get_app_settings,
            save_app_settings,
            get_settings_path,
            list_saved_palettes,
            save_palette_library_item,
            delete_saved_palette,
            save_project,
            load_project,
            list_presets,
            save_preset,
            delete_preset,
            sample_image_path,
            scan_folder,
            collect_dropped_images,
            run_batch,
            cancel_batch,
            start_watch_folder,
            stop_watch_folder,
            open_folder,
            sidecar_available,
            third_party_licenses,
            check_for_update,
            get_app_mode,
            open_external_url
        ])
        .run(tauri::generate_context!())
        .expect("error while running Trasea");
}
