//! External AI vectorization engine (BYOK APIs via OS `curl` + safe local CLI hook).
//!
//! Security & privacy invariants:
//! - Zero network crates added (`trasea-core` remains 0 network crates, 0 GPL/AGPL).
//! - WebView2 / WKWebView stays strictly isolated; all external calls happen here only on
//!   explicit user action.
//! - API keys are stored separately in `ai-keys.json` (mode `0600` on Unix), never exported
//!   to `.trasea` projects, never returned raw to the WebView, and always masked in errors.
//! - Zero hardcoded model lists: `fetch_ai_models` queries official provider endpoints and
//!   returns `models: []` with a clear connection status when unconfigured or offline.

use base64::Engine;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

const AI_CONFIG_FILE: &str = "ai-config.json";
const AI_KEYS_FILE: &str = "ai-keys.json";
const API_TIMEOUT_SECS: &str = "90";
const MODELS_TIMEOUT_SECS: &str = "15";
const DEFAULT_CLI_TIMEOUT_SEC: u64 = 120;
const STDERR_SNIPPET_MAX: usize = 300;

static TEMP_SEQ: AtomicU64 = AtomicU64::new(1);

const VISION_SVG_PROMPT: &str = "Convert this raster image into a clean, compact, standalone SVG vector graphic. \
Use clean cubic Bezier paths and basic geometric shapes with flat solid hex fills matching the image colours, \
a proper viewBox matching the image aspect ratio, no embedded raster <image> tags, no <script>, no external links. \
Return ONLY the valid <svg ...>...</svg> markup.";

fn default_provider() -> String {
    "local".to_string()
}

fn default_cli_timeout_sec() -> u64 {
    DEFAULT_CLI_TIMEOUT_SEC
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    #[serde(default = "default_provider")]
    pub ai_provider: String,
    #[serde(default)]
    pub ai_claude_model: String,
    #[serde(default)]
    pub ai_openai_model: String,
    #[serde(default)]
    pub ai_gemini_model: String,
    #[serde(default)]
    pub ai_cli_executable: String,
    #[serde(default)]
    pub ai_cli_args: Vec<String>,
    #[serde(default = "default_cli_timeout_sec")]
    pub ai_cli_timeout_sec: u64,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            ai_provider: default_provider(),
            ai_claude_model: String::new(),
            ai_openai_model: String::new(),
            ai_gemini_model: String::new(),
            ai_cli_executable: String::new(),
            ai_cli_args: Vec::new(),
            ai_cli_timeout_sec: default_cli_timeout_sec(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiKeys {
    #[serde(default)]
    pub recraft_api_key: String,
    #[serde(default)]
    pub vectorizer_api_id: String,
    #[serde(default)]
    pub vectorizer_api_secret: String,
    #[serde(default)]
    pub claude_api_key: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default)]
    pub gemini_api_key: String,
}

/// Public view of AI configuration returned to the WebView (never includes raw secrets).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfigPublic {
    pub ai_provider: String,
    pub ai_claude_model: String,
    pub ai_openai_model: String,
    pub ai_gemini_model: String,
    pub ai_cli_executable: String,
    pub ai_cli_args: Vec<String>,
    pub ai_cli_timeout_sec: u64,
    pub has_recraft_key: bool,
    pub has_vectorizer_keys: bool,
    pub has_claude_key: bool,
    pub has_openai_key: bool,
    pub has_gemini_key: bool,
}

/// Payload accepted by `save_ai_config`. Secret fields are `Option<String>`:
/// - `None`: keep existing stored key unchanged
/// - `Some("")`: clear stored key
/// - `Some(v)`: overwrite stored key with trimmed `v`
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAiConfigInput {
    pub ai_provider: Option<String>,
    pub ai_claude_model: Option<String>,
    pub ai_openai_model: Option<String>,
    pub ai_gemini_model: Option<String>,
    pub ai_cli_executable: Option<String>,
    pub ai_cli_args: Option<Vec<String>>,
    pub ai_cli_timeout_sec: Option<u64>,
    pub recraft_api_key: Option<String>,
    pub vectorizer_api_id: Option<String>,
    pub vectorizer_api_secret: Option<String>,
    pub claude_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub gemini_api_key: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AiModelItem {
    pub id: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FetchAiModelsResult {
    pub provider: String,
    /// `"connected"`, `"api_key_required"`, `"offline"`, `"error"`, or `"not_applicable"`
    pub status: String,
    pub message: String,
    pub models: Vec<AiModelItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiTestResult {
    pub ok: bool,
    pub provider: String,
    pub message: String,
    pub resolved_path: Option<String>,
}

/// RAII temporary directory that always cleans itself up on drop.
pub struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    pub fn new(prefix: &str) -> Result<Self, String> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let seq = TEMP_SEQ.fetch_add(1, AtomicOrdering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "{prefix}-{}-{nanos}-{seq}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir)
            .map_err(|e| format!("Gagal membuat folder sementara AI: {e}"))?;
        Ok(Self { path: dir })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn normalize_provider(raw: &str) -> String {
    match raw.trim().to_ascii_lowercase().as_str() {
        "recraft" => "recraft",
        "vectorizer_ai" | "vectorizer" => "vectorizer_ai",
        "claude" | "anthropic" => "claude",
        "openai" | "gpt" => "openai",
        "gemini" | "google" => "gemini",
        "cli" => "cli",
        _ => "local",
    }
    .to_string()
}

pub fn load_ai_config(dir: &Path) -> AiConfig {
    let path = dir.join(AI_CONFIG_FILE);
    let mut cfg: AiConfig = std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    cfg.ai_provider = normalize_provider(&cfg.ai_provider);
    cfg.ai_cli_timeout_sec = cfg.ai_cli_timeout_sec.clamp(10, 300);
    cfg
}

pub fn load_ai_keys(dir: &Path) -> AiKeys {
    let path = dir.join(AI_KEYS_FILE);
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn get_ai_config_public(dir: &Path) -> AiConfigPublic {
    let cfg = load_ai_config(dir);
    let keys = load_ai_keys(dir);
    AiConfigPublic {
        ai_provider: cfg.ai_provider,
        ai_claude_model: cfg.ai_claude_model,
        ai_openai_model: cfg.ai_openai_model,
        ai_gemini_model: cfg.ai_gemini_model,
        ai_cli_executable: cfg.ai_cli_executable,
        ai_cli_args: cfg.ai_cli_args,
        ai_cli_timeout_sec: cfg.ai_cli_timeout_sec,
        has_recraft_key: !keys.recraft_api_key.trim().is_empty(),
        has_vectorizer_keys: !keys.vectorizer_api_id.trim().is_empty()
            && !keys.vectorizer_api_secret.trim().is_empty(),
        has_claude_key: !keys.claude_api_key.trim().is_empty(),
        has_openai_key: !keys.openai_api_key.trim().is_empty(),
        has_gemini_key: !keys.gemini_api_key.trim().is_empty(),
    }
}

pub fn save_ai_config(dir: &Path, input: SaveAiConfigInput) -> Result<AiConfigPublic, String> {
    std::fs::create_dir_all(dir)
        .map_err(|e| format!("Gagal membuat folder konfigurasi AI: {e}"))?;

    let mut cfg = load_ai_config(dir);
    if let Some(p) = input.ai_provider {
        cfg.ai_provider = normalize_provider(&p);
    }
    if let Some(m) = input.ai_claude_model {
        cfg.ai_claude_model = m.trim().to_string();
    }
    if let Some(m) = input.ai_openai_model {
        cfg.ai_openai_model = m.trim().to_string();
    }
    if let Some(m) = input.ai_gemini_model {
        cfg.ai_gemini_model = m.trim().to_string();
    }
    if let Some(exe) = input.ai_cli_executable {
        cfg.ai_cli_executable = exe.trim().to_string();
    }
    if let Some(args) = input.ai_cli_args {
        cfg.ai_cli_args = args;
    }
    if let Some(t) = input.ai_cli_timeout_sec {
        cfg.ai_cli_timeout_sec = t.clamp(10, 300);
    }

    let cfg_json = serde_json::to_string_pretty(&cfg)
        .map_err(|e| format!("Gagal menyerialisasi konfigurasi AI: {e}"))?;
    std::fs::write(dir.join(AI_CONFIG_FILE), cfg_json)
        .map_err(|e| format!("Gagal menyimpan konfigurasi AI: {e}"))?;

    let keys_changed = input.recraft_api_key.is_some()
        || input.vectorizer_api_id.is_some()
        || input.vectorizer_api_secret.is_some()
        || input.claude_api_key.is_some()
        || input.openai_api_key.is_some()
        || input.gemini_api_key.is_some();

    if keys_changed {
        let mut keys = load_ai_keys(dir);
        if let Some(v) = input.recraft_api_key {
            keys.recraft_api_key = v.trim().to_string();
        }
        if let Some(v) = input.vectorizer_api_id {
            keys.vectorizer_api_id = v.trim().to_string();
        }
        if let Some(v) = input.vectorizer_api_secret {
            keys.vectorizer_api_secret = v.trim().to_string();
        }
        if let Some(v) = input.claude_api_key {
            keys.claude_api_key = v.trim().to_string();
        }
        if let Some(v) = input.openai_api_key {
            keys.openai_api_key = v.trim().to_string();
        }
        if let Some(v) = input.gemini_api_key {
            keys.gemini_api_key = v.trim().to_string();
        }
        write_keys_file(dir, &keys)?;
    }

    Ok(get_ai_config_public(dir))
}

fn write_keys_file(dir: &Path, keys: &AiKeys) -> Result<(), String> {
    let keys_path = dir.join(AI_KEYS_FILE);
    let json = serde_json::to_string_pretty(keys)
        .map_err(|e| format!("Gagal menyerialisasi kunci AI: {e}"))?;
    std::fs::write(&keys_path, json)
        .map_err(|e| format!("Gagal menyimpan kunci AI: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&keys_path, std::fs::Permissions::from_mode(0o600));
    }
    Ok(())
}

/// Mask every stored API secret so it can never appear in error messages or logs.
pub fn mask_secrets(text: &str, keys: &AiKeys) -> String {
    let mut out = text.to_string();
    for secret in [
        &keys.recraft_api_key,
        &keys.vectorizer_api_id,
        &keys.vectorizer_api_secret,
        &keys.claude_api_key,
        &keys.openai_api_key,
        &keys.gemini_api_key,
    ] {
        let trimmed = secret.trim();
        if trimmed.len() >= 4 {
            out = out.replace(trimmed, "[REDACTED]");
        }
    }
    out
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    let trimmed = s.trim();
    if trimmed.chars().count() <= max_chars {
        trimmed.to_string()
    } else {
        let head: String = trimmed.chars().take(max_chars).collect();
        format!("{head}...")
    }
}

/// Extract the first `<svg ...>...</svg>` block from raw text or Markdown code fences.
pub fn extract_svg_block(raw: &str) -> Option<String> {
    let lower = raw.to_ascii_lowercase();
    let start = lower.find("<svg")?;
    let after_start = &lower[start..];
    // Ensure `<svg` is followed by whitespace, `>`, or `/` (not `<svgfoo`).
    let next_ch = after_start.as_bytes().get(4).copied()?;
    if !matches!(next_ch, b' ' | b'\t' | b'\r' | b'\n' | b'>' | b'/') {
        return None;
    }
    let end_rel = after_start.rfind("</svg>")?;
    let end = start + end_rel + "</svg>".len();
    let slice = raw[start..end].trim();
    if slice.is_empty() {
        None
    } else {
        Some(slice.to_string())
    }
}

/// Natural numeric descending comparator so newer versions sort first (`gpt-4.1` > `gpt-4o`, `claude-sonnet-4-5` > `claude-3-5`).
fn cmp_natural_desc(a: &str, b: &str) -> Ordering {
    let mut ia = a.as_bytes().iter().peekable();
    let mut ib = b.as_bytes().iter().peekable();
    loop {
        match (ia.peek(), ib.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Greater,
            (Some(_), None) => return Ordering::Less,
            (Some(&&ca), Some(&&cb)) => {
                if ca.is_ascii_digit() && cb.is_ascii_digit() {
                    let mut na: u64 = 0;
                    while let Some(&&d) = ia.peek() {
                        if !d.is_ascii_digit() {
                            break;
                        }
                        na = na.saturating_mul(10).saturating_add((d - b'0') as u64);
                        ia.next();
                    }
                    let mut nb: u64 = 0;
                    while let Some(&&d) = ib.peek() {
                        if !d.is_ascii_digit() {
                            break;
                        }
                        nb = nb.saturating_mul(10).saturating_add((d - b'0') as u64);
                        ib.next();
                    }
                    let ord = nb.cmp(&na); // descending
                    if ord != Ordering::Equal {
                        return ord;
                    }
                } else {
                    let ord = cb.to_ascii_lowercase().cmp(&ca.to_ascii_lowercase()); // descending
                    if ord != Ordering::Equal {
                        return ord;
                    }
                    ia.next();
                    ib.next();
                }
            }
        }
    }
}

pub fn parse_claude_models(body: &str) -> Result<Vec<AiModelItem>, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "Respons daftar model Claude tidak valid.".to_string())?;
    let arr = json
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Daftar model Claude kosong atau tidak valid.".to_string())?;
    let mut items: Vec<(String, AiModelItem)> = Vec::new();
    for entry in arr {
        let Some(id) = entry.get("id").and_then(|v| v.as_str()).map(str::trim) else {
            continue;
        };
        if id.is_empty() {
            continue;
        }
        let label = entry
            .get("display_name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(id)
            .to_string();
        let created = entry
            .get("created_at")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        items.push((
            created,
            AiModelItem {
                id: id.to_string(),
                label,
            },
        ));
    }
    items.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| cmp_natural_desc(&a.1.id, &b.1.id))
    });
    let mut seen = std::collections::HashSet::new();
    Ok(items
        .into_iter()
        .map(|(_, item)| item)
        .filter(|m| seen.insert(m.id.clone()))
        .collect())
}

pub fn parse_openai_models(body: &str) -> Result<Vec<AiModelItem>, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "Respons daftar model OpenAI tidak valid.".to_string())?;
    let arr = json
        .get("data")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Daftar model OpenAI kosong atau tidak valid.".to_string())?;
    let excluded = [
        "audio",
        "realtime",
        "transcribe",
        "tts",
        "search",
        "embedding",
        "whisper",
        "dall-e",
        "moderation",
        "instruct",
        "babbage",
        "davinci",
    ];
    let mut items: Vec<(i64, AiModelItem)> = Vec::new();
    for entry in arr {
        let Some(id) = entry.get("id").and_then(|v| v.as_str()).map(str::trim) else {
            continue;
        };
        let lower = id.to_ascii_lowercase();
        let allowed_prefix = lower.starts_with("gpt-")
            || lower.starts_with("o3")
            || lower.starts_with("o4")
            || lower.starts_with("chatgpt-");
        if !allowed_prefix || excluded.iter().any(|bad| lower.contains(bad)) {
            continue;
        }
        let created = entry.get("created").and_then(|v| v.as_i64()).unwrap_or(0);
        items.push((
            created,
            AiModelItem {
                id: id.to_string(),
                label: id.to_string(),
            },
        ));
    }
    items.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| cmp_natural_desc(&a.1.id, &b.1.id))
    });
    let mut seen = std::collections::HashSet::new();
    Ok(items
        .into_iter()
        .map(|(_, item)| item)
        .filter(|m| seen.insert(m.id.clone()))
        .collect())
}

pub fn parse_gemini_models(body: &str) -> Result<Vec<AiModelItem>, String> {
    let json: serde_json::Value =
        serde_json::from_str(body).map_err(|_| "Respons daftar model Gemini tidak valid.".to_string())?;
    let arr = json
        .get("models")
        .and_then(|v| v.as_array())
        .ok_or_else(|| "Daftar model Gemini kosong atau tidak valid.".to_string())?;
    let excluded = ["embedding", "aqa", "tts", "audio", "imagen", "veo"];
    let mut items: Vec<AiModelItem> = Vec::new();
    for entry in arr {
        let Some(name) = entry.get("name").and_then(|v| v.as_str()).map(str::trim) else {
            continue;
        };
        let id = name.strip_prefix("models/").unwrap_or(name);
        let lower = id.to_ascii_lowercase();
        if !lower.starts_with("gemini-") || excluded.iter().any(|bad| lower.contains(bad)) {
            continue;
        }
        let supports_generate = entry
            .get("supportedGenerationMethods")
            .and_then(|v| v.as_array())
            .map(|methods| {
                methods
                    .iter()
                    .filter_map(|m| m.as_str())
                    .any(|m| m == "generateContent")
            })
            .unwrap_or(false);
        if !supports_generate {
            continue;
        }
        let label = entry
            .get("displayName")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(id)
            .to_string();
        items.push(AiModelItem {
            id: id.to_string(),
            label,
        });
    }
    items.sort_by(|a, b| cmp_natural_desc(&a.id, &b.id));
    let mut seen = std::collections::HashSet::new();
    Ok(items
        .into_iter()
        .filter(|m| seen.insert(m.id.clone()))
        .collect())
}

fn new_curl_command() -> Command {
    let mut cmd = Command::new("curl");
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    cmd
}

/// Run `curl` and return `(http_status_code, body_bytes)`.
fn run_curl(args: &[String], keys: &AiKeys) -> Result<(u16, Vec<u8>), String> {
    let mut cmd = new_curl_command();
    cmd.args(["--silent", "--show-error", "--location", "--write-out", "\n%{http_code}"]);
    cmd.args(args);
    let out = cmd.output().map_err(|e| {
        mask_secrets(
            &format!("Gagal menjalankan curl sistem: {e}"),
            keys,
        )
    })?;

    if !out.status.success() {
        let stderr = mask_secrets(&String::from_utf8_lossy(&out.stderr), keys);
        let snippet = truncate_chars(&stderr, STDERR_SNIPPET_MAX);
        if out.status.code() == Some(28) {
            return Err("Permintaan AI melebihi batas waktu (timeout).".to_string());
        }
        return Err(if snippet.is_empty() {
            "Tidak dapat terhubung ke server AI (periksa koneksi internet).".to_string()
        } else {
            format!("Gagal terhubung ke server AI: {snippet}")
        });
    }

    let raw = out.stdout;
    let split_pos = raw.iter().rposition(|&b| b == b'\n');
    let (body, code_bytes) = match split_pos {
        Some(pos) => (&raw[..pos], &raw[pos + 1..]),
        None => (&raw[..], &[][..]),
    };
    let status_code = std::str::from_utf8(code_bytes)
        .ok()
        .and_then(|s| s.trim().parse::<u16>().ok())
        .unwrap_or(0);
    Ok((status_code, body.to_vec()))
}

fn friendly_http_error(provider: &str, status: u16, body: &[u8], keys: &AiKeys) -> String {
    let raw_body = mask_secrets(&String::from_utf8_lossy(body), keys);
    let snippet = truncate_chars(&raw_body, 220);
    match status {
        401 | 403 => format!("Kunci API {provider} ditolak (HTTP {status}). Periksa kembali API key Anda."),
        402 => format!("Kuota atau saldo API {provider} tidak mencukupi (HTTP 402)."),
        429 => format!("Batas permintaan API {provider} tercapai (HTTP 429). Coba lagi beberapa saat."),
        500..=599 => format!("Server {provider} sedang bermasalah (HTTP {status})."),
        0 => format!("Tidak dapat terhubung ke server {provider} (offline atau timeout)."),
        _ => {
            if snippet.is_empty() {
                format!("Permintaan ke {provider} gagal (HTTP {status}).")
            } else {
                format!("Permintaan ke {provider} gagal (HTTP {status}): {snippet}")
            }
        }
    }
}

/// Auto-fetch real active models for `claude`, `openai`, or `gemini`.
/// Never returns hardcoded fallback lists: returns `models: []` with a clear status when
/// API key is missing, offline, or the request fails.
pub fn fetch_ai_models(dir: &Path, provider: &str) -> FetchAiModelsResult {
    let norm = normalize_provider(provider);
    let keys = load_ai_keys(dir);

    match norm.as_str() {
        "claude" => {
            if keys.claude_api_key.trim().is_empty() {
                return FetchAiModelsResult {
                    provider: norm,
                    status: "api_key_required".into(),
                    message: "Masukkan API key Claude untuk memuat daftar model.".into(),
                    models: Vec::new(),
                };
            }
            let args = vec![
                "--connect-timeout".into(),
                "8".into(),
                "--max-time".into(),
                MODELS_TIMEOUT_SECS.into(),
                "-H".into(),
                format!("x-api-key: {}", keys.claude_api_key.trim()),
                "-H".into(),
                "anthropic-version: 2023-06-01".into(),
                "https://api.anthropic.com/v1/models".into(),
            ];
            match run_curl(&args, &keys) {
                Ok((200..=299, body)) => match parse_claude_models(&String::from_utf8_lossy(&body)) {
                    Ok(models) => FetchAiModelsResult {
                        provider: norm,
                        status: "connected".into(),
                        message: format!("Terhubung · {} model tersedia", models.len()),
                        models,
                    },
                    Err(msg) => FetchAiModelsResult {
                        provider: norm,
                        status: "error".into(),
                        message: msg,
                        models: Vec::new(),
                    },
                },
                Ok((code, body)) => FetchAiModelsResult {
                    provider: norm,
                    status: "error".into(),
                    message: friendly_http_error("Claude", code, &body, &keys),
                    models: Vec::new(),
                },
                Err(msg) => FetchAiModelsResult {
                    provider: norm,
                    status: "offline".into(),
                    message: msg,
                    models: Vec::new(),
                },
            }
        }
        "openai" => {
            if keys.openai_api_key.trim().is_empty() {
                return FetchAiModelsResult {
                    provider: norm,
                    status: "api_key_required".into(),
                    message: "Masukkan API key OpenAI untuk memuat daftar model.".into(),
                    models: Vec::new(),
                };
            }
            let args = vec![
                "--connect-timeout".into(),
                "8".into(),
                "--max-time".into(),
                MODELS_TIMEOUT_SECS.into(),
                "-H".into(),
                format!("Authorization: Bearer {}", keys.openai_api_key.trim()),
                "https://api.openai.com/v1/models".into(),
            ];
            match run_curl(&args, &keys) {
                Ok((200..=299, body)) => match parse_openai_models(&String::from_utf8_lossy(&body)) {
                    Ok(models) => FetchAiModelsResult {
                        provider: norm,
                        status: "connected".into(),
                        message: format!("Terhubung · {} model tersedia", models.len()),
                        models,
                    },
                    Err(msg) => FetchAiModelsResult {
                        provider: norm,
                        status: "error".into(),
                        message: msg,
                        models: Vec::new(),
                    },
                },
                Ok((code, body)) => FetchAiModelsResult {
                    provider: norm,
                    status: "error".into(),
                    message: friendly_http_error("OpenAI", code, &body, &keys),
                    models: Vec::new(),
                },
                Err(msg) => FetchAiModelsResult {
                    provider: norm,
                    status: "offline".into(),
                    message: msg,
                    models: Vec::new(),
                },
            }
        }
        "gemini" => {
            if keys.gemini_api_key.trim().is_empty() {
                return FetchAiModelsResult {
                    provider: norm,
                    status: "api_key_required".into(),
                    message: "Masukkan API key Gemini untuk memuat daftar model.".into(),
                    models: Vec::new(),
                };
            }
            let args = vec![
                "--connect-timeout".into(),
                "8".into(),
                "--max-time".into(),
                MODELS_TIMEOUT_SECS.into(),
                "-H".into(),
                format!("x-goog-api-key: {}", keys.gemini_api_key.trim()),
                "https://generativelanguage.googleapis.com/v1beta/models".into(),
            ];
            match run_curl(&args, &keys) {
                Ok((200..=299, body)) => match parse_gemini_models(&String::from_utf8_lossy(&body)) {
                    Ok(models) => FetchAiModelsResult {
                        provider: norm,
                        status: "connected".into(),
                        message: format!("Terhubung · {} model tersedia", models.len()),
                        models,
                    },
                    Err(msg) => FetchAiModelsResult {
                        provider: norm,
                        status: "error".into(),
                        message: msg,
                        models: Vec::new(),
                    },
                },
                Ok((code, body)) => FetchAiModelsResult {
                    provider: norm,
                    status: "error".into(),
                    message: friendly_http_error("Gemini", code, &body, &keys),
                    models: Vec::new(),
                },
                Err(msg) => FetchAiModelsResult {
                    provider: norm,
                    status: "offline".into(),
                    message: msg,
                    models: Vec::new(),
                },
            }
        }
        _ => FetchAiModelsResult {
            provider: norm,
            status: "not_applicable".into(),
            message: "Provider ini tidak memakai pilihan model.".into(),
            models: Vec::new(),
        },
    }
}

/// Locate an executable by explicit path or on the system `PATH`.
pub fn resolve_executable(exe: &str) -> Option<PathBuf> {
    let trimmed = exe.trim();
    if trimmed.is_empty() {
        return None;
    }
    let direct = PathBuf::from(trimmed);
    if direct.components().count() > 1 || direct.is_absolute() {
        return direct.is_file().then_some(direct);
    }

    let path_var = std::env::var_os("PATH")?;
    #[cfg(target_os = "windows")]
    let exts: Vec<String> = {
        let has_ext = direct.extension().is_some();
        if has_ext {
            vec![String::new()]
        } else {
            let pathext = std::env::var("PATHEXT")
                .unwrap_or_else(|_| ".EXE;.CMD;.BAT;.COM".to_string());
            let mut list: Vec<String> = pathext
                .split(';')
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_ascii_lowercase())
                .collect();
            list.insert(0, String::new());
            list
        }
    };
    #[cfg(not(target_os = "windows"))]
    let exts: Vec<String> = vec![String::new()];

    for dir in std::env::split_paths(&path_var) {
        for ext in &exts {
            let candidate = dir.join(format!("{trimmed}{ext}"));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

/// Test configuration without spending paid vectorization credits.
pub fn test_ai_config(dir: &Path, provider_override: Option<&str>) -> AiTestResult {
    let cfg = load_ai_config(dir);
    let keys = load_ai_keys(dir);
    let provider = normalize_provider(provider_override.unwrap_or(&cfg.ai_provider));

    match provider.as_str() {
        "local" => AiTestResult {
            ok: true,
            provider,
            message: "Mesin lokal Trasea siap dipakai (100% offline).".into(),
            resolved_path: None,
        },
        "recraft" => {
            if keys.recraft_api_key.trim().is_empty() {
                AiTestResult {
                    ok: false,
                    provider,
                    message: "API key Recraft belum diisi.".into(),
                    resolved_path: None,
                }
            } else {
                AiTestResult {
                    ok: true,
                    provider,
                    message: "API key Recraft tersimpan dan siap dipakai.".into(),
                    resolved_path: None,
                }
            }
        }
        "vectorizer_ai" => {
            if keys.vectorizer_api_id.trim().is_empty()
                || keys.vectorizer_api_secret.trim().is_empty()
            {
                AiTestResult {
                    ok: false,
                    provider,
                    message: "API ID dan API Secret Vectorizer.AI wajib diisi keduanya.".into(),
                    resolved_path: None,
                }
            } else {
                AiTestResult {
                    ok: true,
                    provider,
                    message: "Kredensial Vectorizer.AI tersimpan dan siap dipakai.".into(),
                    resolved_path: None,
                }
            }
        }
        "claude" | "openai" | "gemini" => {
            let res = fetch_ai_models(dir, &provider);
            AiTestResult {
                ok: res.status == "connected",
                provider,
                message: res.message,
                resolved_path: None,
            }
        }
        "cli" => {
            if cfg.ai_cli_executable.trim().is_empty() {
                return AiTestResult {
                    ok: false,
                    provider,
                    message: "Perintah executable CLI belum diisi.".into(),
                    resolved_path: None,
                };
            }
            match resolve_executable(&cfg.ai_cli_executable) {
                Some(p) => AiTestResult {
                    ok: true,
                    provider,
                    message: format!("Executable CLI ditemukan: {}", p.display()),
                    resolved_path: Some(p.to_string_lossy().to_string()),
                },
                None => AiTestResult {
                    ok: false,
                    provider,
                    message: format!(
                        "Executable '{}' tidak ditemukan di PATH.",
                        cfg.ai_cli_executable
                    ),
                    resolved_path: None,
                },
            }
        }
        _ => AiTestResult {
            ok: false,
            provider,
            message: "Provider AI tidak dikenal.".into(),
            resolved_path: None,
        },
    }
}

/// Safe local CLI execution using `Vec<String>` arguments (no shell string interpolation),
/// hard timeout with child kill, and unconditional temporary directory cleanup via `TempDirGuard`.
pub fn run_cli_hook(
    executable: &str,
    args_template: &[String],
    input_png: &[u8],
    timeout_sec: u64,
    keys: &AiKeys,
) -> Result<String, String> {
    run_cli_hook_with_prefix(executable, args_template, input_png, timeout_sec, keys, "trasea-ai-cli")
}

/// Defensively normalize CLI arguments (e.g. inject `--skip-git-repo-check` for `codex exec`).
pub fn normalize_cli_args(exe: &str, mut args: Vec<String>) -> Vec<String> {
    let file_name = Path::new(exe.trim())
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(exe.trim())
        .to_ascii_lowercase();
    let base = file_name
        .strip_suffix(".exe")
        .or_else(|| file_name.strip_suffix(".cmd"))
        .or_else(|| file_name.strip_suffix(".bat"))
        .unwrap_or(&file_name);
    if base == "codex"
        && args.first().map(|s| s.as_str()) == Some("exec")
        && !args.iter().any(|a| a == "--skip-git-repo-check")
    {
        args.insert(1, "--skip-git-repo-check".to_string());
    }
    args
}

fn run_cli_hook_with_prefix(
    executable: &str,
    args_template: &[String],
    input_png: &[u8],
    timeout_sec: u64,
    keys: &AiKeys,
    temp_prefix: &str,
) -> Result<String, String> {
    let exe = executable.trim();
    if exe.is_empty() {
        return Err("Perintah executable CLI belum diisi.".to_string());
    }
    let tmp = TempDirGuard::new(temp_prefix)?;
    let input_path = tmp.path().join("input.png");
    let output_path = tmp.path().join("output.svg");
    std::fs::write(&input_path, input_png)
        .map_err(|e| format!("Gagal menulis file input sementara: {e}"))?;

    let input_str = input_path.to_string_lossy().to_string();
    let output_str = output_path.to_string_lossy().to_string();
    let uses_output_placeholder = args_template.iter().any(|a| a.contains("{output}"));
    let resolved_args = normalize_cli_args(
        exe,
        args_template
            .iter()
            .map(|tok| tok.replace("{input}", &input_str).replace("{output}", &output_str))
            .collect(),
    );

    let mut cmd = Command::new(exe);
    cmd.current_dir(tmp.path())
        .args(&resolved_args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(target_os = "windows")]
    cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("Gagal menjalankan CLI '{exe}': {e}"))?;

    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let stdout_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stdout_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });
    let stderr_handle = std::thread::spawn(move || {
        let mut buf = Vec::new();
        if let Some(mut s) = stderr_pipe {
            let _ = s.read_to_end(&mut buf);
        }
        buf
    });

    let deadline = Instant::now() + Duration::from_secs(timeout_sec.clamp(1, 300));
    let status = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st,
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    let _ = stdout_handle.join();
                    let _ = stderr_handle.join();
                    return Err(format!(
                        "Perintah CLI '{exe}' melebihi batas waktu ({timeout_sec} detik) dan telah dihentikan."
                    ));
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("Gagal memantau proses CLI '{exe}': {e}"));
            }
        }
    };

    let stdout_bytes = stdout_handle.join().unwrap_or_default();
    let stderr_bytes = stderr_handle.join().unwrap_or_default();
    let stdout_text = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_clean = truncate_chars(
        &mask_secrets(&String::from_utf8_lossy(&stderr_bytes), keys),
        STDERR_SNIPPET_MAX,
    );

    if !status.success() {
        let code_str = status
            .code()
            .map(|c| c.to_string())
            .unwrap_or_else(|| "signal".to_string());
        return Err(if stderr_clean.is_empty() {
            format!("Perintah CLI '{exe}' gagal dengan kode keluar {code_str}.")
        } else {
            format!("Perintah CLI '{exe}' gagal (kode {code_str}): {stderr_clean}")
        });
    }

    let candidate = if uses_output_placeholder && output_path.is_file() {
        std::fs::read_to_string(&output_path)
            .map_err(|e| format!("Gagal membaca file SVG output CLI: {e}"))?
    } else {
        stdout_text
    };

    extract_svg_block(&candidate).ok_or_else(|| {
        if stderr_clean.is_empty() {
            format!("Output dari CLI '{exe}' tidak mengandung blok <svg>...</svg> yang valid.")
        } else {
            format!(
                "Output dari CLI '{exe}' bukan SVG valid. Detail stderr: {stderr_clean}"
            )
        }
    })
}

fn call_recraft_api(input_png: &[u8], keys: &AiKeys) -> Result<String, String> {
    let key = keys.recraft_api_key.trim();
    if key.is_empty() {
        return Err("API key Recraft belum diisi di Pengaturan → Mesin AI.".to_string());
    }
    let tmp = TempDirGuard::new("trasea-ai-recraft")?;
    let input_path = tmp.path().join("input.png");
    std::fs::write(&input_path, input_png)
        .map_err(|e| format!("Gagal menulis file sementara: {e}"))?;

    let args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        API_TIMEOUT_SECS.into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        format!("Authorization: Bearer {key}"),
        "-F".into(),
        format!("file=@{}", input_path.display()),
        "https://external.api.recraft.ai/v1/images/vectorize".into(),
    ];
    let (status, body) = run_curl(&args, keys)?;
    if !(200..=299).contains(&status) {
        return Err(friendly_http_error("Recraft", status, &body, keys));
    }

    let body_text = String::from_utf8_lossy(&body);
    if let Some(svg) = extract_svg_block(&body_text) {
        return Ok(svg);
    }

    let json: serde_json::Value = serde_json::from_str(&body_text)
        .map_err(|_| "Respons Recraft bukan SVG atau JSON valid.".to_string())?;
    let svg_url = json
        .pointer("/image/url")
        .or_else(|| json.pointer("/data/0/url"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Respons Recraft tidak memuat URL SVG.".to_string())?;
    if !svg_url.starts_with("https://") {
        return Err("URL hasil Recraft tidak memakai HTTPS.".to_string());
    }

    let dl_args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        "45".into(),
        svg_url.to_string(),
    ];
    let (dl_status, dl_body) = run_curl(&dl_args, keys)?;
    if !(200..=299).contains(&dl_status) {
        return Err(friendly_http_error("Recraft", dl_status, &dl_body, keys));
    }
    extract_svg_block(&String::from_utf8_lossy(&dl_body))
        .ok_or_else(|| "File unduhan dari Recraft tidak mengandung SVG valid.".to_string())
}

fn call_vectorizer_ai_api(input_png: &[u8], keys: &AiKeys) -> Result<String, String> {
    let api_id = keys.vectorizer_api_id.trim();
    let api_secret = keys.vectorizer_api_secret.trim();
    if api_id.is_empty() || api_secret.is_empty() {
        return Err("API ID dan API Secret Vectorizer.AI belum diisi di Pengaturan → Mesin AI.".to_string());
    }
    let tmp = TempDirGuard::new("trasea-ai-vec")?;
    let input_path = tmp.path().join("input.png");
    std::fs::write(&input_path, input_png)
        .map_err(|e| format!("Gagal menulis file sementara: {e}"))?;

    let args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        API_TIMEOUT_SECS.into(),
        "-X".into(),
        "POST".into(),
        "-u".into(),
        format!("{api_id}:{api_secret}"),
        "-F".into(),
        format!("image=@{}", input_path.display()),
        "-F".into(),
        "output.file_format=svg".into(),
        "https://vectorizer.ai/api/v1/vectorize".into(),
    ];
    let (status, body) = run_curl(&args, keys)?;
    if !(200..=299).contains(&status) {
        return Err(friendly_http_error("Vectorizer.AI", status, &body, keys));
    }
    extract_svg_block(&String::from_utf8_lossy(&body))
        .ok_or_else(|| "Respons Vectorizer.AI tidak mengandung SVG valid.".to_string())
}

fn call_claude_api(input_png: &[u8], model: &str, keys: &AiKeys) -> Result<String, String> {
    let key = keys.claude_api_key.trim();
    if key.is_empty() {
        return Err("API key Claude belum diisi di Pengaturan → Mesin AI.".to_string());
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("Pilih model Claude terlebih dahulu di Pengaturan → Mesin AI.".to_string());
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(input_png);
    let payload = serde_json::json!({
        "model": model,
        "max_tokens": 8192,
        "messages": [{
            "role": "user",
            "content": [
                {
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": "image/png",
                        "data": b64
                    }
                },
                { "type": "text", "text": VISION_SVG_PROMPT }
            ]
        }]
    });
    let tmp = TempDirGuard::new("trasea-ai-claude")?;
    let body_path = tmp.path().join("req.json");
    std::fs::write(&body_path, payload.to_string())
        .map_err(|e| format!("Gagal menulis payload sementara: {e}"))?;

    let args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        API_TIMEOUT_SECS.into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        format!("x-api-key: {key}"),
        "-H".into(),
        "anthropic-version: 2023-06-01".into(),
        "-H".into(),
        "content-type: application/json".into(),
        "--data-binary".into(),
        format!("@{}", body_path.display()),
        "https://api.anthropic.com/v1/messages".into(),
    ];
    let (status, body) = run_curl(&args, keys)?;
    if !(200..=299).contains(&status) {
        return Err(friendly_http_error("Claude", status, &body, keys));
    }
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Respons JSON Claude tidak valid.".to_string())?;
    let text = json
        .get("content")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|item| item.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    extract_svg_block(&text)
        .ok_or_else(|| "Respons Claude tidak mengandung blok <svg>...</svg> yang valid.".to_string())
}

fn call_openai_api(input_png: &[u8], model: &str, keys: &AiKeys) -> Result<String, String> {
    let key = keys.openai_api_key.trim();
    if key.is_empty() {
        return Err("API key OpenAI belum diisi di Pengaturan → Mesin AI.".to_string());
    }
    let model = model.trim();
    if model.is_empty() {
        return Err("Pilih model OpenAI terlebih dahulu di Pengaturan → Mesin AI.".to_string());
    }
    let data_url = format!(
        "data:image/png;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(input_png)
    );
    let payload = serde_json::json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": [
                { "type": "text", "text": VISION_SVG_PROMPT },
                { "type": "image_url", "image_url": { "url": data_url } }
            ]
        }]
    });
    let tmp = TempDirGuard::new("trasea-ai-openai")?;
    let body_path = tmp.path().join("req.json");
    std::fs::write(&body_path, payload.to_string())
        .map_err(|e| format!("Gagal menulis payload sementara: {e}"))?;

    let args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        API_TIMEOUT_SECS.into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        format!("Authorization: Bearer {key}"),
        "-H".into(),
        "Content-Type: application/json".into(),
        "--data-binary".into(),
        format!("@{}", body_path.display()),
        "https://api.openai.com/v1/chat/completions".into(),
    ];
    let (status, body) = run_curl(&args, keys)?;
    if !(200..=299).contains(&status) {
        return Err(friendly_http_error("OpenAI", status, &body, keys));
    }
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Respons JSON OpenAI tidak valid.".to_string())?;
    let text = json
        .pointer("/choices/0/message/content")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    extract_svg_block(text)
        .ok_or_else(|| "Respons OpenAI tidak mengandung blok <svg>...</svg> yang valid.".to_string())
}

fn call_gemini_api(input_png: &[u8], model: &str, keys: &AiKeys) -> Result<String, String> {
    let key = keys.gemini_api_key.trim();
    if key.is_empty() {
        return Err("API key Gemini belum diisi di Pengaturan → Mesin AI.".to_string());
    }
    let model = model.trim().trim_start_matches("models/");
    if model.is_empty() {
        return Err("Pilih model Gemini terlebih dahulu di Pengaturan → Mesin AI.".to_string());
    }
    let b64 = base64::engine::general_purpose::STANDARD.encode(input_png);
    let payload = serde_json::json!({
        "contents": [{
            "parts": [
                { "text": VISION_SVG_PROMPT },
                {
                    "inline_data": {
                        "mime_type": "image/png",
                        "data": b64
                    }
                }
            ]
        }]
    });
    let tmp = TempDirGuard::new("trasea-ai-gemini")?;
    let body_path = tmp.path().join("req.json");
    std::fs::write(&body_path, payload.to_string())
        .map_err(|e| format!("Gagal menulis payload sementara: {e}"))?;

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
    );
    let args = vec![
        "--connect-timeout".into(),
        "10".into(),
        "--max-time".into(),
        API_TIMEOUT_SECS.into(),
        "-X".into(),
        "POST".into(),
        "-H".into(),
        format!("x-goog-api-key: {key}"),
        "-H".into(),
        "Content-Type: application/json".into(),
        "--data-binary".into(),
        format!("@{}", body_path.display()),
        url,
    ];
    let (status, body) = run_curl(&args, keys)?;
    if !(200..=299).contains(&status) {
        return Err(friendly_http_error("Gemini", status, &body, keys));
    }
    let json: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Respons JSON Gemini tidak valid.".to_string())?;
    let text = json
        .pointer("/candidates/0/content/parts")
        .and_then(|v| v.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(|p| p.get("text").and_then(|t| t.as_str()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default();
    extract_svg_block(&text)
        .ok_or_else(|| "Respons Gemini tidak mengandung blok <svg>...</svg> yang valid.".to_string())
}

/// Dispatch PNG bytes to the requested external AI engine and return sanitized `SvgImport`.
pub fn run_external_ai_to_svg_import(
    config_dir: &Path,
    provider_override: Option<&str>,
    input_png: &[u8],
) -> Result<(String, trasea_core::svg_import::SvgImport), String> {
    let cfg = load_ai_config(config_dir);
    let keys = load_ai_keys(config_dir);
    let provider = normalize_provider(provider_override.unwrap_or(&cfg.ai_provider));

    let raw_svg = match provider.as_str() {
        "recraft" => call_recraft_api(input_png, &keys)?,
        "vectorizer_ai" => call_vectorizer_ai_api(input_png, &keys)?,
        "claude" => call_claude_api(input_png, &cfg.ai_claude_model, &keys)?,
        "openai" => call_openai_api(input_png, &cfg.ai_openai_model, &keys)?,
        "gemini" => call_gemini_api(input_png, &cfg.ai_gemini_model, &keys)?,
        "cli" => run_cli_hook(
            &cfg.ai_cli_executable,
            &cfg.ai_cli_args,
            input_png,
            cfg.ai_cli_timeout_sec,
            &keys,
        )?,
        _ => {
            return Err(
                "Mesin AI eksternal belum dipilih (provider saat ini masih 'local').".to_string(),
            )
        }
    };

    let import = trasea_core::svg_import::parse_svg_doc(raw_svg.as_bytes()).map_err(|e| {
        mask_secrets(
            &format!("SVG dari mesin AI ({provider}) gagal diimpor: {e}"),
            &keys,
        )
    })?;
    Ok((provider, import))
}

#[cfg(test)]
mod tests {
    use super::*;
    use trasea_core::color::ColorSpace;
    use trasea_core::export::{to_dxf, to_pdf, to_svg, ExportOptions};
    use trasea_core::{Params, ProductionOptions};

    const SAMPLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="120" height="120" viewBox="0 0 120 120"><rect x="10" y="10" width="100" height="100" rx="16" fill="#2563EB"/><circle cx="60" cy="60" r="28" fill="#FACC15"/></svg>"##;

    fn tiny_png_bytes() -> Vec<u8> {
        let img = image::RgbImage::from_pixel(16, 16, image::Rgb([37, 99, 235]));
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .expect("encode tiny png");
        buf
    }

    fn count_trasea_ai_temp_dirs(prefix: &str) -> usize {
        std::fs::read_dir(std::env::temp_dir())
            .map(|rd| {
                rd.filter_map(|e| e.ok())
                    .filter(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with(&format!("{prefix}-{}", std::process::id()))
                    })
                    .count()
            })
            .unwrap_or(0)
    }

    /// Cross-platform mock CLI helper (PowerShell on Windows, `sh` on Unix/macOS).
    #[cfg(target_os = "windows")]
    fn mock_cli_write_output(svg: &str) -> (String, Vec<String>) {
        let escaped = svg.replace('\'', "''");
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Set-Content -Path '{{output}}' -Value '{escaped}' -Encoding UTF8"),
            ],
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn mock_cli_write_output(svg: &str) -> (String, Vec<String>) {
        let escaped = svg.replace('\'', "'\\''");
        (
            "sh".to_string(),
            vec![
                "-c".into(),
                format!("printf '%s' '{escaped}' > \"$1\""),
                "sh".into(),
                "{output}".into(),
            ],
        )
    }

    #[cfg(target_os = "windows")]
    fn mock_cli_markdown_stdout(svg: &str) -> (String, Vec<String>) {
        let escaped = svg.replace('\'', "''");
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Write-Output 'Here is your clean vector:'; Write-Output '```svg'; Write-Output '{escaped}'; Write-Output '```'"),
            ],
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn mock_cli_markdown_stdout(svg: &str) -> (String, Vec<String>) {
        let escaped = svg.replace('\'', "'\\''");
        (
            "sh".to_string(),
            vec![
                "-c".into(),
                format!("printf 'Here is your clean vector:\\n```svg\\n%s\\n```\\n' '{escaped}'"),
            ],
        )
    }

    #[cfg(target_os = "windows")]
    fn mock_cli_fail_with_long_stderr() -> (String, Vec<String>) {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                "[Console]::Error.WriteLine(('ERR_' + ('X' * 500))); exit 2".into(),
            ],
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn mock_cli_fail_with_long_stderr() -> (String, Vec<String>) {
        (
            "sh".to_string(),
            vec![
                "-c".into(),
                "printf 'ERR_%0500d\\n' 0 >&2; exit 2".into(),
            ],
        )
    }

    #[cfg(target_os = "windows")]
    fn mock_cli_sleep_seconds(secs: u64) -> (String, Vec<String>) {
        (
            "powershell".to_string(),
            vec![
                "-NoProfile".into(),
                "-Command".into(),
                format!("Start-Sleep -Seconds {secs}"),
            ],
        )
    }

    #[cfg(not(target_os = "windows"))]
    fn mock_cli_sleep_seconds(secs: u64) -> (String, Vec<String>) {
        ("sh".to_string(), vec!["-c".into(), format!("sleep {secs}")])
    }

    /// T16-01: CLI hook writes valid SVG to `{output}` and extracts SVG from Markdown `stdout`,
    /// imports cleanly via `trasea_core::svg_import`, and cleans up temporary folders.
    #[test]
    fn t16_01_cli_hook_output_file_and_markdown_stdout_and_temp_cleanup() {
        let png = tiny_png_bytes();
        let keys = AiKeys::default();
        let before = count_trasea_ai_temp_dirs("trasea-ai-t1601");

        // Variant A: `{output}` file
        let (exe_a, args_a) = mock_cli_write_output(SAMPLE_SVG);
        let svg_a = run_cli_hook_with_prefix(&exe_a, &args_a, &png, 15, &keys, "trasea-ai-t1601").expect("CLI {output} should succeed");
        let imp_a = trasea_core::svg_import::parse_svg_doc(svg_a.as_bytes()).expect("svg_import A");
        assert_eq!(imp_a.doc.width, 120);
        assert_eq!(imp_a.doc.height, 120);
        assert!(imp_a.doc.fields.len() >= 2, "expected >= 2 vector layers, got {}", imp_a.doc.fields.len());

        // Variant B: Markdown-wrapped SVG on `stdout`
        let (exe_b, args_b) = mock_cli_markdown_stdout(SAMPLE_SVG);
        let svg_b = run_cli_hook_with_prefix(&exe_b, &args_b, &png, 15, &keys, "trasea-ai-t1601").expect("CLI stdout Markdown should succeed");
        let imp_b = trasea_core::svg_import::parse_svg_doc(svg_b.as_bytes()).expect("svg_import B");
        assert!(imp_b.doc.fields.len() >= 2);

        let after = count_trasea_ai_temp_dirs("trasea-ai-t1601");
        assert_eq!(after, before, "temporary CLI dirs must be cleaned up");
    }

    /// T16-02: Non-SVG / failed CLI returns friendly error with truncated stderr (<= 300 chars);
    /// `<script>` tags inside SVG are stripped cleanly by `svg_import`/`usvg`.
    #[test]
    fn t16_02_cli_non_svg_error_truncation_and_script_sanitization() {
        let png = tiny_png_bytes();
        let keys = AiKeys::default();

        let (exe_fail, args_fail) = mock_cli_fail_with_long_stderr();
        let err = run_cli_hook(&exe_fail, &args_fail, &png, 15, &keys)
            .expect_err("failing CLI must return Err");
        assert!(err.contains("ERR_"), "error should include stderr snippet: {err}");
        assert!(
            err.chars().count() < 420,
            "stderr snippet must be truncated to <= 300 chars, got total len {}",
            err.chars().count()
        );

        let xss_svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="80" viewBox="0 0 80 80"><script>alert(1)</script><rect x="8" y="8" width="64" height="64" fill="#EF4444"/></svg>"##;
        let (exe_xss, args_xss) = mock_cli_write_output(xss_svg);
        let raw = run_cli_hook(&exe_xss, &args_xss, &png, 15, &keys).expect("CLI output");
        let imp = trasea_core::svg_import::parse_svg_doc(raw.as_bytes()).expect("svg_import sanitizes");
        let rendered = to_svg(&imp.doc, &ExportOptions::default());
        assert!(!rendered.to_ascii_lowercase().contains("<script"), "script tag must be stripped");
        assert!(!rendered.contains("alert(1)"), "script body must be stripped");
    }

    /// T16-03: CLI exceeding timeout is killed without hanging and temp dir is cleaned up.
    #[test]
    fn t16_03_cli_timeout_kills_child_and_cleans_temp_dir() {
        let png = tiny_png_bytes();
        let keys = AiKeys::default();
        let before = count_trasea_ai_temp_dirs("trasea-ai-t1603");
        let (exe, args) = mock_cli_sleep_seconds(15);

        let t0 = Instant::now();
        let err = run_cli_hook_with_prefix(&exe, &args, &png, 2, &keys, "trasea-ai-t1603").expect_err("sleep CLI must time out");
        let elapsed = t0.elapsed();
        assert!(err.contains("batas waktu"), "expected timeout message, got: {err}");
        assert!(elapsed < Duration::from_secs(9), "timeout took too long: {elapsed:?}");

        let after = count_trasea_ai_temp_dirs("trasea-ai-t1603");
        assert_eq!(after, before, "temp dir must be cleaned up after timeout");
    }

    /// T16-04: Key storage security (`has*Key` only, no raw secret leak, secret masking in errors)
    /// + real model parser tests for Claude, OpenAI, and Gemini + empty key returns `models: []`.
    #[test]
    fn t16_04_keys_security_masking_and_model_parsers_claude_openai_gemini() {
        let tmp = TempDirGuard::new("trasea-ai-keys-test").expect("temp dir");
        let secret_claude = "sk-ant-test-secret-99887766";
        let secret_openai = "sk-proj-test-secret-55443322";
        let secret_gemini = "AIzaSyTestSecretKey11223344";

        let saved = save_ai_config(
            tmp.path(),
            SaveAiConfigInput {
                ai_provider: Some("claude".into()),
                claude_api_key: Some(secret_claude.into()),
                openai_api_key: Some(secret_openai.into()),
                gemini_api_key: Some(secret_gemini.into()),
                ..Default::default()
            },
        )
        .expect("save_ai_config");

        assert!(saved.has_claude_key);
        assert!(saved.has_openai_key);
        assert!(saved.has_gemini_key);
        assert!(!saved.has_recraft_key);
        assert!(!saved.has_vectorizer_keys);

        let public_json = serde_json::to_string(&get_ai_config_public(tmp.path())).unwrap();
        assert!(!public_json.contains(secret_claude), "public config leaked Claude key");
        assert!(!public_json.contains(secret_openai), "public config leaked OpenAI key");
        assert!(!public_json.contains(secret_gemini), "public config leaked Gemini key");

        // Error masking must redact keys
        let loaded_keys = load_ai_keys(tmp.path());
        let fake_err = friendly_http_error(
            "Claude",
            401,
            format!("invalid api key: {secret_claude} / {secret_openai}").as_bytes(),
            &loaded_keys,
        );
        assert!(!fake_err.contains(secret_claude));
        assert!(!fake_err.contains(secret_openai));

        // Empty API key returns models: [] with status api_key_required (zero hardcoded fallback)
        let empty_dir = TempDirGuard::new("trasea-ai-empty").expect("empty dir");
        let empty_res = fetch_ai_models(empty_dir.path(), "openai");
        assert_eq!(empty_res.status, "api_key_required");
        assert!(empty_res.models.is_empty());

        // Claude model parser
        let claude_json = r#"{
            "data": [
                {"id": "claude-3-5-sonnet-20241022", "display_name": "Claude 3.5 Sonnet", "created_at": "2024-10-22T00:00:00Z"},
                {"id": "claude-sonnet-4-5-20250929", "display_name": "Claude Sonnet 4.5", "created_at": "2025-09-29T00:00:00Z"}
            ]
        }"#;
        let claude_models = parse_claude_models(claude_json).expect("parse claude");
        assert_eq!(claude_models.len(), 2);
        assert_eq!(claude_models[0].id, "claude-sonnet-4-5-20250929");

        // OpenAI model parser (filters out audio/tts/embedding/realtime/search)
        let openai_json = r#"{
            "data": [
                {"id": "gpt-4o-mini", "created": 1721172741},
                {"id": "gpt-4.1", "created": 1744316542},
                {"id": "gpt-4o-audio-preview", "created": 1744316999},
                {"id": "gpt-4o-realtime-preview", "created": 1744316998},
                {"id": "text-embedding-3-large", "created": 1744316997},
                {"id": "tts-1-hd", "created": 1744316996}
            ]
        }"#;
        let openai_models = parse_openai_models(openai_json).expect("parse openai");
        let openai_ids: Vec<&str> = openai_models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(openai_ids, vec!["gpt-4.1", "gpt-4o-mini"]);

        // Gemini model parser (strips models/, filters generateContent & excludes embedding/tts)
        let gemini_json = r#"{
            "models": [
                {"name": "models/gemini-2.0-flash", "displayName": "Gemini 2.0 Flash", "supportedGenerationMethods": ["generateContent"]},
                {"name": "models/gemini-2.5-pro", "displayName": "Gemini 2.5 Pro", "supportedGenerationMethods": ["generateContent"]},
                {"name": "models/gemini-2.5-flash-tts", "displayName": "Gemini TTS", "supportedGenerationMethods": ["generateContent"]},
                {"name": "models/text-embedding-004", "displayName": "Embedding", "supportedGenerationMethods": ["embedContent"]}
            ]
        }"#;
        let gemini_models = parse_gemini_models(gemini_json).expect("parse gemini");
        let gemini_ids: Vec<&str> = gemini_models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(gemini_ids, vec!["gemini-2.5-pro", "gemini-2.0-flash"]);
    }

    /// T162-01: normalize_cli_args injects --skip-git-repo-check for codex exec without duplicating or touching other CLIs.
    #[test]
    fn t162_normalize_cli_args_codex_skip_git_repo_check() {
        assert_eq!(
            normalize_cli_args("codex", vec!["exec".into(), "prompt".into()]),
            vec!["exec", "--skip-git-repo-check", "prompt"]
        );
        assert_eq!(
            normalize_cli_args(
                r"C:\tools\codex.cmd",
                vec!["exec".into(), "--skip-git-repo-check".into(), "prompt".into()]
            ),
            vec!["exec", "--skip-git-repo-check", "prompt"]
        );
        assert_eq!(
            normalize_cli_args("claude", vec!["-p".into(), "prompt".into()]),
            vec!["-p", "prompt"]
        );
    }

    /// T16-05: Doc imported from AI CLI SVG exports cleanly to PDF CMYK with CutContour +
    /// StickerBacking/Bleed (`0 0 0 0 k`) and to DXF (`CUTCONTOUR` without `STICKERBACKING`).
    #[test]
    fn t16_05_ai_svg_exports_pdf_cmyk_cutcontour_sticker_backing_and_dxf() {
        let tmp = TempDirGuard::new("trasea-ai-export").expect("temp dir");
        let (exe, args) = mock_cli_write_output(SAMPLE_SVG);
        save_ai_config(
            tmp.path(),
            SaveAiConfigInput {
                ai_provider: Some("cli".into()),
                ai_cli_executable: Some(exe),
                ai_cli_args: Some(args),
                ai_cli_timeout_sec: Some(20),
                ..Default::default()
            },
        )
        .expect("save cli config");

        let png = tiny_png_bytes();
        let (provider, import) =
            run_external_ai_to_svg_import(tmp.path(), Some("cli"), &png).expect("run AI CLI");
        assert_eq!(provider, "cli");

        let mut params = Params::default();
        params.production = ProductionOptions::default();
        let mut eo = ExportOptions::default();
        eo.color_space = ColorSpace::Cmyk;
        eo.die_cut_contour = true;
        eo.die_cut_offset_mm = 2.0;
        eo.die_cut_spot_name = "CutContour".to_string();
        eo.sticker_backing = true;
        eo.sticker_backing_color = "#FFFFFF".to_string();
        eo.sticker_bleed_mm = 1.5;

        let out = trasea_core::vectorize_svg(import, &params, &eo);
        let pdf = to_pdf(&out.doc, &eo).expect("to_pdf");
        let pdf_str = String::from_utf8_lossy(&pdf);
        assert!(pdf_str.contains("/Separation /CutContour"), "PDF must contain CutContour separation");
        assert!(pdf_str.contains("0 0 0 0 k"), "CMYK PDF must contain pure white StickerBacking (0 0 0 0 k)");

        let dxf = to_dxf(&out.doc, &eo);
        assert!(dxf.contains("CUTCONTOUR"), "DXF must contain CUTCONTOUR layer");
        assert!(!dxf.to_ascii_uppercase().contains("STICKERBACKING"), "DXF must exclude STICKERBACKING");
    }
}
