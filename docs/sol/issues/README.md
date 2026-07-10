# Sol roadmap issue sources

Checked-in source records for the reliable Desktop/mobile fleet roadmap.
Authority and sequencing come from
[`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md); bounded implementation packets
come from the
[`reliable fleet delegation`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md).
The live GitHub issue is the operational copy and claim ledger. When a checked-
in source and live body differ, reconcile both before using the body for
dispatch.

As of 2026-07-10 there are 15 open `roadmap:sol` issue records. Older Sarah-
first/P1 labels do not override Master Revision 25.

## P0 — reliable Desktop/mobile fleet software

- [`app-epic.md`](./app-epic.md) — #8566 parent for R0–R7 Effect Native
  Desktop/mobile reliability.
- [`app-desktop.md`](./app-desktop.md) — #8574 practical OpenCode-parity
  Desktop, authoritative Sync, Fleet cockpit, and D0–D6.
- [`app-mobile.md`](./app-mobile.md) — #8597 Effect Native mobile remote coding,
  useful Khala Code MVP fold-in, Fleet control, and iOS/Android proof.
- [`fc-cloud-codex.md`](./fc-cloud-codex.md) — #8547 first real brokered Codex
  Agent Computer/workroom required by the mobile coding MVP.
- [`fc-4-hybrid-cloud.md`](./fc-4-hybrid-cloud.md) — #8636 explicit owner-local/
  managed-remote target routing through one claim/run/workroom contract.
- [`fc-epic.md`](./fc-epic.md) — #8638 existing Fleet Command substrate
  projected into both clients; not a Sarah front-door epic.
- [`fc-5-dogfood.md`](./fc-5-dogfood.md) — #8640 real Codex+Claude runtime proof
  followed by R3/R7 client acceptance.

Closed Fleet substrate retained for reference:

- [`fc-1-run-contract.md`](./fc-1-run-contract.md) — #8637 durable run
  authority, closed.
- [`fc-2-local-executor.md`](./fc-2-local-executor.md) — #8633 mixed-harness
  supervisor, closed.
- [`fc-3-supervision.md`](./fc-3-supervision.md) — #8639 durable controls,
  projection, reconnect, and receipts, closed/deployed substrate.
- [`fc-khala-inference.md`](./fc-khala-inference.md) — #8600 persona-neutral
  Khala inference, closed substrate.

The minimum remote-workroom slice of #8547/#8636 is P0 because R6/R7 now
requires mobile-originated coding. Advanced elastic placement, additional
providers, and cloud-cost optimization remain post-R7 breadth.

## Maintenance/deferred web

- [`app-web-consolidation.md`](./app-web-consolidation.md) — #8634 production/
  API/integrity maintenance only during R0–R7.
- [`app-forum.md`](./app-forum.md) — #8635 retained Forum maintenance; no
  expansion during R0–R7.

## Paused

- [`app-landing.md`](./app-landing.md) — #8595 landing/root product work.
- [`sarah-presentation.md`](./sarah-presentation.md) — #8610 Sarah/avatar/
  opener/voice/video/presentation work.
- [`role-programs-and-colleagues.md`](./role-programs-and-colleagues.md) —
  #8643 roles/named colleagues/persona expansion.
- [`glass-ui-and-sarah-mobile.md`](./glass-ui-and-sarah-mobile.md) — #8646 and
  #8650 presentation/lowering program; #8647–#8649 are closed historical
  receipts. #8650 may move only for an exact correctness/accessibility/
  platform/R0–R7 blocker.

Paused does not mean abandoned production safety. Security, privacy, data-loss,
active-cost, outage, and already-supported compatibility repairs remain
allowed. No new feature/polish scope starts without an owner reactivation.

## Privacy tripwire

- [`blueprint-correction.md`](./blueprint-correction.md) — #8642 correction,
  deletion, provenance export, and tombstone propagation. Activate after R7 or
  immediately for a real user request/privacy/data-integrity incident.

## Other source records

- [`effect-native-electron-host.md`](./effect-native-electron-host.md) —
  upstream reusable Electron host gap referenced by #8574.

Update checked-in sources, live issue bodies/labels, the master roadmap, and
this index together when scope or disposition changes materially. Historical
receipts remain historical; do not rewrite them into current proof.
