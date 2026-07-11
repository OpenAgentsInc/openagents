# docs/sol

Sol owns the canonical, implementation-grounded OpenAgents roadmap. The active
direction is reliable Desktop/mobile coding and fleet software built with
Effect Native and Khala Sync. OpenAgents mobile absorbs the useful Khala Code
MVP capability set—including coding through brokered remote workrooms—rather
than remaining a supervision-only client. Sarah/persona, relationship-first
front-door, A/V, presentation, landing, portal, optional visual-polish, and
broad web-conversion backlog issues are closed `wontfix` / not-planned. Real
security, production-integrity, compatibility, accessibility, and privacy
defects begin as new bounded issues under their actual owning program.

These documents never replace runtime authority, product promises, owner gates,
live issue state, tests, or receipts. They turn owner decisions into dependency
order, implementation contracts, and honest proof requirements.

## Authority and reading order

When documents disagree, use this order:

1. [`AGENTS.md`](../../AGENTS.md) and
   [`INVARIANTS.md`](../../INVARIANTS.md) for workspace/repository law.
2. Owning schemas, behavior contracts, Blueprint contracts, product promises,
   and runtime policy for enforceable behavior.
3. [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md), Revision 29, for current product
   direction, priority, R0–R7 gates, and issue disposition.
4. Live GitHub issue state and [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md) for
   current ownership and collision avoidance.
5. Current code, tests, deployments, release records, and receipts for factual
   proof state.
6. Dated Sol/Fable analyses for historical argument only when their banner says
   they are superseded.

Current code and live issue state can move faster than prose. Reconcile the
roadmap; never repeat a stale claim merely because it is written here.

## Start here

- [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) — canonical direction: reliable
  Desktop/mobile coding and fleet control is P0; OpenCode-parity Desktop and a
  compact remote-coding mobile client share identity, Khala Sync state, typed
  workroom/actions, outcomes, and receipts.
- [`2026-07-10-episode-195-followup-analysis.md`](./2026-07-10-episode-195-followup-analysis.md)
  — compares the episode's Desktop/mobile, conversation-native delegation,
  scheduled work, history/memory, integrations, local inference, open-source,
  and compute-economics thesis against Revision 28 and proposes bounded
  Revision 29 amendments.
- [`2026-07-10-openagents-desktop-product-architecture.md`](./2026-07-10-openagents-desktop-product-architecture.md)
  — binding Desktop process/data/authority topology: tokenless local Effect
  Native renderer, host-owned Runtime Gateway over existing Pylon/Khala Sync/
  workspace services, early mobile conversation continuation, and the fastest
  F0–F7 delivery path.
- [`issues/desktop-codex-subagent-history.md`](./issues/desktop-codex-subagent-history.md)
  — closed #8674 contract/receipt for loss-accounted historical Codex parent/
  subagent/tool rendering and the Desktop Agents/Item inspector.
- [`issues/desktop-codex-trace-acceptance.md`](./issues/desktop-codex-trace-acceptance.md)
  — #8675 immediate predictable real-Electron trace-workspace UX promise and
  video-readiness proof.
- [`issues/native-streamed-conversation-handoff.md`](./issues/native-streamed-conversation-handoff.md)
  — #8676 first real provider-neutral Desktop stream and physical mobile
  continuation vertical slice.
- [`issues/conversation-fault-convergence.md`](./issues/conversation-fault-convergence.md)
  and [`issues/desktop-effect-scope-topology.md`](./issues/desktop-effect-scope-topology.md)
  — #8677 bounded conversation fault matrix and #8678 executable Effect scope/
  boundary freeze before broad workbench expansion.
- [`2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md`](./2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md)
  — exhaustive legacy capability disposition, Effect Native destination,
  remote-workroom boundary, ordered waves, and physical-device acceptance.
- [`2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md`](./2026-07-10-112832-cdt-reliable-fleet-implementation-delegation.md)
  — ordered, claim-aware implementation packet for bounded coding agents.
- [`2026-07-10-opencode-khala-openagents-desktop-parity-audit.md`](./2026-07-10-opencode-khala-openagents-desktop-parity-audit.md)
  — 20-area OpenCode/Khala/OpenAgents Desktop capability audit; its parity
  findings remain active while Sarah-steerability framing is superseded.
- [`issues/README.md`](./issues/README.md) — checked-in source records and
  current roadmap disposition for the live issue set.
- [`2026-07-10-terra-execution-lane.md`](./2026-07-10-terra-execution-lane.md)
  — Sol↔Terra execution/claim contract under R0–R7.
- [`OPERATING_MODEL.md`](./OPERATING_MODEL.md) — how to select, specify, land,
  verify, and reconcile one bounded implementation slice.
- [`SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`](./SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md)
  — current responsibilities for Desktop, mobile, Effect Native, Khala Sync,
  Fleet/Pylon, evidence, releases, and operations.
- [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md) — same-session and cross-session
  claim, hot-contract, staleness-audit, and release rules.
- [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) — accepted risks, falsifiers,
  tripwires, and review points for the reliability program.

## Current program shape

```text
intent -> policy -> orchestration -> execution
  ^                                  |
  |                                  v
state <- Khala Sync <- evidence <- durable outcome

OpenAgents mobile <----------------> OpenAgents Desktop
remote coding + fleet                 full workbench + fleet
```

The active gates are:

1. **R0:** truthful green Desktop/mobile foundations;
2. **R1:** shared authenticated identity and session;
3. **R2:** authoritative Khala Sync continuity;
4. **R3:** real Fleet operations from both clients;
5. **R4:** interruption, offline, replay, and recovery safety;
6. **R5:** practical OpenCode-parity Desktop workbench;
7. **R6:** compact mobile remote coding and fleet control;
8. **R7:** signed releases and sustained cross-device dogfood.

Desktop and mobile may expose different host capabilities. Mobile files,
terminal, preview, and writeback are brokered remote-workroom capabilities, not
local device authority. The clients never own different identifiers, workroom/
run state, command outcomes, authority, or receipts.
Web remains a supported public/API/operations surface, not the active product-
expansion queue.

## Historical analysis

The dated 2026-07-09 documents preserve the reasoning that preceded Revision
24. Their supersession banners control how they may be used:

- [`2026-07-09-sarah-first-product-architecture.md`](./2026-07-09-sarah-first-product-architecture.md)
  — historical Sarah-first thesis; named front-door conclusion superseded.
- [`2026-07-09-roadmap-system-model.md`](./2026-07-09-roadmap-system-model.md)
  — historical relationship-loop model; current authority loop is in the
  master roadmap.
- [`2026-07-09-execution-sequence-and-critical-path.md`](./2026-07-09-execution-sequence-and-critical-path.md)
  — historical queue; R0–R7 supersedes its sequencing.
- [`2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md)
  — historical C0–C3 runtime cutover; C2 remains substrate proof, not product
  front-door authority.
- [`2026-07-09-greenfield-mobile-desktop-decision.md`](./2026-07-09-greenfield-mobile-desktop-decision.md)
  — greenfield/identity/extraction decisions retained; “Sarah is home” is
  superseded.
- [`2026-07-09-effect-native-strategic-importance.md`](./2026-07-09-effect-native-strategic-importance.md)
  — Effect Native conclusion retained; Sarah-first rationale is historical.
- [`2026-07-09-authority-trust-and-economics.md`](./2026-07-09-authority-trust-and-economics.md)
  — authority/evidence/economic separation retained; Sarah framing historical.
- [`2026-07-09-risks-tensions-and-decision-tests.md`](./2026-07-09-risks-tensions-and-decision-tests.md)
  — objections and tests that helped motivate the reliability reset.
- [`2026-07-09-issue-triage.md`](./2026-07-09-issue-triage.md) — historical
  issue-reset receipt, not the current issue list.
- [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) — compact
  supersession pointer; the old rev-1 30-item queue is no longer dispatchable.

## Working method

Every new or updated Sol artifact should:

1. pin its date and source snapshot;
2. separate current fact, interpretation, recommendation, and owner decision;
3. inspect live issue state and claims before naming a next leaf;
4. reference existing typed services before proposing new schemas;
5. name exact paths, hot contracts, verification, and closeout evidence;
6. preserve Blueprint authority, privacy, approvals, idempotency, and receipt
   boundaries;
7. distinguish code-landed, fixture-proven, deployed/distributed, live-proven,
   owner-accepted, and closed;
8. add supersession banners to dated analysis instead of silently rewriting
   its original argument; and
9. update the master roadmap and this index when priority, proof state, or the
   document set materially changes.

The short version: **build reliable direct software first; keep one typed
authority and Sync reality; treat personas and presentation as optional future
consumers, not the product foundation.**
