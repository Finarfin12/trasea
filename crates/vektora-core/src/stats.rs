//! Statistics of a finished SVG: paths, nodes, size, and (dev mode) how close the render is to the original.

use crate::{Error, Result};
use image::RgbImage;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct Stats {
    pub paths: usize,
    /// Anchors: every M, L and C segment counts as one node (the same counting as the baseline benchmark).
    pub nodes: usize,
    pub bytes: usize,
    /// Mean absolute error of the render against the original (0..255), when computed.
    pub mae: Option<f64>,
}

fn segments_per_command(cmd: char) -> Option<usize> {
    match cmd.to_ascii_uppercase() {
        'M' | 'L' | 'T' => Some(2),
        'H' | 'V' => Some(1),
        'C' => Some(6),
        'S' | 'Q' => Some(4),
        'A' => Some(7),
        _ => None,
    }
}

/// Number of path elements and nodes in the `d` attributes of an SVG string.
pub fn count_svg(svg: &str) -> (usize, usize) {
    let (mut paths, mut nodes) = (0usize, 0usize);
    let bytes = svg.as_bytes();
    let mut i = 0;
    while let Some(pos) = svg[i..].find("<path") {
        let start = i + pos;
        let end = svg[start..].find('>').map(|e| start + e).unwrap_or(svg.len());
        let tag = &svg[start..end];
        paths += 1;
        if let Some(dpos) = tag.find(" d=\"") {
            let rest = &tag[dpos + 4..];
            let d = &rest[..rest.find('"').unwrap_or(rest.len())];
            nodes += count_d(d);
        }
        i = end.min(bytes.len());
        if i >= svg.len() {
            break;
        }
    }
    (paths, nodes)
}

pub fn count_d(d: &str) -> usize {
    let mut nodes = 0;
    let mut cmd: Option<char> = None;
    let mut numbers = 0usize;
    let mut flush = |cmd: Option<char>, numbers: usize, nodes: &mut usize| {
        if let Some(c) = cmd {
            if let Some(per) = segments_per_command(c) {
                *nodes += numbers / per;
            }
        }
    };
    let chars: Vec<char> = d.chars().collect();
    let mut k = 0;
    while k < chars.len() {
        let c = chars[k];
        if c.is_ascii_alphabetic() && c != 'e' && c != 'E' {
            flush(cmd, numbers, &mut nodes);
            cmd = Some(c);
            numbers = 0;
            k += 1;
        } else if c == '-' || c == '.' || c.is_ascii_digit() {
            // one number: sign, digits, dot, digits, optional exponent
            let mut j = k;
            if chars[j] == '-' {
                j += 1;
            }
            let mut seen_dot = false;
            while j < chars.len() && (chars[j].is_ascii_digit() || (chars[j] == '.' && !seen_dot)) {
                if chars[j] == '.' {
                    seen_dot = true;
                }
                j += 1;
            }
            if j < chars.len() && (chars[j] == 'e' || chars[j] == 'E') {
                j += 1;
                if j < chars.len() && (chars[j] == '-' || chars[j] == '+') {
                    j += 1;
                }
                while j < chars.len() && chars[j].is_ascii_digit() {
                    j += 1;
                }
            }
            numbers += 1;
            k = j.max(k + 1);
        } else {
            k += 1;
        }
    }
    flush(cmd, numbers, &mut nodes);
    nodes
}

/// Render an SVG onto white at the given pixel size.
pub fn render_svg(svg: &str, w: u32, h: u32) -> Result<RgbImage> {
    let opt = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_str(svg, &opt).map_err(|e| Error::Other(format!("cannot render the SVG: {e}")))?;
    let mut pix = resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| Error::Other("bad render size".into()))?;
    pix.fill(resvg::tiny_skia::Color::WHITE);
    let size = tree.size();
    let t = resvg::tiny_skia::Transform::from_scale(w as f32 / size.width(), h as f32 / size.height());
    resvg::render(&tree, t, &mut pix.as_mut());
    let mut out = RgbImage::new(w, h);
    for (i, p) in pix.pixels().iter().enumerate() {
        // opaque background, so premultiplied == straight
        out.put_pixel((i as u32) % w, (i as u32) / w, image::Rgb([p.red(), p.green(), p.blue()]));
    }
    Ok(out)
}

/// (mean absolute error 0..255, share of pixels that differ by more than 32).
pub fn compare(a: &RgbImage, b: &RgbImage) -> (f64, f64) {
    let b = if a.dimensions() != b.dimensions() { image::imageops::resize(b, a.width(), a.height(), image::imageops::FilterType::Triangle) } else { b.clone() };
    let (mut sum, mut bad) = (0f64, 0usize);
    for (p, q) in a.pixels().zip(b.pixels()) {
        let d = ((p[0] as i32 - q[0] as i32).abs() + (p[1] as i32 - q[1] as i32).abs() + (p[2] as i32 - q[2] as i32).abs()) as f64 / 3.0;
        sum += d;
        if d > 32.0 {
            bad += 1;
        }
    }
    let n = (a.width() as usize * a.height() as usize).max(1) as f64;
    (sum / n, bad as f64 / n * 100.0)
}

pub fn stats_for(svg: &str, original: Option<&RgbImage>) -> Stats {
    let (paths, nodes) = count_svg(svg);
    let mae = original.and_then(|o| render_svg(svg, o.width(), o.height()).ok().map(|r| compare(o, &r).0));
    Stats { paths, nodes, bytes: svg.len(), mae }
}
