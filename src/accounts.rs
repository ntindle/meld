//! Account/organization fingerprints.
//!
//! Claude Code nests sessions as `<account>/<organization>/local_<chat>.json`.
//! One account (a login, tied to an email) can belong to several
//! organizations (e.g. a personal space and a team org), or just one.
//!
//! meld cannot read the human identity — email, org name, plan — because
//! Claude keeps it behind an encrypted OAuth token. Instead it builds a
//! fingerprint from what's on disk: how many conversations, when the account
//! was last active, and which projects it worked on. That's enough to tell
//! which folder is which.

use crate::manifest::{FileEntry, Manifest};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub struct OrgView {
    pub id: String,
    pub conversations: usize,
    pub last_active: i64,
    pub top_paths: Vec<String>,
}

pub struct AccountView {
    pub root: PathBuf,
    pub conversations: usize,
    pub last_active: i64,
    pub orgs: Vec<OrgView>,
}

/// Build one view per account (top-level login folder), each with its
/// organizations, ordered by most-recently-active first.
pub fn build(manifest: &Manifest) -> Vec<AccountView> {
    let mut accounts = Vec::new();

    for root in &manifest.account_roots {
        let entries = manifest.entries_for(root);

        // Group this account's sessions by organization folder.
        let mut by_org: BTreeMap<String, Vec<&FileEntry>> = BTreeMap::new();
        for e in &entries {
            let org = e.organization().unwrap_or("(none)").to_string();
            by_org.entry(org).or_default().push(e);
        }

        let mut orgs: Vec<OrgView> = by_org
            .into_iter()
            .map(|(id, es)| OrgView {
                id,
                conversations: es.len(),
                last_active: es.iter().map(|e| e.mtime).max().unwrap_or(0),
                top_paths: top_paths(&es),
            })
            .collect();
        orgs.sort_by(|a, b| b.last_active.cmp(&a.last_active));

        accounts.push(AccountView {
            root: root.clone(),
            conversations: entries.len(),
            last_active: entries.iter().map(|e| e.mtime).max().unwrap_or(0),
            orgs,
        });
    }

    accounts.sort_by(|a, b| b.last_active.cmp(&a.last_active));
    accounts
}

/// The most common project directories for a set of sessions, prettified
/// (home collapsed to `~`, at most 3).
fn top_paths(entries: &[&FileEntry]) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for e in entries {
        if let Some(cwd) = &e.cwd {
            *counts.entry(cwd.as_str()).or_default() += 1;
        }
    }
    let mut ranked: Vec<(&str, usize)> = counts.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    ranked.into_iter().take(3).map(|(p, _)| prettify(p)).collect()
}

fn prettify(path: &str) -> String {
    let home = crate::config::home_dir();
    if let Ok(rest) = Path::new(path).strip_prefix(&home) {
        return format!("~/{}", rest.display());
    }
    path.to_string()
}

/// Short 8-char label for a UUID-ish folder name.
pub fn short(id: &str) -> String {
    id.chars().take(8).collect()
}

/// "just now" / "3 days ago" from a unix mtime, relative to `now_secs`.
pub fn humanize_age(mtime: i64, now_secs: i64) -> String {
    if mtime <= 0 {
        return "unknown".into();
    }
    let secs = (now_secs - mtime).max(0);
    let mins = secs / 60;
    let hours = mins / 60;
    let days = hours / 24;
    if days >= 2 {
        format!("{days} days ago")
    } else if days == 1 {
        "yesterday".into()
    } else if hours >= 1 {
        format!("{hours}h ago")
    } else if mins >= 1 {
        format!("{mins}m ago")
    } else {
        "just now".into()
    }
}
