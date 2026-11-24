//! Automatic Die-Cut / Kiss-Cut outer offset contour generator (`CutContour`) using exact 2-pass
//! Felzenszwalb-Huttenlocher Euclidean Distance Transform (`O(W*H)`).
//!
//! - Rasterizes all non-background artwork (fields, primitives, lines, strokes) onto an auto-padded
//!   binary silhouette so the offset contour is never clipped at the canvas edges.
//! - Uses isotropic circular Euclidean dilation (`r_px`), optional island bridging (morphological
//!   closing at `2 * r_px -> erode(r_px)`), and 4-connected border flood-fill hole removal so only
//!   smooth outer contours are produced (zero interior cutouts).
//! - Fits smooth closed cubic Bézier curves in `Doc` working-pixel coordinates.

use crate::dxf::{DxfUnit, MM_PER_PX_96DPI};
use crate::export::{primitive_to_contour, Doc, ExportOptions, LinesAs};
use crate::fit::fit_ring;
use crate::trace::Contour;
use std::collections::VecDeque;
use visioncortex::{BinaryImage, CompoundPathElement, PathSimplifyMode};

const INF: f64 = 1e12;

/// Exact 1D squared Euclidean Distance Transform (Felzenszwalb & Huttenlocher, 2012), `O(n)`.
fn edt_1d(f: &[f64]) -> Vec<f64> {
    let n = f.len();
    if n == 0 {
        return Vec::new();
    }
    let mut v = vec![0usize; n];
    let mut z = vec![0.0f64; n + 1];
    let mut k = 0usize;
    v[0] = 0;
    z[0] = f64::NEG_INFINITY;
    z[1] = f64::INFINITY;

    for q in 1..n {
        let mut s = ((f[q] + (q * q) as f64) - (f[v[k]] + (v[k] * v[k]) as f64))
            / (2.0 * (q as f64 - v[k] as f64));
        while s <= z[k] {
            k -= 1;
            s = ((f[q] + (q * q) as f64) - (f[v[k]] + (v[k] * v[k]) as f64))
                / (2.0 * (q as f64 - v[k] as f64));
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f64::INFINITY;
    }

    let mut d = vec![0.0f64; n];
    let mut k2 = 0usize;
    for q in 0..n {
        while z[k2 + 1] < q as f64 {
            k2 += 1;
        }
        let dq = q as f64 - v[k2] as f64;
        d[q] = dq * dq + f[v[k2]];
    }
    d
}

/// Exact 2D squared Euclidean distance from every pixel to the nearest `true` pixel in `mask`.
pub fn edt_2d_sq(mask: &[bool], w: usize, h: usize) -> Vec<f64> {
    let n = w.saturating_mul(h);
    if n == 0 {
        return Vec::new();
    }
    let mut tmp = vec![INF; n];
    let mut col = vec![0.0f64; h];
    for x in 0..w {
        for y in 0..h {
            col[y] = if mask[y * w + x] { 0.0 } else { INF };
        }
        let dcol = edt_1d(&col);
        for y in 0..h {
            tmp[y * w + x] = dcol[y];
        }
    }
    let mut out = vec![INF; n];
    let mut row = vec![0.0f64; w];
    for y in 0..h {
        let off = y * w;
        row.copy_from_slice(&tmp[off..off + w]);
        let drow = edt_1d(&row);
        out[off..off + w].copy_from_slice(&drow);
    }
    out
}

/// Isotropic circular dilation of radius `r_px` using exact 2D EDT.
fn dilate_edt(mask: &[bool], w: usize, h: usize, r_px: f64) -> Vec<bool> {
    if r_px <= 0.0 {
        return mask.to_vec();
    }
    let r2 = r_px * r_px;
    edt_2d_sq(mask, w, h).into_iter().map(|d2| d2 <= r2).collect()
}

/// Isotropic circular erosion of radius `r_px` using exact 2D EDT on the complement mask.
fn erode_edt(mask: &[bool], w: usize, h: usize, r_px: f64) -> Vec<bool> {
    if r_px <= 0.0 {
        return mask.to_vec();
    }
    let inv: Vec<bool> = mask.iter().map(|&b| !b).collect();
    let r2 = r_px * r_px;
    edt_2d_sq(&inv, w, h).into_iter().map(|d2| d2 > r2).collect()
}

/// Fill all interior holes (pixels not 4-connected to the image border in `!mask`).
fn fill_holes(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let n = w * h;
    if n == 0 {
        return Vec::new();
    }
    let mut outside = vec![false; n];
    let mut queue = VecDeque::new();
    let push = |idx: usize, outside: &mut [bool], queue: &mut VecDeque<usize>| {
        if !mask[idx] && !outside[idx] {
            outside[idx] = true;
            queue.push_back(idx);
        }
    };
    for x in 0..w {
        push(x, &mut outside, &mut queue);
        push((h - 1) * w + x, &mut outside, &mut queue);
    }
    for y in 1..h.saturating_sub(1) {
        push(y * w, &mut outside, &mut queue);
        push(y * w + (w - 1), &mut outside, &mut queue);
    }
    while let Some(idx) = queue.pop_front() {
        let x = idx % w;
        let y = idx / w;
        if x > 0 { push(idx - 1, &mut outside, &mut queue); }
        if x + 1 < w { push(idx + 1, &mut outside, &mut queue); }
        if y > 0 { push(idx - w, &mut outside, &mut queue); }
        if y + 1 < h { push(idx + w, &mut outside, &mut queue); }
    }
    outside.into_iter().map(|is_out| !is_out).collect()
}

/// Flatten a cubic Bézier curve into line segments within tolerance `tol_sq`.
fn flatten_cubic(
    p0: (f64, f64),
    c1: (f64, f64),
    c2: (f64, f64),
    p3: (f64, f64),
    tol_sq: f64,
    depth: u8,
    out: &mut Vec<(f64, f64)>,
) {
    let (dx, dy) = (p3.0 - p0.0, p3.1 - p0.1);
    let len2 = dx * dx + dy * dy;
    let dist_sq = |p: (f64, f64)| -> f64 {
        if len2 <= 1e-12 {
            (p.0 - p0.0).powi(2) + (p.1 - p0.1).powi(2)
        } else {
            let t = (((p.0 - p0.0) * dx + (p.1 - p0.1) * dy) / len2).clamp(0.0, 1.0);
            let qx = p0.0 + t * dx;
            let qy = p0.1 + t * dy;
            (p.0 - qx).powi(2) + (p.1 - qy).powi(2)
        }
    };
    if depth >= 7 || (dist_sq(c1) <= tol_sq && dist_sq(c2) <= tol_sq) {
        out.push(p3);
        return;
    }
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let p01 = mid(p0, c1);
    let p12 = mid(c1, c2);
    let p23 = mid(c2, p3);
    let p012 = mid(p01, p12);
    let p123 = mid(p12, p23);
    let p0123 = mid(p012, p123);
    flatten_cubic(p0, p01, p012, p0123, tol_sq, depth + 1, out);
    flatten_cubic(p0123, p123, p23, p3, tol_sq, depth + 1, out);
}

/// Scanline-fill a set of closed polygons (even-odd rule) into `mask` (OR-ed in).
fn rasterize_contours_evenodd(
    contours: &[Contour],
    map_work: &dyn Fn((f64, f64)) -> (f64, f64),
    mask: &mut [bool],
    w: usize,
    h: usize,
) {
    if contours.is_empty() {
        return;
    }
    let mut edges: Vec<((f64, f64), (f64, f64))> = Vec::new();
    let mut min_y = f64::INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for c in contours {
        let mut pts = Vec::with_capacity(c.curves.len() * 4 + 2);
        let p0 = map_work(c.start);
        pts.push(p0);
        let mut prev = p0;
        for cv in &c.curves {
            let c1 = map_work(cv[0]);
            let c2 = map_work(cv[1]);
            let p3 = map_work(cv[2]);
            flatten_cubic(prev, c1, c2, p3, 0.25, 0, &mut pts);
            prev = p3;
        }
        if pts.len() < 3 {
            continue;
        }
        for i in 0..pts.len() {
            let a = pts[i];
            let b = pts[(i + 1) % pts.len()];
            if (a.1 - b.1).abs() < 1e-9 {
                continue;
            }
            min_y = min_y.min(a.1).min(b.1);
            max_y = max_y.max(a.1).max(b.1);
            edges.push((a, b));
        }
    }
    if edges.is_empty() {
        return;
    }
    let y0 = (min_y.floor() as isize).max(0) as usize;
    let y1 = (max_y.ceil() as isize).min(h as isize - 1) as usize;
    let mut xs: Vec<f64> = Vec::new();
    for y in y0..=y1 {
        let sy = y as f64 + 0.5;
        xs.clear();
        for &(a, b) in &edges {
            if (a.1 <= sy && b.1 > sy) || (b.1 <= sy && a.1 > sy) {
                let x = a.0 + (sy - a.1) * (b.0 - a.0) / (b.1 - a.1);
                xs.push(x);
            }
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let row = y * w;
        let mut i = 0;
        while i + 1 < xs.len() {
            let x_start = (xs[i].floor() as isize).max(0) as usize;
            let x_end = (xs[i + 1].ceil() as isize).min(w as isize - 1) as usize;
            for x in x_start..=x_end {
                mask[row + x] = true;
            }
            i += 2;
        }
    }
}

/// Stamp a circular disk of radius `r` at `(cx, cy)` onto `mask`.
fn stamp_disk(mask: &mut [bool], w: usize, h: usize, cx: f64, cy: f64, r: f64) {
    let rad = r.max(0.5);
    let r2 = rad * rad;
    let x0 = ((cx - rad).floor() as isize).max(0) as usize;
    let x1 = ((cx + rad).ceil() as isize).min(w as isize - 1) as usize;
    let y0 = ((cy - rad).floor() as isize).max(0) as usize;
    let y1 = ((cy + rad).ceil() as isize).min(h as isize - 1) as usize;
    for y in y0..=y1 {
        let dy = y as f64 + 0.5 - cy;
        let row = y * w;
        for x in x0..=x1 {
            let dx = x as f64 + 0.5 - cx;
            if dx * dx + dy * dy <= r2 {
                mask[row + x] = true;
            }
        }
    }
}

/// Rasterize an open Bézier stroke onto `mask`.
fn rasterize_stroke(
    curves: &[[(f64, f64); 4]],
    half_width_px: f64,
    map_work: &dyn Fn((f64, f64)) -> (f64, f64),
    mask: &mut [bool],
    w: usize,
    h: usize,
) {
    if curves.is_empty() {
        return;
    }
    let mut pts = Vec::with_capacity(curves.len() * 6 + 2);
    let p0 = map_work(curves[0][0]);
    pts.push(p0);
    for cv in curves {
        let a = map_work(cv[0]);
        let c1 = map_work(cv[1]);
        let c2 = map_work(cv[2]);
        let b = map_work(cv[3]);
        flatten_cubic(a, c1, c2, b, 0.25, 0, &mut pts);
    }
    let step = (half_width_px * 0.5).clamp(0.5, 2.0);
    for win in pts.windows(2) {
        let (a, b) = (win[0], win[1]);
        let dist = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let n = (dist / step).ceil().max(1.0) as usize;
        for i in 0..=n {
            let t = i as f64 / n as f64;
            stamp_disk(mask, w, h, a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t, half_width_px);
        }
    }
}

/// Build smooth outer die-cut contours around all non-background artwork in `doc`.
/// Returned contours are in `Doc` working-pixel coordinates (may extend slightly outside `[0, w] x [0, h]`).
pub fn build_die_cut_contours(doc: &Doc, opts: &ExportOptions) -> Vec<Contour> {
    if !opts.die_cut_contour || doc.width == 0 || doc.height == 0 {
        return Vec::new();
    }
    let offset_mm = if opts.die_cut_offset_mm.is_finite() {
        opts.die_cut_offset_mm.clamp(0.5, 10.0)
    } else {
        2.0
    };
    build_offset_contours_ex(doc, opts, offset_mm, 96.0)
}

/// Build smooth sticker backing contours (`die_cut_offset_mm + sticker_bleed_mm`) concentric with `CutContour`.
pub fn build_sticker_backing_contours(doc: &Doc, opts: &ExportOptions) -> Vec<Contour> {
    if !opts.die_cut_contour || !opts.sticker_backing || doc.width == 0 || doc.height == 0 {
        return Vec::new();
    }
    let base_mm = if opts.die_cut_offset_mm.is_finite() {
        opts.die_cut_offset_mm.clamp(0.5, 10.0)
    } else {
        2.0
    };
    let bleed_mm = if opts.sticker_bleed_mm.is_finite() {
        opts.sticker_bleed_mm.clamp(0.0, 3.0)
    } else {
        0.0
    };
    build_offset_contours_ex(doc, opts, base_mm + bleed_mm, 96.0)
}

/// Generalized isotropic EDT outer offset contour builder at `offset_mm` and `dpi`.
pub fn build_offset_contours(doc: &Doc, offset_mm: f64, dpi: f64) -> Vec<Contour> {
    let opts = ExportOptions {
        die_cut_contour: true,
        die_cut_offset_mm: offset_mm,
        ..ExportOptions::default()
    };
    build_offset_contours_ex(doc, &opts, offset_mm, dpi)
}

/// Generalized isotropic EDT outer offset contour builder respecting `opts` (lines_as, bridge_islands, target width).
pub fn build_offset_contours_ex(doc: &Doc, opts: &ExportOptions, offset_mm: f64, dpi: f64) -> Vec<Contour> {
    if doc.width == 0 || doc.height == 0 {
        return Vec::new();
    }
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let dpi_mm_per_px = if dpi.is_finite() && dpi > 0.0 { 25.4 / dpi } else { MM_PER_PX_96DPI };

    // Convert physical offset (mm) to source pixels.
    let mm_per_px = match opts.dxf_unit {
        DxfUnit::Mm => match opts.dxf_target_width_mm {
            Some(tw) if tw.is_finite() && tw > 0.0 => tw as f64 / doc.width as f64,
            _ => dpi_mm_per_px,
        },
        DxfUnit::Px => dpi_mm_per_px,
    };
    let offset_mm = if offset_mm.is_finite() {
        offset_mm.clamp(0.0, 15.0)
    } else {
        2.0
    };
    let offset_src_px = offset_mm / mm_per_px.max(1e-6);

    // Choose a clean raster resolution capped at ~900 px longest side for fast, smooth EDT.
    let raw_w = doc.width as f64 + 2.0 * offset_src_px + 16.0;
    let raw_h = doc.height as f64 + 2.0 * offset_src_px + 16.0;
    let rs = (900.0 / raw_w.max(raw_h)).min(1.0).max(0.25);
    let r_px = (offset_src_px * rs).max(1.0);
    // Extra safety margin when bridge_islands runs a 2*r_px closing step
    let pad = if opts.die_cut_bridge_islands {
        (2.0 * r_px).ceil() as usize + 8
    } else {
        r_px.ceil() as usize + 8
    };
    let rw = ((doc.width as f64 * rs).ceil() as usize) + 2 * pad;
    let rh = ((doc.height as f64 * rs).ceil() as usize) + 2 * pad;
    if rw == 0 || rh == 0 {
        return Vec::new();
    }

    let map_work = |p: (f64, f64)| -> (f64, f64) {
        ((p.0 / k) * rs + pad as f64, (p.1 / k) * rs + pad as f64)
    };

    let mut silhouette = vec![false; rw * rh];

    // 1. Non-canvas colour fields (contours + primitives)
    for layer in &doc.fields {
        if layer.canvas {
            continue;
        }
        let mut all = layer.contours.clone();
        for p in &layer.prims {
            all.push(primitive_to_contour(p));
        }
        rasterize_contours_evenodd(&all, &map_work, &mut silhouette, rw, rh);
    }

    // 2. Thin lines / centerline strokes
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                rasterize_contours_evenodd(&l.contours, &map_work, &mut silhouette, rw, rh);
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                rasterize_contours_evenodd(&l.contours, &map_work, &mut silhouette, rw, rh);
            }
            for sl in &doc.strokes {
                for st in &sl.strokes {
                    let half_w = ((st.width / k) * rs * 0.5).max(0.75);
                    rasterize_stroke(&st.curves, half_w, &map_work, &mut silhouette, rw, rh);
                }
            }
        }
    }

    if !silhouette.iter().any(|&b| b) {
        return Vec::new();
    }

    // 3. Isotropic EDT offset + optional island bridging + hole fill
    let solid = if opts.die_cut_bridge_islands {
        // Dilate by 2*r -> fill internal gaps -> erode by r:
        // bridges islands within <= 2*r gap after dilation while keeping exact r_px offset around outer shapes.
        let wide = dilate_edt(&silhouette, rw, rh, 2.0 * r_px);
        let filled_wide = fill_holes(&wide, rw, rh);
        let bridged = erode_edt(&filled_wide, rw, rh, r_px);
        fill_holes(&bridged, rw, rh)
    } else {
        let dilated = dilate_edt(&silhouette, rw, rh, r_px);
        fill_holes(&dilated, rw, rh)
    };

    // 4. Extract outer boundaries via visioncortex & fit smooth closed Béziers
    let mut bin = BinaryImage::new_w_h(rw, rh);
    for (idx, &b) in solid.iter().enumerate() {
        if b {
            bin.set_pixel(idx % rw, idx / rw, true);
        }
    }

    let clusters = bin.to_clusters(false);
    let mut out = Vec::new();
    let pad_f = pad as f64;
    let inv_rs = 1.0 / rs;

    for i in 0..clusters.len() {
        let cluster = clusters.get_cluster(i);
        if cluster.size() < 4 {
            continue;
        }
        let cp = cluster.to_compound_path(PathSimplifyMode::None, 0.0, 0.0, 0, 0.0);
        // Take the first (outermost) closed path of this hole-free cluster.
        for el in cp.iter() {
            if let CompoundPathElement::PathI32(path) = el {
                if path.path.len() < 4 {
                    continue;
                }
                let pts: Vec<(f64, f64)> = path
                    .path
                    .iter()
                    .map(|q| (((q.x as f64) - pad_f) * inv_rs * k, ((q.y as f64) - pad_f) * inv_rs * k))
                    .collect();
                let sigma = (2.0 * inv_rs * k).max(1.5);
                let tol = (0.75 * inv_rs * k).max(0.5);
                let curves = fit_ring(&pts, sigma, tol, 85.0);
                if !curves.is_empty() {
                    out.push(Contour {
                        start: curves[0][0],
                        curves: curves.iter().map(|c| [c[1], c[2], c[3]]).collect(),
                        prim: None,
                    });
                }
                break;
            }
        }
    }

    out
}

/// Compute the expanded SVG bounding box `(min_x, min_y, max_x, max_y)` in SVG pixels covering
/// `[0, doc.width] x [0, doc.height]` plus all `die_cut` contours (with a small 2 px safety margin
/// when contours extend beyond the original canvas).
pub fn die_cut_bounds_svg(doc: &Doc, contours: &[Contour]) -> (f64, f64, f64, f64) {
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let mut min_x = 0.0f64;
    let mut min_y = 0.0f64;
    let mut max_x = doc.width as f64;
    let mut max_y = doc.height as f64;
    let mut extended = false;

    for c in contours {
        let mut consider = |p: (f64, f64)| {
            let sx = p.0 / k;
            let sy = p.1 / k;
            if sx < 0.0 || sy < 0.0 || sx > doc.width as f64 || sy > doc.height as f64 {
                extended = true;
            }
            min_x = min_x.min(sx);
            min_y = min_y.min(sy);
            max_x = max_x.max(sx);
            max_y = max_y.max(sy);
        };
        consider(c.start);
        for cv in &c.curves {
            consider(cv[0]);
            consider(cv[1]);
            consider(cv[2]);
        }
    }

    if extended {
        let pad = 2.0;
        if min_x < 0.0 { min_x = (min_x - pad).floor(); }
        if min_y < 0.0 { min_y = (min_y - pad).floor(); }
        if max_x > doc.width as f64 { max_x = (max_x + pad).ceil(); }
        if max_y > doc.height as f64 { max_y = (max_y + pad).ceil(); }
    }
    (min_x, min_y, max_x, max_y)
}
