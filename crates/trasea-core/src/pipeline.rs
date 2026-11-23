//! The whole pipeline, in two halves so the app can keep the expensive first half in memory:
//! `prepare` = (a) thin-line guard + (b) clean (median or Real-ESRGAN), run once per picture;
//! `vectorize_prepared` = (c) palette + layer tracing + (d) lines on top, run again whenever a setting changes
//! (palette, brush, node budget, smoothness ...) without cleaning the picture again.

use crate::brush::{BrushMap, BrushStroke};
use crate::centerline::{centerline_opts, CenterOpts};
use crate::clean::clean;
use crate::export::{to_svg, Doc, ExportOptions, StrokeData, StrokeLayer};
use crate::heatmap::{node_heatmap, NodeHeatmap};
use crate::lines::{dilate_mask, line_colors, thin_mask, ThinMask, ThinParams};
use crate::palette::{assign, fill_masked, kmeans_palette, refine_thin_regions, remove_small_regions_ex};
use crate::primitives;
use crate::stats::{count_svg, stats_for, Stats};
use crate::symmetry::{self, SymmetryMode};
use crate::text_detect::{self, TextRegion};
use crate::trace::{trace_layers_ex, trace_mask, Layer, TraceCtx, TraceParams};
use crate::{Result, Rgb};
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
    /// The designer's own palette. With at least two different colours it replaces the automatic one.
    pub custom_palette: Option<Vec<Rgb>>,
    /// Stop smoothing when the drawing has at most this many nodes (`None` = just use `smooth`).
    pub target_nodes: Option<usize>,
    /// Correction brush strokes, in source pixels (later strokes paint over earlier ones).
    pub brush_strokes: Vec<BrushStroke>,
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
            custom_palette: None,
            target_nodes: None,
            brush_strokes: Vec::new(),
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
    /// Node density of the drawing (for the preview overlay only; never exported).
    pub heatmap: NodeHeatmap,
    /// The smoothness that was really used (0..1). Differs from `Params::smooth` when a node budget was applied.
    pub effective_smooth: f32,
    /// `Some(true)` when `target_nodes` was met, `Some(false)` when it could not be, `None` without a target.
    pub budget_met: Option<bool>,
}

/// The expensive first half of the pipeline, kept in memory: the loaded picture, its cleaned copy at working size and the
/// thin-line mask of the ORIGINAL.
#[derive(Debug, Clone)]
pub struct PreparedImage {
    pub src: RgbImage,
    pub cleaned: RgbImage,
    pub thin: ThinMask,
    pub used_ai: bool,
    pub notes: Vec<String>,
    /// Working resolution factor the clean step used.
    pub up: u32,
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
/// A designer colour that covers less than this share of the picture is a deliberate accent: its regions are kept.
const ACCENT_SHARE: f64 = 0.01;
/// Largest number of colours a manual palette may have (labels are bytes).
const MAX_CUSTOM_COLORS: usize = 64;
/// Search steps of the node budget (after the first full-smoothness test).
const BUDGET_STEPS: usize = 4;
/// When even full smoothness is over budget, the curve fit and the speck size may be loosened by up to this factor.
const BUDGET_MAX_EXTRA: f64 = 4.0;

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


/// Run the first half once: thin-line guard (from the ORIGINAL) and clean.
pub fn prepare(src: RgbImage, params: &Params, progress: Progress) -> Result<PreparedImage> {
    let up = params.up.max(1);
    progress("Looking for thin lines", 0.05);
    let thin = thin_mask(&src, &ThinParams::for_size(src.width(), src.height()));
    progress(if params.ai_clean { "Cleaning with Real-ESRGAN" } else { "Cleaning" }, 0.15);
    let cleaned = clean(&src, params.ai_clean, params.sidecar_dir.as_deref(), up);
    let mut notes = Vec::new();
    if let Some(n) = cleaned.note {
        notes.push(n);
    }
    Ok(PreparedImage { src, cleaned: cleaned.image, thin, used_ai: cleaned.used_ai, notes, up })
}

/// Both halves in one call (what the CLI and the tests use).
pub fn vectorize(img: &RgbImage, params: &Params, progress: Progress) -> Result<Output> {
    let prep = prepare(img.clone(), params, progress)?;
    vectorize_prepared(&prep, params, progress)
}

/// Everything that does not depend on the smoothness: palette, labels, zone map.
struct Base<'a> {
    prep: &'a PreparedImage,
    thin: Option<&'a ThinMask>,
    palette: Vec<Rgb>,
    labels: Vec<u8>,
    protect: Vec<bool>,
    brush: Option<BrushMap>,
    ww: usize,
    wh: usize,
    s: f64,
    scale: f64,
}

fn dedupe(p: &[Rgb]) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::new();
    for &c in p {
        if !out.contains(&c) {
            out.push(c);
        }
    }
    out.truncate(MAX_CUSTOM_COLORS);
    out
}

fn base<'a>(prep: &'a PreparedImage, params: &Params, notes: &mut Vec<String>) -> Base<'a> {
    let (w, h) = (prep.src.width() as usize, prep.src.height() as usize);
    let up = prep.up.max(1);
    let work = &prep.cleaned;
    let (ww, wh) = (work.width() as usize, work.height() as usize);
    let s = (ww.max(wh) as f64) / REF_LONG_SIDE;
    let scale = ww as f64 / w as f64;
    let thin = params.keep_lines.then_some(&prep.thin);

    // palette lock. Pixels under the thin lines are kept out of the colour estimate and filled from their neighbours.
    let excl: Vec<bool> = match thin {
        Some(tm) => dilate_mask(&upscale_bool(&tm.mask, w, h, ww, wh), ww, wh, (1.5 * up as f64).round() as i32),
        None => vec![false; ww * wh],
    };
    let custom = params.custom_palette.as_ref().map(|p| dedupe(p)).filter(|p| p.len() >= 2);
    if params.custom_palette.is_some() && custom.is_none() {
        notes.push("The manual palette needs at least 2 different colours; the automatic palette was used.".to_string());
    }
    let palette = match &custom {
        Some(p) => p.clone(),
        None => kmeans_palette(work, Some(&excl), params.colors.clamp(2, 16)),
    };
    let mut labels = assign(work, &palette);
    fill_masked(&mut labels, &excl, ww, wh);
    refine_thin_regions(&mut labels, work, &palette, ww, wh, (REF_THIN_AREA * s * s) as usize);

    // a colour the designer kept that covers very little is an accent on purpose: do not merge it away
    let mut protect = vec![false; palette.len()];
    if custom.is_some() {
        let mut area = vec![0usize; palette.len()];
        for &l in &labels {
            area[l as usize] += 1;
        }
        for (i, &a) in area.iter().enumerate() {
            protect[i] = a > 0 && (a as f64) < ACCENT_SHARE * labels.len() as f64;
        }
    }
    let brush = BrushMap::rasterize(&params.brush_strokes, scale, ww, wh);
    Base { prep, thin, palette, labels, protect, brush, ww, wh, s, scale }
}

/// How hard a pass smooths: `t` = smoothness 0..1; `extra` >= 1 loosens the curve fit and the speck size further (node
/// budget only, after full smoothness).
#[derive(Clone, Copy)]
struct Knobs {
    t: f64,
    extra: f64,
}

struct FieldsOut {
    fields: Vec<Layer>,
    symmetry_applied: Option<String>,
    text_regions: Vec<TextRegion>,
    primitives_count: usize,
    sym_plan: Option<symmetry::SymPlan>,
    tp: TraceParams,
}

struct LinesOut {
    line_shapes: Vec<Layer>,
    line_fallback: Vec<Layer>,
    strokes: Vec<StrokeLayer>,
}

fn trace_fields(b: &Base, params: &Params, k: Knobs, with_text: bool) -> FieldsOut {
    let up = b.prep.up.max(1);
    let (ww, wh, s, scale, t) = (b.ww, b.wh, b.s, b.scale, k.t);
    let mut labels = b.labels.clone();
    let speck = 1.0 + (k.extra - 1.0) * 0.5;
    let min_area = (((40.0 + 200.0 * t) * s * s * speck).round()).max(8.0) as usize;
    remove_small_regions_ex(&mut labels, ww, wh, min_area, b.palette.len(), &b.palette, SPECK_BOOST, b.brush.as_ref().map(|m| m.zones.as_slice()), &b.protect);

    // symmetry: one side is copied onto the other on the label picture, so both sides trace to the same geometry
    let mut symmetry_applied = None;
    let sym_plan = symmetry::plan(&labels, ww, wh, params.symmetry);
    if let Some(pl) = &sym_plan {
        symmetry::apply(&mut labels, ww, wh, pl);
        symmetry_applied = Some(pl.name().to_string());
    }
    // text areas (a heads-up for the user; the drawing is not touched)
    let text_regions = if with_text && params.detect_text { text_detect::detect(&labels, ww, wh, up as usize, &b.palette) } else { Vec::new() };

    let tp = TraceParams { corner_deg: lerp(50.0, 80.0, t) + (k.extra - 1.0) * 14.0, sigma: lerp(0.9, 1.8, t) * s, tol: lerp(0.8, 4.0, t) * s * k.extra, min_area };
    let ctx = TraceCtx { prim_scale: params.detect_primitives.then_some(scale), zones: b.brush.as_ref(), protect: Some(&b.protect) };
    let mut fields = trace_layers_ex(&labels, ww, wh, &b.palette, &tp, &ctx);
    if let Some(pl) = &sym_plan {
        symmetry::mirror_layers(&mut fields, pl, (ww as f64, wh as f64));
    }
    let primitives_count = if params.detect_primitives { primitives::extract(&mut fields, scale, (ww as f64, wh as f64)) } else { 0 };
    crate::trace::drop_hidden_duplicates(&mut fields);
    FieldsOut { fields, symmetry_applied, text_regions, primitives_count, sym_plan, tp }
}

/// Thin lines on top: as strokes (centerline + measured width) and, for the "shapes" export, as outlines.
fn trace_lines(b: &Base, k: Knobs, fo: &FieldsOut) -> LinesOut {
    let (w, h) = (b.prep.src.width() as usize, b.prep.src.height() as usize);
    let up = b.prep.up.max(1);
    let (ww, wh, s, scale, t) = (b.ww, b.wh, b.s, b.scale, k.t);
    let mut out = LinesOut { line_shapes: Vec::new(), line_fallback: Vec::new(), strokes: Vec::new() };
    let copts = CenterOpts { merge: true, width_tol: 0.75 * scale };
    let Some(tm) = b.thin else { return out };
    let (assign_c, colors) = line_colors(tm, 3);
    let min_line = ((tm.comps.iter().map(|c| c.area).min().unwrap_or(8) as f64) * (up * up) as f64 * 0.5) as usize;
    let lp = TraceParams { sigma: lerp(0.6, 1.0, t) * s, tol: lerp(0.5, 1.5, t) * s * k.extra, min_area: min_line, ..fo.tp.clone() };
    let stroke_tol = lerp(0.7, 2.0, t) * s * k.extra;
    for (ci, color) in colors.iter().enumerate() {
        let mut group = vec![false; w * h];
        for (i, &l) in tm.labels.iter().enumerate() {
            if l != 0 && assign_c[l as usize - 1] == ci {
                group[i] = true;
            }
        }
        let mut big = upscale_bool_mask(&group, w, h, ww as u32, wh as u32);
        if let Some(pl) = &fo.sym_plan {
            symmetry::apply(&mut big, ww, wh, pl);
        }
        let contours = trace_mask(&to_binary(&big, ww, wh), &lp);
        if !contours.is_empty() {
            out.line_shapes.push(Layer { id: crate::hex(*color), color: *color, contours, canvas: false, prims: vec![] });
        }
        let cl = centerline_opts(&big, ww, wh, stroke_tol, 0.5, &copts);
        if cl.shape_mask.iter().any(|&b| b) {
            let contours = trace_mask(&to_binary(&cl.shape_mask, ww, wh), &lp);
            if !contours.is_empty() {
                out.line_fallback.push(Layer { id: crate::hex(*color), color: *color, contours, canvas: false, prims: vec![] });
            }
        }
        let min_len = MIN_STROKE_LEN * ww as f64 / w as f64;
        let kept: Vec<_> = cl.strokes.iter().filter(|st| { let mut l = 0.0; for cv in &st.curves { l += ((cv[3].0 - cv[0].0).powi(2) + (cv[3].1 - cv[0].1).powi(2)).sqrt(); } l >= min_len }).collect();
        if !kept.is_empty() {
            out.strokes.push(StrokeLayer { color: *color, strokes: kept.iter().map(|st| { let mut d = StrokeData::from(*st); d.width += STROKE_WIDTH_BIAS * scale; d }).collect() });
        }
    }
    out
}

fn make_doc(b: &Base, fields: Vec<Layer>, lines: &LinesOut) -> Doc {
    Doc {
        width: b.prep.src.width(),
        height: b.prep.src.height(),
        scale: b.scale,
        fields,
        line_shapes: lines.line_shapes.clone(),
        line_fallback: lines.line_fallback.clone(),
        strokes: lines.strokes.clone(),
    }
}

fn count_nodes(doc: &Doc) -> usize {
    count_svg(&to_svg(doc, &ExportOptions::default())).1
}

/// The knobs of a position `p` in 0..=1 along the "smoother and smoother" path: the first half raises the smoothness from
/// the chosen one to its maximum, the second half then loosens the curve fit up to `BUDGET_MAX_EXTRA` times.
fn knobs_at(t0: f64, p: f64) -> Knobs {
    if p <= 0.5 {
        Knobs { t: t0 + (1.0 - t0) * (p * 2.0), extra: 1.0 }
    } else {
        Knobs { t: 1.0, extra: 1.0 + (BUDGET_MAX_EXTRA - 1.0) * (p * 2.0 - 1.0) }
    }
}

/// The second half: palette, brush zones, tracing, lines, and (with `target_nodes`) the node budget.
pub fn vectorize_prepared(prep: &PreparedImage, params: &Params, progress: Progress) -> Result<Output> {
    let mut notes = prep.notes.clone();
    progress("Locking the palette", 0.55);
    let b = base(prep, params, &mut notes);
    let t0 = params.smooth.clamp(0.0, 1.0) as f64;

    progress("Tracing", 0.7);
    let mut knobs = Knobs { t: t0, extra: 1.0 };
    let mut fo = trace_fields(&b, params, knobs, true);
    let mut lines = trace_lines(&b, knobs, &fo);
    let mut budget_met = None;
    if let Some(limit) = params.target_nodes.map(|n| n.max(1)) {
        let first = count_nodes(&make_doc(&b, fo.fields.clone(), &lines));
        if first <= limit {
            budget_met = Some(true);
        } else {
            progress("Fitting the node budget", 0.8);
            // While searching, the thin lines stay as traced for the first pass (they are a small, steady part of the
            // total), so only the colour fields are traced again. The final pass then redoes everything at the winner.
            let nodes_at = |p: f64| -> (Knobs, FieldsOut, usize) {
                let k = knobs_at(t0, p);
                let f = trace_fields(&b, params, k, false);
                let n = count_nodes(&make_doc(&b, f.fields.clone(), &lines));
                (k, f, n)
            };
            let (kmax, fmax, nmax) = nodes_at(1.0);
            // the thin lines of the first pass weigh a bit more than those at full smoothness: measure that difference once
            let lines_max = trace_lines(&b, kmax, &fmax);
            let real_max = count_nodes(&make_doc(&b, fmax.fields.clone(), &lines_max));
            let bias = nmax.saturating_sub(real_max);
            if real_max > limit {
                // not reachable: keep the lightest result and say so
                knobs = kmax;
                fo = fmax;
                budget_met = Some(false);
            } else {
                let (mut lo, mut hi) = (0.0f64, 1.0f64);
                let (mut kbest, mut fbest) = (kmax, fmax);
                for _ in 0..BUDGET_STEPS {
                    let mid = (lo + hi) / 2.0;
                    let (k, f, n) = nodes_at(mid);
                    if n.saturating_sub(bias) <= limit {
                        hi = mid;
                        kbest = k;
                        fbest = f;
                    } else {
                        lo = mid;
                    }
                }
                knobs = kbest;
                fo = fbest;
                budget_met = Some(true);
            }
            // keep the text areas found at the chosen smoothness
            if params.detect_text {
                fo.text_regions = trace_fields(&b, params, Knobs { t: knobs.t, extra: 1.0 }, true).text_regions;
            }
            lines = if (knobs.t - kmax.t).abs() < 1e-12 && (knobs.extra - kmax.extra).abs() < 1e-12 { lines_max } else { trace_lines(&b, knobs, &fo) };
            let final_nodes = count_nodes(&make_doc(&b, fo.fields.clone(), &lines));
            if final_nodes > limit {
                budget_met = Some(false);
            }
            if budget_met == Some(false) {
                notes.push(format!("Target <= {limit} nodes is too low for the detail of this picture; the best safe result has {final_nodes} nodes."));
            }
        }
    }

    progress("Writing SVG", 0.95);
    let FieldsOut { fields, symmetry_applied, text_regions, primitives_count, .. } = fo;
    let doc = make_doc(&b, fields, &lines);
    let svg = to_svg(&doc, &ExportOptions::default());
    let stats = stats_for(&svg, params.measure.then_some(&prep.src));
    let heatmap = node_heatmap(&doc);
    let thin_pixels = b.thin.map_or(0, |t| t.count());
    progress("Done", 1.0);
    Ok(Output {
        svg,
        doc,
        stats,
        palette: b.palette.clone(),
        notes,
        used_ai: prep.used_ai,
        thin_pixels,
        primitives_count,
        symmetry_applied,
        text_regions,
        heatmap,
        effective_smooth: knobs.t as f32,
        budget_met,
    })
}
