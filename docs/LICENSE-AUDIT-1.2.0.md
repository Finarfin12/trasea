# Audit Lisensi Komersial — Trasea 1.2.0

- **Tanggal audit:** 2026-11-23
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **353** (+0 crate baru dibanding `1.1.0` — seluruh 8 fitur baru 1.2.0 ditulis murni di Rust/React tanpa dependensi pihak ketiga tambahan)
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows:** **0** (NOL; terverifikasi otomatis oleh tes `V7`, `N-01`, `F-09`, dan `G12-10`)
- **Kesimpulan komersial:** **AMAN UNTUK DISTRIBUSI KOMERSIAL / OPEN-SOURCE** dengan syarat menyertakan `THIRD_PARTY_LICENSES.md` di dalam bundle aplikasi (sudah dibundel otomatis lewat `src-tauri/tauri.conf.json` -> `bundle.resources`).

---

## 1. Modul & Fitur Baru di 1.2.0 (Nol Crate Baru)

Seluruh fitur baru Trasea 1.2.0 ("Stiker, Laser, & Presisi RIP") ditulis murni tanpa menambah crate pihak ketiga di `crates/trasea-core/Cargo.toml`, `src-tauri/Cargo.toml`, maupun `package.json`:

| Fitur 1.2.0 | Implementasi | Dependensi Eksternal Baru |
|---|---|---|
| **F1 — Die-Cut / Kiss-Cut Contour (`CutContour`)** | `crates/trasea-core/src/contour.rs` (Felzenszwalb-Huttenlocher 2D EDT + Marching Squares + Chaikin + Bézier), SVG/PDF/EPS/DXF/ASE | Tidak ada (0) |
| **F2 — Mode Garis Tengah (`Centerline`)** | `crates/trasea-core/src/centerline.rs` + `pipeline.rs` + `dxf.rs` (threshold adaptif + Zhang-Suen thinning + pruning + kurva terbuka) | Tidak ada (0) |
| **F3 — Hapus Latar Otomatis & Defringe** | `crates/trasea-core/src/preprocess.rs` (border flood-fill 4-arah CIE Lab ΔE + defringe 0..3 px) | Tidak ada (0) |
| **F4 — Pilih & Edit Path di Kanvas** | `crates/trasea-core/src/path_edit.rs` (`data-trasea-id`, `Delete`/`ReassignColor`) + `src/App.tsx` | Tidak ada (0) |
| **F5 — Soft-Proof CMYK & Peringatan Gamut** | `crates/trasea-core/src/color.rs` (Neugebauer 8-primaries + ΔE out-of-gamut) + `src/App.tsx` | Tidak ada (0) |
| **F6 — Named Spot Color (PDF/EPS/ASE)** | `crates/trasea-core/src/color.rs` + `export.rs` (`/Separation`, `%%CMYKCustomColor`, ASE `0x0001`) | Tidak ada (0) |
| **F7 — Modal Pengaturan 4 Tab (`settings.json`)** | `src-tauri/src/main.rs` + `src/SettingsModal.tsx` (Umum, Default Ekspor, Performa & GPU, Pintasan Keyboard) | Tidak ada (0) |
| **F8 — Inspektor Kode Warna & Pustaka Palet** | `src/colorUtils.ts` + `src/App.tsx` + `palettes.json` + ekspor `.gpl`/`.json`/`.css` | Tidak ada (0) |

---

## 2. Komponen Biner, Model, & Library Bundled (Tidak Berubah dari 1.1.0)

| Komponen | Lisensi | Komersial? | Status di Trasea 1.2.0 |
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
