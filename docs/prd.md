# Meld PRD

## Problem

Claude Code desktop sessions fragment across accounts. When account limits or
switching happen, continuity is lost even though the session data exists
locally, scattered across multiple account trees.

## Goal

A local, reversible, account-aware sync tool that:

- discovers all Claude session folders automatically
- indexes all JSON chat files (path, size, mtime, sha256, inferred session id)
- detects new or changed files
- mirrors session files across all account trees
- never deletes user data
- handles new account folders appearing later, with no config change

## Primary user

A macOS power user running Claude Code across multiple accounts who wants one
shared local session history.

## Core principles

- Read first, write second
- No deletions
- Atomic writes only
- Backup before mutation
- Cross-account discovery is automatic
- Folder names are not trusted; content is

## Sync model: union mirror

All discovered session files form a shared pool; the pool is mirrored into
every account tree. Account A has X, account B has Y → after sync both have
X and Y.

### Conflict policy (in order)

1. Same hash everywhere → nothing to do.
2. Path missing from a tree → copy; among variants the newest mtime is
   canonical for filling gaps.
3. Same path, different content → never overwrite; flag as conflict, keep all.

## Functional requirements

1. Detect session roots under
   `~/Library/Application Support/Claude/claude-code-sessions/`.
2. Recursively scan all nested folders for JSON session files.
3. Build a manifest: path, size, mtime, hash, inferred session id.
4. Determine which chats exist in which account trees.
5. Replicate missing session files into every account tree.
6. Watch for changes and resync automatically (debounced).
7. Provide dry-run, status, diff, and rollback commands.
8. Handle new account folders without manual config.
9. Snapshot backup before any write operation.

## Non-functional requirements

Fast for thousands of files; safe on partial failure; macOS first
(cross-platform later); small binary; human-readable logs; deterministic.

## Out of scope for MVP

Cloud sync, remote server, chat editing, in-JSON conversation merging,
semantic rewriting, multi-user collaboration.

## MVP success criteria

- New account folder appears → detected automatically.
- Missing session JSON appears in all accounts after sync.
- No existing file is ever deleted or overwritten.
- Repeated syncs are idempotent and non-corrupting.
- User can inspect exactly what changed (`diff`, `status`, snapshots).
