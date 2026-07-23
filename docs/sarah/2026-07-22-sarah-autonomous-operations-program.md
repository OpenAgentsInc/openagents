# Sarah autonomous operations program

Date: 2026-07-22. Status: program spec. This is the north star that the
autonomous Sarah driver builds toward. It does not grant authority. Authority
stays in `AUTHORITY.md` (revision 7) and `docs/authority/SARAH_AUTHORITY.md`
(revision 5). The bounded first implementation is the scheduled autonomous tick.

## Problem

Sarah has the authority, the tools, the dashboard, and the memory infrastructure
to run the company, but she has no heartbeat. She acts only when the owner opens
the mobile app and sends a message. Her last conversation turn was 2026-07-10,
and she has never once dispatched a coding worker. Everything built for her is
latent capability that never fires on its own.

The owner introduced Sarah as the agent who runs the company during parental
leave. Running the company means initiating work, not waiting to be asked.

## Model

Give Sarah a scheduled tick. On each tick, with no owner message required, she:

1. Reads bounded, cited company state through `collectSarahBusinessContext`
   (releases, Full Auto, fleet, Forum, open issues, cloud health).
2. Decides the single next best admitted action, or names the top blocker.
3. Acts through her existing typed capability brokers.
4. Emits a receipt and posts a proactive owner-thread update.

This is a normal Sarah turn with an autonomous trigger and objective. It reuses
the existing turn machinery, authority, redaction, immutability, and receipts.
It adds no new power.

## What a tick may initiate

All through existing brokers, all receipted:

- Delegate a bounded coding task to owner-linked Codex capacity
  (`codex_workers_start`).
- Read Full Auto and steer an existing run (pause, resume, stop) when state
  calls for it.
- Draft blog, document, Forum, or timeline content (`sarah_web_comms`); the
  timeline draft queues for owner review.
- Post a proactive, receipt-backed owner-thread update summarizing what she
  observed and did.
- Record a `NEEDS-OWNER` blocker and continue on a non-blocked item.

## What stays reserved

The tick never widens authority. It cannot publish a stable release without the
independent-verification gate, spend above the tick budget, move value, make
legal or employment commitments, weaken an invariant, or make an unsupported
public claim. A blocked item becomes a `NEEDS-OWNER` note and the tick moves to
the next admitted action. Waiting is not an action.

## Bounds

- Off by default behind an env flag. Flag off means zero behavior change.
- One admitted action or one blocker report per tick.
- A per-tick token budget and a minimum interval between ticks.
- Fail-soft: a tick error never breaks the cron drive or other work.
- Owner-scoped: the tick resolves the one admitted owner and the deterministic
  Sarah thread, exactly like an interactive turn.

## Observability

- Every tick emits a proactive owner-thread update, so the owner sees what Sarah
  did without opening a console.
- Every action emits its existing target receipt.
- The operator dashboard (`/admin/operator`) is the at-a-glance view; a future
  slice adds a Sarah panel (last tick, last action, last owner update, blocker
  count) so "what is Sarah doing right now" is answerable at a glance.

## Success metrics

Sarah is working when the numbers move without the owner prompting her:

- Ticks that take a real admitted action per day (not just reports).
- Coding tasks delegated and closed out.
- Backlog burn-down (open issues, Full Auto runs advanced).
- Owner updates delivered per day.
- Zero reserved-boundary violations; every action carries a receipt.

## Rollout

1. Land the tick behind the default-off flag with tests (the bounded first
   implementation).
2. Owner enables the flag on a conservative interval and budget.
3. Watch the receipts and the owner updates; widen the interval, budget, and
   action set only as the receipts prove it safe.
4. Later slices: the dashboard Sarah panel, richer decision policy, and — once
   the graph memory is enabled — memory-informed continuity across ticks.

## Related

- `AUTHORITY.md` revision 7, `docs/authority/SARAH_AUTHORITY.md` revision 5.
- `docs/sarah/2026-07-22-sarah-company-command-analysis.md` (Episode 260 mandate).
- Sarah's graph memory (#9189) — continuity across ticks once enabled.
- The bounded first implementation is the scheduled autonomous tick landed under
  this program.
