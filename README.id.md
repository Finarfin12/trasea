# Trasea

*English: [README.md](README.md)*

**Aplikasi desktop vektorisasi offline** yang mengubah ilustrasi flat, logo, dan karya *cel-shading* (PNG/JPG/WEBP/BMP) menjadi **SVG, PDF (RGB/CMYK), EPS (RGB/CMYK), dan AutoCAD R12 DXF** bersih dengan node rapi — siap untuk Affinity Designer, Adobe Illustrator, CorelDraw, sablon, laser cutter, CNC router, dan mesin cutting/bordir. Versi **1.1.0 ("Alur Kerja Cepat & Produksi Penuh")**. Lokal, dwibahasa (Indonesia & Inggris), tanpa akun, tanpa cloud, **tanpa permintaan jaringan**.

## Cara kerja

Empat tahap lokal ditambah pra-proses non-destruktif, profil produksi, kendali desainer, alur kerja proyek, batch folder, dan pemantau folder otomatis:

1. **Pra-proses non-destruktif.** Potong area (*crop*) dan putar gambar (`-90°`, `+90°`, `180°`, atau sudut bebas `-45°..+45°`) sebelum pelacakan tanpa mengubah file asli.
2. **Penjaga garis tipis.** Garis gelap tipis (mata, senyum, outline) diekstrak dari gambar terlebih dahulu agar tidak hilang saat pembersihan.
3. **Bersihkan (opsional, mati bawaan).** Real-ESRGAN (anime x4, sidecar bawaan) membuang noise JPEG dan menskalakan ke 2x. Mati: filter median 3 px bekerja 20–40x lebih cepat. Jika GPU tidak mendukung Vulkan, Trasea otomatis mundur ke Median dengan pesan yang jelas.
4. **Kunci palet / Gradasi + pelacakan + garis di atas.** Warna rata (k-means di Lab) atau pita warna Gradasi/Cel-Shading (12–32 warna Lab dengan *bilateral smoothing*) dilacak dengan visioncortex (mesin VTracer) menjadi kurva Bézier kubik ringkas, bentuk dasar murni `<circle>`/`<ellipse>`/`<rect>`, dan tegas simetri. Garis tipis ditempatkan rapi di atas sebagai stroke terbuka atau bentuk isi tertutup.

Format keluaran yang didukung:
- **SVG** dengan layer bernama (`Latar`, `Bidang warna`, `Garis`, opsional `Registrasi`) — plus **Salin SVG** 1 klik (`Ctrl+Shift+C`) ke clipboard
- **PDF** dalam mode **RGB** atau **DeviceCMYK murni** (PDF 1.4 operator `k`/`K`, nol operator RGB, hitam & tanda registrasi `0 0 0 1` K=100% murni)
- **AutoCAD R12 DXF (`AC1009`)** dengan layer per warna (`COLOR_01_RRGGBB`, `THIN_LINES`, `REGISTRATION`), entitas `CIRCLE` murni, sumbu Y CAD, serta satuan `mm` (96 DPI atau lebar target khusus) / `px`
- **EPS** (*Encapsulated PostScript Level 2 / EPSF-3.0*, generator Rust murni RGB atau CMYK tanpa Ghostscript/Inkscape) — Pro
- **`.ase`** (palet Adobe Swatch Exchange, grup `Trasea`, RGB atau CMYK)
- **Film separasi hitam 100% / K=100%** (`<nama>_film01_RRGGBB.svg`/`.pdf`) — Pro
- **`.trasea`** file proyek (JSON `schemaVersion: 1`, kompatibel penuh dengan 1.0.0) menyimpan tautan gambar sumber + SHA-256, setelan potong/putar, goresan kuas, palet manual, daftar gabung warna, dan seluruh pengaturan

### Baru di 1.1.0 ("Alur Kerja Cepat & Produksi Penuh")

Seluruh 9 fitur baru 1.1.0 **terbuka tanpa memerlukan kunci Pro**:

- **Ekspor DXF R12 (Laser / CNC / Plotter).** Writer ASCII AutoCAD R12 (`AC1009`) murni dari Editor, Batch, Pantau Folder, atau CLI (`--dxf`, `--dxf-unit mm|px`, `--dxf-width-mm`), bawaan satuan `mm` 96 DPI dengan entitas `CIRCLE` murni dan layer terpisah per warna.
- **Ekspor Cetak CMYK (PDF, EPS, ASE, Film Separasi).** Pilih ruang warna cetak **RGB** atau **CMYK** (`--color-space cmyk`). Menghasilkan PDF 1.4 DeviceCMYK murni (hanya operator `k`/`K`, nol RGB), EPS CMYK (`setcmykcolor`), dan palet `.ase` CMYK dengan aturan Pure Black (`0 0 0 1` untuk `#000000` dan tanda registrasi).
- **Opsi Sablon Knockout Murni.** Pilih metode separasi **Tumpuk (tanpa celah)** atau **Knockout Murni** (`--sablon-knockout`, trap `0..2 pt`) di mode Sablon agar tinta tidak menumpuk terlalu tebal.
- **Pantau Folder (Watch Folder Otomatis).** Pemantau folder lokal (`Batch folder` → tab **Pantau Folder**, atau CLI `--watch <in> <out>`) dengan debounce kestabilan tulis, isolasi file rusak, log tabel langsung, dan pembaruan otomatis `laporan-trasea.csv`/`.md`.
- **Clipboard Masuk & Keluar.** Tempel gambar/tangkapan layar langsung dengan `Ctrl+V` (atau tombol **Tempel gambar**) dan salin kode SVG bersih dengan `Ctrl+Shift+C` (atau **Salin SVG**) langsung ke Affinity Designer, Illustrator, Inkscape, atau Figma.
- **Pratinjau Outline / Wireframe Instan (`Ctrl+Y`) & Toggle Layer.** Nyalakan mode garis kurva instan (`Ctrl+Y`) serta sembunyikan/tampilkan tiap layer warna, Garis Tipis (`Garis`), dan Registrasi (`Registrasi`) lewat ikon mata tanpa melacak ulang. *(Pintasan Redo menggunakan `Ctrl+Shift+Z`.)*
- **Antarmuka Dwibahasa (Indonesia & Inggris).** Tombol cepat `ID | EN` di bilah atas yang tersimpan otomatis di `settings.json`.
- **Potong & Putar Non-Destruktif.** Kotak potong interaktif, tombol cepat `-90°` / `+90°` / `180°`, serta pelurusan sudut bebas `-45°..+45°` yang tersimpan dalam file proyek `.trasea` tanpa mengubah file gambar asli.
- **Mode Gradasi / Cel-Shading.** Mode produksi ke-4 (`--mode shading`) dengan *bilateral smoothing* di ruang Lab dan 12–32 pita warna halus untuk ilustrasi anime atau *cel-shading*.

### Pencapaian versi sebelumnya

- **1.0.0 ("Siap Dijual"):** Ekspor EPS murni, lisensi offline Ed25519 & trial ramah, modal Privasi & Lisensi, file proyek `.trasea`, Undo/Redo 50 langkah, Preset Kustom, ilustrasi contoh 1 klik, perbaikan kontinuitas garis tipis melengkung.
- **0.5.0 ("Produksi"):** Batch folder + laporan CSV/Markdown, Mode Sablon (2–8 warna spot, tanda registrasi, film separasi `#000000`), Mode Cutting/Bordir *knockout*, pilih target warna saat Gabung palet, ikon macOS `icon.icns`.
- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, kuas koreksi (`Kuas Detail`/`Kuas Halus`), palet manual + pipet median 5×5, heatmap node, target jumlah node, cache `PreparedImage`.
- **0.3.0 ("Geometri rapi"):** Bentuk dasar murni, tegas simetri, sudut tajam vs kurva halus per titik, peringatan area teks offline, histeresis garis tipis utuh.

## Pakai

1. Jalankan installer (`Trasea_1.1.0_x64-setup.exe`, per pengguna, tanpa admin) lalu buka Trasea.
2. Ganti bahasa (`ID | EN`) kapan saja melalui tombol di bilah atas.
3. Klik **Coba contoh gambar**, tekan `Ctrl+V` untuk menempel dari clipboard, seret gambar/proyek `.trasea`/folder ke jendela, atau klik **Buka gambar / proyek…**.
4. Gunakan **Potong & Putar**, **Outline (`Ctrl+Y`)**, ikon mata layer, **Mode Produksi** (**Standar**, **Sablon**, **Cutting / Bordir**, **Gradasi**), Preset Kustom, kuas, atau palet (`Ctrl+Z`/`Ctrl+Shift+Z` untuk Undo/Redo, `Ctrl+S` untuk simpan `.trasea`).
5. Salin SVG (`Ctrl+Shift+C`), ekspor **SVG**, **PDF (RGB/CMYK)**, **DXF (mm/px)**, **Palet .ase**, atau jalankan **Pantau Folder** secara bebas; aktifkan kunci Pro offline di **Privasi & Lisensi** untuk membuka **EPS**, **Film Separasi**, dan **Batch Sekali Jalan**.

## Build dari sumber

Butuh Rust (`x86_64-pc-windows-gnu` + MinGW-w64 di `PATH`, atau MSVC; macOS Universal butuh `aarch64-apple-darwin` + `x86_64-apple-darwin`), Node 22+, dan WebView2 di Windows.

```
npm ci
node scripts/fetch-sidecar.mjs
npx tauri build --target x86_64-pc-windows-gnu
```

Alat pengembang:

```
npm test                                   # 71 tes cargo: V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09, E-01..E-03, L-01..L-03, P-01..P-02, N-01, F-01..F-10
cargo run --release --bin trasea-cli -- in.jpg out.svg [--pdf out.pdf] [--eps out.eps] [--dxf out.dxf] [--ase out.ase] [--color-space rgb|cmyk] [--dxf-unit mm|px] [--dxf-width-mm 120] [--mode standard|sablon|cutting|shading] [--sablon-knockout] [--crop X,Y,W,H] [--rotate DEG] [--separations dir]
cargo run --release --bin trasea-cli -- --batch <folder-input> <folder-output> [--mode standard|sablon|cutting|shading] [--color-space rgb|cmyk] [--pdf] [--eps] [--dxf] [--ase] [--separations] [--subfolders] [--no-report]
cargo run --release --bin trasea-cli -- --watch <folder-input> <folder-output> [--poll-ms 1000] [--debounce-ms 500] [--dxf] [--pdf] [--color-space cmyk]
cargo run --release --bin trasea-keygen -- init | issue --secret <HEX> --name "..." --id "TR-2026-0001" | verify <KEY>
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>
```

Baca [docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md) sebelum distribusi komersial.

## Offline & Lisensi

`trasea-core` dan `src-tauri` tidak menautkan crate HTTP/jaringan apa pun (teruji otomatis oleh `V7`, `N-01`, dan `F-09`), CSP membatasi `connect-src` hanya ke IPC lokal, dan WebView2 berjalan tanpa jaringan latar. Audit lisensi: nol crate GPL/AGPL — lihat [docs/LICENSE-AUDIT-1.1.0.md](docs/LICENSE-AUDIT-1.1.0.md) dan [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).
