# 56. The Grid

> **Status: unimplemented — design draft.** The Grid runs today on its own
> private code: the bare world in [verse/grid.coax](../../verse/grid.coax)
> and the homepage's scene walked at `/realms/the-grid`
> ([newland-walk.coax](../../apps/openagents-web/newland-walk.coax)). This
> chapter plans rebuilding it only from Aethyr's earlier chapters. Nothing
> here runs yet.

The Grid is the smallest whole realm: a floor of lines in a black field, a
figure that walks, runs and jumps on it, a camera behind, other people
walking, and stations to walk up to. It is the first realm to rebuild
because it uses almost every foundation chapter and almost nothing else, so
any seam in the foundations shows here first: the floor that shifts as you
move, the jump that stutters, the feet drawn above the ground.

## What this chapter teaches

- Composing a realm from the foundations: loop, spaces, input, queries,
  movement, camera, renderer, HUD, rooms.
- A world-space floor drawn the same from any position.
- Live data in a realm: the homepage's projects, Pylons and people as
  entities with stable IDs.

## The method

- **Time** ([chapter 1](ch001-a-frame-has-a-fixed-step.md)): one clock;
  the controller steps at the fixed rate; drawing interpolates.
- **Space** ([chapter 2](ch002-spaces-and-units.md)): the floor's lines are
  every 4 m (street lines every 24 m) in world metres; the room's grid units
  convert through one named function (today `WALK_SCALE`, half a grid unit a
  metre), never by an inline factor.
- **Input** ([chapter 4](ch004-input-becomes-typed-actions.md)): keys and the
  two thumbs (left half moves, right half looks, a quick tap jumps, the
  hotbar's dance slot) become move, look, jump, run and act.
- **Ground** ([chapters 7](ch007-ask-the-world-where-things-are.md) and
  [8](ch008-ground-and-collision.md)): a flat plane plus the colliders of the
  terminal and the robots; one ground height.
- **Movement and camera** ([chapters 9](ch009-a-character-moves.md) and
  [10](ch010-the-camera-follows.md)): the walking controller and the
  third-person follow over interpolated state.
- **Drawing** ([chapters 11](ch011-the-wgsl-renderer.md),
  [14](ch014-figures-and-animation.md), [15](ch015-ui-and-the-hud.md)):
  screen-space line quads, the cast skinned on the GPU, the HUD (THE GRID,
  the nearest project, HOME, REALMS).
- **People** ([chapter 49](ch049-rooms-and-presence.md)): the playtest room
  `the-grid` sends everyone else's position; each is an entity
  ([chapter 6](ch006-entities-are-born-and-die.md)).

```jai
grid_step :: (w: *GridWorld, actions: Actions) {
    player_step(*w.player, actions, *w.ground);   // chapter 9 over chapter 7's queries
    room_apply(*w.people, w.room_inbox);          // chapter 49
}
grid_draw :: (w: *GridWorld, alpha: f32, out: *Frame) {
    floor_lines(world_from_camera(follow(*w.cam, lerp(w.prev, w.player, alpha))), out);
}
```

## What exists today

- [verse/grid.coax](../../verse/grid.coax): the bare world as aethyr-cpp's
  `grid.cpp` drew it (lines to 264 m, fading from 6 m to 110 m, a ring at
  your feet, the Pylon terminal 4 m ahead) and the touch `Sticks`.
- [apps/openagents-web/newland-walk.coax](../../apps/openagents-web/newland-walk.coax)
  over [newland.coax](../../apps/openagents-web/newland.coax): the
  homepage's places, trees, roads, Pylons and people walked with
  [verse/player.coax](../../verse/player.coax); a click opens the homepage's
  card; the room posts positions about five times a second.
- [verse/tools/pack_grid.py](../../verse/tools/pack_grid.py) packs the demo's
  Grid scene; [verse/cast.coax](../../verse/cast.coax) reads `Cast.bin`.
- Its edit rows (walkers, names) are in
  [realm-edit.md](../../docs/verse/realm-edit.md).
- In the browser it draws through chapter 11's shared renderer: its page
  names the `the-grid` package ([page-render.coax](../render/page-render.coax)),
  and every GPU object has a ledger handle
  ([registry.coax](../render/registry.coax)). The recorded route is the
  homepage's WALK AROUND, 240 frames with two jumps, the camera, replay, and
  a resource refusal and recovery. Its 320×180 final frame matches the CPU
  reference, WebGPU and Vulkan
  ([route record](../../docs/verification/aethyr-grid-playable-route-2026-11-02.md)).
  A lost GPU device keeps its session ([gpu-recovery.coax](../../coax/web/gpu-recovery.coax)).

Its loop, camera smoothing and grid-unit conversion are its own; the floor's
lines are projected per frame relative to the player, the seam behind the
floor that shifts.

## What it builds

- `apps/openagents-web/newland-walk.coax` reduced to the Grid's content and
  HUD, loading `aethyr/` for everything else; `verse/grid.coax`'s private
  sticks and loop deleted.

## Acceptance checks and oracles

- The floor is one consistent world-space grid from any position: a line at
  world `x = 24` m projects to the same place for the same camera whether the
  player stands at the origin or 200 m away (ec-migrate's acceptance).
- The Grid's jump is smooth: no frame-to-frame camera jump above chapter 10's
  bound during walk, run and jump at 60 FPS (ec-camera's acceptance).
- The figure's feet are at its physics height.
- The page's golden (`coax/web/goldens/`) changes only where a fix is
  visible; its checks pass
  (`coax test apps/openagents-web/newland.coax --steps 400000000`).
- A recorded Grid session replays bit-identically natively and in the
  browser.

## Dependencies

Chapters 1–11, 14, 15 and 49.
