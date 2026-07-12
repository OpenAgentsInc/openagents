# Terra execution lane under the Sol roadmap

- Date: 2026-07-10
- Updated: 2026-07-10 (Revision 25 mobile remote-workroom fold-in)
- Status: active operating amendment
- Authority: owner direction, [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md),
  [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md), and repository invariants
- Working record: [`../terra/README.md`](../terra/README.md)

## Decision

Terra is an authorized parallel implementation lane under the Sol roadmap.
Sol still owns program priority, dependency order, cross-lane integration, and
roadmap reconciliation. Terra owns small, user-observable vertical slices that
can be selected, implemented, verified in the real host, and pushed without
waiting for Sol to perform the implementation itself.

Terra is not a second roadmap and is not permanently a Desktop team. Its first
active home is #8574 because OpenAgents Desktop has a deep, ready parity queue
with bounded host seams and little overlap with the live Phase A fleet burn.
Sol may route Terra to another ready leaf when that creates more useful motion.
The new #8597 M0–M7 queue is defined in the
[`mobile port ledger`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).
Terra may take a disjoint M0–M2/M4–M6 leaf after claim release and contract
freeze; it does not independently design #8547/#8636 isolation, grants,
workroom lifecycle, target fallback, ports, or writeback authority.

## Why this changes throughput

The previous operating model concentrated roadmap maintenance, critical-path
integration, leaf implementation, visual feedback, and closeout in one lane.
That made Sol a serial bottleneck even when safe R0–R7 work was ready.

The split is now explicit:

| Responsibility | Sol | Terra |
| --- | --- | --- |
| Canonical priorities and issue set | owns | consumes and reports drift |
| Shared identity/Sync/Fleet hot-contract integration | owns unless explicitly handed off | does not take implicitly |
| Ready low-collision leaf implementation | may implement | may claim and ship directly |
| Real-host feedback and small corrective passes | reviews when material | owns for its active slice |
| Shared schemas, migrations, generated catalogs, lockfiles, route tables | integration owner | changes only under an explicit hot-contract claim |
| Roadmap and proof-rung reconciliation | owns | supplies the landing receipt |
| Working notes and next-action record | may consult | owns `docs/terra/` |

This is concurrency with one program authority, not two competing backlogs.

## Terra-ready work

Terra may pull a leaf without another roadmap revision when all of these are
true:

1. The work belongs to an open Sol roadmap issue or the owner explicitly
   directs it.
2. It changes one observable outcome and has a falsifiable verification path.
3. The live issue has no conflicting claim, or the claim is explicitly
   released, re-scoped, or handed to Terra.
4. The slice does not independently redefine a shared authority, schema,
   migration, catalog version, behavior registry, package-script key, lockfile,
   route table, product promise, or release identity.
5. The work can land from a clean current-main worktree with a short receipt:
   outcome, commit, verification, limitations, and next move.

Terra does not need Sol to restate an already-clear issue body before starting
such a leaf. Discovery that materially changes scope, authority, sequencing, or
a hot contract returns to Sol for integration ownership.

## Assignment authority

Terra's current assignment and delivery order come from the master roadmap,
live issue, and active claim. This durable lane contract does not cache them.
Factual working notes may live in
[`../terra/CURRENT_STATE.md`](../terra/CURRENT_STATE.md), but they do not
override those authorities.

Terra may make required reusable Effect Native DOM/catalog changes when the
live claim names the shared hot contract before mutation. App-local UI
semantics remain prohibited when the need belongs in the shared catalog or
renderer.

## Boundaries around the runtime proof and client program

Closed #8640 preserves the accepted simultaneous Codex + Claude owner-local
runtime proof through a landed compatibility adapter. It does not make Sarah
the product front door, select current client work, or reopen its closed
substrate issues. Provider readiness is refreshed from current account/runtime
evidence rather than cached here.

Sol owns shared identity/Sync schemas, FleetRun authority/projection, Pylon
claim/execution/retry state, account-health and credential scanners, deployment
type boundaries, migrations, and the exact receipt contract. Terra may work on
disjoint R0–R7 leaves. It touches a shared hot path only through an explicit
Sol handoff recorded in the live issue claim.

## Claim and landing protocol

For every mutating slice, Terra:

1. fetches current `origin/main` and uses a clean worktree;
2. posts or updates the live issue `CLAIM`, including hot files and hot
   contracts;
3. implements the smallest complete vertical slice and removes replaced
   residue;
4. runs focused tests plus the real host/smoke/visual proof proportional to the
   change;
5. pushes the scoped commit to `main` and posts `CLAIM-RELEASE`;
6. updates `docs/terra/` when the working model, parity ledger, or next action
   changed; and
7. sends Sol the landing receipt when proof rung, residual scope, dependency
   state, or next-ready order changed.

Sol reconciles the master roadmap after a material Terra landing. Terra does
not wait for that prose update to begin the next already-ready, non-colliding
leaf.

## Success test

This amendment is working when Terra continuously closes real R0–R7 product
loops while Sol integrates shared identity/Sync/Fleet contracts and live proof,
without duplicate claims, hot-contract collisions, fabricated UI state, or a
second roadmap. If Terra
spends more time waiting for restated instructions than shipping ready leaves,
the pull rules are too narrow. If Sol repeatedly has to unwind Terra landings,
the claim or hot-contract boundary is too loose.
