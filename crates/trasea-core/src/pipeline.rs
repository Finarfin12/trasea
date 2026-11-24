//! The whole pipeline, in two halves so the app can keep the expensive first half in memory:
//! `prepare` = (a) thin-line guard + (b) clean (median or Real-ESRGAN), run once per picture;
//! `vectorize_prepared` = (c) palette + layer tracing + (d) lines on top, run again whenever a setting changes
//! (palette, brush, node budget, smoothness, production profile ...) without cleaning the picture again.

use crate::brush::{BrushMap, BrushStroke};
use crate::centerline::{centerline_opts, extract_full_centerline, CenterOpts};
use crate::clean::clean;
use crate::export::{export_options_for_production, to_svg, Doc, ExportOptions, StrokeData, StrokeLayer};
use crate::heatmap::{node_heatmap, NodeHeatmap};
use crate::lines::{dilate_mask, line_colors, thin_mask, ThinMask, ThinParams};
use crate::palette::{assign, dehalo_label_borders, fill_masked, kmeans_palette, refine_thin_regions, remove_small_regions_ex, rgb_to_lab, smooth_label_jaggies};
use crate::node_edit::{apply_node_edits, NodeEdit};
use crate::path_edit::{apply_path_edits, PathEdit};
use crate::preprocess::{apply_preprocess_rgb_with_mask, PreprocessOptions};
use crate::primitives;
use crate::production::{cut_min_area_px, enforce_min_stroke, filter_micro_regions, screen_print_min_spot_px, ProductionMode, ProductionOptions};
use crate::stats::{compare, count_primitives, count_svg, render_svg, Stats};
use crate::symmetry::{self, SymmetryMode};
use crate::text_detect::{self, TextRegion};
use crate::trace::{trace_layers_ex, trace_layers_knockout, trace_mask, Layer, TraceCtx, TraceParams};
use crate::{Result, Rgb};
use image::{imageops::FilterType, GrayImage, RgbImage};
use std::collections::HashMap;
use std::path::PathBuf;
use visioncortex::BinaryImage;

#[derive(Debug, Clone)]
pub struct Params {
    /// Palette size, 2..=16 (clamped to 2..=8 in ScreenPrint mode).
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
    /// Explicit colour merges `(from, to)`: pixels matching `from` are remapped to `to` instead of nearest remaining colour.
    pub palette_merges: Vec<(Rgb, Rgb)>,
    /// Stop smoothing when the drawing has at most this many nodes (`None` = just use `smooth`).
    pub target_nodes: Option<usize>,
    /// Correction brush strokes, in source pixels (later strokes paint over earlier ones).
    pub brush_strokes: Vec<BrushStroke>,
    /// Production profile (`Standard`, `ScreenPrint`, `CutEmbroidery`, `Shading`, `Centerline`).
    pub production: ProductionOptions,
    /// Optional non-destructive pre-processing (rotation, crop, remove background) applied before tracing.
    pub preprocess: PreprocessOptions,
    /// Non-destructive per-path edits (`Delete`, `ReassignColor`) applied deterministically after tracing.
    pub path_edits: Vec<PathEdit>,
    /// Non-destructive node edits (move node/handle, delete node, toggle smooth, simplify) applied after `path_edits`.
    pub node_edits: Vec<NodeEdit>,
    /// Fingerprint (`Output::trace_fingerprint`) of the unedited trace the edits were made on. When set and different from the
    /// fingerprint of this trace, `path_edits` and `node_edits` are NOT applied (they address shapes by position, and a
    /// different trace puts other shapes at those positions). `None` = legacy behaviour: always apply.
    pub edit_fingerprint: Option<String>,
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
            palette_merges: Vec::new(),
            target_nodes: None,
            brush_strokes: Vec::new(),
            production: ProductionOptions::default(),
            preprocess: PreprocessOptions::default(),
            path_edits: Vec::new(),
            node_edits: Vec::new(),
            edit_fingerprint: None,
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
    /// SHA-256 (hex) of the traced drawing BEFORE any path/node edit. Edits are only valid for the trace they were made on.
    pub trace_fingerprint: String,
    /// True when `path_edits`/`node_edits` were skipped because `Params::edit_fingerprint` does not match this trace.
    pub edits_stale: bool,
}

/// SHA-256 (hex) over a canonical dump of the drawing. The `Debug` text of the document prints every float with its shortest
/// round-trip form, so the same trace always gives the same digest and any change of a coordinate, colour or shape count
/// changes it.
pub fn trace_fingerprint(doc: &Doc) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(format!("{doc:?}").as_bytes());
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// Apply `path_edits` then `node_edits` unless they belong to another trace. Returns (doc, fingerprint of the unedited doc, stale).
fn apply_edits_guarded(doc: Doc, params: &Params) -> (Doc, String, bool) {
    let fp = trace_fingerprint(&doc);
    let has_edits = !params.path_edits.is_empty() || !params.node_edits.is_empty();
    let stale = has_edits && params.edit_fingerprint.as_deref().map_or(false, |f| f != fp);
    if !has_edits || stale {
        return (doc, fp, stale);
    }
    let mut doc = doc;
    if !params.path_edits.is_empty() {
        doc = apply_path_edits(&doc, &params.path_edits);
    }
    if !params.node_edits.is_empty() {
        doc = apply_node_edits(&doc, &params.node_edits);
    }
    (doc, fp, false)
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
    /// Fingerprint of pre-processing applied when building this cache entry.
    pub preprocess: PreprocessOptions,
    /// Optional foreground opacity mask (`true` = opaque foreground, `false` = background stripped via `remove_background`).
    pub fg_mask: Option<Vec<bool>>,
}

pub type Progress<'a> = &'a dyn Fn(&str, f32);

/// Reference size the trace thresholds are written for: thresholds are scaled by working size / this.
const REF_LONG_SIDE: f64 = 1200.0;
/// Regions up to this many working pixels (at the reference size) are checked for a better palette colour.
const REF_THIN_AREA: f64 = 1500.0;
/// Low-contrast specks (JPEG residue, watermark ghosts) are removed up to this many times the normal speck size.
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

fn nearest_in_palette(c: Rgb, palette: &[Rgb]) -> Rgb {
    palette
        .iter()
        .copied()
        .min_by_key(|p| {
            let dr = c[0] as i32 - p[0] as i32;
            let dg = c[1] as i32 - p[1] as i32;
            let db = c[2] as i32 - p[2] as i32;
            dr * dr + dg * dg + db * db
        })
        .unwrap_or(c)
}

/// Edge-preserving bilateral smooth in Lab space (used exclusively by `ProductionMode::Shading`
/// to reduce gradient noise into clean tonal bands without blurring crisp edges).
fn edge_preserving_smooth(img: &RgbImage, radius: i32, delta_e: f32) -> RgbImage {
    let (w, h) = (img.width() as usize, img.height() as usize);
    if w == 0 || h == 0 || radius <= 0 {
        return img.clone();
    }
    let labs: Vec<[f32; 3]> = img.pixels().map(|p| rgb_to_lab(p.0)).collect();
    let sigma_c2 = 2.0 * delta_e * delta_e;
    let sigma_s2 = 2.0 * (radius as f32 * 0.75).powi(2).max(0.5);
    let mut out = RgbImage::new(w as u32, h as u32);
    for y in 0..h as i32 {
        let y0 = (y - radius).max(0);
        let y1 = (y + radius).min(h as i32 - 1);
        for x in 0..w as i32 {
            let x0 = (x - radius).max(0);
            let x1 = (x + radius).min(w as i32 - 1);
            let c_lab = labs[y as usize * w + x as usize];
            let (mut sr, mut sg, mut sb, mut wsum) = (0f32, 0f32, 0f32, 0f32);
            for yy in y0..=y1 {
                let dy2 = ((yy - y) * (yy - y)) as f32;
                let row = yy as usize * w;
                for xx in x0..=x1 {
                    let dx2 = ((xx - x) * (xx - x)) as f32;
                    let nl = labs[row + xx as usize];
                    let de2 = (nl[0] - c_lab[0]).powi(2) + (nl[1] - c_lab[1]).powi(2) + (nl[2] - c_lab[2]).powi(2);
                    let wgt = (-(dx2 + dy2) / sigma_s2 - de2 / sigma_c2).exp();
                    let px = img.get_pixel(xx as u32, yy as u32).0;
                    sr += px[0] as f32 * wgt;
                    sg += px[1] as f32 * wgt;
                    sb += px[2] as f32 * wgt;
                    wsum += wgt;
                }
            }
            if wsum > 0.0 {
                out.put_pixel(
                    x as u32,
                    y as u32,
                    image::Rgb([
                        (sr / wsum).round().clamp(0.0, 255.0) as u8,
                        (sg / wsum).round().clamp(0.0, 255.0) as u8,
                        (sb / wsum).round().clamp(0.0, 255.0) as u8,
                    ]),
                );
            } else {
                out.put_pixel(x as u32, y as u32, *img.get_pixel(x as u32, y as u32));
            }
        }
    }
    out
}

/// Run the first half once: optional non-destructive preprocess, thin-line guard (from the ORIGINAL), and clean.
pub fn prepare(src: RgbImage, params: &Params, progress: Progress) -> Result<PreparedImage> {
    let up = params.up.max(1);
    let mut notes = Vec::new();
    let (src, fg_mask) = if params.preprocess.is_identity() {
        (src, None)
    } else {
        let (img_pp, mask, rep) = apply_preprocess_rgb_with_mask(&src, &params.preprocess);
        if let Some(r) = rep {
            if let Some(w) = r.warning {
                notes.push(w);
            }
        }
        (img_pp, mask)
    };
    progress("Looking for thin lines", 0.05);
    let mut thin = thin_mask(&src, &ThinParams::for_size(src.width(), src.height()));
    if let Some(ref fg) = fg_mask {
        for (i, &keep) in fg.iter().enumerate() {
            if !keep && i < thin.mask.len() {
                thin.mask[i] = false;
                thin.labels[i] = 0;
            }
        }
    }
    progress(if params.ai_clean { "Cleaning with Real-ESRGAN" } else { "Cleaning" }, 0.15);
    let cleaned = clean(&src, params.ai_clean, params.sidecar_dir.as_deref(), up);
    if let Some(n) = cleaned.note {
        notes.push(n);
    }
    Ok(PreparedImage {
        src,
        cleaned: cleaned.image,
        thin,
        used_ai: cleaned.used_ai,
        notes,
        up,
        preprocess: params.preprocess.clone(),
        fg_mask,
    })
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
    let is_shading = params.production.mode == ProductionMode::Shading;
    let shading_buf: Option<RgbImage> = if is_shading {
        Some(edge_preserving_smooth(&prep.cleaned, 2, 14.0))
    } else {
        None
    };
    let work: &RgbImage = shading_buf.as_ref().unwrap_or(&prep.cleaned);
    let (ww, wh) = (work.width() as usize, work.height() as usize);
    let s = (ww.max(wh) as f64) / REF_LONG_SIDE;
    let scale = ww as f64 / w as f64;
    let thin = params.keep_lines.then_some(&prep.thin);

    // palette lock. Pixels under the thin lines (and any removed background) are kept out of the colour estimate.
    let mut excl: Vec<bool> = match thin {
        Some(tm) => dilate_mask(&upscale_bool(&tm.mask, w, h, ww, wh), ww, wh, (1.5 * up as f64).round() as i32),
        None => vec![false; ww * wh],
    };
    if let Some(ref fg) = prep.fg_mask {
        let fg_work = upscale_bool(fg, w, h, ww, wh);
        for (i, &keep) in fg_work.iter().enumerate() {
            if !keep {
                excl[i] = true;
            }
        }
    }

    let is_screen_print = params.production.mode == ProductionMode::ScreenPrint;
    let mut custom = params.custom_palette.as_ref().map(|p| dedupe(p)).filter(|p| p.len() >= 2);
    if params.custom_palette.is_some() && custom.is_none() {
        notes.push("The manual palette needs at least 2 different colours; the automatic palette was used.".to_string());
    }
    if is_screen_print {
        if let Some(ref mut cp) = custom {
            if cp.len() > 8 {
                cp.truncate(8);
                notes.push("Mode Sablon membatasi palet maksimal 8 warna spot.".to_string());
            }
        }
    }

    let palette = match &custom {
        Some(p) => p.clone(),
        None => {
            let max_c = if is_screen_print {
                if params.colors > 8 {
                    notes.push("Mode Sablon membatasi warna spot maksimal 8 warna.".to_string());
                }
                params.colors.clamp(2, 8)
            } else if is_shading {
                params.colors.clamp(12, 32)
            } else {
                params.colors.clamp(2, 16)
            };
            kmeans_palette(work, Some(&excl), max_c)
        }
    };
    if is_screen_print && palette.len() > 6 {
        notes.push(format!("{} warna spot dipilih (> 6 warna dapat menaikkan biaya film sablon).", palette.len()));
    }

    // Optional explicit merges (`src -> dst`): assign against `palette + merged_srcs`, then remap merged labels to `dst`.
    let mut labels = if params.palette_merges.is_empty() || custom.is_none() {
        assign(work, &palette)
    } else {
        let mut raw_map: HashMap<Rgb, Rgb> = HashMap::new();
        for &(from, to) in &params.palette_merges {
            if from != to {
                raw_map.insert(from, to);
            }
        }
        let mut assign_pal = palette.clone();
        let mut remap: Vec<u8> = (0..palette.len() as u8).collect();
        for &(from, _) in &params.palette_merges {
            if assign_pal.contains(&from) || assign_pal.len() >= 255 {
                continue;
            }
            let mut cur = from;
            let mut hops = 0;
            while let Some(&next) = raw_map.get(&cur) {
                cur = next;
                hops += 1;
                if hops > 32 || palette.contains(&cur) {
                    break;
                }
            }
            if let Some(dst_idx) = palette.iter().position(|&c| c == cur) {
                assign_pal.push(from);
                remap.push(dst_idx as u8);
            }
        }
        let mut assigned = assign(work, &assign_pal);
        for l in &mut assigned {
            *l = remap[*l as usize];
        }
        assigned
    };

    fill_masked(&mut labels, &excl, ww, wh);
    refine_thin_regions(&mut labels, work, &palette, ww, wh, (REF_THIN_AREA * s * s) as usize);
    if !is_shading {
        dehalo_label_borders(&mut labels, work, &palette, ww, wh);
    }

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
    micro_removed: usize,
    sym_plan: Option<symmetry::SymPlan>,
    tp: TraceParams,
}

struct LinesOut {
    line_shapes: Vec<Layer>,
    line_fallback: Vec<Layer>,
    strokes: Vec<StrokeLayer>,
    cut_thickened: usize,
}

fn trace_fields(b: &Base, params: &Params, k: Knobs, with_text: bool) -> FieldsOut {
    let up = b.prep.up.max(1);
    let (ww, wh, s, scale, t) = (b.ww, b.wh, b.s, b.scale, k.t);
    let mut labels = b.labels.clone();
    let speck = 1.0 + (k.extra - 1.0) * 0.5;
    let min_area = (((40.0 + 200.0 * t) * s * s * speck).round()).max(8.0) as usize;
    remove_small_regions_ex(&mut labels, ww, wh, min_area, b.palette.len(), &b.palette, SPECK_BOOST, b.brush.as_ref().map(|m| m.zones.as_slice()), &b.protect);
    smooth_label_jaggies(&mut labels, ww, wh, &b.protect);

    // Production micro-region cleanup (Standard keeps 0.4.0 behaviour unchanged).
    let micro_removed = match params.production.mode {
        ProductionMode::Standard | ProductionMode::Shading | ProductionMode::Centerline => 0,
        ProductionMode::ScreenPrint => filter_micro_regions(&mut labels, ww as u32, wh as u32, screen_print_min_spot_px(ww as u32, wh as u32)),
        ProductionMode::CutEmbroidery => filter_micro_regions(&mut labels, ww as u32, wh as u32, cut_min_area_px(ww as u32, wh as u32)),
    };

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

    let use_knockout = params.production.mode == ProductionMode::CutEmbroidery
        || (params.production.mode == ProductionMode::ScreenPrint && params.production.sablon_knockout);
    let (mut fields, primitives_count) = if let Some(ref fg) = b.prep.fg_mask {
        // When exterior background was removed via `remove_background`, trace only opaque foreground
        // regions so no background canvas rectangle is emitted while interior holes stay intact.
        let fg_work = upscale_bool(fg, b.prep.src.width() as usize, b.prep.src.height() as usize, ww, wh);
        let mut f = Vec::new();
        for (ci, &color) in b.palette.iter().enumerate() {
            let mut bin = BinaryImage::new_w_h(ww, wh);
            let mut any = false;
            for idx in 0..(ww * wh) {
                if fg_work[idx] && labels[idx] == ci as u8 {
                    bin.set_pixel(idx % ww, idx / ww, true);
                    any = true;
                }
            }
            if !any {
                continue;
            }
            let contours = crate::trace::trace_mask_ex(&bin, &tp, &ctx);
            if !contours.is_empty() {
                f.push(Layer { id: crate::hex(color), color, contours, canvas: false, prims: vec![] });
            }
        }
        if let Some(pl) = &sym_plan {
            symmetry::mirror_layers(&mut f, pl, (ww as f64, wh as f64));
        }
        let pc = if params.detect_primitives && !use_knockout {
            primitives::extract(&mut f, scale, (ww as f64, wh as f64))
        } else {
            0
        };
        (f, pc)
    } else if use_knockout {
        let mut f = trace_layers_knockout(&labels, ww, wh, &b.palette, &tp, &ctx, params.production.strip_background);
        if let Some(pl) = &sym_plan {
            symmetry::mirror_layers(&mut f, pl, (ww as f64, wh as f64));
        }
        (f, 0)
    } else {
        let mut f = trace_layers_ex(&labels, ww, wh, &b.palette, &tp, &ctx);
        if let Some(pl) = &sym_plan {
            symmetry::mirror_layers(&mut f, pl, (ww as f64, wh as f64));
        }
        let pc = if params.detect_primitives { primitives::extract(&mut f, scale, (ww as f64, wh as f64)) } else { 0 };
        crate::trace::drop_hidden_duplicates(&mut f);
        (f, pc)
    };
    let _ = &mut fields;
    FieldsOut { fields, symmetry_applied, text_regions, primitives_count, micro_removed, sym_plan, tp }
}

/// Thin lines on top: as strokes (centerline + measured width) and, for the "shapes" export, as outlines.
fn trace_lines(b: &Base, params: &Params, k: Knobs, fo: &FieldsOut) -> LinesOut {
    let (w, h) = (b.prep.src.width() as usize, b.prep.src.height() as usize);
    let up = b.prep.up.max(1);
    let (ww, wh, s, scale, t) = (b.ww, b.wh, b.s, b.scale, k.t);
    let mut out = LinesOut { line_shapes: Vec::new(), line_fallback: Vec::new(), strokes: Vec::new(), cut_thickened: 0 };
    let copts = CenterOpts { merge: true, width_tol: 0.75 * scale };
    let Some(tm) = b.thin else { return out };
    let dark = crate::lines::dark_mask(&b.prep.src, &ThinParams::for_size(w as u32, h as u32));
    let (assign_c, colors) = line_colors(tm, 3);
    let min_line = ((tm.comps.iter().map(|c| c.area).min().unwrap_or(8) as f64) * (up * up) as f64 * 0.5) as usize;
    let lp = TraceParams { sigma: lerp(0.6, 1.0, t) * s, tol: lerp(0.5, 1.5, t) * s * k.extra, min_area: min_line, ..fo.tp.clone() };
    let stroke_tol = lerp(0.7, 2.0, t) * s * k.extra;
    let is_cut = params.production.mode == ProductionMode::CutEmbroidery;
    let is_screen = params.production.mode == ProductionMode::ScreenPrint;
    let cut_min_px = if is_cut { params.production.effective_min_stroke_pt() * 96.0 / 72.0 * scale } else { 0.0 };

    for (ci, raw_color) in colors.iter().enumerate() {
        let color = if is_screen { nearest_in_palette(*raw_color, &b.palette) } else { *raw_color };
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
        let cl = centerline_opts(&big, ww, wh, stroke_tol, 0.5, &copts);
        // In CutEmbroidery mode, thicken thin-line masks below min_stroke_pt before tracing closed shapes.
        let shape_mask_for_cut = if is_cut && cut_min_px > 0.0 {
            let min_w = cl
                .strokes
                .iter()
                .map(|st| st.width + STROKE_WIDTH_BIAS * scale)
                .fold(f64::INFINITY, f64::min);
            if min_w.is_finite() && min_w + 1e-6 < cut_min_px {
                let r = ((cut_min_px - min_w) / 2.0).ceil().clamp(1.0, 6.0) as i32;
                out.cut_thickened += cl.strokes.iter().filter(|st| st.width + STROKE_WIDTH_BIAS * scale + 1e-6 < cut_min_px).count();
                Some(dilate_mask(&big, ww, wh, r))
            } else {
                None
            }
        } else {
            None
        };
        let shape_src = shape_mask_for_cut.as_deref().unwrap_or(&big);
        let contours = trace_mask(&to_binary(shape_src, ww, wh), &lp);
        if !contours.is_empty() {
            out.line_shapes.push(Layer { id: crate::hex(color), color, contours, canvas: false, prims: vec![] });
        }
        if cl.shape_mask.iter().any(|&b| b) {
            let contours = trace_mask(&to_binary(&cl.shape_mask, ww, wh), &lp);
            if !contours.is_empty() {
                out.line_fallback.push(Layer { id: crate::hex(color), color, contours, canvas: false, prims: vec![] });
            }
        }
        let min_len = MIN_STROKE_LEN * ww as f64 / w as f64;
        let kept: Vec<_> = cl.strokes.iter().filter(|st| { let mut l = 0.0; for cv in &st.curves { l += ((cv[3].0 - cv[0].0).powi(2) + (cv[3].1 - cv[0].1).powi(2)).sqrt(); } l >= min_len }).collect();
        if !kept.is_empty() {
            let mut layer_strokes: Vec<StrokeData> = kept
                .iter()
                .map(|st| {
                    let mut d = StrokeData::from(*st);
                    d.width += STROKE_WIDTH_BIAS * scale;
                    // Recover clipped outer anti-aliased ink width only where dark_mask confirms dark pixels outside current half-width.
                    let half = d.width * 0.5;
                    let mut outer_dark = 0usize;
                    let mut outer_tot = 0usize;
                    let step = (d.curves.len() / 6).max(1);
                    for cv in d.curves.iter().step_by(step) {
                        let mx = 0.125 * cv[0].0 + 0.375 * cv[1].0 + 0.375 * cv[2].0 + 0.125 * cv[3].0;
                        let my = 0.125 * cv[0].1 + 0.375 * cv[1].1 + 0.375 * cv[2].1 + 0.125 * cv[3].1;
                        let tx = cv[3].0 - cv[0].0;
                        let ty = cv[3].1 - cv[0].1;
                        let tl = (tx * tx + ty * ty).sqrt();
                        if tl < 1e-3 { continue; }
                        let (nx, ny) = (-ty / tl, tx / tl);
                        for side in [-1.0f64, 1.0] {
                            let sx = ((mx + nx * half * side) / scale).round() as i32;
                            let sy = ((my + ny * half * side) / scale).round() as i32;
                            if sx >= 0 && sy >= 0 && (sx as usize) < w && (sy as usize) < h {
                                outer_tot += 1;
                                if dark[sy as usize * w + sx as usize] {
                                    outer_dark += 1;
                                }
                            }
                        }
                    }
                    if d.width / scale >= 3.3 && outer_tot > 0 && outer_dark * 5 >= outer_tot {
                        let frac = (outer_dark as f64 / outer_tot as f64).clamp(0.20, 1.0);
                        d.width += (1.20 + 0.45 * (frac - 0.20) / 0.80) * scale;
                    }
                    d
                })
                .collect();
            crate::centerline::join_stroke_endpoints(&mut layer_strokes, &dark, w, h, scale);
            out.strokes.push(StrokeLayer { color, strokes: layer_strokes });
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

fn count_nodes(doc: &Doc, exp: &ExportOptions) -> usize {
    let mut e = exp.clone();
    e.registration_marks = false;
    count_svg(&to_svg(doc, &e)).1
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

/// The second half: palette, brush zones, tracing, lines, production profile rules, path edits, and (with `target_nodes`) the node budget.
pub fn vectorize_prepared(prep: &PreparedImage, params: &Params, progress: Progress) -> Result<Output> {
    let mut notes = prep.notes.clone();
    let exp_opts = export_options_for_production(&params.production);

    // Dedicated Centerline Mode (pure single-line open Bézier strokes, 0 filled regions).
    if params.production.mode == ProductionMode::Centerline {
        progress("Extracting centerline strokes", 0.6);
        let (w, h) = (prep.src.width() as usize, prep.src.height() as usize);
        let (ww, wh) = (prep.cleaned.width() as usize, prep.cleaned.height() as usize);
        let scale = ww as f64 / w.max(1) as f64;
        let fg_work = prep.fg_mask.as_ref().map(|fg| upscale_bool(fg, w, h, ww, wh));
        let stroke_layer = extract_full_centerline(
            &prep.cleaned,
            fg_work.as_deref(),
            params.production.centerline_invert,
            params.production.centerline_stroke_pt,
            scale,
            params.smooth,
        );
        let ink_color = stroke_layer.color;
        let strokes = if stroke_layer.strokes.is_empty() {
            Vec::new()
        } else {
            vec![stroke_layer]
        };
        let doc = Doc {
            width: prep.src.width(),
            height: prep.src.height(),
            scale,
            fields: Vec::new(),
            line_shapes: Vec::new(),
            line_fallback: Vec::new(),
            strokes,
        };
        let (doc, trace_fp, edits_stale) = apply_edits_guarded(doc, params);
        progress("Writing SVG", 0.95);
        let svg = to_svg(&doc, &exp_opts);
        let (paths, nodes) = count_svg(&svg);
        let mae = if params.measure {
            render_svg(&svg, prep.src.width(), prep.src.height())
                .ok()
                .map(|r| compare(&prep.src, &r).0)
        } else {
            None
        };
        let stats = Stats { paths, nodes, bytes: svg.len(), mae, primitives: count_primitives(&svg) };
        let heatmap = node_heatmap(&doc);
        let palette = if doc.strokes.is_empty() { Vec::new() } else { vec![ink_color] };
        progress("Done", 1.0);
        return Ok(Output {
            svg,
            doc,
            stats,
            palette,
            notes,
            used_ai: prep.used_ai,
            thin_pixels: 0,
            primitives_count: 0,
            symmetry_applied: None,
            text_regions: Vec::new(),
            heatmap,
            effective_smooth: params.smooth.clamp(0.0, 1.0),
            budget_met: None,
            trace_fingerprint: trace_fp,
            edits_stale,
        });
    }

    progress("Locking the palette", 0.55);
    let b = base(prep, params, &mut notes);
    let t0 = params.smooth.clamp(0.0, 1.0) as f64;

    progress("Tracing", 0.7);
    let mut knobs = Knobs { t: t0, extra: 1.0 };
    let mut fo = trace_fields(&b, params, knobs, true);
    let mut lines = trace_lines(&b, params, knobs, &fo);
    let mut budget_met = None;
    if let Some(limit) = params.target_nodes.map(|n| n.max(1)) {
        let first = count_nodes(&make_doc(&b, fo.fields.clone(), &lines), &exp_opts);
        if first <= limit {
            budget_met = Some(true);
        } else {
            progress("Fitting the node budget", 0.8);
            let nodes_at = |p: f64| -> (Knobs, FieldsOut, usize) {
                let k = knobs_at(t0, p);
                let f = trace_fields(&b, params, k, false);
                let n = count_nodes(&make_doc(&b, f.fields.clone(), &lines), &exp_opts);
                (k, f, n)
            };
            let (kmax, fmax, nmax) = nodes_at(1.0);
            let lines_max = trace_lines(&b, params, kmax, &fmax);
            let real_max = count_nodes(&make_doc(&b, fmax.fields.clone(), &lines_max), &exp_opts);
            let bias = nmax.saturating_sub(real_max);
            if real_max > limit {
                knobs = kmax;
                fo = fmax;
                budget_met = Some(false);
            } else {
                let (mut lo, mut hi) = (0.0f64, 1.0f64);
                let mut kbest = kmax;
                let mut fbest: Option<FieldsOut> = None;
                for _ in 0..BUDGET_STEPS {
                    let mid = (lo + hi) / 2.0;
                    let (k, f, n) = nodes_at(mid);
                    let est_bias = (bias as f64 * mid).round() as usize;
                    if n.saturating_sub(est_bias) <= limit {
                        hi = mid;
                        kbest = k;
                        fbest = Some(f);
                    } else {
                        lo = mid;
                    }
                }
                knobs = kbest;
                fo = fbest.unwrap_or(fmax);
                budget_met = Some(true);
            }
            if params.detect_text {
                fo.text_regions = trace_fields(&b, params, Knobs { t: knobs.t, extra: 1.0 }, true).text_regions;
            }
            let at_max = (knobs.t - kmax.t).abs() < 1e-12 && (knobs.extra - kmax.extra).abs() < 1e-12;
            lines = if at_max { lines_max } else {
                let l = trace_lines(&b, params, knobs, &fo);
                if count_nodes(&make_doc(&b, fo.fields.clone(), &l), &exp_opts) > limit && real_max <= limit {
                    knobs = kmax;
                    fo = trace_fields(&b, params, kmax, params.detect_text);
                    lines_max
                } else {
                    l
                }
            };
            let final_nodes = count_nodes(&make_doc(&b, fo.fields.clone(), &lines), &exp_opts);
            if final_nodes > limit {
                budget_met = Some(false);
            }
            if budget_met == Some(false) {
                notes.push(format!("Target <= {limit} nodes is too low for the detail of this picture; the best safe result has {final_nodes} nodes."));
            }
        }
    }

    progress("Writing SVG", 0.95);
    let FieldsOut { fields, symmetry_applied, text_regions, primitives_count, micro_removed, .. } = fo;
    let mut doc = make_doc(&b, fields, &lines);

    // Enforce minimum stroke width in production profiles (Standard = 0.0 -> no-op).
    let min_stroke_pt = params.production.effective_min_stroke_pt();
    let thickened = enforce_min_stroke(&mut doc, min_stroke_pt);
    let total_thickened = if params.production.mode == ProductionMode::CutEmbroidery {
        lines.cut_thickened.max(thickened)
    } else {
        thickened
    };
    if total_thickened > 0 {
        notes.push(format!("{total_thickened} garis ditebalkan ke {:.1} pt.", min_stroke_pt));
    }
    if micro_removed > 0 {
        notes.push(format!("{micro_removed} bintik mikro dibuang."));
    }

    // Apply non-destructive per-path and node edits before stats, SVG, and export (skipped when they belong to another trace).
    let (doc, trace_fp, edits_stale) = apply_edits_guarded(doc, params);

    let svg = to_svg(&doc, &exp_opts);
    // Measure MAE without registration margins so the comparison aligns pixel-for-pixel with the source.
    let (paths, nodes) = count_svg(&svg);
    let mae = if params.measure {
        let mae_svg = if exp_opts.registration_marks {
            to_svg(&doc, &ExportOptions { registration_marks: false, ..exp_opts.clone() })
        } else {
            svg.clone()
        };
        render_svg(&mae_svg, prep.src.width(), prep.src.height())
            .ok()
            .map(|r| compare(&prep.src, &r).0)
    } else {
        None
    };
    let stats = Stats { paths, nodes, bytes: svg.len(), mae, primitives: count_primitives(&svg) };
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
        trace_fingerprint: trace_fp,
        edits_stale,
    })
}
/// Turn an imported `.svg` (see `svg_import`) into an `Output` with the same edits and statistics as a traced drawing:
/// `path_edits` and `node_edits` are applied, then the SVG, node counts and node heatmap are produced. No raster
/// pipeline runs, so there is no MAE (`stats.mae == None`).
pub fn vectorize_svg(import: crate::svg_import::SvgImport, params: &Params, exp_opts: &ExportOptions) -> Output {
    let crate::svg_import::SvgImport { doc, notes } = import;
    let (doc, trace_fp, edits_stale) = apply_edits_guarded(doc, params);
    let svg = to_svg(&doc, exp_opts);
    let (paths, nodes) = count_svg(&svg);
    let stats = Stats { paths, nodes, bytes: svg.len(), mae: None, primitives: count_primitives(&svg) };
    let heatmap = node_heatmap(&doc);
    let mut palette: Vec<Rgb> = doc.fields.iter().map(|l| l.color).collect();
    for sl in &doc.strokes {
        if !palette.contains(&sl.color) {
            palette.push(sl.color);
        }
    }
    Output {
        svg,
        doc,
        stats,
        palette,
        notes,
        used_ai: false,
        thin_pixels: 0,
        primitives_count: 0,
        symmetry_applied: None,
        text_regions: Vec::new(),
        heatmap,
        effective_smooth: params.smooth,
        budget_met: None,
        trace_fingerprint: trace_fp,
        edits_stale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t170_e2e_flat_illustration_with_aa_thin_outline() {
        // Synthetic 4-colour flat illustration (240x240):
        // Cream background (#FCF7EC), red circle/bag body (#B53B3B), olive accent panel (#89995A),
        // and dark green 2-3 px anti-aliased outline (#1F4D36).
        let (w, h) = (240u32, 240u32);
        let cream = [0xFCu8, 0xF7, 0xEC];
        let red = [0xB5u8, 0x3B, 0x3B];
        let olive = [0x89u8, 0x99, 0x5A];
        let dark = [0x1Fu8, 0x4D, 0x36];
        let blend = |a: [u8; 3], b: [u8; 3], t: f32| -> image::Rgb<u8> {
            image::Rgb([
                (a[0] as f32 * (1.0 - t) + b[0] as f32 * t).round() as u8,
                (a[1] as f32 * (1.0 - t) + b[1] as f32 * t).round() as u8,
                (a[2] as f32 * (1.0 - t) + b[2] as f32 * t).round() as u8,
            ])
        };

        let mut img = RgbImage::from_pixel(w, h, image::Rgb(cream));

        // Red rounded body (x in 55..185, y in 50..195) with 2-px dark outline + 1-px AA rim
        for y in 46..199 {
            for x in 51..189 {
                let in_outer = x >= 53 && x < 187 && y >= 48 && y < 197;
                let in_inner = x >= 56 && x < 184 && y >= 51 && y < 194;
                if in_inner {
                    img.put_pixel(x, y, image::Rgb(red));
                } else if in_outer {
                    img.put_pixel(x, y, image::Rgb(dark));
                } else {
                    // 1-px anti-alias halo rim on outer edge
                    img.put_pixel(x, y, blend(dark, cream, 0.55));
                }
            }
        }

        // Olive pocket panel inside the body (x in 78..162, y in 118..172) with 2-px dark outline
        for y in 115..175 {
            for x in 75..165 {
                let in_outer = x >= 76 && x < 164 && y >= 116 && y < 174;
                let in_inner = x >= 79 && x < 161 && y >= 119 && y < 171;
                if in_inner {
                    img.put_pixel(x, y, image::Rgb(olive));
                } else if in_outer {
                    img.put_pixel(x, y, image::Rgb(dark));
                } else {
                    img.put_pixel(x, y, blend(dark, red, 0.5));
                }
            }
        }

        // Thin 2-px diagonal strap extending into cream background with 1-px AA border
        for i in 0..45i32 {
            let cx = 186 + i;
            let cy = 80 - i / 2;
            for dy in -1..=2i32 {
                let yy = cy + dy;
                if cx >= 0 && yy >= 0 && (cx as u32) < w && (yy as u32) < h {
                    if dy == 0 || dy == 1 {
                        img.put_pixel(cx as u32, yy as u32, image::Rgb(dark));
                    } else {
                        img.put_pixel(cx as u32, yy as u32, blend(dark, cream, 0.55));
                    }
                }
            }
        }

        let params = Params {
            colors: 4,
            smooth: 0.5,
            keep_lines: true,
            measure: true,
            ..Params::default()
        };
        let out = vectorize(&img, &params, &|_, _| {}).expect("vectorize should succeed");
        assert!(out.svg.contains("<svg") && out.svg.contains("</svg>"), "SVG must be valid");
        let mae = out.stats.mae.expect("MAE must be measured");
        eprintln!(
            "T170_E2E_METRICS: paths={} nodes={} mae={:.3} thin_pixels={} palette={}",
            out.stats.paths,
            out.stats.nodes,
            mae,
            out.thin_pixels,
            out.palette.len()
        );
        assert!(out.thin_pixels > 0, "thin outline should be detected");
        assert!(out.stats.paths <= 60, "expected compact path count, got {}", out.stats.paths);
        assert!(out.stats.nodes <= 400, "expected compact node count, got {}", out.stats.nodes);
        assert!(mae <= 12.0, "expected low MAE <= 12.0, got {:.3}", mae);
    }
}
