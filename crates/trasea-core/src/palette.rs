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
    remove_small_regions_ex(labels, w, h, min_area, n_labels, palette, boost, None, &[]);
}

/// Like `remove_small_regions`, with a brush zone map (a speck is judged 2.5x larger in a smooth zone and 0.4x in a detail
/// zone) and `protect`: palette indices the designer chose on purpose, whose regions are only dropped when tiny.
pub fn remove_small_regions_ex(labels: &mut [u8], w: usize, h: usize, min_area: usize, n_labels: usize, palette: &[Rgb], boost: f64, zones: Option<&[u8]>, protect: &[bool]) {
    let plab: Vec<[f32; 3]> = palette.iter().map(|&c| rgb_to_lab(c)).collect();
    let max_factor = if zones.is_some() { crate::brush::SMOOTH_SPECK } else { 1.0 };
    let wide = ((min_area as f64) * boost.max(1.0) * max_factor).ceil() as usize;
    let mut comp = vec![0u32; w * h];
    let mut stack = Vec::new();
    let mut regions: Vec<(usize, Vec<usize>)> = Vec::new(); // (label, pixels) for small ones
    // the largest region of every colour: a colour the designer kept on purpose keeps at least that one
    let mut max_comp = vec![0usize; n_labels.max(256)];
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
        max_comp[l as usize] = max_comp[l as usize].max(pix.len());
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
            let factor = if protect.get(l).copied().unwrap_or(false) && pix.len() >= max_comp[l] {
                0.1
            } else if let Some(z) = zones {
                let (mut nd, mut ns) = (0usize, 0usize);
                for &i in &pix {
                    match z[i] {
                        crate::brush::ZONE_DETAIL => nd += 1,
                        crate::brush::ZONE_SMOOTH => ns += 1,
                        _ => {}
                    }
                }
                if nd * 2 > pix.len() { crate::brush::DETAIL_SPECK } else if ns * 2 > pix.len() { crate::brush::SMOOTH_SPECK } else { 1.0 }
            } else {
                1.0
            };
            let limit = (min_area as f64) * factor * (1.0 + (boost.max(1.0) - 1.0) * t);
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

/// Thin light or dark features (a 2-3 px white line on dark blue) are blends of two colours at their edges, so their
/// pixels can land on a third palette colour (beige instead of white). For every small region, take the most extreme
/// pixels of the region in the direction away from its surroundings (the core of the stroke) and snap the region to the
/// palette colour nearest to that core.
pub fn refine_thin_regions(labels: &mut [u8], img: &RgbImage, palette: &[Rgb], w: usize, h: usize, max_area: usize) {
    let lum = |c: [u8; 3]| 0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32;
    let plum: Vec<f32> = palette.iter().map(|&c| lum(c)).collect();
    let plab: Vec<[f32; 3]> = palette.iter().map(|&c| rgb_to_lab(c)).collect();
    let mut comp = vec![0u32; w * h];
    let mut next = 0u32;
    let mut stack = Vec::new();
    let mut changes: Vec<(Vec<usize>, u8)> = Vec::new();
    for s in 0..w * h {
        if comp[s] != 0 {
            continue;
        }
        next += 1;
        let l = labels[s];
        let mut pix = vec![s];
        comp[s] = next;
        stack.push(s);
        let mut big = false;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            let mut nb = [usize::MAX; 4];
            if x > 0 { nb[0] = i - 1; }
            if x + 1 < w { nb[1] = i + 1; }
            if y > 0 { nb[2] = i - w; }
            if y + 1 < h { nb[3] = i + w; }
            for j in nb {
                if j != usize::MAX && comp[j] == 0 && labels[j] == l {
                    comp[j] = next;
                    stack.push(j);
                    pix.push(j);
                }
            }
            if pix.len() > max_area {
                big = true;
                // keep flood-filling so the component is marked, but stop collecting
                pix.truncate(1);
            }
        }
        if big || pix.len() < 6 {
            continue;
        }
        // surroundings: luminance of the neighbouring labels
        let mut surround = 0.0f32;
        let mut count = 0.0f32;
        for &i in &pix {
            let (x, y) = (i % w, i / w);
            for j in [if x > 0 { i - 1 } else { i }, if x + 1 < w { i + 1 } else { i }, if y > 0 { i - w } else { i }, if y + 1 < h { i + w } else { i }] {
                if labels[j] != l {
                    surround += plum[labels[j] as usize];
                    count += 1.0;
                }
            }
        }
        if count == 0.0 {
            continue;
        }
        let surround = surround / count;
        let own = plum[l as usize];
        let lighter = own > surround;
        // core = the 30% most extreme pixels away from the surroundings
        let mut px: Vec<(f32, [u8; 3])> = pix.iter().map(|&i| { let c = img.get_pixel((i % w) as u32, (i / w) as u32).0; (lum(c), c) }).collect();
        px.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
        let n = px.len();
        let take = (n * 3 / 10).max(1);
        let core = if lighter { &px[n - take..] } else { &px[..take] };
        let mut sum = [0u32; 3];
        for (_, c) in core {
            for k in 0..3 {
                sum[k] += c[k] as u32;
            }
        }
        let mean = [(sum[0] / take as u32) as u8, (sum[1] / take as u32) as u8, (sum[2] / take as u32) as u8];
        let ml = rgb_to_lab(mean);
        let best = (0..plab.len()).min_by(|&a, &b| d2(&ml, &plab[a]).partial_cmp(&d2(&ml, &plab[b])).unwrap()).unwrap() as u8;
        if best != l {
            changes.push((pix, best));
        }
    }
    for (pix, to) in changes {
        for i in pix {
            labels[i] = to;
        }
    }
}

/// Remove 1-2 px anti-aliasing halo bands along label borders.
///
/// When quantising a flat illustration, anti-aliased transition pixels between a dark stroke and a
/// light background can land on an intermediate palette colour (e.g. olive or tan). For every thin
/// border pixel (`same_8 <= 5`) sandwiched between two distinct labels `(a, b)` where `plab[l]`
/// lies near the Lab line segment `plab[a] -> plab[b]`, snap the pixel to whichever of `a` or `b`
/// is closer in Lab to the original image pixel. Runs 2 fast passes.
pub fn dehalo_label_borders(labels: &mut [u8], img: &RgbImage, palette: &[Rgb], w: usize, h: usize) {
    if w < 3 || h < 3 || palette.len() < 3 || labels.len() != w * h {
        return;
    }
    let plab: Vec<[f32; 3]> = palette.iter().map(|&c| rgb_to_lab(c)).collect();
    let axes: [(isize, isize); 4] = [(1, 0), (0, 1), (1, 1), (1, -1)];
    let mut next = labels.to_vec();

    for _pass in 0..2 {
        next.copy_from_slice(labels);
        let mut changed = false;

        for y in 1..(h - 1) {
            let yi = y as isize;
            for x in 1..(w - 1) {
                let xi = x as isize;
                let i = y * w + x;
                let l = labels[i];
                if (l as usize) >= plab.len() {
                    continue;
                }

                // Only thin border pixels (<= 5 same-label 8-neighbours), never solid colour interiors.
                let mut same8 = 0u8;
                for dy in -1isize..=1 {
                    let row = (yi + dy) as usize * w;
                    for dx in -1isize..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if labels[row + (xi + dx) as usize] == l {
                            same8 += 1;
                        }
                    }
                }
                if same8 > 5 {
                    continue;
                }

                let ll = plab[l as usize];
                let mut snapped: Option<u8> = None;

                for &(dx, dy) in &axes {
                    let p1 = labels[(yi + dy) as usize * w + (xi + dx) as usize];
                    let a = if p1 == l {
                        let x2 = xi + 2 * dx;
                        let y2 = yi + 2 * dy;
                        if x2 >= 0 && y2 >= 0 && (x2 as usize) < w && (y2 as usize) < h {
                            labels[y2 as usize * w + x2 as usize]
                        } else {
                            p1
                        }
                    } else {
                        p1
                    };

                    let m1 = labels[(yi - dy) as usize * w + (xi - dx) as usize];
                    let b = if m1 == l {
                        let x2 = xi - 2 * dx;
                        let y2 = yi - 2 * dy;
                        if x2 >= 0 && y2 >= 0 && (x2 as usize) < w && (y2 as usize) < h {
                            labels[y2 as usize * w + x2 as usize]
                        } else {
                            m1
                        }
                    } else {
                        m1
                    };

                    if a == l || b == l || a == b {
                        continue;
                    }
                    if (a as usize) >= plab.len() || (b as usize) >= plab.len() {
                        continue;
                    }

                    let la = plab[a as usize];
                    let lb = plab[b as usize];
                    let ab = [lb[0] - la[0], lb[1] - la[1], lb[2] - la[2]];
                    let ab_d2 = ab[0] * ab[0] + ab[1] * ab[1] + ab[2] * ab[2];
                    let ab_de = ab_d2.sqrt();
                    if ab_de < 18.0 {
                        continue;
                    }
                    let al = [ll[0] - la[0], ll[1] - la[1], ll[2] - la[2]];
                    let t = (al[0] * ab[0] + al[1] * ab[1] + al[2] * ab[2]) / ab_d2;
                    if !(0.12..=0.88).contains(&t) {
                        continue;
                    }
                    let proj = [la[0] + t * ab[0], la[1] + t * ab[1], la[2] + t * ab[2]];
                    let perp_de = d2(&ll, &proj).sqrt();
                    if perp_de <= 24.0 && perp_de < 0.35 * ab_de {
                        let orig = rgb_to_lab(img.get_pixel(x as u32, y as u32).0);
                        let da = d2(&orig, &la);
                        let db = d2(&orig, &lb);
                        snapped = Some(if da <= db { a } else { b });
                        break;
                    }
                }

                if let Some(to) = snapped {
                    next[i] = to;
                    changed = true;
                }
            }
        }

        if !changed {
            break;
        }
        labels.copy_from_slice(&next);
    }
}

/// Remove 1-px staircases/protrusions along label boundaries in a single pass.
///
/// Only pixels with at most 1 matching 4-neighbour (`same_4 <= 1`) where a single other label
/// occupies at least 3 of the 4 orthogonal neighbours are reassigned. Protected labels, 90-degree
/// corners (`same_4 == 2`), and 1-2 px straight strokes (`same_4 == 2`) remain untouched.
pub fn smooth_label_jaggies(labels: &mut [u8], w: usize, h: usize, protect: &[bool]) {
    if w < 3 || h < 3 || labels.len() != w * h {
        return;
    }
    let src = labels.to_vec();
    for y in 1..(h - 1) {
        let row = y * w;
        for x in 1..(w - 1) {
            let i = row + x;
            let l = src[i];
            if protect.get(l as usize).copied().unwrap_or(false) {
                continue;
            }
            let nb = [src[i - 1], src[i + 1], src[i - w], src[i + w]];
            let same4 = nb.iter().filter(|&&n| n == l).count();
            if same4 > 1 {
                continue;
            }
            for &cand in &[nb[0], nb[1]] {
                if cand != l && nb.iter().filter(|&&n| n == cand).count() >= 3 {
                    labels[i] = cand;
                    break;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t170_dehalo_removes_aa_halo_and_preserves_solid_olive_block() {
        // Palette: 0 = dark green (#1F4D36), 1 = cream (#FCF7EC), 2 = olive (#89995A)
        let dark: Rgb = [0x1F, 0x4D, 0x36];
        let cream: Rgb = [0xFC, 0xF7, 0xEC];
        let olive: Rgb = [0x89, 0x99, 0x5A];
        let palette = [dark, cream, olive];
        let (w, h) = (64usize, 40usize);
        let mut img = RgbImage::from_pixel(w as u32, h as u32, image::Rgb(cream));
        let mut labels = vec![1u8; w * h];

        // Left dark block x in 4..20, y in 4..36
        for y in 4..36 {
            for x in 4..20 {
                img.put_pixel(x as u32, y as u32, image::Rgb(dark));
                labels[y * w + x] = 0;
            }
            // 2-px anti-alias halo band at x = 20, 21 labelled olive (2)
            // x=20 is darker blend, x=21 is lighter blend
            img.put_pixel(20, y as u32, image::Rgb([0x40, 0x6A, 0x52]));
            labels[y * w + 20] = 2;
            img.put_pixel(21, y as u32, image::Rgb([0xD4, 0xD8, 0xC6]));
            labels[y * w + 21] = 2;
        }

        // Genuine wide olive block x in 36..58, y in 6..34
        for y in 6..34 {
            for x in 36..58 {
                img.put_pixel(x as u32, y as u32, image::Rgb(olive));
                labels[y * w + x] = 2;
            }
        }

        let halo_before = (5..35)
            .flat_map(|y| [labels[y * w + 20], labels[y * w + 21]])
            .filter(|&l| l == 2)
            .count();
        assert_eq!(halo_before, 60);

        dehalo_label_borders(&mut labels, &img, &palette, w, h);

        let halo_after = (5..35)
            .flat_map(|y| [labels[y * w + 20], labels[y * w + 21]])
            .filter(|&l| l == 2)
            .count();
        assert_eq!(halo_after, 0, "expected 2-px halo band to be fully removed");
        // Verify dark side snapped to dark (0) and light side snapped to cream (1)
        assert_eq!(labels[20 * w + 20], 0);
        assert_eq!(labels[20 * w + 21], 1);

        // Genuine solid olive block must remain 100% intact
        let olive_kept = (6..34)
            .flat_map(|y| (36..58).map(move |x| (x, y)))
            .filter(|&(x, y)| labels[y * w + x] == 2)
            .count();
        assert_eq!(olive_kept, 28 * 22, "genuine solid olive block must remain untouched");
    }

    #[test]
    fn t170_smooth_label_jaggies_flattens_protrusion_and_keeps_corners_and_lines() {
        let (w, h) = (32usize, 32usize);
        let mut labels = vec![0u8; w * h];

        // Rectangle of label 1 at x in 4..14, y in 4..14 (sharp 90-degree corners)
        for y in 4..14 {
            for x in 4..14 {
                labels[y * w + x] = 1;
            }
        }
        // Single 1-px jagged protrusion on its right edge at (14, 9)
        labels[9 * w + 14] = 1;
        // Single 1-px notch inside its bottom edge at (9, 13)
        labels[13 * w + 9] = 0;

        // Straight 2-px wide horizontal line of label 2 at x in 4..26, y in 22..24
        for y in 22..24 {
            for x in 4..26 {
                labels[y * w + x] = 2;
            }
        }

        smooth_label_jaggies(&mut labels, w, h, &[false, false, false]);

        // Protrusion flattened and notch filled
        assert_eq!(labels[9 * w + 14], 0, "1-px protrusion should be removed");
        assert_eq!(labels[13 * w + 9], 1, "1-px notch should be filled");

        // All four 90-degree corners of the rectangle remain intact
        assert_eq!(labels[4 * w + 4], 1);
        assert_eq!(labels[4 * w + 13], 1);
        assert_eq!(labels[13 * w + 4], 1);
        assert_eq!(labels[13 * w + 13], 1);

        // 2-px straight line remains 100% intact
        let line_kept = (22..24)
            .flat_map(|y| (4..26).map(move |x| (x, y)))
            .filter(|&(x, y)| labels[y * w + x] == 2)
            .count();
        assert_eq!(line_kept, 2 * 22, "2-px line must remain untouched");
    }
}
