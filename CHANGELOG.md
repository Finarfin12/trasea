# Changelog

## 0.3.0 "Geometri rapi"

- **Basic shapes.** A closed contour without holes that matches a circle, an ellipse or a (rounded) rectangle closely is exported as a pure `<circle>`, `<ellipse>` or `<rect>` (a `rotate` transform only when it is turned by more than 0.5 degrees). The shape is judged on the smoothed raster outline, not on the fitted curve, with a strict tolerance (radial error at most max(0.9 px, 1.5% of the size) and never more than 2.5 px), so a roughly round organic shape stays a path. Contours that touch the picture edge are never judged. Straight edges are written as `L`, not as a flat cubic. Option "Kenali bentuk dasar" (on by default), CLI `--no-primitives`.
- **Symmetry enforcer.** `Tegas simetri`: Mati (default) / Otomatis / Vertikal / Horizontal. The better half of the palette-label picture is copied onto the other half before tracing, and afterwards the contours of the kept side are mirrored as vectors (a contour that crosses the axis is cut on the axis and closed with its own mirror image), so both sides have the same nodes and there is no seam on the axis. `Otomatis` only acts when at least 88% of the pixels match their mirror image. CLI `--symmetry off|auto|vertical|horizontal`.
- **Corner or curve, per point.** The corner test is no longer one global angle. A candidate is a corner when its turning angle holds over a longer stretch or its two arms are straight lines; a tight but organic bend (a leaf tip, the end of a stroke) stays a smooth Bezier. A corner sits where its two arm lines cross, so star tips and letter corners keep their size (smoothing used to pull them in by about half a pixel).
- **Text warning.** `text_detect.rs` looks for rows of letter-sized blobs of one colour on one line (no OCR, no model, offline). The app shows a warning with the number of areas and a **Sorot area teks** button (off by default) that outlines them in the preview. The outlines are never written to the SVG or PDF. CLI `--no-text-detect`.
- **Thin lines that stay in one piece.** Hysteresis: a faded line pixel is kept when it touches a strong line pixel of nearly the same colour (so a dark-green vein that fades into a green leaf no longer falls apart into dots, while the red outline of another shape is not pulled in). Breaks of 1-2 px are bridged only where the line goes on in a straight row of at least 4 pixels on both sides (no thickening, no fusing of parallel lines).
- **Fewer, longer strokes.** Strokes that continue each other through a junction (direction within 35 degrees, width within 0.75 px) are joined into one path; fragments shorter than 1.5 px that hang on a junction are dropped.
- Stats show the number of basic shapes (a `<circle>`, `<ellipse>` or `<rect>` counts as 4 nodes in the node total).
- Tests: G1-G6 and G8 added (`tests/core03.rs`); the 16 older tests (V1-V7 and regressions, W1-W6) still pass unchanged. Dev CLI: `--png out.png` and `--render in.svg out.png W H` draw the result for before/after pictures; bench has `--prev DIR` for a 0.2.0 vs 0.3.0 table.

## 0.2.0 "File siap kerja"

- **Thin lines as strokes.** Thin dark lines are exported as open `<path>` elements with `fill="none"`, a measured `stroke-width` and round caps/joins (centerline from a skeleton, width from the distance transform, rounded to 0.25 px). Lines whose width is too uneven (blobs, brush strokes) stay filled shapes. Export option "lines as stroke / as shape".
- **Named, ordered layers.** The SVG is `Latar` > `Bidang warna` (one sub-group per colour, `id="Warna_01_RRGGBB"`, `data-name="Warna 01 #RRGGBB"`) > `Garis`. No `<style>`, no classes, no filters. (The 0.1.0 layout used a bare `#RRGGBB` id per colour.)
- **Palette export.** `.ase` (Adobe Swatch Exchange), one group "Vektora", global RGB colours named `Vektora NN #RRGGBB`, saved next to the SVG/PDF (optional) or on its own.
- **No gaps between fields**, checked by a test; optional **trap** (0 / 0.25 / 0.5 / 1 pt) grows every colour field by an outline of its own colour.
- **"Clean with AI" is off by default.** The 0.1.0 benchmark showed the median fallback is about as good and 20-40x faster. Turn it on for blurry or badly compressed JPEGs.
- **Fewer nodes.** Low-contrast specks (JPEG residue, watermark ghosts) are merged away up to 25x the normal speck size; the curve fit uses a wider baseline for its split tangent (fewer spurious nodes on noisy edges); thin regions whose edge blends picked a wrong palette colour are re-checked against their core colour.
- **Offline hardening.** WebView2 starts with background networking, component updates, sync, SmartScreen and telemetry features off, all non-loopback traffic sent to a dead local proxy, and DNS for non-local names blocked. See the README for what was measured and what could not be removed.
- Licenses dialog now shows tables and a collapsible crate list instead of raw Markdown.
- CLI: `--ai`, `--lines stroke|shape`, `--trap PT`, `--ase file.ase`.
- Tests: W1-W6 added (centerline width and node count, layer structure, ASE parser, no gaps, trap, AI default); the 0.1.0 tests V1-V7 still pass (V1 follows the new layer layout).

## 0.1.0

First prototype: 4-stage pipeline (thin-line guard, optional Real-ESRGAN clean, palette lock + layered tracing, lines on top), clean SVG and PDF export, Tauri 2 app, benchmark, V1-V7 tests.
