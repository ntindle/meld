use crate::errors::Result;
use anyhow::Context;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// One indexed session file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    /// Absolute path of the account root this file lives under.
    pub account_root: PathBuf,
    /// Absolute path of the file.
    pub file_path: PathBuf,
    /// Path relative to the account root (the sync key).
    pub relative_path: PathBuf,
    pub sha256: String,
    /// Hash of the JSON content with volatile bookkeeping fields
    /// (lastFocusedAt, lastActivityAt, ...) removed. Two files with equal
    /// content_hash are the same conversation, even if activity timestamps
    /// drifted between accounts. Falls back to sha256 for unparseable files.
    #[serde(default)]
    pub content_hash: String,
    pub size: u64,
    /// mtime as seconds since the unix epoch.
    pub mtime: i64,
    /// Session id inferred from JSON content, falling back to filename.
    pub session_id: Option<String>,
    /// Conversation title from the session JSON, when present.
    #[serde(default)]
    pub title: Option<String>,
    /// RFC 3339 timestamp of the scan that last saw this file.
    pub last_seen_at: String,
}

/// The persisted view of every discovered tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub generated_at: String,
    pub sessions_root: PathBuf,
    pub account_roots: Vec<PathBuf>,
    pub entries: Vec<FileEntry>,
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Manifest> {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("reading manifest {}", path.display()))?;
        Ok(serde_json::from_str(&raw)
            .with_context(|| format!("parsing manifest {}", path.display()))?)
    }

    pub fn load_or_default(path: &Path) -> Manifest {
        if path.exists() {
            Manifest::load(path).unwrap_or_default()
        } else {
            Manifest::default()
        }
    }

    /// Atomic save: temp file + rename.
    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_string_pretty(self)?)?;
        std::fs::rename(&tmp, path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), path.display()))?;
        Ok(())
    }

    /// Entries grouped by account root, in the order of `account_roots`.
    pub fn entries_for(&self, root: &Path) -> Vec<&FileEntry> {
        self.entries.iter().filter(|e| e.account_root == root).collect()
    }
}
