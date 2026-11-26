//! Automated test suite for Trasea 1.3.0 ("Editor Node & Vektor Pro"), stage 1 (core engine): G13-01 .. G13-10.

use image::{Rgb, RgbImage};
use std::path::Path;
use trasea_core::color::{ColorSpace, SpotColorMapping};
use trasea_core::dxf::{to_dxf, DxfUnit};
use trasea_core::export::{to_eps, to_pdf, to_svg, Doc, ExportOptions, StrokeData, StrokeLayer};
use trasea_core::node_edit::{
    apply_node_edits, apply_node_edits_report, inspect_path_nodes, primitive_to_contour, HandleSide, NodeEdit,
    PathNodeGeometry,
};
use trasea_core::path_edit::PathEdit;
use trasea_core::preprocess::PreprocessOptions;
use trasea_core::primitives::Primitive;
use trasea_core::project::{
    load_project, save_project, ProjectExportOptions, ProjectFile, ProjectOptions, PROJECT_SCHEMA_VERSION,
};
use trasea_core::stats::count_svg;
use trasea_core::svg_import::{load_svg_doc, parse_svg_doc};
use trasea_core::trace::{Contour, Layer};
use trasea_core::{vectorize, vectorize_svg, Params};

// ---------------------------------------------------------------------------------------------------------------
// builders

fn line_cubic(a: (f64, f64), b: (f64, f64)) -> [(f64, f64); 3] {
    let d = (b.0 - a.0, b.1 - a.1);
    [(a.0 + d.0 / 3.0, a.1 + d.1 / 3.0), (a.0 + 2.0 * d.0 / 3.0, a.1 + 2.0 * d.1 / 3.0), b]
}

/// Closed polygon with straight edges (every node is a sharp corner).
fn polygon(pts: &[(f64, f64)]) -> Contour {
    let n = pts.len();
    Contour {
        start: pts[0],
        curves: (0..n).map(|i| line_cubic(pts[i], pts[(i + 1) % n])).collect(),
        prim: None,
    }
}

/// Smooth circle of `n` nodes (every node has collinear handles).
fn circle(cx: f64, cy: f64, r: f64, n: usize) -> Contour {
    let th = std::f64::consts::TAU / n as f64;
    let h = 4.0 / 3.0 * (th / 4.0).tan() * r;
    let at = |i: usize| {
        let a = th * i as f64;
        (cx + r * a.cos(), cy + r * a.sin())
    };
    let tan = |i: usize| {
        let a = th * i as f64;
        (-a.sin(), a.cos())
    };
    let curves = (0..n)
        .map(|i| {
            let (p0, p1) = (at(i), at((i + 1) % n));
            let (t0, t1) = (tan(i), tan((i + 1) % n));
            [(p0.0 + t0.0 * h, p0.1 + t0.1 * h), (p1.0 - t1.0 * h, p1.1 - t1.1 * h), p1]
        })
        .collect();
    Contour { start: at(0), curves, prim: None }
}

fn layer(color: [u8; 3], contours: Vec<Contour>) -> Layer {
    Layer { id: trasea_core::hex(color), color, contours, canvas: false, prims: Vec::new() }
}

fn doc_with(fields: Vec<Layer>, strokes: Vec<StrokeLayer>, scale: f64) -> Doc {
    Doc {
        width: (200.0 * scale) as u32,
        height: (200.0 * scale) as u32,
        scale,
        fields,
        line_shapes: Vec::new(),
        line_fallback: Vec::new(),
        strokes,
    }
}

fn square(x: f64, y: f64, s: f64) -> Contour {
    polygon(&[(x, y), (x + s, y), (x + s, y + s), (x, y + s)])
}

fn geo(doc: &Doc, id: &str) -> PathNodeGeometry {
    inspect_path_nodes(doc, id).unwrap_or_else(|| panic!("inspect {id}"))
}

fn dist(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}

fn mv(id: &str, sub: usize, node: usize, x: f64, y: f64) -> NodeEdit {
    NodeEdit::MoveNode { target_id: id.into(), subpath_idx: sub, node_idx: node, x, y }
}

// ---------------------------------------------------------------------------------------------------------------

/// G13-01 — Inspeksi geometri node (`inspect_path_nodes`)
#[test]
fn g13_01_inspect_nodes_closed_and_open() {
    let stroke = StrokeData {
        curves: vec![
            [(10.0, 100.0), (20.0, 90.0), (30.0, 90.0), (40.0, 100.0)],
            [(40.0, 100.0), (50.0, 110.0), (60.0, 110.0), (70.0, 100.0)],
            [(70.0, 100.0), (80.0, 90.0), (90.0, 90.0), (100.0, 100.0)],
        ],
        width: 2.0,
    };
    let doc = doc_with(
        vec![layer([255, 0, 0], vec![square(10.0, 10.0, 40.0)]), layer([0, 0, 255], vec![circle(120.0, 60.0, 30.0, 8)])],
        vec![StrokeLayer { color: [0, 0, 0], strokes: vec![stroke] }],
        1.0,
    );

    let sq = geo(&doc, "f:0:0");
    assert_eq!(sq.total_nodes, 4, "square: nodes == segments");
    assert_eq!(sq.color_hex, "#FF0000");
    assert_eq!(sq.subpaths.len(), 1);
    assert!(sq.subpaths[0].closed);
    assert!(sq.subpaths[0].nodes.iter().all(|n| !n.is_smooth), "90 degree corners must be Corner");
    for n in &sq.subpaths[0].nodes {
        assert!(n.x.is_finite() && n.y.is_finite());
        assert!(n.handle_in.is_some() && n.handle_out.is_some());
    }

    let ci = geo(&doc, "f:1:0");
    assert_eq!(ci.total_nodes, 8);
    assert!(ci.subpaths[0].nodes.iter().all(|n| n.is_smooth), "circle nodes must be Smooth");

    let st = geo(&doc, "s:0:0");
    assert_eq!(st.total_nodes, 4, "open stroke with 3 segments has 4 nodes");
    assert!(!st.subpaths[0].closed);
    let nodes = &st.subpaths[0].nodes;
    assert!(nodes[0].handle_in.is_none() && nodes[0].handle_out.is_some());
    assert!(nodes[3].handle_in.is_some() && nodes[3].handle_out.is_none());
    assert!(nodes[1].handle_in.is_some() && nodes[1].handle_out.is_some());

    assert!(inspect_path_nodes(&doc, "f:9:0").is_none());
    assert!(inspect_path_nodes(&doc, "nonsense").is_none());

    // source-pixel coordinates: a doc worked at scale 2 reports the same geometry in source pixels
    let doc2 = doc_with(vec![layer([255, 0, 0], vec![square(20.0, 20.0, 80.0)])], vec![], 2.0);
    let g2 = geo(&doc2, "f:0:0");
    assert!((g2.subpaths[0].nodes[0].x - 10.0).abs() < 1e-9 && (g2.subpaths[0].nodes[2].y - 50.0).abs() < 1e-9);

    // the JSON is camelCase for the front end
    let json = serde_json::to_string(&sq).unwrap();
    assert!(json.contains("\"targetId\"") && json.contains("\"colorHex\"") && json.contains("\"isSmooth\""));
}

/// G13-02 — Geser node & handle (`MoveNode`, `MoveHandle`)
#[test]
fn g13_02_move_node_and_handle() {
    let doc = doc_with(vec![layer([0, 128, 0], vec![circle(100.0, 100.0, 40.0, 8)])], vec![], 1.0);
    let before = geo(&doc, "f:0:0").subpaths[0].nodes.clone();

    let moved = apply_node_edits(&doc, &[mv("f:0:0", 0, 3, before[3].x + 5.0, before[3].y - 7.0)]);
    let after = geo(&moved, "f:0:0").subpaths[0].nodes.clone();
    let (dx, dy) = (5.0, -7.0);
    assert!((after[3].x - before[3].x - dx).abs() < 1e-9 && (after[3].y - before[3].y - dy).abs() < 1e-9);
    let (hi0, hi1) = (before[3].handle_in.unwrap(), after[3].handle_in.unwrap());
    let (ho0, ho1) = (before[3].handle_out.unwrap(), after[3].handle_out.unwrap());
    assert!((hi1.0 - hi0.0 - dx).abs() < 1e-9 && (hi1.1 - hi0.1 - dy).abs() < 1e-9, "in-handle follows the node");
    assert!((ho1.0 - ho0.0 - dx).abs() < 1e-9 && (ho1.1 - ho0.1 - dy).abs() < 1e-9, "out-handle follows the node");
    for i in [0usize, 1, 2, 4, 5, 6, 7] {
        assert!((after[i].x - before[i].x).abs() < 1e-9 && (after[i].y - before[i].y).abs() < 1e-9, "other nodes stay");
        assert_eq!(after[i].handle_out, before[i].handle_out);
    }
    // the original doc is untouched (non-destructive)
    assert_eq!(geo(&doc, "f:0:0").subpaths[0].nodes[3].x, before[3].x);

    // scale 2: edit coordinates are source pixels
    let doc2 = doc_with(vec![layer([0, 128, 0], vec![circle(200.0, 200.0, 80.0, 8)])], vec![], 2.0);
    let b2 = geo(&doc2, "f:0:0").subpaths[0].nodes.clone();
    let m2 = apply_node_edits(&doc2, &[mv("f:0:0", 0, 0, b2[0].x + 3.0, b2[0].y)]);
    let a2 = geo(&m2, "f:0:0").subpaths[0].nodes.clone();
    assert!((a2[0].x - b2[0].x - 3.0).abs() < 1e-9);

    // MoveHandle with mirror_smooth: the opposite handle stays collinear and keeps its length
    let n = &before[2];
    let target = (n.handle_out.unwrap().0 + 6.0, n.handle_out.unwrap().1 + 9.0);
    let mir = apply_node_edits(
        &doc,
        &[NodeEdit::MoveHandle {
            target_id: "f:0:0".into(),
            subpath_idx: 0,
            node_idx: 2,
            which: HandleSide::Out,
            x: target.0,
            y: target.1,
            mirror_smooth: true,
        }],
    );
    let m = geo(&mir, "f:0:0").subpaths[0].nodes[2].clone();
    assert_eq!(m.handle_out.unwrap(), target);
    let (a, b) = ((m.x - m.handle_in.unwrap().0, m.y - m.handle_in.unwrap().1), (m.handle_out.unwrap().0 - m.x, m.handle_out.unwrap().1 - m.y));
    let cross = a.0 * b.1 - a.1 * b.0;
    assert!(cross.abs() < 1e-6 * (a.0.hypot(a.1) * b.0.hypot(b.1)), "handles stay collinear");
    assert!(a.0 * b.0 + a.1 * b.1 > 0.0, "handles point in opposite directions");
    let old_len = dist((n.x, n.y), n.handle_in.unwrap());
    assert!((dist((m.x, m.y), m.handle_in.unwrap()) - old_len).abs() < 1e-9, "opposite handle keeps its length");
    assert!(m.is_smooth);

    // mirror_smooth = false: only this handle moves
    let free = apply_node_edits(
        &doc,
        &[NodeEdit::MoveHandle {
            target_id: "f:0:0".into(),
            subpath_idx: 0,
            node_idx: 2,
            which: HandleSide::Out,
            x: target.0,
            y: target.1,
            mirror_smooth: false,
        }],
    );
    let f = geo(&free, "f:0:0").subpaths[0].nodes[2].clone();
    assert_eq!(f.handle_out.unwrap(), target);
    assert_eq!(f.handle_in, n.handle_in, "opposite handle must not move without mirror");
    assert!(!f.is_smooth);
}

/// G13-03 — Hapus node & batas minimum (`DeleteNode`)
#[test]
fn g13_03_delete_node_and_minimum() {
    let hexagon: Vec<(f64, f64)> = (0..6)
        .map(|i| {
            let a = std::f64::consts::TAU / 6.0 * i as f64;
            (100.0 + 40.0 * a.cos(), 100.0 + 40.0 * a.sin())
        })
        .collect();
    let tri = polygon(&[(10.0, 10.0), (60.0, 10.0), (35.0, 50.0)]);
    let line = StrokeData { curves: vec![[(5.0, 150.0), (20.0, 150.0), (40.0, 150.0), (60.0, 150.0)]], width: 2.0 };
    let doc = doc_with(
        vec![layer([10, 10, 10], vec![polygon(&hexagon)]), layer([200, 0, 0], vec![tri])],
        vec![StrokeLayer { color: [0, 0, 0], strokes: vec![line] }],
        1.0,
    );

    let (out, rep) = apply_node_edits_report(
        &doc,
        &[NodeEdit::DeleteNode { target_id: "f:0:0".into(), subpath_idx: 0, node_idx: 2 }],
    );
    assert_eq!(rep.applied, 1);
    let g = geo(&out, "f:0:0");
    assert_eq!(g.total_nodes, 5, "hexagon minus one node");
    assert!(g.subpaths[0].closed);
    let nodes = &g.subpaths[0].nodes;
    assert!(nodes.iter().all(|n| n.x.is_finite() && n.y.is_finite()));
    // the neighbours of the deleted vertex are now joined by a straight edge
    let (a, b) = (&nodes[1], &nodes[2]);
    let (ho, hi) = (a.handle_out.unwrap(), b.handle_in.unwrap());
    let chord_dev = |p: (f64, f64)| ((p.0 - a.x) * (b.y - a.y) - (p.1 - a.y) * (b.x - a.x)).abs() / dist((a.x, a.y), (b.x, b.y));
    assert!(chord_dev(ho) < 0.05 && chord_dev(hi) < 0.05, "two straight edges merge into one straight edge");

    // deleting the first node of a closed path also works (the start moves)
    let first = apply_node_edits(&doc, &[NodeEdit::DeleteNode { target_id: "f:0:0".into(), subpath_idx: 0, node_idx: 0 }]);
    assert_eq!(geo(&first, "f:0:0").total_nodes, 5);

    // a triangle (3 nodes) and a single segment (2 nodes) refuse safely
    let (tri_out, rep_t) = apply_node_edits_report(
        &doc,
        &[NodeEdit::DeleteNode { target_id: "f:1:0".into(), subpath_idx: 0, node_idx: 1 }],
    );
    assert_eq!(geo(&tri_out, "f:1:0").total_nodes, 3);
    assert_eq!(rep_t.applied, 0);
    assert!(rep_t.skipped.iter().any(|s| s.contains("Batas minimum node")), "{:?}", rep_t.skipped);
    let (st_out, rep_s) = apply_node_edits_report(
        &doc,
        &[NodeEdit::DeleteNode { target_id: "s:0:0".into(), subpath_idx: 0, node_idx: 0 }],
    );
    assert_eq!(geo(&st_out, "s:0:0").total_nodes, 2);
    assert_eq!(rep_s.applied, 0);

    // out-of-range indices and unknown ids never panic
    let (_, rep_x) = apply_node_edits_report(
        &doc,
        &[
            NodeEdit::DeleteNode { target_id: "f:0:0".into(), subpath_idx: 5, node_idx: 0 },
            NodeEdit::DeleteNode { target_id: "f:0:0".into(), subpath_idx: 0, node_idx: 99 },
            NodeEdit::DeleteNode { target_id: "zzz".into(), subpath_idx: 0, node_idx: 0 },
        ],
    );
    assert_eq!(rep_x.applied, 0);
    assert_eq!(rep_x.skipped.len(), 3);

    // an open stroke with several nodes: deleting an end node and a middle node
    let stroke = StrokeData {
        curves: (0..4).map(|i| {
            let x = 10.0 + 20.0 * i as f64;
            [(x, 50.0), (x + 6.0, 40.0), (x + 14.0, 60.0), (x + 20.0, 50.0)]
        }).collect(),
        width: 2.0,
    };
    let d2 = doc_with(vec![], vec![StrokeLayer { color: [0, 0, 0], strokes: vec![stroke] }], 1.0);
    let mid = apply_node_edits(&d2, &[NodeEdit::DeleteNode { target_id: "s:0:0".into(), subpath_idx: 0, node_idx: 2 }]);
    assert_eq!(geo(&mid, "s:0:0").total_nodes, 4);
    let end = apply_node_edits(&d2, &[NodeEdit::DeleteNode { target_id: "s:0:0".into(), subpath_idx: 0, node_idx: 4 }]);
    let ge = geo(&end, "s:0:0");
    assert_eq!(ge.total_nodes, 4);
    assert!(ge.subpaths[0].nodes[3].handle_out.is_none());
}

/// G13-04 — Toggle Corner <-> Smooth (`ToggleSmooth`)
#[test]
fn g13_04_toggle_corner_smooth() {
    let doc = doc_with(vec![layer([0, 0, 0], vec![square(50.0, 50.0, 60.0), circle(150.0, 150.0, 20.0, 8)])], vec![], 1.0);
    // contours 0 (square) and 1 (circle) are separate islands: separate compounds
    let id_sq = "f:0:0";
    let sq0 = geo(&doc, id_sq);
    assert_eq!(sq0.total_nodes, 4, "{sq0:?}");
    assert!(!sq0.subpaths[0].nodes[1].is_smooth);

    let smooth = apply_node_edits(&doc, &[NodeEdit::ToggleSmooth { target_id: id_sq.into(), subpath_idx: 0, node_idx: 1 }]);
    let n1 = geo(&smooth, id_sq).subpaths[0].nodes[1].clone();
    assert!(n1.is_smooth, "corner -> smooth: handles must become collinear tangents");
    assert!(n1.handle_in.unwrap() != (n1.x, n1.y) && n1.handle_out.unwrap() != (n1.x, n1.y));

    // the second toggle folds the handles back to a sharp corner
    let corner = apply_node_edits(&smooth, &[NodeEdit::ToggleSmooth { target_id: id_sq.into(), subpath_idx: 0, node_idx: 1 }]);
    let n1b = geo(&corner, id_sq).subpaths[0].nodes[1].clone();
    assert!(!n1b.is_smooth, "smooth -> corner");
    assert_eq!(n1b.handle_in.unwrap(), (n1b.x, n1b.y));
    assert_eq!(n1b.handle_out.unwrap(), (n1b.x, n1b.y));

    // both toggles in one batch return to a corner as well
    let both = apply_node_edits(
        &doc,
        &[
            NodeEdit::ToggleSmooth { target_id: id_sq.into(), subpath_idx: 0, node_idx: 1 },
            NodeEdit::ToggleSmooth { target_id: id_sq.into(), subpath_idx: 0, node_idx: 1 },
        ],
    );
    assert!(!geo(&both, id_sq).subpaths[0].nodes[1].is_smooth);

    // the end node of an open stroke has no corner to toggle
    let stroke = StrokeData { curves: vec![[(0.0, 0.0), (5.0, 5.0), (15.0, 5.0), (20.0, 0.0)]], width: 1.0 };
    let d = doc_with(vec![], vec![StrokeLayer { color: [0, 0, 0], strokes: vec![stroke] }], 1.0);
    let (_, rep) = apply_node_edits_report(&d, &[NodeEdit::ToggleSmooth { target_id: "s:0:0".into(), subpath_idx: 0, node_idx: 0 }]);
    assert_eq!(rep.applied, 0);
}

/// G13-05 — Simplify path terpilih & konversi primitif
#[test]
fn g13_05_simplify_path_and_primitive_conversion() {
    let doc = doc_with(
        vec![layer([255, 0, 0], vec![square(10.0, 10.0, 30.0)]), layer([0, 0, 255], vec![circle(120.0, 100.0, 40.0, 64)])],
        vec![],
        1.0,
    );
    let sq_before = geo(&doc, "f:0:0");
    assert_eq!(geo(&doc, "f:1:0").total_nodes, 64);

    let out = apply_node_edits(&doc, &[NodeEdit::SimplifyPath { target_id: "f:1:0".into(), tolerance_px: 1.0 }]);
    let simplified = geo(&out, "f:1:0");
    assert!(simplified.total_nodes <= 16, "64-node circle must shrink a lot, got {}", simplified.total_nodes);
    assert!(simplified.total_nodes >= 3);
    // shape preserved: every simplified anchor still lies on the circle
    for n in &simplified.subpaths[0].nodes {
        assert!((dist((n.x, n.y), (120.0, 100.0)) - 40.0).abs() < 1.5, "anchor drifted off the circle: {n:?}");
    }
    // the other path is untouched
    let sq_after = geo(&out, "f:0:0");
    assert_eq!(sq_after.total_nodes, sq_before.total_nodes);
    assert_eq!(serde_json::to_string(&sq_after).unwrap(), serde_json::to_string(&sq_before).unwrap());

    // a very dense open stroke is simplified too
    let dense = StrokeData {
        curves: (0..60)
            .map(|i| {
                let (x0, x1) = (i as f64 * 3.0, (i + 1) as f64 * 3.0);
                let (y0, y1) = (100.0 + 20.0 * (x0 / 25.0).sin(), 100.0 + 20.0 * (x1 / 25.0).sin());
                [(x0, y0), (x0 + 1.0, y0 + (y1 - y0) / 3.0), (x0 + 2.0, y0 + 2.0 * (y1 - y0) / 3.0), (x1, y1)]
            })
            .collect(),
        width: 2.0,
    };
    let d2 = doc_with(vec![], vec![StrokeLayer { color: [0, 0, 0], strokes: vec![dense] }], 1.0);
    assert_eq!(geo(&d2, "s:0:0").total_nodes, 61);
    let s2 = apply_node_edits(&d2, &[NodeEdit::SimplifyPath { target_id: "s:0:0".into(), tolerance_px: 1.0 }]);
    let gs = geo(&s2, "s:0:0");
    assert!(gs.total_nodes < 20, "dense stroke simplified, got {}", gs.total_nodes);
    assert_eq!(s2.strokes[0].strokes[0].width, 2.0, "stroke width kept");

    // a primitive circle turns into 4 editable Bezier segments when a node is moved
    let mut pd = doc_with(vec![layer([0, 160, 0], vec![])], vec![], 1.0);
    pd.fields[0].prims.push(Primitive::Circle { cx: 80.0, cy: 80.0, r: 30.0 });
    pd.fields[0].prims.push(Primitive::Rect { x: 120.0, y: 20.0, w: 40.0, h: 30.0, rx: 0.0, angle_deg: 0.0 });
    let pg = geo(&pd, "fp:0:0");
    assert_eq!(pg.total_nodes, 4);
    assert!(pg.subpaths[0].nodes.iter().all(|n| n.is_smooth));
    assert_eq!(geo(&pd, "fp:0:1").total_nodes, 4);
    assert!(geo(&pd, "fp:0:1").subpaths[0].nodes.iter().all(|n| !n.is_smooth));
    let svg_before = to_svg(&pd, &ExportOptions::default());
    assert!(svg_before.contains("<circle ") && svg_before.contains("<rect "));

    let edited = apply_node_edits(&pd, &[mv("fp:0:0", 0, 0, 120.0, 80.0)]);
    assert_eq!(edited.fields[0].prims.len(), 1, "the circle left the primitives");
    assert_eq!(edited.fields[0].contours.len(), 1, "and became a contour");
    assert_eq!(edited.fields[0].contours[0].curves.len(), 4, "4 cubic segments (kappa)");
    let svg_after = to_svg(&edited, &ExportOptions::default());
    assert!(!svg_after.contains("<circle ") && svg_after.contains("<rect "), "{svg_after}");
    // the converted outline matches the primitive area before the move (circle r=30 -> ~2827 px2)
    let c = primitive_to_contour(&Primitive::Circle { cx: 80.0, cy: 80.0, r: 30.0 });
    let pts: Vec<(f64, f64)> = {
        let mut v = vec![c.start];
        let mut prev = c.start;
        for cv in &c.curves {
            for s in 1..=16 {
                let t = s as f64 / 16.0;
                let mt = 1.0 - t;
                v.push((
                    mt * mt * mt * prev.0 + 3.0 * mt * mt * t * cv[0].0 + 3.0 * mt * t * t * cv[1].0 + t * t * t * cv[2].0,
                    mt * mt * mt * prev.1 + 3.0 * mt * mt * t * cv[0].1 + 3.0 * mt * t * t * cv[1].1 + t * t * t * cv[2].1,
                ));
            }
            prev = cv[2];
        }
        v
    };
    let mut area = 0.0;
    for i in 0..pts.len() {
        let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
        area += a.0 * b.1 - b.0 * a.1;
    }
    let expect = std::f64::consts::PI * 900.0;
    assert!((area.abs() / 2.0 - expect).abs() / expect < 0.005, "kappa circle area");
    // an ellipse and a rounded, rotated rectangle convert as well
    let e = primitive_to_contour(&Primitive::Ellipse { cx: 0.0, cy: 0.0, rx: 20.0, ry: 10.0, angle_deg: 30.0 });
    assert_eq!(e.curves.len(), 4);
    let r = primitive_to_contour(&Primitive::Rect { x: 0.0, y: 0.0, w: 40.0, h: 20.0, rx: 4.0, angle_deg: 15.0 });
    assert_eq!(r.curves.len(), 8);
}

/// G13-06 — Round-trip `.trasea` & propagasi ekspor
#[test]
fn g13_06_project_roundtrip_and_export_propagation() {
    // legacy 1.0 - 1.2 projects (no `nodeEdits`) still deserialize
    for ver in ["1.0.0", "1.1.0", "1.2.0"] {
        let json = format!(
            r#"{{"schemaVersion":1,"appVersion":"{ver}","sourcePath":"a.png","sourceSha256":"x",
            "options":{{"colors":8,"smooth":0.5,"keepLines":true,"aiClean":false,"detectPrimitives":true,
            "symmetry":"off","detectText":true,"pathEdits":[{{"kind":"delete","targetId":"f:1:0"}}]}},
            "exportOptions":{{"linesAs":"stroke","trapPt":0.0}}}}"#
        );
        let p: ProjectFile = serde_json::from_str(&json).unwrap_or_else(|e| panic!("{ver}: {e}"));
        assert!(p.options.node_edits.is_empty(), "{ver}");
    }

    // a 1.3.0 project round-trips with every NodeEdit kind
    let edits = vec![
        mv("f:1:0", 0, 2, 12.5, 7.25),
        NodeEdit::MoveHandle { target_id: "f:1:0".into(), subpath_idx: 0, node_idx: 1, which: HandleSide::In, x: 1.0, y: 2.0, mirror_smooth: true },
        NodeEdit::DeleteNode { target_id: "s:0:0".into(), subpath_idx: 0, node_idx: 3 },
        NodeEdit::ToggleSmooth { target_id: "f:2:1".into(), subpath_idx: 1, node_idx: 0 },
        NodeEdit::SimplifyPath { target_id: "fp:1:0".into(), tolerance_px: 1.5 },
    ];
    let proj = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: "1.3.0".into(),
        source_path: "logo.svg".into(),
        source_sha256: "abc".into(),
        options: ProjectOptions {
            brand_palette_locked: false,
            colors: 6,
            smooth: 50.0,
            keep_lines: true,
            ai_clean: false,
            detect_primitives: true,
            symmetry: "off".into(),
            detect_text: true,
            custom_palette: None,
            palette_merges: Vec::new(),
            target_nodes: None,
            brush_strokes: Vec::new(),
            production_mode: "standard".into(),
            min_stroke_pt: 0.0,
            strip_background: false,
            registration_marks: false,
            sablon_knockout: false,
            preprocess: PreprocessOptions::default(),
            centerline_invert: false,
            centerline_stroke_pt: 1.0,
            path_edits: vec![PathEdit::Delete { target_id: "f:3:0".into() }],
            node_edits: edits.clone(),
            edit_fingerprint: None,
        },
        export_options: ProjectExportOptions {
            microstock_ready: false,
            lines_as: "stroke".into(),
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: false,
            color_space: ColorSpace::Rgb,
            dxf_unit: DxfUnit::Mm,
            dxf_target_width_mm: None,
            die_cut_contour: false,
            die_cut_offset_mm: 2.0,
            die_cut_spot_name: "CutContour".into(),
            die_cut_bridge_islands: true,
            spot_colors: Vec::new(),
            sticker_backing: false,
            sticker_backing_color: "#FFFFFF".into(),
            sticker_bleed_mm: 0.0,
        },
    };
    let tmp = std::env::temp_dir().join("trasea_g13_06_roundtrip.trasea");
    save_project(&tmp, &proj).expect("save");
    let loaded = load_project(&tmp).expect("load");
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(loaded.project, proj, "1.3.0 project round-trip must be lossless");
    let raw = serde_json::to_string(&proj.options.node_edits).unwrap();
    assert!(raw.contains("\"kind\":\"moveNode\"") && raw.contains("\"mirrorSmooth\":true") && raw.contains("\"tolerancePx\""));
    let back: Vec<NodeEdit> = serde_json::from_str(&raw).unwrap();
    assert_eq!(back, edits);

    // the edited position shows up in every export
    let doc = doc_with(vec![layer([200, 30, 30], vec![square(40.0, 40.0, 100.0)])], vec![], 1.0);
    let edited = apply_node_edits(&doc, &[mv("f:0:0", 0, 2, 190.5, 177.25)]);
    let mut eo = ExportOptions::default();
    let svg0 = to_svg(&doc, &eo);
    let svg1 = to_svg(&edited, &eo);
    assert_ne!(svg0, svg1);
    assert!(svg1.contains("190.5") && svg1.contains("177.25"), "SVG carries the new node");
    assert!(!svg0.contains("190.5"));
    assert_ne!(to_eps(&doc, &eo), to_eps(&edited, &eo), "EPS");
    assert_ne!(to_dxf(&doc, &eo), to_dxf(&edited, &eo), "DXF");
    assert_ne!(to_pdf(&doc, &eo).unwrap(), to_pdf(&edited, &eo).unwrap(), "PDF");
    eo.color_space = ColorSpace::Cmyk;
    eo.die_cut_contour = true;
    assert_ne!(to_pdf(&doc, &eo).unwrap(), to_pdf(&edited, &eo).unwrap(), "PDF CMYK + CutContour");
    assert_ne!(to_dxf(&doc, &eo), to_dxf(&edited, &eo), "DXF + CutContour");

    // end-to-end through the raster pipeline: node edits in `Params` change the output SVG
    let mut img = RgbImage::from_pixel(160, 160, Rgb([255, 255, 255]));
    for y in 40..120 {
        for x in 40..120 {
            if (x as f32 - 80.0).powi(2) + (y as f32 - 80.0).powi(2) <= 38.0 * 38.0 {
                img.put_pixel(x, y, Rgb([30, 90, 200]));
            }
        }
    }
    let mut p = Params::default();
    p.measure = false;
    p.detect_primitives = false;
    let base = vectorize(&img, &p, &|_, _| {}).expect("base");
    let svg = base.svg.clone();
    let id = {
        let needle = "data-trasea-id=\"f:";
        let mut ids = Vec::new();
        let mut rest = svg.as_str();
        while let Some(i) = rest.find(needle) {
            let tail = &rest[i + "data-trasea-id=\"".len()..];
            let end = tail.find('"').unwrap();
            ids.push(tail[..end].to_string());
            rest = &tail[end..];
        }
        ids.into_iter().find(|i| i != "f:0:0").expect("colour path id")
    };
    let nodes_base = geo(&base.doc, &id).total_nodes;
    let mut p2 = p.clone();
    p2.node_edits = vec![mv(&id, 0, 0, 3.0, 3.0)];
    let moved = vectorize(&img, &p2, &|_, _| {}).expect("moved");
    assert_ne!(moved.svg, base.svg);
    assert_eq!(geo(&moved.doc, &id).total_nodes, nodes_base);
    assert!(inspect_path_nodes(&moved.doc, &id).unwrap().subpaths[0].nodes[0].x < 5.0);
}

const SVG_BASIC: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100" viewBox="0 0 200 100">
  <path d="M10 10 L60 10 L35 50 Z" fill="#FF0000"/>
  <circle cx="120" cy="30" r="15" fill="#0000FF"/>
  <g transform="translate(20,10) scale(2)"><rect x="10" y="10" width="20" height="20" fill="#00FF00"/></g>
  <path d="M10 90 Q 50 60 90 90" fill="none" stroke="#000000" stroke-width="3"/>
  <path d="M150 80 A 15 15 0 0 1 180 80 Z" fill="#FF0000"/>
</svg>"##;

fn layer_bbox(l: &Layer) -> (f64, f64, f64, f64) {
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for c in &l.contours {
        let mut pts = vec![c.start];
        pts.extend(c.curves.iter().map(|cv| cv[2]));
        for p in pts {
            x0 = x0.min(p.0);
            y0 = y0.min(p.1);
            x1 = x1.max(p.0);
            y1 = y1.max(p.1);
        }
    }
    (x0, y0, x1, y1)
}

/// G13-07 — Impor `.svg` standar (`svg_import`)
#[test]
fn g13_07_svg_import_standard() {
    let imp = parse_svg_doc(SVG_BASIC.as_bytes()).expect("parse");
    let doc = &imp.doc;
    assert_eq!((doc.width, doc.height), (200, 100));
    assert_eq!(doc.scale, 1.0);
    let colors: Vec<[u8; 3]> = doc.fields.iter().map(|l| l.color).collect();
    assert_eq!(colors, vec![[255, 0, 0], [0, 0, 255], [0, 255, 0], [255, 0, 0]], "paint order is kept: the second red shape starts a new layer on top");
    assert_eq!(doc.fields[0].contours.len(), 1, "triangle alone in the first red layer");

    // `translate(20,10) scale(2)` applied to the rect (10,10,20,20) -> (40,30)..(80,70)
    let (x0, y0, x1, y1) = layer_bbox(&doc.fields[2]);
    assert!((x0 - 40.0).abs() < 0.01 && (y0 - 30.0).abs() < 0.01 && (x1 - 80.0).abs() < 0.01 && (y1 - 70.0).abs() < 0.01, "{x0} {y0} {x1} {y1}");
    // circle cx=120 r=15
    let (cx0, _, cx1, _) = layer_bbox(&doc.fields[1]);
    assert!((cx0 - 105.0).abs() < 0.3 && (cx1 - 135.0).abs() < 0.3);

    // stroked quadratic curve -> one stroke of width 3
    assert_eq!(doc.strokes.len(), 1);
    assert_eq!(doc.strokes[0].color, [0, 0, 0]);
    assert_eq!(doc.strokes[0].strokes.len(), 1);
    assert!((doc.strokes[0].strokes[0].width - 3.0).abs() < 1e-6);

    // deterministic ids exist and edits address them
    let svg = to_svg(doc, &ExportOptions::default());
    for id in ["f:0:0", "f:1:0", "f:2:0", "f:3:0", "s:0:0"] {
        assert!(svg.contains(&format!("data-trasea-id=\"{id}\"")), "missing {id} in {svg}");
    }
    assert_eq!(geo(doc, "f:0:0").total_nodes, 3);
    assert_eq!(geo(doc, "f:2:0").total_nodes, 4);

    // viewBox scaling: a 10x10 viewBox shown at 100x100 maps (0..10) -> (0..100)
    let vb = parse_svg_doc(br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 10 10"><rect x="0" y="0" width="10" height="10" fill="#123456"/></svg>"##).unwrap();
    assert_eq!((vb.doc.width, vb.doc.height), (100, 100));
    let (_, _, bx1, by1) = layer_bbox(&vb.doc.fields[0]);
    assert!((bx1 - 100.0).abs() < 0.01 && (by1 - 100.0).abs() < 0.01, "{bx1} {by1}");

    // load from disk and edit through the same pipeline as a traced drawing
    let tmp = std::env::temp_dir().join("trasea_g13_07.svg");
    std::fs::write(&tmp, SVG_BASIC).unwrap();
    let loaded = load_svg_doc(&tmp).unwrap();
    let _ = std::fs::remove_file(&tmp);
    let mut p = Params::default();
    p.path_edits = vec![PathEdit::Delete { target_id: "f:1:0".into() }];
    p.node_edits = vec![mv("f:2:0", 0, 0, 0.0, 0.0)];
    let out = vectorize_svg(loaded, &p, &ExportOptions::default());
    assert!(out.stats.mae.is_none());
    assert_eq!(out.doc.fields.len(), 3, "blue layer deleted");
    assert!(out.stats.nodes > 0 && out.stats.paths > 0);
    check_svg_import_keeps_paint_order();
}

/// G13-08 — Impor `.svg` -> CutContour + PDF CMYK/Spot + DXF
#[test]
fn g13_08_svg_to_cutcontour_pdf_dxf() {
    let imp = parse_svg_doc(SVG_BASIC.as_bytes()).unwrap();
    let mut eo = ExportOptions::default();
    eo.die_cut_contour = true;
    eo.die_cut_offset_mm = 2.0;
    eo.color_space = ColorSpace::Cmyk;
    eo.spot_colors = vec![SpotColorMapping { hex: "#FF0000".into(), spot_name: "Spot_Red".into() }];

    let pdf = to_pdf(&imp.doc, &eo).expect("pdf");
    assert!(pdf.starts_with(b"%PDF-"));
    let s = String::from_utf8_lossy(&pdf);
    assert!(s.contains("/Separation /CutContour /DeviceCMYK"), "PDF missing CutContour separation");
    assert!(s.contains("/Separation /Spot_Red /DeviceCMYK"));
    let dxf = to_dxf(&imp.doc, &eo);
    assert!(dxf.contains("0\nLAYER\n2\nCUTCONTOUR\n"));
    assert!(dxf.contains("0\nPOLYLINE\n8\nCUTCONTOUR\n"));
    let svg = to_svg(&imp.doc, &eo);
    assert!(svg.contains("#EC008C") && svg.contains("CutContour"));
    // the cut line is larger than the artwork bounding box
    assert!(to_eps(&imp.doc, &eo).contains("CutContour"));
}

/// G13-09 — Edge case `.svg` (kosong / rusak / non-path)
#[test]
fn g13_09_svg_edge_cases() {
    assert!(parse_svg_doc(b"").is_err());
    assert!(parse_svg_doc(b"   \n  ").is_err());
    assert!(parse_svg_doc(b"<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"10\"><path d=\"M0 0").is_err(), "broken XML");
    assert!(parse_svg_doc(b"not an svg at all").is_err());
    assert!(parse_svg_doc(&[0xff, 0xfe, 0x00, 0x12, 0x99]).is_err(), "binary garbage");
    assert!(load_svg_doc(Path::new("Z:/does/not/exist.svg")).is_err());

    // an empty (valid) SVG loads as an empty document with a friendly note
    let empty = parse_svg_doc(br#"<svg xmlns="http://www.w3.org/2000/svg" width="50" height="50"></svg>"#).unwrap();
    assert!(empty.doc.fields.is_empty() && empty.doc.strokes.is_empty());
    assert!(!empty.notes.is_empty());
    // an empty doc still exports without panicking
    let eo = ExportOptions { die_cut_contour: true, ..ExportOptions::default() };
    let _ = to_svg(&empty.doc, &eo);
    let _ = to_dxf(&empty.doc, &eo);

    // an embedded raster image and a gradient fill are skipped with a note, the vector path is still imported
    let mixed = br##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="120" height="80">
      <defs><linearGradient id="g"><stop offset="0" stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient></defs>
      <image x="0" y="0" width="1" height="1" xlink:href="data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8z8BQDwAEhQGAhKmMIQAAAABJRU5ErkJggg=="/>
      <rect x="5" y="5" width="30" height="30" fill="url(#g)"/>
      <path d="M50 10 H100 V60 H50 Z" fill="#336699"/>
    </svg>"##;
    let m = parse_svg_doc(mixed).unwrap();
    assert_eq!(m.doc.fields.len(), 1);
    assert_eq!(m.doc.fields[0].color, [0x33, 0x66, 0x99]);
    let joined = m.notes.join(" | ");
    assert!(joined.contains("gambar raster"), "{joined}");
    assert!(joined.contains("gradien"), "{joined}");
}

/// G13-10 — Audit lisensi / offline & zero regresi Mode Standar
#[test]
fn g13_10_offline_audit_and_standard_zero_regression() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).expect("workspace root");
    let core_toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).unwrap();
    let tauri_toml = std::fs::read_to_string(workspace_root.join("src-tauri/Cargo.toml")).unwrap();
    let lock = std::fs::read_to_string(workspace_root.join("Cargo.lock")).unwrap();

    let forbidden = ["reqwest", "hyper", "ureq", "curl", "surf", "isahc", "tonic", "potrace", "ghostscript"];
    for c in forbidden {
        assert!(!core_toml.contains(c), "forbidden crate in trasea-core: {c}");
        assert!(!tauri_toml.contains(c), "forbidden crate in src-tauri: {c}");
    }
    // `usvg` is a direct dependency already (no new crate for SVG import)
    assert!(core_toml.contains("usvg"));

    let mut deps: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let (mut cur, mut in_deps) = (None::<String>, false);
    for line in lock.lines() {
        let t = line.trim();
        if t == "[[package]]" {
            cur = None;
            in_deps = false;
        } else if let Some(rest) = t.strip_prefix("name = \"") {
            if let Some(n) = rest.strip_suffix('"') {
                cur = Some(n.to_string());
                deps.entry(n.to_string()).or_default();
            }
        } else if t == "dependencies = [" {
            in_deps = true;
        } else if in_deps {
            if t == "]" {
                in_deps = false;
            } else if let Some(c) = &cur {
                let d = t.trim_matches(|ch| ch == '"' || ch == ',');
                if let Some(n) = d.split_whitespace().next() {
                    if !n.is_empty() {
                        deps.entry(c.clone()).or_default().push(n.to_string());
                    }
                }
            }
        }
    }
    // transitive closure of trasea-core and of the Tauri app contains no network crate
    // (the Tauri app is checked through its manifest above and through `cargo tree -p trasea -e normal,build`;
    // Cargo.lock also lists optional, never-built platform deps of `tauri`, so only the core is walked here)
    for root in ["trasea-core"] {
        let mut seen = std::collections::HashSet::new();
        let mut q = std::collections::VecDeque::from([root.to_string()]);
        while let Some(pkg) = q.pop_front() {
            if !seen.insert(pkg.clone()) {
                continue;
            }
            assert!(!forbidden.contains(&pkg.as_str()), "network crate {pkg} reachable from {root}");
            for ch in deps.get(&pkg).into_iter().flatten() {
                q.push_back(ch.clone());
            }
        }
        if root == "trasea-core" {
            assert!(seen.len() > 20);
        }
    }
    // no GPL / AGPL licence declared anywhere in the dependency lock-in notes of this repository
    let licences = std::fs::read_to_string(workspace_root.join("THIRD_PARTY_LICENSES.md")).unwrap_or_default();
    assert!(!licences.contains("AGPL"), "AGPL crate listed");

    // Standard mode: node edits are off by default, never touch a drawing without edits, and are deterministic
    let mut logo = RgbImage::from_pixel(160, 160, Rgb([255, 255, 255]));
    for y in 30..130 {
        for x in 30..130 {
            if (x as f32 - 80.0).powi(2) + (y as f32 - 80.0).powi(2) <= 50.0 * 50.0 {
                logo.put_pixel(x, y, Rgb([20, 80, 180]));
            }
            if (55..105).contains(&x) && (65..95).contains(&y) {
                logo.put_pixel(x, y, Rgb([240, 180, 20]));
            }
        }
    }
    let p = Params::default();
    assert!(p.node_edits.is_empty() && p.path_edits.is_empty());
    let a = vectorize(&logo, &p, &|_, _| {}).expect("a");
    let b = vectorize(&logo, &p, &|_, _| {}).expect("b");
    assert_eq!(a.svg, b.svg, "Standard mode is deterministic");
    let mut stale = p.clone();
    stale.node_edits = vec![
        mv("f:99:0", 0, 0, 1.0, 1.0),
        NodeEdit::SimplifyPath { target_id: "nope".into(), tolerance_px: 1.0 },
    ];
    let c = vectorize(&logo, &stale, &|_, _| {}).expect("c");
    assert_eq!(c.svg, a.svg, "stale / unknown node edits change nothing");
    assert_eq!(count_svg(&c.svg), count_svg(&a.svg));
}

/// Part of G13-07: imported shapes keep their paint order (a dark shape above other colours must not punch the dark background).
fn check_svg_import_keeps_paint_order() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200" viewBox="0 0 200 200">
      <rect x="0" y="0" width="200" height="200" fill="#0F1115"/>
      <rect x="20" y="20" width="100" height="60" fill="#0095D9"/>
      <rect x="60" y="40" width="80" height="120" fill="#F2B705"/>
      <g transform="rotate(30 100 50)"><rect x="85" y="30" width="30" height="40" fill="#0F1115"/></g>
      <rect x="140" y="140" width="20" height="20" fill="#E6007E"/>
      <rect x="170" y="140" width="20" height="20" fill="#E6007E"/>
    </svg>"##;
    let imp = parse_svg_doc(svg.as_bytes()).unwrap();
    let colors: Vec<[u8; 3]> = imp.doc.fields.iter().map(|l| l.color).collect();
    assert_eq!(
        colors,
        vec![[0x0F, 0x11, 0x15], [0x00, 0x95, 0xD9], [0xF2, 0xB7, 0x05], [0x0F, 0x11, 0x15], [0xE6, 0x00, 0x7E]],
        "paint order kept; the two disjoint magenta squares share a layer"
    );
    assert_eq!(imp.doc.fields[4].contours.len(), 2);
    let exported = to_svg(&imp.doc, &ExportOptions::default());
    let a = trasea_core::stats::render_svg(svg, 200, 200).unwrap();
    let b = trasea_core::stats::render_svg(&exported, 200, 200).unwrap();
    let (mae, _) = trasea_core::stats::compare(&a, &b);
    assert!(mae < 0.8, "imported drawing must look like the original SVG, MAE {mae:.3}");
}
