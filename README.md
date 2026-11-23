# Vektora (working name)

*Bahasa Indonesia: [README.id.md](README.id.md)*

An **offline Windows desktop app** that turns a flat illustration or a logo (JPG/PNG) into a clean **SVG or PDF** with tidy nodes, meant to open without warnings in Affinity and Adobe Illustrator. Prototype 0.3.0 ("Geometri rapi"). Local repository, no server, no account, **no network request**.

## What it does

Four stages (all on your computer), plus three checks added in 0.3.0 (basic shapes, symmetry, text warning):

1. **Thin-line guard.** Dark strokes thinner than a few pixels (eyes, smiles, outlines) are found in the ORIGINAL image first, because every cleaning step after that tends to erase them. A stroke must be darker than the pixels on both sides of it, so the dark rim that JPEG leaves at the edge of a colour shape is not mistaken for a line.
2. **Clean (optional, off by default).** Real-ESRGAN (anime x4, bundled sidecar) removes JPEG noise and is scaled to 2x. Off: a 3 px median filter is used (about as good on most images, 20-40x faster). If it is on but the sidecar or a GPU is missing, the app falls back to the median filter and says so.
3. **Palette lock + tracing.** 2-16 flat colours (k-means in Lab, edge pixels left out of the estimate so no halo colours appear; low-contrast specks merged away). Each colour is one stacked layer (a smaller colour always sits on top of a bigger one, so there are no gaps), traced with visioncortex (the engine of VTracer) and fitted with as few cubic Beziers as the tolerance allows.
4. **Lines on top.** Each thin line becomes ONE open path: its centerline (from a skeleton) with a measured `stroke-width` and round caps. Lines of uneven width stay filled shapes.

Export, SVG layers (names readable in Illustrator and Affinity):

```
Latar            the background colour
Bidang warna     one sub-group per colour: id="Warna_02_EBD9C1", data-name="Warna 02 #EBD9C1"
Garis            thin lines: <path fill="none" stroke="#223F64" stroke-width="4.12" stroke-linecap="round" ...>
```

No CSS, `<style>`, filter or transform. Also **PDF** (page size = image size) and the palette as **.ase** (Adobe Swatch Exchange, colours named `Vektora 01 #RRGGBB`; import in Illustrator via Swatches and in Affinity via Swatches > Import Palette; not tested by the author, see Limits). Options: thin lines as stroke or as filled shapes; trap 0 / 0.25 / 0.5 / 1 pt for print.

Controls: number of colours (2-16), smoothness (few nodes <-> detail), "keep thin lines", "recognise basic shapes" (on), "enforce symmetry" (off / automatic / vertical / horizontal), "clean with AI". Statistics on screen: paths, nodes, basic shapes, SVG size, colour error, time.

New in 0.3.0:

- **Basic shapes.** A circle, ellipse or (rounded) rectangle that matches closely is written as a pure `<circle>`, `<ellipse>` or `<rect>`; straight edges use `L`. Organic shapes stay paths.
- **Symmetry.** Off by default. When on, one half of a logo is mirrored onto the other, as pixels before tracing and as vectors afterwards, so both sides are identical and no seam is left on the axis.
- **Sharp corners vs smooth curves**, decided per point: star tips and letter corners stay sharp, leaf edges and ripples stay smooth.
- **Text warning.** If the picture seems to hold text, a warning shows how many areas and a **Sorot area teks** button (off by default) outlines them in the preview. The outlines are never exported. Text traced from a picture is always less tidy than retyping it with the real font.
- **Thin lines stay whole** (a fading vein no longer breaks into dots) and strokes that continue through a junction are one path.

## Use

1. Run the installer (`Vektora_0.3.0_x64-setup.exe`, per-user, no admin needed) and start Vektora.
2. Drag an image onto the window (or **Buka gambar**), wait for the conversion (about 1-3 s with the default settings, 10-40 s with AI on, on the integrated GPU of the test PC).
3. Drag the slider to compare the original and the vector, adjust the controls and press **Terapkan ulang** if you want.
4. Choose how thin lines and trap are exported, then **Ekspor SVG**, **Ekspor PDF** or **Palet .ase** (the .ase is saved next to the SVG/PDF when "Sertakan palet .ase" is ticked).

The interface is in Indonesian.

## Build from source

Needs Rust (this repo is built with the `x86_64-pc-windows-gnu` toolchain and a MinGW-w64 in `PATH`; the MSVC toolchain also works), Node 22+ and WebView2 (included in Windows 11).

```
npm ci
node scripts/fetch-sidecar.mjs          # downloads Real-ESRGAN ncnn Vulkan v0.2.5.0 into src-tauri/resources/realesrgan
npx tauri build --target x86_64-pc-windows-gnu   # installer in target/<triple>/release/bundle/nsis/
```

Developer tools:

```
npm test                                   # cargo tests V1-V7, W1-W6 and G1-G8 (synthetic images, no test data needed)
cargo run --release --bin vektora-cli -- in.jpg out.svg --pdf out.pdf --ase out.ase [--colors 8] [--smooth 0.5] [--ai] [--no-lines] [--lines stroke|shape] [--trap 0.5] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--png out.png] [--sidecar DIR]
cargo run --release --bin bench -- --dir <vectorize-bench folder> --sidecar <realesrgan folder>   # writes bench/RESULTS.md
```

The core is the crate `crates/vektora-core` (no UI, no network). The app in `src-tauri` is a thin shell; the UI is React + TypeScript in `src`.

## Offline

The core and the app contain no network code (checked with `cargo tree` in a test), the page can only talk to the local IPC (CSP), and WebView2 is started with background networking, updates, sync and SmartScreen off, all non-local traffic sent to a dead local proxy (`127.0.0.1:9`) and DNS for non-local names blocked. **What could not be removed:** see the report; the Edge WebView2 runtime that Windows provides may still open a connection of its own right after start-up. Vektora never sends image data anywhere.

## Limits of 0.3.0

- Windows x64 only. Real-ESRGAN needs a Vulkan GPU; otherwise the median fallback is used (more nodes, a bit more noise).
- Inputs are raster images (PNG, JPG, WEBP, BMP), longest side limited to 1600 px. Photos are not the target.
- Thin lines are the **dark** strokes only; light strokes on dark areas are traced as ordinary colour fields.
- Whether the SVG, PDF and `.ase` open and import without warnings in Affinity and Illustrator was **not tested by the author**; it is the owner's check (see the report). The PDF has plain grouping, no PDF layers (the converter does not write optional content groups).
- Light thin lines on dark areas (for example white foam lines on navy) are traced as colour fields; where their edges blend, a field may get a neighbouring palette colour (a beige patch instead of white).
- Text detection is a geometric heuristic (rows of letter-sized blobs); it can miss a single big letter or flag a pattern of small shapes. It never changes the drawing.
- Symmetry mirrors the whole picture across one axis; it is meant for logos, not for illustrations. The thin lines are mirrored at pixel level only (their stroke fits are not mirrored as vectors).
- Text detection is a geometric heuristic (rows of letter-sized blobs); it can miss a single big letter or flag a pattern of small shapes. It never changes the drawing.
- Symmetry mirrors the whole picture across one axis; it is meant for logos, not for illustrations. The thin lines are mirrored at pixel level only (their stroke fits are not mirrored as vectors).
- Benchmark numbers are in `bench/RESULTS.md`; the targets of the test plan are not all met (see the report).

## Third-party licenses

MIT for this project. Bundled: Real-ESRGAN model (BSD-3-Clause; check the model-file terms before selling), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), the OpenMP runtime `vcomp140.dll` (Microsoft redistributable). **No Potrace (GPL).** The full list of linked crates is in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) and in the app (button **Lisensi**).

Test image note: some benchmark images carry a stock-photo watermark. They are used for testing only and are not part of this repository or of the app.
