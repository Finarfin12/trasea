<p align="center"><img src="docs/branding/trasea-logo-long.png" alt="Trasea — Clean curves. Ready to cut." width="720"></p>

# Trasea

*English: [README.md](README.md)*

**Aplikasi desktop vektorisasi lokal & privat** (*Local-First & Private*) yang mengubah ilustrasi flat, logo, stiker, garis sketsa, dan karya *cel-shading* (PNG/JPG/WEBP/BMP, atau file SVG yang sudah ada) menjadi **SVG, PDF (RGB/CMYK/Spot), EPS (RGB/CMYK/Spot), dan AutoCAD R12 DXF** bersih dengan node rapi — siap untuk Affinity Designer, Adobe Illustrator, CorelDraw, RIP Roland/Mimaki/Summa, sablon, laser cutter, CNC router, dan mesin cutting/bordir. Versi **1.8.1 ("Mockup Studio Fotorealistis")**. Pemrosesan lokal & privat (gambar dan vektor diproses langsung di komputer tanpa unggah ke cloud dan tetap berjalan tanpa koneksi internet), dwibahasa (Indonesia & Inggris), tanpa akun, dan sepenuhnya gratis: semua fitur terbuka untuk semua orang tanpa aktivasi atau pendaftaran.

## Tangkapan Layar

![Trasea 1.8.0 — Editor Studio: dot-grid artboard, alat node, dan bilah zoom mengambang](docs/screenshots/app_node_edit.png)

| Pratinjau Mockup Produk Instan (5 Preset + PNG Resolusi Tinggi) | Smart Recolor (0 ms) & Isolasi Solo Warna |
| :---: | :---: |
| ![Pratinjau Mockup Produk](docs/screenshots/app_mockup_preview.png) | ![Smart Recolor Duotone dan Solo Warna](docs/screenshots/app_brand_kit_recolor.png) |

| Bandingkan Mesin Berdampingan (Offline vs AI) | Audit Vektor Pra-Cetak & Brand Kit ZIP 1-Klik |
| :---: | :---: |
| ![Bandingkan Mesin Offline vs AI](docs/screenshots/app_compare_engines.png) | ![Audit Vektor dan Ekspor Brand Kit ZIP](docs/screenshots/app_preflight_audit.png) |

| Inspeksi Node (kotak = Corner, lingkaran = Smooth) | Simplify Path |
| :---: | :---: |
| ![Overlay inspeksi node](docs/screenshots/app_node_inspect.png) | ![Simplify path](docs/screenshots/app_node_simplify.png) |

| Impor SVG (buka .svg yang sudah ada sebagai vektor) | Hapus Latar 1-Klik & Garis Potong CutContour |
| :---: | :---: |
| ![Impor SVG](docs/screenshots/app_svg_import.png) | ![Hapus latar dan die-cut](docs/screenshots/app_remove_bg_diecut.png) |

| Mode Garis Tengah / Centerline (Laser / CNC) | Pilih & Edit Path Langsung di Kanvas |
| :---: | :---: |
| ![Mode vektor garis tengah tunggal](docs/screenshots/app_centerline.png) | ![Bilah Pilih Path interaktif dan edit non-destruktif](docs/screenshots/app_path_edit.png) |

| Soft-Proof CMYK, Inspektor Kode Warna & Tinta Spot | Pengaturan Aplikasi & Privasi/Kredit |
| :---: | :---: |
| ![Soft-Proof CMYK dan popover Inspektor Warna](docs/screenshots/app_softproof_spot.png) | ![Modal Pengaturan Aplikasi](docs/screenshots/app_settings.png) |

| Pratinjau Outline (Ctrl+Y) & Toggle Layer | Pantau Folder Otomatis & Log Langsung |
| :---: | :---: |
| ![Mode Outline dan ikon mata layer](docs/screenshots/app_outline_layers.png) | ![Log langsung Pantau Folder](docs/screenshots/app_watch_folder.png) |

## Cara kerja

Empat tahap lokal ditambah pra-proses non-destruktif, lima profil produksi, edit path langsung di kanvas, Soft-Proof CMYK, tinta spot bernama, pustaka palet tersimpan, alur kerja proyek, batch folder, dan pemantau folder otomatis:

1. **Pra-proses non-destruktif.** Potong area (*crop*), putar gambar (`-90°`, `+90°`, `180°`, atau sudut bebas `-45°..+45°`), serta **Hapus Latar Otomatis 1-klik** (flood-fill 4-arah CIE Lab ΔE dari tepi + defringe halo tepi `0..3 px`) tanpa mengubah file asli.
2. **Penjaga garis tipis & Mode Garis Tengah.** Garis gelap tipis diekstrak terlebih dahulu agar tidak hilang saat pembersihan — atau aktifkan mode **Garis Tengah (`Centerline`)** untuk menghasilkan path stroke terbuka murni (0 bidang isi).
3. **Bersihkan (opsional, mati bawaan).** Real-ESRGAN (anime x4, sidecar bawaan) membuang noise JPEG dan menskalakan ke 2x. Mati: filter median 3 px bekerja 20–40x lebih cepat. Jika GPU tidak mendukung Vulkan, Trasea otomatis mundur ke Median dengan pesan yang jelas.
4. **Kunci palet / Gradasi + pelacakan + edit path + garis & kontur.** Warna rata (k-means di Lab) atau pita warna Gradasi/Cel-Shading (12–32 warna Lab dengan *bilateral smoothing*) dilacak dengan visioncortex (mesin VTracer) menjadi kurva Bézier kubik ringkas, bentuk dasar murni `<circle>`/`<ellipse>`/`<rect>`, dan tegas simetri. Hapus atau ubah warna per-path berlaku konsisten di pratinjau maupun seluruh format ekspor.

Format keluaran yang didukung:
- **SVG** dengan layer bernama (`Latar`, `Bidang warna`, `Garis`, opsional `CutContour`, opsional `Registrasi`) — plus **Salin SVG** 1 klik (`Ctrl+Shift+C`) ke clipboard
- **PDF** dalam mode **RGB**, **DeviceCMYK murni** (PDF 1.4 operator `k`/`K`, hitam & registrasi `0 0 0 1` K=100% murni), serta tinta **Named Spot `/Separation`** (termasuk `CutContour` `0,25 pt`)
- **AutoCAD R12 DXF (`AC1009`)** dengan layer terpisah (`COLOR_01_RRGGBB`, `THIN_LINES`, `CENTERLINE`, `CUTCONTOUR`, `REGISTRATION`), entitas `CIRCLE` murni, sumbu Y CAD, serta satuan `mm` (72/96/300 DPI atau lebar target khusus) / `px`
- **EPS** (*Encapsulated PostScript Level 2 / EPSF-3.0*, generator Rust murni RGB, CMYK, atau Named Spot `%%CMYKCustomColor` tanpa Ghostscript/Inkscape)
- **Ekspor palet:** **`.ase`** (Adobe Swatch Exchange RGB/CMYK dengan swatch Global & Spot `0x0001`), **`.gpl`** (GIMP/Inkscape/Krita/Blender), **`.json`**, dan variabel **`.css`**
- **Film separasi hitam 100% / K=100%** (`<nama>_film01_RRGGBB.svg`/`.pdf`)
- **`.trasea`** file proyek (JSON `schemaVersion: 1`, kompatibel penuh dengan 1.0.0 & 1.1.0) menyimpan tautan gambar sumber + SHA-256, potong/putar/hapus latar, goresan kuas, edit path, palet manual, daftar gabung warna, kontur potong, tinta spot, dan setelan ekspor

### Baru di 1.8.1 ("Mockup Studio Fotorealistis")

- **Pratinjau Mockup Studio Fotorealistis.** Meng-upgrade seluruh 5 preset di **Pratinjau Mockup** memakai foto studio lokal bebas lisensi (`tshirt-light`, `tshirt-dark`, `totebag-light`, `card-light`, `card-dark`, `sticker-surface`), *blending* tinta alami (`multiply`) di bahan terang agar lipatan kain/kertas menyatu nyata, overlay tekstur lembut di bahan gelap, pinggiran vinil putih tebal + kilau *glossy* diagonal pada Stiker Die-Cut, serta ubin *squircle* macOS/iOS berbayangan 3 lapis untuk Ikon Aplikasi.
- **Seret Posisi Interaktif & Ekspor PNG 1600×1200 Identik.** Seret posisi vektor langsung di kanvas mockup 4:3, atur slider ukuran (`30%–100%`), klik **Reset posisi**, dan simpan PNG resolusi tinggi (`1600×1200 px`) yang identik dengan pratinjau fotorealistis.

### Baru di 1.8.0 ("Studio Production, Brand Kit & Visual Audit")

- **Pratinjau Mockup Produk Instan.** Lihat hasil vektor Offline maupun AI langsung pada 5 preset produk (**Kaos**, **Stiker Die-Cut**, **Tote Bag**, **Kartu Nama**, dan **Ikon Aplikasi**) dengan permukaan **Terang / Gelap**, slider ukuran (`30%–100%`), dan tombol **Simpan PNG** resolusi tinggi (`1600×1200 px`).
- **Smart Recolor (0 ms) & Ekspor Brand Kit (.zip) 1-Klik.** Ganti variasi warna non-destruktif (**Asli**, **Hitam**, **Putih**, **Duotone** dengan pemilih warna) tanpa mengubah layer `CutContour` (`#EC008C`) dan `Registrasi`, isolasi 1 pelat warna lewat tombol **Solo** (`Alt+Klik` ikon mata), serta ekspor 1 arsip **Brand Kit (.zip)** berisi `color.svg`, `black.svg`, `white.svg`, `cutcontour.pdf`, dan `palette.ase`.
- **Bandingkan Mesin Berdampingan (Offline vs AI).** Bandingkan hasil Offline GPU dan CLI AI lokal secara langsung lengkap dengan metrik `path · node · KB`, lalu pilih sumber vektor aktif dalam 0 ms tanpa trace ulang.
- **Audit Vektor Pra-Cetak & Perbaiki Otomatis.** Deteksi otomatis garis tipis (`< 0,5 pt`), node padat (`> 3.000`), warna mirip (`ΔE < 10`), dan path debu mikro (`< 4 px²`) langsung di tab Ekspor lengkap dengan tombol **Perbaiki otomatis**.
- **Salin SVG Universal (`Ctrl+Shift+C`).** Mendukung hasil Offline maupun AI Import sekaligus menerapkan variasi Smart Recolor yang sedang aktif.

### Baru di 1.7.4

- **Artboard Studio & Bilah Zoom Mengambang.** Menambahkan latar *dot-grid* 20 px halus serta bayangan elevasi *artboard* di tema gelap maupun terang, dilengkapi bilah zoom mengambang di kanan bawah kanvas (`25%`–`400%`, `−` / reset `%` / `+` / `Pas`).
- **Layar Selamat Datang & Terakhir Dibuka.** Dropzone kini terpusat dengan logo Trasea v2, kontras teks bantu lebih jelas, *format pills* (`PNG`, `JPG`, `WebP`, `SVG`, `.trasea`), serta daftar **Terakhir dibuka** lokal (hingga 4 berkas gambar atau proyek `.trasea`).
- **Konsistensi Copywriting & Inspektor Rapi.** Menyeragamkan label aksi header dan layar awal (`Tempel gambar`, `Batch folder`, `Preset kustom`), menghilangkan sapaan formal *"Anda"*, menyeragamkan tombol berbingkai di header, merapatkan seluruh ikon `?` tepat di sebelah labelnya, menambah pemisah grup inspektor, dan mengaktifkan `tabular-nums` pada angka slider.

### Baru di 1.7.3

- **Dropdown `<select>` Model CLI Murni + Input Model Kustom.** Mengganti `<datalist>` di **Pengaturan → Mesin AI → CLI Lokal** menjadi `<select>` standar untuk `codex`, `claude`, `gemini`, dan `opencode` (selalu menampilkan seluruh model terdeteksi lengkap dengan `— Ikuti bawaan CLI —` dan `Model kustom…`), dilengkapi penyegaran daftar otomatis saat berpindah preset CLI dan deteksi model lokal `~/.gemini/settings.json`.
- **Perbaikan Shim `.cmd`/`.bat` Windows & Deteksi OpenCode CLI.** Memprioritaskan resolusi `.exe`/`.cmd`/`.bat` sebelum berkas shim POSIX tanpa ekstensi di Windows (`build_cli_command` lewat `cmd /C` dengan `CREATE_NO_WINDOW`), mengalihkan `stderr` `opencode models` ke `Stdio::null()`, dan menaikkan batas waktu deteksi menjadi 25 detik agar CLI berbasis npm (`opencode.cmd`, `gemini.cmd`) berjalan mulus.

### Baru di 1.7.2

- **Animasi Opening Splash Sinkron First-Paint WebView2.** Timer splash kini dimulai setelah double `requestAnimationFrame` (`950 ms` tampil + `300 ms` fade-out halus) dengan latar solid `#0f1117`, logo Trasea v2 `88×88 px` memantul masuk (*pop-in*), dan cincin *glow* meluas (`.app-splash-ring`), serta dukungan aksesibel `prefers-reduced-motion`.

### Baru di 1.7.1

- **Penjaga Garis Hairline Khusus GPU (`for_ai_cleaned`).** Saat Real-ESRGAN GPU aktif (`used_ai == true`), `thin_mask` memakai `open_radius = 1` sehingga hanya garis *hairline* (≤ 3 px) yang diambil dari sumber mentah, membiarkan outline normal hasil Real-ESRGAN dilacak mulus tanpa ditimpa mask bergerigi.
- **Penggabungan Warna Mirip Hasil AI/CLI (`coalesce_similar_colors`).** Vektor hasil AI/CLI otomatis menggabungkan warna yang hampir identik secara visual (CIE Lab `ΔE < 8,0`) ke warna dominannya tanpa mengubah geometri path, ditambah pembaruan prompt preset CLI untuk palet flat ringkas (≤ 10 warna hex).
- **Animasi Loading AI Trace + Timer & *Micro-Splash* Opening.** Proses AI/CLI kini menampilkan logo Trasea v2 berdenyut halus, *indeterminate progress bar* GPU-accelerated, teks status, serta timer `MM:SS` berjalan; pembukaan aplikasi menampilkan *micro-splash* logo singkat (~700 ms) yang menghormati `prefers-reduced-motion`.

### Baru di 1.7.0 ("Upgrade Kualitas Mesin Offline")

- **Anti-Alias Border Dehaloing (`dehalo_label_borders`).** Dua tahap cepat di ruang warna Lab menghapus pita halo *anti-alias* 1–2 px di pinggiran warna flat sambil menjaga bidang warna solid asli dan pita gradasi mode `Shading` tetap utuh.
- **Kontinuitas Garis Outline Tipis (`lines.rs`).** Menambahkan kelanjutan histeresis satu-sisi (*one-sided ink continuation*) dan penutupan celah 1 px terverifikasi warna di `thin_mask`, sehingga outline gelap di perbatasan bidang warna sedang/gelap tetap menyambung tanpa menyerap blok gelap solid besar.
- **Pembersihan Gerigi Batas Label (`smooth_label_jaggies`).** Filter mayoritas 4-tetangga 1 tahap meratakan tonjolan/lekukan 1 px sebelum ekstraksi kontur sambil menjaga sudut 90°, warna aksen terlindungi, dan garis 1–2 px tetap utuh — menghasilkan kurva lebih mulus dengan node lebih ringkas.

### Baru di 1.6.3

- **Preset OpenCode CLI & Pilihan Model / Reasoning CLI.** Menambah preset 1-klik `OpenCode CLI` (`opencode run --auto -f {input}`), deteksi model CLI lokal (`fetch_cli_models`), serta kontrol Model + Reasoning/Effort untuk `codex`, `claude`, `gemini`, dan `opencode` (preset Codex otomatis memakai effort `low`).
- **Edit Node / Path / Warna Instan pada Hasil AI.** Edit node, pilih path, atau ubah warna pada vektor hasil AI/CLI kini langsung diproses di atas cache SVG AI lewat `apply_ai_vector_edits` tanpa balik ke trace lokal dan tanpa me-reset editan.
- **Sinkronisasi Mesin Pelacakan & Status Loading AI/CLI.** Buka, tempel, crop, atau relink gambar tetap sinkron dengan mesin pelacakan terpilih, dan proses AI/CLI menampilkan status loading khusus tanpa angka `0%` lokal.

### Baru di 1.6.2

- **Label Dinamis Mesin Lokal.** Pilihan mesin pelacakan lokal kini otomatis menampilkan `Lokal · GPU · Offline` atau `Lokal · CPU · Offline` mengikuti pengaturan Performa & GPU.
- **Perbaikan Codex CLI.** Preset `Codex CLI` diperbarui memakai `--skip-git-repo-check --ephemeral --sandbox read-only -i {input} -o {output}`, direktori kerja CLI diisolasi ke folder sementara, dan `--skip-git-repo-check` otomatis disisipkan untuk perintah `codex exec` lama.

### Baru di 1.6.1

- **Pembaruan Logo Aplikasi & Banner README v2.** Seluruh ikon aplikasi desktop (`.ico`, `.icns`, PNG, ikon header, dan favicon) serta banner README (kartu putih dengan wordmark Inter ter-outline, 0 elemen `<text>`) diperbarui ke identitas visual v2.

### Baru di 1.6.0 ("Build macOS & Mesin AI Eksternal")

- **Build macOS Resmi (`.dmg`).** Installer `.dmg` terpisah untuk Apple Silicon (`aarch64`) dan Intel (`x86_64`) yang dibangun otomatis lewat GitHub Actions (`macos-latest`, macOS 11.0+), dengan fallback mulus ke pembersih gambar bawaan.
- **Mesin AI Eksternal (API BYOK & CLI Lokal).** Vektorisasi AI opt-in di samping mesin lokal offline: atur **Recraft V3**, **Vectorizer.AI**, **Claude**, **GPT (OpenAI)**, **Gemini**, atau **CLI Lokal** (`claude`, `gemini`, `codex`, atau skrip Python) di **Pengaturan -> Mesin AI**. Claude, GPT, dan Gemini memuat daftar model aktif secara otomatis dari API resmi (tanpa daftar model hardcode). Kunci API disimpan terpisah di `ai-keys.json` (permission `0600` di Unix), tidak pernah masuk file `.trasea`, dan tidak pernah dikirim mentah ke WebView. Hasil SVG AI langsung masuk ke kanvas Trasea (Alat Node, Warna, Sticker Backing/Bleed, `CutContour`, PDF CMYK, EPS, DXF).

### Baru di 1.5.2

- **Label versi header ringkas.** Header aplikasi kini hanya menampilkan versi rilis (`v1.5.2`) tanpa label tambahan.

### Baru di 1.5.1

- **Versi UI dinamis.** Header aplikasi (`id` & `en`) serta komentar generator ekspor palet kini membaca `APP_VERSION` langsung dari `package.json` agar versi yang tampil selalu selaras dengan biner terpasang.

### Baru di 1.5.0 ("Paket Poles Vektor & Distribusi Publik")

- **Kurva lebih bersih & node ikon lebih ringkas (*Post-Fit Curve Merge*).** Jendela tangen adaptif pada `fit_cubic` ditambah tahap penggabungan Bézier bertetangga memangkas node berlebih pada ikon dan ilustrasi melengkung (turun hingga 6% pada logo dan ≤ 40 node pada ikon lengkung 2-warna) tanpa menumpulkan sudut tajam.
- **Sudut & pertemuan garis tipis tertutup rapi.** Stroke garis tipis kini melewati tahap penyambungan ujung terverifikasi `dark_mask` (`join_stroke_endpoints`) dengan `round` cap/join konsisten di SVG, PDF, dan EPS — menutup celah sudut/T-junction tanpa garis nyasar melintasi celah terang.
- **Latar Stiker Offset / Bleed (`StickerBacking`).** Tambah bidang latar stiker solid (bawaan `#FFFFFF`, warna bebas) plus *bleed* luar `0,0–3,0 mm` yang konsentris dengan `CutContour` pada SVG, PDF (RGB & CMYK putih murni `0 0 0 0 k`), dan EPS. Tidak dimasukkan ke DXF agar mesin potong hanya menerima jalur `CUTCONTOUR`. Tersedia di tab Ekspor dan CLI (`--sticker-backing`, `--sticker-backing-color`, `--sticker-bleed-mm`).
- **Cek pembaruan privasi-pertama.** Tombol manual **Periksa pembaruan** di **Pengaturan → Privasi & Kredit** (plus sakelar opsional saat dibuka, bawaan Mati) hanya mengecek endpoint rilis publik GitHub dari backend Rust (0 cookie, 0 telemetri, timeout 8 detik; `trasea-core` tetap 0 crate jaringan dan WebView2 tetap terisolasi).
- **Versi Portable Windows (`.zip`).** Ekstrak `Trasea_1.7.3_x64-portable.zip` ke mana saja dan langsung jalankan `trasea.exe` tanpa instalasi: bila `portable.flag` ada di sebelah `trasea.exe`, pengaturan dan preset tersimpan lokal di `./data/` tanpa menyentuh `%APPDATA%`.

### Baru di 1.4.0 ("Semua Fitur Terbuka")

- **Semua terbuka.** Ekspor EPS, film separasi hitam 100%, dan Batch Folder sekali jalan kini bisa dipakai siapa saja; tidak ada kunci, trial, atau ajakan aktivasi, dan tidak ada label "(Pro)".
- **Garis tidak lagi hilang setelah "Terapkan ulang".** Edit path dan node terikat pada trace tempat edit dibuat (sidik jari SHA-256). Bila trace berubah, edit lama tidak diterapkan ke bentuk lain; edit direset dengan pemberitahuan, dan Undo mengembalikannya bersama pengaturan lamanya.
- **Pengaturan → Privasi & Kredit.** Pernyataan privasi plus lisensi open-source pihak ketiga.
- **Siap open-source.** `LICENSE` MIT, build lebih ramping (14 crate lebih sedikit), dan installer dirilis sebagai aset GitHub Release.

### Baru di 1.3.0 ("Editor Node")

Fitur baru 1.3.0:

- **Alat Node (`A`).** Klik sebuah path untuk mengeditnya node demi node: seret titik jangkar dan handle Bézier secara real-time (kotak = Corner, lingkaran = Smooth), `Alt`+seret handle untuk memutus tangen, klik dua kali node untuk beralih Corner ↔ Smooth, `Delete` untuk menghapus node, **Tampilkan semua handle**, serta **Simplify** dan **Reset Node** di bilah mengambang. Setiap gerakan adalah satu langkah Undo/Redo; pintasan dapat diatur di Pengaturan.
- **Edit node non-destruktif.** Edit disimpan sebagai `nodeEdits` (piksel sumber) dan diterapkan setelah edit path, sehingga pratinjau dan seluruh ekspor — SVG, PDF, EPS, DXF, CutContour, separasi — memakai geometri hasil edit. Tersimpan di proyek `.trasea` (proyek lama tetap bisa dibuka).
- **Impor SVG.** Buka `.svg` yang sudah ada (dialog Buka atau seret-lepas) sebagai vektor yang bisa diedit tanpa tracing; Pilih Path, Alat Node, pewarnaan ulang, dan semua ekspor berfungsi.
- **Identitas merek baru.** Ikon aplikasi baru (varian mini huruf T dipakai pada ukuran ≤ 32 px), logo header dan favicon, serta banner wordmark berbasis path.

### Baru di 1.2.0 ("Stiker, Laser, & Presisi RIP")

8 fitur 1.2.0:

- **Garis Potong Stiker Die-Cut / Kiss-Cut (`CutContour`).** Kontur potong luar otomatis menggunakan 2D Euclidean Distance Transform + Marching Squares + penghalusan kurva Bézier (`0,5..10,0 mm`, bawaan `2,0 mm`, opsi gabung pulau berdekatan). Menulis tinta spot `CutContour` (`CMYK 0/100/0/0`, `#EC008C`, `0,25 pt`) di SVG, PDF (`/Separation`), EPS (`%%CMYKCustomColor`), DXF (polyline tertutup `CUTCONTOUR`), dan ASE (swatch Spot `0x0001`), lengkap dengan pratinjau langsung dan ikon mata layer.
- **Mode Garis Tengah (`Centerline` / Single-Line).** Mode produksi ke-5 (`--mode centerline`) memakai threshold adaptif, opsi balik warna (*invert* untuk garis terang di latar gelap), penipisan Zhang-Suen, pemangkasan taji, dan kurva Bézier terbuka murni (0 bidang isi; `LWPOLYLINE` terbuka di layer DXF `CENTERLINE`) untuk laser engraving, CNC, dan pen plotter.
- **Hapus Latar Otomatis (1-Klik) & Defringe.** Flood-fill 4-arah dari tepi gambar di ruang CIE Lab (`ΔE 0..100`, bawaan `24`) + defringe halo tepi (`0..3 px`, bawaan `1`) langsung di panel kiri maupun modal Potong & Putar; area tertutup di dalam objek tetap utuh.
- **Pilih & Edit Path Langsung di Kanvas.** Aktifkan alat **Pilih Path** (`Ctrl+E`) untuk menyorot dan mengklik path vektor di kanvas, menghapusnya (`Delete`/`Backspace`), mengubah warnanya ke warna palet lain, atau mereset edit path — non-destruktif, terhubung ke Undo/Redo, dan ikut terekspor ke SVG, PDF, EPS, & DXF.
- **Pratinjau Soft-Proof CMYK & Peringatan Luar Gamut.** Nyalakan **Soft-Proof CMYK** (`Ctrl+P`) untuk simulasi gamut cetak CMYK instan (0 ms) di kanvas, ditambah lencana `⚠` pada warna neon di palet yang berada di luar gamut CMYK.
- **Penamaan Tinta Spot (Named Spot Color) untuk RIP.** Beri nama tinta spot kustom (mis. `White_Underbase`, `PANTONE 185 C`) pada warna palet lewat Inspektor Warna; diekspor sebagai PDF `/Separation`, EPS `%%CMYKCustomColor`, dan ASE Spot (`0x0001`).
- **Modal Pengaturan Aplikasi 5 Tab (`⚙` / `Ctrl+,`) & Ruang Kerja 3 Tab Rapi.** Pengaturan terpadu di `settings.json`: **Umum** (Bahasa ID/EN, Tema Gelap / Terang / Ikuti Sistem, latar kanvas transparan, buka folder otomatis setelah ekspor), **Default Ekspor** (RGB/CMYK, satuan DXF, 72/96/300 DPI, bawaan offset/nama/bridge `CutContour`), **Performa & GPU** (Otomatis GPU vs Selalu Median Cepat, batas Undo 20/50/100), **Pintasan Keyboard** (perekam kombinasi tombol kustom + deteksi bentrok + reset), dan **Privasi & Lisensi**. Dilengkapi toolbar kanvas 1 baris ikon SVG inline dan panel kanan 3 tab tetap (**Pelacakan** / **Warna** / **Ekspor**) dengan *sticky footer* ringkasan & tombol aksi.
- **Inspektor Kode Warna & Pustaka Palet Tersimpan.** Klik chip warna di palet untuk melihat dan menyalin 1-klik kode **HEX**, **RGB**, **CMYK**, dan **HSL**; simpan, terapkan ulang, dan hapus palet brand di `palettes.json`; ekspor palet ke **`.gpl`**, **`.json`**, dan **`.css`** selain `.ase`.

### Pencapaian versi sebelumnya

- **1.2.0 ("Stiker, Laser, & Presisi RIP"):** Garis Potong Die-Cut CutContour, mode Garis Tengah, Hapus Latar 1-Klik, Pilih Path, Soft-Proof CMYK, tinta Spot bernama, Pengaturan Aplikasi, Inspektor Warna & pustaka palet.
- **1.1.0 ("Alur Kerja Cepat & Produksi Penuh"):** Ekspor DXF R12 (`mm`/`px`), ekspor cetak DeviceCMYK murni (PDF/EPS/ASE), Sablon Knockout Murni, Pantau Folder otomatis, Clipboard Masuk/Keluar (`Ctrl+V` / `Ctrl+Shift+C`), pratinjau Outline (`Ctrl+Y`) & ikon mata layer, UI Dwibahasa ID/EN, Potong & Putar non-destruktif, Mode Gradasi/Cel-Shading.
- **1.0.0 ("Siap Dijual"):** Ekspor EPS murni, modal Privasi, file proyek `.trasea`, Undo/Redo, Preset Kustom, ilustrasi contoh 1 klik, perbaikan kontinuitas garis tipis melengkung.
- **0.5.0 ("Produksi"):** Batch folder + laporan CSV/Markdown, Mode Sablon (2–8 warna spot, tanda registrasi, film separasi `#000000`), Mode Cutting/Bordir *knockout*, pilih target warna saat Gabung palet, ikon macOS `icon.icns`.
- **0.4.0 ("Kendali desainer"):** Rebrand Vektora → Trasea, kuas koreksi (`Kuas Detail`/`Kuas Halus`), palet manual + pipet median 5×5, heatmap node, target jumlah node, cache `PreparedImage`.
- **0.3.0 ("Geometri rapi"):** Bentuk dasar murni, tegas simetri, sudut tajam vs kurva halus per titik, peringatan area teks lokal, histeresis garis tipis utuh.

## Pakai

1. **macOS (Apple Silicon & Intel):** Unduh `.dmg` yang sesuai (`aarch64` atau `x86_64`) dari [halaman Releases](https://github.com/Finarfin12/trasea/releases), seret `Trasea.app` ke `Applications`, lalu bila Gatekeeper macOS menahan aplikasi tanpa tanda tangan Apple Developer, jalankan `xattr -cr /Applications/Trasea.app` di Terminal sebelum membukanya.
2. **Windows:** Unduh `Trasea_1.6.3_x64-setup.exe` (atau `.zip` berisi installer yang sama) dari [halaman Releases](https://github.com/Finarfin12/trasea/releases), cocokkan SHA-256 di catatan rilis, jalankan (per pengguna, tanpa admin), lalu buka Trasea.
2. Ganti bahasa (`ID | EN`) atau buka **Pengaturan Aplikasi (`⚙` / `Ctrl+,`)** kapan saja untuk memilih tema Gelap/Terang/Ikuti Sistem, default ekspor, dan pintasan keyboard kustom.
3. Klik **Coba contoh gambar**, tekan `Ctrl+V` untuk menempel dari clipboard, seret gambar/`.svg`/proyek `.trasea`/folder ke jendela, atau klik **Buka gambar / proyek…**.
4. Gunakan **Hapus latar belakang (1-klik)**, **Potong & Putar**, **Mode Produksi** (**Standar**, **Sablon**, **Cutting / Bordir**, **Gradasi**, **Garis Tengah**), **Pilih Path (`Ctrl+E`)**, **Alat Node (`A`)**, **Soft-Proof CMYK (`Ctrl+P`)**, **Outline (`Ctrl+Y`)**, Inspektor Warna (klik chip warna untuk HEX/RGB/CMYK/HSL & tinta spot), dan Pustaka Palet Tersimpan.
5. Aktifkan **Garis Potong Stiker (`CutContour`)** bila menyiapkan stiker, salin SVG (`Ctrl+Shift+C`), ekspor **SVG**, **PDF (RGB/CMYK/Spot)**, **DXF (mm/px)**, **palet `.ase`/`.gpl`/`.json`/`.css`**, jalankan **Pantau Folder**, ekspor **EPS** dan **Film Separasi**, atau proses satu folder dengan **Batch Folder** — semuanya gratis.

## Build dari sumber

Butuh Rust (`x86_64-pc-windows-gnu` + MinGW-w64 di `PATH`, atau MSVC; macOS Universal butuh `aarch64-apple-darwin` + `x86_64-apple-darwin`), Node 22+, dan WebView2 di Windows.

```
npm ci
node scripts/fetch-sidecar.mjs
npx tauri build --target x86_64-pc-windows-gnu
```

Alat pengembang:

```
npm test                                   # 94 tes cargo: V1-V7, W1-W6, G1-G8, D0-D6, T-01..T-09, E-01..E-03, P-01..P-02, N-01, F-01..F-10, G12-01..G12-10, G13-01..G13-10, G14-01..G14-06
cargo run --release --bin trasea-cli -- in.png out.svg [--remove-bg] [--bg-tolerance 24] [--defringe 1] [--mode standard|sablon|cutting|shading|centerline] [--centerline-invert] [--centerline-stroke-pt 1.0] [--die-cut] [--die-cut-mm 2.0] [--die-cut-name CutContour] [--spot "#E63946=PANTONE 185 C"] [--pdf out.pdf] [--eps out.eps] [--dxf out.dxf] [--ase out.ase] [--color-space rgb|cmyk] [--dxf-unit mm|px] [--dxf-width-mm 120] [--crop X,Y,W,H] [--rotate DEG] [--separations dir]
cargo run --release --bin trasea-cli -- --batch <folder-input> <folder-output> [--mode standard|sablon|cutting|shading|centerline] [--remove-bg] [--die-cut] [--color-space rgb|cmyk] [--pdf] [--eps] [--dxf] [--ase] [--separations] [--subfolders] [--no-report]
cargo run --release --bin trasea-cli -- --watch <folder-input> <folder-output> [--poll-ms 1000] [--debounce-ms 500] [--dxf] [--pdf] [--die-cut] [--color-space cmyk]
cargo run --release --bin bench -- --dir <folder vectorize-bench> --sidecar <folder realesrgan>
```

Baca [docs/RELEASE-SIGNING.md](docs/RELEASE-SIGNING.md) sebelum distribusi komersial.

## Pemrosesan Lokal, Privasi, & Kredit

Seluruh pembersihan gambar, vektorisasi, dan ekspor berjalan langsung di komputer Anda tanpa mengunggah desain ke cloud, dan Trasea tetap bekerja tanpa koneksi internet. Crate inti `trasea-core` tidak menautkan crate HTTP/jaringan maupun crate GPL/AGPL apa pun (terverifikasi otomatis oleh `V7`, `N-01`, `F-09`, dan `G12-10`), serta WebView menerapkan CSP ketat ke IPC lokal. Lihat [docs/LICENSE-AUDIT-1.4.0.md](docs/LICENSE-AUDIT-1.4.0.md) dan [THIRD_PARTY_LICENSES.md](THIRD_PARTY_LICENSES.md).

## Lisensi

Trasea gratis dipakai dan dirilis dengan [Lisensi MIT](LICENSE). Bangun dari sumber seperti di atas, atau unduh installer dari [halaman Releases](https://github.com/Finarfin12/trasea/releases).
