//! Local polling Watch Folder engine (`std::fs`, zero network crates, cross-platform deterministic).
//!
//! Polls `in_dir`, debounces newly added/modified image files until their size and mtime stabilize,
//! vectorizes each file using `batch::process_single_file`, updates `laporan-trasea.csv`/`.md`,
//! isolates corrupt files without crashing, and stops cleanly via `Arc<AtomicBool>`.

use crate::batch::{
    process_single_file, resolve_effective_export, scan_image_files, write_report_csv, write_report_md,
    BatchConfig, BatchItemReport, BatchStatus, BatchSummary,
};
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};

#[derive(Debug, Clone)]
pub struct WatchConfig {
    pub in_dir: PathBuf,
    pub out_dir: PathBuf,
    pub batch: BatchConfig,
    /// Polling interval in milliseconds (default 1000 ms).
    pub poll_ms: u64,
    /// Stability debounce wait in milliseconds (default 500 ms) before reading a new/changed file.
    pub debounce_ms: u64,
    /// Also watch immediate child folders (1 level deep).
    pub include_subfolders: bool,
}

impl WatchConfig {
    pub fn new(in_dir: PathBuf, out_dir: PathBuf, mut batch: BatchConfig) -> Self {
        batch.output_dir = out_dir.clone();
        Self {
            in_dir,
            out_dir,
            batch,
            poll_ms: 1000,
            debounce_ms: 500,
            include_subfolders: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchEvent {
    pub file: PathBuf,
    pub file_name: String,
    pub status: BatchStatus,
    pub colors: usize,
    pub paths: usize,
    pub nodes: usize,
    pub mae: Option<f64>,
    pub elapsed_ms: u64,
    pub error: Option<String>,
}

fn file_sig(path: &Path) -> Option<(u64, SystemTime)> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
    Some((meta.len(), modified))
}

/// Sleep up to `total_ms` in small slices, returning early if `stop` becomes true.
fn sleep_interruptible(total_ms: u64, stop: &AtomicBool) -> bool {
    let mut left = total_ms;
    while left > 0 {
        if stop.load(Ordering::Relaxed) {
            return true;
        }
        let step = left.min(50);
        std::thread::sleep(Duration::from_millis(step));
        left = left.saturating_sub(step);
    }
    stop.load(Ordering::Relaxed)
}

fn stem_for_watch(path: &Path, in_dir: &Path, assigned: &mut HashMap<PathBuf, String>, used: &mut HashMap<String, usize>) -> String {
    if let Some(existing) = assigned.get(path) {
        return existing.clone();
    }
    let raw = path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("gambar")
        .to_string();
    let _ = in_dir;
    let key = raw.to_ascii_lowercase();
    let count = used.entry(key).or_insert(0);
    *count += 1;
    let stem = if *count == 1 {
        raw
    } else {
        format!("{raw}_{}", *count)
    };
    assigned.insert(path.to_path_buf(), stem.clone());
    stem
}

/// Watch `config.in_dir` until `stop` is set to `true`.
/// Returns all `WatchEvent`s emitted during the session.
pub fn watch_folder<F>(config: &WatchConfig, stop: Arc<AtomicBool>, on_event: F) -> Result<Vec<WatchEvent>>
where
    F: Fn(&WatchEvent),
{
    if !config.in_dir.is_dir() {
        return Err(Error::Other(format!(
            "Folder pantauan tidak ditemukan: {}",
            config.in_dir.display()
        )));
    }
    std::fs::create_dir_all(&config.out_dir).map_err(|e| {
        Error::Other(format!(
            "Gagal membuat folder output {}: {e}",
            config.out_dir.display()
        ))
    })?;

    let mut cfg = config.batch.clone();
    cfg.output_dir = config.out_dir.clone();
    let eff_export = resolve_effective_export(&cfg);

    let poll_ms = config.poll_ms.max(50);
    let debounce_ms = config.debounce_ms;

    let mut processed: HashMap<PathBuf, (u64, SystemTime)> = HashMap::new();
    let mut assigned_stems: HashMap<PathBuf, String> = HashMap::new();
    let mut used_stems: HashMap<String, usize> = HashMap::new();
    let mut reports_order: Vec<PathBuf> = Vec::new();
    let mut reports_map: HashMap<PathBuf, BatchItemReport> = HashMap::new();
    let mut events: Vec<WatchEvent> = Vec::new();
    let session_start = Instant::now();

    while !stop.load(Ordering::Relaxed) {
        let files = match scan_image_files(&config.in_dir, config.include_subfolders) {
            Ok(f) => f,
            Err(_) => Vec::new(),
        };

        let mut updated_any = false;
        for path in files {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            let Some(sig1) = file_sig(&path) else { continue };
            if processed.get(&path) == Some(&sig1) {
                continue;
            }

            // Debounce: wait and re-check file size & mtime so partially-copied files are skipped until complete.
            if debounce_ms > 0 {
                if sleep_interruptible(debounce_ms, &stop) {
                    break;
                }
                let Some(sig2) = file_sig(&path) else { continue };
                if sig2 != sig1 {
                    continue;
                }
                processed.insert(path.clone(), sig2);
            } else {
                processed.insert(path.clone(), sig1);
            }

            let stem = stem_for_watch(&path, &config.in_dir, &mut assigned_stems, &mut used_stems);
            let t0 = Instant::now();
            let rep = process_single_file(&path, &stem, &cfg, &eff_export);
            let elapsed_ms = t0.elapsed().as_millis() as u64;

            let ev = WatchEvent {
                file: path.clone(),
                file_name: rep.file_name.clone(),
                status: rep.status,
                colors: rep.colors,
                paths: rep.paths,
                nodes: rep.nodes,
                mae: rep.mae,
                elapsed_ms,
                error: rep.error.clone(),
            };
            if !reports_map.contains_key(&path) {
                reports_order.push(path.clone());
            }
            reports_map.insert(path.clone(), rep);
            on_event(&ev);
            events.push(ev);
            updated_any = true;
        }

        if updated_any {
            let items: Vec<BatchItemReport> = reports_order
                .iter()
                .filter_map(|p| reports_map.get(p).cloned())
                .collect();
            let summary = BatchSummary {
                total: items.len(),
                succeeded: items.iter().filter(|r| r.status == BatchStatus::Ok).count(),
                failed: items.iter().filter(|r| r.status == BatchStatus::Failed).count(),
                cancelled: items.iter().filter(|r| r.status == BatchStatus::Cancelled).count(),
                total_secs: session_start.elapsed().as_secs_f64(),
                items,
                report_csv_path: None,
                report_md_path: None,
            };
            if cfg.write_report_csv {
                let _ = write_report_csv(&summary, &config.out_dir.join("laporan-trasea.csv"));
            }
            if cfg.write_report_md {
                let _ = write_report_md(&summary, &config.out_dir.join("laporan-trasea.md"));
            }
        }

        if sleep_interruptible(poll_ms, &stop) {
            break;
        }
    }

    Ok(events)
}