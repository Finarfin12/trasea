// Vektora desktop app: a thin Tauri shell around `vektora-core`. Everything runs on this computer; the app makes no network request.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager, State};
use vektora_core::export::{palette_of, to_ase, to_svg, Doc, ExportOptions, LinesAs};
use vektora_core::{export, hex, load, vectorize, Params, SymmetryMode, TextRegion};

#[derive(Default)]
struct Last {
    doc: Mutex<Option<Doc>>,
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
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    stage: String,
    fraction: f32,
}

/// Folder with the Real-ESRGAN sidecar: bundled next to the app, or VEKTORA_SIDECAR for development.
fn sidecar_dir(app: &AppHandle) -> Option<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(res) = app.path().resource_dir() {
        candidates.push(res.join("realesrgan"));
    }
    if let Ok(dev) = std::env::var("VEKTORA_SIDECAR") {
        candidates.push(PathBuf::from(dev));
    }
    candidates.into_iter().find(|d| d.join(vektora_core::clean::SIDECAR_EXE).is_file())
}

fn png_data_url(img: &image::RgbImage) -> Result<String, String> {
    let mut buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).map_err(|e| e.to_string())?;
    Ok(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}

#[tauri::command]
async fn process(app: AppHandle, last: State<'_, Last>, path: String, params: UiParams) -> Result<Processed, String> {
    let sidecar = sidecar_dir(&app);
    let handle = app.clone();
    let result = tauri::async_runtime::spawn_blocking(move || -> Result<(Processed, Doc), String> {
        let t0 = Instant::now();
        let img = load::load_path(Path::new(&path), load::DEFAULT_MAX_SIDE).map_err(|e| e.to_string())?;
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
        };
        let out = vectorize(&img, &p, &|stage, fraction| {
            let _ = handle.emit("progress", ProgressEvent { stage: stage.to_string(), fraction });
        })
        .map_err(|e| e.to_string())?;
        let processed = Processed {
            svg: out.svg.clone(),
            original: png_data_url(&img)?,
            width: img.width(),
            height: img.height(),
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
        };
        Ok((processed, out.doc))
    })
    .await
    .map_err(|e| format!("the conversion stopped unexpectedly: {e}"))??;
    *last.doc.lock().map_err(|_| "internal error".to_string())? = Some(result.1);
    Ok(result.0)
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
        .invoke_handler(tauri::generate_handler![process, save_file, sidecar_available, third_party_licenses])
        .run(tauri::generate_context!())
        .expect("error while running Vektora");
}
