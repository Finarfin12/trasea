//! Versioned `.trasea` project file (JSON, schemaVersion = 1): saves and restores
//! source image link (+ SHA-256), production mode, palette, merges, brush strokes, and export options.

use crate::brush::BrushStroke;
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

pub const PROJECT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectOptions {
    pub colors: usize,
    pub smooth: f32,
    pub keep_lines: bool,
    pub ai_clean: bool,
    pub detect_primitives: bool,
    pub symmetry: String,
    pub detect_text: bool,
    #[serde(default)]
    pub custom_palette: Option<Vec<String>>,
    #[serde(default)]
    pub palette_merges: Vec<(String, String)>,
    #[serde(default)]
    pub target_nodes: Option<usize>,
    #[serde(default)]
    pub brush_strokes: Vec<BrushStroke>,
    #[serde(default = "default_mode")]
    pub production_mode: String,
    #[serde(default)]
    pub min_stroke_pt: f64,
    #[serde(default)]
    pub strip_background: bool,
    #[serde(default)]
    pub registration_marks: bool,
}

fn default_mode() -> String {
    "standard".to_string()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectExportOptions {
    pub lines_as: String,
    pub trap_pt: f64,
    #[serde(default)]
    pub registration_marks: bool,
    #[serde(default)]
    pub strip_background: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFile {
    pub schema_version: u32,
    pub app_version: String,
    pub source_path: String,
    pub source_sha256: String,
    pub options: ProjectOptions,
    pub export_options: ProjectExportOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedProject {
    pub project: ProjectFile,
    pub source_exists: bool,
    pub hash_matches: bool,
}

/// Compute lowercase hex SHA-256 digest of a file on disk.
pub fn sha256_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(|e| Error::Other(format!("Gagal membaca file {}: {e}", path.display())))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(64);
    for b in digest {
        use std::fmt::Write;
        let _ = write!(&mut out, "{b:02x}");
    }
    Ok(out)
}

/// Write a `.trasea` project file as readable UTF-8 JSON.
pub fn save_project(path: &Path, project: &ProjectFile) -> Result<()> {
    let json = serde_json::to_string_pretty(project)
        .map_err(|e| Error::Other(format!("Gagal menyusun file proyek: {e}")))?;
    std::fs::write(path, json).map_err(|e| Error::Other(format!("Gagal menyimpan proyek {}: {e}", path.display())))
}

/// Load a `.trasea` project file, validate its schema version, and check whether the referenced
/// source image exists and still matches its recorded SHA-256 hash.
pub fn load_project(path: &Path) -> Result<LoadedProject> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| Error::Other(format!("Gagal membuka file proyek {}: {e}", path.display())))?;
    let project: ProjectFile = serde_json::from_str(&raw)
        .map_err(|e| Error::Other(format!("Format file .trasea tidak valid: {e}")))?;
    if project.schema_version != PROJECT_SCHEMA_VERSION {
        return Err(Error::Other(format!(
            "File proyek ini dibuat dengan versi format lebih baru (schemaVersion={}). Perbarui Trasea untuk membukanya.",
            project.schema_version
        )));
    }
    let src = Path::new(&project.source_path);
    let source_exists = !project.source_path.is_empty() && src.is_file();
    let hash_matches = if source_exists && !project.source_sha256.is_empty() {
        sha256_file(src)
            .map(|h| h.eq_ignore_ascii_case(&project.source_sha256))
            .unwrap_or(false)
    } else {
        false
    };
    Ok(LoadedProject {
        project,
        source_exists,
        hash_matches,
    })
}
