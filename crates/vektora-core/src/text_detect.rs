//! Text area detector. Tracing letters never beats typing the text again with the real font, so the user should be told when
//! the picture seems to hold text. This is a light geometric heuristic, no OCR and no model: it looks for rows of
//! letter-sized blobs of one colour that sit on one line with similar heights. It never changes the drawing.

use crate::Rgb;
use serde::Serialize;

/// A suspected text area in SOURCE pixels (the same coordinates as the loaded image).
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextRegion {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
    pub glyph_count: usize,
}

const MIN_H: f64 = 8.0;
const MAX_H: f64 = 120.0;
const MIN_ASPECT: f64 = 0.15;
const MAX_ASPECT: f64 = 2.2;
const MIN_FILL: f64 = 0.15;
const MAX_FILL: f64 = 0.85;
/// Luminance difference (0..255) between a blob and what surrounds it.
const MIN_CONTRAST: f32 = 40.0;

#[derive(Clone)]
struct Blob {
    x0: usize,
    y0: usize,
    x1: usize,
    y1: usize,
    area: usize,
    label: u8,
}

impl Blob {
    fn w(&self) -> f64 {
        (self.x1 - self.x0 + 1) as f64
    }
    fn h(&self) -> f64 {
        (self.y1 - self.y0 + 1) as f64
    }
    fn cy(&self) -> f64 {
        (self.y0 + self.y1) as f64 / 2.0
    }
}

struct Line {
    members: Vec<Blob>,
    avg_h: f64,
    cy: f64,
    x1: f64,
}

fn lum(c: Rgb) -> f32 {
    0.299 * c[0] as f32 + 0.587 * c[1] as f32 + 0.114 * c[2] as f32
}

/// 8-connected blobs of equal label. `labels` is `w` x `h`.
fn blobs(labels: &[u8], w: usize, h: usize) -> Vec<Blob> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    let mut stack = Vec::new();
    for s in 0..w * h {
        if seen[s] {
            continue;
        }
        let l = labels[s];
        seen[s] = true;
        stack.push(s);
        let mut b = Blob { x0: s % w, y0: s / w, x1: s % w, y1: s / w, area: 0, label: l };
        while let Some(i) = stack.pop() {
            let (x, y) = (i % w, i / w);
            b.area += 1;
            b.x0 = b.x0.min(x);
            b.x1 = b.x1.max(x);
            b.y0 = b.y0.min(y);
            b.y1 = b.y1.max(y);
            for dy in -1i64..=1 {
                for dx in -1i64..=1 {
                    let (nx, ny) = (x as i64 + dx, y as i64 + dy);
                    if nx < 0 || ny < 0 || nx >= w as i64 || ny >= h as i64 {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    if !seen[j] && labels[j] == l {
                        seen[j] = true;
                        stack.push(j);
                    }
                }
            }
        }
        // a huge blob can never be a letter: skip the bookkeeping for it
        if b.h() <= MAX_H && b.h() >= MIN_H {
            out.push(b);
        }
    }
    out
}

/// The label that most often surrounds the blob (a ring around its box, own label ignored).
fn surround_label(b: &Blob, labels: &[u8], w: usize, h: usize) -> Option<u8> {
    let mut n = [0u32; 256];
    let (x0, y0) = (b.x0.saturating_sub(2), b.y0.saturating_sub(2));
    let (x1, y1) = ((b.x1 + 2).min(w - 1), (b.y1 + 2).min(h - 1));
    let mut add = |x: usize, y: usize| {
        let l = labels[y * w + x];
        if l != b.label {
            n[l as usize] += 1;
        }
    };
    for x in x0..=x1 {
        add(x, y0);
        add(x, y1);
    }
    for y in y0..=y1 {
        add(x0, y);
        add(x1, y);
    }
    let (best, cnt) = n.iter().enumerate().max_by_key(|(_, c)| **c).map(|(i, c)| (i, *c))?;
    (cnt > 0).then_some(best as u8)
}

/// Find suspected text areas. `labels` is the palette-label picture at working size (`ww` x `wh`), `up` = working px per
/// source px. Coordinates in the result are source pixels.
pub fn detect(labels: &[u8], ww: usize, wh: usize, up: usize, palette: &[Rgb]) -> Vec<TextRegion> {
    let up = up.max(1);
    let (w, h) = (ww / up, wh / up);
    if w < 16 || h < 16 {
        return vec![];
    }
    let mut small = vec![0u8; w * h];
    for y in 0..h {
        for x in 0..w {
            small[y * w + x] = labels[(y * up + up / 2).min(wh - 1) * ww + (x * up + up / 2).min(ww - 1)];
        }
    }
    // letter-like blobs
    let mut letters: Vec<Blob> = Vec::new();
    for b in blobs(&small, w, h) {
        let (bw, bh) = (b.w(), b.h());
        let aspect = bw / bh;
        let fill = b.area as f64 / (bw * bh);
        if !(MIN_ASPECT..=MAX_ASPECT).contains(&aspect) || !(MIN_FILL..=MAX_FILL).contains(&fill) {
            continue;
        }
        let Some(sl) = surround_label(&b, &small, w, h) else { continue };
        let (own, around) = (palette.get(b.label as usize).copied().map(lum), palette.get(sl as usize).copied().map(lum));
        if let (Some(a), Some(c)) = (own, around) {
            if (a - c).abs() < MIN_CONTRAST {
                continue;
            }
        }
        letters.push(b);
    }
    letters.sort_by_key(|b| b.x0);

    // group letters that sit on one line, have similar heights and are close together
    let mut lines: Vec<Line> = Vec::new();
    for b in letters {
        let (bh, bcy) = (b.h(), b.cy());
        let mut best: Option<(usize, f64)> = None;
        for (k, ln) in lines.iter().enumerate() {
            let ratio = bh / ln.avg_h;
            let gap = b.x0 as f64 - ln.x1;
            if (bcy - ln.cy).abs() <= 0.35 * bh.max(ln.avg_h) && (0.6..=1.65).contains(&ratio) && gap <= 1.4 * ln.avg_h && gap >= -0.3 * bh {
                let d = (bcy - ln.cy).abs();
                if best.map_or(true, |(_, bd)| d < bd) {
                    best = Some((k, d));
                }
            }
        }
        match best {
            Some((k, _)) => {
                let ln = &mut lines[k];
                let n = ln.members.len() as f64;
                ln.avg_h = (ln.avg_h * n + bh) / (n + 1.0);
                ln.cy = (ln.cy * n + bcy) / (n + 1.0);
                ln.x1 = ln.x1.max(b.x1 as f64);
                ln.members.push(b);
            }
            None => lines.push(Line { avg_h: bh, cy: bcy, x1: b.x1 as f64, members: vec![b] }),
        }
    }

    let mut out = Vec::new();
    for ln in lines {
        let n = ln.members.len();
        let regular = n >= 3 && {
            let gaps: Vec<f64> = ln.members.windows(2).map(|p| p[1].x0 as f64 - p[0].x1 as f64).collect();
            let mean = gaps.iter().sum::<f64>() / gaps.len() as f64;
            let sd = (gaps.iter().map(|g| (g - mean).powi(2)).sum::<f64>() / gaps.len() as f64).sqrt();
            mean > 0.0 && sd / mean < 0.25 && mean < 0.8 * ln.avg_h
        };
        if n < 4 && !regular {
            continue;
        }
        let x0 = ln.members.iter().map(|m| m.x0).min().unwrap();
        let x1 = ln.members.iter().map(|m| m.x1).max().unwrap();
        let y0 = ln.members.iter().map(|m| m.y0).min().unwrap();
        let y1 = ln.members.iter().map(|m| m.y1).max().unwrap();
        let (x0, y0) = (x0.saturating_sub(2), y0.saturating_sub(2));
        let (x1, y1) = ((x1 + 3).min(w), (y1 + 3).min(h));
        out.push(TextRegion { x: x0 as f64, y: y0 as f64, w: (x1 - x0) as f64, h: (y1 - y0) as f64, glyph_count: n });
    }
    out
}
