# 92. Priority under a budget

> **Status: partly implemented.** Each tick the server fills one table of
> rows (`WireHist`, [everglade-wire.coax](../../apps/openagents-web/everglade-wire.coax)),
> and each seat's rows are cut from it by priority, weight over distance,
> within `WIRE_BUDGET`, 1,000 bytes a tick (30 KB/s); the seat's own row and
> its removals go on top (#12783, commit edd9601102). A seat whose socket has
> more than `EG_BEHIND` (4,096 bytes) waiting gets only its own row that
> tick, and a socket is closed past `OUT_MAX` (256 KB) waiting or when its
> output hasn't emptied for `OUT_AGE_MS` (3 s) (`room-host.c`, commit
> 67b4f62448). Priority doesn't yet accumulate for a row left out, as this
> chapter designs.

A raid puts 16 players, up to 64 raiders, 38 townsfolk and their spells in
one square. Chapter 91 says how to write each entity's state. This chapter
decides which states fit in a client's frame this tick, and what happens to
a client that can't keep up.

## What this chapter teaches

- Estimating the bytes a room sends, before measuring them.
- Priority from distance and relevance, accumulated so nothing starves.
- A per-client byte budget per tick.
- Send queues bounded by age as well as size.

## How many bytes

The research doc (section 11) estimates outbound payload as
`B ≈ N × k × f × s`: recipients, relevant changing entities per recipient,
updates per second and bytes per update. For one full Everglade raid room,
with everyone in view:

```text
N = 16 seats, k = 16 + 64 + 38 = 118 entities, f = 30 Hz, s ≈ 40 bytes
B ≈ 16 × 118 × 30 × 40 = 2,265,600 bytes/s per room
per seat: 118 × 30 × 40 = 141,600 bytes/s, about 1.1 Mbit/s
```

These are estimates, not measurements. Deltas (chapter 91) make most fields
cost nothing when they don't change, but a raid changes positions every
tick. 1.1 Mbit/s is a lot for a phone on a weak network, and a raid is
exactly when players crowd. So a budget is needed.

## The design

**Priority.** Each entity has a priority for each seat:

```jai
p(e, seat) += base(kind_relevance) × falloff(distance(e, seat))   // each tick
```

Write entities in order of `p`, highest first, until the seat's budget for
the tick is spent. An entity that was written resets its `p` to 0. One that
wasn't keeps accumulating, so a far entity is sent less often, but it is
always sent eventually. There are no per-kind thresholds; kind enters only
through `kind_relevance`.

**Always sent.** Some state is never dropped by the budget or by distance:

- your own entity (its `ACK` is what chapter 90 replays from);
- effects whose owner is you (chapter 94): a hold on you is shown even when
  the frame is full.

**Budget.** A seat gets a fixed number of bytes a tick, set per realm. When a
frame is full, the rest wait for the next tick with their priority intact.

**Bounded by age.** A slow client must not collect an unlimited history of
positions it will never use. Track each socket's queued bytes and the age
of its oldest frame. Past a stated age, drop the queued state frames and
send the next one as a snapshot (a new baseline). Close the socket only when
it stays behind.

## What it builds

- The server's per-seat priority and budget in `WireHist` (#12783).
- The host's queue age (`conn.since`), and the counters chapter 99 reports.

## Acceptance checks and oracles

- At the budget, a seat's frames never exceed it, and every entity in view
  is written at least once every `N` ticks (a bound stated with the budget).
- Your own entity and effects on you are in every frame.
- A client throttled to 64 kbit/s recovers to a correct table within one
  second of the throttle ending, without the server closing it.
- The bytes per seat measured in a 60-raider raid are reported next to the
  estimate above (chapter 99).

## Dependencies

Chapter 91 (the record).
