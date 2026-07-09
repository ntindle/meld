# Claude Code desktop session store — observed format

Verified on macOS, Claude desktop, 2026-07-07 (this machine). The Windows
path (`%APPDATA%\Claude\claude-code-sessions`) mirrors the same per-account /
per-workspace layout; the schema is identical across platforms.

## Layout

```
<sessions-root>/
├─ <account-uuid>/                 # one per signed-in account
│  ├─ .DS_Store                    # Finder noise, ignored (macOS)
│  └─ <workspace-uuid>/            # shared across accounts on one machine
│     └─ local_<session-uuid>.json # one file per session
```

`<sessions-root>` per platform (see `config::default_sessions_root`):

- macOS:   `~/Library/Application Support/Claude/claude-code-sessions`
- Windows: `%APPDATA%\Claude\claude-code-sessions`
- Linux:   `~/.config/Claude/claude-code-sessions` (untested — no Linux desktop)

Observed: 3 account roots, all containing the same workspace uuid; session
files overlap partially between accounts (the fragmentation meld fixes).

## Session file schema (top-level keys observed)

Single JSON object. Keys seen:

`sessionId`, `cliSessionId`, `cwd`, `originCwd`, `lastFocusedAt`,
`createdAt`, `lastActivityAt`, `model`, `effort`, `isArchived`, `title`,
`titleSource`, `permissionMode`, `enabledMcpTools`,
`remoteMcpServersConfig`, `chromePermissionMode`, `error`, `errorAt`,
`alwaysAllowedReasons`, `sessionPermissionUpdates`

- `sessionId` matches the `<session-uuid>` in the filename → meld prefers the
  JSON field and falls back to the filename.
- Timestamps appear to be epoch-based / ISO fields; meld does not parse them
  (mtime is used for newest-wins).

## Caveats

- Schema is undocumented and may drift between desktop versions. Meld treats
  files as opaque bytes for sync purposes; only `sessionId` is read, and its
  absence is tolerated.
- There is a sibling `local-agent-mode-sessions/` folder — intentionally NOT
  synced by meld (out of scope until its semantics are understood).
