# 150. Put generated motion in Verse

> **Status: partly implemented.** Motion that chapter 149's generator makes
> toward a destination, on synthetic weights, is converted to the Verse
> character's convention, retargeted through an explicit joint map onto the
> 65-joint skeleton of Verse's `Cast.bin`, written into a Cast's dance slot
> and played by `verse/cast.coax`, fading in from the idle and out to the
> walk. Still to do: motion from the real MotionBricks weights (chapter 149
> runs only its codebook on them), a map for the whole body rather than one
> leg, writing the clip into the shipped `Cast.bin` on a device, a frame of
> the web Grid playing it, a deadline in wall-clock time, and the Unitree G1
> as its own embodiment.

[Chapter 149](ch149-execute-motionbricks.md) generates motion tokens and
decodes them into a clip. This chapter hands that clip to a Verse
character. You name a destination; Psionic generates a clip toward it,
converts it to what Verse expects, and writes it where Verse's character
system ([`verse/cast.coax`](../../verse/cast.coax)) plays clips from. The
code is [`motion-verse.coax`](../lib/motion-verse.coax), in five steps:

- **the target**: Verse's skeleton and its conventions, read from the real
  `Cast.bin`;
- **the joint map**: which generated joint drives which Verse joint, refused
  when a joint is left out, used twice, out of range or under the wrong
  parent;
- **the handoff**: chapter 147's conversion and retargeting, then one
  skinning transform a joint a frame, in `Cast.bin`'s layout;
- **the splice**: the transforms written into a Cast's dance slot, which
  Verse plays and cross-fades with its other clips;
- **the checks on the result**: joint positions against chapter 147's
  forward kinematics, the continuity check of chapter 148, and a foot-contact
  check.

Everything is binary64 until the rows are written as the F32 numbers
`Cast.bin` stores (profile `cpu-f64-motion-verse-v1`). A handoff has at most
32 frames and 65 joints, the most Verse's character has.

## Run it

```shell
coax/bin/coax run psionic/examples/motion-verse.coax
coax/bin/coax run psionic/examples/motion-verse-refuse.coax
coax/bin/coax test psionic/lib/motion-verse.coax
coax/bin/coax mutate psionic/lib/motion-verse.coax --root-only
```

The skeleton fixture is rebuilt and compared with the shipped `Cast.bin` by
a tooling script:

```shell
python3 -I psionic/tests/verse_cast_fixture.py
```

## The target

`Cast.bin` is the file [`verse/tools/pack_cast.py`](../../verse/tools/pack_cast.py)
packs and `verse/cast.coax` reads on iOS and the web. It holds the
character's 65 joints (a parent and a bind position each), seven clips
(idle, walk, sprint, the three parts of a jump, and a dance) and, per frame
and joint, a **skinning transform**: a rotation q and a translation t that
move a point of the character's bind pose to where that frame shows it,
v ↦ q v + t. The bind pose has no rotations of its own; it is the positions
alone.

The program cannot open files, so
[`verse_cast_fixture.py`](../tests/verse_cast_fixture.py) copies the
skeleton and the first frame of the idle clip, as F32 bit patterns, into
[`verse-cast-skeleton.coax`](../fixtures/verse-cast-skeleton.coax), with the
SHA-256 of the `Cast.bin` it read. Joint 0 is a root on the ground; joint 1
is the hips, 0.949 m up; the spine, neck and head are 2 to 6; the arms 7 to
54; the left leg 55 to 59 (thigh, knee, ankle, ball, toe) at x = +0.114 m;
the right leg 60 to 64. The character faces +Z, so +X is its left.

<!-- source: psionic/lib/motion-verse.coax rig -->
```jai
// The Verse character's skeleton, read exactly from the fixture: each
// joint's parent and its bind position, in metres, Y up, facing +Z.
MvRig :: struct {count:int;parent:[MV_JOINTS]int;bind:[MV_JOINTS][3]f64;}
mv_verse_rig :: ()->MvRig {
    r:MvRig;r.count=MV_CAST_JOINTS;
    for j:0..<MV_CAST_JOINTS {
        r.parent[j]=MV_CAST_PARENT[j];
        for i:0..<3 {r.bind[j][i]=f64(f32_from_bits(MV_CAST_BIND_BITS[3*j+i]));}
    }
    return r;
}
// Cast.bin's clips: Y up, +Z forward, right-handed, quaternions x, y, z, w,
// metres, absolute roots, 30 frames a second (verse/cast.coax's FPS).
mv_verse :: ()->PoseConvention {
    return PoseConvention.{up=PoseAxis.pos_y,forward=PoseAxis.pos_z,hand=PoseHand.right,order=PoseOrder.xyzw,unit=PoseUnit.metre,root=PoseRoot.absolute,rate=30};
}
```

`verse/cast.coax` samples clips at 30 frames a second (`FPS`), turns the
character by a yaw about +Y with yaw 0 facing +Z, and stores quaternions as
x, y, z, w. Chapter 149's clips are in chapter 147's glTF convention, which
differs only in the quaternion order (w first). Converting between the two
is chapter 147's `pose_convert`; the handoff calls it rather than swapping
components itself, so a source in any declared convention (Z up,
centimetres, 60 frames a second) arrives the same way.

## The joint map

Chapter 149's small generator is set here to decode four joints: a pelvis,
a hip 0.1 m to its side, a knee 0.4 m below and an ankle 0.42 m below that
(`mv_leg`). It drives the Verse character's left leg: pelvis to the hips
(1), hip to the thigh (55), knee to 56 and ankle to 57.

<!-- source: psionic/lib/motion-verse.coax map -->
```jai
// Source joint j drives Verse joint target[j]; -1 is no target. Every
// source joint needs one, no two share one, and a joint's parent must map
// to its target's parent, so local rotations carry over unchanged. Verse
// joints no source joint maps to keep their bind pose.
MvMap :: struct {count:int;target:[POSE_JOINTS]int;}
MvCheck :: struct {status:MvStatus;joint:int;}
mv_map_check :: (m:MvMap,from:PoseSkeleton,rig:MvRig)->MvCheck {
    out:MvCheck;out.joint= -1;
    if pose_skeleton_check(from)!=PoseStatus.ok || rig.count<1 || rig.count>MV_JOINTS || m.count!=from.count {out.status=MvStatus.shape;return out;}
    for j:0..<from.count {
        t:=m.target[j];out.joint=j;
        if t== -1 {out.status=MvStatus.unmapped;return out;}
        if t<0 || t>=rig.count {out.status=MvStatus.range;return out;}
        for i:0..<j {if m.target[i]==t {out.status=MvStatus.twice;return out;}}
        if j>0 && rig.parent[t]!=m.target[from.parent[j]] {out.status=MvStatus.parent;return out;}
    }
    out.joint= -1;
    return out;
}
// The mapped part of the Verse skeleton as a chapter 147 skeleton: the
// source's joints and parents with Verse's bone offsets. The root's offset
// is zero: the clip's root translation is the root joint itself.
mv_sub :: (m:MvMap,from:PoseSkeleton,rig:MvRig)->PoseSkeleton {
    k:=from;k.offset[0]=[3]f64.[0,0,0];
    for j:1..<from.count {t:=m.target[j];p:=rig.parent[t];for i:0..<3 {k.offset[j][i]=rig.bind[t][i]-rig.bind[p][i];}}
    return k;
}
```

The map is checked joint by joint, parents first, and the first bad joint
is named. A source joint with no target is `unmapped`; a target outside 0 to
64 is `range`; a target used twice is `twice`; and a joint whose source
parent does not map to its target's parent is `parent`. The last rule is
what lets a joint's local rotation carry over unchanged: the knee turns
relative to the thigh in both skeletons. The root may map to any joint;
here the hips, whose parent, the ground root, keeps its bind pose.

Verse joints no source joint drives keep their bind pose relative to their
parent. They are not left behind: they ride on their parents, so the right
leg and the spine follow the hips. Because the bind pose is a T-pose, the
arms stay straight out.

The mapped part of the Verse skeleton becomes a chapter 147 skeleton
(`mv_sub`): the source's four joints and parents, with Verse's bone offsets,
bind position minus parent bind position. By hand, from the fixture:

| Bone | Offset (m) | Length (m) |
| --- | --- | ---: |
| hips to thigh | (0.114, 0.022, 0.007) | 0.1163 |
| thigh to knee | (0, −0.429, 0) | 0.4290 |
| knee to ankle | (0, −0.456, −0.051) | 0.4588 |

The chain from the hips to the ankle is 1.0042 m on Verse's leg and
0.1 + 0.4 + 0.42 = 0.92 m on the source's.

## The handoff

<!-- source: psionic/lib/motion-verse.coax handoff -->
```jai
// A generated clip handed to Verse: the clip in Verse's convention,
// retargeted and in place; the ground travel taken out of it; and per
// frame and Verse joint the skinning transform Cast.bin stores, rotation
// q (w, x, y, z) and translation t, so that a bind-pose point v moves to
// q v + t.
MvHandoff :: struct {
    status:MvStatus;detail:PoseStatus;joint:int;
    sub:PoseSkeleton;clip:PoseClip;joints:int;frames:int;
    travel:[MV_FRAMES][2]f64;q:[MV_FRAMES][MV_JOINTS][4]f64;t:[MV_FRAMES][MV_JOINTS][3]f64;
}
// 1. Check the map. 2. Convert the clip and the source skeleton to
// Verse's convention (chapter 147). 3. Retarget the clip onto the mapped
// Verse bones, scaling the root by the
// chain lengths up from `foot`. 4. Put it in place: the root keeps its
// height, its ground position goes to `travel` and it stands over its
// target's bind position. 5. Pose every Verse joint, parents first:
//   R_k = R_parent L_k,  P_k = P_parent + R_parent (bind_k - bind_parent),
// with L_k the mapped joint's rotation or the identity, the root target at
// the clip's root, and a joint without a parent at its bind position.
// Cast.bin's row is then q = R_k and t = P_k - R_k bind_k.
mv_handoff :: (c:PoseClip,from:PoseSkeleton,m:MvMap,rig:MvRig,foot:int)->MvHandoff {
    out:MvHandoff;check:=mv_map_check(m,from,rig);out.status=check.status;out.joint=check.joint;
    if check.status!=MvStatus.ok {return out;}
    converted:=pose_convert(c,from,mv_verse());out.detail=converted.status;
    if converted.status==PoseStatus.ok && converted.clip.frames>MV_FRAMES {out.status=MvStatus.length;return out;}
    out.sub=mv_sub(m,from,rig);
    if converted.status==PoseStatus.ok {
        bones:=pose_convert_skeleton(from,c.convention,mv_verse());
        retargeted:=pose_retarget(converted.clip,bones.skeleton,out.sub,foot);out.detail=retargeted.status;out.clip=retargeted.clip;
    }
    if out.detail!=PoseStatus.ok {out.status=MvStatus.pose;return out;}
    root:=m.target[0];out.joints=rig.count;out.frames=out.clip.frames;
    for f:0..<out.frames {
        out.travel[f]=[2]f64.[out.clip.frame[f].root[0],out.clip.frame[f].root[2]];
        out.clip.frame[f].root[0]=rig.bind[root][0];out.clip.frame[f].root[2]=rig.bind[root][2];
    }
    work:=pose_internal(out.clip,from.count);
    for f:0..<out.frames {
        local:[MV_JOINTS][4]f64;for k:0..<rig.count {local[k]=[4]f64.[1,0,0,0];}
        for j:0..<from.count {local[m.target[j]]=work.frame[f].rotation[j];}
        r:[MV_JOINTS][4]f64;p:[MV_JOINTS][3]f64;
        for k:0..<rig.count {
            up:=rig.parent[k];
            if k==root {r[k]=local[k];p[k]=work.frame[f].root;}
            else if up<0 {r[k]=local[k];p[k]=rig.bind[k];}
            else {
                d:=pose_rotate(r[up],[3]f64.[rig.bind[k][0]-rig.bind[up][0],rig.bind[k][1]-rig.bind[up][1],rig.bind[k][2]-rig.bind[up][2]]);
                r[k]=pfk_multiply(r[up],local[k]);for i:0..<3 {p[k][i]=p[up][i]+d[i];}
            }
            b:=pose_rotate(r[k],rig.bind[k]);
            out.q[f][k]=r[k];for i:0..<3 {out.t[f][k][i]=p[k][i]-b[i];}
        }
    }
    return out;
}
// Where a joint is: its skinning transform (q, t) applied to its bind
// position.
mv_world :: (q:[4]f64,t:[3]f64,bind:[3]f64)->[3]f64 {
    b:=pose_rotate(q,bind);
    return [3]f64.[b[0]+t[0],b[1]+t[1],b[2]+t[2]];
}
```

Retargeting is chapter 147's `pose_retarget`: the local rotations stay and
the root translation scales by the ratio of the two chains,
r = 1.0042 / 0.92 = 1.0915, so a pelvis 0.9 m up becomes hips 0.982 m up.
Retargeting by copying rotations assumes the two skeletons' bones point the
same way at rest; here both legs hang straight down, the shin of Verse's
within 6.4°.

Verse places the character at the walking controller's position every
frame, so a clip that also moved the root over the ground would walk the
character away from where the controller says it is. The handoff keeps the
root's height, stands it over the hips' bind position, and keeps the ground
travel it took out in `travel`, where the foot check below can add it back.

Then every Verse joint is posed, parents first, and its row is
q = R and t = P − R bind. A clip at rest shows what that means. With every
rotation the identity and the pelvis 0.9 m up, the hips stand at
0.9 × 1.0915 = 0.9823 m, and every joint from the hips down moves by the
same lift, 0.9823 − 0.9489 = 0.0334 m: its row is the identity rotation and
t = (0, 0.0334, 0). The ground root, joint 0, keeps t = 0. A quarter turn of
the knee about +X sends the shin's bind offset (0, −0.456, −0.051) to
(0, 0.051, −0.456), (x, y, z) ↦ (x, −z, y), so the ankle swings back and up
in front of the knee by that much; the ball, which nothing drives, turns
with it.

## Generation under a budget

<!-- source: psionic/lib/motion-verse.coax scene -->
```jai
// A destination becomes chapter 149's condition: standing poses at both
// ends, as mb_walk; the root path a straight line from the origin to
// (x, z), facing along it, and the matching ground velocity.
mv_toward :: (s:MbShape,n:int,x:f64,z:f64)->MbCond {
    c:=mb_walk(s,n,0);frames:=n*MB_FPT;d:=sqrt(x*x+z*z);
    for t:0..<frames {
        u:=f64(t)/f64(frames);
        c.root[t]=[4]f64.[x*u,z*u,ifx d>0 then z/d else 1,ifx d>0 then x/d else 0];
        c.velocity[t]=[2]f64.[x/f64(frames),z/f64(frames)];
    }
    return c;
}
// The most frames the clip may have and the most network passes (one a
// confidence step, and one decoder pass) generation may take before its
// deadline. Both are checked before any work: a refused scene changes
// nothing, so the character keeps the clip it plays.
MvBudget :: struct {frames:int;passes:int;}
MvScene :: struct {status:MvStatus;tokens:MbTokens;clip:MbClip;handoff:MvHandoff;}
mv_scene :: (s:MbShape,predictor:[]f64,decoder:[]f64,n:int,steps:int,x:f64,z:f64,budget:MvBudget,from:PoseSkeleton,m:MvMap,rig:MvRig,foot:int)->MvScene {
    out:MvScene;
    if n*MB_FPT>budget.frames {out.status=MvStatus.length;return out;}
    if steps+1>budget.passes {out.status=MvStatus.deadline;return out;}
    c:=mv_toward(s,n,x,z);g:=mb_generate(s,predictor,n,c,steps);out.tokens=g.tokens;
    out.clip=mb_clip(s,mb_decode(s,decoder,mb_small_codebook(s),g.tokens,c));
    if g.status!=MbStatus.ok || out.clip.status!=MbStatus.ok {out.status=MvStatus.shape;return out;}
    out.handoff=mv_handoff(out.clip.clip,from,m,rig,foot);out.status=out.handoff.status;
    return out;
}
```

A destination (x, z) becomes chapter 149's condition: a straight root path
from the origin, facing along it, and standing poses at both ends. Toward
(3, 4) over 6 tokens (24 frames), frame 12 is at (1.5, 2), facing
(cos, sin) = (0.8, 0.6), with a ground velocity of (0.125, 1/6) m a frame.

A scene has a budget: the most frames the slot it plays in holds, and the
most network passes before its deadline, one a confidence step and one for
the decoder. Both are checked before any work, so a refused scene changes
nothing and the character keeps playing what it played. The deadline is
counted in passes, not seconds, so it is the same on every machine.

Generation here is deterministic: tokens are the most confident codes, never
sampled, so the same destination and weights give the same poses bit for
bit. The synthetic weights are chapter 149's (predictor seed 1, decoder seed
3 at 1/16 scale). They are not trained: the condition reaches the networks,
but nothing makes the motion go to the destination.

## Feet

<!-- source: psionic/lib/motion-verse.coax contact -->
```jai
// A foot joint touches the ground in a frame when it is at most `band`
// above `level`. It is refused below level - band, and when it moves more
// than `slide` over the ground between two touching frames; ground
// positions add the travel taken out of the clip back in.
MvContact :: struct {status:MvStatus;frame:int;touching:int;lowest:f64;lowest_frame:int;slid:f64;slid_frame:int;}
mv_contact :: (h:MvHandoff,rig:MvRig,joint:int,level:f64,band:f64,slide:f64)->MvContact {
    out:MvContact;out.frame= -1;out.slid_frame= -1;out.lowest_frame= -1;out.lowest=POSE_LENGTH_MAX;
    if h.status!=MvStatus.ok || joint<0 || joint>=h.joints {out.status=MvStatus.shape;return out;}
    last:[3]f64;was:=false;
    for f:0..<h.frames {
        p:=mv_world(h.q[f][joint],h.t[f][joint],rig.bind[joint]);p[0]+=h.travel[f][0];p[2]+=h.travel[f][1];
        if p[1]<out.lowest {out.lowest=p[1];out.lowest_frame=f;}
        if p[1]<level-band && out.frame== -1 {out.status=MvStatus.below;out.frame=f;}
        on:=p[1]<=level+band;
        if on {out.touching+=1;}
        if on && was {
            d:=sqrt((p[0]-last[0])*(p[0]-last[0])+(p[2]-last[2])*(p[2]-last[2]));
            if d>out.slid {out.slid=d;out.slid_frame=f;}
            if d>slide && out.frame== -1 {out.status=MvStatus.slide;out.frame=f;}
        }
        last=p;was=on;
    }
    return out;
}
```

Feet are where retargeted motion most often shows its faults: a foot that
should stand still skates over the ground, or sinks into it. The check
puts the travel back to get the foot's place on the ground, then calls a
foot touching when it is within a band of the floor. Between two touching
frames it may move at most `slide`; below the floor by more than the band
it is refused. The floor here is the ankle's bind height, 0.086 m.

## Into Cast.bin

<!-- source: psionic/lib/motion-verse.coax splice -->
```jai
// Write the handoff into the dance slot of an admitted Cast, in place: its
// rows over the dance's first frames (F32, x, y, z, w then the
// translation), then the dance's frame count. A Cast that is not admitted,
// a different joint count or more frames than the slot holds is refused
// and the bytes are left as they were.
mv_splice :: (b:[]u8,h:MvHandoff)->MvStatus {
    c:=verse_cast_open(b);
    if !c.ok {return MvStatus.admission;}
    if h.status!=MvStatus.ok || h.joints!=c.joints {return MvStatus.shape;}
    if h.frames>frames_of(b,c,Move.dancing) {return MvStatus.length;}
    first:=first_of(b,c,Move.dancing);
    for f:0..<h.frames {for k:0..<h.joints {
        at:=c.poses.offset+((first+f)*c.joints+k)*32;q:=h.q[f][k];
        putf(b,at,f32(q[1]));putf(b,at+4,f32(q[2]));putf(b,at+8,f32(q[3]));putf(b,at+12,f32(q[0]));
        for i:0..<3 {putf(b,at+16+4*i,f32(h.t[f][k][i]));}
    }}
    put32(b,c.clips.offset+cast(int)cast(u8)Move.dancing*8+4,h.frames);
    return MvStatus.ok;
}
```

The splice writes into the dance slot because the dance is the clip
`verse/cast.coax` plays on request (`verse_cast_dance`) while you stand,
fading 0.18 s into it, and moving or jumping fades it back out. Nothing in
Verse changes: the Cast is the same format, admitted by the same
`verse_cast_open`, and only the slot's frames and its frame count differ.
The splice refuses a Cast that is not admitted, a handoff of a different
joint count, or more frames than the slot holds, and then writes nothing.

The program builds a Cast of its own around the fixture (`mv_cast_build`):
the real skeleton, the real idle's first frame for every clip but the
dance, and the least mesh and robot admission accepts. The dance slot holds
30 frames, the length of the shipped dance.

## The scene

The [example](../examples/motion-verse.coax) runs the whole path: the map,
a scene toward (0, 1) m, the checks on its result, the splice, and Verse
playing it.

<!-- source: psionic/examples/motion-verse.coax map -->
```jai
    rig:=mv_verse_rig();leg:=mv_leg();map:=mv_leg_map();
    print("Verse skeleton: % joints; hips at % mm up, left thigh %, knee %, ankle % at % mm up\n",rig.count,mvx_mm(rig.bind[1][1]),
        rig.parent[56],rig.parent[57],57,mvx_mm(rig.bind[57][1]));
    check:=mv_map_check(map,leg,rig);part:=mv_sub(map,leg,rig);
    print("map pelvis 1, hip 55, knee 56, ankle 57: %\n",check.status);
    for j:1..<4 {print("bone %: (% % %) mm\n",j,mvx_mm(part.offset[j][0]),mvx_mm(part.offset[j][1]),mvx_mm(part.offset[j][2]));}
    print("chains to the ankle (mm): source %, Verse %\n",mvx_mm(pose_chain_length(leg,3)),mvx_mm(pose_chain_length(part,3)));
```

<!-- source: psionic/examples/motion-verse.coax scene -->
```jai
    s:=mv_shape();predictor:[MB_WEIGHTS_MAX]f64;decoder:[MB_WEIGHTS_MAX]f64;
    mb_synthetic(predictor[..],mb_predictor_plan(s).count,1,1);mb_synthetic_decoder(s,decoder[..],3,0.0625);
    budget:=MvBudget.{frames=30,passes=8};
    scene:=mv_scene(s,predictor[..],decoder[..],7,4,0,1,budget,leg,map,rig,3);h:=scene.handoff;
    print("scene toward (0, 1) m, 7 tokens, 4 steps: %, % frames at % fps\n",scene.status,h.frames,h.clip.convention.rate);
    print("frame 27: generated root (mm) % % %, Verse hips height %, travel % %\n",mvx_mm(scene.clip.clip.frame[27].root[0]),
        mvx_mm(scene.clip.clip.frame[27].root[1]),mvx_mm(scene.clip.clip.frame[27].root[2]),mvx_mm(h.clip.frame[27].root[1]),
        mvx_mm(h.travel[27][0]),mvx_mm(h.travel[27][1]));
    worst:=0.0;
    for f:0..<h.frames {w:=pose_forward(h.sub,h.clip,f);for j:0..<4 {worst=mb_larger(worst,mvx_far(mv_world(h.q[f][map.target[j]],h.t[f][map.target[j]],rig.bind[map.target[j]]),w.position[j]));}}
    print("Verse joints against chapter 147's forward kinematics: every frame within 1 nm: %\n",worst<=0.000000001);
    jump:=mt_continuity(h.clip,h.sub,10*POSE_PI/180,0.1);
    print("continuity: %, worst turn (millidegrees) % at frame %\n",jump.status,mvx_mm(jump.angle*180/POSE_PI),jump.turn_frame);
    foot:=mv_contact(h,rig,57,rig.bind[57][1],0.05,0.02);
    print("left ankle: %, touching % of % frames, lowest % mm, worst slide % mm at frame %\n",foot.status,foot.touching,h.frames,
        mvx_mm(foot.lowest),mvx_mm(foot.slid),foot.slid_frame);
```

<!-- source: psionic/examples/motion-verse.coax play -->
```jai
    bytes:[MVX_BYTES]u8;n:=mv_cast_build(bytes[..],30);b:=bytes[0..<n];
    before:=mv_digest(b);put:=mv_splice(b,h);c:=verse_cast_open(b);
    print("Cast of % bytes: splice %, admitted %, dance % frames, digest % (was %)\n",n,put,c.ok,frames_of(b,c,Move.dancing),mv_digest(b),before);
    again:=mv_scene(s,predictor[..],decoder[..],7,4,0,1,budget,leg,map,rig,3);copy:[MVX_BYTES]u8;m:=mv_cast_build(copy[..],30);
    copied:=mv_splice(copy[0..<m],again.handoff);
    print("the same destination and weights again: %, digest %\n",copied,mv_digest(copy[0..<m]));
    a:Anim;a=verse_cast_update(b,c,a,0,false,0);a=verse_cast_dance(a);a=verse_cast_update(b,c,a,0,false,1);
    print("at 1 s: %, fading from %\n",a.move,a.from);
    out:[MV_JOINTS]Joint;mvx_pose(b,c,a,1.5,out[..]);
    far:=0.0;for k:0..<rig.count {far=mb_larger(far,mvx_far(mvx_v(moved(out[k],v3(f32(rig.bind[k][0]),f32(rig.bind[k][1]),f32(rig.bind[k][2])))),mv_world(h.q[15][k],h.t[15][k],rig.bind[k])));}
    print("at 1.5 s Verse draws dance frame 15: every joint within 10 um of the handoff: %\n",far<=0.00001);
    hips:[3]f64;
    for i:0..<3 {mvx_pose(b,c,a,1+0.09*f64(i),out[..]);hips[i]=f64(moved(out[1],v3(f32(rig.bind[1][0]),f32(rig.bind[1][1]),f32(rig.bind[1][2]))).y);}
    print("hips height (mm) at 1, 1.09 and 1.18 s, into, halfway and out of the fade: % % %\n",mvx_mm(hips[0]),mvx_mm(hips[1]),mvx_mm(hips[2]));
    a=verse_cast_update(b,c,a,1,false,2);
    print("walking at 2 s: %, fading from %\n",a.move,a.from);
```

<!-- source: psionic/examples/motion-verse.coax render -->
```jai
    a0:Anim;mvx_pose(b,c,a0,0,out[..]);mvx_draw("idle, front view:",out[..],rig);
    d:=Anim.{move=Move.dancing,start=0,from=Move.idle,from_start=0,fade_start= -1};
    mvx_pose(b,c,d,0.5,out[..]);mvx_draw("dance frame 15, front view:",out[..],rig);
```

```text
Verse skeleton: 65 joints; hips at 949 mm up, left thigh 55, knee 56, ankle 57 at 86 mm up
map pelvis 1, hip 55, knee 56, ankle 57: ok
bone 1: (114 22 7) mm
bone 2: (0 -429 0) mm
bone 3: (0 -456 -51) mm
chains to the ankle (mm): source 920, Verse 1004
scene toward (0, 1) m, 7 tokens, 4 steps: ok, 28 frames at 30 fps
frame 27: generated root (mm) -421 899 616, Verse hips height 982, travel -460 672
Verse joints against chapter 147's forward kinematics: every frame within 1 nm: true
continuity: ok, worst turn (millidegrees) 66 at frame 1
left ankle: slide, touching 28 of 28 frames, lowest 120 mm, worst slide 29 mm at frame 25
Cast of 76784 bytes: splice ok, admitted true, dance 28 frames, digest 2772531910722619262 (was 17199024032912752677)
the same destination and weights again: ok, digest 2772531910722619262
at 1 s: dancing, fading from idle
at 1.5 s Verse draws dance frame 15: every joint within 10 um of the handoff: true
hips height (mm) at 1, 1.09 and 1.18 s, into, halfway and out of the fade: 908 945 982
walking at 2 s: walking, fading from dancing
idle, front view:
  ........................
  ........................
  ........................
  ........................
  ...........#............
  ...........#............
  .......#..#.#....#......
  ........................
  ...........#............
  ......#.....#......#....
  ........................
  ....#.......#...........
  ....##...#..#.#...###...
  ....##............###...
  ........................
  ........................
  ........................
  .......#........#.......
  ........................
  ........................
  ........................
  ........................
  .......#.........#......
  ...#.#......#...........
dance frame 15, front view:
  ........................
  ........................
  ........................
  ...........#............
  ...........##...........
  #.....#..........#.....#
  ........................
  ...........#............
  ...........#............
  ........................
  ...........#............
  .........#..#.#.........
  ........................
  ........................
  ........................
  ........................
  .........#....#.........
  ........................
  ........................
  ........................
  ........................
  ........................
  .........#.....#........
  .........#..#..#........
```

The numbers follow from the hand calculation. Frame 27's generated pelvis is
at (−0.421, 0.899, 0.616) m; times 1.0915 that is hips 0.982 m up and a
travel of (−0.460, 0.672) m. The four mapped joints agree with chapter 147's
`pose_forward` on the mapped skeleton to within 1 nm in every frame. The
continuity check passes: no joint turns more than 0.066° between frames.

The foot check does not pass, and should not. Standing straight, the ankle
is 0.982 + 0.022 − 0.429 − 0.456 = 0.119 m up, 33 mm above its bind height
and inside the 50 mm band, so it touches in all 28 frames. Meanwhile the
untrained root drifts about 0.81 m in 28 frames, 29 mm a frame at its
worst, and the leg barely moves: the foot skates. That is what the check is
for, and what trained weights have to fix.

Verse plays the clip. At 1 s the dance is asked for and fades in from the
idle. At 1.5 s Verse's own `verse_cast_pose` draws dance frame 15, and every
one of the 65 joints is within 10 µm of the handoff (the rows are F32). The
hips are 0.908 m up in the idle and 0.982 m in the dance; halfway through
the fade they are at 0.945 m, the average. Walking at 2 s fades the dance
out into the walk. The digest, FNV-1a over the dance's rows, is the same
when the scene runs again.

The two front views are a render check in text: each `#` is a joint of the
palette Verse would hand the vertex shader, in 4 cm cells, the character's
left on the right. In the dance frame the arms are in the bind T-pose,
since the map drives one leg.

## Refusals

The [refusal example](../examples/motion-verse-refuse.coax) breaks one
thing at a time.

<!-- source: psionic/examples/motion-verse-refuse.coax refuse -->
```jai
main :: () {
    rig:=mv_verse_rig();leg:=mv_leg();
    m:=mv_leg_map();m.target[3]= -1;r:=mv_map_check(m,leg,rig);print("ankle without a target: %, source joint %\n",r.status,r.joint);
    m=mv_leg_map();m.target[3]=65;r=mv_map_check(m,leg,rig);print("ankle to joint 65: %, source joint %\n",r.status,r.joint);
    m=mv_leg_map();m.target[2]=55;r=mv_map_check(m,leg,rig);print("knee to the thigh again: %, source joint %\n",r.status,r.joint);
    m=mv_leg_map();m.target[2]=61;r=mv_map_check(m,leg,rig);print("knee to the right knee: %, source joint %\n",r.status,r.joint);
    s:=mv_shape();predictor:[MB_WEIGHTS_MAX]f64;decoder:[MB_WEIGHTS_MAX]f64;
    mb_synthetic(predictor[..],mb_predictor_plan(s).count,1,1);mb_synthetic_decoder(s,decoder[..],3,0.0625);
    bytes:[MVR_BYTES]u8;n:=mv_cast_build(bytes[..],30);b:=bytes[0..<n];before:=mv_digest(b);
    budget:=MvBudget.{frames=30,passes=8};
    long:=mv_scene(s,predictor[..],decoder[..],8,4,0,1,budget,leg,mv_leg_map(),rig,3);
    late:=mv_scene(s,predictor[..],decoder[..],7,8,0,1,budget,leg,mv_leg_map(),rig,3);
    print("8 tokens (32 frames) for a 30-frame slot: %; 8 steps and a decode for 8 passes: %\n",long.status,late.status);
    print("splice of the refused scene: %, the dance unchanged: %\n",mv_splice(b,late.handoff),mv_digest(b)==before);
    ok:=mv_scene(s,predictor[..],decoder[..],7,4,0,1,budget,leg,mv_leg_map(),rig,3);
    short:[MVR_BYTES]u8;k:=mv_cast_build(short[..],20);
    print("28 frames into a 20-frame dance: %\n",mv_splice(short[0..<k],ok.handoff));
    bytes[0]=0;print("a Cast with a damaged magic number: %\n",mv_splice(b,ok.handoff));
}
```

```text
ankle without a target: unmapped, source joint 3
ankle to joint 65: range, source joint 3
knee to the thigh again: twice, source joint 2
knee to the right knee: parent, source joint 2
8 tokens (32 frames) for a 30-frame slot: length; 8 steps and a decode for 8 passes: deadline
splice of the refused scene: shape, the dance unchanged: true
28 frames into a 20-frame dance: length
a Cast with a damaged magic number: admission
```

The ankle mapped to nothing, to joint 65 of a 65-joint skeleton, the knee to
the thigh the hip already has, and the knee to the right knee (whose
parent, the right thigh, is not the left thigh the hip maps to) are each
refused at the joint named. Eight tokens are 32 frames, more than the
30-frame slot; eight steps and the decoder are 9 passes, past a deadline of
8. A refused scene's handoff is refused by the splice, and the dance is
unchanged. A 28-frame clip does not fit a 20-frame dance, and a damaged Cast
is not admitted.

## Checks

[motion-verse.coax](../lib/motion-verse.coax) holds 16 checks of its own, with their hand-built clips and handoffs in
[motion-verse-checks.coax](../tests/motion-verse-checks.coax);
loading it also runs those of chapters 147 to 149 and of `verse/cast.coax`.

- **The target.** The skeleton's parents come before their children, the
  left leg's joints and bind positions are those read off `Cast.bin` to the
  millimetre, and the convention is declared in full at 30 frames a second,
  equal to `verse/cast.coax`'s `FPS`.
- **The map.** Every refusal at the joint it names, including the root
  without a target and a rig too small for a target; the right leg maps as
  the left does; the source root on the ground root moves the hips with it,
  and an unmoved ground root stays at its bind position even off the origin;
  the mapped skeleton's offsets by hand.
- **By hand.** The clip at rest (identity rows and one lift), the quarter
  turn of the knee, the destination's root path, and foot tracks that touch,
  slide by exact amounts, sink and are planted again by their travel, with
  each limit tested on the line.
- **Forward kinematics.** 8 clips, half in the Z-up, centimetre, 60-frame
  convention, each joint turned about one of chapter 147's fixed axes:
  every mapped Verse joint's position and rotation equal `pose_forward` on
  the mapped skeleton within 10⁻⁹, and every other joint keeps its bone
  length and turns with its parent.
- **Verse.** A splice writes x, y, z, w and the translation as `verse/cast.coax`
  reads them; `verse_cast_pose` draws each dance frame within 10 µm of the
  handoff; the fade starts at the idle, ends at the dance, keeps each bone's
  length between the idle's and the dance's halfway, and fades out to the
  walk. Refused splices leave the bytes, and the digest covers the dance's
  rows and nothing else.
- **Scenes.** Refused before any work at one token or one pass past the
  budget; the same request gives the same rows; other decoder weights give
  other rows.

`coax mutate --root-only` on the unit leaves zero survivors.

`verse/cast.coax` changed in one place: a helper its checks use, `turn`,
is now `yaw_turn`, so the file loads beside chapter 147's units, whose
checks name a variable `turn`.

## What this chapter does not do

- **Real-weight motion.** The clip comes from chapter 149's synthetic
  weights. Until the root predictor and the real decoder and predictor run,
  the motion does not go where it is sent and its feet skate.
- **The whole body.** One leg is mapped; the other joints keep the bind
  pose. SOMA's 30 joints would need their own map, and chapter 149 does not
  reconstruct SOMA's skeleton yet.
- **The shipped file and the web.** The clip is spliced into a Cast built
  around the real skeleton, not into the shipped `Cast.bin`, and no frame of
  the web Grid shows it: the web Grid loads its Cast as a fixed asset and
  has no way to take a new clip, so the render check here is the joint
  palette Verse computes.
- **Wall-clock deadlines.** The deadline counts network passes. Holding a
  frame-time deadline needs the passes timed on the device.
- **The Unitree G1** is a robot with its own skeleton, joint limits and
  balance; it is a separate embodiment and profile, not a second target for
  this map.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md).
- [Chapter 147](ch147-poses-have-conventions.md): conventions, conversion,
  retargeting and forward kinematics.
- [Chapter 148](ch148-motion-tokens-become-poses.md): the continuity check.
- [Chapter 149](ch149-execute-motionbricks.md): MotionBricks execution.
- The Aethyr engine's figures and animation
  ([Aethyr chapter 14](../../aethyr/book/ch014-figures-and-animation.md)).
- [`verse/cast.coax`](../../verse/cast.coax) and
  [`verse/tools/pack_cast.py`](../../verse/tools/pack_cast.py): the Cast
  format and how Verse plays it.
