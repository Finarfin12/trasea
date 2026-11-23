//! Colour space definitions and deterministic RGB -> CMYK conversion for print export (PDF, EPS, ASE).

use crate::Rgb;
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