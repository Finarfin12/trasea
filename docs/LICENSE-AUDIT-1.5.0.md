# Audit Lisensi — Trasea 1.5.0

- **Tanggal audit:** 2026-11-24
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **339** (0 crate baru dibanding `1.4.0`)
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows (`trasea-core` & `src-tauri`):** **0** (NOL; terverifikasi otomatis oleh tes `V7`, `N-01`, `F-09`, `G12-10`, `G13-10`, dan `G15-07`)
- **Kesimpulan:** **AMAN UNTUK DISTRIBUSI PUBLIK GRATIS / OPEN-SOURCE (MIT)** dengan menyertakan `THIRD_PARTY_LICENSES.md` dan `LICENSE` di dalam installer NSIS maupun paket portable `.zip`.

---

## 1. Perubahan Dependensi di 1.5.0

**Tidak ada crate baru yang ditambahkan** ke `trasea-core` maupun `src-tauri`.

- **Fase 1 (H2 — `join_stroke_endpoints`) & Fase 2 (`StickerBacking` / Bleed):** murni Rust di atas `trasea-core` yang sudah ada.
- **Fase 3 (Cek Pembaruan Ringan `updater.rs` & Mode Portable):** menggunakan utilitas sistem OS `curl` (bawaan Windows 10/11) dengan timeout ketat 5 dtk koneksi / 8 dtk total, 0 cookie, 0 telemetri, dan parser `serde_json` yang sudah ada. WebView2 tetap terisolasi penuh (CSP & `additionalBrowserArgs` tidak dilonggarkan).

---

## 2. Komponen Biner, Model, & Library Bundled (Tidak Berubah dari 1.4.0)

| Komponen | Lisensi | Komersial? | Status di Trasea 1.5.0 |
|---|---|---|---|
| **Real-ESRGAN** (`realesrgan-x4plus-anime`) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **realesrgan-ncnn-vulkan** (v0.2.5.0) | MIT | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **ncnn** (statis di dalam sidecar) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **vcomp140.dll** (OpenMP runtime) | Microsoft VC++ Redistributable | Ya | Terpenuhi |
| **VTracer / visioncortex** | MIT | Ya | Terpenuhi (bukan Potrace GPL) |
| **arboard** (v3.7.0) | MIT OR Apache-2.0 | Ya | Terpenuhi |
| **clipboard-win** (v5.4.3) | BSL-1.0 | Ya | Terpenuhi |
| **React / React DOM / Tauri 2** | MIT / Apache-2.0 | Ya | Terpenuhi |

---

## 3. Pemeriksaan Keamanan & Privasi

1. **Zero GPL/AGPL:** 0 crate GPL/AGPL tertaut di `trasea-core` maupun `trasea`.
2. **Zero Network Crates:** 0 crate HTTP/TLS/jaringan (`reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, `tokio-tungstenite`) di `trasea-core` maupun `src-tauri`.
3. **Zero Rahasia:** 0 kredensial, kunci privat, atau token di dalam repositori.
