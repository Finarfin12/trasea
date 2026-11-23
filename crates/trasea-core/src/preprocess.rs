//! Non-destructive image pre-processing (crop & rotation) before tracing.
//!
//! - Exact 90°/180°/270° rotations use lossless pixel transpositions.
//! - Free-angle rotations use bilinear interpolation on an expanded canvas so corners are never
//!   clipped; exterior corners are filled with transparent (when the image has border transparency)
//!   or the dominant border colour.
//! - Crop coordinates are safely clamped to the rotated image bounds.

use image::{imageops, Rgb, RgbImage, Rgba, RgbaImage};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PreprocessOptions {
    /// Optional crop rectangle `[x, y, w, h]` in pixels (applied after rotation).
    #[serde(default)]
    pub crop: Option<[u32; 4]>,
    /// Clockwise rotation in degrees (`-180.0..=180.0`).
    #[serde(default)]
    pub rotate_deg: f32,
}

impl PreprocessOptions {
    pub fn is_identity(&self) -> bool {
        self.crop.is_none() && (!self.rotate_deg.is_finite() || self.rotate_deg.abs() < 1e-4)
    }
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

/// Apply rotation (exact 90° multiples or bilinear free-angle) followed by clamped crop on an `RgbaImage`.
pub fn apply_preprocess(img: &RgbaImage, pp: &PreprocessOptions) -> RgbaImage {
    if pp.is_identity() {
        return img.clone();
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
    cur
}

/// Convenience wrapper for `RgbImage` used by `pipeline::prepare`.
pub fn apply_preprocess_rgb(img: &RgbImage, pp: &PreprocessOptions) -> RgbImage {
    if pp.is_identity() {
        return img.clone();
    }
    let (w, h) = img.dimensions();
    let mut rgba = RgbaImage::new(w, h);
    for (x, y, p) in img.enumerate_pixels() {
        rgba.put_pixel(x, y, Rgba([p[0], p[1], p[2], 255]));
    }
    let processed = apply_preprocess(&rgba, pp);
    let (pw, ph) = processed.dimensions();
    let mut rgb = RgbImage::new(pw, ph);
    for (x, y, p) in processed.enumerate_pixels() {
        let a = p[3] as u32;
        let blend = |c: u8| (((c as u32) * a + 255 * (255 - a) + 127) / 255) as u8;
        rgb.put_pixel(x, y, Rgb([blend(p[0]), blend(p[1]), blend(p[2])]));
    }
    rgb
}