# 165. Sequences of observations

> **Status: partly implemented.** Observation records (ticks at a fixed
> rate, images, state vectors, actions and a text condition) run on the CPU
> with an exact byte form that refuses missing and out-of-order frames; so
> do a procedural scene with known dynamics, a per-action next-frame model
> fitted by least squares and measured on held-out scenes one step ahead and
> in open-loop rollouts against gates frozen before the fit, and the codec
> and scheduler contracts, run on chapters 145 and 146. Observations taken
> from Aethyr worlds (batched worlds, sensors, rendered pictures and replay,
> Aethyr chapters 31, 33, 34 and 36, all design drafts) are not connected
> yet.

A world model predicts what a scene looks like next from what it looked
like before and from an action. Before any such model, three things must be
settled: how a sequence of observations is stored, how to tell whether a
prediction is any good, and what the model's parts (its codec and its noise
scheduler) must promise to the rest of the system. This chapter builds all
three on a scene small enough to check by hand.

It builds, in [`observe.coax`](../lib/observe.coax) and
[`observe-contracts.coax`](../lib/observe-contracts.coax):

- an **observation record**: frames with ticks at a fixed period, each with
  an image, a state vector and the action taken after it, plus a text
  condition for the whole sequence, and the rules that admit it;
- its **byte form**, which round-trips every bit and refuses frames whose
  ticks are missing or out of order;
- a **scene**: a coloured 2 × 2 square on an 8 × 8 image that each action
  moves one pixel, stopped by the walls, generated from a seed;
- a **conditioned next-frame model**: one 3 × 3 kernel and a bias per
  action, fitted by least squares;
- **measurements** on held-out scenes, one step ahead and in open-loop
  rollouts, against copying a frame, with gates fixed before the fit;
- a **leak-free split**, with the count that shows it;
- the **codec contract** and the **scheduler contract** that chapter 166's
  components must pass, run here on chapter 145's codec and chapter 146's
  schedules.

```shell
coax/bin/coax run psionic/examples/observe.coax
coax/bin/coax run psionic/examples/observe-refuse.coax
coax/bin/coax test psionic/lib/observe.coax
coax/bin/coax test psionic/lib/observe-contracts.coax
```

The [example](../examples/observe.coax) takes about 30 seconds of CPU
time in a debug build, compiling included. The fit is exact, so there is no long training run to record. The
whole transcript is under [Run the example](#run-the-example).

## The record

A sequence holds up to 12 frames. Frame `i` has a tick, the time it was
taken in whatever unit the source uses; the frames are `period` ticks apart.
Each frame carries its pixels (3 channels of 8 × 8, channel by channel as in
[chapter 142](ch142-pixels-convolutions-and-patches.md)), its state (here,
the square's column and row) and the action taken after it. One text
condition, printable ASCII up to 16 bytes, describes the whole sequence.

<!-- source: psionic/lib/observe.coax types -->
```jai
ObStatus :: enum u8 {ok;count;rate;order;missing;range;action;number;text;format;capacity;rank;codec;}
// count frames, period ticks apart; frame i has ticks[i], its state (the
// square's column and row), its pixels (3 channels of 8 by 8, channel by
// channel) and actions[i], the action taken after it. The text is one
// printable ASCII condition for the whole sequence.
ObSeq :: struct {
    count:int;period:int;text_len:int;text:[OB_TEXT_MAX]u8;
    ticks:[OB_FRAMES_MAX]int;actions:[OB_FRAMES_MAX]int;
    state:[OB_FRAMES_MAX*OB_STATE]f32;pixels:[OB_FRAMES_MAX*OB_PIXELS]f32;
}
ObResult :: struct {status:ObStatus;written:int;}
ObParsed :: struct {status:ObStatus;seq:ObSeq;}
// The least-squares sums for each action: 10 by 10 products of features and
// 10 products of features with the target pixel.
ObNormal :: struct {a:[OB_ACTIONS*OB_TAPS*OB_TAPS]f64;b:[OB_ACTIONS*OB_TAPS]f64;rows:[OB_ACTIONS]int;}
// Mean squared error of the model and of the copy baseline over frames.
ObError :: struct {status:ObStatus;model:f64;copy:f64;frames:int;}
```

<!-- source: psionic/lib/observe.coax shape -->
```jai
OB_SIDE :: 8;
OB_PIXELS :: 192;           // 3 channels of 8 by 8
OB_FRAMES_MAX :: 12;
OB_STATE :: 2;              // the square's column and row
OB_TEXT_MAX :: 16;
OB_ACTIONS :: 5;            // stay, right, left, down, up
OB_TAPS :: 10;              // 9 kernel taps and a bias
OB_PARAMS :: 50;
OB_SQUARE :: 2;             // the square's side
OB_LAST :: 6;               // the last column or row the square can start at
OB_PERIOD_MAX :: 255;
OB_TICK_MAX :: 4294967295;  // ticks are stored in 4 bytes
OB_HEAD :: 7;               // "OBS1", count, period, text length
OB_FRAME_BYTES :: 781;      // tick 4, action 1, state 8, pixels 768
```

Admission checks a record in a fixed order and reports the first failure.
The ticks carry the most rules. The gap between neighbours must be exactly
one period. A gap of zero or less is out of order. A gap of a whole number
of periods means frames are missing. Any other gap breaks the rate. Pixels
must be finite and in `[0, 1]`; states need only be finite.

<!-- source: psionic/lib/observe.coax record -->
```jai
ob_finite :: (v:f32)->bool {return (f32_bits(v)&0x7fffffff)<0x7f800000;}
// The first failure, in this order: frame count, period, text, ticks,
// actions, numbers. Pixels must lie in [0, 1]; states need only be finite.
ob_admit :: (s:ObSeq)->ObStatus {
    if s.count<2 || s.count>OB_FRAMES_MAX {return ObStatus.count;}
    if s.period<1 || s.period>OB_PERIOD_MAX {return ObStatus.rate;}
    if s.text_len<0 || s.text_len>OB_TEXT_MAX {return ObStatus.text;}
    for i:0..<s.text_len {if s.text[i]<32 || s.text[i]>126 {return ObStatus.text;}}
    if s.ticks[0]<0 || s.ticks[s.count-1]>OB_TICK_MAX {return ObStatus.range;}
    for i:1..<s.count {
        gap:=s.ticks[i]-s.ticks[i-1];
        if gap<=0 {return ObStatus.order;}
        if gap%s.period!=0 {return ObStatus.rate;}
        if gap!=s.period {return ObStatus.missing;}
    }
    for i:0..<s.count {if s.actions[i]<0 || s.actions[i]>=OB_ACTIONS {return ObStatus.action;}}
    for i:0..<s.count*OB_STATE {if !ob_finite(s.state[i]) {return ObStatus.number;}}
    for i:0..<s.count*OB_PIXELS {v:=s.pixels[i];if !ob_finite(v) || v<0 || v>1 {return ObStatus.number;}}
    return ObStatus.ok;
}
```

With a period of 40 and frame 0 at tick 100, frame 1 must be at 140. At 100
or 60 it is out of order, at 180 or 220 one or two frames are missing, and
at 139, 141 or 150 the rate is broken.

## The byte form

A record is written as a 7-byte header (`OBS1`, the frame count, the period
and the text length), the text, then 781 bytes a frame: the tick in 4
little-endian bytes, the action in 1, and the state and pixels as f32 bit
patterns, 4 bytes each. Reading checks the magic, the counts and the exact
length, rebuilds the record, and then admits it, so a frame whose tick was
changed in transit is refused by the same rule as one built badly in memory.

<!-- source: psionic/lib/observe.coax bytes -->
```jai
ob_size :: (s:ObSeq)->int {return OB_HEAD+s.text_len+s.count*OB_FRAME_BYTES;}
ob_put :: (out:[]u8,at:int,v:u32) {for k:0..<4 {out[at+k]=cast(u8)((v>>cast(u32)(8*k))&255);}}
ob_get :: (b:[]u8,at:int)->u32 {v:u32=0;for k:0..<4 {v|=cast(u32)b[at+k]<<cast(u32)(8*k);}return v;}
// "OBS1", the frame count, the period and the text length, a byte each;
// the text; then each frame: its tick (4 bytes), its action (1), its state
// and pixels as f32 bit patterns (4 bytes each), all little-endian.
ob_write :: (s:ObSeq,out:[]u8)->ObResult {
    st:=ob_admit(s);if st!=ObStatus.ok {return ObResult.{status=st};}
    n:=ob_size(s);if out.count<n {return ObResult.{status=ObStatus.capacity};}
    out[0]=79;out[1]=66;out[2]=83;out[3]=49;
    out[4]=cast(u8)s.count;out[5]=cast(u8)s.period;out[6]=cast(u8)s.text_len;
    for i:0..<s.text_len {out[OB_HEAD+i]=s.text[i];}
    at:=OB_HEAD+s.text_len;
    for f:0..<s.count {
        ob_put(out,at,cast(u32)s.ticks[f]);out[at+4]=cast(u8)s.actions[f];
        for j:0..<OB_STATE {ob_put(out,at+5+4*j,f32_bits(s.state[f*OB_STATE+j]));}
        for j:0..<OB_PIXELS {ob_put(out,at+13+4*j,f32_bits(s.pixels[f*OB_PIXELS+j]));}
        at+=OB_FRAME_BYTES;
    }
    return ObResult.{written=n};
}
// The byte form back to a record, which must then pass admission.
ob_read :: (b:[]u8)->ObParsed {
    r:ObParsed;
    if b.count<OB_HEAD || b[0]!=79 || b[1]!=66 || b[2]!=83 || b[3]!=49 {r.status=ObStatus.format;return r;}
    count:=int(b[4]);tl:=int(b[6]);
    if count<2 || count>OB_FRAMES_MAX {r.status=ObStatus.count;return r;}
    if tl>OB_TEXT_MAX {r.status=ObStatus.text;return r;}
    if b.count!=OB_HEAD+tl+count*OB_FRAME_BYTES {r.status=ObStatus.format;return r;}
    s:ObSeq;s.count=count;s.period=int(b[5]);s.text_len=tl;
    for i:0..<tl {s.text[i]=b[OB_HEAD+i];}
    at:=OB_HEAD+tl;
    for f:0..<count {
        s.ticks[f]=int(ob_get(b,at));s.actions[f]=int(b[at+4]);
        for j:0..<OB_STATE {s.state[f*OB_STATE+j]=f32_from_bits(ob_get(b,at+5+4*j));}
        for j:0..<OB_PIXELS {s.pixels[f*OB_PIXELS+j]=f32_from_bits(ob_get(b,at+13+4*j));}
        at+=OB_FRAME_BYTES;
    }
    r.seq=s;r.status=ob_admit(s);
    return r;
}
```

By hand, for the two-frame record the checks use (ticks 100 and 140, period
40, text `red`, a red square at column 2, row 3, moving right): the header
is `79 66 83 49` (`OBS1`), then `2`, `40`, `3`, then `114 101 100`
(`red`). Frame 0's tick, 100, follows as `100 0 0 0`, then the action `1`.
The state's first value, 2.0, is `0x40000000`, so its bytes are
`0 0 0 64`. In all, `7 + 3 + 2 × 781 = 1572` bytes. Because every value is
stored as its bit pattern, the round trip keeps negative zero, subnormal
pixels and the largest 4-byte tick exactly.

## The scene

Scene `k` draws everything from the seed `1 + 7919 k` with the Park-Miller
generator chapter 145 also uses. The first two draws, modulo 7, place the
square's upper-left corner in columns and rows 0 to 6. The next three pick
each channel's level. The rest pick one action per frame. Action 0 stays,
1 moves right, 2 left, 3 down and 4 up; a move that would leave the image
does nothing.

<!-- source: psionic/lib/observe.coax scene -->
```jai
// One step of the Park-Miller generator: s * 48271 mod (2^31 - 1).
ob_draw :: (s:int)->int #export_c {return (s*48271)%2147483647;}
// Action a moves the square's corner (x, y) one pixel: 0 stays, 1 right,
// 2 left, 3 down, 4 up. A move that would leave the image does nothing.
ob_move :: (x:int,y:int,a:int)->[2]int {
    dx:=[5]int.[0,1,-1,0,0];dy:=[5]int.[0,0,0,1,-1];
    nx:=x+dx[a];ny:=y+dy[a];
    if nx<0 || nx>OB_LAST {nx=x;}
    if ny<0 || ny>OB_LAST {ny=y;}
    return [2]int.[nx,ny];
}
ob_render :: (x:int,y:int,colour:[3]f32,out:[]f32) {
    for c:0..<3 {for row:0..<OB_SIDE {for col:0..<OB_SIDE {
        v:f32=0;if col>=x && col<x+OB_SQUARE && row>=y && row<y+OB_SQUARE {v=colour[c];}
        out[(c*OB_SIDE+row)*OB_SIDE+col]=v;
    }}}
}
// Scene k: a start corner, a colour and the actions, all drawn from seed
// 1 + 7919 k. Training scenes take each channel from 4/8, 6/8 and 8/8;
// held-out scenes from 5/8 and 7/8, so no held-out frame can equal a
// training frame. Frames are 40 ticks apart from tick 0. 2 to 12 frames;
// any other count gives an empty record.
ob_scene :: (k:int,held:bool,frames:int)->ObSeq {
    s:ObSeq;if frames<2 || frames>OB_FRAMES_MAX {return s;}
    d:=1+7919*k;d=ob_draw(d);x:=d%7;d=ob_draw(d);y:=d%7;
    colour:[3]f32;
    for c:0..<3 {
        d=ob_draw(d);
        if held {colour[c]=f32(5+2*(d%2))/8;} else {colour[c]=f32(4+2*(d%3))/8;}
    }
    s.count=frames;s.period=40;
    t:="square";for ch,i:t {s.text[i]=ch;}s.text_len=t.count;
    for i:0..<frames {
        d=ob_draw(d);s.ticks[i]=40*i;s.actions[i]=d%OB_ACTIONS;
        s.state[2*i]=f32(x);s.state[2*i+1]=f32(y);
        ob_render(x,y,colour,s.pixels[i*OB_PIXELS..<(i+1)*OB_PIXELS]);
        m:=ob_move(x,y,s.actions[i]);x=m[0];y=m[1];
    }
    return s;
}
```

Training scenes take each channel's level from 4/8, 6/8 and 8/8; held-out
scenes take it from 5/8 and 7/8. The square is always fully inside the
image, so every frame has lit pixels, and a held-out frame's lit pixels can
never equal a training frame's. That makes the split leak-free by
construction; the leak count below checks it.

## The model

The model predicts the next frame from the last one and the action. Each
output pixel is a weighted sum of the 3 × 3 neighbourhood around the same
pixel in the same channel, plus a bias. The weights depend on the action
but not on the channel or the position, so each action has 10 weights and
the model has 50.

<!-- source: psionic/lib/observe.coax model -->
```jai
// The summed squared difference between one frame of a, from ia, and one
// of b, from ib. Two frames are equal exactly when it is 0: the smallest
// nonzero difference of two f32 values, squared in binary64, does not
// underflow.
ob_sq :: (a:[]f32,ia:int,b:[]f32,ib:int)->f64 {t:=0.0;for i:0..<OB_PIXELS {d:=f64(a[ia+i])-f64(b[ib+i]);t+=d*d;}return t;}
// Pixel i's features, for the frame starting at pixels[at]: its 3 by 3
// neighbourhood in its own channel, row by row from the upper left (taps 0
// to 8, zero outside the image), then 1 (tap 9, the bias).
ob_features :: (pixels:[]f32,at:int,i:int,f:[]f64) {
    c:=i/64;row:=(i/8)%8;col:=i%8;f[9]=1;
    for tap:0..<9 {
        r:=row+tap/3-1;k:=col+tap%3-1;v:=0.0;
        if r>=0 && r<OB_SIDE && k>=0 && k<OB_SIDE {v=f64(pixels[at+(c*OB_SIDE+r)*OB_SIDE+k]);}
        f[tap]=v;
    }
}
// The next frame under action a: each pixel is the dot product of its
// features with the action's 10 weights. The same kernel serves every
// channel.
ob_predict :: (w:[]f64,frame:[]f32,a:int,out:[]f32)->ObStatus {
    if a<0 || a>=OB_ACTIONS {return ObStatus.action;}
    if w.count!=OB_PARAMS || frame.count!=OB_PIXELS {return ObStatus.count;}
    if out.count<OB_PIXELS {return ObStatus.capacity;}
    for v:w {if !(v-v==0) {return ObStatus.number;}}
    for v:frame {if !ob_finite(v) {return ObStatus.number;}}
    tmp:[OB_PIXELS]f32;f:[OB_TAPS]f64;
    for i:0..<OB_PIXELS {
        ob_features(frame,0,i,f[..]);t:=0.0;
        for j:0..<OB_TAPS {t+=w[a*OB_TAPS+j]*f[j];}
        if !(abs(t)<=3.4028234663852886e38) {return ObStatus.number;}
        tmp[i]=f32(t);
    }
    for i:0..<OB_PIXELS {out[i]=tmp[i];}
    return ObStatus.ok;
}
```

The taps are numbered row by row from the upper left: tap 3 is the
neighbour to the left, tap 4 the pixel itself, tap 5 the neighbour to the
right, tap 1 above and tap 7 below. Moving the square right means each
pixel takes its left neighbour's value, so the ideal kernel for "right" is
tap 3 set to 1 and everything else 0. "Stay" is tap 4, "left" tap 5,
"down" tap 1 and "up" tap 7.

By hand, with the square at column 2, row 3 and red 1: pixel (row 3,
column 3) sees red in taps 3, 4, 6 and 7 (left, itself, lower left, below),
so its features are `0 0 0 1 1 0 1 1 0 1`, the last being the bias input.
Under the ideal "right" kernel it takes tap 3: 1, lit. Pixel (row 3,
column 2) has nothing to its left, so it goes dark. The square moves from
columns 2 and 3 to columns 3 and 4.

Copying the frame, the simplest baseline, gets those two pixels in each
of two rows wrong: 4 pixels off by 1, a mean squared error of
`4 / 192 = 0.020833`.

A second hand case, used by the checks: under "left" (action 2) set tap 4
to 1/2 and the bias to 1/8. The square's pixels predict 5/8 and all 188
others 1/8. Against the true next frame, the square at columns 1 and 2:

| Pixels | Predicted | True | Squared error each |
| --- | --- | --- | --- |
| 2 in column 2 | 5/8 | 1 | 9/64 |
| 2 in column 3 | 5/8 | 0 | 25/64 |
| 2 in column 1 | 1/8 | 1 | 49/64 |
| the other 186 | 1/8 | 0 | 1/64 |

The sum is `(18 + 50 + 98 + 186) / 64 = 5.5`.

## Fitting by least squares

The model is linear in its weights, so the best weights for squared error
solve the normal equations: for each action, `A w = b` with `A` the sum of
`f fᵀ` over every pixel of every transition under that action, and `b` the
sum of `f y`, where `f` is the pixel's features and `y` its true next
value. The sums are gathered one record at a time and solved by Gaussian
elimination with partial pivoting, in binary64.

<!-- source: psionic/lib/observe.coax fit -->
```jai
// Add every transition of an admitted record to the least-squares sums of
// the action taken: one row per pixel of the next frame.
ob_gather :: (n:ObNormal,seq:ObSeq)->ObNormal {
    if ob_admit(seq)!=ObStatus.ok {return n;}
    out:=n;s:=seq;f:[OB_TAPS]f64;
    for t:0..<s.count-1 {
        a:=s.actions[t];
        for i:0..<OB_PIXELS {
            ob_features(s.pixels[..],t*OB_PIXELS,i,f[..]);y:=f64(s.pixels[(t+1)*OB_PIXELS+i]);
            for p:0..<OB_TAPS {
                out.b[a*OB_TAPS+p]+=f[p]*y;
                for q:0..<OB_TAPS {out.a[(a*OB_TAPS+p)*OB_TAPS+q]+=f[p]*f[q];}
            }
        }
        out.rows[a]+=OB_PIXELS;
    }
    return out;
}
// Solve m w = r for n unknowns by Gaussian elimination with partial
// pivoting (the first row on ties), in binary64. A pivot at most 1e-12
// times the sum of the diagonal's magnitudes means the sums do not
// determine w.
ob_solve :: (m:[]f64,r:[]f64,n:int,w:[]f64)->ObStatus {
    if n<1 || n>OB_TAPS || m.count!=n*n || r.count!=n || w.count!=n {return ObStatus.count;}
    for v:m {if !(v-v==0) {return ObStatus.number;}}
    for v:r {if !(v-v==0) {return ObStatus.number;}}
    a:[OB_TAPS*OB_TAPS]f64;b:[OB_TAPS]f64;scale:=0.0;
    for i:0..<n*n {a[i]=m[i];}
    for i:0..<n {b[i]=r[i];scale+=abs(m[i*n+i]);}
    for c:0..<n {
        p:=c;for i:c+1..<n {if abs(a[i*n+c])>abs(a[p*n+c]) {p=i;}}
        if abs(a[p*n+c])<=1e-12*scale {return ObStatus.rank;}
        for j:0..<n {t:=a[c*n+j];a[c*n+j]=a[p*n+j];a[p*n+j]=t;}
        t:=b[c];b[c]=b[p];b[p]=t;
        for i:c+1..<n {
            k:=a[i*n+c]/a[c*n+c];
            for j:c..<n {a[i*n+j]-=k*a[c*n+j];}
            b[i]-=k*b[c];
        }
    }
    for c:0..<n {
        i:=n-1-c;t:=b[i];
        for j:i+1..<n {t-=a[i*n+j]*w[j];}
        w[i]=t/a[i*n+i];
    }
    return ObStatus.ok;
}
// Each action's 10 weights from its own sums.
ob_fit :: (sums:ObNormal,w:[]f64)->ObStatus {
    if w.count!=OB_PARAMS {return ObStatus.count;}
    n:=sums;
    for a:0..<OB_ACTIONS {
        s:=ob_solve(n.a[a*OB_TAPS*OB_TAPS..<(a+1)*OB_TAPS*OB_TAPS],n.b[a*OB_TAPS..<(a+1)*OB_TAPS],OB_TAPS,w[a*OB_TAPS..<(a+1)*OB_TAPS]);
        if s!=ObStatus.ok {return s;}
    }
    return ObStatus.ok;
}
```

By hand: `[2 1; 1 3] w = [3; 5]`. Eliminating the first column subtracts
half the first row from the second: `2.5 w₁ = 3.5`, so `w₁ = 1.4`, then
`2 w₀ = 3 − 1.4`, `w₀ = 0.8`. A pivot at most `10⁻¹²` times the sum of the
diagonal's magnitudes means the data do not determine the weights, as when
an action never occurs; the fit then refuses with `rank`.

Without walls every move is a pure shift, the ideal kernel predicts every
transition exactly, and least squares must return it; the checks confirm
this to within `10⁻⁹`. With walls, a push into a wall leaves the square
where it is, which no single kernel can tell apart from a free move. Least
squares then spreads weight between the shifted tap and the pixel itself,
and the fitted kernels in the transcript show it: "right" keeps 0.94 on tap
3 and puts 0.107 on tap 4.

## Measuring predictions

<!-- source: psionic/lib/observe.coax gates -->
```jai
// Frozen before the first fit (chapter 165, "Acceptance"): held-out mean
// squared error as a fraction of copying the frame, at most.
OB_GATE_NEXT :: 0.25;       // one step ahead, from the true previous frame
OB_GATE_SHORT :: 0.5;       // open loop, 2 and 4 steps ahead
OB_GATE_LONG :: 1.0;        // open loop, 8 steps ahead
```

These three gates were written into the source before the first fit ran.
They compare the model's mean squared error with copying a frame, which
needs no learning:

- **One step ahead.** Each transition of each held-out scene is predicted
  from the true frame before it. The model must reach a quarter of the
  copy's error or less.
- **Open loop.** From frame 0 alone, the model predicts `h` frames, each
  from its own last prediction and the recorded action, and the result is
  compared with frame `h`. The baseline copies frame 0. At 2 and 4 steps
  the model must reach half the copy's error; at 8 it must not be worse.

<!-- source: psionic/lib/observe.coax measure -->
```jai
// One step ahead: every transition predicted from the true frame before it,
// against copying that frame.
ob_next :: (w:[]f64,seq:ObSeq)->ObError {
    if w.count!=OB_PARAMS {return ObError.{status=ObStatus.count};}
    st:=ob_admit(seq);if st!=ObStatus.ok {return ObError.{status=st};}
    s:=seq;e:ObError;p:[OB_PIXELS]f32;
    for t:0..<s.count-1 {
        st=ob_predict(w,s.pixels[t*OB_PIXELS..<(t+1)*OB_PIXELS],s.actions[t],p[..]);if st!=ObStatus.ok {return ObError.{status=st};}
        e.model+=ob_sq(p[..],0,s.pixels[..],(t+1)*OB_PIXELS);e.copy+=ob_sq(s.pixels[..],t*OB_PIXELS,s.pixels[..],(t+1)*OB_PIXELS);e.frames+=1;
    }
    e.model/=f64(e.frames*OB_PIXELS);e.copy/=f64(e.frames*OB_PIXELS);
    return e;
}
// Open loop: from frame 0, h predictions each fed the last prediction and
// the recorded action, against frame h; the baseline copies frame 0.
ob_rollout :: (w:[]f64,seq:ObSeq,h:int)->ObError {
    if w.count!=OB_PARAMS {return ObError.{status=ObStatus.count};}
    st:=ob_admit(seq);if st!=ObStatus.ok {return ObError.{status=st};}
    s:=seq;
    if h<1 || h>=s.count {return ObError.{status=ObStatus.count};}
    cur:[OB_PIXELS]f32;nxt:[OB_PIXELS]f32;
    for i:0..<OB_PIXELS {cur[i]=s.pixels[i];}
    for t:0..<h {
        st=ob_predict(w,cur[..],s.actions[t],nxt[..]);if st!=ObStatus.ok {return ObError.{status=st};}
        cur=nxt;
    }
    return ObError.{model=ob_sq(cur[..],0,s.pixels[..],h*OB_PIXELS)/OB_PIXELS,copy=ob_sq(s.pixels[..],0,s.pixels[..],h*OB_PIXELS)/OB_PIXELS,frames=1};
}
// A gate passes when the measurement succeeded and the model's error is at
// most the fraction of the copy's.
ob_pass :: (e:ObError,fraction:f64)->bool {return e.status==ObStatus.ok && e.frames>0 && e.model<=fraction*e.copy;}
```

Open-loop error grows with the horizon because each prediction's error is
fed back in: a model that halved every pixel would leave `2⁻ʰ` of the
square after `h` steps, and the checks confirm that case exactly.

## A leak-free split

<!-- source: psionic/lib/observe.coax leaks -->
```jai
// Scenes [h0, h1) (held-out when held is set) against training scenes
// [t0, t1): how many of the first scenes' frames equal some training frame.
ob_leaks :: (h0:int,h1:int,held:bool,t0:int,t1:int,frames:int)->int {
    n:=0;
    for k:h0..<h1 {
        h:=ob_scene(k,held,frames);
        for f:0..<frames {
            hit:=false;
            for j:t0..<t1 {
                t:=ob_scene(j,false,frames);
                for g:0..<frames {if ob_sq(h.pixels[..],f*OB_PIXELS,t.pixels[..],g*OB_PIXELS)==0 {hit=true;}}
            }
            if hit {n+=1;}
        }
    }
    return n;
}
```

The count compares every frame of the held-out scenes with every frame of
the training scenes, bit for bit. For scenes against themselves it finds
every frame, which shows the count can find a leak when one exists.

## The codec contract

Chapter 166's world model works in a codec's latent space, as chapter 145
explains. Any codec it uses must, over every frame of an admitted record:

1. write one group of codes a frame, and say so;
2. write only codes in range;
3. give the same codes whether frames are encoded together or one at a
   time (no frame may leak into another's codes);
4. give the same codes when run again;
5. give the same codes after packing to bytes and unpacking;
6. decode to finite pixels.

The contract is split in two. A run collects what the codec produced; a
check names the first clause the run breaks. Chapter 166 needs only a run
of its own codec to use the same check.

<!-- source: psionic/lib/observe-contracts.coax contract-types -->
```jai
// Which clause of a contract failed, the first in the order listed.
ObClause :: enum u8 {ok;status;count;range;batch;repeat;pack;finite;steps;start;beta;product;walk;}
OB_CODES_MAX :: 192;        // 12 frames of 16 codes
// What a codec produced over one record: codes for all frames encoded
// together, one at a time and together again; the codes after packing to
// bytes and unpacking; and the decoded frames. written counts what the
// codec said it wrote for each list of codes, in that order, and pixels for
// the decoded frames.
ObCodecRun :: struct {
    status:ObStatus;frames:int;written:[4]int;pixels:int;
    batch:[OB_CODES_MAX]int;single:[OB_CODES_MAX]int;again:[OB_CODES_MAX]int;unpacked:[OB_CODES_MAX]int;
    decoded:[OB_FRAMES_MAX*OB_PIXELS]f32;
}
```

<!-- source: psionic/lib/observe-contracts.coax codec-run -->
```jai
// Run chapter 145's codec over every frame of an admitted record.
ob_codec_run :: (w:[]f32,seq:ObSeq)->ObCodecRun {
    r:ObCodecRun;r.status=ob_admit(seq);if r.status!=ObStatus.ok {return r;}
    s:=seq;n:=s.count;r.frames=n;
    ws:[OB_FRAMES_MAX*CZ_WORK]f32;x:=s.pixels[0..<n*OB_PIXELS];
    e:=cz_encode(w,x,n,r.batch[..],ws[..]);
    if e.status!=CzStatus.ok {r.status=ObStatus.codec;return r;}
    r.written[0]=e.written;
    one:[CZ_LATENT]int;
    for f:0..<n {
        r.written[1]+=cz_encode(w,s.pixels[f*OB_PIXELS..<(f+1)*OB_PIXELS],1,one[..],ws[..]).written;
        for j:0..<CZ_LATENT {r.single[f*CZ_LATENT+j]=one[j];}
    }
    r.written[2]=cz_encode(w,x,n,r.again[..],ws[..]).written;
    bytes:[OB_CODES_MAX/2]u8;
    p:=cz_pack(r.batch[0..<n*CZ_LATENT],bytes[..]);
    r.written[3]=cz_unpack(bytes[0..<p.written],r.unpacked[..]).written;
    d:=cz_decode(w,r.batch[0..<n*CZ_LATENT],n,r.decoded[..],ws[..]);
    r.pixels=d.written;
    return r;
}
```

<!-- source: psionic/lib/observe-contracts.coax codec-check -->
```jai
ob_codec_check :: (r:ObCodecRun)->ObClause {
    if r.status!=ObStatus.ok {return ObClause.status;}
    n:=r.frames*CZ_LATENT;
    if r.frames<1 || r.frames>OB_FRAMES_MAX || r.pixels!=r.frames*OB_PIXELS {return ObClause.count;}
    for k:r.written {if k!=n {return ObClause.count;}}
    for i:0..<n {
        for k:[4]int.[r.batch[i],r.single[i],r.again[i],r.unpacked[i]] {if k< -CZ_LEVELS || k>CZ_LEVELS {return ObClause.range;}}
    }
    for i:0..<n {if r.single[i]!=r.batch[i] {return ObClause.batch;}}
    for i:0..<n {if r.again[i]!=r.batch[i] {return ObClause.repeat;}}
    for i:0..<n {if r.unpacked[i]!=r.batch[i] {return ObClause.pack;}}
    for i:0..<r.pixels {if !ob_finite(r.decoded[i]) {return ObClause.finite;}}
    return ObClause.ok;
}
```

By hand, with chapter 145's probe weights (each latent value is the leaky
activation of red pixel `(4i, 4j)`): a square at column 4, row 4 with red 1
lights only red pixel (4, 4), so only latent (1, 1) is nonzero. tanh 1 is
0.7616; times 7 that is 5.33, code 5. Each of the 4 filters gives the
codes `0 0 0 5`, and decoding puts the level 5/7 on pixel (4, 4) of every
channel.

The contract checks form, not quality. Chapter 145's recorded weights pass
it on a held-out scene in the transcript, yet reconstruct these frames
worse than an all-zero image does, because that codec was trained on
different pictures.

## The scheduler contract

A diffusion or flow head walks noise levels with a schedule from
[chapter 146](ch146-noise-diffusion-and-flow.md). The contract requires an
admitted schedule of 1 to 1,000 steps with `abar₀ = 1`; every beta in
`(0, 1)` and never decreasing; every `abar_t` exactly `abar_{t−1} (1 − β_t)`
and above 0; and a walk of levels, the ones a sampler visits, that starts
at the last level, decreases strictly and ends at 0.

<!-- source: psionic/lib/observe-contracts.coax schedule-check -->
```jai
// n + 1 levels from steps down to 0: level i is steps (n - i) / n, rounded
// down. Returns how many it wrote, or 0 when n is not in 1..steps or out is
// too short.
ob_walk :: (steps:int,n:int,out:[]int)->int {
    if n<1 || n>steps || out.count<n+1 {return 0;}
    for i:0..<n+1 {out[i]=(steps*(n-i))/n;}
    return n+1;
}
ob_schedule_check :: (s:DfSchedule,walk:[]int)->ObClause {
    if s.status!=DfStatus.ok {return ObClause.status;}
    if s.steps<1 || s.steps>DF_STEPS_MAX {return ObClause.steps;}
    if s.abar[0]!=1 {return ObClause.start;}
    for t:1..<s.steps+1 {
        b:=s.beta[t];
        if !(b>0 && b<1) {return ObClause.beta;}
        if t>1 && b<s.beta[t-1] {return ObClause.beta;}
    }
    for t:1..<s.steps+1 {if s.abar[t]!=s.abar[t-1]*(1-s.beta[t]) || !(s.abar[t]>0) {return ObClause.product;}}
    if walk.count<2 || walk[0]!=s.steps || walk[walk.count-1]!=0 {return ObClause.walk;}
    for i:1..<walk.count {if walk[i]>=walk[i-1] {return ObClause.walk;}}
    return ObClause.ok;
}
```

By hand: betas 0.1 and 0.2 give `abar = 1, 0.9, 0.72`, and the walk
`2, 1, 0` passes. A 50-step walk over 1,000 levels visits
`1000 (50 − i) / 50`: 1000, 980, …, 20, 0. A walk of 3 over 10 levels
rounds down: 10, 6, 3, 0. Every schedule chapter 146 admits passes; 120
betas of 0.999 drive the running product to exactly 0, which the contract
refuses.

## Run the example

<!-- source: psionic/examples/observe.coax example -->
```jai
main :: () {
    // 1. A two-frame record: a red square at (2, 3), action "right", then at (3, 3).
    s:=ob_two();bytes:[OB_HEAD+OB_TEXT_MAX+OB_FRAMES_MAX*OB_FRAME_BYTES]u8;
    w:=ob_write(s,bytes[..]);
    print("hand record: %, % bytes; header",w.status,w.written);for i:0..<10 {print(" %",bytes[i]);}print("\n");
    back:=ob_read(bytes[0..<w.written]);
    print("read back: %, equal bit for bit: %\n",back.status,ob_same(back.seq.pixels[..],s.pixels[..]) && ob_same(back.seq.state[..],s.state[..]) && ob_twelve(back.seq.ticks,s.ticks));

    // 2. The ideal kernel for "right" predicts the second frame exactly.
    ideal:[OB_PARAMS]f64;ob_ideal(ideal[..]);e:=ob_next(ideal[..],s);
    print("ideal kernel on the hand record: model MSE %, copy MSE % (4 of 192 pixels off by 1)\n",e.model,show6(e.copy));

    // 3. Fit on training scenes 0..47, 12 frames each.
    sums:ObNormal;for k:0..<TRAIN {sums=ob_gather(sums,ob_scene(k,false,FRAMES));}
    fit:[OB_PARAMS]f64;st:=ob_fit(sums,fit[..]);
    names:=[5]string.["stay","right","left","down","up"];
    print("fit on % transitions: %\n",TRAIN*(FRAMES-1),st);
    for a:0..<OB_ACTIONS {
        print("  % (% pixels):",names[a],sums.rows[a]);for j:0..<OB_TAPS {print(" %",show3(fit[a*OB_TAPS+j]));}print("\n");
    }

    // 4. Held-out scenes 0..15 (colours 5/8 and 7/8 only), one step ahead.
    print("held-out frames equal to a training frame: %\n",ob_leaks(0,HELD,true,0,TRAIN,FRAMES));
    m:=0.0;c:=0.0;
    for k:0..<HELD {r:=ob_next(fit[..],ob_scene(k,true,FRAMES));m+=r.model;c+=r.copy;}
    next:=ObError.{model=m/HELD,copy=c/HELD,frames=HELD*(FRAMES-1)};
    print("one step ahead, held-out: model MSE %, copy MSE %, ratio %, gate <= %: %\n",show6(next.model),show6(next.copy),show3(next.model/next.copy),OB_GATE_NEXT,ob_pass(next,OB_GATE_NEXT));
    m=0;c=0;for k:0..<TRAIN {r:=ob_next(fit[..],ob_scene(k,false,FRAMES));m+=r.model;c+=r.copy;}
    print("one step ahead, training: model MSE %, copy MSE %\n",show6(m/TRAIN),show6(c/TRAIN));

    // 5. Open loop from frame 0 of each held-out scene.
    for h:[4]int.[1,2,4,8] {
        m=0;c=0;for k:0..<HELD {r:=ob_rollout(fit[..],ob_scene(k,true,FRAMES),h);m+=r.model;c+=r.copy;}
        g:=OB_GATE_SHORT;if h==8 {g=OB_GATE_LONG;}
        line:=ObError.{model=m/HELD,copy=c/HELD,frames=HELD};
        print("open loop, horizon %: model MSE %, copy frame 0 MSE %, ratio %",h,show6(line.model),show6(line.copy),show3(line.model/line.copy));
        if h>1 {print(", gate <= %: %",g,ob_pass(line,g));}
        print("\n");
    }

    // 6. Seeded: gathering and fitting again gives the same bits.
    again:ObNormal;for k:0..<TRAIN {again=ob_gather(again,ob_scene(k,false,FRAMES));}
    fit2:[OB_PARAMS]f64;st=ob_fit(again,fit2[..]);same:=true;for i:0..<OB_PARAMS {if f64_bits(fit2[i])!=f64_bits(fit[i]) {same=false;}}
    print("refit from the same seeds, identical weights: %\n",same);

    // 7. The codec contract on chapter 145's recorded weights, over held-out scene 0.
    cw:[CZ_PARAMS]f32;for i:0..<CZ_PARAMS {cw[i]=f32_from_bits(CZ_TRAINED[i]);}
    h0:=ob_scene(0,true,FRAMES);run:=ob_codec_run(cw[..],h0);
    print("codec contract, chapter 145's weights, 12 frames: %, % codes\n",ob_codec_check(run),run.written[0]);
    print("  frame 0 codes:");for i:0..<CZ_LATENT {print(" %",run.batch[i]);}print("\n");
    zero:[OB_PIXELS]f32;rec:=0.0;blank:=0.0;
    for f:0..<FRAMES {rec+=ob_sq(run.decoded[..],f*OB_PIXELS,h0.pixels[..],f*OB_PIXELS);blank+=ob_sq(zero[..],0,h0.pixels[..],f*OB_PIXELS);}
    print("  reconstruction MSE over the 12 frames: %; an all-zero frame's: %\n",show6(rec/(FRAMES*OB_PIXELS)),show6(blank/(FRAMES*OB_PIXELS)));
    bad:=run;bad.single[5]=bad.batch[5]+1;print("  one frame encoded alone gives a different code: %\n",ob_codec_check(bad));

    // 8. The scheduler contract on chapter 146's schedules, with a 50-step walk.
    walk:[1001]int;n:=ob_walk(1000,50,walk[..]);
    print("scheduler contract: cosine 1000 steps, walk of % levels: %\n",n,ob_schedule_check(df_cosine(1000),walk[0..<n]));
    print("scheduler contract: linear 1e-4 to 0.02: %\n",ob_schedule_check(df_linear(1000,0.0001,0.02),walk[0..<n]));
    short:=[3]int.[1000,500,1];print("  a walk that stops at level 1: %\n",ob_schedule_check(df_cosine(1000),short[..]));
    ds:=df_cosine(1000);ds.abar[500]*=1.000001;print("  abar 500 off its running product: %\n",ob_schedule_check(ds,walk[0..<n]));
}
```

```text
hand record: ok, 1572 bytes; header 79 66 83 49 2 40 3 114 101 100
read back: ok, equal bit for bit: true
ideal kernel on the hand record: model MSE 0.0, copy MSE 0.020833 (4 of 192 pixels off by 1)
fit on 528 transitions: ok
  stay (20928 pixels): 0.0 0.0 0.0 0.0 1.0 0.0 0.0 0.0 0.0 0.0
  right (19008 pixels): 0.0 0.0 0.0 0.94 0.107 0.053 0.0 0.0 0.0 0.0
  left (18816 pixels): 0.0 0.0 0.0 0.04 0.071 0.962 0.0 0.0 0.0 0.0
  down (24000 pixels): 0.0 0.956 0.0 0.0 0.081 0.0 0.0 0.041 0.0 0.0
  up (18624 pixels): 0.0 0.028 0.0 0.0 0.044 0.0 0.0 0.977 0.0 0.0
held-out frames equal to a training frame: 0
one step ahead, held-out: model MSE 0.002156, copy MSE 0.026578, ratio 0.081, gate <= 0.25: true
one step ahead, training: model MSE 0.001987, copy MSE 0.02525
open loop, horizon 1: model MSE 0.001546, copy frame 0 MSE 0.026062, ratio 0.059
open loop, horizon 2: model MSE 0.004713, copy frame 0 MSE 0.043365, ratio 0.109, gate <= 0.5: true
open loop, horizon 4: model MSE 0.008382, copy frame 0 MSE 0.048645, ratio 0.172, gate <= 0.5: true
open loop, horizon 8: model MSE 0.011535, copy frame 0 MSE 0.052155, ratio 0.221, gate <= 1.0: true
refit from the same seeds, identical weights: true
codec contract, chapter 145's weights, 12 frames: ok, 192 codes
  frame 0 codes: -4 1 -4 -5 2 -2 2 3 1 -1 2 4 5 1 5 5
  reconstruction MSE over the 12 frames: 0.093499; an all-zero frame's: 0.040039
  one frame encoded alone gives a different code: batch
scheduler contract: cosine 1000 steps, walk of 51 levels: ok
scheduler contract: linear 1e-4 to 0.02: ok
  a walk that stops at level 1: walk
  abar 500 off its running product: product
```

What it shows:

- **The record round-trips.** The header matches the hand calculation, and
  reading the bytes back gives the record bit for bit.
- **The fit.** Each action's kernel puts most of its weight on the tap the
  hand analysis predicts (0.94 to 1.0), with the rest on the pixel itself
  and the opposite tap, the price of walls.
- **Held-out frames.** One step ahead, the model's error is 0.081 of the
  copy's, against a gate of 0.25. The training error (0.001987) is close to
  the held-out error (0.002156): the model has 50 weights and 101,376 pixel
  rows to fit them.
- **Open loop.** The error grows from 0.0015 at one step to 0.0115 at eight
  as predictions feed on predictions, but stays well under copying frame 0
  (0.221 of it at eight steps). All three rollout gates pass.
- **No leaks, and seeded.** No held-out frame equals a training frame, and
  gathering and fitting again from the same seeds gives identical bits.
- **Contracts.** Chapter 145's codec and chapter 146's two schedules pass;
  one changed code, a walk that stops at level 1 and one perturbed `abar`
  each fail on the clause that names them.

## What it refuses

The [refusal example](../examples/observe-refuse.coax) builds one bad input
of each kind and prints the status it gets:

<!-- source: psionic/examples/observe-refuse.coax refuse -->
```jai
main :: () {
    s:=ob_two();b:=s;
    b.count=1;print("a record of 1 frame: %\n",ob_admit(b));
    b=s;b.period=0;print("period 0: %\n",ob_admit(b));
    b=s;b.ticks[1]=100;print("frame 1 at tick 100, the same as frame 0: %\n",ob_admit(b));
    b=s;b.ticks[1]=60;print("frame 1 at tick 60, before frame 0: %\n",ob_admit(b));
    b=s;b.ticks[1]=180;print("frame 1 at tick 180, one frame missing: %\n",ob_admit(b));
    b=s;b.ticks[1]=150;print("frame 1 at tick 150, off the 40-tick rate: %\n",ob_admit(b));
    b=s;b.actions[0]=5;print("action 5: %\n",ob_admit(b));
    b=s;b.pixels[10]=0.0/0.0;print("a NaN pixel: %\n",ob_admit(b));
    b=s;b.pixels[10]=1.5;print("a pixel of 1.5: %\n",ob_admit(b));
    b=s;b.text[1]=10;print("a newline in the text: %\n",ob_admit(b));

    bytes:[OB_HEAD+OB_TEXT_MAX+OB_FRAMES_MAX*OB_FRAME_BYTES]u8;for i:0..<bytes.count {bytes[i]=9;}
    b=s;b.ticks[1]=180;print("write a record with a missing frame: %\n",ob_write(b,bytes[..]).status);
    print("write 1,572 bytes into 1,571: %\n",ob_write(s,bytes[0..<1571]).status);
    untouched:=0;for v:bytes {if v==9 {untouched+=1;}}print("bytes still 9: % of %\n",untouched,bytes.count);
    n:=ob_write(s,bytes[..]).written;
    print("read 1,571 of 1,572 bytes: %\n",ob_read(bytes[0..<n-1]).status);
    bytes[3]=50;print("read \"OBS2\": %\n",ob_read(bytes[0..<n]).status);bytes[3]=49;
    bytes[791]=180;print("read with frame 1's tick changed to 180: %\n",ob_read(bytes[0..<n]).status);bytes[791]=140;
    print("read the untouched bytes: %\n",ob_read(bytes[0..<n]).status);

    w:[OB_PARAMS]f64;ob_ideal(w[..]);out:[OB_PIXELS]f32;
    print("predict under action 7: %\n",ob_predict(w[..],s.pixels[0..<OB_PIXELS],7,out[..]));
    print("predict from 191 pixels: %\n",ob_predict(w[..],s.pixels[0..<191],1,out[..]));
    w[3]=1.0/0.0;print("predict with an infinite weight: %\n",ob_predict(w[..],s.pixels[0..<OB_PIXELS],1,out[..]));w[3]=0;
    sums:ObNormal;sums=ob_gather(sums,s);print("fit when only action 1 was seen: %\n",ob_fit(sums,w[..]));
    print("roll out 2 steps from a 2-frame record: %\n",ob_rollout(w[..],s,2).status);
    print("roll out 0 steps: %\n",ob_rollout(w[..],s,0).status);

    cw:[CZ_PARAMS]f32;cz_probe(cw[..]);
    print("codec run with 1,814 weights: %\n",ob_codec_run(cw[0..<1814],s).status);
    b=s;b.ticks[1]=180;print("codec run over a record with a missing frame: % (contract: %)\n",ob_codec_run(cw[..],b).status,ob_codec_check(ob_codec_run(cw[..],b)));
    bad:=[3]f64.[0.1,0.05,0.2];walk:=[2]int.[3,0];
    print("betas 0.1, 0.05, 0.2: chapter 146 says %, the contract says %\n",df_betas(bad[..]).status,ob_schedule_check(df_betas(bad[..]),walk[..]));
    print("a 6-step walk over 5 levels: % levels written\n",ob_walk(5,6,walk[..]));
}
```

```text
a record of 1 frame: count
period 0: rate
frame 1 at tick 100, the same as frame 0: order
frame 1 at tick 60, before frame 0: order
frame 1 at tick 180, one frame missing: missing
frame 1 at tick 150, off the 40-tick rate: rate
action 5: action
a NaN pixel: number
a pixel of 1.5: number
a newline in the text: text
write a record with a missing frame: missing
write 1,572 bytes into 1,571: capacity
bytes still 9: 9395 of 9395
read 1,571 of 1,572 bytes: format
read "OBS2": format
read with frame 1's tick changed to 180: missing
read the untouched bytes: ok
predict under action 7: action
predict from 191 pixels: count
predict with an infinite weight: number
fit when only action 1 was seen: rank
roll out 2 steps from a 2-frame record: count
roll out 0 steps: count
codec run with 1,814 weights: codec
codec run over a record with a missing frame: missing (contract: status)
betas 0.1, 0.05, 0.2: chapter 146 says order, the contract says status
a 6-step walk over 5 levels: 0 levels written
```

Refused writes leave their output untouched: every byte of the buffer
still holds 9 after the two refused writes.

## Checks

`coax/bin/coax test psionic/lib/observe.coax` and
`coax/bin/coax test psionic/lib/observe-contracts.coax` run these, in debug
and release:

- **The scene.** The generator's first two draws by hand; every move,
  including each wall; every frame equal to a frame drawn independently from
  its state; each state following its action by a rule restated in the
  check; colour levels per split; scene 1's corner, colours and first
  actions recomputed from its seed; same seed, same scene, bit for bit.
- **Admission.** Each rule at and past its edge, and the first failure
  winning when several apply.
- **The byte form.** The header and the first values by hand; exact round
  trips for the hand record, six scenes, a 16-byte text, negative zero, a
  subnormal and the largest tick; and refusals of short, long, foreign and
  altered bytes, including a changed tick read back as `missing` or `rate`.
- **The model.** Features by hand at the centre, edges and corner of the
  image and of a frame stored at an offset; the ideal kernel moving the
  square; the hand error of 5.5; and refusals before anything is written.
- **Least squares.** 1 × 1, 2 × 2 and 3 × 3 solves by hand, a row swap,
  ties broken toward the first row (pinned to the bits it produces), a
  rank-deficient system, the threshold's scale, and the ideal kernel
  recovered from scenes without walls. With walls, the fitted weights
  satisfy `A w = b` to within `10⁻⁸`.
- **Errors.** The hand record exact under the ideal kernel; the copy
  baseline against a count of differing pixels taken from the states; a
  halving model's open-loop error equal to its closed form at 1 to 4 steps;
  teacher-forced sums equal to predictions made one by one.
- **The split.** Every held-out frame compared directly with every training
  frame, and the leak count finding every frame of a scene compared with
  itself.
- **Contracts.** The probe codec's codes and decoded pixels by hand; every
  clause broken alone and in combination; schedules from chapter 146 and by
  hand; walks by hand.

`coax mutate --root-only` leaves zero survivors on both units.

## What this chapter does not do

- It does not take observations from Aethyr. The plan draws worlds from the
  Aethyr book: many worlds stepped together (Aethyr
  [chapter 31](../../aethyr/book/ch031-many-worlds-in-one-step.md)),
  sensors ([33](../../aethyr/book/ch033-sensors-read-the-simulation.md)),
  pictures rendered for policies
  ([34](../../aethyr/book/ch034-pictures-for-policies.md)) and recorded
  replays ([36](../../aethyr/book/ch036-record-and-replay-a-world.md)).
  Those chapters are design drafts. The scene here is generated in Psionic.
- The model reads only the last frame and the action. The record carries a
  state vector and a text condition, but the model uses neither. The scene
  is Markov in its frame, so earlier frames would add nothing here.
- The model is linear and deterministic. It does not generate frames with
  chapter 146's samplers, and it does not work in chapter 145's latent
  space; the contracts state what such a model's parts must satisfy, and
  chapter 166 is where they are used.
- The contracts check form and consistency, not quality or physical
  plausibility.

## Dependencies

- [Chapter 145](ch145-compress-and-reconstruct-an-image.md): the codec the
  codec contract runs on.
- [Chapter 146](ch146-noise-diffusion-and-flow.md): the schedules the
  scheduler contract runs on.
- [Chapter 142](ch142-pixels-convolutions-and-patches.md): the image layout.

## Sources

- [Chapter plan](PLAN.md), Part XVIII.
- [Aethyr book](../../aethyr/book/README.md).
