//! Developer CLI: `trasea-cli in.jpg out.svg [--pdf out.pdf] [--colors N] [--smooth 0..1] [--no-lines] [--ai] [--lines stroke|shape] [--trap PT] [--ase out.ase] [--sidecar DIR]`

use std::path::PathBuf;
use std::time::Instant;
use trasea_core::export::{palette_of, to_ase, to_svg, ExportOptions, LinesAs};
use trasea_core::{export, load, vectorize, Params, SymmetryMode};

/// `#RRGGBB` (or `RRGGBB`) to a colour.
fn parse_hex(s: &str) -> Option<[u8; 3]> {
    let h = s.trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
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
    if args.len() < 2 {
        eprintln!("usage: trasea-cli <input> <output.svg> [--pdf out.pdf] [--colors N] [--smooth 0..1] [--no-lines] [--no-ai] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--target-nodes N] [--palette \"#RRGGBB,#RRGGBB\"] [--png out.png] [--sidecar DIR]");
        std::process::exit(2);
    }
    let (input, output) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    let mut p = Params::default();
    let mut pdf: Option<PathBuf> = None;
    let mut ase: Option<PathBuf> = None;
    let mut png: Option<PathBuf> = None;
    let mut eo = ExportOptions::default();
    p.ai_clean = false;
    let mut i = 2;
    while i < args.len() {
        match args[i].as_str() {
            "--colors" => {
                i += 1;
                p.colors = args[i].parse().unwrap_or(10);
            }
            "--smooth" => {
                i += 1;
                p.smooth = args[i].parse().unwrap_or(0.5);
            }
            "--pdf" => {
                i += 1;
                pdf = Some(PathBuf::from(&args[i]));
            }
            "--ase" => {
                i += 1;
                ase = Some(PathBuf::from(&args[i]));
            }
            "--lines" => {
                i += 1;
                eo.lines_as = if args[i] == "shape" { LinesAs::Shape } else { LinesAs::Stroke };
            }
            "--trap" => {
                i += 1;
                eo.trap_pt = args[i].parse().unwrap_or(0.0);
            }
            "--ai" => p.ai_clean = true,
            "--sidecar" => {
                i += 1;
                p.sidecar_dir = Some(PathBuf::from(&args[i]));
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
                png = Some(PathBuf::from(&args[i]));
            }
            other => {
                eprintln!("unknown option {other}");
                std::process::exit(2);
            }
        }
        i += 1;
    }
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
    if let Some(ase) = ase {
        std::fs::write(&ase, to_ase(&palette_of(&out.doc))).expect("cannot write the ASE");
    }
    if let Some(pdf) = pdf {
        match export::svg_to_pdf(&svg) {
            Ok(bytes) => std::fs::write(&pdf, bytes).expect("cannot write the PDF"),
            Err(e) => eprintln!("PDF: {e}"),
        }
    }
    if let Some(png) = png {
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
        eprintln!("text areas: {} {:?}", out.text_regions.len(), out.text_regions.iter().map(|r| (r.x as i64, r.y as i64, r.w as i64, r.h as i64, r.glyph_count)).collect::<Vec<_>>());
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
