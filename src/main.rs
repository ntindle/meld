use meld::{
    accounts, backup, cli, config, diff, discover, errors, lock, logging, manifest, restore, scan,
    store, sync, watch,
};

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
            if m.store_kind == store::StoreKind::Cli {
                let projects: usize = accounts::build(&m).iter().map(|a| a.orgs.len()).sum();
                logging::info(&format!(
                    "Found {} in {}.",
                    sync::conversations(unique.len()),
                    pluralize(projects, "project", "projects")
                ));
            } else {
                logging::info(&format!(
                    "Found {} with {}.",
                    pluralize(m.account_roots.len(), "account", "accounts"),
                    sync::conversations(unique.len())
                ));
            }
            logging::verbose(&format!("index saved to {}", cfg.manifest_path().display()));
        }
        Command::Accounts => {
            let m = do_scan(&cfg)?;
            print_accounts(&m);
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
                print_diff(&report);
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
fn account_label(root: &std::path::Path) -> String {
    let name = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown");
    format!("Account {}", accounts::short(name))
}

fn print_accounts(m: &Manifest) {
    if m.store_kind == store::StoreKind::Cli {
        print_accounts_cli(m);
        return;
    }
    let views = accounts::build(m);
    let now = chrono::Utc::now().timestamp();
    let total: usize = views.iter().map(|a| a.conversations).sum();
    let org_count: usize = views.iter().map(|a| a.orgs.len()).sum();

    println!(
        "{} across {} — {} in total, all merged together.\n",
        sync::conversations(total),
        pluralize(views.len(), "account", "accounts"),
        pluralize(org_count, "organization", "organizations"),
    );

    for a in &views {
        let name = a.root.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        println!(
            "Account {} · {} · last active {}",
            accounts::short(name),
            sync::conversations(a.conversations),
            accounts::humanize_age(a.last_active, now),
        );
        let n = a.orgs.len();
        for (i, org) in a.orgs.iter().enumerate() {
            let branch = if i + 1 == n { "└" } else { "├" };
            let paths = if org.top_paths.is_empty() {
                String::new()
            } else {
                format!("  ({})", org.top_paths.join(", "))
            };
            println!(
                "  {branch} org {}: {} · {}{}",
                accounts::short(&org.id),
                sync::conversations(org.conversations),
                accounts::humanize_age(org.last_active, now),
                paths,
            );
        }
        println!();
    }
    println!("meld keeps all of these in sync with each other.");
}

/// CLI detail view: one group per project folder (full slug — the truncated
/// form is useless since slugs share long prefixes).
fn print_accounts_cli(m: &Manifest) {
    let views = accounts::build(m);
    let now = chrono::Utc::now().timestamp();
    let total: usize = views.iter().map(|a| a.conversations).sum();
    let orgs: Vec<&accounts::OrgView> = views.iter().flat_map(|a| a.orgs.iter()).collect();

    println!(
        "{} in this machine's CLI history, across {}.\n",
        sync::conversations(total),
        pluralize(orgs.len(), "project", "projects"),
    );
    for o in orgs {
        println!(
            "Project {} · {} · last active {}",
            o.id,
            sync::conversations(o.conversations),
            accounts::humanize_age(o.last_active, now),
        );
    }
    println!("\nOnly one session tree is configured, so everything is already in one place.");
}

fn pluralize(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

fn print_status(m: &Manifest, report: &diff::DiffReport) {
    if m.store_kind == store::StoreKind::Cli {
        print_status_cli(m, report);
        return;
    }
    let unique: std::collections::BTreeSet<_> =
        m.entries.iter().map(|e| &e.relative_path).collect();
    let views = accounts::build(m);
    let org_count: usize = views.iter().map(|a| a.orgs.len()).sum();
    println!(
        "You have {} across {} ({}).\n",
        sync::conversations(unique.len()),
        pluralize(views.len(), "account", "accounts"),
        pluralize(org_count, "organization", "organizations"),
    );
    for a in &views {
        let name = a.root.file_name().and_then(|n| n.to_str()).unwrap_or("?");
        print!("  {}: {}", accounts::short(name), sync::conversations(a.conversations));
        if a.orgs.len() > 1 {
            print!(" across {}", pluralize(a.orgs.len(), "organization", "organizations"));
        }
        println!();
    }
    println!("\n(Run `meld accounts` for more detail on each one.)");
    println!();
    print_status_tail(report, "All accounts are up to date.");
}

/// CLI overview: per-project counts (capped — machines accumulate dozens of
/// project folders) plus the shared sync state.
fn print_status_cli(m: &Manifest, report: &diff::DiffReport) {
    let unique: std::collections::BTreeSet<_> =
        m.entries.iter().map(|e| &e.relative_path).collect();
    let views = accounts::build(m);
    let orgs: Vec<&accounts::OrgView> = views.iter().flat_map(|a| a.orgs.iter()).collect();
    println!(
        "You have {} in {} on this machine.\n",
        sync::conversations(unique.len()),
        pluralize(orgs.len(), "project", "projects"),
    );
    for o in orgs.iter().take(10) {
        println!("  {}: {}", o.id, sync::conversations(o.conversations));
    }
    if orgs.len() > 10 {
        println!("  ... and {} more (see `meld accounts`)", orgs.len() - 10);
    }
    println!("\n(Run `meld accounts` for more detail on each one.)");
    println!();
    print_status_tail(
        report,
        "Only one session tree is configured — nothing to merge.",
    );
}

fn print_status_tail(report: &diff::DiffReport, up_to_date: &str) {
    if report.copies.is_empty() {
        println!("{up_to_date}");
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

fn print_diff(report: &diff::DiffReport) {
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
            account_label(dest),
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
        "store": m.store_kind,
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
    let kind = found.then(|| store::detect_store_kind(&cfg.sessions_root));
    let is_cli = kind == Some(store::StoreKind::Cli);
    ok &= check(
        found,
        if found {
            if is_cli {
                "Claude Code CLI history found on this machine"
            } else {
                "Claude Code is installed and has session data"
            }
        } else {
            "Could not find Claude Code session data — is Claude Code installed?"
        },
    );
    logging::verbose(&format!("sessions root: {}", cfg.sessions_root.display()));
    if is_cli {
        logging::verbose("store type: CLI (JSONL transcripts)");
    }
    if !found {
        for candidate in config::candidate_sessions_roots() {
            logging::verbose(&format!("checked: {}", candidate.display()));
        }
    }

    if found {
        match discover::discover_account_roots(cfg) {
            Ok(roots) => {
                let n = roots.len();
                ok &= check(
                    n > 0,
                    &if is_cli {
                        "CLI session tree found".into()
                    } else if n > 0 {
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
                        } else if cfg!(target_os = "macos") {
                            "Meld doesn't have permission to update your sessions — check Full Disk Access in System Settings"
                        } else {
                            "Meld doesn't have permission to update your sessions — check the folder's permissions"
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
