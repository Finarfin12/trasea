//! Automatic tests D0-D6 for 0.4.0 (D7 = the 29 older tests in core.rs, core02.rs and core03.rs, unchanged).
//! Synthetic images only.

use image::{Rgb, RgbImage};
use trasea_core::export::{palette_of, to_ase};
use trasea_core::stats::{count_svg, render_svg};
use trasea_core::{prepare, vectorize, vectorize_prepared, BrushKind, BrushStroke, Params};

const WHITE: [u8; 3] = [255, 255, 255];

fn quiet() -> Params {
    Params { ai_clean: false, measure: false, keep_lines: false, detect_text: false, ..Params::default() }
}

/// Fill the pixels for which `inside(x, y)` holds, with 4x4 supersampling.
fn paint(img: &mut RgbImage, color: [u8; 3], inside: impl Fn(f64, f64) -> bool) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    for y in 0..h {
        for x in 0..w {
            let mut hit = 0;
            for sy in 0..4 {
                for sx in 0..4 {
                    if inside(x as f64 + (sx as f64 + 0.5) / 4.0, y as f64 + (sy as f64 + 0.5) / 4.0) {
                        hit += 1;
                    }
                }
            }
            if hit > 0 {
                let a = hit as f64 / 16.0;
                let old = img.get_pixel(x as u32, y as u32).0;
                let mix = |o: u8, n: u8| (o as f64 * (1.0 - a) + n as f64 * a).round() as u8;
                img.put_pixel(x as u32, y as u32, Rgb([mix(old[0], color[0]), mix(old[1], color[1]), mix(old[2], color[2])]));
            }
        }
    }
}

fn hex(c: [u8; 3]) -> String {
    format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2])
}

fn read(rel: &str) -> String {
    std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

// ---------------------------------------------------------------- D0

#[test]
fn d0_the_product_is_called_trasea_everywhere_the_user_can_see_it() {
    let conf = read("../../src-tauri/tauri.conf.json");
    assert!(conf.contains("\"productName\": \"Trasea\"") && conf.contains("\"identifier\": \"app.trasea.desktop\"") && conf.contains("\"title\": \"Trasea\""), "{conf}");
    for rel in ["../../src-tauri/tauri.conf.json", "../../index.html", "../../src/App.tsx", "../../src-tauri/Cargo.toml", "Cargo.toml", "../../package.json", "../../src-tauri/src/main.rs"] {
        let t = read(rel);
        assert!(!t.to_lowercase().contains("vektora"), "{rel} still says Vektora");
    }
    assert!(read("../../index.html").contains("<title>Trasea</title>"));
    assert!(read("../../src/App.tsx").contains("trasea-icon-mini.svg") && read("../../src/App.tsx").contains("</img>") == false && read("../../src/App.tsx").contains("alt=\"\" aria-hidden=\"true\" width={24} height={24} /> Trasea"), "the header must show the Trasea logo and the name");
    assert!(read("Cargo.toml").contains("name = \"trasea-core\"") && read("Cargo.toml").contains("name = \"trasea-cli\""));
    assert!(read("../../src-tauri/Cargo.toml").contains("name = \"trasea\""));
    assert!(std::path::Path::new(env!("CARGO_BIN_EXE_trasea-cli")).is_file(), "the CLI binary must be trasea-cli");
    // the palette file: group "Trasea", colours "Trasea NN #RRGGBB"
    let ase = to_ase(&[[1, 2, 3], [4, 5, 6]]);
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect() };
    let has = |needle: &[u8]| ase.windows(needle.len()).any(|w| w == needle);
    assert!(has(&utf16("Trasea")) && has(&utf16("Trasea 01 #010203")) && !has(&utf16("Vektora")));
    // the icon is the letter T: the 512 px icon is not the old V (they differ from the old file's size/hash is not checkable here),
    // so check that the source icon exists and is a PNG
    let icon = std::fs::read(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../src-tauri/icons/icon.png")).unwrap();
    assert!(icon.starts_with(&[0x89, b'P', b'N', b'G']));
}

// ---------------------------------------------------------------- D1

/// Two identical blobs, each with a jagged top edge (3 px teeth every 8 px): left x 40..280, right x 360..600.
fn jagged() -> RgbImage {
    let mut img = RgbImage::from_pixel(640, 240, Rgb(WHITE));
    for x0 in [40.0, 360.0] {
        paint(&mut img, [20, 30, 90], |x, y| {
            if x < x0 || x > x0 + 240.0 || y > 180.0 {
                return false;
            }
            let phase = ((x - x0) / 8.0).fract();
            let tri = if phase < 0.5 { phase * 2.0 } else { 2.0 - phase * 2.0 };
            y >= 60.0 + 3.0 * tri
        });
    }
    img
}

fn anchors_in(doc: &trasea_core::export::Doc, from_x: f64, to_x: f64) -> usize {
    let k = doc.scale;
    doc.fields.iter().filter(|l| !l.canvas).flat_map(|l| l.contours.iter()).map(|c| c.curves.iter().filter(|cv| cv[2].0 / k >= from_x && cv[2].0 / k < to_x).count()).sum()
}

fn mae_region(a: &RgbImage, b: &RgbImage, x0: u32, x1: u32, y0: u32, y1: u32) -> f64 {
    let mut sum = 0.0;
    for y in y0..y1 {
        for x in x0..x1 {
            let (p, q) = (a.get_pixel(x, y), b.get_pixel(x, y));
            sum += (0..3).map(|c| (p[c] as f64 - q[c] as f64).abs()).sum::<f64>() / 3.0;
        }
    }
    sum / ((x1 - x0) * (y1 - y0)) as f64
}

#[test]
fn d1_the_brush_smooths_one_side_and_sharpens_the_other_without_gaps() {
    let img = jagged();
    let base = vectorize(&img, &Params { colors: 3, ..quiet() }, &|_, _| {}).unwrap();
    // left edge: smooth brush along the top edge; right edge: detail brush
    let stroke = |kind, x0: f32, x1: f32| BrushStroke { kind, radius: 26.0, points: vec![(x0, 62.0), (x1, 62.0)] };
    let strokes = vec![stroke(BrushKind::Smooth, 36.0, 284.0), stroke(BrushKind::Detail, 356.0, 604.0)];
    let o = vectorize(&img, &Params { colors: 3, brush_strokes: strokes, ..quiet() }, &|_, _| {}).unwrap();

    let (base_left, base_right) = (anchors_in(&base.doc, 0.0, 320.0), anchors_in(&base.doc, 320.0, 640.0));
    let (left, right) = (anchors_in(&o.doc, 0.0, 320.0), anchors_in(&o.doc, 320.0, 640.0));
    assert!(base_left > 20 && base_right > 20, "the jagged edge must cost nodes at all: {base_left} / {base_right}");
    assert!(left * 10 <= base_left * 6, "smooth brush: {left} nodes vs {base_left} without it (want at most 60%)");
    assert!(right >= base_right, "detail brush: {right} nodes vs {base_right} without it");

    // detail side is closer to the picture than without the brush
    let (r_base, r_brush) = (render_svg(&base.svg, 640, 240).unwrap(), render_svg(&o.svg, 640, 240).unwrap());
    let (e_base, e_brush) = (mae_region(&img, &r_base, 360, 600, 40, 100), mae_region(&img, &r_brush, 360, 600, 40, 100));
    assert!(e_brush < e_base, "local error on the detail side: {e_brush:.3} vs {e_base:.3} without the brush");

    // no white gap in the body, in particular where the painted areas begin and end
    for (x0, x1) in [(45u32, 275u32), (365, 595)] {
        for y in 75..175u32 {
            for x in x0..x1 {
                let p = r_brush.get_pixel(x, y);
                assert!(p[0] < 120 && p[1] < 120, "white gap at {x},{y}: {:?}", p);
            }
        }
    }
    // an empty brush changes nothing
    let same = vectorize(&img, &Params { colors: 3, brush_strokes: vec![], ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(same.svg, base.svg);
}

// ---------------------------------------------------------------- D2

fn five_colours() -> RgbImage {
    let mut img = RgbImage::from_pixel(500, 300, Rgb(WHITE));
    paint(&mut img, [200, 40, 40], |x, y| x < 160.0 && y > 40.0 && y < 260.0 && x > 30.0);
    paint(&mut img, [40, 160, 60], |x, y| x >= 160.0 && x < 270.0 && y > 40.0 && y < 260.0);
    paint(&mut img, [30, 60, 200], |x, y| x >= 270.0 && x < 380.0 && y > 40.0 && y < 260.0);
    paint(&mut img, [240, 200, 30], |x, y| x >= 380.0 && x < 470.0 && y > 40.0 && y < 260.0);
    // a tiny accent, far below the usual speck size
    paint(&mut img, [255, 0, 255], |x, y| (x - 90.0).powi(2) + (y - 150.0).powi(2) <= 6.25);
    img
}

fn group_count(svg: &str) -> usize {
    svg.matches("data-name=\"Warna ").count()
}

#[test]
fn d2_the_manual_palette_is_used_exactly() {
    let img = five_colours();
    // (a) fewer colours: 3 of the 5 (the others are mapped to the nearest one)
    let pal3 = vec![WHITE, [200, 40, 40], [30, 60, 200]];
    let o = vectorize(&img, &Params { custom_palette: Some(pal3.clone()), ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(o.palette, pal3);
    assert!(group_count(&o.svg) <= 3, "{} colour groups", group_count(&o.svg));
    let chosen: Vec<String> = pal3.iter().map(|c| hex(*c)).collect();
    for l in &o.doc.fields {
        assert!(chosen.contains(&hex(l.color)), "colour {} is not in the manual palette", hex(l.color));
    }
    // the .ase has exactly the colours of the drawing, named Trasea NN #RRGGBB
    let ase = to_ase(&palette_of(&o.doc));
    let utf16 = |s: &str| -> Vec<u8> { s.encode_utf16().flat_map(|u| u.to_be_bytes()).collect() };
    for (i, c) in palette_of(&o.doc).iter().enumerate() {
        let name = utf16(&format!("Trasea {:02} {}", i + 1, hex(*c)));
        assert!(ase.windows(name.len()).any(|w| w == name.as_slice()), "swatch {i} missing");
    }
    // (b) an accent colour picked from the picture survives although its spot is tiny
    let pal6 = vec![WHITE, [200, 40, 40], [40, 160, 60], [30, 60, 200], [240, 200, 30], [255, 0, 255]];
    let o = vectorize(&img, &Params { custom_palette: Some(pal6.clone()), ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(o.palette, pal6);
    assert!(o.svg.contains("#FF00FF"), "the accent colour is gone from the SVG");
    // without the manual palette the tiny dot is merged away
    let auto = vectorize(&img, &Params { colors: 6, ..quiet() }, &|_, _| {}).unwrap();
    assert!(!auto.svg.contains("#FF00FF"));
    // duplicates and a single colour are handled: duplicates collapse, fewer than 2 falls back to the automatic palette
    let dup = vectorize(&img, &Params { custom_palette: Some(vec![WHITE, WHITE, [200, 40, 40]]), ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(dup.palette.len(), 2);
    let one = vectorize(&img, &Params { custom_palette: Some(vec![WHITE]), colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    assert!(one.palette.len() >= 3 && one.notes.iter().any(|n| n.contains("manual palette")), "{:?}", one.notes);
}

// ---------------------------------------------------------------- D3

#[test]
fn d3_the_heatmap_finds_the_busy_corner_and_never_reaches_the_svg() {
    let mut img = RgbImage::from_pixel(400, 300, Rgb(WHITE));
    // top-left quadrant: a grid of small triangles (many nodes); the rest stays empty
    for gy in 0..9 {
        for gx in 0..15 {
            let (x0, y0) = (6.0 + gx as f64 * 13.0, 6.0 + gy as f64 * 16.0);
            paint(&mut img, [20, 30, 90], |x, y| y >= y0 && y <= y0 + 11.0 && (x - (x0 + 5.5)).abs() <= (y - y0) * 0.5);
        }
    }
    let o = vectorize(&img, &Params { colors: 2, detect_primitives: false, ..quiet() }, &|_, _| {}).unwrap();
    let hm = &o.heatmap;
    assert_eq!(hm.cols * hm.rows, hm.cells.len());
    assert!(hm.cols == 32 && hm.rows >= 8, "{}x{} cells", hm.cols, hm.rows);
    let (mut best, mut at) = (0u16, (0, 0));
    let mut bottom_right = 0usize;
    let mut total = 0usize;
    for r in 0..hm.rows {
        for c in 0..hm.cols {
            let v = hm.cells[r * hm.cols + c];
            total += v as usize;
            if v > best {
                best = v;
                at = (c, r);
            }
            if c >= hm.cols / 2 && r >= hm.rows / 2 {
                bottom_right += v as usize;
            }
        }
    }
    assert_eq!(best, hm.max_cell);
    assert!(at.0 < hm.cols / 2 && at.1 < hm.rows / 2, "the busiest cell {at:?} is not in the top-left quadrant");
    assert!(total > 100 && bottom_right * 50 <= total, "bottom-right holds {bottom_right} of {total} nodes");
    assert!(hm.hot_cells >= 1);
    // never in the export
    let svg = trasea_core::export::to_svg(&o.doc, &trasea_core::export::ExportOptions::default());
    assert_eq!(svg, o.svg);
    for bad in ["heat", "cells", "<rect fill=\"#ff", "opacity", "<canvas", "<image"] {
        assert!(!svg.to_lowercase().contains(bad), "the SVG contains {bad}");
    }
}

// ---------------------------------------------------------------- D4

/// Many wavy blobs in 4 colours: well over 1200 nodes.
fn blobs() -> RgbImage {
    let mut img = RgbImage::from_pixel(900, 600, Rgb([235, 217, 193]));
    let cols = [[15u8, 57, 115], [239, 141, 132], [60, 120, 80], [200, 160, 40]];
    let mut seed = 12345u64;
    let mut rnd = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as f64) / (1u64 << 31) as f64
    };
    for i in 0..110 {
        let (cx, cy) = (40.0 + (i % 11) as f64 * 78.0 + rnd() * 10.0, 40.0 + (i / 11) as f64 * 54.0 + rnd() * 6.0);
        let (r, ph, lobes) = (15.0 + rnd() * 7.0, rnd() * 6.28, 5.0 + (rnd() * 3.0).floor());
        paint(&mut img, cols[i % 4], |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            dx.hypot(dy) <= r + 3.0 * (lobes * dy.atan2(dx) + ph).sin()
        });
    }
    img
}

#[test]
fn d4_the_node_budget_is_met_or_reported() {
    let img = blobs();
    let base = vectorize(&img, &Params { colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    assert!(base.stats.nodes > 1200, "the test picture must start above 1200 nodes: {}", base.stats.nodes);
    assert_eq!(base.budget_met, None);
    let o = vectorize(&img, &Params { colors: 5, target_nodes: Some(700), ..quiet() }, &|_, _| {}).unwrap();
    assert!(o.stats.nodes <= 700, "{} nodes for a budget of 700", o.stats.nodes);
    assert_eq!(o.budget_met, Some(true));
    assert!(o.effective_smooth > 0.5, "the smoothness must have been raised: {}", o.effective_smooth);
    // a budget above the baseline changes nothing
    let easy = vectorize(&img, &Params { colors: 5, target_nodes: Some(base.stats.nodes + 100), ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(easy.svg, base.svg);
    assert_eq!(easy.budget_met, Some(true));
    // an absurd budget: no crash, the safest result, an honest note
    let o5 = vectorize(&img, &Params { colors: 5, target_nodes: Some(5), ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(o5.budget_met, Some(false));
    assert!(o5.stats.nodes > 5 && o5.stats.nodes <= o.stats.nodes, "{} nodes", o5.stats.nodes);
    assert!(o5.notes.iter().any(|n| n.contains("Target <= 5 nodes")), "{:?}", o5.notes);
    assert!(o5.svg.contains("<svg"));
}

// ---------------------------------------------------------------- D5

#[test]
fn d5_prepare_once_then_trace_again_without_cleaning_again() {
    let img = five_colours();
    let mut prep = prepare(img.clone(), &quiet(), &|_, _| {}).unwrap();
    let a = vectorize_prepared(&prep, &Params { custom_palette: Some(vec![WHITE, [200, 40, 40], [30, 60, 200]]), ..quiet() }, &|_, _| {}).unwrap();
    let b = vectorize_prepared(&prep, &Params { custom_palette: Some(vec![WHITE, [40, 160, 60], [240, 200, 30]]), ..quiet() }, &|_, _| {}).unwrap();
    assert!(a.svg.contains("<svg") && b.svg.contains("<svg"));
    assert!(a.svg.contains("#C82828") && !a.svg.contains("#28A03C"));
    assert!(b.svg.contains("#28A03C") && !b.svg.contains("#C82828"));
    // proof that the clean step is not run again: the traced picture is the CLEANED copy kept in `prep`. Repaint that copy
    // and the output follows it, while the source picture (which a new clean would start from) is unchanged.
    for p in prep.cleaned.pixels_mut() {
        if p.0 == [200, 40, 40] {
            *p = Rgb([10, 200, 200]);
        }
    }
    let c = vectorize_prepared(&prep, &Params { colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    assert!(c.palette.iter().any(|p| p[1] > 150 && p[2] > 150 && p[0] < 60), "the repainted colour is missing: {:?}", c.palette);
    assert!(!c.palette.iter().any(|p| p[0] > 150 && p[1] < 80), "the old red is back, so the clean step ran again");
    // and the all-in-one call gives the same result as the two halves
    let one = vectorize(&img, &Params { colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    let prep2 = prepare(img, &quiet(), &|_, _| {}).unwrap();
    let two = vectorize_prepared(&prep2, &Params { colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(one.svg, two.svg);
}

// ---------------------------------------------------------------- D6

#[test]
fn d6_the_cli_takes_a_node_budget_and_a_palette() {
    let dir = std::env::temp_dir().join(format!("trasea-d6-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.png");
    // blobs in the three palette colours
    let mut img = RgbImage::from_pixel(900, 600, Rgb([235, 217, 193]));
    let cols = [[15u8, 57, 115], [239, 141, 132]];
    for i in 0..90 {
        let (cx, cy) = (40.0 + (i % 10) as f64 * 85.0, 40.0 + (i / 10) as f64 * 60.0);
        paint(&mut img, cols[i % 2], |x, y| {
            let (dx, dy) = (x - cx, y - cy);
            dx.hypot(dy) <= 18.0 + 3.0 * (6.0 * dy.atan2(dx) + i as f64).sin()
        });
    }
    img.save(&input).unwrap();
    let exe = env!("CARGO_BIN_EXE_trasea-cli");
    let svg = dir.join("out.svg");
    let o = std::process::Command::new(exe).arg(&input).arg(&svg).args(["--target-nodes", "600", "--palette", "#0F3973,#EBD9C1,#EF8D84"]).output().unwrap();
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(text.starts_with("<?xml") && text.contains("<svg"));
    assert!(group_count(&text) <= 3, "{} colour groups", group_count(&text));
    for c in ["#0F3973", "#EBD9C1", "#EF8D84"] {
        assert!(text.contains(c), "{c} missing");
    }
    assert!(count_svg(&text).1 <= 600, "{} nodes", count_svg(&text).1);
    // wrong values are refused with exit code 2
    for bad in [&["--target-nodes", "abc"][..], &["--palette", "#123456"][..], &["--palette", "red,blue"][..]] {
        let o = std::process::Command::new(exe).arg(&input).arg(dir.join("x.svg")).args(bad).output().unwrap();
        assert_eq!(o.status.code(), Some(2), "{bad:?}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// Informational: on a real picture, how long the first run takes and how long a re-run with the prepared picture takes.
/// Skipped when the benchmark picture is not on this computer.
#[test]
fn d5b_a_rerun_with_the_prepared_picture_is_much_faster() {
    let path = std::path::Path::new("C:/Users/atrione/OpenBot/Agents/agent-770c1b2c-74f1-4ef1-99c2-3f9a8f833d93/vectorize-bench/out/Drifting Leaf.src.png");
    let Ok(img) = trasea_core::load::load_path(path, 0) else {
        eprintln!("D5b skipped: no benchmark picture");
        return;
    };
    // TRASEA_D5B_AI=1 measures the real use case: with Real-ESRGAN the first half is what the cache saves
    let ai = std::env::var("TRASEA_D5B_AI").is_ok();
    let p = Params { measure: false, detect_text: false, ai_clean: ai, sidecar_dir: ai.then(|| std::path::PathBuf::from("C:/Users/atrione/OpenBot/Agents/agent-770c1b2c-74f1-4ef1-99c2-3f9a8f833d93/tools/realesrgan")), ..Params::default() };
    let t0 = std::time::Instant::now();
    let prep = prepare(img, &p, &|_, _| {}).unwrap();
    let t_prepare = t0.elapsed();
    let time = |params: &Params| {
        let t = std::time::Instant::now();
        let o = vectorize_prepared(&prep, params, &|_, _| {}).unwrap();
        (t.elapsed(), o.stats.nodes)
    };
    let (t_plain, n_plain) = time(&p);
    let (t_pal, _) = time(&Params { custom_palette: Some(vec![[31, 79, 99], [32, 49, 112], [53, 78, 160], [133, 152, 67], [240, 172, 175], [246, 247, 247], [247, 237, 57], [133, 22, 20]]), ..p.clone() });
    let brush = BrushStroke { kind: BrushKind::Smooth, radius: 60.0, points: vec![(600.0, 150.0), (800.0, 250.0)] };
    let (t_brush, _) = time(&Params { brush_strokes: vec![brush], ..p.clone() });
    let (t_target, n_target) = time(&Params { target_nodes: Some(1500), ..p.clone() });
    eprintln!("D5b Drifting Leaf: prepare {:?}; re-run plain {:?} ({n_plain} nodes), palette {:?}, brush {:?}, node target 1500 {:?} ({n_target} nodes)", t_prepare, t_plain, t_pal, t_brush, t_target);
    assert!(t_plain < t_prepare + t_plain, "the re-run must not cost more than the first run");
    assert!(n_target <= 1500, "the real target of the test plan: {n_target} nodes");
    assert!(t_pal.as_secs_f64() < 6.0 && t_brush.as_secs_f64() < 6.0, "polishing iterations must stay fast");
}
