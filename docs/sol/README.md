# docs/sol

Grounded implementation analysis written from the Sol lane: implementation
designs, subsystem consequences, current-state reconciliation, critical-path
reasoning, and explicit counterarguments for OpenAgents.

Sol is complementary to [`docs/fable`](../fable/README.md), not a replacement
for it. **Fable is the high-level strategic planner. Sol turns that strategy
into grounded implementation design and the day-to-day execution roadmap.**
Sol asks a more operational set of questions:

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
3. [`MASTER_ROADMAP.md`](../fable/MASTER_ROADMAP.md) for high-level strategy,
   program constraints, and the current ordered queue head.
4. [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) for grounded
   day-to-day sequencing, residual work, and Sarah-first reconciliation inside
   those strategic constraints.
5. Current code, issue state, tests, and implementation receipts for factual
   status. When they move, Sol must reconcile rather than repeat stale prose.

The initial Sol corpus is based on `origin/main` at `93bfa6b7e3` and
`MASTER_ROADMAP.md` rev 6.19, dated 2026-07-09. Issue state and implementation
status can move faster than these essays; current code, issue state, and
receipts win when they do.

## Start here

- [`IMPLEMENTATION_ROADMAP.md`](./IMPLEMENTATION_ROADMAP.md) — the grounded
  Sarah-first execution plan. It preserves the OAV queue head, promotes the
  missing Sarah→coding vertical slice, reclassifies all 30 open roadmap items,
  and identifies which work is active, parallel, owner-gated, or dependency-
  held.
- [`SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md`](./SUBSYSTEM_IMPLEMENTATION_IMPLICATIONS.md)
  — what Sarah-first concretely requires from Sarah, Khala, Blueprint, Pylon,
  Agent Computers, mobile, desktop, Effect Native, Sync/data, commercial
  systems, evidence, and operations.
- [`OPERATING_MODEL.md`](./OPERATING_MODEL.md) — how Sol should be used day to
  day: inspect live state, select a slice, write the implementation contract,
  land code with its tests and receipts, and reconcile the roadmap.
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
8. Update the implementation roadmap when a landing changes residual work or
   Sarah-first sequencing.
9. Update this README whenever a new Sol document lands.

The short version: **Fable sets the strategic direction; Sol is the grounded
implementation lead that turns it into subsystem designs, next slices, tests,
and receipts—and keeps that plan current day to day.**
