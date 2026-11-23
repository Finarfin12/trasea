//! Correction brush. The designer paints over a picture to say "more detail here" or "smoother here". The strokes are
//! rasterised to a zone map at working resolution; the palette clean-up and the curve fit then read it. Everything still
//! comes from one label picture, so there is no seam at the edge of a painted area.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BrushKind {
    /// Keep small details: tighter curve fit, small regions are kept.
    Detail,
    /// Fewer nodes: looser curve fit, small specks are merged away.
    Smooth,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrushStroke {
    pub kind: BrushKind,
    /// Radius in source image pixels.
    pub radius: f32,
    /// Polyline points (x, y) in source image pixels.
    pub points: Vec<(f32, f32)>,
}

pub const ZONE_NORMAL: u8 = 0;
pub const ZONE_DETAIL: u8 = 1;
pub const ZONE_SMOOTH: u8 = 2;

/// Curve-fit tolerance multiplier inside a painted zone.
pub const DETAIL_TOL: f64 = 0.45;
pub const SMOOTH_TOL: f64 = 2.2;
/// Speck-size multiplier inside a painted zone.
pub const DETAIL_SPECK: f64 = 0.4;
pub const SMOOTH_SPECK: f64 = 2.5;

pub fn tol_factor(zone: u8) -> f64 {
    match zone {
        ZONE_DETAIL => DETAIL_TOL,
        ZONE_SMOOTH => SMOOTH_TOL,
        _ => 1.0,
    }
}

pub fn speck_factor(zone: u8) -> f64 {
    match zone {
        ZONE_DETAIL => DETAIL_SPECK,
        ZONE_SMOOTH => SMOOTH_SPECK,
        _ => 1.0,
    }
}

/// Zone per working pixel: 0 = normal, 1 = detail, 2 = smooth.
#[derive(Debug, Clone)]
pub struct BrushMap {
    pub w: usize,
    pub h: usize,
    pub zones: Vec<u8>,
}

impl BrushMap {
    /// Rasterise the strokes in order (a later stroke paints over an earlier one). `scale` = working px per source px.
    /// `None` when there is nothing to paint.
    pub fn rasterize(strokes: &[BrushStroke], scale: f64, w: usize, h: usize) -> Option<BrushMap> {
        let strokes: Vec<&BrushStroke> = strokes.iter().filter(|s| !s.points.is_empty() && s.radius > 0.0).collect();
        if strokes.is_empty() || w == 0 || h == 0 {
            return None;
        }
        let mut zones = vec![ZONE_NORMAL; w * h];
        for s in strokes {
            let zone = match s.kind {
                BrushKind::Detail => ZONE_DETAIL,
                BrushKind::Smooth => ZONE_SMOOTH,
            };
            let r = s.radius as f64 * scale;
            let pts: Vec<(f64, f64)> = s.points.iter().map(|p| (p.0 as f64 * scale, p.1 as f64 * scale)).collect();
            let step = (r * 0.35).max(1.0);
            let mut stamp = |cx: f64, cy: f64| {
                let (x0, x1) = (((cx - r).floor() as i64).max(0), ((cx + r).ceil() as i64).min(w as i64 - 1));
                let (y0, y1) = (((cy - r).floor() as i64).max(0), ((cy + r).ceil() as i64).min(h as i64 - 1));
                for y in y0..=y1 {
                    for x in x0..=x1 {
                        let (dx, dy) = (x as f64 + 0.5 - cx, y as f64 + 0.5 - cy);
                        if dx * dx + dy * dy <= r * r {
                            zones[y as usize * w + x as usize] = zone;
                        }
                    }
                }
            };
            stamp(pts[0].0, pts[0].1);
            for seg in pts.windows(2) {
                let (a, b) = (seg[0], seg[1]);
                let len = ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt();
                let n = (len / step).ceil().max(1.0) as usize;
                for k in 1..=n {
                    let t = k as f64 / n as f64;
                    stamp(a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t);
                }
            }
        }
        Some(BrushMap { w, h, zones })
    }

    /// Zone at a position in working pixels (0 outside the picture).
    pub fn at(&self, x: f64, y: f64) -> u8 {
        if x < 0.0 || y < 0.0 {
            return ZONE_NORMAL;
        }
        let (xi, yi) = (x as usize, y as usize);
        if xi >= self.w || yi >= self.h {
            return ZONE_NORMAL;
        }
        self.zones[yi * self.w + xi]
    }
}
