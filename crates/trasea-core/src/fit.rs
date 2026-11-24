//! Tidy contours: smooth the pixel staircase, find real corners, and fit as few cubic Beziers as the tolerance allows
//! (Schneider's algorithm, "An Algorithm for Automatically Fitting Digitized Curves", Graphics Gems 1990).

pub type P = (f64, f64);
pub type Bezier = [P; 4];

fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}
fn add(a: P, b: P) -> P {
    (a.0 + b.0, a.1 + b.1)
}
fn mul(a: P, k: f64) -> P {
    (a.0 * k, a.1 * k)
}
fn dot(a: P, b: P) -> f64 {
    a.0 * b.0 + a.1 * b.1
}
fn len(a: P) -> f64 {
    dot(a, a).sqrt()
}
fn norm(a: P) -> P {
    let l = len(a);
    if l < 1e-12 {
        (0.0, 0.0)
    } else {
        (a.0 / l, a.1 / l)
    }
}

/// Circular Gaussian smoothing of a closed ring of points (index based, the staircase has ~1 px spacing).
pub fn smooth_ring(pts: &[P], sigma: f64) -> Vec<P> {
    let n = pts.len();
    if n < 5 || sigma < 0.3 {
        return pts.to_vec();
    }
    let r = (sigma * 3.0).ceil() as i64;
    let r = r.min((n as i64 - 1) / 2).max(1);
    let kernel: Vec<f64> = (-r..=r).map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp()).collect();
    let ksum: f64 = kernel.iter().sum();
    (0..n as i64)
        .map(|i| {
            let mut s = (0.0, 0.0);
            for (j, k) in kernel.iter().enumerate() {
                let idx = (i + j as i64 - r).rem_euclid(n as i64) as usize;
                s = add(s, mul(pts[idx], *k));
            }
            mul(s, 1.0 / ksum)
        })
        .collect()
}

/// Indices of corners: the turning angle between the arms of length `arm` (in points) exceeds `deg`.
pub fn find_corners(pts: &[P], arm: usize, deg: f64) -> Vec<usize> {
    let n = pts.len();
    if n < 2 * arm + 3 {
        return vec![];
    }
    let angle: Vec<f64> = (0..n)
        .map(|i| {
            let a = sub(pts[i], pts[(i + n - arm) % n]);
            let b = sub(pts[(i + arm) % n], pts[i]);
            let (la, lb) = (len(a), len(b));
            if la < 1e-9 || lb < 1e-9 {
                0.0
            } else {
                (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees()
            }
        })
        .collect();
    let mut out = Vec::new();
    for i in 0..n {
        if angle[i] < deg {
            continue;
        }
        // keep only the local maximum inside +-arm
        let mut is_max = true;
        for d in 1..=arm {
            let (l, r) = (angle[(i + n - d) % n], angle[(i + d) % n]);
            if l > angle[i] || (r > angle[i]) || (l == angle[i] && d > 0 && (i + n - d) % n < i) {
                is_max = false;
                break;
            }
        }
        if is_max {
            out.push(i);
        }
    }
    out
}

/// Turning angle (degrees) at point `i` between the chords of length `arm` points on each side.
fn turn_at(pts: &[P], i: usize, arm: usize) -> f64 {
    let n = pts.len();
    let a = sub(pts[i], pts[(i + n - arm) % n]);
    let b = sub(pts[(i + arm) % n], pts[i]);
    let (la, lb) = (len(a), len(b));
    if la < 1e-9 || lb < 1e-9 {
        0.0
    } else {
        (dot(a, b) / (la * lb)).clamp(-1.0, 1.0).acos().to_degrees()
    }
}

/// Straightness (chord / path length, 1.0 = ruler straight) of the arm of `arm` points that ends (`forward` = false) or
/// starts (`forward` = true) at point `i`.
fn arm_straightness(pts: &[P], i: usize, arm: usize, forward: bool) -> f64 {
    let n = pts.len();
    let idx = |k: usize| if forward { (i + k) % n } else { (i + n - k) % n };
    let mut path = 0.0;
    for k in 0..arm {
        path += len(sub(pts[idx(k + 1)], pts[idx(k)]));
    }
    if path < 1e-9 {
        1.0
    } else {
        (len(sub(pts[idx(arm)], pts[i])) / path).min(1.0)
    }
}

/// Turning angle (degrees) between the two straight arms of a corner, measured on lines fitted to the points beyond the
/// rounding that smoothing leaves at the corner. `None` when an arm is not straight (an organic bend).
fn geometric_turn(pts: &[P], i: usize, arm: usize) -> Option<f64> {
    let n = pts.len();
    let (kn, kf) = (arm, (arm * 3).min(n / 4));
    if kf < kn + 3 {
        return None;
    }
    let side = |forward: bool| -> Vec<P> { (kn..=kf).map(|k| if forward { pts[(i + k) % n] } else { pts[(i + n - k) % n] }).collect() };
    let ((_, dl, el), (_, dr, er)) = (line_through(&side(false)), line_through(&side(true)));
    if el > 0.8 || er > 0.8 {
        return None;
    }
    // the arm directions must point away from the corner: orient them along the walk
    let away_l = sub(pts[(i + n - kn) % n], pts[(i + n - kf) % n]);
    let away_r = sub(pts[(i + kf) % n], pts[(i + kn) % n]);
    let dl = if dot(dl, away_l) >= 0.0 { dl } else { mul(dl, -1.0) };
    let dr = if dot(dr, away_r) >= 0.0 { dr } else { mul(dr, -1.0) };
    Some(dot(dl, dr).clamp(-1.0, 1.0).acos().to_degrees())
}

/// Corner or smooth curve, decided locally for every candidate. A real corner keeps its turning angle when it is measured
/// over a longer stretch and has straight arms; a tight but organic bend (a leaf tip, the end of a stroke) keeps turning
/// further away from the point and its arms are curved, so it stays a smooth Bezier.
pub fn find_corners_adaptive(pts: &[P], arm: usize, deg: f64) -> Vec<usize> {
    let n = pts.len();
    if n < 2 * arm + 3 {
        return vec![];
    }
    let arm_mid = (arm * 2).min(n / 4).max(arm);
    let mut out = Vec::new();
    for i in find_corners(pts, arm, deg * 0.55) {
        let a_s = turn_at(pts, i, arm);
        let a_m = turn_at(pts, i, arm_mid);
        let (sl, sr) = (arm_straightness(pts, i, arm_mid, false), arm_straightness(pts, i, arm_mid, true));
        let ratio = a_m / a_s.max(1e-6);
        let straight_arms = sl >= 0.985 && sr >= 0.985;
        let keep = if a_s >= deg {
            ratio <= 1.6 || straight_arms
        } else {
            // a shallower bend is still a corner when it is geometric: straight arms, no further turning
            (ratio <= 1.2 && sl >= 0.99 && sr >= 0.99) || geometric_turn(pts, i, arm).map_or(false, |t| t >= deg * 0.85)
        };
        if keep {
            out.push(i);
        }
    }
    out
}

/// Principal line through points: (centroid, unit direction, largest distance of a point from the line).
fn line_through(pts: &[P]) -> (P, P, f64) {
    let n = pts.len() as f64;
    let c = (pts.iter().map(|p| p.0).sum::<f64>() / n, pts.iter().map(|p| p.1).sum::<f64>() / n);
    let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
    for p in pts {
        let d = sub(*p, c);
        sxx += d.0 * d.0;
        syy += d.1 * d.1;
        sxy += d.0 * d.1;
    }
    let theta = 0.5 * (2.0 * sxy).atan2(sxx - syy);
    let dir = (theta.cos(), theta.sin());
    let dev = pts.iter().map(|p| { let d = sub(*p, c); (d.0 * dir.1 - d.1 * dir.0).abs() }).fold(0.0, f64::max);
    (c, dir, dev)
}

/// Smoothing rounds a corner and moves its point inwards. When both arms are straight, the corner is the crossing of the
/// two arm lines (a star tip or a letter corner stays sharp and keeps its size).
fn refine_corner(sm: &[P], i: usize, arm: usize) -> P {
    let n = sm.len();
    let (kn, kf) = (arm, (arm * 3).min(n / 4));
    if kf < kn + 3 {
        return sm[i];
    }
    let side = |forward: bool| -> Vec<P> { (kn..=kf).map(|k| if forward { sm[(i + k) % n] } else { sm[(i + n - k) % n] }).collect() };
    let ((cl, dl, el), (cr, dr, er)) = (line_through(&side(false)), line_through(&side(true)));
    if el > 0.8 || er > 0.8 {
        return sm[i];
    }
    let det = dl.0 * dr.1 - dl.1 * dr.0;
    if det.abs() < 0.2 {
        return sm[i];
    }
    let (dx, dy) = (cr.0 - cl.0, cr.1 - cl.1);
    let t = (dx * dr.1 - dy * dr.0) / det;
    let q = (cl.0 + dl.0 * t, cl.1 + dl.1 * t);
    if len(sub(q, sm[i])) > 2.5 * arm as f64 {
        sm[i]
    } else {
        q
    }
}

/// Largest distance of the points of `run` from the straight line through its ends.
fn max_dev_from_chord(run: &[P]) -> f64 {
    let (a, b) = (run[0], run[run.len() - 1]);
    let ab = sub(b, a);
    let l = len(ab);
    if l < 1e-9 {
        return f64::MAX;
    }
    run.iter().map(|&p| (ab.0 * (p.1 - a.1) - ab.1 * (p.0 - a.0)).abs() / l).fold(0.0, f64::max)
}

/// Like `max_dev_from_chord`, but ignores `skip` points at each end: smoothing rounds a corner, so the points right next to
/// a corner leave the (refined) edge line even when the edge itself is ruler straight.
fn max_dev_inner(run: &[P], skip: usize) -> f64 {
    if run.len() < 2 * skip + 3 {
        return max_dev_from_chord(run);
    }
    let (a, b) = (run[0], run[run.len() - 1]);
    let ab = sub(b, a);
    let l = len(ab);
    if l < 1e-9 {
        return f64::MAX;
    }
    run[skip..run.len() - skip].iter().map(|&p| (ab.0 * (p.1 - a.1) - ab.1 * (p.0 - a.0)).abs() / l).fold(0.0, f64::max)
}

/// A straight segment written as a cubic whose handles lie exactly on the chord (the exporter writes it as `L`).
fn straight_segment(a: P, b: P) -> Bezier {
    [a, add(a, mul(sub(b, a), 1.0 / 3.0)), add(a, mul(sub(b, a), 2.0 / 3.0)), b]
}

// ---------- Schneider ----------

fn bezier_at(b: &Bezier, t: f64) -> P {
    let mt = 1.0 - t;
    let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
    (b0 * b[0].0 + b1 * b[1].0 + b2 * b[2].0 + b3 * b[3].0, b0 * b[0].1 + b1 * b[1].1 + b2 * b[2].1 + b3 * b[3].1)
}

fn bezier_d1(b: &Bezier, t: f64) -> P {
    let mt = 1.0 - t;
    let q = [mul(sub(b[1], b[0]), 3.0), mul(sub(b[2], b[1]), 3.0), mul(sub(b[3], b[2]), 3.0)];
    add(add(mul(q[0], mt * mt), mul(q[1], 2.0 * mt * t)), mul(q[2], t * t))
}

fn bezier_d2(b: &Bezier, t: f64) -> P {
    let q = [mul(add(sub(b[2], mul(b[1], 2.0)), b[0]), 6.0), mul(add(sub(b[3], mul(b[2], 2.0)), b[1]), 6.0)];
    add(mul(q[0], 1.0 - t), mul(q[1], t))
}

fn chord_params(d: &[P]) -> Vec<f64> {
    let mut u = vec![0.0; d.len()];
    for i in 1..d.len() {
        u[i] = u[i - 1] + len(sub(d[i], d[i - 1]));
    }
    let total = u[d.len() - 1];
    if total > 0.0 {
        for v in u.iter_mut() {
            *v /= total;
        }
    }
    u
}

fn generate(d: &[P], u: &[f64], t1: P, t2: P) -> Bezier {
    let n = d.len();
    let (p0, p3) = (d[0], d[n - 1]);
    let (mut c00, mut c01, mut c11) = (0.0, 0.0, 0.0);
    let (mut x0, mut x1) = (0.0, 0.0);
    for i in 0..n {
        let t = u[i];
        let mt = 1.0 - t;
        let (b0, b1, b2, b3) = (mt * mt * mt, 3.0 * mt * mt * t, 3.0 * mt * t * t, t * t * t);
        let a1 = mul(t1, b1);
        let a2 = mul(t2, b2);
        c00 += dot(a1, a1);
        c01 += dot(a1, a2);
        c11 += dot(a2, a2);
        let tmp = sub(d[i], add(mul(p0, b0 + b1), mul(p3, b2 + b3)));
        x0 += dot(a1, tmp);
        x1 += dot(a2, tmp);
    }
    let det = c00 * c11 - c01 * c01;
    let seg = len(sub(p3, p0));
    // length of the polyline: a handle can never sensibly be longer than half of it
    let run_len: f64 = d.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
    let max_a = 0.5 * run_len;
    let (mut a1, mut a2) = if det.abs() > 1e-12 { ((x0 * c11 - x1 * c01) / det, (c00 * x1 - c01 * x0) / det) } else { (0.0, 0.0) };
    if !(a1.is_finite() && a2.is_finite()) || a1 < 1e-6 * run_len || a2 < 1e-6 * run_len || a1 > max_a || a2 > max_a {
        // fall back to the chord heuristic (a closed run has no chord: use a sixth of its length)
        let f = if seg > 1e-9 { seg / 3.0 } else { run_len / 6.0 };
        a1 = f;
        a2 = f;
    }
    [p0, add(p0, mul(t1, a1)), add(p3, mul(t2, a2)), p3]
}

fn reparam(b: &Bezier, d: &[P], u: &[f64]) -> Vec<f64> {
    d.iter()
        .zip(u)
        .map(|(&p, &t)| {
            let diff = sub(bezier_at(b, t), p);
            let d1 = bezier_d1(b, t);
            let d2 = bezier_d2(b, t);
            let num = dot(diff, d1);
            let den = dot(d1, d1) + dot(diff, d2);
            if den.abs() < 1e-12 {
                t
            } else {
                (t - num / den).clamp(0.0, 1.0)
            }
        })
        .collect()
}

fn max_error(d: &[P], b: &Bezier, u: &[f64]) -> (f64, usize) {
    let mut worst = (0.0, d.len() / 2);
    for i in 1..d.len() - 1 {
        let e = sub(bezier_at(b, u[i]), d[i]);
        let dist = dot(e, e);
        if dist > worst.0 {
            worst = (dist, i);
        }
    }
    worst
}

fn fit_cubic(d: &[P], t1: P, t2: P, err2: f64, out: &mut Vec<Bezier>, depth: usize) {
    let n = d.len();
    if n == 2 {
        let dist = len(sub(d[1], d[0])) / 3.0;
        out.push([d[0], add(d[0], mul(t1, dist)), add(d[1], mul(t2, dist)), d[1]]);
        return;
    }
    let mut u = chord_params(d);
    let mut b = generate(d, &u, t1, t2);
    let (mut e, mut split) = max_error(d, &b, &u);
    if e < err2 {
        out.push(b);
        return;
    }
    if e < err2 * 16.0 {
        for _ in 0..4 {
            u = reparam(&b, d, &u);
            b = generate(d, &u, t1, t2);
            let r = max_error(d, &b, &u);
            e = r.0;
            split = r.1;
            if e < err2 {
                out.push(b);
                return;
            }
        }
    }
    if depth > 40 || split == 0 || split >= n - 1 {
        out.push(b);
        return;
    }
    // a wide baseline: with +-1 px neighbours the direction is dominated by pixel noise
    let k = (n / 12).clamp(5, 14).min(split).min(n - 1 - split).max(1);
    let center = norm(sub(d[split - k], d[split + k]));
    fit_cubic(&d[..=split], t1, center, err2, out, depth + 1);
    fit_cubic(&d[split..], mul(center, -1.0), t2, err2, out, depth + 1);
}

/// Fit an open run of points with cubic Beziers; `t1`/`t2` point into the run from each end.
/// Post-fit greedy merge of adjacent smooth cubic Bézier segments within a single run
/// (between real corners). Reduces redundant nodes on long smooth curves while preserving
/// accuracy (`max_err <= tol * 1.15`) and never merging across real corners.
fn merge_adjacent_beziers(curves: &mut Vec<Bezier>, tol: f64) {
    if curves.len() < 2 || !tol.is_finite() || tol <= 0.0 {
        return;
    }
    let max_err2 = (tol * 0.75) * (tol * 0.75);
    const SAMPLES: usize = 16;
    let mut changed = true;
    while changed && curves.len() >= 2 {
        changed = false;
        let mut i = 0usize;
        while i + 1 < curves.len() {
            let b0 = curves[i];
            let b1 = curves[i + 1];
            // Must be continuous and smoothly joined (do not merge across a cusp/corner)
            if len(sub(b0[3], b1[0])) > 1e-2 {
                i += 1;
                continue;
            }
            let t_out = norm(sub(b0[3], b0[2]));
            let t_in = norm(sub(b1[1], b1[0]));
            if len(t_out) > 0.5 && len(t_in) > 0.5 && dot(t_out, t_in) < 0.70 {
                i += 1;
                continue;
            }
            let mut pts = Vec::with_capacity(SAMPLES * 2 + 1);
            for k in 0..SAMPLES {
                pts.push(bezier_at(&b0, k as f64 / SAMPLES as f64));
            }
            for k in 0..=SAMPLES {
                pts.push(bezier_at(&b1, k as f64 / SAMPLES as f64));
            }
            let t1 = {
                let v = norm(sub(b0[1], b0[0]));
                if len(v) > 0.5 { v } else { norm(sub(b0[3], b0[0])) }
            };
            let t2 = {
                let v = norm(sub(b1[2], b1[3]));
                if len(v) > 0.5 { v } else { norm(sub(b1[0], b1[3])) }
            };
            let mut u = chord_params(&pts);
            let mut m = generate(&pts, &u, t1, t2);
            let mut err2 = max_error(&pts, &m, &u).0;
            if err2 > max_err2 && err2 < max_err2 * 16.0 {
                for _ in 0..4 {
                    u = reparam(&m, &pts, &u);
                    m = generate(&pts, &u, t1, t2);
                    err2 = max_error(&pts, &m, &u).0;
                    if err2 <= max_err2 {
                        break;
                    }
                }
            }
            let run_len: f64 = pts.windows(2).map(|w| len(sub(w[1], w[0]))).sum();
            let h1 = sub(m[1], m[0]);
            let h2 = sub(m[2], m[3]);
            let l1 = len(h1);
            let l2 = len(h2);
            let handles_ok = run_len > 1e-6
                && dot(h1, t1) > 0.0
                && dot(h2, t2) > 0.0
                && l1 >= 1e-4 * run_len
                && l2 >= 1e-4 * run_len
                && l1 <= 0.55 * run_len
                && l2 <= 0.55 * run_len;
            if err2 <= max_err2 && handles_ok {
                curves[i] = m;
                curves.remove(i + 1);
                changed = true;
            } else {
                i += 1;
            }
        }
    }
}

fn fit_open_raw(d: &[P], t1: P, t2: P, tol: f64) -> Vec<Bezier> {
    let mut out = Vec::new();
    if d.len() < 2 {
        return out;
    }
    fit_cubic(d, t1, t2, tol * tol, &mut out, 0);
    out
}

fn fit_open(d: &[P], t1: P, t2: P, tol: f64) -> Vec<Bezier> {
    let mut out = fit_open_raw(d, t1, t2, tol);
    merge_adjacent_beziers(&mut out, tol);
    out
}

/// Insert points so that no edge of the closed ring is longer than `step` (the boundary walker only reports turning points).
pub fn resample_ring(pts: &[P], step: f64) -> Vec<P> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 2);
    for i in 0..n {
        let (a, b) = (pts[i], pts[(i + 1) % n]);
        let d = len(sub(b, a));
        let k = (d / step).ceil().max(1.0) as usize;
        for j in 0..k {
            let t = j as f64 / k as f64;
            out.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    out
}

/// The smoothed outline a ring is fitted to (1 px spacing): used to judge basic shapes independently of the fit tolerance.
pub fn ring_points(raw: &[P], sigma: f64) -> Vec<P> {
    let mut pts = raw.to_vec();
    if pts.len() > 1 && len(sub(pts[0], pts[pts.len() - 1])) < 1e-9 {
        pts.pop();
    }
    if pts.len() < 3 {
        return pts;
    }
    smooth_ring(&resample_ring(&pts, 1.0), sigma)
}

/// Fit a closed ring. Corners (turning angle above `corner_deg`) stay sharp; everything else becomes smooth curves
/// with the fewest segments that stay within `tol` pixels of the smoothed outline.
pub fn fit_ring(raw: &[P], sigma: f64, tol: f64, corner_deg: f64) -> Vec<Bezier> {
    fit_ring_zoned(raw, sigma, tol, corner_deg, None)
}

/// Zone of a point for the correction brush: 0 normal, 1 detail, 2 smooth.
pub type ZoneFn<'a> = &'a dyn Fn(P) -> u8;

/// `fit_ring` with a brush zone function: inside a smooth zone the outline is smoothed more, corners need to be sharper and
/// the curve fit is looser; inside a detail zone the opposite.
pub fn fit_ring_zoned(raw: &[P], sigma: f64, tol: f64, corner_deg: f64, zone: Option<ZoneFn>) -> Vec<Bezier> {
    let mut pts = raw.to_vec();
    // drop a duplicated closing point
    if pts.len() > 1 && len(sub(pts[0], pts[pts.len() - 1])) < 1e-9 {
        pts.pop();
    }
    if pts.len() < 3 {
        return vec![];
    }
    let pts = resample_ring(&pts, 1.0);
    let n = pts.len();
    let sm = match zone {
        None => smooth_ring(&pts, sigma),
        Some(z) => {
            let (base, soft, crisp) = (smooth_ring(&pts, sigma), smooth_ring(&pts, sigma * 1.8), smooth_ring(&pts, sigma * 0.5));
            (0..n).map(|i| match z(pts[i]) { crate::brush::ZONE_SMOOTH => soft[i], crate::brush::ZONE_DETAIL => crisp[i], _ => base[i] }).collect()
        }
    };
    let arm = ((sigma * 2.0).ceil() as usize + 2).min(n / 4).max(1);
    let mut corners = find_corners_adaptive(&sm, arm, corner_deg);
    if let Some(z) = zone {
        // in a smooth zone only a really sharp corner stays a corner
        corners.retain(|&c| z(sm[c]) != crate::brush::ZONE_SMOOTH || turn_at(&sm, c, arm) >= corner_deg * 1.4);
    }

    if corners.is_empty() {
        // smooth closed curve: start anywhere, tangents from the ring neighbours so the join is smooth
        let m = (arm).min(n / 2).max(1);
        let t = norm(sub(sm[m % n], sm[(n - m) % n]));
        let mut run: Vec<P> = sm.clone();
        run.push(sm[0]);
        return fit_open_z(&run, t, mul(t, -1.0), tol, zone);
    }
    corners.sort_unstable();
    let corner_pos: Vec<P> = corners.iter().map(|&c| refine_corner(&sm, c, arm)).collect();
    let mut curves = Vec::new();
    let k = corners.len();
    for ci in 0..k {
        let (a, b) = (corners[ci], corners[(ci + 1) % k]);
        // points from a to b going forward around the ring
        let mut run: Vec<P> = Vec::new();
        let mut i = a;
        loop {
            run.push(sm[i]);
            if i == b && !run.is_empty() && run.len() > 1 {
                break;
            }
            i = (i + 1) % n;
            if k == 1 && i == a {
                run.push(sm[a]);
                break;
            }
        }
        if run.len() < 2 {
            continue;
        }
        // the run starts and ends on a corner: use the refined corner positions
        let last = run.len() - 1;
        run[0] = corner_pos[ci];
        run[last] = corner_pos[(ci + 1) % k];
        // between two corners a run that stays within the tolerance of its chord is a straight edge
        let local_tol = zone.map_or(tol, |z| tol * crate::brush::tol_factor(z(run[run.len() / 2])));
        if k > 1 && run.len() >= 3 && max_dev_inner(&run, arm) <= local_tol * 0.7 {
            curves.push(straight_segment(run[0], run[run.len() - 1]));
            continue;
        }
        let m = arm.min(run.len() - 1).max(1);
        let t1 = norm(sub(run[m], run[0]));
        let t2 = norm(sub(run[run.len() - 1 - m], run[run.len() - 1]));
        curves.extend(fit_open_z(&run, t1, t2, tol, zone));
    }
    curves
}

/// Fit an open polyline (a stroke centerline) with as few Beziers as `tol` allows.
pub fn fit_polyline(pts: &[P], tol: f64) -> Vec<Bezier> {
    if pts.len() < 2 {
        return vec![];
    }
    if pts.len() == 2 {
        let d = len(sub(pts[1], pts[0])) / 3.0;
        let t = norm(sub(pts[1], pts[0]));
        return vec![[pts[0], add(pts[0], mul(t, d)), sub(pts[1], mul(t, d)), pts[1]]];
    }
    if pts.len() >= 3 && max_dev_from_chord(pts) <= tol * 0.5 {
        return vec![straight_segment(pts[0], pts[pts.len() - 1])];
    }
    let m = 4.min(pts.len() - 1);
    let t1 = norm(sub(pts[m], pts[0]));
    let t2 = norm(sub(pts[pts.len() - 1 - m], pts[pts.len() - 1]));
    fit_open_raw(pts, t1, t2, tol)
}

/// `fit_open` with a brush zone function: the run is cut where the zone changes and every piece is fitted with the
/// tolerance of its own zone. The pieces share their end points and tangents, so the curve stays closed and smooth.
fn fit_open_z(d: &[P], t1: P, t2: P, tol: f64, zone: Option<ZoneFn>) -> Vec<Bezier> {
    let Some(z) = zone else { return fit_open(d, t1, t2, tol) };
    if d.len() < 2 {
        return vec![];
    }
    let zs: Vec<u8> = d.iter().map(|&p| z(p)).collect();
    // cut where the zone changes (a piece must be at least 8 points long, shorter blips are absorbed)
    let mut cuts = vec![0usize];
    let (mut cur, mut start) = (zs[0], 0usize);
    for i in 1..d.len() {
        if zs[i] != cur && i - start >= 8 {
            cuts.push(i);
            cur = zs[i];
            start = i;
        }
    }
    cuts.push(d.len() - 1);
    cuts.dedup();
    if cuts.len() < 2 {
        return fit_open(d, t1, t2, tol);
    }
    let mut out = Vec::new();
    for k in 0..cuts.len() - 1 {
        let (a, b) = (cuts[k], cuts[k + 1]);
        let piece = &d[a..=b];
        if piece.len() < 2 {
            continue;
        }
        // majority zone of the piece
        let mut cnt = [0usize; 3];
        for &zz in &zs[a..=b] {
            cnt[(zz as usize).min(2)] += 1;
        }
        let zone_of_piece = (0..3).max_by_key(|&i| cnt[i]).unwrap() as u8;
        let local = tol * crate::brush::tol_factor(zone_of_piece);
        let wide = 5.min(a).min(d.len() - 1 - a);
        let ta = if a == 0 { t1 } else { mul(norm(sub(d[a - wide.max(1)], d[(a + wide.max(1)).min(d.len() - 1)])), -1.0) };
        let wide_b = 5.min(b).min(d.len() - 1 - b);
        let tb = if b == d.len() - 1 { t2 } else { norm(sub(d[b - wide_b.max(1)], d[(b + wide_b.max(1)).min(d.len() - 1)])) };
        let mut piece_out = Vec::new();
        fit_cubic(piece, ta, tb, local * local, &mut piece_out, 0);
        if zone_of_piece != crate::brush::ZONE_DETAIL { merge_adjacent_beziers(&mut piece_out, local); }
        out.extend(piece_out);
    }
    out
}
