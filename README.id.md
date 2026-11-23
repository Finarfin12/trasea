# Trasea

*English: [README.md](README.md)*

**Aplikasi desktop vektorisasi offline** yang mengubah ilustrasi flat dan logo (PNG/JPG/WEBP/BMP) menjadi **SVG, PDF, dan EPS** bersih dengan node rapi — siap untuk Affinity Designer, Adobe Illustrator, CorelDraw, film sablon, dan mesin cutting/bordir. Versi **1.0.0 ("Siap Dijual")**. Lokal, lisensi bayar sekali offline, tanpa akun, tanpa cloud, **tanpa permintaan jaringan**.

## Cara kerja

Empat tahap lokal ditambah profil produksi, kendali desainer, alur kerja proyek, dan pemrosesan batch folder:

1. **Penjaga garis tipis.** Garis gelap tipis (mata, senyum, outline) diekstrak dari gambar ASLI terlebih dahulu agar tidak hilang saat pembersihan.
2. **Bersihkan (opsional, mati bawaan).** Real-ESRGAN (anime x4, sidecar bawaan) membuang noise JPEG dan menskalakan ke 2x. Mati: filter median 3 px bekerja 20–40x lebih cepat. Jika GPU tidak mendukung Vulkan, Trasea otomatis mundur ke Median dengan pesan yang jelas.
3. **Kunci palet + pelacakan.** Warna rata (k-means di Lab, piksel tepi dikecualikan) dilacak dengan visioncortex (mesin VTracer) menjadi kurva Bézier kubik ringkas, lengkap dengan pengenalan bentuk dasar dan tegas simetri.
4. **Garis di atas.** Garis tipis menjadi stroke terbuka yang mudah diedit atau bentuk isi tertutup sesuai mode produksi/ekspor.

Format keluaran yang didukung:
- **SVG** dengan layer bernama (`Latar`, `Bidang warna`, `Garis`, opsional `Registrasi`)
- **PDF** (ukuran halaman = ukuran gambar)
- **EPS** (*Encapsulated PostScript Level 2 / EPSF-3.0*, generator murni tanpa Ghostscript/Inkscape) — Pro
- **`.ase`** (palet Adobe Swatch Exchange, grup `Trasea`)
- **Film separasi hitam 100%** (`<nama>_film01_RRGGBB.svg`/`.pdf`) — Pro
- **`.trasea`** file proyek (JSON `schemaVersion: 1`) menyimpan tautan gambar sumber + SHA-256, goresan kuas, palet manual, daftar gabung warna, dan seluruh pengaturan

### Baru di 1.0.0 ("Siap Dijual")

- **Ekspor EPS murni.** Ekspor file EPSF-3.0 rapi dari Editor, Batch Folder (centang `EPS`), atau CLI (`--eps`), mendukung kurva Bézier bentuk dasar, trap cetak, stroke/outline, dan tanda registrasi.
- **Lisensi offline Ed25519 & trial ramah.** Editor tunggal serta ekspor SVG/PDF/ASE bebas dicoba tanpa watermark. Kunci offline `TRSA1-...` (verifikasi lokal Ed25519, tanpa internet) membuka **Batch Folder**, **Ekspor EPS**, dan **Film Separasi Sablon**. Dilengkapi CLI internal `trasea-keygen` (tidak dibundel ke installer).
- **Modal Privasi & Lisensi.** Menampilkan status lisensi & aktivasi offline, janji 100% offline & bebas telemetri, serta teks lengkap lisensi pihak ketiga.
- **File proyek `.trasea`.** Simpan (`Ctrl+S` / `Ctrl+Shift+S`) dan buka kembali (`Ctrl+O` atau seret-lepas) seluruh pekerjaan termasuk goresan kuas dan palet manual, lengkap dengan dialog taut ulang bila gambar sumber dipindahkan.
- **Undo / Redo penuh (`Ctrl+Z` / `Ctrl+Y`).** Riwayat hingga 50 langkah untuk goresan kuas, edit palet, pergantian mode produksi, dan pengaturan.
- **Custom Presets.** Simpan, pakai ulang, dan hapus setelan favorit di Editor tunggal maupun Batch Folder.
- **Coba contoh gambar 1 klik.** Layar awal menyediakan tombol **Coba contoh gambar** dengan ilustrasi sampel bebas hak cipta bawaan.
- **Kesiapan distribusi komersial.** Audit lisensi dependensi lengkap ([docs/LICENSE-AUDIT-1.0.0.md](docs/LICENSE-AUDIT-1.0.0.md)), teks penuh BSD-3-Clause & MIT di [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md), konfigurasi macOS Universal (`icon.icns` + `bundle.macOS`), dan panduan code signing ([docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md)).

### Pencapaian versi sebelumnya

- **0.5.0 ("Produksi"):** Batch folder + laporan CSV/Markdown, Mode Sablon (2–8 warna spot, tanda registrasi, film separasi `#000000`), Mode Cutting/Bordir *knockout*, pilih target warna saat Gabung palet, ikon macOS `icon.icns`.
- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, kuas koreksi (`Kuas Detail`/`Kuas Halus`), palet manual + pipet median 5×5, heatmap node, target jumlah node, cache `PreparedImage`.
- **0.3.0 ("Geometri rapi"):** Bentuk dasar murni, tegas simetri, sudut tajam vs kurva halus per titik, peringatan area teks offline, histeresis garis tipis utuh.

## Pakai

1. Jalankan installer (`Trasea_1.0.0_x64-setup.exe`, per pengguna, tanpa admin) lalu buka Trasea.
2. Klik **Coba contoh gambar**, seret gambar/proyek `.trasea`/folder ke jendela, atau klik **Buka gambar / proyek…**.
3. Sesuaikan **Mode Produksi** (**Standar**, **Sablon**, **Cutting / Bordir**), Preset Kustom, kuas, atau palet (`Ctrl+Z`/`Ctrl+Y` untuk Undo/Redo, `Ctrl+S` untuk simpan `.trasea`).
4. Ekspor **SVG**, **PDF**, **Palet .ase**, atau (setelah aktivasi Pro offline di **Privasi & Lisensi**) **EPS**, **Film Separasi**, dan **Batch Folder**.

Antarmuka desktop berbahasa Indonesia.

## Build dari sumber

Butuh Rust (`x86_64-pc-windows-gnu` + MinGW-w64 di `PATH`, atau MSVC; macOS Universal butuh `aarch64-apple-darwin` + `x86_64-apple-darwin`), Node 22+, dan WebView2 di Windows.

```
npm ci
node scripts/fetch-sidecar.mjs
npx tauri build --target x86_64-pc-windows-gnu
```

Alat pengembang:

```
npm test                                   # tes cargo V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09, E-01..E-03, L-01..L-03, P-01..P-02, N-01
cargo run --release --bin trasea-cli -- in.jpg out.svg [--pdf out.pdf] [--eps out.eps] [--ase out.ase] [--mode standard|sablon|cutting] [--separations dir] [opsi...]
cargo run --release --bin trasea-cli -- --batch <folder-input> <folder-output> [--mode standard|sablon|cutting] [--pdf] [--eps] [--ase] [--separations] [--subfolders] [--no-report]
cargo run --release --bin trasea-keygen -- init | issue --secret <HEX> --name "..." --id "TR-2026-0001" | verify <KEY>
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>
```

Baca [docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md) sebelum distribusi komersial.

## Offline & Lisensi

`trasea-core` tidak menautkan crate HTTP/jaringan apa pun (teruji otomatis oleh `V7` dan `N-01`), CSP membatasi `connect-src` hanya ke IPC lokal, dan WebView2 berjalan tanpa jaringan latar. Audit lisensi: nol crate GPL/AGPL — lihat [docs/LICENSE-AUDIT-1.0.0.md](docs/LICENSE-AUDIT-1.0.0.md) dan [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
