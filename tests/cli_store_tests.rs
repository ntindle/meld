use meld::{config, diff, discover, hash, manifest::Manifest, scan, store};
use std::fs;
use std::path::Path;
use tempfile::TempDir;

fn test_config(root: &Path, state: &Path) -> config::Config {
    config::Config {
        sessions_root: root.to_path_buf(),
        state_dir: state.to_path_buf(),
        ..config::Config::default()
    }
}

fn backdate(path: &Path) {
    use std::process::Command;
    Command::new("touch")
        .args(["-t", "202001010000", path.to_str().unwrap()])
        .status()
        .unwrap();
}

fn write_file(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    // Backdate so the "recently written" guard doesn't skip it.
    backdate(path);
}

/// A Claude Code CLI tree: `<project>/<session>.jsonl` plus a nested
/// subagent transcript, with non-session noise mixed in.
fn cli_fixture(root: &Path) {
    write_file(
        &root.join("proj-alpha/11111111-2222-3333-4444-555555555555.jsonl"),
        "{\"type\":\"mode\",\"mode\":\"normal\",\"sessionId\":\"11111111-2222-3333-4444-555555555555\"}\n\
         {\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"hello\"}}\n",
    );
    // Noise: never session files, must be ignored by the scanner.
    write_file(&root.join("proj-alpha/notes.md"), "# notes\n");
    write_file(
        &root.join("proj-beta/66666666-7777-8888-9999-000000000000.jsonl"),
        "{\"type\":\"mode\",\"mode\":\"normal\",\"sessionId\":\"66666666-7777-8888-9999-000000000000\"}\n",
    );
    write_file(
        &root.join("proj-beta/agent-xyz.meta.json"),
        "{\"agentType\":\"Explore\"}\n",
    );
    write_file(
        &root.join(
            "proj-beta/66666666-7777-8888-9999-000000000000/subagents/agent-abc123def456.jsonl",
        ),
        "{\"parentUuid\":null,\"isSidechain\":true,\"agentId\":\"abc123def456\",\"type\":\"user\",\"message\":{\"role\":\"user\",\"content\":\"subagent task\"}}\n",
    );
}

fn desktop_fixture(root: &Path) {
    write_file(
        &root.join("acct-a/ws1/local_x.json"),
        r#"{"sessionId":"x"}"#,
    );
    write_file(
        &root.join("acct-b/ws1/local_y.json"),
        r#"{"sessionId":"y"}"#,
    );
}

fn scan_all(cfg: &config::Config) -> Manifest {
    let roots = discover::discover_account_roots(cfg).unwrap();
    scan::scan(cfg, &roots, &Manifest::default()).unwrap()
}

#[test]
fn cli_tree_is_detected_as_cli_store() {
    let tmp = TempDir::new().unwrap();
    cli_fixture(tmp.path());
    assert_eq!(store::detect_store_kind(tmp.path()), store::StoreKind::Cli);
}

#[test]
fn desktop_tree_is_still_detected_as_desktop() {
    let tmp = TempDir::new().unwrap();
    desktop_fixture(tmp.path());
    assert_eq!(
        store::detect_store_kind(tmp.path()),
        store::StoreKind::Desktop
    );
}

#[test]
fn empty_root_defaults_to_desktop() {
    let tmp = TempDir::new().unwrap();
    assert_eq!(
        store::detect_store_kind(tmp.path()),
        store::StoreKind::Desktop
    );
}

#[test]
fn candidates_list_desktop_first_then_cli() {
    let c = config::candidate_sessions_roots();
    assert_eq!(c.len(), 2);
    assert_eq!(c[0], config::desktop_sessions_root());
    assert_eq!(c[1], config::cli_sessions_root());
    assert_eq!(c[1].file_name().and_then(|n| n.to_str()), Some("projects"));
    assert_eq!(
        c[1].parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str()),
        Some(".claude")
    );
}

#[test]
fn prefer_existing_picks_first_store_on_disk() {
    let tmp = TempDir::new().unwrap();
    let missing = tmp.path().join("does-not-exist");
    let present = tmp.path().join("cli-projects");
    fs::create_dir_all(&present).unwrap();
    assert_eq!(
        config::prefer_existing(vec![missing.clone(), present.clone()]),
        present
    );
    // Nothing on disk: fall back to the first (desktop) candidate.
    let missing2 = tmp.path().join("also-missing");
    assert_eq!(
        config::prefer_existing(vec![missing.clone(), missing2]),
        missing
    );
}

#[test]
fn cli_root_is_a_single_account() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    cli_fixture(tmp.path());
    let cfg = test_config(tmp.path(), state.path());
    let roots = discover::discover_account_roots(&cfg).unwrap();
    assert_eq!(roots, vec![tmp.path().to_path_buf()]);
}

#[test]
fn cli_scan_indexes_jsonl_and_ignores_noise() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    cli_fixture(tmp.path());
    let cfg = test_config(tmp.path(), state.path());

    let m = scan_all(&cfg);
    assert_eq!(m.store_kind, store::StoreKind::Cli);
    assert_eq!(m.entries.len(), 3, "2 sessions + 1 subagent transcript");

    let rels: Vec<String> = m
        .entries
        .iter()
        .map(|e| e.relative_path.to_string_lossy().replace('\\', "/"))
        .collect();
    assert!(rels.contains(&"proj-alpha/11111111-2222-3333-4444-555555555555.jsonl".to_string()));
    assert!(rels.contains(&"proj-beta/66666666-7777-8888-9999-000000000000.jsonl".to_string()));
    assert!(rels.contains(
        &"proj-beta/66666666-7777-8888-9999-000000000000/subagents/agent-abc123def456.jsonl"
            .to_string()
    ));
    assert!(
        !rels
            .iter()
            .any(|r| r.ends_with(".md") || r.ends_with(".json")),
        "noise files must not be indexed: {rels:?}"
    );

    // Session ids: content field for top-level sessions, filename for
    // subagent transcripts (whose first line has no sessionId).
    let id_for = |suffix: &str| {
        m.entries
            .iter()
            .find(|e| e.relative_path.to_string_lossy().ends_with(suffix))
            .and_then(|e| e.session_id.clone())
            .unwrap()
    };
    assert_eq!(
        id_for("11111111-2222-3333-4444-555555555555.jsonl"),
        "11111111-2222-3333-4444-555555555555"
    );
    assert_eq!(id_for("agent-abc123def456.jsonl"), "abc123def456");
}

#[test]
fn cli_single_tree_has_nothing_to_sync() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    cli_fixture(tmp.path());
    let cfg = test_config(tmp.path(), state.path());

    let m = scan_all(&cfg);
    assert!(!m.entries.is_empty());
    let report = diff::compute(&m);
    assert!(report.copies.is_empty());
    assert!(report.conflicts.is_empty());
}

#[test]
fn cli_content_hash_is_byte_identity() {
    let tmp = TempDir::new().unwrap();
    let state = TempDir::new().unwrap();
    cli_fixture(tmp.path());
    let cfg = test_config(tmp.path(), state.path());

    let m = scan_all(&cfg);
    assert!(!m.entries.is_empty());
    for e in &m.entries {
        assert_eq!(e.content_hash, e.sha256);
        assert_eq!(e.content_hash, hash::sha256_file(&e.file_path).unwrap());
    }
}

#[test]
fn cli_meta_prefers_first_line_session_id() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("sess.jsonl");
    fs::write(
        &p,
        "{\"sessionId\":\"line-one-id\"}\n{\"sessionId\":\"line-two-id\"}\n",
    )
    .unwrap();
    let meta = scan::read_session_meta(&p);
    assert_eq!(meta.session_id.as_deref(), Some("line-one-id"));
}

#[test]
fn cli_meta_falls_back_to_filename_stripping_agent_prefix() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("agent-abc123.jsonl");
    fs::write(&p, "{\"type\":\"user\"}\n").unwrap();
    let meta = scan::read_session_meta(&p);
    assert_eq!(meta.session_id.as_deref(), Some("abc123"));
}

#[test]
fn manifests_without_store_kind_load_as_desktop() {
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("manifest.json");
    fs::write(
        &path,
        r#"{"generated_at":"","sessions_root":"/x","account_roots":[],"entries":[]}"#,
    )
    .unwrap();
    let m = Manifest::load(&path).unwrap();
    assert_eq!(m.store_kind, store::StoreKind::Desktop);
}

#[test]
fn desktop_meta_still_reads_whole_object() {
    let tmp = TempDir::new().unwrap();
    let p = tmp.path().join("local_x.json");
    fs::write(&p, r#"{"sessionId":"x","title":"T","cwd":"/tmp"}"#).unwrap();
    let meta = scan::read_session_meta(&p);
    assert_eq!(meta.session_id.as_deref(), Some("x"));
    assert_eq!(meta.title.as_deref(), Some("T"));
    assert_eq!(meta.cwd.as_deref(), Some("/tmp"));
}
