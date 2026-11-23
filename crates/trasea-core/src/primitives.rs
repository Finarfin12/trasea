//! Basic shape recogniser. A closed contour without holes that matches a circle, an ellipse or a (rounded) rectangle
//! closely is exported as a pure `<circle>`, `<ellipse>` or `<rect>` instead of a Bezier path. The tolerances are strict
//! on purpose: an organic shape that is only roughly round must stay a path.

use crate::fit::P;
use crate::trace::{Contour, Layer};

#[derive(Debug, Clone, PartialEq)]
pub enum Primitive {
    Circle { cx: f64, cy: f64, r: f64 },
    /// `angle_deg` rotates the ellipse around its centre (the `rx` axis is the one that is rotated).
    Ellipse { cx: f64, cy: f64, rx: f64, ry: f64, angle_deg: f64 },
    /// `x`, `y`, `w`, `h` describe the box before rotating it by `angle_deg` around its centre; `rx` rounds the corners.
    Rect { x: f64, y: f64, w: f64, h: f64, rx: f64, angle_deg: f64 },
}

impl Primitive {
    /// The same shape moved by (dx, dy).
    pub fn shifted(&self, dx: f64, dy: f64) -> Primitive {
        match *self {
            Primitive::Circle { cx, cy, r } => Primitive::Circle { cx: cx + dx, cy: cy + dy, r },
            Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => Primitive::Ellipse { cx: cx + dx, cy: cy + dy, rx, ry, angle_deg },
            Primitive::Rect { x, y, w, h, rx, angle_deg } => Primitive::Rect { x: x + dx, y: y + dy, w, h, rx, angle_deg },
        }
    }

    /// The mirror image across a vertical axis (`vertical` = true: the line x = `axis`) or a horizontal one (y = `axis`).
    pub fn mirrored(&self, vertical: bool, axis: f64) -> Primitive {
        let m = |v: f64| 2.0 * axis - v;
        match *self {
            Primitive::Circle { cx, cy, r } => if vertical { Primitive::Circle { cx: m(cx), cy, r } } else { Primitive::Circle { cx, cy: m(cy), r } },
            Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
                if vertical { Primitive::Ellipse { cx: m(cx), cy, rx, ry, angle_deg: -angle_deg } } else { Primitive::Ellipse { cx, cy: m(cy), rx, ry, angle_deg: -angle_deg } }
            }
            Primitive::Rect { x, y, w, h, rx, angle_deg } => {
                if vertical { Primitive::Rect { x: m(x + w), y, w, h, rx, angle_deg: -angle_deg } } else { Primitive::Rect { x, y: m(y + h), w, h, rx, angle_deg: -angle_deg } }
            }
        }
    }

    /// True when `other` is the same shape within `tol` px (used to count a shape once when layers repeat it).
    pub fn same_as(&self, other: &Primitive, tol: f64) -> bool {
        match (self, other) {
            (Primitive::Circle { cx: a, cy: b, r: c }, Primitive::Circle { cx: d, cy: e, r: f }) => (a - d).abs() <= tol && (b - e).abs() <= tol && (c - f).abs() <= tol,
            (Primitive::Ellipse { cx: a, cy: b, rx: c, ry: d, .. }, Primitive::Ellipse { cx: e, cy: f, rx: g, ry: h, .. }) => (a - e).abs() <= tol && (b - f).abs() <= tol && (c - g).abs() <= tol && (d - h).abs() <= tol,
            (Primitive::Rect { x: a, y: b, w: c, h: d, .. }, Primitive::Rect { x: e, y: f, w: g, h: h, .. }) => (a - e).abs() <= tol && (b - f).abs() <= tol && (c - g).abs() <= tol && (d - h).abs() <= tol,
            _ => false,
        }
    }
}

/// Smallest area (source px²) worth testing.
pub const MIN_AREA: f64 = 16.0;
/// Circle: radial error limit is max(`ABS_TOL` source px, `CIRCLE_REL` x radius).
const ABS_TOL: f64 = 0.9;
const CIRCLE_REL: f64 = 0.015;
const ELLIPSE_REL: f64 = 0.018;
const RECT_REL: f64 = 0.018;
/// The relative limits never allow more than this many source px of error: on a big shape 1.5% would be a visible gap.
const MAX_TOL: f64 = 2.5;
/// An ellipse whose axes differ by less than this ratio is a circle.
const ELLIPSE_MIN_RATIO: f64 = 1.05;

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

/// Flatten a contour (cubic Beziers) into a polygon.
pub fn flatten(c: &Contour) -> Vec<P> {
    let mut out = vec![c.start];
    let mut prev = c.start;
    for cv in &c.curves {
        let steps = 8;
        for k in 1..=steps {
            let t = k as f64 / steps as f64;
            let mt = 1.0 - t;
            let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
            out.push((b0 * prev.0 + b1 * cv[0].0 + b2 * cv[1].0 + b3 * cv[2].0, b0 * prev.1 + b1 * cv[0].1 + b2 * cv[1].1 + b3 * cv[2].1));
        }
        prev = cv[2];
    }
    // the last point is the start again: drop the duplicate
    if out.len() > 1 {
        let (f, l) = (out[0], out[out.len() - 1]);
        if (f.0 - l.0).abs() < 1e-6 && (f.1 - l.1).abs() < 1e-6 {
            out.pop();
        }
    }
    out
}

/// (area, centroid, covariance xx, yy, xy) of the region inside a polygon.
fn region_moments(poly: &[P]) -> Option<(f64, P, f64, f64, f64)> {
    let n = poly.len();
    let (mut a2, mut cx, mut cy, mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let cross = p.0 * q.1 - q.0 * p.1;
        a2 += cross;
        cx += (p.0 + q.0) * cross;
        cy += (p.1 + q.1) * cross;
        sxx += (p.0 * p.0 + p.0 * q.0 + q.0 * q.0) * cross;
        syy += (p.1 * p.1 + p.1 * q.1 + q.1 * q.1) * cross;
        sxy += (p.0 * q.1 + 2.0 * p.0 * p.1 + 2.0 * q.0 * q.1 + q.0 * p.1) * cross;
    }
    let area = a2 / 2.0;
    if area.abs() < 1e-9 {
        return None;
    }
    let (mx, my) = (cx / (6.0 * area), cy / (6.0 * area));
    let (exx, eyy, exy) = (sxx / (12.0 * area), syy / (12.0 * area), sxy / (24.0 * area));
    Some((area.abs(), (mx, my), exx - mx * mx, eyy - my * my, exy - mx * my))
}

/// Algebraic (Kasa) circle fit: (cx, cy, r).
fn fit_circle(pts: &[P]) -> Option<(f64, f64, f64)> {
    let n = pts.len() as f64;
    let (mx, my) = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut suu, mut svv, mut suv, mut suuu, mut svvv, mut suvv, mut svuu) = (0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
    for p in pts {
        let (u, v) = (p.0 - mx, p.1 - my);
        suu += u * u;
        svv += v * v;
        suv += u * v;
        suuu += u * u * u;
        svvv += v * v * v;
        suvv += u * v * v;
        svuu += v * u * u;
    }
    let det = suu * svv - suv * suv;
    if det.abs() < 1e-9 {
        return None;
    }
    let (b1, b2) = (0.5 * (suuu + suvv), 0.5 * (svvv + svuu));
    let uc = (b1 * svv - b2 * suv) / det;
    let vc = (b2 * suu - b1 * suv) / det;
    let r2 = uc * uc + vc * vc + (suu + svv) / n;
    if r2 <= 0.0 {
        return None;
    }
    Some((uc + mx, vc + my, r2.sqrt()))
}

fn try_circle(poly: &[P], scale: f64) -> Option<Primitive> {
    let (cx, cy, r) = fit_circle(poly)?;
    if r < 2.0 * scale {
        return None;
    }
    let err = poly.iter().map(|p| (((p.0 - cx).powi(2) + (p.1 - cy).powi(2)).sqrt() - r).abs()).fold(0.0, f64::max);
    (err <= (ABS_TOL * scale).max((CIRCLE_REL * r).min(MAX_TOL * scale))).then_some(Primitive::Circle { cx, cy, r })
}

fn try_ellipse(poly: &[P], c: P, vxx: f64, vyy: f64, vxy: f64, scale: f64) -> Option<Primitive> {
    // principal axes of the region covariance; a filled ellipse has variance a^2/4 along a semi-axis a
    let tr = vxx + vyy;
    let disc = (((vxx - vyy) / 2.0).powi(2) + vxy * vxy).sqrt();
    let (l1, l2) = (tr / 2.0 + disc, tr / 2.0 - disc);
    if l2 <= 1e-9 {
        return None;
    }
    let (rx, ry) = (2.0 * l1.sqrt(), 2.0 * l2.sqrt());
    if rx / ry < ELLIPSE_MIN_RATIO {
        return None;
    }
    let theta = 0.5 * (2.0 * vxy).atan2(vxx - vyy);
    let (ct, st) = (theta.cos(), theta.sin());
    let mut err = 0.0f64;
    for p in poly {
        let (dx, dy) = (p.0 - c.0, p.1 - c.1);
        let (u, v) = (dx * ct + dy * st, -dx * st + dy * ct);
        let rn = ((u / rx).powi(2) + (v / ry).powi(2)).sqrt();
        let d = (dx * dx + dy * dy).sqrt();
        // distance along the ray from the centre between the point and the ellipse
        let e = if rn < 1e-9 { rx.min(ry) } else { d * (1.0 - 1.0 / rn).abs() };
        err = err.max(e);
    }
    (err <= (ABS_TOL * scale).max((ELLIPSE_REL * ry).min(MAX_TOL * scale))).then_some(Primitive::Ellipse { cx: c.0, cy: c.1, rx, ry, angle_deg: theta.to_degrees() })
}

fn convex_hull(pts: &[P]) -> Vec<P> {
    let mut p: Vec<P> = pts.to_vec();
    p.sort_by(|a, b| a.partial_cmp(b).unwrap());
    p.dedup();
    if p.len() < 3 {
        return p;
    }
    let cross = |o: P, a: P, b: P| (a.0 - o.0) * (b.1 - o.1) - (a.1 - o.1) * (b.0 - o.0);
    let mut h: Vec<P> = Vec::new();
    for &q in &p {
        while h.len() >= 2 && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    let lower = h.len() + 1;
    for &q in p.iter().rev().skip(1) {
        while h.len() >= lower && cross(h[h.len() - 2], h[h.len() - 1], q) <= 0.0 {
            h.pop();
        }
        h.push(q);
    }
    h.pop();
    h
}

/// Signed distance of a point (in the rectangle's frame, centre at the origin) to a rounded rectangle.
fn rounded_rect_dist(u: f64, v: f64, hw: f64, hh: f64, rad: f64) -> f64 {
    let (qx, qy) = (u.abs() - (hw - rad), v.abs() - (hh - rad));
    (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad
}

fn try_rect(poly: &[P], scale: f64) -> Option<Primitive> {
    let hull = convex_hull(poly);
    if hull.len() < 3 {
        return None;
    }
    // minimum-area bounding box over the hull edges
    let mut best: Option<(f64, f64, [f64; 4], f64)> = None; // area, angle, [umin, umax, vmin, vmax], _
    for i in 0..hull.len() {
        let e = sub(hull[(i + 1) % hull.len()], hull[i]);
        let l = (e.0 * e.0 + e.1 * e.1).sqrt();
        if l < 1e-9 {
            continue;
        }
        let (dx, dy) = (e.0 / l, e.1 / l);
        let (mut u0, mut u1, mut v0, mut v1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        for p in &hull {
            let (u, v) = (p.0 * dx + p.1 * dy, -p.0 * dy + p.1 * dx);
            u0 = u0.min(u);
            u1 = u1.max(u);
            v0 = v0.min(v);
            v1 = v1.max(v);
        }
        let area = (u1 - u0) * (v1 - v0);
        if best.map_or(true, |b| area < b.0) {
            best = Some((area, dy.atan2(dx), [u0, u1, v0, v1], 0.0));
        }
    }
    let (_, ang, [u0, u1, v0, v1], _) = best?;
    let (w, h) = (u1 - u0, v1 - v0);
    if w.min(h) < 2.0 * scale {
        return None;
    }
    let (ct, st) = (ang.cos(), ang.sin());
    // back to the original frame: centre of the box
    let (cu, cv) = ((u0 + u1) / 2.0, (v0 + v1) / 2.0);
    let centre = (cu * ct - cv * st, cu * st + cv * ct);
    let to_local = |p: P| {
        let (u, v) = (p.0 * ct + p.1 * st, -p.0 * st + p.1 * ct);
        (u - cu, v - cv)
    };
    let (hw, hh) = (w / 2.0, h / 2.0);
    let tol = (1.0 * scale).max((RECT_REL * w.min(h)).min(MAX_TOL * scale));
    // corner radius from the point nearest to a box corner: a round corner sits rad*(sqrt2-1) away from the corner
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let mut dc = f64::MAX;
    for p in poly {
        let (u, v) = to_local(*p);
        for c in &corners {
            dc = dc.min(((u - c.0).powi(2) + (v - c.1).powi(2)).sqrt());
        }
    }
    let mut rad = dc / (std::f64::consts::SQRT_2 - 1.0);
    if rad < 2.0 * tol {
        rad = 0.0;
    }
    if rad > hw.min(hh) {
        return None;
    }
    let inside = poly.iter().filter(|p| { let (u, v) = to_local(**p); rounded_rect_dist(u, v, hw, hh, rad).abs() <= tol }).count();
    if (inside as f64) < 0.98 * poly.len() as f64 {
        return None;
    }
    // every straight side must really be there: a diamond-like blob would pass the 98% test only with a huge radius
    let mut angle_deg = ang.to_degrees();
    // normalise to (-45, 45]: a rectangle turned by 90 degrees is the same rectangle
    let (mut ww, mut hh2) = (w, h);
    while angle_deg > 45.0 {
        angle_deg -= 90.0;
        std::mem::swap(&mut ww, &mut hh2);
    }
    while angle_deg <= -45.0 {
        angle_deg += 90.0;
        std::mem::swap(&mut ww, &mut hh2);
    }
    Some(Primitive::Rect { x: centre.0 - ww / 2.0, y: centre.1 - hh2 / 2.0, w: ww, h: hh2, rx: rad, angle_deg })
}

/// Recognise a single closed contour. `scale` = working px per source px.
pub fn detect(poly: &[P], scale: f64) -> Option<Primitive> {
    if poly.len() < 6 {
        return None;
    }
    let (area, c, vxx, vyy, vxy) = region_moments(poly)?;
    if area < MIN_AREA * scale * scale {
        return None;
    }
    if let Some(p) = try_circle(poly, scale) {
        return Some(p);
    }
    if let Some(p) = try_ellipse(poly, c, vxx, vyy, vxy, scale) {
        return Some(p);
    }
    try_rect(poly, scale)
}

fn bbox(p: &[P]) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for q in p {
        x0 = x0.min(q.0);
        y0 = y0.min(q.1);
        x1 = x1.max(q.0);
        y1 = y1.max(q.1);
    }
    (x0, y0, x1, y1)
}

fn point_in(pt: P, poly: &[P]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (a, b) = (poly[i], poly[j]);
        if (a.1 > pt.1) != (b.1 > pt.1) && pt.0 < (b.0 - a.0) * (pt.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Replace the recognisable contours of every colour layer by primitives. Only solid contours are touched: one that has a
/// hole (another contour of the same layer inside it) or that is itself a hole stays a path. Returns how many were found.
pub fn extract(layers: &mut [Layer], scale: f64, canvas: (f64, f64)) -> usize {
    // Layers are stacked and every lower layer also holds the islands of the colours above it. Going from the top down, a
    // shape that an upper layer already has with the same outline is completely hidden below it, so it is not written again.
    let mut placed: Vec<Primitive> = Vec::new();
    for layer in layers.iter_mut().rev().filter(|l| !l.canvas) {
        let polys: Vec<Vec<P>> = layer.contours.iter().map(flatten).collect();
        let boxes: Vec<_> = polys.iter().map(|p| bbox(p)).collect();
        let contains_box = |o: usize, i: usize| boxes[o].0 <= boxes[i].0 && boxes[o].1 <= boxes[i].1 && boxes[o].2 >= boxes[i].2 && boxes[o].3 >= boxes[i].3;
        let mut keep = vec![true; polys.len()];
        for i in 0..polys.len() {
            if polys[i].len() < 6 || layer.contours[i].prim.is_none() {
                continue;
            }
            // a contour that runs along the edge of the picture is a cut-off shape, not a whole one
            let edge = 1.5 * scale;
            if boxes[i].0 <= edge || boxes[i].1 <= edge || boxes[i].2 >= canvas.0 - edge || boxes[i].3 >= canvas.1 - edge {
                continue;
            }
            let mut depth = 0;
            let mut has_inside = false;
            for o in 0..polys.len() {
                if o == i {
                    continue;
                }
                if contains_box(o, i) && point_in(polys[i][0], &polys[o]) {
                    depth += 1;
                }
                if contains_box(i, o) && point_in(polys[o][0], &polys[i]) {
                    has_inside = true;
                    break;
                }
            }
            if has_inside || depth % 2 == 1 {
                continue;
            }
            if let Some(p) = layer.contours[i].prim.clone() {
                keep[i] = false;
                if !placed.iter().any(|u| u.same_as(&p, 0.5 * scale)) {
                    placed.push(p.clone());
                    layer.prims.push(p);
                }
            }
        }
        let mut k = 0;
        layer.contours.retain(|_| {
            let r = keep[k];
            k += 1;
            r
        });
    }
    placed.len()
}
