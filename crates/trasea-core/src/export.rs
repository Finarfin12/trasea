//! Export: SVG with named layers (`Latar` > `Bidang warna` > `Garis` > optional `CutContour` > optional `Registrasi`)
//! and deterministic `data-trasea-id` attributes, per-colour black separation films (`to_separation_svgs`,
//! `to_separation_pdfs`, `to_separation_eps`), PDF 1.4 (RGB via svg2pdf, CMYK & Named Spot `/Separation` via native
//! PDF 1.4 writer), EPSF-3.0 (RGB, CMYK, & Named Spot `%%CMYKCustomColor`), DXF (via `dxf.rs`),
//! and Adobe Swatch Exchange (`.ase` in RGB or CMYK, with Global & Spot `0x0001` colour types).

use crate::centerline::Stroke;
use crate::color::{
    eps_string_escape, lookup_spot_name, parse_hex_rgb, pdf_name_encode, rgb_arr_to_cmyk,
    sanitize_spot_name, ColorSpace, SpotColorMapping,
};
use crate::contour::{build_die_cut_contours, build_sticker_backing_contours, die_cut_bounds_svg};
use crate::dxf::DxfUnit;
use crate::path_edit::group_contours_into_compounds;
use crate::primitives::Primitive;
use crate::production::{ProductionMode, ProductionOptions};
use crate::trace::{Contour, Layer};
use crate::{hex, Error, Result, Rgb};
use serde::{Deserialize, Serialize};

pub use crate::dxf::to_dxf;

/// Outer margin (px) added on every side when registration marks are enabled.
pub const REG_MARGIN: f64 = 28.0;

/// Default magenta preview colour for `CutContour` (`CMYK 0/100/0/0`).
pub const DIE_CUT_RGB: Rgb = [236, 0, 140];
pub const DIE_CUT_CMYK: [f32; 4] = [0.0, 1.0, 0.0, 0.0];
/// Default die-cut stroke width in points (QT-2: 0.25 pt).
pub const DIE_CUT_STROKE_PT: f64 = 0.25;

fn default_die_cut_mm() -> f64 {
    2.0
}

fn default_die_cut_spot_name() -> String {
    "CutContour".to_string()
}

fn default_true() -> bool {
    true
}

fn default_sticker_backing_color() -> String {
    "#FFFFFF".to_string()
}

fn sticker_backing_rgb(opts: &ExportOptions) -> Rgb {
    parse_hex_rgb(&opts.sticker_backing_color).unwrap_or([255, 255, 255])
}

/// Thin lines of one colour, as strokes (centerline + width), in working pixels.
#[derive(Debug, Clone)]
pub struct StrokeLayer {
    pub color: Rgb,
    pub strokes: Vec<StrokeData>,
}

#[derive(Debug, Clone)]
pub struct StrokeData {
    pub curves: Vec<[(f64, f64); 4]>,
    /// Width in working pixels.
    pub width: f64,
}

impl From<&Stroke> for StrokeData {
    fn from(s: &Stroke) -> Self {
        StrokeData { curves: s.curves.clone(), width: s.width }
    }
}

/// The traced drawing. Coordinates inside the layers are working pixels; `scale` = working / source size.
#[derive(Debug, Clone)]
pub struct Doc {
    pub width: u32,
    pub height: u32,
    pub scale: f64,
    /// Colour fields, bottom to top (the first one is the background rectangle when `canvas == true`).
    pub fields: Vec<Layer>,
    /// ALL thin lines as filled outlines (used when the export says "lines as shapes").
    pub line_shapes: Vec<Layer>,
    /// Thin lines whose width is too uneven for a stroke: they stay outlines even in stroke mode.
    pub line_fallback: Vec<Layer>,
    /// Thin lines as strokes.
    pub strokes: Vec<StrokeLayer>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinesAs {
    #[default]
    Stroke,
    Shape,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportOptions {
    #[serde(default)]
    pub lines_as: LinesAs,
    /// Print trap in points (0 = none). Colour fields get an outline of their own colour this wide.
    #[serde(default)]
    pub trap_pt: f64,
    /// Add 4 registration targets in an outer margin on a `Registrasi` layer.
    #[serde(default)]
    pub registration_marks: bool,
    /// Omit the background layer (`canvas == true`).
    #[serde(default)]
    pub strip_background: bool,
    /// Output colour space for PDF, EPS, and ASE exports (`Rgb` default, or `Cmyk`).
    #[serde(default)]
    pub color_space: ColorSpace,
    /// Physical unit for DXF export (`Mm` default at 96 DPI, or `Px`).
    #[serde(default)]
    pub dxf_unit: DxfUnit,
    /// Optional target physical width in mm for DXF export (overrides 96 DPI default when `Mm`).
    #[serde(default)]
    pub dxf_target_width_mm: Option<f32>,
    /// Emit an automatic outer Die-Cut / Kiss-Cut contour (`CutContour`).
    #[serde(default)]
    pub die_cut_contour: bool,
    /// Outer offset distance for the die-cut contour in mm (`0.5..=10.0`, default `2.0`).
    #[serde(default = "default_die_cut_mm")]
    pub die_cut_offset_mm: f64,
    /// Spot ink name for the die-cut contour (default `"CutContour"`).
    #[serde(default = "default_die_cut_spot_name")]
    pub die_cut_spot_name: String,
    /// Bridge nearby islands into a single smooth die-cut contour (default `true`).
    #[serde(default = "default_true")]
    pub die_cut_bridge_islands: bool,
    /// Named RIP spot colour mappings (e.g. `#FF0000 -> "Spot_Red"`).
    #[serde(default)]
    pub spot_colors: Vec<SpotColorMapping>,
    /// Emit a solid sticker backing layer beneath `Bidang warna` concentric with `CutContour`.
    #[serde(default)]
    pub sticker_backing: bool,
    /// Fill hex colour for `StickerBacking` (default `"#FFFFFF"`).
    #[serde(default = "default_sticker_backing_color")]
    pub sticker_backing_color: String,
    /// Additional bleed outside `CutContour` in mm (`0.0..=3.0`, default `0.0`).
    #[serde(default)]
    pub sticker_bleed_mm: f64,
}

impl Default for ExportOptions {
    fn default() -> Self {
        ExportOptions {
            lines_as: LinesAs::Stroke,
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: false,
            color_space: ColorSpace::Rgb,
            dxf_unit: DxfUnit::Mm,
            dxf_target_width_mm: None,
            die_cut_contour: false,
            die_cut_offset_mm: default_die_cut_mm(),
            die_cut_spot_name: default_die_cut_spot_name(),
            die_cut_bridge_islands: true,
            spot_colors: Vec::new(),
            sticker_backing: false,
            sticker_backing_color: default_sticker_backing_color(),
            sticker_bleed_mm: 0.0,
        }
    }
}

/// Default `ExportOptions` matching a production profile.
pub fn export_options_for_production(prod: &ProductionOptions) -> ExportOptions {
    match prod.mode {
        ProductionMode::Standard | ProductionMode::Shading => ExportOptions::default(),
        ProductionMode::ScreenPrint => ExportOptions {
            lines_as: LinesAs::Stroke,
            trap_pt: 0.5,
            registration_marks: prod.registration_marks,
            strip_background: prod.strip_background,
            ..ExportOptions::default()
        },
        ProductionMode::CutEmbroidery => ExportOptions {
            lines_as: LinesAs::Shape,
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: prod.strip_background,
            ..ExportOptions::default()
        },
        ProductionMode::Centerline => ExportOptions {
            lines_as: LinesAs::Stroke,
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: true,
            ..ExportOptions::default()
        },
    }
}

/// One 100%-black (`#000000` / K=100%) screen-print separation film for a single spot colour.
#[derive(Debug, Clone)]
pub struct SeparationFilm {
    /// 1-based film number.
    pub index: usize,
    pub color: Rgb,
    /// File name suffix, e.g. `film01_0F3973`.
    pub suffix: String,
    pub svg: String,
}

fn num(v: f64) -> String {
    let s = format!("{:.2}", v);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

fn pt(p: (f64, f64), k: f64) -> String {
    format!("{},{}", num(p.0 / k), num(p.1 / k))
}

/// A cubic whose handles lie on the chord is a straight segment: write it as `L`, which is what a designer expects.
fn is_straight(p0: (f64, f64), c1: (f64, f64), c2: (f64, f64), p3: (f64, f64), k: f64) -> bool {
    let (dx, dy) = (p3.0 - p0.0, p3.1 - p0.1);
    let l = (dx * dx + dy * dy).sqrt();
    if l < 1e-9 {
        return false;
    }
    let tol = 0.03 * k;
    [c1, c2].iter().all(|c| {
        let along = ((c.0 - p0.0) * dx + (c.1 - p0.1) * dy) / l;
        ((dx * (c.1 - p0.1) - dy * (c.0 - p0.0)).abs() / l) <= tol && along >= -tol && along <= l + tol
    })
}

fn path_d(contours: &[Contour], k: f64) -> String {
    let mut d = String::new();
    for c in contours {
        d.push('M');
        d.push_str(&pt(c.start, k));
        let mut prev = c.start;
        for cv in &c.curves {
            if is_straight(prev, cv[0], cv[1], cv[2], k) {
                d.push('L');
                d.push_str(&pt(cv[2], k));
            } else {
                d.push('C');
                d.push_str(&pt(cv[0], k));
                d.push(' ');
                d.push_str(&pt(cv[1], k));
                d.push(' ');
                d.push_str(&pt(cv[2], k));
            }
            prev = cv[2];
        }
        d.push('Z');
    }
    d
}

fn path_d_indexed(contours: &[Contour], indices: &[usize], k: f64) -> String {
    let mut d = String::new();
    for &idx in indices {
        let c = &contours[idx];
        d.push('M');
        d.push_str(&pt(c.start, k));
        let mut prev = c.start;
        for cv in &c.curves {
            if is_straight(prev, cv[0], cv[1], cv[2], k) {
                d.push('L');
                d.push_str(&pt(cv[2], k));
            } else {
                d.push('C');
                d.push_str(&pt(cv[0], k));
                d.push(' ');
                d.push_str(&pt(cv[1], k));
                d.push(' ');
                d.push_str(&pt(cv[2], k));
            }
            prev = cv[2];
        }
        d.push('Z');
    }
    d
}

fn stroke_d(s: &StrokeData, k: f64) -> String {
    let mut d = String::from("M");
    d.push_str(&pt(s.curves[0][0], k));
    for cv in &s.curves {
        if is_straight(cv[0], cv[1], cv[2], cv[3], k) {
            d.push('L');
            d.push_str(&pt(cv[3], k));
        } else {
            d.push('C');
            d.push_str(&pt(cv[1], k));
            d.push(' ');
            d.push_str(&pt(cv[2], k));
            d.push(' ');
            d.push_str(&pt(cv[3], k));
        }
    }
    d
}

/// SVG element of a recognised shape. `extra` carries optional trap stroke and `data-trasea-id` attributes.
fn primitive_svg(p: &Primitive, color: &str, extra: &str, k: f64) -> String {
    let rot = |a: f64, cx: f64, cy: f64| if a.abs() > 0.5 { format!(" transform=\"rotate({} {} {})\"", num(a), num(cx / k), num(cy / k)) } else { String::new() };
    match p {
        Primitive::Circle { cx, cy, r } => format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\"{extra}/>\n", num(cx / k), num(cy / k), num(r / k)),
        Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
            format!("<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" fill=\"{color}\"{extra}{}/>\n", num(cx / k), num(cy / k), num(rx / k), num(ry / k), rot(*angle_deg, *cx, *cy))
        }
        Primitive::Rect { x, y, w, h, rx, angle_deg } => {
            let round = if *rx > 0.0 { format!(" rx=\"{}\"", num(rx / k)) } else { String::new() };
            format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"{round} fill=\"{color}\"{extra}{}/>\n", num(x / k), num(y / k), num(w / k), num(h / k), rot(*angle_deg, x + w / 2.0, y + h / 2.0))
        }
    }
}

/// Four standard crosshair+circle registration marks in the outer margin.
fn registration_marks_svg(w: f64, h: f64, m: f64) -> String {
    let pts = [
        (m / 2.0, m / 2.0),
        (w + 1.5 * m, m / 2.0),
        (m / 2.0, h + 1.5 * m),
        (w + 1.5 * m, h + 1.5 * m),
    ];
    let mut s = String::from("<g id=\"Registrasi\" data-name=\"Registrasi\">\n");
    for (cx, cy) in pts {
        s.push_str(&format!(
            "<circle cx=\"{}\" cy=\"{}\" r=\"6\" fill=\"none\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx),
            num(cy)
        ));
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx - 9.0),
            num(cy),
            num(cx + 9.0),
            num(cy)
        ));
        s.push_str(&format!(
            "<line x1=\"{}\" y1=\"{}\" x2=\"{}\" y2=\"{}\" stroke=\"#000000\" stroke-width=\"0.75\"/>\n",
            num(cx),
            num(cy - 9.0),
            num(cx),
            num(cy + 9.0)
        ));
    }
    s.push_str("</g>\n");
    s
}

/// Colours of the drawing in stacking order (bottom first): the order of the palette numbers (01, 02, ...).
pub fn palette_of(doc: &Doc) -> Vec<Rgb> {
    doc.fields.iter().map(|l| l.color).collect()
}

fn group_names(index: usize, color: Rgb) -> (String, String) {
    let h = hex(color);
    (format!("Warna_{:02}_{}", index, &h[1..]), format!("Warna {:02} {}", index, h))
}

fn sanitize_svg_id(name: &str) -> String {
    let clean = sanitize_spot_name(name).replace(' ', "_");
    if clean.is_empty() {
        "CutContour".to_string()
    } else {
        clean
    }
}

pub fn to_svg(doc: &Doc, opts: &ExportOptions) -> String {
    let k = doc.scale;
    let die_cut_contours = if opts.die_cut_contour {
        build_die_cut_contours(doc, opts)
    } else {
        Vec::new()
    };
    let backing_contours = build_sticker_backing_contours(doc, opts);
    let (dc_min_x, dc_min_y, dc_max_x, dc_max_y) = if die_cut_contours.is_empty() && backing_contours.is_empty() {
        (0.0, 0.0, doc.width as f64, doc.height as f64)
    } else if backing_contours.is_empty() {
        die_cut_bounds_svg(doc, &die_cut_contours)
    } else {
        let mut all = Vec::with_capacity(die_cut_contours.len() + backing_contours.len());
        all.extend_from_slice(&die_cut_contours);
        all.extend_from_slice(&backing_contours);
        die_cut_bounds_svg(doc, &all)
    };
    let content_w = dc_max_x - dc_min_x;
    let content_h = dc_max_y - dc_min_y;
    let shift_x = (-dc_min_x).max(0.0);
    let shift_y = (-dc_min_y).max(0.0);
    let reg_m = if opts.registration_marks { REG_MARGIN } else { 0.0 };
    let total_tx = shift_x + reg_m;
    let total_ty = shift_y + reg_m;
    let needs_transform = total_tx.abs() > 1e-6 || total_ty.abs() > 1e-6;

    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    if opts.registration_marks || !die_cut_contours.is_empty() {
        let vw = content_w + 2.0 * reg_m;
        let vh = content_h + 2.0 * reg_m;
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{vw}\" height=\"{vh}\" viewBox=\"0 0 {vw} {vh}\">\n",
            vw = num(vw),
            vh = num(vh)
        ));
        if needs_transform {
            s.push_str(&format!("<g transform=\"translate({}, {})\">\n", num(total_tx), num(total_ty)));
        }
    } else {
        s.push_str(&format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\">\n",
            w = doc.width,
            h = doc.height
        ));
    }

    // trap: a stroke of the fill colour makes every field grow by half of this width on each side
    let trap_px = opts.trap_pt * 96.0 / 72.0;
    let field = |layer: &Layer, field_idx: usize, index: usize, with_trap: bool, out: &mut String| {
        let (id, name) = group_names(index, layer.color);
        let color = hex(layer.color);
        let trap = if with_trap && trap_px > 0.0 {
            format!(" stroke=\"{color}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", num(trap_px))
        } else {
            String::new()
        };
        out.push_str(&format!("<g id=\"{id}\" data-name=\"{name}\">\n"));
        if !layer.contours.is_empty() {
            let compounds = group_contours_into_compounds(&layer.contours);
            for (ci, members) in compounds.iter().enumerate() {
                out.push_str(&format!(
                    "<path data-trasea-id=\"f:{field_idx}:{ci}\" fill=\"{color}\" fill-rule=\"evenodd\"{trap} d=\"{}\"/>\n",
                    path_d_indexed(&layer.contours, members, k)
                ));
            }
        } else if layer.prims.is_empty() {
            out.push_str(&format!("<path data-trasea-id=\"f:{field_idx}:0\" fill=\"{color}\" fill-rule=\"evenodd\"{trap} d=\"\"/>\n"));
        }
        for (pi, p) in layer.prims.iter().enumerate() {
            let extra = format!(" data-trasea-id=\"fp:{field_idx}:{pi}\"{trap}");
            out.push_str(&primitive_svg(p, &color, &extra, k));
        }
        out.push_str("</g>\n");
    };

    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    let mut idx = 1;
    // 1. Latar: background layer (omitted when strip_background is active or no canvas layer exists)
    if !opts.strip_background && has_canvas_bg {
        s.push_str("<g id=\"Latar\" data-name=\"Latar\">\n");
        field(&doc.fields[0], 0, idx, false, &mut s);
        idx += 1;
        s.push_str("</g>\n");
    } else if !opts.strip_background && !has_canvas_bg && doc.fields.is_empty() {
        s.push_str("<g id=\"Latar\" data-name=\"Latar\">\n</g>\n");
    }
    // 1b. Optional StickerBacking layer (solid fill beneath Bidang warna)
    if !backing_contours.is_empty() {
        let bg_hex = hex(sticker_backing_rgb(opts));
        s.push_str("<g id=\"StickerBacking\" data-name=\"StickerBacking\">\n");
        for c in &backing_contours {
            s.push_str(&format!(
                "<path fill=\"{bg_hex}\" fill-rule=\"evenodd\" d=\"{}\"/>\n",
                path_d(std::slice::from_ref(c), k)
            ));
        }
        s.push_str("</g>\n");
    }
    // 2. Bidang warna: one sub-group per colour
    s.push_str("<g id=\"Bidang_warna\" data-name=\"Bidang warna\">\n");
    let skip_first = if has_canvas_bg { 1 } else { 0 };
    for (fi, layer) in doc.fields.iter().enumerate().skip(skip_first) {
        field(layer, fi, idx, true, &mut s);
        idx += 1;
    }
    s.push_str("</g>\n");
    // 3. Garis: thin lines / centerline strokes on top
    s.push_str("<g id=\"Garis\" data-name=\"Garis\">\n");
    match opts.lines_as {
        LinesAs::Shape => {
            for (li, l) in doc.line_shapes.iter().enumerate() {
                let compounds = group_contours_into_compounds(&l.contours);
                for (ci, members) in compounds.iter().enumerate() {
                    s.push_str(&format!(
                        "<path data-trasea-id=\"ls:{li}:{ci}\" fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>\n",
                        hex(l.color),
                        path_d_indexed(&l.contours, members, k)
                    ));
                }
            }
        }
        LinesAs::Stroke => {
            for (li, l) in doc.line_fallback.iter().enumerate() {
                let compounds = group_contours_into_compounds(&l.contours);
                for (ci, members) in compounds.iter().enumerate() {
                    s.push_str(&format!(
                        "<path data-trasea-id=\"lf:{li}:{ci}\" fill=\"{}\" fill-rule=\"evenodd\" d=\"{}\"/>\n",
                        hex(l.color),
                        path_d_indexed(&l.contours, members, k)
                    ));
                }
            }
            for (sli, sl) in doc.strokes.iter().enumerate() {
                let color = hex(sl.color);
                for (si, st) in sl.strokes.iter().enumerate() {
                    if st.curves.is_empty() {
                        continue;
                    }
                    s.push_str(&format!(
                        "<path data-trasea-id=\"s:{sli}:{si}\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{}\"/>\n",
                        num(st.width / k),
                        stroke_d(st, k)
                    ));
                }
            }
        }
    }
    s.push_str("</g>\n");

    // 4. Optional Die-Cut Contour layer (`CutContour`)
    if !die_cut_contours.is_empty() {
        let spot_display = sanitize_spot_name(&opts.die_cut_spot_name);
        let group_id = sanitize_svg_id(&spot_display);
        let cut_hex = hex(DIE_CUT_RGB);
        let stroke_px = DIE_CUT_STROKE_PT * 96.0 / 72.0;
        s.push_str(&format!("<g id=\"{group_id}\" data-name=\"{spot_display}\">\n"));
        for c in &die_cut_contours {
            s.push_str(&format!(
                "<path fill=\"none\" stroke=\"{cut_hex}\" stroke-width=\"{}\" stroke-linejoin=\"round\" d=\"{}\"/>\n",
                num(stroke_px),
                path_d(std::slice::from_ref(c), k)
            ));
        }
        s.push_str("</g>\n");
    }

    if needs_transform {
        s.push_str("</g>\n");
    }
    if opts.registration_marks {
        s.push_str(&registration_marks_svg(content_w, content_h, REG_MARGIN));
    }
    s.push_str("</svg>\n");
    s
}

fn nearest_color_idx(color: Rgb, palette: &[Rgb]) -> Option<usize> {
    if let Some(pos) = palette.iter().position(|&c| c == color) {
        return Some(pos);
    }
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, c)| {
            let dr = color[0] as i32 - c[0] as i32;
            let dg = color[1] as i32 - c[1] as i32;
            let db = color[2] as i32 - c[2] as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|(i, _)| i)
}

/// Build a single-colour black (`#000000`) sub-document for separation film `fi` so SVG, PDF (RGB/CMYK),
/// and EPS (RGB/CMYK) separations share identical geometry and registration coordinates.
fn separation_film_doc(doc: &Doc, opts: &ExportOptions, active_fields: &[&Layer], colors: &[Rgb], fi: usize) -> Doc {
    let layer = active_fields[fi];
    let black: Rgb = [0, 0, 0];
    let mut field_copy = layer.clone();
    field_copy.color = black;
    field_copy.id = "#000000".to_string();

    let mut line_shapes = Vec::new();
    let mut line_fallback = Vec::new();
    let mut strokes = Vec::new();
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                if nearest_color_idx(l.color, colors) == Some(fi) {
                    let mut lc = l.clone();
                    lc.color = black;
                    line_shapes.push(lc);
                }
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                if nearest_color_idx(l.color, colors) == Some(fi) {
                    let mut lc = l.clone();
                    lc.color = black;
                    line_fallback.push(lc);
                }
            }
            for sl in &doc.strokes {
                if nearest_color_idx(sl.color, colors) == Some(fi) {
                    strokes.push(StrokeLayer {
                        color: black,
                        strokes: sl.strokes.clone(),
                    });
                }
            }
        }
    }
    Doc {
        width: doc.width,
        height: doc.height,
        scale: doc.scale,
        fields: vec![field_copy],
        line_shapes,
        line_fallback,
        strokes,
    }
}

/// Generate one 100%-black (`#000000`) separation film SVG per active spot colour, with identical
/// `viewBox`, identical registration mark coordinates, and a bottom-margin film label.
pub fn to_separation_svgs(doc: &Doc, opts: &ExportOptions) -> Vec<SeparationFilm> {
    let k = doc.scale;
    let active_fields: Vec<&Layer> = doc
        .fields
        .iter()
        .filter(|l| !(opts.strip_background && l.canvas))
        .collect();
    if active_fields.is_empty() {
        return Vec::new();
    }
    let colors: Vec<Rgb> = active_fields.iter().map(|l| l.color).collect();
    let total = active_fields.len();
    let vw = doc.width as f64 + 2.0 * REG_MARGIN;
    let vh = doc.height as f64 + 2.0 * REG_MARGIN;
    let trap_px = opts.trap_pt * 96.0 / 72.0;
    let reg = registration_marks_svg(doc.width as f64, doc.height as f64, REG_MARGIN);

    active_fields
        .iter()
        .enumerate()
        .map(|(fi, layer)| {
            let index = fi + 1;
            let h_hex = hex(layer.color);
            let suffix = format!("film{:02}_{}", index, &h_hex[1..]);
            let black = "#000000";
            let trap = if !layer.canvas && trap_px > 0.0 {
                format!(" stroke=\"{black}\" stroke-width=\"{}\" stroke-linejoin=\"round\"", num(trap_px))
            } else {
                String::new()
            };

            let mut s = String::new();
            s.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
            s.push_str(&format!(
                "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{vw}\" height=\"{vh}\" viewBox=\"0 0 {vw} {vh}\">\n",
                vw = num(vw),
                vh = num(vh)
            ));
            s.push_str(&format!("<g transform=\"translate({}, {})\">\n", num(REG_MARGIN), num(REG_MARGIN)));
            s.push_str(&format!("<g id=\"Film_{:02}_{}\" data-name=\"Film {:02} {}\">\n", index, &h_hex[1..], index, h_hex));
            if !layer.contours.is_empty() || layer.prims.is_empty() {
                s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\"{trap} d=\"{}\"/>\n", path_d(&layer.contours, k)));
            }
            for p in &layer.prims {
                s.push_str(&primitive_svg(p, black, &trap, k));
            }
            match opts.lines_as {
                LinesAs::Shape => {
                    for l in &doc.line_shapes {
                        if nearest_color_idx(l.color, &colors) == Some(fi) {
                            s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", path_d(&l.contours, k)));
                        }
                    }
                }
                LinesAs::Stroke => {
                    for l in &doc.line_fallback {
                        if nearest_color_idx(l.color, &colors) == Some(fi) {
                            s.push_str(&format!("<path fill=\"{black}\" fill-rule=\"evenodd\" d=\"{}\"/>\n", path_d(&l.contours, k)));
                        }
                    }
                    for sl in &doc.strokes {
                        if nearest_color_idx(sl.color, &colors) == Some(fi) {
                            for st in &sl.strokes {
                                if st.curves.is_empty() {
                                    continue;
                                }
                                s.push_str(&format!(
                                    "<path fill=\"none\" stroke=\"{black}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\" d=\"{}\"/>\n",
                                    num(st.width / k),
                                    stroke_d(st, k)
                                ));
                            }
                        }
                    }
                }
            }
            s.push_str("</g>\n</g>\n");
            s.push_str(&reg);
            s.push_str(&format!(
                "<text x=\"{}\" y=\"{}\" font-family=\"sans-serif\" font-size=\"10\" fill=\"{black}\">Film {:02}/{:02} — {}</text>\n",
                num(REG_MARGIN),
                num(vh - 8.0),
                index,
                total,
                h_hex
            ));
            s.push_str("</svg>\n");

            SeparationFilm { index, color: layer.color, suffix, svg: s }
        })
        .collect()
}

/// Generate separation film PDFs (100% K in CMYK mode, `#000000` in RGB mode) alongside their SVG films.
pub fn to_separation_pdfs(doc: &Doc, opts: &ExportOptions) -> Result<Vec<(SeparationFilm, Vec<u8>)>> {
    let films = to_separation_svgs(doc, opts);
    if films.is_empty() {
        return Ok(Vec::new());
    }
    if opts.color_space == ColorSpace::Rgb {
        let mut out = Vec::with_capacity(films.len());
        for film in films {
            let pdf = svg_to_pdf(&film.svg)?;
            out.push((film, pdf));
        }
        return Ok(out);
    }
    let active_fields: Vec<&Layer> = doc
        .fields
        .iter()
        .filter(|l| !(opts.strip_background && l.canvas))
        .collect();
    let colors: Vec<Rgb> = active_fields.iter().map(|l| l.color).collect();
    let total = active_fields.len();
    let mut out = Vec::with_capacity(films.len());
    for (fi, film) in films.into_iter().enumerate() {
        let sub_doc = separation_film_doc(doc, opts, &active_fields, &colors, fi);
        let sub_opts = ExportOptions {
            registration_marks: true,
            strip_background: false,
            color_space: ColorSpace::Cmyk,
            die_cut_contour: false,
            spot_colors: Vec::new(),
            ..opts.clone()
        };
        let label = format!("Film {:02}/{:02} - {}", film.index, total, hex(film.color));
        let pdf = to_pdf_native(&sub_doc, &sub_opts, Some(&label));
        out.push((film, pdf));
    }
    Ok(out)
}

/// Generate separation film EPS files (100% K in CMYK mode, `#000000` in RGB mode).
pub fn to_separation_eps(doc: &Doc, opts: &ExportOptions) -> Vec<(SeparationFilm, String)> {
    let films = to_separation_svgs(doc, opts);
    if films.is_empty() {
        return Vec::new();
    }
    let active_fields: Vec<&Layer> = doc
        .fields
        .iter()
        .filter(|l| !(opts.strip_background && l.canvas))
        .collect();
    let colors: Vec<Rgb> = active_fields.iter().map(|l| l.color).collect();
    films
        .into_iter()
        .enumerate()
        .map(|(fi, film)| {
            let sub_doc = separation_film_doc(doc, opts, &active_fields, &colors, fi);
            let sub_opts = ExportOptions {
                registration_marks: true,
                strip_background: false,
                die_cut_contour: false,
                spot_colors: Vec::new(),
                ..opts.clone()
            };
            let eps = to_eps(&sub_doc, &sub_opts);
            (film, eps)
        })
        .collect()
}

/// SVG text -> PDF bytes (page size = drawing size, RGB colour space).
pub fn svg_to_pdf(svg: &str) -> Result<Vec<u8>> {
    let mut opt = svg2pdf::usvg::Options::default();
    opt.fontdb_mut().load_system_fonts();
    let tree = svg2pdf::usvg::Tree::from_str(svg, &opt).map_err(|e| Error::Other(format!("cannot read the SVG for PDF export: {e}")))?;
    svg2pdf::to_pdf(&tree, svg2pdf::ConversionOptions::default(), svg2pdf::PageOptions::default()).map_err(|e| Error::Other(format!("PDF export failed: {e}")))
}

/// Export `Doc` to PDF bytes respecting `opts.color_space`, `opts.spot_colors`, and `opts.die_cut_contour`:
/// - `ColorSpace::Rgb` without spot colours or die-cut: converts via `svg_to_pdf(&to_svg(doc, opts))`.
/// - `ColorSpace::Cmyk` or when Named Spot Colors / `CutContour` are active: emits a pure PDF 1.4
///   document with `/Separation /<SpotName> /DeviceCMYK` colour space resources and `CS`/`SCN`/`cs`/`scn`.
pub fn to_pdf(doc: &Doc, opts: &ExportOptions) -> Result<Vec<u8>> {
    if opts.color_space == ColorSpace::Rgb && opts.spot_colors.is_empty() && !opts.die_cut_contour {
        svg_to_pdf(&to_svg(doc, opts))
    } else {
        Ok(to_pdf_native(doc, opts, None))
    }
}

fn cmyk_fmt(c: [f32; 4]) -> String {
    let f = |v: f32| {
        let clamped = if v.is_finite() { v.clamp(0.0, 1.0) } else { 0.0 };
        if clamped <= 0.00005 {
            "0".to_string()
        } else if clamped >= 0.99995 {
            "1".to_string()
        } else {
            let s = format!("{:.4}", clamped);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
    };
    format!("{} {} {} {}", f(c[0]), f(c[1]), f(c[2]), f(c[3]))
}

fn rgb_pdf_fmt(c: Rgb) -> String {
    format!(
        "{:.4} {:.4} {:.4}",
        c[0] as f64 / 255.0,
        c[1] as f64 / 255.0,
        c[2] as f64 / 255.0
    )
}

fn pdf_escape_text(s: &str) -> String {
    s.replace('\\', "\\\\").replace('(', "\\(").replace(')', "\\)")
}

/// Registry of Named Spot Colors for one PDF or EPS export.
struct SpotRegistry {
    /// `(resource_key "CS1", sanitized_name, fallback_cmyk, optional_source_rgb)`
    entries: Vec<(String, String, [f32; 4], Option<Rgb>)>,
}

impl SpotRegistry {
    fn build(opts: &ExportOptions, has_die_cut: bool) -> Self {
        let mut entries: Vec<(String, String, [f32; 4], Option<Rgb>)> = Vec::new();
        let mut push_unique = |name: String, cmyk: [f32; 4], rgb: Option<Rgb>| {
            let clean = sanitize_spot_name(&name);
            if !entries.iter().any(|(_, n, _, _)| n == &clean) {
                let key = format!("CS{}", entries.len() + 1);
                entries.push((key, clean, cmyk, rgb));
            }
        };
        if has_die_cut {
            push_unique(opts.die_cut_spot_name.clone(), DIE_CUT_CMYK, None);
        }
        for m in &opts.spot_colors {
            if let Some(rgb) = parse_hex_rgb(&m.hex) {
                push_unique(m.spot_name.clone(), rgb_arr_to_cmyk(rgb), Some(rgb));
            }
        }
        Self { entries }
    }

    fn key_for_rgb(&self, rgb: Rgb) -> Option<&str> {
        self.entries
            .iter()
            .find(|(_, _, _, src)| *src == Some(rgb))
            .map(|(k, _, _, _)| k.as_str())
    }

    fn entry_for_rgb(&self, rgb: Rgb) -> Option<(&str, [f32; 4])> {
        self.entries
            .iter()
            .find(|(_, _, _, src)| *src == Some(rgb))
            .map(|(_, name, cmyk, _)| (name.as_str(), *cmyk))
    }

    fn key_for_name(&self, raw_name: &str) -> Option<&str> {
        let clean = sanitize_spot_name(raw_name);
        self.entries
            .iter()
            .find(|(_, n, _, _)| n == &clean)
            .map(|(k, _, _, _)| k.as_str())
    }
}

/// Pure PDF 1.4 writer supporting DeviceCMYK, DeviceRGB, Named Spot `/Separation` colour spaces,
/// and `CutContour` die-cut layers.
fn to_pdf_native(doc: &Doc, opts: &ExportOptions, footer_label: Option<&str>) -> Vec<u8> {
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let m = if opts.registration_marks { REG_MARGIN } else { 0.0 };

    let die_cut_contours = if opts.die_cut_contour {
        build_die_cut_contours(doc, opts)
    } else {
        Vec::new()
    };
    let backing_contours = build_sticker_backing_contours(doc, opts);
    let (dc_min_x, dc_min_y, dc_max_x, dc_max_y) = if die_cut_contours.is_empty() && backing_contours.is_empty() {
        (0.0, 0.0, doc.width as f64, doc.height as f64)
    } else if backing_contours.is_empty() {
        die_cut_bounds_svg(doc, &die_cut_contours)
    } else {
        let mut all = Vec::with_capacity(die_cut_contours.len() + backing_contours.len());
        all.extend_from_slice(&die_cut_contours);
        all.extend_from_slice(&backing_contours);
        die_cut_bounds_svg(doc, &all)
    };
    let shift_x = (-dc_min_x).max(0.0);
    let shift_y = (-dc_min_y).max(0.0);
    let content_w = dc_max_x - dc_min_x;
    let content_h = dc_max_y - dc_min_y;
    let total_w = content_w + 2.0 * m;
    let total_h = content_h + 2.0 * m;

    let spots = SpotRegistry::build(opts, !die_cut_contours.is_empty());

    let map_svg = |sx: f64, sy: f64| -> (f64, f64) { (sx + shift_x, total_h - (sy + shift_y)) };
    let map_work = |p: (f64, f64)| -> (f64, f64) { map_svg(p.0 / k + m, p.1 / k + m) };

    let mut stream = String::with_capacity(16 * 1024);
    stream.push_str("q\n");

    let emit_contours = |contours: &[Contour], out: &mut String| {
        for c in contours {
            let p0 = map_work(c.start);
            out.push_str(&format!("{} {} m\n", ps_num(p0.0), ps_num(p0.1)));
            let mut prev = c.start;
            for cv in &c.curves {
                if is_straight(prev, cv[0], cv[1], cv[2], k) {
                    let e = map_work(cv[2]);
                    out.push_str(&format!("{} {} l\n", ps_num(e.0), ps_num(e.1)));
                } else {
                    let c1 = map_work(cv[0]);
                    let c2 = map_work(cv[1]);
                    let e = map_work(cv[2]);
                    out.push_str(&format!(
                        "{} {} {} {} {} {} c\n",
                        ps_num(c1.0), ps_num(c1.1),
                        ps_num(c2.0), ps_num(c2.1),
                        ps_num(e.0), ps_num(e.1)
                    ));
                }
                prev = cv[2];
            }
            out.push_str("h\n");
        }
    };

    let set_fill = |color: Rgb, out: &mut String| {
        if let Some(cs_key) = spots.key_for_rgb(color) {
            out.push_str(&format!("/{cs_key} cs 1 scn\n"));
        } else {
            match opts.color_space {
                ColorSpace::Cmyk => out.push_str(&format!("{} k\n", cmyk_fmt(rgb_arr_to_cmyk(color)))),
                ColorSpace::Rgb => out.push_str(&format!("{} rg\n", rgb_pdf_fmt(color))),
            }
        }
    };

    let set_stroke = |color: Rgb, out: &mut String| {
        if let Some(cs_key) = spots.key_for_rgb(color) {
            out.push_str(&format!("/{cs_key} CS 1 SCN\n"));
        } else {
            match opts.color_space {
                ColorSpace::Cmyk => out.push_str(&format!("{} K\n", cmyk_fmt(rgb_arr_to_cmyk(color)))),
                ColorSpace::Rgb => out.push_str(&format!("{} RG\n", rgb_pdf_fmt(color))),
            }
        }
    };

    let trap_px = opts.trap_pt.max(0.0) * 96.0 / 72.0;
    let emit_layer = |layer: &Layer, with_trap: bool, out: &mut String| {
        let mut all_contours: Vec<Contour> = layer.contours.clone();
        for p in &layer.prims {
            all_contours.push(primitive_to_contour(p));
        }
        if all_contours.is_empty() {
            return;
        }
        set_fill(layer.color, out);
        if with_trap && trap_px > 0.0 {
            set_stroke(layer.color, out);
            out.push_str(&format!("1 j {} w\n", ps_num(trap_px)));
            emit_contours(&all_contours, out);
            out.push_str("B*\n");
        } else {
            emit_contours(&all_contours, out);
            out.push_str("f*\n");
        }
    };

    // 1. Background + 1b. StickerBacking + 2. Colour fields
    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    if !opts.strip_background && has_canvas_bg {
        emit_layer(&doc.fields[0], false, &mut stream);
    }
    if !backing_contours.is_empty() {
        let bg_rgb = sticker_backing_rgb(opts);
        if opts.color_space == ColorSpace::Cmyk && bg_rgb == [255, 255, 255] {
            stream.push_str("0 0 0 0 k\n");
        } else {
            set_fill(bg_rgb, &mut stream);
        }
        emit_contours(&backing_contours, &mut stream);
        stream.push_str("f*\n");
    }
    let skip_first = if has_canvas_bg { 1 } else { 0 };
    for layer in doc.fields.iter().skip(skip_first) {
        emit_layer(layer, true, &mut stream);
    }

    // 3. Thin lines / Centerline strokes
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                emit_layer(l, false, &mut stream);
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                emit_layer(l, false, &mut stream);
            }
            for sl in &doc.strokes {
                set_stroke(sl.color, &mut stream);
                stream.push_str("1 J 1 j\n");
                for st in &sl.strokes {
                    if st.curves.is_empty() {
                        continue;
                    }
                    let w_pdf = (st.width / k).max(0.1);
                    stream.push_str(&format!("{} w\n", ps_num(w_pdf)));
                    let p0 = map_work(st.curves[0][0]);
                    stream.push_str(&format!("{} {} m\n", ps_num(p0.0), ps_num(p0.1)));
                    for cv in &st.curves {
                        if is_straight(cv[0], cv[1], cv[2], cv[3], k) {
                            let e = map_work(cv[3]);
                            stream.push_str(&format!("{} {} l\n", ps_num(e.0), ps_num(e.1)));
                        } else {
                            let c1 = map_work(cv[1]);
                            let c2 = map_work(cv[2]);
                            let e = map_work(cv[3]);
                            stream.push_str(&format!(
                                "{} {} {} {} {} {} c\n",
                                ps_num(c1.0), ps_num(c1.1),
                                ps_num(c2.0), ps_num(c2.1),
                                ps_num(e.0), ps_num(e.1)
                            ));
                        }
                    }
                    stream.push_str("S\n");
                }
            }
        }
    }

    // 4. Die-Cut Contour layer (`/Separation /CutContour /DeviceCMYK`, stroke 0.25 pt)
    if !die_cut_contours.is_empty() {
        if let Some(cut_key) = spots.key_for_name(&opts.die_cut_spot_name) {
            stream.push_str(&format!("/{cut_key} CS 1 SCN\n1 j {} w\n", ps_num(DIE_CUT_STROKE_PT)));
        } else {
            stream.push_str(&format!("0 1 0 0 K\n1 j {} w\n", ps_num(DIE_CUT_STROKE_PT)));
        }
        emit_contours(&die_cut_contours, &mut stream);
        stream.push_str("S\n");
    }

    // 5. Registration marks (Pure K = 0 0 0 1 in CMYK, 0 0 0 in RGB)
    if opts.registration_marks {
        let pts = [
            (m / 2.0 - shift_x, m / 2.0 - shift_y),
            (content_w + 1.5 * m - shift_x, m / 2.0 - shift_y),
            (m / 2.0 - shift_x, content_h + 1.5 * m - shift_y),
            (content_w + 1.5 * m - shift_x, content_h + 1.5 * m - shift_y),
        ];
        match opts.color_space {
            ColorSpace::Cmyk => stream.push_str("0 0 0 1 K\n0.75 w\n"),
            ColorSpace::Rgb => stream.push_str("0 0 0 RG\n0.75 w\n"),
        }
        const KAPPA: f64 = 0.5522847498307936;
        let r = 6.0;
        let kr = r * KAPPA;
        for (cx, cy) in pts {
            let (px, py) = map_svg(cx, cy);
            stream.push_str(&format!(
                "{} {} m\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\n{} {} {} {} {} {} c\nh S\n",
                ps_num(px + r), ps_num(py),
                ps_num(px + r), ps_num(py + kr), ps_num(px + kr), ps_num(py + r), ps_num(px), ps_num(py + r),
                ps_num(px - kr), ps_num(py + r), ps_num(px - r), ps_num(py + kr), ps_num(px - r), ps_num(py),
                ps_num(px - r), ps_num(py - kr), ps_num(px - kr), ps_num(py - r), ps_num(px), ps_num(py - r),
                ps_num(px + kr), ps_num(py - r), ps_num(px + r), ps_num(py - kr), ps_num(px + r), ps_num(py)
            ));
            stream.push_str(&format!(
                "{} {} m {} {} l {} {} m {} {} l S\n",
                ps_num(px - 9.0), ps_num(py), ps_num(px + 9.0), ps_num(py),
                ps_num(px), ps_num(py - 9.0), ps_num(px), ps_num(py + 9.0)
            ));
        }
    }

    if let Some(lbl) = footer_label {
        stream.push_str(&format!(
            "0 0 0 1 k\nBT /F1 10 Tf {} 8 Td ({}) Tj ET\n",
            ps_num(REG_MARGIN),
            pdf_escape_text(lbl)
        ));
    }

    stream.push_str("Q\n");

    // Build `/ColorSpace` resource dictionary for registered Named Spot Colors.
    let cs_resource = if spots.entries.is_empty() {
        String::new()
    } else {
        let mut dict = String::from(" /ColorSpace <<");
        for (key, name, cmyk, _) in &spots.entries {
            let encoded = pdf_name_encode(name);
            let c1 = cmyk_fmt(*cmyk);
            dict.push_str(&format!(
                " /{key} [/Separation /{encoded} /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [{c1}] /N 1 >>]"
            ));
        }
        dict.push_str(" >>");
        dict
    };

    // Assemble PDF 1.4 container with exact byte offsets.
    let mut pdf = Vec::<u8>::with_capacity(stream.len() + 1024 + cs_resource.len());
    pdf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");
    let mut offsets = vec![0usize; 6]; // 1..=5

    offsets[1] = pdf.len();
    pdf.extend_from_slice(b"1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n");

    offsets[2] = pdf.len();
    pdf.extend_from_slice(b"2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n");

    offsets[3] = pdf.len();
    let page_obj = format!(
        "3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Resources << /Font << /F1 5 0 R >>{cs_resource} >> /Contents 4 0 R >>\nendobj\n",
        ps_num(total_w),
        ps_num(total_h)
    );
    pdf.extend_from_slice(page_obj.as_bytes());

    offsets[4] = pdf.len();
    let stream_bytes = stream.as_bytes();
    let header4 = format!("4 0 obj\n<< /Length {} >>\nstream\n", stream_bytes.len());
    pdf.extend_from_slice(header4.as_bytes());
    pdf.extend_from_slice(stream_bytes);
    pdf.extend_from_slice(b"endstream\nendobj\n");

    offsets[5] = pdf.len();
    pdf.extend_from_slice(b"5 0 obj\n<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>\nendobj\n");

    let xref_offset = pdf.len();
    pdf.extend_from_slice(b"xref\n0 6\n0000000000 65535 f \n");
    for &off in &offsets[1..=5] {
        pdf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    let trailer = format!(
        "trailer\n<< /Size 6 /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n"
    );
    pdf.extend_from_slice(trailer.as_bytes());
    pdf
}

// ---------- Adobe Swatch Exchange ----------

fn ase_name(out: &mut Vec<u8>, name: &str) {
    let units: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    out.extend_from_slice(&(units.len() as u16).to_be_bytes());
    for u in units {
        out.extend_from_slice(&u.to_be_bytes());
    }
}

/// `.ase` file: one group "Trasea" with the palette as global RGB colours named `Trasea NN #RRGGBB`.
pub fn to_ase(palette: &[Rgb]) -> Vec<u8> {
    to_ase_with_color_space(palette, ColorSpace::Rgb)
}

/// `.ase` file in either RGB (`RGB ` + 3 floats) or CMYK (`CMYK` + 4 floats) colour model.
pub fn to_ase_with_color_space(palette: &[Rgb], color_space: ColorSpace) -> Vec<u8> {
    to_ase_with_spots(palette, color_space, &[], None)
}

/// `.ase` file supporting Named Spot Colors (ASE colour type `0x0001` Spot vs `0x0000` Global)
/// and an optional `die_cut_spot` swatch (e.g. `"CutContour"` at CMYK `0/100/0/0`).
pub fn to_ase_with_spots(
    palette: &[Rgb],
    color_space: ColorSpace,
    spot_colors: &[SpotColorMapping],
    die_cut_spot: Option<&str>,
) -> Vec<u8> {
    let die_cut_clean = die_cut_spot.map(sanitize_spot_name);
    let extra_cut = die_cut_clean
        .as_ref()
        .filter(|name| {
            !spot_colors
                .iter()
                .any(|m| sanitize_spot_name(&m.spot_name) == **name && parse_hex_rgb(&m.hex).map_or(false, |rgb| palette.contains(&rgb)))
        })
        .cloned();

    let total_swatches = palette.len() + if extra_cut.is_some() { 1 } else { 0 };
    let mut out = Vec::new();
    out.extend_from_slice(b"ASEF");
    out.extend_from_slice(&1u16.to_be_bytes()); // version 1.0
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&((total_swatches + 2) as u32).to_be_bytes()); // blocks: group start, colours, group end

    // group start
    let mut gs = Vec::new();
    ase_name(&mut gs, "Trasea");
    out.extend_from_slice(&0xC001u16.to_be_bytes());
    out.extend_from_slice(&(gs.len() as u32).to_be_bytes());
    out.extend_from_slice(&gs);

    let emit_swatch = |out: &mut Vec<u8>, name: &str, rgb: Rgb, cmyk_override: Option<[f32; 4]>, is_spot: bool| {
        let mut body = Vec::new();
        ase_name(&mut body, name);
        match color_space {
            ColorSpace::Rgb if !is_spot || cmyk_override.is_none() => {
                body.extend_from_slice(b"RGB ");
                for ch in rgb {
                    body.extend_from_slice(&(ch as f32 / 255.0).to_be_bytes());
                }
            }
            _ => {
                body.extend_from_slice(b"CMYK");
                let cmyk = cmyk_override.unwrap_or_else(|| rgb_arr_to_cmyk(rgb));
                for ch in cmyk {
                    body.extend_from_slice(&ch.to_be_bytes());
                }
            }
        }
        let color_type: u16 = if is_spot { 0x0001 } else { 0x0000 }; // 0x0001 = Spot, 0x0000 = Global
        body.extend_from_slice(&color_type.to_be_bytes());
        out.extend_from_slice(&0x0001u16.to_be_bytes());
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        out.extend_from_slice(&body);
    };

    for (i, &c) in palette.iter().enumerate() {
        if let Some(spot_name) = lookup_spot_name(c, spot_colors) {
            emit_swatch(&mut out, &spot_name, c, None, true);
        } else {
            let label = format!("Trasea {:02} {}", i + 1, hex(c));
            emit_swatch(&mut out, &label, c, None, false);
        }
    }

    if let Some(cut_name) = extra_cut {
        emit_swatch(&mut out, &cut_name, DIE_CUT_RGB, Some(DIE_CUT_CMYK), true);
    }

    // group end
    out.extend_from_slice(&0xC002u16.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out
}

// ---------- Encapsulated PostScript (EPSF-3.0 / Level 2) ----------

fn ps_num(v: f64) -> String {
    let safe = if v.is_finite() { v } else { 0.0 };
    num(safe)
}

fn ps_color(c: Rgb, cs: ColorSpace, spots: &SpotRegistry) -> String {
    if let Some((name, cmyk)) = spots.entry_for_rgb(c) {
        return format!("{} ({}) SPOT\n", cmyk_fmt(cmyk), eps_string_escape(name));
    }
    match cs {
        ColorSpace::Rgb => format!(
            "{:.4} {:.4} {:.4} RGB\n",
            c[0] as f64 / 255.0,
            c[1] as f64 / 255.0,
            c[2] as f64 / 255.0
        ),
        ColorSpace::Cmyk => format!("{} CMYK\n", cmyk_fmt(rgb_arr_to_cmyk(c))),
    }
}

fn rotate_pt(p: (f64, f64), center: (f64, f64), angle_deg: f64) -> (f64, f64) {
    if angle_deg.abs() <= 0.5 {
        return p;
    }
    let rad = angle_deg.to_radians();
    let (s, c) = rad.sin_cos();
    let dx = p.0 - center.0;
    let dy = p.1 - center.1;
    (center.0 + dx * c - dy * s, center.1 + dx * s + dy * c)
}

/// Convert a `Primitive` into closed cubic Bézier segments (working pixels) so EPS/PDF/DXF need no exotic operators.
pub(crate) fn primitive_to_contour(p: &Primitive) -> Contour {
    const KAPPA: f64 = 0.5522847498307936;
    match *p {
        Primitive::Circle { cx, cy, r } => {
            let k = r * KAPPA;
            let p0 = (cx + r, cy);
            let p1 = (cx, cy + r);
            let p2 = (cx - r, cy);
            let p3 = (cx, cy - r);
            Contour {
                start: p0,
                curves: vec![
                    [(cx + r, cy + k), (cx + k, cy + r), p1],
                    [(cx - k, cy + r), (cx - r, cy + k), p2],
                    [(cx - r, cy - k), (cx - k, cy - r), p3],
                    [(cx + k, cy - r), (cx + r, cy - k), p0],
                ],
                prim: None,
            }
        }
        Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
            let (kx, ky) = (rx * KAPPA, ry * KAPPA);
            let center = (cx, cy);
            let r = |q: (f64, f64)| rotate_pt(q, center, angle_deg);
            let p0 = r((cx + rx, cy));
            let p1 = r((cx, cy + ry));
            let p2 = r((cx - rx, cy));
            let p3 = r((cx, cy - ry));
            Contour {
                start: p0,
                curves: vec![
                    [r((cx + rx, cy + ky)), r((cx + kx, cy + ry)), p1],
                    [r((cx - kx, cy + ry)), r((cx - rx, cy + ky)), p2],
                    [r((cx - rx, cy - ky)), r((cx - kx, cy - ry)), p3],
                    [r((cx + kx, cy - ry)), r((cx + rx, cy - ky)), p0],
                ],
                prim: None,
            }
        }
        Primitive::Rect { x, y, w, h, rx, angle_deg } => {
            let center = (x + w / 2.0, y + h / 2.0);
            let r = |q: (f64, f64)| rotate_pt(q, center, angle_deg);
            let line = |a: (f64, f64), b: (f64, f64)| [
                (a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0),
                (a.0 + 2.0 * (b.0 - a.0) / 3.0, a.1 + 2.0 * (b.1 - a.1) / 3.0),
                b,
            ];
            let rad = rx.clamp(0.0, (w * 0.5).min(h * 0.5));
            if rad <= 0.01 {
                let p0 = r((x, y));
                let p1 = r((x + w, y));
                let p2 = r((x + w, y + h));
                let p3 = r((x, y + h));
                Contour {
                    start: p0,
                    curves: vec![line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)],
                    prim: None,
                }
            } else {
                let c = rad * KAPPA;
                let a0 = r((x + rad, y));
                let a1 = r((x + w - rad, y));
                let b0 = r((x + w, y + rad));
                let b1 = r((x + w, y + h - rad));
                let c0 = r((x + w - rad, y + h));
                let c1 = r((x + rad, y + h));
                let d0 = r((x, y + h - rad));
                let d1 = r((x, y + rad));
                Contour {
                    start: a0,
                    curves: vec![
                        line(a0, a1),
                        [r((x + w - rad + c, y)), r((x + w, y + rad - c)), b0],
                        line(b0, b1),
                        [r((x + w, y + h - rad + c)), r((x + w - rad + c, y + h)), c0],
                        line(c0, c1),
                        [r((x + rad - c, y + h)), r((x, y + h - rad + c)), d0],
                        line(d0, d1),
                        [r((x, y + rad - c)), r((x + rad - c, y)), a0],
                    ],
                    prim: None,
                }
            }
        }
    }
}

/// Pure PostScript Level 2 / EPSF-3.0 export directly from `Doc` (no Ghostscript or Inkscape needed).
/// Supports RGB (`setrgbcolor`), CMYK (`setcmykcolor`, zero `setrgbcolor` in output),
/// Named Spot Colors (`%%DocumentCustomColors:`, `%%CMYKCustomColor:`, Level-2 Separation `/SPOT`),
/// and `CutContour` die-cut layers.
pub fn to_eps(doc: &Doc, opts: &ExportOptions) -> String {
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let m = if opts.registration_marks { REG_MARGIN } else { 0.0 };

    let die_cut_contours = if opts.die_cut_contour {
        build_die_cut_contours(doc, opts)
    } else {
        Vec::new()
    };
    let backing_contours = build_sticker_backing_contours(doc, opts);
    let (dc_min_x, dc_min_y, dc_max_x, dc_max_y) = if die_cut_contours.is_empty() && backing_contours.is_empty() {
        (0.0, 0.0, doc.width as f64, doc.height as f64)
    } else if backing_contours.is_empty() {
        die_cut_bounds_svg(doc, &die_cut_contours)
    } else {
        let mut all = Vec::with_capacity(die_cut_contours.len() + backing_contours.len());
        all.extend_from_slice(&die_cut_contours);
        all.extend_from_slice(&backing_contours);
        die_cut_bounds_svg(doc, &all)
    };
    let shift_x = (-dc_min_x).max(0.0);
    let shift_y = (-dc_min_y).max(0.0);
    let content_w = dc_max_x - dc_min_x;
    let content_h = dc_max_y - dc_min_y;
    let total_w = content_w + 2.0 * m;
    let total_h = content_h + 2.0 * m;
    let bb_w = total_w.ceil() as u32;
    let bb_h = total_h.ceil() as u32;
    let cs = opts.color_space;
    let spots = SpotRegistry::build(opts, !die_cut_contours.is_empty());

    let mut s = String::new();
    s.push_str("%!PS-Adobe-3.0 EPSF-3.0\n");
    s.push_str("%%Creator: Trasea\n");
    s.push_str(&format!("%%BoundingBox: 0 0 {bb_w} {bb_h}\n"));
    s.push_str(&format!("%%HiResBoundingBox: 0 0 {:.2} {:.2}\n", total_w, total_h));
    s.push_str("%%LanguageLevel: 2\n");
    s.push_str("%%Pages: 1\n");
    if !spots.entries.is_empty() {
        let names: Vec<String> = spots
            .entries
            .iter()
            .map(|(_, name, _, _)| format!("({})", eps_string_escape(name)))
            .collect();
        s.push_str(&format!("%%DocumentCustomColors: {}\n", names.join(" ")));
        for (_, name, cmyk, _) in &spots.entries {
            s.push_str(&format!(
                "%%CMYKCustomColor: {} ({})\n",
                cmyk_fmt(*cmyk),
                eps_string_escape(name)
            ));
        }
    }
    s.push_str("%%EndComments\n");
    s.push_str("%%BeginProlog\n");
    s.push_str("/M { moveto } bind def\n");
    s.push_str("/L { lineto } bind def\n");
    s.push_str("/C { curveto } bind def\n");
    s.push_str("/Z { closepath } bind def\n");
    s.push_str("/F { eofill } bind def\n");
    s.push_str("/S { stroke } bind def\n");
    match cs {
        ColorSpace::Rgb => s.push_str("/RGB { setrgbcolor } bind def\n"),
        ColorSpace::Cmyk => s.push_str("/CMYK { setcmykcolor } bind def\n"),
    }
    if !spots.entries.is_empty() {
        // PostScript Level 2 Separation colour space with automatic Level-1 CMYK fallback.
        s.push_str(
            "/SPOT {\n  /setcolorspace where {\n    pop\n    5 1 roll 4 array astore /_sc exch def\n    [/Separation 3 -1 roll /DeviceCMYK [ /dup _sc 0 get /mul /exch /dup _sc 1 get /mul /exch /dup _sc 2 get /mul /exch _sc 3 get /mul ] cvx] setcolorspace 1 setcolor\n  } {\n    pop setcmykcolor\n  } ifelse\n} bind def\n",
        );
    }
    s.push_str("%%EndProlog\n");
    s.push_str("gsave\n");

    let map_svg = |sx: f64, sy: f64| -> (f64, f64) { (sx + shift_x, total_h - (sy + shift_y)) };
    let map_work = |p: (f64, f64)| -> (f64, f64) { map_svg(p.0 / k + m, p.1 / k + m) };

    let emit_contours = |contours: &[Contour], out: &mut String| {
        for c in contours {
            let p0 = map_work(c.start);
            out.push_str(&format!("{} {} M\n", ps_num(p0.0), ps_num(p0.1)));
            let mut prev = c.start;
            for cv in &c.curves {
                if is_straight(prev, cv[0], cv[1], cv[2], k) {
                    let e = map_work(cv[2]);
                    out.push_str(&format!("{} {} L\n", ps_num(e.0), ps_num(e.1)));
                } else {
                    let c1 = map_work(cv[0]);
                    let c2 = map_work(cv[1]);
                    let e = map_work(cv[2]);
                    out.push_str(&format!(
                        "{} {} {} {} {} {} C\n",
                        ps_num(c1.0), ps_num(c1.1),
                        ps_num(c2.0), ps_num(c2.1),
                        ps_num(e.0), ps_num(e.1)
                    ));
                }
                prev = cv[2];
            }
            out.push_str("Z\n");
        }
    };

    let trap_px = opts.trap_pt.max(0.0) * 96.0 / 72.0;
    let emit_layer = |layer: &Layer, with_trap: bool, out: &mut String| {
        let mut all_contours: Vec<Contour> = layer.contours.clone();
        for p in &layer.prims {
            all_contours.push(primitive_to_contour(p));
        }
        if all_contours.is_empty() {
            return;
        }
        out.push_str(&ps_color(layer.color, cs, &spots));
        out.push_str("newpath\n");
        emit_contours(&all_contours, out);
        if with_trap && trap_px > 0.0 {
            out.push_str(&format!("gsave F grestore 1 setlinejoin {} setlinewidth S\n", ps_num(trap_px)));
        } else {
            out.push_str("F\n");
        }
    };

    // 1. Latar + 1b. StickerBacking + 2. Bidang warna
    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    if !opts.strip_background && has_canvas_bg {
        emit_layer(&doc.fields[0], false, &mut s);
    }
    if !backing_contours.is_empty() {
        let bg_rgb = sticker_backing_rgb(opts);
        s.push_str("% StickerBacking\n");
        if cs == ColorSpace::Cmyk && bg_rgb == [255, 255, 255] {
            s.push_str("0 0 0 0 CMYK\n");
        } else {
            s.push_str(&ps_color(bg_rgb, cs, &spots));
        }
        s.push_str("newpath\n");
        emit_contours(&backing_contours, &mut s);
        s.push_str("F\n");
    }
    let skip_first = if has_canvas_bg { 1 } else { 0 };
    for layer in doc.fields.iter().skip(skip_first) {
        emit_layer(layer, true, &mut s);
    }

    // 3. Garis / Centerline strokes
    match opts.lines_as {
        LinesAs::Shape => {
            for l in &doc.line_shapes {
                emit_layer(l, false, &mut s);
            }
        }
        LinesAs::Stroke => {
            for l in &doc.line_fallback {
                emit_layer(l, false, &mut s);
            }
            for sl in &doc.strokes {
                s.push_str(&ps_color(sl.color, cs, &spots));
                for st in &sl.strokes {
                    if st.curves.is_empty() {
                        continue;
                    }
                    let w_ps = (st.width / k).max(0.1);
                    s.push_str(&format!("1 setlinecap 1 setlinejoin {} setlinewidth newpath\n", ps_num(w_ps)));
                    let p0 = map_work(st.curves[0][0]);
                    s.push_str(&format!("{} {} M\n", ps_num(p0.0), ps_num(p0.1)));
                    for cv in &st.curves {
                        if is_straight(cv[0], cv[1], cv[2], cv[3], k) {
                            let e = map_work(cv[3]);
                            s.push_str(&format!("{} {} L\n", ps_num(e.0), ps_num(e.1)));
                        } else {
                            let c1 = map_work(cv[1]);
                            let c2 = map_work(cv[2]);
                            let e = map_work(cv[3]);
                            s.push_str(&format!(
                                "{} {} {} {} {} {} C\n",
                                ps_num(c1.0), ps_num(c1.1),
                                ps_num(c2.0), ps_num(c2.1),
                                ps_num(e.0), ps_num(e.1)
                            ));
                        }
                    }
                    s.push_str("S\n");
                }
            }
        }
    }

    // 4. Die-Cut Contour layer (Spot CutContour, 0.25 pt)
    if !die_cut_contours.is_empty() {
        let cut_name = eps_string_escape(&opts.die_cut_spot_name);
        s.push_str(&format!(
            "{} ({cut_name}) SPOT\n1 setlinejoin {} setlinewidth newpath\n",
            cmyk_fmt(DIE_CUT_CMYK),
            ps_num(DIE_CUT_STROKE_PT)
        ));
        emit_contours(&die_cut_contours, &mut s);
        s.push_str("S\n");
    }

    // 5. Registration marks (4 corners in outer margin)
    if opts.registration_marks {
        let pts = [
            (m / 2.0 - shift_x, m / 2.0 - shift_y),
            (content_w + 1.5 * m - shift_x, m / 2.0 - shift_y),
            (m / 2.0 - shift_x, content_h + 1.5 * m - shift_y),
            (content_w + 1.5 * m - shift_x, content_h + 1.5 * m - shift_y),
        ];
        match cs {
            ColorSpace::Rgb => s.push_str("0 0 0 RGB 0.75 setlinewidth\n"),
            ColorSpace::Cmyk => s.push_str("0 0 0 1 CMYK 0.75 setlinewidth\n"),
        }
        const KAPPA: f64 = 0.5522847498307936;
        let r = 6.0;
        let kr = r * KAPPA;
        for (cx, cy) in pts {
            let (px, py) = map_svg(cx, cy);
            s.push_str(&format!(
                "newpath {} {} M {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C {} {} {} {} {} {} C Z S\n",
                ps_num(px + r), ps_num(py),
                ps_num(px + r), ps_num(py + kr), ps_num(px + kr), ps_num(py + r), ps_num(px), ps_num(py + r),
                ps_num(px - kr), ps_num(py + r), ps_num(px - r), ps_num(py + kr), ps_num(px - r), ps_num(py),
                ps_num(px - r), ps_num(py - kr), ps_num(px - kr), ps_num(py - r), ps_num(px), ps_num(py - r),
                ps_num(px + kr), ps_num(py - r), ps_num(px + r), ps_num(py - kr), ps_num(px + r), ps_num(py)
            ));
            s.push_str(&format!(
                "newpath {} {} M {} {} L {} {} M {} {} L S\n",
                ps_num(px - 9.0), ps_num(py), ps_num(px + 9.0), ps_num(py),
                ps_num(px), ps_num(py - 9.0), ps_num(px), ps_num(py + 9.0)
            ));
        }
    }

    s.push_str("grestore\nshowpage\n%%Trailer\n%%EOF\n");
    s
}
