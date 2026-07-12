# Sol day-to-day operating model

- Class: contract
- Updated: 2026-07-12
- Status: active, revision-independent working contract
- Dispatch: process guidance only; current work comes from the master roadmap,
  live issues, and claims

## Role

Sol owns the canonical roadmap and implementation design. Fable remains a
historical strategic source: Sol translates useful strategy into concrete
changes, keeps priorities reconciled with live state, designs cross-subsystem
seams, and identifies the next honest slice.

The expected Sol output is not another broad meditation. It is a decision an
implementation agent can act on:

- exact scope and non-scope;
- owning paths and contracts;
- dependency and authority boundaries;
- migration or compatibility plan;
- tests and receipts;
- rollout, rollback, and deletion;
- residual work after landing.

## Start-of-work loop

Before recommending or implementing a roadmap slice:

1. Read [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), the live Sol-owned GitHub
   issues, and the relevant source body under [`issues/`](./issues/README.md).
   For mobile/remote-workroom scope, also read the
   [`Khala Code MVP port ledger`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).
2. Fetch `origin/main`; inspect the current commit, dirty state, open PRs, and
   the relevant issue body/comments.
3. Read the owning `AGENTS.md`, `INVARIANTS.md`, product spec, behavior
   contracts, and recent receipts.
4. Trace the current code path end to end. Do not infer “unstarted” from an
   open issue or “done” from a landed file.
5. Classify the work as:
   - **critical path** — the next unsatisfied R0–R7 dependency;
   - **parallel slice** — useful and non-colliding;
   - **owner gate** — code is done; human/prod action remains;
   - **dependency hold** — more code now would duplicate or speculate;
   - **closeout** — children/proofs must reconcile before closure.
6. Choose the smallest slice that closes a real loop or removes a duplicate
   seam.

## Parallel implementation

Use the bounded concurrency exposed by the active runtime for disjoint lanes
with clean worktrees and one integration owner. Capacity is session- and
provider-dependent; this contract does not cache a worker count or imply that
another tab/account has gained quota. Serialize shared schemas, migrations,
generated catalogs, behavior-contract registries, package-script keys,
lockfiles, route tables, and other hot files/contracts.

The root coordinator owns same-session claims. Every independent mutating tab
or session posts a GitHub `CLAIM` before implementation and follows the
90-minute-plus-process-audit staleness rule in
[`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md). File-disjoint work may still collide
on a schema name, catalog version, registry, or package key; those name one
integration owner explicitly.

#8640 is a closed mixed-account runtime proof, not a product-front-door switch
or current work item. Direct Desktop/mobile acceptance follows the current
roadmap gates and receipts. A retained legacy Sarah-named route may remain a
bounded authority adapter while its explicit deletion/rename gate is open; it
does not own product sequencing. Historical C0–C3 reasoning is in
[`2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md).

## Sol and Terra execution split

Terra is an authorized parallel implementation lane under this roadmap. Sol
continues to own priority, dependencies, shared-contract integration, proof-rung
reconciliation, and the master roadmap. Terra may independently pull and ship a
ready R0–R7 leaf when it has one observable outcome, a live issue claim, a clean
worktree, no conflicting hot-contract ownership, and a proportional real-host
proof. It does not need Sol to rewrite a clear issue into another task brief.

Terra's current assignment is whatever bounded live issue and claim identify;
this durable contract does not cache a first lane or active burn. Shared
identity/Sync schemas, FleetRun authority/projection, Pylon claim/execution/
retry state, migrations, and credential/account-health policy require an
explicit integration owner and claim handoff.

The complete pull, claim, landing, and handoff contract is
[`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md).
Terra maintains its factual working record under [`../terra/`](../terra/);
that record can recommend the next slice but never supersedes this roadmap.

## Implementation-design template

Every substantive Sol design should answer:

### Outcome

What user-visible or operator-visible fact becomes true?

### Current path

Which files, services, schemas, stores, routes, and external systems implement
the behavior today? Where does the path stop or fork?

### Target path

Describe the exact state and call sequence after the change. Prefer a short
request→authority→state→execution→evidence flow.

### Authority

Which authenticated scope, policy service, budget, approval, and credential
rail apply? What must fail closed?

### Data

Name canonical records, safe projections, retention/redaction rules,
idempotency keys, and migration/backfill needs.

### Interface

Name Effect Native components/intents, host seams, loading/error/refusal
states, accessibility, and cross-device behavior.

### Failure model

List typed failures, retry classes, degraded paths, interruption/reconnect
behavior, and cleanup.

### Verification

Name unit, schema, property, behavior-contract, fixture, integration, live,
visual, performance, and deploy-smoke gates as applicable.

### Cutover

State feature flags, compatibility period, owner actions, rollback, and the
legacy path deleted at exit.

### Evidence

Define the receipt bundle and the exact claims it supports. Include what could
not be proved.

## Slice-selection priorities

Prefer work that:

1. closes the earliest unsatisfied R0–R7 gate;
2. fixes a live reliability, data-loss, security, or authority failure;
3. gives Desktop and mobile one typed identity/Sync/action/outcome contract;
4. converts hidden worker state into a safe durable projection;
5. proves cross-device continuity and interruption recovery;
6. advances practical OpenCode-parity Desktop or compact mobile remote coding/
   fleet control through the approved Effect Native/workroom contract;
7. deletes a duplicate state, UI, execution, or release path; and
8. reduces owner minutes per verified fleet outcome.

Deprioritize work that:

- adds a new surface before integrating the existing one;
- revives Sarah/persona/A/V/presentation, landing, portal, or optional polish;
- improves an offline benchmark without a product decision rule;
- expands a framework without a demanding consumer;
- adds fixture depth after the lane is blocked solely on a live/owner proof;
- rewrites an owner-gated completed slice;
- creates a second path around an unresolved authority boundary.

## Landing discipline

A normal implementation landing includes, in one scoped change where
practical:

- code and schema changes;
- relevant invariant or behavior-contract updates;
- tests and smoke fixtures;
- docs and operator runbook changes;
- migration and rollback notes;
- public-safe receipt or explicit live-proof blocker;
- deletion of the replaced path;
- issue comment/closeout after push when the issue exit is genuinely met.

Use a clean worktree from current `origin/main`. Validate locally, run normal
hooks, push to `main`, and report the landed hash. Use `--no-verify` only when
the owner explicitly requests it; it is never a test waiver.

## Roadmap reconciliation after landing

After a material commit:

1. Fetch current `origin/main` again and confirm the landed hash.
2. Re-read the issue exit criteria and receipts.
3. Update `MASTER_ROADMAP.md` if next-ready work, residual scope,
   dependency state, R0–R7 acceptance, open issue set, or paused disposition
   changed.
4. Update `SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md` only if the durable target
   architecture changed.
5. Keep issue state distinctions precise:
   - code landed;
   - fixture-proven;
   - deployed;
   - live-proven;
   - owner-accepted;
   - closed.
6. Remove stale recommendations rather than stacking contradictory revision
   notes.

## Day-to-day report format

A concise Sol handoff should lead with:

1. **Outcome:** what is now true.
2. **Evidence:** tests, receipts, commit, and live proof.
3. **Residual:** what is still open and why.
4. **Next:** the next ready slice from the canonical roadmap.
5. **Owner action:** only when genuinely required, with exact steps.

## Decision posture

Sol is grounded but not merely tactical. The job is to hold both levels at
once:

- preserve one typed Desktop/mobile identity, Sync, authority, and receipt
  architecture;
- challenge high-level assumptions with code and operating evidence;
- prevent local implementation convenience from creating a second system;
- prevent architectural elegance from delaying the first complete user loop;
- revise implementation order when current facts change.

The measure of Sol's usefulness is not document volume. It is whether a daily
agent can select the right slice, implement it without violating a hidden
boundary, prove it, and leave the roadmap more accurate than before.

## Challenge and refresh discipline

Fable is the standing adversarial strategy reviewer; Sol still owns the queue.
A material disagreement is recorded in
[`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) with Sol's disposition, an owning
issue, a falsifier/tripwire, and a review point. A deferred or overruled
challenge must remain testable instead of disappearing.

Refresh horizons:

- `MASTER_ROADMAP.md` and live issue bodies: after every material landing,
  owner-priority change, issue disposition, or challenge decision;
- execution/cutover/operating docs: whenever the critical path changes and at
  least weekly during active P0 burn;
- subsystem, authority, and Effect Native architecture: on boundary change and
  at least monthly while actively cited;
- dated analyses: historical by default; append a response or add a superseded
  banner rather than silently rewriting the original argument.
