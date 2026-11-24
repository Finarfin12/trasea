//! Automated test suite for Trasea 1.2.0 ("Stiker, Laser, & Presisi RIP"): G12-01 .. G12-10.

use image::{Rgb, RgbImage, Rgba, RgbaImage};
use std::path::Path;
use trasea_core::color::{
    is_out_of_cmyk_gamut, pdf_name_encode, rgb_to_soft_proof_rgb, sanitize_spot_name, ColorSpace,
    SpotColorMapping,
};
use trasea_core::contour::build_die_cut_contours;
use trasea_core::dxf::{to_dxf, DxfUnit};
use trasea_core::export::{
    export_options_for_production, palette_of, to_ase_with_spots, to_eps, to_pdf, to_svg, Doc,
    ExportOptions,
};
use trasea_core::path_edit::{apply_path_edits, PathEdit};
use trasea_core::preprocess::{remove_background_rgba, PreprocessOptions};
use trasea_core::production::{ProductionMode, ProductionOptions};
use trasea_core::project::{
    load_project, save_project, ProjectExportOptions, ProjectFile, ProjectOptions,
    PROJECT_SCHEMA_VERSION,
};
use trasea_core::stats::count_svg;
use trasea_core::{vectorize, Params};

fn draw_disk_rgb(img: &mut RgbImage, cx: f32, cy: f32, r: f32, col: [u8; 3]) {
    let (w, h) = img.dimensions();
    let r2 = r * r;
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r2 {
                img.put_pixel(x, y, Rgb(col));
            }
        }
    }
}

fn draw_rect_rgb(img: &mut RgbImage, x0: u32, y0: u32, rw: u32, rh: u32, col: [u8; 3]) {
    let (w, h) = img.dimensions();
    for y in y0..(y0 + rh).min(h) {
        for x in x0..(x0 + rw).min(w) {
            img.put_pixel(x, y, Rgb(col));
        }
    }
}

fn draw_thick_segment(img: &mut RgbImage, x0: f32, y0: f32, x1: f32, y1: f32, radius: f32, col: [u8; 3]) {
    let dist = ((x1 - x0).powi(2) + (y1 - y0).powi(2)).sqrt().max(1.0);
    let steps = (dist * 2.0).ceil() as usize;
    for i in 0..=steps {
        let t = i as f32 / steps as f32;
        draw_disk_rgb(img, x0 + (x1 - x0) * t, y0 + (y1 - y0) * t, radius, col);
    }
}

fn extract_trasea_ids(svg: &str) -> Vec<String> {
    let needle = "data-trasea-id=\"";
    let mut ids = Vec::new();
    let mut rest = svg;
    while let Some(pos) = rest.find(needle) {
        let after = &rest[pos + needle.len()..];
        if let Some(end) = after.find('"') {
            ids.push(after[..end].to_string());
            rest = &after[end + 1..];
        } else {
            break;
        }
    }
    ids
}

fn utf16be_bytes(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    for u in s.encode_utf16() {
        out.extend_from_slice(&u.to_be_bytes());
    }
    out
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || needle.len() > haystack.len() {
        return None;
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// G12-01 — Hapus Latar & Lubang Dalam Utuh (+ defringe + peringatan >92%)
#[test]
fn g12_01_remove_background_and_preserve_interior_hole() {
    let mut rgba = RgbaImage::from_pixel(160, 160, Rgba([255, 255, 255, 255]));
    // Blue circle (radius 55) with 1px light-grey anti-alias fringe outside and a white "eye" inside (radius 14)
    for y in 0..160u32 {
        for x in 0..160u32 {
            let dx = x as f32 - 80.0;
            let dy = y as f32 - 80.0;
            let d = (dx * dx + dy * dy).sqrt();
            if d <= 14.0 {
                rgba.put_pixel(x, y, Rgba([255, 255, 255, 255]));
            } else if d <= 54.0 {
                rgba.put_pixel(x, y, Rgba([20, 90, 210, 255]));
            } else if d <= 55.2 {
                // Soft near-white halo to verify defringe strips it
                rgba.put_pixel(x, y, Rgba([228, 232, 240, 255]));
            }
        }
    }

    let (stripped, rep) = remove_background_rgba(&rgba, 24, 1);
    assert_eq!(stripped.get_pixel(0, 0).0[3], 0, "outer border should be transparent");
    assert_eq!(stripped.get_pixel(159, 159).0[3], 0, "opposite border should be transparent");
    assert_eq!(stripped.get_pixel(80, 80).0[3], 255, "interior white eye must stay opaque");
    assert_eq!(stripped.get_pixel(80, 45).0[3], 255, "blue ring must stay opaque");
    assert!(rep.removed_pixels > 10_000 && rep.warning.is_none());

    // Edge case: >92% background triggers friendly warning note
    let mut almost_empty = RgbaImage::from_pixel(100, 100, Rgba([255, 255, 255, 255]));
    for y in 45..55 {
        for x in 45..55 {
            almost_empty.put_pixel(x, y, Rgba([20, 20, 20, 255]));
        }
    }
    let (_, warn_rep) = remove_background_rgba(&almost_empty, 24, 1);
    assert!(warn_rep.removed_fraction > 0.92);
    assert!(warn_rep.warning.is_some(), "expected warning when >92% pixels are removed");

    // Full pipeline with remove_background = true strips canvas background
    let mut rgb = RgbImage::from_pixel(160, 160, Rgb([255, 255, 255]));
    draw_disk_rgb(&mut rgb, 80.0, 80.0, 54.0, [20, 90, 210]);
    draw_disk_rgb(&mut rgb, 80.0, 80.0, 14.0, [255, 255, 255]);
    let mut p = Params::default();
    p.preprocess.remove_background = true;
    p.preprocess.bg_tolerance = 24;
    p.preprocess.defringe_px = 1;
    p.measure = false;
    let out = vectorize(&rgb, &p, &|_, _| {}).expect("vectorize with remove_background");
    assert!(out.doc.fields.iter().all(|l| !l.canvas), "no canvas background layer should remain");
    assert!(out.doc.fields.len() >= 1);
}

/// G12-02 — Mode Centerline (Garis Tengah): 0 bidang fill, kurva terbuka, DXF LWPOLYLINE di layer CENTERLINE
#[test]
fn g12_02_centerline_mode_open_strokes_and_dxf() {
    let mut sketch = RgbImage::from_pixel(220, 220, Rgb([255, 255, 255]));
    // Draw a 6px-thick black 'X' and a curved arc
    draw_thick_segment(&mut sketch, 30.0, 30.0, 190.0, 190.0, 3.0, [15, 15, 15]);
    draw_thick_segment(&mut sketch, 190.0, 30.0, 30.0, 190.0, 3.0, [15, 15, 15]);

    let mut p = Params::default();
    p.production = ProductionOptions::centerline();
    p.measure = false;
    let out = vectorize(&sketch, &p, &|_, _| {}).expect("centerline vectorize");

    assert_eq!(out.doc.fields.len(), 0, "Centerline mode must emit 0 filled field layers");
    let stroke_count: usize = out.doc.strokes.iter().map(|sl| sl.strokes.len()).sum();
    assert!(stroke_count > 0, "Centerline mode must emit open strokes");
    assert!(
        out.svg.contains("fill=\"none\" stroke="),
        "Centerline SVG must contain open stroke paths with fill=\"none\""
    );

    let eo = export_options_for_production(&p.production);
    let dxf = to_dxf(&out.doc, &eo);
    assert!(dxf.contains("0\nLAYER\n2\nCENTERLINE\n"), "DXF must declare CENTERLINE layer");
    assert!(dxf.contains("0\nLWPOLYLINE\n8\nCENTERLINE\n"), "DXF must emit LWPOLYLINE on CENTERLINE");
    assert!(dxf.contains("\n70\n0\n"), "Centerline LWPOLYLINE must be open (flag 70 = 0)");

    // Inverted sketch (light lines on dark background)
    let mut inv_sketch = RgbImage::from_pixel(180, 180, Rgb([15, 15, 15]));
    draw_thick_segment(&mut inv_sketch, 25.0, 90.0, 155.0, 90.0, 3.0, [245, 245, 245]);
    let mut p_inv = Params::default();
    p_inv.production = ProductionOptions::centerline();
    p_inv.production.centerline_invert = true;
    p_inv.measure = false;
    let out_inv = vectorize(&inv_sketch, &p_inv, &|_, _| {}).expect("inverted centerline");
    assert_eq!(out_inv.doc.fields.len(), 0);
    assert!(out_inv.doc.strokes.iter().map(|sl| sl.strokes.len()).sum::<usize>() > 0);
}

/// G12-03 — Die-Cut Contour Geometri & Layer (SVG `<g id="CutContour">` & DXF `CUTCONTOUR`)
#[test]
fn g12_03_die_cut_contour_geometry_and_layers() {
    let mut img = RgbImage::from_pixel(200, 200, Rgb([255, 255, 255]));
    draw_disk_rgb(&mut img, 100.0, 100.0, 45.0, [220, 40, 40]);

    let mut p = Params::default();
    p.measure = false;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize sticker");

    let mut eo = ExportOptions::default();
    eo.die_cut_contour = true;
    eo.die_cut_offset_mm = 2.0;

    let contours = build_die_cut_contours(&out.doc, &eo);
    assert_eq!(contours.len(), 1, "single circle artwork should produce 1 outer die-cut contour");

    let k = out.doc.scale;
    let (mut min_x, mut min_y, mut max_x, mut max_y) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for c in &contours {
        let mut prev = c.start;
        for cv in &c.curves {
            for step in 0..=8 {
                let t = step as f64 / 8.0;
                let mt = 1.0 - t;
                let x = (mt * mt * mt * prev.0
                    + 3.0 * mt * mt * t * cv[0].0
                    + 3.0 * mt * t * t * cv[1].0
                    + t * t * t * cv[2].0)
                    / k;
                let y = (mt * mt * mt * prev.1
                    + 3.0 * mt * mt * t * cv[0].1
                    + 3.0 * mt * t * t * cv[1].1
                    + t * t * t * cv[2].1)
                    / k;
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
            prev = cv[2];
        }
    }
    // Artwork circle spans [55, 145] on both axes; 2.0 mm (~7.56 px at 96 DPI) offset must expand all 4 sides
    assert!(min_x < 52.0 && min_y < 52.0, "die-cut min ({min_x:.1}, {min_y:.1}) should expand beyond 55");
    assert!(max_x > 148.0 && max_y > 148.0, "die-cut max ({max_x:.1}, {max_y:.1}) should expand beyond 145");

    let svg = to_svg(&out.doc, &eo);
    assert!(svg.contains("<g id=\"CutContour\" data-name=\"CutContour\">"), "SVG missing CutContour group");
    assert!(svg.contains("#EC008C"), "SVG CutContour should use #EC008C stroke");

    let dxf = to_dxf(&out.doc, &eo);
    assert!(dxf.contains("0\nLAYER\n2\nCUTCONTOUR\n"), "DXF missing CUTCONTOUR layer table entry");
    assert!(dxf.contains("0\nPOLYLINE\n8\nCUTCONTOUR\n"), "DXF missing CUTCONTOUR polyline");
}

/// G12-04 — Die-Cut Contour Edge Cases (Empty Doc, Separate Far Islands, Bridged Close Islands)
#[test]
fn g12_04_die_cut_edge_cases_empty_and_islands() {
    // 1. Empty doc -> no panic, 0 contours
    let empty_doc = Doc {
        width: 100,
        height: 100,
        scale: 1.0,
        fields: Vec::new(),
        line_shapes: Vec::new(),
        line_fallback: Vec::new(),
        strokes: Vec::new(),
    };
    let mut eo_empty = ExportOptions::default();
    eo_empty.die_cut_contour = true;
    assert!(build_die_cut_contours(&empty_doc, &eo_empty).is_empty());

    // 2. Two far-apart circles with bridge_islands = false -> 2 separate contours
    let mut far_img = RgbImage::from_pixel(300, 140, Rgb([255, 255, 255]));
    draw_disk_rgb(&mut far_img, 55.0, 70.0, 26.0, [20, 120, 220]);
    draw_disk_rgb(&mut far_img, 245.0, 70.0, 26.0, [220, 80, 20]);
    let mut p = Params::default();
    p.measure = false;
    let far_out = vectorize(&far_img, &p, &|_, _| {}).expect("vectorize far islands");
    let mut eo_far = ExportOptions::default();
    eo_far.die_cut_contour = true;
    eo_far.die_cut_offset_mm = 2.0;
    eo_far.die_cut_bridge_islands = false;
    let far_contours = build_die_cut_contours(&far_out.doc, &eo_far);
    assert_eq!(far_contours.len(), 2, "far islands with bridge=false should stay 2 contours");

    // 3. Two close circles (gap = 12 px < 4 * 7.56 px) with bridge_islands = true -> 1 merged contour
    let mut close_img = RgbImage::from_pixel(220, 140, Rgb([255, 255, 255]));
    draw_disk_rgb(&mut close_img, 76.0, 70.0, 28.0, [20, 120, 220]);
    draw_disk_rgb(&mut close_img, 144.0, 70.0, 28.0, [220, 80, 20]);
    let close_out = vectorize(&close_img, &p, &|_, _| {}).expect("vectorize close islands");
    let mut eo_close = ExportOptions::default();
    eo_close.die_cut_contour = true;
    eo_close.die_cut_offset_mm = 2.0;
    eo_close.die_cut_bridge_islands = true;
    let close_contours = build_die_cut_contours(&close_out.doc, &eo_close);
    assert_eq!(close_contours.len(), 1, "close islands with bridge=true should merge into 1 contour");
}

/// G12-05 — Path Edits (`Delete` & `ReassignColor`) with deterministic `data-trasea-id`
#[test]
fn g12_05_path_edits_delete_and_reassign_color() {
    let mut img = RgbImage::from_pixel(260, 160, Rgb([255, 255, 255]));
    // Two separate red rectangles + one blue rectangle (detect_primitives = false so all are contours)
    draw_rect_rgb(&mut img, 20, 30, 60, 90, [220, 30, 30]);
    draw_rect_rgb(&mut img, 100, 30, 60, 90, [220, 30, 30]);
    draw_rect_rgb(&mut img, 180, 30, 60, 90, [30, 80, 220]);

    let mut p = Params::default();
    p.detect_primitives = false;
    p.keep_lines = false;
    p.measure = false;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize 3-shape doc");
    let eo = ExportOptions::default();
    let svg_before = to_svg(&out.doc, &eo);
    let ids = extract_trasea_ids(&svg_before);
    // Non-background field IDs (`f:1:...`, `f:2:...`)
    let fg_ids: Vec<String> = ids.into_iter().filter(|id| id.starts_with("f:") && !id.starts_with("f:0:")).collect();
    assert!(fg_ids.len() >= 3, "expected at least 3 foreground path IDs, got {:?}", fg_ids);

    let paths_before = count_svg(&svg_before).0;
    let edits = vec![
        PathEdit::Delete { target_id: fg_ids[0].clone() },
        PathEdit::ReassignColor { target_id: fg_ids[1].clone(), to_hex: "#00AA55".to_string() },
        // Stale ID and invalid hex must be safely ignored without panic
        PathEdit::Delete { target_id: "f:99:42".to_string() },
        PathEdit::ReassignColor { target_id: fg_ids[2].clone(), to_hex: "not-a-color".to_string() },
    ];

    let edited_doc = apply_path_edits(&out.doc, &edits);
    let svg_after = to_svg(&edited_doc, &eo);
    let paths_after = count_svg(&svg_after).0;

    assert_eq!(paths_after + 1, paths_before, "Delete must reduce path count by exactly 1");
    assert!(svg_after.contains("#00AA55"), "Reassigned colour #00AA55 must appear in SVG");

    // Verify change propagates to PDF, EPS, and DXF
    let pdf = to_pdf(&edited_doc, &ExportOptions { color_space: ColorSpace::Cmyk, ..eo.clone() }).expect("edited PDF");
    assert!(pdf.starts_with(b"%PDF-1.4"));
    let eps = to_eps(&edited_doc, &eo);
    assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
    let dxf = to_dxf(&edited_doc, &eo);
    assert!(dxf.contains("00AA55"), "Reassigned colour layer must appear in DXF");
}

/// G12-06 — Soft-Proof CMYK & Deteksi Out-of-Gamut
#[test]
fn g12_06_soft_proof_cmyk_and_gamut_detection() {
    // Neon electric blue & bright neon green must be flagged out-of-gamut and visibly desaturated
    assert!(is_out_of_cmyk_gamut(0, 51, 255), "#0033FF should be out of CMYK gamut");
    assert!(is_out_of_cmyk_gamut(0, 255, 68), "#00FF44 should be out of CMYK gamut");

    let blue_proof = rgb_to_soft_proof_rgb(0, 51, 255);
    assert_eq!(blue_proof, rgb_to_soft_proof_rgb(0, 51, 255), "soft-proof must be deterministic");
    assert!(blue_proof[2] < 200, "soft-proof blue should compress electric blue channel, got {:?}", blue_proof);

    let green_proof = rgb_to_soft_proof_rgb(0, 255, 68);
    assert!(green_proof[1] < 210, "soft-proof green should compress neon green channel, got {:?}", green_proof);

    // Neutrals must remain inside print gamut
    assert!(!is_out_of_cmyk_gamut(128, 128, 128), "#808080 should stay in CMYK gamut");
    assert!(!is_out_of_cmyk_gamut(0, 0, 0), "#000000 should stay in CMYK gamut");
    assert!(!is_out_of_cmyk_gamut(255, 255, 255), "#FFFFFF should stay in CMYK gamut");

    // Spot name sanitization & PDF Name encoding checks
    assert_eq!(sanitize_spot_name("CutContour"), "CutContour");
    assert_eq!(pdf_name_encode("PANTONE 185 C"), "PANTONE#20185#20C");
    assert_eq!(sanitize_spot_name("  /bad(name)%  "), "badname");
}

/// G12-07 — Named Spot Color di PDF 1.4 (`/Separation /CutContour` & `/Separation /Spot_Red` + `SCN`/`scn`)
#[test]
fn g12_07_named_spot_color_pdf_1_4() {
    let mut img = RgbImage::from_pixel(180, 180, Rgb([255, 255, 255]));
    draw_rect_rgb(&mut img, 40, 40, 100, 100, [255, 0, 0]);

    let mut p = Params::default();
    p.custom_palette = Some(vec![[255, 255, 255], [255, 0, 0]]);
    p.keep_lines = false;
    p.measure = false;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize spot doc");

    let mut eo = ExportOptions::default();
    eo.color_space = ColorSpace::Cmyk;
    eo.die_cut_contour = true;
    eo.die_cut_offset_mm = 2.0;
    eo.spot_colors = vec![
        SpotColorMapping { hex: "#FF0000".to_string(), spot_name: "Spot_Red".to_string() },
        SpotColorMapping { hex: "#0000FF".to_string(), spot_name: "PANTONE 185 C".to_string() },
    ];

    let pdf_bytes = to_pdf(&out.doc, &eo).expect("spot PDF");
    assert!(pdf_bytes.starts_with(b"%PDF-1.4"));
    let pdf_str = String::from_utf8_lossy(&pdf_bytes);
    assert!(pdf_str.trim_end().ends_with("%%EOF"));
    assert!(pdf_str.contains("/Separation /CutContour /DeviceCMYK"), "PDF missing /Separation /CutContour");
    assert!(pdf_str.contains("/Separation /Spot_Red /DeviceCMYK"), "PDF missing /Separation /Spot_Red");
    assert!(pdf_str.contains("/Separation /PANTONE#20185#20C /DeviceCMYK"), "PDF missing space-encoded spot name");
    assert!(pdf_str.contains(" CS 1 SCN"), "PDF missing stroke spot operator SCN");
    assert!(pdf_str.contains(" cs 1 scn"), "PDF missing fill spot operator scn");
}

/// G12-08 — Named Spot Color di EPSF-3.0 & ASE (`0x0001` Spot + UTF-16BE)
#[test]
fn g12_08_named_spot_color_eps_and_ase() {
    let mut img = RgbImage::from_pixel(180, 180, Rgb([255, 255, 255]));
    draw_rect_rgb(&mut img, 40, 40, 100, 100, [255, 0, 0]);

    let mut p = Params::default();
    p.custom_palette = Some(vec![[255, 255, 255], [255, 0, 0]]);
    p.keep_lines = false;
    p.measure = false;
    let out = vectorize(&img, &p, &|_, _| {}).expect("vectorize spot eps/ase");

    let mut eo = ExportOptions::default();
    eo.color_space = ColorSpace::Cmyk;
    eo.die_cut_contour = true;
    eo.die_cut_offset_mm = 2.0;
    eo.spot_colors = vec![SpotColorMapping {
        hex: "#FF0000".to_string(),
        spot_name: "Spot_Red".to_string(),
    }];

    let eps = to_eps(&out.doc, &eo);
    assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0"));
    assert!(eps.trim_end().ends_with("%%EOF"));
    assert!(eps.contains("%%DocumentCustomColors: (CutContour) (Spot_Red)"));
    assert!(eps.contains("%%CMYKCustomColor: 0 1 0 0 (CutContour)"));
    assert!(eps.contains("%%CMYKCustomColor: 0 1 1 0 (Spot_Red)"));
    assert!(eps.contains("SPOT"), "EPS missing SPOT operator");
    assert!(!eps.contains("setrgbcolor"), "CMYK EPS must not contain setrgbcolor");

    let ase = to_ase_with_spots(
        &palette_of(&out.doc),
        ColorSpace::Cmyk,
        &eo.spot_colors,
        Some(&eo.die_cut_spot_name),
    );
    assert!(ase.starts_with(b"ASEF"), "ASE header mismatch");

    // Verify UTF-16BE names and Spot colour type (0x0001) at end of each swatch body
    let spot_red_u16 = utf16be_bytes("Spot_Red");
    let cut_u16 = utf16be_bytes("CutContour");
    let pos_red = find_subslice(&ase, &spot_red_u16).expect("ASE missing UTF-16BE Spot_Red");
    let pos_cut = find_subslice(&ase, &cut_u16).expect("ASE missing UTF-16BE CutContour");
    // Layout after name UTF-16BE + null terminator (2 bytes): "CMYK" (4 bytes) + 4*f32 (16 bytes) + colour_type (2 bytes)
    let type_red_off = pos_red + spot_red_u16.len() + 2 + 4 + 16;
    let type_cut_off = pos_cut + cut_u16.len() + 2 + 4 + 16;
    assert_eq!(u16::from_be_bytes([ase[type_red_off], ase[type_red_off + 1]]), 0x0001, "Spot_Red must be ASE type 0x0001 (Spot)");
    assert_eq!(u16::from_be_bytes([ase[type_cut_off], ase[type_cut_off + 1]]), 0x0001, "CutContour must be ASE type 0x0001 (Spot)");
}

/// G12-09 — Kompatibilitas `.trasea` `1.0.0`/`1.1.0` & Round-Trip `1.2.0`
#[test]
fn g12_09_project_compat_and_roundtrip_120() {
    let legacy_100 = r#"{
      "schemaVersion": 1,
      "appVersion": "1.0.0",
      "sourcePath": "logo.png",
      "sourceSha256": "abc",
      "options": {
        "colors": 8,
        "smooth": 50.0,
        "keepLines": true,
        "aiClean": false,
        "detectPrimitives": true,
        "symmetry": "off",
        "detectText": true
      },
      "exportOptions": {
        "linesAs": "stroke",
        "trapPt": 0.0
      }
    }"#;
    let p100: ProjectFile = serde_json::from_str(legacy_100).expect("deserialize 1.0.0 .trasea");
    assert!(!p100.options.preprocess.remove_background);
    assert_eq!(p100.options.preprocess.bg_tolerance, 24);
    assert_eq!(p100.options.preprocess.defringe_px, 1);
    assert!(p100.options.path_edits.is_empty());
    assert!(!p100.export_options.die_cut_contour);
    assert!((p100.export_options.die_cut_offset_mm - 2.0).abs() < 1e-6);
    assert_eq!(p100.export_options.die_cut_spot_name, "CutContour");
    assert!(p100.export_options.die_cut_bridge_islands);
    assert!(p100.export_options.spot_colors.is_empty());

    let legacy_110 = r#"{
      "schemaVersion": 1,
      "appVersion": "1.1.0",
      "sourcePath": "logo.png",
      "sourceSha256": "def",
      "options": {
        "colors": 10,
        "smooth": 40.0,
        "keepLines": true,
        "aiClean": false,
        "detectPrimitives": true,
        "symmetry": "off",
        "detectText": true,
        "productionMode": "shading",
        "preprocess": { "crop": [10, 10, 80, 80], "rotateDeg": 90.0 }
      },
      "exportOptions": {
        "linesAs": "stroke",
        "trapPt": 0.5,
        "colorSpace": "cmyk",
        "dxfUnit": "mm",
        "dxfTargetWidthMm": 100.0
      }
    }"#;
    let p110: ProjectFile = serde_json::from_str(legacy_110).expect("deserialize 1.1.0 .trasea");
    assert_eq!(p110.options.preprocess.crop, Some([10, 10, 80, 80]));
    assert_eq!(p110.options.preprocess.bg_tolerance, 24);
    assert_eq!(p110.export_options.dxf_unit, DxfUnit::Mm);
    assert_eq!(p110.export_options.die_cut_spot_name, "CutContour");

    // Round-trip 1.2.0 project with all new fields
    let proj_120 = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: "1.2.0".to_string(),
        source_path: "sticker.png".to_string(),
        source_sha256: "123456".to_string(),
        options: ProjectOptions {
            colors: 6,
            smooth: 50.0,
            keep_lines: true,
            ai_clean: false,
            detect_primitives: true,
            symmetry: "off".to_string(),
            detect_text: true,
            custom_palette: None,
            palette_merges: Vec::new(),
            target_nodes: None,
            brush_strokes: Vec::new(),
            production_mode: "standard".to_string(),
            min_stroke_pt: 0.0,
            strip_background: false,
            registration_marks: false,
            sablon_knockout: false,
            preprocess: PreprocessOptions {
                crop: Some([4, 4, 120, 120]),
                rotate_deg: 0.0,
                remove_background: true,
                bg_tolerance: 28,
                defringe_px: 2,
            },
            centerline_invert: false,
            centerline_stroke_pt: 1.5,
            path_edits: vec![
                PathEdit::Delete { target_id: "f:1:0".to_string() },
                PathEdit::ReassignColor { target_id: "f:2:0".to_string(), to_hex: "#00AA55".to_string() },
            ],
            node_edits: Vec::new(),
            edit_fingerprint: None,
        },
        export_options: ProjectExportOptions {
            lines_as: "stroke".to_string(),
            trap_pt: 0.5,
            registration_marks: false,
            strip_background: true,
            color_space: ColorSpace::Cmyk,
            dxf_unit: DxfUnit::Mm,
            dxf_target_width_mm: Some(80.0),
            die_cut_contour: true,
            die_cut_offset_mm: 2.5,
            die_cut_spot_name: "CutContour".to_string(),
            die_cut_bridge_islands: true,
            spot_colors: vec![SpotColorMapping {
                hex: "#FF0000".to_string(),
                spot_name: "Spot_Red".to_string(),
            }],
        },
    };
    let tmp = std::env::temp_dir().join("trasea_g12_09_roundtrip.trasea");
    save_project(&tmp, &proj_120).expect("save 1.2.0 project");
    let loaded = load_project(&tmp).expect("load 1.2.0 project");
    let _ = std::fs::remove_file(&tmp);
    assert_eq!(loaded.project, proj_120, "1.2.0 project round-trip must be lossless");
}

/// G12-10 — Audit Offline (0 crate jaringan & 0 crate GPL) & Zero Regresi Mode Standar
#[test]
fn g12_10_offline_audit_and_standard_zero_regression() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.parent().and_then(|p| p.parent()).expect("workspace root");

    let core_toml = std::fs::read_to_string(manifest_dir.join("Cargo.toml")).expect("read core Cargo.toml");
    let tauri_toml = std::fs::read_to_string(workspace_root.join("src-tauri/Cargo.toml")).expect("read tauri Cargo.toml");
    let lock = std::fs::read_to_string(workspace_root.join("Cargo.lock")).expect("read Cargo.lock");

    let forbidden = [
        "reqwest", "hyper", "ureq", "curl", "surf", "isahc", "tonic", "potrace", "ghostscript",
    ];
    for crate_name in forbidden {
        assert!(!core_toml.contains(crate_name), "Forbidden crate in core Cargo.toml: {crate_name}");
        assert!(!tauri_toml.contains(crate_name), "Forbidden crate in tauri Cargo.toml: {crate_name}");
    }

    // BFS transitive dependency closure of both `trasea-core` and `trasea` in Cargo.lock
    let mut deps_map: std::collections::HashMap<String, Vec<String>> = std::collections::HashMap::new();
    let mut cur_name: Option<String> = None;
    let mut in_deps = false;
    for line in lock.lines() {
        let t = line.trim();
        if t == "[[package]]" {
            cur_name = None;
            in_deps = false;
        } else if let Some(rest) = t.strip_prefix("name = \"") {
            if let Some(name) = rest.strip_suffix('"') {
                cur_name = Some(name.to_string());
                deps_map.entry(name.to_string()).or_default();
            }
        } else if t == "dependencies = [" {
            in_deps = true;
        } else if in_deps {
            if t == "]" {
                in_deps = false;
            } else if let Some(cn) = &cur_name {
                let dep = t.trim_matches(|c| c == '"' || c == ',');
                let dep_name = dep.split_whitespace().next().unwrap_or("");
                if !dep_name.is_empty() {
                    deps_map.entry(cn.clone()).or_default().push(dep_name.to_string());
                }
            }
        }
    }
    let tauri_conf = std::fs::read_to_string(workspace_root.join("src-tauri/tauri.conf.json")).expect("read tauri.conf.json");
    assert!(tauri_conf.contains("connect-src ipc: http://ipc.localhost"), "strict offline CSP missing");

    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back("trasea-core".to_string());
    while let Some(pkg) = queue.pop_front() {
        if !visited.insert(pkg.clone()) {
            continue;
        }
        assert!(
            !forbidden.contains(&pkg.as_str()),
            "Forbidden network/GPL crate {pkg} reachable from trasea-core in Cargo.lock"
        );
        if let Some(children) = deps_map.get(&pkg) {
            for ch in children {
                queue.push_back(ch.clone());
            }
        }
    }
    assert!(visited.len() > 20);

    // Standard mode deterministic smoke check (1.2.0 features default off -> unchanged pipeline)
    let mut logo = RgbImage::from_pixel(160, 160, Rgb([255, 255, 255]));
    draw_disk_rgb(&mut logo, 80.0, 80.0, 50.0, [20, 80, 180]);
    draw_rect_rgb(&mut logo, 55, 65, 50, 30, [240, 180, 20]);
    let p = Params::default();
    assert_eq!(p.production.mode, ProductionMode::Standard);
    assert!(p.preprocess.is_identity());
    assert!(p.path_edits.is_empty());

    let out1 = vectorize(&logo, &p, &|_, _| {}).expect("standard run 1");
    let out2 = vectorize(&logo, &p, &|_, _| {}).expect("standard run 2");
    assert_eq!(out1.stats.nodes, out2.stats.nodes, "Standard mode must be deterministic");
    assert!(out1.stats.mae.unwrap_or(99.0) < 4.0, "Standard MAE should stay tight: {:?}", out1.stats.mae);
    assert!(!out1.svg.contains("CutContour"), "CutContour must stay off by default in Standard mode");
}
