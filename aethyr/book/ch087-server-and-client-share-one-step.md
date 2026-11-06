# 87. Server and client share one step and one world

> **Status: partly implemented.** The page and oa-room run the same Coax
> movement step on the same input
> ([everglade-step.coax](../../apps/openagents-web/everglade-step.coax),
> #12706), the same SRD rules unit
> ([everglade5e-room-rules.coax](../../apps/openagents-web/everglade5e-room-rules.coax)),
> and, since commit 5f7d50e2d1, load the same world document, `everglade`, in
> every setup (#12773). Talking, doors and boats now run through room intents
> (`room-everglade.coax`); quest-board stages remain page-owned.

The server owns the scene, but a client still has to move its own player
before the server answers (chapter 90). That prediction is correct only if
the client runs exactly the server's code on exactly the server's data. This
chapter adds that rule: one step, one set of rules, one world source.

## What this chapter teaches

- Why shared code is a correctness requirement, not a convenience.
- One movement step called in one order by both programs.
- One world document, loaded by both programs from one key.
- What goes wrong when either is duplicated.

## The design

Write the simulation once in Coax and link it into both programs:

- The page compiles it to Wasm and calls it to predict its own player.
- The server compiles it to C and calls it for every seat.

Both call the step in the same order on the same input and the same world:

1. `everglade_move_world` gives the world where the player stands.
2. `eg_step_begin` applies a throw, the input, the pace and low gravity.
3. `eg_step_advance` moves the controller among the world's bodies.
4. `everglade_move_after` applies lifts, currents and the city's edge.

<!-- source: apps/openagents-web/everglade-step.coax shared-step -->
```jai
// Before the advance: a blast's throw into your velocity; held or busy, no input of yours; your input;
// prone or caught, still; the pace and grip; the camera's yaw; a fork's low gravity while you're up.
eg_step_begin :: (p0: walk.Player, input: EgMoveIn, w: EgMoveWorld, dt: f32) -> walk.Player #export_c {
  p := p0;
  if w.kx != 0 || w.ky != 0 || w.kz != 0 {
    p.vel.x += w.kx; p.vel.z += w.kz;
    if w.ky > 0 { p.vel.y = ifx w.ky > p.vel.y then w.ky else p.vel.y; p.on_ground = false; }
  }
  if w.busy || w.still { p = walk.walk_input(p, 0, 0, 0, 0, false, false, 0, 0); }
  else { p = walk.walk_input(p, input.key_x, input.key_y, input.pad_x, input.pad_y, input.run, input.jump && w.jump_ok, 0, 0); }
  p.pace = w.pace; p.grip = w.grip;
  p.theta = input.theta;
  if !p.on_ground { p.vel.y += w.low_grav * dt; }
  return p;
}
```

The input crosses the wire as whole hundredths (`wire_axis`), so the page
steps the input as the wire carried it (`net_as_sent`), not the float it
started with. Otherwise the two programs step values that differ in the last
bits, and a long walk ends in different places.

### One world source

The world document is chapter 51's realm document: the placed things, their
colliders and the edits. If the page and the server load different
documents, the step is the same but its world is not. Before #12773 the room
read `world/everglade-5e` and the page read `everglade`. Locally the page got
nothing, so a crate placed at (0, −82.51) stood only on the server: holding W
walked you in place, with a correction every update.

The rule: both programs read one document key, from one place, in every
setup. Locally, `bin/room --dev` serves the same document the page reads
(`coax/web/room/local.coax`).

## Run it

```shell
coax/bin/coax test apps/openagents-web/everglade-step.coax --only "step:"
```

```text
ok   #test step: the server's word through the wire's tenth of a millisecond: every re-run steps as the server did (1 case, 1143494 steps, 96 ms)
ok   #test step: the server's word on the input just stepped (nothing to re-run) leaves the drawn feet where they were (1 case, 166481 steps, 6 ms)
5 of 295 checks (--only 'step:'), 5 passed, 0 failed
```

The output is cut to its last lines; the first three checks pass too.

## What is left

- More of the shared step and world adapters can move behind Aethyr
  interfaces (#12845). Keep native and worker observations separate from
  source-sharing claims.

## Acceptance checks

- The same recorded inputs stepped by the page's Wasm and the server's C
  reach the same position at every step (the `step:` checks).
- The drift check's position probe (chapter 100) agrees offline and in a
  room, including at the crate.

## Dependencies

Chapters 9 (the controller), 51 (the realm document) and 86 (the server).
