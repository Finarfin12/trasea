//! Non-destructive image pre-processing (rotation, crop, and 1-click border background removal + defringe)
//! before tracing.
//!
//! - Exact 90°/180°/270° rotations use lossless pixel transpositions.
//! - Free-angle rotations use bilinear interpolation on an expanded canvas so corners are never
//!   clipped; exterior corners are filled with transparent (when the image has border transparency)
//!   or the dominant border colour.
//! - Crop coordinates are safely clamped to the rotated image bounds.
//! - `remove_background` uses 4-connected CIE Lab ΔE border flood-fill + optional halo defringe
//!   (0..=3 px) so closed interior areas matching the background colour (e.g. white eyes/shirts)
//!   stay completely intact.

use crate::palette::rgb_to_lab;
use crate::Rgb as CoreRgb;
use image::{imageops, Rgb, RgbImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

fn default_bg_tolerance() -> u8 {
    24
}

fn default_defringe_px() -> u8 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreprocessOptions {
    /// Optional crop rectangle `[x, y, w, h]` in pixels (applied after rotation).
    #[serde(default)]
    pub crop: Option<[u32; 4]>,
    /// Clockwise rotation in degrees (`-180.0..=180.0`).
    #[serde(default)]
    pub rotate_deg: f32,
    /// Enable automatic border flood-fill background removal (1-click, non-destructive).
    #[serde(default)]
    pub remove_background: bool,
    /// CIE Lab ΔE colour tolerance for background flood-fill (`0..=100`, default `24`).
    #[serde(default = "default_bg_tolerance")]
    pub bg_tolerance: u8,
    /// Edge halo erosion layers in pixels (`0..=3`, default `1`).
    #[serde(default = "default_defringe_px")]
    pub defringe_px: u8,
}

impl Default for PreprocessOptions {
    fn default() -> Self {
        Self {
            crop: None,
            rotate_deg: 0.0,
            remove_background: false,
            bg_tolerance: default_bg_tolerance(),
            defringe_px: default_defringe_px(),
        }
    }
}

impl PreprocessOptions {
    pub fn is_identity(&self) -> bool {
        !self.remove_background
            && self.crop.is_none()
            && (!self.rotate_deg.is_finite() || self.rotate_deg.abs() < 1e-4)
    }
}

#[derive(Debug, Clone)]
pub struct RemoveBgReport {
    pub bg_rgb: CoreRgb,
    pub removed_pixels: usize,
    pub total_pixels: usize,
    pub removed_fraction: f32,
    pub warning: Option<String>,
}

/// Normalize angle to `(-180.0, 180.0]`.
fn normalize_deg(deg: f32) -> f32 {
    if !deg.is_finite() {
        return 0.0;
    }
    let mut a = deg % 360.0;
    if a > 180.0 {
        a -= 360.0;
    } else if a <= -180.0 {
        a += 360.0;
    }
    a
}

/// Estimate the dominant border colour of an RGBA image (or transparent if borders are transparent).
fn border_fill_rgba(img: &RgbaImage) -> Rgba<u8> {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return Rgba([255, 255, 255, 255]);
    }
    let mut rs = Vec::new();
    let mut gs = Vec::new();
    let mut bs = Vec::new();
    let mut transparent = 0usize;
    let mut total = 0usize;
    let mut sample = |p: Rgba<u8>| {
        total += 1;
        if p[3] < 250 {
            transparent += 1;
        } else {
            rs.push(p[0]);
            gs.push(p[1]);
            bs.push(p[2]);
        }
    };
    let step_x = (w / 64).max(1);
    let step_y = (h / 64).max(1);
    for x in (0..w).step_by(step_x as usize) {
        sample(*img.get_pixel(x, 0));
        sample(*img.get_pixel(x, h - 1));
    }
    for y in (0..h).step_by(step_y as usize) {
        sample(*img.get_pixel(0, y));
        sample(*img.get_pixel(w - 1, y));
    }
    if total > 0 && transparent * 4 >= total {
        return Rgba([0, 0, 0, 0]);
    }
    if rs.is_empty() {
        return Rgba([255, 255, 255, 255]);
    }
    rs.sort_unstable();
    gs.sort_unstable();
    bs.sort_unstable();
    let m = rs.len() / 2;
    Rgba([rs[m], gs[m], bs[m], 255])
}

/// Estimate the dominant opaque border colour in CIE Lab space (densest 8-unit Lab cluster).
fn dominant_border_lab(img: &RgbaImage) -> (CoreRgb, [f32; 3]) {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        let rgb = [255, 255, 255];
        return (rgb, rgb_to_lab(rgb));
    }
    let step_x = (w / 128).max(1);
    let step_y = (h / 128).max(1);
    // Bucket key -> (count, sum_r, sum_g, sum_b, sum_l, sum_a, sum_b_lab)
    let mut buckets: HashMap<(i32, i32, i32), (usize, u64, u64, u64, f64, f64, f64)> = HashMap::new();
    let mut add = |p: Rgba<u8>| {
        if p[3] < 128 {
            return;
        }
        let rgb = [p[0], p[1], p[2]];
        let lab = rgb_to_lab(rgb);
        let key = (
            (lab[0] / 8.0).round() as i32,
            (lab[1] / 8.0).round() as i32,
            (lab[2] / 8.0).round() as i32,
        );
        let entry = buckets.entry(key).or_insert((0, 0, 0, 0, 0.0, 0.0, 0.0));
        entry.0 += 1;
        entry.1 += rgb[0] as u64;
        entry.2 += rgb[1] as u64;
        entry.3 += rgb[2] as u64;
        entry.4 += lab[0] as f64;
        entry.5 += lab[1] as f64;
        entry.6 += lab[2] as f64;
    };
    for x in (0..w).step_by(step_x as usize) {
        add(*img.get_pixel(x, 0));
        add(*img.get_pixel(x, h - 1));
    }
    for y in (0..h).step_by(step_y as usize) {
        add(*img.get_pixel(0, y));
        add(*img.get_pixel(w - 1, y));
    }

    if let Some((_, &(cnt, sr, sg, sb, sl, sa, sb_l))) = buckets.iter().max_by_key(|(_, v)| v.0) {
        let c = cnt.max(1) as f64;
        let rgb = [
            (sr as f64 / c).round().clamp(0.0, 255.0) as u8,
            (sg as f64 / c).round().clamp(0.0, 255.0) as u8,
            (sb as f64 / c).round().clamp(0.0, 255.0) as u8,
        ];
        return (rgb, [(sl / c) as f32, (sa / c) as f32, (sb_l / c) as f32]);
    }
    let rgb = [255, 255, 255];
    (rgb, rgb_to_lab(rgb))
}

#[inline]
fn lab_dist_sq(a: [f32; 3], b: [f32; 3]) -> f32 {
    let dl = a[0] - b[0];
    let da = a[1] - b[1];
    let db = a[2] - b[2];
    dl * dl + da * da + db * db
}

/// Remove outer background via 4-connected CIE Lab ΔE flood-fill from all 4 borders, followed by
/// `defringe_px` (0..=3) halo erosion passes on foreground pixels adjacent to the removed mask.
/// Interior closed regions matching the background colour are never touched.
pub fn remove_background_rgba(
    img: &RgbaImage,
    bg_tolerance: u8,
    defringe_px: u8,
) -> (RgbaImage, RemoveBgReport) {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let total = w.saturating_mul(h);
    if total == 0 {
        return (
            img.clone(),
            RemoveBgReport {
                bg_rgb: [255, 255, 255],
                removed_pixels: 0,
                total_pixels: 0,
                removed_fraction: 0.0,
                warning: None,
            },
        );
    }

    let tol = (bg_tolerance.min(100)) as f32;
    let tol_sq = tol * tol;
    let defringe_tol = tol * 1.45;
    let defringe_tol_sq = defringe_tol * defringe_tol;
    let (bg_rgb, bg_lab) = dominant_border_lab(img);

    let mut labs: Vec<[f32; 3]> = Vec::with_capacity(total);
    let mut alphas: Vec<u8> = Vec::with_capacity(total);
    for p in img.pixels() {
        labs.push(rgb_to_lab([p[0], p[1], p[2]]));
        alphas.push(p[3]);
    }

    let mut is_bg = vec![false; total];
    let mut queue = VecDeque::new();

    let try_seed = |idx: usize, is_bg: &mut [bool], queue: &mut VecDeque<usize>| {
        if is_bg[idx] {
            return;
        }
        if alphas[idx] < 128 || lab_dist_sq(labs[idx], bg_lab) <= tol_sq {
            is_bg[idx] = true;
            queue.push_back(idx);
        }
    };

    for x in 0..w {
        try_seed(x, &mut is_bg, &mut queue);
        try_seed((h - 1) * w + x, &mut is_bg, &mut queue);
    }
    for y in 1..h.saturating_sub(1) {
        try_seed(y * w, &mut is_bg, &mut queue);
        try_seed(y * w + (w - 1), &mut is_bg, &mut queue);
    }

    // 4-connected BFS flood-fill from border seeds.
    while let Some(idx) = queue.pop_front() {
        let x = idx % w;
        let y = idx / w;
        if x > 0 {
            try_seed(idx - 1, &mut is_bg, &mut queue);
        }
        if x + 1 < w {
            try_seed(idx + 1, &mut is_bg, &mut queue);
        }
        if y > 0 {
            try_seed(idx - w, &mut is_bg, &mut queue);
        }
        if y + 1 < h {
            try_seed(idx + w, &mut is_bg, &mut queue);
        }
    }

    // Defringe: strip up to `defringe_px` (0..=3) halo pixels adjacent to removed background.
    let passes = defringe_px.min(3);
    let mut to_erode: Vec<usize> = Vec::new();
    for _ in 0..passes {
        to_erode.clear();
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let idx = row + x;
                if is_bg[idx] {
                    continue;
                }
                let adj_bg = (x > 0 && is_bg[idx - 1])
                    || (x + 1 < w && is_bg[idx + 1])
                    || (y > 0 && is_bg[idx - w])
                    || (y + 1 < h && is_bg[idx + w]);
                if adj_bg && lab_dist_sq(labs[idx], bg_lab) <= defringe_tol_sq {
                    to_erode.push(idx);
                }
            }
        }
        if to_erode.is_empty() {
            break;
        }
        for &idx in &to_erode {
            is_bg[idx] = true;
        }
    }

    let mut out = img.clone();
    let mut removed = 0usize;
    for (idx, p) in out.pixels_mut().enumerate() {
        if is_bg[idx] {
            *p = Rgba([0, 0, 0, 0]);
            removed += 1;
        }
    }

    let frac = removed as f32 / total as f32;
    let warning = if frac > 0.92 {
        Some(format!(
            "Hapus Latar menyapu {:.0}% piksel (>92%). Turunkan toleransi latar agar objek tidak ikut hilang.",
            frac * 100.0
        ))
    } else {
        None
    };

    (
        out,
        RemoveBgReport {
            bg_rgb,
            removed_pixels: removed,
            total_pixels: total,
            removed_fraction: frac,
            warning,
        },
    )
}

fn rotate_free_rgba(img: &RgbaImage, deg_cw: f32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 {
        return img.clone();
    }
    let rad = (deg_cw as f64).to_radians();
    let (sin_a, cos_a) = rad.sin_cos();
    let nw = ((w as f64 * cos_a.abs()) + (h as f64 * sin_a.abs())).ceil().max(1.0) as u32;
    let nh = ((w as f64 * sin_a.abs()) + (h as f64 * cos_a.abs())).ceil().max(1.0) as u32;

    let fill = border_fill_rgba(img);
    let mut out = RgbaImage::from_pixel(nw, nh, fill);
    let cx_in = (w as f64 - 1.0) * 0.5;
    let cy_in = (h as f64 - 1.0) * 0.5;
    let cx_out = (nw as f64 - 1.0) * 0.5;
    let cy_out = (nh as f64 - 1.0) * 0.5;

    let w_max = (w - 1) as f64;
    let h_max = (h - 1) as f64;

    for y in 0..nh {
        let dy = y as f64 - cy_out;
        for x in 0..nw {
            let dx = x as f64 - cx_out;
            // Inverse clockwise rotation (screen coords, Y down):
            // forward: x' = dx*cos - dy*sin, y' = dx*sin + dy*cos
            // inverse: sx = dx*cos + dy*sin, sy = -dx*sin + dy*cos
            let sx = dx * cos_a + dy * sin_a + cx_in;
            let sy = -dx * sin_a + dy * cos_a + cy_in;
            if sx < 0.0 || sy < 0.0 || sx > w_max || sy > h_max {
                continue;
            }
            let x0 = sx.floor() as u32;
            let y0 = sy.floor() as u32;
            let x1 = (x0 + 1).min(w - 1);
            let y1 = (y0 + 1).min(h - 1);
            let fx = sx - x0 as f64;
            let fy = sy - y0 as f64;

            let p00 = img.get_pixel(x0, y0).0;
            let p10 = img.get_pixel(x1, y0).0;
            let p01 = img.get_pixel(x0, y1).0;
            let p11 = img.get_pixel(x1, y1).0;

            let mut px = [0u8; 4];
            for c in 0..4 {
                let top = p00[c] as f64 * (1.0 - fx) + p10[c] as f64 * fx;
                let bot = p01[c] as f64 * (1.0 - fx) + p11[c] as f64 * fx;
                let v = top * (1.0 - fy) + bot * fy;
                px[c] = v.round().clamp(0.0, 255.0) as u8;
            }
            out.put_pixel(x, y, Rgba(px));
        }
    }
    out
}

/// Apply rotation -> clamped crop -> optional background removal + defringe on an `RgbaImage`,
/// returning both the resulting RGBA image and any background-removal report.
pub fn apply_preprocess_ex(
    img: &RgbaImage,
    pp: &PreprocessOptions,
) -> (RgbaImage, Option<RemoveBgReport>) {
    if pp.is_identity() {
        return (img.clone(), None);
    }
    let deg = normalize_deg(pp.rotate_deg);
    let mut cur = if deg.abs() < 1e-4 {
        img.clone()
    } else if (deg - 90.0).abs() < 1e-3 {
        imageops::rotate90(img)
    } else if (deg + 90.0).abs() < 1e-3 {
        imageops::rotate270(img)
    } else if (deg.abs() - 180.0).abs() < 1e-3 {
        imageops::rotate180(img)
    } else {
        rotate_free_rgba(img, deg)
    };

    if let Some([cx, cy, cw, ch]) = pp.crop {
        let (w, h) = cur.dimensions();
        if w > 0 && h > 0 && cw > 0 && ch > 0 {
            let x0 = cx.min(w - 1);
            let y0 = cy.min(h - 1);
            let rw = cw.max(1).min(w - x0);
            let rh = ch.max(1).min(h - y0);
            cur = imageops::crop_imm(&cur, x0, y0, rw, rh).to_image();
        }
    }

    let report = if pp.remove_background {
        let (stripped, rep) = remove_background_rgba(&cur, pp.bg_tolerance, pp.defringe_px);
        cur = stripped;
        Some(rep)
    } else {
        None
    };

    (cur, report)
}

/// Apply rotation, clamped crop, and optional background removal on an `RgbaImage`.
pub fn apply_preprocess(img: &RgbaImage, pp: &PreprocessOptions) -> RgbaImage {
    apply_preprocess_ex(img, pp).0
}

/// Apply preprocessing to an `RgbImage`, returning the flattened RGB image, an optional foreground
/// mask (`Some(mask)` when `pp.remove_background` is enabled, `true` = opaque foreground), and any
/// background-removal report.
pub fn apply_preprocess_rgb_with_mask(
    img: &RgbImage,
    pp: &PreprocessOptions,
) -> (RgbImage, Option<Vec<bool>>, Option<RemoveBgReport>) {
    if pp.is_identity() {
        return (img.clone(), None, None);
    }
    let (w, h) = img.dimensions();
    let mut rgba = RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels() {
        rgba.put_pixel(x, y, Rgba([p[0], p[1], p[2], 255]));
    }
    let (processed, report) = apply_preprocess_ex(&rgba, pp);
    let (pw, ph) = processed.dimensions();
    let mut rgb = RgbImage::new(pw, ph);
    let mut fg_mask = if pp.remove_background {
        Some(Vec::with_capacity((pw as usize) * (ph as usize)))
    } else {
        None
    };
    for (x, y, p) in processed.enumerate_pixels() {
        let a = p[3] as u32;
        if let Some(ref mut mask) = fg_mask {
            mask.push(p[3] >= 128);
        }
        let blend = |c: u8| (((c as u32) * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.put_pixel(x, y, Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    (rgb, fg_mask, report)
}

/// Convenience wrapper for `RgbImage` (backward-compatible with 1.1.0 callers).
pub fn apply_preprocess_rgb(img: &RgbImage, pp: &PreprocessOptions) -> RgbImage {
    apply_preprocess_rgb_with_mask(img, pp).0
}
