//! Automated tests for Trasea 1.1.0 ("Alur Kerja Cepat & Produksi Penuh") — Tahap 1 (F-01..F-10).

use image::{RgbImage, Rgba, RgbaImage};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use trasea_core::batch::{run_batch, BatchConfig, BatchStatus};
use trasea_core::color::{rgb_to_cmyk, ColorSpace};
use trasea_core::dxf::{to_dxf, DxfUnit, MM_PER_PX_96DPI};
use trasea_core::export::{
    palette_of, to_ase_with_color_space, to_eps, to_pdf, to_separation_eps, to_separation_pdfs,
    to_separation_svgs, Doc, ExportOptions, LinesAs, StrokeData, StrokeLayer, REG_MARGIN,
};
use trasea_core::pipeline::{vectorize, Params};
use trasea_core::preprocess::{apply_preprocess, PreprocessOptions};
use trasea_core::primitives::Primitive;
use trasea_core::production::{ProductionMode, ProductionOptions};
use trasea_core::project::{
    load_project, save_project, sha256_file, ProjectExportOptions, ProjectFile, ProjectOptions,
    PROJECT_SCHEMA_VERSION,
};
use trasea_core::trace::{Contour, Layer};
use trasea_core::watch::{watch_folder, WatchConfig};

const WHITE: [u8; 3] = [255, 255, 255];
const BLACK: [u8; 3] = [0, 0, 0];
const BLUE: [u8; 3] = [20, 80, 170];
const RED: [u8; 3] = [220, 45, 45];

fn temp_dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("trasea-11-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn rect_contour(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
    let line = |a: (f64, f64), b: (f64, f64)| [
        (a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0),
        (a.0 + 2.0 * (b.0 - a.0) / 3.0, a.1 + 2.0 * (b.1 - a.1) / 3.0),
        b,
    ];
    let (p0, p1, p2, p3) = ((x0, y0), (x1, y0), (x1, y1), (x0, y1));
    Contour {
        start: p0,
        curves: vec![line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)],
        prim: None,
    }
}

fn sample_doc_with_circle_and_black() -> Doc {
    let bg = Layer {
        id: "#FFFFFF".into(),
        color: WHITE,
        contours: vec![rect_contour(0.0, 0.0, 400.0, 200.0)],
        canvas: true,
        prims: vec![],
    };
    let blue = Layer {
        id: "#1450AA".into(),
        color: BLUE,
        contours: vec![rect_contour(20.0, 20.0, 160.0, 140.0)],
        canvas: false,
        prims: vec![Primitive::Circle { cx: 280.0, cy: 100.0, r: 45.0 }],
    };
    let black = Layer {
        id: "#000000".into(),
        color: BLACK,
        contours: vec![rect_contour(180.0, 30.0, 210.0, 170.0)],
        canvas: false,
        prims: vec![],
    };
    let strokes = vec![StrokeLayer {
        color: BLACK,
        strokes: vec![StrokeData {
            curves: vec![[(30.0, 180.0), (140.0, 160.0), (260.0, 200.0), (370.0, 180.0)]],
            width: 3.0,
        }],
    }];
    Doc {
        width: 200,
        height: 100,
        scale: 2.0,
        fields: vec![bg, blue, black],
        line_shapes: vec![],
        line_fallback: vec![],
        strokes,
    }
}

fn concentric_scene(w: u32, h: u32) -> RgbImage {
    let mut img = RgbImage::from_pixel(w, h, image::Rgb(WHITE));
    for y in 20..h - 20 {
        for x in 20..w - 20 {
            img.put_pixel(x, y, image::Rgb(BLUE));
        }
    }
    let (cx, cy, r) = (w as f32 * 0.5, h as f32 * 0.5, w.min(h) as f32 * 0.22);
    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            if dx * dx + dy * dy <= r * r {
                img.put_pixel(x, y, image::Rgb(RED));
            }
        }
    }
    img
}

fn gradient_scene(w: u32, h: u32) -> RgbImage {
    let mut img = RgbImage::new(w, h);
    let (cx, cy) = (w as f32 * 0.5, h as f32 * 0.5);
    let max_r = (w.min(h) as f32 * 0.45).max(1.0);
    for y in 0..h {
        let ty = y as f32 / (h - 1).max(1) as f32;
        for x in 0..w {
            let tx = x as f32 / (w - 1).max(1) as f32;
            let dx = x as f32 - cx;
            let dy = y as f32 - cy;
            let d = (dx * dx + dy * dy).sqrt();
            let px = if d < max_r {
                // Smooth shaded sphere from bright warm yellow to deep crimson
                let u = (d / max_r).clamp(0.0, 1.0);
                let r = (252.0 * (1.0 - 0.55 * u)).round() as u8;
                let g = (224.0 * (1.0 - 0.85 * u)).round() as u8;
                let b = (110.0 * (1.0 - 0.70 * u)).round() as u8;
                [r, g, b]
            } else {
                // Smooth sky-to-horizon gradient
                let r = (22.0 + 175.0 * ty + 25.0 * tx).clamp(0.0, 255.0).round() as u8;
                let g = (48.0 + 145.0 * ty + 15.0 * (1.0 - tx)).clamp(0.0, 255.0).round() as u8;
                let b = (125.0 + 95.0 * (1.0 - ty) + 20.0 * tx).clamp(0.0, 255.0).round() as u8;
                [r, g, b]
            };
            img.put_pixel(x, y, image::Rgb(px));
        }
    }
    img
}

/// F-01: Pure ASCII DXF R12 structure, colour layers, CIRCLE primitive, units (mm/px + target width), no NaN/inf.
#[test]
fn f01_dxf_export_valid_structure_layers_and_units() {
    let doc = sample_doc_with_circle_and_black();

    // 1. mm with explicit target width 100 mm
    let opts_100mm = ExportOptions {
        dxf_unit: DxfUnit::Mm,
        dxf_target_width_mm: Some(100.0),
        registration_marks: false,
        ..Default::default()
    };
    let dxf = to_dxf(&doc, &opts_100mm);
    assert!(dxf.starts_with("0\nSECTION\n2\nHEADER\n"), "missing DXF HEADER");
    assert!(dxf.contains("AC1009"), "expected AutoCAD R12 AC1009");
    assert!(dxf.contains("$INSUNITS\n70\n4\n"), "expected mm INSUNITS=4");
    assert!(dxf.contains("$EXTMAX\n10\n100\n20\n50\n"), "expected 100x50 mm extents, got:\n{}", &dxf[..dxf.len().min(400)]);
    assert!(dxf.contains("2\nTABLES\n") && dxf.contains("2\nLAYER\n"), "missing LAYER table");
    assert!(dxf.contains("COLOR_01_FFFFFF") && dxf.contains("COLOR_02_1450AA") && dxf.contains("THIN_LINES"), "missing colour/line layers");
    assert!(dxf.contains("2\nENTITIES\n") && dxf.contains("0\nCIRCLE\n"), "missing native CIRCLE entity");
    assert!(dxf.contains("0\nPOLYLINE\n") && dxf.contains("0\nVERTEX\n") && dxf.contains("0\nSEQEND\n"), "missing POLYLINE/VERTEX/SEQEND");
    assert!(dxf.trim_end().ends_with("0\nEOF"), "missing EOF");
    assert!(!dxf.contains("NaN") && !dxf.contains("inf") && !dxf.contains("Inf"), "DXF must not contain NaN/inf");

    // 2. Default mm (96 DPI: 200 px * 25.4 / 96 = 52.9167 mm) + registration layer
    let opts_default_mm = ExportOptions {
        dxf_unit: DxfUnit::Mm,
        dxf_target_width_mm: None,
        registration_marks: true,
        ..Default::default()
    };
    let dxf_reg = to_dxf(&doc, &opts_default_mm);
    assert!(dxf_reg.contains("REGISTRATION"), "missing REGISTRATION layer");
    let expected_w = (doc.width as f64 + 2.0 * REG_MARGIN) * MM_PER_PX_96DPI;
    let w_str = format!("{:.4}", expected_w);
    let w_trim = w_str.trim_end_matches('0').trim_end_matches('.');
    assert!(dxf_reg.contains(&format!("$EXTMAX\n10\n{w_trim}\n")), "default 96 DPI width mismatch (expected {w_trim})");

    // 3. px unit
    let opts_px = ExportOptions { dxf_unit: DxfUnit::Px, ..Default::default() };
    let dxf_px = to_dxf(&doc, &opts_px);
    assert!(dxf_px.contains("$INSUNITS\n70\n0\n"), "expected unitless INSUNITS=0 for px");
    assert!(dxf_px.contains("$EXTMAX\n10\n200\n20\n100\n"), "expected 200x100 px extents");
}

/// F-02: CMYK export in PDF, EPS, and ASE uses DeviceCMYK operators/tags, zero RGB operators, and Pure Black 0 0 0 1.
#[test]
fn f02_cmyk_export_pdf_eps_and_ase_uses_cmyk_and_pure_black() {
    assert_eq!(rgb_to_cmyk(0, 0, 0), [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(rgb_to_cmyk(6, 7, 8), [0.0, 0.0, 0.0, 1.0]);

    let doc = sample_doc_with_circle_and_black();
    let opts = ExportOptions {
        color_space: ColorSpace::Cmyk,
        registration_marks: true,
        trap_pt: 0.5,
        ..Default::default()
    };

    // PDF CMYK: must parse cleanly with lopdf, use k/K, Pure Black 0 0 0 1, and contain zero rg/RG.
    let pdf_bytes = to_pdf(&doc, &opts).expect("CMYK PDF export");
    let pdf_doc = lopdf::Document::load_mem(&pdf_bytes).expect("lopdf should parse pure CMYK PDF");
    let pages = pdf_doc.get_pages();
    assert_eq!(pages.len(), 1);
    let page_id = *pages.values().next().unwrap();
    let content_bytes = pdf_doc.get_page_content(page_id).expect("page content");
    let content = String::from_utf8_lossy(&content_bytes);
    assert!(content.contains(" k\n") && content.contains(" K\n"), "CMYK PDF missing k/K operators");
    assert!(content.contains("0 0 0 1 k\n") && content.contains("0 0 0 1 K\n"), "CMYK PDF missing Pure Black 0 0 0 1");
    assert!(!content.contains(" rg\n") && !content.contains(" RG\n"), "CMYK PDF must not emit rg/RG operators");

    // EPS CMYK: must use setcmykcolor + 0 0 0 1 CMYK, zero setrgbcolor/RGB.
    let eps = to_eps(&doc, &opts);
    assert!(eps.contains("/CMYK { setcmykcolor } bind def"), "EPS CMYK missing setcmykcolor prolog");
    assert!(eps.contains("0 0 0 1 CMYK"), "EPS CMYK missing Pure Black 0 0 0 1");
    assert!(!eps.contains("setrgbcolor") && !eps.contains(" RGB\n"), "EPS CMYK must not contain RGB operators");

    // ASE CMYK: must carry CMYK colour tag and exact [0, 0, 0, 1] floats for #000000.
    let pal = palette_of(&doc);
    let ase = to_ase_with_color_space(&pal, ColorSpace::Cmyk);
    assert!(ase.starts_with(b"ASEF"));
    assert!(ase.windows(4).any(|w| w == b"CMYK"), "ASE CMYK missing CMYK tag");
    assert!(!ase.windows(4).any(|w| w == b"RGB "), "ASE CMYK must not contain RGB tag");
    let mut pure_k_bytes = Vec::new();
    pure_k_bytes.extend_from_slice(b"CMYK");
    for v in [0.0f32, 0.0, 0.0, 1.0] {
        pure_k_bytes.extend_from_slice(&v.to_be_bytes());
    }
    assert!(ase.windows(pure_k_bytes.len()).any(|w| w == pure_k_bytes.as_slice()), "ASE CMYK missing Pure Black [0,0,0,1]");
}

/// F-03: Screen Print pure knockout separations have zero core overlap between spot colours + 4 identical registration marks.
#[test]
fn f03_sablon_knockout_separations_avoid_core_overlap_with_marks() {
    let img = concentric_scene(180, 180);
    let mut prod = ProductionOptions::screen_print();
    prod.sablon_knockout = true;
    let params = Params {
        colors: 3,
        smooth: 0.4,
        keep_lines: false,
        detect_text: false,
        measure: false,
        production: prod,
        ..Params::default()
    };
    let out = vectorize(&img, &params, &|_, _| {}).unwrap();
    let opts = ExportOptions {
        registration_marks: true,
        trap_pt: 0.0,
        color_space: ColorSpace::Cmyk,
        ..Default::default()
    };
    let films = to_separation_svgs(&out.doc, &opts);
    assert_eq!(films.len(), 3, "expected 3 spot films (white, blue, red), got {}", films.len());

    // Find blue and red films
    let blue_film = films.iter().find(|f| f.color[2] > 120 && f.color[0] < 80).expect("blue film");
    let red_film = films.iter().find(|f| f.color[0] > 160 && f.color[2] < 80).expect("red film");

    // Identical viewBox and 4 registration marks in every film
    let expected_vw = (out.doc.width as f64 + 2.0 * REG_MARGIN) as u32;
    let expected_vb = format!("viewBox=\"0 0 {expected_vw} {expected_vw}\"");
    for f in &films {
        assert!(f.svg.contains(&expected_vb), "film {} viewBox mismatch", f.suffix);
        assert!(f.svg.contains("id=\"Registrasi\""), "film {} missing registration marks", f.suffix);
        assert_eq!(f.svg.matches("r=\"6\" fill=\"none\" stroke=\"#000000\"").count(), 4);
    }

    // Render blue and red films and verify center of the red circle is black in red_film and white (knocked out!) in blue_film.
    let render_blue = trasea_core::stats::render_svg(&blue_film.svg, expected_vw, expected_vw).unwrap();
    let render_red = trasea_core::stats::render_svg(&red_film.svg, expected_vw, expected_vw).unwrap();
    let mid = expected_vw / 2;
    let pb = render_blue.get_pixel(mid, mid).0;
    let pr = render_red.get_pixel(mid, mid).0;
    assert!(pr[0] < 20 && pr[1] < 20 && pr[2] < 20, "center should be solid black on red film, got {pr:?}");
    assert!(pb[0] > 235 && pb[1] > 235 && pb[2] > 235, "center must be knocked out (white) on blue film, got {pb:?}");

    // Also verify CMYK PDF & EPS separation films use pure K = 0 0 0 1 with zero RGB.
    let sep_pdfs = to_separation_pdfs(&out.doc, &opts).unwrap();
    assert_eq!(sep_pdfs.len(), 3);
    for (_, bytes) in &sep_pdfs {
        let pdoc = lopdf::Document::load_mem(bytes).expect("valid separation CMYK PDF");
        let pid = *pdoc.get_pages().values().next().unwrap();
        let txt = String::from_utf8_lossy(&pdoc.get_page_content(pid).unwrap()).into_owned();
        assert!(txt.contains("0 0 0 1 k") && txt.contains("0 0 0 1 K"));
        assert!(!txt.contains(" rg\n") && !txt.contains(" RG\n"));
    }
    let sep_eps = to_separation_eps(&out.doc, &opts);
    assert_eq!(sep_eps.len(), 3);
    for (_, eps) in &sep_eps {
        assert!(eps.contains("0 0 0 1 CMYK") && !eps.contains("setrgbcolor"));
    }
}

/// F-04: Batch run exports DXF + CMYK PDF/EPS/ASE alongside SVG and writes CSV/MD reports.
#[test]
fn f04_batch_exports_dxf_and_cmyk_alongside_svg() {
    let dir = temp_dir("f04");
    let in_dir = dir.join("in");
    let out_dir = dir.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    let p1 = in_dir.join("badge.png");
    concentric_scene(160, 120).save(&p1).unwrap();

    let cfg = BatchConfig {
        inputs: vec![p1],
        output_dir: out_dir.clone(),
        params: Params { ai_clean: false, measure: false, detect_text: false, ..Params::default() },
        export: ExportOptions::default(),
        write_pdf: true,
        write_eps: true,
        write_dxf: true,
        write_ase: true,
        write_separations: false,
        write_report_csv: true,
        write_report_md: true,
        color_space: ColorSpace::Cmyk,
        dxf_unit: DxfUnit::Mm,
        dxf_target_width_mm: Some(100.0),
        max_workers: Some(1),
    };
    let summary = run_batch(&cfg, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert_eq!(summary.succeeded, 1);
    assert!(out_dir.join("badge.svg").is_file());
    assert!(out_dir.join("badge.dxf").is_file());
    assert!(out_dir.join("badge.pdf").is_file());
    assert!(out_dir.join("badge.eps").is_file());
    assert!(out_dir.join("badge.ase").is_file());
    assert!(out_dir.join("laporan-trasea.csv").is_file());
    assert!(out_dir.join("laporan-trasea.md").is_file());

    let dxf = std::fs::read_to_string(out_dir.join("badge.dxf")).unwrap();
    assert!(dxf.contains("AC1009") && dxf.contains("$EXTMAX\n10\n100\n"));
    let eps = std::fs::read_to_string(out_dir.join("badge.eps")).unwrap();
    assert!(eps.contains("setcmykcolor") && !eps.contains("setrgbcolor"));
    let ase = std::fs::read(out_dir.join("badge.ase")).unwrap();
    assert!(ase.windows(4).any(|w| w == b"CMYK"));
    let pdf = std::fs::read(out_dir.join("badge.pdf")).unwrap();
    let pdoc = lopdf::Document::load_mem(&pdf).unwrap();
    let pid = *pdoc.get_pages().values().next().unwrap();
    let ptxt = String::from_utf8_lossy(&pdoc.get_page_content(pid).unwrap()).into_owned();
    assert!(ptxt.contains(" k\n") && !ptxt.contains(" rg\n"));
    let _ = std::fs::remove_dir_all(&dir);
}

/// F-05: Non-destructive crop & rotation (exact 90° and free 15°) are accurate and never modify source file on disk.
#[test]
fn f05_preprocess_crop_and_rotate_non_destructive_and_accurate() {
    let dir = temp_dir("f05");
    let src_path = dir.join("rect.png");
    let mut rgba = RgbaImage::from_pixel(120, 80, Rgba([240, 240, 240, 255]));
    for y in 20..60 {
        for x in 10..60 {
            rgba.put_pixel(x, y, Rgba([220, 40, 40, 255]));
        }
    }
    rgba.save(&src_path).unwrap();
    let hash_before = sha256_file(&src_path).unwrap();

    // Exact crop
    let cropped = apply_preprocess(&rgba, &PreprocessOptions { crop: Some([10, 20, 50, 40]), rotate_deg: 0.0 });
    assert_eq!(cropped.dimensions(), (50, 40));
    assert_eq!(cropped.get_pixel(25, 20).0, [220, 40, 40, 255]);

    // Exact 90 deg rotation swaps dimensions
    let rot90 = apply_preprocess(&rgba, &PreprocessOptions { crop: None, rotate_deg: 90.0 });
    assert_eq!(rot90.dimensions(), (80, 120));

    // Free angle 15 deg expands canvas cleanly without panicking
    let rot15 = apply_preprocess(&rgba, &PreprocessOptions { crop: None, rotate_deg: 15.0 });
    assert!(rot15.width() > 120 && rot15.height() > 80);

    // End-to-end vectorize with preprocess (rotate 90 + crop 60x70)
    let rgb = trasea_core::load::load_path(&src_path, 1600).unwrap();
    let out = vectorize(
        &rgb,
        &Params {
            preprocess: PreprocessOptions { crop: Some([5, 10, 60, 70]), rotate_deg: 90.0 },
            ai_clean: false,
            measure: false,
            detect_text: false,
            ..Params::default()
        },
        &|_, _| {},
    )
    .unwrap();
    assert_eq!((out.doc.width, out.doc.height), (60, 70));

    let hash_after = sha256_file(&src_path).unwrap();
    assert_eq!(hash_before, hash_after, "source file on disk must never be altered");
    let _ = std::fs::remove_dir_all(&dir);
}

/// F-06: `.trasea` v1.0.0 backward compatibility + v1.1.0 full roundtrip (preprocess, colorSpace, sablonKnockout, dxf).
#[test]
fn f06_project_10_backward_compat_and_11_roundtrip() {
    let dir = temp_dir("f06");
    let img_path = dir.join("src.png");
    concentric_scene(80, 80).save(&img_path).unwrap();
    let sha = sha256_file(&img_path).unwrap();

    // 1. Exact 1.0.0 JSON without any 1.1.0 fields must load cleanly with defaults.
    let v10_path = dir.join("legacy-1.0.0.trasea");
    let v10_json = serde_json::json!({
        "schemaVersion": 1,
        "appVersion": "1.0.0",
        "sourcePath": img_path.to_string_lossy(),
        "sourceSha256": sha,
        "options": {
            "colors": 8,
            "smooth": 50.0,
            "keepLines": true,
            "aiClean": false,
            "detectPrimitives": true,
            "symmetry": "off",
            "detectText": true,
            "customPalette": null,
            "paletteMerges": [],
            "targetNodes": null,
            "brushStrokes": [],
            "productionMode": "standard",
            "minStrokePt": 0.0,
            "stripBackground": false,
            "registrationMarks": false
        },
        "exportOptions": {
            "linesAs": "stroke",
            "trapPt": 0.0,
            "registrationMarks": false,
            "stripBackground": false
        }
    });
    std::fs::write(&v10_path, serde_json::to_string_pretty(&v10_json).unwrap()).unwrap();
    let loaded10 = load_project(&v10_path).expect("1.0.0 .trasea must still load cleanly");
    assert!(loaded10.source_exists && loaded10.hash_matches);
    assert!(!loaded10.project.options.sablon_knockout);
    assert!(loaded10.project.options.preprocess.is_identity());
    assert_eq!(loaded10.project.export_options.color_space, ColorSpace::Rgb);
    assert_eq!(loaded10.project.export_options.dxf_unit, DxfUnit::Mm);
    assert_eq!(loaded10.project.export_options.dxf_target_width_mm, None);

    // 2. Full 1.1.0 roundtrip
    let v11_path = dir.join("v11.trasea");
    let proj11 = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: "1.1.0".into(),
        source_path: img_path.to_string_lossy().to_string(),
        source_sha256: sha,
        options: ProjectOptions {
            colors: 6,
            smooth: 60.0,
            keep_lines: true,
            ai_clean: false,
            detect_primitives: true,
            symmetry: "off".into(),
            detect_text: true,
            custom_palette: None,
            palette_merges: vec![],
            target_nodes: None,
            brush_strokes: vec![],
            production_mode: "screenPrint".into(),
            min_stroke_pt: 1.0,
            strip_background: false,
            registration_marks: true,
            sablon_knockout: true,
            preprocess: PreprocessOptions { crop: Some([4, 6, 70, 68]), rotate_deg: -12.5 },
        },
        export_options: ProjectExportOptions {
            lines_as: "stroke".into(),
            trap_pt: 0.5,
            registration_marks: true,
            strip_background: false,
            color_space: ColorSpace::Cmyk,
            dxf_unit: DxfUnit::Mm,
            dxf_target_width_mm: Some(120.0),
        },
    };
    save_project(&v11_path, &proj11).unwrap();
    let loaded11 = load_project(&v11_path).unwrap();
    assert_eq!(loaded11.project, proj11);
    let _ = std::fs::remove_dir_all(&dir);
}

/// F-07: ProductionMode::Shading achieves lower colour MAE than ProductionMode::Standard (10 colours) on a smooth gradient image.
#[test]
fn f07_shading_mode_improves_gradient_image_mae() {
    let img = gradient_scene(240, 160);
    let std_out = vectorize(
        &img,
        &Params {
            colors: 10,
            smooth: 0.5,
            keep_lines: false,
            detect_text: false,
            measure: true,
            production: ProductionOptions::standard(),
            ..Params::default()
        },
        &|_, _| {},
    )
    .unwrap();
    let shd_out = vectorize(
        &img,
        &Params {
            colors: 18,
            smooth: 0.5,
            keep_lines: false,
            detect_text: false,
            measure: true,
            production: ProductionOptions::shading(),
            ..Params::default()
        },
        &|_, _| {},
    )
    .unwrap();
    let mae_std = std_out.stats.mae.unwrap();
    let mae_shd = shd_out.stats.mae.unwrap();
    assert!(shd_out.palette.len() > std_out.palette.len(), "Shading should use more tonal bands ({} vs {})", shd_out.palette.len(), std_out.palette.len());
    assert!(shd_out.stats.paths > 0 && shd_out.stats.nodes > 0);
    assert!(
        mae_shd < mae_std,
        "Shading mode MAE ({mae_shd:.2}) should be lower than Standard 10-color MAE ({mae_std:.2})"
    );
}

/// F-08: Watch Folder detects new PNG, writes SVG+DXF+reports within 5s, skips unchanged file on subsequent polls,
/// survives corrupt files without crashing, and stops cleanly via AtomicBool.
#[test]
fn f08_watch_folder_processes_new_file_once_and_stops_cleanly() {
    let dir = temp_dir("f08");
    let in_dir = dir.join("in");
    let out_dir = dir.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();

    let batch = BatchConfig {
        output_dir: out_dir.clone(),
        params: Params { ai_clean: false, measure: false, detect_text: false, ..Params::default() },
        write_dxf: true,
        write_report_csv: true,
        write_report_md: true,
        ..BatchConfig::default()
    };
    let mut wcfg = WatchConfig::new(in_dir.clone(), out_dir.clone(), batch);
    wcfg.poll_ms = 80;
    wcfg.debounce_ms = 100;

    let stop = Arc::new(AtomicBool::new(false));
    let stop_bg = Arc::clone(&stop);
    let seen = Arc::new(Mutex::new(Vec::<(String, BatchStatus)>::new()));
    let seen_bg = Arc::clone(&seen);

    let handle = std::thread::spawn(move || {
        watch_folder(&wcfg, stop_bg, |ev| {
            seen_bg.lock().unwrap().push((ev.file_name.clone(), ev.status));
        })
    });

    // Drop 1 valid PNG and 1 corrupt PNG
    concentric_scene(100, 100).save(in_dir.join("good.png")).unwrap();
    std::fs::write(in_dir.join("broken.png"), b"not-a-valid-png").unwrap();

    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if seen.lock().unwrap().len() >= 2 && out_dir.join("good.dxf").is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    assert!(out_dir.join("good.svg").is_file(), "watcher did not write good.svg in < 5s");
    assert!(out_dir.join("good.dxf").is_file(), "watcher did not write good.dxf in < 5s");
    assert!(out_dir.join("laporan-trasea.csv").is_file(), "watcher did not write laporan-trasea.csv");
    assert!(out_dir.join("laporan-trasea.md").is_file(), "watcher did not write laporan-trasea.md");

    // Wait 3 more polling cycles and verify unchanged files are NOT reprocessed.
    std::thread::sleep(Duration::from_millis(320));
    let events_snap = seen.lock().unwrap().clone();
    assert_eq!(events_snap.len(), 2, "unchanged files must not be reprocessed: {events_snap:?}");
    assert!(events_snap.iter().any(|(n, s)| n == "good.png" && *s == BatchStatus::Ok));
    assert!(events_snap.iter().any(|(n, s)| n == "broken.png" && *s == BatchStatus::Failed));

    // Stop cleanly
    let t_stop = Instant::now();
    stop.store(true, Ordering::Relaxed);
    let res = handle.join().expect("watch thread joined cleanly").expect("watch_folder Ok");
    assert!(t_stop.elapsed() < Duration::from_secs(2), "watcher took too long to stop");
    assert_eq!(res.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}

/// F-09: trasea-core transitive dependency graph in Cargo.lock + both Cargo.toml manifests + CSP
/// remain 100% offline (zero network crates) and free of GPL-only crates.
#[test]
fn f09_offline_audit_zero_network_crates_still_passes() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let core_toml = std::fs::read_to_string(root.join("crates/trasea-core/Cargo.toml")).unwrap();
    let tauri_toml = std::fs::read_to_string(root.join("src-tauri/Cargo.toml")).unwrap();
    let tauri_conf = std::fs::read_to_string(root.join("src-tauri/tauri.conf.json")).unwrap();
    let lock = std::fs::read_to_string(root.join("Cargo.lock")).unwrap();

    let banned = [
        "reqwest", "ureq", "hyper", "curl", "surf", "isahc", "attohttpc",
        "octocrab", "sentry", "posthog", "tokio-tungstenite", "potrace", "ghostscript",
    ];
    for b in banned {
        assert!(!core_toml.contains(b), "banned crate {b} in trasea-core/Cargo.toml");
        assert!(!tauri_toml.contains(b), "banned crate {b} in src-tauri/Cargo.toml");
    }
    assert!(tauri_conf.contains("connect-src ipc: http://ipc.localhost"), "strict offline CSP missing");

    // Parse Cargo.lock package dependency map and BFS from trasea-core.
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
    let mut visited = std::collections::HashSet::new();
    let mut queue = std::collections::VecDeque::new();
    queue.push_back("trasea-core".to_string());
    while let Some(pkg) = queue.pop_front() {
        if !visited.insert(pkg.clone()) {
            continue;
        }
        assert!(
            !banned.contains(&pkg.as_str()),
            "banned network/GPL crate {pkg} reachable from trasea-core in Cargo.lock"
        );
        if let Some(children) = deps_map.get(&pkg) {
            for ch in children {
                queue.push_back(ch.clone());
            }
        }
    }
    assert!(visited.len() > 20, "expected transitive dependency closure of trasea-core to be non-trivial");
}

/// F-10: Sanity check that 1.1.0 defaults preserve 1.0.0 Standard behaviour unchanged.
#[test]
fn f10_regression_defaults_preserve_1_0_behaviour() {
    let p = Params::default();
    assert_eq!(p.colors, 10);
    assert!((p.smooth - 0.5).abs() < 1e-6);
    assert_eq!(p.production.mode, ProductionMode::Standard);
    assert!(!p.production.sablon_knockout);
    assert!(p.preprocess.is_identity());

    let eo = ExportOptions::default();
    assert_eq!(eo.lines_as, LinesAs::Stroke);
    assert_eq!(eo.color_space, ColorSpace::Rgb);
    assert_eq!(eo.dxf_unit, DxfUnit::Mm);
    assert_eq!(eo.dxf_target_width_mm, None);
}
