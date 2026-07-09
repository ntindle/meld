# Meld

One shared Claude Code chat history across all of your accounts.

If you use Claude Code desktop with more than one account, each account keeps
its own separate session history on your Mac. Switch accounts and your recent
conversations seem to disappear. They are still on your machine — just stored
in a different account folder.

Meld fixes this. It finds every account's session folder, works out which
conversations are missing where, and copies them so every account sees the
same complete history. Everything happens locally on your Mac. Nothing is
uploaded anywhere, nothing is ever deleted, and every change can be undone.

## Install

Recommended:

```bash
curl -fsSL https://raw.githubusercontent.com/siddhjagani/meld/main/install.sh | bash
```

With Homebrew:

```bash
brew install siddhjagani/tap/meld
```

On Windows (PowerShell, no administrator needed):

```powershell
irm https://raw.githubusercontent.com/siddhjagani/meld/main/install.ps1 | iex
```

macOS (Apple Silicon and Intel) and Windows are supported.

## Get started in three commands

```bash
meld doctor          # 1. confirm meld can see your session folders
meld sync --dry-run  # 2. preview exactly what would be copied
meld sync            # 3. bring every account up to date
```

That is the whole workflow. Run `meld sync` again any time; if there is
nothing to do, it says so and touches nothing.

## Keep accounts in sync automatically

```bash
meld watch
```

Leave this running and meld will notice new conversations as they appear and
copy them across your accounts within a couple of seconds. Press Ctrl-C to
stop. Signing into a brand-new account later requires no setup — meld detects
the new account folder on its own and fills it with your existing history.

## Check where things stand

```bash
meld status
```

Shows each account, how many conversations it has, whether anything is
pending, and any conflicts (see below). Add `--json` if you want
machine-readable output.

```bash
meld diff
```

Lists the exact files a sync would copy, and where they would go.

## Undo a sync

Before meld writes anything, it saves a complete snapshot of all your session
folders. To go back to how things were:

```bash
meld restore
```

This returns every file to its state at the last snapshot. Restore is also
non-destructive: conversations created after the snapshot are left alone.
You can take a snapshot manually at any time with `meld backup`.

## About conflicts

Occasionally the same conversation exists in two accounts with different
content — usually because it was continued separately in each. Meld will
never guess which version you want. Both copies stay exactly as they are, and
the conflict is listed in `meld status` so you can see it. Nothing about a
conflict blocks the rest of your history from syncing.

## Your safety, guaranteed by design

- Meld never deletes a file. There is no code path that removes your data.
- Meld never overwrites a file. If content differs, both versions are kept.
- Every sync is preceded by a full snapshot you can restore with one command.
- Files are written atomically, so an interruption (crash, power loss) can
  never leave a half-written conversation behind.
- Everything is local. Meld makes no network connections.

## Where meld keeps its own files

Everything meld owns lives in one folder: `~/.meld` (settings, its index,
and snapshots). Your conversations are never moved from their normal
location. Removing `~/.meld` resets meld completely without touching any of
your chat history.

## Settings (optional)

Meld works with zero configuration. If you want to change defaults — for
example, keep more snapshots — create a settings file and edit it:

```bash
meld config init     # creates ~/.meld/config.toml
meld config show     # view current settings
```

## All commands

| Command | What it does |
|---|---|
| `meld sync` | Copy missing conversations into every account |
| `meld sync --dry-run` | Preview a sync without changing anything |
| `meld watch` | Sync automatically whenever something changes |
| `meld status` | Show accounts, counts, pending work, conflicts |
| `meld diff` | List exactly what a sync would copy |
| `meld backup` | Save a snapshot now |
| `meld restore` | Roll back to the newest snapshot |
| `meld doctor` | Verify folders and permissions are healthy |
| `meld scan` | Rebuild meld's index of your sessions |
| `meld config init` | Create the optional settings file |

## Uninstall

```bash
rm /usr/local/bin/meld     # or: brew uninstall meld
rm -rf ~/.meld             # optional: remove settings and snapshots
```

Your conversations are unaffected either way.
