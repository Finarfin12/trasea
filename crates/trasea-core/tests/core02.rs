//! Automatic tests W1-W6 for 0.2.0 (W7 = the 0.1.0 tests in core.rs). Synthetic images only.

use image::{Rgb, RgbImage};
use trasea_core::centerline::centerline;
use trasea_core::export::{palette_of, to_ase, to_svg, ExportOptions, LinesAs};
use trasea_core::stats::{count_svg, render_svg};
use trasea_core::trace::{trace_mask, TraceParams};
use trasea_core::{hex, vectorize, Params};
use visioncortex::BinaryImage;

fn nodes_of_contours(c: &[trasea_core::trace::Contour]) -> usize {
    c.iter().map(|k| 1 + k.curves.len()).sum()
}

fn disk_line(mask: &mut [bool], w: usize, pts: &[(f64, f64)], width: f64) {
    // paint a thick polyline by stamping discs
    for seg in pts.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let n = (((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt() * 2.0).ceil() as usize + 1;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let (cx, cy) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            let r = width / 2.0;
            for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
                for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                    if ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt() <= r {
                        mask[y as usize * w + x as usize] = true;
                    }
                }
            }
        }
    }
}

fn outline_nodes(mask: &[bool], w: usize, h: usize) -> usize {
    let mut bin = BinaryImage::new_w_h(w, h);
    for (i, &m) in mask.iter().enumerate() {
        if m {
            bin.set_pixel(i % w, i / w, true);
        }
    }
    nodes_of_contours(&trace_mask(&bin, &TraceParams { corner_deg: 65.0, sigma: 1.4, tol: 1.0, min_area: 4 }))
}

#[test]
fn w1_centerline_gives_one_stroke_with_the_measured_width() {
    let (w, h) = (240usize, 160usize);
    for width in [1.0f64, 2.0, 3.0, 4.0] {
        // straight line
        let mut m = vec![false; w * h];
        for y in 80..80 + width as usize {
            for x in 20..220 {
                m[y * w + x] = true;
            }
        }
        let r = centerline(&m, w, h, 1.0, 0.5);
        assert!(r.shape_mask.iter().all(|&b| !b), "width {width}: a plain line must not fall back to a shape");
        assert_eq!(r.strokes.len(), 1, "width {width}: one stroke per line");
        assert!((r.strokes[0].width - width).abs() <= 0.5, "width {width}: measured {}", r.strokes[0].width);
        let stroke_nodes: usize = 1 + r.strokes[0].curves.len();
        assert!(stroke_nodes * 3 <= outline_nodes(&m, w, h).max(stroke_nodes * 3), "nodes");
    }
    // curved line: one stroke, far fewer nodes than its outline
    let mut m = vec![false; w * h];
    let arc: Vec<(f64, f64)> = (0..=60).map(|i| { let a = i as f64 / 60.0 * std::f64::consts::PI; (120.0 - 90.0 * a.cos(), 130.0 - 90.0 * a.sin()) }).collect();
    disk_line(&mut m, w, &arc, 3.0);
    // the pipeline works at ~2x the source size with a tolerance of about 2.7 working px at the default smoothness
    let r = centerline(&m, w, h, 3.0, 0.5);
    assert_eq!(r.strokes.len(), 1);
    assert!((r.strokes[0].width - 3.0).abs() <= 0.5, "arc width {}", r.strokes[0].width);
    let stroke_nodes = 1 + r.strokes[0].curves.len();
    let outline = outline_nodes(&m, w, h);
    assert!(stroke_nodes * 3 <= outline, "stroke {stroke_nodes} nodes vs outline {outline}");
    // a Y branch: three strokes meeting at one point
    let mut m = vec![false; w * h];
    disk_line(&mut m, w, &[(120.0, 140.0), (120.0, 80.0)], 3.0);
    disk_line(&mut m, w, &[(120.0, 80.0), (70.0, 30.0)], 3.0);
    disk_line(&mut m, w, &[(120.0, 80.0), (170.0, 30.0)], 3.0);
    let r = centerline(&m, w, h, 1.0, 0.5);
    assert_eq!(r.strokes.len(), 3, "a Y has three branches");
    // a fat blob is not a stroke
    let mut m = vec![false; w * h];
    disk_line(&mut m, w, &[(100.0, 80.0), (110.0, 80.0)], 30.0);
    let r = centerline(&m, w, h, 1.0, 0.5);
    assert!(r.strokes.is_empty() && r.shape_mask.iter().any(|&b| b), "a blob must stay a shape");
}

fn scene_with_lines() -> RgbImage {
    let mut img = RgbImage::from_pixel(400, 300, Rgb([250, 205, 50]));
    for y in 60..240 {
        for x in 60..340 {
            let d = (((x as f32 - 200.0).powi(2) + (y as f32 - 150.0).powi(2)) as f32).sqrt();
            if d < 90.0 {
                img.put_pixel(x, y, Rgb([40, 110, 250]));
            }
        }
    }
    let mut line = |x0: i32, y0: i32, x1: i32, y1: i32, wdt: i32| {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let (x, y) = (x0 + (x1 - x0) * s / steps, y0 + (y1 - y0) * s / steps);
            for dy in 0..wdt {
                for dx in 0..wdt {
                    img.put_pixel((x + dx) as u32, (y + dy) as u32, Rgb([15, 15, 15]));
                }
            }
        }
    };
    line(30, 40, 170, 50, 2);
    line(150, 130, 260, 170, 3);
    line(220, 250, 360, 270, 2);
    img
}

fn params() -> Params {
    Params { ai_clean: false, measure: false, ..Params::default() }
}

#[test]
fn w2_svg_has_named_ordered_layers_and_stroke_paths() {
    let o = vectorize(&scene_with_lines(), &params(), &|_, _| {}).unwrap();
    let svg = &o.svg;
    for bad in ["<style", "class=", "filter"] {
        assert!(!svg.contains(bad), "SVG contains {bad}");
    }
    let (a, b, c) = (svg.find("id=\"Latar\"").unwrap(), svg.find("id=\"Bidang_warna\"").unwrap(), svg.find("id=\"Garis\"").unwrap());
    assert!(a < b && b < c, "layers must be ordered Latar, Bidang warna, Garis");
    assert!(svg.contains("data-name=\"Latar\"") && svg.contains("data-name=\"Bidang warna\"") && svg.contains("data-name=\"Garis\""));
    // one sub-group per colour with a readable data-name
    let colours = palette_of(&o.doc);
    for (i, col) in colours.iter().enumerate() {
        assert!(svg.contains(&format!("data-name=\"Warna {:02} {}\"", i + 1, hex(*col))), "missing group for colour {}", i + 1);
        assert_eq!(svg.matches(&format!("data-name=\"Warna {:02} ", i + 1)).count(), 1);
    }
    // lines are stroked paths without fill
    let garis = &svg[c..];
    let stroked: Vec<&str> = garis.lines().filter(|l| l.contains("stroke-width")).collect();
    assert!(!stroked.is_empty(), "no stroked line paths");
    for l in &stroked {
        assert!(l.contains("fill=\"none\"") && l.contains("stroke=\"#") && l.contains("stroke-linecap=\"round\"") && l.contains("stroke-linejoin=\"round\""), "bad stroke path: {l}");
    }
    // "lines as shapes" gives filled paths instead
    let shapes = to_svg(&o.doc, &ExportOptions { lines_as: LinesAs::Shape, trap_pt: 0.0, ..Default::default() });
    assert!(!shapes[shapes.find("id=\"Garis\"").unwrap()..].contains("stroke-width"));
    // strokes need far fewer nodes than the outlines of the same lines
    let g_nodes = |s: &str| { let g = &s[s.find("id=\"Garis\"").unwrap()..]; count_svg(g).1 };
    assert!(g_nodes(svg) * 2 <= g_nodes(&shapes), "stroke nodes {} vs shape nodes {}", g_nodes(svg), g_nodes(&shapes));
}

/// Minimal ASE reader for the test.
fn read_ase(b: &[u8]) -> (String, Vec<(String, [f32; 3], u16)>) {
    assert_eq!(&b[0..4], b"ASEF");
    assert_eq!(u16::from_be_bytes([b[4], b[5]]), 1);
    let blocks = u32::from_be_bytes([b[8], b[9], b[10], b[11]]) as usize;
    let mut pos = 12;
    let mut group = String::new();
    let mut colours = Vec::new();
    let utf16 = |bytes: &[u8]| -> String {
        let n = u16::from_be_bytes([bytes[0], bytes[1]]) as usize;
        let units: Vec<u16> = (0..n - 1).map(|i| u16::from_be_bytes([bytes[2 + 2 * i], bytes[3 + 2 * i]])).collect();
        String::from_utf16(&units).unwrap()
    };
    for _ in 0..blocks {
        let ty = u16::from_be_bytes([b[pos], b[pos + 1]]);
        let len = u32::from_be_bytes([b[pos + 2], b[pos + 3], b[pos + 4], b[pos + 5]]) as usize;
        let body = &b[pos + 6..pos + 6 + len];
        match ty {
            0xC001 => group = utf16(body),
            0x0001 => {
                let nlen = u16::from_be_bytes([body[0], body[1]]) as usize;
                let name = utf16(body);
                let off = 2 + nlen * 2;
                assert_eq!(&body[off..off + 4], b"RGB ");
                let f = |k: usize| f32::from_be_bytes([body[off + 4 + 4 * k], body[off + 5 + 4 * k], body[off + 6 + 4 * k], body[off + 7 + 4 * k]]);
                let ctype = u16::from_be_bytes([body[off + 16], body[off + 17]]);
                colours.push((name, [f(0), f(1), f(2)], ctype));
            }
            0xC002 => {}
            other => panic!("unknown block {other:#x}"),
        }
        pos += 6 + len;
    }
    assert_eq!(pos, b.len(), "no trailing bytes");
    (group, colours)
}

#[test]
fn w3_ase_has_the_palette_with_the_right_names() {
    let o = vectorize(&scene_with_lines(), &params(), &|_, _| {}).unwrap();
    let pal = palette_of(&o.doc);
    let (group, colours) = read_ase(&to_ase(&pal));
    assert_eq!(group, "Trasea");
    assert_eq!(colours.len(), pal.len());
    for (i, ((name, rgb, ctype), c)) in colours.iter().zip(&pal).enumerate() {
        assert_eq!(name, &format!("Trasea {:02} {}", i + 1, hex(*c)));
        for k in 0..3 {
            assert!((rgb[k] - c[k] as f32 / 255.0).abs() < 1e-6);
        }
        assert_eq!(*ctype, 0, "global colour");
    }
}

#[test]
fn w4_no_background_shows_between_touching_fields() {
    // four vertical bands that touch each other; the first (left) one is the background layer
    let (w, h) = (400u32, 200u32);
    let cols = [[200u8, 30, 30], [30, 160, 60], [30, 60, 200], [240, 200, 40]];
    let mut img = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            // slightly slanted edges so the boundaries are not pixel aligned
            let xs = x as f32 + (y as f32) * 0.07;
            let band = ((xs / 100.0).floor() as usize).min(3);
            img.put_pixel(x, y, Rgb(cols[band]));
        }
    }
    let o = vectorize(&img, &Params { keep_lines: false, colors: 6, ..params() }, &|_, _| {}).unwrap();
    // paint the background colour magenta: any magenta pixel inside the other bands is a seam
    let mut doc = o.doc.clone();
    doc.fields[0].color = [255, 0, 255];
    let svg = to_svg(&doc, &ExportOptions::default());
    let r = render_svg(&svg, w, h).unwrap();
    // the magenta band is the original background band; look right of it
    let bg_cols = o.doc.fields[0].color;
    let band_x = cols.iter().position(|c| *c == bg_cols).unwrap_or(0);
    let mut seam = 0;
    let mut total = 0;
    for y in 0..h {
        for x in 0..w {
            let xs = x as f32 + (y as f32) * 0.07;
            let band = ((xs / 100.0).floor() as usize).min(3);
            if band == band_x {
                continue;
            }
            total += 1;
            let p = r.get_pixel(x, y);
            if p[0] > 200 && p[1] < 90 && p[2] > 200 {
                seam += 1;
            }
        }
    }
    assert!(total > 0);
    assert!(seam <= 2, "{seam} background pixels show through between the fields");
}

#[test]
fn w5_trap_grows_the_fields_by_a_measurable_amount() {
    let img = scene_with_lines();
    let o = vectorize(&img, &Params { keep_lines: false, ..params() }, &|_, _| {}).unwrap();
    let plain = to_svg(&o.doc, &ExportOptions::default());
    let trapped = to_svg(&o.doc, &ExportOptions { lines_as: LinesAs::Stroke, trap_pt: 1.0, ..Default::default() });
    assert!(!plain.contains("stroke-linejoin=\"round\"") || !plain.contains("stroke-width"));
    assert!(trapped.contains("stroke-width=\"1.33\""), "trap 1 pt must be 1.33 px");
    // blue disc: count the pixels of its colour in both renders
    let (a, b) = (render_svg(&plain, 400, 300).unwrap(), render_svg(&trapped, 400, 300).unwrap());
    let blue = |im: &RgbImage| im.pixels().filter(|p| p[2] > 200 && p[0] < 90).count();
    let (na, nb) = (blue(&a), blue(&b));
    // perimeter 2*pi*90 = 565 px; the stroke adds about 0.67 px on each side => ~380 px more
    assert!(nb > na + 200, "trap added only {} px (plain {na}, trapped {nb})", nb as i64 - na as i64);
}

#[test]
fn w6_ai_clean_is_off_by_default() {
    assert!(!Params::default().ai_clean);
}
