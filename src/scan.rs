use crate::config::Config;
use crate::errors::Result;
use crate::hash::sha256_file;
use crate::logging;
use crate::manifest::{FileEntry, Manifest};
use chrono::Utc;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use walkdir::WalkDir;

/// Files modified within this window are considered "possibly still being
/// written" and skipped for one scan cycle.
const WRITE_QUIET_SECS: i64 = 2;

/// Walk every account root and build a fresh manifest. Hashes are reused
/// from `previous` when size+mtime are unchanged, so rescans are cheap.
pub fn scan(cfg: &Config, roots: &[PathBuf], previous: &Manifest) -> Result<Manifest> {
    let now = Utc::now();
    let mut entries = Vec::new();

    for root in roots {
        for item in WalkDir::new(root).follow_links(false) {
            let item = match item {
                Ok(i) => i,
                Err(e) => {
                    logging::warn(&format!("skipping unreadable entry: {e}"));
                    continue;
                }
            };
            if !item.file_type().is_file() {
                continue;
            }
            let path = item.path();
            if cfg.is_ignored(path) {
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            let meta = match item.metadata() {
                Ok(m) => m,
                Err(e) => {
                    logging::warn(&format!("stat failed for {}: {e}", path.display()));
                    continue;
                }
            };
            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            if now.timestamp() - mtime < WRITE_QUIET_SECS {
                logging::verbose(&format!(
                    "skipping (recently written): {}",
                    path.display()
                ));
                continue;
            }

            let size = meta.len();
            let relative_path = path.strip_prefix(root)?.to_path_buf();

            // Reuse hashes if the file looks unchanged since the last scan.
            let prev = previous.entries.iter().find(|e| {
                e.file_path == path
                    && e.size == size
                    && e.mtime == mtime
                    && !e.content_hash.is_empty()
            });
            let (sha256, reused_content_hash) = match prev {
                Some(p) => (p.sha256.clone(), Some(p.content_hash.clone())),
                None => match sha256_file(path) {
                    Ok(h) => (h, None),
                    Err(e) => {
                        logging::warn(&format!("hash failed for {}: {e}", path.display()));
                        continue;
                    }
                },
            };

            let (session_id, title) = read_session_meta(path);
            let content_hash = reused_content_hash
                .or_else(|| normalized_hash(path))
                .unwrap_or_else(|| sha256.clone());
            entries.push(FileEntry {
                account_root: root.clone(),
                file_path: path.to_path_buf(),
                relative_path,
                sha256,
                content_hash,
                size,
                mtime,
                session_id,
                title,
                last_seen_at: now.to_rfc3339(),
            });
        }
    }

    Ok(Manifest {
        generated_at: now.to_rfc3339(),
        sessions_root: cfg.sessions_root.clone(),
        account_roots: roots.to_vec(),
        entries,
    })
}

/// Bookkeeping fields Claude Code rewrites every time a session is merely
/// opened or focused. Differences in these fields do not make two copies of
/// a conversation "different" for sync purposes.
const VOLATILE_KEYS: &[&str] = &[
    "lastFocusedAt",
    "lastActivityAt",
    "completedTurns",
    "errorAt",
];

/// Hash of the session JSON with volatile keys stripped and keys sorted
/// (serde_json maps preserve insertion order of the source, so re-serializing
/// a filtered BTreeMap gives a canonical form). None if the file isn't a
/// JSON object.
fn normalized_hash(path: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let obj = v.as_object()?;
    let filtered: std::collections::BTreeMap<&String, &serde_json::Value> = obj
        .iter()
        .filter(|(k, _)| !VOLATILE_KEYS.contains(&k.as_str()))
        .collect();
    let canonical = serde_json::to_string(&filtered).ok()?;
    use sha2::{Digest, Sha256};
    Some(format!("{:x}", Sha256::digest(canonical.as_bytes())))
}

/// Session id and title from JSON content. The id falls back to the
/// `local_<uuid>.json` filename convention when the field is absent.
pub fn read_session_meta(path: &Path) -> (Option<String>, Option<String>) {
    let mut id = None;
    let mut title = None;
    if let Ok(raw) = std::fs::read_to_string(path) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&raw) {
            id = v.get("sessionId").and_then(|s| s.as_str()).map(String::from);
            title = v
                .get("title")
                .and_then(|s| s.as_str())
                .map(str::trim)
                .filter(|t| !t.is_empty())
                .map(String::from);
        }
    }
    if id.is_none() {
        id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.strip_prefix("local_"))
            .map(String::from);
    }
    (id, title)
}
