# Remote Workspace Sync Design

Status: planning document only. This file specifies a future remote-session
workspace sync path and changes no runtime invariant.

Grounding:

- Current pinned checkout materializer:
  `apps/pylon/src/workspace-materializer.ts`
- Crabbox audit:
  `docs/autopilot-coder/2026-06-13-crabbox-pylon-audit.md`

## Boundary

Pylon's assignment-intake workspace contract remains unchanged:
`workspace.kind = "git_checkout"` means a validated public GitHub repository
at a pinned 40-character commit SHA. The current materializer rejects malformed
payloads before filesystem work, fetches the pinned commit, materializes an
assignment-scoped detached checkout or worktree under a Pylon-owned cache, and
projects only public-safe refs.

Dirty-overlay sync is a separate operator/session execution feature. It is for
remote sessions where an operator has a local dirty checkout and wants to run a
bounded command on a remote execution box using the same working state. It does
not alter assignment payload validation, accepted-work authority, proof
authority, payment authority, public projections, or the redaction law around
local paths.

## Goal

For a remote execution session, Pylon should sync exactly the Git-managed local
working set needed to reproduce the operator's dirty checkout on the remote
box:

1. Seed the remote directory from a reachable origin commit.
2. Overlay only the local dirty delta.
3. Skip transfer entirely when local and remote fingerprints match.
4. Fail closed on unexpectedly large transfers or mass deletion sets.

This follows the Crabbox pattern summarized in the audit: manifest from
`git ls-files --cached --others --exclude-standard -z`, built-in excludes plus
ignore file handling, local and remote fingerprints, remote Git seeding, and
large-sync/mass-deletion guardrails.

## Inputs

The remote sync feature should require an explicit local repository root and a
remote execution target owned by the session. It should derive repository state
from Git, not from assignment payload text.

Minimum local state:

- repository root path, local-only
- current `HEAD` commit
- reachable origin URL or configured upstream remote
- selected seed commit, normally `HEAD` or the merge base/base commit chosen by
  the session launcher
- dirty working tree manifest
- `.pylonignore`, if present

Minimum remote state:

- remote workdir path, local/session-private metadata
- remote seed commit currently checked out, if any
- last applied sync fingerprint, if any
- last applied manifest/deletion accounting, if retained

Remote paths and local paths are diagnostics-only and must not appear in public
closeouts, Forum posts, issue comments, or browser projections.

## Manifest

The manifest is Git-managed. Pylon should enumerate candidate paths with:

```sh
git ls-files --cached --others --exclude-standard -z
```

This includes tracked files and nonignored untracked files, while excluding
paths ignored by `.gitignore`, `.git/info/exclude`, and global Git excludes.
The NUL-delimited form is required so path names are not parsed by whitespace.

Pylon then applies additional filtering:

- Reject absolute paths and paths with `..` traversal.
- Normalize to repository-relative POSIX paths for hashing and transport.
- Exclude `.git/` and all Git internals.
- Exclude default generated/dependency/cache directories.
- Apply `.pylonignore` from the repository root.

Default excludes should be conservative and operator-overridable only by
explicit configuration:

```text
.git/
node_modules/
dist/
build/
coverage/
.next/
.turbo/
.cache/
target/
.DS_Store
*.log
```

`.pylonignore` uses gitignore-style patterns and is evaluated after
`git ls-files --exclude-standard`, so it can further reduce the sync set but
does not make Git-ignored files eligible by default. A later implementation may
add an explicit allowlist mode for unusual generated inputs, but the default
path should avoid dependency caches, build output, local runtime state, and
large binary trees.

The manifest record should include path, file mode class, size, mtime seconds
if needed for fast preflight, and content hash for the final fingerprint.
Symlinks should be represented as symlinks and hashed by link target text, not
by traversing outside the repository.

## Fingerprinting

Pylon should compute a local fingerprint from the normalized manifest:

- repository identity
- seed commit
- `.pylonignore` content hash or absence marker
- default exclude version
- sorted path entries
- file mode class
- file content hash, or symlink target hash
- deletion set relative to the remote seeded tree

The remote side should compute or read the same fingerprint for its current
workdir. If the local fingerprint matches the remote fingerprint, Pylon skips
the file transfer and records a sync-skip event for the private session stream.

The fingerprint must be independent of absolute local and remote paths. It
should be stable across machines for identical repository content and dirty
state.

Remote state may cache the last fingerprint in a Pylon-owned metadata file
outside public projections. The cache is only an optimization: if the metadata
is missing or malformed, the remote side recomputes or Pylon performs a guarded
sync.

## Remote Git Seed

Before overlaying dirty files, Pylon should seed the remote workdir from a
reachable origin commit:

1. Validate that the local repository has a seed commit hash.
2. Confirm the remote can fetch that commit from a reachable origin URL.
3. Initialize or reuse a remote Git checkout under the session workdir.
4. Fetch the seed commit, preferably depth-1 when sufficient.
5. Check out the seed commit detached.
6. Verify `git cat-file -e <seed>^{commit}` before overlay.

The remote seed makes the transfer proportional to local changes instead of the
whole repository. The overlay then sends only files whose local content differs
from the seeded tree plus deletion markers for files removed locally.

If the remote cannot fetch the seed commit from the configured origin, Pylon
fails the remote sync by default. Full archive upload should be a separate
explicit operator mode with stricter size caps and audit text, not an automatic
fallback.

## Dirty Overlay

After seeding, Pylon applies the dirty delta:

- modified tracked files
- newly added tracked files
- nonignored untracked files included by the manifest
- deletions for tracked files removed locally

The overlay must not copy `.git/` or mutate remote Git refs beyond the detached
seed checkout. It should make the remote workdir match the local manifest, not
try to create a commit. Remote execution runs against a dirty detached checkout.

The local dirty state remains operator/session state. Assignment intake still
uses the pinned `git_checkout` payload and does not accept arbitrary dirty
trees from users or agents as authority.

## Guardrails

Remote sync fails closed before transfer when preflight exceeds configured
limits. Initial limits should be explicit constants and surfaced in private
operator diagnostics:

- maximum total bytes in dirty overlay
- maximum single file size
- maximum file count
- maximum deletion count
- maximum deletion ratio relative to the seeded tracked tree
- maximum top-level directory contribution by bytes or file count
- forbidden path classes after ignore filtering

Mass deletions require special care. If the local manifest implies deleting a
large share of the seeded checkout, Pylon should stop and require an explicit
operator override for that session. The failure should report counts and top
directories, but not publish local paths beyond private diagnostics.

Large transfer failures should explain which guardrail tripped and how to
reduce the manifest, usually by adding `.pylonignore` entries or committing the
intended base state before starting the remote session.

## Session Records

Remote sync should produce private session events and refs-only public-safe
records:

- `sync.started`
- `sync.seeded`
- `sync.skipped` when fingerprints match
- `sync.overlay_applied`
- `sync.failed_guardrail`
- `sync.failed_seed_unreachable`

Private diagnostics may include byte counts, file counts, deletion counts,
guardrail names, and remote target kind. Public projections should carry only
stable refs, state, freshness timestamps, and cleanup receipt refs where
needed, following the existing workspace lease projection style.

## Relationship To Current Materializer

The current materializer remains the source of truth for assignment workspaces:

- `gitCheckoutWorkspaceFrom` continues to validate only the pinned
  `git_checkout` shape.
- `defaultGitCheckoutRunner` remains a clean detached checkout of the pinned
  public origin commit.
- `createGitWorktreeCheckoutRunner` remains a local implementation strategy for
  isolated pinned worktrees and shared bare-repo cache reuse.
- `workingDirectory` remains local-only and redacted from public surfaces.
- Workspace lease cleanup continues to act only inside Pylon-owned cache roots.

Remote dirty-overlay sync should be implemented beside this path, not inside
the assignment-intake validator. It may reuse concepts from the materializer
such as stable refs, cache-local metadata, cleanup receipts, and public-safe
projections, but it must not weaken the pinned checkout contract.

## Open Implementation Questions

- Whether `.pylonignore` should support negation patterns initially or only
  simple excludes.
- Whether the first transport should be `rsync`, tar-over-SSH, or a delegated
  provider upload API.
- How remote fingerprint metadata should be stored for providers that do not
  expose a normal filesystem.
- Which operator override shape is acceptable for large deletes and full
  archive fallback.
- Whether remote Git seed commit should default to local `HEAD`, upstream merge
  base, or the assignment's pinned commit when a remote session is launched
  from assignment work.
