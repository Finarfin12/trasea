/* Trasea Landing Page — Releases API, OS Detector, Feature Tabs, Bilingual EN/ID */
(function () {
  "use strict";

  var RELEASE_API = "https://api.github.com/repos/Finarfin12/trasea/releases/latest";
  var REPO_API = "https://api.github.com/repos/Finarfin12/trasea";
  var CACHE_KEY = "trasea_latest_release_v1";
  var REPO_CACHE_KEY = "trasea_repo_meta_v1";
  var CACHE_TTL_MS = 15 * 60 * 1000;
  var LANG_KEY = "trasea_lang";

  var FALLBACK_RELEASE = {
    tag: "v1.8.2",
    htmlUrl: "https://github.com/Finarfin12/trasea/releases/tag/v1.8.2",
    assets: {
      winSetup: {
        name: "Trasea_1.8.2_x64-setup.exe",
        url: "https://github.com/Finarfin12/trasea/releases/download/v1.8.2/Trasea_1.8.2_x64-setup.exe",
        size: 15195543
      },
      winPortable: {
        name: "Trasea_1.8.2_x64-portable.zip",
        url: "https://github.com/Finarfin12/trasea/releases/download/v1.8.2/Trasea_1.8.2_x64-portable.zip",
        size: 18124418
      },
      macArm: {
        name: "Trasea_1.8.2_aarch64.dmg",
        url: "https://github.com/Finarfin12/trasea/releases/download/v1.8.2/Trasea_1.8.2_aarch64.dmg",
        size: 7256324
      },
      macIntel: {
        name: "Trasea_1.8.2_x64.dmg",
        url: "https://github.com/Finarfin12/trasea/releases/download/v1.8.2/Trasea_1.8.2_x64.dmg",
        size: 7678593
      }
    }
  };

  var I18N = {
    en: {
      "a11y.skip": "Skip to main content",
      "a11y.langGroup": "Language",
      "a11y.featureTabs": "Trasea features",
      "nav.features": "Features",
      "nav.formats": "Formats",
      "nav.download": "Download",
      "nav.faq": "FAQ",
      "nav.getTrasea": "Download",
      "hero.badge": "Free & Open Source · MIT",
      "hero.title": "Clean curves. Ready to cut.",
      "hero.subtitle": "Local-first, 100% offline desktop vectorization studio. Turn flat artwork, logos, stickers, and sketches into tidy SVG, CMYK/Spot PDF, EPS, and AutoCAD R12 DXF — with zero cloud uploads and zero subscriptions.",
      "hero.ctaWin": "Download for Windows",
      "hero.ctaMac": "Download for macOS (Apple Silicon)",
      "hero.ctaFallback": "Download Trasea",
      "hero.allDownloads": "All downloads",
      "hero.altWin": "Portable (.zip) & macOS",
      "hero.altMac": "Intel Mac (.dmg)",
      "hero.altLinkDefault": "See all 4 installers",
      "hero.proof": "16 public releases · 122 automated Rust tests · 8 export formats · MIT open source",
      "hero.proofStars": "GitHub stars",
      "github.starFallback": "★ Star",
      "trust.offlineTitle": "Your artwork stays on disk",
      "trust.offlineDesc": "Client logos and NDA drafts never leave your machine — trace and export instantly with no server queue.",
      "trust.freeTitle": "Zero Subscription",
      "trust.freeDesc": "Completely free and open source. Every pro print tool unlocked without accounts or watermarks.",
      "trust.formatsTitle": "RGB · CMYK · Spot · DXF",
      "trust.formatsDesc": "Built for Illustrator, Affinity, CorelDraw, Roland/Mimaki RIPs, screen printing, and laser/CNC.",
      "trust.aiTitle": "Compare Offline & AI SVG",
      "trust.aiDesc": "Import SVG output from a local AI CLI tool and compare it side-by-side against Rust/VTracer with live path, node, and KB metrics.",
      "features.eyebrow": "STUDIO SHOWCASE",
      "features.title": "Edit nodes, preview mockups, and audit before export.",
      "features.subtitle": "Inspect every stage from pixel cleanup and node editing to photorealistic mockups, brand kits, and pre-flight print checks.",
      "tabs.studio.label": "Studio Editor & Nodes",
      "tabs.studio.title": "Precision Node Editing & Brush Corrections",
      "tabs.studio.desc": "Fine-tune vector curves directly on an infinite dot-grid artboard. Inspect corner and smooth nodes, simplify paths, or paint Detail and Smooth brush corrections before exporting.",
      "tabs.studio.p1": "Pen Tool node editing with Corner & Smooth inspection overlay",
      "tabs.studio.p2": "Detail Brush (+) and Smooth Brush for localized curve refinement",
      "tabs.studio.p3": "Outline wireframe (Ctrl+Y), floating zoom bar, and layer toggles",
      "tabs.studio.cap1": "Studio Editor — Pen Tool, Detail/Smooth brushes, and live vector artboard",
      "tabs.studio.cap2": "Node Inspect — square corners and smooth curve handles at a glance",
      "tabs.mockup.label": "Photorealistic Mockups",
      "tabs.mockup.title": "Instant Photorealistic Product Mockups",
      "tabs.mockup.desc": "Preview your vector on real studio photography before sending files to clients or print shops. Drag to reposition, scale smoothly, switch light or dark surfaces, and save high-res PNGs.",
      "tabs.mockup.p1": "5 built-in presets: T-Shirt, Die-Cut Sticker, Tote Bag, Business Card, App Icon",
      "tabs.mockup.p2": "Natural fabric & paper multiply blending with drag-to-reposition",
      "tabs.mockup.p3": "1-click 1600×1200 PNG export for instant client sign-off",
      "tabs.mockup.cap1": "Photorealistic studio mockup — natural multiply blend and 1600×1200 PNG export",
      "tabs.recolor.label": "Smart Recolor & Brand Kit",
      "tabs.recolor.title": "0 ms Smart Recolor & 1-Click Brand Kit ZIP",
      "tabs.recolor.desc": "Switch between Original, All-Black, All-White, and Duotone variations in zero milliseconds, isolate any ink with Solo view, and bundle the entire brand delivery into one structured ZIP archive.",
      "tabs.recolor.p1": "Instant Original, Black, White, and Duotone colorways",
      "tabs.recolor.p2": "Solo ink isolation (Alt+Click eye) without touching CutContour or Registration",
      "tabs.recolor.p3": "1-click Brand Kit (.zip) containing all colorways, PNGs, and palettes",
      "tabs.recolor.cap1": "Smart Recolor & Solo isolation — instant brand colorways ready to package",
      "tabs.audit.label": "Pre-Flight Audit",
      "tabs.audit.title": "Automated Pre-Flight Vector Audit & Auto-Fix",
      "tabs.audit.desc": "Catch plotter and press headaches before export. Trasea scans for microscopic speckle paths, redundant nodes, and open shapes, then repairs them in a single click.",
      "tabs.audit.p1": "Detects tiny micro-paths, excessive nodes, and unclosed geometry",
      "tabs.audit.p2": "1-click Auto-Fix cleans output while preserving crisp artwork detail",
      "tabs.audit.p3": "Integrated directly inside the Export panel alongside Brand Kit ZIP",
      "tabs.audit.cap1": "Pre-Flight Audit — inspect vector health and fix issues before cutting or printing",
      "tabs.compare.label": "Compare Offline vs AI",
      "tabs.compare.title": "Side-by-Side Engine Comparison (Offline vs AI)",
      "tabs.compare.desc": "Compare Trasea's instant local Rust vector engine side-by-side against imported AI CLI vectors with real path, node, and file-size metrics — then pick the winner in one click.",
      "tabs.compare.p1": "Synchronized split comparison with live paths · nodes · KB metrics",
      "tabs.compare.p2": "Copy SVG (Ctrl+Shift+C) and Brand Kit ZIP work on both Offline and AI sources",
      "tabs.compare.p3": "Offline engine stays 100% private with zero external calls required",
      "tabs.compare.cap1": "Compare Engines — inspect path count, node count, and SVG weight side by side",
      "tabs.production.label": "Die-Cut, Centerline & CMYK",
      "tabs.production.title": "Die-Cut Contours, Centerline Laser & CMYK Spot Ink",
      "tabs.production.desc": "Purpose-built tools for sticker shops, screen printers, and fabrication labs: 1-click background removal with CutContour bleed, single-line Centerline strokes for laser/CNC, and CMYK soft-proofing with named Spot separations.",
      "tabs.production.p1": "1-click Remove Background + offset CutContour (0.25 pt Spot)",
      "tabs.production.p2": "Centerline mode outputs pure single-line open Bézier strokes for engraving",
      "tabs.production.p3": "CMYK Soft-Proof, Color Code Inspector, and Named Spot /Separation PDFs",
      "tabs.production.cap1": "Remove Background & Die-Cut CutContour",
      "tabs.production.cap2": "Centerline Single-Line (Laser / CNC)",
      "tabs.production.cap3": "CMYK Soft-Proof & Named Spot Ink",
      "formats.eyebrow": "PRODUCTION OUTPUTS",
      "formats.title": "From web-ready SVG to press-ready CMYK and CAD DXF.",
      "formats.subtitle": "For web & brand designers, print shops, and laser workshops — pure-Rust generators with no Ghostscript or Inkscape required.",
      "formats.svgTitle": "Layered Vector & Clipboard",
      "formats.svgDesc": "Named layers (Background, Colour Fields, Thin Lines, CutContour, Registration) plus 1-click Copy SVG (Ctrl+Shift+C).",
      "formats.pdfTitle": "RGB, DeviceCMYK & Spot",
      "formats.pdfDesc": "Pure K=100% black & registration marks plus Named Spot /Separation inks including 0.25 pt CutContour.",
      "formats.epsTitle": "PostScript Level 2",
      "formats.epsDesc": "Native EPSF-3.0 output in RGB, CMYK, or Spot %%CMYKCustomColor for legacy print RIPs and vinyl software.",
      "formats.dxfTitle": "AutoCAD AC1009 CAD",
      "formats.dxfDesc": "Per-color layers, native CIRCLE entities, CAD Y-axis orientation, and mm (72/96/300 DPI or target width) / px units.",
      "formats.palTitle": "Reusable Swatch Palettes",
      "formats.palDesc": "Export Adobe Swatch Exchange (.ase Global/Spot), GIMP/Inkscape (.gpl), JSON tokens, and CSS variables.",
      "formats.kitTitle": "Client-Ready Delivery Bundle",
      "formats.kitDesc": "Packages Original, Black, White, and Duotone SVGs alongside high-res PNG previews and swatch files in one click.",
      "formats.projTitle": "Non-Destructive Projects",
      "formats.projDesc": "Saves source SHA-256 link, crop/rotate/remove-bg state, brush strokes, node edits, spot inks, and export presets.",
      "formats.sepTitle": "100%-K Screen Films",
      "formats.sepDesc": "Generate individual 100%-black / 100%-K separation films per ink color with registration marks ready for exposure.",
      "download.eyebrow": "DOWNLOAD",
      "download.title": "Install natively on Windows or macOS.",
      "download.subtitlePrefix": "Latest release",
      "download.subtitleSuffix": "· Free, open source, zero telemetry.",
      "download.recommended": "Recommended",
      "download.winSetupTitle": "Windows Installer",
      "download.winSetupDesc": "Standard NSIS setup for Windows 10 and Windows 11 with Start Menu shortcut and clean uninstaller.",
      "download.btnWinSetup": "Download .exe",
      "download.winPortableTitle": "Windows Portable",
      "download.winPortableDesc": "Standalone ZIP archive — extract anywhere and launch without administrator privileges.",
      "download.btnWinPortable": "Download .zip",
      "download.macArmTitle": "macOS (M1 / M2 / M3 / M4)",
      "download.macArmDesc": "Native ARM64 disk image optimized for Apple Silicon Macs.",
      "download.btnMacArm": "Download Apple Silicon .dmg",
      "download.macIntelTitle": "macOS (Intel)",
      "download.macIntelDesc": "Native x86_64 disk image for Intel-based Mac computers.",
      "download.btnMacIntel": "Download Intel .dmg",
      "download.noteWinTitle": "Windows SmartScreen",
      "download.noteWinDesc": "Community builds are unsigned. If SmartScreen appears, choose More info → Run anyway.",
      "download.noteMacTitle": "macOS Gatekeeper",
      "download.noteMacDesc": "If macOS blocks the first launch, run <code>xattr -cr /Applications/Trasea.app</code> in Terminal or allow it in Privacy &amp; Security.",
      "download.noteVerifyTitle": "Checksums & Releases",
      "download.noteReleasesLink": "GitHub Releases & SHA-256",
      "download.noteSigningLink": "Verification guide",
      "faq.title": "Frequently asked questions",
      "faq.q1": "Is Trasea completely free and open source?",
      "faq.a1": "Yes. Trasea is released under the MIT License. Every feature — including CMYK/Spot PDF, AutoCAD DXF, Photorealistic Mockups, Brand Kit ZIP, and Pre-Flight Audit — is unlocked with no account, watermark, or subscription.",
      "faq.q2": "Does Trasea upload my artwork to any server?",
      "faq.a2": "No. Preprocessing, VTracer curve fitting, node editing, and PDF/DXF generation all execute natively in Rust on your CPU/GPU—even with Wi-Fi switched off.",
      "faq.q3": "What is the difference between Offline Tracing and AI CLI Import?",
      "faq.a3": "Offline Tracing uses Trasea's built-in Rust/VTracer engine for instant, fully local vectorization. AI CLI Import lets you bring in SVG output from a local AI CLI tool and compare both results side-by-side with live path, node, and KB metrics.",
      "faq.q4": "Which operating systems are supported?",
      "faq.a4": "Trasea provides native 64-bit builds for Windows 10/11 (NSIS Installer and Portable ZIP) as well as macOS (both Apple Silicon ARM64 and Intel x64 DMG packages).",
      "faq.q5": "How do I report a bug or request a feature?",
      "faq.a5": "Open an issue directly on GitHub at Finarfin12/trasea/issues with your OS version, steps to reproduce, or a sample file description.",
      "footer.tagline": "Clean curves. Ready to cut. Local-first open-source vectorization studio.",
      "footer.releases": "Releases",
      "footer.changelog": "Changelog",
      "footer.license": "MIT License",
      "footer.issues": "Report Issue",
      "footer.meta": "MIT License · Rust + Tauri · Zero telemetry"
    },
    id: {
      "a11y.skip": "Lompat ke konten utama",
      "a11y.langGroup": "Bahasa",
      "a11y.featureTabs": "Fitur Trasea",
      "nav.features": "Fitur",
      "nav.formats": "Format",
      "nav.download": "Unduh",
      "nav.faq": "FAQ",
      "nav.getTrasea": "Unduh",
      "hero.badge": "Gratis & Open Source · MIT",
      "hero.title": "Kurva bersih. Siap potong.",
      "hero.subtitle": "Studio vektorisasi desktop lokal yang 100% offline. Ubah ilustrasi flat, logo, stiker, dan sketsa menjadi SVG, PDF CMYK/Spot, EPS, dan AutoCAD R12 DXF rapi — tanpa unggah ke cloud dan tanpa langganan.",
      "hero.ctaWin": "Unduh untuk Windows",
      "hero.ctaMac": "Unduh untuk macOS (Apple Silicon)",
      "hero.ctaFallback": "Unduh Trasea",
      "hero.allDownloads": "Semua unduhan",
      "hero.altWin": "Versi Portabel (.zip) & macOS",
      "hero.altMac": "macOS Intel (.dmg)",
      "hero.altLinkDefault": "Lihat semua 4 installer",
      "hero.proof": "16 rilis publik · 122 uji otomatis Rust · 8 format ekspor · Open source MIT",
      "hero.proofStars": "bintang GitHub",
      "github.starFallback": "★ Bintang",
      "trust.offlineTitle": "Karya tetap di komputermu",
      "trust.offlineDesc": "Logo klien dan draf rahasia tidak pernah keluar dari perangkat — trace dan ekspor instan tanpa antre server.",
      "trust.freeTitle": "Tanpa Langganan",
      "trust.freeDesc": "Gratis penuh dan open source. Semua fitur cetak profesional terbuka tanpa akun maupun watermark.",
      "trust.formatsTitle": "RGB · CMYK · Spot · DXF",
      "trust.formatsDesc": "Siap masuk Illustrator, Affinity, CorelDraw, RIP Roland/Mimaki, sablon, serta mesin laser dan CNC.",
      "trust.aiTitle": "Bandingkan Offline & AI SVG",
      "trust.aiDesc": "Impor hasil SVG dari CLI AI lokal lalu bandingkan berdampingan dengan Rust/VTracer lewat metrik path, node, dan KB secara langsung.",
      "features.eyebrow": "PAMERAN STUDIO",
      "features.title": "Edit node, cek mockup, dan audit vektor sebelum ekspor.",
      "features.subtitle": "Lihat setiap tahap mulai dari pembersihan gambar dan edit node hingga mockup fotorealistis, Brand Kit ZIP, dan audit pra-cetak.",
      "tabs.studio.label": "Editor Studio & Node",
      "tabs.studio.title": "Edit Node Presisi & Koreksi Kuas",
      "tabs.studio.desc": "Sempurnakan kurva vektor langsung di atas artboard dot-grid. Periksa titik sudut dan lengkung, sederhanakan path, atau sapukan Kuas Detail dan Kuas Halus sebelum ekspor.",
      "tabs.studio.p1": "Alat Node (Pen Tool) dengan overlay inspeksi titik sudut dan lengkung",
      "tabs.studio.p2": "Kuas Detail (+) dan Kuas Halus untuk perbaikan kurva di area tertentu",
      "tabs.studio.p3": "Mode Outline (Ctrl+Y), bilah zoom mengambang, dan toggle layer",
      "tabs.studio.cap1": "Editor Studio — Pen Tool, Kuas Detail/Halus, dan artboard vektor langsung",
      "tabs.studio.cap2": "Inspeksi Node — titik sudut kotak dan kurva halus lingkaran terlihat jelas",
      "tabs.mockup.label": "Mockup Fotorealistis",
      "tabs.mockup.title": "Pratinjau Mockup Produk Fotorealistis Instan",
      "tabs.mockup.desc": "Uji tampilan vektor di atas foto studio nyata sebelum mengirim file ke klien atau percetakan. Seret untuk geser posisi, atur ukuran, ganti bahan terang atau gelap, lalu simpan PNG resolusi tinggi.",
      "tabs.mockup.p1": "5 preset siap pakai: Kaos, Stiker Die-Cut, Tote Bag, Kartu Nama, Ikon Aplikasi",
      "tabs.mockup.p2": "Blending multiply alami menyatu dengan tekstur kain & kertas plus seret posisi",
      "tabs.mockup.p3": "Ekspor PNG 1600×1200 sekali klik untuk persetujuan klien lebih cepat",
      "tabs.mockup.cap1": "Mockup studio fotorealistis — tekstur kain menyatu alami dan ekspor PNG 1600×1200",
      "tabs.recolor.label": "Smart Recolor & Brand Kit",
      "tabs.recolor.title": "Smart Recolor 0 ms & Brand Kit ZIP 1-Klik",
      "tabs.recolor.desc": "Ganti variasi warna Asli, Hitam Penuh, Putih Penuh, dan Duotone dalam nol milidetik, isolasi satu tinta dengan mode Solo, lalu kemas seluruh paket identitas ke satu arsip ZIP rapi.",
      "tabs.recolor.p1": "Variasi warna instan: Asli, Hitam, Putih, dan Duotone tanpa trace ulang",
      "tabs.recolor.p2": "Isolasi warna Solo (Alt+Klik mata) sambil menjaga layer CutContour & Registrasi",
      "tabs.recolor.p3": "Ekspor Brand Kit (.zip) 1-klik berisi seluruh variasi warna, PNG, dan palet",
      "tabs.recolor.cap1": "Smart Recolor & isolasi Solo — variasi warna brand instan siap kemas",
      "tabs.audit.label": "Audit Pra-Cetak",
      "tabs.audit.title": "Audit Vektor Pra-Cetak & Perbaikan Otomatis",
      "tabs.audit.desc": "Cegah kendala mesin cutting dan cetak sebelum ekspor. Trasea memindai path mikro, node berlebih, dan bentuk belum tertutup, lalu membersihkannya dalam satu klik.",
      "tabs.audit.p1": "Mendeteksi bintik path mikro, node terlalu padat, dan geometri terbuka",
      "tabs.audit.p2": "Tombol Perbaiki Otomatis merapikan hasil tanpa mengorbankan detail gambar",
      "tabs.audit.p3": "Terintegrasi langsung di tab Ekspor berdampingan dengan Brand Kit ZIP",
      "tabs.audit.cap1": "Audit Pra-Cetak — periksa kesehatan vektor dan perbaiki sebelum cetak atau potong",
      "tabs.compare.label": "Bandingkan Offline vs AI",
      "tabs.compare.title": "Bandingkan Mesin Berdampingan (Offline vs AI)",
      "tabs.compare.desc": "Bandingkan hasil mesin vektor Rust lokal Trasea berdampingan dengan vektor impor AI CLI lengkap dengan metrik path, node, dan ukuran KB — lalu pilih hasil terbaik dalam satu klik.",
      "tabs.compare.p1": "Tampilan banding berdampingan dengan metrik path · node · KB nyata",
      "tabs.compare.p2": "Salin SVG (Ctrl+Shift+C) dan Brand Kit ZIP mendukung sumber Offline maupun AI",
      "tabs.compare.p3": "Mesin Offline tetap 100% privat tanpa perlu koneksi luar",
      "tabs.compare.cap1": "Bandingkan Mesin — bandingkan jumlah path, node, dan bobot SVG secara langsung",
      "tabs.production.label": "Die-Cut, Centerline & CMYK",
      "tabs.production.title": "Garis Potong Die-Cut, Centerline Laser & Tinta Spot CMYK",
      "tabs.production.desc": "Perangkat khusus untuk usaha stiker, sablon, dan bengkel fabrikasi: hapus latar 1-klik lengkap dengan kontur CutContour, garis tunggal Centerline untuk laser/CNC, serta Soft-Proof CMYK dengan separasi tinta Spot bernama.",
      "tabs.production.p1": "Hapus Latar 1-klik + kontur potong CutContour (Spot 0,25 pt)",
      "tabs.production.p2": "Mode Centerline menghasilkan garis Bézier terbuka tunggal untuk grafir & CNC",
      "tabs.production.p3": "Soft-Proof CMYK, Inspektor Kode Warna, dan PDF Named Spot /Separation",
      "tabs.production.cap1": "Hapus Latar & Garis Potong CutContour",
      "tabs.production.cap2": "Garis Tengah Tunggal (Laser / CNC)",
      "tabs.production.cap3": "Soft-Proof CMYK & Tinta Spot Bernama",
      "formats.eyebrow": "KELUARAN PRODUKSI",
      "formats.title": "Dari SVG siap web sampai CMYK siap cetak dan DXF CAD.",
      "formats.subtitle": "Untuk desainer web & brand, print shop, dan workshop laser — generator Rust murni tanpa perlu Ghostscript atau Inkscape.",
      "formats.svgTitle": "Vektor Berlayer & Clipboard",
      "formats.svgDesc": "Layer bernama (Latar, Bidang Warna, Garis Tipis, CutContour, Registrasi) plus Salin SVG 1-klik (Ctrl+Shift+C).",
      "formats.pdfTitle": "RGB, DeviceCMYK & Spot",
      "formats.pdfDesc": "Hitam K=100% murni & tanda registrasi serta tinta Named Spot /Separation termasuk CutContour 0,25 pt.",
      "formats.epsTitle": "PostScript Level 2",
      "formats.epsDesc": "Keluaran EPSF-3.0 asli dalam RGB, CMYK, atau Spot %%CMYKCustomColor untuk RIP cetak dan mesin cutting.",
      "formats.dxfTitle": "AutoCAD AC1009 CAD",
      "formats.dxfDesc": "Layer per warna, entitas CIRCLE asli, sumbu Y CAD, serta satuan mm (72/96/300 DPI atau lebar target) dan px.",
      "formats.palTitle": "Pustaka Palet Swatch",
      "formats.palDesc": "Ekspor Adobe Swatch Exchange (.ase Global/Spot), GIMP/Inkscape (.gpl), token JSON, dan variabel CSS.",
      "formats.kitTitle": "Paket Kirim Klien Siap Pakai",
      "formats.kitDesc": "Mengemas SVG Asli, Hitam, Putih, dan Duotone bersama pratinjau PNG resolusi tinggi dan palet dalam satu klik.",
      "formats.projTitle": "File Proyek Non-Destruktif",
      "formats.projDesc": "Menyimpan tautan SHA-256 sumber, potong/putar/hapus latar, goresan kuas, edit node, tinta spot, dan setelan ekspor.",
      "formats.sepTitle": "Film Sablon Hitam 100%-K",
      "formats.sepDesc": "Hasilkan film separasi hitam 100% / K=100% terpisah per warna tinta lengkap dengan tanda registrasi siap afdruk.",
      "download.eyebrow": "UNDUH",
      "download.title": "Pasang native di Windows atau macOS.",
      "download.subtitlePrefix": "Rilis terbaru",
      "download.subtitleSuffix": "· Gratis, open source, tanpa telemetri.",
      "download.recommended": "Rekomendasi",
      "download.winSetupTitle": "Installer Windows",
      "download.winSetupDesc": "Paket setup NSIS standar untuk Windows 10 dan Windows 11 lengkap dengan pintasan Start Menu.",
      "download.btnWinSetup": "Unduh .exe",
      "download.winPortableTitle": "Windows Portabel",
      "download.winPortableDesc": "Arsip ZIP mandiri — cukup ekstrak di folder mana saja dan jalankan tanpa izin administrator.",
      "download.btnWinPortable": "Unduh .zip",
      "download.macArmTitle": "macOS (M1 / M2 / M3 / M4)",
      "download.macArmDesc": "Paket disk image ARM64 asli yang dioptimalkan untuk Mac Apple Silicon.",
      "download.btnMacArm": "Unduh Apple Silicon .dmg",
      "download.macIntelTitle": "macOS (Intel)",
      "download.macIntelDesc": "Paket disk image x86_64 asli untuk komputer Mac berbasis prosesor Intel.",
      "download.btnMacIntel": "Unduh Intel .dmg",
      "download.noteWinTitle": "Windows SmartScreen",
      "download.noteWinDesc": "Build komunitas belum bertanda tangan digital. Jika SmartScreen muncul, pilih More info → Run anyway.",
      "download.noteMacTitle": "macOS Gatekeeper",
      "download.noteMacDesc": "Jika macOS menahan pembukaan pertama, jalankan <code>xattr -cr /Applications/Trasea.app</code> di Terminal atau izinkan lewat Privacy &amp; Security.",
      "download.noteVerifyTitle": "Checksum & Rilis",
      "download.noteReleasesLink": "GitHub Releases & SHA-256",
      "download.noteSigningLink": "Panduan verifikasi",
      "faq.title": "Pertanyaan yang sering diajukan",
      "faq.q1": "Apakah Trasea benar-benar gratis dan open source?",
      "faq.a1": "Ya. Trasea dirilis di bawah lisensi MIT. Seluruh fitur — termasuk PDF CMYK/Spot, AutoCAD DXF, Mockup Fotorealistis, Brand Kit ZIP, dan Audit Pra-Cetak — terbuka penuh tanpa akun, watermark, atau biaya langganan.",
      "faq.q2": "Apakah Trasea mengunggah gambar ke server luar?",
      "faq.a2": "Tidak. Praproses gambar, pelacakan kurva VTracer, edit node, hingga pembuatan PDF/DXF semuanya dieksekusi native oleh Rust di CPU/GPU kamu—tetap lancar meski Wi-Fi dimatikan.",
      "faq.q3": "Apa perbedaan Offline Tracing dan Impor AI CLI?",
      "faq.a3": "Offline Tracing memakai mesin Rust/VTracer bawaan Trasea untuk vektorisasi lokal seketika. Impor AI CLI memungkinkan pembandingan langsung hasil SVG dari tool AI CLI lokal berdampingan dengan metrik path, node, dan KB.",
      "faq.q4": "Sistem operasi apa saja yang didukung?",
      "faq.a4": "Trasea menyediakan build 64-bit asli untuk Windows 10/11 (Installer NSIS dan ZIP Portabel) serta macOS (paket DMG Apple Silicon ARM64 maupun Intel x64).",
      "faq.q5": "Bagaimana cara melapor bug atau mengusulkan fitur baru?",
      "faq.a5": "Buka issue langsung di GitHub Finarfin12/trasea/issues dengan menyertakan versi OS, langkah reproduksi, atau contoh kasus.",
      "footer.tagline": "Kurva bersih. Siap potong. Studio vektorisasi lokal dan open source.",
      "footer.releases": "Rilis",
      "footer.changelog": "Catatan Perubahan",
      "footer.license": "Lisensi MIT",
      "footer.issues": "Laporkan Bug",
      "footer.meta": "Lisensi MIT · Rust + Tauri · Tanpa telemetri"
    }
  };

  var state = {
    lang: "en",
    os: "other",
    release: FALLBACK_RELEASE,
    stars: null
  };

  function formatSize(bytes) {
    if (!bytes || typeof bytes !== "number") return "";
    var mb = bytes / (1024 * 1024);
    return mb.toFixed(1) + " MB";
  }

  function detectOS() {
    var platform = "";
    if (navigator.userAgentData && navigator.userAgentData.platform) {
      platform = String(navigator.userAgentData.platform).toLowerCase();
    } else if (navigator.platform) {
      platform = String(navigator.platform).toLowerCase();
    }
    var ua = String(navigator.userAgent || "").toLowerCase();
    if (/android|iphone|ipad|ipod/.test(ua)) return "mobile";
    if (platform.indexOf("win") !== -1 || ua.indexOf("windows") !== -1) return "windows";
    if (platform.indexOf("mac") !== -1 || ua.indexOf("mac os") !== -1 || ua.indexOf("macintosh") !== -1) return "macos";
    return "other";
  }

  function parseReleasePayload(data) {
    if (!data || !data.tag_name || !Array.isArray(data.assets)) return null;
    var mapped = {
      tag: data.tag_name,
      htmlUrl: data.html_url || FALLBACK_RELEASE.htmlUrl,
      assets: {
        winSetup: FALLBACK_RELEASE.assets.winSetup,
        winPortable: FALLBACK_RELEASE.assets.winPortable,
        macArm: FALLBACK_RELEASE.assets.macArm,
        macIntel: FALLBACK_RELEASE.assets.macIntel
      }
    };

    data.assets.forEach(function (asset) {
      var name = String(asset.name || "");
      var item = {
        name: name,
        url: asset.browser_download_url,
        size: asset.size || 0
      };
      if (/_x64-setup\.exe$/i.test(name)) mapped.assets.winSetup = item;
      else if (/_x64-portable\.zip$/i.test(name)) mapped.assets.winPortable = item;
      else if (/_aarch64\.dmg$/i.test(name)) mapped.assets.macArm = item;
      else if (/_x64\.dmg$/i.test(name)) mapped.assets.macIntel = item;
    });

    return mapped;
  }

  function loadLatestRelease() {
    try {
      var raw = sessionStorage.getItem(CACHE_KEY);
      if (raw) {
        var cached = JSON.parse(raw);
        if (cached && cached.ts && Date.now() - cached.ts < CACHE_TTL_MS && cached.release) {
          state.release = cached.release;
          applyReleaseUI();
          return;
        }
      }
    } catch (_) {
      // Ignore storage errors
    }

    if (typeof fetch !== "function") return;

    fetch(RELEASE_API, { headers: { Accept: "application/vnd.github+json" } })
      .then(function (res) {
        if (!res.ok) throw new Error("GitHub API status " + res.status);
        return res.json();
      })
      .then(function (json) {
        var parsed = parseReleasePayload(json);
        if (!parsed) return;
        state.release = parsed;
        try {
          sessionStorage.setItem(CACHE_KEY, JSON.stringify({ ts: Date.now(), release: parsed }));
        } catch (_) {
          // Ignore storage quota errors
        }
        applyReleaseUI();
      })
      .catch(function () {
        // Silent fallback to FALLBACK_RELEASE (v1.8.2)
      });
  }

  function applyRepoMetaUI() {
    var starPill = document.getElementById("github-star-pill");
    var proofStrip = document.getElementById("hero-proof-strip");
    var hasStars = typeof state.stars === "number" && state.stars >= 1;

    if (starPill) {
      starPill.textContent = hasStars ? "★ " + state.stars : t("github.starFallback");
    }

    if (proofStrip) {
      var baseProof = t("hero.proof");
      proofStrip.textContent = hasStars
        ? baseProof + " · ★ " + state.stars + " " + t("hero.proofStars")
        : baseProof;
    }
  }

  function loadRepoMeta() {
    try {
      var raw = sessionStorage.getItem(REPO_CACHE_KEY);
      if (raw) {
        var cached = JSON.parse(raw);
        if (cached && cached.ts && Date.now() - cached.ts < CACHE_TTL_MS && typeof cached.stars === "number") {
          state.stars = cached.stars;
          applyRepoMetaUI();
          return;
        }
      }
    } catch (_) {
      // Ignore storage errors
    }

    if (typeof fetch !== "function") return;

    fetch(REPO_API, { headers: { Accept: "application/vnd.github+json" } })
      .then(function (res) {
        if (!res.ok) throw new Error("GitHub Repo API status " + res.status);
        return res.json();
      })
      .then(function (json) {
        if (!json || typeof json.stargazers_count !== "number") return;
        state.stars = json.stargazers_count;
        try {
          sessionStorage.setItem(REPO_CACHE_KEY, JSON.stringify({ ts: Date.now(), stars: state.stars }));
        } catch (_) {
          // Ignore storage quota errors
        }
        applyRepoMetaUI();
      })
      .catch(function () {
        // Silent fallback without showing ★ 0 or console errors
      });
  }

  function t(key) {
    var dict = I18N[state.lang] || I18N.en;
    return Object.prototype.hasOwnProperty.call(dict, key) ? dict[key] : (I18N.en[key] || key);
  }

  function applyReleaseUI() {
    var rel = state.release || FALLBACK_RELEASE;
    var tag = rel.tag || FALLBACK_RELEASE.tag;

    var heroVer = document.getElementById("hero-version");
    var chromeVer = document.getElementById("chrome-version");
    var dlVer = document.getElementById("download-version");
    if (heroVer) heroVer.textContent = tag;
    if (chromeVer) chromeVer.textContent = tag;
    if (dlVer) dlVer.textContent = tag;

    var allRelLink = document.getElementById("link-all-releases");
    if (allRelLink && rel.htmlUrl) allRelLink.href = rel.htmlUrl;

    var slots = [
      { id: "win-setup", asset: rel.assets.winSetup },
      { id: "win-portable", asset: rel.assets.winPortable },
      { id: "mac-arm", asset: rel.assets.macArm },
      { id: "mac-intel", asset: rel.assets.macIntel }
    ];

    slots.forEach(function (slot) {
      if (!slot.asset) return;
      var linkEl = document.getElementById("link-" + slot.id);
      var metaEl = document.getElementById("meta-" + slot.id);
      if (linkEl && slot.asset.url) linkEl.href = slot.asset.url;
      if (metaEl) {
        var sz = formatSize(slot.asset.size);
        metaEl.textContent = sz ? slot.asset.name + " · " + sz : slot.asset.name;
      }
    });

    // Primary hero CTA based on detected OS
    var primaryBtn = document.getElementById("primary-download-btn");
    var primaryLabel = document.getElementById("primary-download-label");
    var assetMeta = document.getElementById("hero-asset-meta");
    var altLink = document.getElementById("hero-alt-link");

    var cards = document.querySelectorAll(".dl-card");
    cards.forEach(function (card) {
      card.classList.remove("is-recommended");
      var badge = card.querySelector(".dl-rec-badge");
      if (badge) badge.hidden = true;
    });

    if (state.os === "windows" && rel.assets.winSetup) {
      if (primaryBtn) primaryBtn.href = rel.assets.winSetup.url;
      if (primaryLabel) primaryLabel.textContent = t("hero.ctaWin");
      if (assetMeta) {
        assetMeta.textContent = rel.assets.winSetup.name + " · " + formatSize(rel.assets.winSetup.size);
      }
      if (altLink) {
        altLink.href = "#download";
        altLink.textContent = t("hero.altWin");
      }
      highlightCard("win-setup");
    } else if (state.os === "macos" && rel.assets.macArm) {
      if (primaryBtn) primaryBtn.href = rel.assets.macArm.url;
      if (primaryLabel) primaryLabel.textContent = t("hero.ctaMac");
      if (assetMeta) {
        assetMeta.textContent = rel.assets.macArm.name + " · " + formatSize(rel.assets.macArm.size);
      }
      if (altLink && rel.assets.macIntel) {
        altLink.href = rel.assets.macIntel.url;
        altLink.textContent = t("hero.altMac");
      }
      highlightCard("mac-arm");
    } else {
      if (primaryBtn) primaryBtn.href = "#download";
      if (primaryLabel) primaryLabel.textContent = t("hero.ctaFallback");
      if (assetMeta) assetMeta.textContent = tag + " · Windows & macOS";
      if (altLink) {
        altLink.href = rel.htmlUrl || "https://github.com/Finarfin12/trasea/releases/latest";
        altLink.textContent = t("hero.altLinkDefault");
      }
    }
  }

  function highlightCard(platformKey) {
    var card = document.querySelector('.dl-card[data-platform="' + platformKey + '"]');
    if (!card) return;
    card.classList.add("is-recommended");
    var badge = card.querySelector(".dl-rec-badge");
    if (badge) badge.hidden = false;
  }

  function applyLanguage(lang) {
    state.lang = lang === "id" ? "id" : "en";
    document.documentElement.lang = state.lang;

    try {
      localStorage.setItem(LANG_KEY, state.lang);
    } catch (_) {
      // Ignore storage errors
    }

    var textNodes = document.querySelectorAll("[data-i18n]");
    textNodes.forEach(function (el) {
      var key = el.getAttribute("data-i18n");
      var val = t(key);
      if (key === "download.noteMacDesc") {
        el.innerHTML = val;
      } else {
        el.textContent = val;
      }
    });

    var ariaNodes = document.querySelectorAll("[data-i18n-aria]");
    ariaNodes.forEach(function (el) {
      var key = el.getAttribute("data-i18n-aria");
      el.setAttribute("aria-label", t(key));
    });

    var langButtons = document.querySelectorAll(".lang-btn");
    langButtons.forEach(function (btn) {
      var active = btn.getAttribute("data-lang") === state.lang;
      btn.classList.toggle("is-active", active);
      btn.setAttribute("aria-pressed", active ? "true" : "false");
    });

    applyReleaseUI();
    applyRepoMetaUI();
  }

  function initTabs() {
    var tabs = Array.prototype.slice.call(document.querySelectorAll(".tab-btn"));
    var panels = Array.prototype.slice.call(document.querySelectorAll(".tab-panel"));
    if (!tabs.length) return;

    function activateTab(tabId) {
      tabs.forEach(function (btn) {
        var active = btn.getAttribute("data-tab") === tabId;
        btn.classList.toggle("is-active", active);
        btn.setAttribute("aria-selected", active ? "true" : "false");
      });
      panels.forEach(function (panel) {
        var match = panel.id === "panel-" + tabId;
        panel.classList.toggle("is-active", match);
        panel.hidden = !match;
      });
    }

    tabs.forEach(function (btn, idx) {
      btn.addEventListener("click", function () {
        activateTab(btn.getAttribute("data-tab"));
      });
      btn.addEventListener("keydown", function (e) {
        var nextIdx = -1;
        if (e.key === "ArrowRight") nextIdx = (idx + 1) % tabs.length;
        else if (e.key === "ArrowLeft") nextIdx = (idx - 1 + tabs.length) % tabs.length;
        if (nextIdx !== -1) {
          e.preventDefault();
          tabs[nextIdx].focus();
          activateTab(tabs[nextIdx].getAttribute("data-tab"));
        }
      });
    });
  }

  function initLanguageToggle() {
    var initial = "en";
    try {
      var saved = localStorage.getItem(LANG_KEY);
      if (saved === "id" || saved === "en") initial = saved;
    } catch (_) {
      // Ignore storage errors
    }

    var buttons = document.querySelectorAll(".lang-btn");
    buttons.forEach(function (btn) {
      btn.addEventListener("click", function () {
        applyLanguage(btn.getAttribute("data-lang"));
      });
    });

    applyLanguage(initial);
  }

  function init() {
    state.os = detectOS();
    initTabs();
    initLanguageToggle();
    loadLatestRelease();
    loadRepoMeta();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", init);
  } else {
    init();
  }
})();
