# 96. Casts and combat under latency

> **Status: unimplemented — design draft.** Today a cast is checked when the
> server hears it (slot, level, the 8-tick cooldown, alive) and strikes when
> its cast bar ends (commit edebf848cc); a cast at a seat or creature lands
> where the target stands then (`eg_where` in `rm_eg_end`). None of this is
> written down as rules, a command carries no client time, and nothing is
> tested under latency. #12790 writes, implements and tests the rules.

Movement can be predicted and corrected. A hit can't be taken back once
someone saw their HP drop. So combat needs rules that say exactly what is
checked when, and what each player sees before the server answers. This
chapter adds those rules for a target-based game like Everglade.

## What this chapter teaches

- Latency compensation as a game rule, decided per game.
- When range and line of sight are checked.
- How old a command may be, and how client times are bounded.
- Ordering simultaneous actions in one tick.
- What the page predicts and what waits for the server.

## The design

The research doc (section 10) lists six questions the design must answer.
This design answers them as follows; #12790 settles the numbers.

1. **Range and sight.** Check both at cast start, against the server's
   positions at the tick the cast is stepped. Check range again at cast end
   with a stated tolerance (for example 10% of the spell's range), so a
   target walking out of range at the last moment is still hit, but one that
   ran 30 m away is not.
2. **Command age.** A cast carries the client tick it was pressed on, in
   server time (chapter 93's clock). The server accepts it if it is at most a
   stated age old (for example 250 ms) and refuses it as `late` otherwise.
   It never accepts a time from the future.
3. **Client times are bounded.** A client time earlier than the client's
   last accepted command, or later than now, is clamped, never trusted.
4. **The target dies mid-cast.** The cast ends with no effect and no cost,
   and the page shows `TARGET LOST`.
5. **Order within a tick.** Apply in a fixed order: interrupts, then
   casts that end this tick in seat order, then damage, then healing, then
   deaths. A heal and a killing blow on the same tick resolve by that order,
   the same way on every replay.
6. **What can be rewound.** Movement is predicted and corrected (chapter
   90). Combat is never rewound: an HP change, a condition or a death is
   final when the server sends it.

**What the page predicts.** The cast bar and the casting animation start at
once on the press. Damage numbers, conditions, deaths and XP wait for the
server. A refused cast stops the bar and says why.

## The latency budget

The research doc's illustrative budget for a 20 Hz server is about 136 ms
from press to result. Everglade's 30 Hz tick adds about 16.7 ms on average
instead of 25 ms:

```text
outbound 50 + tick wait 16.7 + processing 3 + return 50 + draw 8 ≈ 128 ms
```

Prediction makes the press feel immediate; it doesn't make the result
arrive sooner.

## What it builds

- The rules written in the multiplayer spec and enforced in
  `room-everglade.coax`'s cast path and `everglade5e-room-rules.coax`.
- A client tick on `C` frames (a wire change, so it goes with chapter 98's
  version check).

## Acceptance checks and oracles

- At 250 ms with 2% loss (chapter 100's shaped network): a cast at a target
  that steps out of range at the last moment hits; one at a target 30 m away
  is refused.
- A cast whose target dies first costs nothing.
- A heal and a killing blow in one tick resolve the same way in 100 replays.
- A cast stamped in the future is clamped, and one older than the bound is
  refused as `late`.

## Dependencies

Chapters 89 (intents), 90 (prediction) and 94 (effects).
