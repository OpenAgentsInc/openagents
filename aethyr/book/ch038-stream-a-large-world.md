# 38. Stream a large world

> **Status: unimplemented — design draft.** Today a realm loads its whole
> asset pack on entry and keeps every object resident; Everglade chooses its
> near props by distance with hysteresis, and the space camera keeps a
> floating origin in f64. This chapter plans Aethyr's streaming: a world in
> cells, loaded and simulated by interest, within declared budgets. Nothing
> here runs yet.

A realm the size of a town fits in memory; a realm the size of a continent,
or a solar system, does not. Streaming decides which parts of the world are
loaded, which are simulated at full rate, which are drawn in detail, and
which are only known to exist, and it changes those decisions as the player
moves without a hitch, a pop or a different simulation result.

## What this chapter teaches

- World cells and their content-hashed packs.
- Interest: distance rings for loading, simulating and drawing, with
  hysteresis.
- A floating origin, so far positions stay precise.
- Sleeping and waking physics at a cell's edge.
- Loading on a budget, ahead of need, without blocking a frame.

## The method

Cells are squares (or cubes) of side `L` keyed by integer coordinates
`c = ⌊x / L⌋`. Each cell has a state, and the player's position sets the
target state by ring:

```jai
target(c) = draw     if d(c) ≤ R_draw
          = simulate if d(c) ≤ R_sim
          = resident if d(c) ≤ R_load
          = absent
R_draw ≤ R_sim ≤ R_load
```

A cell changes state only when the target has differed for a movement of
`h` metres (hysteresis), so walking along a ring's edge does not thrash.
Loads are requested at `R_load + v · t_load`, the distance the player can
cover in the measured load time, and taken against chapter 16's pack-byte
and memory budgets; a refused load is reported and retried, never half
applied.

Positions are stored in f64 world metres or as `(cell, offset)` pairs; the
camera subtracts its own centre in f64 before anything reaches f32, so a
hull plate and a planet's orbit draw with the same camera.

A cell leaving `simulate` puts its bodies to sleep with their state saved;
entering wakes them from that state. Bodies near the edge belong to exactly
one cell (by centre of mass), and contacts across an edge are resolved by
the cell that owns the lower ID, so the result does not depend on load
order.

## What exists today

- [apps/openagents-web/everglade-lod.coax](../../apps/openagents-web/everglade-lod.coax):
  small props drawn within `LOD_NEAR` 52 m, shrubs within `LOD_FAR` 72 m, a
  phone at 0.7 of those, re-chosen only after the eye moves `LOD_STEP`
  2.5 m.
- [verse/spacecam.coax](../../verse/spacecam.coax): a floating-origin
  camera in f64 with zoom over twelve decades, checked by "a hull at 5 AU
  draws to the pixel".
- [verse/rigid.coax](../../verse/rigid.coax): island sleep (`SLEEP_LINEAR`,
  `SLEEP_TIME`) that streaming reuses at cell edges.
- [coax/web/host.js](../../coax/web/host.js): one streamed pack per page with
  progress messages (605), and later assets (sounds, particles, fonts)
  fetched after the first frame.

## The design in Aethyr

A world manifest lists its cells, each with a pack digest, bounds and
neighbour links. The stream controller runs once per step on the
interpolated player position (chapter 1), computes targets, and issues
loads and state changes; the loader runs asynchronously and hands finished
cells to the next step boundary, so a cell appears between steps, never
mid-step. Entity IDs (chapter 6) are stable across unload and reload. The
renderer draws only `draw` cells; physics (Part IV) steps only `simulate`
cells, with batched worlds (chapter 31) unaffected.

## What it builds

- `aethyr/world/cells.coax`: cell keys, manifests and states.
- `aethyr/world/stream.coax`: interest rings, hysteresis and load requests.
- `aethyr/world/origin.coax`: floating-origin positions shared by
  rendering, physics and queries.

## Acceptance checks and oracles

- Determinism: the same session replayed (chapter 5) with different load
  timings (fast, slow, out of order) gives bit-identical simulation states.
- No pops: walking a ring's edge back and forth within `h` changes no cell
  state.
- Precision: an object 10⁹ m from the origin draws within one pixel of its
  exact projection.
- A planted load over the pack budget is refused and reported, and the
  frame rate stays within its target while loads run.
- Sleeping a cell and waking it restores its bodies exactly.

## Dependencies

Chapters 1, 2, 5, 6, 7, 16, and Part IV (chapters 17–22 for bodies and
contact, 31 for batched worlds, 36 for recordings). From Coax: R12 units and
frames, R15 deterministic jobs, R16 replay and snapshots.
