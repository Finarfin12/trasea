//! Node density map. Counts the anchors of the finished drawing per cell of a coarse grid, so the app can show where the
//! nodes are piling up. It only reads the drawing: it never reaches the SVG or the PDF.

use crate::export::Doc;
use crate::primitives::Primitive;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NodeHeatmap {
    pub cols: usize,
    pub rows: usize,
    /// Node count per cell (row-major, length = cols * rows).
    pub cells: Vec<u16>,
    pub max_cell: u16,
    /// Cells holding at least 60% of the busiest cell (and at least 4 nodes).
    pub hot_cells: usize,
}

/// Cells along the long side of the picture.
pub const LONG_SIDE_CELLS: usize = 32;
/// Cells along the short side, at least.
pub const MIN_SHORT_CELLS: usize = 8;

/// Count the nodes of the default export (colour fields, basic shapes, thin lines as strokes) per grid cell.
pub fn node_heatmap(doc: &Doc) -> NodeHeatmap {
    let (w, h) = (doc.width.max(1) as f64, doc.height.max(1) as f64);
    let cell = w.max(h) / LONG_SIDE_CELLS as f64;
    let cols = ((w / cell).ceil() as usize).max(if w >= h { LONG_SIDE_CELLS } else { MIN_SHORT_CELLS });
    let rows = ((h / cell).ceil() as usize).max(if h >= w { LONG_SIDE_CELLS } else { MIN_SHORT_CELLS });
    let mut cells = vec![0u16; cols * rows];
    let k = doc.scale;
    let mut add = |x: f64, y: f64| {
        let (cx, cy) = (((x / k / cell) as i64).clamp(0, cols as i64 - 1) as usize, ((y / k / cell) as i64).clamp(0, rows as i64 - 1) as usize);
        let c = &mut cells[cy * cols + cx];
        *c = c.saturating_add(1);
    };
    for l in doc.fields.iter().chain(doc.line_fallback.iter()) {
        for c in &l.contours {
            add(c.start.0, c.start.1);
            for cv in &c.curves {
                add(cv[2].0, cv[2].1);
            }
        }
        for p in &l.prims {
            match *p {
                Primitive::Circle { cx, cy, r } => {
                    for (dx, dy) in [(r, 0.0), (-r, 0.0), (0.0, r), (0.0, -r)] {
                        add(cx + dx, cy + dy);
                    }
                }
                Primitive::Ellipse { cx, cy, rx, ry, angle_deg } => {
                    let (s, c) = angle_deg.to_radians().sin_cos();
                    for (px, py) in [(rx * c, rx * s), (-rx * c, -rx * s), (-ry * s, ry * c), (ry * s, -ry * c)] {
                        add(cx + px, cy + py);
                    }
                }
                Primitive::Rect { x, y, w, h, .. } => {
                    for (px, py) in [(x, y), (x + w, y), (x + w, y + h), (x, y + h)] {
                        add(px, py);
                    }
                }
            }
        }
    }
    for sl in &doc.strokes {
        for st in &sl.strokes {
            if let Some(f) = st.curves.first() {
                add(f[0].0, f[0].1);
            }
            for cv in &st.curves {
                add(cv[3].0, cv[3].1);
            }
        }
    }
    let max_cell = cells.iter().copied().max().unwrap_or(0);
    let hot = ((max_cell as f64) * 0.6).max(4.0);
    let hot_cells = cells.iter().filter(|&&c| c as f64 >= hot).count();
    NodeHeatmap { cols, rows, cells, max_cell, hot_cells }
}
