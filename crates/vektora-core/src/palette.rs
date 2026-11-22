//! Palette lock: k-means in Lab, every pixel snapped to the palette (no in-between colours = no halo).

use crate::Rgb;
use image::RgbImage;

#[inline]
fn srgb_to_lin(c: u8) -> f32 {
    let v = c as f32 / 255.0;
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn rgb_to_lab(c: Rgb) -> [f32; 3] {
    let (r, g, b) = (srgb_to_lin(c[0]), srgb_to_lin(c[1]), srgb_to_lin(c[2]));
    let x = (0.4124564 * r + 0.3575761 * g + 0.1804375 * b) / 0.95047;
    let y = 0.2126729 * r + 0.7151522 * g + 0.0721750 * b;
    let z = (0.0193339 * r + 0.119192 * g + 0.9503041 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

#[inline]
fn d2(a: &[f32; 3], b: &[f32; 3]) -> f32 {
    let (x, y, z) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    x * x + y * y + z * z
}

/// Tiny deterministic RNG (so the same input always gives the same palette).
struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((self.0 >> 33) as f32) / (1u64 << 31) as f32
    }
}

/// A pixel is "flat" when the pixels 2 px away in four directions have almost the same colour. Edge pixels, which are
/// blends of two colours, are left out of the palette estimate so no halo colours are invented.
fn is_flat(img: &RgbImage, x: i32, y: i32) -> bool {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let c = img.get_pixel(x as u32, y as u32).0;
    for (dx, dy) in [(-2, 0), (2, 0), (0, -2), (0, 2)] {
        let (nx, ny) = ((x + dx).clamp(0, w - 1), (y + dy).clamp(0, h - 1));
        let n = img.get_pixel(nx as u32, ny as u32).0;
        let d = (c[0] as i32 - n[0] as i32).abs() + (c[1] as i32 - n[1] as i32).abs() + (c[2] as i32 - n[2] as i32).abs();
        if d > 30 {
            return false;
        }
    }
    true
}

/// k-means (k-means++ init) over a sample of the pixels that are not `excluded`.
/// Returns centres as RGB (refined to the mean RGB of the pixels they own).
pub fn kmeans_palette(img: &RgbImage, excluded: Option<&[bool]>, k: usize) -> Vec<Rgb> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let k = k.clamp(1, 32);
    // sample (stride) up to ~60k pixels
    let total = w * h;
    let stride = (total / 60_000).max(1);
    let mut lab: Vec<[f32; 3]> = Vec::new();
    let mut rgb: Vec<Rgb> = Vec::new();
    let mut i = 0usize;
    while i < total {
        if excluded.map_or(true, |e| !e[i]) && is_flat(img, (i % w) as i32, (i / w) as i32) {
            let p = img.get_pixel((i % w) as u32, (i / w) as u32).0;
            rgb.push(p);
            lab.push(rgb_to_lab(p));
        }
        i += stride;
    }
    if lab.is_empty() {
        // no flat pixel at all (a photo?): fall back to every pixel
        let mut i = 0usize;
        while i < total {
            let p = img.get_pixel((i % w) as u32, (i / w) as u32).0;
            rgb.push(p);
            lab.push(rgb_to_lab(p));
            i += stride;
        }
    }
    // dedupe identical colours cheaply by weighting through a hash of quantised Lab? Keep simple: sample as is.
    let mut rng = Lcg(0x9E3779B97F4A7C15);
    let mut centers: Vec<[f32; 3]> = vec![lab[(rng.next() * lab.len() as f32) as usize % lab.len()]];
    let mut dist: Vec<f32> = lab.iter().map(|p| d2(p, &centers[0])).collect();
    while centers.len() < k {
        let sum: f32 = dist.iter().sum();
        if sum <= 1e-3 {
            break;
        }
        let mut t = rng.next() * sum;
        let mut pick = 0;
        for (j, &d) in dist.iter().enumerate() {
            t -= d;
            if t <= 0.0 {
                pick = j;
                break;
            }
        }
        centers.push(lab[pick]);
        for (j, p) in lab.iter().enumerate() {
            dist[j] = dist[j].min(d2(p, centers.last().unwrap()));
        }
    }
    let mut assign = vec![0usize; lab.len()];
    for _ in 0..16 {
        let mut changed = false;
        for (j, p) in lab.iter().enumerate() {
            let best = (0..centers.len()).min_by(|&a, &b| d2(p, &centers[a]).partial_cmp(&d2(p, &centers[b])).unwrap()).unwrap();
            if best != assign[j] {
                assign[j] = best;
                changed = true;
            }
        }
        let mut sums = vec![([0f32; 3], 0f32); centers.len()];
        for (j, p) in lab.iter().enumerate() {
            let s = &mut sums[assign[j]];
            for c in 0..3 {
                s.0[c] += p[c];
            }
            s.1 += 1.0;
        }
        for (c, s) in centers.iter_mut().zip(&sums) {
            if s.1 > 0.0 {
                *c = [s.0[0] / s.1, s.0[1] / s.1, s.0[2] / s.1];
            }
        }
        if !changed {
            break;
        }
    }
    // merge centres that are almost the same colour (JPEG noise splits one colour in two)
    let mut merged = true;
    while merged && centers.len() > 1 {
        merged = false;
        'm: for a in 0..centers.len() {
            for b in (a + 1)..centers.len() {
                if d2(&centers[a], &centers[b]) < 12.0 * 12.0 {
                    for t in assign.iter_mut() {
                        if *t == b {
                            *t = a;
                        } else if *t > b {
                            *t -= 1;
                        }
                    }
                    centers.remove(b);
                    merged = true;
                    break 'm;
                }
            }
        }
    }
    // final colours = mean RGB of the owned pixels (exact, not a Lab round trip)
    let mut acc = vec![([0u64; 3], 0u64); centers.len()];
    for (j, c) in rgb.iter().enumerate() {
        let a = &mut acc[assign[j]];
        for t in 0..3 {
            a.0[t] += c[t] as u64;
        }
        a.1 += 1;
    }
    let mut out: Vec<Rgb> = acc.iter().filter(|a| a.1 > 0).map(|a| [(a.0[0] / a.1) as u8, (a.0[1] / a.1) as u8, (a.0[2] / a.1) as u8]).collect();
    out.sort();
    out.dedup();
    out
}

/// Nearest palette index for every pixel (palette distance in Lab).
pub fn assign(img: &RgbImage, pal: &[Rgb]) -> Vec<u8> {
    let plab: Vec<[f32; 3]> = pal.iter().map(|&c| rgb_to_lab(c)).collect();
    let mut cache: std::collections::HashMap<[u8; 3], u8> = std::collections::HashMap::new();
    img.pixels()
        .map(|p| {
            *cache.entry(p.0).or_insert_with(|| {
                let l = rgb_to_lab(p.0);
                (0..plab.len()).min_by(|&a, &b| d2(&l, &plab[a]).partial_cmp(&d2(&l, &plab[b])).unwrap()).unwrap() as u8
            })
        })
        .collect()
}

/// Reassign connected same-label regions smaller than `min_area` to their most common neighbouring label.
/// Small regions that differ little from their surroundings (JPEG residue, watermark ghosts) are removed up to
/// `boost` times `min_area`; clearly different ones (real details) only below `min_area`.
pub fn remove_small_regions(labels: &mut [u8], w: usize, h: usize, min_area: usize, n_labels: usize, palette: &[Rgb], boost: f64) {
    let plab: Vec<[f32; 3]> = palette.iter().map(|&c| rgb_to_lab(c)).collect();
    let wide = ((min_area as f64) * boost.max(1.0)).ceil() as usize;
    let mut comp = vec![0u32; w * h];
    let mut stack = Vec::new();
    let mut regions: Vec<(usize, Vec<usize>)> = Vec::new(); // (label, pixels) for small ones
    let mut next = 0u32;
    for s in 0..w * h {
        if comp[s] != 0 {
            continue;
        }
        next += 1;
        let l = labels[s];
        let mut pix = vec![s];
        comp[s] = next;
        stack.push(s);
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut push = |j: usize| {
                if comp[j] == 0 && labels[j] == l {
                    comp[j] = next;
                    stack.push(j);
                    pix.push(j);
                }
            };
            if x > 0 {
                push(i - 1);
            }
            if x + 1 < w {
                push(i + 1);
            }
            if y > 0 {
                push(i - w);
            }
            if y + 1 < h {
                push(i + w);
            }
        }
        if pix.len() < wide {
            regions.push((l as usize, pix));
        }
    }
    for (l, pix) in regions {
        let mut votes = vec![0u32; n_labels];
        for &i in &pix {
            let (x, y) = (i % w, i / w);
            let mut v = |j: usize| {
                let lj = labels[j] as usize;
                if lj != l {
                    votes[lj] += 1;
                }
            };
            if x > 0 {
                v(i - 1);
            }
            if x + 1 < w {
                v(i + 1);
            }
            if y > 0 {
                v(i - w);
            }
            if y + 1 < h {
                v(i + w);
            }
        }
        if let Some((best, &cnt)) = votes.iter().enumerate().max_by_key(|&(_, &c)| c) {
            // the less the region differs from what surrounds it, the larger it may be and still be merged away
            let de = (d2(&plab[l], &plab[best])).sqrt() as f64;
            let t = ((30.0 - de) / 30.0).clamp(0.0, 1.0);
            let limit = (min_area as f64) * (1.0 + (boost.max(1.0) - 1.0) * t);
            if (pix.len() as f64) >= limit {
                continue;
            }
            if cnt > 0 {
                for &i in &pix {
                    labels[i] = best as u8;
                }
            }
        }
    }
}

/// Fill the pixels under `mask` (e.g. dilated thin lines) from the nearest unmasked pixel, so colour fields run
/// continuously underneath the lines.
pub fn fill_masked(labels: &mut [u8], mask: &[bool], w: usize, h: usize) {
    let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
    let mut done: Vec<bool> = mask.iter().map(|&m| !m).collect();
    for i in 0..w * h {
        if done[i] {
            let (x, y) = (i % w, i / w);
            let near_masked = (x > 0 && !done[i - 1]) || (x + 1 < w && !done[i + 1]) || (y > 0 && !done[i - w]) || (y + 1 < h && !done[i + w]);
            if near_masked {
                queue.push_back(i);
            }
        }
    }
    while let Some(i) = queue.pop_front() {
        let (x, y) = (i % w, i / w);
        let mut visit = |j: usize, q: &mut std::collections::VecDeque<usize>, labels: &mut [u8], done: &mut [bool]| {
            if !done[j] {
                labels[j] = labels[i];
                done[j] = true;
                q.push_back(j);
            }
        };
        if x > 0 {
            visit(i - 1, &mut queue, labels, &mut done);
        }
        if x + 1 < w {
            visit(i + 1, &mut queue, labels, &mut done);
        }
        if y > 0 {
            visit(i - w, &mut queue, labels, &mut done);
        }
        if y + 1 < h {
            visit(i + w, &mut queue, labels, &mut done);
        }
    }
}
