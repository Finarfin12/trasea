use trasea_core::export::{to_svg, ExportOptions};
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
