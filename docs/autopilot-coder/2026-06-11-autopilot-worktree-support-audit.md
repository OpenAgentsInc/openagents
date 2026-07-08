# Autopilot Worktree Support Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This note describes how OpenAgents should handle worktree-style execution for
Autopilot coding assignments using the system's existing contracts:
Effect Schema payloads, Pylon assignment leases, capability refs, public-safe
closeouts, and the `workspace.kind = "git_checkout"` workspace shape.

## Finding

OpenAgents should keep worktree support behind the Pylon workspace
materialization boundary. It should not become a new user-facing workspace
kind or a routing shortcut.

The public assignment contract already has the right shape:

- `openagents.autopilot_coding_assignment.v1`
- `workspace.kind = "git_checkout"`
- public GitHub repository for v1
- pinned commit SHA for execution
- caller-supplied verification command as argv
- ref-only closeout
- no deploy, spend, settlement, payout, or publication authority

The first local-adapter lane has already proven the shape in production through
#4756 and is integrated on `main`. That implementation materializes an
isolated detached checkout under the Pylon cache and verifies the result on the
device. That is enough for the B2 promise, but it is not yet a reusable,
adapter-neutral worktree manager with shared fetch cache, lease state, cleanup
receipts, and projection freshness.

The remaining work is therefore hardening and parity, not a contract rewrite.

## Current State

Live or integrated:

- The Autopilot roadmap defines a shared normalized coding-assignment payload.
- The work-order request contract accepts public repository coding work and
  rejects private repositories until grants and secret brokerage are modeled.
- The normalized assignment payload includes `workspace.kind = "git_checkout"`
  with repository, pinned commit, and verification command data.
- Pylon assignment records can carry the normalized coding assignment payload.
- Pylon closeout ingestion converts worker closeouts into public-safe
  Autopilot execution closeouts.
- #4756 is closed with a production receipt for an API-submitted public
  repository task, owner-Pylon execution, device-local verification, and
  delivered work-order readback.
- The #4756 branch work was merged to `main`; the merge is visible in the
  current history as `432372461`.

Live or integrated (added after the original query — #4792 closed later on
2026-06-11):

- `codex_agent_task` is a known Pylon job kind.
- The local Codex lane has fixture execution, dispatch support, CI-safe smoke,
  and live fixture evidence.
- Real public-repository `git_checkout` parity for the Codex lane landed with
  CX5 (#4792, closed 2026-06-11): API order
  `autopilot_work_order.c63284d5-e24a-4f4a-aeab-4be45ffd8d72` placed on a
  codex-only Pylon, executed at the pinned commit through the shared
  `git_checkout` contract, closeout
  `assignment.closeout.b6d31228033e1009fe773326` accepted with
  `git_checkout_verified_passed`. The Codex executor consumes the same
  validator and checkout runner as the Claude lane (`gitCheckoutWorkspaceFrom`
  and `defaultClaudeAgentCheckoutRunner`), so no contract fork appeared — but
  that shared logic is exported from `apps/pylon/src/claude-agent-executor.ts`
  rather than an adapter-neutral module, which is the extraction seam below.

Not live as a distinct shared subsystem:

- Adapter-neutral `WorkspaceMaterializer`.
- Native `git worktree` support backed by a shared local bare-repo cache.
- Workspace lease records with materialization state, TTL, retention policy,
  cleanup receipt refs, and projection metadata.
- Browser-visible workspace state in the Autopilot work list/detail UI.
- Private repository checkout.
- Secret-brokered source access.
- PR writeback from the Pylon execution lane.
- Settlement or payout authority from workspace execution.

## Issue Map

Queried on 2026-06-11.

| Issue | State | Relevance |
| --- | --- | --- |
| #4786 | Open | Autopilot MVP ladder epic. Workspace hardening belongs under this umbrella if filed as product work. |
| #4755 | Closed | First live local-adapter prerequisite for B2. |
| #4756 | Closed | B2 API-submitted `git_checkout` task through owner-Pylon execution and no-spend closeout. |
| #4757 | Open | Marching-orders agent. This will pressure-test repeatable public issue to work order to Pylon execution loops. |
| #4758 | Open | Work list/detail UI. This needs public-safe workspace, verification, artifact, and review projections. |
| #4751 | Open | Projection staleness epic. Workspace projections must obey this once they are visible. |
| #4752 | Closed | OpenAPI route freshness gate. Any new route surface must keep it green. |
| #4793 | Closed | Codex executor lane epic. CX1–CX5 complete; closed 2026-06-11 after the original query. |
| #4789 | Closed | Codex bounded executor gate for fixture work. |
| #4790 | Closed | `codex_agent_task` work class, dispatch, and fixture smokes. |
| #4791 | Closed | Codex live fixture run. |
| #4792 | Closed | Codex API parity with `codex_agent_task` plus the existing `git_checkout` workspace contract. Closed 2026-06-11 with a live receipt after the original query. |
| #4796 | Open | Deploy-gate tooling for projection freshness. Relevant to workspace status projections. |

At the original query time, no open issue directly scoped an adapter-neutral
native worktree manager. With #4792 now closed and both adapters consuming the
same checkout runner out of `claude-agent-executor.ts`, the condition this
audit set ("after #4792 exposes concrete duplication") is met: the shared
logic exists but lives in an adapter-named module. Follow-up issues are filed
under #4786: #4798 (adapter-neutral materializer extraction, no behavior
change) and #4799 (native shared-cache `git worktree` manager with leases and
TTL cleanup receipts, behind the service from #4798).

## Design Boundary

OpenAgents should keep the API-level workspace kind as `git_checkout`.
"Worktree" should remain an implementation strategy inside Pylon.

The contract should stay:

- provider: `github`
- visibility: `public` for v1
- repository: owner/name
- branch: informational context only
- commit SHA: required for execution
- verification command: argv plus `commandRef`

If a caller submits a branch without a commit, the server should resolve it to
a commit before dispatch or reject it for executable work. Runner
materialization should never depend on mutable branch state.

## Effect-Style Shape

Production-facing additions should follow the same OpenAgents style:

- Effect Schema for external records and projections.
- Tagged errors for typed refusal arms.
- Capability refs for admission.
- Policy refs and receipt refs for authority.
- Public-safe projections with no raw logs, source content, local paths, prompt
  text, provider payloads, secrets, payment material, or customer data.
- Side effects behind services rather than route-local shelling.
- Shared contracts in `packages/*` when multiple apps decode them.
- App-specific composition in `apps/*`.

The Pylon app can keep its local execution modules simple, but the decode
boundary for assignment payloads should remain schema-shaped and should reject
foreign or malformed work classes before any filesystem work starts.

## Recommended Architecture

### 1. Promote workspace materialization into a service

The current first-adapter implementation has enough checkout logic to satisfy
B2. The next step is to extract that behavior into one local service consumed
by all coding adapters.

Recommended local return shape:

```ts
type MaterializedWorkspace = {
  workspaceRef: string
  workingDirectory: string
  sourceRef: string
  cleanupRef: string
}
```

`workingDirectory` is local-only. It must never appear in progress events,
artifact refs, closeouts, public projections, issue comments, Forum posts, or
browser UI.

### 2. Add native worktree support behind the service

The current detached checkout is correctness-safe. Native `git worktree`
support is a performance, reuse, and cleanup improvement.

Implementation rules:

1. Keep a bare or mirror cache under Pylon-owned cache paths, keyed by a stable
   hash of the public repository full name.
2. Fetch only the requested public repository remote.
3. Verify the commit object exists before materialization.
4. Create an isolated worktree under an assignment-scoped path.
5. Use detached worktrees by default.
6. If a local branch is necessary, name it from an internal assignment hash,
   never from user text.
7. Run the adapter in the isolated directory.
8. Run verification through argv, not a shell string.
9. Emit public-safe refs only.
10. Remove or retain the worktree based on explicit retention policy.
11. Run pruning only inside the Pylon-owned cache.

Reject:

- user-supplied local paths
- credentialed remote URLs
- private repository names
- arbitrary extra remotes
- absolute verification paths
- `..` traversal
- shell-shaped verification commands
- mutable branch-only execution

### 3. Admit by capability

Pylon admission should continue to decide execution eligibility from declared
capabilities, lifecycle, heartbeat freshness, backend support, lease expiry,
and payment mode.

Useful capability refs:

- `capability.pylon.git_checkout`
- `capability.pylon.workspace_materializer.v1`
- `capability.pylon.workspace_cleanup_receipts.v1`
- `capability.pylon.local_codex`

For adapter-specific work, the required adapter capability should choose the
runner. For adapter-agnostic work, use explicit owner preference in Pylon
configuration. Do not silently substitute one adapter for another after
dispatch.

### 4. Keep authority separated

Workspace execution can create evidence. It must not create accepted work,
deployment authority, PR authority, spend authority, settlement authority,
payout authority, or Forum publication authority.

Correct closeout shape:

- `artifactRefs`
- `buildRefs`
- `closeoutRefs`
- `proofRefs`
- `resultRefs`
- `summaryRefs`
- `testRefs`
- `previewRefs` containing only stable workspace refs
- `redacted: true`
- `payoutClaimAllowed: false` for no-spend lanes
- `settlementState: "not_applicable"` for no-spend lanes

If raw diffs or patches are needed later, put them behind the artifact layer
with audience-specific projection and authority receipts. Do not put raw patch
text, file contents, runner logs, session files, or local paths into public
closeouts.

### 5. Project status, not mechanics

#4758 should show:

- order state
- assignment ref
- selected lane
- adapter ref
- workspace status ref
- verification status
- artifact and proof refs
- review actions

It should not show:

- local checkout paths
- local branch names
- raw prompts
- raw test logs
- raw diffs
- provider session IDs
- cache or worktree mechanics except in operator-only diagnostics

Every visible projection should carry freshness metadata once #4751/#4796 land.

## Tests And Smokes

Minimum unit coverage:

- decode accepts public GitHub `git_checkout` with 40-character commit SHA;
- decode rejects private repositories, unsafe repository names, unpinned
  commits, private paths, absolute verification paths, `..`, and shell-shaped
  command strings;
- materializer creates an isolated workspace under Pylon cache only;
- materializer rejects missing commit objects;
- two concurrent assignments for the same repository get separate workspace
  refs and directories;
- cleanup removes only the assignment-scoped workspace;
- verification uses argv and cannot execute shell injection;
- closeout redaction scan rejects local paths, raw logs, raw patches, provider
  payloads, secrets, and payment material;
- capability admission refuses a `git_checkout` lease when the Pylon lacks the
  workspace capability.

Minimum integrated coverage:

- API work order with public repository checkout dispatches a Pylon assignment
  that carries `workspace.kind = "git_checkout"`;
- owner Pylon materializes workspace, runs adapter, verifies, closeouts, and
  moves the work order to `delivered`;
- Codex parity repeats the same path under #4792;
- route and OpenAPI coverage stays current after any new route surface;
- projections carry freshness fields once #4751/#4796 are active gates.

Minimum live proof:

- one no-spend owner-Pylon run against a public fixture repository for each
  adapter;
- pinned commit recorded;
- verification command recorded by ref;
- delivered work order readback shows public-safe closeout;
- review remains pending until a human or authorized reviewer acts.

## Risks

Branch drift: mutable branch checkout would make receipts non-reproducible.
Execution should require a commit SHA.

Local path leakage: workspace paths must never enter progress, closeout,
artifact refs, public UI, Forum posts, or issue comments.

Command injection: verification must remain argv-only and run without a shell.

Cleanup mistakes: cleanup must operate only under Pylon-owned cache roots and
only from internal workspace refs.

Private repository creep: v1 rejects private repositories. Do not add private
source support until repository grants, secret brokerage, audience-specific
artifacts, and source-access receipts are modeled.

Adapter fork: Codex should consume the same workspace contract proven by B2.
A second contract would create avoidable policy drift.

Status staleness: workspace state must obey #4751's freshness invariant when
projected.

## Recommended Next Step

1. Done: #4792 closed on 2026-06-11 with `codex_agent_task` consuming the
   existing `git_checkout` workspace contract end to end, receipt-backed.
2. Done (#4798): the shared validator and checkout runner now live in the
   adapter-neutral `apps/pylon/src/workspace-materializer.ts`, consumed by
   both executors, with the `MaterializedWorkspace` return shape above, a
   cache-root-guarded cleanup primitive, and no behavior change (refs,
   directories, and closeouts unchanged; full Pylon suite and both CI-safe
   smokes green).
3. Done (#4799): native `git worktree` support is the default strategy
   behind the service — shared bare-repo cache keyed by hashed repository
   full name, commit-object verification before materialization, detached
   assignment-scoped worktrees, workspace lease records with TTL/retention
   and cleanup receipt refs, opportunistic expiry sweeps, the two workspace
   capability refs declared at go-online, and a public-safe lease projection
   carrying `generatedAt`. Live-proven against the B2 fixture repository:
   cold materialization ~0.7s, warm cross-adapter ~70ms. See
   `apps/pylon/docs/workspace-materializer.md`.

The desired end state is one OpenAgents workspace contract, one Pylon
materializer, multiple adapters, and ref-only evidence across every surface.
