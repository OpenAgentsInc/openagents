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

- placement and capability facts;
- host-independent project references;
- bounded checkpoint policy and content;
- placement events;
- move and failback receipts;
- coordinator commands and state; and
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

## Desktop projection

The Desktop Sync host registers the canonical portable command mutator. The
Desktop main process and sandboxed preload expose two narrow operations:

- read the confirmed portable projection; and
- request a schema-decoded portable command.

The projection is available only when owner-scoped Sync is live. A queued
command does not create optimistic attachment authority. The renderer shows the
confirmed session reference, attachment reference, generation, target class,
health, custody, and queued command count. Invalid confirmed rows are shown as
projection issues and are not used as authority.

## Evidence

The current evidence receipt is
`apps/openagents-desktop/benchmarks/ide/2026-07-20-ide-13-portability.json`.
Candidate commit `8c30629bb9116ad0f15561b33d6194c8d5360895` produced the
receipt on macOS arm64 with Node 24.13.1.

The model p95 was 0.087 ms and its p99 was 0.092 ms. Checkpoint schema decode
p95 was 0.024 ms and its p99 was 0.056 ms. These values only measure the local
model and schema boundary. They do not measure a host move.

The focused verification passed 43 tests, the Desktop production build, the
Desktop typecheck, and the IDE authority-boundary check.

## Placement truth

The current placement evidence is not the IDE-13 acceptance matrix.

| Target class | Evidence | Current result |
| --- | --- | --- |
| owner local | real local contract process on macOS arm64 | The model and checkpoint boundary passed. No cross-host move was made. |
| owner managed | deterministic simulator | No real owner-managed host cohort ran. |
| OpenAgents managed | deterministic simulator and existing managed placement projection | SBX-09 live acceptance is absent. |
| managed provider | not run | No audited provider is admitted or claimed. |

## Open acceptance work

Issue `#9041` stays open. These items are not complete:

- real owner-managed and OpenAgents-managed move and failback cohorts;
- each admitted provider cohort;
- a real transition fault matrix for partitions, crash, event reorder, lease
  expiry, auth revoke, provider loss, and older recovery points;
- packaged move journeys on each claimed target;
- end-to-end quiesce, checkpoint, upload, redeem, attach, helper readiness,
  failback, size, CPU, memory, network, lease, and teardown percentiles; and
- independent owner and AssuranceSpec review.

Do not describe this state as a portable IDE platform or Cursor parity.
