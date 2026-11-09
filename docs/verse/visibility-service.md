# Visibility service

GitHub #13073. Engine module: `aethyr/visibility.coax`.

Health bars showed through hills and walls because each zone asked "can I see
that?" in its own way, or not at all. The visibility service gives every
caller one answer: name plates, health bars, damage numbers, targeting and
monster sight.

## The question

You ask whether an eye sees a target. The eye is any point: the camera, or a
monster's eyes. The target is one point, or a vertical run of up to four
points, such as a body from its feet to its head. A target is visible when
any of its points is visible, so a body half behind a low wall still shows
its plate.

```coax
q := VisQuery.{eye = camera_eye, target = VisTarget.{at = feet, rise = 1.8, points = 3}, range = 40};
v := vis_line(colliders, land, q);
if v.status == VisStatus.visible { /* draw the plate */ }
```

Each answer is a `Visibility`:

- `status`: `visible`, `occluded`, `out_of_range`, `offscreen` or `invalid`.
  `invalid` is the zero value, so an answer that nobody filled in never
  reads as visible.
- `source`: `line` (CPU) or `depth` (GPU depth image).
- `blocker`: what hid the first hidden point: `terrain`, `collider` or
  `depth`.
- `seen` and `tested`: how many of the target's points were seen and tested.
  A HUD can fade a plate by `seen / tested`.

## Sources

### CPU line: `vis_line`

`vis_line(colliders, land, query)` tests each target point in two steps:

1. The land. If the zone's ground is a height function, not a mesh, pass a
   terrain profile (`VisTerrain`). `vis_terrain_count(q)` says how many
   heights the line needs: one a metre of flat distance, at most 64.
   `vis_terrain_point(q, k)` says where to sample height `k`. Every point of
   a vertical target stands over the same flat path, so one profile serves
   them all. A line hides when it passes more than `VIS_TERRAIN_SKIN` (5 cm)
   under a sampled height.
2. The colliders. The service calls `query_clear` from `aethyr/query.coax`,
   so boxes, capsules, heightfield meshes and the flat ground all hide.

A hit within the target's `slack` (30 cm by default) of a point is the
target's own body, such as its capsule or the ground at its feet, and does
not hide it. A zone whose land is a `QueryHeightfield` mesh passes an empty
`VisTerrain` and lets the colliders hold the land.

### GPU depth: `vis_depth`

`vis_depth(image, x, y, depth, bias, radius)` compares a target's pixel and
depth with a depth image that the renderer read back. The image is WebGPU's
0..1 depth, standard (`depthCompare: "less"`) or reversed. The target is
visible when any pixel within `radius` holds nothing nearer than its depth
minus `bias`. The radius lets a target at a wall's edge show; the bias lets
the target's own surface pass. `vis_project` gives a world point's pixel and
depth through a column-major view-projection matrix, and
`vis_perspective_depth` gives a standard perspective's depth for a view
distance.

The depth image is pixel-exact for everything the renderer draws, including
foliage and props that have no collider. A read-back image is one or two
frames old, so a caller uses it for presentation (plates, bars, numbers) and
keeps `vis_line` for gameplay (targeting, monster sight), which must not
depend on what the GPU drew.

The web host does not yet copy depth back to the page; until it does,
callers use `vis_line`. The depth path is complete and tested on the CPU so
that the host change only has to supply the image.

## Flicker: `vis_settle`

A plate at a wall's edge can switch every frame. `vis_settle(memo, answer,
hold)` keeps a shown or hidden state, and changes it only after `hold`
answers in a row disagree. An `invalid` answer changes nothing. Out of range
and off screen count as hidden.

## Layers

| Layer | Owns | Files |
| --- | --- | --- |
| Engine | The query, both sources, the settle state | `aethyr/visibility.coax` |
| Game | The world's colliders and land for a line | `verse/experimental-dungeon.coax` (`dg_sight`), `verse/experimental-village.coax` |
| Zone | Wiring: which eye, which target, which hold | the zone's HUD and combat files |

Zones only wire the service in. A zone never walks its own line.

## Rules

- Gameplay (targeting, monster sight, line of effect) uses `vis_line` only.
- Presentation may use `vis_depth` when an image is available, else
  `vis_line`.
- A caller that draws a plate keeps one `VisMemo` per plate and settles every
  answer.
- A query that the colliders refuse answers `invalid`. A caller treats
  `invalid` as "keep the last state", never as visible.
