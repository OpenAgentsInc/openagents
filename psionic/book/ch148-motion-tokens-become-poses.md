# 148. Motion tokens become poses

> **Status: partly implemented.** A codebook of pose segments, a
> nearest-code encoder, a lookup decoder, a continuity check and masked-token
> filling with a counted bigram model run on the CPU. The masked-token model
> that the plan calls for, an attention model trained by gradients
> (chapters 10, 103 and 104), and a learned decoder are not built yet.
> MotionBricks' own tokenizer and weights are chapter 149's.

A motion model such as MotionBricks does not write joint angles directly. It
writes a short list of whole numbers, called tokens, and a decoder turns each
token into a few frames of poses. This chapter builds both halves at a size
you can check by hand: a codebook whose entries are pose segments, an encoder
that replaces each segment of a clip with its nearest entry, a decoder that
does the reverse, and a model that fills in tokens someone has hidden. A
continuity check refuses decoded motion that jumps.

Everything runs on one CPU core in binary64 on top of
[chapter 147](ch147-poses-have-conventions.md)'s pose profile. The library is
[motion-token.coax](../lib/motion-token.coax). It uses chapter 147's
conventions, quaternion admission, 6D form and working-form conversion, and
chapter 167's quaternion product and matrix. A codebook holds at most 16 codes
of at most 8 frames each, and a clip at most 64 frames, so at most 64 tokens.

## Run it

```shell
coax/bin/coax run psionic/examples/motion-tokens.coax
coax/bin/coax test psionic/lib/motion-token.coax
coax/bin/coax mutate psionic/lib/motion-token.coax --root-only
```

## A code is a pose segment

A codebook declares a chapter 147 convention, a joint count, a segment length
in frames and up to 16 codes. Code e, frame k, holds every joint's local
rotation (w, x, y, z) and the root's *step*: how far the root moved since the
frame before. Steps, not positions, make a code mean "walk forward one
stride" wherever the walker is.

Two frames are compared joint by joint in the 6D form from chapter 147: the
first two columns of the rotation matrix. The 6D form is the same for q and
−q, and it changes smoothly with the rotation, so a squared difference of
6D vectors is a fair measure of how far apart two rotations are. For two
turns about X that differ by an angle d, the first column, (1, 0, 0), does not
move, and the second, (0, cos θ, sin θ), moves along a unit circle by a chord
of length 2 sin(d/2). The squared distance is **4 sin²(d/2)**. The roots add
their squared step difference, times the codebook's root weight.

<!-- source: psionic/lib/motion-token.coax distance -->
```jai
// The 6D form keeps the first two columns of the rotation matrix. It is the
// same for q and -q and changes smoothly, so squared differences of it
// compare two rotations.
mt_turn :: (a:[4]f64,b:[4]f64)->f64 {
    x:=pose_matrix_six(pfk_matrix(a));y:=pose_matrix_six(pfk_matrix(b));
    sum:f64=0;for i:0..<6 {d:=x[i]-y[i];sum+=d*d;}return sum;
}
// Squared distance between two frames of the same joints: every joint's
// turn, plus the weighted squared difference of the roots, which hold steps.
mt_frame_distance :: (a:PoseFrame,b:PoseFrame,joints:int,weight:f64)->f64 {
    sum:f64=0;for j:0..<joints {sum+=mt_turn(a.rotation[j],b.rotation[j]);}
    for i:0..<3 {d:=a.root[i]-b.root[i];sum+=weight*d*d;}
    return sum;
}
// The closest two codes, all frames summed; the lower pair on ties.
mt_separation :: (b:MtCodebook)->MtSeparation {
    out:MtSeparation;out.first= -1;out.second= -1;
    for e:0..<b.codes {for o:e+1..<b.codes {
        d:f64=0;for k:0..<b.segment {d+=mt_frame_distance(b.entry[e][k],b.entry[o][k],b.joints,b.root_weight);}
        if out.first== -1 || d<out.squared {out.first=e;out.second=o;out.squared=d;}
    }}
    out.distance=sqrt(out.squared);
    return out;
}
```

The checks compute the same distance a second way, without matrices, in
[motion-token-checks.coax](../tests/motion-token-checks.coax). With
r = conj(a) · b = (w, x, y, z), the relative rotation's matrix R satisfies
4 − 2(R₀₀ + R₁₁) = 4(x² + y²) + 8z², and that is the squared distance between
the 6D forms of a and b.

A codebook is admitted when its convention is declared in full, its counts
are in range, its root weight is a finite number ≥ 0, every rotation is a
unit quaternion and no two codes are within 2⁻³⁰ of each other. Two codes that
close could not be told apart, so they are refused as `duplicate`.

<!-- source: psionic/lib/motion-token.coax codebook -->
```jai
mt_codebook_check :: (b:MtCodebook)->MtCheck {
    status:=pose_convention_check(b.convention);if status!=PoseStatus.ok {return MtCheck.{status=MtStatus.pose,detail=status};}
    if b.joints<1 || b.joints>POSE_JOINTS || b.codes<1 || b.codes>MT_CODES || b.segment<1 || b.segment>MT_SEGMENT {return MtCheck.{status=MtStatus.shape};}
    status=pose_value(b.root_weight,POSE_LENGTH_MAX);if status==PoseStatus.ok && b.root_weight<0 {status=PoseStatus.range;}
    if status!=PoseStatus.ok {return MtCheck.{status=MtStatus.pose,detail=status};}
    for e:0..<b.codes {for k:0..<b.segment {status=mt_frame_value(b.entry[e][k],b.joints);if status!=PoseStatus.ok {return MtCheck.{status=MtStatus.pose,detail=status};}}}
    // Two codes within the tolerance could not be told apart.
    if mt_separation(b).squared<=POSE_TOLERANCE && b.codes>1 {return MtCheck.{status=MtStatus.duplicate};}
    return MtCheck.{};
}
mt_same_convention :: (a:PoseConvention,b:PoseConvention)->bool {
    return a.up==b.up && a.forward==b.forward && a.hand==b.hand && a.order==b.order && a.unit==b.unit && a.root==b.root && a.rate==b.rate;
}
```

## Encode by hand

Take the smallest useful case: one joint, one frame per token, root weight 1
and three codes.

| Code | Turn about X | Root step along Z |
| --- | --- | --- |
| 0 | 0° | 0 m |
| 1 | 45° | 0.25 m |
| 2 | 90° | 0.5 m |

The clip has three frames at 0°, 50° and 80°, with roots at z = 0, 0.25 and
0.75 m, so its steps are 0.25 m into frame 1 and 0.5 m into frame 2. Frame 0
has no frame before it, so its step is unknown and does not count; the
encoder keeps frame 0's root as the sequence's *origin* instead.

Squared distances, from 4 sin²(d/2) plus the squared step difference:

| Frame | Code 0 | Code 1 | Code 2 | Token |
| --- | --- | --- | --- | --- |
| 0 (0°) | 0 | 4 sin² 22.5° = 0.585786 | 4 sin² 45° = 2 | 0 |
| 1 (50°, 0.25 m) | 4 sin² 25° + 0.25² = 0.776925 | 4 sin² 2.5° = 0.007611 | 4 sin² 20° + 0.25² = 0.530411 | 1 |
| 2 (80°, 0.5 m) | 4 sin² 40° + 0.5² = 1.902704 | 4 sin² 17.5° + 0.25² = 0.424196 | 4 sin² 5° = 0.030384 | 2 |

The tokens are 0, 1, 2. Each token's error is the square root of its winning
distance: 0, 2 sin 2.5° = 0.087239 and 2 sin 5° = 0.174311.

The decoder copies code rotations and adds code steps to the origin: turns of
0°, 45° and 90°, and roots at z = 0, 0.25 and 0.75 m.

### What the round trip can promise

Write these down before running anything:

1. **Codebook members come back exactly.** If a clip was made by decoding a
   token sequence, encoding finds distance 0 (up to the rounding in
   recomputing a step, (r + d) − r), so it recovers the same tokens, and
   decoding them repeats the same arithmetic, so the frames come back bit for
   bit.
2. **The error is at most the size of the change.** If a clip differs from a
   member by distance ε on some token, the nearest code is at most ε away, so
   that token's decoded segment is within ε of the input.
3. **Small changes keep the tokens.** Let D be the smallest distance between
   two codes, the *separation*. If ε < D/2, then by the triangle inequality
   every other code is more than D − ε > ε away, so the original token still
   wins.

For the hand codebook, codes 0 and 1 differ by 4 sin² 22.5° + 0.25² =
0.648286, as do codes 1 and 2; codes 0 and 2 by 2 + 0.25. So
D = √0.648286 = 0.805162 and D/2 = 0.402581. Both nonzero errors above are
below D/2. The library reports the closest pair too, the lower pair on ties;
in binary64, 4 sin² 22.5° computed between 45° and 90° comes out a few units
in the last place smaller than between 0° and 45°, so the example names codes
1 and 2.

On a tie, the lower code wins. The encoder starts from code 0 and replaces
the best only on a strictly smaller distance.

<!-- source: psionic/lib/motion-token.coax encode -->
```jai
// Each segment of the clip becomes the code nearest to it; on a tie the
// lower code wins. The clip must use the codebook's convention exactly:
// convert it with chapter 147's pose_convert first.
mt_encode :: (b:MtCodebook,s:PoseSkeleton,c:PoseClip)->MtTokens {
    out:MtTokens;check:=mt_codebook_check(b);out.status=check.status;out.detail=check.detail;
    if out.status!=MtStatus.ok {return out;}
    out.detail=pose_clip_check(c,s);if out.detail!=PoseStatus.ok {out.status=MtStatus.pose;return out;}
    if !mt_same_convention(c.convention,b.convention) {out.status=MtStatus.convention;return out;}
    if s.count!=b.joints || c.frames%b.segment!=0 {out.status=MtStatus.shape;return out;}
    // Work in w,x,y,z rotations with each root after the first replaced by
    // its step.
    w:=pose_internal(c,s.count);out.sequence.count=c.frames/b.segment;out.sequence.origin=w.frame[0].root;
    for back:1..<c.frames {f:=c.frames-back;for x:0..<3 {w.frame[f].root[x]-=w.frame[f-1].root[x];}}
    for i:0..<out.sequence.count {
        best:= -1;least:f64;
        for e:0..<b.codes {
            // Frame 0 has no step, so its root does not count.
            d:f64;
            for k:0..<b.segment {f:=i*b.segment+k;d+=mt_frame_distance(b.entry[e][k],w.frame[f],b.joints,ifx f>0 then b.root_weight else 0.0);}
            if best== -1 || d<least {best=e;least=d;}
        }
        out.sequence.token[i]=best;out.distance[i]=sqrt(least);
    }
    for i:1..<out.sequence.count {if out.distance[i]>out.distance[out.worst] {out.worst=i;}}
    out.error=out.distance[out.worst];
    return out;
}
```

<!-- source: psionic/lib/motion-token.coax decode -->
```jai
// Codes become frames in order; each root is the one before it plus the
// code's step, starting from the origin.
mt_decode :: (b:MtCodebook,q:MtSequence)->MtClip {
    out:MtClip;check:=mt_codebook_check(b);out.status=check.status;out.detail=check.detail;
    if out.status!=MtStatus.ok {return out;}
    if q.count<1 || q.count*b.segment>POSE_FRAMES {out.status=MtStatus.shape;return out;}
    for x:q.origin {out.detail=pose_value(x,POSE_LENGTH_MAX);if out.detail!=PoseStatus.ok {out.status=MtStatus.pose;return out;}}
    for i:0..<q.count {if q.token[i]<0 || q.token[i]>=b.codes {out.status=MtStatus.token;return out;}}
    w:PoseClip;w.frames=q.count*b.segment;
    for f:0..<w.frames {
        code:=b.entry[q.token[f/b.segment]][f%b.segment];w.frame[f].rotation=code.rotation;
        if f==0 {w.frame[f].root=q.origin;} else {for i:0..<3 {w.frame[f].root[i]=w.frame[f-1].root[i]+code.root[i];}}
    }
    out.clip=pose_external(w,b.joints,b.convention);
    return out;
}
```

The decoder refuses a token outside 0 .. codes − 1 as `token`, a mask
included; a sequence that is empty or longer than 64 frames as `shape`; and
an origin that is not a finite number within 10⁶ as `pose`. The encoder
refuses a clip whose length is not a whole number of segments as `shape`,
a clip that chapter 147 would refuse as `pose` with chapter 147's status in
`detail`, and a clip whose convention differs from the codebook's in any of
its seven fields as `convention`. It does not convert silently; call
chapter 147's `pose_convert` first.

## A walking leg

The example's second codebook moves chapter 147's three-joint leg (hip, knee
and foot, with 0.5 m and 0.4 m bones). A 12-frame cycle turns the hip
30 sin(30u)° about X at phase u and bends the knee 20 (1 − cos 30u)°. Code 0
stands still; codes 1 to 6 each hold two phases of the walk and step the root
0.05 m forward each frame.

<!-- source: psionic/lib/motion-token.coax gait -->
```jai
// A leg walking: phase u of a 12-frame cycle turns the hip 30 sin(30u)
// degrees about X and bends the knee 20 (1 - cos(30u)) degrees; the root
// steps 0.05 m along +Z each frame. Code 0 stands still; code p, from 1 to
// 6, holds phases 2p - 2 and 2p - 1.
MT_GAIT_TURN :: 0.3490658503988659; // 20 degrees
MT_GAIT_STEP :: 0.1;
mt_gait_codebook :: ()->MtCodebook {
    b:MtCodebook;b.convention=pose_gltf();b.joints=3;b.codes=7;b.segment=2;b.root_weight=1;
    for k:0..<b.segment {for j:0..<b.joints {b.entry[0][k].rotation[j]=[4]f64.[1,0,0,0];}}
    for p:1..<b.codes {for k:0..<b.segment {
        u:=f64(2*p-2+k)*POSE_PI/6;
        b.entry[p][k].rotation[0]=mt_x(30*sin(u));b.entry[p][k].rotation[1]=mt_x(20*(1-cos(u)));
        b.entry[p][k].rotation[2]=[4]f64.[1,0,0,0];b.entry[p][k].root=[3]f64.[0,0,0.05];
    }}
    return b;
}
// stand tokens of standing, cycles whole walk cycles, then standing to 16.
mt_gait_sequence :: (stand:int,cycles:int)->MtSequence {
    q:MtSequence;q.count=16;q.origin=[3]f64.[0,0.9,0];
    for i:0..<6*cycles {q.token[stand+i]=1+i%6;}
    return q;
}
```

The closest pair of codes is standing and code 1, phases 0 and 1. Frame 0
differs only by the step, 0.05² = 0.0025. Frame 1 differs by the hip's 15°,
4 sin² 7.5° = 0.068148; the knee's 20 (1 − cos 30°) = 2.679°,
4 sin² 1.340° = 0.002186; and the step, 0.0025. The sum is 0.075334, so
D = 0.274472 and D/2 = 0.137236.

The example decodes the token sequence 01234560 (stand, one full cycle,
stand) and encodes it again. Then it perturbs every frame: the hip turns one
more degree and each step grows by 1 mm. Each frame then differs from its code
by 4 sin² 0.5° + 0.001² in squared distance, and a token's two frames by
twice that, so the stated bound is √(2 · (4 sin² 0.5° + 10⁻⁶)) = 0.024723.
That is far below D/2, so the tokens must come back unchanged.

## Continuity

A decoded clip can be wrong in a way no single token shows: two codes that
are each fine can meet at a boundary where the leg snaps. The continuity
check measures, between every pair of neighbouring frames, each joint's turn
(the angle of conj(a) · b, through chapter 147's arctangent) and the length
of the root's step. It reports the worst of each and where it first happens,
and the first frame and joint that break a bound, with joint −1 meaning the
root. A turn or step equal to its bound passes.

<!-- source: psionic/lib/motion-token.coax continuity -->
```jai
// Between neighbouring frames, each joint turns by the angle of
// conj(a) b and the root moves by the length of its step. Any turn above
// max_turn radians or step above max_step refuses the clip.
mt_continuity :: (c:PoseClip,s:PoseSkeleton,max_turn:f64,max_step:f64)->MtJump {
    out:MtJump;out.frame= -1;out.turn_frame= -1;out.turn_joint= -1;out.step_frame= -1;
    out.detail=pose_clip_check(c,s);
    if out.detail==PoseStatus.ok {out.detail=pose_value(max_turn,POSE_PI);if max_turn<0 {out.detail=PoseStatus.range;}}
    if out.detail==PoseStatus.ok {out.detail=pose_value(max_step,POSE_LENGTH_MAX);if max_step<0 {out.detail=PoseStatus.range;}}
    if out.detail!=PoseStatus.ok {out.status=MtStatus.pose;return out;}
    w:=pose_internal(c,s.count);
    for f:1..<w.frames {
        a:=w.frame[f-1];b:=w.frame[f];
        for j:0..<s.count {
            r:=pfk_multiply([4]f64.[a.rotation[j][0],-a.rotation[j][1],-a.rotation[j][2],-a.rotation[j][3]],b.rotation[j]);
            turn:=pose_norm3(pose_quat_vector(r));
            if turn>out.angle {out.angle=turn;out.turn_frame=f;out.turn_joint=j;}
            if turn>max_turn && out.frame== -1 {out.frame=f;out.joint=j;}
        }
        d:=[3]f64.[b.root[0]-a.root[0],b.root[1]-a.root[1],b.root[2]-a.root[2]];
        step:=pose_norm3(d);if step>out.step {out.step=step;out.step_frame=f;}
        if step>max_step && out.frame== -1 {out.frame=f;out.joint= -1;}
    }
    if out.frame!= -1 {out.status=MtStatus.jump;}
    return out;
}
```

The walk uses 20° and 0.1 m as bounds. By hand, the hip's largest change in
the cycle is 15° (0° to 15° at the start, and −15° to 0° back to standing),
the knee's is 10°, and the largest step is 0.05 m, so 01234560 passes. Code 3
holds phases 4 and 5, where the hip is at 30 sin 120° = 25.98° and the knee at
20 (1 − cos 120°) = 30°. Putting it straight after standing (003) makes frame
4 jump: the hip, joint 0, is the first joint over 20°, and the knee's 30° is
the worst turn.

## Fill in masked tokens

MotionBricks generates tokens by masking: it starts from a sequence whose
tokens are all, or partly, hidden, predicts every hidden one, keeps the most
confident predictions and repeats. This chapter's model is the smallest one
that can play that role: it counts, in training sequences, how often each
code follows each other code.

<!-- source: psionic/lib/motion-token.coax train -->
```jai
// Training counts which code follows which.
mt_train :: (m:MtModel,q:MtSequence)->MtModelResult {
    out:MtModelResult;out.model=m;out.status=mt_tokens_ok(q,m.codes,false);if out.status!=MtStatus.ok {return out;}
    for i:1..<q.count {out.model.pair[q.token[i-1]][q.token[i]]+=1;}
    out.model.sequences+=1;
    return out;
}
```

A hidden position's score for code c is the count of (left, c) when the left
neighbour is known plus the count of (c, right) when the right one is. Its
prediction is the highest-scoring code, the lower one on ties, and its
confidence is that score. With M hidden tokens and S steps, M (S − s) / S
(rounded down) stay hidden after step s. Each step predicts from the tokens
known when it began and unmasks the most confident positions, the lower
position first on ties.

<!-- source: psionic/lib/motion-token.coax fill -->
```jai
// A masked position's score for code c is the count of (left, c) when the
// left neighbour is known plus the count of (c, right) when the right one
// is. Its prediction is the best code (lower on ties), its confidence that
// score. After step s of S, M (S - s) / S of the M masked tokens remain:
// each step unmasks the most confident positions (lower first on ties),
// all predicted from the tokens known when the step began.
mt_fill :: (m:MtModel,q:MtSequence,steps:int)->MtFill {
    out:MtFill;out.sequence=q;out.status=mt_tokens_ok(q,m.codes,true);
    if out.status==MtStatus.ok && (steps<1 || steps>MT_TOKENS) {out.status=MtStatus.shape;}
    if out.status!=MtStatus.ok {return out;}
    for i:0..<q.count {if q.token[i]==MT_MASK {out.masked+=1;}}
    left:=out.masked;
    for s:1..<steps+1 {
        guess:[MT_TOKENS]int;score:[MT_TOKENS]int;
        for i:0..<q.count {
            if out.sequence.token[i]!=MT_MASK {continue;}
            l:=ifx i>0 then out.sequence.token[i-1] else MT_MASK;r:=ifx i+1<q.count then out.sequence.token[i+1] else MT_MASK;
            for c:0..<m.codes {
                v:=0;if l!=MT_MASK {v+=m.pair[l][c];}if r!=MT_MASK {v+=m.pair[c][r];}
                if c==0 || v>score[i] {score[i]=v;guess[i]=c;}
            }
        }
        reveal:=left-out.masked*(steps-s)/steps;
        for n:0..<reveal {
            pick:= -1;
            for i:0..<q.count {if out.sequence.token[i]==MT_MASK && out.step[i]==0 && (pick== -1 || score[i]>score[pick]) {pick=i;}}
            out.step[pick]=s;
        }
        for i:0..<q.count {if out.step[i]==s {out.sequence.token[i]=guess[i];out.confidence[i]=score[i];left-=1;}}
    }
    return out;
}
```

### Fill by hand

Sequences are written as text here and in the example: `mt_parse` reads
digits and a to f as codes 0 to 15 and an underscore as a mask, and refuses
anything else later as an unknown token. Train on 0122, 0120 and 0012 (codes 0, 1 and 2). The pairs are (0, 1) three
times, (1, 2) three times, and (0, 0), (2, 2) and (2, 0) once each. Fill 0___
(an underscore is a mask) in 3 steps; M = 3, so 2, 1 and 0 stay hidden after
steps 1, 2 and 3, and each step unmasks one position.

| Step | Position 1 | Position 2 | Position 3 | Unmasked |
| --- | --- | --- | --- | --- |
| 1 | left 0: code 1 scores 3 | no neighbour: code 0, 0 | no neighbour: code 0, 0 | position 1 = 1 |
| 2 | | left 1: code 2 scores 3 | no neighbour: code 0, 0 | position 2 = 2 |
| 3 | | | left 2: codes 0 and 2 score 1; 0 wins | position 3 = 0 |

The result is 0120. With one step instead, every prediction is made at once
from 0___, and the result is 0100: positions 2 and 3 had no known neighbour.
That is why the schedule unmasks a few tokens at a time.

## Run the example

The [example](../examples/motion-tokens.coax) encodes and decodes the hand
clip, round-trips and perturbs the walk, checks continuity, trains the model
and fills held-out walks, and shows six refusals. Values are printed in
millionths, millimetres or thousandths of a degree.

<!-- source: psionic/examples/motion-tokens.coax hand -->
```jai
    hand:=mt_hand_codebook();one:=mt_one_joint();
    near:=mt_separation(hand);
    print("hand separation (millionths): % between codes % and %\n",mtx_micro(near.distance),near.first,near.second);
    t:=mt_encode(hand,one,mt_hand_clip());
    mtx_print("hand tokens: ",t.sequence);
    print("hand distances (millionths): % % %\n",mtx_micro(t.distance[0]),mtx_micro(t.distance[1]),mtx_micro(t.distance[2]));
    d:=mt_decode(hand,t.sequence);
    for k:0..<d.clip.frames {print("decoded frame %: turn (millidegrees) %, root z (mm) %\n",k,mtx_degrees(d.clip.frame[k].rotation[0]),mtx_milli(d.clip.frame[k].root[2]));}
```

<!-- source: psionic/examples/motion-tokens.coax gait -->
```jai
    gait:=mt_gait_codebook();leg:=pose_leg(1);
    near=mt_separation(gait);
    print("gait separation (millionths): % between codes % and %, half: %\n",mtx_micro(near.distance),near.first,near.second,mtx_micro(near.distance/2));
    walk:=mt_decode(gait,mt_at(mt_parse("01234560"),[3]f64.[0,0.9,0]));
    back:=mt_encode(gait,leg,walk.clip);
    mtx_print("member tokens: ",back.sequence);
    print("member error below 1e-15: %, frames identical: %\n",back.error<1e-15,mt_same_frames(mt_decode(gait,back.sequence).clip,walk.clip,3));
    // Stated before the run: 1 degree of hip and 1 mm of step on both frames
    // of a token move it at most sqrt(2 (4 sin^2(0.5 deg) + 0.001^2)).
    h:=sin(0.5*POSE_PI/180);bound:=sqrt(2*(4*h*h+0.000001));
    moved:=mt_encode(gait,leg,mtx_perturb(walk.clip,1,0.001));
    mtx_print("perturbed tokens: ",moved.sequence);
    print("perturbed error (millionths): %, bound: %\n",mtx_micro(moved.error),mtx_micro(bound));
```

<!-- source: psionic/examples/motion-tokens.coax continuity -->
```jai
    j:=mt_continuity(walk.clip,leg,MT_GAIT_TURN,MT_GAIT_STEP);
    print("walk: %, worst turn (millidegrees) %, worst step (mm) %\n",j.status,mtx_milli(j.angle*180/POSE_PI),mtx_milli(j.step));
    skip:=mt_decode(gait,mt_at(mt_parse("003"),[3]f64.[0,0.9,0]));
    j=mt_continuity(skip.clip,leg,MT_GAIT_TURN,MT_GAIT_STEP);
    print("stand, stand, code 3: % at frame %, joint %, worst turn (millidegrees) %\n",j.status,j.frame,j.joint,mtx_milli(j.angle*180/POSE_PI));
```

The model trains on six walks of 16 tokens: one or more standing tokens, one
or two full cycles, then standing to the end. Each walk is decoded, perturbed
as above and encoded again before it is counted, so the model learns from
tokens that came through the encoder. Each of the four held-out walks
has a combination of standing and cycle counts that no training walk has. Each is masked two ways:
every third token from the second, and four tokens in a row from position 5.

<!-- source: psionic/examples/motion-tokens.coax train -->
```jai
    // Training: six walks, each decoded, perturbed and encoded again.
    stands:=[6]int.[1,3,1,3,2,4];cycles:=[6]int.[1,1,2,2,1,1];
    model:=mt_model(gait.codes);
    for n:0..<6 {
        q:=mt_gait_sequence(stands[n],cycles[n]);
        heard:=mt_encode(gait,leg,mtx_perturb(mt_decode(gait,q).clip,1,0.001));
        same:=true;for i:0..<q.count {if heard.sequence.token[i]!=q.token[i] {same=false;}}
        model=mt_train(model,heard.sequence).model;
        print("train %: tokens recovered %\n",n,same);
    }
    print("pairs: stand-stand %, stand-walk %, walk-stand %\n",model.pair[0][0],model.pair[0][1],model.pair[6][0]);
    // Held out: walks the model never saw, with tokens masked two ways.
    held:=[4][2]int.[[2]int.[0,1],[2]int.[0,2],[2]int.[2,2],[2]int.[4,2]];
    right:=0;hidden:=0;whole:=0;smooth:=0;
    for n:0..<4 {for way:0..<2 {
        q:=mt_gait_sequence(held[n][0],held[n][1]);
        masked:=ifx way==0 then mtx_mask_thirds(q) else mtx_mask_run(q);
        f:=mt_fill(model,masked,4);
        good:=0;for i:0..<q.count {if masked.token[i]==MT_MASK && f.sequence.token[i]==q.token[i] {good+=1;}}
        right+=good;hidden+=f.masked;if good==f.masked {whole+=1;}
        c:=mt_continuity(mt_decode(gait,f.sequence).clip,leg,MT_GAIT_TURN,MT_GAIT_STEP);if c.status==MtStatus.ok {smooth+=1;}
        mtx_print("held ",q);mtx_print("mask ",masked);mtx_print("fill ",f.sequence);
        print("  % of % right, continuity %\n",good,f.masked,c.status);
    }}
    print("held out: % of % masked tokens right, % of 8 sequences whole, % of 8 continuous\n",right,hidden,whole,smooth);
```

<!-- source: psionic/examples/motion-tokens.coax refuse -->
```jai
    zup:=walk.clip;zup.convention=pose_zup();
    print("clip in another convention: %\n",mt_encode(gait,leg,zup).status);
    odd:=walk.clip;odd.frames=7;
    print("7 frames at 2 a token: %\n",mt_encode(gait,leg,odd).status);
    print("code 7 of 7: %\n",mt_decode(gait,mt_parse("017")).status);
    print("a mask left in: %\n",mt_decode(gait,mt_parse("01_")).status);
    print("a model asked for 0 steps: %\n",mt_fill(model,mt_parse("0_2"),0).status);
    twins:=gait;twins.entry[6]=twins.entry[5];
    print("two equal codes: %\n",mt_codebook_check(twins).status);
```

```text
hand separation (millionths): 805162 between codes 1 and 2
hand tokens: 012
hand distances (millionths): 0 87239 174311
decoded frame 0: turn (millidegrees) 0, root z (mm) 0
decoded frame 1: turn (millidegrees) 45000, root z (mm) 250
decoded frame 2: turn (millidegrees) 90000, root z (mm) 750
gait separation (millionths): 274472 between codes 0 and 1, half: 137236
member tokens: 01234560
member error below 1e-15: true, frames identical: true
perturbed tokens: 01234560
perturbed error (millionths): 24723, bound: 24723
walk: ok, worst turn (millidegrees) 15000, worst step (mm) 50
stand, stand, code 3: jump at frame 4, joint 0, worst turn (millidegrees) 30000
train 0: tokens recovered true
train 1: tokens recovered true
train 2: tokens recovered true
train 3: tokens recovered true
train 4: tokens recovered true
train 5: tokens recovered true
pairs: stand-stand 36, stand-walk 6, walk-stand 6
held 1234560000000000
mask 1_34_60_00_00_00
fill 1234560000000000
  5 of 5 right, continuity ok
held 1234560000000000
mask 12345____0000000
fill 1234500000000000
  3 of 4 right, continuity jump
held 1234561234560000
mask 1_34_61_34_60_00
fill 1234561234560000
  5 of 5 right, continuity ok
held 1234561234560000
mask 12345____4560000
fill 1234561234560000
  4 of 4 right, continuity ok
held 0012345612345600
mask 0_12_45_12_45_00
fill 0012345612345000
  4 of 5 right, continuity jump
held 0012345612345600
mask 00123____2345600
fill 0012345612345600
  4 of 4 right, continuity ok
held 0000123456123456
mask 0_00_23_56_23_56
fill 0000023456123456
  4 of 5 right, continuity jump
held 0000123456123456
mask 00001____6123456
fill 0000123456123456
  4 of 4 right, continuity ok
held out: 33 of 36 masked tokens right, 5 of 8 sequences whole, 5 of 8 continuous
clip in another convention: convention
7 frames at 2 a token: shape
code 7 of 7: token
a mask left in: token
a model asked for 0 steps: shape
two equal codes: duplicate
```

The output matches the hand work: tokens 012 with errors 0, 0.087239 and
0.174311, separation 0.805162, and the decoded turns and roots. The walk's
separation is 0.274472. The member round trip recovers 01234560 with frames
identical to the bit. The perturbed walk keeps every token, and its error is
the stated bound to the printed precision, because every frame after the
first moved by the same amount. The walk passes continuity with a worst turn of 15° and a worst
step of 50 mm, and 003 jumps at frame 4.

On the held-out walks the counted model gets 33 of 36 hidden tokens right
and restores 5 of 8 sequences exactly. All three misses are the same
mistake: a hidden token beside a standing token is filled with standing.
Standing follows standing 36 times in the training walks, while any one
walking pair appears at most 8 times, so the sum of counts favours standing.
Between code 5 and standing, for example, code 6 scores 8 + 6 = 14 and
standing scores 0 + 36 = 36. The continuity check refuses exactly those three
sequences: standing straight after code 5 snaps the hip from −30° to 0°, and
standing straight before code 2 snaps it from 0° to 26°. A model that weighs
more context, such as the attention model the plan calls for, should make
fewer of these mistakes; the continuity check keeps the ones it does make
from reaching a character.

Each refusal is a typed `MtStatus`: `convention` for a clip in Z-up
coordinates handed to a Y-up codebook, `shape` for 7 frames at 2 per token,
`token` for code 7 of 7 and for a mask passed to the decoder, `shape` for a
fill with 0 steps, and `duplicate` for a codebook with two equal codes.

## Checks

[motion-token.coax](../lib/motion-token.coax) holds 21 checks of its
own, which run in debug and release with chapter 147's and chapter 167's
checks. The independent ones are:

- **The 6D distance in closed form.** For 49 pairs of turns about X, the
  library's distance and the matrix-free oracle 4(x² + y²) + 8z² both equal
  4 sin²(d/2); q and −q are at distance 0.
- **The hand trace.** The table above: tokens, errors, separation and the
  decoded roots and turns, and the tie rule on a codebook where two codes
  are equally far from a frame. Separation ties are checked on codes whose
  steps are exact binary fractions.
- **Brute force.** On 42 clips of dense rotations and steps along all three
  axes, every token matches a brute-force search over the oracle's
  distances: the one code that no code beats and no lower code ties.
- **Round trip.** 168 token sequences decode and encode back to the same
  tokens and the same frames, bit for bit.
- **The bound.** On 105 perturbed walks, a bound computed before encoding is
  checked to be below D/2, every token is recovered, and every error is at
  most the bound.
- **Continuity and filling by hand.** The walk's worst turn and step, the
  first frame past each bound, the fill table above, the one-step fill and
  the schedule's counts for 9 masks over 3 and 4 steps.
- **Continuity on a dense rotation.** A 0.7 rad turn about a tilted axis on
  top of a dense rotation, and a root step of (0.375, 0.5, −1.5), exactly
  1.625 m.
- **Refusals.** Every convention field changed in turn, the codebook's limits
  at and past each edge (including a full 16 × 8 codebook and two codes
  exactly 2⁻³⁰ apart), unknown tokens, masks, bad lengths, origins and text
  longer than 64 tokens.

`coax mutate --root-only` on the unit leaves zero survivors.

## What this chapter does not do

- The masked-token model is a count of neighbouring pairs, not the attention
  model trained by gradients that the plan describes. It needs chapter 10's
  attention and chapters 103 and 104's training, applied to tokens.
- The decoder is a table lookup. A learned decoder that blends neighbouring
  codes, so that boundaries are smooth by construction, is not built.
- Nothing here reads MotionBricks' tokenizer or weights; that is
  [chapter 149](ch149-execute-motionbricks.md).
- Codes are compared in the codebook's own convention only. Converting a clip
  first is the caller's job, through chapter 147.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [Chapter 147](ch147-poses-have-conventions.md): pose and skeleton
  conventions, the 6D form and the working form.
