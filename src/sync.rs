use crate::backup;
use crate::config::Config;
use crate::diff::DiffReport;
use crate::errors::Result;
use crate::logging;
use anyhow::Context;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Debug, Default)]
pub struct SyncOutcome {
    pub copied: usize,
    pub skipped: usize,
    pub conflicts: usize,
    pub snapshot: Option<PathBuf>,
}

/// Apply a diff: copy every missing file into every account tree.
///
/// Safety invariants:
/// - never deletes anything
/// - never overwrites an existing file (a file that appeared since the scan
///   is skipped, not clobbered)
/// - snapshots all trees before the first write unless `no_backup`
/// - writes are temp-file + fsync + rename, so a crash can't leave a
///   half-written session file where Claude Code will read it
pub fn apply(
    cfg: &Config,
    report: &DiffReport,
    roots: &[PathBuf],
    dry_run: bool,
    no_backup: bool,
) -> Result<SyncOutcome> {
    let mut out = SyncOutcome {
        conflicts: report.conflicts.len(),
        ..Default::default()
    };

    if report.copies.is_empty() {
        logging::info("All accounts are already up to date.");
        return Ok(out);
    }

    if dry_run {
        logging::info(&format!(
            "This would copy {}. Nothing has been changed.",
            conversations(report.copies.len())
        ));
        for c in &report.copies {
            logging::info(&format!("  - {}", describe(&c.source)));
            logging::verbose(&format!(
                "    {} -> {}",
                c.source.file_path.display(),
                c.dest_path.display()
            ));
        }
        logging::info("Run `meld sync` to apply.");
        out.copied = report.copies.len();
        return Ok(out);
    }

    if !no_backup {
        logging::info("Saving a backup of your sessions first...");
        out.snapshot = Some(backup::snapshot(cfg, roots)?);
    }

    for c in &report.copies {
        // Re-check at write time: the file may have appeared since the scan.
        if c.dest_path.exists() {
            logging::verbose(&format!(
                "skipping, destination now exists: {}",
                c.dest_path.display()
            ));
            out.skipped += 1;
            continue;
        }
        if let Some(parent) = c.dest_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = fs::read(&c.source.file_path)
            .with_context(|| format!("reading {}", c.source.file_path.display()))?;

        let tmp = c.dest_path.with_extension("meld-tmp");
        {
            let mut f = fs::File::create(&tmp)
                .with_context(|| format!("creating {}", tmp.display()))?;
            f.write_all(&data)?;
            f.sync_all()?;
        }
        fs::rename(&tmp, &c.dest_path)
            .with_context(|| format!("renaming into place {}", c.dest_path.display()))?;

        // Preserve mtime so newest-wins stays stable across trees.
        let _ = filetime_set(&c.dest_path, c.source.mtime);

        logging::verbose(&format!(
            "copied {} -> {}",
            c.source.relative_path.display(),
            c.dest_path.display()
        ));
        out.copied += 1;
    }

    logging::info(&format!(
        "Done. {} copied — all accounts now share the same history.",
        conversations(out.copied)
    ));
    if out.conflicts > 0 {
        logging::info(&format!(
            "{} with two versions were left untouched (see `meld status`).",
            conversations(out.conflicts)
        ));
    }
    Ok(out)
}

/// "1 conversation" / "N conversations"
pub fn conversations(n: usize) -> String {
    if n == 1 {
        "1 conversation".into()
    } else {
        format!("{n} conversations")
    }
}

/// Human-friendly one-liner for a session file: its title when known,
/// otherwise a shortened session id.
pub fn describe(e: &crate::manifest::FileEntry) -> String {
    if let Some(t) = &e.title {
        format!("\"{t}\"")
    } else if let Some(id) = &e.session_id {
        format!("untitled conversation ({})", &id[..id.len().min(8)])
    } else {
        e.relative_path.display().to_string()
    }
}

/// Set mtime (and atime) via utimes; best-effort.
fn filetime_set(path: &std::path::Path, mtime: i64) -> Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let c = CString::new(path.as_os_str().as_bytes())?;
    let tv = [
        libc_timeval { tv_sec: mtime, tv_usec: 0 },
        libc_timeval { tv_sec: mtime, tv_usec: 0 },
    ];
    unsafe { libc_utimes(c.as_ptr(), tv.as_ptr()) };
    Ok(())
}

#[repr(C)]
struct libc_timeval {
    tv_sec: i64,
    tv_usec: i32,
}

extern "C" {
    #[link_name = "utimes"]
    fn libc_utimes(path: *const std::os::raw::c_char, times: *const libc_timeval) -> i32;
}
