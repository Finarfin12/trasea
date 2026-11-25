//! Pre-Flight Vector Audit & Auto-Fix (Trasea v1.8.0).
//!
//! Inspects a `Doc` before production export (print / cutting / web) for:
//! - `thin_stroke`: strokes narrower than 0.5 pt
//! - `high_nodes`: total cubic nodes exceeding 3,000
//! - `similar_colors`: distinct palette swatches with CIE Lab ΔE < 10.0
//! - `micro_paths`: tiny non-canvas contours with bounding-box area < 4.0 px²

use crate::export::Doc;
use crate::palette::rgb_to_lab;
use crate::svg_import::coalesce_similar_colors;
use crate::trace::Contour;
use crate::Rgb;
use serde::{Deserialize, Serialize};

pub const MIN_STROKE_PT: f64 = 0.5;
pub const MAX_RECOMMENDED_NODES: usize = 3000;
pub const SIMILAR_COLOR_DELTA_E: f32 = 10.0;
pub const MICRO_PATH_AREA_PX2: f64 = 4.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditIssue {
    pub code: String,
    pub severity: String,
    pub count: usize,
    pub detail: String,
    pub fixable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditReport {
    pub ok: bool,
    pub total_paths: usize,
    pub total_nodes: usize,
    pub total_colors: usize,
    pub issues: Vec<AuditIssue>,
}

fn contour_bbox_area_src(c: &Contour, scale: f64) -> f64 {
    let s = if scale > 0.0 { scale } else { 1.0 };
    let (mut x0, mut y0, mut x1, mut y1) = (c.start.0, c.start.1, c.start.0, c.start.1);
    for cv in &c.curves {
        for p in cv {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    ((x1 - x0) / s).max(0.0) * ((y1 - y0) / s).max(0.0)
}

fn count_paths_and_nodes(doc: &Doc) -> (usize, usize) {
    let mut paths = 0usize;
    let mut nodes = 0usize;
    for l in &doc.fields {
        paths += l.contours.len() + l.prims.len();
        for c in &l.contours {
            nodes += c.curves.len().max(1);
        }
        nodes += l.prims.len() * 4;
    }
    for sl in &doc.strokes {
        paths += sl.strokes.len();
        for st in &sl.strokes {
            nodes += st.curves.len() + 1;
        }
    }
    (paths, nodes)
}

fn unique_colors(doc: &Doc) -> Vec<Rgb> {
    let mut out: Vec<Rgb> = Vec::new();
    for l in &doc.fields {
        if !out.contains(&l.color) {
            out.push(l.color);
        }
    }
    for sl in &doc.strokes {
        if !out.contains(&sl.color) {
            out.push(sl.color);
        }
    }
    out
}

fn count_similar_color_pairs(doc: &Doc) -> usize {
    let pal = unique_colors(doc);
    let labs: Vec<[f32; 3]> = pal.iter().map(|&c| rgb_to_lab(c)).collect();
    let max_d2 = SIMILAR_COLOR_DELTA_E * SIMILAR_COLOR_DELTA_E;
    let mut pairs = 0usize;
    for i in 0..labs.len() {
        for j in (i + 1)..labs.len() {
            let d2 = (labs[i][0] - labs[j][0]).powi(2)
                + (labs[i][1] - labs[j][1]).powi(2)
                + (labs[i][2] - labs[j][2]).powi(2);
            if d2 < max_d2 {
                pairs += 1;
            }
        }
    }
    pairs
}

/// Run a non-destructive pre-flight production audit on `doc`.
pub fn audit_doc(doc: &Doc) -> AuditReport {
    let scale = if doc.scale > 0.0 { doc.scale } else { 1.0 };
    let (total_paths, total_nodes) = count_paths_and_nodes(doc);
    let total_colors = unique_colors(doc).len();
    let mut issues = Vec::new();

    // 1. Thin strokes (< 0.5 pt, where 1 px at 96 DPI = 0.75 pt)
    let mut thin_strokes = 0usize;
    for sl in &doc.strokes {
        for st in &sl.strokes {
            let width_pt = (st.width / scale) * 0.75;
            if width_pt < MIN_STROKE_PT {
                thin_strokes += 1;
            }
        }
    }
    if thin_strokes > 0 {
        issues.push(AuditIssue {
            code: "thin_stroke".into(),
            severity: "warn".into(),
            count: thin_strokes,
            detail: format!("{thin_strokes} stroke < {MIN_STROKE_PT} pt"),
            fixable: true,
        });
    }

    // 2. High node count (> 3000)
    if total_nodes > MAX_RECOMMENDED_NODES {
        issues.push(AuditIssue {
            code: "high_nodes".into(),
            severity: "warn".into(),
            count: total_nodes,
            detail: format!("{total_nodes} nodes (> {MAX_RECOMMENDED_NODES})"),
            fixable: true,
        });
    }

    // 3. Similar colors (CIE Lab ΔE < 10.0)
    let similar_pairs = count_similar_color_pairs(doc);
    if similar_pairs > 0 {
        issues.push(AuditIssue {
            code: "similar_colors".into(),
            severity: "warn".into(),
            count: similar_pairs,
            detail: format!("{similar_pairs} color pair(s) with ΔE < 10"),
            fixable: true,
        });
    }

    // 4. Micro paths (non-canvas contour bbox area < 4.0 px²)
    let mut micro_paths = 0usize;
    for l in &doc.fields {
        if l.canvas {
            continue;
        }
        for c in &l.contours {
            if contour_bbox_area_src(c, scale) < MICRO_PATH_AREA_PX2 {
                micro_paths += 1;
            }
        }
    }
    if micro_paths > 0 {
        issues.push(AuditIssue {
            code: "micro_paths".into(),
            severity: "info".into(),
            count: micro_paths,
            detail: format!("{micro_paths} micro path(s) < {MICRO_PATH_AREA_PX2} px²"),
            fixable: true,
        });
    }

    AuditReport {
        ok: issues.is_empty(),
        total_paths,
        total_nodes,
        total_colors,
        issues,
    }
}

/// Apply safe automatic fixes to `doc`:
/// 1. Raise strokes thinner than 0.5 pt up to 0.5 pt.
/// 2. Merge near-duplicate colors (ΔE < 10.0).
/// 3. Drop non-canvas micro-path dust (< 4.0 px²).
/// 4. Lightly simplify redundant short segments when total nodes > 3000.
///
/// Returns the number of fixes applied.
pub fn auto_fix_doc(doc: &mut Doc) -> usize {
    let scale = if doc.scale > 0.0 { doc.scale } else { 1.0 };
    let mut fixes = 0usize;

    // 1. Raise thin strokes to 0.5 pt (= 0.5 / 0.75 px in source coords)
    let min_width_work = (MIN_STROKE_PT / 0.75) * scale;
    for sl in &mut doc.strokes {
        for st in &mut sl.strokes {
            let width_pt = (st.width / scale) * 0.75;
            if width_pt < MIN_STROKE_PT {
                st.width = min_width_work;
                fixes += 1;
            }
        }
    }

    // 2. Merge similar colors (ΔE < 10.0)
    let colors_before = unique_colors(doc).len();
    coalesce_similar_colors(doc, SIMILAR_COLOR_DELTA_E);
    let colors_after = unique_colors(doc).len();
    if colors_before > colors_after {
        fixes += colors_before - colors_after;
    }

    // 3. Remove micro-path dust on non-canvas layers
    for l in &mut doc.fields {
        if l.canvas {
            continue;
        }
        let before = l.contours.len();
        l.contours
            .retain(|c| contour_bbox_area_src(c, scale) >= MICRO_PATH_AREA_PX2);
        fixes += before.saturating_sub(l.contours.len());
    }
    doc.fields
        .retain(|l| l.canvas || !l.contours.is_empty() || !l.prims.is_empty());

    // 4. Light segment simplification if still above node budget
    let (_, nodes_now) = count_paths_and_nodes(doc);
    if nodes_now > MAX_RECOMMENDED_NODES {
        let min_seg_sq = (1.25 * scale).powi(2);
        for l in &mut doc.fields {
            if l.canvas {
                continue;
            }
            for c in &mut l.contours {
                if c.curves.len() <= 6 {
                    continue;
                }
                let mut prev = c.start;
                let mut kept = Vec::with_capacity(c.curves.len());
                let total = c.curves.len();
                for (idx, seg) in c.curves.iter().enumerate() {
                    let end = seg[2];
                    let dx = end.0 - prev.0;
                    let dy = end.1 - prev.1;
                    let is_last = idx + 1 == total;
                    if is_last || (dx * dx + dy * dy) >= min_seg_sq {
                        kept.push(*seg);
                        prev = end;
                    } else {
                        fixes += 1;
                    }
                }
                if kept.len() >= 3 {
                    c.curves = kept;
                }
            }
        }
    }

    fixes
}
