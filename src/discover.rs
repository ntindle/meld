use crate::config::Config;
use crate::errors::{MeldError, Result};
use std::path::PathBuf;

/// Find every account root: the immediate subdirectories of the sessions
/// root. Folder names are not trusted for anything beyond identity — content
/// decides everything downstream.
pub fn discover_account_roots(cfg: &Config) -> Result<Vec<PathBuf>> {
    let root = &cfg.sessions_root;
    if !root.exists() {
        return Err(MeldError::RootNotFound(root.display().to_string()).into());
    }
    // A CLI tree has no per-account folders: the whole tree is one account.
    if crate::store::detect_store_kind(root) == crate::store::StoreKind::Cli {
        return Ok(vec![root.clone()]);
    }
    let mut roots: Vec<PathBuf> = std::fs::read_dir(root)?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
        .map(|e| e.path())
        .filter(|p| !cfg.is_ignored(p))
        .collect();
    roots.sort();
    Ok(roots)
}
