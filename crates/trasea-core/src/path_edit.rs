//! Non-destructive per-path edits (`Delete` and `ReassignColor`) using deterministic `data-trasea-id`
//! identifiers, applied consistently before statistics, SVG preview, and all vector exports.
//!
//! Deterministic ID format:
//! - Colour field contour compound (outer boundary + enclosed holes): `"f:{field_idx}:{compound_idx}"`
//! - Colour field primitive (`<circle>`/`<ellipse>`/`<rect>`): `"fp:{field_idx}:{prim_idx}"`
//! - Open stroke path: `"s:{stroke_layer_idx}:{stroke_idx}"`
//! - Line-layer contour compound: `"l:{layer_idx}:{compound_idx}"` (applies to `line_fallback` or `line_shapes`)

use crate::color::parse_hex_rgb;
use crate::export::{Doc, StrokeData, StrokeLayer};
use crate::primitives::Primitive;
use crate::trace::{Contour, Layer};
use crate::{hex, Rgb};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PathEdit {
    Delete {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
    },
    ReassignColor {
        #[serde(rename = "targetId", alias = "target_id")]
        target_id: String,
        #[serde(rename = "toHex", alias = "to_hex")]
        to_hex: String,
    },
}

fn contour_polygon(c: &Contour) -> Vec<(f64, f64)> {
    let mut pts = Vec::with_capacity(c.curves.len() * 4 + 1);
    pts.push(c.start);
    let mut prev = c.start;
    for cv in &c.curves {
        for step in 1..=4 {
            let t = step as f64 / 4.0;
            let mt = 1.0 - t;
            let x = mt * mt * mt * prev.0
                + 3.0 * mt * mt * t * cv[0].0
                + 3.0 * mt * t * t * cv[1].0
                + t * t * t * cv[2].0;
            let y = mt * mt * mt * prev.1
                + 3.0 * mt * mt * t * cv[0].1
                + 3.0 * mt * t * t * cv[1].1
                + t * t * t * cv[2].1;
            pts.push((x, y));
        }
        prev = cv[2];
    }
    pts
}

fn polygon_abs_area(pts: &[(f64, f64)]) -> f64 {
    if pts.len() < 3 {
        return 0.0;
    }
    let mut a = 0.0;
    let mut j = pts.len() - 1;
    for i in 0..pts.len() {
        a += (pts[j].0 + pts[i].0) * (pts[j].1 - pts[i].1);
        j = i;
    }
    (a * 0.5).abs()
}

fn polygon_bbox(pts: &[(f64, f64)]) -> (f64, f64, f64, f64) {
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in pts {
        if x < min_x { min_x = x; }
        if y < min_y { min_y = y; }
        if x > max_x { max_x = x; }
        if y > max_y { max_y = y; }
    }
    (min_x, min_y, max_x, max_y)
}

fn point_in_polygon(p: (f64, f64), poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = poly.len().saturating_sub(1);
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        let intersect = ((yi > p.1) != (yj > p.1))
            && (p.0 < (xj - xi) * (p.1 - yi) / (yj - yi + 1e-12) + xi);
        if intersect {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Group contours of a single layer into independent shapes (outer boundary + its enclosed holes)
/// so `fill-rule="evenodd"` holes always stay paired with their enclosing outer boundary while
/// separate islands each receive their own deterministic `data-trasea-id`.
pub fn group_contours_into_compounds(contours: &[Contour]) -> Vec<Vec<usize>> {
    let n = contours.len();
    if n == 0 {
        return Vec::new();
    }
    if n == 1 {
        return vec![vec![0]];
    }

    let polys: Vec<Vec<(f64, f64)>> = contours.iter().map(contour_polygon).collect();
    let areas: Vec<f64> = polys.iter().map(|p| polygon_abs_area(p)).collect();
    let bboxes: Vec<(f64, f64, f64, f64)> = polys.iter().map(|p| polygon_bbox(p)).collect();

    // Sort indices by area descending so any enclosing contour precedes its holes.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| areas[b].partial_cmp(&areas[a]).unwrap_or(std::cmp::Ordering::Equal));

    let mut depth = vec![0usize; n];
    let mut parent = vec![None::<usize>; n];

    for (pos_i, &i) in order.iter().enumerate() {
        let poly_i = &polys[i];
        if poly_i.is_empty() {
            continue;
        }
        let samples = [
            poly_i[0],
            poly_i[poly_i.len() / 3],
            poly_i[(poly_i.len() * 2) / 3],
        ];
        let mut enclosing = 0usize;
        let mut immediate: Option<usize> = None;
        for &j in &order[..pos_i] {
            if areas[j] <= areas[i] + 1e-6 {
                continue;
            }
            let (min_x, min_y, max_x, max_y) = bboxes[j];
            let hits = samples
                .iter()
                .filter(|&&s| {
                    s.0 >= min_x
                        && s.0 <= max_x
                        && s.1 >= min_y
                        && s.1 <= max_y
                        && point_in_polygon(s, &polys[j])
                })
                .count();
            if hits >= 2 {
                enclosing += 1;
                immediate = Some(j);
            }
        }
        depth[i] = enclosing;
        parent[i] = immediate;
    }

    // Even-depth contours start a compound (outer shell); odd-depth contours attach to their parent shell.
    // Emit shallowest depth first so nested depth-2 islands always paint cleanly after outer shells.
    let mut outer_indices: Vec<usize> = (0..n).filter(|&i| depth[i] % 2 == 0).collect();
    outer_indices.sort_by_key(|&i| (depth[i], i));

    let mut compound_of_outer: HashMap<usize, usize> = HashMap::new();
    let mut compounds: Vec<Vec<usize>> = Vec::new();

    for i in outer_indices {
        let cid = compounds.len();
        compound_of_outer.insert(i, cid);
        compounds.push(vec![i]);
    }
    for i in 0..n {
        if depth[i] % 2 == 1 {
            if let Some(p) = parent[i] {
                if let Some(&cid) = compound_of_outer.get(&p) {
                    compounds[cid].push(i);
                    continue;
                }
            }
            // Fallback if hole had no resolved outer parent
            compounds.push(vec![i]);
        }
    }

    compounds
}

/// Apply a sequence of `PathEdit` operations non-destructively to `doc`, returning a new `Doc`.
///
/// Each target is resolved against the document shown when that edit was made.
pub fn apply_path_edits(doc: &Doc, edits: &[PathEdit]) -> Doc {
    edits.iter().fold(doc.clone(), |current, edit| apply_single_path_edit(&current, edit))
}

fn apply_single_path_edit(doc: &Doc, edit: &PathEdit) -> Doc {
    let edits = std::slice::from_ref(edit);
    if edits.is_empty() {
        return doc.clone();
    }

    let mut deleted: HashSet<String> = HashSet::new();
    let mut recolored: HashMap<String, Rgb> = HashMap::new();

    for edit in edits {
        match edit {
            PathEdit::Delete { target_id } => {
                let id = target_id.trim().to_string();
                if !id.is_empty() {
                    recolored.remove(&id);
                    deleted.insert(id);
                }
            }
            PathEdit::ReassignColor { target_id, to_hex } => {
                let id = target_id.trim().to_string();
                if !id.is_empty() && !deleted.contains(&id) {
                    if let Some(rgb) = parse_hex_rgb(to_hex) {
                        recolored.insert(id, rgb);
                    }
                }
            }
        }
    }

    if deleted.is_empty() && recolored.is_empty() {
        return doc.clone();
    }

    // 1. Process colour fields (contours + primitives)
    let mut new_fields: Vec<Layer> = Vec::with_capacity(doc.fields.len());
    let mut moved_contours: Vec<(Rgb, Vec<Contour>)> = Vec::new();
    let mut moved_prims: Vec<(Rgb, Primitive)> = Vec::new();

    for (fi, layer) in doc.fields.iter().enumerate() {
        let compounds = group_contours_into_compounds(&layer.contours);
        let mut kept_contours: Vec<Contour> = Vec::new();
        for (ci, member_indices) in compounds.iter().enumerate() {
            let id = format!("f:{fi}:{ci}");
            if deleted.contains(&id) {
                continue;
            }
            if let Some(&target_rgb) = recolored.get(&id) {
                if target_rgb != layer.color {
                    let group: Vec<Contour> = member_indices.iter().map(|&idx| layer.contours[idx].clone()).collect();
                    moved_contours.push((target_rgb, group));
                    continue;
                }
            }
            for &idx in member_indices {
                kept_contours.push(layer.contours[idx].clone());
            }
        }

        let mut kept_prims: Vec<Primitive> = Vec::new();
        for (pi, prim) in layer.prims.iter().enumerate() {
            let id = format!("fp:{fi}:{pi}");
            if deleted.contains(&id) {
                continue;
            }
            if let Some(&target_rgb) = recolored.get(&id) {
                if target_rgb != layer.color {
                    moved_prims.push((target_rgb, prim.clone()));
                    continue;
                }
            }
            kept_prims.push(prim.clone());
        }

        if !kept_contours.is_empty() || !kept_prims.is_empty() {
            new_fields.push(Layer {
                id: layer.id.clone(),
                color: layer.color,
                contours: kept_contours,
                canvas: layer.canvas,
                prims: kept_prims,
            });
        }
    }

    // Helper: attach moved contours/primitives to existing non-canvas layer of `target_rgb`, or create a new layer.
    let find_or_add_field = |fields: &mut Vec<Layer>, target_rgb: Rgb| -> usize {
        if let Some(pos) = fields.iter().position(|l| l.color == target_rgb && !l.canvas) {
            pos
        } else if let Some(pos) = fields.iter().position(|l| l.color == target_rgb) {
            pos
        } else {
            fields.push(Layer {
                id: hex(target_rgb),
                color: target_rgb,
                contours: Vec::new(),
                canvas: false,
                prims: Vec::new(),
            });
            fields.len() - 1
        }
    };

    for (target_rgb, group) in moved_contours {
        let dst = find_or_add_field(&mut new_fields, target_rgb);
        new_fields[dst].contours.extend(group);
    }
    for (target_rgb, prim) in moved_prims {
        let dst = find_or_add_field(&mut new_fields, target_rgb);
        new_fields[dst].prims.push(prim);
    }

    // 2. Process line layers (`line_shapes` and `line_fallback`)
    let process_line_layers = |layers: &[Layer], prefix: &str| -> Vec<Layer> {
        let mut out: Vec<Layer> = Vec::with_capacity(layers.len());
        let mut moved: Vec<(Rgb, Vec<Contour>)> = Vec::new();
        for (li, layer) in layers.iter().enumerate() {
            let compounds = group_contours_into_compounds(&layer.contours);
            let mut kept = Vec::new();
            for (ci, member_indices) in compounds.iter().enumerate() {
                let id_specific = format!("{prefix}:{li}:{ci}");
                let id_generic = format!("l:{li}:{ci}");
                if deleted.contains(&id_specific) || deleted.contains(&id_generic) {
                    continue;
                }
                let target = recolored.get(&id_specific).or_else(|| recolored.get(&id_generic));
                if let Some(&target_rgb) = target {
                    if target_rgb != layer.color {
                        let group: Vec<Contour> = member_indices.iter().map(|&idx| layer.contours[idx].clone()).collect();
                        moved.push((target_rgb, group));
                        continue;
                    }
                }
                for &idx in member_indices {
                    kept.push(layer.contours[idx].clone());
                }
            }
            if !kept.is_empty() {
                out.push(Layer {
                    id: layer.id.clone(),
                    color: layer.color,
                    contours: kept,
                    canvas: false,
                    prims: Vec::new(),
                });
            }
        }
        for (target_rgb, group) in moved {
            if let Some(pos) = out.iter().position(|l| l.color == target_rgb) {
                out[pos].contours.extend(group);
            } else {
                out.push(Layer {
                    id: hex(target_rgb),
                    color: target_rgb,
                    contours: group,
                    canvas: false,
                    prims: Vec::new(),
                });
            }
        }
        out
    };

    let new_line_shapes = process_line_layers(&doc.line_shapes, "ls");
    let new_line_fallback = process_line_layers(&doc.line_fallback, "lf");

    // 3. Process open strokes
    let mut new_strokes: Vec<StrokeLayer> = Vec::with_capacity(doc.strokes.len());
    let mut moved_strokes: Vec<(Rgb, StrokeData)> = Vec::new();
    for (sli, sl) in doc.strokes.iter().enumerate() {
        let mut kept = Vec::new();
        for (si, st) in sl.strokes.iter().enumerate() {
            let id = format!("s:{sli}:{si}");
            if deleted.contains(&id) {
                continue;
            }
            if let Some(&target_rgb) = recolored.get(&id) {
                if target_rgb != sl.color {
                    moved_strokes.push((target_rgb, st.clone()));
                    continue;
                }
            }
            kept.push(st.clone());
        }
        if !kept.is_empty() {
            new_strokes.push(StrokeLayer {
                color: sl.color,
                strokes: kept,
            });
        }
    }
    for (target_rgb, st) in moved_strokes {
        if let Some(pos) = new_strokes.iter().position(|sl| sl.color == target_rgb) {
            new_strokes[pos].strokes.push(st);
        } else {
            new_strokes.push(StrokeLayer {
                color: target_rgb,
                strokes: vec![st],
            });
        }
    }

    Doc {
        width: doc.width,
        height: doc.height,
        scale: doc.scale,
        fields: new_fields,
        line_shapes: new_line_shapes,
        line_fallback: new_line_fallback,
        strokes: new_strokes,
    }
}
