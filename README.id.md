# Trasea

*English: [README.md](README.md)*

**Aplikasi desktop Windows offline** yang mengubah ilustrasi flat atau logo (PNG/JPG/WEBP/BMP) menjadi **SVG atau PDF** bersih dengan node rapi, siap dibuka di Affinity Designer, Adobe Illustrator, film sablon, maupun mesin cutting/bordir. Versi **0.5.0 ("Produksi")**. Repo lokal, tanpa server, tanpa akun, **tanpa permintaan jaringan**.

## Cara kerja

Empat tahap (semuanya di komputermu), ditambah profil produksi, kendali desainer, dan pemrosesan batch folder:

1. **Penjaga garis tipis.** Garis gelap yang lebih tipis dari beberapa piksel (mata, senyum, outline) dicari lebih dulu dari gambar ASLI agar tidak hilang saat pembersihan.
2. **Bersihkan (opsional, mati secara bawaan).** Real-ESRGAN (anime x4, sidecar yang dibundel) membuang noise JPEG lalu menskalakan ke 2x. Mati: filter median 3 px berjalan 20–40x lebih cepat dan biasanya sama bagus.
3. **Kunci palet + pelacakan.** Warna rata (k-means di Lab, piksel tepi tidak ikut menghitung palet agar tidak ada halo; bintik kontras rendah digabung) dilacak dengan visioncortex (mesin VTracer) menjadi kurva Bezier kubik yang ringkas.
4. **Garis di atas.** Tiap garis tipis menjadi stroke terbuka yang mudah diedit (garis tengah + `stroke-width` terukur) atau bentuk isi tertutup sesuai mode produksi/ekspor.

Susunan layer SVG:

```
Latar            warna latar (dihilangkan bila "Hapus latar belakang" aktif)
Bidang warna     satu sub-grup per warna: id="Warna_02_EBD9C1", data-name="Warna 02 #EBD9C1"
Garis            garis tipis (stroke atau bentuk isi tertutup)
Registrasi       4 tanda registrasi sudut (opsional, aktif bawaan di Mode Sablon)
```

Mendukung ekspor **PDF** (ukuran halaman = ukuran gambar), **.ase** (Adobe Swatch Exchange, grup `Trasea`, warna `Trasea 01 #RRGGBB`), serta **film separasi hitam 100%** (`<nama>_film01_RRGGBB.svg`/`.pdf`).

### Baru di 0.5.0 ("Produksi")

- **Batch folder + laporan per file.** Proses satu folder penuh (termasuk opsional sub-folder 1 tingkat) atau seret banyak file/folder sekaligus ke jendela aplikasi. Berjalan berurutan bila AI aktif dan paralel ringan (`min(cpu/2, 4)`) bila AI mati. File rusak dicatat gagal tanpa menghentikan antrean, nama file duplikat otomatis diberi akhiran (`logo_2.svg`), tombol **Batal** berhenti rapi, menghasilkan `laporan-trasea.csv` dan `laporan-trasea.md`, serta klik satu baris laporan langsung membuka gambar di editor tunggal.
- **Mode Sablon (`ScreenPrint`).** Membatasi warna spot 2–8 (bawaan 4; peringatan jika > 6 warna), mengaktifkan trap 0,5 pt, menyerap bintik mikro, menebalkan garis di bawah batas cetak (`min_stroke_pt`, bawaan 1,0 pt), menambah 4 tanda registrasi di layer `Registrasi`, dan mengekspor file film separasi hitam 100% (`#000000`) terpisah per warna lengkap dengan tanda registrasi presisi dan label film.
- **Mode Cutting Sticker / Bordir (`CutEmbroidery`).** Pelacakan *knockout* bebas tumpang tindih (pisau *plotter* tidak memotong ganda), menghapus latar belakang secara bawaan (bisa dimatikan), mengubah semua garis menjadi bentuk isi tertutup dengan tebal minimum (`min_stroke_pt`, bawaan 1,5 pt), menyaring bintik mikro *weeding*, mengarahkan kehalusan bawaan ke 70, dan mengunci trap ke `0 pt`.
- **Polesan 0.4.0.** Tombol **Gabung** di Palet Manual kini bisa memilih warna target mana pun (warna terdekat tetap disarankan di urutan pertama) dan memetakan piksel ke warna target pilihan tersebut; ikon macOS `icon.icns` diperbarui ke logo huruf `T` Trasea.

### Pencapaian versi sebelumnya

- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, kuas koreksi (`Kuas Detail` / `Kuas Halus`), palet manual interaktif + pipet median 5×5, heatmap node (mati bawaan, pratinjau saja), target jumlah node (`--target-nodes`), cache `PreparedImage` di memori agar iterasi ulang cepat, penghapusan kontur duplikat tersembunyi.
- **0.3.0 ("Geometri rapi"):** Deteksi bentuk dasar (`<circle>`/`<ellipse>`/`<rect>`), tegas simetri, sudut tajam vs kurva halus per titik, peringatan area teks offline + sorotan pratinjau, histeresis garis tipis utuh.

## Pakai

1. Jalankan installer (`Trasea_0.5.0_x64-setup.exe`, per pengguna, tanpa admin) lalu buka Trasea.
2. Seret gambar ke jendela (atau **Buka gambar…**), atau klik **Batch folder** (atau seret folder/banyak gambar) untuk memproses sekaligus.
3. Pilih **Mode Produksi** (**Standar**, **Sablon**, atau **Cutting / Bordir**), sesuaikan pengaturan, lalu tekan **Terapkan ulang** bila perlu.
4. Ekspor **SVG**, **PDF**, **Palet .ase**, atau (di Mode Sablon) **Ekspor film separasi…**.

Antarmuka berbahasa Indonesia.

## Build dari sumber

Butuh Rust (toolchain `x86_64-pc-windows-gnu` + MinGW-w64 di `PATH`, atau MSVC), Node 22+, dan WebView2 (sudah ada di Windows 11).

```
npm ci
node scripts/fetch-sidecar.mjs          # mengunduh Real-ESRGAN ncnn Vulkan v0.2.5.0 ke src-tauri/resources/realesrgan
npx tauri build --target x86_64-pc-windows-gnu   # installer di target/<triple>/release/bundle/nsis/
```

Alat pengembang:

```
npm test                                   # tes cargo V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09 (gambar sintetis)
cargo run --release --bin trasea-cli -- in.jpg out.svg [--mode standard|sablon|cutting] [--pdf out.pdf] [--ase out.ase] [--separations dir] [--colors 8] [--smooth 0.5] [--min-stroke 1.0] [--reg-marks|--no-reg-marks] [--strip-bg|--keep-bg] [--ai] [--no-lines] [--lines stroke|shape] [--trap 0.5] [--no-primitives] [--symmetry off|auto|vertical|horizontal] [--no-text-detect] [--target-nodes N] [--palette "#RRGGBB,#RRGGBB"] [--png out.png] [--sidecar DIR]
cargo run --release --bin trasea-cli -- --batch <folder-input> <folder-output> [--mode standard|sablon|cutting] [--subfolders] [--pdf] [--ase] [--separations] [--no-report] [opsi...]
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>   # menulis bench/RESULTS.md
```

Intinya ada di crate `crates/trasea-core` (tanpa UI, tanpa jaringan). Aplikasi di `src-tauri` hanya lapisan tipis; UI-nya React + TypeScript di `src`.

## Offline

Inti dan aplikasi tidak berisi kode jaringan (diperiksa dengan `cargo tree` di sebuah tes), halaman hanya bisa bicara ke IPC lokal (CSP), dan WebView2 dijalankan dengan jaringan latar, pembaruan, sinkronisasi, dan SmartScreen mati, semua lalu lintas non-lokal diarahkan ke proxy lokal yang mati (`127.0.0.1:9`), dan DNS nama non-lokal diblokir. Trasea tidak pernah mengirim data gambar ke mana pun.

## Batas versi 0.5.0

- Hanya installer Windows x64 (ikon macOS `.icns` sudah diperbarui; build macOS penuh direncanakan di 1.0.0). Real-ESRGAN butuh GPU Vulkan; bila tidak ada dipakai median.
- Masukan berupa gambar raster (PNG, JPG, WEBP, BMP), sisi terpanjang dibatasi 1600 px. Foto bukan sasaran.
- Garis tipis = garis **gelap** saja; garis terang di area gelap dilacak sebagai bidang warna biasa.
- Mode Cutting / Bordir menghasilkan SVG/PDF *knockout* bersih siap masuk software cutting/bordir (Cricut, Silhouette, Hatch, Wilcom, Affinity, Illustrator); format jahitan biner (`.dst`, `.pes`, `.jef`) dan EPS di luar cakupan 0.5.0.
- Pemeriksaan buka file SVG/PDF/`.ase`/film separasi di Affinity Designer, Adobe Illustrator, atau Cricut Design Space merupakan uji manual pemilik (`M-01`..`M-03`).
- Deteksi teks adalah heuristik geometri (tanpa OCR) dan tidak pernah mengubah gambar.

## Lisensi pihak ketiga

MIT untuk proyek ini. Dibundel: model Real-ESRGAN (BSD-3-Clause), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), runtime OpenMP `vcomp140.dll` (redistributable Microsoft). **Tanpa Potrace (GPL).** Daftar lengkap crate ada di [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) dan di aplikasi (tombol **Lisensi**).