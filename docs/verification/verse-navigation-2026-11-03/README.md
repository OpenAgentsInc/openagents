# Visible WoW navigation, 2026-11-03

Issues #12728–#12730. RTX 4080, native Verse window on desk 2, a 2548×1428
Coax desk, the existing Classic Era installation. WoW's Graphics page
confirmed Windowed (Fullscreen), 2548×1428 and 100% render scale. The
launcher now preserves that setup across a fresh host-runner start.

[Screenshot](native-wow.png) shows the actual character near the graveyard
gate in Coldridge Valley. No character position was injected or simulation
advanced outside ordinary leased keyboard input.

## What was measured

| Trial | Result | Record |
| --- | --- | --- |
| 250 ms W calibration | (19509,45709) → (19521,45732), heading 39648 unchanged | Local `/tmp/verse-navigation/calibrate-*.ppm` |
| 120 ms D calibration | Heading 39648 → 36502, position unchanged | Same local captures |
| Initial rule waypoint | Walked toward the boundary, then detected no progress and stopped at the wall | [raw journal](rules-wall.jsonl) |
| New nearby rule waypoint | Four left turns, one forward pulse, reached radius | [raw journal](rules-recovery.jsonl) |
| Initial numeric Elephant prompt | Approached the target, then oscillated and exhausted 24 actions | [raw journal](elephant-numeric.jsonl) |
| Compact semantic Elephant prompt | Eight right turns and two forward pulses, reached radius | [raw journal](elephant-compact.jsonl) |
| Next waypoint toward the gate | Four left turns and two forward pulses, reached radius | [raw journal](elephant-gate.jsonl) |
| Longer waypoint up the snowy clearing | Nine local decisions, reached radius | [raw journal](elephant-clearing.jsonl) |

The compact Elephant trial started at (19639,45749), heading 48135, and
finished at (19624,45775), heading 28311, within the declared radius of
(19630,45770). Its ten local model calls reported 188–375 ms. The next
waypoint finished at (19626,45830), within radius of (19630,45840).
The clearing trial used the installed controller and moved from
(19637,45891) to (19670,46110), within radius of (19670,46120), in nine
actions. The live game remained at native resolution throughout.
Each step observed again; arrival was measured, not inferred from a key ACK.

These are raw development journals, preserved without rewriting. The early
ones predate the terminal-stop/proposal rows and contain a prototype profile
spelling. Paths to full PPM captures are local `/tmp` artifacts; the selected
PNG above is retained here. A capture counter identifies a desk capture,
not a game simulation tick.

## Reused components and model

`aethyr/ai/pilot.coax` supplies bounded `Intent`s and external event
envelopes. The WoW adapter uses Aethyr's angle arithmetic and Psionic's
existing `GdContext`/ticket for one current observation. The added generic
`psionic/lib/game/choice.coax` validates the full pinned distribution before
staging an enum. The loop then checks the current sensor again and acquires
the target window's short input lease.

The local service is original Elephant through Psionic CUDA on
127.0.0.1:8790, identity
`elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.
The existing machine GPU switch loaded it; there was no remote or toy
fallback. Elephant receives a semantic left/right/ahead observation from the
position/facing sensor, not screenshot pixels. Its confidence is not a
navigation quality guarantee.

## Checks

- Shared Aethyr pilot: 39 entry-source mutants killed, zero survivors.
- WoW strip decoder: 87 mutants killed, zero survivors.
- Geometry/duration/key adapter: 91 entry-source mutants killed, zero survivors.
- Psionic choice decoder: 120 entry-source mutants killed, zero survivors.
- Controller closure: 148 Coax checks passed. Shared direct CLI: 87 passed.
- [Live menu/cancellation fixture](sanitized-checks.txt): a menu refuses
  before input; a stop file during an observed held key clears it; SIGKILL
  during an observed held key clears it. Repeated with the controller built
  under ASan/UBSan, leak checks enabled; no sanitizer finding.
- [Native statistics rollover](rollover.txt): 226 presentations with a
  64-frame sample capacity, 34 recent samples, zero live GPU objects at close.
- Aethyr book structure/source/link check passed. Navigation protobuf
  compiled with protoc 34.1. Nix flake evaluation passed.
- `tools-check` reports nine unrelated tools without reason headers on the
  final rebased main (six before the concurrent changes). The only new
  foreign SDK code here is the read-only WoW Lua sampler,
  with its reason declared in the source.

## Limits

This qualifies short feedback-controlled waypoints in map 1426. Waypoints
were selected after inspecting the images. There is no general terrain map,
obstacle planner, combat policy or autonomous quest selection. The wall
trial demonstrates why position/heading alone are insufficient for arbitrary
navigation. A sensed dead/combat/menu state, target change, stale sensor,
wrong decision identity, illegal proposal, no progress, stop or budget end
refuses further input. Controller records do not replay WoW's server state.
