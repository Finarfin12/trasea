//! Pure AutoCAD R12/R2000 ASCII DXF exporter for laser cutting, CNC, and vinyl plotters.
//!
//! - Separate layers per colour (`COLOR_01_RRGGBB`, `THIN_LINES`, `CENTERLINE`, `CUTCONTOUR`, `REGISTRATION`).
//! - `Primitive::Circle` emitted as native `CIRCLE` entities; Bézier contours adaptively flattened
//!   to `POLYLINE` + `VERTEX` + `SEQEND` (and pure open `LWPOLYLINE` on `CENTERLINE` in Centerline mode).
//! - CAD bottom-left Y axis (`y_dxf = (total_h - y_svg) * scale`).
//! - Physical unit `mm` (96 DPI default: `1 px = 0.2645833 mm`, or overridden by `dxf_target_width_mm`)
//!   or `px` (`scale = 1.0`).

use crate::contour::{build_die_cut_contours, die_cut_bounds_svg};
use crate::export::{primitive_to_contour, Doc, ExportOptions, LinesAs, REG_MARGIN};
use crate::primitives::Primitive;
use crate::trace::{Contour, Layer};
use crate::{hex, Rgb};
use serde::{Deserialize, Serialize};

/// 25.4 mm / 96 px = 0.2645833333... mm per CSS/SVG pixel.
pub const MM_PER_PX_96DPI: f64 = 25.4 / 96.0;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DxfUnit {
    #[default]
    Mm,
    Px,
}

impl DxfUnit {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "mm" => Some(Self::Mm),
            "px" => Some(Self::Px),
            _ => None,
        }
    }
}

fn dxf_num(v: f64) -> String {
    let safe = if v.is_finite() { v } else { 0.0 };
    let s = format!("{:.4}", safe);
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" || s.is_empty() {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Map an RGB colour to a simple standard AutoCAD Color Index (1..=7) so CAD/laser apps show distinct layers.
fn rgb_to_aci(c: Rgb) -> u8 {
    let palette: [(u8, Rgb); 7] = [
        (1, [255, 0, 0]),     // Red
        (2, [255, 255, 0]),   // Yellow
        (3, [0, 255, 0]),     // Green
        (4, [0, 255, 255]),   // Cyan
        (5, [0, 0, 255]),     // Blue
        (6, [255, 0, 255]),   // Magenta
        (7, [24, 24, 24]),    // White/Black (ACI 7 renders black on light canvas, white on dark)
    ];
    palette
        .iter()
        .min_by_key(|(_, p)| {
            let dr = c[0] as i32 - p[0] as i32;
            let dg = c[1] as i32 - p[1] as i32;
            let db = c[2] as i32 - p[2] as i32;
            dr * dr + dg * dg + db * db
        })
        .map(|&(aci, _)| aci)
        .unwrap_or(7)
}

/// Point-to-segment distance squared.
fn dist_pt_seg_sq(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len2 = dx * dx + dy * dy;
    if len2 <= 1e-12 {
        return (p.0 - a.0).powi(2) + (p.1 - a.1).powi(2);
    }
    let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len2).clamp(0.0, 1.0);
    let proj = (a.0 + t * dx, a.1 + t * dy);
    (p.0 - proj.0).powi(2) + (p.1 - proj.1).powi(2)
}

/// Adaptively flatten a cubic Bézier segment (p0, c1, c2, p3) into `out` (excluding `p0`, including `p3`).
fn flatten_cubic(
    p0: (f64, f64),
    c1: (f64, f64),
    c2: (f64, f64),
    p3: (f64, f64),
    tol_sq: f64,
    depth: u8,
    out: &mut Vec<(f64, f64)>,
) {
    let d1 = dist_pt_seg_sq(c1, p0, p3);
    let d2 = dist_pt_seg_sq(c2, p0, p3);
    if depth >= 8 || (d1 <= tol_sq && d2 <= tol_sq) {
        out.push(p3);
        return;
    }
    let mid = |a: (f64, f64), b: (f64, f64)| ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
    let p01 = mid(p0, c1);
    let p12 = mid(c1, c2);
    let p23 = mid(c2, p3);
    let p012 = mid(p01, p12);
    let p123 = mid(p12, p23);
    let p0123 = mid(p012, p123);
    flatten_cubic(p0, p01, p012, p0123, tol_sq, depth + 1, out);
    flatten_cubic(p0123, p123, p23, p3, tol_sq, depth + 1, out);
}

fn push_polyline(out: &mut String, layer: &str, pts: &[(f64, f64)], closed: bool) {
    if pts.len() < 2 {
        return;
    }
    out.push_str(&format!(
        "0\nPOLYLINE\n8\n{layer}\n66\n1\n70\n{}\n",
        if closed { 1 } else { 0 }
    ));
    for &(x, y) in pts {
        out.push_str(&format!(
            "0\nVERTEX\n8\n{layer}\n10\n{}\n20\n{}\n30\n0\n",
            dxf_num(x),
            dxf_num(y)
        ));
    }
    out.push_str(&format!("0\nSEQEND\n8\n{layer}\n"));
}

fn push_lwpolyline(out: &mut String, layer: &str, pts: &[(f64, f64)], closed: bool) {
    if pts.len() < 2 {
        return;
    }
    out.push_str(&format!(
        "0\nLWPOLYLINE\n8\n{layer}\n90\n{}\n70\n{}\n",
        pts.len(),
        if closed { 1 } else { 0 }
    ));
    for &(x, y) in pts {
        out.push_str(&format!("10\n{}\n20\n{}\n", dxf_num(x), dxf_num(y)));
    }
}

fn push_circle(out: &mut String, layer: &str, cx: f64, cy: f64, r: f64) {
    if !r.is_finite() || r <= 1e-6 {
        return;
    }
    out.push_str(&format!(
        "0\nCIRCLE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0\n40\n{}\n",
        dxf_num(cx),
        dxf_num(cy),
        dxf_num(r)
    ));
}

fn push_line(out: &mut String, layer: &str, x0: f64, y0: f64, x1: f64, y1: f64) {
    out.push_str(&format!(
        "0\nLINE\n8\n{layer}\n10\n{}\n20\n{}\n30\n0\n11\n{}\n21\n{}\n31\n0\n",
        dxf_num(x0),
        dxf_num(y0),
        dxf_num(x1),
        dxf_num(y1)
    ));
}

fn contour_to_cad_pts<F>(c: &Contour, tol_sq: f64, map_work: &F) -> Vec<(f64, f64)>
where
    F: Fn((f64, f64)) -> (f64, f64),
{
    let mut work_pts = Vec::with_capacity(c.curves.len() * 4 + 1);
    work_pts.push(c.start);
    let mut prev = c.start;
    for cv in &c.curves {
        flatten_cubic(prev, cv[0], cv[1], cv[2], tol_sq, 0, &mut work_pts);
        prev = cv[2];
    }
    // Drop redundant closing point when identical to start (POLYLINE flag 70=1 closes it).
    if work_pts.len() > 2 {
        let first = work_pts[0];
        let last = *work_pts.last().unwrap();
        if (first.0 - last.0).abs() < 1e-4 && (first.1 - last.1).abs() < 1e-4 {
            work_pts.pop();
        }
    }
    work_pts.into_iter().map(map_work).collect()
}

/// Export `Doc` to ASCII DXF (`AC1009`).
pub fn to_dxf(doc: &Doc, opts: &ExportOptions) -> String {
    let k = if doc.scale.is_finite() && doc.scale > 0.0 { doc.scale } else { 1.0 };
    let m = if opts.registration_marks { REG_MARGIN } else { 0.0 };

    let die_cut_contours = if opts.die_cut_contour {
        build_die_cut_contours(doc, opts)
    } else {
        Vec::new()
    };
    let (dc_min_x, dc_min_y, dc_max_x, dc_max_y) = if die_cut_contours.is_empty() {
        (0.0, 0.0, doc.width as f64, doc.height as f64)
    } else {
        die_cut_bounds_svg(doc, &die_cut_contours)
    };
    let shift_x = (-dc_min_x).max(0.0);
    let shift_y = (-dc_min_y).max(0.0);
    let content_w = dc_max_x - dc_min_x;
    let content_h = dc_max_y - dc_min_y;
    let total_w = content_w + 2.0 * m;
    let total_h = content_h + 2.0 * m;

    let unit_scale = match opts.dxf_unit {
        DxfUnit::Px => 1.0,
        DxfUnit::Mm => match opts.dxf_target_width_mm {
            Some(tw) if tw.is_finite() && tw > 0.0 && doc.width > 0 => tw as f64 / doc.width as f64,
            _ => MM_PER_PX_96DPI,
        },
    };
    let insunits = match opts.dxf_unit {
        DxfUnit::Mm => 4, // millimetres
        DxfUnit::Px => 0, // unitless
    };

    let ext_w = total_w * unit_scale;
    let ext_h = total_h * unit_scale;

    let map_svg = |sx: f64, sy: f64| -> (f64, f64) {
        ((sx + shift_x) * unit_scale, (total_h - (sy + shift_y)) * unit_scale)
    };
    let map_work = |p: (f64, f64)| -> (f64, f64) {
        map_svg(p.0 / k + m, p.1 / k + m)
    };
    // Adaptive flattening tolerance: 0.25 source px in working-pixel space.
    let tol_work = 0.25 * k;
    let tol_sq = tol_work * tol_work;

    // Collect active colour layers and their DXF layer names.
    let has_canvas_bg = doc.fields.first().map_or(false, |l| l.canvas);
    let active_fields: Vec<&Layer> = doc
        .fields
        .iter()
        .enumerate()
        .filter(|(i, _)| !(*i == 0 && has_canvas_bg && opts.strip_background))
        .map(|(_, l)| l)
        .collect();

    let is_pure_centerline = doc.fields.is_empty()
        && doc.line_shapes.is_empty()
        && doc.line_fallback.is_empty()
        && doc.strokes.iter().any(|sl| !sl.strokes.is_empty());

    let mut layer_defs: Vec<(String, u8)> = Vec::new();
    layer_defs.push(("0".to_string(), 7));
    let mut field_layer_names: Vec<String> = Vec::with_capacity(active_fields.len());
    for (idx, l) in active_fields.iter().enumerate() {
        let h = hex(l.color);
        let name = format!("COLOR_{:02}_{}", idx + 1, &h[1..]);
        layer_defs.push((name.clone(), rgb_to_aci(l.color)));
        field_layer_names.push(name);
    }

    let has_lines = match opts.lines_as {
        LinesAs::Shape => !doc.line_shapes.is_empty(),
        LinesAs::Stroke => !doc.line_fallback.is_empty() || doc.strokes.iter().any(|sl| !sl.strokes.is_empty()),
    };
    if is_pure_centerline {
        layer_defs.push(("CENTERLINE".to_string(), 7));
    } else if has_lines {
        layer_defs.push(("THIN_LINES".to_string(), 7));
    }
    if !die_cut_contours.is_empty() {
        layer_defs.push(("CUTCONTOUR".to_string(), 6)); // Magenta
    }
    if opts.registration_marks {
        layer_defs.push(("REGISTRATION".to_string(), 7));
    }

    let mut out = String::with_capacity(16 * 1024);

    // 1. HEADER
    out.push_str("0\nSECTION\n2\nHEADER\n");
    out.push_str("9\n$ACADVER\n1\nAC1009\n");
    out.push_str(&format!("9\n$INSUNITS\n70\n{insunits}\n"));
    out.push_str("9\n$EXTMIN\n10\n0\n20\n0\n30\n0\n");
    out.push_str(&format!(
        "9\n$EXTMAX\n10\n{}\n20\n{}\n30\n0\n",
        dxf_num(ext_w),
        dxf_num(ext_h)
    ));
    out.push_str("0\nENDSEC\n");

    // 2. TABLES (LTYPE + LAYER)
    out.push_str("0\nSECTION\n2\nTABLES\n");
    out.push_str("0\nTABLE\n2\nLTYPE\n70\n1\n");
    out.push_str("0\nLTYPE\n2\nCONTINUOUS\n70\n0\n3\nSolid line\n72\n65\n73\n0\n40\n0\n");
    out.push_str("0\nENDTAB\n");

    out.push_str(&format!("0\nTABLE\n2\nLAYER\n70\n{}\n", layer_defs.len()));
    for (name, aci) in &layer_defs {
        out.push_str(&format!(
            "0\nLAYER\n2\n{name}\n70\n0\n62\n{aci}\n6\nCONTINUOUS\n"
        ));
    }
    out.push_str("0\nENDTAB\n");
    out.push_str("0\nENDSEC\n");

    // 3. ENTITIES
    out.push_str("0\nSECTION\n2\nENTITIES\n");

    for (layer, lname) in active_fields.iter().zip(&field_layer_names) {
        for c in &layer.contours {
            let pts = contour_to_cad_pts(c, tol_sq, &map_work);
            push_polyline(&mut out, lname, &pts, true);
        }
        for prim in &layer.prims {
            match *prim {
                Primitive::Circle { cx, cy, r } => {
                    let (dcx, dcy) = map_work((cx, cy));
                    let dr = (r / k) * unit_scale;
                    push_circle(&mut out, lname, dcx, dcy, dr);
                }
                _ => {
                    let c = primitive_to_contour(prim);
                    let pts = contour_to_cad_pts(&c, tol_sq, &map_work);
                    push_polyline(&mut out, lname, &pts, true);
                }
            }
        }
    }

    if is_pure_centerline {
        for sl in &doc.strokes {
            for st in &sl.strokes {
                if st.curves.is_empty() {
                    continue;
                }
                let mut work_pts = Vec::with_capacity(st.curves.len() * 4 + 1);
                work_pts.push(st.curves[0][0]);
                for cv in &st.curves {
                    flatten_cubic(cv[0], cv[1], cv[2], cv[3], tol_sq, 0, &mut work_pts);
                }
                let cad_pts: Vec<(f64, f64)> = work_pts.into_iter().map(&map_work).collect();
                push_lwpolyline(&mut out, "CENTERLINE", &cad_pts, false);
            }
        }
    } else if has_lines {
        match opts.lines_as {
            LinesAs::Shape => {
                for l in &doc.line_shapes {
                    for c in &l.contours {
                        let pts = contour_to_cad_pts(c, tol_sq, &map_work);
                        push_polyline(&mut out, "THIN_LINES", &pts, true);
                    }
                }
            }
            LinesAs::Stroke => {
                for l in &doc.line_fallback {
                    for c in &l.contours {
                        let pts = contour_to_cad_pts(c, tol_sq, &map_work);
                        push_polyline(&mut out, "THIN_LINES", &pts, true);
                    }
                }
                for sl in &doc.strokes {
                    for st in &sl.strokes {
                        if st.curves.is_empty() {
                            continue;
                        }
                        let mut work_pts = Vec::with_capacity(st.curves.len() * 4 + 1);
                        work_pts.push(st.curves[0][0]);
                        for cv in &st.curves {
                            flatten_cubic(cv[0], cv[1], cv[2], cv[3], tol_sq, 0, &mut work_pts);
                        }
                        let cad_pts: Vec<(f64, f64)> = work_pts.into_iter().map(&map_work).collect();
                        push_polyline(&mut out, "THIN_LINES", &cad_pts, false);
                    }
                }
            }
        }
    }

    // Die-cut outer contour on dedicated CUTCONTOUR layer (closed POLYLINE)
    for c in &die_cut_contours {
        let pts = contour_to_cad_pts(c, tol_sq, &map_work);
        push_polyline(&mut out, "CUTCONTOUR", &pts, true);
    }

    if opts.registration_marks {
        let (w, h) = (content_w, content_h);
        let pts = [
            (m / 2.0 - shift_x, m / 2.0 - shift_y),
            (w + 1.5 * m - shift_x, m / 2.0 - shift_y),
            (m / 2.0 - shift_x, h + 1.5 * m - shift_y),
            (w + 1.5 * m - shift_x, h + 1.5 * m - shift_y),
        ];
        let r_cad = 6.0 * unit_scale;
        let arm = 9.0 * unit_scale;
        for (cx, cy) in pts {
            let (px, py) = map_svg(cx, cy);
            push_circle(&mut out, "REGISTRATION", px, py, r_cad);
            push_line(&mut out, "REGISTRATION", px - arm, py, px + arm, py);
            push_line(&mut out, "REGISTRATION", px, py - arm, px, py + arm);
        }
    }

    out.push_str("0\nENDSEC\n0\nEOF\n");
    out
}
