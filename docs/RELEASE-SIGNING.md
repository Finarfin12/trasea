# Panduan Build Rilis, Code Signing, & Distribusi — Trasea

Dokumen ini merangkum langkah persiapan distribusi Trasea untuk **Windows (Authenticode)** dan **macOS (Universal Binary + Notarization)**. Trasea gratis dan tidak memakai sistem lisensi, sehingga tidak ada kunci aktivasi yang perlu dikelola.

---

## 1. Windows Code Signing (Authenticode + NSIS Installer)

Konfigurasi di `src-tauri/tauri.conf.json` sudah menyiapkan field `bundle.windows.certificateThumbprint`, `digestAlgorithm: "sha256"`, dan `timestampUrl`.

### Opsi A — Sertifikat OV/EV (DigiCert / Sectigo / SSL.com) di Windows Certificate Store
1. Pasang sertifikat atau USB token EV di mesin build Windows, lalu catat **SHA-1 Thumbprint** dari `certmgr.msc`.
2. Buat file konfigurasi overlay lokal `src-tauri/tauri.release.conf.json` (jangan commit thumbprint privat bila tidak perlu):
   ```json
   {
     "bundle": {
       "windows": {
         "certificateThumbprint": "YOUR_40_CHAR_SHA1_THUMBPRINT",
         "digestAlgorithm": "sha256",
         "timestampUrl": "http://timestamp.digicert.com"
       }
     }
   }
   ```
3. Jalankan build rilis dengan overlay:
   ```powershell
   npx tauri build --target x86_64-pc-windows-gnu --config src-tauri/tauri.release.conf.json
   ```

### Opsi B — Microsoft Azure Trusted Signing
1. Siapkan akun Azure Trusted Signing & endpoint profil.
2. Gunakan `signtool.exe` + `Azure.CodeSigning.Dlib.dll` melalui `bundle.windows.signCommand` di `tauri.release.conf.json`.
3. Verifikasi tanda tangan digital hasil build:
   ```powershell
   signtool verify /pa /v "target\x86_64-pc-windows-gnu\release\bundle\nsis\Trasea_1.4.0_x64-setup.exe"
   ```

---

## 2. macOS Universal Binary & Notarization (Apple Silicon + Intel)

Konfigurasi `src-tauri/tauri.conf.json` sudah menyertakan:
- `icons/icon.icns` (logo huruf `T` Trasea, 58 KB)
- `bundle.macOS.minimumSystemVersion: "11.0"`
- `resources/sample/`

### Langkah build di mesin macOS (Xcode Command Line Tools terpasang)
1. Pasang kedua target Rust:
   ```bash
   rustup target add aarch64-apple-darwin x86_64-apple-darwin
   ```
2. Unduh biner macOS `realesrgan-ncnn-vulkan` (release v0.2.5.0 macOS) ke `src-tauri/resources/realesrgan/` (berikan izin eksekusi `chmod +x`).
3. Set variabel lingkungan Apple Developer ID & Notarization:
   ```bash
   export APPLE_SIGNING_IDENTITY="Developer ID Application: Your Name (TEAMID)"
   export APPLE_ID="you@example.com"
   export APPLE_PASSWORD="app-specific-password"
   export APPLE_TEAM_ID="TEAMID"
   ```
4. Build Universal Binary (`.app` + `.dmg`):
   ```bash
   npm ci
   npx tauri build --target universal-apple-darwin
   ```
5. Verifikasi tanda tangan & notarization:
   ```bash
   codesign --verify --deep --strict --verbose=2 "target/universal-apple-darwin/release/bundle/macos/Trasea.app"
   spctl --assess --type execute --verbose "target/universal-apple-darwin/release/bundle/macos/Trasea.app"
   ```

---

## 3. Daftar Cek Sebelum Mengunggah Installer (GitHub Releases / Web Sendiri)

- [ ] Installer Windows lolos `signtool verify /pa` tanpa peringatan SmartScreen merah.
- [ ] File `THIRD_PARTY_LICENSES.md` ikut terpasang di folder aplikasi (terverifikasi otomatis lewat `bundle.resources`).
- [ ] Uji manual `M-01` (EPS di Illustrator / Affinity / CorelDraw) dan uji pasang di mesin bersih tanpa internet.
