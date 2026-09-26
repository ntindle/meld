# Claude Code desktop session store — observed format

Verified on macOS, Claude desktop, 2026-07-07 (this machine). The Windows
path (`%APPDATA%\Claude\claude-code-sessions`) mirrors the same per-account /
per-workspace layout; the schema is identical across platforms.

## Layout

```
<sessions-root>/
├─ <account-uuid>/                 # one per Claude login (an email/account)
│  ├─ .DS_Store                    # Finder noise, ignored (macOS)
│  └─ <organization-uuid>/         # a personal space, a team org, etc.
│     └─ local_<session-uuid>.json # one file per session
```

Corrected 2026-07-17: the second level is per-**organization**, not a
shared "workspace". One account (login) can have more than one organization
folder nested under it — e.g. a personal space and a team it belongs to —
or just one. Two accounts can also happen to contain the same organization
(the org id repeats under each account it's shared with), which is exactly
the fragmentation meld fixes: `<account>/<org>/<chat>.json` is the sync key,
and meld mirrors it wherever that org appears.

Confusingly, organization ids and account ids are drawn from the same UUID
namespace, so an organization id can be identical to some other, unrelated
account's id purely by coincidence. Confirmed on this machine: one
account's org folder had the same UUID as a sibling top-level account. This
does not cause incorrect merging — meld keys off the full relative path
(`<account>/<org>/<chat>`), never off an org id in isolation — but it can
look alarming when eyeballing the raw folders. `meld accounts` exists
specifically so you don't have to eyeball them.

Claude does not expose the human-readable identity (email, organization
display name, plan) anywhere meld can safely read: the OAuth token is
Electron `safeStorage`-encrypted with its key in the macOS Keychain, and
`lastKnownAccountUuid` in `config.json` isn't guaranteed to match any
session folder at all. Meld will not decrypt that token or touch Keychain
secrets. Instead it fingerprints each account/org from what's already on
disk — conversation count, last-active time (max mtime), and the most
common `cwd` values — good enough to tell folders apart without ever
handling a credential.

`<sessions-root>` per platform (see `config::default_sessions_root`):

- macOS:   `~/Library/Application Support/Claude/claude-code-sessions`
- Windows: `%APPDATA%\Claude\claude-code-sessions`
- Linux:   `~/.config/Claude/claude-code-sessions` (untested — no Linux desktop)

Observed on this machine: 3 accounts; one of them nests 2 organizations
(21 and 1 conversations respectively), the other two have 1 organization
each. Session files overlap partially between accounts sharing an
organization — the fragmentation meld fixes.

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

## CLI session store (added 2026-09-26)

Observed on Windows, Claude Code CLI (no desktop app installed). The CLI
keeps its history in `~/.claude/projects` on every platform — one folder per
project, JSONL transcripts instead of single-object JSON:

```
<projects>/
└─ <project-slug>/                # cwd with separators flattened to `-`
   ├─ <session-uuid>.jsonl        # one file per session (JSON lines)
   └─ <session-uuid>/             # nested transcripts for that session
      └─ subagents/
         └─ agent_<id>.jsonl
```

Notes for meld:

- Session files are `*.jsonl` at any depth; the sync key is the path
  relative to the projects folder, exactly like the desktop store.
- The first line of a top-level session carries `sessionId` (matching the
  filename); subagent transcripts have no `sessionId`, so the id falls back
  to the filename minus the `agent-` prefix.
- The tree also contains non-session files that meld must NOT index:
  `*.meta.json`, `*.forked-skill.json` (agent metadata), plus `memory/`,
  `plans/` notes and attachments (`.md`, `.txt`, `.pdf`, ...). The scanner
  therefore filters by extension per store: `.json` for desktop, `.jsonl`
  for CLI.
- Transcripts can be huge (168 MB seen, 826 MB total on the observed
  machine), so metadata is read from the first line only (bounded 64 KiB)
  and the volatile-key normalization is skipped — CLI identity is exact
  bytes (sha256).
- A CLI tree has no per-account folders, so the whole tree counts as one
  account: scan/status/diff work, and sync is a safe no-op until a second
  tree is configured. Store detection is content-based (`local_*.json` →
  desktop, `*.jsonl` → CLI, empty → desktop); see `src/store.rs`.
- `meld doctor` probes the desktop path first, then `~/.claude/projects`,
  and reports which store it found.
