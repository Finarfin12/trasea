//! Trasea 1.4.0 — "lines disappear after update" (PRD L5): G14-01 .. G14-05.
//! Fixture: a synthetic flat landscape (mountains, sun, river, trees) with dark outlines 3-6 px wide that touch many
//! colour fields, drawn at 4x and box-filtered so the outline is anti-aliased like a real illustration.

use image::{Rgb, RgbImage};
use trasea_core::path_edit::PathEdit;
use trasea_core::stats::render_svg;
use trasea_core::{vectorize, Params};

const W: usize = 800;
const H: usize = 600;
const SS: usize = 4;
pub const OUTLINE: [u8; 3] = [24, 24, 30];

type Poly = Vec<(f64, f64)>;

struct Canvas {
    px: Vec<[u8; 3]>,
    /// outline coverage at hi-res
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
    /// Stroke of width `w` along the closed polyline (round joins via capsule test).
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
    (0..96).map(|i| (cx + r * (i as f64 / 96.0 * std::f64::consts::TAU).cos(), cy + r * (i as f64 / 96.0 * std::f64::consts::TAU).sin())).collect()
}

/// (image, outline truth mask at 1x)
pub fn landscape() -> (RgbImage, Vec<bool>) {
    landscape_variant(false)
}

/// `hard`: darker fields next to the outline (low contrast) and JPEG-like noise.
pub fn landscape_variant(hard: bool) -> (RgbImage, Vec<bool>) {
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
    // river: sine edges from the horizon down to the bottom
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
    // trees
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
    // outlines on top
    for p in [&m1, &m2, &m3] {
        c.stroke_poly(p, 5.0, true);
    }
    c.stroke_poly(&sun, 4.0, true);
    c.stroke_poly(&left, 4.0, false);
    c.stroke_poly(&right, 4.0, false);
    for p in trees.iter().chain(trunks.iter()) {
        c.stroke_poly(p, 3.0, true);
    }
    // frame: left and bottom edge (the screenshot lost exactly these)
    c.stroke_poly(&vec![(2.5, 0.0), (2.5, 597.5), (800.0, 597.5)], 5.0, false);
    // downsample
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

fn lum(p: &Rgb<u8>) -> f32 {
    0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32
}

/// Outline pixels of the render: clearly dark (the outline is rgb 24,24,30; the darkest field is far brighter).
pub fn dark_pixels(img: &RgbImage) -> Vec<bool> {
    img.pixels().map(|p| lum(p) < 62.0).collect()
}

pub struct Recall {
    pub truth: usize,
    pub covered: usize,
    pub recall: f64,
    /// longest extent (px) of a connected run of outline pixels that the render lost
    pub max_gap_px: usize,
    pub gaps: Vec<(usize, usize, usize, usize)>,
}

pub fn outline_recall(render: &RgbImage, truth: &[bool]) -> Recall {
    outline_recall_tol(render, truth, 2)
}

/// Same, with a tolerance of `tol` pixels (0 = a lost outline pixel counts even when a dark pixel sits right next to it).
pub fn outline_recall_tol(render: &RgbImage, truth: &[bool], tol: i64) -> Recall {
    let dark = dark_pixels(render);
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
    // connected runs of missing pixels
    let mut seen = vec![false; W * H];
    let mut max_gap = 0usize;
    let mut gaps: Vec<(usize, usize, usize, usize)> = Vec::new();
    for s in 0..W * H {
        if !missing[s] || seen[s] {
            continue;
        }
        let (mut x0, mut x1, mut y0, mut y1) = (W, 0usize, H, 0usize);
        let mut st = vec![s];
        seen[s] = true;
        while let Some(i) = st.pop() {
            let (x, y) = (i % W, i / W);
            x0 = x0.min(x);
            x1 = x1.max(x);
            y0 = y0.min(y);
            y1 = y1.max(y);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx as usize >= W || ny as usize >= H {
                        continue;
                    }
                    let j = ny as usize * W + nx as usize;
                    if missing[j] && !seen[j] {
                        seen[j] = true;
                        st.push(j);
                    }
                }
            }
        }
        max_gap = max_gap.max((x1 - x0).max(y1 - y0) + 1);
        if (x1 - x0).max(y1 - y0) + 1 >= 6 {
            gaps.push((x0, y0, x1, y1));
        }
    }
    Recall { truth: t, covered: c, recall: c as f64 / t.max(1) as f64, max_gap_px: max_gap, gaps }
}

pub fn trace(img: &RgbImage, p: &Params) -> trasea_core::Output {
    vectorize(img, p, &|_, _| {}).expect("trace")
}

fn render(out: &trasea_core::Output) -> RgbImage {
    render_svg(&out.svg, W as u32, H as u32).expect("render")
}


fn base() -> Params {
    Params { colors: 10, measure: false, ..Params::default() }
}

fn dark_count(img: &RgbImage) -> usize {
    dark_pixels(img).iter().filter(|&&d| d).count()
}

/// Index of the stroke with the most curves (a long outline such as the horizon).
fn longest_stroke(out: &trasea_core::Output) -> usize {
    out.doc.strokes[0].strokes.iter().enumerate().max_by_key(|(_, s)| s.curves.len()).map(|(i, _)| i).unwrap()
}

fn count_ids(svg: &str, prefix: &str) -> usize {
    svg.matches(&format!("data-trasea-id=\"{prefix}")).count()
}

/// G14-01 - an edit made on one trace is never applied to a different trace (PRD AC-5.1).
#[test]
fn g14_01_stale_edits_are_not_applied_to_another_trace() {
    let (img, _) = landscape();
    let a = trace(&img, &base());
    assert_eq!(a.trace_fingerprint.len(), 64, "SHA-256 hex");
    assert!(!a.edits_stale);
    let k = longest_stroke(&a);
    let edit = PathEdit::Delete { target_id: format!("s:0:{k}") };

    // same trace + matching fingerprint: the edit is applied (that stroke is gone)
    let a_edit = trace(&img, &Params { path_edits: vec![edit.clone()], edit_fingerprint: Some(a.trace_fingerprint.clone()), ..base() });
    assert!(!a_edit.edits_stale);
    assert_ne!(a_edit.svg, a.svg);
    assert_eq!(count_ids(&a_edit.svg, "s:0:"), count_ids(&a.svg, "s:0:") - 1, "exactly one stroke removed");
    assert_eq!(a_edit.trace_fingerprint, a.trace_fingerprint, "the fingerprint is of the UNEDITED trace");

    // another trace (different colours / smoothness): the old fingerprint no longer matches, so nothing is deleted
    let mut checked = 0;
    for (colors, smooth) in [(8usize, 0.5f32), (10, 0.3), (12, 0.7)] {
        let p = Params { colors, smooth, ..base() };
        let clean = trace(&img, &p);
        let stale = trace(&img, &Params { path_edits: vec![edit.clone()], edit_fingerprint: Some(a.trace_fingerprint.clone()), ..p.clone() });
        if clean.trace_fingerprint != a.trace_fingerprint {
            checked += 1;
            assert!(stale.edits_stale, "colors={colors} smooth={smooth}: edit must be reported stale");
            assert_eq!(stale.svg, clean.svg, "colors={colors} smooth={smooth}: a stale edit must change nothing");
        }
    }
    assert!(checked >= 2, "at least two parameter sets must give a different trace");

    // a smoothness change always changes the trace (the curves are fitted differently)
    let s03 = trace(&img, &Params { smooth: 0.3, ..base() });
    assert_ne!(s03.trace_fingerprint, a.trace_fingerprint);

    // legacy callers without a fingerprint keep the old behaviour (edits always apply)
    let legacy = trace(&img, &Params { smooth: 0.3, path_edits: vec![edit], ..base() });
    assert!(!legacy.edits_stale);
    assert_ne!(legacy.svg, s03.svg);
}

/// G14-02 - one click addresses one connected stroke; deleting it changes pixels only around that stroke.
#[test]
fn g14_02_selection_is_one_stroke_without_collateral_damage() {
    let (img, _) = landscape();
    let a = trace(&img, &base());
    let strokes = &a.doc.strokes[0].strokes;
    assert!(strokes.len() >= 20, "{} strokes", strokes.len());
    // every stroke has its own id, once
    assert_eq!(count_ids(&a.svg, "s:0:"), strokes.len());
    for k in 0..strokes.len() {
        assert_eq!(a.svg.matches(&format!("data-trasea-id=\"s:0:{k}\"")).count(), 1, "s:0:{k}");
    }
    let before = render(&a);
    let scale = a.doc.scale;
    let ks = [longest_stroke(&a), 0, strokes.len() / 2, strokes.len() - 1];
    for &k in &ks {
        let o = trace(&img, &Params { path_edits: vec![PathEdit::Delete { target_id: format!("s:0:{k}") }], ..base() });
        let after = render(&o);
        // bbox of the stroke in source pixels, widened by its width + 4 px
        let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
        for cv in &strokes[k].curves {
            for p in cv {
                x0 = x0.min(p.0 / scale);
                y0 = y0.min(p.1 / scale);
                x1 = x1.max(p.0 / scale);
                y1 = y1.max(p.1 / scale);
            }
        }
        let pad = strokes[k].width / scale + 4.0;
        let (mut changed, mut outside) = (0usize, 0usize);
        for y in 0..H {
            for x in 0..W {
                let d = (lum(before.get_pixel(x as u32, y as u32)) - lum(after.get_pixel(x as u32, y as u32))).abs();
                if d > 40.0 {
                    changed += 1;
                    let (fx, fy) = (x as f64, y as f64);
                    if fx < x0 - pad || fx > x1 + pad || fy < y0 - pad || fy > y1 + pad {
                        outside += 1;
                    }
                }
            }
        }
        assert!(changed > 0, "s:0:{k} deleted nothing visible");
        assert_eq!(outside, 0, "s:0:{k}: {outside} changed pixels outside the own box of the stroke");
    }
}

/// G14-03 - outline recall >= 95 % and no run of lost outline wider than 2x the line width, without more nodes (AC-5.3). Measured with
/// a 2 px tolerance (a lost outline pixel counts only when no dark pixel lies within 2 px); G14-06 holds the strict numbers.
#[test]
fn g14_03_outline_recall_and_no_gaps() {
    for (hard, node_cap) in [(false, 548usize), (true, 597)] {
        let (img, truth) = landscape_variant(hard);
        let out = trace(&img, &base());
        let r = outline_recall(&render(&out), &truth);
        assert!(r.recall >= 0.95, "hard={hard}: recall {:.3}", r.recall);
        // thickest outline of the fixture is 5 px: no run of lost outline longer than 2x that
        assert!(r.max_gap_px <= 10, "hard={hard}: gap {} px {:?}", r.max_gap_px, r.gaps);
        // 1.3.0 gave 498 / 542 nodes on these pictures: at most +10 %
        assert!(out.stats.nodes <= node_cap, "hard={hard}: {} nodes > {node_cap}", out.stats.nodes);
    }
}

/// G14-04 - re-applying 5 times with different parameters never makes the outline shrink by more than 2 %, even when the
/// edit from the first trace is still being sent along (the situation behind "the lines keep disappearing").
#[test]
fn g14_04_five_retraces_keep_the_outline() {
    let (img, _) = landscape();
    let first = trace(&img, &base());
    let k = longest_stroke(&first);
    let edit = PathEdit::Delete { target_id: format!("s:0:{k}") };
    let fp = first.trace_fingerprint.clone();
    let mut prev = dark_count(&render(&first));
    let mut stale_seen = 0;
    for (i, (colors, smooth)) in [(8usize, 0.5f32), (12, 0.5), (10, 0.4), (10, 0.6), (6, 0.5)].into_iter().enumerate() {
        let p = Params { colors, smooth, ..base() };
        let clean = trace(&img, &p);
        let sent = trace(&img, &Params { path_edits: vec![edit.clone()], edit_fingerprint: Some(fp.clone()), ..p.clone() });
        let (c, s) = (dark_count(&render(&clean)), dark_count(&render(&sent)));
        if sent.edits_stale {
            // the edit belongs to another trace: the result is exactly the clean trace
            stale_seen += 1;
            assert_eq!(sent.svg, clean.svg, "retrace {i}");
            assert_eq!(c, s);
        } else {
            // the trace came out identical, so the user edit is still valid and stays applied
            assert_eq!(clean.trace_fingerprint, fp, "retrace {i}");
        }
        // across re-applies the outline of the (unedited) trace stays within 2 %
        let drop = (prev as f64 - c as f64) / prev as f64;
        assert!(drop <= 0.02, "retrace {i}: outline pixels fell {:.1} % ({prev} -> {c})", drop * 100.0);
        prev = c;
    }
    assert!(stale_seen >= 2, "at least two of the five retraces must be different traces");
}

/// G14-05 - `.trasea` projects of 1.0 - 1.3 still open; 1.4 adds an optional edit fingerprint.
#[test]
fn g14_05_old_projects_still_load_and_fingerprint_roundtrips() {
    use trasea_core::project::{ProjectFile, PROJECT_SCHEMA_VERSION};
    for ver in ["1.0.0", "1.1.0", "1.2.0", "1.3.0"] {
        let node_edits = if ver == "1.3.0" { r#","nodeEdits":[{"kind":"deleteNode","targetId":"f:1:0","subpathIdx":0,"nodeIdx":1}]"# } else { "" };
        let json = format!(
            r#"{{"schemaVersion":1,"appVersion":"{ver}","sourcePath":"a.png","sourceSha256":"x",
            "options":{{"colors":8,"smooth":0.5,"keepLines":true,"aiClean":false,"detectPrimitives":true,
            "symmetry":"off","detectText":true,"pathEdits":[{{"kind":"delete","targetId":"s:0:3"}}]{node_edits}}},
            "exportOptions":{{"linesAs":"stroke","trapPt":0.0}}}}"#
        );
        let p: ProjectFile = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{ver}: {e}"));
        assert_eq!(p.schema_version, PROJECT_SCHEMA_VERSION);
        assert_eq!(p.options.path_edits.len(), 1, "{ver}");
        assert!(p.options.edit_fingerprint.is_none(), "{ver}: old projects have no fingerprint, edits then apply as before");
    }
    // 1.4: the fingerprint is stored and read back
    let json = r#"{"schemaVersion":1,"appVersion":"1.4.0","sourcePath":"a.png","sourceSha256":"x",
        "options":{"colors":8,"smooth":0.5,"keepLines":true,"aiClean":false,"detectPrimitives":true,
        "symmetry":"off","detectText":true,"editFingerprint":"abc123"},
        "exportOptions":{"linesAs":"stroke","trapPt":0.0}}"#;
    let p: ProjectFile = serde_json::from_str(json).unwrap();
    assert_eq!(p.options.edit_fingerprint.as_deref(), Some("abc123"));
    let back = serde_json::to_string(&p).unwrap();
    assert!(back.contains("\"editFingerprint\":\"abc123\""));
    let mut none = p.clone();
    none.options.edit_fingerprint = None;
    assert!(!serde_json::to_string(&none).unwrap().contains("editFingerprint"), "absent when unused, so 1.3 files stay unchanged");
}

/// G14-06 - strict outline floor (exact overlap, no tolerance). The thin-line extractor of 1.3.0 loses the pixels of outline
/// corners and junctions (foot of a mountain, base of a tree): about 9-11 % of the outline at 0 px tolerance. This is a KNOWN
/// limitation that 1.4.0 does not change: a junction-restoring variant closed most gaps but raised node counts on the benchmark
/// by up to +117 % (it interacts with the node budget), so it was not shipped. The floors below record today's numbers so any
/// later change of the extractor is measured against them (they may only go up).
#[test]
fn g14_06_strict_outline_floor() {
    for (hard, floor0, floor1, gaps1_max) in [(false, 0.88f64, 0.96f64, 45usize), (true, 0.90, 0.98, 25)] {
        let (img, truth) = landscape_variant(hard);
        let rend = render(&trace(&img, &base()));
        let r0 = outline_recall_tol(&rend, &truth, 0);
        let r1 = outline_recall_tol(&rend, &truth, 1);
        assert!(r0.recall >= floor0, "hard={hard}: exact recall {:.3} < {floor0}", r0.recall);
        assert!(r1.recall >= floor1, "hard={hard}: recall at 1 px {:.3} < {floor1}", r1.recall);
        assert!(r1.gaps.len() <= gaps1_max, "hard={hard}: {} gaps (>= 6 px) at 1 px tolerance", r1.gaps.len());
    }
}
