//! Thin-line guard. Finds dark strokes that are thinner than a few pixels in the ORIGINAL image (eyes, smiles, outlines),
//! because every smoothing/cleaning step after this one tends to erase them.
//!
//! mask   = dark pixels (clearly darker than their neighbourhood)
//! thin   = mask - opening(mask, disk r)        // structures thinner than ~2r+1 px
//! then small components are dropped, and the rest is traced at a higher resolution and stacked on top.

use crate::Rgb;
use image::RgbImage;

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

/// Row spans (dy, dx_max) of a disk of radius r.
fn disk(r: i32) -> Vec<(i32, i32)> {
    (-r..=r).map(|dy| (dy, (((r * r - dy * dy) as f32).sqrt()).floor() as i32)).collect()
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

/// Detect thin dark lines in `img`.
pub fn thin_mask(img: &RgbImage, p: &ThinParams) -> ThinMask {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let dark = dark_mask(img, p);
    let opened = dilate(&erode(&dark, w, h, p.open_radius), w, h, p.open_radius);
    let valley = valley_mask(img, p);
    let thin: Vec<bool> = (0..w * h).map(|i| dark[i] && valley[i] && !opened[i]).collect();
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
