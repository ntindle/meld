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
    let kind = crate::store::detect_store_kind(&cfg.sessions_root);
    // Session files differ per store: single-object JSON for desktop,
    // JSONL transcripts for CLI. Anything else in the tree (agent
    // metadata, notes, attachments) is never a session.
    let want_ext = match kind {
        crate::store::StoreKind::Desktop => "json",
        crate::store::StoreKind::Cli => "jsonl",
    };
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
            if path.extension().and_then(|e| e.to_str()) != Some(want_ext) {
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
                        logging::warn(&format!("hash failed for {}: {e:#}", path.display()));
                        continue;
                    }
                },
            };

            let meta = read_session_meta(path);
            // Volatile-key normalization only applies to single-object
            // desktop JSON; JSONL transcripts hash by exact bytes.
            let normalized = if want_ext == "json" {
                normalized_hash(path)
            } else {
                None
            };
            let content_hash = reused_content_hash
                .or(normalized)
                .unwrap_or_else(|| sha256.clone());
            entries.push(FileEntry {
                account_root: root.clone(),
                file_path: path.to_path_buf(),
                relative_path,
                sha256,
                content_hash,
                size,
                mtime,
                session_id: meta.session_id,
                title: meta.title,
                cwd: meta.cwd,
                last_seen_at: now.to_rfc3339(),
            });
        }
    }

    Ok(Manifest {
        generated_at: now.to_rfc3339(),
        sessions_root: cfg.sessions_root.clone(),
        account_roots: roots.to_vec(),
        entries,
        store_kind: kind,
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

/// Lightweight session metadata read from the JSON.
#[derive(Default)]
pub struct SessionMeta {
    pub session_id: Option<String>,
    pub title: Option<String>,
    pub cwd: Option<String>,
}

/// Session id, title and cwd from JSON content. Desktop files are single
/// JSON objects and are read whole; JSONL transcripts can be hundreds of
/// megabytes, so only the first line is parsed (it carries the identifying
/// fields). The id falls back to the filename — minus the `local_` /
/// `agent-` prefix conventions — when the field is absent.
pub fn read_session_meta(path: &Path) -> SessionMeta {
    let is_jsonl = path.extension().and_then(|e| e.to_str()) == Some("jsonl");
    let mut meta = if is_jsonl {
        read_first_line_json(path)
            .map(|v| session_meta_from_value(&v))
            .unwrap_or_default()
    } else if let Ok(raw) = std::fs::read_to_string(path) {
        serde_json::from_str::<serde_json::Value>(&raw)
            .map(|v| session_meta_from_value(&v))
            .unwrap_or_default()
    } else {
        SessionMeta::default()
    };
    if meta.session_id.is_none() {
        meta.session_id = path.file_stem().and_then(|s| s.to_str()).map(|s| {
            s.strip_prefix("local_")
                .or_else(|| s.strip_prefix("agent-"))
                .unwrap_or(s)
                .to_string()
        });
    }
    meta
}

fn session_meta_from_value(v: &serde_json::Value) -> SessionMeta {
    let str_field = |key: &str| {
        v.get(key)
            .and_then(|s| s.as_str())
            .map(str::trim)
            .filter(|t| !t.is_empty())
            .map(String::from)
    };
    SessionMeta {
        session_id: v
            .get("sessionId")
            .and_then(|s| s.as_str())
            .map(String::from),
        title: str_field("title"),
        cwd: str_field("cwd"),
    }
}

/// Parse only the first line of a file as JSON. Bounded: lines past the cap
/// fail to parse and the caller falls back to the filename.
fn read_first_line_json(path: &Path) -> Option<serde_json::Value> {
    use std::io::{BufRead, Read};
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file.take(64 * 1024));
    let mut line = String::new();
    reader.read_line(&mut line).ok()?;
    serde_json::from_str(line.trim()).ok()
}
