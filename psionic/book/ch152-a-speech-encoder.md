# 152. A speech encoder

> **Status: partly implemented.** The FastConformer encoder runs on the CPU
> with seeded weights: subsampling, lengths and masks, Conformer layers with
> relative-position attention and the convolution module, a CTC head and
> greedy decoding. Each piece matches an independent binary64 reference, and
> a padded batch matches each item run alone bit for bit. Comparing against
> intermediate tensors recorded from the pinned Parakeet checkpoint needs that
> checkpoint, which is chapter 153's job, and is not done yet.

[Chapter 151](ch151-waveforms-and-spectral-features.md) turned audio into
log-mel features: 128 numbers every 10 ms. A speech recognizer's encoder
reads those frames and writes a shorter sequence of vectors, one every 80 ms,
each describing a stretch of sound in context. Parakeet's encoder is NVIDIA
NeMo's FastConformer. This chapter builds its architecture in
[`conformer.coax`](../lib/conformer.coax):

- **subsampling**: three stride-2 convolutions over time and frequency (a
  full 3 × 3 convolution, then two depthwise-separable ones, each followed by
  ReLU) that cut the frame rate by eight, then a linear map to the model
  width;
- **lengths and masks**: every batch item keeps its own valid length through
  each stage, and no frame past it is ever read;
- **Conformer layers**: half a feed-forward module, self-attention with
  Transformer-XL relative positions, a convolution module (pointwise, gated
  linear unit, depthwise over time, batch norm, swish, pointwise), the other
  half feed-forward module, and a layer norm;
- a **CTC head** (a linear map and log-softmax per frame) and **greedy CTC
  decoding**.

The weights here are seeded random numbers, not Parakeet's. They are enough
to check that every operation computes what its definition says; they are
not enough to recognize speech, so the tokens the example decodes mean
nothing. Loading the 0.6-billion-parameter checkpoint and comparing with its
recorded tensors is [chapter 153](ch153-transcribe-with-parakeet.md)'s work.

Everything stores f32 values and sums in binary64 with chapter 4's
compensated accumulator, rounding once to f32 per stored value. That is the
profile `psionic-conformer-v1`. A batch has at most 8 items of at most 2,048
input frames (20 seconds at a 10 ms hop) of at most 128 mels; the subsampling
has at most 64 channels; the model is at most 128 wide with a 512-wide
feed-forward, 8 layers and 31 depthwise taps; the head has at most 1,024
tokens and the blank. The convolutions are [chapter
142](ch142-pixels-convolutions-and-patches.md)'s `px_conv_direct`, which also
bounds each convolution at 2^20 values and 2^24 multiply-adds.

## Run it

The programs are [`speech-encoder.coax`](../examples/speech-encoder.coax) and
[`speech-encoder-refuse.coax`](../examples/speech-encoder-refuse.coax); the
checks' references are in [`conformer-checks.coax`](../tests/conformer-checks.coax).

```shell
coax/bin/coax run psionic/examples/speech-encoder.coax
coax/bin/coax run psionic/examples/speech-encoder-refuse.coax
coax/bin/coax test psionic/lib/conformer.coax
coax/bin/coax mutate psionic/lib/conformer.coax --root-only
```

```text
1 frames -> 1 -> 1 -> 1 encoded
8 frames -> 4 -> 2 -> 1 encoded
9 frames -> 5 -> 3 -> 2 encoded
101 frames -> 51 -> 26 -> 13 encoded
1000 frames -> 500 -> 250 -> 125 encoded
2048 frames -> 1024 -> 512 -> 256 encoded
positions 1 0 -1 per row: 10.0 11.0 12.0 / 20.0 21.0 22.0; shifted: 11.0 12.0 / 20.0 21.0
features: 101 and 61 frames of 128 mels, padded to 101
plan: 15017 weights; time 101 -> 51 -> 26 -> 13, mels 128 -> 64 -> 32 -> 16; work 79744 values
batch: ok, 416 values, encoded lengths 13 and 8
item 0: alone equals the batch bit for bit: true; padded frames zero: true; within 1e-4 of the binary64 reference: true
item 1: alone equals the batch bit for bit: true; padded frames zero: true; within 1e-4 of the binary64 reference: true
item 0, frame 0, first 4 of 16: -0.5567 -0.4468 -0.6538 2.353
head: ok, 234 values; item 0 frame 0 log-probabilities sum to 1: true
item 0 greedy: 1 tokens: 5
item 1 greedy: 1 tokens: 5
best per frame a a _ a b b _ _ c: 4 tokens: 0 0 1 2
```

The example takes about 7 seconds in a debug build. The sections below work
the first lines by hand before showing the code that prints them.

## The types

<!-- source: psionic/lib/conformer.coax types -->
```jai
CfStatus :: enum u8 {ok;shape;limit;length;capacity;alias;number;}
// mels features a frame; channels in the subsampling convolutions; dim the
// model width, split evenly into heads; hidden the feed-forward width; kernel
// the depthwise convolution's taps (odd); vocab tokens, and the blank after
// them. xscale multiplies the subsampled frames by sqrt(dim).
CfShape :: struct {mels:int;channels:int;dim:int;heads:int;hidden:int;kernel:int;layers:int;vocab:int;xscale:bool;}
// Sizes for a batch padded to a frame count: t1..t3 and f1..f3 are time and
// frequency after each stride-2 stage. Counts are f32 values. The work array
// holds the masked input, chapter 142's scratch, the convolution stages, one
// item's layer work and the encoded frames, at the offsets given.
CfPlan :: struct {status:CfStatus;t1:int;t2:int;t3:int;f1:int;f2:int;f3:int;weights:int;input:int;output:int;logits:int;
    conv:int;layer:int;at_scratch:int;at_conv:int;at_layer:int;at_output:int;work:int;}
CfResult :: struct {status:CfStatus;written:int;}
```

<!-- source: psionic/lib/conformer.coax bounds -->
```jai
CF_MELS_MAX :: 128;
CF_CHANNELS_MAX :: 64;
CF_DIM_MAX :: 128;
CF_HIDDEN_MAX :: 512;
CF_KERNEL_MAX :: 31;
CF_LAYERS_MAX :: 8;
CF_VOCAB_MAX :: 1024;
CF_FRAMES_MAX :: 2048;
CF_BATCH_MAX :: 8;
CF_TOKENS_MAX :: 256;      // CF_FRAMES_MAX / 8
CF_FEATURES_MAX :: 1024;   // CF_CHANNELS_MAX * 16, the width of 128 mels after three halvings
CF_EPSILON :: 0.00001;     // layer and batch norm
// Admitted magnitudes. With them no stored value can pass f32: the largest,
// after the subsampling, is under 4e32 at every bound above.
CF_FEATURE_MAX :: 10000.0;
CF_WEIGHT_MAX :: 1000.0;   // batch norm variances: any finite value >= 0
CF_FRAME_MAX :: 100000.0;  // what the CTC head reads; a layer norm gives at most 12,300
```

## Lengths, by hand

Each subsampling stage is a convolution with a 3-tap kernel, stride 2 and one
frame of zeros on each side. A convolution's output length is
(n + 2 · padding − kernel) / stride + 1, rounded down, which here is
(n − 1) / 2 + 1: n / 2 rounded up.

One second of audio at a 10 ms hop is 101 frames (chapter 151 centres the
first window on sample 0). Then:

- stage 1: (101 − 1) / 2 + 1 = 51
- stage 2: (51 − 1) / 2 + 1 = 26
- stage 3: (26 − 1) / 2 + 1 = 12 + 1 = 13 (25 / 2 rounds down to 12)

Three roundings up of a halving are one rounding up of an eighth, so the
encoded length is (n + 7) / 8 rounded down: (101 + 7) / 8 = 13.5, so 13.
Twenty seconds, 2,048 frames, gives exactly 256. The frequency axis goes
through the same three stages: 128 mels become 64, 32 and then 16.

<!-- source: psionic/lib/conformer.coax lengths -->
```jai
// One stride-2 stage: kernel 3, one frame of zeros on each side.
// (n + 2 - 3) / 2 + 1, rounded down, which is n/2 rounded up.
cf_stage :: (n:int)->int {return (n-1)/2+1;}
cf_length :: (frames:int)->int {return cf_stage(cf_stage(cf_stage(frames)));}
```

A `#forall` check runs every frame count from 1 to 2,048 and requires
`cf_length` to equal (n + 7) / 8 and also to equal the output height that
chapter 142's convolution planner computes for the three stages, which uses
its own formula with the dilated kernel span.

## Planning a batch

The plan describes the three convolutions in chapter 142's terms (time is the
image's height and frequency its width), asks its planner for each output
size, and lays out one work array: the masked input, chapter 142's scratch,
the stage outputs (the first convolution's output is the largest, and later
stages reuse it), one item's layer work, and the encoded frames.

<!-- source: psionic/lib/conformer.coax plan -->
```jai
// The subsampling convolutions in chapter 142's terms: time is the image's
// height and frequency its width.
cf_conv0 :: (s:CfShape,batch:int,frames:int)->PxConv {
    return PxConv.{batch=batch,channels=1,height=frames,width=s.mels,filters=s.channels,kernel_h=3,kernel_w=3,stride_h=2,stride_w=2,pad_h=1,pad_w=1};
}
cf_depthwise :: (s:CfShape,batch:int,t:int,f:int)->PxConv {
    c:=s.channels;
    return PxConv.{batch=batch,channels=c,height=t,width=f,filters=c,kernel_h=3,kernel_w=3,stride_h=2,stride_w=2,pad_h=1,pad_w=1,groups=c};
}
cf_pointwise :: (s:CfShape,batch:int,t:int,f:int)->PxConv {
    return PxConv.{batch=batch,channels=s.channels,height=t,width=f,filters=s.channels,kernel_h=1,kernel_w=1};
}
cf_plan :: (s:CfShape,batch:int,frames:int)->CfPlan {
    status:=cf_shape_check(s);if status!=CfStatus.ok {return CfPlan.{status=status};}
    if batch>CF_BATCH_MAX || frames>CF_FRAMES_MAX {return CfPlan.{status=CfStatus.limit};}
    // Chapter 142 refuses a batch or frame count below 1, and work past its bounds.
    c0:=px_conv_plan(cf_conv0(s,batch,frames));
    if c0.status==PxStatus.limit {return CfPlan.{status=CfStatus.limit};}
    if c0.status!=PxStatus.ok {return CfPlan.{status=CfStatus.shape};}
    d1:=px_conv_plan(cf_depthwise(s,batch,c0.out_h,c0.out_w));
    d2:=px_conv_plan(cf_depthwise(s,batch,d1.out_h,d1.out_w));
    p:=CfPlan.{t1=c0.out_h,f1=c0.out_w,t2=d1.out_h,f2=d1.out_w,t3=d2.out_h,f3=d2.out_w};
    p.weights=cf_weight_count(s);
    p.input=batch*s.mels*frames;p.output=batch*p.t3*s.dim;p.logits=batch*p.t3*(s.vocab+1);
    // The first convolution's output is the largest; later stages reuse it.
    p.conv=c0.y;p.layer=6*p.t3*s.dim;
    p.at_scratch=p.input;p.at_conv=p.at_scratch+p.conv;p.at_layer=p.at_conv+p.conv;p.at_output=p.at_layer+p.layer;
    p.work=p.at_output+p.output;
    return p;
}
```

## The weights

All weights live in one f32 array: the subsampling, then each layer's
modules in the order they run, then the head. A linear map's matrix is
(out, in), row by row, as PyTorch stores it, with its bias right after it.

<!-- source: psionic/lib/conformer.coax layout -->
```jai
// Where each weight starts in the flat f32 array. A linear map's matrix is
// (out, in), row by row, and its bias follows it.
// Feed-forward: layer norm gain and shift, (hidden, dim), (dim, hidden).
CfFf :: struct {norm:int;w1:int;w2:int;end:int;}
// Attention: norm; query, key, value, output maps with biases; the position
// map without a bias; the per-head biases u and v added to the query.
CfAtt :: struct {norm:int;q:int;k:int;v:int;o:int;pos:int;u:int;bias_v:int;end:int;}
// Convolution module: norm; pointwise (2 dim, dim); depthwise (dim, kernel)
// and bias; batch norm mean, variance, gain and shift; pointwise (dim, dim).
CfConv :: struct {norm:int;pw1:int;dw:int;bn:int;pw2:int;end:int;}
CfLayer :: struct {ff1:CfFf;att:CfAtt;conv:CfConv;ff2:CfFf;out:int;end:int;}
// Subsampling: 3 x 3 convolution from one channel; depthwise 3 x 3 and
// pointwise 1 x 1 twice; (dim, channels x f3). Each kernel is followed by its bias.
CfSub :: struct {conv0:int;dw1:int;pw1:int;dw2:int;pw2:int;out:int;end:int;}
cf_ff_at :: (s:CfShape,base:int)->CfFf {
    d:=s.dim;h:=s.hidden;a:=CfFf.{norm=base,w1=base+2*d};a.w2=a.w1+h*d+h;a.end=a.w2+d*h+d;return a;
}
cf_att_at :: (s:CfShape,base:int)->CfAtt {
    d:=s.dim;a:=CfAtt.{norm=base,q=base+2*d};
    a.k=a.q+d*d+d;a.v=a.k+d*d+d;a.o=a.v+d*d+d;a.pos=a.o+d*d+d;a.u=a.pos+d*d;a.bias_v=a.u+d;a.end=a.bias_v+d;return a;
}
cf_conv_at :: (s:CfShape,base:int)->CfConv {
    d:=s.dim;a:=CfConv.{norm=base,pw1=base+2*d};a.dw=a.pw1+2*d*d+2*d;a.bn=a.dw+d*s.kernel+d;a.pw2=a.bn+4*d;a.end=a.pw2+d*d+d;return a;
}
cf_layer_at :: (s:CfShape,base:int)->CfLayer {
    l:CfLayer;l.ff1=cf_ff_at(s,base);l.att=cf_att_at(s,l.ff1.end);l.conv=cf_conv_at(s,l.att.end);
    l.ff2=cf_ff_at(s,l.conv.end);l.out=l.ff2.end;l.end=l.out+2*s.dim;return l;
}
cf_sub_at :: (s:CfShape)->CfSub {
    c:=s.channels;a:CfSub;a.dw1=10*c;a.pw1=a.dw1+10*c;a.dw2=a.pw1+c*c+c;a.pw2=a.dw2+10*c;a.out=a.pw2+c*c+c;
    a.end=a.out+s.dim*c*cf_length(s.mels)+s.dim;return a;
}
cf_layer_size :: (s:CfShape)->int {return cf_layer_at(s,0).end;}
cf_layer_base :: (s:CfShape,layer:int)->int {return cf_sub_at(s).end+layer*cf_layer_size(s);}
cf_head_at :: (s:CfShape)->int {return cf_layer_base(s,s.layers);}
cf_weight_count :: (s:CfShape)->int {return cf_head_at(s)+(s.vocab+1)*(s.dim+1);}
```

For the small shape the checks use (16 mels, 3 channels, width 8, 2 heads,
hidden 16, kernel 3, one layer, 3 tokens), the subsampling takes
30 + 30 + 12 + 30 + 12 values for its five kernels and biases, then
8 × (3 × 2) + 8 = 56 for the linear map from 3 channels × 2 frequencies:
170. One layer takes 1,288 and the head 4 × 8 + 4 = 36, for 1,494 in all.
The layout check asserts these numbers.

`cf_init` fills the array from chapter 53's counter-based generator: weight i
takes draw i of the seed's stream, so it does not depend on the order of the
fills. A separate check lists every region with its centre and spread in its
own words, requires the regions to tile the array exactly, and recomputes
each weight from `cr_draw`.

<!-- source: psionic/lib/conformer.coax init -->
```jai
// Seeded weights: matrices uniform in +-1/sqrt(fan in); biases in +-0.1;
// norm gains in 1 +- 0.1; batch norm variances in 1 +- 0.5. Weight i takes
// draw i of the seed's counter stream, so it does not depend on fill order.
cf_fill :: (w:[]f32,at:int,n:int,center:f64,spread:f64,seed:u64) {
    for i:at..<at+n {u:=f64(cr_bits(seed,cast(u64)i)>>11)/9007199254740992.0;w[i]=f32(center+spread*(2*u-1));}
}
cf_fill_linear :: (w:[]f32,at:int,rows:int,cols:int,seed:u64) {
    cf_fill(w,at,rows*cols,0,1/sqrt(f64(cols)),seed);cf_fill(w,at+rows*cols,rows,0,0.1,seed);
}
cf_fill_norm :: (w:[]f32,at:int,d:int,seed:u64) {cf_fill(w,at,d,1,0.1,seed);cf_fill(w,at+d,d,0,0.1,seed);}
cf_fill_ff :: (s:CfShape,w:[]f32,a:CfFf,seed:u64) {
    cf_fill_norm(w,a.norm,s.dim,seed);cf_fill_linear(w,a.w1,s.hidden,s.dim,seed);cf_fill_linear(w,a.w2,s.dim,s.hidden,seed);
}
cf_init :: (s:CfShape,w:[]f32,seed:u64)->CfStatus {
    status:=cf_shape_check(s);if status!=CfStatus.ok {return status;}
    if w.count!=cf_weight_count(s) {return CfStatus.shape;}
    c:=s.channels;d:=s.dim;sub:=cf_sub_at(s);
    cf_fill_linear(w,sub.conv0,c,9,seed);cf_fill_linear(w,sub.dw1,c,9,seed);cf_fill_linear(w,sub.pw1,c,c,seed);
    cf_fill_linear(w,sub.dw2,c,9,seed);cf_fill_linear(w,sub.pw2,c,c,seed);cf_fill_linear(w,sub.out,d,c*cf_length(s.mels),seed);
    for k:0..<s.layers {
        l:=cf_layer_at(s,cf_layer_base(s,k));
        cf_fill_ff(s,w,l.ff1,seed);cf_fill_ff(s,w,l.ff2,seed);cf_fill_norm(w,l.out,d,seed);
        a:=l.att;cf_fill_norm(w,a.norm,d,seed);
        maps:=[4]int.[a.q,a.k,a.v,a.o];for m:maps {cf_fill_linear(w,m,d,d,seed);}
        cf_fill(w,a.pos,d*d,0,1/sqrt(f64(d)),seed);cf_fill(w,a.u,d,0,0.1,seed);cf_fill(w,a.bias_v,d,0,0.1,seed);
        v:=l.conv;cf_fill_norm(w,v.norm,d,seed);cf_fill_linear(w,v.pw1,2*d,d,seed);cf_fill_linear(w,v.dw,d,s.kernel,seed);
        cf_fill(w,v.bn,d,0,0.1,seed);cf_fill(w,v.bn+d,d,1,0.5,seed);cf_fill_norm(w,v.bn+2*d,d,seed);cf_fill_linear(w,v.pw2,d,d,seed);
    }
    cf_fill_linear(w,cf_head_at(s),s.vocab+1,d,seed);
    return CfStatus.ok;
}
```

### Bounds that rule out overflow

The encoder admits features within ±10,000 and weights within ±1,000, except
batch norm variances, which need only be finite and not negative. Then no
stored value can pass the f32 range, 3.4 × 10^38, and the code needs no
overflow checks. The largest values come out of the subsampling at the
bounds (64 channels, 128 mels):

- first convolution: 9 taps × 1,000 × 10,000 ≈ 9.0 × 10^7
- depthwise: 9 × 1,000 × 9.0 × 10^7 ≈ 8.1 × 10^11
- pointwise over 64 channels: ≈ 5.2 × 10^16
- depthwise, then pointwise: ≈ 4.7 × 10^20, then ≈ 3.0 × 10^25
- the linear map over 64 × 16 = 1,024 inputs: ≈ 3.1 × 10^31, times
  √128 ≈ 11.3: ≈ 3.5 × 10^32

Inside a layer, everything starts from a layer norm, whose output is at most
√127 × 1,000 + 1,000 ≈ 12,300. The largest value in a layer is in the
convolution module, where batch norm can divide by √(10⁻⁵) ≈ 316: about
2 × 10^24. The head reads encoded frames within ±100,000.

<!-- source: psionic/lib/conformer.coax admit -->
```jai
// Every weight finite and within CF_WEIGHT_MAX, except batch norm variances,
// which only need to be finite and not negative.
cf_weights_ok :: (s:CfShape,w:[]f32)->bool {
    for v:w {if !psionic_tensor_finite_bits(f32_bits(v)) {return false;}}
    from:=0;d:=s.dim;
    for k:0..<s.layers {
        var:=cf_layer_at(s,cf_layer_base(s,k)).conv.bn+d;
        for i:from..<var {if abs(w[i])>CF_WEIGHT_MAX {return false;}}
        for i:var..<var+d {if w[i]<0 {return false;}}
        from=var+d;
    }
    for i:from..<w.count {if abs(w[i])>CF_WEIGHT_MAX {return false;}}
    return true;
}
// Every feature of every valid frame finite and within CF_FEATURE_MAX.
cf_features_ok :: (x:[]f32,lengths:[]int,mels:int,frames:int)->bool {
    for b:0..<lengths.count {for m:0..<mels {for t:0..<lengths[b] {
        v:=x[(b*mels+m)*frames+t];if !psionic_tensor_finite_bits(f32_bits(v)) || abs(v)>CF_FEATURE_MAX {return false;}
    }}}
    return true;
}
// Every value of every valid encoded frame finite and within CF_FRAME_MAX.
cf_frames_ok :: (enc:[]f32,lengths:[]int,frames:int,dim:int)->bool {
    for b:0..<lengths.count {for i:b*frames*dim..<(b*frames+lengths[b])*dim {
        if !psionic_tensor_finite_bits(f32_bits(enc[i])) || abs(enc[i])>CF_FRAME_MAX {return false;}
    }}
    return true;
}
```

## Linear maps and the feed-forward module

<!-- source: psionic/lib/conformer.coax linear -->
```jai
// out[r] = bias[r] + sum over c of W[r][c] x[c], scaled by k; W is (rows,
// cols) at w[at] and the bias follows it unless bias is false.
cf_linear :: (w:[]f32,at:int,rows:int,cols:int,bias:bool,x:[]f32,k:f64,out:[]f32) {
    for r:0..<rows {
        t:PsionicAccumulator;if bias {t=psionic_ops_accumulate(t,f64(w[at+rows*cols+r]));}
        for c:0..<cols {t=psionic_ops_accumulate(t,f64(w[at+r*cols+c])*f64(x[c]));}
        out[r]=f32(psionic_ops_total(t)*k);
    }
}
// Layer norm over one frame: gain at w[at], shift after it.
cf_norm :: (w:[]f32,at:int,d:int,x:[]f32,out:[]f32) {
    m:PsionicAccumulator;for i:0..<d {m=psionic_ops_accumulate(m,f64(x[i]));}
    mean:=psionic_ops_total(m)/f64(d);
    v:PsionicAccumulator;for i:0..<d {e:=f64(x[i])-mean;v=psionic_ops_accumulate(v,e*e);}
    inv:=1/sqrt(psionic_ops_total(v)/f64(d)+CF_EPSILON);
    for i:0..<d {out[i]=f32((f64(x[i])-mean)*inv*f64(w[at+i])+f64(w[at+d+i]));}
}
```

The feed-forward module widens each frame to the hidden size, applies swish
(x times the logistic function of x), and narrows it back. A Conformer layer
has two, one at each end, and each adds half of its output to the frame: the
"macaron" arrangement.

<!-- source: psionic/lib/conformer.coax feed-forward -->
```jai
// Norm, widen, swish, narrow; half of the result joins the residual.
cf_feed_forward :: (s:CfShape,w:[]f32,a:CfFf,h:[]f32) {
    d:=s.dim;n:[CF_DIM_MAX]f32;z:[CF_HIDDEN_MAX]f32;y:[CF_DIM_MAX]f32;
    cf_norm(w,a.norm,d,h,n[..]);cf_linear(w,a.w1,s.hidden,d,true,n[..],1,z[..]);
    for i:0..<s.hidden {z[i]=f32(cf_silu(f64(z[i])));}
    cf_linear(w,a.w2,d,s.hidden,true,z[..],1,y[..]);
    for i:0..<d {h[i]=f32(f64(h[i])+0.5*f64(y[i]));}
}
```

## Relative positions

A Transformer-XL score between query frame i and key frame j has two parts.
The content part compares the query with the key. The position part compares
the query with an embedding of the distance i − j, so the same distance
scores the same way anywhere in the utterance:

score(i, j) = ((q_i + u) · k_j + (q_i + v) · p_(i−j)) / √(head width)

Here u and v are learned per-head vectors, and p_r is a learned linear map
applied to a sinusoid of r.

<!-- source: psionic/lib/conformer.coax positions -->
```jai
// The sinusoid of relative position pos: entry 2m is sin(pos f_m) and 2m + 1
// is cos(pos f_m), with f_m = 10000^(-2m/dim).
cf_position :: (pos:int,d:int,out:[]f32) {
    for m:0..<d/2 {
        f:=exp(f64(2*m)*-(log(10000.0)/f64(d)));
        out[2*m]=f32(sin(f64(pos)*f));out[2*m+1]=f32(cos(f64(pos)*f));
    }
}
```

### The shift, by hand

NeMo does not look up p_(i−j) for each pair. It builds a table of the
positions T − 1 down to −(T − 1), computes every query against every row of
it, and then moves the entries into place with a reshape. Take T = 2: the
table rows hold positions 1, 0 and −1, and the scores of the two queries
against them are

```text
query 0:  10  11  12      (positions 1, 0, -1)
query 1:  20  21  22
```

Pad a zero column on the left and read the eight numbers in order:
0 10 11 12 0 20 21 22. Read them as 4 rows of 2, drop the first row (0 10),
and read the remaining six as 2 rows of 3: (11 12 0) and (20 21 22). Keep the
first 2 columns:

```text
query 0:  11  12          (key 0: position 0; key 1: position -1)
query 1:  20  21          (key 0: position 1; key 1: position 0)
```

Every entry now sits where i − j says it should, and the padding zero was
dropped. The transcript's second line shows `cf_shift` doing exactly this.
`cf_shift_source` gives, for output (i, j), the cell of the score matrix it
reads, by the same reshape arithmetic: a flat index in the padded matrix,
then its row and column.

<!-- source: psionic/lib/conformer.coax shift -->
```jai
// Row k of the position table holds position tokens - 1 - k, so one query's
// position scores run from tokens - 1 down to -(tokens - 1). The shift pads a
// zero column on the left of the (tokens, 2 tokens - 1) score matrix, reads
// the result as 2 tokens rows of tokens, drops the first row, reads the rest
// as tokens rows of 2 tokens - 1, and keeps the first tokens columns. This
// gives the cell of that matrix an output (i, j) reads; column -1 is the pad.
CfCell :: struct {row:int;column:int;}
cf_shift_source :: (tokens:int,i:int,j:int)->CfCell {
    flat:=tokens+i*(2*tokens-1)+j;   // in the padded (tokens, 2 tokens) matrix
    return CfCell.{row=flat/(2*tokens),column=flat%(2*tokens)-1};
}
cf_shift :: (tokens:int,full:[]f64,out:[]f64) {
    for i:0..<tokens {for j:0..<tokens {
        c:=cf_shift_source(tokens,i,j);v:=0.0;if c.column>=0 {v=full[c.row*(2*tokens-1)+c.column];}
        out[i*tokens+j]=v;
    }}
}
```

A `#forall` check runs the shift for every T from 1 to 32 and requires each
output to hold position i − j, read from row i, and never the padding.

## Attention

The softmax subtracts the largest score before taking exponentials, so no
exponential exceeds 1. It finds the largest with a comparison-free maximum
(chapter 158's form) so the check can distinguish every choice it makes.

<!-- source: psionic/lib/conformer.coax softmax -->
```jai
// Scores to probabilities in place, the largest subtracted before exp.
cf_softmax :: (score:[]f64) {
    top:=score[0];for j:1..<score.count {top=cf_fmax(top,score[j]);}
    z:PsionicAccumulator;for j:0..<score.count {score[j]=exp(score[j]-top);z=psionic_ops_accumulate(z,score[j]);}
    total:=psionic_ops_total(z);for j:0..<score.count {score[j]/=total;}
}
```

The attention reads only an item's first `count` frames: a padded key is
never scored. The position term goes through the shift above, as NeMo
computes it; the reference computes p_(i−j) for each pair directly instead.

<!-- source: psionic/lib/conformer.coax attention -->
```jai
// Multi-head self-attention over an item's first count frames of h (tokens by
// dim, f32), with Transformer-XL relative positions: the score of query i
// and key j is ((q_i + u) . k_j + (q_i + v) . p_(i - j)) / sqrt(head width),
// where p is the position map applied to the sinusoid table. The position
// term goes through the shift above, as NeMo computes it. Keys past count
// are never read. work holds 6 x count x dim values.
cf_attention :: (s:CfShape,w:[]f32,a:CfAtt,h:[]f32,count:int,work:[]f32) {
    d:=s.dim;dk:=d/s.heads;n:=count*d;
    q:=cf_part(work,0,n);k:=cf_part(work,1,n);v:=cf_part(work,2,n);ctx:=cf_part(work,3,n);p:=work[4*n..<6*n-d];
    x:[CF_DIM_MAX]f32;
    for t:0..<count {
        cf_norm(w,a.norm,d,cf_part(h,t,d),x[..]);
        cf_linear(w,a.q,d,d,true,x[..],1,cf_part(q,t,d));cf_linear(w,a.k,d,d,true,x[..],1,cf_part(k,t,d));
        cf_linear(w,a.v,d,d,true,x[..],1,cf_part(v,t,d));
    }
    for r:0..<2*count-1 {cf_position(count-1-r,d,x[..]);cf_linear(w,a.pos,d,d,false,x[..],1,cf_part(p,r,d));}
    scale:=1/sqrt(f64(dk));score:[CF_TOKENS_MAX]f64;
    for hd:0..<s.heads {for i:0..<count {
        for j:0..<count {
            ac:PsionicAccumulator;bd:PsionicAccumulator;c:=cf_shift_source(count,i,j);
            for m:hd*dk..<(hd+1)*dk {
                ac=psionic_ops_accumulate(ac,(f64(q[i*d+m])+f64(w[a.u+m]))*f64(k[j*d+m]));
                if c.column>=0 {bd=psionic_ops_accumulate(bd,(f64(q[c.row*d+m])+f64(w[a.bias_v+m]))*f64(p[c.column*d+m]));}
            }
            score[j]=(psionic_ops_total(ac)+psionic_ops_total(bd))*scale;
        }
        cf_softmax(score[0..<count]);
        for m:hd*dk..<(hd+1)*dk {
            o:PsionicAccumulator;for j:0..<count {o=psionic_ops_accumulate(o,score[j]*f64(v[j*d+m]));}
            ctx[i*d+m]=f32(psionic_ops_total(o));
        }
    }}
    y:[CF_DIM_MAX]f32;
    for t:0..<count {
        cf_linear(w,a.o,d,d,true,cf_part(ctx,t,d),1,y[..]);
        for m:0..<d {h[t*d+m]=f32(f64(h[t*d+m])+f64(y[m]));}
    }
}
```

Two of the biases are invisible in the output. A bias on the keys adds
(q_i + u) · b to every score in query i's row, and a bias on the position map
adds (q_i + v) · b to every score in the row; softmax ignores a constant added
to a whole row. The check therefore also compares the intermediate tensors,
q, k, v and the position table, against the reference, so a missing bias is
caught even though the output would not show it.

## The convolution module

The convolution module mixes each channel over nearby frames. After a layer
norm, a pointwise map doubles the width, and a gated linear unit halves it
again: the first half times the logistic function of the second. A
depthwise convolution then runs over time, one kernel per channel, centred on
the frame. Taps that fall before the first frame or past the item's last one
read nothing.

With one channel, taps (1, 2, 3) and gated values (1, 10, 100) on three
frames, and a zero bias:

- frame 0 reads frames −1, 0, 1: 2 × 1 + 3 × 10 = 32 (frame −1 is absent)
- frame 1 reads frames 0, 1, 2: 1 × 1 + 2 × 10 + 3 × 100 = 321
- frame 2 reads frames 1, 2, 3: 1 × 10 + 2 × 100 = 210 (frame 3 is absent)

Batch norm uses stored statistics, as at inference:
(x − mean) / √(variance + 10⁻⁵) × gain + shift. Swish and a pointwise map
finish the module.

<!-- source: psionic/lib/conformer.coax convolution -->
```jai
// Norm, pointwise to twice the width, a gated linear unit (first half times
// the sigmoid of the second), a depthwise convolution over time whose taps
// past either end of the item read nothing, batch norm with stored
// statistics, swish, pointwise back; the result joins the residual.
cf_convolution :: (s:CfShape,w:[]f32,a:CfConv,h:[]f32,count:int,work:[]f32) {
    d:=s.dim;g:=work[0..<count*d];n:[CF_DIM_MAX]f32;two:[2*CF_DIM_MAX]f32;
    for t:0..<count {
        cf_norm(w,a.norm,d,cf_part(h,t,d),n[..]);cf_linear(w,a.pw1,2*d,d,true,n[..],1,two[..]);
        for c:0..<d {g[t*d+c]=f32(f64(two[c])*cf_sigmoid(f64(two[d+c])));}
    }
    half:=(s.kernel-1)/2;y:[CF_DIM_MAX]f32;
    for t:0..<count {
        for c:0..<d {
            acc:PsionicAccumulator;acc=psionic_ops_accumulate(acc,f64(w[a.dw+d*s.kernel+c]));
            for j:0..<s.kernel {u:=t+j-half;if u<0 || u>=count {continue;}acc=psionic_ops_accumulate(acc,f64(w[a.dw+c*s.kernel+j])*f64(g[u*d+c]));}
            b:=(psionic_ops_total(acc)-f64(w[a.bn+c]))/sqrt(f64(w[a.bn+d+c])+CF_EPSILON)*f64(w[a.bn+2*d+c])+f64(w[a.bn+3*d+c]);
            n[c]=f32(cf_silu(b));
        }
        cf_linear(w,a.pw2,d,d,true,n[..],1,y[..]);
        for c:0..<d {h[t*d+c]=f32(f64(h[t*d+c])+f64(y[c]));}
    }
}
```

## A layer

<!-- source: psionic/lib/conformer.coax layer -->
```jai
// Macaron order: half feed-forward, attention, convolution, half
// feed-forward, then a layer norm. Each piece adds to the running frames.
cf_layer :: (s:CfShape,w:[]f32,l:CfLayer,h:[]f32,count:int,work:[]f32) {
    d:=s.dim;
    for t:0..<count {cf_feed_forward(s,w,l.ff1,cf_part(h,t,d));}
    cf_attention(s,w,l.att,h,count,work);cf_convolution(s,w,l.conv,h,count,work);
    n:[CF_DIM_MAX]f32;
    for t:0..<count {
        cf_feed_forward(s,w,l.ff2,cf_part(h,t,d));cf_norm(w,l.out,d,cf_part(h,t,d),n[..]);
        for c:0..<d {h[t*d+c]=n[c];}
    }
}
```

## Subsampling and masks

The subsampling runs over the whole padded batch at once, through chapter
142's convolution. Before the first convolution, every frame past an item's
length is replaced by zero, whatever it held. After each ReLU stage, every
frame at or past the item's length at that stage is zeroed again. Without
that, a padded frame would hold the bias, and the next stage's kernel, which
reaches one frame past the end when a length is odd, would read it. With it,
each stage sees exactly the zeros it would see if the item ran alone.

NeMo's subsampling may not zero frames between stages; this chapter does, so
that a padded batch gives each item's result alone. Chapter 153 compares the
two on recorded tensors.

The depthwise and pointwise stages write their output over their input:
chapter 142 finishes every output value in its scratch array before it
copies, so this is safe, and the stages need only one buffer between them.

<!-- source: psionic/lib/conformer.coax subsample -->
```jai
// One convolution through chapter 142 with the kernel at w[at] and its bias
// after it. y may be x: chapter 142 finishes every output in scratch before
// it copies. Admission (cf_encode) rules out every refusal.
cf_conv :: (c:PxConv,x:[]f32,w:[]f32,at:int,y:[]f32,scratch:[]f32)->PxStatus {
    k:=c.filters*c.channels/c.groups*c.kernel_h*c.kernel_w;
    return px_conv_direct(c,x,w[at..<at+k],w[at+k..<at+k+c.filters],y,scratch,false).status;
}
// ReLU, and zero every frame of a (batch, channels, time, freq) tensor at or
// past each item's valid length after the given number of stages.
cf_mask :: (buf:[]f32,batch:int,channels:int,t:int,f:int,lengths:[]int,stages:int) {
    for b:0..<batch {
        valid:=lengths[b];for k:0..<stages {valid=cf_stage(valid);}
        for c:0..<channels {for r:0..<t {for x:0..<f {
            i:=((b*channels+c)*t+r)*f+x;v:=cf_fmax(f64(buf[i]),0);if r>=valid {v=0;}buf[i]=f32(v);
        }}}
    }
}
// Features (batch, mels, frames) to (batch, t3, dim): stride-2 convolutions,
// masked after each stage so a padded frame never reaches a valid one, then
// the linear map (and sqrt(dim) when xscale). work is the plan's.
cf_subsample :: (s:CfShape,w:[]f32,x:[]f32,lengths:[]int,batch:int,frames:int,work:[]f32,out:[]f32) {
    p:=cf_plan(s,batch,frames);m:=s.mels;c:=s.channels;sub:=cf_sub_at(s);
    a:=work[0..<p.input];scratch:=work[p.at_scratch..<p.at_conv];y:=work[p.at_conv..<p.at_layer];
    for i:0..<batch {for t:0..<frames {for f:0..<m {
        v:f32=0;if t<lengths[i] {v=x[(i*m+f)*frames+t];}a[(i*frames+t)*m+f]=v;
    }}}
    n2:=batch*c*p.t2*p.f2;n3:=batch*c*p.t3*p.f3;
    #assert cf_conv(cf_conv0(s,batch,frames),a,w,sub.conv0,y,scratch)==PxStatus.ok;cf_mask(y,batch,c,p.t1,p.f1,lengths,1);
    #assert cf_conv(cf_depthwise(s,batch,p.t1,p.f1),y,w,sub.dw1,y[0..<n2],scratch[0..<n2])==PxStatus.ok;
    #assert cf_conv(cf_pointwise(s,batch,p.t2,p.f2),y[0..<n2],w,sub.pw1,y[0..<n2],scratch[0..<n2])==PxStatus.ok;
    cf_mask(y,batch,c,p.t2,p.f2,lengths,2);
    #assert cf_conv(cf_depthwise(s,batch,p.t2,p.f2),y[0..<n2],w,sub.dw2,y[0..<n3],scratch[0..<n3])==PxStatus.ok;
    #assert cf_conv(cf_pointwise(s,batch,p.t3,p.f3),y[0..<n3],w,sub.pw2,y[0..<n3],scratch[0..<n3])==PxStatus.ok;
    cf_mask(y,batch,c,p.t3,p.f3,lengths,3);
    // Each frame's channels and frequencies, channel-major, map to the width.
    k:=1.0;if s.xscale {k=sqrt(f64(s.dim));}
    wide:=c*p.f3;feature:[CF_FEATURES_MAX]f32;d:=s.dim;
    for i:0..<batch {for t:0..<p.t3 {
        row:=cf_part(out,i*p.t3+t,d);
        if t>=cf_length(lengths[i]) {for e:0..<d {row[e]=0;}continue;}
        for ch:0..<c {for f:0..<p.f3 {feature[ch*p.f3+f]=y[((i*c+ch)*p.t3+t)*p.f3+f];}}
        cf_linear(w,sub.out,d,wide,true,feature[..],k,row);
    }}
}
```

## Encoding a batch

The encoder admits everything before it writes anything: the shape, the
lengths, the buffers and the numbers. Then it subsamples the batch and runs
every layer on each item's valid frames.

<!-- source: psionic/lib/conformer.coax encode -->
```jai
// Features (batch, mels, frames), each item valid for its first lengths[b]
// frames; out (batch, t3, dim) with frames past an item's encoded length
// zero; out_lengths[b] the encoded length. Nothing is written on refusal.
cf_encode :: (s:CfShape,w:[]f32,x:[]f32,lengths:[]int,batch:int,frames:int,work:[]f32,out:[]f32,out_lengths:[]int,overlap:bool)->CfResult {
    p:=cf_plan(s,batch,frames);if p.status!=CfStatus.ok {return CfResult.{status=p.status};}
    if lengths.count!=batch || x.count!=p.input || w.count!=p.weights {return CfResult.{status=CfStatus.shape};}
    for n:lengths {if n<1 || n>frames {return CfResult.{status=CfStatus.length};}}
    if out.count<p.output || out_lengths.count<batch || work.count<p.work {return CfResult.{status=CfStatus.capacity};}
    if overlap {return CfResult.{status=CfStatus.alias};}
    if !cf_weights_ok(s,w) || !cf_features_ok(x,lengths,s.mels,frames) {return CfResult.{status=CfStatus.number};}
    h:=work[p.at_output..<p.work];layer:=work[p.at_layer..<p.at_output];each:=p.t3*s.dim;
    cf_subsample(s,w,x,lengths,batch,frames,work,h);
    for b:0..<batch {for k:0..<s.layers {
        cf_layer(s,w,cf_layer_at(s,cf_layer_base(s,k)),cf_part(h,b,each),cf_length(lengths[b]),layer);
    }}
    for i:0..<p.output {out[i]=h[i];}
    for b:0..<batch {out_lengths[b]=cf_length(lengths[b]);}
    return CfResult.{written=p.output};
}
```

The example encodes one second and 0.6 seconds of chapter 151's test signal
as a batch padded to 101 frames, then each alone, and compares:

<!-- source: psionic/examples/speech-encoder.coax encode -->
```jai
    s:=CfShape.{mels=128,channels=4,dim=16,heads=2,hidden=64,kernel=9,layers=2,vocab=8,xscale=true};
    p:=cf_plan(s,2,T);w:[16384]f32;work:[81920]f32;out:[2*13*16]f32;got:[2]int;
    print("plan: % weights; time 101 -> % -> % -> %, mels 128 -> % -> % -> %; work % values\n",p.weights,p.t1,p.t2,p.t3,p.f1,p.f2,p.f3,p.work);
    #assert cf_init(s,w[0..<p.weights],152)==CfStatus.ok;
    r:=cf_encode(s,w[0..<p.weights],x[..],lengths[..],2,T,work[..],out[..],got[..],false);
    print("batch: %, % values, encoded lengths % and %\n",r.status,r.written,got[0],got[1]);
    for b:0..<2 {
        L:=lengths[b];one:[128*101]f32;for m:0..<128 {for t:0..<L {one[m*L+t]=x[(b*128+m)*T+t];}}
        alone:[13*16]f32;n:[1]int;only:=[1]int.[L];
        a:=cf_encode(s,w[0..<p.weights],one[0..<128*L],only[..],1,L,work[..],alone[..],n[..],false);
        same:=a.status==CfStatus.ok && px_same(out[b*208..<b*208+n[0]*16],alone[0..<n[0]*16]);
        zero:=true;for i:n[0]*16..<208 {if out[b*208+i]!=0 {zero=false;}}
        ref:[13*16]f64;rw:[48000]f64;k:=cf_ref_encode(s,w[0..<p.weights],one[0..<128*L],L,L,ref[..],rw[..]);
        worst:=cf_worst(alone[0..<k*16],ref[0..<k*16]);
        print("item %: alone equals the batch bit for bit: %; padded frames zero: %; within 1e-4 of the binary64 reference: %\n",b,same,zero,k==n[0] && worst<=0.0001);
    }
    print("item 0, frame 0, first 4 of 16: % % % %\n",round4(out[0]),round4(out[1]),round4(out[2]),round4(out[3]));
    
```

The batch gives 13 and 8 encoded frames, as the length formula says for 101
and 61 input frames, and each item's frames are bit for bit the same as when
it runs alone. That is exact, not within a tolerance, because a padded item
does the same arithmetic in the same order as the item alone. A check puts
NaN in every padded frame of a three-item batch and gets the same result.

## The CTC head and greedy decoding

A connectionist temporal classification (CTC) head scores each encoded frame
against every token and one extra class, the blank, which means "no new
token here". Greedy decoding takes each frame's best class, merges a class
that repeats the frame before it, and drops blanks. Take best classes, with
a, b and c the tokens and _ the blank:

```text
a  a  _  a  b  b  _  _  c
```

Merging repeats gives a _ a b _ c; dropping blanks gives a a b c. The blank
between the two a's is what keeps them separate: without it, "a a" would
merge into one a. The transcript's last line shows `cf_greedy` giving 0 0 1
2 for this sequence.

<!-- source: psionic/lib/conformer.coax head -->
```jai
// One frame's vocab + 1 log-probabilities: the linear map, then log-softmax
// with the largest score subtracted first.
cf_head_row :: (s:CfShape,w:[]f32,frame:[]f32,out:[]f32) {
    k:=s.vocab+1;z:[CF_VOCAB_MAX+1]f32;cf_linear(w,cf_head_at(s),k,s.dim,true,frame,1,z[..]);
    top:=f64(z[0]);for c:1..<k {top=cf_fmax(top,f64(z[c]));}
    sum:PsionicAccumulator;for c:0..<k {sum=psionic_ops_accumulate(sum,exp(f64(z[c])-top));}
    lse:=top+log(psionic_ops_total(sum));
    for c:0..<k {out[c]=f32(f64(z[c])-lse);}
}
// The CTC head over encoded frames (batch, frames, dim). Frames past an
// item's encoded length get zeros and are not read.
cf_head :: (s:CfShape,w:[]f32,enc:[]f32,lengths:[]int,batch:int,frames:int,logp:[]f32,overlap:bool)->CfResult {
    status:=cf_shape_check(s);if status!=CfStatus.ok {return CfResult.{status=status};}
    if batch<1 || frames<1 || lengths.count!=batch || enc.count!=batch*frames*s.dim || w.count!=cf_weight_count(s) {return CfResult.{status=CfStatus.shape};}
    for n:lengths {if n<1 || n>frames {return CfResult.{status=CfStatus.length};}}
    k:=s.vocab+1;if logp.count<batch*frames*k {return CfResult.{status=CfStatus.capacity};}
    if overlap {return CfResult.{status=CfStatus.alias};}
    if !cf_weights_ok(s,w) || !cf_frames_ok(enc,lengths,frames,s.dim) {return CfResult.{status=CfStatus.number};}
    for b:0..<batch {for t:0..<frames {
        out:=cf_part(logp,b*frames+t,k);
        if t>=lengths[b] {for c:0..<k {out[c]=0;}continue;}
        cf_head_row(s,w,cf_part(enc,b*frames+t,s.dim),out);
    }}
    return CfResult.{written=batch*frames*k};
}
// Greedy CTC: each frame's best class (the lowest on a tie); drop a class
// equal to the frame before's, then drop blanks. The blank is the last class.
cf_greedy :: (logp:[]f32,frames:int,classes:int,tokens:[]int)->CfResult {
    if frames<1 || classes<2 || logp.count<frames*classes {return CfResult.{status=CfStatus.shape};}
    if tokens.count<frames {return CfResult.{status=CfStatus.capacity};}
    prev:=-1;n:=0;
    for t:0..<frames {
        best:=0;for c:1..<classes {if logp[t*classes+c]>logp[t*classes+best] {best=c;}}
        if best!=classes-1 && best!=prev {tokens[n]=best;n+=1;}
        prev=best;
    }
    return CfResult.{written=n};
}
```

Parakeet's decoder is a token-and-duration transducer, not CTC; chapter 153
builds it. CTC is the simplest head that turns encoder frames into tokens, so
this chapter uses it to show the encoder's output in use.

## Refusals

Each broken input is refused with its status, and the output and lengths keep
their old values:

<!-- source: psionic/examples/speech-encoder-refuse.coax refuse -->
```jai
main :: () {
    s:=cf_tiny();w:[1494]f32;#assert cf_init(s,w[..],1)==CfStatus.ok;
    b:=s;b.heads=3;print("width 8 in 3 heads: %\n",cf_shape_check(b));
    b=s;b.kernel=4;print("an even kernel: %\n",cf_shape_check(b));
    b=s;b.mels=129;print("129 mels: %\n",cf_shape_check(b));
    print("2,049 frames: %\n",cf_plan(s,1,2049).status);
    print("9 items: %\n",cf_plan(s,9,20).status);
    big:=s;big.mels=128;big.channels=64;print("64 channels on 2,048 frames of 128 mels: %\n",cf_plan(big,1,2048).status);

    T:=9;x:[144]f32;cf_fixture_features(x[..],16,T,1);work:[496]f32;out:[16]f32;n:=[1]int.[-7];lens:=[1]int.[9];
    for i:0..<16 {out[i]=-7;}
    print("143 features for 16 mels x 9 frames: %\n",cf_encode(s,w[..],x[0..<143],lens[..],1,T,work[..],out[..],n[..],false).status);
    lens[0]=10;print("a length of 10 in 9 frames: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    lens[0]=0;print("a length of 0: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    lens[0]=9;print("room for 15 of 16 outputs: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[0..<15],n[..],false).status);
    p:=cf_plan(s,1,T);print("work one value short of %: %\n",p.work,cf_encode(s,w[..],x[..],lens[..],1,T,work[0..<p.work-1],out[..],n[..],false).status);
    x[8]=cf_nan();print("a NaN feature: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    x[8]=10001;print("a feature of 10,001: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    x[8]=0;l:=cf_layer_at(s,cf_layer_base(s,0));w[l.conv.bn+8]=-1;
    print("a negative batch norm variance: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    w[l.conv.bn+8]=1;w[cf_sub_at(s).out+48]=1001;
    print("a bias of 1,001: %\n",cf_encode(s,w[..],x[..],lens[..],1,T,work[..],out[..],n[..],false).status);
    print("outputs still -7: % %, length still %\n",out[0],out[15],n[0]);
    logp:[8]f32;print("greedy over 10 frames of an 8-value row: %\n",cf_greedy(logp[..],10,4,lens[..]).status);
}
```

```text
width 8 in 3 heads: shape
an even kernel: shape
129 mels: limit
2,049 frames: limit
9 items: limit
64 channels on 2,048 frames of 128 mels: limit
143 features for 16 mels x 9 frames: shape
a length of 10 in 9 frames: length
a length of 0: length
room for 15 of 16 outputs: capacity
work one value short of 496: capacity
a NaN feature: number
a feature of 10,001: number
a negative batch norm variance: number
a bias of 1,001: number
outputs still -7: -7.0 -7.0, length still -7
greedy over 10 frames of an 8-value row: shape
```

## How it is checked

The references in [`conformer-checks.coax`](../tests/conformer-checks.coax)
share no code with the encoder. They use binary64 throughout with plain sums
in index order. Convolutions copy their input into an explicitly zero-padded
array; attention fills the whole score matrix with the sinusoid of i − j
computed for each pair, with no position table and no shift; the depthwise
convolution reads an explicitly zero-padded sequence. The tolerances were set
before the first comparison:

- each piece (a feed-forward module, attention over 5 frames, the convolution
  module, a whole layer) and the subsampling: within 10⁻⁵ of the reference.
  The measured differences are at most 1.3 × 10⁻⁷. The check also requires
  each module to change its input by more than 0.01, so a module that does
  nothing fails;
- the whole two-layer encoder: within 10⁻⁴; measured, 2.2 × 10⁻⁷;
- q, k, v and the position table: within 10⁻⁵ and 10⁻⁶;
- a padded batch: equal bit for bit to each item alone, with padded output
  frames exactly 0;
- lengths: every frame count from 1 to 2,048 against (n + 7) / 8 and chapter
  142's planner; the shift for every length from 1 to 32;
- the softmax, the head's log-softmax and greedy decoding by hand, including
  scores of ±1,000 that would overflow without the subtracted maximum;
- the weight layout by hand, and every seeded weight recomputed.

`coax mutate psionic/lib/conformer.coax --root-only` makes 804 mutants of the
unit's own code; the checks kill all 804.

## What this chapter does not do

- It does not load Parakeet's weights or compare with tensors recorded from
  NeMo. Matching those, including whether NeMo zeroes padded frames between
  subsampling stages, is chapter 153's job.
- It does not stream. The attention sees the whole utterance and the
  depthwise convolution is centred; chunked, cached inference is
  [chapter 154](ch154-transcribe-while-someone-speaks.md)'s.
- It does not train. The weights are seeded, so the decoded tokens are
  arbitrary.
- It does not measure speed. Everything runs on one CPU core in a fixed
  order.

## Dependencies

- [Chapter 10](ch010-attention-without-training.md): scaled dot-product
  attention, which this chapter extends with relative positions.
- [Chapter 4](ch004-numbers-and-matrices.md): the compensated accumulator.
- [Chapter 53](ch053-choosing-the-next-token.md): the counter-based generator.
- [Chapter 142](ch142-pixels-convolutions-and-patches.md): convolution with
  stride, padding and groups.
- [Chapter 151](ch151-waveforms-and-spectral-features.md): log-mel features.
- [Chapter 158](ch158-a-field-around-a-shape.md): the comparison-free maximum.

## Sources

- [Chapter plan](PLAN.md), Part XV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Parakeet entry.
