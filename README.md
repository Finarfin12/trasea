# Trasea

*Bahasa Indonesia: [README.id.md](README.id.md)*

An **offline desktop vectorization app** that turns flat illustrations and logos (PNG/JPG/WEBP/BMP) into clean **SVG, PDF, and EPS** vectors with tidy nodes — ready for Affinity Designer, Adobe Illustrator, CorelDraw, screen printing, and cutting plotters. Version **1.0.0 ("Siap Dijual")**. Local-first, one-time offline license, no account, no cloud, **no network request**.

## What it does

Four local stages plus production profiles, designer controls, project workflow, and batch folder processing:

1. **Thin-line guard.** Dark strokes thinner than a few pixels (eyes, smiles, outlines) are extracted from the ORIGINAL image first so cleaning never erases them.
2. **Clean (optional, off by default).** Real-ESRGAN (anime x4, bundled sidecar) removes JPEG noise and scales to 2x. Off: a 3 px median filter runs 20–40x faster. If a GPU lacks Vulkan support, Trasea automatically falls back to Median with a clear Indonesian notice.
3. **Palette lock + tracing.** Flat colours (k-means in Lab, edge pixels excluded) traced with visioncortex (VTracer engine) into compact cubic Béziers, plus pure `<circle>`/`<ellipse>`/`<rect>` recognition and symmetry enforcement.
4. **Lines on top.** Thin lines become editable open strokes (centerline + measured `stroke-width`) or closed filled shapes depending on export/production mode.

Supported outputs:
- **SVG** with named layers (`Latar`, `Bidang warna`, `Garis`, optional `Registrasi`)
- **PDF** (page size = drawing size)
- **EPS** (*Encapsulated PostScript Level 2 / EPSF-3.0*, pure Rust generator without Ghostscript/Inkscape) — Pro
- **`.ase`** (Adobe Swatch Exchange palette, group `Trasea`)
- **100%-black separation films** (`<name>_film01_RRGGBB.svg`/`.pdf`) — Pro
- **`.trasea`** project files (JSON `schemaVersion: 1`) preserving source link + SHA-256, brush strokes, manual palette, merges, and settings

### New in 1.0.0 ("Siap Dijual")

- **Pure EPS export.** Export clean EPSF-3.0 files from the Editor, Batch Folder (`EPS` checkbox), or CLI (`--eps`), with Bézier conversion of basic shapes, print trap, strokes/shapes, and registration marks.
- **Offline Ed25519 license & friendly trial.** Single-image editor and SVG/PDF/ASE exports are free to try without watermarks. Offline `TRSA1-...` keys (verified locally via Ed25519, zero network) unlock **Batch Folder**, **EPS export**, and **Screen Print separation films**. Includes owner CLI `trasea-keygen` (not bundled in the installer).
- **100% Offline & Privacy modal.** View license status, activate/deactivate offline keys, read the zero-telemetry privacy pledge, and browse complete third-party licenses.
- **`.trasea` project files.** Save (`Ctrl+S` / `Ctrl+Shift+S`) and reopen (`Ctrl+O` or drag-and-drop) complete sessions including brush strokes and manual palette merges, with friendly source-image relinking if moved.
- **Full Undo / Redo (`Ctrl+Z` / `Ctrl+Y`).** 50-step history across brush strokes, palette edits, production modes, and settings.
- **Custom Presets.** Save and reuse your own parameter presets across both the single-image Editor and Batch Folder view.
- **1-click sample illustration.** Click **Coba contoh gambar** on the welcome screen to try Trasea immediately with a bundled copyright-free sample.
- **Commercial release readiness.** Full dependency license audit ([docs/LICENSE-AUDIT-1.0.0.md](docs/LICENSE-AUDIT-1.0.0.md)), complete BSD-3-Clause & MIT texts in [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md), macOS Universal config (`icon.icns` + `bundle.macOS`), and signing guide ([docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md)).

### Earlier milestones

- **0.5.0 ("Produksi"):** Batch folder processing + CSV/Markdown reports, Screen Print mode (2–8 spot colours, registration marks, `#000000` separation films), Cutting/Embroidery knockout mode, targeted manual palette merge, macOS `icon.icns`.
- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, correction brush (`Kuas Detail`/`Kuas Halus`), interactive palette + 5×5 median eyedropper, node heatmap, node target budget, `PreparedImage` cache.
- **0.3.0 ("Geometri rapi"):** Pure basic-shape detection, symmetry enforcer, per-point corners vs smooth curves, offline text warning, unbroken thin-line hysteresis.

## Use

1. Run the installer (`Trasea_1.0.0_x64-setup.exe`, per-user, no admin needed) and launch Trasea.
2. Click **Coba contoh gambar**, drag an image/`.trasea` project/folder onto the window, or click **Buka gambar / proyek…**.
3. Adjust **Mode Produksi** (**Standar**, **Sablon**, **Cutting / Bordir**), Custom Presets, brush strokes, or palette (use `Ctrl+Z`/`Ctrl+Y` to undo/redo and `Ctrl+S` to save `.trasea`).
4. Export **SVG**, **PDF**, **Palet .ase**, or (after offline Pro activation in **Privasi & Lisensi**) **EPS**, **Film Separasi**, and **Batch Folder**.

The desktop interface is in Indonesian.

## Build from source

Needs Rust (`x86_64-pc-windows-gnu` + MinGW-w64 in `PATH`, or MSVC; macOS Universal requires `aarch64-apple-darwin` + `x86_64-apple-darwin`), Node 22+, and WebView2 on Windows.

```
npm ci
node scripts/fetch-sidecar.mjs
npx tauri build --target x86_64-pc-windows-gnu
```

Developer tools:

```
npm test                                   # cargo tests V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09, E-01..E-03, L-01..L-03, P-01..P-02, N-01
cargo run --release --bin trasea-cli -- in.jpg out.svg [--pdf out.pdf] [--eps out.eps] [--ase out.ase] [--mode standard|sablon|cutting] [--separations dir] [options...]
cargo run --release --bin trasea-cli -- --batch <input-dir> <output-dir> [--mode standard|sablon|cutting] [--pdf] [--eps] [--ase] [--separations] [--subfolders] [--no-report]
cargo run --release --bin trasea-keygen -- init | issue --secret <HEX> --name "..." --id "TR-2026-0001" | verify <KEY>
cargo run --release --bin bench -- --dir <vectorize-bench folder> --sidecar <realesrgan folder>
```

See [docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md) before commercial distribution.

## Offline & Licenses

`trasea-core` links zero HTTP/network crates (verified by `V7` and `N-01`), CSP restricts `connect-src` strictly to local IPC, and WebView2 launches with background networking disabled. License audit: zero GPL/AGPL crates — see [docs/LICENSE-AUDIT-1.0.0.md](docs/LICENSE-AUDIT-1.0.0.md) and [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
