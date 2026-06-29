# The Workspace Materializer

Issues #4798 and #4799 (epic #4786). Source:
`apps/pylon/src/workspace-materializer.ts`; tests:
`apps/pylon/tests/workspace-materializer.test.ts` and
`apps/pylon/tests/workspace-worktree.test.ts`. Background:
`docs/autopilot-coder/2026-06-11-autopilot-worktree-support-audit.md`.

## What it is

The adapter-neutral service behind every `workspace.kind = "git_checkout"`
coding assignment. Both local coding adapters — the Claude Agent lane
(B2 #4756) and the Codex lane (CX5 #4792) — consume the same validator,
the same checkout strategy, and the same cleanup law from this one module.
"Worktree" is an implementation strategy inside Pylon; the wire contract
never changes.

## The contract

`gitCheckoutWorkspaceFrom` decodes the shared payload and rejects, before
any filesystem work: private repositories, non-GitHub providers, unsafe
repository names, unpinned or malformed commits (a 40-character SHA is
required), branch values carrying traversal, absolute verification paths,
`..` in arguments, and shell-shaped verification commands. Verification is
argv-only and runs without a shell.

## Materialization strategies

- **`git_worktree` (default).** A shared bare-repo cache lives under the
  Pylon-owned `workspace-git-cache` root, keyed by a stable hash of the
  repository full name. The pinned commit is fetched depth-1 only when its
  object is missing, verified with `git cat-file -e <sha>^{commit}` before
  any worktree work, pinned against gc with a cache-local ref, and checked
  out as an isolated detached worktree at the assignment-scoped path.
  Operations against one bare cache are serialized in process and across
  Pylon processes with a heartbeat lock directory, so concurrent Codex fleet
  assignments cannot race on Git's shared lockfiles. Git commands retry
  bounded transient lock failures, and auto-gc/maintenance is disabled on
  every materialization because background maintenance can otherwise escape
  the critical section and collide with a sibling checkout (#6434). Concurrent
  assignments for the same repository get separate refs and directories.
  Measured against the live B2 fixture repository: cold materialization
  ~0.7s, warm cross-adapter materialization ~70ms. The cross-process gate is
  `bun apps/pylon/scripts/concurrent-checkout-proof.ts --workers 12`, which
  asserts zero `workspace_checkout_failed` outcomes for many workers sharing
  one bare cache.
- **`detached_checkout`.** The original B2-proven full detached checkout
  (`defaultGitCheckoutRunner`), available by explicit injection.
- **`injected`.** Test seams.

## Leases, TTL, and cleanup receipts

`materializeGitCheckoutWorkspaceWithLease` writes a workspace lease record
(`openagents.pylon.workspace_lease.v1`) under `workspace-leases` carrying
materialization state, TTL (default 24h), retention policy
(`retain_until_ttl` default, `remove_on_closeout` via `releaseWorkspace`),
and — once cleaned — a cleanup receipt ref. Every materialization starts
with an opportunistic sweep (`cleanupExpiredWorkspaces`), so the cache is
self-maintaining without a daemon. Cleanup acts only on targets that
resolve strictly inside the recorded Pylon-owned cache root; a tampered
record pointing elsewhere is never acted on.

Cleanup deletes only workspaces it can prove are clean. A dirty or unreadable
git worktree is retained and the lease records a retention reason such as
`retention.workspace.dirty`; this keeps half-finished lane work available for
operator review instead of silently deleting it during TTL cleanup or closeout
release.

## Lane-scoped change capture

`captureWorkspaceChanges` and `commitWorkspaceChanges` operate from the
materialized lane worktree only. They first verify that the requested
working directory resolves strictly under the Pylon-owned cache root and that
the git top-level is exactly that lane directory. Staging uses
`git add -- <lane-relative paths>` computed from that lane's own diff and
refuses traversal, absolute paths, git metadata paths, and paths that are not
currently changed in the lane. It never shells out through `git add -A` and
never stages from an ambient operator checkout.

Change captures expose `workspaceRef`, `sourceRef`, base/head commits,
counts, file refs, commit refs, and freshness. Raw changed paths stay in the
local-only capture field for staging and are omitted from
`publicWorkspaceChangeCaptureProjection`.

`detectWorkspaceChangeConflicts` compares file refs across lane captures from
the same source. When two concurrent lanes edit the same file, the result is
`conflicted` with explicit conflict refs, so a merge/rebase decision can be
surfaced instead of silently allowing last-writer-wins.

## Projection and redaction law

`workingDirectory` and all cache mechanics are local-only: they never
appear in progress events, artifact refs, closeouts, public projections,
issue comments, Forum posts, or browser UI.
`publicWorkspaceLeaseProjection` and
`publicWorkspaceChangeCaptureProjection` emit refs, state, counts, policy,
and `generatedAt` freshness only (#4751), and are rebuilt on every state
transition by the write paths.

## Capability declaration

`withWorkspaceMaterializerCapability` declares
`capability.pylon.workspace_materializer.v1` and
`capability.pylon.workspace_cleanup_receipts.v1` at go-online when at
least one local coding lane (`capability.pylon.local_claude_agent` or
`capability.pylon.local_codex`) is declared, and strips stale
declarations when none remains.

## Authority boundary

Workspace execution creates evidence only. It never creates accepted
work, deployment authority, PR authority, spend authority, settlement
authority, payout authority, or Forum publication authority. Closeouts
stay ref-only with the no-spend invariants intact.
