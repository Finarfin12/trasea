//! Open an `.svg` file directly as a `Doc` (Trasea 1.3.0), without tracing: `usvg` resolves every `transform`,
//! `viewBox`, `<use>`, `<circle>`, `<ellipse>`, `<rect>`, `<polygon>`, `<polyline>`, `<line>`, arc and quadratic
//! segment into plain cubic Beziers in document coordinates (`doc.scale == 1.0`).
//!
//! * A path with a solid fill becomes closed contours of a colour layer. Paint order is preserved: consecutive
//!   paths of the same colour share a layer only when they do not overlap, otherwise a new layer starts on top.
//! * A path with a solid stroke becomes open stroke(s) of a `StrokeLayer` with the stroke width.
//! * Gradients, patterns, embedded images and text without fonts cannot be represented: they are skipped and
//!   reported in `notes` (the rest of the drawing still loads).
//!
//! The resulting layers carry the same deterministic `data-trasea-id` scheme (`f:`, `s:`) as a traced drawing, so
//! path edits, node edits, die-cut contours and every export work on an imported SVG exactly like on a trace.

use crate::export::{Doc, StrokeData, StrokeLayer};
use crate::fit::Bezier;
use crate::trace::{Contour, Layer};
use crate::{hex, Error, Result, Rgb};
use std::path::Path;
use usvg::tiny_skia_path::{PathSegment, Transform};

/// Largest accepted document side (pixels): protects against absurd `width`/`height` values.
const MAX_SIDE: f64 = 20_000.0;

#[derive(Debug, Clone)]
pub struct SvgImport {
    pub doc: Doc,
    /// Friendly messages about what could not be imported (never an error).
    pub notes: Vec<String>,
}

/// Read and parse an `.svg` file.
pub fn load_svg_doc(path: &Path) -> Result<SvgImport> {
    let bytes = std::fs::read(path).map_err(|e| Error::Load(format!("{}: {e}", path.display())))?;
    parse_svg_doc(&bytes)
}

/// Parse SVG bytes (plain text, UTF-8 / UTF-16 is not needed; `.svgz` is handled by `usvg`).
pub fn parse_svg_doc(bytes: &[u8]) -> Result<SvgImport> {
    if bytes.iter().all(|b| b.is_ascii_whitespace()) {
        return Err(Error::Load("file SVG kosong".into()));
    }
    let opt = usvg::Options::default();
    let tree = usvg::Tree::from_data(bytes, &opt).map_err(|e| Error::Load(format!("SVG tidak valid: {e}")))?;
    let size = tree.size();
    let (w, h) = (size.width() as f64, size.height() as f64);
    if !(w.is_finite() && h.is_finite()) || w < 1.0 || h < 1.0 || w > MAX_SIDE || h > MAX_SIDE {
        return Err(Error::Load(format!("ukuran SVG tidak masuk akal ({w} x {h})")));
    }

    let mut ctx = Ctx::default();
    walk(tree.root(), &mut ctx);

    let mut notes = Vec::new();
    if ctx.gradients > 0 {
        notes.push(format!("{} bentuk dengan gradien/pola dilewati (hanya warna solid yang didukung).", ctx.gradients));
    }
    if ctx.images > 0 {
        notes.push(format!("{} gambar raster tertanam dilewati; hanya path vektor yang dimuat.", ctx.images));
    }
    if ctx.text_skipped > 0 {
        notes.push(format!("{} teks tanpa font tersedia dilewati; ubah teks ke outline (path) di editor asal.", ctx.text_skipped));
    }
    if ctx.fill_after_stroke {
        notes.push("Garis (stroke) selalu digambar di atas semua warna isi, sehingga urutan tumpuk stroke dan isi bisa berbeda dari file asal.".into());
    }
    if ctx.layers.is_empty() && ctx.strokes.is_empty() {
        notes.push("Tidak ada path vektor berwarna solid yang bisa dimuat dari SVG ini.".into());
    }

    let doc = Doc {
        width: w.ceil() as u32,
        height: h.ceil() as u32,
        scale: 1.0,
        fields: ctx.layers,
        line_shapes: Vec::new(),
        line_fallback: Vec::new(),
        strokes: ctx.strokes,
    };
    Ok(SvgImport { doc, notes })
}

#[derive(Default)]
struct Ctx {
    layers: Vec<Layer>,
    strokes: Vec<StrokeLayer>,
    gradients: usize,
    images: usize,
    text_skipped: usize,
    /// A solid fill came after a stroke in the document: strokes are always drawn above all fills in a `Doc`.
    fill_after_stroke: bool,
}

type BBox = (f64, f64, f64, f64);

fn bbox_of(c: &Contour) -> BBox {
    let (mut x0, mut y0, mut x1, mut y1) = (c.start.0, c.start.1, c.start.0, c.start.1);
    for cv in &c.curves {
        for p in cv {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    (x0, y0, x1, y1)
}

fn bbox_overlap(a: BBox, b: BBox) -> bool {
    a.0 <= b.2 && b.0 <= a.2 && a.1 <= b.3 && b.1 <= a.3
}

fn solid(paint: &usvg::Paint) -> Option<Rgb> {
    match paint {
        usvg::Paint::Color(c) => Some([c.red, c.green, c.blue]),
        _ => None,
    }
}

fn walk(group: &usvg::Group, ctx: &mut Ctx) {
    for node in group.children() {
        match node {
            usvg::Node::Group(g) => walk(g, ctx),
            usvg::Node::Path(p) => add_path(p, ctx),
            usvg::Node::Image(_) => ctx.images += 1,
            usvg::Node::Text(t) => {
                let flat = t.flattened();
                if flat.children().is_empty() {
                    ctx.text_skipped += 1;
                } else {
                    walk(flat, ctx);
                }
            }
        }
    }
}

/// A flattened sub-path: cubic segments and whether it was closed.
struct SubPath {
    curves: Vec<Bezier>,
    closed: bool,
}

fn line_bezier(a: (f64, f64), b: (f64, f64)) -> Bezier {
    let d = (b.0 - a.0, b.1 - a.1);
    [a, (a.0 + d.0 / 3.0, a.1 + d.1 / 3.0), (a.0 + 2.0 * d.0 / 3.0, a.1 + 2.0 * d.1 / 3.0), b]
}

fn subpaths_of(path: &usvg::tiny_skia_path::Path, ts: Transform) -> Vec<SubPath> {
    let Some(path) = path.clone().transform(ts) else { return Vec::new() };
    let pt = |p: usvg::tiny_skia_path::Point| (p.x as f64, p.y as f64);
    let mut out: Vec<SubPath> = Vec::new();
    let mut cur: Option<SubPath> = None;
    let mut start = (0.0, 0.0);
    let mut last = (0.0, 0.0);
    let finish = |sp: Option<SubPath>, out: &mut Vec<SubPath>| {
        if let Some(sp) = sp {
            if !sp.curves.is_empty() {
                out.push(sp);
            }
        }
    };
    for seg in path.segments() {
        match seg {
            PathSegment::MoveTo(p) => {
                finish(cur.take(), &mut out);
                start = pt(p);
                last = start;
                cur = Some(SubPath { curves: Vec::new(), closed: false });
            }
            PathSegment::LineTo(p) => {
                let q = pt(p);
                if let Some(sp) = cur.as_mut() {
                    if (q.0 - last.0).abs() > 1e-9 || (q.1 - last.1).abs() > 1e-9 {
                        sp.curves.push(line_bezier(last, q));
                    }
                }
                last = q;
            }
            PathSegment::QuadTo(c, p) => {
                let (c, q) = (pt(c), pt(p));
                if let Some(sp) = cur.as_mut() {
                    let c1 = (last.0 + 2.0 / 3.0 * (c.0 - last.0), last.1 + 2.0 / 3.0 * (c.1 - last.1));
                    let c2 = (q.0 + 2.0 / 3.0 * (c.0 - q.0), q.1 + 2.0 / 3.0 * (c.1 - q.1));
                    sp.curves.push([last, c1, c2, q]);
                }
                last = q;
            }
            PathSegment::CubicTo(c1, c2, p) => {
                let q = pt(p);
                if let Some(sp) = cur.as_mut() {
                    sp.curves.push([last, pt(c1), pt(c2), q]);
                }
                last = q;
            }
            PathSegment::Close => {
                if let Some(mut sp) = cur.take() {
                    if (last.0 - start.0).abs() > 1e-9 || (last.1 - start.1).abs() > 1e-9 {
                        sp.curves.push(line_bezier(last, start));
                    }
                    sp.closed = true;
                    out.push(sp);
                }
                last = start;
                // a new sub-path may continue from the start point without a MoveTo
                cur = Some(SubPath { curves: Vec::new(), closed: false });
            }
        }
    }
    finish(cur.take(), &mut out);
    out
}

/// Make sure a closed contour has at least two segments (a node path needs 2+ anchors).
fn closed_curves(mut c: Vec<Bezier>) -> Option<Vec<Bezier>> {
    if c.is_empty() {
        return None;
    }
    if c.len() == 1 {
        // split the single (curved) segment in two halves
        let b = c[0];
        let mid = |p: (f64, f64), q: (f64, f64)| ((p.0 + q.0) / 2.0, (p.1 + q.1) / 2.0);
        let (ab, bc, cd) = (mid(b[0], b[1]), mid(b[1], b[2]), mid(b[2], b[3]));
        let (abc, bcd) = (mid(ab, bc), mid(bc, cd));
        let m = mid(abc, bcd);
        c = vec![[b[0], ab, abc, m], [m, bcd, cd, b[3]]];
    }
    Some(c)
}

fn add_path(p: &usvg::Path, ctx: &mut Ctx) {
    if !p.is_visible() {
        return;
    }
    let ts = p.abs_transform();
    let subs = subpaths_of(p.data(), ts);
    if subs.is_empty() {
        return;
    }

    if let Some(fill) = p.fill() {
        match solid(fill.paint()) {
            Some(rgb) => {
                let mut contours: Vec<Contour> = Vec::new();
                for sp in &subs {
                    // an open sub-path is filled as if it were closed
                    let mut curves = sp.curves.clone();
                    if !sp.closed {
                        if let (Some(f), Some(l)) = (curves.first(), curves.last()) {
                            let (a, b) = (l[3], f[0]);
                            if (a.0 - b.0).abs() > 1e-9 || (a.1 - b.1).abs() > 1e-9 {
                                curves.push(line_bezier(a, b));
                            }
                        }
                    }
                    if let Some(c) = closed_curves(curves) {
                        contours.push(Contour {
                            start: c[0][0],
                            curves: c.iter().map(|b| [b[1], b[2], b[3]]).collect(),
                            prim: None,
                        });
                    }
                }
                if !contours.is_empty() {
                    // Paint order is kept exactly: a path joins the LAST layer only when it has the same colour and does
                    // not overlap anything already in it (a layer is drawn with `evenodd`, so overlapping shapes of the
                    // same colour would punch holes into each other). Otherwise it starts a new layer on top.
                    let join = ctx.layers.last().map_or(false, |l| {
                        l.color == rgb
                            && contours.iter().all(|c| {
                                let b = bbox_of(c);
                                l.contours.iter().all(|o| !bbox_overlap(b, bbox_of(o)))
                            })
                    });
                    if !join {
                        ctx.layers.push(Layer { id: hex(rgb), color: rgb, contours: Vec::new(), canvas: false, prims: Vec::new() });
                    }
                    if !ctx.strokes.is_empty() {
                        ctx.fill_after_stroke = true;
                    }
                    ctx.layers.last_mut().unwrap().contours.extend(contours);
                }
            }
            None => ctx.gradients += 1,
        }
    }

    if let Some(stroke) = p.stroke() {
        match solid(stroke.paint()) {
            Some(rgb) => {
                // stroke width in document pixels: the transform may scale
                let s = ((ts.sx * ts.sy - ts.kx * ts.ky).abs() as f64).sqrt();
                let width = (stroke.width().get() as f64 * if s > 0.0 { s } else { 1.0 }).max(0.01);
                let list: Vec<StrokeData> = subs
                    .iter()
                    .filter(|sp| !sp.curves.is_empty())
                    .map(|sp| StrokeData { curves: sp.curves.clone(), width })
                    .collect();
                if !list.is_empty() {
                    match ctx.strokes.iter_mut().find(|l| l.color == rgb) {
                        Some(l) => l.strokes.extend(list),
                        None => ctx.strokes.push(StrokeLayer { color: rgb, strokes: list }),
                    }
                }
            }
            None => ctx.gradients += 1,
        }
    }
}
