# IDE-13 portable capability foundation

Date: 2026-07-20
Issue: `#9041`
Status: implemented foundation, acceptance is not complete

## Scope

This change adds the first Desktop consumer for confirmed portable session
authority. It also adds one bounded attachment model and one main-owned Effect
coordinator. This change does not claim the portable IDE exit.

The existing portable session service remains the durable authority. The new
Desktop code does not create a second session identity or a second durable move
ledger.

## Contract and authority

`packages/portable-session-contract/src/ide13-contract.ts` defines identified
Effect Schemas for these facts:

- placement and capability facts.
- host-independent project references.
- bounded checkpoint policy and content.
- placement events.
- move and failback receipts.
- coordinator commands and state.
- tagged failures.

The checkpoint contract excludes secrets, process state, native state, Vim
widget state, and theme widget state. Vim and theme values are destination
settings. The renderer contract contains opaque references. It does not contain
an absolute root, a credential, an endpoint, or a native handle.

`apps/openagents-desktop/src/ide/portable-coordinator-service.ts` supplies a
`Context.Service` with a `Layer.effect` implementation. Named `Effect.fn`
operations serialize move commands. The service checks the exact session,
project, attachment, and generation before work starts. It checks the exact
attachment and generation again before an IDE mutation.

The move order is quiesce, checkpoint, source validation, destination stage,
destination validation, source revoke, destination attach, and fresh helper
start. A completed move increases the attachment generation. A stale
generation cannot mutate. A replay returns the prior receipt. A cancel before
source revoke removes the staged destination and resumes the source. A failure
after source revoke is visible as degraded state.

## Model result

The bounded model uses a maximum depth of 12. It explored 14 states and 252
transitions. It tested 80 stale-generation write attempts. It found no
counterexample in the admitted model.

The negative control attaches the destination before source revoke. The checker
returns a trace for the `source_revoked_before_attach` invariant.

Deterministic fault tests fail each coordinator adapter step in turn. A failure
before source revoke keeps generation 1 attached to the source. A failure after
source revoke keeps generation 1 visible in degraded state and does not attach
a second writer.

## Desktop projection

The Desktop Sync host registers the canonical portable command mutator. The
Desktop main process and sandboxed preload expose two narrow operations:

- read the confirmed portable projection.
- request a schema-decoded portable command.

The projection is available only when owner-scoped Sync is live. A queued
command does not create optimistic attachment authority. The renderer shows the
confirmed session reference, attachment reference, generation, target class,
health, custody, and queued command count. Invalid confirmed rows are shown as
projection issues and are not used as authority.

## Evidence

The current evidence receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability.json`.
Candidate commit `7c9cd53cc28d76d6cda8f6b37fc2622f02c17cb6` produced the
current real owner-local receipt on macOS arm64 with Node 24.13.1. Evidence
commit `a14004645a` records the aggregate non-acceptance receipt.

The real owner-local cohort completed one move from generation 1 to generation
2 and one failback to generation 3. It also completed activation replay, stale
generation refusal, abort cleanup, encrypted artifact deletion, and final
helper cleanup. The destination started a real PTY, the signed TypeScript LSP,
and a filesystem watcher. DAP and native helpers stayed unsupported.

The performance cohort completed 10 full runs. The p95 values were 0.47 ms for
quiesce, 43.97 ms for checkpoint, 51.27 ms for upload, 93.20 ms for redeem,
57.48 ms for attach, and 0.02 ms for helper readiness. Failback p95 was 537.58
ms. Teardown p95 was 305.11 ms. All 16 phase and resource metric rows passed.
The checked-in receipt records the raw values and p50, p95, and p99 values.

The fault inventory contains all 27 required rows. A source-controlled probe
injected a transient partition at each of the eight production phase dispatch
boundaries. All eight real local runs passed and left no recorded residue. A
real local checkpoint-store crash also passed. Nine rows passed only with
production components and simulator fixtures. Nine rows did not run. These
limits keep acceptance false.

A separate real local proof moved one refs-only accepted work item. A bounded
registered handler ran one time at generation 2. Replay did not run it again,
and generation 3 refused the stale request. No live process state moved.

The packaged macOS arm64 application stayed live during a same-host move and
failback. It used isolated signed-out local proof. It did not authenticate
Sync or initiate the move. The package and owner-local target left no recorded
process or temporary-root residue.

Focused verification passed 29 tests in eight files, both typechecks, the
package and ASAR gates, and the real receipt generators. This result is
regression, simulator, packaged fail-closed, and real local evidence. It is
not IDE-13 acceptance.

## Placement truth

The current placement evidence is not the IDE-13 acceptance matrix.

| Target class | Evidence | Current result |
| --- | --- | --- |
| owner local | real local production composition on macOS arm64 | Ten same-device target moves and failbacks passed with real PTY, signed TypeScript LSP, watcher, encrypted custody, replay, stale-generation refusal, cleanup, and complete phase and resource distributions. One bounded accepted work item also resumed and settled. A signed-out packaged shell stayed live during one separate journey. No cross-host or package-initiated claim is made. |
| owner managed | deterministic simulator | No real owner-managed host cohort ran. |
| OpenAgents managed | deterministic simulator and source-wired signed TypeScript LSP | The Linux x64 root filesystem was not rebuilt. No live Firecracker cohort ran. |
| managed provider | not run | No audited provider is admitted or claimed. |

## Open acceptance work

Issue `#9041` stays open. These items are not complete:

- real owner-managed and OpenAgents-managed move and failback cohorts.
- each admitted provider cohort.
- signed DAP and native executable profiles for each target that claims these
  capabilities.
- the owner-managed enrollment and checkpoint-key custody decision in
  `NEEDS_OWNER.md`.
- real local evidence for nine simulator-only fault rows and the nine fault
  rows that did not run.
- package-initiated authenticated move journeys on each claimed target.
- arbitrary provider executor resume and crash recovery after handler
  completion but before durable settlement.
- end-to-end quiesce, checkpoint, upload, redeem, attach, helper readiness,
  failback, size, CPU, memory, network, lease, and teardown distributions on
  each other claimed target.
- independent owner and AssuranceSpec review.

Do not describe this state as a portable IDE platform or Cursor parity.
