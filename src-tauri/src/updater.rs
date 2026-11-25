//! Privacy-first lightweight update checker (QT-1=A, QT-2=A).
//!
//! - Uses OS `curl` (built into Windows 10/11) with strict 5s connect / 8s total timeout,
//!   zero cookies, zero user IDs, and zero telemetry, keeping `trasea-core` at 0 network crates
//!   and WebView2 strictly isolated.
//! - Queries only the public GitHub Release endpoint `Finarfin12/trasea/releases/latest`.

use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use std::process::Command;

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const DEFAULT_RELEASES_API: &str = "https://api.github.com/repos/Finarfin12/trasea/releases/latest";
const DEFAULT_RELEASES_WEB: &str = "https://github.com/Finarfin12/trasea/releases/latest";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_notes: String,
    pub download_url: String,
    pub is_portable: bool,
    pub asset_name: Option<String>,
    pub asset_url: Option<String>,
    pub asset_size: Option<u64>,
    pub sha256_url: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppModeInfo {
    pub is_portable: bool,
    pub current_version: String,
}

pub fn is_portable_mode() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("portable.flag").exists()))
        .unwrap_or(false)
}

fn parse_semver(v: &str) -> Option<(u64, u64, u64)> {
    let clean = v.trim().trim_start_matches('v').trim_start_matches('V');
    let core = clean.split('-').next().unwrap_or(clean);
    let mut parts = core.split('.');
    let maj = parts.next()?.parse::<u64>().ok()?;
    let min = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    let pat = parts.next().unwrap_or("0").parse::<u64>().ok()?;
    Some((maj, min, pat))
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_semver(latest), parse_semver(current)) {
        (Some(l), Some(c)) => l > c,
        _ => false,
    }
}

pub fn check_latest_release() -> Result<UpdateInfo, String> {
    let current = env!("CARGO_PKG_VERSION").to_string();
    let url = std::env::var("TRASEA_UPDATE_URL")
        .ok()
        .filter(|u| !u.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASES_API.to_string());

    let mut cmd = Command::new("curl");
    cmd.args([
        "--disable",
        "--silent",
        "--show-error",
        "--fail",
        "--location",
        "--connect-timeout",
        "5",
        "--max-time",
        "8",
        "-H",
        &format!("User-Agent: Trasea/{current}"),
        "-H",
        "Accept: application/vnd.github+json",
        &url,
    ]);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let out = cmd
        .output()
        .map_err(|_| "Tidak dapat menghubungi server pembaruan (periksa koneksi internet).".to_string())?;

    if !out.status.success() {
        return Err("Tidak dapat terhubung ke GitHub Releases (sedang offline atau timeout).".to_string());
    }

    let body = String::from_utf8_lossy(&out.stdout);
    let json: serde_json::Value = serde_json::from_str(&body)
        .map_err(|_| "Respons server pembaruan tidak valid.".to_string())?;

    let tag = json
        .get("tag_name")
        .and_then(|v| v.as_str())
        .unwrap_or(&current);
    let latest = tag.trim().trim_start_matches('v').trim_start_matches('V').to_string();
    let download_url = json
        .get("html_url")
        .and_then(|v| v.as_str())
        .filter(|u| u.starts_with("https://github.com/Finarfin12/trasea/"))
        .unwrap_or(DEFAULT_RELEASES_WEB)
        .to_string();
    let raw_notes = json
        .get("body")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    let release_notes: String = raw_notes.chars().take(600).collect();

    let is_portable = is_portable_mode();
    let assets: Vec<ReleaseAsset> = json["assets"].as_array().into_iter().flatten()
        .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
        .collect();
    let asset = select_release_asset(&assets, std::env::consts::OS, std::env::consts::ARCH, is_portable);
    let checksum = asset.and_then(|asset| select_checksum(&assets, asset));

    Ok(UpdateInfo {
        has_update: is_newer(&latest, &current),
        current_version: current,
        latest_version: latest,
        release_notes,
        download_url,
        is_portable,
        asset_name: asset.map(|a| a.name.clone()),
        asset_url: asset.map(|a| a.browser_download_url.clone()),
        asset_size: asset.map(|a| a.size),
        sha256_url: checksum.map(|a| a.browser_download_url.clone()),
    })
}

const RELEASE_ASSET_PREFIX: &str = "https://github.com/Finarfin12/trasea/releases/download/";

#[derive(Debug, Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

fn asset_tag<'a>(url: &'a str, name: &str) -> Option<&'a str> {
    let (tag, basename) = url.strip_prefix(RELEASE_ASSET_PREFIX)?.split_once('/')?;
    let safe = |value: &str| !value.is_empty() && !value.contains("..")
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b));
    (safe(tag) && safe(basename) && basename == name).then_some(tag)
}

fn installer_suffix(os: &str, arch: &str, portable: bool) -> Option<&'static str> {
    match (os, arch, portable) {
        ("windows", _, true) => Some("_x64-portable.zip"),
        ("windows", _, false) => Some("_x64-setup.exe"),
        ("macos", "aarch64", _) => Some("_aarch64.dmg"),
        ("macos", "x86_64", _) => Some("_x64.dmg"),
        _ => None,
    }
}

fn select_release_asset<'a>(assets: &'a [ReleaseAsset], os: &str, arch: &str, portable: bool) -> Option<&'a ReleaseAsset> {
    let suffix = installer_suffix(os, arch, portable)?;
    assets.iter().find(|asset| asset.name.starts_with("Trasea_") && asset.name.ends_with(suffix)
        && asset_tag(&asset.browser_download_url, &asset.name).is_some())
}

fn select_checksum<'a>(assets: &'a [ReleaseAsset], asset: &ReleaseAsset) -> Option<&'a ReleaseAsset> {
    let tag = asset_tag(&asset.browser_download_url, &asset.name)?;
    let sidecar = format!("{}.sha256", asset.name);
    let find = |name: &str| {
        assets.iter().find(|entry| entry.name == name && asset_tag(&entry.browser_download_url, name) == Some(tag))
    };
    find(&sidecar).or_else(|| find("SHA256SUMS.txt"))
}

fn verify_checksum(path: &Path, checksum: &str, basename: &str, is_sidecar: bool) -> Result<(), String> {
    let mut hashes = checksum.lines().filter_map(|line| {
        let (hash, name) = line.trim().split_once(char::is_whitespace).unwrap_or((line.trim(), ""));
        let name = name.trim().trim_start_matches('*');
        (name == basename || (is_sidecar && name.is_empty())).then_some(hash)
    });
    let hash = hashes.next().filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or("Missing or invalid SHA-256 checksum.")?;
    if hashes.next().is_some() {
        return Err("Multiple SHA-256 checksums found for this installer.".into());
    }
    let actual = trasea_core::project::sha256_file(path).map_err(|e| e.to_string())?;
    if !actual.eq_ignore_ascii_case(hash) {
        return Err("SHA-256 mismatch. The installer was discarded.".into());
    }
    Ok(())
}

fn download(url: &str, destination: &Path, max_bytes: u64) -> Result<(), String> {
    let mut cmd = Command::new("curl");
    cmd.args(["--disable", "--silent", "--show-error", "--fail", "--location", "--max-redirs", "5",
        "--proto", "=https", "--proto-redir", "=https", "--connect-timeout", "10", "--max-time", "180",
        "--max-filesize", &max_bytes.to_string(), "--output"])
        .arg(destination).arg("--").arg(url);
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000);
    let status = cmd.output().map_err(|e| e.to_string())?;
    if !status.status.success() {
        return Err("Download failed. Check your connection and try again.".into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadUpdateResult {
    saved_path: String,
    sha256_verified: bool,
    launched: bool,
}

pub fn download_and_launch_update(downloads: &Path, url: &str, name: &str, checksum_url: Option<&str>, portable: bool) -> Result<DownloadUpdateResult, String> {
    let tag = asset_tag(url, name).ok_or("Installer URL is outside the Trasea release repository.")?;
    let latest = check_latest_release()?;
    if !latest.has_update || latest.is_portable != portable || latest.asset_name.as_deref() != Some(name)
        || latest.asset_url.as_deref() != Some(url) || latest.sha256_url.as_deref() != checksum_url {
        return Err("The update has changed. Check for updates again.".into());
    }
    let size = latest.asset_size.filter(|size| *size > 0).ok_or("Invalid installer size.")?;
    fs::create_dir_all(downloads).map_err(|e| e.to_string())?;
    let nonce = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_nanos();
    let staging = downloads.join(format!(".trasea-update-{}-{nonce}", std::process::id()));
    fs::create_dir(&staging).map_err(|e| e.to_string())?;
    let final_path = downloads.join(name);
    let result = (|| {
        let part = staging.join(format!("{name}.part"));
        download(url, &part, size)?;
        if fs::metadata(&part).map_err(|e| e.to_string())?.len() != size {
            return Err("The installer download is incomplete.".into());
        }
        if let Some(checksum_url) = checksum_url {
            let sidecar = format!("{name}.sha256");
            let is_sidecar = asset_tag(checksum_url, &sidecar) == Some(tag);
            if !is_sidecar && asset_tag(checksum_url, "SHA256SUMS.txt") != Some(tag) {
                return Err("Invalid checksum URL.".into());
            }
            let checksum = staging.join("checksum");
            download(checksum_url, &checksum, 1024 * 1024)?;
            verify_checksum(&part, &fs::read_to_string(checksum).map_err(|e| e.to_string())?, name, is_sidecar)?;
        }
        fs::hard_link(&part, &final_path).or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists
                && trasea_core::project::sha256_file(&part).ok().zip(trasea_core::project::sha256_file(&final_path).ok())
                    .is_some_and(|(a, b)| a == b) {
                Ok(())
            } else {
                Err(error)
            }
        }).map_err(|e| format!("Could not save installer: {e}"))?;
        Ok(DownloadUpdateResult {
            saved_path: final_path.to_string_lossy().into_owned(),
            sha256_verified: checksum_url.is_some(),
            launched: launch_installer(&final_path),
        })
    })();
    let _ = fs::remove_dir_all(staging);
    result
}

fn launch_installer(path: &Path) -> bool {
    #[cfg(target_os = "windows")]
    {
        if path.extension().is_some_and(|ext| ext == "zip") {
            return Command::new("explorer.exe").arg(format!("/select,{}", path.display())).spawn().is_ok();
        }
        Command::new(path).spawn().is_ok()
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(path).status().is_ok_and(|status| status.success())
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        let _ = path;
        false
    }
}

pub fn open_download_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/Finarfin12/trasea/") {
        return Err("URL tidak diizinkan.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("explorer.exe");
        cmd.arg(url);
        cmd.creation_flags(0x08000000);
        cmd.spawn()
            .map_err(|e| format!("Gagal membuka browser: {e}"))?;
        Ok(())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Gagal membuka browser: {e}"))?;
        Ok(())
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|e| format!("Gagal membuka browser: {e}"))?;
        Ok(())
    }
    #[cfg(not(any(target_os = "windows", unix)))]
    {
        let _ = url;
        Err("Membuka browser tidak didukung di platform ini.".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selects_all_platform_installers_and_the_correct_checksum() {
        let suffixes = ["_x64-setup.exe", "_x64-portable.zip", "_aarch64.dmg", "_x64.dmg"];
        let targets = [("windows","x86_64",false), ("windows","x86_64",true), ("macos","aarch64",false), ("macos","x86_64",false)];
        let mut assets: Vec<_> = suffixes.iter().map(|suffix| {
            let name = format!("Trasea_1.9.0{suffix}");
            ReleaseAsset { browser_download_url: format!("{RELEASE_ASSET_PREFIX}v1.9.0/{name}"), name, size: 64 }
        }).collect();
        assets.push(ReleaseAsset { name: "SHA256SUMS.txt".into(), browser_download_url: format!("{RELEASE_ASSET_PREFIX}v1.9.0/SHA256SUMS.txt"), size: 64 });
        let name = format!("{}.sha256", assets[0].name);
        assets.push(ReleaseAsset { browser_download_url: format!("{RELEASE_ASSET_PREFIX}v1.9.0/{name}"), name, size: 64 });

        for (index, (os, arch, portable)) in targets.into_iter().enumerate() {
            let installer = select_release_asset(&assets, os, arch, portable).unwrap();
            assert_eq!(installer.name, assets[index].name);
            assert_eq!(select_checksum(&assets, installer).unwrap().name, assets[if index == 0 { 5 } else { 4 }].name);
        }
        assert!(select_release_asset(&assets, "linux", "x86_64", false).is_none());
    }

    #[test]
    fn rejects_urls_outside_exact_repo_asset_paths() {
        for url in [
            "https://github.com/other/trasea/releases/download/v1/setup.exe",
            "https://github.com/Finarfin12/trasea/releases/download/../setup.exe",
            "https://github.com/Finarfin12/trasea/releases/download/v1/%2esetup.exe",
            "https://github.com/Finarfin12/trasea/releases/download/v1/setup.exe?query",
        ] {
            assert!(asset_tag(url, "setup.exe").is_none());
        }
        assert_eq!(asset_tag(&format!("{RELEASE_ASSET_PREFIX}v1/setup.exe"), "setup.exe"), Some("v1"));
    }

    #[test]
    fn verifies_only_a_matching_checksum() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let hash = trasea_core::project::sha256_file(&path).unwrap();
        assert!(verify_checksum(&path, &format!("{hash}  installer.exe"), "installer.exe", false).is_ok());
        assert!(verify_checksum(&path, &hash, "installer.exe", true).is_ok());
        assert!(verify_checksum(&path, &format!("{}  installer.exe", "0".repeat(64)), "installer.exe", false).is_err());
        assert!(verify_checksum(&path, &format!("{hash}  other.exe"), "installer.exe", false).is_err());
    }
}
