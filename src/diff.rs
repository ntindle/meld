use crate::manifest::{FileEntry, Manifest};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// A file that should be copied into an account tree.
#[derive(Debug, Clone)]
pub struct CopyAction {
    pub source: FileEntry,
    pub dest_root: PathBuf,
    pub dest_path: PathBuf,
}

/// Same relative path exists in two trees with different content. Never
/// resolved silently — surfaced to the user.
#[derive(Debug, Clone)]
pub struct Conflict {
    pub relative_path: PathBuf,
    pub variants: Vec<FileEntry>,
}

#[derive(Debug, Default)]
pub struct DiffReport {
    pub copies: Vec<CopyAction>,
    pub conflicts: Vec<Conflict>,
    /// relative_path -> roots that already have it (for status output).
    pub coverage: BTreeMap<PathBuf, Vec<PathBuf>>,
}

/// Union-mirror diff: every relative path found anywhere should exist in
/// every account root.
///
/// Conflict policy, in order:
/// 1. same relative path + same hash everywhere → nothing to do
/// 2. path missing from a root → copy (newest-mtime variant is canonical)
/// 3. same relative path, different hashes → copy is still safe for roots
///    that lack the file entirely, but existing files are NEVER overwritten;
///    the divergence is reported as a conflict.
pub fn compute(manifest: &Manifest) -> DiffReport {
    let mut by_rel: BTreeMap<PathBuf, Vec<&FileEntry>> = BTreeMap::new();
    for e in &manifest.entries {
        by_rel.entry(e.relative_path.clone()).or_default().push(e);
    }

    let mut report = DiffReport::default();

    for (rel, entries) in &by_rel {
        let have_roots: Vec<PathBuf> =
            entries.iter().map(|e| e.account_root.clone()).collect();
        report.coverage.insert(rel.clone(), have_roots.clone());

        // Canonical source: newest mtime wins among variants.
        let canonical = entries
            .iter()
            .max_by_key(|e| (e.mtime, e.size))
            .expect("non-empty group");

        // Distinct normalized hashes at the same relative path = conflict.
        // content_hash ignores volatile bookkeeping fields, so two copies of
        // a conversation that only differ in "last opened" timestamps are
        // treated as identical rather than flagged.
        let mut hashes: Vec<&str> =
            entries.iter().map(|e| e.content_hash.as_str()).collect();
        hashes.sort();
        hashes.dedup();
        if hashes.len() > 1 {
            report.conflicts.push(Conflict {
                relative_path: rel.clone(),
                variants: entries.iter().map(|e| (*e).clone()).collect(),
            });
        }

        for root in &manifest.account_roots {
            if !have_roots.contains(root) {
                report.copies.push(CopyAction {
                    source: (*canonical).clone(),
                    dest_root: root.clone(),
                    dest_path: root.join(rel),
                });
            }
        }
    }

    report
}
