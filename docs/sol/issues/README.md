# Sol roadmap issue sources

Checked-in source records for the reliable Desktop/mobile fleet roadmap.
Authority and sequencing come from
[`../MASTER_ROADMAP.md`](../MASTER_ROADMAP.md); bounded implementation packets
come from the
[`reliable fleet delegation`](../2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md).
The live GitHub issue is the operational copy and claim ledger. When a checked-
in source and live body differ, reconcile both before using the body for
dispatch.

As of the 2026-07-11 cutover decomposition there are 36 open `roadmap:sol`
records: the original eight program/track/proof records, reopened topology
parent #8678, and ordered local-coding leaves #8681–#8707. Older Sarah-first/P1
labels and Revision 29 issue prose do not override Master Revision 31.

## P0 — reliable Desktop/mobile fleet software

- [`app-program.md`](./app-program.md) — #8566 sole program parent for R0–R7 Effect Native
  Desktop/mobile reliability.
- [`app-desktop.md`](./app-desktop.md) — #8574 Desktop implementation track: practical OpenCode-parity
  Desktop, authoritative Sync, Fleet cockpit, and D0–D6.
- [`app-mobile.md`](./app-mobile.md) — #8597 mobile implementation track: Effect Native remote coding,
  useful Khala Code MVP fold-in, Fleet control, and iOS/Android proof.
- [`fc-cloud-codex.md`](./fc-cloud-codex.md) — #8547 first real brokered Codex
  Agent Computer/workroom required by the mobile coding MVP.
- [`fc-4-hybrid-cloud.md`](./fc-4-hybrid-cloud.md) — #8636 explicit owner-local/
  managed-remote target routing through one claim/run/workroom contract.
- [`fc-substrate.md`](./fc-substrate.md) — #8638 existing Fleet Command substrate
  projected into both clients; not another program or epic.
- [`fc-5-dogfood.md`](./fc-5-dogfood.md) — #8640 real Codex+Claude runtime proof
  narrowed to one accepted simultaneous Phase A receipt; R3/R7 stay with the
  client/program records.

Immediate bounded queue:

- [`../2026-07-11-openagents-coding-cutover-issue-plan.md`](../2026-07-11-openagents-coding-cutover-issue-plan.md) —
  CUT-01–CUT-27, live as #8681–#8707 with GitHub sub-issue relationships,
  dependency order, parallel-safe lanes, completion criteria, and receipts for
  replacing ordinary Codex/Claude Code UI use with installed OpenAgents
  Desktop plus physical mobile continuation/supervision.

- [`native-streamed-conversation-handoff.md`](./native-streamed-conversation-handoff.md) —
  #8676 one real provider-neutral Desktop stream through the host-owned Runtime
  Gateway, immediately continued on physical mobile with the same refs.
- [`conversation-fault-convergence.md`](./conversation-fault-convergence.md) —
  #8677 bounded lost-ack/duplicate/gap/offline/restart/revocation/migration
  convergence proof for the shared conversation seam.

Closed bounded Desktop leaves and proofs:

- [`desktop-codex-subagent-history.md`](./desktop-codex-subagent-history.md) —
  #8674 loss-accounted historical Codex parent/subagent/tool rendering and the
  opinionated Desktop Agents/Item inspector, delivered through Runtime Gateway
  v4 with synthetic scale and structure-only real-history receipts.
- [`desktop-codex-trace-acceptance.md`](./desktop-codex-trace-acceptance.md) —
  #8675 closed real-Electron predictable trace-workspace UX promise and
  public-safe acceptance receipt.
- [`local-first-identity.md`](./local-first-identity.md) — #8666 immutable
  device-local identity/authority plus optional reversible OpenAgents account
  linking on Desktop and mobile.

Completed topology parent with bounded residual leaves:

- [`desktop-effect-scope-topology.md`](./desktop-effect-scope-topology.md) —
  #8678 combines its initial manifest/cache declarations with closed #8683 and
  #8684 source-coupled perimeter enforcement, replaceability, lifecycle
  disposal, structured correlation, and full Desktop host acceptance.

Revision 31 work without a decomposed issue graph still includes portable
session authority, owner-managed/provider targets, the general capability
broker, and any-host mobile plus persona-neutral voice. Live local multi-agent
supervision is now #8691/#8692, while host movement remains later work. Do not
silently broaden #8676/#8547/#8636 or CUT-27 to absorb those contracts.

Closed #8673 at `bf4037e923` is the schema-bounded Runtime Gateway v3 confirmed
timeline seam consumed by #8674; it explicitly did not add visible UI.

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

## Closed `wontdo` / not-planned records

- [`app-web-consolidation.md`](./app-web-consolidation.md) — #8634 broad web
  consolidation.
- [`app-forum.md`](./app-forum.md) — #8635 broad Forum conversion.
- [`app-landing.md`](./app-landing.md) — #8595 landing/root product work.
- [`sarah-presentation.md`](./sarah-presentation.md) — #8610 Sarah/avatar/
  opener/voice/video/presentation work.
- [`role-programs-and-colleagues.md`](./role-programs-and-colleagues.md) —
  #8643 roles/named colleagues/persona expansion.
- [`glass-ui-and-sarah-mobile.md`](./glass-ui-and-sarah-mobile.md) — #8646 and
  #8650 presentation/lowering program; #8647–#8649 remain closed historical
  receipts.
- [`blueprint-correction.md`](./blueprint-correction.md) — #8642 correction,
  deletion, provenance export, and tombstone-propagation product backlog.

These records are closed with GitHub reason `not planned`, labeled `wontfix`,
and removed from `roadmap:sol`. Do not reopen them as dormant epics. A real
security, privacy, data-loss, accessibility, outage, or supported-surface defect
gets a new bounded incident/task under its actual owning program.

Completed bounded leaves, including #8653's recent local Codex-history
projection and
[`desktop-runtime-gateway.md`](./desktop-runtime-gateway.md) (#8655) plus
[`desktop-sync-host.md`](./desktop-sync-host.md) (#8656) and
[`mobile-sync-host.md`](./mobile-sync-host.md) (#8657),
[`mobile-session-vault.md`](./mobile-session-vault.md) (#8658), and
[`mobile-session-recovery.md`](./mobile-session-recovery.md) (#8659), and
[`mobile-session-pkce.md`](./mobile-session-pkce.md) (#8660), and
[`desktop-session-vault.md`](./desktop-session-vault.md) (#8661), and
[`desktop-session-recovery.md`](./desktop-session-recovery.md) (#8662), and
[`desktop-session-loopback-policy.md`](./desktop-session-loopback-policy.md)
(#8663), and [`desktop-session-pkce.md`](./desktop-session-pkce.md) (#8664),
plus [`desktop-session-controls.md`](./desktop-session-controls.md) (#8665), remain
closed receipts rather than open P0 queue items.

## Other source records

- [`effect-native-electron-host.md`](./effect-native-electron-host.md) —
  upstream reusable Electron host gap referenced by #8574.

Update checked-in sources, live issue bodies/labels, the master roadmap, and
this index together when scope or disposition changes materially. Historical
receipts remain historical; do not rewrite them into current proof.
