# Terra execution lane under the Sol roadmap

- Date: 2026-07-10
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

## Why this changes throughput

The previous operating model concentrated roadmap maintenance, critical-path
integration, leaf implementation, visual feedback, and closeout in one lane.
That made Sol a serial bottleneck even when safe P1 work was ready.

The split is now explicit:

| Responsibility | Sol | Terra |
| --- | --- | --- |
| Canonical priorities and issue set | owns | consumes and reports drift |
| P0 hot-contract integration | owns unless explicitly handed off | does not take implicitly |
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

## Current Terra assignment: #8574 Desktop parity

The Desktop lane is beyond its scaffold-only state. Through `f4cb8ed18e`, it
has a hardened Electron/Effect Native host, a minimal real chat surface,
host-owned bounded thread persistence, a host-held gateway bridge with honest
failure, shared typed icons and glass material lowering, and the start of a
real local-workspace vertical slice: user-selected root, bounded root listing,
and bounded read-only file preview. It also has a dedicated Settings surface
that reads bounded Codex account readiness and starts Pylon's isolated
device-auth flow without touching default `~/.codex`; the headless smoke proves
the typed awaiting-browser state, not completion of real owner authentication.

The active Terra delivery order is:

1. Complete one real owner Codex reconnect through the Settings path when the
   owner is available, recording the account-ready receipt without exposing
   credentials. This is an owner proof gate, not a reason to idle.
2. Complete project/editor/review as one coherent local-workspace slice:
   bounded editing and save, typed Git status/diff, review UI, failure states,
   and Electron smoke.
3. Add the terminal workbench through a bounded process/session host service;
   never expose generic renderer-to-shell authority.
4. Add command palette and hotkeys over the typed intent registry so capability
   growth does not restore permanent developer chrome.
5. Connect Fleet, approvals, receipts, Forum, provider details, and diagnostics
   only to their owning authoritative services; keep chat as the quiet default.
6. Finish identity, packaging, fuse, signing, notarization, update, rollback,
   and clean-machine proof under their explicit owner gates.

Terra may make required reusable Effect Native DOM/catalog changes when the
live #8574 claim names the shared hot contract before mutation. App-local UI
semantics remain prohibited when the need belongs in the shared catalog or
renderer.

## Boundaries around the P0 burn

C1 is crossed: #8637, #8633, and #8639 are closed, fixture-proven, and the
minimum-safe supervision stack is deployed. The serial P0 is #8640 Phase A: a
clean simultaneous Codex + Claude owner-local burn through Sarah. Grok is
postponed by owner decision because the connected account is quota/payment
exhausted; its existing adapter and receipt coverage remain regression
substrate but do not block the cutover.

Until Phase A closes, Sol owns the live-burn hot paths: Sarah FleetRun
authority/projection, Pylon claim/execution/retry state, account-health and
credential scanners, deployment type boundaries, migrations, and the exact
receipt contract. Terra may work beside that burn on disjoint P1 leaves. Terra
touches a P0 hot path only through an explicit Sol handoff recorded in the live
issue claim.

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

This amendment is working when Terra continuously closes real P1 product loops
while Sol closes the Phase A/hybrid critical path, without duplicate claims,
hot-contract collisions, fabricated UI state, or a second roadmap. If Terra
spends more time waiting for restated instructions than shipping ready leaves,
the pull rules are too narrow. If Sol repeatedly has to unwind Terra landings,
the claim or hot-contract boundary is too loose.
