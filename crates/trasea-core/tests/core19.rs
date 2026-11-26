use trasea_core::stock_meta::{
    parse_ai_metadata_json, sanitize_metadata, to_adobe_stock_csv, validate_adobe_stock_rows,
    StockMetadata, ADOBE_STOCK_CATEGORIES,
};

fn sample() -> StockMetadata {
    StockMetadata {
        filename: "mountain.svg".into(),
        title: "Mountain and river illustration".into(),
        keywords: vec!["mountain".into(), "river".into()],
        category_id: 11,
        releases: String::new(),
    }
}

#[test]
fn normalizes_without_reordering_keywords() {
    let mut input = sample();
    input.title = format!("   {}\n Test", "é".repeat(201));
    input.keywords = [" Cloud ", "MOUNTAIN", "cloud", " ", "blue sky"]
        .into_iter().map(String::from).chain((0..55).map(|i| format!("item {i}"))).collect();
    input.category_id = 22;
    let normalized = sanitize_metadata(input);
    assert_eq!(normalized.title, "é".repeat(200));
    assert_eq!(&normalized.keywords[..3], &["cloud", "mountain", "blue sky"]);
    assert_eq!(normalized.keywords.len(), 49);
    assert_eq!(normalized.category_id, 8);
    assert_eq!(ADOBE_STOCK_CATEGORIES[7], "Graphic Resources");
}

#[test]
fn parses_plain_and_fenced_ai_json_and_keeps_local_filename() {
    let json = r#"{"title":" River  art ","keywords":["RIVER","river","ink"],"categoryId":11,"filename":"remote.svg"}"#;
    let plain = parse_ai_metadata_json(json, "local.svg").unwrap();
    let wrapped = format!("Metadata follows: {}json\n{json}\n{}.", "\x60".repeat(3), "\x60".repeat(3));
    assert_eq!(plain, parse_ai_metadata_json(&wrapped, "local.svg").unwrap());
    assert_eq!(plain.title, "River art");
    assert_eq!(plain.keywords, ["river", "ink"]);
    assert_eq!(plain.filename, "local.svg");
    assert_eq!(plain.category_id, 11);
    assert_eq!(parse_ai_metadata_json(&json.replace(":11", ":-20"), "local.svg").unwrap().category_id, 8);
    assert!(parse_ai_metadata_json(r#"{"title":"x","keywords":[]}"#, "local.svg").is_err());
    assert!(parse_ai_metadata_json("{ malformed }", "local.svg").is_err());
}

#[test]
fn writes_rfc4180_utf8_with_exact_headers() {
    let mut row = sample();
    row.filename = "montaña,01.svg".into();
    row.title = "Mountain \"art\", flat".into();
    row.releases = "file.jpg".into();
    assert_eq!(to_adobe_stock_csv(&[row]),
        "Filename,Title,Keywords,Category,Releases\r\n\"montaña,01.svg\",\"Mountain \"\"art\"\", flat\",\"mountain,river\",11,file.jpg\r\n");
}

#[test]
fn validates_adobe_csv_limits_without_changing_manual_draft() {
    let mut row = sample();
    assert!(validate_adobe_stock_rows(&[row.clone()]).is_ok());
    row.title = "é".repeat(71);
    assert_eq!(validate_adobe_stock_rows(&[row.clone()]), Err("metadata_csv_title"));
    row.title = "Valid title".into();
    row.filename = "C:/original.svg".into();
    assert_eq!(validate_adobe_stock_rows(&[row.clone()]), Err("metadata_csv_filename"));
    row.filename = "original.svg".into();
    row.keywords = vec!["=formula".into()];
    assert_eq!(validate_adobe_stock_rows(&[row]), Err("metadata_csv_keywords"));
}
