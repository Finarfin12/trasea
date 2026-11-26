use trasea_core::audit::{audit_doc, auto_fix_doc};
use trasea_core::color::ColorSpace;
use trasea_core::export::{microstock_scale_factor, to_eps, to_svg, ExportOptions};
use trasea_core::path_edit::{apply_path_edits, PathEdit};
use trasea_core::pipeline::replay_vector_edits;
use trasea_core::svg_import::parse_svg_doc;
use trasea_core::{vectorize_svg, Params};

#[test]
fn sequential_recolor_tracks_single_shape_across_layer_changes() {
    let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <circle cx="30" cy="30" r="8" fill="#e07a5f"/>
        <rect x="60" y="60" width="20" height="18" fill="#123456"/>
    </svg>"##;
    let import = parse_svg_doc(svg.as_bytes()).unwrap();
    let doc = import.doc;
    let options = ExportOptions::default();
    let mut current = doc.clone();
    let mut edits = Vec::new();
    let mut previous = "#E07A5F";
    for color in ["#2a9d8f", "#9d4f2a", "#f4d35e"] {
        let rendered = to_svg(&current, &options);
        let shape = rendered.lines().find(|line| line.contains("data-trasea-id") && line.to_ascii_uppercase().contains(&previous.to_ascii_uppercase())).unwrap();
        let id = shape.split_once("data-trasea-id=\"").unwrap().1.split('"').next().unwrap();
        let edit = PathEdit::ReassignColor { target_id: id.into(), to_hex: color.into() };
        current = apply_path_edits(&current, std::slice::from_ref(&edit));
        edits.push(edit);
        previous = color;
    }
    let final_svg = to_svg(&apply_path_edits(&doc, &edits), &options);
    assert_eq!(final_svg, to_svg(&current, &options));
    assert!(final_svg.lines().any(|line| line.contains("data-trasea-id") && line.to_ascii_lowercase().contains("#f4d35e")));
    assert!(final_svg.lines().any(|line| line.contains("data-trasea-id") && line.contains("#123456")));
}

#[test]
fn fast_replay_matches_full_svg_edit_and_reset() {
    let input = r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100">
        <path fill="#123456" d="M 10 10 L 80 10 L 10 80 Z"/>
    </svg>"##;
    let import = parse_svg_doc(input.as_bytes()).unwrap();
    let options = ExportOptions::default();
    let params = Params { retain_base_doc: true, ..Params::default() };
    let unedited = vectorize_svg(import.clone(), &params, &options);
    let edited_params = Params {
        path_edits: vec![PathEdit::ReassignColor { target_id: "f:0:0".into(), to_hex: "#ff0000".into() }],
        edit_fingerprint: Some(unedited.trace_fingerprint.clone()),
        ..params
    };
    let fully_edited = vectorize_svg(import, &edited_params, &options);
    let base = fully_edited.base_doc.unwrap();
    let replayed = replay_vector_edits(&base, &edited_params);
    assert_eq!(to_svg(&replayed, &options), fully_edited.svg);
    assert_ne!(fully_edited.svg, unedited.svg);
    assert_eq!(to_svg(&replay_vector_edits(&base, &Params::default()), &options), unedited.svg);
}

#[test]
fn microstock_svg_and_eps_scale_to_rgb_artboards() {
    for (width, height) in [(800, 600), (4800, 3200), (10_000, 8_000)] {
        let input = format!(r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}"><path fill="#98a210" d="M 10 10 L 80 20 L 20 80 Z"/></svg>"##);
        let doc = parse_svg_doc(input.as_bytes()).unwrap().doc;
        let opts = ExportOptions { microstock_ready: true, color_space: ColorSpace::Cmyk, ..ExportOptions::default() };
        let svg = to_svg(&doc, &opts);
        let eps = to_eps(&doc, &opts);
        let svg_box = svg.split_once("viewBox=\"0 0 ").unwrap().1.split('"').next().unwrap();
        let eps_box = eps.lines().find_map(|line| line.strip_prefix("%%BoundingBox: 0 0 ")).unwrap();
        assert_eq!(svg_box, eps_box);
        let dimensions: Vec<f64> = svg_box.split_whitespace().map(|n| n.parse().unwrap()).collect();
        let area = dimensions[0] * dimensions[1];
        assert!((16_000_000.0..=65_000_000.0).contains(&area), "{area}");
        assert!((dimensions[0] / dimensions[1] - width as f64 / height as f64).abs() < 0.001);
        assert!(eps.contains("setrgbcolor") && !eps.contains("setcmykcolor"));
        assert!(svg.contains("scale(") && !svg.contains("<image"));
    }
    assert_eq!(microstock_scale_factor(0.0, 600.0), 1.0);
}

#[test]
fn microstock_audit_reports_dense_and_stray_paths_and_fixes_only_strays() {
    use trasea_core::trace::Contour;
    let input = br##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600"><path fill="#98a210" d="M 10 10 L 80 20 L 20 80 Z"/></svg>"##;
    let mut doc = parse_svg_doc(input).unwrap().doc;
    let layer = &mut doc.fields[0];
    let mut dense = layer.contours[0].clone();
    dense.curves = dense.curves.repeat(45);
    layer.contours.push(dense);
    layer.contours.push(Contour { start: (1.0, 1.0), curves: vec![], prim: None });
    let before = audit_doc(&doc);
    assert!(before.issues.iter().any(|i| i.code == "artboard_below_15mp" && !i.fixable));
    assert!(before.issues.iter().any(|i| i.code == "dense_jagged_paths"));
    assert!(before.issues.iter().any(|i| i.code == "degenerate_paths" && i.fixable && i.count == 1));
    assert!(auto_fix_doc(&mut doc) >= 1);
    let after = audit_doc(&doc);
    assert!(!after.issues.iter().any(|i| i.code == "degenerate_paths"));
    assert!(after.issues.iter().any(|i| i.code == "dense_jagged_paths"));
}

#[test]
fn recoloring_layer_keeps_geometry_and_path_ids() {
    let input = br##"<svg xmlns="http://www.w3.org/2000/svg" width="800" height="600">
        <path fill="#98a210" d="M 10 10 L 80 20 L 20 80 Z"/>
        <path fill="#98a210" d="M 90 10 L 180 20 L 120 80 Z"/>
        <path fill="#123456" d="M 200 200 L 340 200 L 200 350 Z"/>
    </svg>"##;
    let doc = parse_svg_doc(input).unwrap().doc;
    let edit = PathEdit::ReassignLayerColor { from_hex: "#98A210".into(), to_hex: "#00FF77".into() };
    let original = to_svg(&doc, &ExportOptions::default());
    let changed = to_svg(&apply_path_edits(&doc, &[edit]), &ExportOptions::default());
    let paths = |s: &str| s.lines().filter(|line| line.contains("data-trasea-id")).map(str::to_owned).collect::<Vec<_>>();
    assert_eq!(paths(&changed), paths(&original.replace("#98A210", "#00FF77")));
}
