# Changelog

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
