//! Smart Recolor (0 ms non-destructive SVG recoloring) & 1-Click Brand Kit ZIP export (Trasea v1.8.0).

use crate::export::{palette_of, svg_to_pdf, to_ase_with_spots, to_pdf, to_svg, Doc, ExportOptions};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RecolorMode {
    #[default]
    Original,
    MonoBlack,
    MonoWhite,
    Duotone,
}

fn parse_hex6(s: &str) -> Option<[u8; 3]> {
    let h = s.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&h[0..2], 16).ok()?;
    let g = u8::from_str_radix(&h[2..4], 16).ok()?;
    let b = u8::from_str_radix(&h[4..6], 16).ok()?;
    Some([r, g, b])
}

fn normalize_hex(hex_opt: Option<&str>, fallback: &str) -> String {
    if let Some(s) = hex_opt {
        let trimmed = s.trim();
        if parse_hex6(trimmed).is_some() {
            return trimmed.to_ascii_uppercase();
        }
    }
    fallback.to_string()
}

fn is_cutcontour_magenta(rgb: [u8; 3]) -> bool {
    rgb == [0xEC, 0x00, 0x8C]
}

fn map_hex_color(
    hex_str: &str,
    mode: RecolorMode,
    primary: &str,
    secondary: &str,
) -> Option<String> {
    let rgb = parse_hex6(hex_str)?;
    if is_cutcontour_magenta(rgb) {
        return None;
    }
    match mode {
        RecolorMode::Original => None,
        RecolorMode::MonoBlack => Some("#000000".to_string()),
        RecolorMode::MonoWhite => Some("#FFFFFF".to_string()),
        RecolorMode::Duotone => {
            let lum = (0.2126 * rgb[0] as f32 + 0.7152 * rgb[1] as f32 + 0.0722 * rgb[2] as f32)
                / 255.0;
            if lum < 0.52 {
                Some(primary.to_string())
            } else {
                Some(secondary.to_string())
            }
        }
    }
}

/// Find protected `<g ... id="CutContour"...>...</g>` and `<g ... id="Registrasi"...>...</g>`
/// byte spans so Smart Recolor never alters production cut-lines or registration marks.
fn protected_group_ranges(svg: &str) -> Vec<(usize, usize)> {
    let mut ranges = Vec::new();
    let bytes = svg.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if let Some(rel) = svg[i..].find("<g") {
            let start = i + rel;
            let after_g = start + 2;
            if after_g < bytes.len()
                && !matches!(bytes[after_g], b' ' | b'\t' | b'\n' | b'\r' | b'>')
            {
                i = after_g;
                continue;
            }
            let Some(tag_end_rel) = svg[start..].find('>') else {
                break;
            };
            let tag_end = start + tag_end_rel;
            let open_tag = &svg[start..=tag_end];
            let is_protected = open_tag.contains("id=\"CutContour\"")
                || open_tag.contains("id='CutContour'")
                || open_tag.contains("id=\"Registrasi\"")
                || open_tag.contains("id='Registrasi'")
                || open_tag.contains("data-trasea-layer=\"CutContour\"")
                || open_tag.contains("data-trasea-layer=\"Registrasi\"");
            if is_protected {
                // Match nested <g ...> ... </g>
                let mut depth = 1usize;
                let mut pos = tag_end + 1;
                while pos < bytes.len() && depth > 0 {
                    let next_open = svg[pos..].find("<g");
                    let next_close = svg[pos..].find("</g>");
                    match (next_open, next_close) {
                        (Some(o), Some(c)) if o < c => {
                            let abs_o = pos + o;
                            let ch = bytes.get(abs_o + 2).copied().unwrap_or(b' ');
                            if matches!(ch, b' ' | b'\t' | b'\n' | b'\r' | b'>') {
                                depth += 1;
                            }
                            pos = abs_o + 2;
                        }
                        (_, Some(c)) => {
                            depth -= 1;
                            pos = pos + c + 4;
                        }
                        _ => {
                            pos = bytes.len();
                            break;
                        }
                    }
                }
                ranges.push((start, pos));
                i = pos;
                continue;
            }
            i = tag_end + 1;
        } else {
            break;
        }
    }
    ranges
}

fn recolor_chunk(chunk: &str, mode: RecolorMode, primary: &str, secondary: &str) -> String {
    let mut out = String::with_capacity(chunk.len());
    let bytes = chunk.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        // Match fill="#RRGGBB", stroke="#RRGGBB", fill:#RRGGBB, stroke:#RRGGBB
        if bytes[i] == b'#' && i + 7 <= bytes.len() {
            let cand = &chunk[i..i + 7];
            let prev = if i > 0 { bytes[i - 1] } else { b' ' };
            let next = bytes.get(i + 7).copied().unwrap_or(b' ');
            let valid_boundary = matches!(prev, b'"' | b'\'' | b':' | b' ')
                && !next.is_ascii_hexdigit();
            if valid_boundary {
                if let Some(mapped) = map_hex_color(cand, mode, primary, secondary) {
                    out.push_str(&mapped);
                    i += 7;
                    continue;
                }
            }
        }
        let ch = chunk[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Non-destructively recolor an SVG string while keeping `CutContour` (`#EC008C`) and `Registrasi` intact.
pub fn recolor_svg(
    svg: &str,
    mode: RecolorMode,
    primary: Option<&str>,
    secondary: Option<&str>,
) -> String {
    if mode == RecolorMode::Original {
        return svg.to_string();
    }
    let pri = normalize_hex(primary, "#111827");
    let sec = normalize_hex(secondary, "#6D8CFF");

    let protected = protected_group_ranges(svg);
    if protected.is_empty() {
        return recolor_chunk(svg, mode, &pri, &sec);
    }

    let mut out = String::with_capacity(svg.len());
    let mut cursor = 0usize;
    for (s, e) in protected {
        if s > cursor {
            out.push_str(&recolor_chunk(&svg[cursor..s], mode, &pri, &sec));
        }
        out.push_str(&svg[s..e]);
        cursor = e;
    }
    if cursor < svg.len() {
        out.push_str(&recolor_chunk(&svg[cursor..], mode, &pri, &sec));
    }
    out
}

fn sanitize_base_name(base_name: &str) -> String {
    let cleaned: String = base_name
        .trim()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = cleaned.trim_matches('-');
    if trimmed.is_empty() {
        "trasea-brand".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Standard IEEE 802.3 CRC-32 for ZIP entries.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320u32 & mask);
        }
    }
    !crc
}

/// Build a deterministic, spec-compliant ZIP archive (Stored method 0, zero external dependencies).
fn pack_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    let mut central = Vec::new();

    for &(name, data) in entries {
        let offset = buf.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len() as u16;

        // Local file header (30 bytes + name + data)
        buf.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        buf.extend_from_slice(&20u16.to_le_bytes()); // version needed: 2.0
        buf.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 filename flag
        buf.extend_from_slice(&0u16.to_le_bytes()); // compression: 0 (Stored)
        buf.extend_from_slice(&0u16.to_le_bytes()); // mod time
        buf.extend_from_slice(&0x5421u16.to_le_bytes()); // mod date
        buf.extend_from_slice(&crc.to_le_bytes());
        buf.extend_from_slice(&size.to_le_bytes());
        buf.extend_from_slice(&size.to_le_bytes());
        buf.extend_from_slice(&name_len.to_le_bytes());
        buf.extend_from_slice(&0u16.to_le_bytes()); // extra len
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(data);

        // Central directory header (46 bytes + name)
        central.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        central.extend_from_slice(&20u16.to_le_bytes()); // version made by
        central.extend_from_slice(&20u16.to_le_bytes()); // version needed
        central.extend_from_slice(&0x0800u16.to_le_bytes()); // UTF-8 flag
        central.extend_from_slice(&0u16.to_le_bytes()); // Stored
        central.extend_from_slice(&0u16.to_le_bytes()); // mod time
        central.extend_from_slice(&0x5421u16.to_le_bytes()); // mod date
        central.extend_from_slice(&crc.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&size.to_le_bytes());
        central.extend_from_slice(&name_len.to_le_bytes());
        central.extend_from_slice(&0u16.to_le_bytes()); // extra len
        central.extend_from_slice(&0u16.to_le_bytes()); // comment len
        central.extend_from_slice(&0u16.to_le_bytes()); // disk start
        central.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        central.extend_from_slice(&0u32.to_le_bytes()); // external attrs
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name_bytes);
    }

    let cd_offset = buf.len() as u32;
    let cd_size = central.len() as u32;
    let count = entries.len() as u16;
    buf.extend_from_slice(&central);

    // End of central directory record (22 bytes)
    buf.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // disk number
    buf.extend_from_slice(&0u16.to_le_bytes()); // start disk
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    buf.extend_from_slice(&cd_size.to_le_bytes());
    buf.extend_from_slice(&cd_offset.to_le_bytes());
    buf.extend_from_slice(&0u16.to_le_bytes()); // comment len

    buf
}

/// Build a 5-file production Brand Kit ZIP from a traced `Doc`:
/// 1. `{base}-color.svg`
/// 2. `{base}-black.svg`
/// 3. `{base}-white.svg`
/// 4. `{base}-cutcontour.pdf`
/// 5. `{base}-palette.ase`
pub fn build_brand_kit_zip(doc: &Doc, base_name: &str, opts: &ExportOptions) -> Result<Vec<u8>> {
    let base = sanitize_base_name(base_name);
    let color_svg = to_svg(doc, opts);
    let black_svg = recolor_svg(&color_svg, RecolorMode::MonoBlack, None, None);
    let white_svg = recolor_svg(&color_svg, RecolorMode::MonoWhite, None, None);

    let mut cut_opts = opts.clone();
    cut_opts.die_cut_contour = true;
    let cut_pdf = to_pdf(doc, &cut_opts)?;

    let pal = palette_of(doc);
    let ase = to_ase_with_spots(
        &pal,
        opts.color_space,
        &opts.spot_colors,
        Some(cut_opts.die_cut_spot_name.as_str()),
    );

    let n1 = format!("{base}-color.svg");
    let n2 = format!("{base}-black.svg");
    let n3 = format!("{base}-white.svg");
    let n4 = format!("{base}-cutcontour.pdf");
    let n5 = format!("{base}-palette.ase");

    Ok(pack_zip(&[
        (&n1, color_svg.as_bytes()),
        (&n2, black_svg.as_bytes()),
        (&n3, white_svg.as_bytes()),
        (&n4, &cut_pdf),
        (&n5, &ase),
    ]))
}

/// Build a 4-file Brand Kit ZIP directly from an imported/AI SVG string:
/// 1. `{base}-color.svg`
/// 2. `{base}-black.svg`
/// 3. `{base}-white.svg`
/// 4. `{base}-color.pdf`
pub fn build_brand_kit_zip_from_svg(svg: &str, base_name: &str) -> Result<Vec<u8>> {
    if svg.trim().is_empty() {
        return Err(Error::Other("SVG kosong, tidak bisa membuat Brand Kit ZIP".into()));
    }
    let base = sanitize_base_name(base_name);
    let black_svg = recolor_svg(svg, RecolorMode::MonoBlack, None, None);
    let white_svg = recolor_svg(svg, RecolorMode::MonoWhite, None, None);
    let color_pdf = svg_to_pdf(svg)?;

    let n1 = format!("{base}-color.svg");
    let n2 = format!("{base}-black.svg");
    let n3 = format!("{base}-white.svg");
    let n4 = format!("{base}-color.pdf");

    Ok(pack_zip(&[
        (&n1, svg.as_bytes()),
        (&n2, black_svg.as_bytes()),
        (&n3, white_svg.as_bytes()),
        (&n4, &color_pdf),
    ]))
}
