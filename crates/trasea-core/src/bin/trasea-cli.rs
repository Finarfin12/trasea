//! Trasea CLI: single-file conversion, `--batch <in-dir> <out-dir>`, `--watch <in-dir> --out <out-dir>`, and `--render` helper.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use trasea_core::batch::{run_batch, scan_image_files, BatchConfig, BatchStatus};
use trasea_core::color::ColorSpace;
use trasea_core::dxf::DxfUnit;
use trasea_core::export::{
    export_options_for_production, palette_of, to_ase_with_color_space, to_dxf, to_eps, to_pdf,
    to_separation_eps, to_separation_pdfs, to_separation_svgs, to_svg, LinesAs,
};
use trasea_core::watch::{watch_folder, WatchConfig};
use trasea_core::{load, vectorize, Params, ProductionMode, ProductionOptions, SymmetryMode};

/// `#RRGGBB` (or `RRGGBB`) to a colour.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// `x,y,w,h` to `[u32; 4]`.
fn parse_crop(s: &str) -> Option<[u32; 4]> {
    let parts: Vec<u32> = s.split(',').filter_map(|p| p.trim().parse::<u32>().ok()).collect();
    if parts.len() == 4 && parts[2] > 0 && parts[3] > 0 {
        Some([parts[0], parts[1], parts[2], parts[3]])
    } else {
        None
    }
}

fn usage() {
    eprintln!(
        "usage:\n  trasea-cli <input> <output.svg> [--mode standard|sablon|cutting|shading] [--sablon-knockout] [--pdf out.pdf] [--eps out.eps] [--dxf out.dxf] [--ase out.ase] [--separations DIR] [--color-space rgb|cmyk] [--dxf-unit mm|px] [--dxf-width-mm MM] [--crop x,y,w,h] [--rotate DEG] [--colors N] [--smooth 0..1] [--lines stroke|shape] [--trap PT] [--min-stroke PT] [--reg-marks|--no-reg-marks] [--strip-bg|--keep-bg] [--no-lines] [--ai|--no-ai] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--target-nodes N] [--palette \"#RRGGBB,#RRGGBB\"] [--png out.png] [--sidecar DIR]\n  trasea-cli --batch <input-dir> <output-dir> [--mode standard|sablon|cutting|shading] [--sablon-knockout] [--subfolders] [--pdf] [--eps] [--dxf] [--ase] [--separations] [--color-space rgb|cmyk] [--dxf-unit mm|px] [--dxf-width-mm MM] [--no-report] [options...]\n  trasea-cli --watch <input-dir> --out <output-dir> [--poll-ms MS] [--debounce-ms MS] [--watch-timeout-ms MS] [batch options...]"
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
    let is_watch = args.first().map(String::as_str) == Some("--watch");
    let is_folder_mode = is_batch || is_watch;

    if (is_batch && args.len() < 3) || (is_watch && args.len() < 2) || (!is_folder_mode && args.len() < 2) {
        usage();
        std::process::exit(2);
    }

    let (input, mut output, start_i) = if is_batch {
        (PathBuf::from(&args[1]), PathBuf::from(&args[2]), 3usize)
    } else if is_watch {
        // Accept both `--watch <in_dir> --out <out_dir>` and `--watch <in_dir> <out_dir>`
        if args.get(2).map(String::as_str) == Some("--out") {
            if args.len() < 4 {
                usage();
                std::process::exit(2);
            }
            (PathBuf::from(&args[1]), PathBuf::from(&args[3]), 4usize)
        } else if args.len() >= 3 && !args[2].starts_with("--") {
            (PathBuf::from(&args[1]), PathBuf::from(&args[2]), 3usize)
        } else {
            (PathBuf::from(&args[1]), PathBuf::new(), 2usize)
        }
    } else {
        (PathBuf::from(&args[0]), PathBuf::from(&args[1]), 2usize)
    };

    let mut p = Params::default();
    p.ai_clean = false;
    let mut pdf_path: Option<PathBuf> = None;
    let mut eps_path: Option<PathBuf> = None;
    let mut dxf_path: Option<PathBuf> = None;
    let mut ase_path: Option<PathBuf> = None;
    let mut png_path: Option<PathBuf> = None;
    let mut sep_dir: Option<PathBuf> = None;
    let mut batch_pdf = false;
    let mut batch_eps = false;
    let mut batch_dxf = false;
    let mut batch_ase = false;
    let mut batch_separations = false;
    let mut batch_subfolders = false;
    let mut batch_report = true;
    let mut poll_ms: u64 = 1000;
    let mut debounce_ms: u64 = 500;
    let mut watch_timeout_ms: Option<u64> = None;

    let mut mode: Option<ProductionMode> = None;
    let mut sablon_knockout = false;
    let mut colors_set = false;
    let mut smooth_set = false;
    let mut lines_override: Option<LinesAs> = None;
    let mut trap_override: Option<f64> = None;
    let mut min_stroke_override: Option<f64> = None;
    let mut reg_marks_override: Option<bool> = None;
    let mut strip_bg_override: Option<bool> = None;
    let mut color_space = ColorSpace::Rgb;
    let mut dxf_unit = DxfUnit::Mm;
    let mut dxf_width_mm: Option<f32> = None;

    let mut i = start_i;
    while i < args.len() {
        match args[i].as_str() {
            "--out" => {
                i += 1;
                if let Some(v) = args.get(i) {
                    output = PathBuf::from(v);
                }
            }
            "--mode" => {
                i += 1;
                match args.get(i).and_then(|v| ProductionMode::parse(v)) {
                    Some(m) => mode = Some(m),
                    None => {
                        eprintln!("--mode needs one of: standard, sablon, cutting, shading");
                        std::process::exit(2);
                    }
                }
            }
            "--sablon-knockout" => sablon_knockout = true,
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
            "--color-space" => {
                i += 1;
                match args.get(i).and_then(|v| ColorSpace::parse(v)) {
                    Some(cs) => color_space = cs,
                    None => {
                        eprintln!("--color-space needs one of: rgb, cmyk");
                        std::process::exit(2);
                    }
                }
            }
            "--dxf-unit" => {
                i += 1;
                match args.get(i).and_then(|v| DxfUnit::parse(v)) {
                    Some(u) => dxf_unit = u,
                    None => {
                        eprintln!("--dxf-unit needs one of: mm, px");
                        std::process::exit(2);
                    }
                }
            }
            "--dxf-width-mm" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|&v| v.is_finite() && v > 0.0) {
                    Some(w) => dxf_width_mm = Some(w),
                    None => {
                        eprintln!("--dxf-width-mm needs a positive number (e.g. 100)");
                        std::process::exit(2);
                    }
                }
            }
            "--crop" => {
                i += 1;
                match args.get(i).and_then(|v| parse_crop(v)) {
                    Some(rect) => p.preprocess.crop = Some(rect),
                    None => {
                        eprintln!("--crop needs x,y,w,h (positive w and h)");
                        std::process::exit(2);
                    }
                }
            }
            "--rotate" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite()) {
                    Some(deg) => p.preprocess.rotate_deg = deg,
                    None => {
                        eprintln!("--rotate needs a finite angle in degrees");
                        std::process::exit(2);
                    }
                }
            }
            "--pdf" => {
                if is_folder_mode {
                    batch_pdf = true;
                } else {
                    i += 1;
                    pdf_path = args.get(i).map(PathBuf::from);
                }
            }
            "--eps" => {
                if is_folder_mode {
                    batch_eps = true;
                } else {
                    i += 1;
                    eps_path = args.get(i).map(PathBuf::from);
                }
            }
            "--dxf" => {
                if is_folder_mode {
                    batch_dxf = true;
                } else {
                    i += 1;
                    dxf_path = args.get(i).map(PathBuf::from);
                }
            }
            "--ase" => {
                if is_folder_mode {
                    batch_ase = true;
                } else {
                    i += 1;
                    ase_path = args.get(i).map(PathBuf::from);
                }
            }
            "--separations" => {
                if is_folder_mode {
                    batch_separations = true;
                } else {
                    i += 1;
                    sep_dir = args.get(i).map(PathBuf::from);
                }
            }
            "--subfolders" => batch_subfolders = true,
            "--no-report" => batch_report = false,
            "--poll-ms" => {
                i += 1;
                poll_ms = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(1000);
            }
            "--debounce-ms" => {
                i += 1;
                debounce_ms = args.get(i).and_then(|v| v.parse().ok()).unwrap_or(500);
            }
            "--watch-timeout-ms" => {
                i += 1;
                watch_timeout_ms = args.get(i).and_then(|v| v.parse().ok());
            }
            "--lines" => {
                i += 1;
                lines_override = match args.get(i).map(String::as_str) {
                    Some("shape") => Some(LinesAs::Shape),
                    _ => Some(LinesAs::Stroke),
                };
            }
            "--trap" => {
                i += 1;
                trap_override = args.get(i).and_then(|v| v.parse().ok());
            }
            "--min-stroke" => {
                i += 1;
                min_stroke_override = args.get(i).and_then(|v| v.parse().ok());
            }
            "--reg-marks" => reg_marks_override = Some(true),
            "--no-reg-marks" => reg_marks_override = Some(false),
            "--strip-bg" => strip_bg_override = Some(true),
            "--keep-bg" => strip_bg_override = Some(false),
            "--no-lines" => p.keep_lines = false,
            "--ai" => p.ai_clean = true,
            "--no-ai" => p.ai_clean = false,
            "--no-primitives" => p.detect_primitives = false,
            "--no-text-detect" => p.detect_text = false,
            "--symmetry" => {
                i += 1;
                match args.get(i).and_then(|v| SymmetryMode::parse(v)) {
                    Some(s) => p.symmetry = s,
                    None => {
                        eprintln!("--symmetry needs one of: off, auto, vertical, horizontal");
                        std::process::exit(2);
                    }
                }
            }
            "--target-nodes" => {
                i += 1;
                match args.get(i).and_then(|v| v.parse::<usize>().ok()).filter(|&n| n > 0) {
                    Some(n) => p.target_nodes = Some(n),
                    None => {
                        eprintln!("--target-nodes needs a positive integer");
                        std::process::exit(2);
                    }
                }
            }
            "--palette" => {
                i += 1;
                let list: Vec<[u8; 3]> = args.get(i).map(|s| s.split(',').filter_map(parse_hex).collect()).unwrap_or_default();
                if list.len() < 2 {
                    eprintln!("--palette needs at least 2 hex colours separated by commas, e.g. \"#FFFFFF,#000000\"");
                    std::process::exit(2);
                }
                p.custom_palette = Some(list);
            }
            "--png" => {
                i += 1;
                png_path = args.get(i).map(PathBuf::from);
            }
            "--sidecar" => {
                i += 1;
                p.sidecar_dir = args.get(i).map(PathBuf::from);
            }
            other => {
                eprintln!("unknown option: {other}");
                usage();
                std::process::exit(2);
            }
        }
        i += 1;
    }

    // Apply production mode defaults before explicit overrides.
    let active_mode = mode.unwrap_or(ProductionMode::Standard);
    let mut prod = match active_mode {
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
        ProductionMode::Shading => {
            if !colors_set {
                p.colors = 18;
            }
            ProductionOptions::shading()
        }
    };
    prod.sablon_knockout = sablon_knockout;
    if let Some(ms) = min_stroke_override {
        prod.min_stroke_pt = ms.max(0.0);
    }
    if let Some(rm) = reg_marks_override {
        prod.registration_marks = rm;
    }
    if let Some(sb) = strip_bg_override {
        prod.strip_background = sb;
    }
    p.production = prod;

    let mut eo = export_options_for_production(&p.production);
    eo.color_space = color_space;
    eo.dxf_unit = dxf_unit;
    eo.dxf_target_width_mm = dxf_width_mm;
    if let Some(la) = lines_override {
        if p.production.mode != ProductionMode::CutEmbroidery {
            eo.lines_as = la;
        }
    }
    if let Some(tp) = trap_override {
        if p.production.mode != ProductionMode::CutEmbroidery {
            eo.trap_pt = tp.max(0.0);
        }
    }
    if let Some(rm) = reg_marks_override {
        eo.registration_marks = rm;
    }
    if let Some(sb) = strip_bg_override {
        eo.strip_background = sb;
    }

    if is_watch {
        if output.as_os_str().is_empty() {
            eprintln!("--watch requires --out <output-dir>");
            usage();
            std::process::exit(2);
        }
        let batch_cfg = BatchConfig {
            inputs: Vec::new(),
            output_dir: output.clone(),
            params: p,
            export: eo,
            write_pdf: batch_pdf,
            write_eps: batch_eps,
            write_dxf: batch_dxf,
            write_ase: batch_ase,
            write_separations: batch_separations,
            write_report_csv: batch_report,
            write_report_md: batch_report,
            color_space,
            dxf_unit,
            dxf_target_width_mm: dxf_width_mm,
            max_workers: None,
        };
        let mut wcfg = WatchConfig::new(input.clone(), output.clone(), batch_cfg);
        wcfg.poll_ms = poll_ms;
        wcfg.debounce_ms = debounce_ms;
        wcfg.include_subfolders = batch_subfolders;

        let stop = Arc::new(AtomicBool::new(false));
        if let Some(ms) = watch_timeout_ms {
            let s2 = Arc::clone(&stop);
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(ms));
                s2.store(true, Ordering::Relaxed);
            });
        }
        eprintln!("watching {} -> {} (poll {}ms, debounce {}ms)", input.display(), output.display(), poll_ms, debounce_ms);
        match watch_folder(&wcfg, stop, |ev| {
            eprintln!(
                "[watch] {} -> {} (node {}, {}ms){}",
                ev.file_name,
                ev.status.label(),
                ev.nodes,
                ev.elapsed_ms,
                ev.error.as_deref().map(|e| format!(" | err: {e}")).unwrap_or_default()
            );
        }) {
            Ok(events) => {
                println!("watch selesai: {} event diproses -> {}", events.len(), output.display());
            }
            Err(e) => {
                eprintln!("error: {e}");
                std::process::exit(1);
            }
        }
        return;
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
            eprintln!("Tidak ada file gambar didukung (.png/.jpg/.webp/.bmp) di {}", input.display());
            std::process::exit(1);
        }
        let cfg = BatchConfig {
            inputs: files,
            output_dir: output.clone(),
            params: p,
            export: eo,
            write_pdf: batch_pdf,
            write_eps: batch_eps,
            write_dxf: batch_dxf,
            write_ase: batch_ase,
            write_separations: batch_separations,
            write_report_csv: batch_report,
            write_report_md: batch_report,
            color_space,
            dxf_unit,
            dxf_target_width_mm: dxf_width_mm,
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
    if let Some(eps) = &eps_path {
        std::fs::write(eps, to_eps(&out.doc, &eo)).expect("cannot write the EPS");
    }
    if let Some(dxf) = &dxf_path {
        std::fs::write(dxf, to_dxf(&out.doc, &eo)).expect("cannot write the DXF");
    }
    if let Some(ase) = &ase_path {
        std::fs::write(ase, to_ase_with_color_space(&palette_of(&out.doc), eo.color_space)).expect("cannot write the ASE");
    }
    if let Some(pdf) = &pdf_path {
        match to_pdf(&out.doc, &eo) {
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
        }
        if pdf_path.is_some() {
            if let Ok(pdfs) = to_separation_pdfs(&out.doc, &eo) {
                for (film, bytes) in pdfs {
                    let f_pdf = sdir.join(format!("{stem}_{}.pdf", film.suffix));
                    let _ = std::fs::write(&f_pdf, bytes);
                }
            }
        }
        if eps_path.is_some() {
            for (film, eps_text) in to_separation_eps(&out.doc, &eo) {
                let f_eps = sdir.join(format!("{stem}_{}.eps", film.suffix));
                let _ = std::fs::write(&f_eps, eps_text);
            }
        }
        eprintln!("separations: {} films written to {}", films.len(), sdir.display());
    }
    if let Some(png) = png_path {
        match trasea_core::stats::render_svg(&svg, out.doc.width, out.doc.height) {
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
