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
any filesystem work: unbrokered private repositories, non-GitHub providers,
unsafe repository names, unpinned or malformed commits (a 40-character SHA is
required), branch values carrying traversal, absolute verification paths,
`..` in arguments, and shell-shaped verification commands. Verification is
argv-only and runs without a shell. Private GitHub checkouts are admitted only
when the assignment carries a `github_user_oauth` SCM broker whose
`repositoryRef`, `github.com` host, and exact `/<owner>/<repo>.git` path
match the checkout; anonymous fallback is refused for private repos.

Optional `scmAuthBroker` metadata is ref-only. The materializer accepts only
the `openagents.pylon.scm_auth_broker.v1` shape, HTTPS broker URLs without
embedded credentials, one of the known broker kinds (`forge_git_access` or
`github_user_oauth`), public-safe auth refs, a repository ref, and an allowed
`https` protocol + host + path prefix. Raw Forge tokens, GitHub PATs,
credentialed URLs, secret-looking values, unsafe fallbacks, and cache TTLs
outside the bounded limit are rejected before checkout.

## Brokered Git Credentials

When a `git_checkout` workspace carries `scmAuthBroker`, Pylon installs a
Git credential helper for that task workspace. The helper script, broker
config, and short cache live under Git's private admin directory for the
materialized worktree; they are not files in the checked-out repository and
are not projected publicly. Shared bare-cache fetches also install a transient
helper while the cache lock is held, and linked worktrees receive a
`gitdir`-scoped include that points only that task's Git admin directory at its
helper config. The materializer does not enable Git's worktree-config
extension, so the bare-cache checkout semantics stay unchanged.

The helper implements Git's credential-helper protocol. For each `get`
operation it first checks the requested protocol, host, and path prefix; an
out-of-scope request fails closed unless the assignment explicitly configured
`anonymous_read_only`. In-scope requests call the broker URL with the
workspace's public refs and the Pylon process's control-plane auth from the
environment, then return the short-lived username/password pair only to Git
stdout. The helper keeps a protocol+host+path-scoped cache bounded by the
assignment TTL setting (default 60s, max 1h) and the broker response expiry.
It never reads an embedded SCM token from the workspace payload, helper
config, repository config, or remote URL.

For Khala Code Agent Computers (#8475), the GitHub broker URL is
`/api/pylon/github/git-credentials`. The Worker authenticates the executor's
agent bearer, derives the expected `github-identity:token:<userId>` storage ref
from that owner, verifies the requested repository through GitHub before
returning a credential, and emits no token material in public responses or error
bodies. The MVP GitHub OAuth path is intentionally cache-bounded and scanner-
guarded; a GitHub App installation-token broker is the least-privilege upgrade
path once app installation is owner-approved.

BA-D3 adds the runtime enforcement sweep. `scanLongLivedScmCredentials` scans
bounded worker roots for GitHub PATs, raw Forge git tokens, credentialed Git
URLs, and Git authorization extraheaders. Codex and Claude `git_checkout`
runs invoke that scanner on the materialized checkout plus the selected
isolated account home (`CODEX_HOME` or `CLAUDE_CONFIG_DIR`) before verification
or PR publication; any finding becomes a typed
`scm_credential_policy_failed` refusal. Lease cleanup also scans before dirty
retention: a workspace that contains long-lived SCM credential material is
removed and receives a cleanup receipt even if it otherwise has dirty files.
The scanner deliberately allows the bounded short-lived helper cache under Git
admin state; that cache is not a long-lived SCM credential.

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

## Prepared-worktree cache

BA-E1 adds an optional prepared-worktree cache above the shared bare object
cache. When a lease configured with `preparedWorktreeCacheRoot` closes out
cleanly, cleanup snapshots the source checkout into a local prepared entry keyed
by repository full name plus pinned baseline commit. The snapshot record uses
`openagents.pylon.prepared_worktree_cache.v1` and the typed reuse reason
`post_completion_snapshot`.

On a later matching materialization, Pylon validates the prepared entry before
reuse: metadata must match the repo+baseline key, the prepared checkout must be
a Git root at the expected baseline, and `git status --porcelain` must be
clean. Invalid, dirty, or stale prepared entries are removed and the materializer
falls back to the normal `git_worktree` path. A cache hit restores by local
clone, `git reset --hard <baseline>`, and `git clean -ffdx`, then records the
typed reason `restore_quick_sync_reset`.

The cache is local-only. Prepared directories and metadata stay out of public
lease projections and closeouts. A byte budget
(`preparedWorktreeDiskBudgetBytes`, default 5 GiB) is enforced by evicting the
oldest prepared entries until the cache is back under budget; if a single entry
is larger than the configured budget, it is evicted too.

## Prebuilt-baseline cache

BA-E2 adds an optional prebuilt-baseline cache for cold dispatches that do not
already have an exact prepared-worktree hit. The cache is keyed by repository
full name plus branch and stores a local registry row using
`openagents.pylon.prebuilt_baseline_cache.v1`. A row records the newest upstream
branch commit observed within the bounded refresh cadence, the setup result,
the local prebuilt directory, and honest hit/miss counters.

When `prebuiltBaselineCacheRoot` is configured, materialization first checks
the exact prepared cache, then the prebuilt registry. If the registry has not
reached `prebuiltBaselineRefreshCadenceSeconds` (default 15 minutes), Pylon
does not re-query upstream. If the cadence is due, it checks the upstream branch
ref and rebuilds the prebuilt baseline when the branch advanced. Matching cold
dispatches restore by copying the prebuilt directory, preserving ignored setup
artifacts such as installed dependencies, then `git reset --hard <baseline>`
before handing the workspace to the executor.

Prebuilt rows are metrics, not authority. A requested pinned commit that does
not match the current prebuilt baseline records a miss with
`reason.workspace_prebuilt_baseline.requested_commit_not_prebuilt` and falls
back to the normal `git_worktree` materializer. Refresh, setup, or restore
failures likewise record misses and fall back. Registry rows and local
directories are never exposed through public lease projections.

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
