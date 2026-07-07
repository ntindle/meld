use meld::{backup, cli, config, diff, discover, errors, lock, logging, manifest, restore, scan, sync, watch};

use clap::Parser;
use cli::{Cli, Command, ConfigAction};
use config::Config;
use errors::Result;
use manifest::Manifest;

fn main() {
    let cli = Cli::parse();
    logging::set_verbose(cli.verbose);
    logging::set_json(cli.json);
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<()> {
    let cfg = Config::load()?;

    match cli.command {
        Command::Config { action: ConfigAction::Init } => {
            let path = Config::init()?;
            logging::info(&format!("wrote {}", path.display()));
        }
        Command::Config { action: ConfigAction::Show } => {
            println!("{}", toml::to_string_pretty(&cfg)?);
        }
        Command::Scan => {
            let m = do_scan(&cfg)?;
            let unique: std::collections::BTreeSet<_> =
                m.entries.iter().map(|e| &e.relative_path).collect();
            logging::info(&format!(
                "Found {} accounts with {}.",
                m.account_roots.len(),
                sync::conversations(unique.len())
            ));
            logging::verbose(&format!("index saved to {}", cfg.manifest_path().display()));
        }
        Command::Status => {
            let m = do_scan(&cfg)?;
            let report = diff::compute(&m);
            if cli.json {
                print_status_json(&m, &report)?;
            } else {
                print_status(&m, &report);
            }
        }
        Command::Diff => {
            let m = do_scan(&cfg)?;
            let report = diff::compute(&m);
            if cli.json {
                print_status_json(&m, &report)?;
            } else {
                print_diff(&m, &report);
            }
        }
        Command::Sync { dry_run, no_backup } => {
            let _lock = lock::Lock::acquire(&cfg)?;
            let roots = discover::discover_account_roots(&cfg)?;
            let m = do_scan(&cfg)?;
            let report = diff::compute(&m);
            sync::apply(&cfg, &report, &roots, dry_run, no_backup)?;
            if !dry_run {
                let m2 = scan::scan(&cfg, &roots, &m)?;
                m2.save(&cfg.manifest_path())?;
            }
        }
        Command::Watch { dry_run, no_backup } => {
            let _lock = lock::Lock::acquire(&cfg)?;
            watch::watch(&cfg, dry_run, no_backup)?;
        }
        Command::Backup => {
            let roots = discover::discover_account_roots(&cfg)?;
            backup::snapshot(&cfg, &roots)?;
        }
        Command::Restore { snapshot, dry_run } => {
            let _lock = lock::Lock::acquire(&cfg)?;
            restore::restore(&cfg, snapshot.as_deref(), dry_run)?;
        }
        Command::Doctor => doctor(&cfg)?,
    }
    Ok(())
}

fn do_scan(cfg: &Config) -> Result<Manifest> {
    let roots = discover::discover_account_roots(cfg)?;
    let previous = Manifest::load_or_default(&cfg.manifest_path());
    let m = scan::scan(cfg, &roots, &previous)?;
    m.save(&cfg.manifest_path())?;
    Ok(m)
}

/// "Account 2 (bac90339)" — stable, human-scannable account label.
fn account_label(m: &Manifest, root: &std::path::Path) -> String {
    let idx = m
        .account_roots
        .iter()
        .position(|r| r == root)
        .map(|i| i + 1)
        .unwrap_or(0);
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    let short: String = name.chars().take(8).collect();
    format!("Account {idx} ({short})")
}

fn print_status(m: &Manifest, report: &diff::DiffReport) {
    let unique: std::collections::BTreeSet<_> =
        m.entries.iter().map(|e| &e.relative_path).collect();
    println!(
        "You have {} across {} accounts.\n",
        sync::conversations(unique.len()),
        m.account_roots.len()
    );
    for root in &m.account_roots {
        println!(
            "  {}: {}",
            account_label(m, root),
            sync::conversations(m.entries_for(root).len())
        );
    }
    println!();
    if report.copies.is_empty() {
        println!("All accounts are up to date.");
    } else {
        println!(
            "{} waiting to sync. Run `meld sync` to update all accounts.",
            sync::conversations(report.copies.len())
        );
    }
    if !report.conflicts.is_empty() {
        let n = report.conflicts.len();
        println!(
            "\n{} {} in two versions (both kept, nothing was changed):",
            sync::conversations(n),
            if n == 1 { "exists" } else { "exist" }
        );
        for cf in &report.conflicts {
            let newest = cf.variants.iter().max_by_key(|v| v.mtime);
            if let Some(v) = newest {
                println!("  - {}", sync::describe(v));
            }
        }
        println!("These are usually the same chat continued in different accounts.");
    }
}

fn print_diff(m: &Manifest, report: &diff::DiffReport) {
    if report.copies.is_empty() {
        println!("Nothing to sync — every account has the same conversations.");
        return;
    }
    // Group pending copies by destination account.
    let mut by_dest: std::collections::BTreeMap<&std::path::Path, Vec<&diff::CopyAction>> =
        std::collections::BTreeMap::new();
    for c in &report.copies {
        by_dest.entry(c.dest_root.as_path()).or_default().push(c);
    }
    for (dest, copies) in by_dest {
        println!(
            "{} will receive {}:",
            account_label(m, dest),
            sync::conversations(copies.len())
        );
        for c in copies {
            println!("  - {}", sync::describe(&c.source));
        }
    }
    println!("\nRun `meld sync` to apply, or `meld sync --dry-run` to preview again.");
}

fn print_status_json(m: &Manifest, report: &diff::DiffReport) -> Result<()> {
    let out = serde_json::json!({
        "sessions_root": m.sessions_root,
        "account_roots": m.account_roots,
        "total_files": m.entries.len(),
        "pending_copies": report.copies.iter().map(|c| serde_json::json!({
            "relative_path": c.source.relative_path,
            "dest_root": c.dest_root,
        })).collect::<Vec<_>>(),
        "conflicts": report.conflicts.iter().map(|c| serde_json::json!({
            "relative_path": c.relative_path,
            "variants": c.variants.len(),
        })).collect::<Vec<_>>(),
    });
    println!("{}", serde_json::to_string_pretty(&out)?);
    Ok(())
}

fn doctor(cfg: &Config) -> Result<()> {
    let mut ok = true;
    let check = |pass: bool, msg: &str| {
        println!("{} {msg}", if pass { "✓" } else { "✗" });
        pass
    };

    let found = cfg.sessions_root.exists();
    ok &= check(
        found,
        if found {
            "Claude Code is installed and has session data"
        } else {
            "Could not find Claude Code session data — is the desktop app installed?"
        },
    );
    logging::verbose(&format!("sessions root: {}", cfg.sessions_root.display()));

    if found {
        match discover::discover_account_roots(cfg) {
            Ok(roots) => {
                let n = roots.len();
                ok &= check(
                    n > 0,
                    &if n > 0 {
                        format!("{n} account{} found", if n == 1 { "" } else { "s" })
                    } else {
                        "No accounts found yet — sign in to Claude Code first".into()
                    },
                );
                if let Some(r) = roots.first() {
                    let probe = r.join(".meld-doctor-probe");
                    let writable = std::fs::write(&probe, b"probe").is_ok();
                    let _ = std::fs::remove_file(&probe);
                    ok &= check(
                        writable,
                        if writable {
                            "Meld is allowed to read and update your sessions"
                        } else {
                            "Meld doesn't have permission to update your sessions — check Full Disk Access in System Settings"
                        },
                    );
                }
            }
            Err(e) => {
                ok &= check(false, &format!("Could not look for accounts: {e}"));
            }
        }
    }

    let state_ok = std::fs::create_dir_all(&cfg.state_dir).is_ok();
    ok &= check(
        state_ok,
        if state_ok {
            "Backups and settings are ready"
        } else {
            "Could not prepare meld's own folder"
        },
    );
    logging::verbose(&format!("meld folder: {}", cfg.state_dir.display()));

    let snaps = backup::list_snapshots(cfg)?.len();
    if snaps > 0 {
        println!(
            "✓ {snaps} backup{} available (undo any sync with `meld restore`)",
            if snaps == 1 { "" } else { "s" }
        );
    }

    if ok {
        println!("\nEverything looks good. Run `meld sync` whenever you like.");
        Ok(())
    } else {
        anyhow::bail!("some checks failed (see above)")
    }
}
