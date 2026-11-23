//! Non-destructive node editing (Trasea 1.3.0): inspect the anchors and Bezier handles of one path and apply
//! `MoveNode`, `MoveHandle`, `DeleteNode`, `ToggleSmooth` and `SimplifyPath` edits on top of the deterministic
//! `data-trasea-id` identifiers of `path_edit` (`f:` colour field, `fp:` primitive, `l:`/`ls:`/`lf:` line layers,
//! `s:` open stroke).
//!
//! * Coordinates of every `NodeEdit` and of `inspect_path_nodes` are in **source pixels** (the SVG document space);
//!   the `Doc` stores working pixels, so they are converted with `doc.scale` (working = source * scale).
//! * `subpath_idx` is the position of the contour inside its compound (outer boundary first, then its holes, in the
//!   order returned by `group_contours_into_compounds`). A stroke or a primitive always has one subpath (0).
//! * A primitive (`fp:`) that receives an edit is converted into a real cubic Bezier contour first
//!   (`primitive_to_contour`) and moved from `layer.prims` to `layer.contours`.
//! * Edits are applied after `apply_path_edits`, so every export (SVG, PDF, EPS, DXF, CutContour, separations)
//!   sees the edited geometry. Unknown ids / indices and rejected operations are skipped without panicking.

use crate::export::{Doc, StrokeData};
use crate::fit::{fit_polyline, fit_ring, Bezier, P};
use crate::path_edit::group_contours_into_compounds;
use crate::primitives::Primitive;
use crate::trace::Contour;
use crate::hex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Cubic Bezier constant for a quarter circle.
const KAPPA: f64 = 0.552_284_749_8;
/// A node is smooth when its two handles are collinear within this angle (degrees).
const SMOOTH_DEG: f64 = 8.0;
/// Handles shorter than this (working px) do not count as a tangent.
const MIN_HANDLE: f64 = 1e-3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HandleSide {
    In,
    Out,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NodeEdit {
    MoveNode {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "subpathIdx", alias = "subpath_idx")]
        subpath_idx: usize,
        #[serde(rename = "nodeIdx", alias = "node_idx")]
        node_idx: usize,
        x: f64,
        y: f64,
    },
    MoveHandle {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "subpathIdx", alias = "subpath_idx")]
        subpath_idx: usize,
        #[serde(rename = "nodeIdx", alias = "node_idx")]
        node_idx: usize,
        which: HandleSide,
        x: f64,
        y: f64,
        #[serde(rename = "mirrorSmooth", alias = "mirror_smooth", default)]
        mirror_smooth: bool,
    },
    DeleteNode {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "subpathIdx", alias = "subpath_idx")]
        subpath_idx: usize,
        #[serde(rename = "nodeIdx", alias = "node_idx")]
        node_idx: usize,
    },
    ToggleSmooth {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "subpathIdx", alias = "subpath_idx")]
        subpath_idx: usize,
        #[serde(rename = "nodeIdx", alias = "node_idx")]
        node_idx: usize,
    },
    SimplifyPath {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "tolerancePx", alias = "tolerance_px")]
        tolerance_px: f64,
    },
}

impl NodeEdit {
    pub fn target_id(&self) -> &str {
        match self {
            NodeEdit::MoveNode { target_id, .. }
            | NodeEdit::MoveHandle { target_id, .. }
            | NodeEdit::DeleteNode { target_id, .. }
            | NodeEdit::ToggleSmooth { target_id, .. }
            | NodeEdit::SimplifyPath { target_id, .. } => target_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NodePoint {
    pub index: usize,
    pub x: f64,
    pub y: f64,
    pub handle_in: Option<(f64, f64)>,
    pub handle_out: Option<(f64, f64)>,
    pub is_smooth: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubpathNodes {
    pub subpath_index: usize,
    pub closed: bool,
    pub nodes: Vec<NodePoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathNodeGeometry {
    pub target_id: String,
    pub color_hex: String,
    pub total_nodes: usize,
    pub subpaths: Vec<SubpathNodes>,
}

/// What `apply_node_edits_report` did: how many edits changed something and why the others were skipped.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeEditReport {
    pub applied: usize,
    pub skipped: Vec<String>,
}

// ---------------------------------------------------------------------------------------------------------------
// small vector helpers

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
fn mul(a: P, k: f64) -> P {
    (a.0 * k, a.1 * k)
}
fn len(a: P) -> f64 {
    (a.0 * a.0 + a.1 * a.1).sqrt()
}
fn dist(a: P, b: P) -> f64 {
    len(sub(a, b))
}
fn finite(p: P) -> bool {
    p.0.is_finite() && p.1.is_finite()
}

fn dist_to_line(p: P, a: P, b: P) -> f64 {
    let ab = sub(b, a);
    let l = len(ab);
    if l < 1e-9 {
        return dist(p, a);
    }
    ((p.0 - a.0) * ab.1 - (p.1 - a.1) * ab.0).abs() / l
}

fn rotate_about(p: P, c: P, deg: f64) -> P {
    if deg.abs() < 1e-9 {
        return p;
    }
    let (s, co) = deg.to_radians().sin_cos();
    let d = sub(p, c);
    (c.0 + d.0 * co - d.1 * s, c.1 + d.0 * s + d.1 * co)
}

fn line_bezier(a: P, b: P) -> Bezier {
    let d = sub(b, a);
    [a, add(a, mul(d, 1.0 / 3.0)), add(a, mul(d, 2.0 / 3.0)), b]
}

// ---------------------------------------------------------------------------------------------------------------
// primitives -> contours

/// A circle, ellipse or rectangle as a closed contour of cubic Beziers (working pixels).
pub fn primitive_to_contour(prim: &Primitive) -> Contour {
    let beziers: Vec<Bezier> = match *prim {
        Primitive::Circle { cx, cy, r } => ellipse_beziers(cx, cy, r, r, 0.0),
        Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => ellipse_beziers(cx, cy, rx, ry, angle_deg),
        Primitive::Rect { x, y, w, h, rx, angle_deg } => rect_beziers(x, y, w, h, rx, angle_deg),
    };
    beziers_to_contour(&beziers)
}

fn ellipse_beziers(cx: f64, cy: f64, rx: f64, ry: f64, angle_deg: f64) -> Vec<Bezier> {
    let c = (cx, cy);
    let rot = |x: f64, y: f64| rotate_about((x, y), c, angle_deg);
    let (kx, ky) = (rx * KAPPA, ry * KAPPA);
    vec![
        [rot(cx + rx, cy), rot(cx + rx, cy + ky), rot(cx + kx, cy + ry), rot(cx, cy + ry)],
        [rot(cx, cy + ry), rot(cx - kx, cy + ry), rot(cx - rx, cy + ky), rot(cx - rx, cy)],
        [rot(cx - rx, cy), rot(cx - rx, cy - ky), rot(cx - kx, cy - ry), rot(cx, cy - ry)],
        [rot(cx, cy - ry), rot(cx + kx, cy - ry), rot(cx + rx, cy - ky), rot(cx + rx, cy)],
    ]
}

fn rect_beziers(x: f64, y: f64, w: f64, h: f64, rx: f64, angle_deg: f64) -> Vec<Bezier> {
    let c = (x + w / 2.0, y + h / 2.0);
    let rot = |p: P| rotate_about(p, c, angle_deg);
    let r = rx.max(0.0).min(w.min(h) / 2.0);
    if r < 0.01 {
        let pts = [rot((x, y)), rot((x + w, y)), rot((x + w, y + h)), rot((x, y + h))];
        return (0..4).map(|i| line_bezier(pts[i], pts[(i + 1) % 4])).collect();
    }
    let k = r * KAPPA;
    // eight segments: four straight edges and four rounded corners, clockwise from the top edge
    let seg_line = |a: P, b: P| line_bezier(rot(a), rot(b));
    let seg_curve = |a: P, c1: P, c2: P, b: P| [rot(a), rot(c1), rot(c2), rot(b)];
    vec![
        seg_line((x + r, y), (x + w - r, y)),
        seg_curve((x + w - r, y), (x + w - r + k, y), (x + w, y + r - k), (x + w, y + r)),
        seg_line((x + w, y + r), (x + w, y + h - r)),
        seg_curve((x + w, y + h - r), (x + w, y + h - r + k), (x + w - r + k, y + h), (x + w - r, y + h)),
        seg_line((x + w - r, y + h), (x + r, y + h)),
        seg_curve((x + r, y + h), (x + r - k, y + h), (x, y + h - r + k), (x, y + h - r)),
        seg_line((x, y + h - r), (x, y + r)),
        seg_curve((x, y + r), (x, y + r - k), (x + r - k, y), (x + r, y)),
    ]
}

// ---------------------------------------------------------------------------------------------------------------
// canonical node path

/// Anchors with optional handles. Closed path: `n` anchors, all handles present. Open path: `m + 1` anchors,
/// `hin[0]` and `hout[m]` are absent.
#[derive(Debug, Clone)]
struct NPath {
    closed: bool,
    pos: Vec<P>,
    hin: Vec<Option<P>>,
    hout: Vec<Option<P>>,
}

impl NPath {
    fn from_beziers(b: &[Bezier], closed: bool) -> Option<NPath> {
        let m = b.len();
        if m == 0 || (closed && m < 2) {
            return None;
        }
        if closed {
            let mut pos = Vec::with_capacity(m);
            let mut hin = vec![None; m];
            let mut hout = vec![None; m];
            for i in 0..m {
                pos.push(b[i][0]);
                hout[i] = Some(b[i][1]);
                hin[(i + 1) % m] = Some(b[i][2]);
            }
            Some(NPath { closed, pos, hin, hout })
        } else {
            let mut pos = Vec::with_capacity(m + 1);
            let mut hin = vec![None; m + 1];
            let mut hout = vec![None; m + 1];
            for i in 0..m {
                pos.push(b[i][0]);
                hout[i] = Some(b[i][1]);
                hin[i + 1] = Some(b[i][2]);
            }
            pos.push(b[m - 1][3]);
            Some(NPath { closed, pos, hin, hout })
        }
    }

    fn to_beziers(&self) -> Vec<Bezier> {
        let n = self.pos.len();
        let h_out = |i: usize| self.hout[i].unwrap_or(self.pos[i]);
        let h_in = |i: usize| self.hin[i].unwrap_or(self.pos[i]);
        if self.closed {
            (0..n).map(|i| {
                let j = (i + 1) % n;
                [self.pos[i], h_out(i), h_in(j), self.pos[j]]
            }).collect()
        } else {
            (0..n.saturating_sub(1)).map(|i| [self.pos[i], h_out(i), h_in(i + 1), self.pos[i + 1]]).collect()
        }
    }

    fn from_contour(c: &Contour) -> Option<NPath> {
        let mut prev = c.start;
        let mut bz = Vec::with_capacity(c.curves.len());
        for cv in &c.curves {
            bz.push([prev, cv[0], cv[1], cv[2]]);
            prev = cv[2];
        }
        NPath::from_beziers(&bz, true)
    }

    fn to_contour(&self) -> Contour {
        beziers_to_contour(&self.to_beziers())
    }

    fn from_stroke(s: &StrokeData) -> Option<NPath> {
        NPath::from_beziers(&s.curves, false)
    }

    fn node_count(&self) -> usize {
        self.pos.len()
    }

    fn min_nodes(&self) -> usize {
        if self.closed { 3 } else { 2 }
    }

    fn is_smooth(&self, i: usize) -> bool {
        let (Some(hi), Some(ho)) = (self.hin[i], self.hout[i]) else { return false };
        let a = sub(self.pos[i], hi);
        let b = sub(ho, self.pos[i]);
        let (la, lb) = (len(a), len(b));
        if la <= MIN_HANDLE || lb <= MIN_HANDLE {
            return false;
        }
        let cos = ((a.0 * b.0 + a.1 * b.1) / (la * lb)).clamp(-1.0, 1.0);
        cos.acos().to_degrees() < SMOOTH_DEG
    }

    /// Points along the path about one working pixel apart (anchors included), for re-fitting.
    fn sample(&self) -> Vec<P> {
        let bz = self.to_beziers();
        let mut pts = Vec::new();
        for b in &bz {
            let poly = dist(b[0], b[1]) + dist(b[1], b[2]) + dist(b[2], b[3]);
            let steps = (poly.ceil() as usize).clamp(4, 96);
            for s in 0..steps {
                pts.push(bezier_at(b, s as f64 / steps as f64));
            }
        }
        if !self.closed {
            if let Some(b) = bz.last() {
                pts.push(b[3]);
            }
        }
        pts
    }
}

fn bezier_at(b: &Bezier, t: f64) -> P {
    let mt = 1.0 - t;
    (
        mt * mt * mt * b[0].0 + 3.0 * mt * mt * t * b[1].0 + 3.0 * mt * t * t * b[2].0 + t * t * t * b[3].0,
        mt * mt * mt * b[0].1 + 3.0 * mt * mt * t * b[1].1 + 3.0 * mt * t * t * b[2].1 + t * t * t * b[3].1,
    )
}

fn beziers_to_contour(b: &[Bezier]) -> Contour {
    Contour {
        start: b[0][0],
        curves: b.iter().map(|c| [c[1], c[2], c[3]]).collect(),
        prim: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// edit operations on one NPath (working pixels)

fn op_move_node(p: &mut NPath, i: usize, to: P) -> Result<(), String> {
    if i >= p.node_count() {
        return Err(format!("node {i} tidak ada"));
    }
    if !finite(to) {
        return Err("koordinat tidak valid".into());
    }
    let d = sub(to, p.pos[i]);
    p.pos[i] = to;
    if let Some(h) = p.hin[i] {
        p.hin[i] = Some(add(h, d));
    }
    if let Some(h) = p.hout[i] {
        p.hout[i] = Some(add(h, d));
    }
    Ok(())
}

fn op_move_handle(p: &mut NPath, i: usize, which: HandleSide, to: P, mirror: bool) -> Result<(), String> {
    if i >= p.node_count() {
        return Err(format!("node {i} tidak ada"));
    }
    if !finite(to) {
        return Err("koordinat tidak valid".into());
    }
    let (this, other) = match which {
        HandleSide::In => (&mut p.hin, &mut p.hout),
        HandleSide::Out => (&mut p.hout, &mut p.hin),
    };
    if this[i].is_none() {
        return Err(format!("node {i} tidak punya handle ini"));
    }
    this[i] = Some(to);
    if mirror {
        if let Some(o) = other[i] {
            let dir = sub(to, p.pos[i]);
            let dl = len(dir);
            if dl > MIN_HANDLE {
                let ol = len(sub(o, p.pos[i]));
                let keep = if ol > MIN_HANDLE { ol } else { dl };
                other[i] = Some(sub(p.pos[i], mul(dir, keep / dl)));
            }
        }
    }
    Ok(())
}

fn is_straight(a: P, c1: P, c2: P, b: P) -> bool {
    let tol = 0.05_f64.max(dist(a, b) * 1e-4);
    dist_to_line(c1, a, b) <= tol && dist_to_line(c2, a, b) <= tol
}

fn op_delete_node(p: &mut NPath, i: usize) -> Result<(), String> {
    let n = p.node_count();
    if i >= n {
        return Err(format!("node {i} tidak ada"));
    }
    if n <= p.min_nodes() {
        return Err("Batas minimum node tercapai".into());
    }
    if !p.closed && (i == 0 || i == n - 1) {
        // removing an end of an open path just drops its first / last segment
        p.pos.remove(i);
        p.hin.remove(i);
        p.hout.remove(i);
        if i == 0 {
            p.hin[0] = None;
        } else {
            let last = p.node_count() - 1;
            p.hout[last] = None;
        }
        return Ok(());
    }
    let prev = (i + n - 1) % n;
    let next = (i + 1) % n;
    let p0 = p.pos[prev];
    let p1 = p.pos[i];
    let p2 = p.pos[next];
    let c1 = p.hout[prev].unwrap_or(p0);
    let c2 = p.hin[i].unwrap_or(p1);
    let c3 = p.hout[i].unwrap_or(p1);
    let c4 = p.hin[next].unwrap_or(p2);

    let (n1, n4) = if is_straight(p0, c1, c2, p1) && is_straight(p1, c3, c4, p2) {
        // two straight edges become one straight edge between the neighbours
        let d = sub(p2, p0);
        (add(p0, mul(d, 1.0 / 3.0)), sub(p2, mul(d, 1.0 / 3.0)))
    } else {
        // merge two cubics into one: the tangents are stretched by the share of the parameter range the first /
        // second piece covered, but never beyond half of the total control polygon (no overshoot)
        let l1 = dist(p0, c1) + dist(c1, c2) + dist(c2, p1);
        let l2 = dist(p1, c3) + dist(c3, c4) + dist(c4, p2);
        let total = (l1 + l2).max(1e-9);
        let a = (l1 / total).clamp(0.05, 0.95);
        let cap = total * 0.5;
        let stretch = |anchor: P, h: P, k: f64| -> P {
            let v = sub(h, anchor);
            let l = len(v);
            if l < 1e-9 {
                return anchor;
            }
            let nl = (l * k).min(cap);
            add(anchor, mul(v, nl / l))
        };
        (stretch(p0, c1, 1.0 / a), stretch(p2, c4, 1.0 / (1.0 - a)))
    };
    p.hout[prev] = Some(n1);
    p.hin[next] = Some(n4);
    p.pos.remove(i);
    p.hin.remove(i);
    p.hout.remove(i);
    Ok(())
}

fn op_toggle_smooth(p: &mut NPath, i: usize) -> Result<(), String> {
    let n = p.node_count();
    if i >= n {
        return Err(format!("node {i} tidak ada"));
    }
    if !p.closed && (i == 0 || i == n - 1) {
        return Err("titik ujung path terbuka tidak punya sudut".into());
    }
    let pos = p.pos[i];
    if p.is_smooth(i) {
        // smooth -> corner: fold both handles onto the anchor
        p.hin[i] = Some(pos);
        p.hout[i] = Some(pos);
        return Ok(());
    }
    let prev = p.pos[(i + n - 1) % n];
    let next = p.pos[(i + 1) % n];
    let dir = sub(next, prev);
    let dl = len(dir);
    if dl < 1e-9 {
        return Err("tetangga node berimpit".into());
    }
    let u = mul(dir, 1.0 / dl);
    p.hin[i] = Some(sub(pos, mul(u, dist(pos, prev) / 3.0)));
    p.hout[i] = Some(add(pos, mul(u, dist(pos, next) / 3.0)));
    Ok(())
}

fn op_simplify(p: &mut NPath, tol_working: f64) -> Result<(), String> {
    let mut tol = tol_working.max(0.05);
    let pts = p.sample();
    // A circle can be fitted with two segments, below the minimum node count of a closed path: tighten the
    // tolerance (up to 4 times, halving it) until the result is a valid path.
    for _ in 0..5 {
        let fitted: Vec<Bezier> = if p.closed {
            fit_ring(&pts, 0.5, tol, 65.0)
        } else {
            fit_polyline(&pts, tol)
        };
        if let Some(np) = NPath::from_beziers(&fitted, p.closed) {
            if np.node_count() >= p.min_nodes() && np.pos.iter().all(|q| finite(*q)) {
                if np.node_count() >= p.node_count() {
                    return Err("path sudah paling sederhana".into());
                }
                *p = np;
                return Ok(());
            }
        }
        tol *= 0.5;
        if tol < 0.02 {
            break;
        }
    }
    Err("path tidak bisa disederhanakan lebih lanjut".into())
}

/// Apply one edit to the subpaths of one target. `k` converts source pixels to working pixels.
fn apply_one(paths: &mut [NPath], edit: &NodeEdit, k: f64) -> Result<(), String> {
    let sub_of = |idx: usize| -> Result<usize, String> {
        if idx < paths.len() { Ok(idx) } else { Err(format!("subpath {idx} tidak ada")) }
    };
    match edit {
        NodeEdit::MoveNode { subpath_idx, node_idx, x, y, .. } => {
            let s = sub_of(*subpath_idx)?;
            op_move_node(&mut paths[s], *node_idx, (x * k, y * k))
        }
        NodeEdit::MoveHandle { subpath_idx, node_idx, which, x, y, mirror_smooth, .. } => {
            let s = sub_of(*subpath_idx)?;
            op_move_handle(&mut paths[s], *node_idx, *which, (x * k, y * k), *mirror_smooth)
        }
        NodeEdit::DeleteNode { subpath_idx, node_idx, .. } => {
            let s = sub_of(*subpath_idx)?;
            op_delete_node(&mut paths[s], *node_idx)
        }
        NodeEdit::ToggleSmooth { subpath_idx, node_idx, .. } => {
            let s = sub_of(*subpath_idx)?;
            op_toggle_smooth(&mut paths[s], *node_idx)
        }
        NodeEdit::SimplifyPath { tolerance_px, .. } => {
            if !tolerance_px.is_finite() || *tolerance_px <= 0.0 {
                return Err("toleransi tidak valid".into());
            }
            let mut any = false;
            let mut last_err = String::new();
            for p in paths.iter_mut() {
                match op_simplify(p, tolerance_px * k) {
                    Ok(()) => any = true,
                    Err(e) => last_err = e,
                }
            }
            if any { Ok(()) } else { Err(last_err) }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------
// target resolution

/// Where an id lives inside a `Doc`.
enum Loc {
    /// Colour field `fi`, compound `ci`.
    Field(usize, usize),
    FieldPrim(usize, usize),
    /// `which` = 0 line_shapes, 1 line_fallback.
    Line(usize, usize, usize),
    Stroke(usize, usize),
}

fn parse_id(id: &str, doc: &Doc) -> Option<Loc> {
    let mut it = id.trim().split(':');
    let kind = it.next()?;
    let a: usize = it.next()?.parse().ok()?;
    let b: usize = it.next()?.parse().ok()?;
    if it.next().is_some() {
        return None;
    }
    match kind {
        "f" => Some(Loc::Field(a, b)),
        "fp" => Some(Loc::FieldPrim(a, b)),
        "ls" => Some(Loc::Line(0, a, b)),
        "lf" => Some(Loc::Line(1, a, b)),
        "l" => {
            if a < doc.line_shapes.len() {
                Some(Loc::Line(0, a, b))
            } else {
                Some(Loc::Line(1, a, b))
            }
        }
        "s" => Some(Loc::Stroke(a, b)),
        _ => None,
    }
}

fn compound_members(contours: &[Contour], ci: usize) -> Option<Vec<usize>> {
    group_contours_into_compounds(contours).into_iter().nth(ci)
}

fn scale_of(doc: &Doc) -> f64 {
    if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 }
}

/// Nodes and handles of one path in source pixels, for the node tool overlay. `None` when the id is unknown.
pub fn inspect_path_nodes(doc: &Doc, target_id: &str) -> Option<PathNodeGeometry> {
    let k = scale_of(doc);
    let (paths, color): (Vec<NPath>, [u8; 3]) = match parse_id(target_id, doc)? {
        Loc::Field(fi, ci) => {
            let layer = doc.fields.get(fi)?;
            let members = compound_members(&layer.contours, ci)?;
            (members.iter().filter_map(|&m| NPath::from_contour(&layer.contours[m])).collect(), layer.color)
        }
        Loc::FieldPrim(fi, pi) => {
            let layer = doc.fields.get(fi)?;
            let prim = layer.prims.get(pi)?;
            (NPath::from_contour(&primitive_to_contour(prim)).into_iter().collect(), layer.color)
        }
        Loc::Line(which, li, ci) => {
            let layers = if which == 0 { &doc.line_shapes } else { &doc.line_fallback };
            let layer = layers.get(li)?;
            let members = compound_members(&layer.contours, ci)?;
            (members.iter().filter_map(|&m| NPath::from_contour(&layer.contours[m])).collect(), layer.color)
        }
        Loc::Stroke(sli, si) => {
            let sl = doc.strokes.get(sli)?;
            let st = sl.strokes.get(si)?;
            (NPath::from_stroke(st).into_iter().collect(), sl.color)
        }
    };
    if paths.is_empty() {
        return None;
    }
    let to_src = |q: P| (q.0 / k, q.1 / k);
    let subpaths: Vec<SubpathNodes> = paths
        .iter()
        .enumerate()
        .map(|(si, p)| SubpathNodes {
            subpath_index: si,
            closed: p.closed,
            nodes: (0..p.node_count())
                .map(|i| {
                    let q = to_src(p.pos[i]);
                    NodePoint {
                        index: i,
                        x: q.0,
                        y: q.1,
                        handle_in: p.hin[i].map(to_src),
                        handle_out: p.hout[i].map(to_src),
                        is_smooth: p.is_smooth(i),
                    }
                })
                .collect(),
        })
        .collect();
    let total_nodes = subpaths.iter().map(|s| s.nodes.len()).sum();
    Some(PathNodeGeometry { target_id: target_id.trim().to_string(), color_hex: hex(color), total_nodes, subpaths })
}

/// Apply node edits non-destructively and return a new `Doc` (see the module docs).
pub fn apply_node_edits(doc: &Doc, edits: &[NodeEdit]) -> Doc {
    apply_node_edits_report(doc, edits).0
}

pub fn apply_node_edits_report(doc: &Doc, edits: &[NodeEdit]) -> (Doc, NodeEditReport) {
    let mut report = NodeEditReport::default();
    if edits.is_empty() {
        return (doc.clone(), report);
    }
    // group the edits by target id, keeping their order (ids are resolved against the input doc)
    let mut order: Vec<String> = Vec::new();
    let mut by_target: HashMap<String, Vec<&NodeEdit>> = HashMap::new();
    for e in edits {
        let id = e.target_id().trim().to_string();
        if id.is_empty() {
            report.skipped.push("target kosong".into());
            continue;
        }
        by_target.entry(id.clone()).or_insert_with(|| {
            order.push(id.clone());
            Vec::new()
        }).push(e);
    }

    let k = scale_of(doc);
    let mut out = doc.clone();
    // primitives that were turned into contours: (field idx, prim idx, contour)
    let mut converted: Vec<(usize, usize, Contour)> = Vec::new();

    for id in &order {
        let list = &by_target[id];
        let Some(loc) = parse_id(id, doc) else {
            report.skipped.push(format!("{id}: id tidak dikenal"));
            continue;
        };
        // collect the subpaths of the target
        let mut paths: Vec<NPath> = Vec::new();
        let mut members: Vec<usize> = Vec::new();
        match &loc {
            Loc::Field(fi, ci) => {
                let Some(layer) = doc.fields.get(*fi) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                let Some(m) = compound_members(&layer.contours, *ci) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                for &mi in &m {
                    if let Some(p) = NPath::from_contour(&layer.contours[mi]) {
                        paths.push(p);
                        members.push(mi);
                    }
                }
            }
            Loc::FieldPrim(fi, pi) => {
                let Some(prim) = doc.fields.get(*fi).and_then(|l| l.prims.get(*pi)) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                if let Some(p) = NPath::from_contour(&primitive_to_contour(prim)) {
                    paths.push(p);
                }
            }
            Loc::Line(which, li, ci) => {
                let layers = if *which == 0 { &doc.line_shapes } else { &doc.line_fallback };
                let Some(layer) = layers.get(*li) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                let Some(m) = compound_members(&layer.contours, *ci) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                for &mi in &m {
                    if let Some(p) = NPath::from_contour(&layer.contours[mi]) {
                        paths.push(p);
                        members.push(mi);
                    }
                }
            }
            Loc::Stroke(sli, si) => {
                let Some(st) = doc.strokes.get(*sli).and_then(|s| s.strokes.get(*si)) else { report.skipped.push(format!("{id}: path tidak ada")); continue };
                if let Some(p) = NPath::from_stroke(st) {
                    paths.push(p);
                }
            }
        }
        if paths.is_empty() {
            report.skipped.push(format!("{id}: path kosong"));
            continue;
        }
        let mut changed = vec![false; paths.len()];
        for e in list {
            let before: Vec<usize> = paths.iter().map(|p| p.node_count()).collect();
            let snapshot: Vec<NPath> = paths.clone();
            match apply_one(&mut paths, e, k) {
                Ok(()) => {
                    report.applied += 1;
                    for (si, p) in paths.iter().enumerate() {
                        let moved = p.node_count() != before[si]
                            || p.pos.iter().zip(&snapshot[si].pos).any(|(a, b)| a != b)
                            || p.hin != snapshot[si].hin
                            || p.hout != snapshot[si].hout;
                        if moved {
                            changed[si] = true;
                        }
                    }
                    // an edit that moved nothing on any subpath still counts as applied but needs no write-back
                }
                Err(msg) => report.skipped.push(format!("{id}: {msg}")),
            }
        }
        // write the edited subpaths back
        match &loc {
            Loc::Field(fi, _) => {
                for (si, mi) in members.iter().enumerate() {
                    if changed[si] {
                        out.fields[*fi].contours[*mi] = paths[si].to_contour();
                    }
                }
            }
            Loc::Line(which, li, _) => {
                let layers = if *which == 0 { &mut out.line_shapes } else { &mut out.line_fallback };
                for (si, mi) in members.iter().enumerate() {
                    if changed[si] {
                        layers[*li].contours[*mi] = paths[si].to_contour();
                    }
                }
            }
            Loc::FieldPrim(fi, pi) => {
                if changed[0] {
                    converted.push((*fi, *pi, paths[0].to_contour()));
                }
            }
            Loc::Stroke(sli, si) => {
                if changed[0] {
                    let bz = paths[0].to_beziers();
                    if !bz.is_empty() {
                        out.strokes[*sli].strokes[*si].curves = bz;
                    }
                }
            }
        }
    }

    // primitives become editable contours: drop them from `prims` (highest index first) and append the contours
    converted.sort_by(|a, b| (a.0, std::cmp::Reverse(a.1)).cmp(&(b.0, std::cmp::Reverse(b.1))));
    for (fi, pi, contour) in converted {
        let layer = &mut out.fields[fi];
        if pi < layer.prims.len() {
            layer.prims.remove(pi);
            layer.contours.push(contour);
        }
    }
    (out, report)
}
