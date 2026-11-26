//! Validation and CSV formatting for stock metadata. No provider or file I/O.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const ADOBE_STOCK_CATEGORIES: [&str; 21] = [
    "Animals", "Buildings and Architecture", "Business", "Drinks", "The Environment",
    "States of Mind", "Food", "Graphic Resources", "Hobbies and Leisure", "Industry",
    "Landscape", "Lifestyle", "People", "Plants and Flowers", "Culture and Religion",
    "Science", "Social Issues", "Sports", "Technology", "Transport", "Travel",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StockMetadata {
    pub filename: String,
    pub title: String,
    pub keywords: Vec<String>,
    pub category_id: u8,
    pub releases: String,
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn sanitize_metadata(mut raw: StockMetadata) -> StockMetadata {
    raw.filename = raw.filename.trim().to_string();
    raw.title = single_line(&raw.title).chars().take(200).collect::<String>().trim_end().into();
    let mut seen = HashSet::new();
    raw.keywords = raw.keywords.iter()
        .map(|keyword| single_line(keyword).to_lowercase())
        .filter(|keyword| !keyword.is_empty() && seen.insert(keyword.clone()))
        .take(49)
        .collect();
    raw.category_id = if (1..=ADOBE_STOCK_CATEGORIES.len() as u8).contains(&raw.category_id) {
        raw.category_id
    } else {
        8
    };
    raw.releases = raw.releases.trim().to_string();
    raw
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AiMetadata {
    title: String,
    keywords: Vec<String>,
    #[serde(default)]
    category_id: serde_json::Value,
}

/// Accept plain or fenced JSON, including surrounding provider text.
pub fn parse_ai_metadata_json(raw_text: &str, default_filename: &str) -> Result<StockMetadata, String> {
    if raw_text.len() > 65_536 {
        return Err("metadata_response_invalid".into());
    }
    for (start, _) in raw_text.match_indices('{').take(64) {
        let mut objects = serde_json::Deserializer::from_str(&raw_text[start..]).into_iter::<AiMetadata>();
        let Some(Ok(candidate)) = objects.next() else { continue };
        let row = sanitize_metadata(StockMetadata {
            filename: default_filename.into(),
            title: candidate.title,
            keywords: candidate.keywords,
            category_id: candidate.category_id.as_u64().and_then(|id| u8::try_from(id).ok()).unwrap_or(8),
            releases: String::new(),
        });
        if !row.title.is_empty() && !row.keywords.is_empty() {
            return Ok(row);
        }
    }
    Err("metadata_response_invalid".into())
}

fn csv_field(text: &str) -> String {
    if text.contains([',', '"', '\r', '\n']) {
        format!("\"{}\"", text.replace('"', "\"\""))
    } else {
        text.into()
    }
}

pub fn to_adobe_stock_csv(rows: &[StockMetadata]) -> String {
    let mut csv = String::from("Filename,Title,Keywords,Category,Releases\r\n");
    for row in rows {
        let row = sanitize_metadata(row.clone());
        let fields = [
            csv_field(&row.filename),
            csv_field(&row.title),
            csv_field(&row.keywords.join(",")),
            row.category_id.to_string(),
            csv_field(&row.releases),
        ];
        csv.push_str(&fields.join(","));
        csv.push_str("\r\n");
    }
    csv
}

/// Check contributor CSV limits separately from the draft editor's limits.
pub fn validate_adobe_stock_rows(rows: &[StockMetadata]) -> Result<(), &'static str> {
    if rows.is_empty() || rows.len() > 5_000 {
        return Err("metadata_csv_rows");
    }
    let plain_cell = |text: &str| {
        !text.chars().any(char::is_control) && !text.trim_start().starts_with(['=', '+', '-', '@'])
    };
    for row in rows {
        let row = sanitize_metadata(row.clone());
        if row.filename.chars().count() > 30 || !plain_cell(&row.filename)
            || row.filename.contains(['/', '\\'])
            || !row.filename.rsplit_once('.').is_some_and(|(stem, ext)| !stem.is_empty() && !ext.is_empty()) {
            return Err("metadata_csv_filename");
        }
        if row.title.is_empty() || row.title.chars().count() > 70 || !plain_cell(&row.title)
            || row.title.contains([',', '"']) {
            return Err("metadata_csv_title");
        }
        if row.keywords.is_empty() || row.keywords.iter().any(|word| !plain_cell(word) || word.contains(',')) {
            return Err("metadata_csv_keywords");
        }
        if row.releases.chars().count() > 30 || !plain_cell(&row.releases) {
            return Err("metadata_csv_releases");
        }
    }
    Ok(())
}
