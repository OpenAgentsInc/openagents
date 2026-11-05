# 94. Effects are state, events are cues

> **Status: partly implemented.** A lasting world ability (Reverse Gravity,
> Wind Wall, Wall of Stone) is drawn only from its server entity, and its
> cast event is a cue (#12775, commit 26b09b5e11). Every condition drawn or
> said on the page reads one status state, filled once a frame
> ([everglade-status.coax](../../apps/openagents-web/everglade-status.coax),
> #12776, commit c3c2805dc6). The wire carries effects as `WK_EFFECT` rows
> with their cause, `T0` and `DUR` (commit 67b4f62448), so a room shows
> `HELD · HOLD PERSON` with its seconds left (commit 3370a41ddc). Your own
> lasting abilities show at your press and take the room's entity as that
> cast (commit f54bb0f001).

A spell that holds a player for 3 s has two parts: the fact that the player
is held, and the gold rings, the sound and the `HELD` line that show it.
This chapter adds the rule that keeps them apart: lasting effects are
replicated state, and everything you see or hear of them is derived from
that state.

## What this chapter teaches

- Conditions, zones, buffs, holds and walls as entities with a start and a
  duration.
- Particles, sound, rings and status lines derived from those entities.
- One-shot events as cosmetic cues only.
- Why this makes late joiners correct and double application impossible.

## The failure it removes

Before #12775, the page applied Reverse Gravity twice in one frame: once
when it heard the zone's entity, and once for the `e cast` event. The
second application turned the effect off again, so a room drew none of the
303 to 311 particles the offline page drew. Any other on/off spell would
end the same way, and Wind Wall and Wall of Stone would raise two. A page
that joined after the cast heard no event at all.

Both bugs have one cause: the event carried state.

## The design

**Lasting effects are entities.** Each condition, buff, debuff, hold or zone
that lasts is its own replicated entity (chapter 91), kind `WK_EFFECT`:

- `OWNER`: the entity it is on, as a target code.
- `P1`: its cause in one namespace: the SRD spell id when the rules made it,
  `1000 + AB_*` when an Everglade ability made it.
- `P2`: its condition bit (`WC_*`).
- `T0`: the tick it started. `DUR`: its length in ticks, 0 while it is held
  up by concentration until its `x ID`.

Zones and walls are already entities (`WK_ZONE`, `WK_STONE`) and get `T0`
and `DUR` too.

**Everything shown is derived.** Each frame the page fills one status state
from the entity table. The status lines, the rings on every body, the
particles and the hotbar read only that state.

<!-- source: apps/openagents-web/everglade-status.coax status-effect -->
```jai
// An effect on a body: WHO (ST_YOU, or a TGT code: 1000 + SS a seat, 0-999 a watch actor, 2000 + EID a
// creature), its condition bits (WC_*), the spell that made it (ST_*; -1 not said) and its seconds left (-1 held up, or not said).
StEffect :: struct { who: int = -1; cond: int; cause: int = -1; left: f32 = -1; }
EgStatus :: struct { n: int; e: [ST_MAX] StEffect; }
```

Seconds left are `T0 + DUR − tick`, from the rules' own clock, so a late
joiner sees the same seconds as everyone else.

**Events are cues.** An `e cast` plays a sound and a flash from its seed. It
never creates, toggles or ends an effect. Applying a cue twice plays it
twice and changes no state, so double application can't happen.

**Your own effects are never dropped.** An effect on the receiving seat goes
in every frame regardless of the budget (chapter 92). You always see what
holds you, with the spell's name and its time left.

## What is left

- `WK_EFFECT` with `T0` and `DUR` on the wire (#12783), then the status
  state reads it (#12776).
- The drift check still reports the Reverse Gravity particles, Wall of
  Stone panels, conditions on you and HELD lines as drifting at some
  profiles (#12718's table at `cbb9a1ce9b`; the local run at `a941197cf1`
  matches for Reverse Gravity, Wind Wall and Wall of Stone).

## Acceptance checks

- A page that joins 2 s into a 6 s hold sees the hold, its rings and
  `HELD · HOLD PERSON · 4 S`.
- Replaying a recorded session with every `e` event duplicated draws the
  same frames.
- The drift check's spell and condition probes hold at local, 100 ms and
  250 ms.

## Dependencies

Chapters 72 (effects made of particles) and 91 (the record).
