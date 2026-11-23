//! Tracing with visioncortex (the engine of VTracer): palette layers and thin lines become spline contours.

use crate::fit::fit_ring;
use crate::Rgb;
use visioncortex::{BinaryImage, CompoundPath, CompoundPathElement, PathSimplifyMode};

#[derive(Debug, Clone)]
pub struct TraceParams {
    /// Turning angle in degrees above which a point is a real corner. Higher = rounder.
    pub corner_deg: f64,
    /// Smoothing of the pixel staircase, in boundary steps (about working pixels).
    pub sigma: f64,
    /// Curve-fit tolerance in working pixels. Higher = fewer nodes.
    pub tol: f64,
    /// Regions smaller than this many working pixels are dropped.
    pub min_area: usize,
}

/// Extras for tracing: basic-shape detection (`prim_scale` = working px per source px) and the correction-brush zones.
#[derive(Default, Clone, Copy)]
pub struct TraceCtx<'a> {
    pub prim_scale: Option<f64>,
    pub zones: Option<&'a crate::brush::BrushMap>,
    /// Palette indices the designer kept on purpose (tiny accents): their layers keep even very small regions.
    pub protect: Option<&'a [bool]>,
}

/// One closed contour made of cubic Beziers, in working-pixel coordinates.
#[derive(Debug, Clone)]
pub struct Contour {
    pub start: (f64, f64),
    pub curves: Vec<[(f64, f64); 3]>,
    /// Set when the outline is a circle, ellipse or rectangle within a strict tolerance (see `primitives`).
    pub prim: Option<crate::primitives::Primitive>,
}

#[derive(Debug, Clone)]
pub struct Layer {
    pub id: String,
    pub color: Rgb,
    pub contours: Vec<Contour>,
    /// A plain rectangle covering the whole canvas (the lowest colour layer).
    pub canvas: bool,
    /// Solid shapes of this layer that were recognised as a circle, ellipse or rectangle (see `primitives`).
    pub prims: Vec<crate::primitives::Primitive>,
}

fn to_contours(cp: &CompoundPath, p: &TraceParams, ctx: &TraceCtx) -> Vec<Contour> {
    let mut out = Vec::new();
    for el in cp.iter() {
        if let CompoundPathElement::PathI32(path) = el {
            if path.path.len() < 4 {
                continue;
            }
            // the boundary walker reports cell corners; +0.5 would be the pixel centre, but the mask edge is the corner
            
            let pts: Vec<(f64, f64)> = path.path.iter().map(|q| (q.x as f64, q.y as f64)).collect();
            // the zone map is in working pixels; the ring is in the one-pixel padded frame of `trace_mask_ex`
            let zone_fn = ctx.zones.map(|bm| move |q: (f64, f64)| bm.at(q.0 - 1.0, q.1 - 1.0));
            let curves = match &zone_fn {
                Some(f) => crate::fit::fit_ring_zoned(&pts, p.sigma, p.tol, p.corner_deg, Some(f)),
                None => fit_ring(&pts, p.sigma, p.tol, p.corner_deg),
            };
            if curves.is_empty() {
                continue;
            }
            // a basic shape is judged on the smoothed raster outline: it does not depend on the curve-fit tolerance
            let prim = ctx.prim_scale.and_then(|s| crate::primitives::detect(&crate::fit::ring_points(&pts, p.sigma), s));
            out.push(Contour { start: curves[0][0], curves: curves.iter().map(|c| [c[1], c[2], c[3]]).collect(), prim });
        }
    }
    out
}

/// Trace a binary mask into contours (outer boundaries and holes), dropping connected regions below `min_area`.
pub fn trace_mask(mask: &BinaryImage, p: &TraceParams) -> Vec<Contour> {
    trace_mask_ex(mask, p, &TraceCtx::default())
}

/// Like `trace_mask`; with `prim_scale` (working px per source px) every contour is also tested for a basic shape.
pub fn trace_mask_ex(mask: &BinaryImage, p: &TraceParams, ctx: &TraceCtx) -> Vec<Contour> {
    // pad by one empty pixel: the boundary walker does not trace along the image edge, so a region that touches the
    // edge would otherwise lose that side of its outline
    let (w, h) = (mask.width, mask.height);
    let mut padded = BinaryImage::new_w_h(w + 2, h + 2);
    for y in 0..h {
        for x in 0..w {
            if mask.get_pixel(x, y) {
                padded.set_pixel(x + 1, y + 1, true);
            }
        }
    }
    let mut out = trace_padded(&padded, p, ctx);
    for c in out.iter_mut() {
        let sh = |q: (f64, f64)| (q.0 - 1.0, q.1 - 1.0);
        c.start = sh(c.start);
        for cv in c.curves.iter_mut() {
            *cv = [sh(cv[0]), sh(cv[1]), sh(cv[2])];
        }
        if let Some(pr) = c.prim.take() {
            c.prim = Some(pr.shifted(-1.0, -1.0));
        }
    }
    out
}

fn trace_padded(mask: &BinaryImage, p: &TraceParams, ctx: &TraceCtx) -> Vec<Contour> {
    let clusters = mask.to_clusters(false);
    let mut out = Vec::new();
    for i in 0..clusters.len() {
        let c = clusters.get_cluster(i);
        if c.size() < p.min_area {
            continue;
        }
        let cp = c.to_compound_path(PathSimplifyMode::None, 0.0, 0.0, 0, 0.0);
        out.extend(to_contours(&cp, p, ctx));
    }
    out
}

/// Stacked colour layers. Colours are ranked by area (largest first); layer i covers every pixel whose colour has
/// rank >= i, so a smaller colour always sits on top of a bigger one and there are no gaps between them.
pub fn trace_layers(labels: &[u8], w: usize, h: usize, palette: &[Rgb], p: &TraceParams) -> Vec<Layer> {
    trace_layers_ex(labels, w, h, palette, p, &TraceCtx::default())
}

/// Like `trace_layers`, and tests every contour for a basic shape when `prim_scale` is given.
pub fn trace_layers_ex(labels: &[u8], w: usize, h: usize, palette: &[Rgb], p: &TraceParams, ctx: &TraceCtx) -> Vec<Layer> {
    let mut area = vec![0usize; palette.len()];
    for &l in labels {
        area[l as usize] += 1;
    }
    let mut order: Vec<usize> = (0..palette.len()).filter(|&i| area[i] > 0).collect();
    order.sort_by(|&a, &b| area[b].cmp(&area[a]).then(a.cmp(&b)));
    let mut rank = vec![usize::MAX; palette.len()];
    for (r, &i) in order.iter().enumerate() {
        rank[i] = r;
    }
    let mut layers = Vec::new();
    for (r, &ci) in order.iter().enumerate() {
        let color = palette[ci];
        let id = crate::hex(color);
        if r == 0 {
            let (wf, hf) = (w as f64, h as f64);
            let line = |a: (f64, f64), b: (f64, f64)| [(a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0), (a.0 + 2.0 * (b.0 - a.0) / 3.0, a.1 + 2.0 * (b.1 - a.1) / 3.0), b];
            let (p0, p1, p2, p3) = ((0.0, 0.0), (wf, 0.0), (wf, hf), (0.0, hf));
            layers.push(Layer { id, color, canvas: true, prims: vec![], contours: vec![Contour { start: p0, curves: vec![line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)], prim: None }] });
            continue;
        }
        let mut mask = BinaryImage::new_w_h(w, h);
        for (i, &l) in labels.iter().enumerate() {
            if rank[l as usize] != usize::MAX && rank[l as usize] >= r {
                mask.set_pixel(i % w, i / w, true);
            }
        }
        let accent = ctx.protect.map_or(false, |pr| pr.get(ci).copied().unwrap_or(false));
        let pp = if accent { TraceParams { min_area: p.min_area.min(8), ..p.clone() } } else { p.clone() };
        let contours = trace_mask_ex(&mask, &pp, ctx);
        if !contours.is_empty() {
            layers.push(Layer { id, color, canvas: false, prims: vec![], contours });
        }
    }
    layers
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> TraceParams {
        TraceParams { corner_deg: 65.0, sigma: 1.5, tol: 1.0, min_area: 10 }
    }

    fn bbox(c: &Contour) -> (f64, f64, f64, f64) {
        let mut pts = vec![c.start];
        for cv in &c.curves {
            pts.extend_from_slice(cv);
        }
        let xs = pts.iter().map(|p| p.0);
        let ys = pts.iter().map(|p| p.1);
        (xs.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max))
    }

    #[test]
    fn region_touching_the_edge_keeps_its_whole_outline() {
        // top 40 rows filled, with a hole inside, on a 100x100 canvas
        let mut m = BinaryImage::new_w_h(100, 100);
        for y in 0..40 {
            for x in 0..100 {
                m.set_pixel(x, y, true);
            }
        }
        for y in 10..20 {
            for x in 30..60 {
                m.set_pixel(x, y, false);
            }
        }
        let cs = trace_mask(&m, &params());
        let b: Vec<_> = cs.iter().map(bbox).collect();
        println!("{b:?}");
        assert!(b.iter().any(|&(x0, x1, y0, y1)| x0 <= 0.5 && x1 >= 99.5 && y0 <= 0.5 && y1 >= 39.5), "outer outline missing: {b:?}");
    }
}

/// Layers are stacked, and a separate island of a small colour shows up as a contour in every layer below its own. An
/// outer contour whose outline is exactly the same as an outer contour in a layer above it is covered completely, so it is
/// dropped: the picture does not change and the file gets lighter. Holes are never dropped (a hole is not covered by
/// anything). Returns how many contours were dropped.
pub fn drop_hidden_duplicates(layers: &mut [Layer]) -> usize {
    use crate::primitives::{bbox, flatten, point_in};
    use std::collections::hash_map::DefaultHasher;
    use std::collections::HashSet;
    use std::hash::{Hash, Hasher};
    let key = |c: &Contour| -> u64 {
        let q = |v: f64| (v * 100.0).round() as i64;
        let mut h = DefaultHasher::new();
        (q(c.start.0), q(c.start.1), c.curves.len()).hash(&mut h);
        for cv in &c.curves {
            for p in cv {
                (q(p.0), q(p.1)).hash(&mut h);
            }
        }
        h.finish()
    };
    let mut seen: HashSet<u64> = HashSet::new();
    let mut dropped = 0;
    for layer in layers.iter_mut().rev().filter(|l| !l.canvas) {
        // even depth = outer outline (not inside an odd number of other contours of the same layer)
        let polys: Vec<Vec<(f64, f64)>> = layer.contours.iter().map(flatten).collect();
        let boxes: Vec<_> = polys.iter().map(|p| bbox(p)).collect();
        let outer: Vec<bool> = (0..polys.len())
            .map(|i| {
                let mut depth = 0;
                for o in 0..polys.len() {
                    if o != i && boxes[o].0 <= boxes[i].0 && boxes[o].1 <= boxes[i].1 && boxes[o].2 >= boxes[i].2 && boxes[o].3 >= boxes[i].3 && !polys[i].is_empty() && point_in(polys[i][0], &polys[o]) {
                        depth += 1;
                    }
                }
                depth % 2 == 0
            })
            .collect();
        let mut mine: Vec<u64> = Vec::new();
        let mut idx = 0;
        layer.contours.retain(|c| {
            let is_outer = outer[idx];
            idx += 1;
            if !is_outer {
                return true;
            }
            let k = key(c);
            if seen.contains(&k) {
                dropped += 1;
                false
            } else {
                mine.push(k);
                true
            }
        });
        seen.extend(mine);
    }
    dropped
}
