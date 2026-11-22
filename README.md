# Vektora (working name)

*Bahasa Indonesia: [README.id.md](README.id.md)*

An **offline Windows desktop app** that turns a flat illustration or a logo (JPG/PNG) into a clean **SVG or PDF** with tidy nodes, meant to open without warnings in Affinity and Adobe Illustrator. Prototype 0.1.0. Local repository, no server, no account, **no network request**.

## What it does

Four stages (all on your computer):

1. **Thin-line guard.** Dark strokes thinner than a few pixels (eyes, smiles, outlines) are found in the ORIGINAL image first, because every cleaning step after that tends to erase them. A stroke must be darker than the pixels on both sides of it, so the dark rim that JPEG leaves at the edge of a colour shape is not mistaken for a line.
2. **Clean (optional).** Real-ESRGAN (anime x4, bundled sidecar `realesrgan-ncnn-vulkan.exe`) removes JPEG noise; the result is scaled to 2x. Without the sidecar, without a usable GPU, or with the toggle off, a 3 px median filter is used and the app says so.
3. **Palette lock + tracing.** The colours are reduced to 2-16 flat colours (k-means in Lab, edge pixels left out of the estimate so no in-between "halo" colours appear). Each colour becomes one stacked layer, traced with visioncortex (the engine of VTracer) and fitted with as few cubic Beziers as the tolerance allows. Thresholds scale with the image size.
4. **Lines on top.** The thin lines are traced at 2x and stacked above the colour fields.

Export: **SVG** (one `<g id="#RRGGBB">` per colour, `fill` directly on the path, no CSS, no `<style>`, no filter, no transform, `viewBox` = image size) and **PDF** (page size = image size).

Controls: number of colours (2-16), smoothness (few nodes <-> detail), "keep thin lines", "clean with AI". Statistics on screen: paths, nodes, SVG size, colour error, time.

## Use

1. Run the installer (`Vektora_0.1.0_x64-setup.exe`, per-user, no admin needed) and start Vektora.
2. Drag an image onto the window (or **Buka gambar**), wait for the conversion (about 1 s without AI, 10-35 s with AI on the integrated GPU of the test PC).
3. Drag the slider to compare the original and the vector, adjust the controls and press **Terapkan ulang** if you want.
4. **Ekspor SVG** or **Ekspor PDF**.

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
npm test                                   # cargo tests V1-V7 (synthetic images, no test data needed)
cargo run --release --bin vektora-cli -- in.jpg out.svg --pdf out.pdf [--colors 8] [--smooth 0.5] [--no-ai] [--no-lines] [--sidecar DIR]
cargo run --release --bin bench -- --dir <vectorize-bench folder> --sidecar <realesrgan folder>   # writes bench/RESULTS.md
```

The core is the crate `crates/vektora-core` (no UI, no network). The app in `src-tauri` is a thin shell; the UI is React + TypeScript in `src`.

## Limits of 0.1.0

- Windows x64 only. Real-ESRGAN needs a Vulkan GPU; otherwise the median fallback is used (more nodes, a bit more noise).
- Inputs are raster images (PNG, JPG, WEBP, BMP), longest side limited to 1600 px. Photos are not the target.
- Thin lines are the **dark** strokes only; light strokes on dark areas are traced as ordinary colour fields.
- Whether the SVG and PDF open without warnings in Affinity and Illustrator was **not tested by the author**; it is the owner's check (see the report).
- Benchmark numbers are in `bench/RESULTS.md`; the targets of the test plan are not all met (see the report).

## Third-party licenses

MIT for this project. Bundled: Real-ESRGAN model (BSD-3-Clause; check the model-file terms before selling), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), the OpenMP runtime `vcomp140.dll` (Microsoft redistributable). **No Potrace (GPL).** The full list of linked crates is in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) and in the app (button **Lisensi**).

Test image note: some benchmark images carry a stock-photo watermark. They are used for testing only and are not part of this repository or of the app.
