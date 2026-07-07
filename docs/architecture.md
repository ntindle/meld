# Architecture

Pipeline (each stage is one module in `src/`):

```
discover ──▶ scan ──▶ manifest ──▶ diff ──▶ sync
   ▲                                          │
   └────────────── watch (debounced) ◀────────┘
              backup / restore / lock around everything
```

## Layers

1. **Discovery** (`discover.rs`) — account roots = immediate subdirectories of
   the sessions root. Nothing about names is trusted; they are only identity.
2. **Scanner** (`scan.rs`) — walkdir over each root; collects `.json` files;
   sha256 each (reused from the previous manifest when size+mtime unchanged);
   infers session id from JSON `sessionId`, falling back to the
   `local_<uuid>.json` filename. Files modified <2 s ago are skipped for one
   cycle (write-quiet guard).
3. **Manifest** (`manifest.rs`) — the current truth, persisted as pretty JSON
   at `~/.meld/manifest.json` with an atomic temp+rename save. JSON over
   SQLite is deliberate: the store is thousands of small files at most.
4. **Diff engine** (`diff.rs`) — groups entries by relative path; computes
   copies (union mirror) and conflicts (same path, >1 distinct hash). The
   newest-mtime variant is canonical only for filling absent trees.
5. **Sync engine** (`sync.rs`) — snapshot first (unless `--no-backup`), then
   for each copy: re-check destination existence (race guard), write to
   `<dest>.meld-tmp`, fsync, rename, preserve source mtime via `utimes`.
6. **Watcher** (`watch.rs`) — `notify` on the sessions root, recursive, so new
   account folders are seen. Events are drained until quiet for
   `debounce_ms`, then a full scan→diff→sync cycle runs. Meld's own
   `.meld-tmp` files are filtered to avoid feedback loops.
7. **Safety** (`backup.rs`, `restore.rs`, `lock.rs`) — timestamped full
   snapshots under `~/.meld/backups/` (pruned to `max_snapshots`); restore
   copies snapshot files back (atomically) and never deletes newer files;
   a pid lockfile serializes sync/watch/restore, reclaiming stale locks.

## Failure model

- Crash mid-sync: worst case is an orphan `.meld-tmp` file; destinations are
  either the old file, absent, or the fully-written new file — never partial.
- Concurrent Claude Code writes: write-quiet guard skips hot files; the
  destination re-check prevents clobbering files that appeared mid-cycle.
- Bad manifest: it is a cache, not truth — deleted/corrupt manifests are
  rebuilt from disk on the next scan.
