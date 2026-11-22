//! Developer CLI: `vektora-cli in.jpg out.svg [--pdf out.pdf] [--colors N] [--smooth 0..1] [--no-lines] [--ai] [--lines stroke|shape] [--trap PT] [--ase out.ase] [--sidecar DIR]`

use std::path::PathBuf;
use std::time::Instant;
use vektora_core::export::{palette_of, to_ase, to_svg, ExportOptions, LinesAs};
use vektora_core::{export, load, vectorize, Params};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        eprintln!("usage: vektora-cli <input> <output.svg> [--pdf out.pdf] [--colors N] [--smooth 0..1] [--no-lines] [--no-ai] [--sidecar DIR]");
        std::process::exit(2);
    }
    let (input, output) = (PathBuf::from(&args[0]), PathBuf::from(&args[1]));
    let mut p = Params::default();
    let mut pdf: Option<PathBuf> = None;
    let mut ase: Option<PathBuf> = None;
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
    for n in &out.notes {
        eprintln!("note: {n}");
    }
    println!(
        "paths {} | nodes {} | {} KB | MAE {} | thin px {} | AI {} | {:.1}s",
        out.stats.paths,
        out.stats.nodes,
        out.stats.bytes / 1024,
        out.stats.mae.map_or("-".to_string(), |m| format!("{m:.2}")),
        out.thin_pixels,
        out.used_ai,
        t0.elapsed().as_secs_f32()
    );
}
