use image::{Rgb, RgbImage};
use trasea_core::{palette_io::{parse_hex_list, parse_swatch_bytes}, vectorize, Params};

#[test]
fn hex_list_preserves_order_deduplicates_and_caps() {
    assert_eq!(parse_hex_list("#aBc, 003344;\n #AABBCC"), vec![[170, 187, 204], [0, 51, 68]]);
    let long = (0..20).map(|n| format!("#{n:06x}")).collect::<Vec<_>>().join(" ");
    assert_eq!(parse_hex_list(&long).len(), 16);
    assert!(parse_hex_list("#12345 xyz").is_empty());
}

#[test]
fn imports_text_swatches_and_rejects_invalid_data() {
    let expected = vec![[0, 17, 34], [51, 68, 85]];
    for (name, data) in [
        ("sample.gpl", "GIMP Palette\nName: Test\n0 17 34 first\n51 68 85 second\n0 17 34 duplicate"),
        ("sample.json", r##"{"first":"#012","second":"#334455"}"##),
        ("sample.css", ":root { --one: #012; --two: #334455; }"),
        ("sample.txt", "#012 334455"),
    ] {
        assert_eq!(parse_swatch_bytes(name, data.as_bytes()).unwrap(), expected);
    }
    assert!(parse_swatch_bytes("bad.json", b"{ \"#112233 #445566\"").is_err());
    assert!(parse_swatch_bytes("bad.css", b"--one: #012; --two: #001122;").is_err());
    assert!(parse_swatch_bytes("bad.gpl", b"0 17 34\n51 68 85").is_err());
    assert!(parse_swatch_bytes("bad.txt", &[0xff]).is_err());
}

#[test]
fn imports_ase_rgb_cmyk_gray_and_rejects_truncation() {
    let mut bytes = b"ASEF\x00\x01\x00\x00\x00\x00\x00\x03".to_vec();
    for (model, values) in [
        (b"RGB ", &[1.0f32, 0.0, 0.0][..]),
        (b"CMYK", &[1.0, 0.0, 1.0, 0.0][..]),
        (b"GRAY", &[0.5][..]),
    ] {
        let mut block = vec![0, 1, 0, 0];
        block.extend(model);
        for value in values {
            block.extend(value.to_be_bytes());
        }
        block.extend([0, 0]);
        bytes.extend(1u16.to_be_bytes());
        bytes.extend((block.len() as u32).to_be_bytes());
        bytes.extend(block);
    }
    assert_eq!(parse_swatch_bytes("swatch.ase", &bytes).unwrap(), vec![[255,0,0], [0,255,0], [128;3]]);
    bytes.pop();
    assert!(parse_swatch_bytes("swatch.ase", &bytes).is_err());
}

#[test]
fn traced_fills_stay_inside_the_brand_palette() {
    let palette = vec![[200, 10, 20], [20, 190, 30], [30, 40, 180]];
    let input = RgbImage::from_fn(120, 60, |x, _| {
        Rgb([[220, 15, 25], [25, 200, 40], [30, 40, 210]][(x / 40) as usize])
    });
    let result = vectorize(&input, &Params { custom_palette: Some(palette.clone()), brand_palette_locked: true, ..Params::default() }, &|_, _| {}).unwrap();
    assert_eq!(result.palette, palette);
    let fills: Vec<_> = result.svg.split("fill=\"#").skip(1)
        .filter_map(|tail| tail.split_once('"').map(|(color, _)| color.to_ascii_uppercase()))
        .collect();
    assert!(!fills.is_empty());
    assert!(fills.iter().all(|fill| palette.iter().any(|color| trasea_core::hex(*color)[1..] == *fill)));
}
