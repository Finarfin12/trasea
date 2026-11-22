# Vektora (nama sementara)

*English: [README.md](README.md)*

**Aplikasi desktop Windows offline** yang mengubah ilustrasi flat atau logo (JPG/PNG) menjadi **SVG atau PDF** bersih dengan node yang rapi, supaya terbuka tanpa peringatan di Affinity dan Adobe Illustrator. Prototipe 0.2.0 ("File siap kerja"). Repo lokal, tanpa server, tanpa akun, **tanpa permintaan jaringan**.

## Cara kerja

Empat tahap (semuanya di komputermu):

1. **Penjaga garis tipis.** Garis gelap yang lebih tipis dari beberapa piksel (mata, senyum, outline) dicari lebih dulu dari gambar ASLI, karena setiap tahap pembersihan sesudahnya cenderung menghapusnya. Sebuah garis harus lebih gelap daripada piksel di kedua sisinya, jadi pinggiran gelap yang ditinggalkan JPEG di tepi bidang warna tidak salah dikira garis.
2. **Bersihkan (opsional, mati secara bawaan).** Real-ESRGAN (anime x4, sidecar yang dibundel) membuang noise JPEG lalu diturunkan ke 2x. Mati: dipakai filter median 3 px (hampir sama bagus pada kebanyakan gambar, 20-40x lebih cepat). Bila dinyalakan tetapi sidecar atau GPU tidak ada, aplikasi kembali ke median dan memberi tahu.
3. **Kunci palet + pelacakan.** 2-16 warna rata (k-means di Lab, piksel tepi tidak ikut menghitung palet sehingga tidak ada warna halo; bintik kontras rendah digabung). Tiap warna satu lapisan bertumpuk (warna yang lebih kecil selalu di atas yang lebih besar, jadi tidak ada celah), dilacak dengan visioncortex (mesin VTracer) lalu dicocokkan dengan sesedikit mungkin kurva Bezier kubik sesuai toleransi.
4. **Garis di atas.** Tiap garis tipis menjadi SATU path terbuka: garis tengahnya (dari skeleton) dengan `stroke-width` terukur dan ujung bulat. Garis dengan tebal tidak merata tetap berupa bentuk isi.

Ekspor, layer SVG (namanya terbaca di Illustrator dan Affinity):

```
Latar            warna latar
Bidang warna     satu sub-grup per warna: id="Warna_02_EBD9C1", data-name="Warna 02 #EBD9C1"
Garis            garis tipis: <path fill="none" stroke="#223F64" stroke-width="4.12" stroke-linecap="round" ...>
```

Tanpa CSS, `<style>`, filter, atau transform. Juga **PDF** (ukuran halaman = ukuran gambar) dan palet sebagai **.ase** (Adobe Swatch Exchange, warna bernama `Vektora 01 #RRGGBB`; impor di Illustrator lewat Swatches dan di Affinity lewat Swatches > Import Palette; tidak diuji oleh pembuat, lihat Batas). Opsi: garis tipis sebagai stroke atau bentuk isi; trap 0 / 0,25 / 0,5 / 1 pt untuk cetak.

Kontrol: jumlah warna (2-16), kehalusan (sedikit node <-> detail), "jaga garis tipis", "bersihkan dengan AI". Statistik di layar: path, node, ukuran SVG, selisih warna, waktu.

## Pakai

1. Jalankan installer (`Vektora_0.2.0_x64-setup.exe`, per pengguna, tanpa admin) lalu buka Vektora.
2. Seret gambar ke jendela (atau **Buka gambar**), tunggu konversi (sekitar 1-3 dtk dengan pengaturan bawaan, 10-40 dtk dengan AI menyala, di GPU terintegrasi PC uji).
3. Geser slider untuk membandingkan asli dan vektor, ubah kontrol, tekan **Terapkan ulang** bila perlu.
4. Pilih cara ekspor garis tipis dan trap, lalu **Ekspor SVG**, **Ekspor PDF** atau **Palet .ase** (berkas .ase disimpan di samping SVG/PDF bila "Sertakan palet .ase" dicentang).

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
npm test                                   # tes cargo V1-V7 dan W1-W6 (gambar sintetis, tanpa data uji)
cargo run --release --bin vektora-cli -- in.jpg out.svg --pdf out.pdf --ase out.ase [--colors 8] [--smooth 0.5] [--ai] [--no-lines] [--lines stroke|shape] [--trap 0.5] [--sidecar DIR]
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>   # menulis bench/RESULTS.md
```

Intinya ada di crate `crates/vektora-core` (tanpa UI, tanpa jaringan). Aplikasi di `src-tauri` hanya lapisan tipis; UI-nya React + TypeScript di `src`.

## Offline

Inti dan aplikasi tidak berisi kode jaringan (diperiksa dengan `cargo tree` di sebuah tes), halaman hanya bisa bicara ke IPC lokal (CSP), dan WebView2 dijalankan dengan jaringan latar, pembaruan, sinkronisasi, dan SmartScreen mati, semua lalu lintas non-lokal diarahkan ke proxy lokal yang mati (`127.0.0.1:9`), dan DNS nama non-lokal diblokir. **Yang tidak bisa dihilangkan:** lihat laporan; runtime Edge WebView2 bawaan Windows mungkin masih membuka satu koneksi sendiri sesaat setelah mulai. Vektora tidak pernah mengirim data gambar ke mana pun.

## Batas versi 0.2.0

- Hanya Windows x64. Real-ESRGAN butuh GPU Vulkan; bila tidak ada dipakai median (node lebih banyak, sedikit lebih noisy).
- Masukan berupa gambar raster (PNG, JPG, WEBP, BMP), sisi terpanjang dibatasi 1600 px. Foto bukan sasaran.
- Garis tipis = garis **gelap** saja; garis terang di area gelap dilacak sebagai bidang warna biasa.
- Apakah SVG, PDF, dan `.ase` terbuka dan terimpor tanpa peringatan di Affinity dan Illustrator **tidak diuji oleh pembuat**; itu pemeriksaan pemilik (lihat laporan). PDF hanya berisi pengelompokan biasa, tanpa layer PDF (konverternya tidak menulis optional content group).
- Garis tipis terang di area gelap (misalnya garis busa putih di atas biru tua) dilacak sebagai bidang warna; bila tepinya bercampur, sebuah bidang bisa mendapat warna palet di sebelahnya (bercak krem, bukan putih).
- Angka benchmark ada di `bench/RESULTS.md`; tidak semua target test plan tercapai (lihat laporan).

## Lisensi pihak ketiga

MIT untuk proyek ini. Dibundel: model Real-ESRGAN (BSD-3-Clause; cek ketentuan file model sebelum dijual), Real-ESRGAN-ncnn-vulkan (MIT), ncnn (BSD-3-Clause), VTracer/visioncortex (MIT), runtime OpenMP `vcomp140.dll` (redistributable Microsoft). **Tanpa Potrace (GPL).** Daftar lengkap crate ada di [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md) dan di aplikasi (tombol **Lisensi**).

Catatan gambar uji: sebagian gambar benchmark ber-watermark stok foto. Hanya untuk uji, tidak termasuk repo maupun aplikasi.
