//! Export: clean SVG 1.1 (one `<g id="#RRGGBB">` per colour, `fill` on the path, no CSS/filter/transform) and PDF.

use crate::trace::{Contour, Layer};
use crate::{hex, Error, Result};

/// The traced drawing. Coordinates inside the layers are working pixels; `scale` = working / source size.
#[derive(Debug, Clone)]
pub struct Doc {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    /// Colour fields, bottom to top.
    pub fields: Vec<Layer>,
    /// Thin lines, stacked above the fields.
    pub lines: Vec<Layer>,
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

fn path_d(contours: &[Contour], k: f64) -> String {
    let mut d = String::new();
    for c in contours {
        d.push('M');
        d.push_str(&pt(c.start, k));
        for cv in &c.curves {
            d.push('C');
            d.push_str(&pt(cv[0], k));
            d.push(' ');
            d.push_str(&pt(cv[1], k));
            d.push(' ');
            d.push_str(&pt(cv[2], k));
        }
        d.push('Z');
    }
    d
}

pub fn to_svg(doc: &Doc) -> String {
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n",
        w = doc.width,
        h = doc.height
    ));
    let mut emit = |layer: &Layer, id: String| {
        let color = hex(layer.color);
        s.push_str(&format!("<g id=\"{id}\">\n<path fill=\"{color}\" fill-rule=\"evenodd\" d=\"{}\"/>\n</g>\n", path_d(&layer.contours, doc.scale)));
    };
    for l in &doc.fields {
        emit(l, l.id.clone());
    }
    for l in &doc.lines {
        emit(l, format!("lines-{}", l.id));
    }
    s.push_str("</svg>\n");
    s
}

/// SVG text -> PDF bytes (page size = drawing size).
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = svg2pdf::usvg::Tree::from_str(svg, &opt).map_err(|e| Error::Other(format!("cannot read the SVG for PDF export: {e}")))?;
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default()).map_err(|e| Error::Other(format!("PDF export failed: {e}")))
}
