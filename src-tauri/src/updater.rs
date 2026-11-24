//! Privacy-first lightweight update checker (QT-1=A, QT-2=A).
//!
//! - Uses OS `curl` (built into Windows 10/11) with strict 5s connect / 8s total timeout,
//!   zero cookies, zero user IDs, and zero telemetry, keeping `trasea-core` at 0 network crates
//!   and WebView2 strictly isolated.
//! - Queries only the public GitHub Release endpoint `Finarfin12/trasea/releases/latest`.

use serde::Serialize;
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

    Ok(UpdateInfo {
        has_update: is_newer(&latest, &current),
        current_version: current,
        latest_version: latest,
        release_notes,
        download_url,
        is_portable: is_portable_mode(),
    })
}

pub fn open_download_url(url: &str) -> Result<(), String> {
    if !url.starts_with("https://github.com/Finarfin12/trasea/") {
        return Err("URL tidak diizinkan.".to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("cmd");
        cmd.args(["/C", "start", "", url]);
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
