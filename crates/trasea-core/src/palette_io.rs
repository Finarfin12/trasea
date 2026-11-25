//! Imports brand colors without performing file or network I/O.
use crate::{Error, Result, Rgb};

fn invalid(message: &str) -> Error {
    Error::Other(message.into())
}

fn parse_hex(token: &str) -> Option<Rgb> {
    let value = token.strip_prefix('#').unwrap_or(token);
    if !value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match value.len() {
        3 if token.starts_with('#') => {
            let n = u16::from_str_radix(value, 16).ok()?;
            Some([(n >> 8) as u8 * 17, ((n >> 4) & 15) as u8 * 17, (n & 15) as u8 * 17])
        }
        6 => {
            let n = u32::from_str_radix(value, 16).ok()?;
            Some([(n >> 16) as u8, (n >> 8) as u8, n as u8])
        }
        _ => None,
    }
}

fn collect_colors(colors: impl Iterator<Item = Rgb>) -> Vec<Rgb> {
    let mut unique = Vec::new();
    for color in colors {
        if !unique.contains(&color) {
            unique.push(color);
            if unique.len() == 16 {
                break;
            }
        }
    }
    unique
}

pub fn parse_hex_list(input: &str) -> Vec<Rgb> {
    collect_colors(input.split(|c: char| c.is_whitespace() || c == ',' || c == ';').filter_map(parse_hex))
}

fn embedded_hex(text: &str) -> impl Iterator<Item = Rgb> + '_ {
    text.split_inclusive(|c: char| !(c.is_ascii_alphanumeric() || c == '#' || c == '_' || c == '-'))
        .filter_map(|word| parse_hex(word.trim_end_matches(|c: char| !c.is_ascii_alphanumeric())).filter(|_| word.starts_with('#')))
}

fn ase_color(block: &[u8]) -> Option<Option<Rgb>> {
    let name_units = u16::from_be_bytes(block.get(..2)?.try_into().ok()?) as usize;
    let data = block.get(2 + name_units * 2..)?;
    let model = data.get(..4)?;
    let count = match model {
        b"RGB " => 3,
        b"CMYK" => 4,
        b"GRAY" => 1,
        _ => return Some(None),
    };
    let mut channels = [0.0; 4];
    for (slot, raw) in channels.iter_mut().zip(data.get(4..4 + count * 4)?.chunks_exact(4)) {
        *slot = f32::from_be_bytes(raw.try_into().ok()?);
        if !slot.is_finite() || !(0.0..=1.0).contains(slot) {
            return None;
        }
    }
    data.get(4 + count * 4..6 + count * 4)?;
    let rgb = match model {
        b"CMYK" => [channels[0], channels[1], channels[2]].map(|c| (1.0 - c) * (1.0 - channels[3])),
        b"GRAY" => [channels[0]; 3],
        _ => [channels[0], channels[1], channels[2]],
    };
    Some(Some(rgb.map(|c| (c * 255.0).round() as u8)))
}

fn ase_colors(bytes: &[u8]) -> Option<Vec<Rgb>> {
    if !bytes.starts_with(b"ASEF") {
        return None;
    }
    let count = u32::from_be_bytes(bytes.get(8..12)?.try_into().ok()?);
    let mut remaining = bytes.get(12..)?;
    let mut colors = Vec::new();
    for _ in 0..count {
        let kind = u16::from_be_bytes(remaining.get(..2)?.try_into().ok()?);
        let len = u32::from_be_bytes(remaining.get(2..6)?.try_into().ok()?) as usize;
        remaining = remaining.get(6..)?;
        let block = remaining.get(..len)?;
        remaining = remaining.get(len..)?;
        if kind == 1 {
            colors.extend(ase_color(block)?);
        }
    }
    Some(colors)
}

pub fn parse_swatch_bytes(filename: &str, bytes: &[u8]) -> Result<Vec<Rgb>> {
    let ext = filename.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    let colors = if ext == "ase" || bytes.starts_with(b"ASEF") {
        collect_colors(ase_colors(bytes).ok_or_else(|| invalid("Invalid ASE swatch."))?.into_iter())
    } else {
        let text = std::str::from_utf8(bytes).map_err(|_| invalid("Swatch text must be UTF-8."))?;
        let text = text.trim_start_matches('\u{feff}');
        if ext == "gpl" || text.starts_with("GIMP Palette") {
            let mut lines = text.lines();
            if lines.next().map(str::trim) != Some("GIMP Palette") {
                return Err(invalid("Invalid GIMP palette header."));
            }
            collect_colors(lines.filter_map(|line| {
                let mut cells = line.split_whitespace();
                Some([cells.next()?.parse().ok()?, cells.next()?.parse().ok()?, cells.next()?.parse().ok()?])
            }))
        } else {
            match ext.as_str() {
                "json" => {
                    serde_json::from_str::<serde_json::Value>(text).map_err(|_| invalid("Invalid JSON swatch."))?;
                    collect_colors(embedded_hex(text))
                }
                "css" => collect_colors(embedded_hex(text)),
                "txt" => parse_hex_list(text),
                _ => return Err(invalid("Unsupported swatch format.")),
            }
        }
    };
    if colors.len() < 2 {
        return Err(invalid("A brand palette needs at least two distinct colors."));
    }
    Ok(colors)
}
