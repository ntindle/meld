//! Session-store detection.
//!
//! Meld understands two on-disk layouts:
//!
//! - Desktop: `<root>/<account>/<organization>/local_<chat>.json`
//!   (one JSON object per file)
//! - CLI: `<root>/<project>/<session>.jsonl`, plus nested transcripts such
//!   as `<project>/<session>/subagents/agent_<id>.jsonl` (JSON lines)
//!
//! Detection is content-based, so an explicit `sessions_root` override keeps
//! working whatever the folder happens to be named.

use serde::{Deserialize, Serialize};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StoreKind {
    #[default]
    Desktop,
    Cli,
}

/// Classify `root` by the session files it contains. The walk is bounded:
/// session files sit at most a few levels deep in both layouts, and huge
/// trees must not slow down every command. A `local_*.json` file is the
/// desktop marker and wins over `*.jsonl`; a tree with neither (e.g. an
/// empty root) defaults to desktop.
pub fn detect_store_kind(root: &Path) -> StoreKind {
    const MAX_DEPTH: usize = 4;
    const MAX_ENTRIES: usize = 2000;

    let mut saw_jsonl = false;
    let mut visited = 0usize;
    for item in WalkDir::new(root).follow_links(false).max_depth(MAX_DEPTH) {
        visited += 1;
        if visited > MAX_ENTRIES {
            break;
        }
        let item = match item {
            Ok(i) => i,
            Err(_) => continue,
        };
        if !item.file_type().is_file() {
            continue;
        }
        let name = item.file_name().to_string_lossy();
        if name.starts_with("local_") && name.ends_with(".json") {
            return StoreKind::Desktop;
        }
        if name.ends_with(".jsonl") {
            saw_jsonl = true;
        }
    }
    if saw_jsonl {
        StoreKind::Cli
    } else {
        StoreKind::Desktop
    }
}
