//! Trasea CLI: single-file conversion, `--batch <in-dir> <out-dir>`, and `--render` helper.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::time::Instant;
use trasea_core::batch::{run_batch, scan_image_files, BatchConfig, BatchStatus};
use trasea_core::export::{export_options_for_production, palette_of, to_ase, to_eps, to_separation_svgs, to_svg, LinesAs};
use trasea_core::{export, load, vectorize, Params, ProductionMode, ProductionOptions, SymmetryMode};

/// `#RRGGBB` (or `RRGGBB`) to a colour.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

fn usage() {
    eprintln!(
        "usage:\n  trasea-cli <input> <output.svg> [--mode standard|sablon|cutting] [--pdf out.pdf] [--eps out.eps] [--ase out.ase] [--separations DIR] [--colors N] [--smooth 0..1] [--lines stroke|shape] [--trap PT] [--min-stroke PT] [--reg-marks|--no-reg-marks] [--strip-bg|--keep-bg] [--no-lines] [--ai|--no-ai] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--target-nodes N] [--palette \"#RRGGBB,#RRGGBB\"] [--png out.png] [--sidecar DIR]\n  trasea-cli --batch <input-dir> <output-dir> [--mode standard|sablon|cutting] [--subfolders] [--pdf] [--eps] [--ase] [--separations] [--no-report] [options...]"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // dev helper: `trasea-cli --render in.svg out.png WIDTH HEIGHT` draws an SVG to a PNG (for before/after pictures)
    if args.first().map(String::as_str) == Some("--render") && args.len() >= 5 {
        let svg = std::fs::read_to_string(&args[1]).expect("cannot read the SVG");
        let (w, h): (u32, u32) = (args[3].parse().expect("width"), args[4].parse().expect("height"));
        trasea_core::stats::render_svg(&svg, w, h).expect("cannot render the SVG").save(&args[2]).expect("cannot write the PNG");
        return;
    }

    let is_batch = args.first().map(String::as_str) == Some("--batch");
    if (is_batch && args.len() < 3) || (!is_batch && args.len() < 2) {
        usage();
        std::process::exit(2);
    }

    let (input, output, start_i) = if is_batch {
        (PathBuf::from(&args[1]), PathBuf::from(&args[2]), 3usize)
    } else {
        (PathBuf::from(&args[0]), PathBuf::from(&args[1]), 2usize)
    };

    let mut p = Params::default();
    p.ai_clean = false;
    let mut pdf_path: Option<PathBuf> = None;
    let mut eps_path: Option<PathBuf> = None;
    let mut ase_path: Option<PathBuf> = None;
    let mut png_path: Option<PathBuf> = None;
    let mut sep_dir: Option<PathBuf> = None;
    let mut batch_pdf = false;
    let mut batch_eps = false;
    let mut batch_ase = false;
    let mut batch_separations = false;
    let mut batch_subfolders = false;
    let mut batch_report = true;

    let mut mode: Option<ProductionMode> = None;
    let mut colors_set = false;
    let mut smooth_set = false;
    let mut lines_override: Option<LinesAs> = None;
    let mut trap_override: Option<f64> = None;
    let mut min_stroke_override: Option<f64> = None;
    let mut reg_marks_override: Option<bool> = None;
    let mut strip_bg_override: Option<bool> = None;

    let mut i = start_i;
    while i < args.len() {
        match args[i].as_str() {
            "--mode" => {
                i += 1;
                match args.get(i).and_then(|v| ProductionMode::parse(v)) {
                    Some(m) => mode = Some(m),
                    None => {
                        eprintln!("--mode needs one of: standard, sablon, cutting");
                        std::process::exit(2);
                    }
                }
            }
            "--colors" => {
                i += 1;
                p.colors = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(10);
                colors_set = true;
            }
            "--smooth" => {
                i += 1;
                p.smooth = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(0.5);
                smooth_set = true;
            }
            "--pdf" => {
                if is_batch {
                    batch_pdf = true;
                } else {
                    i += 1;
                    pdf_path = args.get(i).map(PathBuf::from);
                }
            }
            "--eps" => {
                if is_batch {
                    batch_eps = true;
                } else {
                    i += 1;
                    eps_path = args.get(i).map(PathBuf::from);
                }
            }
            "--ase" => {
                if is_batch {
                    batch_ase = true;
                } else {
                    i += 1;
                    ase_path = args.get(i).map(PathBuf::from);
                }
            }
            "--separations" => {
                if is_batch {
                    batch_separations = true;
                } else {
                    i += 1;
                    sep_dir = args.get(i).map(PathBuf::from);
                }
            }
            "--subfolders" => batch_subfolders = true,
            "--no-report" => batch_report = false,
            "--lines" => {
                i += 1;
                lines_override = Some(if args.get(i).map(String::as_str) == Some("shape") { LinesAs::Shape } else { LinesAs::Stroke });
            }
            "--trap" => {
                i += 1;
                trap_override = args.get(i).and_then(|v| v.parse().ok());
            }
            "--min-stroke" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<f64>().ok()).filter(|v| *v >= 0.0) {
                    Some(v) => min_stroke_override = Some(v),
                    None => {
                        eprintln!("--min-stroke needs a number >= 0 (points)");
                        std::process::exit(2);
                    }
                }
            }
            "--reg-marks" => reg_marks_override = Some(true),
            "--no-reg-marks" => reg_marks_override = Some(false),
            "--strip-bg" => strip_bg_override = Some(true),
            "--keep-bg" => strip_bg_override = Some(false),
            "--ai" => p.ai_clean = true,
            "--sidecar" => {
                i += 1;
                p.sidecar_dir = args.get(i).map(PathBuf::from);
            }
            "--no-lines" => p.keep_lines = false,
            "--no-ai" => p.ai_clean = false,
            "--no-primitives" => p.detect_primitives = false,
            "--no-text-detect" => p.detect_text = false,
            "--target-nodes" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()).filter(|n| *n > 0) {
                    Some(n) => p.target_nodes = Some(n),
                    None => {
                        eprintln!("--target-nodes needs a whole number above 0");
                        std::process::exit(2);
                    }
                }
            }
            "--palette" => {
                i += 1;
                let parsed: Option<Vec<[u8; 3]>> = args.get(i).and_then(|v| v.split(',').map(|c| parse_hex(c.trim())).collect());
                match parsed.filter(|c| c.len() >= 2) {
                    Some(c) => p.custom_palette = Some(c),
                    None => {
                        eprintln!("--palette needs at least 2 colours like \"#0F3973,#EBD9C1\"");
                        std::process::exit(2);
                    }
                }
            }
            "--symmetry" => {
                i += 1;
                match args.get(i).and_then(|v| SymmetryMode::parse(v)) {
                    Some(m) => p.symmetry = m,
                    None => {
                        eprintln!("--symmetry needs one of: off, auto, vertical, horizontal");
                        std::process::exit(2);
                    }
                }
            }
            "--png" => {
                i += 1;
                png_path = args.get(i).map(PathBuf::from);
            }
            other => {
                eprintln!("unknown option {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // Apply production mode defaults, then explicit CLI overrides.
    p.production = match mode.unwrap_or(ProductionMode::Standard) {
        ProductionMode::Standard => ProductionOptions::standard(),
        ProductionMode::ScreenPrint => {
            if !colors_set {
                p.colors = 4;
            }
            ProductionOptions::screen_print()
        }
        ProductionMode::CutEmbroidery => {
            if !smooth_set {
                p.smooth = 0.7;
            }
            ProductionOptions::cut_embroidery()
        }
    };
    if let Some(ms) = min_stroke_override {
        p.production.min_stroke_pt = ms;
    }
    if let Some(rm) = reg_marks_override {
        p.production.registration_marks = rm;
    }
    if let Some(sb) = strip_bg_override {
        p.production.strip_background = sb;
    }

    let mut eo = export_options_for_production(&p.production);
    if let Some(l) = lines_override {
        if p.production.mode != ProductionMode::CutEmbroidery {
            eo.lines_as = l;
        }
    }
    if let Some(t) = trap_override {
        if p.production.mode != ProductionMode::CutEmbroidery {
            eo.trap_pt = t;
        }
    }

    if is_batch {
        let files = match scan_image_files(&input, batch_subfolders) {
            Ok(f) => f,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };
        if files.is_empty() {
            eprintln!("Tidak ada file gambar yang didukung di {}", input.display());
            std::process::exit(1);
        }
        let cfg = BatchConfig {
            inputs: files,
            output_dir: output.clone(),
            params: p,
            export: eo,
            write_pdf: batch_pdf,
            write_eps: batch_eps,
            write_ase: batch_ase,
            write_separations: batch_separations,
            write_report_csv: batch_report,
            write_report_md: batch_report,
            max_workers: None,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let summary = match run_batch(&cfg, cancel, |prog| {
            if let Some(ref rep) = prog.item_report {
                eprintln!(
                    "[{}/{}] {} -> {} (node {}, {:.1}s)",
                    prog.done,
                    prog.total,
                    rep.file_name,
                    rep.status.label(),
                    rep.nodes,
                    rep.secs
                );
            }
        }) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        };
        println!(
            "batch selesai: total {} | sukses {} | gagal {} | dibatalkan {} | {:.1}s -> {}",
            summary.total,
            summary.succeeded,
            summary.failed,
            summary.cancelled,
            summary.total_secs,
            output.display()
        );
        if summary.items.iter().all(|it| it.status == BatchStatus::Failed) {
            std::process::exit(1);
        }
        return;
    }

    // Single-file mode
    let t0 = Instant::now();
    let img = match load::load_path(&input, load::DEFAULT_MAX_SIDE) {
        Ok(i) => i,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let out = match vectorize(&img, &p, &|stage, f| eprintln!("[{:>3.0}%] {stage}", f * 100.0)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    };
    let svg = to_svg(&out.doc, &eo);
    std::fs::write(&output, &svg).expect("cannot write the SVG");
    if let Some(eps) = eps_path {
        std::fs::write(&eps, to_eps(&out.doc, &eo)).expect("cannot write the EPS");
    }
    if let Some(ase) = ase_path {
        std::fs::write(&ase, to_ase(&palette_of(&out.doc))).expect("cannot write the ASE");
    }
    if let Some(pdf) = &pdf_path {
        match export::svg_to_pdf(&svg) {
            Ok(bytes) => std::fs::write(pdf, bytes).expect("cannot write the PDF"),
            Err(e) => eprintln!("PDF: {e}"),
        }
    }
    if let Some(sdir) = sep_dir {
        std::fs::create_dir_all(&sdir).expect("cannot create separations dir");
        let stem = output.file_stem().and_then(|s| s.to_str()).unwrap_or("gambar");
        let films = to_separation_svgs(&out.doc, &eo);
        for film in &films {
            let f_svg = sdir.join(format!("{stem}_{}.svg", film.suffix));
            std::fs::write(&f_svg, &film.svg).expect("cannot write separation SVG");
            if pdf_path.is_some() {
                if let Ok(bytes) = export::svg_to_pdf(&film.svg) {
                    let f_pdf = sdir.join(format!("{stem}_{}.pdf", film.suffix));
                    let _ = std::fs::write(&f_pdf, bytes);
                }
            }
        }
        eprintln!("separations: {} films written to {}", films.len(), sdir.display());
    }
    if let Some(png) = png_path {
        match trasea_core::stats::render_svg(&svg, img.width(), img.height()) {
            Ok(r) => r.save(&png).expect("cannot write the PNG"),
            Err(e) => eprintln!("PNG: {e}"),
        }
    }
    for n in &out.notes {
        eprintln!("note: {n}");
    }
    if let Some(met) = out.budget_met {
        eprintln!("node budget {}: smoothness used {:.2}", if met { "met" } else { "NOT met" }, out.effective_smooth);
    }
    if std::env::var("VK_DEBUG_PRIMS").is_ok() {
        for l in &out.doc.fields {
            for pr in &l.prims {
                eprintln!("prim in {}: {pr:?}", trasea_core::hex(l.color));
            }
        }
    }
    if let Some(s) = &out.symmetry_applied {
        eprintln!("symmetry applied: {s}");
    }
    if !out.text_regions.is_empty() {
        eprintln!(
            "text areas: {} {:?}",
            out.text_regions.len(),
            out.text_regions.iter().map(|r| (r.x as i64, r.y as i64, r.w as i64, r.h as i64, r.glyph_count)).collect::<Vec<_>>()
        );
    }
    println!(
        "paths {} | nodes {} | primitives {} | {} KB | MAE {} | thin px {} | AI {} | {:.1}s",
        out.stats.paths,
        out.stats.nodes,
        out.stats.primitives,
        out.stats.bytes / 1024,
        out.stats.mae.map_or("-".to_string(), |m| format!("{m:.2}")),
        out.thin_pixels,
        out.used_ai,
        t0.elapsed().as_secs_f32()
    );
}