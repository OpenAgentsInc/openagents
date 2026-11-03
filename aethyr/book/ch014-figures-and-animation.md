# 14. Figures and animation

> **Status: partly implemented.** The pose moved into
> [aethyr/anim/pose.coax](../anim/pose.coax): the rotations, a cross-fade
> blended joint by joint, the skeleton chained from the root and the
> skinning palette. [aethyr/skin.coax](../skin.coax) plays its clips through
> it, bit for bit as before. The Grid's cast (`verse/cast.coax`) and the
> rounded-bone figures (`aethyr/figure.coax`) still pose their joints in
> their own files (see [Still to do](#still-to-do)).

A character on screen is a skeleton, a pose each frame and something drawn
from it: a skinned mesh or a body of rounded bones. Every path in the Verse
agrees on the essentials: local joint motion blended per joint, then chained
from the root so bones keep their length. Aethyr makes the pose the shared
value and the drawing a choice.

## What this chapter teaches

- A joint's local pose, and a cross-fade blended joint by joint before any
  parent's transform.
- The posed skeleton, chained from the root, and the palette the vertex
  shader reads.
- Gaits advanced by distance, so feet keep pace with the ground.
- Motions with joint limits that refuse, never bend.

## A cross-fade, joint by joint

When the clip changes, the character blends from the pose it left to the
new clip's over a fade (`SKIN_FADE`, 0.22 s, unless the character sets its
own). The blend happens on each joint's local pose, before its parents'
transforms accumulate:

<!-- source: aethyr/anim/pose.coax blend -->
```jai
// How far a cross-fade has gone `age` seconds into one of `fade` seconds: smoothstepped, so it
// starts and ends gently, and 1 from its end on.
pose_fade :: (age: f32, fade: f32) -> f32 {
  f := clamp01(age / fade);
  return f * f * (3 - 2 * f);
}
// One joint of a cross-fade from pose a to pose b, `s` of the way: blended locally, before any
// parent's transform, the translation along the straight line and the rotation along the shorter
// arc. Blending the posed (world) transforms instead would shorten bones halfway through.
pose_blend :: (a: SkinLocal, b: SkinLocal, s: f32) -> SkinLocal {
  return SkinLocal.{t = mix(a.t, b.t, s), q = slerp(a.q, b.q, s)};
}
```

The translation moves along the straight line between the two poses and the
rotation along the shorter arc (`slerp`, as the old Rust engine's glam
library did it). Blending the posed, world-space transforms instead would
pull a hand straight toward its new place and shorten the arm halfway
through.

## The skeleton and its palette

<!-- source: aethyr/anim/pose.coax chain -->
```jai
// Joint j's rotation gq[j] and place gt[j] in the character's space, from its local pose l and
// its parent p's, already chained: joints come parents first, so the root is done before its
// children. A root (p below 0), or a joint whose parent doesn't come before it, is where its local
// pose puts it.
pose_chain :: (gq: [] Quat, gt: [] Point, p: int, j: int, l: SkinLocal) {
  if p < 0 || p >= j { gq[j] = l.q; gt[j] = l.t; }
  else { gq[j] = quat_unit(quat_mul(gq[p], l.q)); gt[j] = add(gt[p], quat_rotate(gq[p], l.t)); }
}

// Joint j's skinning matrix into out[j * 12 ..]: three rows of four, its posed rotation's rows
// and place times its inverse bind (bind[j * 12 ..], three rows of four), for the vertex shader.
pose_palette :: (q: Quat, t: Point, bind: [] f32, j: int, out: [] f32) {
  ex := quat_rotate(q, v3(1, 0, 0)); ey := quat_rotate(q, v3(0, 1, 0)); ez := quat_rotate(q, v3(0, 0, 1));
  b := j * 12;
  for row: 0..<3 {
    m0 := ifx row == 0 then ex.x else ifx row == 1 then ex.y else ex.z;
    m1 := ifx row == 0 then ey.x else ifx row == 1 then ey.y else ey.z;
    m2 := ifx row == 0 then ez.x else ifx row == 1 then ez.y else ez.z;
    m3 := ifx row == 0 then t.x else ifx row == 1 then t.y else t.z;
    for col: 0..<4 {
      v := m0 * bind[b + col] + m1 * bind[b + 4 + col] + m2 * bind[b + 8 + col];
      if col == 3 { v += m3; }
      out[b + row * 4 + col] = v;
    }
  }
}
```

Joints come parents first, so `pose_chain` can place each joint from its
parent's place in one pass from the root. `pose_palette` then writes each
joint's skinning matrix, its posed rotation and place times its inverse
bind, as three rows of four for the vertex shader. `skin_palette` in
[aethyr/skin.coax](../skin.coax) is now these two calls for each joint, and
`skin_advance` blends with `pose_fade` and `pose_blend`. Everglade's Ranger
and people and the 5E lab's bodies play through them, and their call sites
didn't change.

The clip that plays comes from the movement (`skin_motion`): in the air
jumps, below 0.1 m/s stands, below 5 m/s walks, and faster runs. A gait, a
clip that travels, advances by the distance moved, `time += speed · dt /
metres · seconds`, so the feet keep pace with the ground. Any other clip runs
on the clock.

## A worked example

Everglade's Ranger has 75 joints and eight clips. Its walk is a loop of
1.333 s that travels 1.3 m; its run is a loop of 0.933 s that travels 5 m.
Work these out by hand:

1. **The fade's share.** 0.1 s into a fade of 0.22 s, a = 0.1 / 0.22 =
   0.4545. Smoothstepped, a²(3 − 2a) = 0.2066 · 2.0909 = 0.4320.
2. **A walk by distance.** One second at 1.3 m/s moves 1.3 m, one whole
   loop, so the walk is back at its start: 0 s into the clip.
3. **A run by distance.** Half a second at 6 m/s moves 3 m, 3/5 of a loop:
   0.6 · 0.933 = 0.56 s into the clip.
4. **A jump on the clock.** The jump doesn't travel, so half a second is
   0.5 s into it.
5. **The clip for a speed.** 0.05 m/s is below 0.1, so the Ranger stands;
   1.3 m/s walks; 6 m/s runs; in the air it jumps.

The example [aethyr/examples/pose-a-ranger.coax](../examples/pose-a-ranger.coax)
plays the Ranger through a recorded sequence at 60 frames a second: stand
for half a second, walk at 1.3 m/s for a second, run at 6 m/s, jump, and
stand. Each frame's palette goes into one digest. It also checks every bone
against the two poses it is blended between, and reads a motion that turns a
shoulder past its limit:

<!-- source: aethyr/examples/pose-a-ranger.coax example -->
```jai
// FNV-1a over 32-bit words, kept in 32 bits.
mix32 :: (h: int, v: int) -> int { return ((h ^ (v & 4294967295)) * 16777619) & 4294967295; }

// The bone from joint j's parent to j in the pose now, and its length.
bone :: (r: SkinRig, s: [] Skin, j: int) -> f32 {
  d := sub(skin_joint(r, s, j), skin_joint(r, s, r.parent[j]));
  return length(d);
}

main :: () {
  d: [1] Ranger;
  ranger_load(d[..]);
  r := ranger_rig(d[..]);
  s: [1] Skin;
  // The recorded sequence: stand half a second, walk at 1.3 m/s, run at 6, jump, then stand.
  clips := [5] int.[SKIN_IDLE, SKIN_WALK, SKIN_RUN, SKIN_JUMP, SKIN_IDLE];
  speeds := [5] f32.[0, 1.3, 6, 6, 0];
  frames := [5] int.[30, 60, 30, 30, 30];
  palette: [RANGER_JOINTS * 12] f32;
  h := 2166136261;
  n := 0;
  worst := 0.0f32; worst_frame := 0; worst_joint := 0;
  for k: 0..<5 {
    for f: 0..<frames[k] {
      skin_advance(r, s[..], clips[k], speeds[k], 1.0 / 60.0);
      skin_palette(r, s[..], palette[..]);
      for i: 0..<palette.count { h = mix32(h, cast(int) f32_bits(palette[i])); }
      n += 1;
      if k == 1 && f == 6 {
        // 0.1 s into the walk: the fade's share, smoothstepped
        a := (s[0].clock - s[0].changed) / SKIN_FADE;
        print("0.1 s into the walk: % of the fade, a share of %\n", r4(a), r4(a * a * (3 - 2 * a)));
      }
      // Each bone against the same bone in the clip's own pose at this time: a fade blends
      // each joint locally, so a bone keeps the length the two poses give it.
      for j: 1..<r.joints {
        target := length(skin_sample(r, s[0].clip, j, s[0].time).t);
        from := length(s[0].from[j].t);
        lo_len := ifx target < from then target else from;
        hi_len := ifx target > from then target else from;
        b := bone(r, s[..], j);
        off := ifx b < lo_len then lo_len - b else ifx b > hi_len then b - hi_len else 0.0f32;
        if off > worst { worst = off; worst_frame = n; worst_joint = j; }
      }
    }
    faded := (s[0].clock - s[0].changed) / SKIN_FADE;
    print("after % frames of % at % m/s: % s into the clip, its fade % done\n", frames[k], RANGER_CLIP_NAME[clips[k]], speeds[k], r4(s[0].time), ifx faded >= 1 then "all" else "not");
  }
  print("% frames, % joints: palette digest %\n", n, r.joints, h);
  print("bones outside their two poses' lengths: at most % m (frame %, joint %)\n", worst, worst_frame, worst_joint);
  // A motion that turns a joint past its limit is refused with the line and the range, not bent.
  m: [1] Motion;
  ok := verse_motion_read(m[..], "motion wave; rig person; seconds 2; once; at 0.5; right_shoulder 0 0 -250");
  print("a wave past the shoulder's limit read %: %\n", ok, verse_motion_error(m[..]));
  print("clip for 0.05 m/s %, 1.3 m/s %, 6 m/s %, in the air %\n", RANGER_CLIP_NAME[skin_motion(0.05, false)], RANGER_CLIP_NAME[skin_motion(1.3, false)], RANGER_CLIP_NAME[skin_motion(6, false)], RANGER_CLIP_NAME[skin_motion(1.3, true)]);
}
```

```shell
coax/bin/coax run aethyr/examples/pose-a-ranger.coax
```

```text
after 30 frames of idle at 0.0 m/s: 0.5 s into the clip, its fade all done
0.1 s into the walk: 0.4545 of the fade, a share of 0.432
after 60 frames of walk at 1.3 m/s: 0.0 s into the clip, its fade all done
after 30 frames of run at 6.0 m/s: 0.56 s into the clip, its fade all done
after 30 frames of jump at 6.0 m/s: 0.5 s into the clip, its fade all done
after 30 frames of idle at 0.0 m/s: 0.5 s into the clip, its fade all done
180 frames, 75 joints: palette digest 3351927923
bones outside their two poses' lengths: at most 2.3841858e-07 m (frame 38, joint 17)
a wave past the shoulder's limit read false: line 6: right_shoulder roll is outside -190..40
clip for 0.05 m/s idle, 1.3 m/s walk, 6 m/s run, in the air jump
```

The digest is the independent check of the move. It was recorded with
`aethyr/skin.coax` as it was before the pose moved out of it, and it is the
same after, in debug and release builds. Each bone stays within its two
poses' lengths to 2.4 × 10⁻⁷ m, one rounding step of a 32-bit float at that
size, through every fade.

The wave is the broken case. Its sixth line turns the right shoulder's roll
to −250°, outside the joint's −190° to 40°, and
[aethyr/motion.coax](../motion.coax) refuses the whole motion with the line
and the allowed range rather than bending the turn to fit.

## Checks

```shell
coax/bin/coax test aethyr/anim/pose.coax --library
coax/bin/coax mutate aethyr/anim/pose.coax --library --root-only
coax/bin/coax test aethyr/skin.coax --library
coax/bin/coax mutate aethyr/skin.coax --library --root-only
```

`aethyr/anim/pose.coax` has nine checks of its own: the quaternion
arithmetic worked on small numbers (Hamilton's product of (1, 2, 3, 4) and
(5, 6, 7, 8) is (24, 48, 48, −6)), quarter turns about each axis, slerp's
length, short way, ends and both of its branches at their edges, the fade's
share, one joint's blend, the chain (a root, a child turned with its parent,
and a parent that comes after its joint), and the palette's rows against a
hand-worked bind. `coax mutate` kills all 119 of its mutants.

`aethyr/skin.coax` keeps its clip, key, gait, fade, skinning and turn checks,
with one more: a character's own fade length replaces `SKIN_FADE`. Its
changed lines have no surviving mutants. 87 older survivors remain elsewhere
in `skin.coax`, in its bounds and key lookup; the file had 112 before this
change.

## Still to do

- **The Grid's cast.** `verse/cast.coax` reads the Grid's character and
  robots from `Cast.bin` and blends its seven clips over 0.18 s with its own
  `nlerp` and quaternion type. Moving it onto `pose.coax` must keep the
  Grid's recorded CPU, WebGPU and Vulkan frame identical.
- **Figures and motions.** The rounded-bone figures (`aethyr/figure.coax`)
  and motion documents (`aethyr/motion.coax`) pose their joints from pitch,
  yaw and roll in their own code. Writing a `Pose` from them, with a motion
  layered over a gait replacing only the joints its keys name, remains.
- **Foot slip.** A measured bound on a stance foot's world speed during a
  walk is not recorded yet.
- **The rest of `skin.coax`'s mutants.** The 87 survivors above.
- **Collision from the pose.** The Grid's robot capsules come from the same
  palette that draws them (`verse_cast_bodies`); the same for skinned
  characters through `pose_chain` is still to do.

## Dependencies

Chapters [1](ch001-a-frame-has-a-fixed-step.md) (interpolated state),
[2](ch002-spaces-and-units.md) (local and world spaces),
[6](ch006-entities-are-born-and-die.md) (entities that own a rig) and
[9](ch009-a-character-moves.md) (the movement that chooses clips). From Coax:
R12 units and frames for joint angles and positions.
