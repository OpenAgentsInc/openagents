# Git And GitHub Workflow System Audit

Date: 2026-06-11

This is system #35 from the Bun/Effect terminal-agent systems list. It captures
the repository and GitHub workflow layer a terminal coding agent should own:
repository identity, safe Git state reads, bounded diff capture, branch and
pull-request delivery, review comment retrieval, and safety policy.

The key rule is that Git workflow is not just shell execution. It is a typed
delivery subsystem with explicit authority and audit records.

## Target

Build a Git workflow system that can support:

- Local coding sessions.
- Worktree-backed missions.
- Hosted job replay.
- Pull-request creation and update.
- Review comment ingestion.
- Issue and work-order writeback.
- Public evidence records.
- Non-Git workspaces where only artifacts are available.

The runtime should know when it is in a repository, which repository it is, what
changed, which branch owns the work, and what delivery action is authorized.

## User-Visible Capability

Users should be able to:

- See current branch, remote, cleanliness, and PR status.
- Ask the agent to commit a scoped set of changes.
- Ask the agent to push and open or update a pull request.
- Ask the agent to inspect PR comments.
- Ask the agent to review a pull request.
- Hand off a patch or artifact when Git delivery is not available.

The agent should refuse or escalate risky actions such as destructive resets,
force pushes, hook bypasses, or committing probable secrets.

## Core Model

Use typed records around repository state and delivery intent.

```ts
interface RepositoryIdentity {
  readonly workspaceId: string
  readonly gitRoot?: string
  readonly canonicalRoot?: string
  readonly remoteHost?: string
  readonly owner?: string
  readonly repo?: string
  readonly remoteHash?: string
}

interface GitStateSnapshot {
  readonly repository: RepositoryIdentity
  readonly headSha?: string
  readonly branchName?: string
  readonly defaultBranch?: string
  readonly upstream?: string
  readonly clean: boolean
  readonly worktreeCount?: number
  readonly capturedAt: string
}

interface DeliveryIntent {
  readonly runId: string
  readonly repository: RepositoryIdentity
  readonly mode: "commit" | "push" | "pull_request" | "patch_only"
  readonly files: readonly string[]
  readonly policy: GitSafetyPolicy
  readonly requestedBy: "user" | "scheduler" | "work_order"
}
```

The model should separate factual Git state from workflow policy. A clean
repository can still be unauthorized for push; an authorized run can still be
blocked by dirty unrelated files.

## Repository Identity

The repository reader should:

- Find the nearest Git root.
- Support normal repositories, worktrees, and submodules.
- Resolve worktrees to a canonical repository identity where shared project
  state must be shared.
- Treat submodules as separate repositories.
- Normalize remotes so SSH and HTTPS variants map to the same identity.
- Produce a non-sensitive remote hash for public logs.
- Cache common reads while invalidating when Git metadata changes.

Worktree handling needs explicit validation. Metadata files inside a repository
can be attacker-controlled, so a worktree pointer should be accepted only when
it matches the structure created by normal Git worktree operations.

## Safe Ref Parsing

Do not pass raw branch or ref text from Git metadata into shell commands.

Rules:

- Accept only full-length commit hashes where a hash is expected.
- Accept branch names through an allowlist suitable for path joins and command
  arguments.
- Reject path traversal, leading dashes, empty path components, control
  characters, shell metacharacters, and whitespace in names read from metadata.
- Treat invalid metadata as unknown state rather than trying to repair it.

This protects prompt construction, status displays, PR commands, and replay
records from repository-controlled injection.

## Diff And Change Capture

The diff subsystem should return bounded summaries first and full details on
demand.

Recommended shapes:

```ts
interface DiffSummary {
  readonly filesCount: number
  readonly linesAdded: number
  readonly linesRemoved: number
  readonly perFile: readonly FileDiffSummary[]
  readonly truncated: boolean
}

interface ChangeCapture {
  readonly baseRef?: string
  readonly baseSha?: string
  readonly headSha?: string
  readonly branchName?: string
  readonly patchRef?: string
  readonly formatPatchRef?: string
  readonly untrackedFileRefs: readonly string[]
}
```

Diff rules:

- Skip or mark diffs during merge, rebase, cherry-pick, or revert states unless
  the user explicitly asks to inspect that state.
- Use quick stats before loading large diffs.
- Cap per-file and total detail volume.
- Detect binary files before reading them.
- Capture untracked text files separately with size and count limits.
- Prefer merge-base with a remote branch for replay.
- Fall back honestly when shallow clones or missing remotes prevent a stable
  base.

## Commit Workflow

Commit creation should be an explicit delivery action.

Safety policy:

- Never amend unless the user explicitly asks.
- Never bypass hooks unless the user explicitly asks.
- Never update Git config from a generic commit command.
- Never commit likely secret files without warning and explicit user intent.
- Do not create empty commits by default.
- Use a noninteractive commit message path.
- Include attribution only through configured policy.
- Keep staging scoped to the intended files.

The agent should inspect status and diff before committing and should report
what it committed afterward.

## Pull-Request Workflow

Pull-request delivery should be a state machine:

1. Confirm repository identity.
2. Confirm branch is not the default branch, or create an approved branch.
3. Confirm working tree scope.
4. Commit or verify existing commits.
5. Push branch.
6. Detect existing PR for the branch.
7. Create or update PR title and body.
8. Attach evidence and test plan.
9. Return PR URL.

PR body generation should be structured:

- Summary.
- Test plan.
- Risk or rollout note when relevant.
- Linked work order or issue.
- Evidence artifacts.
- Attribution if configured.

Do not rely on a chat prompt alone. The runtime should have typed delivery
steps and an audit record for each step.

## Review And Comment Ingestion

The GitHub integration should ingest:

- PR status.
- Draft, open, merged, and closed state.
- Review decision.
- PR-level comments.
- Inline review comments.
- Diff hunk context.
- Threading where available.
- Referenced file and line numbers.

The response shape should preserve enough context for the agent to act on
comments without scraping rendered web pages.

## GitHub Adapter

GitHub can be accessed through a CLI, API token, app installation, or MCP
connector. Treat all of these as adapters behind one service.

```ts
interface GitHostingService {
  readonly getPrStatus: (
    repo: RepositoryIdentity,
    branch: string,
  ) => Effect.Effect<PullRequestStatus | null, GitHostingError>

  readonly createOrUpdatePullRequest: (
    request: PullRequestDeliveryRequest,
  ) => Effect.Effect<PullRequestDeliveryResult, GitHostingError>

  readonly listReviewThreads: (
    request: ReviewThreadRequest,
  ) => Effect.Effect<readonly ReviewThread[], GitHostingError>
}
```

The adapter may use shell commands internally, but the rest of the runtime
should see typed records and typed errors.

## Effect Services

Recommended service split:

- `RepositoryService`: roots, remotes, branch, worktree count, clean state.
- `GitMetadataReader`: safe metadata parsing without process spawns where
  possible.
- `DiffService`: bounded summaries and on-demand hunks.
- `ChangeCaptureService`: replayable patch and untracked-file capture.
- `GitPolicy`: destructive command policy and secret-file checks.
- `CommitService`: staging and commit creation.
- `PullRequestService`: create, update, and status.
- `ReviewIngestionService`: PR and review comment ingestion.
- `DeliveryLedger`: audit records for commit, push, PR, and fallback delivery.

## Safety Rules

- No force push to default branches.
- No destructive reset from automated workflow.
- No hook bypass unless explicitly requested.
- No interactive Git commands.
- No secret-file commit without explicit user authorization.
- No shell interpolation of branch names read from repository metadata.
- No public artifact containing private remotes unless redacted.
- No writeback without a delivery authority record.

## Tests

Minimum coverage:

- Git root detection for normal repos, worktrees, and submodules.
- Canonical identity for multiple worktrees.
- Malicious metadata rejection.
- Safe ref parser fixtures.
- Remote normalization fixtures.
- Diff caps for large file counts and large hunks.
- Transient Git state behavior.
- Untracked file capture limits.
- Commit workflow refusal on no changes.
- Secret-file warning.
- PR create versus update behavior.
- Review comment parsing.
- Redacted public evidence projection.

## OpenAgents Translation Notes

Checked the open OpenAgents issue list on 2026-06-11.

Related live roadmap issues:

- #4769 covers repo connect and per-mission data-scope UX.
- #4779 covers writeback symmetry through the artifact and authority layer to
  PR drafts.
- #4773 covers API parity for MVP surfaces.
- #4777 covers the first live negotiated labor job pointed at a real backlog
  issue.
- #4786 is the Autopilot MVP ladder epic.

OpenAgents appears to have explicit roadmap coverage for repository connection
and PR writeback, but not a dedicated issue for low-level Git safety,
repository identity, diff bounding, or review-thread ingestion. Those should be
called out as implementation prerequisites for #4779.

Recommended OpenAgents shape:

- Add `RepositoryIdentity`, `GitStateSnapshot`, `ChangeCapture`, and
  `DeliveryAuthority` records.
- Make PR draft writeback consume `ChangeCapture` and produce public-safe
  `DeliveryReceipt` records.
- Gate every writeback through the artifact and authority layer.
- Keep GitHub adapter details out of the mission domain model.
- Link delivered PRs back to missions, work orders, spend records, and
  evidence artifacts.

## Decision

Build this as a typed workflow subsystem. Shell commands are implementation
details; the durable product needs repository identity, delivery authority,
change capture, and PR evidence records that survive across local, hosted, and
delegated execution.
