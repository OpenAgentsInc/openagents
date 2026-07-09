# docs/sol

Grounded implementation analysis written from the Sol lane: implementation
designs, subsystem consequences, current-state reconciliation, critical-path
reasoning, and explicit counterarguments for OpenAgents.

**Sol now owns the canonical OpenAgents roadmap.** Fable remains high-level
strategic source material, but its master roadmap is superseded. Sol turns
strategy and owner decisions into grounded implementation design and the
day-to-day issue set. Sol asks an operational set of questions:

- What must change in each subsystem for the strategy to become true?
- What is already implemented, what is only partially landed, and what proof
  is still missing?
- Which slice should an implementation agent take next?
- Which contracts, invariants, tests, migrations, and receipts must move with
  that slice?
- Which strategic tensions should cause the implementation plan to change?

These are orientation and analysis artifacts. They do not change runtime
authority, widen public claims, flip product promises, authorize spend, or
override an owner gate.

## Authority and reading order

When documents disagree, use this order:

1. [`AGENTS.md`](../../AGENTS.md) and [`INVARIANTS.md`](../../INVARIANTS.md)
   for repository law and authority boundaries.
2. Product specs, behavior contracts, promise records, and owning-surface
   invariants for durable intent and enforceable claims.
3. [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) for canonical program direction,
   priority, issue set, and execution order.
4. Fable analyses for historical strategy and arguments, never current queue
   authority.
5. Current code, issue state, tests, and implementation receipts for factual
   status. When they move, Sol must reconcile rather than repeat stale prose.

The current roadmap reset is dated 2026-07-09 and recorded in
[`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) rev 5. Issue state and
implementation status can move faster than essays; current code, issue state,
and receipts win when they do.

## Start here

- [`MASTER_ROADMAP.md`](./MASTER_ROADMAP.md) — the unified roadmap: Sarah Fleet
  Command is P0, presentation is parallel, 15 active issues plus two gated P2
  issues replace the old 30-item queue, most public pages retire, and the
  product becomes three Effect Native OpenAgents applications over one
  seven-layer relationship loop.
- [`2026-07-09-greenfield-mobile-desktop-decision.md`](./2026-07-09-greenfield-mobile-desktop-decision.md)
  — binding implementation decision for the new React Native mobile and
  Electron desktop apps, legacy freezes, mobile identity/icon, and extraction
  rules.
- [`2026-07-09-issue-triage.md`](./2026-07-09-issue-triage.md) — exact receipt
  for the 7 rewritten issues, 8 new issues, and 23 superseded/postponed
  closures.
- [`2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md`](./2026-07-09-codex-parallelism-and-sarah-fleet-cutover.md)
  — current same-session sub-agent capacity versus additional Codex tabs, and
  the exact C0–C3 gates for moving routine coding from this Codex app to the
  Sarah/Khala/Pylon workflow.
- [`issues/README.md`](./issues/README.md) — source bodies for the canonical
  live GitHub issue set.
- [`SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`](./SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md)
  — what Sarah-first concretely requires from Sarah, Khala, Blueprint, Pylon,
  Agent Computers, mobile, desktop, Effect Native, Sync/data, commercial
  systems, evidence, and operations.
- [`OPERATING_MODEL.md`](./OPERATING_MODEL.md) — how Sol should be used day to
  day: inspect live state, select a slice, write the implementation contract,
  land code with its tests and receipts, and reconcile the roadmap.
- [`CLAIM_PROTOCOL.md`](./CLAIM_PROTOCOL.md) — same-session coordination and
  the cross-session GitHub CLAIM/staleness/hot-contract protocol.
- [`CHALLENGE_LEDGER.md`](./CHALLENGE_LEDGER.md) — Fable challenges, Sol
  dispositions, falsifiers, tripwires, and revisit points.
- [`2026-07-09-roadmap-system-model.md`](./2026-07-09-roadmap-system-model.md)
  — my compact model of the entire roadmap: one relationship surface over a
  typed, receipt-bearing work system. It maps the major layers and explains
  what the P0–P7 sequence is really accumulating.
- [`2026-07-09-sarah-first-product-architecture.md`](./2026-07-09-sarah-first-product-architecture.md)
  — why Sarah-first is an architectural consolidation rather than a mascot or
  funnel decision; how conversation, canvas, memory, dispatch, approvals, and
  receipts form one product loop.
- [`2026-07-09-effect-native-strategic-importance.md`](./2026-07-09-effect-native-strategic-importance.md)
  — why Effect Native matters to the product thesis, not merely to code reuse:
  it is the typed interaction and rendering boundary that lets Sarah's product
  remain one system across web, mobile, desktop, and canvas.
- [`2026-07-09-execution-sequence-and-critical-path.md`](./2026-07-09-execution-sequence-and-critical-path.md)
  — the roadmap recast as dependency chains and convergence milestones,
  including the first Sarah-to-fleet vertical slice and the places where
  parallel work is safe.
- [`2026-07-09-authority-trust-and-economics.md`](./2026-07-09-authority-trust-and-economics.md)
  — the non-negotiable control model: Sarah interprets and presents, while
  typed services authorize, execute, meter, and prove. It also separates the
  economic rails that must not be conflated.
- [`2026-07-09-risks-tensions-and-decision-tests.md`](./2026-07-09-risks-tensions-and-decision-tests.md)
  — the strongest objections to the current plan, early warning signals, and
  tests that should cause the team to narrow, reorder, or revise it.

Historical Sol snapshot:

- [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) — superseded rev 1
  reconciliation of the old Fable 30-item queue. Retained only to show why the
  deeper reset became necessary.

## Working method

New Sol analysis should:

1. Pin its date and source snapshot.
2. Separate verified state from interpretation and recommendation.
3. Inspect current code, issues, PRs, and receipts before describing status.
4. Link to the owning source rather than copying an entire status ledger.
5. Name exact files, contracts, dependencies, tests, and exit receipts for an
   implementation recommendation.
6. Name counterarguments and failure conditions, not just the preferred case.
7. Treat receipts, promise state, and authority boundaries as harder than
   prose.
8. Update the master roadmap when a landing changes residual work or
   Sarah-first sequencing.
9. Update this README whenever a new Sol document lands.

The short version: **Sol owns the roadmap and grounded implementation design;
Fable remains strategic source material.**
