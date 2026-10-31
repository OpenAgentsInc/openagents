# 4. Input becomes typed actions

> **Status: unimplemented — design draft.** Each realm reads raw events
> itself today: the browser host forwards keys, pointers, the wheel and phone
> tilt through `coaxial_key`, `coaxial_pointer`, `coaxial_wheel` and
> `coaxial_tilt`, and each realm turns them into movement its own way
> (`verse_player_input`, the Grid's `Sticks`, Ice Cream Man's `H_*` bits).
> This chapter plans one mapping to typed actions, sampled once per fixed
> step. Engine-core task: `ec-input` (#12609); the engine-core agent fills
> this chapter in as the module lands. Nothing here runs yet.

Input arrives as events, at any time, from keyboards, mice, fingers and
gamepads. The simulation runs in fixed steps (chapter 1). Between them sits a
mapping: raw events become a small set of typed actions (move, look, jump,
run, act, zoom), and each step reads one sample of them. If that mapping is
in each realm, a phone stick and a keyboard can disagree about running, and a
quick tap can be lost between two frames that ran no step, or counted twice
in a frame that ran three.

## What this chapter teaches

- Actions as the engine's input vocabulary, separate from devices.
- Levels (held axes) versus edges (presses).
- Sampling once per step: no lost and no doubled presses.
- Dead zones, clamping and non-finite input.
- One action stream for desktop and phone.

## The method

Devices write into an input buffer between frames. At each fixed step the
loop takes one `Actions` sample:

```jai
Actions :: struct {
  move:  Vec2      // stick or WASD, |move| ≤ 1, zero inside the dead zone
  look:  Vec2      // radians this step (drag, arrows, tilt)
  zoom:  f32       // steps of zoom this step
  run:   bool      // level
  jump:  Edge      // pressed since the last step that consumed it
  act:   Edge
}
edge_consume(e) = (e.pressed_count > 0)  then reset to 0
```

Levels are sampled: whatever is held at the step. Edges are counted: a press
is recorded when it happens and consumed by exactly one step, the first one
after it. A frame that runs no step keeps the press; a frame that runs three
steps delivers it to the first. Look deltas accumulate and are split across
the frame's steps, so turning speed does not depend on the step count.

Mapping is data: a table from device inputs to actions (keys, a touch
region, a pad axis), with run's rule declared once. The walking controller's
rule today is the owner's: moving runs by default and Shift walks; on a
phone a full stick push runs and a light one walks. Non-finite values are
replaced by zero, axes are clamped to `[−1, 1]`, and a stick inside the dead
zone is no input.

## What exists today

- [coax/web/host.js](../../coax/web/host.js): `KEYS` (named keys as negative
  codes), `coaxial_key`, `coaxial_key_up`, `coaxial_blur` (nothing is held),
  `coaxial_pointer(state, x, y, phase, kind, id)` with touch and mouse told
  apart, `coaxial_wheel`, `coaxial_tilt`.
- [verse/player.coax](../../verse/player.coax): `verse_player_input` clamps
  axes, drops non-finite values, and treats jump as an edge
  (`pressed := jump && !p.jump_held`), applied at once rather than at the
  next step; `verse_runs` and `RUN_BY_DEFAULT`; `DEAD_ZONE :: 0.2`.
- [verse/grid.coax](../../verse/grid.coax): `Sticks`, the touch controls: a
  left-half move stick, right-half look, a quick right tap to jump, a hotbar
  slot.
- `verse/icecream/play.coax`: its own held-key bits (`H_W`, `H_SPACE`, …)
  and `HOLD_JUMP`, `HOLD_FIRE`.
- `openagents.engine.v1.InputIntent` in `proto/` for remote and agent input
  ([protocol chapter](../../docs/verse/openagents-verse-protocol.md)).

## What it builds

- `aethyr/input.coax`: `Actions`, `Edge`, the device-to-action table, the
  input buffer the host writes, `input_sample(buffer, step_index, steps)`
  called by chapter 1's loop once per step, and the conversion from
  `InputIntent` for agent input.
- It replaces per-realm input paths: realms read `Actions`, never keys.

## Acceptance checks and oracles

- The same action stream drives desktop and phone: a recorded keyboard
  session and a touch session mapped to the same `Actions` sequence produce
  the same states.
- No input is lost or doubled between frames and steps: property checks over
  random press timings and frame traces (including frames with zero and with
  `N_max` steps) show each press consumed by exactly one step.
- Look splitting: total yaw after a drag is independent of frame rate.
- Oracle: a reference event-time model that assigns each press to the first
  step whose start follows it.
- Losing focus (`coaxial_blur`) releases every level the same step.

## Dependencies

[Chapter 1](ch001-a-frame-has-a-fixed-step.md) (sampled per step) and
[chapter 2](ch002-spaces-and-units.md) (look in radians, pointer in screen
space). From Coax: R13 clocks and sample-and-hold
([Coax roadmap](../../coax/ROADMAP.md)), protocol messages
([Coax chapter 18](../../coax/book/ch18-protocol.md)). Background:
[game-engine systems inventory, section 34](../../docs/research/game-engine-systems.md).
