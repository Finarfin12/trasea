//! Automatic tests G1-G6 and G8 for 0.3.0 (G7 = the 0.1.0 tests in core.rs and the 0.2.0 tests in core02.rs, unchanged).
//! Synthetic images only.

use image::{Rgb, RgbImage};
use std::time::Instant;
use trasea_core::centerline::{centerline_opts, CenterOpts};
use trasea_core::export::{svg_to_pdf, to_svg, ExportOptions};
use trasea_core::primitives::Primitive;
use trasea_core::stats::render_svg;
use trasea_core::text_detect;
use trasea_core::{vectorize, Params, SymmetryMode};

const WHITE: [u8; 3] = [255, 255, 255];

fn quiet() -> Params {
    Params { ai_clean: false, measure: false, keep_lines: false, ..Params::default() }
}

/// Fill the pixels for which `inside(x, y)` holds, with 4x4 supersampling for clean anti-aliased edges.
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

fn disc(cx: f64, cy: f64, r: f64) -> impl Fn(f64, f64) -> bool {
    move |x, y| (x - cx).powi(2) + (y - cy).powi(2) <= r * r
}

fn ellipse(cx: f64, cy: f64, rx: f64, ry: f64, deg: f64) -> impl Fn(f64, f64) -> bool {
    let (c, s) = (deg.to_radians().cos(), deg.to_radians().sin());
    move |x, y| {
        let (dx, dy) = (x - cx, y - cy);
        let (u, v) = (dx * c + dy * s, -dx * s + dy * c);
        (u / rx).powi(2) + (v / ry).powi(2) <= 1.0
    }
}

fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> impl Fn(f64, f64) -> bool {
    move |x, y| x >= x0 && x <= x1 && y >= y0 && y <= y1
}

/// Value of an attribute of the first tag that starts with `tag`.
fn attr(svg: &str, tag: &str, name: &str) -> Option<f64> {
    let start = svg.find(tag)?;
    let end = svg[start..].find('>')? + start;
    let t = &svg[start..end];
    let key = format!(" {name}=\"");
    let p = t.find(&key)? + key.len();
    t[p..p + t[p..].find('"')?].parse().ok()
}

fn count(svg: &str, needle: &str) -> usize {
    svg.matches(needle).count()
}

// ---------------------------------------------------------------- G1

#[test]
fn g1_basic_shapes_become_pure_shapes_and_organic_ones_stay_paths() {
    let mut img = RgbImage::from_pixel(440, 320, Rgb(WHITE));
    paint(&mut img, [30, 70, 210], disc(80.0, 80.0, 40.0)); // (a) circle
    paint(&mut img, [210, 40, 50], ellipse(280.0, 80.0, 70.0, 32.0, 30.0)); // (b) tilted ellipse
    paint(&mut img, [30, 160, 60], rect(40.0, 200.0, 160.0, 260.0)); // (c) rectangle
    // (d) organic wavy blob
    paint(&mut img, [240, 190, 20], |x, y| {
        let (dx, dy) = (x - 320.0, y - 230.0);
        dx.hypot(dy) <= 46.0 + 12.0 * (5.0 * dy.atan2(dx)).sin()
    });
    let o = vectorize(&img, &Params { colors: 6, ..quiet() }, &|_, _| {}).unwrap();
    let svg = &o.svg;

    let (cx, cy, r) = (attr(svg, "<circle", "cx").expect("no <circle>"), attr(svg, "<circle", "cy").unwrap(), attr(svg, "<circle", "r").unwrap());
    assert!((cx - 80.0).abs() <= 1.5 && (cy - 80.0).abs() <= 1.5 && (r - 40.0).abs() <= 1.5, "circle {cx},{cy} r{r}");

    let (ex, ey) = (attr(svg, "<ellipse", "cx").expect("no <ellipse>"), attr(svg, "<ellipse", "cy").unwrap());
    let (rx, ry) = (attr(svg, "<ellipse", "rx").unwrap(), attr(svg, "<ellipse", "ry").unwrap());
    assert!((ex - 280.0).abs() <= 1.5 && (ey - 80.0).abs() <= 1.5 && (rx - 70.0).abs() <= 1.5 && (ry - 32.0).abs() <= 1.5, "ellipse {ex},{ey} {rx}x{ry}");
    let rot = o.doc.fields.iter().flat_map(|l| l.prims.iter()).find_map(|p| if let Primitive::Ellipse { angle_deg, .. } = p { Some(*angle_deg) } else { None }).unwrap();
    assert!((rot - 30.0).abs() <= 2.0, "ellipse angle {rot}");

    let (rx0, ry0, rw, rh) = (attr(svg, "<rect", "x").expect("no <rect>"), attr(svg, "<rect", "y").unwrap(), attr(svg, "<rect", "width").unwrap(), attr(svg, "<rect", "height").unwrap());
    assert!((rx0 - 40.0).abs() <= 1.5 && (ry0 - 200.0).abs() <= 1.5 && (rw - 120.0).abs() <= 1.5 && (rh - 60.0).abs() <= 1.5, "rect {rx0},{ry0} {rw}x{rh}");

    assert_eq!(o.primitives_count, 3, "exactly the three clean shapes");
    assert_eq!(o.stats.primitives, 3);
    // the wavy blob stays a path, in its own colour layer
    let blob = o.doc.fields.iter().find(|l| l.color[0] > 200 && l.color[1] > 150 && l.color[2] < 90).expect("yellow layer");
    assert!(blob.prims.is_empty() && !blob.contours.is_empty(), "the organic blob was taken for a basic shape");
    // the PDF of the same drawing is valid
    let pdf = svg_to_pdf(svg).expect("pdf");
    assert!(pdf.starts_with(b"%PDF"));
    lopdf::Document::load_mem(&pdf).expect("the PDF must parse");
    // and the render of the SVG is close to the picture
    let r = render_svg(svg, 440, 320).unwrap();
    let mae = img.pixels().zip(r.pixels()).map(|(a, b)| (0..3).map(|c| (a[c] as f64 - b[c] as f64).abs()).sum::<f64>() / 3.0).sum::<f64>() / (440.0 * 320.0);
    assert!(mae < 3.0, "render MAE {mae}");
    // switching the option off gives paths only
    let off = vectorize(&img, &Params { colors: 6, detect_primitives: false, ..quiet() }, &|_, _| {}).unwrap();
    assert!(count(&off.svg, "<circle") + count(&off.svg, "<ellipse") + count(&off.svg, "<rect") == 0 && off.primitives_count == 0);
}

#[test]
fn g1b_a_rounded_rectangle_and_a_turned_square_are_recognised() {
    let mut img = RgbImage::from_pixel(360, 240, Rgb(WHITE));
    // rounded rectangle 40..180 x 60..160 with radius 18
    paint(&mut img, [20, 20, 160], |x, y| {
        let (hx, hy, rad) = (70.0, 50.0, 18.0);
        let (qx, qy) = ((x - 110.0).abs() - (hx - rad), (y - 110.0).abs() - (hy - rad));
        (qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0)) <= rad
    });
    // square turned by 20 degrees
    let (c, s) = (20f64.to_radians().cos(), 20f64.to_radians().sin());
    paint(&mut img, [200, 60, 60], |x, y| {
        let (dx, dy) = (x - 270.0, y - 120.0);
        (dx * c + dy * s).abs() <= 40.0 && (-dx * s + dy * c).abs() <= 40.0
    });
    let o = vectorize(&img, &Params { colors: 4, ..quiet() }, &|_, _| {}).unwrap();
    let rects: Vec<&Primitive> = o.doc.fields.iter().flat_map(|l| l.prims.iter()).filter(|p| matches!(p, Primitive::Rect { .. })).collect();
    assert_eq!(rects.len(), 2, "{rects:?}");
    assert!(rects.iter().any(|p| matches!(p, Primitive::Rect { rx, .. } if (*rx / 2.0 - 18.0).abs() <= 3.0)), "the rounded corners are lost");
    assert!(rects.iter().any(|p| matches!(p, Primitive::Rect { angle_deg, .. } if (angle_deg - 20.0).abs() <= 2.0)), "the turned square lost its angle");
    assert!(o.svg.contains("<rect") && o.svg.contains("transform=\"rotate(19.9"), "a rotated rect needs a rotate transform");
}

// ---------------------------------------------------------------- G2

/// A symmetric logo about x = 200 (picture 400 wide) with `skew` pixels of flaw on the right side only.
fn logo(skew: f64) -> RgbImage {
    let mut img = RgbImage::from_pixel(400, 300, Rgb(WHITE));
    let navy = [20, 40, 120];
    let red = [200, 40, 40];
    // body: a rectangle across the middle
    paint(&mut img, navy, rect(150.0, 150.0, 250.0 + skew, 250.0));
    // two eyes (discs) and two triangular wings, mirrored about x = 200
    paint(&mut img, red, disc(120.0, 90.0, 30.0));
    paint(&mut img, red, disc(280.0 + skew, 90.0 + skew * 0.5, 30.0 + skew * 0.5));
    paint(&mut img, red, |x, y| y >= 160.0 && y <= 240.0 && x >= 60.0 && x <= 150.0 && (x - 60.0) * 80.0 >= (y - 160.0) * 90.0 * 0.0 + (y - 160.0) * 20.0);
    paint(&mut img, red, |x, y| y >= 160.0 && y <= 240.0 && x >= 250.0 + skew && x <= 340.0 + skew && (340.0 + skew - x) * 80.0 >= (y - 160.0) * 20.0);
    img
}

/// Share of pixels that differ from their mirror image, for the best vertical axis near the middle (columns x and c - x).
fn mismatch(svg: &str, w: u32, h: u32) -> f64 {
    let r = render_svg(svg, w, h).unwrap();
    let mut best = f64::MAX;
    for c in (w as i64 - 1 - 4)..=(w as i64 - 1 + 4) {
        let mut bad = 0usize;
        for y in 0..h {
            for x in 0..w {
                let mx = c - x as i64;
                if mx < 0 || mx >= w as i64 {
                    continue;
                }
                let (a, b) = (r.get_pixel(x, y), r.get_pixel(mx as u32, y));
                if (0..3).any(|k| (a[k] as i32 - b[k] as i32).abs() > 40) {
                    bad += 1;
                }
            }
        }
        best = best.min(bad as f64 / (w * h) as f64);
    }
    best
}

#[test]
fn g2_symmetry_makes_both_sides_identical() {
    let img = logo(2.0);
    let off = vectorize(&img, &Params { colors: 4, ..quiet() }, &|_, _| {}).unwrap();
    assert!(off.symmetry_applied.is_none());
    for mode in [SymmetryMode::Auto, SymmetryMode::Vertical] {
        let o = vectorize(&img, &Params { colors: 4, symmetry: mode, ..quiet() }, &|_, _| {}).unwrap();
        assert_eq!(o.symmetry_applied.as_deref(), Some("vertical"), "{mode:?}");
        let (m_on, m_off) = (mismatch(&o.svg, 400, 300), mismatch(&off.svg, 400, 300));
        assert!(m_on <= 0.0015, "{mode:?}: {:.3}% of the pixels differ from their mirror image", m_on * 100.0);
        assert!(m_on < m_off, "symmetry must improve things: {m_on} vs {m_off}");
        // both eyes are identical circles, placed symmetrically about x = 200, with no seam on the axis
        let eyes: Vec<(f64, f64, f64)> = o.doc.fields.iter().flat_map(|l| l.prims.iter()).filter_map(|p| if let Primitive::Circle { cx, cy, r } = p { Some((*cx / o.doc.scale, *cy / o.doc.scale, *r / o.doc.scale)) } else { None }).collect();
        let (l, r) = (eyes.iter().find(|e| e.0 < 200.0).unwrap(), eyes.iter().find(|e| e.0 > 200.0).unwrap());
        // the axis is the best fit of the flawed picture, so it may sit a pixel off the middle; the eyes mirror each other about it
        assert!((l.0 + r.0 - 400.0).abs() < 2.1 && (l.1 - r.1).abs() < 0.3 && (l.2 - r.2).abs() < 0.3, "eyes {l:?} {r:?}");
        // one body path or shape across the axis: no white column in the middle
        let rendered = render_svg(&o.svg, 400, 300).unwrap();
        for y in 160..240 {
            for x in 195..205 {
                let p = rendered.get_pixel(x, y);
                assert!(p[0] < 230 || p[1] < 230 || p[2] < 230, "white seam at {x},{y}");
            }
        }
    }
}

#[test]
fn g2b_an_asymmetric_picture_is_left_alone_by_auto() {
    let mut img = RgbImage::from_pixel(400, 300, Rgb(WHITE));
    paint(&mut img, [20, 40, 120], disc(110.0, 100.0, 50.0));
    paint(&mut img, [200, 40, 40], rect(230.0, 60.0, 380.0, 120.0));
    paint(&mut img, [30, 150, 60], |x, y| (x - 260.0).abs() + (y - 220.0).abs() <= 50.0);
    let auto = vectorize(&img, &Params { colors: 5, symmetry: SymmetryMode::Auto, ..quiet() }, &|_, _| {}).unwrap();
    assert!(auto.symmetry_applied.is_none(), "an asymmetric picture was mirrored: {:?}", auto.symmetry_applied);
    let off = vectorize(&img, &Params { colors: 5, ..quiet() }, &|_, _| {}).unwrap();
    assert_eq!(auto.svg, off.svg, "Auto must not change an asymmetric picture");
}

// ---------------------------------------------------------------- G3

#[test]
fn g3_sharp_corners_stay_sharp_and_smooth_curves_stay_smooth() {
    let mut img = RgbImage::from_pixel(440, 260, Rgb(WHITE));
    // 5-point star centred at (120, 130): outer radius 100, inner 40
    let tips: Vec<(f64, f64)> = (0..5).map(|k| { let a = -std::f64::consts::FRAC_PI_2 + k as f64 * 2.0 * std::f64::consts::PI / 5.0; (120.0 + 100.0 * a.cos(), 130.0 + 100.0 * a.sin()) }).collect();
    let verts: Vec<(f64, f64)> = (0..10).map(|k| { let a = -std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::PI / 5.0; let r = if k % 2 == 0 { 100.0 } else { 40.0 }; (120.0 + r * a.cos(), 130.0 + r * a.sin()) }).collect();
    paint(&mut img, [200, 30, 40], |x, y| {
        // point in polygon
        let mut inside = false;
        let mut j = verts.len() - 1;
        for i in 0..verts.len() {
            let (a, b) = (verts[i], verts[j]);
            if (a.1 > y) != (b.1 > y) && x < (b.0 - a.0) * (y - a.1) / (b.1 - a.1) + a.0 {
                inside = !inside;
            }
            j = i;
        }
        inside
    });
    paint(&mut img, [20, 60, 200], disc(330.0, 130.0, 70.0)); // a smooth circle, kept as a path here
    let o = vectorize(&img, &Params { colors: 3, detect_primitives: false, ..quiet() }, &|_, _| {}).unwrap();

    let star = o.doc.fields.iter().find(|l| l.color[0] > 150 && l.color[2] < 90).expect("red layer");
    let circle = o.doc.fields.iter().find(|l| l.color[2] > 150 && l.color[0] < 90).expect("blue layer");
    // the tips: every real tip has a contour end point within 2 source px (the smoothing must not blunt them)
    let k = o.doc.scale;
    let ends: Vec<(f64, f64)> = star.contours.iter().flat_map(|c| c.curves.iter().map(|cv| (cv[2].0 / k, cv[2].1 / k))).collect();
    for t in &tips {
        let d = ends.iter().map(|e| (e.0 - t.0).hypot(e.1 - t.1)).fold(f64::MAX, f64::min);
        assert!(d <= 2.0, "star tip {t:?} is {d:.2} px from the nearest corner");
    }
    // the star edges are straight `L` segments, the circle has none
    let d_of = |svg: &str, color: &str| -> String {
        let start = svg.find(&format!("fill=\"{color}\" fill-rule")).expect("path of colour");
        let rest = &svg[start..];
        rest[..rest.find("/>").unwrap()].to_string()
    };
    let hexc = |c: [u8; 3]| format!("#{:02X}{:02X}{:02X}", c[0], c[1], c[2]);
    let star_d = d_of(&o.svg, &hexc(star.color));
    // the layers are stacked, so the blue layer also holds the star as an island: look at the circle's own contour only
    let mut only_circle = o.doc.clone();
    only_circle.fields.retain(|l| l.canvas || l.color == circle.color);
    for l in only_circle.fields.iter_mut() {
        l.contours.retain(|c| c.start.0 / k > 240.0);
    }
    let circle_d = d_of(&to_svg(&only_circle, &ExportOptions::default()), &hexc(circle.color));
    assert!(count(&star_d, "L") >= 8, "the star has {} straight segments, want at least 8 of 10", count(&star_d, "L"));
    assert_eq!(count(&circle_d, "L"), 0, "the circle must stay made of curves");
    assert!(count(&circle_d, "C") >= 3 && count(&circle_d, "C") <= 12, "circle curves: {}", count(&circle_d, "C"));
    // no corner points on the circle: every join between its curves is smooth (tangent directions agree)
    for c in circle.contours.iter().filter(|c| c.start.0 / k > 240.0) {
        let mut prev = c.start;
        let mut prev_c2: Option<(f64, f64)> = None;
        for cv in &c.curves {
            if let Some(p2) = prev_c2 {
                let (a, b) = ((prev.0 - p2.0, prev.1 - p2.1), (cv[0].0 - prev.0, cv[0].1 - prev.1));
                let (la, lb) = (a.0.hypot(a.1), b.0.hypot(b.1));
                if la > 1e-6 && lb > 1e-6 {
                    let ang = ((a.0 * b.0 + a.1 * b.1) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees();
                    assert!(ang <= 12.0, "kink of {ang:.1} degrees on a smooth circle");
                }
            }
            prev = cv[2];
            prev_c2 = Some(cv[1]);
        }
    }
}

// ---------------------------------------------------------------- G4

fn glyph(img: &mut RgbImage, x: f64, y: f64, w: f64, h: f64, t: f64) {
    // a square ring: a rough stand-in for a letter (about 74% filled)
    paint(img, [10, 10, 10], rect(x, y, x + w, y + h));
    paint(img, WHITE, rect(x + t, y + t, x + w - t, y + h - t));
}

#[test]
fn g4_a_row_of_letter_like_blobs_is_reported_as_text() {
    let mut img = RgbImage::from_pixel(600, 200, Rgb(WHITE));
    for i in 0..6 {
        glyph(&mut img, 80.0 + i as f64 * 30.0, 70.0 + (i % 2) as f64, 18.0, 28.0, 5.0);
    }
    let o = vectorize(&img, &Params { colors: 3, ..quiet() }, &|_, _| {}).unwrap();
    assert!(!o.text_regions.is_empty(), "no text area found");
    let r = &o.text_regions[0];
    assert!(r.x <= 80.0 && r.x + r.w >= 80.0 + 5.0 * 30.0 + 18.0 && r.y <= 70.0 && r.y + r.h >= 98.0, "the box {r:?} must cover the row of blobs");
    assert!(r.glyph_count >= 5, "glyph count {}", r.glyph_count);
    // the box is only for the preview: it never reaches the SVG or the PDF
    assert!(!o.svg.contains("glyph") && !o.svg.contains("text") && !o.svg.contains("stroke-dasharray"));
    let off = vectorize(&img, &Params { colors: 3, detect_text: false, ..quiet() }, &|_, _| {}).unwrap();
    assert!(off.text_regions.is_empty());
    assert_eq!(off.svg, o.svg, "the text detector must not change the drawing");
}

#[test]
fn g4b_plain_geometric_icons_are_not_text() {
    let mut img = RgbImage::from_pixel(500, 300, Rgb(WHITE));
    paint(&mut img, [20, 60, 200], disc(110.0, 140.0, 70.0));
    paint(&mut img, [200, 40, 40], rect(210.0, 80.0, 330.0, 220.0));
    paint(&mut img, [30, 150, 60], |x, y| (x - 410.0).abs() + (y - 150.0).abs() <= 70.0);
    paint(&mut img, [240, 190, 20], disc(250.0, 40.0, 20.0));
    paint(&mut img, [240, 190, 20], disc(300.0, 40.0, 14.0));
    let o = vectorize(&img, &Params { colors: 6, ..quiet() }, &|_, _| {}).unwrap();
    assert!(o.text_regions.is_empty(), "false positive: {:?}", o.text_regions);
}

#[test]
fn g4c_text_detection_is_fast() {
    // a 2400 x 1680 label picture (the working size of a 1200 x 840 image) with two text rows
    let (ww, wh, up) = (2400usize, 1680usize, 2usize);
    let mut labels = vec![0u8; ww * wh];
    for row in 0..2 {
        for i in 0..8 {
            let (x0, y0) = (200 + i * 70, 300 + row * 400);
            for y in y0..y0 + 56 {
                for x in x0..x0 + 36 {
                    let edge = x < x0 + 10 || x >= x0 + 26 || y < y0 + 10 || y >= y0 + 46;
                    if edge {
                        labels[y * ww + x] = 1;
                    }
                }
            }
        }
    }
    let t0 = Instant::now();
    let regions = text_detect::detect(&labels, ww, wh, up, &[[255, 255, 255], [10, 10, 10]]);
    let ms = t0.elapsed().as_millis();
    assert_eq!(regions.len(), 2, "{regions:?}");
    assert!(ms < 150, "text detection took {ms} ms");
}

// ---------------------------------------------------------------- G5

/// A thin line of colour `line` on `bg` between two points, `width` px thick, hard pixels.
fn thin_line(img: &mut RgbImage, a: (f64, f64), b: (f64, f64), width: f64, color_at: impl Fn(f64) -> [u8; 3], skip: impl Fn(f64) -> bool) {
    let n = ((b.0 - a.0).hypot(b.1 - a.1) * 3.0) as usize;
    for k in 0..=n {
        let t = k as f64 / n as f64;
        if skip(t) {
            continue;
        }
        let (cx, cy) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
        let r = width / 2.0;
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                if ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt() <= r {
                    img.put_pixel(x as u32, y as u32, Rgb(color_at(t)));
                }
            }
        }
    }
}

/// x range covered by the stroked paths of the "Garis" group, in source px.
fn stroke_paths(svg: &str) -> Vec<String> {
    let g = &svg[svg.find("id=\"Garis\"").unwrap()..];
    g.lines().filter(|l| l.contains("stroke-width") && l.starts_with("<path")).map(|l| l.to_string()).collect()
}

fn path_x_extent(d_tag: &str) -> (f64, f64) {
    let d = &d_tag[d_tag.find(" d=\"").unwrap() + 4..];
    let d = &d[..d.find('"').unwrap()];
    let nums: Vec<f64> = d.split(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-')).filter_map(|s| s.parse().ok()).collect();
    let xs: Vec<f64> = nums.iter().step_by(2).copied().collect();
    (xs.iter().cloned().fold(f64::MAX, f64::min), xs.iter().cloned().fold(f64::MIN, f64::max))
}

#[test]
fn g5a_a_faded_line_with_a_one_pixel_break_stays_one_piece() {
    let bg = [137u8, 150, 70];
    let mut img = RgbImage::from_pixel(400, 200, Rgb(bg));
    // a thin vein: dark at both ends, faded in the middle (medium contrast), with a 1 px break at t = 0.5
    let colour = |t: f64| -> [u8; 3] {
        let fade = 1.0 - 4.0 * (t - 0.5).powi(2); // 0 at the ends, 1 in the middle
        let dark = [70.0, 100.0, 70.0];
        let mid = [108.0, 130.0, 70.0];
        [0, 1, 2].map(|c| (dark[c] + (mid[c] - dark[c]) * fade).round() as u8)
    };
    let (a, b): ((f64, f64), (f64, f64)) = ((30.0, 90.0), (370.0, 118.0));
    let len = (b.0 - a.0).hypot(b.1 - a.1);
    thin_line(&mut img, a, b, 2.0, colour, |t| (t - 0.5).abs() * len < 0.6);
    let o = vectorize(&img, &Params { colors: 4, keep_lines: true, ..quiet() }, &|_, _| {}).unwrap();
    let strokes = stroke_paths(&o.svg);
    assert_eq!(strokes.len(), 1, "the vein fell apart into {} pieces: {strokes:?}", strokes.len());
    let (x0, x1) = path_x_extent(&strokes[0]);
    assert!(x1 - x0 >= 0.92 * (b.0 - a.0), "the stroke covers {x0:.0}..{x1:.0} of {}..{}", a.0, b.0);
}

#[test]
fn g5b_a_long_curved_thin_line_is_one_or_two_paths_and_is_all_there() {
    let bg = [225u8, 225, 225];
    let ink = [50u8, 50, 55];
    let mut img = RgbImage::from_pixel(260, 180, Rgb(bg));
    // an arc about 120 px long, width 2.0 -> 2.5 px along its length
    let n = 400;
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let ang = std::f64::consts::PI * (0.15 + 0.5 * t);
        let (cx, cy) = (130.0 + 80.0 * ang.cos(), 150.0 - 80.0 * ang.sin());
        let r = 1.0 + 0.25 * t;
        for y in (cy - r - 1.0).floor() as i64..=(cy + r + 1.0).ceil() as i64 {
            for x in (cx - r - 1.0).floor() as i64..=(cx + r + 1.0).ceil() as i64 {
                if ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt() <= r {
                    img.put_pixel(x as u32, y as u32, Rgb(ink));
                }
            }
        }
    }
    let o = vectorize(&img, &Params { colors: 3, keep_lines: true, ..quiet() }, &|_, _| {}).unwrap();
    let strokes = stroke_paths(&o.svg);
    assert!((1..=2).contains(&strokes.len()), "{} stroke paths", strokes.len());
    // recall: the dark pixels of the original are still dark in the render (within one pixel)
    let r = render_svg(&o.svg, 260, 180).unwrap();
    let dark = |p: &Rgb<u8>| p[0] < 140;
    let (mut total, mut hit) = (0, 0);
    for y in 1..179u32 {
        for x in 1..259u32 {
            if img.get_pixel(x, y)[0] < 100 {
                total += 1;
                let near = (-1i32..=1).any(|dy| (-1i32..=1).any(|dx| dark(r.get_pixel((x as i32 + dx) as u32, (y as i32 + dy) as u32))));
                if near {
                    hit += 1;
                }
            }
        }
    }
    assert!(total > 200 && hit as f64 / total as f64 >= 0.9, "recall {hit}/{total}");
}

#[test]
fn g5c_strokes_that_continue_through_a_junction_are_joined() {
    let (w, h) = (240usize, 160usize);
    let mut m = vec![false; w * h];
    let mut line = |a: (f64, f64), b: (f64, f64), width: f64| {
        let n = ((b.0 - a.0).hypot(b.1 - a.1) * 2.0) as usize + 1;
        for k in 0..=n {
            let t = k as f64 / n as f64;
            let (cx, cy) = (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
            for y in (cy - width).floor() as i64..=(cy + width).ceil() as i64 {
                for x in (cx - width).floor() as i64..=(cx + width).ceil() as i64 {
                    if ((x as f64 + 0.5 - cx).powi(2) + (y as f64 + 0.5 - cy).powi(2)).sqrt() <= width / 2.0 {
                        m[y as usize * w + x as usize] = true;
                    }
                }
            }
        }
    };
    line((20.0, 80.0), (220.0, 80.0), 3.0); // a long line ...
    line((120.0, 80.0), (120.0, 20.0), 3.0); // ... with a branch that leaves it in the middle
    let joined = centerline_opts(&m, w, h, 1.0, 0.5, &CenterOpts::default());
    let apart = centerline_opts(&m, w, h, 1.0, 0.5, &CenterOpts { merge: false, width_tol: 0.75 });
    assert_eq!(apart.strokes.len(), 3, "without merging a T has three strokes");
    assert_eq!(joined.strokes.len(), 2, "the long line must be one stroke and the branch another");
    let longest = joined.strokes.iter().map(|s| s.curves.iter().map(|c| (c[3].0 - c[0].0).abs()).sum::<f64>()).fold(0.0, f64::max);
    assert!(longest > 180.0, "the joined stroke spans {longest:.0} px");
}

// ---------------------------------------------------------------- G6 and G8

#[test]
fn g6_the_defaults_of_0_3_0() {
    let p = Params::default();
    assert!(p.detect_primitives);
    assert_eq!(p.symmetry, SymmetryMode::Off);
    assert!(p.detect_text);
    assert!(!p.ai_clean);
}

#[test]
fn g8_the_cli_accepts_the_new_flags_and_writes_valid_files() {
    let dir = std::env::temp_dir().join(format!("trasea-g8-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("in.png");
    logo(1.0).save(&input).unwrap();
    let exe = env!("CARGO_BIN_EXE_trasea-cli");
    let run = |args: &[&str]| std::process::Command::new(exe).arg(&input).args(args).output().unwrap();
    let svg = dir.join("out.svg");
    let pdf = dir.join("out.pdf");
    let o = run(&[svg.to_str().unwrap(), "--no-primitives", "--symmetry", "auto", "--no-text-detect", "--pdf", pdf.to_str().unwrap()]);
    assert!(o.status.success(), "{}", String::from_utf8_lossy(&o.stderr));
    let text = std::fs::read_to_string(&svg).unwrap();
    assert!(text.starts_with("<?xml") && text.contains("<svg") && !text.contains("<circle"));
    let bytes = std::fs::read(&pdf).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    lopdf::Document::load_mem(&bytes).expect("valid PDF");
    // default flags: the discs of the logo come out as <circle>
    let svg2 = dir.join("out2.svg");
    let o = run(&[svg2.to_str().unwrap(), "--symmetry", "vertical"]);
    assert!(o.status.success());
    assert!(std::fs::read_to_string(&svg2).unwrap().contains("<circle"));
    // a wrong value is refused with exit code 2
    let o = run(&[dir.join("x.svg").to_str().unwrap(), "--symmetry", "diagonal"]);
    assert_eq!(o.status.code(), Some(2));
    let _ = std::fs::remove_dir_all(&dir);
    let _ = to_svg; // keep the import used when the asserts above change
    let _ = ExportOptions::default();
}
