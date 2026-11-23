//! Automatic tests E-01..E-03, L-01..L-03, P-01..P-02, N-01 for Trasea 1.0.0 ("Siap Dijual").

use image::{Rgb, RgbImage};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use trasea_core::batch::{run_batch, scan_image_files, BatchConfig};
use trasea_core::brush::{BrushKind, BrushStroke};
use trasea_core::export::{to_eps, Doc, ExportOptions, StrokeData, StrokeLayer, REG_MARGIN};
use trasea_core::license::{generate_keypair, sign_license, verify_license, LicenseInfo};
use trasea_core::primitives::Primitive;
use trasea_core::project::{load_project, save_project, sha256_file, ProjectExportOptions, ProjectFile, ProjectOptions, PROJECT_SCHEMA_VERSION};
use trasea_core::trace::{Contour, Layer};
use trasea_core::Params;

const WHITE: [u8; 3] = [255, 255, 255];

fn quiet() -> Params {
    Params { ai_clean: false, measure: false, keep_lines: false, detect_text: false, ..Params::default() }
}

fn rect_contour(x0: f64, y0: f64, x1: f64, y1: f64) -> Contour {
    let line = |a: (f64, f64), b: (f64, f64)| [
        (a.0 + (b.0 - a.0) / 3.0, a.1 + (b.1 - a.1) / 3.0),
        (a.0 + 2.0 * (b.0 - a.0) / 3.0, a.1 + 2.0 * (b.1 - a.1) / 3.0),
        b,
    ];
    let (p0, p1, p2, p3) = ((x0, y0), (x1, y0), (x1, y1), (x0, y1));
    Contour { start: p0, curves: vec![line(p0, p1), line(p1, p2), line(p2, p3), line(p3, p0)], prim: None }
}

fn rich_doc() -> Doc {
    let bg = Layer {
        id: "#FFFFFF".into(),
        color: WHITE,
        contours: vec![rect_contour(0.0, 0.0, 200.0, 200.0)],
        canvas: true,
        prims: vec![],
    };
    let shapes = Layer {
        id: "#0F3973".into(),
        color: [15, 57, 115],
        contours: vec![rect_contour(10.0, 10.0, 50.0, 50.0)],
        canvas: false,
        prims: vec![
            Primitive::Circle { cx: 100.0, cy: 100.0, r: 30.0 },
            Primitive::Ellipse { cx: 140.0, cy: 60.0, rx: 24.0, ry: 12.0, angle_deg: 25.0 },
            Primitive::Rect { x: 40.0, y: 120.0, w: 60.0, h: 40.0, rx: 8.0, angle_deg: 15.0 },
        ],
    };
    let strokes = vec![StrokeLayer {
        color: [20, 20, 20],
        strokes: vec![StrokeData {
            curves: vec![[(20.0, 20.0), (50.0, 10.0), (80.0, 30.0), (120.0, 20.0)]],
            width: 3.0,
        }],
    }];
    Doc {
        width: 100,
        height: 100,
        scale: 2.0,
        fields: vec![bg, shapes],
        line_shapes: vec![],
        line_fallback: vec![],
        strokes,
    }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("trasea-10-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

// ---------------------------------------------------------------- E-01

#[test]
fn e01_eps_export_contains_valid_epsf3_header_geometry_and_eof() {
    let doc = rich_doc();
    let eps = to_eps(&doc, &ExportOptions::default());
    assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0
"), "missing EPSF-3.0 header");
    assert!(eps.contains("%%BoundingBox: 0 0 100 100
"), "missing BoundingBox");
    assert!(eps.contains("%%HiResBoundingBox: 0 0 100.00 100.00
"), "missing HiResBoundingBox");
    assert!(eps.contains("eofill") && eps.contains("stroke") && eps.contains("curveto"), "missing PS operators");
    assert!(eps.contains(" C
") && eps.contains("
F
") && eps.contains("
S
"), "missing drawn curves/fills/strokes");
    assert!(eps.trim_end().ends_with("%%EOF"), "missing %%EOF");
    assert!(!eps.contains("NaN") && !eps.contains("inf") && !eps.contains("Infinity"), "EPS contains invalid float");
}

// ---------------------------------------------------------------- E-02

#[test]
fn e02_eps_registration_marks_and_trap_expand_bbox_and_emit_marks() {
    let doc = rich_doc();
    let opts = ExportOptions { registration_marks: true, trap_pt: 0.5, ..Default::default() };
    let eps = to_eps(&doc, &opts);
    let expected = (doc.width as f64 + 2.0 * REG_MARGIN).ceil() as u32;
    let bb = format!("%%BoundingBox: 0 0 {expected} {expected}
");
    assert!(eps.contains(&bb), "expected expanded {bb}");
    assert!(eps.contains("0 0 0 RGB 0.75 setlinewidth"), "missing registration marks stroke");
    assert!(eps.contains("gsave F grestore 1 setlinejoin") && eps.contains("setlinewidth S"), "missing print trap");
    // 4 registration targets -> 4 crosshair blocks after the 0 0 0 RGB marker
    let reg_part = &eps[eps.find("0 0 0 RGB 0.75 setlinewidth").unwrap()..];
    assert_eq!(reg_part.matches(" Z S
").count(), 4, "expected 4 circular registration targets");
}

// ---------------------------------------------------------------- E-03

#[test]
fn e03_batch_with_write_eps_produces_valid_eps_alongside_svg() {
    let root = temp_dir("e03");
    let in_dir = root.join("in");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    for (name, col) in [("one.png", [200u8, 40, 40]), ("two.png", [40, 90, 200])] {
        let mut img = RgbImage::from_pixel(160, 160, Rgb(WHITE));
        for y in 30..130 {
            for x in 30..130 {
                img.put_pixel(x, y, Rgb(col));
            }
        }
        img.save(in_dir.join(name)).unwrap();
    }
    let files = scan_image_files(&in_dir, false).unwrap();
    let cfg = BatchConfig {
        inputs: files,
        output_dir: out_dir.clone(),
        params: quiet(),
        export: ExportOptions::default(),
        write_pdf: false,
        write_eps: true,
        write_ase: false,
        write_separations: false,
        write_report_csv: false,
        write_report_md: false,
        max_workers: None,
    };
    let summary = run_batch(&cfg, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert_eq!(summary.succeeded, 2);
    for stem in ["one", "two"] {
        assert!(out_dir.join(format!("{stem}.svg")).is_file());
        let eps = std::fs::read_to_string(out_dir.join(format!("{stem}.eps"))).unwrap();
        assert!(eps.starts_with("%!PS-Adobe-3.0 EPSF-3.0") && eps.trim_end().ends_with("%%EOF"));
    }
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- L-01

#[test]
fn l01_sign_and_verify_ed25519_license_roundtrip() {
    let (secret, pubkey) = generate_keypair().unwrap();
    let info = LicenseInfo {
        v: 1,
        id: "TR-2026-0042".into(),
        name: "Budi Santoso".into(),
        tier: "pro".into(),
        issued: "2026-11-23".into(),
    };
    let key = sign_license(&info, &secret).unwrap();
    assert!(key.starts_with("TRSA1-"));
    let parsed = verify_license(&key, &pubkey).unwrap();
    assert_eq!(parsed, info);
}

// ---------------------------------------------------------------- L-02

#[test]
fn l02_tampered_payload_signature_or_prefix_is_rejected() {
    let (secret, pubkey) = generate_keypair().unwrap();
    let info = LicenseInfo {
        v: 1,
        id: "TR-2026-0001".into(),
        name: "Tester".into(),
        tier: "pro".into(),
        issued: "2026-11-23".into(),
    };
    let key = sign_license(&info, &secret).unwrap();
    // Flip 1 character in payload
    let mut chars: Vec<char> = key.chars().collect();
    chars[8] = if chars[8] == 'A' { 'B' } else { 'A' };
    let bad_payload: String = chars.into_iter().collect();
    assert!(verify_license(&bad_payload, &pubkey).is_err(), "tampered payload must fail");

    // Flip 1 character in signature
    let mut sig_chars: Vec<char> = key.chars().collect();
    let last = sig_chars.len() - 2;
    sig_chars[last] = if sig_chars[last] == 'A' { 'B' } else { 'A' };
    let bad_sig: String = sig_chars.into_iter().collect();
    assert!(verify_license(&bad_sig, &pubkey).is_err(), "tampered signature must fail");

    // Wrong prefix
    let bad_prefix = key.replacen("TRSA1-", "TRSA2-", 1);
    assert!(verify_license(&bad_prefix, &pubkey).is_err(), "wrong prefix must fail");
}

// ---------------------------------------------------------------- L-03

#[test]
fn l03_license_signed_by_different_private_key_is_rejected() {
    let (secret_a, _pub_a) = generate_keypair().unwrap();
    let (_secret_b, pub_b) = generate_keypair().unwrap();
    let info = LicenseInfo {
        v: 1,
        id: "TR-2026-0099".into(),
        name: "Wrong Key".into(),
        tier: "pro".into(),
        issued: "2026-11-23".into(),
    };
    let key = sign_license(&info, &secret_a).unwrap();
    assert!(verify_license(&key, &pub_b).is_err(), "key signed by another private key must fail");
}

// ---------------------------------------------------------------- P-01

#[test]
fn p01_project_file_roundtrip_preserves_all_fields_and_sha256() {
    let root = temp_dir("p01");
    let img_path = root.join("art.png");
    RgbImage::from_pixel(64, 64, Rgb([10, 40, 120])).save(&img_path).unwrap();
    let sha = sha256_file(&img_path).unwrap();

    let proj = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: "1.0.0".into(),
        source_path: img_path.to_string_lossy().to_string(),
        source_sha256: sha,
        options: ProjectOptions {
            colors: 6,
            smooth: 65.0,
            keep_lines: true,
            ai_clean: false,
            detect_primitives: true,
            symmetry: "vertical".into(),
            detect_text: true,
            custom_palette: Some(vec!["#0F3973".into(), "#EBD9C1".into(), "#EF8D84".into()]),
            palette_merges: vec![("#F7ED39".into(), "#0F3973".into())],
            target_nodes: Some(1200),
            brush_strokes: vec![BrushStroke { kind: BrushKind::Detail, radius: 30.0, points: vec![(10.0, 20.0), (40.0, 50.0)] }],
            production_mode: "screenPrint".into(),
            min_stroke_pt: 1.0,
            strip_background: false,
            registration_marks: true,
        },
        export_options: ProjectExportOptions {
            lines_as: "stroke".into(),
            trap_pt: 0.5,
            registration_marks: true,
            strip_background: false,
        },
    };
    let p_path = root.join("demo.trasea");
    save_project(&p_path, &proj).unwrap();
    let loaded = load_project(&p_path).unwrap();
    assert!(loaded.source_exists);
    assert!(loaded.hash_matches);
    assert_eq!(loaded.project, proj);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- P-02

#[test]
fn p02_project_load_handles_missing_or_modified_source_gracefully() {
    let root = temp_dir("p02");
    let img_path = root.join("source.png");
    RgbImage::from_pixel(40, 40, Rgb([200, 50, 50])).save(&img_path).unwrap();
    let sha = sha256_file(&img_path).unwrap();

    let proj = ProjectFile {
        schema_version: PROJECT_SCHEMA_VERSION,
        app_version: "1.0.0".into(),
        source_path: img_path.to_string_lossy().to_string(),
        source_sha256: sha,
        options: ProjectOptions {
            colors: 4,
            smooth: 70.0,
            keep_lines: true,
            ai_clean: false,
            detect_primitives: true,
            symmetry: "off".into(),
            detect_text: true,
            custom_palette: None,
            palette_merges: vec![],
            target_nodes: None,
            brush_strokes: vec![BrushStroke { kind: BrushKind::Smooth, radius: 45.0, points: vec![(5.0, 5.0)] }],
            production_mode: "cutEmbroidery".into(),
            min_stroke_pt: 1.5,
            strip_background: true,
            registration_marks: false,
        },
        export_options: ProjectExportOptions {
            lines_as: "shape".into(),
            trap_pt: 0.0,
            registration_marks: false,
            strip_background: true,
        },
    };
    let p_path = root.join("moved.trasea");
    save_project(&p_path, &proj).unwrap();

    // Case 1: source modified -> source_exists == true, hash_matches == false, strokes preserved
    RgbImage::from_pixel(40, 40, Rgb([50, 200, 50])).save(&img_path).unwrap();
    let modified = load_project(&p_path).unwrap();
    assert!(modified.source_exists);
    assert!(!modified.hash_matches);
    assert_eq!(modified.project.options.brush_strokes.len(), 1);

    // Case 2: source deleted/moved -> source_exists == false, hash_matches == false, strokes preserved
    std::fs::remove_file(&img_path).unwrap();
    let missing = load_project(&p_path).unwrap();
    assert!(!missing.source_exists);
    assert!(!missing.hash_matches);
    assert_eq!(missing.project.options.brush_strokes.len(), 1);
    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- N-01

#[test]
fn n01_offline_guarantee_no_network_crates_and_strict_csp() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let cargo_toml = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    for forbidden in ["reqwest", "hyper", "ureq", "curl", "surf", "tokio-tungstenite", "rustls", "native-tls", "openssl"] {
        assert!(!cargo_toml.contains(forbidden), "forbidden network crate {forbidden} found in trasea-core/Cargo.toml");
    }
    let tauri_conf = std::fs::read_to_string(root.join("../../src-tauri/tauri.conf.json")).unwrap();
    assert!(
        tauri_conf.contains("connect-src ipc: http://ipc.localhost"),
        "tauri.conf.json CSP must restrict connect-src to local IPC"
    );
    assert!(tauri_conf.contains("icons/icon.icns"), "macOS icon.icns must be listed in tauri.conf.json");
}

#[test]
fn thin_circle_outline_stays_continuous_without_diagonal_breaks() {
    // Regression test: a 4-px dark outline on a 640x480 illustration must not split at 45-deg diagonals.
    let (w, h) = (320u32, 320u32);
    let mut img = RgbImage::from_pixel(w, h, Rgb([243, 232, 210]));
    let (cx, cy, r) = (160.0f32, 160.0f32, 60.0f32);
    for y in 0..h {
        for x in 0..w {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            if d <= r - 2.0 {
                img.put_pixel(x, y, Rgb([235, 118, 71]));
            } else if d <= r + 2.0 {
                img.put_pixel(x, y, Rgb([24, 30, 42]));
            }
        }
    }
    let tm = trasea_core::lines::thin_mask(&img, &trasea_core::lines::ThinParams::for_size(w, h));
    assert_eq!(tm.comps.len(), 1, "4-px circular outline should remain a single connected component, got {}", tm.comps.len());
    let out = trasea_core::pipeline::vectorize(&img, &Params { measure: false, detect_text: false, ..Params::default() }, &|_, _| {}).unwrap();
    let total_strokes: usize = out.doc.strokes.iter().map(|s| s.strokes.len()).sum();
    let total_fallback: usize = out.doc.line_fallback.iter().map(|l| l.contours.len()).sum();
    assert!(
        total_strokes + total_fallback <= 2,
        "circular outline should not shatter into broken dashes (strokes={total_strokes}, fallback={total_fallback})"
    );
}

#[test]
fn shallow_curved_thin_line_bridges_knights_move_gaps() {
    // 1:2.5 shallow curve drawn 2 px wide per scanline (creates 1-px knight's-move breaks).
    let (w, h) = (260u32, 180u32);
    let mut img = RgbImage::from_pixel(w, h, image::Rgb([243, 232, 210]));
    for y in 30..150i32 {
        let t = (y - 30) as f32 / 120.0;
        let cx = (30.0 + t * 200.0 + (t * std::f32::consts::PI).sin() * 18.0).round() as i32;
        for dx in 0..2 {
            let x = cx + dx;
            if x >= 0 && x < w as i32 {
                img.put_pixel(x as u32, y as u32, image::Rgb([24, 30, 42]));
            }
        }
    }
    let out = trasea_core::pipeline::vectorize(
        &img,
        &Params { measure: false, detect_text: false, ..Params::default() },
        &|_, _| {},
    )
    .unwrap();
    let total_strokes: usize = out.doc.strokes.iter().map(|s| s.strokes.len()).sum();
    assert!(
        total_strokes >= 1 && total_strokes <= 2,
        "shallow curve should bridge knight's-move gaps into 1-2 continuous strokes (got {total_strokes})"
    );
    assert!(out.thin_pixels >= 180, "expected most curve pixels kept (got {})", out.thin_pixels);
}

#[test]
fn sample_illustration_keeps_trees_mountains_and_continuous_river() {
    let sample = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../src-tauri/resources/sample/sample-illustration.png");
    let img = image::open(&sample).expect("load sample").to_rgb8();
    let out = trasea_core::pipeline::vectorize(
        &img,
        &Params { colors: 10, smooth: 0.5, measure: true, detect_text: false, ..Params::default() },
        &|_, _| {},
    )
    .unwrap();
    // All key scene colours must remain distinct (sky, grass, dark tree green, 2 mountain blues, river blue, sun orange).
    assert!(out.palette.len() >= 7, "expected >=7 colours in sample palette, got {}", out.palette.len());
    let has_near = |target: [u8; 3], max_d: i32| {
        out.palette.iter().any(|c| {
            let d = (c[0] as i32 - target[0] as i32).abs()
                + (c[1] as i32 - target[1] as i32).abs()
                + (c[2] as i32 - target[2] as i32).abs();
            d <= max_d
        })
    };
    assert!(has_near([0x26, 0x56, 0x3E], 25), "pine tree dark green missing from palette: {:?}", out.palette);
    assert!(has_near([0x22, 0x3A, 0x5E], 25), "center mountain dark blue missing: {:?}", out.palette);
    assert!(has_near([0x3E, 0x5C, 0x8A], 25), "side mountain blue missing: {:?}", out.palette);
    assert!(has_near([0x6A, 0xAE, 0xCA], 25), "river light blue missing: {:?}", out.palette);
    let mae = out.stats.mae.expect("mae measured");
    assert!(mae < 1.8, "sample illustration MAE too high: {mae:.2}");
    let total_strokes: usize = out.doc.strokes.iter().map(|s| s.strokes.len()).sum();
    assert!(total_strokes <= 15, "river/mountain strokes fragmented into too many segments: {total_strokes}");
}