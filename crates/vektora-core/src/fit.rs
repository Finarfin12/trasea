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
    let center = norm(sub(d[split - 1], d[split + 1]));
    fit_cubic(&d[..=split], t1, center, err2, out, depth + 1);
    fit_cubic(&d[split..], mul(center, -1.0), t2, err2, out, depth + 1);
}

/// Fit an open run of points with cubic Beziers; `t1`/`t2` point into the run from each end.
fn fit_open(d: &[P], t1: P, t2: P, tol: f64) -> Vec<Bezier> {
    let mut out = Vec::new();
    if d.len() < 2 {
        return out;
    }
    fit_cubic(d, t1, t2, tol * tol, &mut out, 0);
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

/// Fit a closed ring. Corners (turning angle above `corner_deg`) stay sharp; everything else becomes smooth curves
/// with the fewest segments that stay within `tol` pixels of the smoothed outline.
pub fn fit_ring(raw: &[P], sigma: f64, tol: f64, corner_deg: f64) -> Vec<Bezier> {
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
    let sm = smooth_ring(&pts, sigma);
    let arm = ((sigma * 2.0).ceil() as usize + 2).min(n / 4).max(1);
    let mut corners = find_corners(&sm, arm, corner_deg);

    if corners.is_empty() {
        // smooth closed curve: start anywhere, tangents from the ring neighbours so the join is smooth
        let m = (arm).min(n / 2).max(1);
        let t = norm(sub(sm[m % n], sm[(n - m) % n]));
        let mut run: Vec<P> = sm.clone();
        run.push(sm[0]);
        return fit_open(&run, t, mul(t, -1.0), tol);
    }
    corners.sort_unstable();
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
        let m = arm.min(run.len() - 1).max(1);
        let t1 = norm(sub(run[m], run[0]));
        let t2 = norm(sub(run[run.len() - 1 - m], run[run.len() - 1]));
        curves.extend(fit_open(&run, t1, t2, tol));
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
    let m = 4.min(pts.len() - 1);
    let t1 = norm(sub(pts[m], pts[0]));
    let t2 = norm(sub(pts[pts.len() - 1 - m], pts[pts.len() - 1]));
    fit_open(pts, t1, t2, tol)
}
