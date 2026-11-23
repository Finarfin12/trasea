//! Symmetry enforcer. A logo that should be symmetric but carries small raster flaws gets one side copied onto the other
//! BEFORE tracing, on the palette-label picture. Both sides then trace to the same geometry, and there is no seam on the
//! axis (the tracer sees one solid picture), which cutting and mirroring curves afterwards would not guarantee.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymmetryMode {
    #[default]
    Off,
    Auto,
    /// Mirror left-right (the axis is a vertical line x = cx).
    Vertical,
    /// Mirror top-bottom (the axis is a horizontal line y = cy).
    Horizontal,
}

impl SymmetryMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "off" | "mati" => Some(Self::Off),
            "auto" | "otomatis" => Some(Self::Auto),
            "vertical" | "vertikal" => Some(Self::Vertical),
            "horizontal" => Some(Self::Horizontal),
            _ => None,
        }
    }
}

/// How `Auto` decides: the share of matching pixels (same palette label as the mirror image) must reach this.
pub const AUTO_SCORE: f64 = 0.88;

#[derive(Debug, Clone)]
pub struct SymPlan {
    /// true = mirror left-right (axis is a vertical line).
    pub vertical: bool,
    /// Twice the axis position in pixel indices (so a half-pixel axis stays an integer).
    pub axis2: i64,
    /// Copy the low side (left / top) onto the high side; false = copy the high side.
    pub keep_low: bool,
    pub score: f64,
}

impl SymPlan {
    pub fn name(&self) -> &'static str {
        if self.vertical {
            "vertical"
        } else {
            "horizontal"
        }
    }
}

fn most_common(labels: &[u8]) -> u8 {
    let mut n = [0usize; 256];
    for &l in labels {
        n[l as usize] += 1;
    }
    (0..256).max_by_key(|&i| (n[i], 255 - i)).unwrap() as u8
}

/// Mirror of pixel (x, y) for the axis (None when it falls outside the picture).
fn mirror_of(x: usize, y: usize, w: usize, h: usize, vertical: bool, axis2: i64) -> Option<(usize, usize)> {
    if vertical {
        let mx = axis2 - x as i64;
        (mx >= 0 && (mx as usize) < w).then_some((mx as usize, y))
    } else {
        let my = axis2 - y as i64;
        (my >= 0 && (my as usize) < h).then_some((x, my as usize))
    }
}

/// Share of matching pixels across the axis, over the pixels where at least one of the two is foreground.
fn score(labels: &[u8], fg: &[bool], w: usize, h: usize, vertical: bool, axis2: i64) -> f64 {
    let (mut same, mut total) = (0usize, 0usize);
    for y in 0..h {
        for x in 0..w {
            // the pair is visited once: from the low side
            let low = if vertical { 2 * (x as i64) < axis2 } else { 2 * (y as i64) < axis2 };
            if !low {
                continue;
            }
            let Some((mx, my)) = mirror_of(x, y, w, h, vertical, axis2) else { continue };
            let (a, b) = (y * w + x, my * w + mx);
            if fg[a] || fg[b] {
                total += 1;
                if labels[a] == labels[b] {
                    same += 1;
                }
            }
        }
    }
    if total == 0 {
        0.0
    } else {
        same as f64 / total as f64
    }
}

/// Region-boundary pixels on one side of the axis (a noisier side has more of them).
fn roughness(labels: &[u8], w: usize, h: usize, vertical: bool, axis2: i64, low: bool) -> usize {
    let mut n = 0;
    for y in 0..h {
        for x in 0..w {
            let pos2 = if vertical { 2 * x as i64 } else { 2 * y as i64 };
            if (low && pos2 >= axis2) || (!low && pos2 <= axis2) || mirror_of(x, y, w, h, vertical, axis2).is_none() {
                continue;
            }
            let l = labels[y * w + x];
            if (x + 1 < w && labels[y * w + x + 1] != l) || (y + 1 < h && labels[(y + 1) * w + x] != l) {
                n += 1;
            }
        }
    }
    n
}

fn axis_for(fg: &[bool], w: usize, h: usize, vertical: bool) -> Option<i64> {
    let (mut sum, mut cnt) = (0f64, 0usize);
    for (i, &f) in fg.iter().enumerate() {
        if f {
            sum += if vertical { (i % w) as f64 } else { (i / w) as f64 };
            cnt += 1;
        }
    }
    let _ = h;
    (cnt > 0).then(|| (2.0 * sum / cnt as f64).round() as i64)
}

/// Decide whether and how to mirror. `Off` and an image without a foreground give `None`; `Auto` also gives `None` when
/// neither axis reaches `AUTO_SCORE`.
pub fn plan(labels: &[u8], w: usize, h: usize, mode: SymmetryMode) -> Option<SymPlan> {
    if mode == SymmetryMode::Off || w < 4 || h < 4 {
        return None;
    }
    let bg = most_common(labels);
    let fg: Vec<bool> = labels.iter().map(|&l| l != bg).collect();
    let try_axis = |vertical: bool| -> Option<SymPlan> {
        let a0 = axis_for(&fg, w, h, vertical)?;
        // the centre of mass moves with every small flaw: look for the axis with the best match around it
        let (mut axis2, mut s) = (a0, score(labels, &fg, w, h, vertical, a0));
        for d in (-4i64..=4).filter(|&d| d != 0) {
            let sc = score(labels, &fg, w, h, vertical, a0 + d);
            if sc > s + 1e-9 {
                axis2 = a0 + d;
                s = sc;
            }
        }
        let (lo, hi) = (roughness(labels, w, h, vertical, axis2, true), roughness(labels, w, h, vertical, axis2, false));
        Some(SymPlan { vertical, axis2, keep_low: lo <= hi, score: s })
    };
    match mode {
        SymmetryMode::Vertical => try_axis(true),
        SymmetryMode::Horizontal => try_axis(false),
        SymmetryMode::Auto => {
            let best = [try_axis(true), try_axis(false)].into_iter().flatten().max_by(|a, b| a.score.partial_cmp(&b.score).unwrap());
            best.filter(|p| p.score >= AUTO_SCORE)
        }
        SymmetryMode::Off => None,
    }
}

/// Copy the kept side onto the other side. Pixels on the axis itself, and pixels whose mirror is outside the picture, stay.
pub fn apply<T: Copy>(data: &mut [T], w: usize, h: usize, p: &SymPlan) {
    for y in 0..h {
        for x in 0..w {
            let pos2 = if p.vertical { 2 * x as i64 } else { 2 * y as i64 };
            let on_kept_side = if p.keep_low { pos2 < p.axis2 } else { pos2 > p.axis2 };
            if !on_kept_side {
                continue;
            }
            if let Some((mx, my)) = mirror_of(x, y, w, h, p.vertical, p.axis2) {
                data[my * w + mx] = data[y * w + x];
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Vector side. The label picture is exactly symmetric after `apply`, but the Bezier fit is not mirror-invariant (it starts
// somewhere and walks one way), so the two sides could still differ by the fit tolerance. Here the contours of the kept
// side are mirrored as they are, and a contour that crosses the axis is cut on the axis and closed with its own mirror
// image. A contour that does not fit this pattern is left as it is.

use crate::trace::{Contour, Layer};

type Pt = (f64, f64);
type Seg = [Pt; 4];

fn coord(p: Pt, vertical: bool) -> f64 {
    if vertical {
        p.0
    } else {
        p.1
    }
}

fn mirror_pt(p: Pt, vertical: bool, a: f64) -> Pt {
    if vertical {
        (2.0 * a - p.0, p.1)
    } else {
        (p.0, 2.0 * a - p.1)
    }
}

fn bez(s: &Seg, t: f64) -> Pt {
    let mt = 1.0 - t;
    let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
    (b0 * s[0].0 + b1 * s[1].0 + b2 * s[2].0 + b3 * s[3].0, b0 * s[0].1 + b1 * s[1].1 + b2 * s[2].1 + b3 * s[3].1)
}

fn lerp(a: Pt, b: Pt, t: f64) -> Pt {
    (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)
}

/// de Casteljau: (the part before t, the part after t).
fn split(s: &Seg, t: f64) -> (Seg, Seg) {
    let (p01, p12, p23) = (lerp(s[0], s[1], t), lerp(s[1], s[2], t), lerp(s[2], s[3], t));
    let (p012, p123) = (lerp(p01, p12, t), lerp(p12, p23, t));
    let m = lerp(p012, p123, t);
    ([s[0], p01, p012, m], [m, p123, p23, s[3]])
}

fn segments(c: &Contour) -> Vec<Seg> {
    let mut prev = c.start;
    c.curves
        .iter()
        .map(|cv| {
            let s = [prev, cv[0], cv[1], cv[2]];
            prev = cv[2];
            s
        })
        .collect()
}

fn from_segments(segs: &[Seg], prim: Option<crate::primitives::Primitive>) -> Contour {
    Contour { start: segs[0][0], curves: segs.iter().map(|s| [s[1], s[2], s[3]]).collect(), prim }
}

/// Parameters where a segment crosses the axis (sign changes of coord - a), found by sampling and bisection.
fn crossings(s: &Seg, vertical: bool, a: f64) -> Vec<f64> {
    let f = |t: f64| coord(bez(s, t), vertical) - a;
    let steps = 24;
    let mut out = Vec::new();
    let mut prev = f(0.0);
    for k in 1..=steps {
        let t1 = k as f64 / steps as f64;
        let cur = f(t1);
        if (prev < 0.0) != (cur < 0.0) && !(prev == 0.0 && cur == 0.0) {
            let (mut lo, mut hi) = ((k - 1) as f64 / steps as f64, t1);
            let lo_neg = prev < 0.0;
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if (f(mid) < 0.0) == lo_neg {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            out.push(0.5 * (lo + hi));
        }
        prev = cur;
    }
    out
}

/// The sub-path of the closed ring `segs` that runs forward from crossing `from` to crossing `to` (both `(segment, t)`).
fn arc(segs: &[Seg], from: (usize, f64), to: (usize, f64)) -> Vec<Seg> {
    let n = segs.len();
    let mut out = Vec::new();
    if from.0 == to.0 && from.1 < to.1 {
        let (_, right) = split(&segs[from.0], from.1);
        let (mid, _) = split(&right, (to.1 - from.1) / (1.0 - from.1));
        return vec![mid];
    }
    out.push(split(&segs[from.0], from.1).1);
    let mut i = (from.0 + 1) % n;
    while i != to.0 {
        out.push(segs[i]);
        i = (i + 1) % n;
    }
    out.push(split(&segs[to.0], to.1).0);
    out
}

fn mirror_seg_reversed(s: &Seg, vertical: bool, a: f64) -> Seg {
    [mirror_pt(s[3], vertical, a), mirror_pt(s[2], vertical, a), mirror_pt(s[1], vertical, a), mirror_pt(s[0], vertical, a)]
}

/// Make a contour that crosses the axis exactly twice symmetric: keep its half on the kept side and close it with the
/// mirror image of that half.
fn symmetrize(c: &Contour, vertical: bool, a: f64, keep_low: bool) -> Option<Contour> {
    let segs = segments(c);
    if segs.is_empty() {
        return None;
    }
    let mut cross: Vec<(usize, f64)> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        for t in crossings(s, vertical, a) {
            cross.push((i, t));
        }
    }
    if cross.len() != 2 {
        return None;
    }
    let (c1, c2) = (cross[0], cross[1]);
    let x_arc = arc(&segs, c1, c2);
    let y_arc = arc(&segs, c2, c1);
    // which of the two arcs lies on the kept side?
    let mid = |p: &Vec<Seg>| {
        let m = &p[p.len() / 2];
        coord(bez(m, 0.5), vertical)
    };
    let x_low = mid(&x_arc) < a;
    let mut kept = if x_low == keep_low { x_arc } else { y_arc };
    // put the two ends exactly on the axis
    let force = |p: Pt| if vertical { (a, p.1) } else { (p.0, a) };
    let n = kept.len();
    kept[0][0] = force(kept[0][0]);
    kept[n - 1][3] = force(kept[n - 1][3]);
    let mut all: Vec<Seg> = kept.clone();
    for s in kept.iter().rev() {
        all.push(mirror_seg_reversed(s, vertical, a));
    }
    Some(from_segments(&all, c.prim.clone()))
}

fn mirrored_contour(c: &Contour, vertical: bool, a: f64) -> Contour {
    Contour {
        start: mirror_pt(c.start, vertical, a),
        curves: c.curves.iter().map(|cv| [mirror_pt(cv[0], vertical, a), mirror_pt(cv[1], vertical, a), mirror_pt(cv[2], vertical, a)]).collect(),
        prim: c.prim.as_ref().map(|p| p.mirrored(vertical, a)),
    }
}

/// Make the traced colour layers exactly symmetric across the plan's axis. `canvas` = working size of the picture.
pub fn mirror_layers(layers: &mut [Layer], plan: &SymPlan, canvas: (f64, f64)) {
    let vertical = plan.vertical;
    // the axis in continuous working coordinates (pixel index axis2/2 is the pixel centre)
    let a = plan.axis2 as f64 / 2.0 + 0.5;
    let extent = if vertical { canvas.0 } else { canvas.1 };
    let reach = a.min(extent - a);
    let eps = 0.5;
    for layer in layers.iter_mut().filter(|l| !l.canvas) {
        let mut out: Vec<Contour> = Vec::new();
        let mut copies: Vec<Contour> = Vec::new();
        for c in layer.contours.drain(..) {
            let pts = std::iter::once(c.start).chain(c.curves.iter().flat_map(|cv| cv.iter().copied()));
            let (mut lo, mut hi) = (f64::MAX, f64::MIN);
            for p in pts {
                lo = lo.min(coord(p, vertical));
                hi = hi.max(coord(p, vertical));
            }
            if lo < a - reach - eps || hi > a + reach + eps {
                out.push(c); // reaches past the part that was mirrored
            } else if hi <= a + eps {
                if plan.keep_low {
                    copies.push(mirrored_contour(&c, vertical, a));
                    out.push(c);
                }
            } else if lo >= a - eps {
                if !plan.keep_low {
                    copies.push(mirrored_contour(&c, vertical, a));
                    out.push(c);
                }
            } else {
                let sym = symmetrize(&c, vertical, a, plan.keep_low);
                out.push(sym.unwrap_or(c));
            }
        }
        out.extend(copies);
        layer.contours = out;
    }
}
