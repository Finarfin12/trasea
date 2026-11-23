//! Export: SVG with named layers (`Latar` > `Bidang warna` > `Garis` > optional `Registrasi`),
//! per-colour black separation films (`to_separation_svgs`), PDF, and Adobe Swatch Exchange (.ase).

use crate::centerline::Stroke;
use crate::primitives::Primitive;
use crate::production::{ProductionMode, ProductionOptions};
use crate::trace::{Contour, Layer};
use crate::{hex, Error, Result, Rgb};

/// Outer margin (px) added on every side when registration marks are enabled.
pub const REG_MARGIN: f64 = 28.0;

/// Thin lines of one colour, as strokes (centerline + width), in working pixels.
#[derive(Debug, Clone)]
pub struct StrokeLayer {
    pub color: Rgb,
    pub strokes: Vec<StrokeData>,
}

#[derive(Debug, Clone)]
pub struct StrokeData {
    pub curves: Vec<[(f64, f64); 4]>,
    /// Width in working pixels.
    pub width: f64,
}

impl From<&Stroke> for StrokeData {
    fn from(s: &Stroke) -> Self {
        StrokeData { curves: s.curves.clone(), width: s.width }
    }
}

/// The traced drawing. Coordinates inside the layers are working pixels; `scale` = working / source size.
#[derive(Debug, Clone)]
pub struct Doc {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    /// Colour fields, bottom to top (the first one is the background rectangle when `canvas == true`).
    pub fields: Vec<Layer>,
    /// ALL thin lines as filled outlines (used when the export says "lines as shapes").
    pub line_shapes: Vec<Layer>,
    /// Thin lines whose width is too uneven for a stroke: they stay outlines even in stroke mode.
    pub line_fallback: Vec<Layer>,
    /// Thin lines as strokes.
    pub strokes: Vec<StrokeLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinesAs {
    Stroke,
    Shape,
}

#[derive(Debug, Clone)]
pub struct ExportOptions {
    pub lines_as: LinesAs,
    /// Print trap in points (0 = none). Colour fields get an outline of their own colour this wide.
    pub trap_pt: f64,
    /// Add 4 registration targets in an outer margin on a `Registrasi` layer.
    pub registration_marks: bool,
    /// Omit the background layer (`canvas == true`).
    pub strip_background: bool,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            lines_as: LinesAs::Stroke,
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: false,
        }
    }
}

/// Default `ExportOptions` matching a production profile.
pub fn export_options_for_production(prod: &ProductionOptions) -> ExportOptions {
    match prod.mode {
        ProductionMode::Standard => ExportOptions::default(),
        ProductionMode::ScreenPrint => ExportOptions {
            lines_as: LinesAs::Stroke,
            trap_pt: 0.5,
            registration_marks: prod.registration_marks,
            strip_background: prod.strip_background,
        },
        ProductionMode::CutEmbroidery => ExportOptions {
            lines_as: LinesAs::Shape,
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: prod.strip_background,
        },
    }
}

/// One 100%-black (`#000000`) screen-print separation film for a single spot colour.
#[derive(Debug, Clone)]
pub struct SeparationFilm {
    /// 1-based film number.
    pub index: usize,
    pub color: Rgb,
    /// File name suffix, e.g. `film01_0F3973`.
    pub suffix: String,
    pub svg: String,
}

fn num(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn pt(p: (f64, f64), k: f64) -> String {
    format!("{},{}", num(p.0 / k), num(p.1 / k))
}

/// A cubic whose handles lie on the chord is a straight segment: write it as `L`, which is what a designer expects.
fn is_straight(p0: (f64, f64), c1: (f64, f64), c2: (f64, f64), p3: (f64, f64), k: f64) -> bool {
    let (dx, dy) = (p3.0 - p0.0, p3.1 - p0.1);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 {
        return false;
    }
    let tol = 0.03 * k;
    [c1, c2].iter().all(|c| {
        let along = ((c.0 - p0.0) * dx + (c.1 - p0.1) * dy) / l;
        ((dx * (c.1 - p0.1) - dy * (c.0 - p0.0)).abs() / l) <= tol && along >= -tol && along <= l + tol
    })
}

fn path_d(contours: &[Contour], k: f64) -> String {
    let mut d = String::new();
    for c in contours {
        d.push('M');
        d.push_str(&pt(c.start, k));
        let mut prev = c.start;
        for cv in &c.curves {
            if is_straight(prev, cv[0], cv[1], cv[2], k) {
                d.push('L');
                d.push_str(&pt(cv[2], k));
            } else {
                d.push('C');
                d.push_str(&pt(cv[0], k));
                d.push(' ');
                d.push_str(&pt(cv[1], k));
                d.push(' ');
                d.push_str(&pt(cv[2], k));
            }
            prev = cv[2];
        }
        d.push('Z');
    }
    d
}

fn stroke_d(s: &StrokeData, k: f64) -> String {
    let mut d = String::from("M");
    d.push_str(&pt(s.curves[0][0], k));
    for cv in &s.curves {
        if is_straight(cv[0], cv[1], cv[2], cv[3], k) {
            d.push('L');
            d.push_str(&pt(cv[3], k));
        } else {
            d.push('C');
            d.push_str(&pt(cv[1], k));
            d.push(' ');
            d.push_str(&pt(cv[2], k));
            d.push(' ');
            d.push_str(&pt(cv[3], k));
        }
    }
    d
}

/// SVG element of a recognised shape. `extra` carries the optional trap stroke attributes.
fn primitive_svg(p: &Primitive, color: &str, extra: &str, k: f64) -> String {
    let rot = |a: f64, cx: f64, cy: f64| if a.abs() > 0.5 { format!(" transform=\"rotate({} {} {})\"", num(a), num(cx / k), num(cy / k)) } else { String::new() };
    match p {
        Primitive::Circle { cx, cy, r } => format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\"{extra}/>\n", num(cx / k), num(cy / k), num(r / k)),
        Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
            format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{color}\"{extra}{}/>\n", num(cx / k), num(cy / k), num(rx / k), num(ry / k), rot(*angle_deg, *cx, *cy))
        }
        Primitive::Rect { x, y, w, h, rx, angle_deg } => {
            let round = if *rx > 0.0 { format!(" rx=\"{}\"", num(rx / k)) } else { String::new() };
            format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{round} fill=\"{color}\"{extra}{}/>\n", num(x / k), num(y / k), num(w / k), num(h / k), rot(*angle_deg, x + w / 2.0, y + h / 2.0))
        }
    }
}

/// Four standard crosshair+circle registration marks in the outer margin.
fn registration_marks_svg(w: f64, h: f64, m: f64) -> String {
    let pts = [
        (m / 2.0, m / 2.0),
        (w + 1.5 * m, m / 2.0),
        (m / 2.0, h + 1.5 * m),
        (w + 1.5 * m, h + 1.5 * m),
    ];
    let mut s = String::from("<g id=\"Registrasi\" data-name=\"Registrasi\">\n");
    for (cx, cy) in pts {
        s.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"none\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx),
            num(cy)
        ));
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx - 9.0),
            num(cy),
            num(cx + 9.0),
            num(cy)
        ));
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx),
            num(cy - 9.0),
            num(cx),
            num(cy + 9.0)
        ));
    }
    s.push_str("</g>\n");
    s
}

/// Colours of the drawing in stacking order (bottom first): the order of the palette numbers (01, 02, ...).
pub fn palette_of(doc: &Doc) -> Vec<Rgb> {
    doc.fields.iter().map(|l| l.color).collect()
}

fn group_names(index: usize, color: Rgb) -> (String, String) {
    let h = hex(color);
    (format!("Warna_{:02}_{}", index, &h[1..]), format!("Warna {:02} {}", index, h))
}

pub fn to_svg(doc: &Doc, opts: &ExportOptions) -> String {
    let k = doc.scale;
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    if opts.registration_marks {
        let vw = doc.width as f64 + 2.0 * REG_MARGIN;
        let vh = doc.height as f64 + 2.0 * REG_MARGIN;
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{vw}\" height=\"{vh}\" viewBox=\"0 0 {vw} {vh}\">\n",
            vw = num(vw),
            vh = num(vh)
        ));
        s.push_str(&format!("<g transform=\"translate({}, {})\">\n", num(REG_MARGIN), num(REG_MARGIN)));
    } else {
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n",
            w = doc.width,
            h = doc.height
        ));
    }

    // trap: a stroke of the fill colour makes every field grow by half of this width on each side
    let trap_px = opts.trap_pt * 96.0 / 72.0;
    let field = |layer: &Layer, index: usize, with_trap: bool, out: &mut String| {
        let (id, name) = group_names(index, layer.color);
        let color = hex(layer.color);
        let trap = if with_trap && trap_px > 0.0 { format!(" stroke=\"{color}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", num(trap_px)) } else { String::new() };
        out.push_str(&format!("<g id=\"{id}\" data-name=\"{name}\">\n"));
        // a layer whose every solid was recognised as a basic shape has no path left
        if !layer.contours.is_empty() || layer.prims.is_empty() {
            out.push_str(&format!("<path fill=\"{color}\" fill-rule=\"evenodd\"{trap} d=\"{}\"/>\n", path_d(&layer.contours, k)));
        }
        for p in &layer.prims {
            out.push_str(&primitive_svg(p, &color, &trap, k));
        }
        out.push_str("</g>\n");
    };

    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    let mut idx = 1;
    // 1. Latar: background layer (omitted when strip_background is active or no canvas layer exists)
    if !opts.strip_background && has_canvas_bg {
        s.push_str("<g id=\"Latar\" data-name=\"Latar\">\n");
        field(&doc.fields[0], idx, false, &mut s);
        idx += 1;
        s.push_str("</g>\n");
    } else if !opts.strip_background && !has_canvas_bg && doc.fields.is_empty() {
        s.push_str("<g id=\"Latar\" data-name=\"Latar\">\n</g>\n");
    }
    // 2. Bidang warna: one sub-group per colour
    s.push_str("<g id=\"Bidang_warna\" data-name=\"Bidang warna\">\n");
    let skip_first = if has_canvas_bg { 1 } else { 0 };
    for layer in doc.fields.iter().skip(skip_first) {
        field(layer, idx, true, &mut s);
        idx += 1;
    }
    s.push_str("</g>\n");
    // 3. Garis: thin lines on top
    s.push_str("<g id=\"Garis\" data-name=\"Garis\">\n");
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                s.push_str(&format!("<path fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", hex(l.color), path_d(&l.contours, k)));
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                s.push_str(&format!("<path fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", hex(l.color), path_d(&l.contours, k)));
            }
            for sl in &doc.strokes {
                let color = hex(sl.color);
                for st in &sl.strokes {
                    if st.curves.is_empty() {
                        continue;
                    }
                    s.push_str(&format!(
                        "<path fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{}\"/>\n",
                        num(st.width / k),
                        stroke_d(st, k)
                    ));
                }
            }
        }
    }
    s.push_str("</g>\n");
    if opts.registration_marks {
        s.push_str("</g>\n");
        s.push_str(&registration_marks_svg(doc.width as f64, doc.height as f64, REG_MARGIN));
    }
    s.push_str("</svg>\n");
    s
}

fn nearest_color_idx(color: Rgb, palette: &[Rgb]) -> Option<usize> {
    if let Some(pos) = palette.iter().position(|&c| c == color) {
        return Some(pos);
    }
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| {
            let dr = color[0] as i32 - c[0] as i32;
            let dg = color[1] as i32 - c[1] as i32;
            let db = color[2] as i32 - c[2] as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(i, _)| i)
}

/// Generate one 100%-black (`#000000`) separation film SVG per active spot colour, with identical
/// `viewBox`, identical registration mark coordinates, and a bottom-margin film label.
pub fn to_separation_svgs(doc: &Doc, opts: &ExportOptions) -> Vec<SeparationFilm> {
    let k = doc.scale;
    let active_fields: Vec<&Layer> = doc
        .fields
        .iter()
        .filter(|l| !(opts.strip_background && l.canvas))
        .collect();
    if active_fields.is_empty() {
        return Vec::new();
    }
    let colors: Vec<Rgb> = active_fields.iter().map(|l| l.color).collect();
    let total = active_fields.len();
    let vw = doc.width as f64 + 2.0 * REG_MARGIN;
    let vh = doc.height as f64 + 2.0 * REG_MARGIN;
    let trap_px = opts.trap_pt * 96.0 / 72.0;
    let reg = registration_marks_svg(doc.width as f64, doc.height as f64, REG_MARGIN);

    active_fields
        .iter()
        .enumerate()
        .map(|(fi, layer)| {
            let index = fi + 1;
            let h_hex = hex(layer.color);
            let suffix = format!("film{:02}_{}", index, &h_hex[1..]);
            let black = "#000000";
            let trap = if !layer.canvas && trap_px > 0.0 {
                format!(" stroke=\"{black}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", num(trap_px))
            } else {
                String::new()
            };

            let mut s = String::new();
            s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            s.push_str(&format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{vw}\" height=\"{vh}\" viewBox=\"0 0 {vw} {vh}\">\n",
                vw = num(vw),
                vh = num(vh)
            ));
            s.push_str(&format!("<g transform=\"translate({}, {})\">\n", num(REG_MARGIN), num(REG_MARGIN)));
            s.push_str(&format!("<g id=\"Film_{:02}_{}\" data-name=\"Film {:02} {}\">\n", index, &h_hex[1..], index, h_hex));
            if !layer.contours.is_empty() || layer.prims.is_empty() {
                s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\"{trap} d=\"{}\"/>\n", path_d(&layer.contours, k)));
            }
            for p in &layer.prims {
                s.push_str(&primitive_svg(p, black, &trap, k));
            }
            // Include thin lines assigned to this spot colour.
            match opts.lines_as {
                LinesAs::Shape => {
                    for l in &doc.line_shapes {
                        if nearest_color_idx(l.color, &colors) == Some(fi) {
                            s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", path_d(&l.contours, k)));
                        }
                    }
                }
                LinesAs::Stroke => {
                    for l in &doc.line_fallback {
                        if nearest_color_idx(l.color, &colors) == Some(fi) {
                            s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", path_d(&l.contours, k)));
                        }
                    }
                    for sl in &doc.strokes {
                        if nearest_color_idx(sl.color, &colors) == Some(fi) {
                            for st in &sl.strokes {
                                if st.curves.is_empty() {
                                    continue;
                                }
                                s.push_str(&format!(
                                    "<path fill=\"none\" stroke=\"{black}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{}\"/>\n",
                                    num(st.width / k),
                                    stroke_d(st, k)
                                ));
                            }
                        }
                    }
                }
            }
            s.push_str("</g>\n</g>\n");
            s.push_str(&reg);
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"10\" fill=\"{black}\">Film {:02}/{:02} — {}</text>\n",
                num(REG_MARGIN),
                num(vh - 8.0),
                index,
                total,
                h_hex
            ));
            s.push_str("</svg>\n");

            SeparationFilm { index, color: layer.color, suffix, svg: s }
        })
        .collect()
}

/// SVG text -> PDF bytes (page size = drawing size).
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = svg2pdf::usvg::Tree::from_str(svg, &opt).map_err(|e| Error::Other(format!("cannot read the SVG for PDF export: {e}")))?;
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default()).map_err(|e| Error::Other(format!("PDF export failed: {e}")))
}

// ---------- Adobe Swatch Exchange ----------

fn ase_name(out: &mut Vec<u8>, name: &str) {
    let units: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    out.extend_from_slice(&(units.len() as u16).to_be_bytes());
    for u in units {
        out.extend_from_slice(&u.to_be_bytes());
    }
}

/// `.ase` file: one group "Trasea" with the palette as global RGB colours named `Trasea NN #RRGGBB`.
pub fn to_ase(palette: &[Rgb]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"ASEF");
    out.extend_from_slice(&1u16.to_be_bytes()); // version 1.0
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&((palette.len() + 2) as u32).to_be_bytes()); // blocks: group start, colours, group end
    // group start
    let mut gs = Vec::new();
    ase_name(&mut gs, "Trasea");
    out.extend_from_slice(&0xC001u16.to_be_bytes());
    out.extend_from_slice(&(gs.len() as u32).to_be_bytes());
    out.extend_from_slice(&gs);
    for (i, c) in palette.iter().enumerate() {
        let mut body = Vec::new();
        ase_name(&mut body, &format!("Trasea {:02} {}", i + 1, hex(*c)));
        body.extend_from_slice(b"RGB ");
        for ch in c {
            body.extend_from_slice(&(*ch as f32 / 255.0).to_be_bytes());
        }
        body.extend_from_slice(&0u16.to_be_bytes()); // colour type: global
        out.extend_from_slice(&0x0001u16.to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
    }
    // group end
    out.extend_from_slice(&0xC002u16.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}
// ---------- Encapsulated PostScript (EPSF-3.0 / Level 2) ----------

fn ps_num(v: f64) -> String {
    let safe = if v.is_finite() { v } else { 0.0 };
    num(safe)
}

fn ps_rgb(c: Rgb) -> String {
    format!(
        "{:.4} {:.4} {:.4} RGB
",
        c[0] as f64 / 255.0,
        c[1] as f64 / 255.0,
        c[2] as f64 / 255.0
    )
}

fn rotate_pt(p: (f64, f64), center: (f64, f64), angle_deg: f64) -> (f64, f64) {
    if angle_deg.abs() <= 0.5 {
        return p;
    }
    let rad = angle_deg.to_radians();
    let (s, c) = rad.sin_cos();
    let dx = p.0 - center.0;
    let dy = p.1 - center.1;
    (center.0 + dx * c - dy * s, center.1 + dx * s + dy * c)
}

/// Convert a `Primitive` into closed cubic Bézier segments (working pixels) so EPS needs no exotic operators.
fn primitive_to_contour(p: &Primitive) -> Contour {
    const KAPPA: f64 = 0.5522847498307936;
    match *p {
        Primitive::Circle { cx, cy, r } => {
            let k = r * KAPPA;
            let p0 = (cx + r, cy);
            let p1 = (cx, cy + r);
            let p2 = (cx - r, cy);
            let p3 = (cx, cy - r);
            Contour {
                start: p0,
                curves: vec![
                    [(cx + r, cy + k), (cx + k, cy + r), p1],
                    [(cx - k, cy + r), (cx - r, cy + k), p2],
                    [(cx - r, cy - k), (cx - k, cy - r), p3],
                    [(cx + k, cy - r), (cx + r, cy - k), p0],
                ],
                prim: None,
            }
        }
        Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
            let (kx, ky) = (rx * KAPPA, ry * KAPPA);
            let center = (cx, cy);
            let r = |q: (f64, f64)| rotate_pt(q, center, angle_deg);
            let p0 = r((cx + rx, cy));
            let p1 = r((cx, cy + ry));
            let p2 = r((cx - rx, cy));
            let p3 = r((cx, cy - ry));
            Contour {
                start: p0,
                curves: vec![
                    [r((cx + rx, cy + ky)), r((cx + kx, cy + ry)), p1],
                    [r((cx - kx, cy + ry)), r((cx - rx, cy + ky)), p2],
                    [r((cx - rx, cy - ky)), r((cx - kx, cy - ry)), p3],
                    [r((cx + kx, cy - ry)), r((cx + rx, cy - ky)), p0],
                ],
                prim: None,
            }
        }
        Primitive::Rect { x, y, w, h, rx, angle_deg } => {
            let center = (x + w / 2.0, y + h / 2.0);
            let r = |q: (f64, f64)| rotate_pt(q, center, angle_deg);
            let line = |a: (f64, f64), b: (f64, f64)| [
                (a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0),
                (a.0 + 2.0 * (b.0 - a.0) / 3.0, a.1 + 2.0 * (b.1 - a.1) / 3.0),
                b,
            ];
            let rad = rx.clamp(0.0, (w * 0.5).min(h * 0.5));
            if rad <= 0.01 {
                let p0 = r((x, y));
                let p1 = r((x + w, y));
                let p2 = r((x + w, y + h));
                let p3 = r((x, y + h));
                Contour {
                    start: p0,
                    curves: vec![line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)],
                    prim: None,
                }
            } else {
                let c = rad * KAPPA;
                let a0 = r((x + rad, y));
                let a1 = r((x + w - rad, y));
                let b0 = r((x + w, y + rad));
                let b1 = r((x + w, y + h - rad));
                let c0 = r((x + w - rad, y + h));
                let c1 = r((x + rad, y + h));
                let d0 = r((x, y + h - rad));
                let d1 = r((x, y + rad));
                Contour {
                    start: a0,
                    curves: vec![
                        line(a0, a1),
                        [r((x + w - rad + c, y)), r((x + w, y + rad - c)), b0],
                        line(b0, b1),
                        [r((x + w, y + h - rad + c)), r((x + w - rad + c, y + h)), c0],
                        line(c0, c1),
                        [r((x + rad - c, y + h)), r((x, y + h - rad + c)), d0],
                        line(d0, d1),
                        [r((x, y + rad - c)), r((x + rad - c, y)), a0],
                    ],
                    prim: None,
                }
            }
        }
    }
}

/// Pure PostScript Level 2 / EPSF-3.0 export directly from `Doc` (no Ghostscript or Inkscape needed).
/// Uses bottom-left PostScript coordinates (`y_ps = total_h - y_svg`), `eofill`, cubic Béziers,
/// optional print trap, strokes/shapes, and optional 4-corner registration marks.
pub fn to_eps(doc: &Doc, opts: &ExportOptions) -> String {
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let m = if opts.registration_marks { REG_MARGIN } else { 0.0 };
    let total_w = doc.width as f64 + 2.0 * m;
    let total_h = doc.height as f64 + 2.0 * m;
    let bb_w = total_w.ceil() as u32;
    let bb_h = total_h.ceil() as u32;

    let mut s = String::new();
    s.push_str("%!PS-Adobe-3.0 EPSF-3.0
");
    s.push_str("%%Creator: Trasea 1.0.0
");
    s.push_str(&format!("%%BoundingBox: 0 0 {bb_w} {bb_h}
"));
    s.push_str(&format!("%%HiResBoundingBox: 0 0 {:.2} {:.2}
", total_w, total_h));
    s.push_str("%%LanguageLevel: 2
");
    s.push_str("%%Pages: 1
");
    s.push_str("%%EndComments
");
    s.push_str("%%BeginProlog
");
    s.push_str("/M { moveto } bind def
");
    s.push_str("/L { lineto } bind def
");
    s.push_str("/C { curveto } bind def
");
    s.push_str("/Z { closepath } bind def
");
    s.push_str("/F { eofill } bind def
");
    s.push_str("/S { stroke } bind def
");
    s.push_str("/RGB { setrgbcolor } bind def
");
    s.push_str("%%EndProlog
");
    s.push_str("%%Page: 1 1
");
    s.push_str("gsave
");

    let map_work = |p: (f64, f64)| -> (f64, f64) { (m + p.0 / k, total_h - (m + p.1 / k)) };
    let map_svg = |x: f64, y: f64| -> (f64, f64) { (x, total_h - y) };

    let emit_contours = |contours: &[Contour], out: &mut String| {
        for c in contours {
            let st = map_work(c.start);
            out.push_str(&format!("{} {} M
", ps_num(st.0), ps_num(st.1)));
            let mut prev = c.start;
            for cv in &c.curves {
                if is_straight(prev, cv[0], cv[1], cv[2], k) {
                    let e = map_work(cv[2]);
                    out.push_str(&format!("{} {} L
", ps_num(e.0), ps_num(e.1)));
                } else {
                    let c1 = map_work(cv[0]);
                    let c2 = map_work(cv[1]);
                    let e = map_work(cv[2]);
                    out.push_str(&format!(
                        "{} {} {} {} {} {} C
",
                        ps_num(c1.0), ps_num(c1.1),
                        ps_num(c2.0), ps_num(c2.1),
                        ps_num(e.0), ps_num(e.1)
                    ));
                }
                prev = cv[2];
            }
            out.push_str("Z
");
        }
    };

    let trap_px = opts.trap_pt.max(0.0) * 96.0 / 72.0;
    let emit_layer = |layer: &Layer, with_trap: bool, out: &mut String| {
        let mut all_contours: Vec<Contour> = layer.contours.clone();
        for p in &layer.prims {
            all_contours.push(primitive_to_contour(p));
        }
        if all_contours.is_empty() {
            return;
        }
        out.push_str(&ps_rgb(layer.color));
        out.push_str("newpath
");
        emit_contours(&all_contours, out);
        if with_trap && trap_px > 0.0 {
            out.push_str(&format!("gsave F grestore 1 setlinejoin {} setlinewidth S
", ps_num(trap_px)));
        } else {
            out.push_str("F
");
        }
    };

    // 1. Latar + 2. Bidang warna
    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    if !opts.strip_background && has_canvas_bg {
        emit_layer(&doc.fields[0], false, &mut s);
    }
    let skip_first = if has_canvas_bg { 1 } else { 0 };
    for layer in doc.fields.iter().skip(skip_first) {
        emit_layer(layer, true, &mut s);
    }

    // 3. Garis
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                emit_layer(l, false, &mut s);
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                emit_layer(l, false, &mut s);
            }
            for sl in &doc.strokes {
                s.push_str(&ps_rgb(sl.color));
                for st in &sl.strokes {
                    if st.curves.is_empty() {
                        continue;
                    }
                    let w_ps = (st.width / k).max(0.1);
                    s.push_str(&format!("1 setlinecap 1 setlinejoin {} setlinewidth newpath
", ps_num(w_ps)));
                    let p0 = map_work(st.curves[0][0]);
                    s.push_str(&format!("{} {} M
", ps_num(p0.0), ps_num(p0.1)));
                    for cv in &st.curves {
                        if is_straight(cv[0], cv[1], cv[2], cv[3], k) {
                            let e = map_work(cv[3]);
                            s.push_str(&format!("{} {} L
", ps_num(e.0), ps_num(e.1)));
                        } else {
                            let c1 = map_work(cv[1]);
                            let c2 = map_work(cv[2]);
                            let e = map_work(cv[3]);
                            s.push_str(&format!(
                                "{} {} {} {} {} {} C
",
                                ps_num(c1.0), ps_num(c1.1),
                                ps_num(c2.0), ps_num(c2.1),
                                ps_num(e.0), ps_num(e.1)
                            ));
                        }
                    }
                    s.push_str("S
");
                }
            }
        }
    }

    // 4. Registration marks (4 corners in outer margin)
    if opts.registration_marks {
        let (w, h) = (doc.width as f64, doc.height as f64);
        let pts = [
            (m / 2.0, m / 2.0),
            (w + 1.5 * m, m / 2.0),
            (m / 2.0, h + 1.5 * m),
            (w + 1.5 * m, h + 1.5 * m),
        ];
        s.push_str("0 0 0 RGB 0.75 setlinewidth
");
        const KAPPA: f64 = 0.5522847498307936;
        let r = 6.0;
        let kr = r * KAPPA;
        for (cx, cy) in pts {
            let (px, py) = map_svg(cx, cy);
            s.push_str(&format!(
                "newpath {} {} M {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C Z S
",
                ps_num(px + r), ps_num(py),
                ps_num(px + r), ps_num(py + kr), ps_num(px + kr), ps_num(py + r), ps_num(px), ps_num(py + r),
                ps_num(px - kr), ps_num(py + r), ps_num(px - r), ps_num(py + kr), ps_num(px - r), ps_num(py),
                ps_num(px - r), ps_num(py - kr), ps_num(px - kr), ps_num(py - r), ps_num(px), ps_num(py - r),
                ps_num(px + kr), ps_num(py - r), ps_num(px + r), ps_num(py - kr), ps_num(px + r), ps_num(py)
            ));
            s.push_str(&format!(
                "newpath {} {} M {} {} L {} {} M {} {} L S
",
                ps_num(px - 9.0), ps_num(py), ps_num(px + 9.0), ps_num(py),
                ps_num(px), ps_num(py - 9.0), ps_num(px), ps_num(py + 9.0)
            ));
        }
    }

    s.push_str("grestore
showpage
%%Trailer
%%EOF
");
    s
}
