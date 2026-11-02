# 14. Figures and animation

> **Status: unimplemented — design draft.** Today three animation paths
> exist side by side: the Grid's cast (`verse/cast.coax`), skinned rigs
> (`aethyr/skin.coax`) and rounded-bone figures with checked motions
> (`aethyr/figure.coax`, `aethyr/motion.coax`). This chapter plans one Aethyr
> pose pipeline that all three feed. Nothing here runs yet.

A character on screen is a skeleton, a pose per frame and something drawn
from it: a skinned mesh or a body of rounded bones. Every path in the Verse
already agrees on the essentials (local joint motion blended per joint,
then chained from the root so bones keep their length) but each does it in
its own file. Aethyr makes the pose the shared value and the drawing a
choice.

## What this chapter teaches

- A rig: joints parents first, rest transforms and inverse binds.
- Sampling a clip: linear translation and slerped rotation between keys.
- Cross-fades blended per joint before the parents' transforms accumulate.
- Gaits advanced by distance travelled, so feet keep pace with the ground.
- Motions as small checked documents with joint limits that refuse, never
  bend.
- Skinning palettes for the vertex shader.

## The method

For joint `j` with parent `p(j)`, a clip gives local translation `t_j` and
rotation `q_j` at time `τ`. A cross-fade from pose A to B blends locally:

```jai
t_j = lerp(t_j^A, t_j^B, s),  q_j = slerp(q_j^A, q_j^B, s),  s = smoothstep(0, T, age)
G_j = G_{p(j)} · [q_j | t_j]          // root first
palette_j = G_j · inverse_bind_j      // three rows of four to the shader
```

Blending world transforms instead would shorten bones mid-fade. A gait's
phase advances by distance, `φ += |Δx| / loop_metres`, and any other clip
runs on the clock. Clip choice follows movement: airborne jumps, below
0.1 m/s stands, below 5 m/s walks, faster runs.

## What exists today

- [verse/cast.coax](../../verse/cast.coax): the Grid's character and robots
  from `Cast.bin` (packed by `verse/tools/pack_cast.py` from aethyr-cpp
  assets). `verse_cast_open` admits every buffer, index, joint and frame;
  seven clips cross-fade over 0.18 s; robot capsules
  (`verse_cast_bodies`) are placed with the same palette that draws them.
- [aethyr/skin.coax](../../aethyr/skin.coax): rigs and clips as the old Rust
  engine played them, `SKIN_FADE` 0.22 s, distance-driven gaits, and
  `skin_palette`; Everglade's Ranger is its first user.
- [aethyr/figure.coax](../../aethyr/figure.coax): person and four-legged rigs
  of round cones, shapes set by an agent or a slider, gaits worked out from
  a clock, and `verse_figure_limit` per joint.
- [aethyr/motion.coax](../../aethyr/motion.coax): motions as text (`motion`,
  `rig`, `seconds`, keys with joint angles), each turn checked against its
  joint's limits and refused with the line and the allowed range.

## The design in Aethyr

```jai
Rig  :: struct { joints: int; parent: [MAX] s16; rest: [MAX] Xform; inv_bind: [MAX] M34; }
Pose :: struct { local: [MAX] Xform; }            // what clips and motions write
pose_sample(rig, clip, τ) -> Pose
pose_blend(a, b, s) -> Pose                       // per joint, local
pose_world(rig, pose) -> [MAX] Xform              // root first
```

Clips, gaits and motion documents all write a `Pose`; layering a motion
over a gait replaces only the joints its keys name. Drawing then takes
`pose_world` either to a skinning palette (skinned meshes) or to round-cone
bones (figures). Collision bodies come from the same world transforms, so
what a player bumps is what is drawn. Animation runs on the interpolated
state (chapter 1) and never steps physics.

## What it builds

- `aethyr/anim/rig.coax`, `aethyr/anim/clip.coax`, `aethyr/anim/pose.coax`:
  rigs, sampling, blending and world transforms.
- `aethyr/anim/motion.coax`: the motion document parser and limit checks,
  moved from `verse/`.
- `aethyr/anim/draw.coax`: palettes and round-cone placement.

## Acceptance checks and oracles

- Bone lengths are constant to 1e-5 m through every cross-fade and motion
  layer.
- The Grid's cast, Everglade's Ranger and Dead Gears' figures produce the
  same palettes as their current modules over a recorded clip sequence
  (exact, or within f32 rounding where the old path differs only in order).
- Foot slip during a walk, measured as stance-foot world speed, stays below
  a stated bound.
- Every motion document that the current parser refuses is still refused,
  with the same line and range.
- Mutation: 0 survivors in pose blending and limit checks.

## Dependencies

Chapters 1 (interpolated state), 2 (local and world spaces), 6 (entities
that own a rig) and 9 (the movement that chooses clips). From Coax: R12
units and frames for joint angles and positions.
