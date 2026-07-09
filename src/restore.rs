use crate::backup;
use crate::config::Config;
use crate::errors::{MeldError, Result};
use crate::logging;
use anyhow::Context;
use walkdir::WalkDir;

/// Restore files from the newest snapshot (or a named one). Restore is also
/// non-destructive: it puts snapshot files back in place but never deletes
/// files that were created after the snapshot.
pub fn restore(cfg: &Config, name: Option<&str>, dry_run: bool) -> Result<usize> {
    let snaps = backup::list_snapshots(cfg)?;
    let snap = match name {
        Some(n) => snaps
            .iter()
            .find(|s| s.file_name().and_then(|f| f.to_str()) == Some(n))
            .cloned()
            .ok_or(MeldError::NoSnapshot)?,
        None => snaps.last().cloned().ok_or(MeldError::NoSnapshot)?,
    };
    let label = snap
        .file_name()
        .and_then(|f| f.to_str())
        .and_then(pretty_stamp)
        .unwrap_or_else(|| snap.display().to_string());
    logging::info(&format!("Restoring your sessions from the backup taken {label}..."));
    logging::verbose(&format!("backup location: {}", snap.display()));

    let mut restored = 0usize;
    for item in WalkDir::new(&snap).follow_links(false) {
        let item = item?;
        if !item.file_type().is_file() {
            continue;
        }
        let rel = item.path().strip_prefix(&snap)?;
        let target = cfg.sessions_root.join(rel);
        if dry_run {
            logging::verbose(&format!("would restore {}", target.display()));
            restored += 1;
            continue;
        }
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        // Atomic write: copy to temp, then rename over the target.
        let tmp = target.with_extension("meld-restore-tmp");
        std::fs::copy(item.path(), &tmp)
            .with_context(|| format!("restoring {}", target.display()))?;
        crate::manifest::replace_file(&tmp, &target)?;
        restored += 1;
        logging::verbose(&format!("restored {}", target.display()));
    }
    if dry_run {
        logging::info(&format!(
            "This would put back {restored} files. Nothing has been changed."
        ));
    } else {
        logging::info(&format!(
            "Done. {restored} files put back. Conversations created after the backup were kept."
        ));
    }
    Ok(restored)
}

/// "20260707T143626010Z" -> "on 2026-07-07 at 14:36"
fn pretty_stamp(name: &str) -> Option<String> {
    let (date, rest) = name.split_once('T')?;
    if date.len() != 8 || rest.len() < 4 {
        return None;
    }
    Some(format!(
        "on {}-{}-{} at {}:{}",
        &date[..4],
        &date[4..6],
        &date[6..8],
        &rest[..2],
        &rest[2..4]
    ))
}
