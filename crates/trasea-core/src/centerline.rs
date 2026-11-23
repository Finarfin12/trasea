//! Centerline extraction: thin-line mask -> skeleton -> graph -> pruned strokes -> Bezier paths + measured width.
//!
//! A line that has an even width becomes ONE open path with `stroke-width`, instead of a filled outline (which needs
//! two edges and many more nodes, and cannot be thickened in an editor). Lines whose width varies a lot (brush
//! strokes, blobs) are reported as `shape` so the caller keeps their outline.

use crate::fit::{fit_polyline, Bezier, P};

pub struct Stroke {
    /// Open path in working-pixel coordinates.
    pub curves: Vec<Bezier>,
    /// Stroke width in working pixels, rounded to `WIDTH_STEP`.
    pub width: f64,
}

pub struct CenterlineResult {
    pub strokes: Vec<Stroke>,
    /// Mask pixels (working resolution) of components that are NOT good strokes (uneven width): trace these as shapes.
    pub shape_mask: Vec<bool>,
}

const WIDTH_STEP: f64 = 0.25;

// ---------- Zhang-Suen thinning ----------

fn thin(mask: &[bool], w: usize, h: usize) -> Vec<bool> {
    let mut img: Vec<u8> = mask.iter().map(|&b| b as u8).collect();
    // keep a 1 px empty frame so neighbour reads never go out of range
    for x in 0..w {
        img[x] = 0;
        img[(h - 1) * w + x] = 0;
    }
    for y in 0..h {
        img[y * w] = 0;
        img[y * w + w - 1] = 0;
    }
    let mut changed = true;
    let mut to_clear: Vec<usize> = Vec::new();
    while changed {
        changed = false;
        for step in 0..2 {
            to_clear.clear();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if img[i] == 0 {
                        continue;
                    }
                    // p2..p9 clockwise from north
                    let p = [img[i - w], img[i - w + 1], img[i + 1], img[i + w + 1], img[i + w], img[i + w - 1], img[i - 1], img[i - w - 1]];
                    let b: u8 = p.iter().sum();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&k| p[k] == 0 && p[(k + 1) % 8] == 1).count();
                    if a != 1 {
                        continue;
                    }
                    let ok = if step == 0 { p[0] * p[2] * p[4] == 0 && p[2] * p[4] * p[6] == 0 } else { p[0] * p[2] * p[6] == 0 && p[0] * p[4] * p[6] == 0 };
                    if ok {
                        to_clear.push(i);
                    }
                }
            }
            if !to_clear.is_empty() {
                changed = true;
                for &i in &to_clear {
                    img[i] = 0;
                }
            }
        }
    }
    // Topology-preserving L-corner cleanup: Zhang-Suen leaves 2-px diagonal staircases
    // whose extra corner pixels create false deg>=3 junction clusters along 45-deg curves.
    // Removing a pixel with two orthogonal neighbours (e.g. N & E) when S, W, SW are all 0
    // is provably 8-connectivity-preserving and never deletes an endpoint.
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            if img[i] == 0 {
                continue;
            }
            let (n, ne, e, se, s, sw, ww, nw) = (
                img[i - w],
                img[i - w + 1],
                img[i + 1],
                img[i + w + 1],
                img[i + w],
                img[i + w - 1],
                img[i - 1],
                img[i - w - 1],
            );
            let redundant = (n == 1 && e == 1 && s == 0 && ww == 0 && sw == 0)
                || (e == 1 && s == 1 && ww == 0 && n == 0 && nw == 0)
                || (s == 1 && ww == 1 && n == 0 && e == 0 && ne == 0)
                || (ww == 1 && n == 1 && e == 0 && s == 0 && se == 0);
            if redundant {
                img[i] = 0;
            }
        }
    }
    img.iter().map(|&v| v == 1).collect()
}

// ---------- distance transform (chamfer 3-4, in pixels) ----------

fn distance_inside(mask: &[bool], w: usize, h: usize) -> Vec<f32> {
    const INF: f32 = 1e9;
    let mut d: Vec<f32> = mask.iter().map(|&b| if b { INF } else { 0.0 }).collect();
    let (a, b) = (1.0f32, 1.4142f32);
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            if d[i] == 0.0 {
                continue;
            }
            let mut v = d[i];
            if x > 0 {
                v = v.min(d[i - 1] + a);
            } else {
                v = v.min(a);
            }
            if y > 0 {
                v = v.min(d[i - w] + a);
                if x > 0 {
                    v = v.min(d[i - w - 1] + b);
                }
                if x + 1 < w {
                    v = v.min(d[i - w + 1] + b);
                }
            } else {
                v = v.min(a);
            }
            d[i] = v;
        }
    }
    for y in (0..h).rev() {
        for x in (0..w).rev() {
            let i = y * w + x;
            if d[i] == 0.0 {
                continue;
            }
            let mut v = d[i];
            if x + 1 < w {
                v = v.min(d[i + 1] + a);
            } else {
                v = v.min(a);
            }
            if y + 1 < h {
                v = v.min(d[i + w] + a);
                if x + 1 < w {
                    v = v.min(d[i + w + 1] + b);
                }
                if x > 0 {
                    v = v.min(d[i + w - 1] + b);
                }
            } else {
                v = v.min(a);
            }
            d[i] = v;
        }
    }
    d
}

// ---------- skeleton graph ----------

fn neighbours(i: usize, w: usize, skel: &[bool]) -> Vec<usize> {
    let (x, y) = ((i % w) as i32, (i / w) as i32);
    let mut out = Vec::with_capacity(8);
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if dx == 0 && dy == 0 {
                continue;
            }
            let (nx, ny) = (x + dx, y + dy);
            if nx < 0 || ny < 0 {
                continue;
            }
            let j = ny as usize * w + nx as usize;
            if j < skel.len() && skel[j] && (nx as usize) < w {
                out.push(j);
            }
        }
    }
    out
}

struct Edge {
    a: usize,
    b: usize,
    /// pixel indices from node a to node b (endpoints are node pixels)
    pix: Vec<usize>,
}

/// Skeleton -> edges between nodes (endpoints and junction clusters). Returns (edges, node count).
fn build_graph(skel: &[bool], w: usize, h: usize) -> (Vec<Edge>, usize) {
    let n = w * h;
    let deg: Vec<usize> = (0..n).map(|i| if skel[i] { neighbours(i, w, skel).len() } else { 0 }).collect();
    // junction pixel = more than 2 neighbours; cluster adjacent junction pixels into one node
    let mut node_of = vec![usize::MAX; n];
    let mut nodes = 0usize;
    for i in 0..n {
        if !skel[i] || node_of[i] != usize::MAX {
            continue;
        }
        if deg[i] == 1 || deg[i] == 0 {
            node_of[i] = nodes;
            nodes += 1;
        } else if deg[i] > 2 {
            let id = nodes;
            nodes += 1;
            let mut stack = vec![i];
            node_of[i] = id;
            while let Some(p) = stack.pop() {
                for q in neighbours(p, w, skel) {
                    if deg[q] > 2 && node_of[q] == usize::MAX {
                        node_of[q] = id;
                        stack.push(q);
                    }
                }
            }
        }
    }
    let mut visited = vec![false; n];
    let mut edges = Vec::new();
    // walk from every node pixel along every non-node neighbour
    for start in 0..n {
        if !skel[start] || node_of[start] == usize::MAX {
            continue;
        }
        for first in neighbours(start, w, skel) {
            if node_of[first] != usize::MAX && node_of[first] == node_of[start] {
                continue; // inside the same junction cluster
            }
            if node_of[first] == usize::MAX && visited[first] {
                continue;
            }
            let mut pix = vec![start];
            let (mut prev, mut cur) = (start, first);
            loop {
                pix.push(cur);
                if node_of[cur] != usize::MAX {
                    break;
                }
                visited[cur] = true;
                let next = neighbours(cur, w, skel).into_iter().find(|&q| q != prev && !(pix.len() > 1 && q == pix[pix.len() - 2]));
                match next {
                    Some(q) => {
                        prev = cur;
                        cur = q;
                    }
                    None => break,
                }
                if pix.len() > n {
                    break;
                }
            }
            let end = *pix.last().unwrap();
            if node_of[end] != usize::MAX && !(node_of[end] == node_of[start] && pix.len() < 3) {
                edges.push(Edge { a: node_of[start], b: node_of[end], pix });
            }
        }
    }
    // an edge walked from both ends appears twice: keep one
    let mut seen = std::collections::HashSet::new();
    edges.retain(|e| {
        let (f, l) = (e.pix[1.min(e.pix.len() - 1)], e.pix[e.pix.len().saturating_sub(2)]);
        let key = (e.a.min(e.b), e.a.max(e.b), f.min(l), f.max(l), e.pix.len());
        seen.insert(key)
    });
    (edges, nodes)
}

/// Remove short side branches that end at an endpoint, then join edges through nodes that are left with two edges.
fn prune_and_merge(mut edges: Vec<Edge>, nodes: usize, min_spur: usize) -> Vec<Edge> {
    loop {
        let mut deg = vec![0usize; nodes];
        for e in &edges {
            deg[e.a] += 1;
            deg[e.b] += 1;
        }
        // shortest spur or tiny junction loop first
        let mut best: Option<(usize, usize)> = None;
        for (k, e) in edges.iter().enumerate() {
            let spur = (deg[e.a] == 1 && deg[e.b] >= 3)
                || (deg[e.b] == 1 && deg[e.a] >= 3)
                || (e.a == e.b && deg[e.a] >= 4 && e.pix.len() < min_spur);
            if spur && e.pix.len() < min_spur && best.map_or(true, |(_, l)| e.pix.len() < l) {
                best = Some((k, e.pix.len()));
            }
        }
        match best {
            Some((k, _)) => {
                edges.remove(k);
            }
            None => break,
        }
    }
    // merge through degree-2 nodes
    loop {
        let mut deg = vec![Vec::new(); nodes];
        for (k, e) in edges.iter().enumerate() {
            deg[e.a].push(k);
            deg[e.b].push(k);
        }
        let target = (0..nodes).find(|&nd| {
            deg[nd].len() == 2 && deg[nd][0] != deg[nd][1] && {
                let (e0, e1) = (&edges[deg[nd][0]], &edges[deg[nd][1]]);
                !(e0.a == e0.b || e1.a == e1.b)
            }
        });
        let Some(nd) = target else { break };
        let (i0, i1) = (deg[nd][0], deg[nd][1]);
        let (hi, lo) = (i0.max(i1), i0.min(i1));
        let e_hi = edges.remove(hi);
        let e_lo = edges.remove(lo);
        // orient both so that they meet at nd
        let mut first = e_lo;
        if first.b != nd {
            first.pix.reverse();
            std::mem::swap(&mut first.a, &mut first.b);
        }
        let mut second = e_hi;
        if second.a != nd {
            second.pix.reverse();
            std::mem::swap(&mut second.a, &mut second.b);
        }
        let mut pix = first.pix;
        pix.extend(second.pix);
        edges.push(Edge { a: first.a, b: second.b, pix });
    }
    edges
}

fn pt(i: usize, w: usize) -> P {
    ((i % w) as f64 + 0.5, (i / w) as f64 + 0.5)
}

/// Resample an open polyline to ~1 px spacing, then Gaussian-smooth it (ends stay fixed). Skeleton pixel paths have
/// uneven step lengths (1 or 1.41) and staircase noise; this gives the curve fit a clean input.
fn smooth_open(p: &[P], sigma: f64) -> Vec<P> {
    if p.len() < 3 {
        return p.to_vec();
    }
    let mut dense: Vec<P> = vec![p[0]];
    for seg in p.windows(2) {
        let (a, b) = (seg[0], seg[1]);
        let d = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
        let k = d.ceil().max(1.0) as usize;
        for j in 1..=k {
            let t = j as f64 / k as f64;
            dense.push((a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t));
        }
    }
    let n = dense.len();
    let r = (sigma * 3.0).ceil() as i64;
    let kernel: Vec<f64> = (-r..=r).map(|i| (-(i * i) as f64 / (2.0 * sigma * sigma)).exp()).collect();
    let mut out = dense.clone();
    for i in 1..n - 1 {
        let (mut sx, mut sy, mut sw) = (0.0, 0.0, 0.0);
        for (j, k) in kernel.iter().enumerate() {
            let idx = i as i64 + j as i64 - r;
            if idx < 0 || idx >= n as i64 {
                continue;
            }
            sx += dense[idx as usize].0 * k;
            sy += dense[idx as usize].1 * k;
            sw += k;
        }
        out[i] = (sx / sw, sy / sw);
    }
    out
}

/// Connected components (8-neighbour) of `mask`: (labels 1-based, count).
fn components(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, usize) {
    let mut lab = vec![0u32; w * h];
    let mut n = 0u32;
    let mut stack = Vec::new();
    for s in 0..w * h {
        if !mask[s] || lab[s] != 0 {
            continue;
        }
        n += 1;
        lab[s] = n;
        stack.push(s);
        while let Some(i) = stack.pop() {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if mask[j] && lab[j] == 0 {
                        lab[j] = n;
                        stack.push(j);
                    }
                }
            }
        }
    }
    (lab, n as usize)
}

/// Extract strokes from a thin-line mask at working resolution.
/// `tol` = curve-fit tolerance in working pixels. `uneven` = relative width spread above which a component stays a shape.
pub fn centerline(mask: &[bool], w: usize, h: usize, tol: f64, uneven: f64) -> CenterlineResult {
    centerline_opts(mask, w, h, tol, uneven, &CenterOpts::default())
}

pub fn centerline_opts(mask: &[bool], w: usize, h: usize, tol: f64, uneven: f64, opts: &CenterOpts) -> CenterlineResult {
    let dt = distance_inside(mask, w, h);
    let (labels, ncomp) = components(mask, w, h);
    let mut shape_mask = vec![false; w * h];
    let mut strokes = Vec::new();

    // work component by component: classify, then skeletonise only the good ones
    let mut comp_pixels: Vec<Vec<usize>> = vec![Vec::new(); ncomp + 1];
    for (i, &l) in labels.iter().enumerate() {
        if l != 0 {
            comp_pixels[l as usize].push(i);
        }
    }
    for c in 1..=ncomp {
        let pixels = &comp_pixels[c];
        if pixels.len() < 4 {
            continue;
        }
        // bounding box with a margin, so thinning runs on a small crop
        let (mut x0, mut y0, mut x1, mut y1) = (usize::MAX, usize::MAX, 0usize, 0usize);
        for &i in pixels {
            let (x, y) = (i % w, i / w);
            x0 = x0.min(x);
            y0 = y0.min(y);
            x1 = x1.max(x);
            y1 = y1.max(y);
        }
        let (cw, ch) = (x1 - x0 + 3, y1 - y0 + 3);
        let mut crop = vec![false; cw * ch];
        for &i in pixels {
            crop[(i / w - y0 + 1) * cw + (i % w - x0 + 1)] = true;
        }
        let skel = thin(&crop, cw, ch);
        let skel_len = skel.iter().filter(|&&b| b).count();
        if skel_len < 2 {
            for &i in pixels {
                shape_mask[i] = true;
            }
            continue;
        }
        // width along the skeleton (2 x distance to the edge, minus the pixel quantisation)
        let mut widths: Vec<f64> = Vec::with_capacity(skel_len);
        for (k, &s) in skel.iter().enumerate() {
            if s {
                let gi = (k / cw + y0 - 1) * w + (k % cw + x0 - 1);
                widths.push((2.0 * dt[gi] as f64 - 1.0).max(1.0));
            }
        }
        let mean = widths.iter().sum::<f64>() / widths.len() as f64;
        let var = widths.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / widths.len() as f64;
        let cv = var.sqrt() / mean.max(1.0);
        // The distance transform is exact only for odd widths (an even width reads one pixel low). The area per skeleton
        // pixel is the better width estimate for an even line, so the edge widths are scaled to it. A ratio far from
        // 1-2 means this is not a plain line (a blob or a tangle).
        let area_fit = pixels.len() as f64 / (skel_len as f64 * mean).max(1.0);
        if cv > uneven || !(0.7..=2.4).contains(&area_fit) || mean > 24.0 {
            for &i in pixels {
                shape_mask[i] = true;
            }
            continue;
        }
        let (edges, nn) = build_graph(&skel, cw, ch);
        let spur = (3.0 * mean + 4.0).round() as usize;
        let edges = prune_and_merge(edges, nn, spur);
        if edges.is_empty() {
            for &i in pixels {
                shape_mask[i] = true;
            }
            continue;
        }
        // each edge as a global polyline with its measured width (raw, before rounding)
        let mut segs: Vec<Seg> = Vec::new();
        for e in edges {
            let poly: Vec<P> = e.pix.iter().map(|&i| { let q = pt(i, cw); (q.0 + x0 as f64 - 1.0, q.1 + y0 as f64 - 1.0) }).collect();
            if poly.len() < 2 {
                continue;
            }
            let mut sum = 0.0;
            for &i in &e.pix {
                let gi = (i / cw + y0 - 1) * w + (i % cw + x0 - 1);
                sum += (2.0 * dt[gi] as f64 - 1.0).max(1.0);
            }
            let wraw = (sum / e.pix.len() as f64) * area_fit;
            segs.push(Seg { a: e.a, b: e.b, poly, wraw });
        }
        // tiny fragments that hang on a junction (shorter than 1.5 px) are noise, not strokes
        segs.retain(|s| poly_len(&s.poly) >= MICRO_FRAGMENT || s.a == s.b);
        if opts.merge {
            merge_through_junctions(&mut segs, opts.width_tol);
        }
        for s in segs {
            let sm = smooth_open(&s.poly, 2.0);
            let width = (s.wraw / WIDTH_STEP).round() * WIDTH_STEP;
            let curves = fit_polyline(&sm, tol);
            if !curves.is_empty() {
                strokes.push(Stroke { curves, width: width.max(WIDTH_STEP * 2.0) });
            }
        }
    }
    CenterlineResult { strokes, shape_mask }
}

/// Fragments shorter than this (working px) that hang on a junction are dropped.
const MICRO_FRAGMENT: f64 = 1.5;
/// Two strokes meeting at a junction continue each other when their directions differ by less than this.
const MERGE_ANGLE_DEG: f64 = 35.0;

/// One skeleton edge as a polyline from node `a` to node `b`.
struct Seg {
    a: usize,
    b: usize,
    poly: Vec<P>,
    wraw: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct CenterOpts {
    /// Join strokes that continue each other through a junction.
    pub merge: bool,
    /// Largest width difference (working px) between two strokes that may be joined.
    pub width_tol: f64,
}

impl Default for CenterOpts {
    fn default() -> Self {
        CenterOpts { merge: true, width_tol: 0.75 }
    }
}

fn poly_len(p: &[P]) -> f64 {
    p.windows(2).map(|s| ((s[1].0 - s[0].0).powi(2) + (s[1].1 - s[0].1).powi(2)).sqrt()).sum()
}

/// Unit direction leaving the end of `poly` (its start when `at_start`) into the polyline, over about `k` points.
fn leaving_dir(poly: &[P], at_start: bool, k: usize) -> P {
    let n = poly.len();
    let k = k.min(n - 1).max(1);
    let (from, to) = if at_start { (poly[0], poly[k]) } else { (poly[n - 1], poly[n - 1 - k]) };
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    let l = (dx * dx + dy * dy).sqrt().max(1e-9);
    (dx / l, dy / l)
}

/// At every junction (3 or more strokes), pair strokes that go on in nearly the same direction and have about the same
/// width, and join each pair into one polyline. Greedy, best (straightest) pair first.
fn merge_through_junctions(segs: &mut Vec<Seg>, width_tol: f64) {
    loop {
        let mut best: Option<(f64, usize, bool, usize, bool)> = None; // deviation, edge i, i-at-start, edge j, j-at-start
        let max_node = segs.iter().map(|s| s.a.max(s.b)).max().unwrap_or(0);
        for nd in 0..=max_node {
            // ends of non-loop edges at this node
            let mut ends: Vec<(usize, bool)> = Vec::new();
            let mut total = 0;
            for (k, s) in segs.iter().enumerate() {
                if s.a == nd {
                    total += 1;
                    if s.a != s.b {
                        ends.push((k, true));
                    }
                }
                if s.b == nd {
                    total += 1;
                    if s.a != s.b {
                        ends.push((k, false));
                    }
                }
            }
            if total < 3 {
                continue;
            }
            for x in 0..ends.len() {
                for y in x + 1..ends.len() {
                    let ((i, si), (j, sj)) = (ends[x], ends[y]);
                    if i == j || (segs[i].wraw - segs[j].wraw).abs() > width_tol {
                        continue;
                    }
                    let (di, dj) = (leaving_dir(&segs[i].poly, si, 8), leaving_dir(&segs[j].poly, sj, 8));
                    // straight continuation: di is opposite to dj
                    let cosv = (-(di.0 * dj.0 + di.1 * dj.1)).clamp(-1.0, 1.0);
                    let dev = cosv.acos().to_degrees();
                    if dev < MERGE_ANGLE_DEG && best.map_or(true, |b| dev < b.0) {
                        best = Some((dev, i, si, j, sj));
                    }
                }
            }
        }
        let Some((_, i, si, j, sj)) = best else { break };
        // orient: first ends at the junction, second starts at it
        let mut first = std::mem::replace(&mut segs[i], Seg { a: 0, b: 0, poly: vec![], wraw: 0.0 });
        let mut second = std::mem::replace(&mut segs[j], Seg { a: 0, b: 0, poly: vec![], wraw: 0.0 });
        if si {
            first.poly.reverse();
            std::mem::swap(&mut first.a, &mut first.b);
        }
        if !sj {
            second.poly.reverse();
            std::mem::swap(&mut second.a, &mut second.b);
        }
        let (l1, l2) = (poly_len(&first.poly).max(1.0), poly_len(&second.poly).max(1.0));
        let wraw = (first.wraw * l1 + second.wraw * l2) / (l1 + l2);
        // both ends sit in the junction cluster: meet in the middle
        let (e1, s2) = (*first.poly.last().unwrap(), second.poly[0]);
        let mid = ((e1.0 + s2.0) / 2.0, (e1.1 + s2.1) / 2.0);
        let mut poly = first.poly;
        poly.pop();
        poly.push(mid);
        poly.extend(second.poly.into_iter().skip(1));
        let merged = Seg { a: first.a, b: second.b, poly, wraw };
        let (hi, lo) = (i.max(j), i.min(j));
        segs.remove(hi);
        segs.remove(lo);
        segs.push(merged);
    }
}

// ---------- Full-image Centerline Mode (Trasea 1.2.0) ----------

/// Bounded Zhang-Suen thinning with L-corner cleanup (caps iterations so solid blocks never hang).
fn thin_bounded(mask: &[bool], w: usize, h: usize, max_iters: usize) -> Vec<bool> {
    if w < 3 || h < 3 {
        return vec![false; w * h];
    }
    let mut img: Vec<u8> = mask.iter().map(|&b| b as u8).collect();
    for x in 0..w {
        img[x] = 0;
        img[(h - 1) * w + x] = 0;
    }
    for y in 0..h {
        img[y * w] = 0;
        img[y * w + w - 1] = 0;
    }
    let mut to_clear: Vec<usize> = Vec::new();
    for _ in 0..max_iters {
        let mut changed = false;
        for step in 0..2 {
            to_clear.clear();
            for y in 1..h - 1 {
                for x in 1..w - 1 {
                    let i = y * w + x;
                    if img[i] == 0 {
                        continue;
                    }
                    let p = [
                        img[i - w],
                        img[i - w + 1],
                        img[i + 1],
                        img[i + w + 1],
                        img[i + w],
                        img[i + w - 1],
                        img[i - 1],
                        img[i - w - 1],
                    ];
                    let b: u8 = p.iter().sum();
                    if !(2..=6).contains(&b) {
                        continue;
                    }
                    let a = (0..8).filter(|&k| p[k] == 0 && p[(k + 1) % 8] == 1).count();
                    if a != 1 {
                        continue;
                    }
                    let ok = if step == 0 {
                        p[0] * p[2] * p[4] == 0 && p[2] * p[4] * p[6] == 0
                    } else {
                        p[0] * p[2] * p[6] == 0 && p[0] * p[4] * p[6] == 0
                    };
                    if ok {
                        to_clear.push(i);
                    }
                }
            }
            if !to_clear.is_empty() {
                changed = true;
                for &i in &to_clear {
                    img[i] = 0;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for y in 1..h - 1 {
        for x in 1..w - 1 {
            let i = y * w + x;
            if img[i] == 0 {
                continue;
            }
            let (n, ne, e, se, s, sw, ww, nw) = (
                img[i - w],
                img[i - w + 1],
                img[i + 1],
                img[i + w + 1],
                img[i + w],
                img[i + w - 1],
                img[i - 1],
                img[i - w - 1],
            );
            let redundant = (n == 1 && e == 1 && s == 0 && ww == 0 && sw == 0)
                || (e == 1 && s == 1 && ww == 0 && n == 0 && nw == 0)
                || (s == 1 && ww == 1 && n == 0 && e == 0 && ne == 0)
                || (ww == 1 && n == 1 && e == 0 && s == 0 && se == 0);
            if redundant {
                img[i] = 0;
            }
        }
    }
    img.iter().map(|&v| v == 1).collect()
}

/// Global Otsu threshold over luminance histogram (returns `(threshold, separation_contrast)`).
fn otsu_threshold(hist: &[u64; 256], total: u64) -> (u8, f64) {
    if total == 0 {
        return (128, 0.0);
    }
    let mut sum_all = 0.0f64;
    for (i, &c) in hist.iter().enumerate() {
        sum_all += i as f64 * c as f64;
    }
    let mut sum_bg = 0.0f64;
    let mut w_bg = 0u64;
    let mut best_var = -1.0f64;
    let mut best_t = 128u8;
    let mut best_diff = 0.0f64;
    for (t, &c) in hist.iter().enumerate() {
        w_bg += c;
        if w_bg == 0 {
            continue;
        }
        let w_fg = total - w_bg;
        if w_fg == 0 {
            break;
        }
        sum_bg += t as f64 * c as f64;
        let m_bg = sum_bg / w_bg as f64;
        let m_fg = (sum_all - sum_bg) / w_fg as f64;
        let diff = (m_fg - m_bg).abs();
        let var = (w_bg as f64) * (w_fg as f64) * diff * diff;
        if var > best_var {
            best_var = var;
            best_t = t as u8;
            best_diff = diff;
        }
    }
    (best_t, best_diff)
}

/// Trace pure closed degree-2 cycles in `skel` that `build_graph` skips (e.g. circles or O-rings).
fn extract_pure_cycles(skel: &[bool], w: usize, h: usize, used_in_edges: &[bool]) -> Vec<Vec<usize>> {
    let n = w * h;
    let mut visited = used_in_edges.to_vec();
    let mut cycles = Vec::new();
    for start in 0..n {
        if !skel[start] || visited[start] {
            continue;
        }
        let nbs = neighbours(start, w, skel);
        if nbs.len() != 2 {
            continue;
        }
        let mut pix = vec![start];
        visited[start] = true;
        let mut prev = start;
        let mut cur = nbs[0];
        while cur != start && !visited[cur] && pix.len() < n {
            visited[cur] = true;
            pix.push(cur);
            let next = neighbours(cur, w, skel).into_iter().find(|&q| q != prev && (!visited[q] || q == start));
            match next {
                Some(q) => {
                    prev = cur;
                    cur = q;
                }
                None => break,
            }
        }
        if pix.len() >= 4 {
            if cur == start {
                pix.push(start);
            }
            cycles.push(pix);
        }
    }
    cycles
}

/// Extract pure open centerline Bézier strokes across an entire image (laser engraving, pen plotter,
/// ink/pencil sketches). Produces 0 filled regions and never hangs on solid blocks.
pub fn extract_full_centerline(
    img: &image::RgbImage,
    fg_mask: Option<&[bool]>,
    invert: bool,
    stroke_pt: f64,
    scale: f64,
    smooth: f32,
) -> crate::export::StrokeLayer {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let n = w.saturating_mul(h);
    if n == 0 {
        return crate::export::StrokeLayer {
            color: [0, 0, 0],
            strokes: Vec::new(),
        };
    }

    // 1. Luminance + Otsu histogram + 2D integral image for 15x15 local mean
    let mut lum = vec![255u8; n];
    let mut hist = [0u64; 256];
    let mut valid_count = 0u64;
    for (idx, p) in img.pixels().enumerate() {
        let valid = fg_mask.map_or(true, |m| m.get(idx).copied().unwrap_or(true));
        if !valid {
            lum[idx] = 255;
            continue;
        }
        let y = (0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32)
            .round()
            .clamp(0.0, 255.0) as u8;
        let v = if invert { 255 - y } else { y };
        lum[idx] = v;
        hist[v as usize] += 1;
        valid_count += 1;
    }

    let (t_otsu, contrast) = otsu_threshold(&hist, valid_count);
    let iw = w + 1;
    let mut integ = vec![0u64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row_sum = 0u64;
        for x in 0..w {
            row_sum += lum[y * w + x] as u64;
            integ[(y + 1) * iw + (x + 1)] = integ[y * iw + (x + 1)] + row_sum;
        }
    }

    // 2. Conservative adaptive threshold (combines Otsu + 15x15 local mean)
    let half = 7isize;
    let mut ink_mask = vec![false; n];
    let mut sum_r = 0u64;
    let mut sum_g = 0u64;
    let mut sum_b = 0u64;
    let mut ink_cnt = 0u64;

    if contrast >= 12.0 {
        let global_cut = (t_otsu as f32).min(215.0);
        for y in 0..h {
            let y0 = (y as isize - half).max(0) as usize;
            let y1 = (y as isize + half).min(h as isize - 1) as usize;
            for x in 0..w {
                let idx = y * w + x;
                if fg_mask.map_or(false, |m| !m.get(idx).copied().unwrap_or(true)) {
                    continue;
                }
                let x0 = (x as isize - half).max(0) as usize;
                let x1 = (x as isize + half).min(w as isize - 1) as usize;
                let area = ((x1 - x0 + 1) * (y1 - y0 + 1)) as f32;
                let sum = integ[(y1 + 1) * iw + (x1 + 1)]
                    + integ[y0 * iw + x0]
                    - integ[(y1 + 1) * iw + x0]
                    - integ[y0 * iw + (x1 + 1)];
                let local_mean = sum as f32 / area;
                let v = lum[idx] as f32;
                let is_ink = v < 230.0
                    && (v < (local_mean - 12.0).min(global_cut + 14.0) || v <= global_cut - 6.0);
                if is_ink {
                    ink_mask[idx] = true;
                    let p = img.get_pixel(x as u32, y as u32).0;
                    sum_r += p[0] as u64;
                    sum_g += p[1] as u64;
                    sum_b += p[2] as u64;
                    ink_cnt += 1;
                }
            }
        }
    }

    let ink_color = if ink_cnt > 0 {
        [
            (sum_r / ink_cnt).min(255) as u8,
            (sum_g / ink_cnt).min(255) as u8,
            (sum_b / ink_cnt).min(255) as u8,
        ]
    } else {
        [0, 0, 0]
    };

    if ink_cnt < 2 {
        return crate::export::StrokeLayer {
            color: ink_color,
            strokes: Vec::new(),
        };
    }

    // 3. Thinning + skeleton graph + pruning + Bézier fitting
    let skel = thin_bounded(&ink_mask, w, h, 128);
    let (edges, nn) = build_graph(&skel, w, h);
    let spur = (4.0 * scale.max(1.0)).round().max(4.0) as usize;
    let edges = prune_and_merge(edges, nn, spur);

    let mut used = vec![false; n];
    for e in &edges {
        for &p in &e.pix {
            used[p] = true;
        }
    }
    let cycles = extract_pure_cycles(&skel, w, h, &used);

    let stroke_w_px = (stroke_pt.clamp(0.25, 12.0) * 96.0 / 72.0 * scale.max(0.5)).max(0.5);
    let t = smooth.clamp(0.0, 1.0) as f64;
    let tol = (0.7 + 1.8 * t) * scale.max(1.0);
    let sigma = 1.6 + 1.2 * t;

    let mut segs: Vec<Seg> = Vec::with_capacity(edges.len());
    for e in edges {
        let poly: Vec<P> = e.pix.iter().map(|&i| pt(i, w)).collect();
        if poly.len() >= 2 && poly_len(&poly) >= MICRO_FRAGMENT * scale.max(1.0) {
            segs.push(Seg {
                a: e.a,
                b: e.b,
                poly,
                wraw: stroke_w_px,
            });
        }
    }
    merge_through_junctions(&mut segs, stroke_w_px + 1.0);

    let mut strokes: Vec<crate::export::StrokeData> = Vec::new();
    for s in segs {
        let sm = smooth_open(&s.poly, sigma);
        let curves = fit_polyline(&sm, tol);
        if !curves.is_empty() {
            strokes.push(crate::export::StrokeData {
                curves,
                width: stroke_w_px,
            });
        }
    }
    for cyc in cycles {
        let poly: Vec<P> = cyc.iter().map(|&i| pt(i, w)).collect();
        if poly.len() >= 4 {
            let sm = smooth_open(&poly, sigma);
            let curves = fit_polyline(&sm, tol);
            if !curves.is_empty() {
                strokes.push(crate::export::StrokeData {
                    curves,
                    width: stroke_w_px,
                });
            }
        }
    }

    crate::export::StrokeLayer {
        color: ink_color,
        strokes,
    }
}
