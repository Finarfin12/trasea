# Trasea

*Bahasa Indonesia: [README.id.md](README.id.md)*

An **offline desktop vectorization app** that turns flat illustrations, logos, and cel-shaded artwork (PNG/JPG/WEBP/BMP) into clean **SVG, PDF (RGB/CMYK), EPS (RGB/CMYK), and AutoCAD R12 DXF** vectors with tidy nodes — ready for Affinity Designer, Adobe Illustrator, CorelDraw, screen printing, laser cutters, CNC routers, and vinyl plotters. Version **1.1.0 ("Alur Kerja Cepat & Produksi Penuh")**. Local-first, bilingual (Indonesian & English), no account, no cloud, **no network request**.

## What it does

Four local stages plus non-destructive pre-processing, production profiles, designer controls, project workflow, batch folder processing, and automatic watch folders:

1. **Non-destructive pre-processing.** Optional crop and rotation (`-90°`, `+90°`, `180°`, or free angle `-45°..+45°`) before tracing, without modifying the source image.
2. **Thin-line guard.** Dark strokes thinner than a few pixels (eyes, smiles, outlines) are extracted from the image first so cleaning never erases them.
3. **Clean (optional, off by default).** Real-ESRGAN (anime x4, bundled sidecar) removes JPEG noise and scales to 2x. Off: a 3 px median filter runs 20–40x faster. If a GPU lacks Vulkan support, Trasea automatically falls back to Median with a clear notice.
4. **Palette lock / Shading + tracing + lines on top.** Flat colours (k-means in Lab) or Shading/Cel-Shading bands (12–32 Lab colours with bilateral smoothing) are traced with visioncortex (VTracer engine) into compact cubic Béziers, pure `<circle>`/`<ellipse>`/`<rect>` primitives, and optional symmetry enforcement. Thin lines stack cleanly on top as editable open strokes or closed shapes.

Supported outputs:
- **SVG** with named layers (`Latar`, `Bidang warna`, `Garis`, optional `Registrasi`) — plus 1-click **Copy SVG** (`Ctrl+Shift+C`) to clipboard
- **PDF** in **RGB** or pure **DeviceCMYK** (PDF 1.4 `k`/`K` operators, zero RGB operators, pure `0 0 0 1` K=100% black & registration marks)
- **AutoCAD R12 DXF (`AC1009`)** with per-colour layers (`COLOR_01_RRGGBB`, `THIN_LINES`, `REGISTRATION`), native `CIRCLE` entities, CAD Y-axis, and `mm` (96 DPI or custom target width) / `px` units
- **EPS** (*Encapsulated PostScript Level 2 / EPSF-3.0*, pure Rust generator in RGB or CMYK without Ghostscript/Inkscape) — Pro
- **`.ase`** (Adobe Swatch Exchange palette, group `Trasea`, RGB or CMYK)
- **100%-black / 100%-K separation films** (`<name>_film01_RRGGBB.svg`/`.pdf`) — Pro
- **`.trasea`** project files (JSON `schemaVersion: 1`, backwards-compatible with 1.0.0) preserving source link + SHA-256, crop/rotate preprocess, brush strokes, manual palette, merges, and settings

### New in 1.1.0 ("Alur Kerja Cepat & Produksi Penuh")

All 9 new 1.1.0 features are **open without requiring a Pro key**:

- **DXF R12 Export (Laser / CNC / Plotter).** Pure ASCII AutoCAD R12 (`AC1009`) writer from Editor, Batch, Watch Folder, or CLI (`--dxf`, `--dxf-unit mm|px`, `--dxf-width-mm`), defaulting to `mm` at 96 DPI with native `CIRCLE` entities and separated colour layers.
- **CMYK Print Export (PDF, EPS, ASE, Separations).** Switch print colour space between **RGB** and **CMYK** (`--color-space cmyk`). Emits pure DeviceCMYK PDF 1.4 (`k`/`K` only, zero RGB operators), CMYK EPS (`setcmykcolor`), and CMYK `.ase` swatches with a deterministic Pure Black rule (`0 0 0 1` for `#000000` and registration marks).
- **Screen Print Pure Knockout option.** Choose between gap-free **Stacking** or **Pure Knockout** (`--sablon-knockout`, `0..2 pt` trap) in Screen Print mode so inks do not overprint thickly.
- **Watch Folder (Auto-Vectorize).** Local polling watcher (`Batch & Watch` -> **Watch Folder**, or CLI `--watch <in> <out>`) with write-stability debounce, corrupt-file isolation, live event log, and automatic `laporan-trasea.csv`/`.md` updates.
- **Clipboard In & Out.** Paste images/screenshots directly with `Ctrl+V` (or **Paste image**) and copy clean SVG markup with `Ctrl+Shift+C` (or **Copy SVG**) straight into Affinity Designer, Illustrator, Inkscape, or Figma.
- **Instant Outline / Wireframe (`Ctrl+Y`) & Layer Visibility.** Toggle blueprint wireframe preview (`Ctrl+Y`) and hide/show individual colour layers, Thin Lines (`Garis`), and Registration (`Registrasi`) via eye icons without re-tracing. *(Redo shortcut is `Ctrl+Shift+Z`.)*
- **Bilingual UI (Indonesian & English).** Instant `ID | EN` header switch persisted locally in `settings.json`.
- **Non-Destructive Crop & Rotate.** Interactive crop rectangle, quick `-90°` / `+90°` / `180°` buttons, and `-45°..+45°` free-angle straightening saved inside `.trasea` projects without touching the original file.
- **Shading / Cel-Shading Mode.** Dedicated 4th production mode (`--mode shading`) with Lab bilateral pre-smoothing and 12–32 colour bands for softly shaded anime/cel illustrations.

### Earlier milestones

- **1.0.0 ("Siap Dijual"):** Pure EPS export, offline Ed25519 licensing & friendly trial, 100% Offline & Privacy modal, `.trasea` project files, 50-step Undo/Redo, Custom Presets, 1-click sample illustration, continuous thin-line extraction fix.
- **0.5.0 ("Produksi"):** Batch folder processing + CSV/Markdown reports, Screen Print mode (2–8 spot colours, registration marks, `#000000` separation films), Cutting/Embroidery knockout mode, targeted manual palette merge, macOS `icon.icns`.
- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, correction brush (`Kuas Detail`/`Kuas Halus`), interactive palette + 5×5 median eyedropper, node heatmap, node target budget, `PreparedImage` cache.
- **0.3.0 ("Geometri rapi"):** Pure basic-shape detection, symmetry enforcer, per-point corners vs smooth curves, offline text warning, unbroken thin-line hysteresis.

## Use

1. Run the installer (`Trasea_1.1.0_x64-setup.exe`, per-user, no admin needed) and launch Trasea.
2. Switch language (`ID | EN`) anytime in the header.
3. Click **Try sample image**, press `Ctrl+V` to paste from clipboard, drag an image/`.trasea` project/folder onto the window, or click **Open image / project…**.
4. Use **Crop & Rotate**, **Outline (`Ctrl+Y`)**, layer eye toggles, **Production Mode** (**Standard**, **Screen Print**, **Cut / Embroidery**, **Shading**), Custom Presets, brush strokes, or palette (`Ctrl+Z`/`Ctrl+Shift+Z` to undo/redo, `Ctrl+S` to save `.trasea`).
5. Copy SVG (`Ctrl+Shift+C`), export **SVG**, **PDF (RGB/CMYK)**, **DXF (mm/px)**, **Palette .ase**, or run **Watch Folder** freely; activate an offline Pro key in **Privacy & License** to also unlock **EPS**, **Separation Films**, and one-shot **Batch Folder**.

## Build from source

Needs Rust (`x86_64-pc-windows-gnu` + MinGW-w64 in `PATH`, or MSVC; macOS Universal requires `aarch64-apple-darwin` + `x86_64-apple-darwin`), Node 22+, and WebView2 on Windows.

```
npm ci
node scripts/fetch-sidecar.mjs
npx tauri build --target x86_64-pc-windows-gnu
```

Developer tools:

```
npm test                                   # 71 cargo tests: V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09, E-01..E-03, L-01..L-03, P-01..P-02, N-01, F-01..F-10
cargo run --release --bin trasea-cli -- in.jpg out.svg [--pdf out.pdf] [--eps out.eps] [--dxf out.dxf] [--ase out.ase] [--color-space rgb|cmyk] [--dxf-unit mm|px] [--dxf-width-mm 120] [--mode standard|sablon|cutting|shading] [--sablon-knockout] [--crop X,Y,W,H] [--rotate DEG] [--separations dir]
cargo run --release --bin trasea-cli -- --batch <input-dir> <output-dir> [--mode standard|sablon|cutting|shading] [--color-space rgb|cmyk] [--pdf] [--eps] [--dxf] [--ase] [--separations] [--subfolders] [--no-report]
cargo run --release --bin trasea-cli -- --watch <input-dir> <output-dir> [--poll-ms 1000] [--debounce-ms 500] [--dxf] [--pdf] [--color-space cmyk]
cargo run --release --bin trasea-keygen -- init | issue --secret <HEX> --name "..." --id "TR-2026-0001" | verify <KEY>
cargo run --release --bin bench -- --dir <vectorize-bench folder> --sidecar <realesrgan folder>
```

See [docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md) before commercial distribution.

## Offline & Licenses

`trasea-core` and `src-tauri` link zero HTTP/network crates (verified by `V7`, `N-01`, and `F-09`), CSP restricts `connect-src` strictly to local IPC, and WebView2 launches with background networking disabled. License audit: zero GPL/AGPL crates — see [docs/LICENSE-AUDIT-1.1.0.md](docs/LICENSE-AUDIT-1.1.0.md) and [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
