# Audit Lisensi Komersial — Trasea 1.0.0

- **Tanggal audit:** 2026-11-23
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **345**
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows:** **0** (NOL; terverifikasi juga lewat `V7` & `N-01`)
- **Kesimpulan komersial:** **AMAN UNTUK DISTRIBUSI KOMERSIAL** dengan syarat menyertakan `THIRD_PARTY_LICENSES.md` di dalam bundle aplikasi (sudah dibundel otomatis lewat `src-tauri/tauri.conf.json` -> `bundle.resources`).

---

## 1. Biner & Model Bawaan (Sidecar)

| Komponen | Lisensi | Komersial? | Kewajiban Redistribusi | Status di Trasea 1.0.0 |
|---|---|---|---|---|
| **Real-ESRGAN** (`realesrgan-x4plus-anime`) | BSD-3-Clause | Ya | Sertakan hak cipta & disclaimer BSD-3-Clause; jangan pakai nama penulis untuk endorsement | Terpenuhi di `THIRD_PARTY_LICENSES.md` & modal **Privasi & Lisensi** |
| **realesrgan-ncnn-vulkan** (v0.2.5.0) | MIT | Ya | Sertakan hak cipta & teks MIT | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **ncnn** (statis di dalam sidecar) | BSD-3-Clause | Ya | Sertakan hak cipta & teks BSD-3-Clause Tencent | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **vcomp140.dll** (OpenMP runtime) | Microsoft VC++ Redistributable | Ya | Boleh didistribusikan bersama aplikasi Windows sesuai ketentuan Microsoft | Terpenuhi |
| **VTracer / visioncortex** | MIT | Ya | Sertakan teks MIT (bukan Potrace GPL) | Terpenuhi |
| **ed25519-dalek** (v2.2.0) | BSD-3-Clause | Ya | Sertakan teks BSD-3-Clause | Terpenuhi |
| **sample-illustration.png** | Karya sintetis internal Trasea | Ya | Bebas hak cipta pihak ketiga (dibuat 100% programatik untuk Trasea) | Terpenuhi |

---

## 2. Dependensi Frontend (`package.json` runtime)

| Paket | Versi | Lisensi |
|---|---|---|
| @tauri-apps/api | 2.12.5 | Apache-2.0 OR MIT |
| @tauri-apps/plugin-dialog | 2.8.4 | MIT OR Apache-2.0 |
| react | 19.3.1 | MIT |
| react-dom | 19.3.1 | MIT |

---

## 3. Ringkasan Distribusi Lisensi Crate Rust (345 crate)

| Ekspresi Lisensi (SPDX) | Jumlah Crate |
|---|---:|
| MIT OR Apache-2.0 | 157 |
| MIT | 54 |
| Apache-2.0 OR MIT | 44 |
| Unicode-3.0 | 18 |
| MIT/Apache-2.0 | 15 |
| Unlicense OR MIT | 13 |
| BSD-3-Clause | 7 |
| MIT OR Apache-2.0 OR Zlib | 6 |
| MPL-2.0 | 5 |
| Zlib OR Apache-2.0 OR MIT | 3 |
| Zlib | 3 |
| BSD-2-Clause | 2 |
| Apache-2.0 | 2 |
| MIT OR Zlib OR Apache-2.0 | 2 |
| BSD-3-Clause OR Apache-2.0 | 2 |
| Unlicense/MIT | 2 |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 2 |
| 0BSD OR MIT OR Apache-2.0 | 1 |
| BSD-3-Clause AND MIT | 1 |
| BSD-3-Clause/MIT | 1 |
| Apache-2.0 AND MIT | 1 |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 |
| (Apache-2.0 OR MIT) AND BSD-3-Clause | 1 |
| Apache-2.0 / MIT | 1 |
| Apache-2.0 OR BSL-1.0 | 1 |

---

## 4. Pemeriksaan Keamanan & Privasi

1. **Zero GPL/AGPL:** Tidak ada satu pun crate berlisensi copyleft kuat (`GPL`, `AGPL`) yang tertaut ke `trasea-core` maupun `trasea`.
2. **Zero Network Crates:** `cargo tree` untuk `trasea-core` dan `trasea` tidak memuat `reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, maupun `tokio-tungstenite` (dijaga otomatis oleh tes `v7_no_network_code_is_linked` dan `n01_offline_guarantee_no_network_crates_and_strict_csp`).
3. **Kunci privat Ed25519 tidak ada di repo:** Hanya *public key* 32-byte yang tertanam di `crates/trasea-core/src/license.rs`; `trasea-keygen` adalah binary terpisah yang tidak masuk konfigurasi bundle Tauri.
