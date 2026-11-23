//! Automatic tests T-01..T-09 (+ palette merge target) for Trasea 0.5.0 ("Produksi").
//! Synthetic images only; complements the 37 tests from 0.1.0..0.4.0.

use image::{Rgb, RgbImage};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use trasea_core::batch::{run_batch, scan_image_files, BatchConfig, BatchStatus};
use trasea_core::export::{to_separation_svgs, to_svg, Doc, ExportOptions, StrokeData, StrokeLayer, REG_MARGIN};
use trasea_core::production::{enforce_min_stroke, filter_micro_regions, ProductionOptions};
use trasea_core::trace::{Contour, Layer};
use trasea_core::{vectorize, Params};

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

fn synthetic_doc(colors: &[[u8; 3]]) -> Doc {
    let fields: Vec<Layer> = colors
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let (x0, y0, x1, y1) = if i == 0 {
                (0.0, 0.0, 200.0, 200.0)
            } else {
                let o = (i as f64) * 20.0;
                (o, o, o + 60.0, o + 60.0)
            };
            Layer {
                id: trasea_core::hex(c),
                color: c,
                contours: vec![rect_contour(x0, y0, x1, y1)],
                canvas: i == 0,
                prims: vec![],
            }
        })
        .collect();
    Doc {
        width: 100,
        height: 100,
        scale: 2.0,
        fields,
        line_shapes: vec![],
        line_fallback: vec![],
        strokes: vec![],
    }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("trasea-05-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn simple_logo(color: [u8; 3]) -> RgbImage {
    let mut img = RgbImage::from_pixel(200, 200, Rgb(WHITE));
    for y in 40..160 {
        for x in 40..160 {
            img.put_pixel(x, y, Rgb(color));
        }
    }
    img
}

// ---------------------------------------------------------------- T-01

#[test]
fn t01_filter_micro_regions_absorbs_small_spots_and_keeps_large_island() {
    let (w, h) = (40u32, 40u32);
    let mut labels = vec![0u8; (w * h) as usize];
    // Spot 1: 2x2 = 4 px (< 12)
    for y in 2..4 {
        for x in 2..4 {
            labels[(y * w + x) as usize] = 1;
        }
    }
    // Spot 2: 2x4 = 8 px (< 12)
    for y in 10..12 {
        for x in 10..14 {
            labels[(y * w + x) as usize] = 2;
        }
    }
    // Island 3: 5x10 = 50 px (>= 12)
    for y in 20..25 {
        for x in 20..30 {
            labels[(y * w + x) as usize] = 3;
        }
    }

    let removed = filter_micro_regions(&mut labels, w, h, 12);
    assert_eq!(removed, 2, "expected 2 micro-spots removed");
    assert_eq!(labels.iter().filter(|&&l| l == 1).count(), 0, "4 px spot should be absorbed");
    assert_eq!(labels.iter().filter(|&&l| l == 2).count(), 0, "8 px spot should be absorbed");
    assert_eq!(labels.iter().filter(|&&l| l == 3).count(), 50, "50 px island must stay intact");
}

// ---------------------------------------------------------------- T-02

#[test]
fn t02_enforce_min_stroke_raises_thin_strokes_only() {
    let scale = 2.0;
    let pt_to_px = |pt: f64| pt * 96.0 / 72.0 * scale;
    let mut doc = synthetic_doc(&[WHITE, [20, 20, 20]]);
    let seg = vec![[(10.0, 10.0), (20.0, 10.0), (30.0, 10.0), (40.0, 10.0)]];
    doc.strokes.push(StrokeLayer {
        color: [20, 20, 20],
        strokes: vec![
            StrokeData { curves: seg.clone(), width: pt_to_px(0.4) },
            StrokeData { curves: seg.clone(), width: pt_to_px(0.8) },
            StrokeData { curves: seg, width: pt_to_px(2.0) },
        ],
    });

    let count = enforce_min_stroke(&mut doc, 1.0);
    assert_eq!(count, 2, "two strokes below 1.0 pt should be thickened");
    let ws: Vec<f64> = doc.strokes[0].strokes.iter().map(|s| s.width).collect();
    assert!((ws[0] - pt_to_px(1.0)).abs() < 1e-6);
    assert!((ws[1] - pt_to_px(1.0)).abs() < 1e-6);
    assert!((ws[2] - pt_to_px(2.0)).abs() < 1e-6);
}

// ---------------------------------------------------------------- T-03

#[test]
fn t03_svg_registration_marks_adds_margin_and_four_targets() {
    let doc = synthetic_doc(&[WHITE, [15, 57, 115]]);
    let svg = to_svg(&doc, &ExportOptions { registration_marks: true, ..Default::default() });
    let expected_w = doc.width as f64 + 2.0 * REG_MARGIN;
    let expected_h = doc.height as f64 + 2.0 * REG_MARGIN;
    let vb = format!("viewBox=\"0 0 {} {}\"", expected_w as u32, expected_h as u32);
    assert!(svg.contains(&vb), "viewBox should grow by 2*margin ({vb}): {svg}");
    let reg_pos = svg.find("<g id=\"Registrasi\"").expect("Registrasi layer missing");
    let reg_slice = &svg[reg_pos..];
    let circles = reg_slice.matches("<circle ").count();
    assert_eq!(circles, 4, "must contain 4 registration target circles");
    // Verify coordinates lie outside the inner [28..128] drawing area
    assert!(reg_slice.contains("cx=\"14\" cy=\"14\""), "top-left mark missing");
    assert!(reg_slice.contains("cx=\"142\" cy=\"142\""), "bottom-right mark missing");
}

// ---------------------------------------------------------------- T-04

#[test]
fn t04_separation_svgs_are_100_percent_black_with_identical_viewbox() {
    let doc = synthetic_doc(&[WHITE, [15, 57, 115], [220, 60, 40], [40, 150, 80]]);
    let films_all = to_separation_svgs(&doc, &ExportOptions::default());
    assert_eq!(films_all.len(), 4, "4 colours -> 4 films");

    let films_nobg = to_separation_svgs(&doc, &ExportOptions { strip_background: true, ..Default::default() });
    assert_eq!(films_nobg.len(), 3, "strip_background=true -> 3 films");

    let vb = format!("viewBox=\"0 0 {} {}\"", (doc.width as f64 + 2.0 * REG_MARGIN) as u32, (doc.height as f64 + 2.0 * REG_MARGIN) as u32);
    for (i, f) in films_all.iter().enumerate() {
        assert!(f.svg.contains(&vb), "film {} viewBox mismatch", i + 1);
        assert!(f.svg.contains(&format!("Film {:02}/04", i + 1)), "film number label missing");
        assert!(f.svg.contains(&trasea_core::hex(f.color)), "hex label missing");
        assert!(f.svg.contains("<g id=\"Registrasi\""), "registration marks missing in film");
        // Every fill= and stroke= attribute value must be "#000000" or "none"
        for attr in ["fill=\"", "stroke=\""] {
            let mut rest = f.svg.as_str();
            while let Some(p) = rest.find(attr) {
                let after = &rest[p + attr.len()..];
                let val = &after[..after.find('"').unwrap()];
                assert!(val == "#000000" || val == "none", "film {} has non-black {attr}{val}\"", i + 1);
                rest = after;
            }
        }
    }
}

// ---------------------------------------------------------------- T-05

#[test]
fn t05_cut_embroidery_strips_background_no_open_strokes_and_zero_overlap() {
    // Synthetic 3-colour drawing on white background: two touching blocks + a concentric ring/circle
    let mut img = RgbImage::from_pixel(300, 240, Rgb(WHITE));
    let red = [210, 40, 40];
    let blue = [30, 80, 190];
    let green = [40, 160, 70];
    for y in 40..140 {
        for x in 30..130 {
            img.put_pixel(x, y, Rgb(red));
        }
        for x in 130..230 {
            img.put_pixel(x, y, Rgb(blue));
        }
    }
    // Concentric green disc inside the red block (tests hole vs inner shape knockout)
    for y in 65..115 {
        for x in 55..105 {
            img.put_pixel(x, y, Rgb(green));
        }
    }

    let p = Params {
        colors: 4,
        smooth: 0.7,
        keep_lines: true,
        production: ProductionOptions::cut_embroidery(),
        ..quiet()
    };
    let out = vectorize(&img, &p, &|_, _| {}).unwrap();
    assert!(!out.svg.contains("<g id=\"Latar\""), "background layer must be stripped in CutEmbroidery");
    assert!(!out.svg.contains("<path fill=\"none\" stroke="), "CutEmbroidery must not emit open strokes");
    assert!(out.doc.fields.len() >= 3, "expected 3 foreground layers, got {}", out.doc.fields.len());

    // Render each field layer separately onto transparent RGBA and verify 0 pixels are opaque in >1 layer.
    let (w, h) = (out.doc.width, out.doc.height);
    let mut opaque_owner = vec![0usize; (w * h) as usize];
    let mut overlap_px = 0usize;
    for (li, layer) in out.doc.fields.iter().enumerate() {
        let single_doc = Doc {
            width: w,
            height: h,
            scale: out.doc.scale,
            fields: vec![layer.clone()],
            line_shapes: vec![],
            line_fallback: vec![],
            strokes: vec![],
        };
        let layer_svg = to_svg(&single_doc, &ExportOptions { strip_background: true, ..Default::default() });
        let tree = resvg::usvg::Tree::from_str(&layer_svg, &resvg::usvg::Options::default()).unwrap();
        let mut pix = resvg::tiny_skia::Pixmap::new(w, h).unwrap();
        resvg::render(&tree, resvg::tiny_skia::Transform::identity(), &mut pix.as_mut());
        for (idx, px) in pix.pixels().iter().enumerate() {
            if px.alpha() == 255 {
                if opaque_owner[idx] != 0 {
                    overlap_px += 1;
                } else {
                    opaque_owner[idx] = li + 1;
                }
            }
        }
    }
    assert_eq!(overlap_px, 0, "knockout layers must have 0 opaque overlapping pixels, found {overlap_px}");
}

// ---------------------------------------------------------------- T-06

#[test]
fn t06_screen_print_caps_colors_activates_trap_and_enforces_min_stroke() {
    // 10 distinct coloured blocks + a 1 px dark thin line
    let mut img = RgbImage::from_pixel(400, 240, Rgb(WHITE));
    let cols: [[u8; 3]; 9] = [
        [220, 30, 30], [30, 180, 50], [30, 60, 210],
        [230, 190, 20], [180, 40, 190], [30, 190, 200],
        [230, 110, 20], [120, 70, 40], [110, 110, 110],
    ];
    for (i, &c) in cols.iter().enumerate() {
        let bx = (i % 3) as u32 * 120 + 20;
        let by = (i / 3) as u32 * 65 + 20;
        for y in by..(by + 45) {
            for x in bx..(bx + 90) {
                img.put_pixel(x, y, Rgb(c));
            }
        }
    }
    // Thin 1 px dark line on white strip at bottom
    for x in 30..370 {
        img.put_pixel(x, 225, Rgb([10, 10, 10]));
    }

    let mut prod = ProductionOptions::screen_print();
    prod.min_stroke_pt = 1.0;
    let p = Params {
        colors: 10,
        keep_lines: true,
        production: prod,
        ..quiet()
    };
    let out = vectorize(&img, &p, &|_, _| {}).unwrap();
    assert!(out.palette.len() <= 8, "ScreenPrint must cap spot colours to <= 8, got {}", out.palette.len());
    assert!(out.svg.contains("stroke-linejoin=\"round\""), "trap should be active on colour fields");
    assert!(out.svg.contains("<g id=\"Registrasi\""), "registration marks should be active");
    let min_px = 1.0 * 96.0 / 72.0 * out.doc.scale;
    for sl in &out.doc.strokes {
        for st in &sl.strokes {
            assert!(st.width + 1e-6 >= min_px, "stroke width {} < min_px {}", st.width, min_px);
        }
    }
}

// ---------------------------------------------------------------- T-07

#[test]
fn t07_batch_handles_valid_corrupt_and_ignored_files_and_writes_reports() {
    let root = temp_dir("t07");
    let in_dir = root.join("in");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();

    simple_logo([200, 40, 40]).save(in_dir.join("a.png")).unwrap();
    simple_logo([40, 160, 60]).save(in_dir.join("b.png")).unwrap();
    simple_logo([40, 80, 200]).save(in_dir.join("c.png")).unwrap();
    std::fs::write(in_dir.join("corrupt.png"), b"not a valid png file").unwrap();
    std::fs::write(in_dir.join("notes.txt"), b"ignore me").unwrap();

    let files = scan_image_files(&in_dir, false).unwrap();
    assert_eq!(files.len(), 4, "should pick 4 pngs and ignore notes.txt");

    let cfg = BatchConfig {
        inputs: files,
        output_dir: out_dir.clone(),
        params: quiet(),
        export: ExportOptions::default(),
        write_pdf: false,
        write_ase: false,
        write_separations: false,
        write_report_csv: true,
        write_report_md: true,
        max_workers: None,
    };
    let summary = run_batch(&cfg, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert_eq!(summary.total, 4);
    assert_eq!(summary.succeeded, 3);
    assert_eq!(summary.failed, 1);
    assert_eq!(summary.cancelled, 0);
    assert!(out_dir.join("a.svg").is_file());
    assert!(out_dir.join("b.svg").is_file());
    assert!(out_dir.join("c.svg").is_file());
    assert!(!out_dir.join("corrupt.svg").exists());

    let csv = std::fs::read_to_string(out_dir.join("laporan-trasea.csv")).unwrap();
    assert!(csv.starts_with("Nama file,Status,Warna,Path,Node,MAE,Peringatan,Waktu (dtk),Output,Error"));
    assert!(csv.contains("corrupt.png,gagal"));
    let md = std::fs::read_to_string(out_dir.join("laporan-trasea.md")).unwrap();
    assert!(md.contains("Sukses:** 3") && md.contains("Gagal:** 1") && md.contains("corrupt.png"));

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- T-08

#[test]
fn t08_batch_cancellation_stops_cleanly_after_first_file() {
    let root = temp_dir("t08");
    let in_dir = root.join("in");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&in_dir).unwrap();

    simple_logo([200, 40, 40]).save(in_dir.join("01.png")).unwrap();
    simple_logo([40, 160, 60]).save(in_dir.join("02.png")).unwrap();
    simple_logo([40, 80, 200]).save(in_dir.join("03.png")).unwrap();

    let files = scan_image_files(&in_dir, false).unwrap();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_cb = cancel.clone();

    let cfg = BatchConfig {
        inputs: files,
        output_dir: out_dir.clone(),
        params: quiet(),
        export: ExportOptions::default(),
        write_pdf: false,
        write_ase: false,
        write_separations: false,
        write_report_csv: true,
        write_report_md: true,
        max_workers: Some(1),
    };
    let summary = run_batch(&cfg, cancel, move |prog| {
        if let Some(ref rep) = prog.item_report {
            if rep.status == BatchStatus::Ok {
                cancel_cb.store(true, Ordering::Relaxed);
            }
        }
    })
    .unwrap();

    assert_eq!(summary.succeeded, 1, "first file should succeed");
    assert_eq!(summary.cancelled, 2, "remaining two files should be cancelled");
    assert_eq!(summary.failed, 0);
    assert!(out_dir.join("01.svg").is_file());
    assert!(!out_dir.join("02.svg").exists());
    assert!(!out_dir.join("03.svg").exists());
    assert!(out_dir.join("laporan-trasea.csv").is_file());
    assert!(out_dir.join("laporan-trasea.md").is_file());

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- T-09

#[test]
fn t09_batch_duplicate_filenames_across_subfolders_do_not_overwrite() {
    let root = temp_dir("t09");
    let sub_a = root.join("in").join("klien_a");
    let sub_b = root.join("in").join("klien_b");
    let out_dir = root.join("out");
    std::fs::create_dir_all(&sub_a).unwrap();
    std::fs::create_dir_all(&sub_b).unwrap();

    simple_logo([200, 40, 40]).save(sub_a.join("logo.png")).unwrap();
    simple_logo([40, 80, 200]).save(sub_b.join("logo.png")).unwrap();

    let files = scan_image_files(&root.join("in"), true).unwrap();
    assert_eq!(files.len(), 2);

    let cfg = BatchConfig {
        inputs: files,
        output_dir: out_dir.clone(),
        params: quiet(),
        export: ExportOptions::default(),
        write_pdf: false,
        write_ase: false,
        write_separations: false,
        write_report_csv: false,
        write_report_md: false,
        max_workers: None,
    };
    let summary = run_batch(&cfg, Arc::new(AtomicBool::new(false)), |_| {}).unwrap();
    assert_eq!(summary.succeeded, 2);
    assert!(out_dir.join("logo.svg").is_file(), "logo.svg missing");
    assert!(out_dir.join("logo_2.svg").is_file(), "logo_2.svg missing");

    let _ = std::fs::remove_dir_all(&root);
}

// ---------------------------------------------------------------- Polish 0.4.0: targeted palette merge

#[test]
fn polish_palette_merge_maps_pixels_to_chosen_non_nearest_target() {
    // 3 colours on white: orange [230, 120, 30], red [210, 40, 40] (nearest to orange), blue [30, 60, 210] (far from orange)
    let mut img = RgbImage::from_pixel(240, 120, Rgb(WHITE));
    let orange = [230, 120, 30];
    let red = [210, 40, 40];
    let blue = [30, 60, 210];
    for y in 20..100 {
        for x in 20..80 {
            img.put_pixel(x, y, Rgb(orange));
        }
        for x in 90..150 {
            img.put_pixel(x, y, Rgb(red));
        }
        for x in 160..220 {
            img.put_pixel(x, y, Rgb(blue));
        }
    }
    // Merge orange explicitly into blue (even though red is much closer to orange!)
    let p = Params {
        custom_palette: Some(vec![WHITE, red, blue]),
        palette_merges: vec![(orange, blue)],
        ..quiet()
    };
    let out = vectorize(&img, &p, &|_, _| {}).unwrap();
    let rendered = trasea_core::stats::render_svg(&out.svg, img.width(), img.height()).unwrap();
    // Centre of what used to be the orange block (x=50, y=60) must now be blue, not red!
    let px = rendered.get_pixel(50, 60).0;
    assert!(px[2] > 150 && px[0] < 80, "expected orange block merged into blue, got RGB {px:?}");
}