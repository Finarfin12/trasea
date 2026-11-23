# Trasea

*Bahasa Indonesia: [README.id.md](README.id.md)*

An **offline Windows desktop app** that turns flat illustrations or logos (PNG/JPG/WEBP/BMP) into clean **SVG or PDF** vectors with tidy nodes, ready for Affinity Designer, Adobe Illustrator, screen printing, and cutting plotters. Version **0.5.0 ("Produksi")**. Local repository, no server, no account, **no network request**.

## What it does

Four stages (all on your computer), plus production profiles, designer controls, and batch folder processing:

1. **Thin-line guard.** Dark strokes thinner than a few pixels (eyes, smiles, outlines) are found in the ORIGINAL image first, because cleaning steps tend to erase them.
2. **Clean (optional, off by default).** Real-ESRGAN (anime x4, bundled sidecar) removes JPEG noise and scales to 2x. Off: a 3 px median filter runs 20–40x faster and is usually just as good.
3. **Palette lock + tracing.** Flat colours (k-means in Lab, edge pixels excluded so no halo colours appear; low-contrast specks merged away) traced with visioncortex (VTracer engine) into compact cubic Beziers.
4. **Lines on top.** Each thin line becomes an editable open stroke (centerline + measured `stroke-width`) or a closed filled outline depending on export/production mode.

Export SVG layer layout:

```
Latar            background colour (omitted when "Hapus latar belakang" is active)
Bidang warna     one sub-group per colour: id="Warna_02_EBD9C1", data-name="Warna 02 #EBD9C1"
Garis            thin lines (editable strokes or closed filled shapes)
Registrasi       4 corner registration marks (optional, default on in Screen Print mode)
```

Also exports **PDF** (page size = drawing size), **.ase** (Adobe Swatch Exchange, group `Trasea`, swatches `Trasea 01 #RRGGBB`), and **100%-black separation films** (`<name>_film01_RRGGBB.svg`/`.pdf`).

### New in 0.5.0 ("Produksi")

- **Batch folder processing + per-file reports.** Process an entire folder (with optional 1-level subfolders) or drag-and-drop multiple files/folders onto the window. Runs sequentially when AI cleaning is enabled and with bounded parallelism (`min(cpu/2, 4)`) otherwise. Isolates corrupt files without stopping the batch, avoids filename collisions (`logo_2.svg`), supports clean cancellation, exports `laporan-trasea.csv` and `laporan-trasea.md`, and lets you click any report row to open that file directly in the single-image editor.
- **Screen Print mode (`Sablon`).** Caps spot colours to 2–8 (default 4; warns above 6), enables 0.5 pt print trap, absorbs micro-specks, thickens thin strokes to at least `min_stroke_pt` (default 1.0 pt), adds 4 corner registration marks on layer `Registrasi`, and exports per-colour 100%-black (`#000000`) separation film SVGs/PDFs with matching registration marks and film labels.
- **Cutting Sticker / Embroidery mode (`Cutting / Bordir`).** Non-overlapping knockout tracing (no stacked layers underneath), strips the background layer by default (toggleable), turns all thin lines into closed filled shapes thickened to `min_stroke_pt` (default 1.5 pt), filters small weeding regions, nudges smoothness to 70, and locks print trap to `0 pt`.
- **0.4.0 polish.** Manual palette **Gabung** lets you pick any target colour (suggesting the nearest colour first) and remaps merged pixels to that exact target; macOS `icon.icns` updated to the Trasea `T` logo.

### Earlier milestones

- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, correction brush (`Kuas Detail` / `Kuas Halus`), interactive manual palette + 5×5 median eyedropper, node density heatmap (off by default, preview only), node budget target (`--target-nodes`), in-memory `PreparedImage` cache for fast iterations, hidden-duplicate contour removal.
- **0.3.0 ("Geometri rapi"):** Pure `<circle>`/`<ellipse>`/`<rect>` basic-shape detection, symmetry enforcer, per-point sharp corners vs smooth curves, offline text-area warning + preview highlight, unbroken thin-line hysteresis.

## Use

1. Run the installer (`Trasea_0.5.0_x64-setup.exe`, per-user, no admin needed) and start Trasea.
2. Drag an image onto the window (or click **Buka gambar…**), or click **Batch folder** (or drop a folder/multiple images) to process a batch.
3. Choose a **Mode Produksi** (**Standar**, **Sablon**, or **Cutting / Bordir**), adjust controls, and press **Terapkan ulang** if needed.
4. Export **SVG**, **PDF**, **Palet .ase**, or (in Sablon mode) **Ekspor film separasi…**.

The desktop interface is in Indonesian.

## Build from source

Needs Rust (`x86_64-pc-windows-gnu` toolchain + MinGW-w64 in `PATH`, or MSVC toolchain), Node 22+, and WebView2 (included in Windows 11).

```
npm ci
node scripts/fetch-sidecar.mjs          # downloads Real-ESRGAN ncnn Vulkan v0.2.5.0 into src-tauri/resources/realesrgan
npx tauri build --target x86_64-pc-windows-gnu   # installer in target/<triple>/release/bundle/nsis/
```

Developer tools:

```
npm test                                   # cargo tests V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09 (synthetic images)
cargo run --release --bin trasea-cli -- in.jpg out.svg [--mode standard|sablon|cutting] [--pdf out.pdf] [--ase out.ase] [--separations dir] [--colors 8] [--smooth 0.5] [--min-stroke 1.0] [--reg-marks|--no-reg-marks] [--strip-bg|--keep-bg] [--ai] [--no-lines] [--lines stroke|shape] [--trap 0.5] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--target-nodes N] [--palette "#RRGGBB,#RRGGBB"] [--png out.png] [--sidecar DIR]
cargo run --release --bin trasea-cli -- --batch <input-dir> <output-dir> [--mode standard|sablon|cutting] [--subfolders] [--pdf] [--ase] [--separations] [--no-report] [options...]
cargo run --release --bin bench -- --dir <vectorize-bench folder> --sidecar <realesrgan folder>   # writes bench/RESULTS.md
```

The core is the crate `crates/trasea-core` (no UI, no network). The app in `src-tauri` is a thin shell; the UI is React + TypeScript in `src`.

## Offline

The core and app contain no network code (verified with `cargo tree` in a test), the page can only talk to local IPC (CSP), and WebView2 starts with background networking, updates, sync, and SmartScreen disabled, non-local traffic sent to a dead local proxy (`127.0.0.1:9`), and non-local DNS blocked. Trasea never sends image data anywhere.

## Limits of 0.5.0

- Windows x64 installer only (macOS `.icns` icon is updated, full macOS build planned for 1.0.0). Real-ESRGAN needs a Vulkan GPU; otherwise median fallback is used.
- Inputs are raster images (PNG, JPG, WEBP, BMP), longest side limited to 1600 px. Photos are not the target.
- Thin lines detect **dark** strokes only; light strokes on dark areas are traced as colour fields.
- Cutting / Embroidery mode exports clean knockout SVG/PDF ready for plotting/digitizing apps (Cricut, Silhouette, Hatch, Wilcom, Affinity, Illustrator); binary stitch formats (`.dst`, `.pes`, `.jef`) and EPS are out of scope for 0.5.0.
- Opening exported SVG/PDF/`.ase`/separation films in Affinity Designer, Adobe Illustrator, or Cricut Design Space is a manual owner check (`M-01`..`M-03`).
- Text detection is a geometric heuristic (no OCR) and never modifies the drawing.

## Third-party licenses

MIT for this project. Bundled: Real-ESRGAN model (BSD-3-Clause), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), OpenMP runtime `vcomp140.dll` (Microsoft redistributable). **No Potrace (GPL).** Full crate list in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) and inside the app (**Lisensi**).