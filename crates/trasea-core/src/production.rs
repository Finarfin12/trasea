//! Production profiles for Trasea 0.5.0: Screen Print (`#14`) and Cutting Sticker / Embroidery (`#15`).
//! Keeps deterministic post-processing rules out of `pipeline.rs`.

use crate::export::Doc;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProductionMode {
    #[default]
    Standard,
    ScreenPrint,
    CutEmbroidery,
}

impl ProductionMode {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_ascii_lowercase().as_str() {
            "standard" | "standar" => Some(Self::Standard),
            "screenprint" | "screen-print" | "screen_print" | "sablon" => Some(Self::ScreenPrint),
            "cutembroidery" | "cut-embroidery" | "cut_embroidery" | "cutting" | "cut" | "bordir" => Some(Self::CutEmbroidery),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "Standar",
            Self::ScreenPrint => "Sablon",
            Self::CutEmbroidery => "Cutting / Bordir",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductionOptions {
    #[serde(default)]
    pub mode: ProductionMode,
    /// Minimum stroke width in points (pt). 0.0 = disabled (Standard), 1.0 (ScreenPrint), 1.5 (CutEmbroidery).
    #[serde(default)]
    pub min_stroke_pt: f64,
    /// Omit the background layer (index 0) on CutEmbroidery. Default true in CutEmbroidery, false otherwise.
    #[serde(default)]
    pub strip_background: bool,
    /// Add 4 registration marks in the outer margin (ScreenPrint). Default true in ScreenPrint, false otherwise.
    #[serde(default)]
    pub registration_marks: bool,
}

impl Default for ProductionOptions {
    fn default() -> Self {
        Self::standard()
    }
}

impl ProductionOptions {
    pub fn standard() -> Self {
        Self {
            mode: ProductionMode::Standard,
            min_stroke_pt: 0.0,
            strip_background: false,
            registration_marks: false,
        }
    }

    pub fn screen_print() -> Self {
        Self {
            mode: ProductionMode::ScreenPrint,
            min_stroke_pt: 1.0,
            strip_background: false,
            registration_marks: true,
        }
    }

    pub fn cut_embroidery() -> Self {
        Self {
            mode: ProductionMode::CutEmbroidery,
            min_stroke_pt: 1.5,
            strip_background: true,
            registration_marks: false,
        }
    }

    /// Effective minimum stroke width in points for the active mode.
    pub fn effective_min_stroke_pt(&self) -> f64 {
        if self.min_stroke_pt > 0.0 {
            self.min_stroke_pt
        } else {
            match self.mode {
                ProductionMode::Standard => 0.0,
                ProductionMode::ScreenPrint => 1.0,
                ProductionMode::CutEmbroidery => 1.5,
            }
        }
    }
}

/// Default micro-spot threshold for ScreenPrint (~0.04% of image area, min 12 px^2).
pub fn screen_print_min_spot_px(w: u32, h: u32) -> usize {
    ((w as usize).saturating_mul(h as usize) * 4 / 10_000).max(12)
}

/// Default weeding safety threshold for CutEmbroidery (~0.15% of image area, min 36 px^2,
/// capped at 600 working px^2 so letter counters on padded logos are preserved).
pub fn cut_min_area_px(w: u32, h: u32) -> usize {
    ((w as usize).saturating_mul(h as usize) * 15 / 10_000).clamp(36, 600)
}

/// Find 4-connected regions in `labels` with area < `min_area_px` and absorb each into its
/// majority 4-connected boundary neighbour. Returns the number of absorbed micro-regions.
pub fn filter_micro_regions(labels: &mut [u8], w: u32, h: u32, min_area_px: usize) -> usize {
    let (wu, hu) = (w as usize, h as usize);
    let n = wu.saturating_mul(hu);
    if min_area_px <= 1 || n == 0 || labels.len() != n {
        return 0;
    }

    let mut visited = vec![false; n];
    let mut queue = VecDeque::new();
    let mut pixels = Vec::new();
    let mut removed = 0usize;

    for start in 0..n {
        if visited[start] {
            continue;
        }
        let lbl = labels[start];
        visited[start] = true;
        queue.clear();
        pixels.clear();
        queue.push_back(start);

        while let Some(idx) = queue.pop_front() {
            pixels.push(idx);
            let x = idx % wu;
            let y = idx / wu;
            if x > 0 {
                let nb = idx - 1;
                if !visited[nb] && labels[nb] == lbl {
                    visited[nb] = true;
                    queue.push_back(nb);
                }
            }
            if x + 1 < wu {
                let nb = idx + 1;
                if !visited[nb] && labels[nb] == lbl {
                    visited[nb] = true;
                    queue.push_back(nb);
                }
            }
            if y > 0 {
                let nb = idx - wu;
                if !visited[nb] && labels[nb] == lbl {
                    visited[nb] = true;
                    queue.push_back(nb);
                }
            }
            if y + 1 < hu {
                let nb = idx + wu;
                if !visited[nb] && labels[nb] == lbl {
                    visited[nb] = true;
                    queue.push_back(nb);
                }
            }
        }

        if pixels.len() >= min_area_px {
            continue;
        }

        // Count 4-connected neighbours outside this component.
        let mut counts = [0usize; 256];
        let mut best_lbl: Option<u8> = None;
        let mut best_count = 0usize;
        for &idx in &pixels {
            let x = idx % wu;
            let y = idx / wu;
            let mut consider = |nb: usize| {
                let nl = labels[nb];
                if nl != lbl {
                    let c = counts[nl as usize] + 1;
                    counts[nl as usize] = c;
                    if c > best_count || (c == best_count && best_lbl.map_or(true, |b| nl < b)) {
                        best_count = c;
                        best_lbl = Some(nl);
                    }
                }
            };
            if x > 0 {
                consider(idx - 1);
            }
            if x + 1 < wu {
                consider(idx + 1);
            }
            if y > 0 {
                consider(idx - wu);
            }
            if y + 1 < hu {
                consider(idx + wu);
            }
        }

        if let Some(target) = best_lbl {
            for &idx in &pixels {
                labels[idx] = target;
            }
            removed += 1;
        }
    }

    removed
}

/// Raise every stroke in `doc.strokes` narrower than `min_stroke_pt` (converted to working pixels)
/// up to `min_stroke_pt`. Returns how many strokes were thickened.
pub fn enforce_min_stroke(doc: &mut Doc, min_stroke_pt: f64) -> usize {
    if min_stroke_pt <= 0.0 {
        return 0;
    }
    let min_px = min_stroke_pt * 96.0 / 72.0 * doc.scale;
    let mut thickened = 0usize;
    for sl in &mut doc.strokes {
        for st in &mut sl.strokes {
            if st.width + 1e-9 < min_px {
                st.width = min_px;
                thickened += 1;
            }
        }
    }
    thickened
}