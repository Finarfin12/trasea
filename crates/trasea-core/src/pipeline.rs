//! The whole pipeline: (a) thin-line guard -> (b) AI clean -> (c) palette lock + layer tracing -> (d) lines on top.

use crate::clean::clean;
use crate::centerline::{centerline_opts, CenterOpts};
use crate::export::{to_svg, Doc, ExportOptions, StrokeData, StrokeLayer};
use crate::lines::{dilate_mask, line_colors, thin_mask, ThinMask, ThinParams};
use crate::palette::{assign, fill_masked, kmeans_palette, refine_thin_regions, remove_small_regions};
use crate::stats::{stats_for, Stats};
use crate::symmetry::{self, SymmetryMode};
use crate::text_detect::{self, TextRegion};
use crate::primitives;
use crate::trace::{trace_layers_ex, trace_mask, Layer, TraceParams};
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
    /// Write circles, ellipses and rectangles that match closely as pure `<circle>`, `<ellipse>`, `<rect>`.
    pub detect_primitives: bool,
    /// Mirror one half of the picture onto the other before tracing (`Off` by default).
    pub symmetry: SymmetryMode,
    /// Look for areas that probably hold text and report them (the drawing is not changed).
    pub detect_text: bool,
}

impl Default for Params {
    fn default() -> Self {
        Params {
            colors: 10,
            smooth: 0.5,
            keep_lines: true,
            ai_clean: false,
            sidecar_dir: None,
            up: 2,
            measure: true,
            detect_primitives: true,
            symmetry: SymmetryMode::Off,
            detect_text: true,
        }
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
    /// Circles, ellipses and rectangles recognised (also in `stats.primitives`).
    pub primitives_count: usize,
    /// "vertical" or "horizontal" when the symmetry enforcer changed the picture, else `None`.
    pub symmetry_applied: Option<String>,
    /// Areas that probably hold text, in source pixels. They are never written to the SVG/PDF.
    pub text_regions: Vec<TextRegion>,
}

pub type Progress<'a> = &'a dyn Fn(&str, f32);

/// Reference size the trace thresholds are written for: thresholds are scaled by working size / this.
const REF_LONG_SIDE: f64 = 1200.0;
/// Low-contrast specks (JPEG residue, watermark ghosts) are removed up to this many times the normal speck size.
/// Regions up to this many working pixels (at the reference size) are checked for a better palette colour.
const REF_THIN_AREA: f64 = 1500.0;
const SPECK_BOOST: f64 = 25.0;
/// Strokes shorter than this (source px) are dropped (degenerate skeleton bits).
const MIN_STROKE_LEN: f64 = 1.0;
/// The thin-line mask holds only the dark core of a line; add half a source pixel so strokes match the original width.
const STROKE_WIDTH_BIAS: f64 = 0.5;

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Upscale a boolean mask to the working size (bilinear on the mask, then threshold at 50%).
fn upscale_bool_mask(mask: &[bool], w: usize, h: usize, ww: u32, wh: u32) -> Vec<bool> {
    let mut g = GrayImage::new(w as u32, h as u32);
    for (i, &m) in mask.iter().enumerate() {
        g.put_pixel((i % w) as u32, (i / w) as u32, image::Luma([if m { 255 } else { 0 }]));
    }
    let big = image::imageops::resize(&g, ww, wh, FilterType::Triangle);
    big.pixels().map(|p| p[0] >= 128).collect()
}

fn to_binary(mask: &[bool], w: usize, h: usize) -> BinaryImage {
    let mut bin = BinaryImage::new_w_h(w, h);
    for (i, &m) in mask.iter().enumerate() {
        if m {
            bin.set_pixel(i % w, i / w, true);
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
    refine_thin_regions(&mut labels, &work, &palette, ww, wh, (REF_THIN_AREA * s * s) as usize);
    let min_area = ((40.0 + 200.0 * t) * s * s).round().max(8.0) as usize;
    remove_small_regions(&mut labels, ww, wh, min_area, palette.len(), &palette, SPECK_BOOST);
    let scale = ww as f64 / w as f64;

    // symmetry: one side is copied onto the other on the label picture, so both sides trace to the same geometry
    let mut symmetry_applied = None;
    let sym_plan = symmetry::plan(&labels, ww, wh, params.symmetry);
    if let Some(pl) = &sym_plan {
        symmetry::apply(&mut labels, ww, wh, pl);
        symmetry_applied = Some(pl.name().to_string());
    }

    // text areas (a heads-up for the user; the drawing is not touched)
    let text_regions = if params.detect_text { text_detect::detect(&labels, ww, wh, up as usize, &palette) } else { Vec::new() };

    progress("Tracing colour fields", 0.7);
    let tp = TraceParams { corner_deg: lerp(50.0, 80.0, t), sigma: lerp(0.9, 1.8, t) * s, tol: lerp(0.8, 4.0, t) * s, min_area };
    let mut fields = trace_layers_ex(&labels, ww, wh, &palette, &tp, params.detect_primitives.then_some(scale));
    if let Some(pl) = &sym_plan {
        symmetry::mirror_layers(&mut fields, pl, (ww as f64, wh as f64));
    }
    let primitives_count = if params.detect_primitives { primitives::extract(&mut fields, scale, (ww as f64, wh as f64)) } else { 0 };

    // (d) thin lines on top: as strokes (centerline + measured width) and, for the "shapes" export, as outlines
    progress("Tracing thin lines", 0.88);
    let mut line_shapes: Vec<Layer> = Vec::new();
    let mut line_fallback: Vec<Layer> = Vec::new();
    let mut strokes: Vec<StrokeLayer> = Vec::new();
    let copts = CenterOpts { merge: true, width_tol: 0.75 * scale };
    if let Some(tm) = &thin {
        let (assign_c, colors) = line_colors(tm, 3);
        let min_line = ((tm.comps.iter().map(|c| c.area).min().unwrap_or(8) as f64) * (up * up) as f64 * 0.5) as usize;
        let lp = TraceParams { sigma: lerp(0.6, 1.0, t) * s, tol: lerp(0.5, 1.5, t) * s, min_area: min_line, ..tp.clone() };
        let stroke_tol = lerp(0.7, 2.0, t) * s;
        for (ci, color) in colors.iter().enumerate() {
            let mut group = vec![false; w * h];
            for (i, &l) in tm.labels.iter().enumerate() {
                if l != 0 && assign_c[l as usize - 1] == ci {
                    group[i] = true;
                }
            }
            let mut big = upscale_bool_mask(&group, w, h, ww as u32, wh as u32);
            if let Some(pl) = &sym_plan {
                symmetry::apply(&mut big, ww, wh, pl);
            }
            let contours = trace_mask(&to_binary(&big, ww, wh), &lp);
            if !contours.is_empty() {
                line_shapes.push(Layer { id: crate::hex(*color), color: *color, contours, canvas: false, prims: vec![] });
            }
            let cl = centerline_opts(&big, ww, wh, stroke_tol, 0.5, &copts);
            if cl.shape_mask.iter().any(|&b| b) {
                let contours = trace_mask(&to_binary(&cl.shape_mask, ww, wh), &lp);
                if !contours.is_empty() {
                    line_fallback.push(Layer { id: crate::hex(*color), color: *color, contours, canvas: false, prims: vec![] });
                }
            }
            let min_len = MIN_STROKE_LEN * ww as f64 / w as f64;
            let kept: Vec<_> = cl.strokes.iter().filter(|st| { let mut l = 0.0; for cv in &st.curves { l += ((cv[3].0 - cv[0].0).powi(2) + (cv[3].1 - cv[0].1).powi(2)).sqrt(); } l >= min_len }).collect();
            if !kept.is_empty() {
                strokes.push(StrokeLayer { color: *color, strokes: kept.iter().map(|st| { let mut d = StrokeData::from(*st); d.width += STROKE_WIDTH_BIAS * scale; d }).collect() });
            }
        }
    }

    progress("Writing SVG", 0.95);
    let doc = Doc { width: img.width(), height: img.height(), scale: ww as f64 / w as f64, fields, line_shapes, line_fallback, strokes };
    let svg = to_svg(&doc, &ExportOptions::default());
    let stats = stats_for(&svg, params.measure.then_some(img));
    progress("Done", 1.0);
    Ok(Output { svg, doc, stats, palette, notes, used_ai: cleaned.used_ai, thin_pixels, primitives_count, symmetry_applied, text_regions })
}
