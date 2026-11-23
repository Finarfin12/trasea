//! Colour space definitions, deterministic RGB -> CMYK conversion, offline CMYK soft-proof,
//! out-of-gamut detection, and Named Spot Color helpers for print RIP export (PDF, EPS, ASE).

use crate::palette::rgb_to_lab;
use crate::{hex, Rgb};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ColorSpace {
    #[default]
    Rgb,
    Cmyk,
}

impl ColorSpace {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "rgb" => Some(Self::Rgb),
            "cmyk" => Some(Self::Cmyk),
            _ => None,
        }
    }
}

/// Mapping from a palette hex colour (e.g. `"#EC008C"`) to a named RIP spot ink
/// (e.g. `"CutContour"`, `"White_Underbase"`, `"PANTONE 185 C"`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpotColorMapping {
    /// Source colour in uppercase `#RRGGBB`.
    pub hex: String,
    /// Spot ink name (ASCII-sanitized when written to PDF/EPS/ASE).
    pub spot_name: String,
}

/// Parse `#RRGGBB` or `RRGGBB` into `[r, g, b]`.
pub fn parse_hex_rgb(s: &str) -> Option<Rgb> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 {
        return None;
    }
    let v = u32::from_str_radix(h, 16).ok()?;
    Some([(v >> 16) as u8, (v >> 8) as u8, v as u8])
}

/// Sanitize a spot ink name to safe printable ASCII (no PDF/PostScript delimiters or control chars).
/// Falls back to `"SpotColor"` if empty after sanitization.
pub fn sanitize_spot_name(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | '+') {
            out.push(ch);
            prev_space = false;
        } else if ch == ' ' && !prev_space && !out.is_empty() {
            out.push(' ');
            prev_space = true;
        }
    }
    let trimmed = out.trim().to_string();
    if trimmed.is_empty() {
        "SpotColor".to_string()
    } else {
        trimmed
    }
}

/// Encode a spot ink name as a valid PDF Name Object token (without leading `/`).
/// Spaces and non-regular bytes are `#XX`-escaped per PDF 1.4 §3.2.4.
pub fn pdf_name_encode(raw: &str) -> String {
    let clean = sanitize_spot_name(raw);
    let mut out = String::with_capacity(clean.len() + 8);
    for b in clean.bytes() {
        let regular = (0x21..=0x7E).contains(&b)
            && !matches!(
                b,
                b'#' | b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
            );
        if regular {
            out.push(b as char);
        } else {
            out.push_str(&format!("#{b:02X}"));
        }
    }
    if out.is_empty() {
        "SpotColor".to_string()
    } else {
        out
    }
}

/// Escape a spot name for PostScript/EPS literal strings `( ... )`.
pub fn eps_string_escape(raw: &str) -> String {
    sanitize_spot_name(raw)
        .replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
}

/// Look up a sanitized spot name for `color` in `spots` (case-insensitive hex comparison).
pub fn lookup_spot_name(color: Rgb, spots: &[SpotColorMapping]) -> Option<String> {
    let target = hex(color);
    for m in spots {
        if let Some(rgb) = parse_hex_rgb(&m.hex) {
            if rgb == color {
                return Some(sanitize_spot_name(&m.spot_name));
            }
        } else if m.hex.trim().eq_ignore_ascii_case(&target) {
            return Some(sanitize_spot_name(&m.spot_name));
        }
    }
    None
}

/// Deterministic sRGB -> CMYK conversion with moderate GCR and a Pure Black rule.
///
/// - Neutral very dark blacks (`max <= 12` and `max - min <= 6`, including `#000000`) map strictly
///   to `[0.0, 0.0, 0.0, 1.0]` (pure 100% K) for crisp registration marks and linework.
/// - Pure white maps to `[0.0, 0.0, 0.0, 0.0]`.
/// - All output channels are clamped to `[0.0, 1.0]` and guaranteed finite.
pub fn rgb_to_cmyk(r: u8, g: u8, b: u8) -> [f32; 4] {
    let max_c = r.max(g).max(b);
    let min_c = r.min(g).min(b);
    if max_c <= 12 && (max_c - min_c) <= 6 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    if min_c == 255 {
        return [0.0, 0.0, 0.0, 0.0];
    }
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let k = (1.0 - rf.max(gf).max(bf)).clamp(0.0, 1.0);
    if k >= 0.9999 {
        return [0.0, 0.0, 0.0, 1.0];
    }
    let inv = 1.0 - k;
    let c = ((1.0 - rf - k) / inv).clamp(0.0, 1.0);
    let m = ((1.0 - gf - k) / inv).clamp(0.0, 1.0);
    let y = ((1.0 - bf - k) / inv).clamp(0.0, 1.0);
    [c, m, y, k]
}

#[inline]
pub fn rgb_arr_to_cmyk(rgb: Rgb) -> [f32; 4] {
    rgb_to_cmyk(rgb[0], rgb[1], rgb[2])
}

/// Offline deterministic CMYK soft-proof simulation (`sRGB -> CMYK -> simulated coated offset print sRGB`).
///
/// Uses a Neugebauer 8-primary subtractive coated-print model (ISO Coated v2 / FOGRA39 inspired primaries)
/// with ~14% midtone dot gain on chromatic inks, compressing out-of-gamut screen neons (electric blues,
/// bright greens, hot magentas) while preserving neutral greys, whites, and blacks cleanly.
pub fn rgb_to_soft_proof_rgb(r: u8, g: u8, b: u8) -> Rgb {
    let [c0, m0, y0, k0] = rgb_to_cmyk(r, g, b);

    // Midtone dot gain (~14% peak at 50% coverage) on chromatic inks.
    let dot_gain = |v: f32| -> f32 { (v + 0.14 * v * (1.0 - v)).clamp(0.0, 1.0) };
    let c = dot_gain(c0);
    let m = dot_gain(m0);
    let y = dot_gain(y0);
    // Slight dot gain on K only when chromatic inks are absent stays gentle so neutral greys stay close.
    let k = (k0 + 0.05 * k0 * (1.0 - k0)).clamp(0.0, 1.0);

    // ISO Coated v2 inspired Neugebauer primaries (sRGB 0..1)
    const W: [f32; 3] = [0.992, 0.988, 0.976];   // Coated paper white
    const C: [f32; 3] = [0.000, 0.624, 0.890];   // Process Cyan
    const M: [f32; 3] = [0.898, 0.000, 0.490];   // Process Magenta
    const Y: [f32; 3] = [1.000, 0.929, 0.000];   // Process Yellow
    const R: [f32; 3] = [0.890, 0.024, 0.075];   // M + Y
    const G: [f32; 3] = [0.000, 0.588, 0.251];   // C + Y
    const B: [f32; 3] = [0.192, 0.153, 0.514];   // C + M
    const CMY: [f32; 3] = [0.212, 0.180, 0.169]; // C + M + Y overprint
    const K_INK: [f32; 3] = [0.055, 0.055, 0.055]; // Rich coated black (~#0E0E0E)

    let ic = 1.0 - c;
    let im = 1.0 - m;
    let iy = 1.0 - y;

    let w_w = ic * im * iy;
    let w_c = c * im * iy;
    let w_m = ic * m * iy;
    let w_y = ic * im * y;
    let w_r = ic * m * y;
    let w_g = c * im * y;
    let w_b = c * m * iy;
    let w_cmy = c * m * y;

    let mut out = [0u8; 3];
    for ch in 0..3 {
        let cmy_val = w_w * W[ch]
            + w_c * C[ch]
            + w_m * M[ch]
            + w_y * Y[ch]
            + w_r * R[ch]
            + w_g * G[ch]
            + w_b * B[ch]
            + w_cmy * CMY[ch];
        // Black ink attenuation over the CMY substrate.
        let printed = cmy_val * (1.0 - k) + K_INK[ch] * k;
        out[ch] = (printed * 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// Returns true when the CIE76 ΔE shift between the original sRGB colour and its CMYK soft-proof
/// simulation exceeds 12.0 (significant print gamut compression warning).
pub fn is_out_of_cmyk_gamut(r: u8, g: u8, b: u8) -> bool {
    let orig = rgb_to_lab([r, g, b]);
    let proof = rgb_to_lab(rgb_to_soft_proof_rgb(r, g, b));
    let dl = orig[0] - proof[0];
    let da = orig[1] - proof[1];
    let db = orig[2] - proof[2];
    (dl * dl + da * da + db * db).sqrt() > 12.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pure_black_and_white() {
        assert_eq!(rgb_to_cmyk(0, 0, 0), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(rgb_to_cmyk(8, 10, 11), [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(rgb_to_cmyk(255, 255, 255), [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn primaries_are_finite_and_in_range() {
        let red = rgb_to_cmyk(255, 0, 0);
        assert!((red[0] - 0.0).abs() < 1e-5);
        assert!((red[1] - 1.0).abs() < 1e-5);
        assert!((red[2] - 1.0).abs() < 1e-5);
        assert!((red[3] - 0.0).abs() < 1e-5);
        for v in red {
            assert!(v.is_finite() && (0.0..=1.0).contains(&v));
        }
    }
}
