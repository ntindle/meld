use crate::config::Config;
use crate::errors::Result;
use crate::logging;
use anyhow::Context;
use chrono::Utc;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Snapshot every account tree into ~/.meld/backups/<timestamp>/ preserving
/// the layout relative to the sessions root. Returns the snapshot dir.
pub fn snapshot(cfg: &Config, roots: &[PathBuf]) -> Result<PathBuf> {
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%3fZ").to_string();
    let dest = cfg.backups_dir().join(&stamp);
    std::fs::create_dir_all(&dest)?;

    let mut copied = 0usize;
    for root in roots {
        for item in WalkDir::new(root).follow_links(false) {
            let item = item?;
            if !item.file_type().is_file() {
                continue;
            }
            let path = item.path();
            if cfg.is_ignored(path) {
                continue;
            }
            let rel = path.strip_prefix(&cfg.sessions_root)?;
            let target = dest.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(path, &target)
                .with_context(|| format!("backing up {}", path.display()))?;
            copied += 1;
        }
    }
    logging::info(&format!(
        "Backup saved ({copied} files). You can undo changes any time with `meld restore`."
    ));
    logging::verbose(&format!("backup location: {}", dest.display()));
    prune(cfg)?;
    Ok(dest)
}

pub fn list_snapshots(cfg: &Config) -> Result<Vec<PathBuf>> {
    let dir = cfg.backups_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut snaps: Vec<PathBuf> = std::fs::read_dir(&dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .collect();
    snaps.sort(); // timestamps sort lexicographically
    Ok(snaps)
}

/// Keep only the newest `max_snapshots`.
fn prune(cfg: &Config) -> Result<()> {
    let snaps = list_snapshots(cfg)?;
    if snaps.len() > cfg.max_snapshots {
        for old in &snaps[..snaps.len() - cfg.max_snapshots] {
            logging::verbose(&format!("pruning old snapshot {}", old.display()));
            std::fs::remove_dir_all(old)?;
        }
    }
    Ok(())
}

/// Copy a single file into the current snapshot-like backup area before an
/// overwrite. Not used by the union-mirror MVP (it never overwrites), but
/// kept for future conflict-resolution modes.
#[allow(dead_code)]
pub fn backup_file(cfg: &Config, path: &Path) -> Result<PathBuf> {
    let stamp = Utc::now().format("%Y%m%dT%H%M%S%3fZ").to_string();
    let rel = path.strip_prefix(&cfg.sessions_root)?;
    let target = cfg.backups_dir().join(format!("file-{stamp}")).join(rel);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(path, &target)?;
    Ok(target)
}
