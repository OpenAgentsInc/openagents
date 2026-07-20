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
Candidate commit `233993911ab23c54c0ad68e60ef2470cedd2be9b` produced the
current real owner-local receipt on macOS arm64 with Node 24.13.1. Evidence
commit `7bcb816b2f` records the aggregate non-acceptance receipt.

The real owner-local cohort completed one move from generation 1 to generation
2 and one failback to generation 3. It also completed activation replay, stale
generation refusal, abort cleanup, encrypted artifact deletion, and final
helper cleanup. The destination started a real PTY, the signed TypeScript LSP,
and a filesystem watcher. DAP and native helpers stayed unsupported.

The cohort checkpoint was 1,377 bytes. The measured move phase values were
5.84 ms for quiesce, 52.29 ms for checkpoint, 66.73 ms for upload, 128.57 ms
for redeem, 68.35 ms for attach, and 0.04 ms for helper readiness. Failback was
565.42 ms. Teardown was 314.79 ms. These are one-sample values. They do not
ratify p50, p95, or p99 distributions.

Focused verification passed the real cohort, packaged owner-local LSP,
managed LSP, executable-profile, authority, Google Cloud, Pylon typecheck, and
Desktop evidence-contract checks. This result is regression and local cohort
evidence. It is not IDE-13 acceptance.

## Placement truth

The current placement evidence is not the IDE-13 acceptance matrix.

| Target class | Evidence | Current result |
| --- | --- | --- |
| owner local | real local production composition on macOS arm64 | A same-device target move and failback passed with real PTY, signed TypeScript LSP, watcher, encrypted custody, replay, stale-generation refusal, and cleanup. No cross-host claim is made. |
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
- a real transition fault matrix for partitions, crash, event reorder, lease
  expiry, auth revoke, provider loss, and older recovery points.
- packaged move journeys on each claimed target.
- end-to-end quiesce, checkpoint, upload, redeem, attach, helper readiness,
  failback, size, CPU, memory, network, lease, and teardown distributions on
  each claimed target.
- independent owner and AssuranceSpec review.

Do not describe this state as a portable IDE platform or Cursor parity.
