# Vektora (nama sementara)

*English: [README.md](README.md)*

**Aplikasi desktop Windows offline** yang mengubah ilustrasi flat atau logo (JPG/PNG) menjadi **SVG atau PDF** bersih dengan node yang rapi, supaya terbuka tanpa peringatan di Affinity dan Adobe Illustrator. Prototipe 0.1.0. Repo lokal, tanpa server, tanpa akun, **tanpa permintaan jaringan**.

## Cara kerja

Empat tahap (semuanya di komputermu):

1. **Penjaga garis tipis.** Garis gelap yang lebih tipis dari beberapa piksel (mata, senyum, outline) dicari lebih dulu dari gambar ASLI, karena setiap tahap pembersihan sesudahnya cenderung menghapusnya. Sebuah garis harus lebih gelap daripada piksel di kedua sisinya, jadi pinggiran gelap yang ditinggalkan JPEG di tepi bidang warna tidak salah dikira garis.
2. **Bersihkan (opsional).** Real-ESRGAN (anime x4, sidecar `realesrgan-ncnn-vulkan.exe` yang dibundel) membuang noise JPEG; hasilnya diturunkan ke 2x. Tanpa sidecar, tanpa GPU yang bisa dipakai, atau saat tombolnya mati, dipakai filter median 3 px dan aplikasi memberi tahu.
3. **Kunci palet + pelacakan.** Warna dikurangi menjadi 2-16 warna rata (k-means di Lab; piksel tepi tidak ikut menghitung palet sehingga tidak muncul warna "halo" di antara). Tiap warna menjadi satu lapisan bertumpuk, dilacak dengan visioncortex (mesin VTracer) lalu dicocokkan dengan sesedikit mungkin kurva Bezier kubik sesuai toleransi. Ambang diskalakan menurut ukuran gambar.
4. **Garis di atas.** Garis tipis dilacak pada 2x dan ditumpuk di atas bidang warna.

Ekspor: **SVG** (satu `<g id="#RRGGBB">` per warna, `fill` langsung di path, tanpa CSS, tanpa `<style>`, tanpa filter, tanpa transform, `viewBox` = ukuran gambar) dan **PDF** (ukuran halaman = ukuran gambar).

Kontrol: jumlah warna (2-16), kehalusan (sedikit node <-> detail), "jaga garis tipis", "bersihkan dengan AI". Statistik di layar: path, node, ukuran SVG, selisih warna, waktu.

## Pakai

1. Jalankan installer (`Vektora_0.1.0_x64-setup.exe`, per pengguna, tanpa admin) lalu buka Vektora.
2. Seret gambar ke jendela (atau **Buka gambar**), tunggu konversi (sekitar 1 dtk tanpa AI, 10-35 dtk dengan AI di GPU terintegrasi PC uji).
3. Geser slider untuk membandingkan asli dan vektor, ubah kontrol, tekan **Terapkan ulang** bila perlu.
4. **Ekspor SVG** atau **Ekspor PDF**.

Antarmuka berbahasa Indonesia.

## Build dari sumber

Butuh Rust (repo ini dibangun dengan toolchain `x86_64-pc-windows-gnu` dan MinGW-w64 di `PATH`; toolchain MSVC juga bisa), Node 22+ dan WebView2 (sudah ada di Windows 11).

```
npm ci
node scripts/fetch-sidecar.mjs          # mengunduh Real-ESRGAN ncnn Vulkan v0.2.5.0 ke src-tauri/resources/realesrgan
npx tauri build --target x86_64-pc-windows-gnu   # installer di target/<triple>/release/bundle/nsis/
```

Alat pengembang:

```
npm test                                   # tes cargo V1-V7 (gambar sintetis, tanpa data uji)
cargo run --release --bin vektora-cli -- in.jpg out.svg --pdf out.pdf [--colors 8] [--smooth 0.5] [--no-ai] [--no-lines] [--sidecar DIR]
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>   # menulis bench/RESULTS.md
```

Intinya ada di crate `crates/vektora-core` (tanpa UI, tanpa jaringan). Aplikasi di `src-tauri` hanya lapisan tipis; UI-nya React + TypeScript di `src`.

## Batas versi 0.1.0

- Hanya Windows x64. Real-ESRGAN butuh GPU Vulkan; bila tidak ada dipakai median (node lebih banyak, sedikit lebih noisy).
- Masukan berupa gambar raster (PNG, JPG, WEBP, BMP), sisi terpanjang dibatasi 1600 px. Foto bukan sasaran.
- Garis tipis = garis **gelap** saja; garis terang di area gelap dilacak sebagai bidang warna biasa.
- Apakah SVG dan PDF terbuka tanpa peringatan di Affinity dan Illustrator **tidak diuji oleh pembuat**; itu pemeriksaan pemilik (lihat laporan).
- Angka benchmark ada di `bench/RESULTS.md`; tidak semua target test plan tercapai (lihat laporan).

## Lisensi pihak ketiga

MIT untuk proyek ini. Dibundel: model Real-ESRGAN (BSD-3-Clause; cek ketentuan file model sebelum dijual), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), runtime OpenMP `vcomp140.dll` (redistributable Microsoft). **Tanpa Potrace (GPL).** Daftar lengkap crate ada di [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) dan di aplikasi (tombol **Lisensi**).

Catatan gambar uji: sebagian gambar benchmark ber-watermark stok foto. Hanya untuk uji, tidak termasuk repo maupun aplikasi.
