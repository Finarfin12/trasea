//! The whole pipeline: (a) thin-line guard -> (b) AI clean -> (c) palette lock + layer tracing -> (d) lines on top.

use crate::clean::clean;
use crate::export::{to_svg, Doc};
use crate::lines::{dilate_mask, line_colors, thin_mask, ThinMask, ThinParams};
use crate::palette::{assign, fill_masked, kmeans_palette, remove_small_regions};
use crate::stats::{stats_for, Stats};
use crate::trace::{trace_layers, trace_mask, Layer, TraceParams};
use crate::Result;
use image::{imageops::FilterType, GrayImage, RgbImage};
use std::path::PathBuf;
use visioncortex::BinaryImage;

#[derive(Debug, Clone)]
pub struct Params {
    /// Palette size, 2..=16.
    pub colors: usize,
    /// 0.0 = most detail (more nodes) .. 1.0 = smoothest (fewest nodes).
    pub smooth: f32,
    /// Trace thin dark lines from the original and stack them on top.
    pub keep_lines: bool,
    /// Clean the image with Real-ESRGAN first (median filter when off or unavailable).
    pub ai_clean: bool,
    /// Folder with `realesrgan-ncnn-vulkan.exe` and `models/`.
    pub sidecar_dir: Option<PathBuf>,
    /// Working resolution relative to the loaded image (the architecture uses 2).
    pub up: u32,
    /// Compute the render-vs-original error (slower; for the benchmark and the UI statistics).
    pub measure: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params { colors: 10, smooth: 0.5, keep_lines: true, ai_clean: true, sidecar_dir: None, up: 2, measure: true }
    }
}

#[derive(Debug, Clone)]
pub struct Output {
    pub svg: String,
    pub doc: Doc,
    pub stats: Stats,
    /// Palette actually used (RGB).
    pub palette: Vec<[u8; 3]>,
    /// Messages for the user (e.g. "AI cleaning was skipped ...").
    pub notes: Vec<String>,
    pub used_ai: bool,
    pub thin_pixels: usize,
}

pub type Progress<'a> = &'a dyn Fn(&str, f32);

/// Reference size the trace thresholds are written for: thresholds are scaled by working size / this.
const REF_LONG_SIDE: f64 = 1200.0;

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Upscale a boolean mask to the working size (bilinear on the mask, then threshold at 50%).
fn upscale_mask(mask: &[bool], w: usize, h: usize, ww: u32, wh: u32) -> BinaryImage {
    let mut g = GrayImage::new(w as u32, h as u32);
    for (i, &m) in mask.iter().enumerate() {
        g.put_pixel((i % w) as u32, (i / w) as u32, image::Luma([if m { 255 } else { 0 }]));
    }
    let big = image::imageops::resize(&g, ww, wh, FilterType::Triangle);
    let mut bin = BinaryImage::new_w_h(ww as usize, wh as usize);
    for (i, p) in big.pixels().enumerate() {
        if p[0] >= 128 {
            bin.set_pixel(i % ww as usize, i / ww as usize, true);
        }
    }
    bin
}

fn upscale_bool(mask: &[bool], w: usize, h: usize, ww: usize, wh: usize) -> Vec<bool> {
    let mut out = vec![false; ww * wh];
    for y in 0..wh {
        let sy = (y * h / wh).min(h - 1);
        for x in 0..ww {
            out[y * ww + x] = mask[sy * w + (x * w / ww).min(w - 1)];
        }
    }
    out
}

pub fn vectorize(img: &RgbImage, params: &Params, progress: Progress) -> Result<Output> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let up = params.up.max(1);
    let mut notes = Vec::new();

    // (a) thin-line guard, from the ORIGINAL image
    progress("Looking for thin lines", 0.05);
    let thin: Option<ThinMask> = params.keep_lines.then(|| thin_mask(img, &ThinParams::for_size(img.width(), img.height())));
    let thin_pixels = thin.as_ref().map_or(0, |t| t.count());

    // (b) clean
    progress(if params.ai_clean { "Cleaning with Real-ESRGAN" } else { "Cleaning" }, 0.15);
    let cleaned = clean(img, params.ai_clean, params.sidecar_dir.as_deref(), up);
    if let Some(n) = cleaned.note {
        notes.push(n);
    }
    let work = cleaned.image;
    let (ww, wh) = (work.width() as usize, work.height() as usize);
    let s = (ww.max(wh) as f64) / REF_LONG_SIDE;
    let t = params.smooth.clamp(0.0, 1.0) as f64;

    // (c) palette lock. Pixels under the thin lines are kept out of the colour estimate and filled from their neighbours.
    progress("Locking the palette", 0.55);
    let excl: Vec<bool> = match &thin {
        Some(tm) => dilate_mask(&upscale_bool(&tm.mask, w, h, ww, wh), ww, wh, (1.5 * up as f64).round() as i32),
        None => vec![false; ww * wh],
    };
    let k = params.colors.clamp(2, 16);
    let palette = kmeans_palette(&work, Some(&excl), k);
    let mut labels = assign(&work, &palette);
    fill_masked(&mut labels, &excl, ww, wh);
    let min_area = ((40.0 + 200.0 * t) * s * s).round().max(8.0) as usize;
    remove_small_regions(&mut labels, ww, wh, min_area, palette.len());

    progress("Tracing colour fields", 0.7);
    let tp = TraceParams { corner_deg: lerp(50.0, 80.0, t), sigma: lerp(0.9, 1.8, t) * s, tol: lerp(0.8, 4.0, t) * s, min_area };
    let fields = trace_layers(&labels, ww, wh, &palette, &tp);

    // (d) thin lines on top
    progress("Tracing thin lines", 0.88);
    let mut lines: Vec<Layer> = Vec::new();
    if let Some(tm) = &thin {
        let (assign_c, colors) = line_colors(tm, 3);
        // line tracing keeps more detail than the fields: shorter segments, fewer dropped pieces
        let lp = TraceParams { sigma: lerp(0.6, 1.0, t) * s, tol: lerp(0.5, 1.5, t) * s, min_area: ((tm.comps.iter().map(|c| c.area).min().unwrap_or(8) as f64) * (up * up) as f64 * 0.5) as usize, ..tp.clone() };
        for (ci, color) in colors.iter().enumerate() {
            let mut group = vec![false; w * h];
            for (i, &l) in tm.labels.iter().enumerate() {
                if l != 0 && assign_c[l as usize - 1] == ci {
                    group[i] = true;
                }
            }
            let bin = upscale_mask(&group, w, h, ww as u32, wh as u32);
            let contours = trace_mask(&bin, &lp);
            if !contours.is_empty() {
                lines.push(Layer { id: crate::hex(*color), color: *color, contours, canvas: false });
            }
        }
    }

    progress("Writing SVG", 0.95);
    let doc = Doc { width: img.width(), height: img.height(), scale: ww as f64 / w as f64, fields, lines };
    let svg = to_svg(&doc);
    let stats = stats_for(&svg, params.measure.then_some(img));
    progress("Done", 1.0);
    Ok(Output { svg, doc, stats, palette, notes, used_ai: cleaned.used_ai, thin_pixels })
}
