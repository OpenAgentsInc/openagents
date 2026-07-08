# Open Issue Delegation Plan

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

## Purpose

This document splits the currently outstanding Autopilot and Pack A issues
into parallel workstreams for delegated agents.

It initially covered the open issue set checked with `gh issue list` on
2026-06-11: #4749, #4766-#4783, #4785-#4786, and #4813-#4823. The later Pack B
readiness pass added #4824-#4830 for account, credential, and policy
hardening.

Current status as of the 2026-06-12 follow-up review: Pack A (#4813-#4823),
Pack B (#4824-#4830), Pack C (#4831-#4835), and public hygiene #4836/#4837
are closed. Do not claim those issues from this historical plan. The remaining
open tail is #4768, #4772, #4777, #4781, #4782, #4783, #4786, and W3 #4749.
Those issues are now live-evidence or training/evaluation gates rather than
new implementation-pack decomposition.

The goal is to let multiple agents make progress concurrently without making
each issue carry a full product end-to-end verification burden. Individual
agents should run focused tests, contract checks, and local smokes for their
slice. The final cross-lane proof is assigned to Agent Gate after the relevant
implementation lanes have merged.

## Operating Rules For Every Agent

Each delegated agent owns issue status as well as code.

1. Work from a clean, branch-backed worktree.

   ```sh
   git fetch origin
   git worktree add -b agent/<agent-slug> ../openagents-<agent-slug> origin/main
   cd ../openagents-<agent-slug>
   ```

   If the branch name already exists, add a date or short suffix instead of
   reusing a dirty branch.

2. Before editing, read the relevant repository instructions:

   - `AGENTS.md`
   - `INVARIANTS.md`
   - Any nested `AGENTS.md` or `INVARIANTS.md` under the files being changed.

3. Comment on every assigned issue when claiming it. The first comment should
   name the agent, worktree path, intended scope, planned checks, and expected
   merge wave.

4. Keep status comments current. Comment when a material slice lands, when a
   blocker appears, when the branch is ready to merge, and when the issue is
   closed.

5. Keep each worktree scoped to the assigned issues. Do not mix unrelated
   refactors, unrelated docs, or broad formatting churn into a lane branch.

6. Rebase before asking for or performing a merge:

   ```sh
   git fetch origin
   git rebase origin/main
   ```

7. After the rebase, rerun the focused checks listed in the issue comment and
   in the acceptance section of the issue body.

8. Merge to `main` only after the branch is rebased and the scoped checks pass.
   Prefer a fast-forward merge when coordinating locally:

   ```sh
   git checkout main
   git pull --ff-only origin main
   git merge --ff-only agent/<agent-slug>
   git push origin main
   ```

   If the repo is using PRs for the lane, merge the rebased PR and confirm the
   resulting main commit instead.

9. Close assigned child issues only after the relevant commits are on `main`
   and the issue has a final comment with the main commit, checks run, and any
   deferred verification clearly named.

10. Do not close parent or gate issues from a lane branch. Agent Gate owns the
    final closeout for #4768, #4772, #4786, #4813, and the deferred full E2E
    verification record.

## Status Comment Templates

Claim comment:

```md
Agent <Name> starting this issue.

Worktree: <path>
Branch: agent/<slug>
Scope: <short implementation scope>
Planned checks: <focused tests/smokes/contracts>
Expected merge wave: <Wave N>
Deferred verification: <none, or Gate-owned E2E item>
```

Progress comment:

```md
Agent <Name> status update.

Done: <landed or nearly landed slice>
Current checks: <passing/failing/not yet run>
Blockers/coordination: <issue refs or none>
Next: <next concrete step>
```

Merge-ready comment:

```md
Agent <Name> merge-ready.

Rebased on: origin/main @ <sha>
Checks: <commands and result>
Docs/API manifest updates: <refs or n/a>
Deferred to Gate: <specific proof, if any>
```

Close comment:

```md
Merged to main in <sha>.

Checks: <commands and result>
Deferred verification, if any: <Gate issue or none>

Closing.
```

## Delegated Agents

### Agent Chronos: Runtime Supervision

Primary issues:

| Issue     | Responsibility                                                |
| --------- | ------------------------------------------------------------- |
| #4814 PA1 | Task supervisor for scheduled and background Autopilot runs   |
| #4815 PA2 | Schedule and continuation receipts for unattended work        |
| #4820 PA7 | Structured event log replay discipline for Pack A projections |

Secondary coordination:

- #4768 M10, because overnight proof needs the task, schedule, and event
  receipts before Agent Gate can make a durable claim.
- #4773 A1, because agent API status should project the same event state as
  browser and Pylon status.

Expected output:

- Durable task/schedule/event records or their narrow missing pieces.
- Focused tests around no-double-fire behavior, continuation/skip receipts,
  replay projection shape, and public-safe `generatedAt`/staleness metadata
  where a new projection is added.
- Issue comments that say exactly which full E2E checks are deferred to Agent
  Gate.

Merge wave: Wave 1.

### Agent Beacon: Attention, Approval, And Companion Surfaces

Primary issues:

| Issue      | Responsibility                                                   |
| ---------- | ---------------------------------------------------------------- |
| #4816 PA3  | Notification and attention coordinator                           |
| #4817 PA4  | Mobile and web companion projection                              |
| #4822 PA9  | Permission and approval contract for headless/background actions |
| #4823 PA10 | Accessibility and non-interactive contract                       |

Secondary coordination:

- #4768 M10, because unattended runs must not hide prompts or completions.
- #4773 A1, because API parity must expose typed waiting/denial/approval
  states, not browser-only prompts.
- Agent Chronos on task state names and event refs.

Expected output:

- Typed attention, waiting, approval, denial, completion, and failure states.
- Companion-safe projections for decision and run status where in scope.
- Headless/CI behavior that fails with typed blockers instead of hanging.
- Accessibility/non-interactive behavior for any new controls or API output.

Merge wave: Wave 1.

### Agent Scope: Provider Account And Routing Readiness

Primary issues:

| Issue     | Responsibility                                          |
| --------- | ------------------------------------------------------- |
| #4766 M8  | Account-pool dashboard                                  |
| #4767 M9  | Rate-limit rotation proof, including live leg readiness |
| #4771 M13 | Provider peers and ToS-compliance review                |

Secondary coordination:

- #4821 PA8, because rate-limit and account-pool status must be joined to
  usage/cost-stop projections.
- #4772 M14, because the MVP exit gate needs a truthful rate-limit claim.

Expected output:

- Account lease/load/cooldown/reset/reconnect status that does not expose raw
  credentials.
- Provider-peer review notes before adding or broadening provider flows.
- A split M9 proof status: deterministic CI-safe leg versus live two-account
  leg. The live proof can be deferred to Agent Gate if the required accounts
  or environment are not ready in this lane.

Merge wave: Wave 1 for dashboard/review foundations, Wave 2 for proof
readiness.

### Agent Ledger: Budgets, Receipts, Payments, And Settlement

Primary issues:

| Issue     | Responsibility                             |
| --------- | ------------------------------------------ |
| #4770 M12 | Team budgets and spend-to-evidence joins   |
| #4774 A2  | Agent payment in both currencies           |
| #4780 P4  | Settlement bridge from USD credits to sats |
| #4785 P9  | Settlement visibility law                  |
| #4819 PA6 | Artifact and receipt ledger                |
| #4821 PA8 | Usage budget and cost-stop projections     |

Secondary coordination:

- #4782 and #4783, because spare-capacity provider mode and Lane C fanout must
  not claim paid market readiness without settlement and visibility records.
- #4768 and #4772, because MVP proof needs resolvable artifacts and budget
  stops.

Expected output:

- Receipt refs for tasks, schedules, reviews, smokes, artifacts, payment
  events, budget stops, and settlement visibility where those records are in
  scope.
- Team/per-mission budget joins that can drill into mission, ledger, and
  artifact evidence without leaking secrets or private repo data.
- Public settlement visibility checks before any live labor-market payout
  claim.

Merge wave: Wave 1 for Pack A receipt/budget foundations, Wave 2-3 for
payments and settlement.

### Agent Aperture: Repo Scope, Mission Records, And Writeback

Primary issues:

| Issue     | Responsibility                                                   |
| --------- | ---------------------------------------------------------------- |
| #4769 M11 | Repo connect, per-mission data-scope UX, placement explanations  |
| #4778 P2  | Mission/work-order record unification                            |
| #4779 P3  | Writeback symmetry through artifact/authority layer to PR drafts |

Secondary coordination:

- Agent Ledger on artifact and receipt refs.
- Agent Bridge on API parity for work-order creation and status.
- Agent Gate on what counts as acceptable proof for PR-draft claims.

Expected output:

- Per-mission repo/data-scope declarations and placement explanations.
- Shared mission/work-order record semantics across front doors.
- PR draft/writeback behavior routed through the artifact and authority layer,
  with focused tests on scope, refs, and denial behavior.

Merge wave: Wave 2, with writeback finalization in Wave 3 if it depends on
Ledger receipts.

### Agent Bridge: Agent API, Forum Intake, And Autonomic Work Creation

Primary issues:

| Issue    | Responsibility                                       |
| -------- | ---------------------------------------------------- |
| #4773 A1 | API parity contract                                  |
| #4775 A3 | Forum interaction spawns a real Autopilot work order |
| #4776 A4 | Autonomics spawn coding threads                      |

Secondary coordination:

- Agent Chronos on runtime/event status.
- Agent Beacon on approval and headless blockers.
- Agent Aperture on mission/work-order records.

Expected output:

- API parity matrix updates so no MVP capability remains browser-only.
- Forum-to-work-order intake path with scoped admission, budget, review, and
  status records.
- Autonomic coding-thread proposal flow that remains proposal/approval-gated
  where policy requires it.
- OpenAPI and capability manifest updates for every changed route or route
  shape.

Merge wave: Wave 1 for A1 matrix scaffolding, Wave 2 for intake/autonomics,
Wave 3 for final parity rows after dependent lanes merge.

### Agent Market: Labor-Market Activation

Primary issues:

| Issue    | Responsibility                     |
| -------- | ---------------------------------- |
| #4777 P1 | First live negotiated labor job    |
| #4781 P5 | Backlog faucet for the open market |
| #4782 P6 | Spare-capacity provider mode       |
| #4783 P7 | Lane C fanout                      |

Secondary coordination:

- Agent Ledger on settlement, payout eligibility, and visibility.
- Agent Aperture on issue/work-order/writeback records.
- Agent Scope on provider availability and routing constraints.

Expected output:

- Default-off or explicitly opt-in labor-market paths until settlement and
  visibility gates are ready.
- First negotiated job path tied to a real backlog issue, with quote,
  acceptance, artifact, and settlement refs.
- Backlog faucet and Lane C fanout controls that clearly state capacity,
  public-tier, and policy limits.

Merge wave: Wave 2 for default-off/spec or dry-run slices, Wave 3 for live
market slices after Ledger and Scope gates are satisfied.

### Agent Gate: Proof, Closeout, And Evaluation

Primary issues:

| Issue     | Responsibility                                                |
| --------- | ------------------------------------------------------------- |
| #4818 PA5 | Smoke receipt authority for Pack A MVP proofs                 |
| #4768 M10 | Overnight unattended proof across lanes and surfaces          |
| #4772 M14 | MVP exit review and door-open decision record                 |
| #4786     | Parent epic closeout tracking                                 |
| #4813     | Pack A parent closeout tracking                               |
| #4749 W3  | Student-program evaluation issue, kept separate from MVP gate |

Secondary coordination:

- Every other agent, because Gate owns the final proof record but should not
  implement every dependency.

Expected output:

- Smoke receipt authority and proof-boundary language that other lanes can
  cite.
- Final M10 proof after required Pack A children are merged.
- Final M14 decision record that names exact dates, issue refs, commits,
  smokes, deferred items, and remaining claim limits.
- Parent closeout comments for #4786 and #4813 after child issues are closed
  or intentionally deferred.
- #4749 kept honest as a separate research/evaluation issue: comment current
  blocked status if W2 verified-token input is still missing, or run only the
  allowed preparation steps without implying MVP readiness.

Merge wave: Wave 1 for smoke authority scaffolding, Wave 4 for full E2E and
parent closeout.

## Pack B Addendum: Provider, Account, And Policy Hardening

Pack B was filed after the initial delegation plan as #4824-#4830. These
issues should not reopen closed M8/M9 work. They should gate broad
provider-peer claims in #4771 and any later work that depends on raw
credentials, account telemetry, managed policy state, retention guarantees, or
provider security review.

Primary ownership:

| Issue        | Owner        | Responsibility                                                                       |
| ------------ | ------------ | ------------------------------------------------------------------------------------ |
| #4824 PACK B | Agent Scope  | Parent tracking for Pack B readiness, child status, and #4771 timing                 |
| #4825 PB1    | Agent Scope  | Authentication and credential storage boundary for provider accounts                 |
| #4827 PB3    | Agent Scope  | Security review gate for provider peers and account leases                           |
| #4828 PB4    | Agent Scope  | Telemetry/privacy fixtures for account health and provider routing                   |
| #4826 PB2    | Agent Ledger | Resolved settings/configuration snapshots for provider, budget, and policy decisions |
| #4829 PB5    | Agent Ledger | Retention/deletion rules for credential, lease, telemetry, and policy records        |
| #4830 PB6    | Agent Ledger | Minimal managed policy snapshots for team and approved-user gates                    |

Agent Gate should cite Pack B only if #4768 or #4772 proof evidence relies on
provider credentials, account telemetry, or managed policy state. Otherwise the
Gate lane should keep Pack B parallel to the proof closeout.

## Merge Waves

### Wave 0: Claims And Conflict Map

All agents:

- Create clean worktrees from `origin/main`.
- Comment claim templates on assigned issues.
- Identify likely shared files before editing.
- Avoid starting full E2E claims.

Likely shared files and surfaces:

- `apps/openagents.com/workers/api/src/openagents-openapi.ts`
- `apps/openagents.com/workers/api/src/openagents-capability-manifest.ts`
- App-specific `INVARIANTS.md` files if a policy or projection law changes.
- Autopilot status, artifact, ledger, account, and work-order models.
- `docs/autopilot-coder/implementation-log.md`
- `docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`

### Wave 1: Pack A Foundations

Parallel owners:

- Agent Chronos: #4814, #4815, #4820.
- Agent Beacon: #4816, #4817, #4822, #4823.
- Agent Ledger: #4819, #4821.
- Agent Gate: #4818 scaffolding.
- Agent Scope: #4766 first slice and #4771 review notes.
- Agent Bridge: #4773 matrix scaffold.

Expected verification:

- Focused unit/contract tests.
- Projection metadata checks where public or agent-readable status changed.
- No full overnight or live paid E2E requirement in this wave.

### Wave 2: Product And Record Integration

Parallel owners:

- Agent Scope: #4767 proof readiness and remaining #4766/#4771 work.
- Agent Ledger: #4770 and #4774.
- Agent Aperture: #4769, #4778, early #4779.
- Agent Bridge: #4775, #4776, A1 parity rows tied to merged Wave 1 surfaces.
- Agent Market: #4777 and #4781 dry-run/default-off slices.

Expected verification:

- API and capability manifest checks for changed routes.
- Focused route, worker, model, and UI tests.
- Issue comments explicitly naming any Gate-owned proof still pending.

### Wave 3: Market, Settlement, And Writeback

Parallel owners:

- Agent Ledger: #4780 and #4785.
- Agent Market: #4782 and #4783 after settlement/visibility gates are ready.
- Agent Aperture: #4779 final writeback symmetry.
- Agent Bridge: final A1 rows after all product surfaces expose API peers.

Expected verification:

- Settlement visibility checks before live labor-market payout claims.
- Writeback scope/authority checks before PR-draft claims.
- Default-off or opt-in guard checks for market fanout and provider mode.

### Wave 4: Gate And Parent Closeout

Owner:

- Agent Gate.

Expected verification:

- #4767 live leg if Agent Scope did not already complete it.
- #4768 overnight unattended proof across required lanes and surfaces.
- #4772 MVP exit review/door-open decision record.
- #4786 and #4813 parent issue comments and closeout if acceptance is met.
- Explicit non-close notes for any child issue or post-MVP lane that remains
  intentionally open.

## Deferred E2E Policy

It is acceptable for individual lane issues to defer full E2E verification to
Agent Gate when the lane has merged focused evidence and clearly comments what
is deferred.

It is not acceptable to close a proof issue by saying "will be tested later"
without a concrete Gate-owned issue, proof boundary, and required evidence.

Use this split:

| Issue class                 | Lane agents must provide                                   | Gate must provide                                                        |
| --------------------------- | ---------------------------------------------------------- | ------------------------------------------------------------------------ |
| Pack A child implementation | Focused tests, contracts, smokes, docs, issue comments     | Cross-lane proof only when needed by M10/M14/A1                          |
| Product feature rungs       | Scoped acceptance checks and route/UI/model tests          | Only if the feature is part of the MVP proof claim                       |
| M9 live routing             | Deterministic leg and readiness evidence                   | Live two-account proof if not already completed                          |
| M10 overnight proof         | Dependencies and receipts                                  | Final overnight run and proof record                                     |
| M14 exit gate               | Evidence refs from all lanes                               | Final decision record and parent issue closeout                          |
| P-rung market work          | Dry-run/default-off proof where live settlement is blocked | No market go-live claim until settlement and visibility gates are merged |
| W3 evaluation               | Prep/design only while the W2 corpus is unavailable        | Not part of MVP gate; keep blocked or separately verified                |

## Dependency Rules

- #4768 should not close before #4814, #4815, #4816, #4818, #4819, #4820,
  #4821, #4822, and #4823 have either closed or have explicit non-blocking
  carve-outs in the M10 proof record.
- #4772 should not close before #4767 live-proof status, #4768, #4773 parity
  proof, and the relevant #4813 child issues are resolved or explicitly
  deferred.
- #4773 should not close while a shipped MVP browser capability lacks an
  agent-API peer or a documented exception.
- #4782 and #4783 should not make live paid labor-market claims before #4780
  and #4785 are merged.
- #4779 should not claim writeback symmetry until PR-draft creation, artifact
  refs, authority checks, and issue status behavior are all covered.
- #4749 should stay separate from the Autopilot MVP proof. It can be prepared
  in parallel, but its acceptance depends on W2 verified-token input and its
  own evaluation artifacts.

## Final Closeout Order

Recommended closeout order after merged work reaches `main`:

1. Close Pack A child issues that have merged scoped acceptance evidence.
2. Close product rung issues whose acceptance is complete and whose issue
   comments name any Gate-owned proof still pending.
3. Run Agent Gate proof work for #4767 live leg if needed, then #4768.
4. Close #4773 only after the final API parity matrix is true or exceptions
   are documented.
5. Close #4772 with the MVP exit decision record.
6. Close #4813 if all Pack A children are closed or explicitly deferred with
   non-MVP carve-outs.
7. Close #4786 only when its remaining child rungs are closed or the parent
   body/comment thread records the exact open post-MVP tail.
