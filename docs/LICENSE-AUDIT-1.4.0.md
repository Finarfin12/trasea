# Audit Lisensi — Trasea 1.4.0

- **Tanggal audit:** 2026-11-24
- **Target build:** `x86_64-pc-windows-gnu` (via `cargo metadata --filter-platform x86_64-pc-windows-gnu`)
- **Total crate Rust pihak ketiga:** **339** (−14 dibanding `1.3.0`; **0 crate baru**)
- **Crate GPL / AGPL / LGPL murni:** **0** (NOL)
- **Crate HTTP / jaringan di pohon build Windows:** **0** (NOL; terverifikasi otomatis oleh tes `V7`, `N-01`, `F-09`, dan `G12-10`)
- **Kesimpulan:** **AMAN UNTUK DISTRIBUSI GRATIS / OPEN-SOURCE** (kode sumber berlisensi MIT, lihat `LICENSE`) dengan syarat menyertakan `THIRD_PARTY_LICENSES.md` di dalam bundle aplikasi (sudah dibundel lewat `src-tauri/tauri.conf.json` -> `bundle.resources`).

---

## 1. Perubahan di 1.4.0: sistem lisensi Pro dihapus

Trasea 1.4.0 dibagikan gratis. Mesin lisensi (`license.rs`, `trasea-keygen`, verifikasi Ed25519, tiga command Tauri, UI aktivasi) dihapus seluruhnya. Dependensi yang hanya dipakai lisensi ikut hilang dari `trasea-core`:

| Dependensi yang dihapus dari `trasea-core` | Crate transitif yang ikut keluar dari build Windows |
|---|---|
| `ed25519-dalek` | `ed25519`, `curve25519-dalek`, `curve25519-dalek-derive`, `signature`, `pkcs8`, `spki`, `der`, `const-oid`, `base64ct`, `subtle`, `zeroize` |
| `getrandom` 0.3 | `getrandom` 0.2.17, `rand_core` 0.6.4 |
| `base64` (hanya dipakai lisensi di core) | — (`base64` 0.22 tetap ada sebagai dependensi `src-tauri` untuk pratinjau PNG) |

`sha2` tetap dipakai (`project.rs` dan sidik jari trace). Tidak ada crate baru. Berkas `license.key` lama di folder konfigurasi pengguna diabaikan (tidak dibaca dan tidak dihapus).

Perubahan non-dependensi: sidik jari trace SHA-256 (`sha2` yang sudah ada) untuk edit path/node, dan aset merek tidak berubah dari 1.3.0.

---

## 2. Komponen Biner, Model, & Library Bundled (Tidak Berubah dari 1.3.0)

| Komponen | Lisensi | Komersial? | Status di Trasea 1.4.0 |
|---|---|---|---|
| **Real-ESRGAN** (`realesrgan-x4plus-anime`) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` & modal **Privasi & Lisensi** |
| **realesrgan-ncnn-vulkan** (v0.2.5.0) | MIT | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **ncnn** (statis di dalam sidecar) | BSD-3-Clause | Ya | Terpenuhi di `THIRD_PARTY_LICENSES.md` |
| **vcomp140.dll** (OpenMP runtime) | Microsoft VC++ Redistributable | Ya | Terpenuhi |
| **VTracer / visioncortex** | MIT | Ya | Terpenuhi (bukan Potrace GPL) |
| **arboard** (v3.7.0) | MIT OR Apache-2.0 | Ya | Terpenuhi |
| **clipboard-win** (v5.4.3) | BSL-1.0 | Ya | Terpenuhi |
| **React / React DOM / Tauri 2** | MIT / Apache-2.0 | Ya | Terpenuhi |

---

## 3. Pemeriksaan Keamanan & Privasi

1. **Zero GPL/AGPL:** Tidak ada satu pun crate berlisensi copyleft kuat (`GPL`, `AGPL`) yang tertaut ke `trasea-core` maupun `trasea`.
2. **Zero Network Crates di Mesin Inti:** `trasea-core` dan `src-tauri` tidak memuat `reqwest`, `hyper`, `ureq`, `curl`, `rustls`, `openssl`, maupun `tokio-tungstenite` (terverifikasi otomatis oleh tes `v7_no_network_code_is_linked`, `n01_offline_guarantee_no_network_crates_and_strict_csp`, `f09_no_network_crates_and_strict_csp_1_1_0`, dan `g12_10_zero_network_zero_gpl_and_strict_csp`).
3. **Tidak ada kunci atau rahasia di repositori:** sistem lisensi sudah dihapus, dan pemindaian seluruh riwayat git tidak menemukan kunci privat, token, atau kredensial (hasil di `LAPORAN-1.4.0.md`).
