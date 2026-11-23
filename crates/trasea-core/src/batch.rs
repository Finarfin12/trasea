//! Reusable batch folder engine and report writers (CSV + Markdown) for CLI, Watch Folder, and Tauri UI.

use crate::color::ColorSpace;
use crate::dxf::DxfUnit;
use crate::export::{
    export_options_for_production, palette_of, to_dxf, to_eps, to_pdf,
    to_separation_eps, to_separation_pdfs, to_separation_svgs, to_svg, ExportOptions,
};
use crate::load;
use crate::pipeline::{vectorize, Params};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub const SUPPORTED_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "bmp"];

pub fn is_supported_image(path: &Path) -> bool {
    path.is_file()
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| SUPPORTED_EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
            .unwrap_or(false)
}

/// Scan `dir` for supported image files. When `include_subfolders` is true, also scans immediate
/// child directories (1 level deep). Non-image files are ignored without error.
pub fn scan_image_files(dir: &Path, include_subfolders: bool) -> Result<Vec<PathBuf>> {
    if !dir.is_dir() {
        return Err(Error::Other(format!("Folder tidak ditemukan: {}", dir.display())));
    }
    let mut files = Vec::new();
    let entries = std::fs::read_dir(dir).map_err(|e| Error::Other(format!("Gagal membaca folder {}: {e}", dir.display())))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if is_supported_image(&path) {
            files.push(path);
        } else if include_subfolders && path.is_dir() {
            if let Ok(sub) = std::fs::read_dir(&path) {
                for se in sub.flatten() {
                    let sp = se.path();
                    if is_supported_image(&sp) {
                        files.push(sp);
                    }
                }
            }
        }
    }
    files.sort();
    Ok(files)
}

#[derive(Debug, Clone)]
pub struct BatchConfig {
    pub inputs: Vec<PathBuf>,
    pub output_dir: PathBuf,
    pub params: Params,
    pub export: ExportOptions,
    pub write_pdf: bool,
    pub write_eps: bool,
    pub write_dxf: bool,
    pub write_ase: bool,
    pub write_separations: bool,
    pub write_report_csv: bool,
    pub write_report_md: bool,
    pub color_space: ColorSpace,
    pub dxf_unit: DxfUnit,
    pub dxf_target_width_mm: Option<f32>,
    /// Optional worker cap (None = 1 when ai_clean else min(cpu/2, 4)).
    pub max_workers: Option<usize>,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            inputs: Vec::new(),
            output_dir: PathBuf::new(),
            params: Params::default(),
            export: ExportOptions::default(),
            write_pdf: false,
            write_eps: false,
            write_dxf: false,
            write_ase: false,
            write_separations: false,
            write_report_csv: true,
            write_report_md: true,
            color_space: ColorSpace::Rgb,
            dxf_unit: DxfUnit::Mm,
            dxf_target_width_mm: None,
            max_workers: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BatchStatus {
    Ok,
    Failed,
    Cancelled,
}

impl BatchStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ok => "sukses",
            Self::Failed => "gagal",
            Self::Cancelled => "dibatalkan",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchItemReport {
    pub input_path: PathBuf,
    pub file_name: String,
    pub status: BatchStatus,
    pub error: Option<String>,
    pub svg_path: Option<PathBuf>,
    pub colors: usize,
    pub paths: usize,
    pub nodes: usize,
    pub mae: Option<f64>,
    pub warnings: Vec<String>,
    pub secs: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchProgress {
    pub done: usize,
    pub total: usize,
    pub current_file: String,
    pub stage: String,
    pub item_report: Option<BatchItemReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchSummary {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub total_secs: f64,
    pub items: Vec<BatchItemReport>,
    pub report_csv_path: Option<PathBuf>,
    pub report_md_path: Option<PathBuf>,
}

/// Assign collision-free output stems (`logo`, `logo_2`, `logo_3`, ...) preserving input order.
fn assign_unique_stems(inputs: &[PathBuf]) -> Vec<String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    inputs
        .iter()
        .map(|p| {
            let raw = p
                .file_stem()
                .and_then(|s| s.to_str())
                .filter(|s| !s.is_empty())
                .unwrap_or("gambar")
                .to_string();
            let key = raw.to_ascii_lowercase();
            let count = seen.entry(key).or_insert(0);
            *count += 1;
            if *count == 1 {
                raw
            } else {
                format!("{raw}_{}", *count)
            }
        })
        .collect()
}

/// Resolve effective export options for a batch/watch run, combining production-mode defaults
/// with explicit caller overrides (`config.export` and `config.color_space` / `dxf_unit` / `dxf_target_width_mm`).
pub fn resolve_effective_export(config: &BatchConfig) -> ExportOptions {
    let mut eff_export = config.export.clone();
    if config.color_space != ColorSpace::Rgb {
        eff_export.color_space = config.color_space;
    }
    if config.dxf_unit != DxfUnit::Mm {
        eff_export.dxf_unit = config.dxf_unit;
    }
    if config.dxf_target_width_mm.is_some() {
        eff_export.dxf_target_width_mm = config.dxf_target_width_mm;
    }
    let prod_exp = export_options_for_production(&config.params.production);
    match config.params.production.mode {
        crate::production::ProductionMode::CutEmbroidery => {
            eff_export.lines_as = crate::export::LinesAs::Shape;
            eff_export.trap_pt = 0.0;
            eff_export.registration_marks = false;
            eff_export.strip_background = config.params.production.strip_background;
        }
        crate::production::ProductionMode::ScreenPrint => {
            eff_export.registration_marks = config.params.production.registration_marks;
            if eff_export.trap_pt <= 0.0 && !config.params.production.sablon_knockout {
                eff_export.trap_pt = prod_exp.trap_pt;
            }
        }
        crate::production::ProductionMode::Standard | crate::production::ProductionMode::Shading => {}
        crate::production::ProductionMode::Centerline => {
            eff_export.lines_as = crate::export::LinesAs::Stroke;
            eff_export.strip_background = true;
            eff_export.trap_pt = 0.0;
        }
    }
    eff_export
}

/// Process a single image file according to `cfg` and `eff_export`. Reused by both `run_batch` and `watch_folder`.
pub fn process_single_file(input: &Path, stem: &str, cfg: &BatchConfig, eff_export: &ExportOptions) -> BatchItemReport {
    let file_name = input
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("gambar")
        .to_string();
    let t0 = Instant::now();

    let img = match load::load_path(input, load::DEFAULT_MAX_SIDE) {
        Ok(im) => im,
        Err(e) => {
            return BatchItemReport {
                input_path: input.to_path_buf(),
                file_name,
                status: BatchStatus::Failed,
                error: Some(e.to_string()),
                svg_path: None,
                colors: 0,
                paths: 0,
                nodes: 0,
                mae: None,
                warnings: vec![],
                secs: t0.elapsed().as_secs_f64(),
            };
        }
    };

    let out = match vectorize(&img, &cfg.params, &|_, _| {}) {
        Ok(o) => o,
        Err(e) => {
            return BatchItemReport {
                input_path: input.to_path_buf(),
                file_name,
                status: BatchStatus::Failed,
                error: Some(e.to_string()),
                svg_path: None,
                colors: 0,
                paths: 0,
                nodes: 0,
                mae: None,
                warnings: vec![],
                secs: t0.elapsed().as_secs_f64(),
            };
        }
    };

    let svg = to_svg(&out.doc, eff_export);
    let svg_path = cfg.output_dir.join(format!("{stem}.svg"));
    if let Err(e) = std::fs::write(&svg_path, &svg) {
        return BatchItemReport {
            input_path: input.to_path_buf(),
            file_name,
            status: BatchStatus::Failed,
            error: Some(format!("Gagal menulis SVG: {e}")),
            svg_path: None,
            colors: out.palette.len(),
            paths: out.stats.paths,
            nodes: out.stats.nodes,
            mae: out.stats.mae,
            warnings: vec![],
            secs: t0.elapsed().as_secs_f64(),
        };
    }

    let mut warnings = Vec::new();
    if !out.text_regions.is_empty() {
        warnings.push(format!("{} area teks", out.text_regions.len()));
    }
    for n in &out.notes {
        warnings.push(n.clone());
    }

    if cfg.write_pdf {
        match to_pdf(&out.doc, eff_export) {
            Ok(bytes) => {
                let pdf_path = cfg.output_dir.join(format!("{stem}.pdf"));
                if let Err(e) = std::fs::write(&pdf_path, bytes) {
                    warnings.push(format!("Gagal tulis PDF: {e}"));
                }
            }
            Err(e) => warnings.push(format!("PDF dilewati: {e}")),
        }
    }

    if cfg.write_eps {
        let eps_path = cfg.output_dir.join(format!("{stem}.eps"));
        if let Err(e) = std::fs::write(&eps_path, to_eps(&out.doc, eff_export)) {
            warnings.push(format!("Gagal tulis EPS: {e}"));
        }
    }

    if cfg.write_dxf {
        let dxf_path = cfg.output_dir.join(format!("{stem}.dxf"));
        if let Err(e) = std::fs::write(&dxf_path, to_dxf(&out.doc, eff_export)) {
            warnings.push(format!("Gagal tulis DXF: {e}"));
        }
    }

    if cfg.write_ase {
        let ase_path = cfg.output_dir.join(format!("{stem}.ase"));
        let cut_spot = eff_export
            .die_cut_contour
            .then_some(eff_export.die_cut_spot_name.as_str());
        let ase_bytes = crate::export::to_ase_with_spots(
            &palette_of(&out.doc),
            eff_export.color_space,
            &eff_export.spot_colors,
            cut_spot,
        );
        if let Err(e) = std::fs::write(&ase_path, ase_bytes) {
            warnings.push(format!("Gagal tulis ASE: {e}"));
        }
    }

    if cfg.write_separations {
        let films = to_separation_svgs(&out.doc, eff_export);
        for film in &films {
            let f_svg = cfg.output_dir.join(format!("{stem}_{}.svg", film.suffix));
            if let Err(e) = std::fs::write(&f_svg, &film.svg) {
                warnings.push(format!("Gagal tulis film {}: {e}", film.suffix));
            }
        }
        if cfg.write_pdf {
            match to_separation_pdfs(&out.doc, eff_export) {
                Ok(pdfs) => {
                    for (film, bytes) in pdfs {
                        let f_pdf = cfg.output_dir.join(format!("{stem}_{}.pdf", film.suffix));
                        if let Err(e) = std::fs::write(&f_pdf, bytes) {
                            warnings.push(format!("Gagal tulis PDF film {}: {e}", film.suffix));
                        }
                    }
                }
                Err(e) => warnings.push(format!("PDF film dilewati: {e}")),
            }
        }
        if cfg.write_eps {
            for (film, eps) in to_separation_eps(&out.doc, eff_export) {
                let f_eps = cfg.output_dir.join(format!("{stem}_{}.eps", film.suffix));
                if let Err(e) = std::fs::write(&f_eps, eps) {
                    warnings.push(format!("Gagal tulis EPS film {}: {e}", film.suffix));
                }
            }
        }
    }

    let (paths, nodes) = crate::stats::count_svg(&svg);
    BatchItemReport {
        input_path: input.to_path_buf(),
        file_name,
        status: BatchStatus::Ok,
        error: None,
        svg_path: Some(svg_path),
        colors: palette_of(&out.doc).len(),
        paths,
        nodes,
        mae: out.stats.mae,
        warnings,
        secs: t0.elapsed().as_secs_f64(),
    }
}

/// Run batch conversion over `config.inputs`.
/// - Sequential when `config.params.ai_clean` is true (avoids GPU VRAM contention).
/// - Bounded parallel (`min(cpu/2, 4)`) when `ai_clean` is false.
/// - Safe cancellation via `cancel` (checked before each file; remaining items marked `Cancelled`).
/// - Per-file error isolation (broken files do not abort the batch).
pub fn run_batch<F>(config: &BatchConfig, cancel: Arc<AtomicBool>, on_progress: F) -> Result<BatchSummary>
where
    F: Fn(BatchProgress) + Sync,
{
    std::fs::create_dir_all(&config.output_dir)
        .map_err(|e| Error::Other(format!("Gagal membuat folder output {}: {e}", config.output_dir.display())))?;

    let t0 = Instant::now();
    let total = config.inputs.len();
    let stems = assign_unique_stems(&config.inputs);
    let eff_export = resolve_effective_export(config);

    let auto_workers = if config.params.ai_clean {
        1
    } else {
        std::thread::available_parallelism()
            .map(|n| (n.get() / 2).clamp(1, 4))
            .unwrap_or(1)
    };
    let workers = config
        .max_workers
        .map(|w| w.max(1))
        .unwrap_or(auto_workers)
        .min(total.max(1));

    let results: Mutex<Vec<Option<BatchItemReport>>> = Mutex::new(vec![None; total]);
    let next_idx = AtomicUsize::new(0);
    let done_count = AtomicUsize::new(0);
    let progress_lock = Mutex::new(());

    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| {
                loop {
                    let i = next_idx.fetch_add(1, Ordering::SeqCst);
                    if i >= total {
                        break;
                    }
                    let input = &config.inputs[i];
                    let file_name = input
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("gambar")
                        .to_string();

                    if cancel.load(Ordering::Relaxed) {
                        let rep = BatchItemReport {
                            input_path: input.clone(),
                            file_name: file_name.clone(),
                            status: BatchStatus::Cancelled,
                            error: None,
                            svg_path: None,
                            colors: 0,
                            paths: 0,
                            nodes: 0,
                            mae: None,
                            warnings: vec![],
                            secs: 0.0,
                        };
                        results.lock().unwrap()[i] = Some(rep.clone());
                        let done = done_count.fetch_add(1, Ordering::SeqCst) + 1;
                        let _g = progress_lock.lock().unwrap();
                        on_progress(BatchProgress {
                            done,
                            total,
                            current_file: file_name,
                            stage: "Dibatalkan".to_string(),
                            item_report: Some(rep),
                        });
                        continue;
                    }

                    {
                        let _g = progress_lock.lock().unwrap();
                        on_progress(BatchProgress {
                            done: done_count.load(Ordering::SeqCst),
                            total,
                            current_file: file_name.clone(),
                            stage: "Memproses".to_string(),
                            item_report: None,
                        });
                    }

                    let rep = process_single_file(input, &stems[i], config, &eff_export);
                    results.lock().unwrap()[i] = Some(rep.clone());
                    let done = done_count.fetch_add(1, Ordering::SeqCst) + 1;
                    {
                        let _g = progress_lock.lock().unwrap();
                        on_progress(BatchProgress {
                            done,
                            total,
                            current_file: file_name,
                            stage: match rep.status {
                                BatchStatus::Ok => "Selesai".to_string(),
                                BatchStatus::Failed => "Gagal".to_string(),
                                BatchStatus::Cancelled => "Dibatalkan".to_string(),
                            },
                            item_report: Some(rep),
                        });
                    }
                }
            });
        }
    });

    let items: Vec<BatchItemReport> = results
        .into_inner()
        .unwrap()
        .into_iter()
        .flatten()
        .collect();
    let succeeded = items.iter().filter(|r| r.status == BatchStatus::Ok).count();
    let failed = items.iter().filter(|r| r.status == BatchStatus::Failed).count();
    let cancelled = items.iter().filter(|r| r.status == BatchStatus::Cancelled).count();
    let total_secs = t0.elapsed().as_secs_f64();

    let mut summary = BatchSummary {
        total,
        succeeded,
        failed,
        cancelled,
        total_secs,
        items,
        report_csv_path: None,
        report_md_path: None,
    };

    if config.write_report_csv {
        let p = config.output_dir.join("laporan-trasea.csv");
        write_report_csv(&summary, &p)?;
        summary.report_csv_path = Some(p);
    }
    if config.write_report_md {
        let p = config.output_dir.join("laporan-trasea.md");
        write_report_md(&summary, &p)?;
        summary.report_md_path = Some(p);
    }

    Ok(summary)
}

fn csv_escape(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

pub fn write_report_csv(summary: &BatchSummary, path: &Path) -> Result<()> {
    let mut out = String::from("Nama file,Status,Warna,Path,Node,MAE,Peringatan,Waktu (dtk),Output,Error\n");
    for it in &summary.items {
        let mae = it.mae.map_or(String::new(), |m| format!("{m:.2}"));
        let warns = it.warnings.join("; ");
        let svg = it
            .svg_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("");
        let err = it.error.as_deref().unwrap_or("");
        let cols = [
            csv_escape(&it.file_name),
            csv_escape(it.status.label()),
            it.colors.to_string(),
            it.paths.to_string(),
            it.nodes.to_string(),
            mae,
            csv_escape(&warns),
            format!("{:.2}", it.secs),
            csv_escape(svg),
            csv_escape(err),
        ];
        out.push_str(&cols.join(","));
        out.push('\n');
    }
    std::fs::write(path, out).map_err(|e| Error::Other(format!("Gagal menulis laporan CSV: {e}")))
}

fn md_escape(s: &str) -> String {
    s.replace('|', "\\|").replace('\n', " ")
}

pub fn write_report_md(summary: &BatchSummary, path: &Path) -> Result<()> {
    let mut out = String::new();
    out.push_str("# Laporan Batch Trasea\n\n");
    out.push_str(&format!(
        "- **Total file:** {}\n- **Sukses:** {}\n- **Gagal:** {}\n- **Dibatalkan:** {}\n- **Total waktu:** {:.1} detik\n\n",
        summary.total, summary.succeeded, summary.failed, summary.cancelled, summary.total_secs
    ));

    let attention: Vec<&BatchItemReport> = summary
        .items
        .iter()
        .filter(|it| it.status == BatchStatus::Failed || !it.warnings.is_empty())
        .collect();
    if !attention.is_empty() {
        out.push_str("## Perlu Ditinjau\n\n");
        for it in attention {
            let reason = if let Some(err) = &it.error {
                format!("Gagal: {err}")
            } else {
                it.warnings.join("; ")
            };
            out.push_str(&format!("- **{}** — {}\n", md_escape(&it.file_name), md_escape(&reason)));
        }
        out.push('\n');
    }

    out.push_str("## Ringkasan per File\n\n");
    out.push_str("| Nama file | Status | Warna | Path | Node | MAE | Peringatan | Waktu (dtk) |\n");
    out.push_str("|---|---|---:|---:|---:|---:|---|---:|\n");
    for it in &summary.items {
        let mae = it.mae.map_or("-".to_string(), |m| format!("{m:.2}"));
        let note = if let Some(err) = &it.error {
            format!("Error: {err}")
        } else if it.warnings.is_empty() {
            "-".to_string()
        } else {
            it.warnings.join("; ")
        };
        out.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {:.1} |\n",
            md_escape(&it.file_name),
            it.status.label(),
            it.colors,
            it.paths,
            it.nodes,
            mae,
            md_escape(&note),
            it.secs
        ));
    }

    std::fs::write(path, out).map_err(|e| Error::Other(format!("Gagal menulis laporan Markdown: {e}")))
}
