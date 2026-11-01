# 145. Compress and reconstruct an image

> **Status: partly implemented.** A small convolutional image codec runs on
> the CPU: an encoder from 3 × 8 × 8 pixels to a 4 × 2 × 2 latent, a 4-bit
> quantizer, a decoder built from transposed convolutions, exact gradients,
> training with AdamW on procedural images, and reconstruction error measured
> on held-out images against gates frozen before the run. The plan's
> teaching target, the pinned Cosmos-0.1-Tokenizer-CI8x8 checkpoint (its
> wavelet transform, normalization and attention blocks, and its
> intermediates compared with recorded references), is not built yet.

A learned image codec maps an image to a small latent tensor and back. World
models and diffusion models work in such latents rather than in pixels,
because the latent is much smaller and keeps what matters about the image.
This chapter builds the smallest codec that shows every part of one: the
encoder that shrinks the image, the quantizer that turns the latent into a
few bits, the decoder that grows it back, and the measurement that says how
much was lost.

It builds five things, in [`image-codec.coax`](../lib/image-codec.coax):
- **upsampling** by transposed convolution, the decoder's layer, defined from
  [chapter 142](ch142-pixels-convolutions-and-patches.md)'s convolution
  gradient and checked against a direct scatter definition;
- a **quantizer** that maps each latent value to one of 15 levels, a 4-bit
  code, and packs two codes a byte;
- the **codec**: two strided convolutions down, the quantizer, two
  transposed convolutions up;
- its **gradients**, through the quantizer by the straight-through rule, and
  **training** with [chapter 124](ch124-adamw-and-explicit-precision.md)'s
  AdamW on images generated in Coax;
- **reconstruction error** (mean squared error and PSNR) and the
  **compression ratio**, on validation and test images the training never
  saw.

```shell
coax/bin/coax run psionic/examples/image-codec.coax
coax/bin/coax run psionic/examples/image-codec-refuse.coax
coax/bin/coax test psionic/lib/image-codec.coax
```

```text
upsample [1, 10] with taps [1, 2, 3, 4]: ok, 2.0 13.0 24.0 30.0
hand-set codec, codes of channel 0: 3 0 7 -3
decoded red pixels (0,0) (0,4) (4,0) (4,4): 0.42857143 0.0 1.0 -0.05357143
levels k/7 that quantize back to k: 15 of 15
image 0, red row 0: 0.28125 0.3125 0.34375 0.375 0.40625 0.4375 0.46875 0.5
mean-image baseline, test: MSE 0.046745, PSNR 13.3 dB
200 training steps: ok
  step 0: loss 0.093668
  step 50: loss 0.04206
  step 100: loss 0.024552
  step 150: loss 0.016447
  step 199: loss 0.017331
  validation after 200 steps, quantized: PSNR 17.91 dB
recorded weights, 1,500 steps:
  training, quantized: MSE 0.004784, PSNR 23.2 dB
  validation, quantized: MSE 0.004997, PSNR 23.01 dB
  test, continuous latent: MSE 0.005293, PSNR 22.76 dB
  test, quantized: MSE 0.006013, PSNR 22.21 dB
compression: 192 bytes to 8 bytes, 24 to 1
gate PSNR >= 22.0 dB: true; gate MSE <= baseline / 4.0: true
test image 0, codes: 2 0 4 3 -2 -1 -2 -2 4 3 2 0 -3 -2 -3 -3
packed: 121 171 101 85 171 121 84 68
unpacked codes equal the codes: true
  original red row 0: 0.2890625 0.2578125 0.1015625 0.0703125 0.0390625 0.0078125 0.1015625 0.0703125
  decoded red row 0: 0.339 0.3 0.223 0.177 0.153 0.169 0.138 0.17
decode(codes) equals the quantized forward pass: true
```

The [example](../examples/image-codec.coax) takes about 20 seconds in a
debug build, most of it in 200 training steps. The weights it measures come
from a longer, recorded run, described under [The recorded
run](#the-recorded-run).

## The shape of the codec

An image here is 3 channels of 8 by 8 pixels, each value in `[0, 1]`, laid
out channel by channel as in chapter 142: 192 values. The codec keeps 16.

| Layer | Operation | Output |
| --- | --- | --- |
| encoder 1 | conv 4 × 4, stride 2, padding 1, 8 filters; leaky activation | 8 × 4 × 4 |
| encoder 2 | conv 4 × 4, stride 2, padding 1, 4 filters; tanh | 4 × 2 × 2 |
| quantizer | nearest of the levels k / 7, k in −7..7 | 16 codes |
| decoder 1 | transposed conv 4 × 4, stride 2, padding 1, 8 channels; leaky | 8 × 4 × 4 |
| decoder 2 | transposed conv 4 × 4, stride 2, padding 1, 3 channels | 3 × 8 × 8 |

A kernel of 4 with stride 2 and padding 1 halves each side:
`(8 + 2·1 − 4) / 2 + 1 = 4`. The leaky activation keeps positive values and
multiplies negative ones by 1/8, so no unit stops passing gradient. Each
latent value comes out of tanh, in `(−1, 1)`, so the quantizer's 15 levels
cover its whole range.

The decoder's layers are the encoder's shapes run backwards: decoder 1
transposes a convolution shaped like encoder 2, and decoder 2 one shaped
like encoder 1. All 1,815 parameters sit in one vector:

<!-- source: psionic/lib/image-codec.coax shape -->
```jai
CZ_SIDE :: 8;               // image side, pixels
CZ_PIXELS :: 192;           // 3 channels of 8 by 8
CZ_HIDDEN :: 128;           // 8 channels of 4 by 4, between the two layers
CZ_LATENT :: 16;            // 4 channels of 2 by 2
CZ_LEVELS :: 7;             // a latent value is k / 7 for k in -7..7: 15 levels
CZ_BATCH_MAX :: 64;
CZ_LEAK :: 0.125;           // the activation's slope below zero
// One parameter vector: each layer's weights, then its bias.
CZ_E1W :: 0;      // encoder 1: 8 filters over 3 channels, 4 by 4 taps
CZ_E1B :: 384;
CZ_E2W :: 392;    // encoder 2: 4 filters over 8 channels
CZ_E2B :: 904;
CZ_D1W :: 908;    // decoder 1: transposes a conv shaped like encoder 2
CZ_D1B :: 1420;   //   bias per output channel (8)
CZ_D2W :: 1428;   // decoder 2: transposes a conv shaped like encoder 1
CZ_D2B :: 1812;   //   bias per output channel (3)
CZ_PARAMS :: 1815;
// Workspace per image: the forward pass (encoder 1 before and after its
// activation, encoder 2's output, the latent, decoder 1 before and after its
// activation, the reconstruction), then the backward pass's gradients.
CZ_FORWARD :: 736;
CZ_WORK :: 1200;
// The two convolution shapes. Kernel 4, stride 2, padding 1 halves each side.
cz_outer :: (n:int)->PxConv {
    return PxConv.{batch=n,channels=3,height=8,width=8,filters=8,kernel_h=4,kernel_w=4,stride_h=2,stride_w=2,pad_h=1,pad_w=1};
}
cz_inner :: (n:int)->PxConv {
    return PxConv.{batch=n,channels=8,height=4,width=4,filters=4,kernel_h=4,kernel_w=4,stride_h=2,stride_w=2,pad_h=1,pad_w=1};
}
```

The types every call returns:

<!-- source: psionic/lib/image-codec.coax types -->
```jai
CzStatus :: enum u8 {ok;batch;shape;code;number;capacity;}
CzResult :: struct {status:CzStatus;written:int;}
CzLoss :: struct {status:CzStatus;loss:f64;}
// Training: steps of AdamW on minibatches of batch images taken in order,
// optionally through the quantizer (straight-through), at rate lr halved
// after each quarter of the steps.
CzTrain :: struct {steps:int;batch:int;lr:f64;quantize:bool=true;}
```

## Upsampling is the convolution's gradient

A convolution with stride 2 maps many pixels to few. Its transpose maps few
to many: each input value spreads, through the kernel, onto the pixels its
window covered. Chapter 142 already computes exactly that sum: the
convolution's input gradient `dx` takes an output gradient `dy` shaped like
the convolution's output and returns values shaped like its input. Upsampling
is that sum with the decoder's values in the place of `dy`.

### Upsampling by hand

Take a 1 × 4 convolution with kernel 4, stride 2 and padding 1. It has 2
outputs; output `q` reads input pixels `2q − 1 + a` for taps `a = 0..3`.
Transposed, input `u = [1, 10]` with taps `w = [1, 2, 3, 4]` gives 4 pixels,
where pixel `r` collects `u[q]·w[a]` for every `q` and `a` with
`2q − 1 + a = r`:

| Pixel r | Taps that land on it | Value |
| --- | --- | --- |
| 0 | q = 0, a = 1 | 1·2 = 2 |
| 1 | q = 0, a = 2; q = 1, a = 0 | 1·3 + 10·1 = 13 |
| 2 | q = 0, a = 3; q = 1, a = 1 | 1·4 + 10·2 = 24 |
| 3 | q = 1, a = 2 | 10·3 = 30 |

The first line of the transcript prints `2.0 13.0 24.0 30.0`. Each output
pixel gathers from at most two inputs, through different taps: interior
pixels are the sum of two products and the edges of one.

<!-- source: psionic/lib/image-codec.coax up -->
```jai
// One value rounded once to f32, refused when it does not fit. Inputs are
// checked to be finite first, so every sum is a finite binary64 value.
cz_put :: (v:f64,out:[]f32,i:int)->bool {if !psionic_ops_fits_f32(v) {return false;}out[i]=f32(v);return true;}
// The transposed convolution of c: u is shaped like c's output (batch,
// filters, out_h, out_w), the result like c's input (batch, channels,
// height, width). Output pixel (n, ch, row, col) gathers every u value whose
// window put a tap on it, times that tap's weight: chapter 142's input
// gradient with u in the place of dy. Bias, one per output channel, may be
// empty.
cz_up_into :: (c:PxConv,p:PxConvPlan,u:[]f32,w:[]f32,bias:[]f32,out:[]f32)->bool {
    area:=c.height*c.width;
    for i:0..<p.x {
        v:=px_conv_dx(c,p,w,u,i);if bias.count>0 {v+=f64(bias[(i/area)%c.channels]);}
        if !cz_put(v,out,i) {return false;}
    }
    return true;
}
// The checked form: shapes first, then every value into scratch, and only
// when all of them fit, into out.
cz_up :: (c:PxConv,u:[]f32,w:[]f32,bias:[]f32,out:[]f32,scratch:[]f32)->CzResult {
    p:=px_conv_plan(c);if p.status!=PxStatus.ok {return CzResult.{status=CzStatus.shape};}
    if u.count!=p.y || w.count!=p.w || (bias.count!=0 && bias.count!=c.channels) {return CzResult.{status=CzStatus.shape};}
    if out.count<p.x || scratch.count<p.x {return CzResult.{status=CzStatus.capacity};}
    if !px_finite(u) || !px_finite(w) || !px_finite(bias) {return CzResult.{status=CzStatus.number};}
    if !cz_up_into(c,p,u,w,bias,scratch) {return CzResult.{status=CzStatus.number};}
    for i:0..<p.x {out[i]=scratch[i];}
    return CzResult.{written=p.x};
}
```

The check `upsampling equals its scatter definition on every setting`
writes the transpose a second way, as the textbook describes it: every input
value adds its weight-times-value to each pixel its taps reach. Over 16
combinations of stride, padding, dilation and groups, on values that are
multiples of 1/256, the two agree bit for bit. The same check confirms the
adjoint identity `⟨up(u), v⟩ = ⟨u, conv(v)⟩` exactly, which is what makes
upsampling the transpose and not just some way to grow an image.

## A latent becomes 4 bits

Each latent value `t` in `[−1, 1]` becomes the nearest level `k / 7`:
`k = floor(7t + 1/2)`, clamped to `−7..7`. That is 15 levels, which fit in 4
bits as `k + 7` in `0..14`. Two codes share a byte, the first in the low half.
The nibble 15 is never written, so reading one is a refusal, not a 16th level.

<!-- source: psionic/lib/image-codec.coax quantize -->
```jai
cz_tanh :: (a:f64)->f64 {
    t:=exp(-2*abs(a));v:=(1-t)/(1+t);
    if a<0 {return -v;}
    return v;
}
// A latent value t in [-1, 1] becomes the nearest of 15 levels k / 7: code
// k = floor(7 t + 1/2), clamped to -7..7 as (|k + 7| - |k - 7|) / 2. The
// level is k / 7 rounded once to f32.
cz_code :: (t:f64)->int {
    k:=floor(t*CZ_LEVELS+0.5);
    return int((abs(k+CZ_LEVELS)-abs(k-CZ_LEVELS))/2);
}
cz_level :: (k:int)->f32 {return f32(f64(k)/CZ_LEVELS);}
// Codes in 4 bits each, k + 7 in 0..14, the first of a pair in the low half.
cz_pack :: (codes:[]int,bytes:[]u8)->CzResult {
    if codes.count<CZ_LATENT || codes.count%CZ_LATENT!=0 {return CzResult.{status=CzStatus.shape};}
    if bytes.count<codes.count/2 {return CzResult.{status=CzStatus.capacity};}
    for k:codes {if k< -CZ_LEVELS || k>CZ_LEVELS {return CzResult.{status=CzStatus.code};}}
    for j:0..<codes.count/2 {bytes[j]=cast(u8)(codes[2*j]+CZ_LEVELS+16*(codes[2*j+1]+CZ_LEVELS));}
    return CzResult.{written=codes.count/2};
}
cz_unpack :: (bytes:[]u8,codes:[]int)->CzResult {
    if bytes.count<CZ_LATENT/2 || bytes.count%(CZ_LATENT/2)!=0 {return CzResult.{status=CzStatus.shape};}
    if codes.count<2*bytes.count {return CzResult.{status=CzStatus.capacity};}
    for b:bytes {if int(b)%16==15 || int(b)/16==15 {return CzResult.{status=CzStatus.code};}}
    for j:0..<bytes.count {codes[2*j]=int(bytes[j])%16-CZ_LEVELS;codes[2*j+1]=int(bytes[j])/16-CZ_LEVELS;}
    return CzResult.{written=2*bytes.count};
}
```

The round trip from a code is exact: level `k / 7`, rounded once to f32,
quantizes back to `k` for all 15 codes, and anything within just under half
a step (1/14 ≈ 0.0714) of a level returns that level's code. The other
direction is lossy by design: two latents closer than a step share a code.

## Encode and decode by hand

Before training, a hand-set model shows the path. Every weight is zero
except tap (1, 1) of the first input channel in each layer, which is 1.

**Encoder 1.** Output `(i, j)` of a stride-2, padding-1 conv puts tap `(a, b)`
on pixel `(2i − 1 + a, 2j − 1 + b)`, so tap (1, 1) reads pixel `(2i, 2j)` of
the red channel. Encoder 2 does the same again. Latent `(i, j)` in each of
the 4 latent channels is therefore red pixel `(4i, 4j)`, through one leak.

Give the red channel 0.5, −0.25, 2 and −3 at `(0, 0)`, `(0, 4)`, `(4, 0)`
and `(4, 4)`:

| Pixel | After the leak | tanh | 7·tanh | Code |
| --- | --- | --- | --- | --- |
| 0.5 | 0.5 | 0.4621 | 3.235 | 3 |
| −0.25 | −0.03125 | −0.0312 | −0.219 | 0 |
| 2 | 2 | 0.9640 | 6.748 | 7 |
| −3 | −0.375 | −0.3584 | −2.508 | −3 |

The last row rounds `−2.508 + 0.5 = −2.008` down to −3.

**Decoder.** Decoder 1's tap (1, 1) puts latent `(i, j)` on pixel `(2i, 2j)`
of each hidden channel, then the leak; decoder 2 puts hidden pixel `(r, s)`
on output pixel `(2r, 2s)`. So output pixel `(4i, 4j)` of every channel is
the level `k / 7`, leaked when negative: `3/7 = 0.42857143`, 0, 1 and
`−3/7 · 1/8 = −0.05357143`. Every other output pixel is 0. Lines 2 and 3 of
the transcript print these codes and values.

<!-- source: psionic/lib/image-codec.coax forward -->
```jai
// The leak without a branch: max(v, 0) + (min(v, 0)) / 8, in binary64,
// where each part is exact.
cz_leak :: (v:f32)->f32 {a:=f64(v);return f32((a+abs(a))/2+CZ_LEAK*((a-abs(a))/2));}
cz_slope :: (v:f32)->f64 {if v>0 {return 1;}return CZ_LEAK;}
cz_conv_into :: (c:PxConv,p:PxConvPlan,x:[]f32,w:[]f32,bias:[]f32,out:[]f32)->bool {
    for i:0..<p.y {if !cz_put(px_conv_value(c,p,x,w,bias,i),out,i) {return false;}}
    return true;
}
// Workspace regions, each times the batch n: encoder 1 before and after the
// activation, encoder 2's output, the latent, decoder 1 before and after the
// activation, the reconstruction.
CZ_A1 :: 0;CZ_H1 :: 128;CZ_A2 :: 256;CZ_Z :: 272;CZ_A3 :: 288;CZ_H3 :: 416;CZ_Y :: 544;
// Everything from position at on; each reader and writer stays in its region.
cz_from :: (v:[]f32,at:int)->[]f32 {return v[at..<v.count];}
cz_encoder :: (w:[]f32,x:[]f32,n:int,ws:[]f32)->bool {
    outer:=cz_outer(n);inner:=cz_inner(n);po:=px_conv_plan(outer);pi:=px_conv_plan(inner);
    if !cz_conv_into(outer,po,x,cz_from(w,CZ_E1W),cz_from(w,CZ_E1B),cz_from(ws,n*CZ_A1)) {return false;}
    for i:0..<po.y {ws[n*CZ_H1+i]=cz_leak(ws[n*CZ_A1+i]);}
    return cz_conv_into(inner,pi,cz_from(ws,n*CZ_H1),cz_from(w,CZ_E2W),cz_from(w,CZ_E2B),cz_from(ws,n*CZ_A2));
}
// The latent in its region becomes the reconstruction in its region.
cz_decoder :: (w:[]f32,n:int,ws:[]f32)->bool {
    outer:=cz_outer(n);inner:=cz_inner(n);po:=px_conv_plan(outer);pi:=px_conv_plan(inner);
    if !cz_up_into(inner,pi,cz_from(ws,n*CZ_Z),cz_from(w,CZ_D1W),cz_from(w,CZ_D1B),cz_from(ws,n*CZ_A3)) {return false;}
    for i:0..<pi.x {ws[n*CZ_H3+i]=cz_leak(ws[n*CZ_A3+i]);}
    return cz_up_into(outer,po,cz_from(ws,n*CZ_H3),cz_from(w,CZ_D2W),cz_from(w,CZ_D2B),cz_from(ws,n*CZ_Y));
}
// The whole model on n admitted images; the latent is tanh of encoder 2's
// output, through the quantizer when quantize is set.
cz_forward :: (w:[]f32,x:[]f32,n:int,ws:[]f32,quantize:bool)->CzStatus {
    if !cz_encoder(w,x,n,ws) {return CzStatus.number;}
    pi:=px_conv_plan(cz_inner(n));
    for i:0..<pi.y {
        t:=cz_tanh(f64(ws[n*CZ_A2+i]));ws[n*CZ_Z+i]=f32(t);
        if quantize {ws[n*CZ_Z+i]=cz_level(cz_code(t));}
    }
    if !cz_decoder(w,n,ws) {return CzStatus.number;}
    return CzStatus.ok;
}
```

`cz_encode` stops after the quantizer and returns the codes; `cz_decode`
starts from codes and returns pixels. Decoding a code gives exactly what the
forward pass with quantization gives, bit for bit; the example checks this on
a test image.

<!-- source: psionic/lib/image-codec.coax codec -->
```jai
cz_admit :: (w:[]f32,n:int)->CzStatus {
    if n<1 || n>CZ_BATCH_MAX {return CzStatus.batch;}
    if w.count!=CZ_PARAMS {return CzStatus.shape;}
    return CzStatus.ok;
}
// Images to codes, 16 an image in (channel, row, column) order.
cz_encode :: (w:[]f32,x:[]f32,n:int,codes:[]int,ws:[]f32)->CzResult {
    s:=cz_admit(w,n);if s!=CzStatus.ok {return CzResult.{status=s};}
    if x.count!=n*CZ_PIXELS {return CzResult.{status=CzStatus.shape};}
    if codes.count<n*CZ_LATENT || ws.count<n*CZ_FORWARD {return CzResult.{status=CzStatus.capacity};}
    if !px_finite(x) || !px_finite(w) {return CzResult.{status=CzStatus.number};}
    if !cz_encoder(w,x,n,ws) {return CzResult.{status=CzStatus.number};}
    pi:=px_conv_plan(cz_inner(n));
    for i:0..<pi.y {codes[i]=cz_code(cz_tanh(f64(ws[n*CZ_A2+i])));}
    return CzResult.{written=pi.y};
}
// Codes to images: each code k is the latent value k / 7.
cz_decode :: (w:[]f32,codes:[]int,n:int,out:[]f32,ws:[]f32)->CzResult {
    s:=cz_admit(w,n);if s!=CzStatus.ok {return CzResult.{status=s};}
    if codes.count!=n*CZ_LATENT {return CzResult.{status=CzStatus.shape};}
    if out.count<n*CZ_PIXELS || ws.count<n*CZ_FORWARD {return CzResult.{status=CzStatus.capacity};}
    for k:codes {if k< -CZ_LEVELS || k>CZ_LEVELS {return CzResult.{status=CzStatus.code};}}
    if !px_finite(w) {return CzResult.{status=CzStatus.number};}
    for i:0..<codes.count {ws[n*CZ_Z+i]=cz_level(codes[i]);}
    if !cz_decoder(w,n,ws) {return CzResult.{status=CzStatus.number};}
    po:=px_conv_plan(cz_outer(n));
    for i:0..<po.x {out[i]=ws[n*CZ_Y+i];}
    return CzResult.{written=po.x};
}
```

## Gradients

Training needs the gradient of the mean squared error

`L = (1 / 192n) · Σ (y − x)²`

over a batch of `n` images with respect to every parameter. Chapter 142
provides the pieces, and each layer's rule follows from it:

- An encoder layer is chapter 142's convolution, so its weight, bias and
  input gradients are chapter 142's `dw`, `db` and `dx`.
- An upsampling layer is `y = Aᵀu`, where `A` is the convolution it
  transposes. Its input gradient is `A·dy`, the convolution itself applied
  to the output gradient. Its weight gradient is chapter 142's `dw` with the
  convolution's input and output swapped: `dy` in the place of the image and
  `u` in the place of the output gradient.
- The leak multiplies the gradient by 1 or 1/8, by the sign of its input.
- tanh multiplies it by `1 − t²`.
- The quantizer has no useful gradient (it is flat between levels), so the
  gradient passes straight through it, as if it were the identity. This is
  the straight-through estimator.

<!-- source: psionic/lib/image-codec.coax backward -->
```jai
// Mean squared error over every pixel and its gradient for every parameter.
// An upsampling layer's input gradient is the convolution it transposes,
// applied to its output gradient; its weight gradient is chapter 142's
// weight gradient with input and output swapped; its bias gradient sums its
// output gradient per channel: chapter 142's bias gradient of a channel-by-
// channel 1 by 1 convolution with that output. The quantizer passes its gradient straight
// through; tanh multiplies it by 1 - t^2.
CZ_DY :: 736;CZ_DA3 :: 928;CZ_DZ :: 1056;CZ_DA1 :: 1072;
cz_sums :: (c:PxConv)->PxConv {return PxConv.{batch=c.batch,channels=c.channels,height=c.height,width=c.width,filters=c.channels,kernel_h=1,kernel_w=1,groups=c.channels};}
cz_gradient :: (w:[]f32,x:[]f32,n:int,quantize:bool,ws:[]f32,g:[]f64)->CzLoss {
    s:=cz_admit(w,n);if s!=CzStatus.ok {return CzLoss.{status=s};}
    if x.count!=n*CZ_PIXELS || g.count!=CZ_PARAMS {return CzLoss.{status=CzStatus.shape};}
    if ws.count<n*CZ_WORK {return CzLoss.{status=CzStatus.capacity};}
    if !px_finite(x) || !px_finite(w) {return CzLoss.{status=CzStatus.number};}
    s=cz_forward(w,x,n,ws,quantize);if s!=CzStatus.ok {return CzLoss.{status=s};}
    outer:=cz_outer(n);inner:=cz_inner(n);po:=px_conv_plan(outer);pi:=px_conv_plan(inner);
    so:=cz_sums(outer);si:=cz_sums(inner);pso:=px_conv_plan(so);psi:=px_conv_plan(si);
    none:[1]f32;total:=0.0;scale:=2.0/f64(po.x);
    for i:0..<po.x {
        e:=f64(ws[n*CZ_Y+i])-f64(x[i]);total+=e*e;
        if !cz_put(scale*e,ws,n*CZ_DY+i) {return CzLoss.{status=CzStatus.number};}
    }
    // Decoder 2.
    for f:0..<so.filters {g[CZ_D2B+f]=px_conv_db(so,pso,cz_from(ws,n*CZ_DY),f);}
    for i:0..<po.w {g[CZ_D2W+i]=px_conv_dw(outer,po,cz_from(ws,n*CZ_DY),cz_from(ws,n*CZ_H3),i);}
    for i:0..<po.y {
        v:=px_conv_value(outer,po,cz_from(ws,n*CZ_DY),cz_from(w,CZ_D2W),none[0..<0],i)*cz_slope(ws[n*CZ_A3+i]);
        if !cz_put(v,ws,n*CZ_DA3+i) {return CzLoss.{status=CzStatus.number};}
    }
    // Decoder 1.
    for f:0..<si.filters {g[CZ_D1B+f]=px_conv_db(si,psi,cz_from(ws,n*CZ_DA3),f);}
    for i:0..<pi.w {g[CZ_D1W+i]=px_conv_dw(inner,pi,cz_from(ws,n*CZ_DA3),cz_from(ws,n*CZ_Z),i);}
    for i:0..<pi.y {
        t:=cz_tanh(f64(ws[n*CZ_A2+i]));
        v:=px_conv_value(inner,pi,cz_from(ws,n*CZ_DA3),cz_from(w,CZ_D1W),none[0..<0],i)*(1-t*t);
        if !cz_put(v,ws,n*CZ_DZ+i) {return CzLoss.{status=CzStatus.number};}
    }
    // Encoder 2.
    for i:0..<pi.w {g[CZ_E2W+i]=px_conv_dw(inner,pi,cz_from(ws,n*CZ_H1),cz_from(ws,n*CZ_DZ),i);}
    for f:0..<inner.filters {g[CZ_E2B+f]=px_conv_db(inner,pi,cz_from(ws,n*CZ_DZ),f);}
    for i:0..<pi.x {
        v:=px_conv_dx(inner,pi,cz_from(w,CZ_E2W),cz_from(ws,n*CZ_DZ),i)*cz_slope(ws[n*CZ_A1+i]);
        if !cz_put(v,ws,n*CZ_DA1+i) {return CzLoss.{status=CzStatus.number};}
    }
    // Encoder 1.
    for i:0..<po.w {g[CZ_E1W+i]=px_conv_dw(outer,po,x,cz_from(ws,n*CZ_DA1),i);}
    for f:0..<outer.filters {g[CZ_E1B+f]=px_conv_db(outer,po,cz_from(ws,n*CZ_DA1),f);}
    return CzLoss.{loss=total/f64(po.x)};
}
```

With the quantizer off, the gradient is exact, and the check `gradients meet
central differences` compares it with `(L(θ + h) − L(θ − h)) / 2h` at
h = 2⁻¹⁰ for the first, a middle and the last parameter of every layer. f32
activations round at about 6 parts in 10⁸, so the check allows 10⁻⁵ plus 1
percent. Measured once outside the checks over all 1,815 parameters on two
images, the largest difference is 3.8 · 10⁻⁶. With the quantizer on, the
latent does not move when a decoder parameter does, so the same comparison
holds for the decoder's parameters there too, and the check makes it.

Every layer here sums in binary64 and rounds each stored value once to f32.
Inputs are checked to be finite before any sum, and a sum that does not fit
in f32 is refused, so a NaN, an infinity or an overflow anywhere ends the
call with `number`.

## Procedural images

The images are generated in Coax, so the data is exact and anyone can
regenerate it. Image `k` starts from seed `1 + 7919k` and takes 14 draws
from the Park–Miller generator, `s ← 48271·s mod (2³¹ − 1)`. Each channel is a
base level in `[1/4, 3/4]`, plus a slope across and a slope down of up to
`4/64` a pixel, plus a 4 × 4 tile at an even offset that adds up to 1/4,
clamped to `[0, 1]`. Every value is a multiple of 1/128.

<!-- source: psionic/lib/image-codec.coax data -->
```jai
// One step of the Park-Miller generator: s * 48271 mod (2^31 - 1).
cz_draw :: (s:int)->int {return (s*48271)%2147483647;}
// v clamped to [0, 1] without a branch: (v + |v|) / 2 is max(v, 0).
cz_clamp01 :: (v:f64)->f64 {low:=(v+abs(v))/2;return 1-((1-low)+abs(1-low))/2;}
// Image k: each channel a base level in [1/4, 3/4] plus a slope across and
// a slope down, then a 4 by 4 tile at an even offset that adds up to 1/4
// to each channel, clamped to [0, 1]. Every value is a multiple of 1/128.
cz_image :: (k:int,out:[]f32) {
    s:=1+7919*k;d:[14]int;for j:0..<14 {s=cz_draw(s);d[j]=s;}
    top:=2*(d[9]%3);left:=2*(d[10]%3);
    for c:0..<3 {
        base:=f64(8+d[3*c]%17)/32;across:=f64(d[3*c+1]%9-4)/64;down:=f64(d[3*c+2]%9-4)/64;tile:=f64(d[11+c]%9-4)/16;
        for y:0..<CZ_SIDE {for x:0..<CZ_SIDE {
            v:=base+across*(f64(x)-3.5)+down*(f64(y)-3.5);
            if y>=top && y<top+4 && x>=left && x<left+4 {v+=tile;}
            out[(c*CZ_SIDE+y)*CZ_SIDE+x]=f32(cz_clamp01(v));
        }}
    }
}
```

Image 0 by hand: from seed 1 the draws begin 48271, 182605794 and
1291394886. The red base is `8 + 48271 mod 17 = 16`, so 16/32 = 0.5. The
slopes are `182605794 mod 9 − 4 = 2` and `1291394886 mod 9 − 4 = 2`, so
2/64 = 1/32 each. Red pixel (0, 0) is `0.5 + (1/32)(−3.5) + (1/32)(−3.5) =
0.28125`, and each step right adds 1/32: the transcript's fifth line is
`0.28125 0.3125 … 0.5`. The tile sits at rows 4 to 7, so row 0 has none.

Training uses images 0 to 191. Validation uses images 4096 to 4111, and the
choice of run below was made on them. Test uses images 8192 to 8255, which
decide the gates. The check `the procedural images` confirms that every value
is a multiple of 1/128 in `[0, 1]`, that both ends are reached, that no two
of the first 64 images are equal, and that all 64 match an independent
restatement of the rule above, in
[`image-codec-checks.coax`](../tests/image-codec-checks.coax) with the other
check helpers.

## Training

The initial weights are uniform in `[−a, a]` with `a = √(6 / fan-in)`, on a
grid of 1/4096 so that every one is exact in f32. The output bias starts at
1/2, the middle of the pixel range. Each step takes the next 16 training
images in order, runs the forward pass with the quantizer on, and makes one
AdamW step (β₁ 0.9, β₂ 0.999, ε 10⁻⁸, no weight decay). The rate halves after
each quarter of the steps.

<!-- source: psionic/lib/image-codec.coax train -->
```jai
// Weights uniform in [-a, a] with a = sqrt(6 / fan-in), on a grid of 1/4096
// so that every one is exact in f32; biases zero but the output's, 1/2.
cz_init :: (params:[]f64,seed:int) {
    s:=seed;
    for i:0..<CZ_PARAMS {
        a:=0.0;
        if i<CZ_E1B {a=sqrt(6.0/48);}
        else if i>=CZ_E2W && i<CZ_E2B {a=sqrt(6.0/128);}
        else if i>=CZ_D1W && i<CZ_D1B {a=sqrt(6.0/16);}    // 4 channels, 4 taps reach a pixel
        else if i>=CZ_D2W {a=sqrt(6.0/32);}                // 8 channels, 4 taps
        s=cz_draw(s);u:=f64(s%8193)/4096-1;
        params[i]=floor(a*u*4096+0.5)/4096;
        if i>=CZ_D2B {params[i]=0.5;}
    }
}
cz_rate :: (cfg:CzTrain,step:int)->f64 {return cfg.lr*pow(0.5,f64((4*step)/cfg.steps));}
cz_train_admit :: (cfg:CzTrain,count:int)->CzStatus {
    if cfg.steps<1 || cfg.batch<1 || cfg.batch>CZ_BATCH_MAX || count<cfg.batch || count%cfg.batch!=0 {return CzStatus.batch;}
    return CzStatus.ok;
}
// losses[s] is step s's minibatch loss before its update.
cz_train :: (params:[]f64,m:[]f64,v:[]f64,x:[]f32,count:int,cfg:CzTrain,losses:[]f64)->CzStatus {
    s:=cz_train_admit(cfg,count);if s!=CzStatus.ok {return s;}
    if params.count!=CZ_PARAMS || m.count!=CZ_PARAMS || v.count!=CZ_PARAMS || x.count!=count*CZ_PIXELS || losses.count<cfg.steps {return CzStatus.shape;}
    ws:[CZ_BATCH_MAX*CZ_WORK]f32;w:[CZ_PARAMS]f32;g:[CZ_PARAMS]f64;none:[1]f32;state:AwState;b:=cfg.batch;
    for t:0..<cfg.steps {
        for i:0..<CZ_PARAMS {w[i]=f32(params[i]);}
        start:=(t*b)%count;
        r:=cz_gradient(w[..],x[start*CZ_PIXELS..<(start+b)*CZ_PIXELS],b,cfg.quantize,ws[..],g[..]);
        if r.status!=CzStatus.ok {return r.status;}
        losses[t]=r.loss;
        group:=[1]AwGroup.[AwGroup.{first=0,count=CZ_PARAMS,lr=cz_rate(cfg,t),beta1=0.9,beta2=0.999,eps=0.00000001}];
        a:=aw_step(group[..],AwPolicy.{},state,params,g[..],m,v,none[0..<0],false);
        if a.status!=AwStatus.ok {return CzStatus.number;}
        state=a.state;
    }
    return CzStatus.ok;
}
// Mean squared error over count images, one at a time; -1 when refused.
cz_mse :: (params:[]f64,x:[]f32,count:int,quantize:bool)->f64 {
    if count<1 || params.count!=CZ_PARAMS || x.count!=count*CZ_PIXELS {return -1;}
    w:[CZ_PARAMS]f32;for i:0..<CZ_PARAMS {w[i]=f32(params[i]);}
    if !px_finite(x) || !px_finite(w[..]) {return -1;}
    ws:[CZ_FORWARD]f32;total:=0.0;
    for k:0..<count {
        image:=x[k*CZ_PIXELS..<(k+1)*CZ_PIXELS];
        if cz_forward(w[..],image,1,ws[..],quantize)!=CzStatus.ok {return -1;}
        for i:0..<CZ_PIXELS {e:=f64(ws[CZ_Y+i])-f64(image[i]);total+=e*e;}
    }
    return total/f64(count*CZ_PIXELS);
}
// Peak signal-to-noise ratio for values in [0, 1]: 10 log10(1 / mse).
cz_psnr :: (mse:f64)->f64 {return -10*log(mse)/log(10.0);}
```

## Acceptance

These gates were written into the library before the first training run:

<!-- source: psionic/lib/image-codec.coax gates -->
```jai
// Frozen before the first training run (chapter 145, "Acceptance").
CZ_GATE_PSNR :: 22.0;       // test PSNR with quantized latents, dB, at least
CZ_GATE_GAIN :: 4.0;        // test MSE at most a quarter of the mean image's
```

The baseline is the mean training image, predicted for every image. Its
test PSNR is 13.3 dB, so the second gate asks for an error at most a quarter
of that, about 19.3 dB. The first gate asks for 22 dB: a root-mean-square
error of about 0.08 on a scale of 1, with the quantizer on.

**Compression.** An image is 192 bytes at 8 bits a value. Its code is 16
values of 4 bits, 8 bytes: 24 to 1. Fifteen levels need log₂ 15 ≈ 3.91 bits,
so packing two codes a byte wastes about 2 percent.

### How the run was chosen

The first configuration missed both gates on validation. Every configuration
tried, all from the same initial weights (seed 7), batch 16:

| Training images | Steps | Rate | Validation PSNR, quantized |
| --- | --- | --- | --- |
| 48 | 300 | 0.01 | 18.58 dB |
| 192 | 600 | 0.01 | 21.94 dB |
| 192 | 600 | 0.02 | 22.47 dB |
| 192 | 800 | 0.02 | 22.60 dB |
| 192 | 1,500 | 0.01 | 23.01 dB |

Forty-eight images were too few: the training error was 20.27 dB while
validation stayed near 18.6. The 800-step run was the longest that fit in a
debug example of under 30 seconds, and it was evaluated on the test images
once: 21.97 dB, 0.03 dB short of the first gate. The 1,500-step run had the
best validation score, so it became the recorded run, and the test numbers
below are a second look at the test images. A reader should weigh the 22.21
dB below with that in mind: the margin over the gate is small, and the
evaluation is not a first look.

### The recorded run

[`psionic/tests/image-codec-train.coax`](../tests/image-codec-train.coax)
trains for 1,500 steps and prints a Coax source file: the losses as comments,
then the trained weights as f32 bit patterns. Its output is
[`psionic/fixtures/image-codec-weights.coax`](../fixtures/image-codec-weights.coax),
which the example loads. To regenerate and compare it, which takes about 10
seconds in a release build:

```shell
coax/bin/coax run psionic/tests/image-codec-train.coax --release | diff - psionic/fixtures/image-codec-weights.coax
```

The arithmetic is deterministic (fixed summation order, one rounding per
stored value, no threads), so the file comes back byte for byte.

## Run the example

<!-- source: psionic/examples/image-codec.coax example -->
```jai
main :: () {
    // 1. Upsampling by hand: a 1 by 4 conv, kernel 4, stride 2, padding 1, transposed.
    c:=PxConv.{channels=1,height=1,width=4,filters=1,kernel_h=1,kernel_w=4,stride_w=2,pad_w=1};
    u:=[2]f32.[1,10];k4:=[4]f32.[1,2,3,4];none:[1]f32;up:[4]f32;s4:[4]f32;
    r:=cz_up(c,u[..],k4[..],none[0..<0],up[..],s4[..]);print("upsample [1, 10] with taps [1, 2, 3, 4]: %",r.status);show(",",up[..]);

    // 2. The hand-set codec: latent (i, j) is red pixel (4i, 4j), through one leak.
    w:[CZ_PARAMS]f32;cz_probe(w[..]);x:[CZ_PIXELS]f32;x[0]=0.5;x[4]=-0.25;x[32]=2;x[36]=-3;
    ws:[64*CZ_WORK]f32;codes:[64*CZ_LATENT]int;y:[64*CZ_PIXELS]f32;
    e:=cz_encode(w[..],x[..],1,codes[..],ws[..]);codes_line("hand-set codec, codes of channel 0:",codes[0..<4]);
    e=cz_decode(w[..],codes[0..<16],1,y[..],ws[..]);
    print("decoded red pixels (0,0) (0,4) (4,0) (4,4): % % % %\n",y[0],y[4],y[32],y[36]);

    // 3. The quantizer: every level comes back as its own code.
    back:=0;for k:-7..<8 {if cz_code(f64(cz_level(k)))==k {back+=1;}}
    print("levels k/7 that quantize back to k: % of 15\n",back);

    // 4. Data: training images 0..191, validation images 4096..4111 (used to
    // choose the run), test images 8192..8255 (used only for the gates).
    train:[TRAIN*CZ_PIXELS]f32;held:[HELD*CZ_PIXELS]f32;test:[TEST*CZ_PIXELS]f32;
    for k:0..<TRAIN {cz_image(k,train[k*CZ_PIXELS..<(k+1)*CZ_PIXELS]);}
    for k:0..<HELD {cz_image(4096+k,held[k*CZ_PIXELS..<(k+1)*CZ_PIXELS]);}
    for k:0..<TEST {cz_image(8192+k,test[k*CZ_PIXELS..<(k+1)*CZ_PIXELS]);}
    show("image 0, red row 0:",train[0..<8]);
    // The baseline: predict the mean training image for every image.
    mean:[CZ_PIXELS]f64;for k:0..<TRAIN {for i:0..<CZ_PIXELS {mean[i]+=f64(train[k*CZ_PIXELS+i])/TRAIN;}}
    base:=0.0;for k:0..<TEST {for i:0..<CZ_PIXELS {d:=f64(test[k*CZ_PIXELS+i])-mean[i];base+=d*d;}}base/=TEST*CZ_PIXELS;
    print("mean-image baseline, test: MSE %, PSNR % dB\n",cz_r(base,1000000),cz_r(cz_psnr(base),100));

    // 5. Train briefly from the start: 200 AdamW steps, 16 images a step,
    // through the quantizer. The recorded run continues to 1,500 steps.
    params:[CZ_PARAMS]f64;m:[CZ_PARAMS]f64;v:[CZ_PARAMS]f64;losses:[STEPS]f64;cz_init(params[..],7);
    cfg:=CzTrain.{steps=STEPS,batch=16,lr=0.01};
    status:=cz_train(params[..],m[..],v[..],train[..],TRAIN,cfg,losses[..]);print("200 training steps: %\n",status);
    for t:0..<STEPS {if t%50==0 || t==STEPS-1 {print("  step %: loss %\n",t,cz_r(losses[t],1000000));}}
    print("  validation after 200 steps, quantized: PSNR % dB\n",cz_r(cz_psnr(cz_mse(params[..],held[..],HELD,true)),100));

    // 6. The recorded 1,500-step weights: reconstruction error, then the
    // frozen gates on the test images.
    for i:0..<CZ_PARAMS {params[i]=f64(f32_from_bits(CZ_TRAINED[i]));}
    rows:=[4]string.["training, quantized","validation, quantized","test, continuous latent","test, quantized"];
    mse:=[4]f64.[cz_mse(params[..],train[..],TRAIN,true),cz_mse(params[..],held[..],HELD,true),
        cz_mse(params[..],test[..],TEST,false),cz_mse(params[..],test[..],TEST,true)];
    print("recorded weights, 1,500 steps:\n");
    for i:0..<4 {print("  %: MSE %, PSNR % dB\n",rows[i],cz_r(mse[i],1000000),cz_r(cz_psnr(mse[i]),100));}
    print("compression: % bytes to % bytes, % to 1\n",CZ_PIXELS,CZ_LATENT/2,CZ_PIXELS/(CZ_LATENT/2));
    print("gate PSNR >= % dB: %; gate MSE <= baseline / %: %\n",CZ_GATE_PSNR,cz_psnr(mse[3])>=CZ_GATE_PSNR,CZ_GATE_GAIN,mse[3]*CZ_GATE_GAIN<=base);

    // 7. One test image to 8 bytes and back.
    for i:0..<CZ_PARAMS {w[i]=f32(params[i]);}
    e=cz_encode(w[..],test[0..<CZ_PIXELS],1,codes[0..<16],ws[..]);codes_line("test image 0, codes:",codes[0..<16]);
    bytes:[8]u8;p:=cz_pack(codes[0..<16],bytes[..]);print("packed:");for b:bytes {print(" %",b);}print("\n");
    again:[16]int;p=cz_unpack(bytes[..],again[..]);same:=true;for i:0..<16 {if again[i]!=codes[i] {same=false;}}
    print("unpacked codes equal the codes: %\n",same);
    e=cz_decode(w[..],again[..],1,y[..],ws[..]);
    show("  original red row 0:",test[0..<8]);
    print("  decoded red row 0:");for i:0..<8 {print(" %",cz_r(f64(y[i]),1000));}print("\n");
    // Decoding the codes gives exactly what the quantized forward pass gives.
    q:=cz_forward(w[..],test[0..<CZ_PIXELS],1,ws[0..<CZ_WORK],true);exact:=true;
    for i:0..<CZ_PIXELS {if ws[CZ_Y+i]!=y[i] {exact=false;}}
    print("decode(codes) equals the quantized forward pass: %\n",exact && q==CzStatus.ok);
}
```

The example trains for 200 steps to show the loss falling, from 0.0937 to
0.0173, then switches to the recorded weights. On the test images the
recorded codec reaches 22.21 dB with the quantizer on and 22.76 dB with
continuous latents, against 13.3 dB for the mean image: both gates pass.
Quantization costs about half a decibel. The test image's red row shows
what the error looks like: the original falls from 0.29 to a dark tile
(columns 2 to 5, down to 0.008) and rises again; the decoded row falls and
rises with it but smooths the tile's sharp edges into a gradual slope and
misses its depth. Sixteen 4-bit numbers cannot hold 192 pixels exactly, and
sharp edges are what a codec this small gives up first. The minibatch loss
at step 199 is a little above step 150's because each step sees different
images; the validation PSNR after 200 steps, 17.91 dB, is the steadier
measure.

## What the codec refuses

Every entry point checks its counts, its codes and its numbers before it
writes anything. The [refusal example](../examples/image-codec-refuse.coax)
tries each malformed input in turn:

<!-- source: psionic/examples/image-codec-refuse.coax refuse -->
```jai
main :: () {
    w:[CZ_PARAMS]f32;cz_probe(w[..]);x:[2*CZ_PIXELS]f32;ws:[2*CZ_WORK]f32;
    codes:[32]int;for i:0..<32 {codes[i]=-9;}
    print("encode 0 images: %\n",cz_encode(w[..],x[0..<0],0,codes[..],ws[..]).status);
    print("encode 65 images: %\n",cz_encode(w[..],x[..],65,codes[..],ws[..]).status);
    print("191 values for one 3x8x8 image: %\n",cz_encode(w[..],x[0..<191],1,codes[..],ws[..]).status);
    print("1,814 weights for 1,815: %\n",cz_encode(w[0..<1814],x[0..<192],1,codes[..],ws[..]).status);
    print("room for 31 codes of 32: %\n",cz_encode(w[..],x[..],2,codes[0..<31],ws[..]).status);
    x[100]=0.0/0.0;print("a NaN pixel: %\n",cz_encode(w[..],x[..],2,codes[..],ws[..]).status);x[100]=0;
    untouched:=0;for k:codes {if k==-9 {untouched+=1;}}print("codes still -9: % of 32\n",untouched);

    for i:0..<32 {codes[i]=0;}y:[2*CZ_PIXELS]f32;for i:0..<2*CZ_PIXELS {y[i]=-7;}
    print("decode 15 codes as one image: %\n",cz_decode(w[..],codes[0..<15],1,y[..],ws[..]).status);
    print("decode 32 codes as 1 image: %\n",cz_decode(w[..],codes[..],1,y[..],ws[..]).status);
    codes[17]=8;print("code 8, past the last level 7: %\n",cz_decode(w[..],codes[..],2,y[..],ws[..]).status);
    codes[17]=-8;print("code -8: %\n",cz_decode(w[..],codes[..],2,y[..],ws[..]).status);codes[17]=0;
    w[CZ_D2W]=1.0/0.0;print("an infinite decoder weight: %\n",cz_decode(w[..],codes[..],2,y[..],ws[..]).status);w[CZ_D2W]=0;
    untouched=0;for v:y {if v==-7 {untouched+=1;}}print("pixels still -7: % of 384\n",untouched);

    bytes:=[8]u8.[0x77,0x77,0x77,0xf7,0x77,0x77,0x77,0x77];back:[16]int;
    print("a byte holding the nibble 15: %\n",cz_unpack(bytes[..],back[..]).status);
    print("7 bytes for one image's 8: %\n",cz_unpack(bytes[0..<7],back[..]).status);
    print("pack 15 codes: %\n",cz_pack(codes[0..<15],bytes[..]).status);

    c:=PxConv.{channels=1,height=1,width=4,filters=1,kernel_h=1,kernel_w=4,stride_w=2,pad_w=1};
    u:=[2]f32.[1,10];k4:=[4]f32.[1,2,3,4];none:[1]f32;up:[4]f32;s:[4]f32;
    bad:=c;bad.pad_w=4;print("upsample with padding 4 on a 4-tap window: %\n",cz_up(bad,u[..],k4[..],none[0..<0],up[..],s[..]).status);
    print("upsample 1 value where the conv has 2 outputs: %\n",cz_up(c,u[0..<1],k4[..],none[0..<0],up[..],s[..]).status);
    print("upsample into 3 of 4 outputs: %\n",cz_up(c,u[..],k4[..],none[0..<0],up[0..<3],s[..]).status);
    u[0]=f32_from_bits(0x7f7fffff);print("an upsampled sum past the largest f32: %\n",cz_up(c,u[..],k4[..],none[0..<0],up[..],s[..]).status);

    params:[CZ_PARAMS]f64;m:[CZ_PARAMS]f64;v:[CZ_PARAMS]f64;losses:[4]f64;
    print("train 2 images in batches of 3: %\n",cz_train(params[..],m[..],v[..],x[..],2,CzTrain.{steps=4,batch=3,lr=0.01},losses[..]));
    print("train 0 steps: %\n",cz_train(params[..],m[..],v[..],x[..],2,CzTrain.{steps=0,batch=2,lr=0.01},losses[..]));
    print("train with room for 4 losses of 5 steps: %\n",cz_train(params[..],m[..],v[..],x[..],2,CzTrain.{steps=5,batch=2,lr=0.01},losses[..]));
}
```

```text
encode 0 images: batch
encode 65 images: batch
191 values for one 3x8x8 image: shape
1,814 weights for 1,815: shape
room for 31 codes of 32: capacity
a NaN pixel: number
codes still -9: 32 of 32
decode 15 codes as one image: shape
decode 32 codes as 1 image: shape
code 8, past the last level 7: code
code -8: code
an infinite decoder weight: number
pixels still -7: 384 of 384
a byte holding the nibble 15: code
7 bytes for one image's 8: shape
pack 15 codes: shape
upsample with padding 4 on a 4-tap window: shape
upsample 1 value where the conv has 2 outputs: shape
upsample into 3 of 4 outputs: capacity
an upsampled sum past the largest f32: number
train 2 images in batches of 3: batch
train 0 steps: batch
train with room for 4 losses of 5 steps: shape
```

## Checks

`coax/bin/coax test psionic/lib/image-codec.coax` runs these, in debug and
release:

- **Upsampling by hand.** The 1 × 4 transpose above, with and without bias,
  and a kernel-2, stride-2 transpose that turns each value into a two-pixel
  block.
- **Upsampling against its definition.** The scatter definition and the
  adjoint identity over 16 settings of stride, padding, dilation and groups,
  bit for bit.
- **Quantization and packing.** Every level's round trip, the boundaries at
  ±0.0714 and ±0.9286, clamping, tanh by hand, and packed bytes checked by
  hand (codes 7 and −7 pack to `0x0e`).
- **The hand-set codec.** The table above, every other output pixel zero,
  the quantized forward pass equal to decoding, and each layer's bias moving
  exactly the values worked out for it.
- **Gradients.** Central differences on the first, a middle and the last
  parameter of every layer, again through the quantizer for the decoder, and
  the refusals of the gradient itself, including an overflow in decoder 1's
  output gradient.
- **Data.** The image rule restated independently, the first two generator
  draws by hand, the value grid and range, and 64 distinct images.
- **Training.** Initial weights on their grid and in their ranges (the first
  one by hand: −747/4096), the rate schedule, PSNR at 20 and 30 dB, each
  admission bound at and past its edge, two steps of `cz_train` equal to two
  gradients and AdamW steps made by hand, bit for bit, and three steps that
  lower the error by more than 10 percent. The quantized error image by image
  equals the error through `cz_encode` and `cz_decode`, bit for bit.
- **Refusals.** Batch sizes 0 and 65 (64 is admitted), wrong counts of
  pixels, weights, codes and bytes, codes ±8 (±7 are admitted), the nibble 15
  in either half, NaN and infinite values, and sums past the largest f32 in
  each layer; each leaves its output untouched.

`coax mutate --root-only` on the unit leaves zero survivors.

## What this chapter does not do

- It does not execute Cosmos-0.1-Tokenizer-CI8x8, the plan's teaching
  target. That needs the pinned checkpoint and its operations: a wavelet
  transform, group normalization and attention blocks (chapter 10), and the
  encoder's intermediates, latent and reconstruction compared with recorded
  references within a frozen tolerance. Nothing here claims compatibility
  with any Cosmos codec.
- The images are 8 × 8 procedural pictures, and the reported quality is for
  them only. Natural images, larger sizes and a perceptual measure are not
  attempted.
- The quantizer is a fixed 15-level grid per value. A learned codebook in the
  style of chapter 148's nearest-code encoder is not built here.
- Upsampling is a CPU sum through chapter 142's input gradient. It is not an
  operator in chapter 5's registry, and it is not lowered into chapter 38's
  tensor IR.

## Dependencies

- [Chapter 142](ch142-pixels-convolutions-and-patches.md): convolution and
  its gradients.
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW.
- [Chapter 4](ch004-numbers-and-matrices.md): the compensated sum every
  convolution uses.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Cosmos notes.
