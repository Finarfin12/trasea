//! Export: SVG with named layers (`Latar` > `Bidang warna` > `Garis`), PDF, and an Adobe Swatch Exchange palette (.ase).

use crate::centerline::Stroke;
use crate::primitives::Primitive;
use crate::trace::{Contour, Layer};
use crate::{hex, Error, Result, Rgb};

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
    /// Colour fields, bottom to top (the first one is the background rectangle).
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
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions { lines_as: LinesAs::Stroke, trap_pt: 0.0 }
    }
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
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n",
        w = doc.width,
        h = doc.height
    ));
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

    // 1. Latar: the background colour (a plain rectangle)
    s.push_str("<g id=\"Latar\" data-name=\"Latar\">\n");
    let mut idx = 1;
    if let Some(bg) = doc.fields.first() {
        field(bg, idx, false, &mut s);
        idx += 1;
    }
    s.push_str("</g>\n");
    // 2. Bidang warna: one sub-group per colour
    s.push_str("<g id=\"Bidang_warna\" data-name=\"Bidang warna\">\n");
    for layer in doc.fields.iter().skip(1) {
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
    s.push_str("</g>\n</svg>\n");
    s
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
