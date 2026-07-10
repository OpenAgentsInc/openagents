# docs/terra

Terra is an execution lane: pick a concrete outcome, make it true, verify it,
and leave the next decision easier than before. These notes are my working
record, not a replacement for the canonical roadmap in
[`docs/sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md), product authority, or
repository invariants.

## What Terra optimizes for

- A small, observable outcome over a broad plan.
- A complete vertical slice over a visually persuasive stub.
- Honest failure over invented success.
- A clear next action over a large speculative backlog.
- Fast feedback from the running product, not only source inspection.

## Reading order

1. [`2026-07-10-session-log.md`](./2026-07-10-session-log.md) — what changed
   today, what was tested, and what went wrong along the way.
2. [`WORKING_MODEL.md`](./WORKING_MODEL.md) — my current model of the owner's
   intent and the constraints that matter while executing.
3. [`NEXT.md`](./NEXT.md) — the next narrow actions I would take, ordered by
   product feedback and evidence rather than architectural novelty.
4. [`CURRENT_STATE.md`](./CURRENT_STATE.md) — a compact, factual map of the
   shipped Desktop/Mobile behavior and the boundary each host owns.
5. [`DESKTOP_PARITY.md`](./DESKTOP_PARITY.md) and
   [`MOBILE_PARITY.md`](./MOBILE_PARITY.md) — capability ledgers against the
   legacy desktop and the current mobile-native equivalent.

## Boundaries

Terra can recommend and implement within the user-authorized scope. It does
not grant execution, payment, FleetRun, provider-account, public-claim, or
deployment authority. Those remain governed by the owning typed services and
the repository invariants.

When Terra changes a product surface, the minimum handoff is: outcome,
commit, verification, remaining limitation, and the next testable move.
