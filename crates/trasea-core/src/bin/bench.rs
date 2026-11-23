//! Benchmark: `cargo run --release --bin bench -- [--dir <vectorize-bench>] [--sidecar <dir>] [--out bench]`
//! Runs the pipeline on the 10 test images and writes `bench/RESULTS.md` (+ results.json). The inputs and the
//! produced SVG/PNG files stay outside git (some inputs carry a stock-photo watermark and are test-only).

use image::RgbImage;
use std::path::PathBuf;
use std::time::Instant;
use trasea_core::lines::{dilate_mask, thin_mask, ThinParams};
use trasea_core::export::{to_svg, ExportOptions, LinesAs};
use trasea_core::stats::{compare, count_svg, render_svg};
use trasea_core::{load, vectorize, Params};

const DEFAULT_DIR: &str = "C:/Users/atrione/OpenBot/Agents/agent-770c1b2c-74f1-4ef1-99c2-3f9a8f833d93/vectorize-bench";
const DEFAULT_SIDECAR: &str = "C:/Users/atrione/OpenBot/Agents/agent-770c1b2c-74f1-4ef1-99c2-3f9a8f833d93/tools/realesrgan";

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Illustration,
    Logo,
}

struct Item {
    label: &'static str,
    file: &'static str,
    kind: Kind,
    /// Node count to beat from the test plan (B1: ESRGAN 2x baseline; B3: hand-made vector x2 is the limit).
    target_nodes: Option<usize>,
}

const ITEMS: &[Item] = &[
    Item { label: "Orang (6 people)", file: "a5ace655e0efac144731489e3c42ec6f", kind: Kind::Illustration, target_nodes: Some(1903) },
    Item { label: "Coastal Drift", file: "Coastal Drift", kind: Kind::Illustration, target_nodes: Some(2208) },
    Item { label: "Wartec", file: "d92c7c3eca71dddd688db2988652f362", kind: Kind::Illustration, target_nodes: Some(4337) },
    Item { label: "Drifting Leaf", file: "Drifting Leaf", kind: Kind::Illustration, target_nodes: Some(6159) },
    Item { label: "Logo taekwondo", file: "logo taekwondo", kind: Kind::Logo, target_nodes: Some(113) },
    Item { label: "Logo golf", file: "logo atrione golf", kind: Kind::Logo, target_nodes: Some(413) },
    Item { label: "Logo Bangga", file: "Logo_Bangga_Buatan_Indonesia", kind: Kind::Logo, target_nodes: Some(645) },
    Item { label: "Logo Ari Reliyanto", file: "Logo of Ari Reliyanto 1", kind: Kind::Logo, target_nodes: None },
    Item { label: "Logo kos kausar", file: "logo kos kausar -Recovered-", kind: Kind::Logo, target_nodes: None },
    Item { label: "Logo atrione taekwondo", file: "atrione takewondo", kind: Kind::Logo, target_nodes: None },
];

/// Share of thin-line pixels of the ORIGINAL that are still covered (within 1 px) by thin lines of the render.
fn thin_recall(orig: &RgbImage, render: &RgbImage) -> f64 {
    let p = ThinParams::for_size(orig.width(), orig.height());
    let a = thin_mask(orig, &p);
    let b = thin_mask(render, &p);
    let (w, h) = (a.w, a.h);
    let near = dilate_mask(&b.mask, w, h, 1);
    let total = a.count().max(1);
    let hit = (0..w * h).filter(|&i| a.mask[i] && near[i]).count();
    hit as f64 / total as f64
}

struct Row {
    nodes: usize,
    paths: usize,
    kb: usize,
    mae: f64,
    recall: f64,
}

/// Debug picture: the original dimmed, thin-line pixels green when the render shows them and red when it lost them.
fn missed_overlay(orig: &RgbImage, render: &RgbImage) -> RgbImage {
    let p = ThinParams::for_size(orig.width(), orig.height());
    let a = thin_mask(orig, &p);
    let b = thin_mask(render, &p);
    let near = dilate_mask(&b.mask, a.w, a.h, 1);
    let mut out = orig.clone();
    for (i, px) in out.pixels_mut().enumerate() {
        let base = [px[0] / 2 + 100, px[1] / 2 + 100, px[2] / 2 + 100];
        *px = image::Rgb(if a.mask[i] { if near[i] { [0, 180, 0] } else { [230, 0, 0] } } else { base });
    }
    out
}

fn measure_svg(svg: &str, orig: &RgbImage) -> Option<Row> {
    let (paths, nodes) = count_svg(svg);
    let r = render_svg(svg, orig.width(), orig.height()).ok()?;
    if let Ok(dir) = std::env::var("VK_MISSED_DIR") {
        let _ = missed_overlay(orig, &r).save(std::path::Path::new(&dir).join(format!("{}x{}-{}.png", orig.width(), orig.height(), nodes)));
    }
    let (mae, _) = compare(orig, &r);
    Some(Row { nodes, paths, kb: svg.len() / 1024, mae, recall: thin_recall(orig, &r) })
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut dir = PathBuf::from(DEFAULT_DIR);
    let mut sidecar = PathBuf::from(DEFAULT_SIDECAR);
    let mut out = PathBuf::from("bench");
    let mut smooth = 0.5f32;
    let mut only: Option<String> = None;
    let mut ai_mode = "both".to_string();
    // folder with the `<file>.noai.svg` outputs of the previous version (for the before/after table)
    let mut prev_dir: Option<PathBuf> = None;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--dir" => {
                i += 1;
                dir = PathBuf::from(&args[i]);
            }
            "--sidecar" => {
                i += 1;
                sidecar = PathBuf::from(&args[i]);
            }
            "--smooth" => {
                i += 1;
                smooth = args[i].parse().unwrap_or(0.5);
            }
            "--ai" => {
                i += 1;
                ai_mode = args[i].clone();
            }
            "--only" => {
                i += 1;
                only = Some(args[i].clone());
            }
            "--out" => {
                i += 1;
                out = PathBuf::from(&args[i]);
            }
            "--prev" => {
                i += 1;
                prev_dir = Some(PathBuf::from(&args[i]));
            }
            _ => {}
        }
        i += 1;
    }
    let work = out.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let have_ai = sidecar.join(trasea_core::clean::SIDECAR_EXE).is_file();
    println!("sidecar: {} ({})", sidecar.display(), if have_ai { "found" } else { "MISSING: only the median variant is run" });

    let mut md = String::new();
    md.push_str("# Trasea 0.1.0 benchmark\n\n");
    md.push_str("Counting: nodes = every M, L and C segment (the same counter as the baseline benchmark). MAE = mean absolute RGB error of the render against the 1200 px source (0..255, white background). Thin-line recall = share of the original's thin-line pixels (dark strokes thinner than about 2r+1 px, found by the same detector the guard uses) that the render still shows within 1 px. Time = whole pipeline on this PC (integrated AMD GPU for Real-ESRGAN).\n\n");
    let mut json = Vec::new();
    let mut modes: Vec<String> = Vec::new();
    let mut cmp: Vec<String> = Vec::new();
    let mut total_secs = 0.0f64;

    for (title, kind) in [("Illustrations (B1, B2, B4)", Kind::Illustration), ("Logos (B3)", Kind::Logo)] {
        md.push_str(&format!("## {title}\n\n| file | variant | paths | nodes | KB | MAE | thin-line recall | time s |\n|---|---|---:|---:|---:|---:|---:|---:|\n"));
        for it in ITEMS.iter().filter(|it| it.kind == kind && only.as_ref().map_or(true, |o| it.label.to_lowercase().contains(&o.to_lowercase()) || it.file.to_lowercase().contains(&o.to_lowercase()))) {
            let src_png = dir.join("out").join(format!("{}.src.png", it.file));
            let Ok(orig) = load::load_path(&src_png, 0) else {
                eprintln!("missing {}", src_png.display());
                continue;
            };
            // reference row: the earlier baseline (illustrations) or the owner's hand-made vector (logos)
            if kind == Kind::Illustration {
                let b = dir.join("out").join(format!("{}.flat.esrgan-2x.svg", it.file));
                if let Some(r) = std::fs::read_to_string(&b).ok().and_then(|svg| measure_svg(&svg, &orig)) {
                    md.push_str(&format!("| {} | baseline: VTracer on ESRGAN 2x | {} | {} | {} | {:.2} | {:.0}% | - |\n", it.label, r.paths, r.nodes, r.kb, r.mae, r.recall * 100.0));
                }
            } else {
                let hand = dir.join("in").join(format!("{}.svg", it.file));
                if let Some(r) = std::fs::read_to_string(&hand).ok().and_then(|svg| measure_svg(&svg, &orig)) {
                    let nodes = it.target_nodes.map_or(format!("{}*", r.nodes), |n| format!("{n}"));
                    md.push_str(&format!("| {} | hand-made vector (owner) | {} | {} | {} | {:.2} | {:.0}% | - |\n", it.label, r.paths, nodes, r.kb, r.mae, r.recall * 100.0));
                }
            }
            let mut variants: Vec<(&str, bool)> = vec![("Trasea (median, no AI)", false)];
            if have_ai && ai_mode != "off" {
                variants.insert(0, ("Trasea (AI clean)", true));
            }
            if ai_mode == "on" {
                variants.retain(|v| v.1);
            }
            for (name, ai) in variants {
                let params = Params { smooth, ai_clean: ai, sidecar_dir: Some(sidecar.clone()), measure: false, ..Params::default() };
                let t0 = Instant::now();
                let res = vectorize(&orig, &params, &|_, _| {});
                let secs = t0.elapsed().as_secs_f64();
                total_secs += secs;
                match res {
                    Ok(o) => {
                        let tag = if ai { "ai" } else { "noai" };
                        let _ = std::fs::write(work.join(format!("{}.{tag}.svg", it.file)), &o.svg);
                        if !ai {
                            let shape = to_svg(&o.doc, &ExportOptions { lines_as: LinesAs::Shape, trap_pt: 0.0, ..Default::default() });
                            let (_, ns) = count_svg(&shape);
                            let (_, nk) = count_svg(&o.svg);
                            let gn = |svg: &str| svg.find("id=\"Garis\"").map_or(0, |i| count_svg(&svg[i..]).1);
                            modes.push(format!("| {} | {} | {} | {} | {} |", it.label, nk, ns, gn(&o.svg), gn(&shape)));
                            let gp = |svg: &str| svg.find("id=\"Garis\"").map_or(0, |i| count_svg(&svg[i..]).0);
                            let prev = prev_dir.as_ref().and_then(|d| std::fs::read_to_string(d.join(format!("{}.noai.svg", it.file))).ok());
                            let (pp, pn) = prev.as_ref().map_or(("-".to_string(), "-".to_string()), |s| (gp(s).to_string(), count_svg(s).1.to_string()));
                            cmp.push(format!("| {} | {} | {} | {} | {} | {} | {} | {} |", it.label, pp, gp(&o.svg), pn, nk, o.stats.primitives, o.text_regions.len(), o.symmetry_applied.as_deref().unwrap_or("-")));
                        }
                        if let Some(r) = measure_svg(&o.svg, &orig) {
                            let note = if ai && !o.used_ai { " (AI FAILED)" } else { "" };
                            md.push_str(&format!("| {} | {}{} | {} | {} | {} | {:.2} | {:.0}% | {:.1} |\n", it.label, name, note, r.paths, r.nodes, r.kb, r.mae, r.recall * 100.0, secs));
                            json.push(serde_json::json!({"file": it.file, "variant": name, "paths": r.paths, "nodes": r.nodes, "kb": r.kb, "mae": r.mae, "recall": r.recall, "secs": secs, "target_nodes": it.target_nodes}));
                            println!("{:<24} {:<26} nodes {:>6} (target {:>5}) MAE {:>5.2} recall {:>4.0}% {:>5.1}s", it.label, name, r.nodes, it.target_nodes.map_or("-".to_string(), |n| n.to_string()), r.mae, r.recall * 100.0, secs);
                        }
                    }
                    Err(e) => println!("{}: error {e}", it.label),
                }
            }
        }
        md.push('\n');
    }
    md.push_str("## 0.2.0 vs 0.3.0 (tanpa AI)\n\n| file | path di layer Garis 0.2.0 | path di layer Garis 0.3.0 | node total 0.2.0 | node total 0.3.0 | bentuk dasar | area teks terdeteksi | simetri |\n|---|---:|---:|---:|---:|---:|---:|---|\n");
    for c in &cmp {
        md.push_str(c);
        md.push('\n');
    }
    md.push_str(&format!("\nWaktu total seluruh gambar benchmark (semua varian yang dijalankan): {total_secs:.1} detik.\n\n"));
    md.push_str("## Garis tipis sebagai stroke vs bentuk (0.3.0, tanpa AI)

| file | node total (stroke) | node total (bentuk) | node grup Garis (stroke) | node grup Garis (bentuk) |
|---|---:|---:|---:|---:|
");
    for m in &modes {
        md.push_str(m);
        md.push_str("
");
    }
    md.push_str("
`*` = counted from the hand-made SVG with this counter (shapes such as rect or circle are not counted).\n");
    std::fs::write(out.join("RESULTS.md"), &md).unwrap();
    std::fs::write(out.join("results.json"), serde_json::to_string_pretty(&json).unwrap()).unwrap();
}
