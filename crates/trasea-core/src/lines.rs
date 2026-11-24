//! Thin-line guard. Finds dark strokes that are thinner than a few pixels in the ORIGINAL image (eyes, smiles, outlines),
//! because every smoothing/cleaning step after this one tends to erase them.
//!
//! mask   = dark pixels (clearly darker than their neighbourhood)
//! thin   = mask - opening(mask, disk r)        // structures thinner than ~2r+1 px
//! then small components are dropped, and the rest is traced at a higher resolution and stacked on top.

use crate::Rgb;
use image::RgbImage;

/// Largest RGB distance between a faded line pixel and the mean colour of the strong pixels of its line.
const WEAK_COLOR_DIST: f32 = 65.0;
/// A break is bridged only when the line goes on in a straight row of at least this many pixels on both sides.
const BRIDGE_RUN: i64 = 4;

#[derive(Debug, Clone)]
pub struct ThinParams {
    /// Radius of the opening disk: strokes up to ~2r+1 px wide count as "thin".
    pub open_radius: i32,
    /// Radius of the neighbourhood used for the adaptive (local) threshold.
    pub local_radius: i32,
    /// A pixel is dark when it is this much darker than the local mean (0..255 luminance).
    pub delta: f32,
    /// ...and darker than this absolute luminance.
    pub max_lum: f32,
    /// A line pixel must be darker than the pixels on both sides of it by at least this much.
    pub valley_delta: f32,
    /// Components smaller than this (pixels) are dropped.
    pub min_area: usize,
    /// Hysteresis: a weaker line pixel (this much darker than the local mean) is kept when it touches a strong one.
    /// 0 switches hysteresis off.
    pub weak_delta: f32,
    /// ...with this valley depth, and darker than this luminance.
    pub weak_valley: f32,
    pub weak_max_lum: f32,
    /// Gaps of up to this many pixels along a line (in a straight run) are bridged. 0 = off.
    pub bridge_gap: usize,
}

impl ThinParams {
    /// Defaults scaled to the image size (tuned on ~600-1600 px illustrations).
    pub fn for_size(w: u32, h: u32) -> Self {
        let m = w.min(h) as f32;
        ThinParams {
            open_radius: ((m / 200.0).round() as i32).clamp(2, 4),
            local_radius: ((m / 50.0).round() as i32).clamp(6, 24),
            delta: 22.0,
            max_lum: 175.0,
            valley_delta: 18.0,
            min_area: ((m / 20.0) as usize).max(15),
            weak_delta: 12.0,
            weak_valley: 9.0,
            weak_max_lum: 200.0,
            bridge_gap: 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub area: usize,
    pub color: Rgb,
}

#[derive(Debug, Clone)]
pub struct ThinMask {
    pub w: usize,
    pub h: usize,
    /// true = part of a kept thin line.
    pub mask: Vec<bool>,
    /// Component id (1-based) per pixel, 0 = none. Index into `comps` is id-1.
    pub labels: Vec<u32>,
    pub comps: Vec<Component>,
}

impl ThinMask {
    pub fn count(&self) -> usize {
        self.mask.iter().filter(|&&b| b).count()
    }
}

pub fn luminance(img: &RgbImage) -> Vec<f32> {
    img.pixels().map(|p| 0.299 * p[0] as f32 + 0.587 * p[1] as f32 + 0.114 * p[2] as f32).collect()
}

/// Local mean with a (2r+1) square window, via an integral image.
fn local_mean(lum: &[f32], w: usize, h: usize, r: i32) -> Vec<f32> {
    let mut integ = vec![0f64; (w + 1) * (h + 1)];
    for y in 0..h {
        let mut row = 0f64;
        for x in 0..w {
            row += lum[y * w + x] as f64;
            integ[(y + 1) * (w + 1) + x + 1] = integ[y * (w + 1) + x + 1] + row;
        }
    }
    let mut out = vec![0f32; w * h];
    for y in 0..h as i32 {
        let y0 = (y - r).max(0) as usize;
        let y1 = ((y + r + 1) as usize).min(h);
        for x in 0..w as i32 {
            let x0 = (x - r).max(0) as usize;
            let x1 = ((x + r + 1) as usize).min(w);
            let s = integ[y1 * (w + 1) + x1] - integ[y0 * (w + 1) + x1] - integ[y1 * (w + 1) + x0] + integ[y0 * (w + 1) + x0];
            out[y as usize * w + x as usize] = (s / ((y1 - y0) * (x1 - x0)) as f64) as f32;
        }
    }
    out
}

/// Row spans (dy, dx_max) of an isotropic disk of radius r (half-pixel outer margin so 45-deg diagonals match 0/90-deg width).
fn disk(r: i32) -> Vec<(i32, i32)> {
    let rf = r as f32 + 0.5;
    (-r..=r).map(|dy| (dy, ((rf * rf - (dy * dy) as f32).max(0.0).sqrt()).floor() as i32)).collect()
}

fn erode(src: &[bool], w: usize, h: usize, r: i32) -> Vec<bool> {
    let d = disk(r);
    let mut out = vec![false; w * h];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !src[y as usize * w + x as usize] {
                continue;
            }
            let mut ok = true;
            'o: for &(dy, dx) in &d {
                // outside the image counts as "set": a shape that touches the edge must not be eroded there
                let yy = y + dy;
                if yy < 0 || yy >= h as i32 {
                    continue;
                }
                for xx in (x - dx).max(0)..=(x + dx).min(w as i32 - 1) {
                    if !src[yy as usize * w + xx as usize] {
                        ok = false;
                        break 'o;
                    }
                }
            }
            out[y as usize * w + x as usize] = ok;
        }
    }
    out
}

fn dilate(src: &[bool], w: usize, h: usize, r: i32) -> Vec<bool> {
    let d = disk(r);
    let mut out = vec![false; w * h];
    for y in 0..h as i32 {
        for x in 0..w as i32 {
            if !src[y as usize * w + x as usize] {
                continue;
            }
            for &(dy, dx) in &d {
                let yy = y + dy;
                if yy < 0 || yy >= h as i32 {
                    continue;
                }
                for xx in (x - dx).max(0)..=(x + dx).min(w as i32 - 1) {
                    out[yy as usize * w + xx as usize] = true;
                }
            }
        }
    }
    out
}

/// Public dilation (used to keep line pixels out of the colour fields).
pub fn dilate_mask(src: &[bool], w: usize, h: usize, r: i32) -> Vec<bool> {
    if r <= 0 {
        return src.to_vec();
    }
    dilate(src, w, h, r)
}

/// Dark mask: clearly darker than the local mean and darker than `max_lum`.
pub fn dark_mask(img: &RgbImage, p: &ThinParams) -> Vec<bool> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let lum = luminance(img);
    let mean = local_mean(&lum, w, h, p.local_radius);
    (0..w * h).map(|i| lum[i] < mean[i] - p.delta && lum[i] < p.max_lum).collect()
}

/// Valley test: a real thin line is darker than the pixels on BOTH sides of it. The darker rim that JPEG ringing leaves
/// along the edge of a coloured shape is darker than one side only, so it fails this test.
pub fn valley_mask(img: &RgbImage, p: &ThinParams) -> Vec<bool> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let lum = luminance(img);
    let k = p.open_radius + 2;
    let need = p.valley_delta;
    let axes: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            axes.iter().any(|&(dx, dy)| {
                let at = |s: i32| {
                    let (xx, yy) = ((x + s * dx * k).clamp(0, w as i32 - 1), (y + s * dy * k).clamp(0, h as i32 - 1));
                    lum[yy as usize * w + xx as usize]
                };
                at(1) > lum[i] + need && at(-1) > lum[i] + need
            })
        })
        .collect()
}

/// 8-connected component labelling; returns (labels (1-based), areas).
fn label8(mask: &[bool], w: usize, h: usize) -> (Vec<u32>, Vec<usize>) {
    let mut labels = vec![0u32; w * h];
    let mut areas = Vec::new();
    let mut stack = Vec::new();
    for start in 0..w * h {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        let id = areas.len() as u32 + 1;
        let mut area = 0usize;
        labels[start] = id;
        stack.push(start);
        while let Some(i) = stack.pop() {
            area += 1;
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if mask[j] && labels[j] == 0 {
                        labels[j] = id;
                        stack.push(j);
                    }
                }
            }
        }
        areas.push(area);
    }
    (labels, areas)
}

/// Bridge short breaks inside a line. A break of 1..=`max_gap` unset pixels along one of the four axes is filled when
/// the line continues on both sides (two set pixels in a row each side). That joins collinear line ends and never
/// thickens a line, and it does not fuse two parallel lines (their pixels are not in a row along the bridge axis).
fn bridge_gaps(thin: &mut [bool], w: usize, h: usize, max_gap: usize) {
    if max_gap == 0 {
        return;
    }
    let src = thin.to_vec();
    let at = |x: i64, y: i64| -> bool { x >= 0 && y >= 0 && (x as usize) < w && (y as usize) < h && src[y as usize * w + x as usize] };
    let axes: [(i64, i64); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !src[y as usize * w + x as usize] {
                continue;
            }
            for &(dx, dy) in &axes {
                // x,y is the last pixel of a run of at least two along the axis
                if !(0..BRIDGE_RUN).all(|j| at(x - j * dx, y - j * dy)) || at(x + dx, y + dy) {
                    continue;
                }
                for l in 1..=max_gap as i64 {
                    let gap_clear = (1..=l).all(|k| !at(x + k * dx, y + k * dy));
                    if !gap_clear {
                        break;
                    }
                    if (1..=BRIDGE_RUN).all(|j| at(x + (l + j) * dx, y + (l + j) * dy)) {
                        for k in 1..=l {
                            let (gx, gy) = (x + k * dx, y + k * dy);
                            if gx >= 0 && gy >= 0 && (gx as usize) < w && (gy as usize) < h {
                                thin[gy as usize * w + gx as usize] = true;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    // Bridge 1-px knight's-move gaps on shallow/steep thin curves (e.g. slope 1:2 or 1:3 where scanlines
    // step by 2 px so 8-connectivity breaks). Require continuation away from the gap so parallel lines never fuse.
    for y in 0..h as i64 {
        for x in 0..w as i64 {
            if !at(x, y) {
                continue;
            }
            for sy in [-1i64, 1] {
                if at(x + 2, y + sy)
                    && !at(x + 1, y)
                    && !at(x + 1, y + sy)
                    && !at(x, y + sy)
                    && !at(x + 2, y)
                    && (at(x - 1, y) || at(x + 3, y + sy))
                {
                    thin[y as usize * w + (x + 1) as usize] = true;
                }
            }
            for sx in [-1i64, 1] {
                if at(x + sx, y + 2)
                    && !at(x, y + 1)
                    && !at(x + sx, y + 1)
                    && !at(x + sx, y)
                    && !at(x, y + 2)
                    && (at(x, y - 1) || at(x + sx, y + 3))
                {
                    thin[(y + 1) as usize * w + x as usize] = true;
                }
            }
        }
    }
}

/// Detect thin dark lines in `img`.
pub fn thin_mask(img: &RgbImage, p: &ThinParams) -> ThinMask {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let dark = dark_mask(img, p);
    let opened = dilate(&erode(&dark, w, h, p.open_radius), w, h, p.open_radius);
    let valley = valley_mask(img, p);
    let strong: Vec<bool> = (0..w * h).map(|i| dark[i] && valley[i] && !opened[i]).collect();
    let mut thin = strong.clone();

    // hysteresis: a thin line of medium contrast fades in and out along its length. Weak pixels that are connected to a
    // strong pixel belong to the same line, so keep them (a guard stops a weak mass from swallowing the strong seed).
    if p.weak_delta > 0.0 {
        let pw = ThinParams { delta: p.weak_delta, valley_delta: p.weak_valley, max_lum: p.weak_max_lum, ..p.clone() };
        let dark_w = dark_mask(img, &pw);
        let opened_w = dilate(&erode(&dark_w, w, h, p.open_radius), w, h, p.open_radius);
        let valley_w = valley_mask(img, &pw);
        let cand: Vec<bool> = (0..w * h).map(|i| strong[i] || (dark_w[i] && valley_w[i] && !opened_w[i])).collect();
        let (lab, areas) = label8(&cand, w, h);
        let mut strong_n = vec![0usize; areas.len() + 1];
        let mut strong_sum = vec![[0f32; 3]; areas.len() + 1];
        for i in 0..w * h {
            if strong[i] && lab[i] != 0 {
                let l = lab[i] as usize;
                strong_n[l] += 1;
                let px = img.get_pixel((i % w) as u32, (i / w) as u32);
                for c in 0..3 {
                    strong_sum[l][c] += px[c] as f32;
                }
            }
        }
        // a faded pixel of the same line is close in colour to the strong pixels of that line (a vein fades into the leaf);
        // a dark pixel of another colour (the red outline of a shape) is not a line and must stay a colour field
        let max_d2 = WEAK_COLOR_DIST * WEAK_COLOR_DIST;
        for i in 0..w * h {
            let l = lab[i] as usize;
            if l == 0 || !cand[i] || strong_n[l] == 0 || areas[l - 1] > 6 * strong_n[l] + 20 {
                continue;
            }
            if !strong[i] {
                let px = img.get_pixel((i % w) as u32, (i / w) as u32);
                let n = strong_n[l] as f32;
                let d2: f32 = (0..3).map(|c| (px[c] as f32 - strong_sum[l][c] / n).powi(2)).sum();
                if d2 > max_d2 {
                    continue;
                }
            }
            thin[i] = true;
        }

        // One-sided ink continuation (max 3 steps from current mask): keeps dark outline segments
        // that border a medium/dark colour fill on one side and a light background on the other,
        // where the two-sided valley check would otherwise break the outline.
        let lum = luminance(img);
        let mean = local_mean(&lum, w, h, p.local_radius);
        let k_val = p.open_radius + 2;
        let val_axes: [(i32, i32); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
        let one_sided_ok = |idx: usize, base: [f32; 3]| -> bool {
            let (x, y) = ((idx % w) as i32, (idx / w) as i32);
            let l0 = lum[idx];
            val_axes.iter().any(|&(dx, dy)| {
                let sample = |s: i32| {
                    let (xx, yy) = ((x + s * dx * k_val).clamp(0, w as i32 - 1), (y + s * dy * k_val).clamp(0, h as i32 - 1));
                    let si = yy as usize * w + xx as usize;
                    let c = img.get_pixel(xx as u32, yy as u32).0;
                    let cd2 = (c[0] as f32 - base[0]).powi(2) + (c[1] as f32 - base[1]).powi(2) + (c[2] as f32 - base[2]).powi(2);
                    (lum[si], cd2)
                };
                let (lp, dp2) = sample(1);
                let (ln, dn2) = sample(-1);
                let pos_bright = lp > l0 + p.weak_valley;
                let neg_bright = ln > l0 + p.weak_valley;
                (pos_bright ^ neg_bright) && dp2 > max_d2 && dn2 > max_d2
            })
        };

        let (seed_lab, seed_areas) = label8(&thin, w, h);
        let mut comp_sum = vec![[0f32; 3]; seed_areas.len() + 1];
        for i in 0..w * h {
            let l = seed_lab[i] as usize;
            if l != 0 {
                let px = img.get_pixel((i % w) as u32, (i / w) as u32).0;
                for c in 0..3 {
                    comp_sum[l][c] += px[c] as f32;
                }
            }
        }
        let mut comp_rgb = vec![[0f32; 3]; seed_areas.len() + 1];
        let mut comp_lum = vec![0f32; seed_areas.len() + 1];
        for (idx, &area) in seed_areas.iter().enumerate() {
            let id = idx + 1;
            let n = area.max(1) as f32;
            let rgb = [comp_sum[id][0] / n, comp_sum[id][1] / n, comp_sum[id][2] / n];
            comp_rgb[id] = rgb;
            comp_lum[id] = 0.299 * rgb[0] + 0.587 * rgb[1] + 0.114 * rgb[2];
        }

        let mut depth = vec![u8::MAX; w * h];
        let mut owner = seed_lab;
        let mut queue = std::collections::VecDeque::new();
        for i in 0..w * h {
            if thin[i] {
                depth[i] = 0;
                queue.push_back(i);
            }
        }
        let one_sided_delta = p.weak_delta * 0.75;
        while let Some(i) = queue.pop_front() {
            let d = depth[i];
            if d >= 3 {
                continue;
            }
            let cid = owner[i];
            let base = comp_rgb[cid as usize];
            let max_ink_lum = (comp_lum[cid as usize] + 35.0).min(p.weak_max_lum);
            let (x, y) = ((i % w) as i32, (i / w) as i32);
            for dy in -1..=1 {
                let ny = y + dy;
                if ny < 0 || ny >= h as i32 {
                    continue;
                }
                for dx in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let nx = x + dx;
                    if nx < 0 || nx >= w as i32 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if depth[j] != u8::MAX {
                        continue;
                    }
                    if lum[j] > max_ink_lum || (mean[j] - lum[j]) < one_sided_delta {
                        continue;
                    }
                    let px = img.get_pixel(nx as u32, ny as u32).0;
                    let cd2 = (px[0] as f32 - base[0]).powi(2) + (px[1] as f32 - base[1]).powi(2) + (px[2] as f32 - base[2]).powi(2);
                    if cd2 > max_d2 {
                        continue;
                    }
                    if !one_sided_ok(j, base) {
                        continue;
                    }
                    thin[j] = true;
                    depth[j] = d + 1;
                    owner[j] = cid;
                    queue.push_back(j);
                }
            }
        }

        // Color-verified 1-px closing: fill isolated 1-px breaks between opposite mask pixels
        // only when the original pixel is genuinely dark and matches the ink colour.
        if w >= 3 && h >= 3 {
            let snap = thin.clone();
            let close_d2 = (WEAK_COLOR_DIST + 15.0) * (WEAK_COLOR_DIST + 15.0);
            let opp_axes: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
            for y in 1..(h - 1) {
                let yi = y as isize;
                for x in 1..(w - 1) {
                    let xi = x as isize;
                    let i = y * w + x;
                    if snap[i] || opened_w[i] || lum[i] > p.weak_max_lum {
                        continue;
                    }
                    let px = img.get_pixel(x as u32, y as u32).0;
                    for &(dx, dy) in &opp_axes {
                        let i1 = (yi + dy) as usize * w + (xi + dx) as usize;
                        let i2 = (yi - dy) as usize * w + (xi - dx) as usize;
                        if snap[i1] && snap[i2] {
                            let c1 = img.get_pixel((i1 % w) as u32, (i1 / w) as u32).0;
                            let c2 = img.get_pixel((i2 % w) as u32, (i2 / w) as u32).0;
                            let avg = [
                                (c1[0] as f32 + c2[0] as f32) * 0.5,
                                (c1[1] as f32 + c2[1] as f32) * 0.5,
                                (c1[2] as f32 + c2[2] as f32) * 0.5,
                            ];
                            let d2 = (px[0] as f32 - avg[0]).powi(2)
                                + (px[1] as f32 - avg[1]).powi(2)
                                + (px[2] as f32 - avg[2]).powi(2);
                            if d2 <= close_d2 {
                                thin[i] = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
    }
    bridge_gaps(&mut thin, w, h, p.bridge_gap);
    let (labels0, areas) = label8(&thin, w, h);

    // keep big enough components, renumber, mean colour
    let mut remap = vec![0u32; areas.len() + 1];
    let mut comps = Vec::new();
    for (i, &a) in areas.iter().enumerate() {
        if a >= p.min_area {
            comps.push(Component { area: a, color: [0, 0, 0] });
            remap[i + 1] = comps.len() as u32;
        }
    }
    let mut sums = vec![[0u64; 3]; comps.len()];
    let mut labels = vec![0u32; w * h];
    let mut mask = vec![false; w * h];
    for i in 0..w * h {
        let l = remap[labels0[i] as usize];
        if l == 0 {
            continue;
        }
        labels[i] = l;
        mask[i] = true;
        let px = img.get_pixel((i % w) as u32, (i / w) as u32);
        for c in 0..3 {
            sums[l as usize - 1][c] += px[c] as u64;
        }
    }
    for (c, s) in comps.iter_mut().zip(&sums) {
        for k in 0..3 {
            c.color[k] = (s[k] / c.area as u64) as u8;
        }
    }
    ThinMask { w, h, mask, labels, comps }
}

/// Group components into at most `k` line colours (small k-means weighted by area).
/// Returns one colour per component (index into the returned palette) and the palette.
pub fn line_colors(tm: &ThinMask, k: usize) -> (Vec<usize>, Vec<Rgb>) {
    let n = tm.comps.len();
    if n == 0 {
        return (vec![], vec![]);
    }
    let k = k.min(n).max(1);
    // init: darkest component, then farthest-first
    let mut centers: Vec<[f32; 3]> = Vec::new();
    let lum = |c: &Rgb| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    let first = (0..n).min_by(|&a, &b| lum(&tm.comps[a].color).partial_cmp(&lum(&tm.comps[b].color)).unwrap()).unwrap();
    centers.push(tm.comps[first].color.map(|v| v as f32));
    let d2 = |a: &[f32; 3], b: &[f32; 3]| (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>();
    while centers.len() < k {
        let far = (0..n)
            .map(|i| {
                let c = tm.comps[i].color.map(|v| v as f32);
                (i, centers.iter().map(|m| d2(&c, m)).fold(f32::MAX, f32::min))
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .unwrap();
        if far.1 < 40.0 * 40.0 {
            break; // everything is already close to a centre
        }
        centers.push(tm.comps[far.0].color.map(|v| v as f32));
    }
    let mut assign = vec![0usize; n];
    for _ in 0..8 {
        for i in 0..n {
            let c = tm.comps[i].color.map(|v| v as f32);
            assign[i] = (0..centers.len()).min_by(|&a, &b| d2(&c, &centers[a]).partial_cmp(&d2(&c, &centers[b])).unwrap()).unwrap();
        }
        for (ci, ctr) in centers.iter_mut().enumerate() {
            let (mut s, mut wsum) = ([0f32; 3], 0f32);
            for i in (0..n).filter(|&i| assign[i] == ci) {
                let wgt = tm.comps[i].area as f32;
                for k in 0..3 {
                    s[k] += tm.comps[i].color[k] as f32 * wgt;
                }
                wsum += wgt;
            }
            if wsum > 0.0 {
                *ctr = [s[0] / wsum, s[1] / wsum, s[2] / wsum];
            }
        }
    }
    let pal = centers.iter().map(|c| [c[0].round() as u8, c[1].round() as u8, c[2].round() as u8]).collect();
    (assign, pal)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t170_thin_mask_continues_one_sided_outline_and_ignores_large_dark_block() {
        let (w, h) = (160u32, 120u32);
        let cream = image::Rgb([250, 245, 234]);
        let medium_red = image::Rgb([110, 12, 12]);
        let ink = image::Rgb([28, 36, 32]);
        let mut img = RgbImage::from_pixel(w, h, cream);

        // Medium-dark red fill patch touching the bottom of the outline for x in 60..66
        // so those outline pixels only have light background on one side (above).
        for y in 42..85 {
            for x in 60..66 {
                img.put_pixel(x, y, medium_red);
            }
        }

        // Continuous 2-px dark outline across y = 40..42, x = 20..120
        for y in 40..42 {
            for x in 20..120 {
                img.put_pixel(x, y, ink);
            }
        }

        // Large dark solid block (> 12 px wide) far from the thin line
        for y in 75..105 {
            for x in 105..140 {
                img.put_pixel(x, y, ink);
            }
        }

        let tm = thin_mask(&img, &ThinParams::for_size(w, h));

        // Every x along the outline (including the one-sided segment x = 60..66) must be present
        let mut covered_cols = 0usize;
        for x in 24..116 {
            let col_hit = (39..=42).any(|y| tm.mask[y as usize * w as usize + x as usize]);
            if col_hit {
                covered_cols += 1;
            }
        }
        assert_eq!(covered_cols, 92, "one-sided outline segment should stay continuous without gaps");

        // Interior of the large dark block (35x30) must NOT be absorbed into the thin mask
        let block_interior_hits = (80..100)
            .flat_map(|y| (110..135).map(move |x| (x, y)))
            .filter(|&(x, y)| tm.mask[y as usize * w as usize + x as usize])
            .count();
        assert_eq!(block_interior_hits, 0, "large dark solid block must not be absorbed as a thin line");
    }
}
