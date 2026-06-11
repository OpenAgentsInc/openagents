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
  Operations against one bare cache are serialized in process; concurrent
  assignments for the same repository get separate refs and directories.
  Measured against the live B2 fixture repository: cold materialization
  ~0.7s, warm cross-adapter materialization ~70ms.
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

## Projection and redaction law

`workingDirectory` and all cache mechanics are local-only: they never
appear in progress events, artifact refs, closeouts, public projections,
issue comments, Forum posts, or browser UI.
`publicWorkspaceLeaseProjection` emits refs, state, policy, and
`generatedAt` freshness only (#4751), and is rebuilt on every state
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
