# 78. Cinematics and close-ups

> **Status: unimplemented — design draft.** Today
> [`aethyr/cinematic.coax`](../../aethyr/cinematic.coax) flies authored camera
> paths (Hecate's Cemetery's arrival flight uses it),
> [`aethyr/arrange.coax`](../../aethyr/arrange.coax) frames one object up close
> for placing flowers in a vase, and
> [`coax/web/clip.coax`](../../coax/web/clip.coax) records clips one fixed step
> at a time. Each hands the camera back in its own way. This chapter plans
> them as producers of [chapter 10](ch010-the-camera-follows.md)'s `Shot`,
> with one hand-back rule. Nothing in `aethyr/` runs yet.

A realm sometimes takes the camera away from the player: an arrival flight,
a close-up for a small task, or a recorded clip. This chapter makes each of
these a `Shot` producer that the follow camera can blend to and from, so the
view never jumps and the player can always skip.

## What this chapter teaches

- How to author a camera flight as keys of time and a `Shot`.
- How to fly between keys with a spline and per-key easing.
- How to cut, skip and hand the view back without a jump.
- How to frame an object up close and point at parts of it.
- How to record a clip from a replay at any size and frame rate.

## Outline

1. **Keys and flights.** A `Flight` holds up to `FLIGHT_KEYS` (16) keys.
   `verse_flight_key` adds a key at a time after the last one, with an eye,
   a look point, a cut flag and an easing: `EASE_LINEAR`, `EASE_SMOOTH`,
   `EASE_IN` or `EASE_OUT`. Keys out of order are refused.
2. **Flying through keys.** Positions follow a Catmull-Rom spline through
   the keys (`fl_spline`), eased per key (`fl_ease`). A cut jumps to its
   key. `verse_flight_take` reports the current take, so the host changes
   the scene behind a cut.
3. **Skip and hand back.** A key or a touch skips the flight. At the end the
   view blends to the player's live `Shot` over `HANDBACK` (0.9 s). With
   reduced motion, or with no keys, the flight ends at once. A free orbit
   takes the last shot with `verse_orbit_toward`.
4. **Close-ups.** `Arrange` frames an object at `ARRANGE_DIST` (1.05 m),
   looking down `ARRANGE_PITCH` (0.6 rad) into its opening. It blends in and
   out over `ARRANGE_IN` (0.7 s), facing the way the world camera faced, so
   the blend moves in rather than swinging round. A pointer ray
   (`verse_shot_ray`) places a stem (`ARRANGE_PLACE`) or takes one out
   (`ARRANGE_TAKE`), up to `ARRANGE_STEMS` (24).
5. **One blend rule.** Aethyr gives every producer the same blend: a
   frame-rate-independent ease from chapter 10, and a check that the eye
   never moves more than a stated distance in one frame.
6. **Clips from replays.** The recorder draws one fixed step a frame, so a
   slow machine never makes a clip stutter. Staged moments (`?stage=NAME`)
   set up a scene from a seed. A recorded session from
   [chapter 5](ch005-replay-a-session-exactly.md) re-renders a moment at any
   size, angle or frame rate. Each clip has a sidecar label: source, game,
   commit, stage or replay ID, seed, size and frame rate.
7. **Camera paths for clips.** The planned items in
   [clips.md](../../docs/verse/clips.md): inputs on a timeline, orbit and
   dolly paths, slow motion by smaller steps, and 9:16 and square crops from
   one run.

## What exists today

- [aethyr/cinematic.coax](../../aethyr/cinematic.coax): flights, keys,
  easing, cuts, skip and hand-back, with `#test`s.
- [aethyr/arrange.coax](../../aethyr/arrange.coax): the close-up arranging
  view, used by [apps/openagents-web/garden.coax](../../apps/openagents-web/garden.coax).
- [apps/openagents-web/villa.coax](../../apps/openagents-web/villa.coax):
  Hecate's Cemetery's arrival flight.
- [aethyr/camera.coax](../../aethyr/camera.coax): `Shot` and the orbit.
- [coax/web/clip.coax](../../coax/web/clip.coax) and
  [docs/verse/clips.md](../../docs/verse/clips.md): the clip recorder and
  staged moments.
- [coax/web/og-render.ts](../../coax/web/og-render.ts): link-preview cards
  shot from stages with `&card=1`.

## What it builds

- `aethyr/shot/flight.coax`: authored flights as `Shot` producers.
- `aethyr/shot/closeup.coax`: close-ups with pointing.
- `aethyr/shot/blend.coax`: the shared blend and its per-frame bound.
- `aethyr/tools/clip.coax`: clips from replays and stages.

## Acceptance checks and oracles

The chapter is done when:

- `cinematic.coax`'s and `arrange.coax`'s checks pass on the Aethyr modules.
- Hecate's arrival flight hands the camera over with no frame-to-frame eye
  move above the chapter 10 bound, at 30, 60 and 144 FPS.
- Skipping at any frame ends in the live `Shot` within `HANDBACK`.
- Reduced motion ends every flight at once.
- A clip rendered twice from the same replay gives identical frames.

## Dependencies

Chapters 1 (the clock), 5 (replay), 10 (the `Shot` and its easing), 14
(figures drawn in close-ups) and 15 (skip and reduced-motion controls).
