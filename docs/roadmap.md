# Roadmap

## Phase 1 — foundation ✅
repo scaffold, config, discovery, scanner, manifest, status output

## Phase 2 — sync core ✅
diff engine, copy engine, backups, dry-run, rollback

## Phase 3 — automation ✅
file watcher, auto-rescan, auto-sync, health checks (`doctor`)

## Phase 4 — polish (partial)
- [x] JSON output mode (`--json` on status/diff)
- [ ] colored output
- [ ] progress bars
- [ ] shell completions
- [ ] packaged release binaries (homebrew tap?)
- [ ] LaunchAgent plist for `meld watch` at login

## Phase 5 — hardening (partial)
- [x] conflict detection (never auto-resolved)
- [x] integration tests against sample trees
- [ ] interactive conflict resolution (`meld resolve`)
- [ ] schema drift detection (warn when session JSON keys change)
- [ ] corruption safeguards (validate JSON parses before mirroring)
- [ ] Linux/Windows session-store paths
