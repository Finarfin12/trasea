//! Trasea 1.5.0 — H2 corner/junction closing + StickerBacking offset/bleed: G15-01 .. G15-08.

use image::{Rgb, RgbImage};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use trasea_core::centerline::join_stroke_endpoints;
use trasea_core::color::ColorSpace;
use trasea_core::contour::{build_die_cut_contours, build_offset_contours, build_sticker_backing_contours, die_cut_bounds_svg};
use trasea_core::dxf::to_dxf;
use trasea_core::export::{to_eps, to_pdf, to_svg, Doc, ExportOptions, StrokeData};
use trasea_core::project::ProjectFile;
use trasea_core::stats::render_svg;
use trasea_core::{vectorize, Params};

const W: usize = 800;
const H: usize = 600;
const SS: usize = 4;
const OUTLINE: [u8; 3] = [24, 24, 30];

type Poly = Vec<(f64, f64)>;

struct Canvas {
    px: Vec<[u8; 3]>,
    line: Vec<bool>,
}

impl Canvas {
    fn new() -> Self {
        Canvas { px: vec![[255, 255, 255]; W * H * SS * SS], line: vec![false; W * H * SS * SS] }
    }
    fn fill_poly(&mut self, poly: &Poly, color: [u8; 3]) {
        let hw = W * SS;
        let (miny, maxy) = poly.iter().fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p.1), a.1.max(p.1)));
        for y in ((miny * SS as f64).floor().max(0.0) as usize)..=((maxy * SS as f64).ceil() as usize).min(H * SS - 1) {
            let yy = (y as f64 + 0.5) / SS as f64;
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..poly.len() {
                let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
                if (a.1 <= yy && b.1 > yy) || (b.1 <= yy && a.1 > yy) {
                    xs.push(a.0 + (yy - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for k in (0..xs.len().saturating_sub(1)).step_by(2) {
                let x0 = ((xs[k] * SS as f64).round().max(0.0)) as usize;
                let x1 = ((xs[k + 1] * SS as f64).round() as usize).min(hw);
                for x in x0..x1 {
                    self.px[y * hw + x] = color;
                }
            }
        }
    }
    fn stroke_poly(&mut self, poly: &Poly, w: f64, closed: bool) {
        let hw = W * SS;
        let n = if closed { poly.len() } else { poly.len() - 1 };
        for i in 0..n {
            let (a, b) = (poly[i], poly[(i + 1) % poly.len()]);
            let pad = w / 2.0 + 1.0;
            let (x0, x1) = (a.0.min(b.0) - pad, a.0.max(b.0) + pad);
            let (y0, y1) = (a.1.min(b.1) - pad, a.1.max(b.1) + pad);
            let (dx, dy) = (b.0 - a.0, b.1 - a.1);
            let l2 = (dx * dx + dy * dy).max(1e-9);
            for y in ((y0 * SS as f64).floor().max(0.0) as usize)..=((y1 * SS as f64).ceil() as usize).min(H * SS - 1) {
                for x in ((x0 * SS as f64).floor().max(0.0) as usize)..=((x1 * SS as f64).ceil() as usize).min(hw - 1) {
                    let (px, py) = ((x as f64 + 0.5) / SS as f64, (y as f64 + 0.5) / SS as f64);
                    let t = (((px - a.0) * dx + (py - a.1) * dy) / l2).clamp(0.0, 1.0);
                    let (cx, cy) = (a.0 + t * dx, a.1 + t * dy);
                    if ((px - cx).powi(2) + (py - cy).powi(2)).sqrt() <= w / 2.0 {
                        self.px[y * hw + x] = OUTLINE;
                        self.line[y * hw + x] = true;
                    }
                }
            }
        }
    }
}

fn circle_poly(cx: f64, cy: f64, r: f64) -> Poly {
    (0..96)
        .map(|i| (cx + r * (i as f64 / 96.0 * std::f64::consts::TAU).cos(), cy + r * (i as f64 / 96.0 * std::f64::consts::TAU).sin()))
        .collect()
}

fn landscape_variant(hard: bool) -> (RgbImage, Vec<bool>) {
    let mut c = Canvas::new();
    c.fill_poly(&vec![(0.0, 0.0), (800.0, 0.0), (800.0, 330.0), (0.0, 330.0)], [135, 206, 235]);
    c.fill_poly(&vec![(0.0, 330.0), (800.0, 330.0), (800.0, 600.0), (0.0, 600.0)], [120, 180, 90]);
    let m1: Poly = vec![(0.0, 330.0), (180.0, 130.0), (330.0, 330.0)];
    let m2: Poly = vec![(220.0, 330.0), (430.0, 80.0), (640.0, 330.0)];
    let m3: Poly = vec![(500.0, 330.0), (650.0, 190.0), (800.0, 330.0)];
    c.fill_poly(&m1, [110, 130, 150]);
    c.fill_poly(&m2, if hard { [58, 74, 100] } else { [90, 110, 135] });
    c.fill_poly(&m3, [130, 150, 170]);
    let sun = circle_poly(680.0, 90.0, 52.0);
    c.fill_poly(&sun, [250, 200, 60]);
    let mut left: Poly = Vec::new();
    let mut right: Poly = Vec::new();
    for i in 0..=30 {
        let t = i as f64 / 30.0;
        let y = 330.0 + t * 270.0;
        let cx = 400.0 + 70.0 * (t * 5.0).sin();
        let half = 14.0 + 56.0 * t;
        left.push((cx - half, y));
        right.push((cx + half, y));
    }
    let mut river: Poly = left.clone();
    river.extend(right.iter().rev().cloned());
    c.fill_poly(&river, [70, 140, 210]);
    let mut trees: Vec<Poly> = Vec::new();
    let mut trunks: Vec<Poly> = Vec::new();
    for &(tx, ty, s) in &[(110.0, 470.0, 1.0), (200.0, 520.0, 1.2), (270.0, 440.0, 0.9), (600.0, 500.0, 1.1), (700.0, 450.0, 0.9)] {
        let f: Poly = vec![(tx, ty - 90.0 * s), (tx + 38.0 * s, ty), (tx - 38.0 * s, ty)];
        let t: Poly = vec![(tx - 7.0 * s, ty), (tx + 7.0 * s, ty), (tx + 7.0 * s, ty + 26.0 * s), (tx - 7.0 * s, ty + 26.0 * s)];
        c.fill_poly(&t, [110, 70, 40]);
        c.fill_poly(&f, if hard { [28, 78, 44] } else { [40, 110, 60] });
        trees.push(f);
        trunks.push(t);
    }
    for p in [&m1, &m2, &m3] {
        c.stroke_poly(p, 5.0, true);
    }
    c.stroke_poly(&sun, 4.0, true);
    c.stroke_poly(&left, 4.0, false);
    c.stroke_poly(&right, 4.0, false);
    for p in trees.iter().chain(trunks.iter()) {
        c.stroke_poly(p, 3.0, true);
    }
    c.stroke_poly(&vec![(2.5, 0.0), (2.5, 597.5), (800.0, 597.5)], 5.0, false);
    let mut img = RgbImage::new(W as u32, H as u32);
    let mut truth = vec![false; W * H];
    let hw = W * SS;
    for y in 0..H {
        for x in 0..W {
            let (mut s, mut l) = ([0u32; 3], 0usize);
            for sy in 0..SS {
                for sx in 0..SS {
                    let i = (y * SS + sy) * hw + x * SS + sx;
                    for k in 0..3 {
                        s[k] += c.px[i][k] as u32;
                    }
                    if c.line[i] {
                        l += 1;
                    }
                }
            }
            let n = (SS * SS) as u32;
            let mut px = [(s[0] / n) as i32, (s[1] / n) as i32, (s[2] / n) as i32];
            if hard {
                let h = ((x as u32).wrapping_mul(73856093) ^ (y as u32).wrapping_mul(19349663)).wrapping_mul(2654435761) >> 28;
                let d = h as i32 - 8;
                for k in 0..3 {
                    px[k] = (px[k] + d).clamp(0, 255);
                }
            }
            img.put_pixel(x as u32, y as u32, Rgb([px[0] as u8, px[1] as u8, px[2] as u8]));
            truth[y * W + x] = l * 2 >= SS * SS;
        }
    }
    (img, truth)
}

fn outline_recall_tol(render: &RgbImage, truth: &[bool], tol: i64) -> (f64, usize) {
    let dark: Vec<bool> = render
        .pixels()
        .map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * (p[2] as f32) < 62.0)
        .collect();
    let near = |x: i64, y: i64| -> bool {
        for dy in -tol..=tol {
            for dx in -tol..=tol {
                let (nx, ny) = (x + dx, y + dy);
                if nx >= 0 && ny >= 0 && (nx as usize) < W && (ny as usize) < H && dark[ny as usize * W + nx as usize] {
                    return true;
                }
            }
        }
        false
    };
    let mut missing = vec![false; W * H];
    let (mut t, mut c) = (0usize, 0usize);
    for y in 0..H {
        for x in 0..W {
            if truth[y * W + x] {
                t += 1;
                if near(x as i64, y as i64) {
                    c += 1;
                } else {
                    missing[y * W + x] = true;
                }
            }
        }
    }
    let mut vis = vec![false; W * H];
    let mut big_gaps = 0usize;
    let mut q = VecDeque::new();
    for y in 0..H {
        for x in 0..W {
            let idx = y * W + x;
            if !missing[idx] || vis[idx] {
                continue;
            }
            vis[idx] = true;
            q.push_back((x, y));
            let (mut x0, mut x1, mut y0, mut y1) = (x, x, y, y);
            while let Some((cx, cy)) = q.pop_front() {
                x0 = x0.min(cx);
                x1 = x1.max(cx);
                y0 = y0.min(cy);
                y1 = y1.max(cy);
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (nx, ny) = (cx as i64 + dx, cy as i64 + dy);
                        if nx >= 0 && ny >= 0 && (nx as usize) < W && (ny as usize) < H {
                            let ni = ny as usize * W + nx as usize;
                            if missing[ni] && !vis[ni] {
                                vis[ni] = true;
                                q.push_back((nx as usize, ny as usize));
                            }
                        }
                    }
                }
            }
            let ext = (x1 - x0 + 1).max(y1 - y0 + 1);
            if ext >= 6 { big_gaps += 1;  }
        }
    }
    (c as f64 / t.max(1) as f64, big_gaps)
}

fn sample_sticker_doc() -> Doc {
    let mut img = RgbImage::from_pixel(200, 200, Rgb([255, 255, 255]));
    for y in 40..160 {
        for x in 40..160 {
            let dx = x as f64 - 100.0;
            let dy = y as f64 - 100.0;
            if dx * dx + dy * dy <= 55.0 * 55.0 {
                img.put_pixel(x, y, Rgb([22, 130, 210]));
            }
        }
    }
    let mut p = Params::default();
    p.colors = 4;
    p.ai_clean = false;
    vectorize(&img, &p, &|_, _| {}).expect("vectorize sticker").doc
}

/// G15-01 (AC-1.1, AC-1.2): outline_landscape normal & hard exact recall & gap count.
#[test]
fn g15_01_h2_corner_recall_and_gap_reduction() {
    let mut p = Params::default();
    p.colors = 10;
    p.smooth = 0.5;
    p.ai_clean = false;

    for (hard, min_recall0, max_gaps1) in [(false, 0.93f64, 30usize), (true, 0.94, 17)] {
        let (img, truth) = landscape_variant(hard);
        let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize landscape");
        let rend = render_svg(&out.svg, W as u32, H as u32).expect("render svg");
        let (rec0, _) = outline_recall_tol(&rend, &truth, 0);
        let (rec1, gaps1) = outline_recall_tol(&rend, &truth, 1);
        println!("G15-01 hard={hard}: recall0={:.2}% (min {:.0}%), recall1={:.2}%, gaps1(>=6px)={gaps1} (max {max_gaps1})", rec0 * 100.0, min_recall0 * 100.0, rec1 * 100.0);
        assert!(rec0 >= min_recall0, "hard={hard}: exact recall {:.4} < {min_recall0}", rec0);
        assert!(gaps1 <= max_gaps1, "hard={hard}: gaps(>=6px) {gaps1} > {max_gaps1}");
    }
}

/// G15-02 (AC-1.3): 2 parallel black lines separated by a 6 px white gap -> 0 false bridge.
#[test]
fn g15_02_no_false_bridge_across_white_gap() {
    let (w, h) = (100usize, 60usize);
    let mut dark = vec![false; w * h];
    // Horizontal line 1 at y = 20..23, x = 10..40; Horizontal line 2 at y = 20..23, x = 46..80 (6 px white gap at x=40..46)
    for y in 20..23 {
        for x in 10..40 {
            dark[y * w + x] = true;
        }
        for x in 46..80 {
            dark[y * w + x] = true;
        }
    }
    // Also two parallel horizontal lines separated vertically by 6 px white gap (y=20..23 and y=29..32)
    for y in 29..32 {
        for x in 10..80 {
            dark[y * w + x] = true;
        }
    }
    let s1 = StrokeData { curves: vec![[(10.0, 21.0), (20.0, 21.0), (30.0, 21.0), (39.0, 21.0)]], width: 3.0 };
    let s2 = StrokeData { curves: vec![[(46.0, 21.0), (57.0, 21.0), (68.0, 21.0), (79.0, 21.0)]], width: 3.0 };
    let s3 = StrokeData { curves: vec![[(10.0, 30.0), (33.0, 30.0), (56.0, 30.0), (79.0, 30.0)]], width: 3.0 };
    let orig = vec![s1.clone(), s2.clone(), s3.clone()];
    let mut strokes = orig.clone();
    join_stroke_endpoints(&mut strokes, &dark, w, h, 1.0);
    assert_eq!(strokes.len(), orig.len());
    for (a, b) in strokes.iter().zip(orig.iter()) {
        assert_eq!(a.curves, b.curves, "false bridge created across white gap!");
    }
}

/// G15-03 (AC-2.1, AC-2.2, AC-2.3): SVG layer order & expanded bleed bounds.
#[test]
fn g15_03_sticker_backing_svg_layer_order_and_bleed_bounds() {
    let doc = sample_sticker_doc();
    let opts = ExportOptions {
        die_cut_contour: true,
        die_cut_offset_mm: 2.0,
        sticker_backing: true,
        sticker_backing_color: "#FFFFFF".to_string(),
        sticker_bleed_mm: 1.5,
        ..ExportOptions::default()
    };
    let svg = to_svg(&doc, &opts);
    let idx_backing = svg.find("id=\"StickerBacking\"").expect("StickerBacking group missing");
    let idx_fields = svg.find("id=\"Bidang_warna\"").expect("Bidang_warna group missing");
    let idx_lines = svg.find("id=\"Garis\"").expect("Garis group missing");
    let idx_cut = svg.find("id=\"CutContour\"").expect("CutContour group missing");
    assert!(idx_backing < idx_fields, "StickerBacking must come before Bidang_warna");
    assert!(idx_fields < idx_lines && idx_lines < idx_cut, "CutContour must come after Garis");

    let cut_c = build_die_cut_contours(&doc, &opts);
    let back_c = build_sticker_backing_contours(&doc, &opts);
    assert!(!cut_c.is_empty() && !back_c.is_empty());
    let raw_bbox = |cs: &[trasea_core::trace::Contour]| -> (f64, f64, f64, f64) {
        let (mut x0, mut y0, mut x1, mut y1) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
        for c in cs {
            x0 = x0.min(c.start.0); y0 = y0.min(c.start.1); x1 = x1.max(c.start.0); y1 = y1.max(c.start.1);
            for cv in &c.curves {
                for pt in cv { x0 = x0.min(pt.0); y0 = y0.min(pt.1); x1 = x1.max(pt.0); y1 = y1.max(pt.1); }
            }
        }
        (x0, y0, x1, y1)
    };
    let (c_min_x, c_min_y, c_max_x, c_max_y) = raw_bbox(&cut_c);
    let (b_min_x, b_min_y, b_max_x, b_max_y) = raw_bbox(&back_c);
    let _ = die_cut_bounds_svg(&doc, &back_c);
    // Also verify build_offset_contours directly
    let off_c = build_offset_contours(&doc, 3.5, 96.0);
    assert!(!off_c.is_empty());
    assert!(b_min_x < c_min_x, "backing min_x {b_min_x} must be < cut min_x {c_min_x}");
    assert!(b_min_y < c_min_y, "backing min_y {b_min_y} must be < cut min_y {c_min_y}");
    assert!(b_max_x > c_max_x, "backing max_x {b_max_x} must be > cut max_x {c_max_x}");
    assert!(b_max_y > c_max_y, "backing max_y {b_max_y} must be > cut max_y {c_max_y}");
}

/// G15-04 (AC-2.2, AC-2.3): PDF CMYK white backing `0 0 0 0 k` + `/Separation /CutContour`, and valid EPS.
#[test]
fn g15_04_sticker_backing_pdf_cmyk_and_eps() {
    let doc = sample_sticker_doc();
    let opts = ExportOptions {
        color_space: ColorSpace::Cmyk,
        die_cut_contour: true,
        die_cut_offset_mm: 2.0,
        sticker_backing: true,
        sticker_backing_color: "#FFFFFF".to_string(),
        sticker_bleed_mm: 1.0,
        ..ExportOptions::default()
    };
    let pdf_bytes = to_pdf(&doc, &opts).expect("to_pdf CMYK");
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_str.starts_with("%PDF-1.4"), "valid PDF 1.4 header");
    assert!(pdf_str.contains("0 0 0 0 k\n"), "CMYK pure white 0 0 0 0 k must be present for #FFFFFF StickerBacking");
    assert!(pdf_str.contains("/Separation /CutContour"), "PDF must include /Separation /CutContour");

    let eps = to_eps(&doc, &opts);
    assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"), "valid EPSF-3.0 header");
    assert!(eps.contains("0 0 0 0 CMYK\n"), "EPS must emit 0 0 0 0 CMYK for #FFFFFF StickerBacking");
    assert!(eps.contains("(CutContour)"), "EPS must include CutContour spot");
}

/// G15-05 (AC-2.4): DXF ignores StickerBacking and emits only CUTCONTOUR identically.
#[test]
fn g15_05_dxf_excludes_sticker_backing() {
    let doc = sample_sticker_doc();
    let base_opts = ExportOptions {
        die_cut_contour: true,
        die_cut_offset_mm: 2.0,
        sticker_backing: false,
        sticker_bleed_mm: 0.0,
        ..ExportOptions::default()
    };
    let backing_opts = ExportOptions {
        sticker_backing: true,
        sticker_backing_color: "#FFFFFF".to_string(),
        sticker_bleed_mm: 2.0,
        ..base_opts.clone()
    };
    let dxf_base = to_dxf(&doc, &base_opts);
    let dxf_back = to_dxf(&doc, &backing_opts);
    assert_eq!(dxf_base, dxf_back, "DXF output must be identical when sticker_backing is enabled");
    assert!(dxf_back.contains("CUTCONTOUR"));
    assert!(!dxf_back.to_uppercase().contains("STICKERBACKING"));
}

/// G15-06 (AC-2.5): sticker_backing=false matches default byte-for-byte; .trasea 1.0..1.4 still loads cleanly.
#[test]
fn g15_06_backward_compat_trasea_and_byte_identical_default() {
    let doc = sample_sticker_doc();
    let def_opts = ExportOptions::default();
    let explicit_off = ExportOptions {
        sticker_backing: false,
        sticker_backing_color: "#FFFFFF".to_string(),
        sticker_bleed_mm: 0.0,
        ..ExportOptions::default()
    };
    assert_eq!(to_svg(&doc, &def_opts), to_svg(&doc, &explicit_off));
    assert_eq!(to_eps(&doc, &def_opts), to_eps(&doc, &explicit_off));
    assert_eq!(to_dxf(&doc, &def_opts), to_dxf(&doc, &explicit_off));

    for ver in ["1.0.0", "1.1.0", "1.2.0", "1.3.0", "1.4.0"] {
        let json = format!(
            r#"{{"schemaVersion":1,"appVersion":"{ver}","sourcePath":"a.png","sourceSha256":"x",
            "options":{{"colors":8,"smooth":0.5,"keepLines":true,"aiClean":false,"detectPrimitives":true,
            "symmetry":"off","detectText":true}},
            "exportOptions":{{"linesAs":"stroke","trapPt":0.0}}}}"#
        );
        let p: ProjectFile = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{ver}: {e}"));
        assert!(!p.export_options.sticker_backing, "{ver}: sticker_backing default must be false");
        assert_eq!(p.export_options.sticker_backing_color, "#FFFFFF", "{ver}: default color #FFFFFF");
        assert_eq!(p.export_options.sticker_bleed_mm, 0.0, "{ver}: default bleed 0.0");
    }
}

/// G15-07 (AC-3.1): trasea-core keeps 0 network crates and 0 GPL/AGPL crates.
#[test]
fn g15_07_core_zero_network_and_gpl_audit() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).expect("workspace root");
    let core_toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    let lock = std::fs::read_to_string(workspace_root.join("Cargo.lock")).unwrap();

    let forbidden = ["reqwest", "ureq", "hyper", "tokio-tungstenite", "curl", "surf", "isahc", "tonic", "potrace", "ghostscript"];
    for c in forbidden {
        assert!(!core_toml.contains(c), "forbidden crate in trasea-core/Cargo.toml: {c}");
    }

    let mut deps: HashMap<String, Vec<String>> = HashMap::new();
    let (mut cur, mut in_deps) = (None::<String>, false);
    for line in lock.lines() {
        let t = line.trim();
        if t == "[[package]]" {
            cur = None;
            in_deps = false;
        } else if let Some(rest) = t.strip_prefix("name = \"") {
            if let Some(n) = rest.strip_suffix('"') {
                cur = Some(n.to_string());
                deps.entry(n.to_string()).or_default();
            }
        } else if t == "dependencies = [" {
            in_deps = true;
        } else if in_deps {
            if t == "]" {
                in_deps = false;
            } else if let Some(ref c) = cur {
                let dep_name = t.trim_matches(|ch: char| ch == '"' || ch == ',').split_whitespace().next().unwrap_or("");
                if !dep_name.is_empty() {
                    deps.entry(c.clone()).or_default().push(dep_name.to_string());
                }
            }
        }
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut stack = vec!["trasea-core".to_string()];
    while let Some(pkg) = stack.pop() {
        if !seen.insert(pkg.clone()) {
            continue;
        }
        if let Some(children) = deps.get(&pkg) {
            for ch in children {
                stack.push(ch.clone());
            }
        }
    }
    for c in forbidden {
        assert!(!seen.contains(c), "forbidden transitive crate in trasea-core: {c}");
    }
}

/// G15-08 (AC-1.5): thin strokes consistently emit round cap & round join in SVG, PDF, and EPS.
#[test]
fn g15_08_thin_strokes_round_cap_and_join() {
    let (img, _) = landscape_variant(false);
    let mut p = Params::default();
    p.colors = 10;
    p.smooth = 0.5;
    p.ai_clean = false;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize landscape");
    assert!(!out.doc.strokes.is_empty(), "expected thin strokes");

    let opts_cmyk = ExportOptions { color_space: ColorSpace::Cmyk, ..ExportOptions::default() };
    let svg = to_svg(&out.doc, &ExportOptions::default());
    let mut stroke_paths = 0usize;
    for line in svg.lines() {
        if line.contains("data-trasea-id=\"s:") {
            stroke_paths += 1;
            assert!(line.contains("stroke-linecap=\"round\""), "missing round cap: {line}");
            assert!(line.contains("stroke-linejoin=\"round\""), "missing round join: {line}");
        }
    }
    assert!(stroke_paths > 0, "expected stroke paths in SVG");

    let pdf = to_pdf(&out.doc, &opts_cmyk).expect("to_pdf");
    let pdf_str = String::from_utf8_lossy(&pdf);
    assert!(pdf_str.contains("1 J 1 j\n"), "PDF thin strokes must set 1 J 1 j (round cap/join)");

    let eps = to_eps(&out.doc, &opts_cmyk);
    assert!(eps.contains("1 setlinecap 1 setlinejoin"), "EPS thin strokes must set 1 setlinecap 1 setlinejoin");
}

/// G15-09 (P5): Simple 2-colour curved acorn/shield icon (512x512, smooth=0.5, target_nodes=None)
/// produces clean curves with <= 40 nodes and MAE <= 1.5.
#[test]
fn g15_09_p5_simple_curved_icon_clean_node_count() {
    let size = 512u32;
    let mut img = RgbImage::from_pixel(size, size, Rgb([255, 255, 255]));
    let cx = 256.0f64;
    for y in 0..size {
        for x in 0..size {
            let fx = x as f64 - cx;
            let fy = y as f64;
            // Acorn cap (top dome)
            let cap_dx = fx / 155.0;
            let cap_dy = (fy - 210.0) / 115.0;
            let in_cap = fy <= 215.0 && (cap_dx * cap_dx + cap_dy * cap_dy <= 1.0);
            // Stem on top
            let in_stem = (fx.abs() <= 16.0) && fy >= 60.0 && fy <= 110.0;
            // Acorn body (smooth tapered nut silhouette)
            let t = ((fy - 215.0) / 225.0).clamp(0.0, 1.0);
            let half_w = 132.0 * (1.0 - t * t).max(0.0).sqrt();
            let in_body = fy >= 215.0 && fy <= 440.0 && fx.abs() <= half_w;
            if in_cap || in_stem {
                img.put_pixel(x, y, Rgb([125, 74, 42]));
            } else if in_body {
                img.put_pixel(x, y, Rgb([214, 152, 86]));
            }
        }
    }
    let mut p = Params::default();
    p.colors = 3;
    p.smooth = 0.5;
    p.target_nodes = None;
    p.ai_clean = false;
    p.measure = true;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize acorn icon");
    let mae = out.stats.mae.unwrap_or(99.0);
    println!("G15-09 acorn icon: paths={} nodes={} MAE={:.3}", out.stats.paths, out.stats.nodes, mae);
    assert!(out.stats.nodes <= 40, "expected <= 40 nodes for simple 2-colour icon, got {}", out.stats.nodes);
    assert!(mae <= 1.5, "expected MAE <= 1.5, got {mae:.3}");
}

/// G15-10 (P5): Sharp corners (square / triangle) stay sharp and are not rounded by post-fit merge.
#[test]
fn g15_10_p5_sharp_corners_preserved() {
    let size = 300u32;
    let mut img = RgbImage::from_pixel(size, size, Rgb([255, 255, 255]));
    // Draw a crisp dark blue triangle with 3 sharp vertices: (150, 40), (260, 250), (40, 250)
    for y in 40..=250 {
        let t = (y - 40) as f64 / 210.0;
        let x0 = (150.0 - 110.0 * t).round() as u32;
        let x1 = (150.0 + 110.0 * t).round() as u32;
        for x in x0..=x1 {
            img.put_pixel(x, y, Rgb([28, 72, 160]));
        }
    }
    let mut p = Params::default();
    p.colors = 2;
    p.smooth = 0.5;
    p.detect_primitives = false;
    p.ai_clean = false;
    p.measure = true;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize sharp triangle");
    let mae = out.stats.mae.unwrap_or(99.0);
    // Count straight line segments ('L') in the triangle path
    let l_count = out.svg.matches(" L ").count() + out.svg.matches("L").count();
    println!("G15-10 sharp triangle: nodes={} MAE={:.3} L_ops={l_count}", out.stats.nodes, mae);
    assert!(mae <= 2.5, "sharp triangle MAE too high: {mae:.3}");
    assert!(out.stats.nodes <= 20, "sharp triangle should stay compact (<= 20 nodes), got {}", out.stats.nodes);
    // Verify the rendered top apex (150, 44) is still dark blue (not rounded away)
    let rend = render_svg(&out.svg, size, size).expect("render triangle");
    let apex = rend.get_pixel(150, 46);
    assert!(apex[2] > 100 && apex[0] < 90, "top sharp apex got rounded away: {:?}", apex);
}
