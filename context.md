# Meld — working context

Last updated: 2026-07-07

## Current state

MVP implemented end-to-end (PRD phases 1–3, plus parts of 4/5):

- discover / scan / manifest / status — done
- diff engine, sync (atomic copy, backups, dry-run), restore — done
- watcher with debounce + auto-sync, new-account detection — done
- doctor, config init/show, `--json` output, pid lock — done
- integration tests (`tests/`) + `scripts/smoke-test.sh` — done, passing

Not yet done (roadmap tail): colored output, progress bars, shell
completions, packaged release binaries, schema-drift detection.

## Verified facts about the real store (this machine, 2026-07-07)

- Layout: `claude-code-sessions/<account-uuid>/<workspace-uuid>/local_<session-uuid>.json`
- 3 account roots exist; the workspace uuid is shared across them.
- Session JSON is a single object with keys incl. `sessionId`, `cliSessionId`,
  `cwd`, `createdAt`, `lastActivityAt`, `title`, `model`, `isArchived`, …
- `sessionId` matches the `local_<uuid>` filename → used as the inferred id.
- `.DS_Store` files exist at every level (ignored via config).

## Key decisions

- **Union mirror** model exactly as the PRD: pool all files, mirror everywhere.
- **Conflicts are never auto-resolved**: same relative path, different hash →
  report only; the newest variant is only used to fill trees that lack the
  path entirely.
- **JSON manifest instead of SQLite** — thousands of small files max; keeps
  the binary dependency-free of native SQLite.
- **No `libc` crate**: two tiny `extern "C"` shims (`kill` for stale-lock
  detection, `utimes` for mtime preservation). macOS-first per PRD.
- mtime is preserved on copy so idempotency and newest-wins are stable.

## Real-store sync: DONE (2026-07-07)

First real sync ran successfully: 29 files copied, all 3 accounts now hold
18/18 session files. Snapshot at `~/.meld/backups/20260707T143626010Z`.
4 conflicts remain flagged (same session id, divergent content between the
`a363…` and `d9d7…` accounts) — deliberately left untouched.

## Distribution (added 2026-07-07)

- `install.sh` — curl-able installer (prebuilt download, cargo fallback).
- `.github/workflows/release.yml` — builds arm64+x86_64 macOS binaries on `v*` tags.
- `Formula/meld.rb` — Homebrew formula for a personal tap (core name `meld`
  is taken by the GUI diff tool → must be `brew install OWNER/tap/meld`).
- Blocked on: pushing the repo to GitHub and replacing `OWNER` placeholders.

## Next steps (in order)

1. Optional: LaunchAgent plist for `meld watch` at login.
3. Phase 4 polish: colored output, `--json` on more commands, completions.
4. Conflict-resolution UX (`meld resolve`?) once real conflicts are observed.
