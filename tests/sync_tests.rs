use meld::{config::Config, diff, discover, manifest::Manifest, scan, sync};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn test_config(root: &Path, state: &Path) -> Config {
    let mut cfg = Config::default();
    cfg.sessions_root = root.to_path_buf();
    cfg.state_dir = state.to_path_buf();
    cfg
}

fn write_session(root: &Path, account: &str, ws: &str, session: &str, body: &str) {
    let dir = root.join(account).join(ws);
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("local_{session}.json"));
    fs::write(&path, body).unwrap();
    // Backdate so the "recently written" guard doesn't skip it.
    backdate(&path);
}

fn backdate(path: &Path) {
    use std::process::Command;
    Command::new("touch")
        .args(["-t", "202001010000", path.to_str().unwrap()])
        .status()
        .unwrap();
}

fn scan_all(cfg: &Config) -> Manifest {
    let roots = discover::discover_account_roots(cfg).unwrap();
    scan::scan(cfg, &roots, &Manifest::default()).unwrap()
}

#[test]
fn union_mirror_copies_missing_files_both_ways() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    write_session(root, "acct-a", "ws1", "x", r#"{"sessionId":"x"}"#);
    write_session(root, "acct-b", "ws1", "y", r#"{"sessionId":"y"}"#);

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();
    assert_eq!(roots.len(), 2);

    let m = scan_all(&cfg);
    assert_eq!(m.entries.len(), 2);

    let report = diff::compute(&m);
    assert_eq!(report.copies.len(), 2, "each account is missing one file");
    assert!(report.conflicts.is_empty());

    let out = sync::apply(&cfg, &report, &roots, false, true).unwrap();
    assert_eq!(out.copied, 2);

    assert!(root.join("acct-a/ws1/local_y.json").exists());
    assert!(root.join("acct-b/ws1/local_x.json").exists());
    // Originals untouched.
    assert!(root.join("acct-a/ws1/local_x.json").exists());
    assert!(root.join("acct-b/ws1/local_y.json").exists());
}

#[test]
fn sync_is_idempotent() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    write_session(root, "acct-a", "ws1", "x", r#"{"sessionId":"x"}"#);
    write_session(root, "acct-b", "ws1", "y", r#"{"sessionId":"y"}"#);

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();

    let report = diff::compute(&scan_all(&cfg));
    sync::apply(&cfg, &report, &roots, false, true).unwrap();

    // Copied files inherit source mtime, so no re-backdating is needed.
    let report2 = diff::compute(&scan_all(&cfg));
    assert!(report2.copies.is_empty(), "second sync should be a no-op");
}

#[test]
fn dry_run_writes_nothing() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    write_session(root, "acct-a", "ws1", "x", r#"{"sessionId":"x"}"#);
    write_session(root, "acct-b", "ws1", "y", r#"{"sessionId":"y"}"#);

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();
    let report = diff::compute(&scan_all(&cfg));

    sync::apply(&cfg, &report, &roots, true, true).unwrap();
    assert!(!root.join("acct-a/ws1/local_y.json").exists());
    assert!(!root.join("acct-b/ws1/local_x.json").exists());
}

#[test]
fn existing_files_are_never_overwritten() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    // Same relative path, different content in each account: conflict.
    write_session(root, "acct-a", "ws1", "x", r#"{"sessionId":"x","v":1}"#);
    write_session(root, "acct-b", "ws1", "x", r#"{"sessionId":"x","v":2}"#);

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();
    let report = diff::compute(&scan_all(&cfg));

    assert_eq!(report.conflicts.len(), 1);
    assert!(report.copies.is_empty(), "both trees have the path; no copy");

    sync::apply(&cfg, &report, &roots, false, true).unwrap();
    let a = fs::read_to_string(root.join("acct-a/ws1/local_x.json")).unwrap();
    let b = fs::read_to_string(root.join("acct-b/ws1/local_x.json")).unwrap();
    assert!(a.contains(r#""v":1"#));
    assert!(b.contains(r#""v":2"#));
}

#[test]
fn new_account_folder_is_filled_on_next_sync() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    write_session(root, "acct-a", "ws1", "x", r#"{"sessionId":"x"}"#);

    let cfg = test_config(root, state.path());
    // A brand-new (empty) account folder appears later.
    fs::create_dir_all(root.join("acct-new")).unwrap();

    let roots = discover::discover_account_roots(&cfg).unwrap();
    assert_eq!(roots.len(), 2);

    let report = diff::compute(&scan_all(&cfg));
    assert_eq!(report.copies.len(), 1);
    sync::apply(&cfg, &report, &roots, false, true).unwrap();
    assert!(root.join("acct-new/ws1/local_x.json").exists());
}
