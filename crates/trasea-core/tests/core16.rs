//! Integration tests for Trasea v1.8.0 (Pre-Flight Audit, Auto-Fix, Smart Recolor, Brand Kit ZIP).

use trasea_core::audit::{audit_doc, auto_fix_doc};
use trasea_core::brand_kit::{
    build_brand_kit_zip, build_brand_kit_zip_from_svg, recolor_svg, RecolorMode,
};
use trasea_core::export::{Doc, ExportOptions, StrokeData, StrokeLayer};
use trasea_core::trace::{Contour, Layer};

fn rect_contour(x: f64, y: f64, w: f64, h: f64) -> Contour {
    let p0 = (x, y);
    let p1 = (x + w, y);
    let p2 = (x + w, y + h);
    let p3 = (x, y + h);
    Contour {
        start: p0,
        curves: vec![
            [p0, p1, p1],
            [p1, p2, p2],
            [p2, p3, p3],
            [p3, p0, p0],
        ],
        prim: None,
    }
}

fn make_problematic_doc() -> Doc {
    Doc {
        width: 200,
        height: 200,
        scale: 1.0,
        fields: vec![
            Layer {
                id: "#FFFFFF".into(),
                color: [255, 255, 255],
                contours: vec![rect_contour(0.0, 0.0, 200.0, 200.0)],
                canvas: true,
                prims: Vec::new(),
            },
            // Two near-identical blues (ΔE ~ 3-4)
            Layer {
                id: "#1E40AF".into(),
                color: [0x1E, 0x40, 0xAF],
                contours: vec![rect_contour(20.0, 20.0, 60.0, 60.0)],
                canvas: false,
                prims: Vec::new(),
            },
            Layer {
                id: "#1F42B3".into(),
                color: [0x1F, 0x42, 0xB3],
                contours: vec![
                    rect_contour(100.0, 20.0, 50.0, 50.0),
                    // Micro-path dust: 1.2 x 1.2 = 1.44 px² (< 4.0 px²)
                    rect_contour(180.0, 180.0, 1.2, 1.2),
                ],
                canvas: false,
                prims: Vec::new(),
            },
        ],
        line_shapes: Vec::new(),
        line_fallback: Vec::new(),
        strokes: vec![StrokeLayer {
            color: [0x10, 0x10, 0x10],
            strokes: vec![StrokeData {
                curves: vec![[(10.0, 100.0), (50.0, 100.0), (90.0, 100.0), (140.0, 100.0)]],
                // 0.2 pt -> ~0.26 px
                width: 0.25,
            }],
        }],
    }
}

/// Minimal ZIP central-directory reader to verify our generated ZIP bytes cleanly.
fn read_zip_entries(zip: &[u8]) -> Vec<(String, usize)> {
    assert!(zip.len() >= 22, "zip too short");
    let eocd_pos = zip
        .windows(4)
        .rposition(|w| w == [0x50, 0x4B, 0x05, 0x06])
        .expect("missing EOCD signature");
    let count = u16::from_le_bytes([zip[eocd_pos + 10], zip[eocd_pos + 11]]) as usize;
    let cd_offset = u32::from_le_bytes([
        zip[eocd_pos + 16],
        zip[eocd_pos + 17],
        zip[eocd_pos + 18],
        zip[eocd_pos + 19],
    ]) as usize;

    let mut out = Vec::new();
    let mut pos = cd_offset;
    for _ in 0..count {
        assert_eq!(&zip[pos..pos + 4], &[0x50, 0x4B, 0x01, 0x02], "bad CD signature");
        let size = u32::from_le_bytes([zip[pos + 24], zip[pos + 25], zip[pos + 26], zip[pos + 27]]) as usize;
        let name_len = u16::from_le_bytes([zip[pos + 28], zip[pos + 29]]) as usize;
        let extra_len = u16::from_le_bytes([zip[pos + 30], zip[pos + 31]]) as usize;
        let comment_len = u16::from_le_bytes([zip[pos + 32], zip[pos + 33]]) as usize;
        let name = std::str::from_utf8(&zip[pos + 46..pos + 46 + name_len])
            .expect("utf8 filename")
            .to_string();
        out.push((name, size));
        pos += 46 + name_len + extra_len + comment_len;
    }
    out
}

#[test]
fn t01_audit_detects_thin_stroke_similar_colors_and_micro_paths() {
    let doc = make_problematic_doc();
    let report = audit_doc(&doc);
    assert!(!report.ok, "problematic doc must fail audit");
    let codes: Vec<&str> = report.issues.iter().map(|i| i.code.as_str()).collect();
    assert!(codes.contains(&"thin_stroke"), "expected thin_stroke, got {codes:?}");
    assert!(codes.contains(&"similar_colors"), "expected similar_colors, got {codes:?}");
    assert!(codes.contains(&"micro_paths"), "expected micro_paths, got {codes:?}");
    for issue in &report.issues {
        assert!(issue.count > 0);
    }
}

#[test]
fn t02_auto_fix_resolves_audit_issues() {
    let mut doc = make_problematic_doc();
    let before = audit_doc(&doc);
    assert!(!before.ok);

    let fixes = auto_fix_doc(&mut doc);
    assert!(fixes >= 3, "expected at least 3 fixes applied, got {fixes}");

    let after = audit_doc(&doc);
    assert!(after.issues.len() < before.issues.len(), "issues must decrease");
    assert!(after.ok, "all issues should be resolved cleanly, left: {:?}", after.issues);
    assert!(
        (doc.strokes[0].strokes[0].width / doc.scale) * 0.75 >= 0.5 - 1e-6,
        "thin stroke must be raised to >= 0.5 pt"
    );
}

#[test]
fn t03_recolor_svg_preserves_cutcontour_and_registrasi() {
    let sample = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120">
  <g id="Bidang warna">
    <path d="M10,10 H60 V60 H10 Z" fill="#0095D9"/>
    <path d="M60,60 H110 V110 H60 Z" fill="#F2B705"/>
  </g>
  <g id="CutContour">
    <path d="M5,5 H115 V115 H5 Z" fill="none" stroke="#EC008C" stroke-width="0.25pt"/>
  </g>
  <g id="Registrasi">
    <circle cx="10" cy="10" r="3" stroke="#111111" fill="none"/>
  </g>
</svg>"##;

    let black = recolor_svg(sample, RecolorMode::MonoBlack, None, None);
    assert!(black.contains("fill=\"#000000\""), "main shapes must turn black");
    assert!(!black.contains("#0095D9"), "original blue should be replaced");
    assert!(black.contains("stroke=\"#EC008C\""), "CutContour magenta must stay intact");
    assert!(black.contains("stroke=\"#111111\""), "Registrasi must stay intact");

    let white = recolor_svg(sample, RecolorMode::MonoWhite, None, None);
    assert!(white.contains("fill=\"#FFFFFF\""), "main shapes must turn white");
    assert!(white.contains("stroke=\"#EC008C\""), "CutContour magenta must stay intact");

    let duo = recolor_svg(sample, RecolorMode::Duotone, Some("#1E293B"), Some("#38BDF8"));
    assert!(duo.contains("#1E293B") || duo.contains("#38BDF8"), "duotone colors must be applied");
    assert!(duo.contains("stroke=\"#EC008C\""), "CutContour magenta must stay intact in duotone");
}

#[test]
fn t04_build_brand_kit_zip_doc_and_svg() {
    let mut doc = make_problematic_doc();
    auto_fix_doc(&mut doc);
    let opts = ExportOptions::default();

    let zip_bytes = build_brand_kit_zip(&doc, "Trasea Logo", &opts).expect("brand kit zip");
    let entries = read_zip_entries(&zip_bytes);
    assert_eq!(entries.len(), 5, "Doc Brand Kit ZIP must contain 5 entries: {entries:?}");
    for (name, size) in &entries {
        assert!(*size > 100, "entry {name} must be > 100 bytes, got {size}");
    }
    let names: Vec<&str> = entries.iter().map(|e| e.0.as_str()).collect();
    assert!(names.contains(&"Trasea-Logo-color.svg"));
    assert!(names.contains(&"Trasea-Logo-black.svg"));
    assert!(names.contains(&"Trasea-Logo-white.svg"));
    assert!(names.contains(&"Trasea-Logo-cutcontour.pdf"));
    assert!(names.contains(&"Trasea-Logo-palette.ase"));

    let ai_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120"><rect x="10" y="10" width="100" height="100" fill="#0095D9"/><circle cx="60" cy="60" r="30" fill="#E6007E"/></svg>"##;
    let ai_zip = build_brand_kit_zip_from_svg(ai_svg, "ai-crest").expect("ai brand kit zip");
    let ai_entries = read_zip_entries(&ai_zip);
    assert_eq!(ai_entries.len(), 4, "AI SVG Brand Kit ZIP must contain 4 entries: {ai_entries:?}");
    for (name, size) in &ai_entries {
        assert!(*size > 100, "AI entry {name} must be > 100 bytes, got {size}");
    }
}
