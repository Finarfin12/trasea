# Audit Lisensi Komersial — Trasea 1.3.0

- **Tanggal audit:** 2026-11-24
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **353** (+0 crate baru dibanding `1.2.0` — Alat Node, edit node non-destruktif, dan impor SVG ditulis murni di Rust/React tanpa dependensi pihak ketiga tambahan)
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows:** **0** (NOL; terverifikasi otomatis oleh tes `V7`, `N-01`, `F-09`, dan `G12-10`)
- **Kesimpulan komersial:** **AMAN UNTUK DISTRIBUSI KOMERSIAL / OPEN-SOURCE** dengan syarat menyertakan `THIRD_PARTY_LICENSES.md` di dalam bundle aplikasi (sudah dibundel otomatis lewat `src-tauri/tauri.conf.json` -> `bundle.resources`).

---

## 1. Modul & Fitur Baru di 1.3.0 (Nol Crate Baru)

| Fitur 1.3.0 | Implementasi | Dependensi Eksternal Baru |
|---|---|---|
| **Inspeksi & edit node (`MoveNode`, `MoveHandle`, `DeleteNode`, `ToggleSmooth`, `SimplifyPath`)** | `crates/trasea-core/src/node_edit.rs` (memakai `fit.rs` yang sudah ada) | Tidak ada (0) |
| **Impor SVG ke `Doc` yang bisa diedit** | `crates/trasea-core/src/svg_import.rs` + `pipeline.rs` (`vectorize_svg`) | Tidak ada (0) |
| **Alat Node di kanvas (overlay 60 fps)** | `src/NodeOverlay.tsx` + `src/App.tsx` + perintah Tauri `inspect_path_nodes` | Tidak ada (0) |
| **Penyimpanan `.trasea` dengan `nodeEdits`** | `crates/trasea-core/src/project.rs` (`#[serde(default)]`, kompatibel mundur) | Tidak ada (0) |
| **Identitas merek baru (ikon .ico/.icns/.png, logo header, banner)** | Aset SVG/PNG turunan di `docs/branding/`, `src-tauri/icons/`, `src/assets/` | Tidak ada (0) di runtime |

Catatan aset merek: wordmark pada banner dikonversi dari teks Inter menjadi path (glyph outline). Inter berlisensi **SIL Open Font License 1.1**, yang mengizinkan penyematan/turunan outline; tidak ada file font yang dibundel. Alat bantu pembuatan aset (`sharp`, `opentype.js`) hanya dipakai di mesin pengembang dan tidak masuk bundle maupun `package.json`.

---

## 2. Komponen Biner, Model, & Library Bundled (Tidak Berubah dari 1.2.0)

| Komponen | Lisensi | Komersial? | Status di Trasea 1.3.0 |
|---|---|---|---|
| **Real-ESRGAN** (`realesrgan-x4plus-anime`) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` & modal **Privasi & Lisensi** |
| **realesrgan-ncnn-vulkan** (v0.2.5.0) | MIT | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **ncnn** (statis di dalam sidecar) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **vcomp140.dll** (OpenMP runtime) | Microsoft VC++ Redistributable | Ya | Terpenuhi |
| **VTracer / visioncortex** | MIT | Ya | Terpenuhi (bukan Potrace GPL) |
| **ed25519-dalek** (v2.2.0) | BSD-3-Clause | Ya | Terpenuhi |
| **arboard** (v3.7.0) | MIT OR Apache-2.0 | Ya | Terpenuhi |
| **clipboard-win** (v5.4.3) | BSL-1.0 | Ya | Terpenuhi |
| **React / React DOM / Tauri 2** | MIT / Apache-2.0 | Ya | Terpenuhi |

---

## 3. Pemeriksaan Keamanan & Privasi

1. **Zero GPL/AGPL:** Tidak ada satu pun crate berlisensi copyleft kuat (`GPL`, `AGPL`) yang tertaut ke `trasea-core` maupun `trasea`.
2. **Zero Network Crates di Mesin Inti:** `trasea-core` dan `src-tauri` tidak memuat `reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, maupun `tokio-tungstenite` (terverifikasi otomatis oleh tes `v7_no_network_code_is_linked`, `n01_offline_guarantee_no_network_crates_and_strict_csp`, `f09_no_network_crates_and_strict_csp_1_1_0`, dan `g12_10_zero_network_zero_gpl_and_strict_csp`).
3. **Kunci privat Ed25519 tidak ada di repo:** Hanya *public key* 32-byte yang tertanam di `crates/trasea-core/src/license.rs`; `trasea-keygen` adalah binary terpisah yang tidak masuk konfigurasi bundle Tauri.
