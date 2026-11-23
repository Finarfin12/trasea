# Audit Lisensi Komersial — Trasea 1.1.0

- **Tanggal audit:** 2026-11-23
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **353** (+8 crate dari `1.0.0` karena tambahan `arboard 3.7.0` untuk Clipboard lokal Windows/macOS/Linux)
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows:** **0** (NOL; terverifikasi juga lewat tes `V7`, `N-01`, dan `F-09`)
- **Kesimpulan komersial:** **AMAN UNTUK DISTRIBUSI KOMERSIAL / OPEN-SOURCE** dengan syarat menyertakan `THIRD_PARTY_LICENSES.md` di dalam bundle aplikasi (sudah dibundel otomatis lewat `src-tauri/tauri.conf.json` -> `bundle.resources`).

---

## 1. Biner, Model, & Komponen Baru di 1.1.0

| Komponen | Lisensi | Komersial? | Kewajiban Redistribusi | Status di Trasea 1.1.0 |
|---|---|---|---|---|
| **Real-ESRGAN** (`realesrgan-x4plus-anime`) | BSD-3-Clause | Ya | Sertakan hak cipta & disclaimer BSD-3-Clause; jangan pakai nama penulis untuk endorsement | Terpenuhi di `THIRD_PARTY_LICENSES.md` & modal **Privasi & Lisensi** |
| **realesrgan-ncnn-vulkan** (v0.2.5.0) | MIT | Ya | Sertakan hak cipta & teks MIT | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **ncnn** (statis di dalam sidecar) | BSD-3-Clause | Ya | Sertakan hak cipta & teks BSD-3-Clause Tencent | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **vcomp140.dll** (OpenMP runtime) | Microsoft VC++ Redistributable | Ya | Boleh didistribusikan bersama aplikasi Windows sesuai ketentuan Microsoft | Terpenuhi |
| **VTracer / visioncortex** | MIT | Ya | Sertakan teks MIT (bukan Potrace GPL) | Terpenuhi |
| **ed25519-dalek** (v2.2.0) | BSD-3-Clause | Ya | Sertakan teks BSD-3-Clause | Terpenuhi |
| **arboard** (v3.7.0, *baru di 1.1.0*) | MIT OR Apache-2.0 | Ya | Sertakan atribusi MIT/Apache-2.0 (clipboard lokal sistem, nol jaringan) | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **clipboard-win** (v5.4.3, transitif `arboard`) | BSL-1.0 | Ya | Lisensi Boost permisif tanpa kewajiban redistribusi biner | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **sample-illustration.png** | Karya sintetis internal Trasea | Ya | Bebas hak cipta pihak ketiga (dibuat 100% programatik untuk Trasea) | Terpenuhi |

Catatan: Modul baru `trasea-core` (`color.rs`, `dxf.rs`, `preprocess.rs`, `watch.rs`, PDF 1.4 DeviceCMYK, dan mode Shading Lab bilateral) seluruhnya ditulis murni di Rust tanpa crate eksternal baru di `crates/trasea-core/Cargo.toml` (Watch Folder memakai polling `std::fs`, DXF & CMYK PDF ditulis murni).

---

## 2. Dependensi Frontend (`package.json` runtime)

| Paket | Versi | Lisensi |
|---|---|---|
| @tauri-apps/api | 2.12.5 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-dialog | 2.8.4 | MIT OR Apache-2.0 |
| react | 19.3.1 | MIT |
| react-dom | 19.3.1 | MIT |

---

## 3. Ringkasan Distribusi Lisensi Crate Rust (353 crate)

| Ekspresi Lisensi (SPDX) | Jumlah Crate |
|---|---:|
| MIT OR Apache-2.0 | 159 |
| MIT | 56 |
| Apache-2.0 OR MIT | 44 |
| Unicode-3.0 | 18 |
| MIT/Apache-2.0 | 15 |
| Unlicense OR MIT | 13 |
| BSD-3-Clause | 7 |
| MIT OR Apache-2.0 OR Zlib | 6 |
| MPL-2.0 | 5 |
| Zlib OR Apache-2.0 OR MIT | 3 |
| Zlib | 3 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| BSD-3-Clause OR Apache-2.0 | 2 |
| Apache-2.0 | 2 |
| BSD-2-Clause | 2 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 2 |
| BSL-1.0 | 2 |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 |
| Unlicense/MIT | 2 |
| Apache-2.0 OR BSL-1.0 | 1 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| Apache-2.0 AND MIT | 1 |
| BSD-3-Clause/MIT | 1 |
| BSD-3-Clause AND MIT | 1 |
| Apache-2.0 / MIT | 1 |
| (Apache-2.0 OR MIT) AND BSD-3-Clause | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |

---

## 4. Pemeriksaan Keamanan & Privasi

1. **Zero GPL/AGPL:** Tidak ada satu pun crate berlisensi copyleft kuat (`GPL`, `AGPL`) yang tertaut ke `trasea-core` maupun `trasea`.
2. **Zero Network Crates:** `cargo metadata --filter-platform x86_64-pc-windows-gnu` untuk `trasea-core` dan `trasea` tidak memuat `reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, maupun `tokio-tungstenite` (dijaga otomatis oleh tes `v7_no_network_code_is_linked`, `n01_offline_guarantee_no_network_crates_and_strict_csp`, dan `f09_no_network_crates_and_strict_csp_1_1_0`).
3. **Kunci privat Ed25519 tidak ada di repo:** Hanya *public key* 32-byte yang tertanam di `crates/trasea-core/src/license.rs`; `trasea-keygen` adalah binary terpisah yang tidak masuk konfigurasi bundle Tauri.
