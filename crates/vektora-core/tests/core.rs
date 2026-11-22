//! Automatic tests V1-V7 (see the test plan). Synthetic images only: no test data from outside the repo.

use image::{Rgb, RgbImage};
use std::collections::BTreeSet;
use vektora_core::stats::{count_svg, render_svg};
use vektora_core::{export, hex, vectorize, Params};

fn lerp3(a: [u8; 3], b: [u8; 3], t: f32) -> Rgb<u8> {
    Rgb([0, 1, 2].map(|i| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t).round() as u8))
}

/// Flat illustration: yellow field, a blue circle, a red rectangle; edges are anti-aliased (1 px blend).
fn flat_scene(w: u32, h: u32) -> RgbImage {
    let (yellow, blue, red) = ([250, 205, 50], [40, 110, 250], [230, 60, 60]);
    let mut img = RgbImage::new(w, h);
    let (cx, cy, r) = (w as f32 * 0.35, h as f32 * 0.5, h as f32 * 0.3);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 + 0.5 - cx).powi(2) + (y as f32 + 0.5 - cy).powi(2)).sqrt();
            let in_circle = (r - d + 0.5).clamp(0.0, 1.0);
            let rx = ((x as f32 + 0.5) - w as f32 * 0.6).abs();
            let ry = ((y as f32 + 0.5) - h as f32 * 0.5).abs();
            let in_rect = (w as f32 * 0.15 - rx + 0.5).clamp(0.0, 1.0).min((h as f32 * 0.25 - ry + 0.5).clamp(0.0, 1.0));
            let mut c = lerp3(yellow, blue, in_circle);
            c = lerp3([c[0], c[1], c[2]], red, in_rect);
            img.put_pixel(x, y, c);
        }
    }
    img
}

fn params() -> Params {
    Params { ai_clean: false, measure: false, ..Params::default() }
}

fn fills(svg: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for part in svg.split("fill=\"").skip(1) {
        out.insert(part[..part.find('"').unwrap()].to_uppercase());
    }
    out
}

fn group_ids(svg: &str) -> Vec<String> {
    svg.split("<g id=\"").skip(1).map(|p| p[..p.find('"').unwrap()].to_string()).collect()
}

#[test]
fn v1_svg_is_clean_and_parses() {
    let img = flat_scene(300, 200);
    let o = vectorize(&img, &Params { keep_lines: false, ..params() }, &|_, _| {}).unwrap();
    // parses
    let tree = resvg::usvg::Tree::from_str(&o.svg, &resvg::usvg::Options::default()).expect("usvg must parse the SVG");
    assert_eq!((tree.size().width() as u32, tree.size().height() as u32), (300, 200));
    // nothing an editor would stumble on
    for bad in ["<style", "class=", "filter", "transform", "<defs", "<use", "opacity"] {
        assert!(!o.svg.contains(bad), "SVG contains {bad}");
    }
    assert!(o.svg.contains("viewBox=\"0 0 300 200\""));
    // one group per colour, id = hex of the colour, fill set directly on the path
    let ids = group_ids(&o.svg);
    let unique: BTreeSet<_> = ids.iter().collect();
    assert_eq!(ids.len(), unique.len(), "group ids must be unique");
    assert_eq!(ids.len(), o.palette.len().min(ids.len()));
    for id in &ids {
        assert!(id.starts_with('#') && id.len() == 7, "bad group id {id}");
        assert!(o.svg.contains(&format!("fill=\"{id}\"")));
    }
}

#[test]
fn v2_pdf_opens_and_has_the_right_page_size() {
    let img = flat_scene(300, 200);
    let o = vectorize(&img, &params(), &|_, _| {}).unwrap();
    let pdf = export::svg_to_pdf(&o.svg).expect("PDF export");
    assert!(pdf.starts_with(b"%PDF-"));
    let doc = lopdf::Document::load_mem(&pdf).expect("a PDF reader must open it");
    let pages = doc.get_pages();
    assert_eq!(pages.len(), 1);
    let page = doc.get_dictionary(*pages.values().next().unwrap()).unwrap();
    let mb = page.get(b"MediaBox").unwrap().as_array().unwrap();
    let n: Vec<f32> = mb.iter().map(|o| o.as_float().unwrap()).collect();
    assert_eq!((n[2] - n[0]).round() as u32, 300);
    assert_eq!((n[3] - n[1]).round() as u32, 200);
}

#[test]
fn v3_palette_has_at_most_n_colours_and_no_halo_colours() {
    let img = flat_scene(300, 200);
    for n in [3usize, 4, 8] {
        let o = vectorize(&img, &Params { colors: n, keep_lines: false, ..params() }, &|_, _| {}).unwrap();
        assert!(o.palette.len() <= n, "asked {n}, got {}", o.palette.len());
    }
    let o = vectorize(&img, &Params { colors: 8, keep_lines: false, ..params() }, &|_, _| {}).unwrap();
    // the image has exactly three real colours; every drawn colour must be close to one of them (no blends)
    let truth = [[250i32, 205, 50], [40, 110, 250], [230, 60, 60]];
    for c in &o.palette {
        let near = truth.iter().any(|t| (0..3).map(|i| (t[i] - c[i] as i32).abs()).sum::<i32>() < 30);
        assert!(near, "halo colour {} in the palette", hex(*c));
    }
    // and the SVG only uses palette colours
    let used: BTreeSet<String> = fills(&o.svg);
    let pal: BTreeSet<String> = o.palette.iter().map(|c| hex(*c)).collect();
    assert!(used.is_subset(&pal), "SVG uses colours outside the palette: {:?}", used.difference(&pal).collect::<Vec<_>>());
}

#[test]
fn v4_thin_lines_are_kept() {
    // black 1-3 px lines on top of colour fields
    let mut img = flat_scene(400, 300);
    let mut truth = vec![false; 400 * 300];
    let mut draw = |x0: i32, y0: i32, x1: i32, y1: i32, wdt: i32| {
        let steps = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
        for s in 0..=steps {
            let x = x0 + (x1 - x0) * s / steps;
            let y = y0 + (y1 - y0) * s / steps;
            for dy in 0..wdt {
                for dx in 0..wdt {
                    let (px, py) = (x + dx, y + dy);
                    if px >= 0 && py >= 0 && px < 400 && py < 300 {
                        img.put_pixel(px as u32, py as u32, Rgb([15, 15, 15]));
                        truth[py as usize * 400 + px as usize] = true;
                    }
                }
            }
        }
    };
    draw(40, 60, 180, 70, 1); // 1 px
    draw(60, 220, 200, 250, 2); // 2 px
    draw(250, 40, 360, 120, 3); // 3 px
    draw(30, 150, 120, 150, 2);
    let o = vectorize(&img, &params(), &|_, _| {}).unwrap();
    let r = render_svg(&o.svg, 400, 300).unwrap();
    let total = truth.iter().filter(|&&t| t).count();
    let mut hit = 0;
    for y in 0..300i32 {
        for x in 0..400i32 {
            if !truth[(y * 400 + x) as usize] {
                continue;
            }
            // dark in the render within 1 px
            let dark = (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    let (xx, yy) = ((x + dx).clamp(0, 399), (y + dy).clamp(0, 299));
                    let p = r.get_pixel(xx as u32, yy as u32);
                    (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32) < 110.0
                })
            });
            if dark {
                hit += 1;
            }
        }
    }
    let share = hit as f64 / total as f64;
    assert!(share >= 0.90, "only {:.0}% of the line pixels are covered", share * 100.0);
}

fn wavy_scene(w: u32, h: u32) -> RgbImage {
    // curved boundaries (a sine "sea" and two circles) so node counts depend on contour length, not on straight edges
    let (sand, sea, boat) = ([235, 215, 190], [15, 57, 115], [239, 141, 132]);
    let mut img = RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let fx = x as f32 / w as f32;
            let fy = y as f32 / h as f32;
            let edge = 0.45 + 0.06 * (fx * 12.0).sin();
            let mut c = if fy < edge { sand } else { sea };
            let d1 = ((fx - 0.3).powi(2) + (fy - 0.75).powi(2)).sqrt();
            let d2 = ((fx - 0.7).powi(2) + (fy - 0.7).powi(2)).sqrt();
            if d1 < 0.12 || d2 < 0.08 {
                c = boat;
            }
            img.put_pixel(x, y, Rgb(c));
        }
    }
    img
}

#[test]
fn v5_threshold_scaling_keeps_nodes_flat_when_the_input_is_bigger() {
    let small = wavy_scene(600, 400);
    let big = image::imageops::resize(&small, 1200, 800, image::imageops::FilterType::Lanczos3);
    let n1 = vectorize(&small, &params(), &|_, _| {}).unwrap().stats.nodes;
    let n2 = vectorize(&big, &params(), &|_, _| {}).unwrap().stats.nodes;
    assert!(n2 as f64 <= n1 as f64 * 1.3, "2x input gave {n2} nodes vs {n1} for 1x");
}

#[test]
fn v6_missing_sidecar_falls_back_with_a_clear_message() {
    let img = flat_scene(200, 150);
    let o = vectorize(&img, &Params { ai_clean: true, sidecar_dir: None, measure: false, ..Params::default() }, &|_, _| {}).unwrap();
    assert!(!o.used_ai);
    assert!(o.notes.iter().any(|n| n.contains("AI cleaning was skipped") && n.contains("median")), "notes: {:?}", o.notes);
    assert!(!o.svg.is_empty());
    // a folder that exists but has no sidecar in it
    let empty = std::env::temp_dir().join("vektora-empty-sidecar");
    std::fs::create_dir_all(&empty).unwrap();
    let o = vectorize(&img, &Params { ai_clean: true, sidecar_dir: Some(empty), measure: false, ..Params::default() }, &|_, _| {}).unwrap();
    assert!(!o.used_ai && o.notes.iter().any(|n| n.contains("not found")), "notes: {:?}", o.notes);
}

#[test]
fn v7_no_network_code_is_linked() {
    // The lock file may list crates that only other platforms or dev tooling need, so ask cargo for what is really
    // compiled into the Windows build of the core and of the app.
    let network = ["reqwest", "hyper", "ureq", "curl", "isahc", "surf", "attohttpc", "tungstenite", "tokio-tungstenite", "native-tls", "openssl", "rustls", "h2"];
    for pkg in ["vektora-core", "vektora"] {
        let out = std::process::Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()))
            .args(["tree", "-p", pkg, "--target", "x86_64-pc-windows-gnu", "-e", "normal,build", "--prefix", "none", "--workspace"])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("cargo tree");
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.trim().is_empty(), "cargo tree gave nothing for {pkg}: {}", String::from_utf8_lossy(&out.stderr));
        for line in text.lines() {
            let name = line.split_whitespace().next().unwrap_or("");
            assert!(!network.contains(&name), "network crate {name} is in the dependency tree of {pkg}");
        }
    }
    assert!(count_svg("<path d=\"M0,0C1,1 2,2 3,3Z\"/>").1 == 2);
}
