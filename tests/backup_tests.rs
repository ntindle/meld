use meld::{backup, config::Config, discover, restore};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn test_config(root: &Path, state: &Path) -> Config {
    let mut cfg = Config::default();
    cfg.sessions_root = root.to_path_buf();
    cfg.state_dir = state.to_path_buf();
    cfg
}

#[test]
fn snapshot_and_restore_roundtrip() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    let file = root.join("acct-a/ws1/local_x.json");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, r#"{"sessionId":"x","v":"original"}"#).unwrap();

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();

    backup::snapshot(&cfg, &roots).unwrap();
    assert_eq!(backup::list_snapshots(&cfg).unwrap().len(), 1);

    // Corrupt the file, then restore.
    fs::write(&file, "garbage").unwrap();
    let restored = restore::restore(&cfg, None, false).unwrap();
    assert_eq!(restored, 1);
    let body = fs::read_to_string(&file).unwrap();
    assert!(body.contains("original"));
}

#[test]
fn restore_does_not_delete_new_files() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    let root = tmp.path();
    let old = root.join("acct-a/ws1/local_x.json");
    fs::create_dir_all(old.parent().unwrap()).unwrap();
    fs::write(&old, r#"{"sessionId":"x"}"#).unwrap();

    let cfg = test_config(root, state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();
    backup::snapshot(&cfg, &roots).unwrap();

    // A new file appears after the snapshot.
    let new = root.join("acct-a/ws1/local_new.json");
    fs::write(&new, r#"{"sessionId":"new"}"#).unwrap();

    restore::restore(&cfg, None, false).unwrap();
    assert!(new.exists(), "restore must never delete files");
}
