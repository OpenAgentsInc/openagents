# P0 PROGRAM: reliable Effect Native Desktop/mobile coding and fleet software

- Issue: #8566
- Status: sole active program parent under Master Revision 27
- Authority: [`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md)
- Execution:
  [`../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)

## Owner direction

Desktop and mobile are the active product clients. OpenAgents web remains a
supported public/API/operations surface, but web expansion does not preempt the
reliability program. The Sarah product surface is removed. Any future assistant
must consume the same typed actions without becoming a new authority or front
door.

Both clients use one Effect Native application/component/intent grammar and
one Khala Sync identity/state/outcome reality:

```text
OpenAgents mobile <---- Khala Sync ----> OpenAgents Desktop
remote coding + fleet                    full workbench + fleet
```

## Program tracks and bounded work

- #8574 — Desktop track: D0–D6, practical OpenCode parity, Sync, Fleet,
  security, lifecycle, and distribution.
- #8597 — mobile track: R0/R1/R2/R3/R6, full useful Khala Code MVP
  capability fold-in, remote coding, fleet controls, receipts, and iOS/Android.
- #8547/#8636 — bounded P0 remote-workroom execution and target-routing tasks
  required by the
  mobile coding exit; advanced elastic/provider breadth remains follow-on.
- closed #8638 — existing Fleet substrate consumed by the clients; it is not
  another epic.
- #8640 — bounded real mixed-account runtime/client proof.

The retired web/presentation/privacy-backlog records #8595, #8610, #8634,
#8635, #8642, #8643, #8646, and #8650 are closed `wontfix` / not-planned.
Production defects and real privacy requests get a new bounded incident or task;
these backlog tombstones are not dormant epics.

## Shared implementation laws

- Effect Native owns typed components, state, intents, and command IDs;
  Electron/React Native/DOM/native/canvas are hosts or renderers.
- Khala Sync owns canonical cross-device projections and durable mutation
  outcomes. Device-local state is cache/offline queue, never a second authority.
- Pylon/Worker/Cloud/data/payment services retain their own typed authority.
- Desktop and mobile may expose different capabilities but share identifiers,
  versions, outcome grammar, privacy classes, and receipt refs.
- Every mutation carries owner scope, target ref, idempotency, version/
  commutative semantics, and durable accepted/rejected/failed/unknown outcome.
- Existing auth, promise, receipt, payment, release, and security contracts stay
  green through migration.
- Deprecated mobile and Electrobun clients are extraction sources, not
  conversion targets or parallel shipping products.
- New shared component/host demand goes upstream; app-local state/UI
  architectures do not regrow.

## R0–R7 exit

1. Desktop and mobile are truthfully green from clean state.
2. Both use one authenticated owner/org/device/session contract.
3. Conversation/project/Fleet/work/attempt/approval/command/outcome/receipt
   state converges through Khala Sync across restart and handoff.
4. One real Codex+Claude FleetRun can be inspected and controlled from both
   clients with zero duplicate claims or false outcomes.
5. Offline, lost-ack, out-of-order, stale generation, refetch, restart,
   migration, and rollback behavior fails closed and converges.
6. Desktop completes D0–D6 practical OpenCode parity; mobile completes R6
   compact remote coding plus fleet control through typed workroom capabilities
   without unsafe local filesystem/process/credential authority.
7. Signed/installable iOS, Android, and Desktop releases survive sustained
   owner dogfood with exact receipts.
8. Legacy product/install/release/update paths are deleted or unable to ship
   only after successor and migration proof.

#8566 closes last, after child issues, R7, issue/docs reconciliation, and legacy
retirement agree. Sarah/persona/A/V/presentation work is not an exit criterion.
