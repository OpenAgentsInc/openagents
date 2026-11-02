# 166. One complete Cosmos3-Edge path

> **Status: partly implemented.** The pinned checkpoint's manifest (four
> weight files, 1,188 tensors, read from their headers) is checked against
> its configuration; three real-weight slices (the latent input and output
> projections and the VAE encoder's first convolution) match float64
> references; and the whole conditioned generation path (observation,
> codec, two-tower denoiser, flow steps, decode) runs end to end on small
> synthetic weights, passes chapter 165's codec and scheduler contracts and
> matches an independent oracle at every stage. Not done yet: the 28-layer
> transformer, the full Wan VAE and the vision tower on original weights,
> the pipeline's own order-2 UniPC sampler and native flow schedule,
> reference tensors from NVIDIA's implementation, and a task evaluation on
> Aethyr worlds.

Cosmos3-Edge is NVIDIA's small world model: given frames, a text prompt or
robot actions, it generates the frames that come next. This chapter follows
one conditioned generation through it, from an observation record to a
decoded frame, and says exactly which parts run here on the original
weights and which run on small synthetic weights in the same shape.

It builds, in [`cosmos.coax`](../lib/cosmos.coax):

- the **manifest**: every weight file's size and SHA-256, every tensor
  group's shapes, and a check that 19 key shapes follow from the
  configuration files;
- **real-weight slices**: rows of the transformer's latent input and output
  projections and the VAE encoder's first convolution, computed from the
  checkpoint's bf16 words and compared with float64 references;
- a **codec** that patchifies frames the way Cosmos3-Edge's VAE does and
  passes chapter 165's codec contract;
- a **flow schedule** built from the scheduler configuration's Karras
  sigmas, which passes chapter 165's scheduler contract;
- a **two-tower denoiser**: one layer that runs the conditioning frame
  through understanding weights and the noisy latent through generation
  weights, with joint attention, as the checkpoint's tensor names lay out;
- **Euler steps** on the flow ODE and the **whole path**, checked stage by
  stage against an independent oracle;
- a **task evaluation**, reported apart from numerical parity.

```shell
coax/bin/coax test psionic/lib/cosmos.coax
coax/bin/coax run psionic/examples/cosmos-path.coax
coax/bin/coax run psionic/examples/cosmos-path-refuse.coax
python3 -I psionic/tests/cosmos_slices.py DIR   # tooling: regenerates the manifest and slices
```

The [example](../examples/cosmos-path.coax) and the
[refusals](../examples/cosmos-path-refuse.coax) each finish in under 15 seconds on one core.

Profile `psionic-cosmos-v1`: binary64 sums in a fixed order, f32 pixels,
one CPU core, one 3 × 8 × 8 frame a step, at most 8 sampling steps.

## The pinned checkpoint

The checkpoint is
[`nvidia/Cosmos3-Edge` at revision `344d602b`](https://huggingface.co/nvidia/Cosmos3-Edge/tree/344d602b128d1bbdacb43b08d0a3626f46343e29).
Its weights are in four safetensors files, all bf16:

| File | Bytes | Tensors | What it holds |
| --- | --- | --- | --- |
| `transformer/…-00001-of-00002` | 5,000,040,072 | 426 | 22 layers of both towers, the token embedding |
| `transformer/…-00002-of-00002` | 1,739,335,512 | 123 | 6 more layers, `proj_in`, `proj_out`, the time embedder, action projections, `lm_head`, final norms |
| `vae/diffusion_pytorch_model` | 1,409,400,600 | 196 | a Wan 2.2 VAE (`AutoencoderKLWan`) |
| `vision_encoder/model` | 978,739,880 | 443 | a 27-layer vision encoder and its projector |

[`psionic/fixtures/cosmos3-edge-manifest.txt`](../fixtures/cosmos3-edge-manifest.txt)
records each file's SHA-256 and every tensor group. The tool
[`cosmos_slices.py`](../tests/cosmos_slices.py) wrote it from the headers
alone: the first eight bytes of a safetensors file give the header's
length, and the header is JSON. The two transformer shard and vision files
were read with HTTP range requests for their headers only; the second shard
and the VAE (both under 5 GB) were downloaded and their SHA-256 checked
against the published values. Nothing was unpickled and none of the
repository's code was run. The four files hold 4,563,688,396 parameters.

The terms are the OpenMDW-1.1 release, and the model card includes
NVIDIA-owned data, so a complete reproduction of the original training
corpus is not established, and this chapter does not attempt one.

### Two towers in one transformer

The tensor names show the architecture. Each of the 28 layers holds 19
tensors: two towers of 8 (2 norms, 2 MLP matrices, 4 attention matrices)
and 3 extra norms.

- The **understanding tower** (`to_q`, `to_k`, `to_v`, `to_out`, `mlp`,
  `input_layernorm`) reads text and images autoregressively, as a language
  model does; `lm_head` turns its last state into token logits.
- The **generation tower** (`add_q_proj`, `add_k_proj`, `add_v_proj`,
  `to_add_out`, `mlp_moe_gen`, the `_moe_gen` norms) reads noisy video
  latents and predicts a flow velocity. It normalizes its own queries and
  keys (`norm_added_q`, `norm_added_k`) and sees the understanding tower's
  keys through one more norm (`k_norm_und_for_gen`).

The towers share one attention: generation tokens attend to understanding
tokens and to each other. `proj_in` maps a latent patch (48 channels × 2 ×
2 = 192 numbers) to the hidden width 2,048, and `proj_out` maps back.

<!-- source: psionic/lib/cosmos.coax config -->
```jai
// From the pinned config.json, transformer/config.json and vae/config.json.
CS_HIDDEN :: 2048;          // hidden_size
CS_HEADS :: 16;             // num_attention_heads
CS_KV_HEADS :: 8;           // num_key_value_heads
CS_HEAD_DIM :: 128;         // head_dim, stated, not hidden / heads
CS_FFN :: 9216;             // intermediate_size, relu2
CS_LAYERS :: 28;            // num_hidden_layers
CS_VOCAB :: 131072;         // vocab_size
CS_LATENT_CH :: 48;         // latent_channel = the VAE's z_dim
CS_LATENT_PATCH :: 2;       // latent_patch_size
CS_ACTION_DIM :: 64;        // action_dim
CS_DOMAINS :: 32;           // num_embodiment_domains
CS_VAE_IN :: 12;            // 3 colours x patch 2 x 2
CS_VAE_BASE :: 160;         // base_dim
CS_VAE_DEC_BASE :: 256;     // decoder_base_dim
CS_VAE_TOP :: 4;            // dim_mult's last entry
CS_VIS_HIDDEN :: 1152;      // vision_config.hidden_size
CS_VIS_PATCH :: 16;         // vision_config.patch_size
CS_VIS_MERGE :: 2;          // spatial_merge_size
CS_PROJ_HIDDEN :: 11520;    // projector merger_intermediate_size
CS_TIME_FREQ :: 256;        // the time embedder's input width: in the header only
CS_SIGMA_MIN :: 0.147;      // scheduler sigma_min
CS_SIGMA_MAX :: 200.0;      // scheduler sigma_max
CS_RHO :: 7.0;              // Karras spacing exponent
CS_TRAIN_LEVELS :: 1000;    // num_train_timesteps
```

The configuration states `head_dim` 128 with 16 query heads and 8 key and
value heads: query projections are 2,048 rows and key projections 1,024.
The check computes 19 shapes from these numbers and compares them with the
headers:

<!-- source: psionic/lib/cosmos.coax manifest -->
```jai
// The 19 shapes chapter 166 checks, each as 5 numbers padded with 1, in
// the order of CS_HEADER_SHAPES.
cs_config_shapes :: (out:[]int) {
    q:=CS_HEADS*CS_HEAD_DIM;kv:=CS_KV_HEADS*CS_HEAD_DIM;patch:=CS_LATENT_CH*CS_LATENT_PATCH*CS_LATENT_PATCH;
    s:=[19][5]int.[
        [5]int.[q,CS_HIDDEN,1,1,1], [5]int.[kv,CS_HIDDEN,1,1,1],          // to_q, to_k: understanding
        [5]int.[q,CS_HIDDEN,1,1,1], [5]int.[kv,CS_HIDDEN,1,1,1],          // add_q_proj, add_k_proj: generation
        [5]int.[CS_HEAD_DIM,1,1,1,1],                                     // norm_added_q: per head
        [5]int.[CS_FFN,CS_HIDDEN,1,1,1], [5]int.[CS_HIDDEN,CS_FFN,1,1,1], // mlp.up, mlp_moe_gen.down
        [5]int.[CS_VOCAB,CS_HIDDEN,1,1,1],                                // embed_tokens
        [5]int.[CS_HIDDEN,patch,1,1,1], [5]int.[patch,CS_HIDDEN,1,1,1],   // proj_in, proj_out
        [5]int.[CS_HIDDEN,CS_TIME_FREQ,1,1,1],                            // time_embedder.linear_1
        [5]int.[CS_DOMAINS,CS_ACTION_DIM,1,1,1],                          // action_proj_out.bias
        [5]int.[CS_VAE_BASE,CS_VAE_IN,3,3,3],                             // encoder.conv_in
        [5]int.[2*CS_LATENT_CH,CS_VAE_BASE*CS_VAE_TOP,3,3,3],             // encoder.conv_out: mean and log-variance
        [5]int.[CS_VAE_DEC_BASE*CS_VAE_TOP,CS_LATENT_CH,3,3,3],           // decoder.conv_in
        [5]int.[CS_VAE_IN,CS_VAE_DEC_BASE,3,3,3],                         // decoder.conv_out
        [5]int.[CS_LATENT_CH,CS_LATENT_CH,1,1,1],                         // post_quant_conv
        [5]int.[CS_VIS_HIDDEN,3*CS_VIS_PATCH*CS_VIS_PATCH,1,1,1],         // vision patch_embedding
        [5]int.[CS_PROJ_HIDDEN,CS_VIS_HIDDEN*CS_VIS_MERGE*CS_VIS_MERGE,1,1,1]]; // projector linear_fc1
    for i:0..<19 {for j:0..<5 {out[i*5+j]=s[i][j];}}
}
// Tensors a layer holds: two towers, each with 2 norms, 2 MLP matrices and
// 4 attention matrices, plus 3 key and query norms the generation tower uses.
cs_per_layer :: ()->int {return 2*(2+2+4)+3;}
```

All 95 numbers match. The time embedder's input width, 256, is not in any
configuration file; it is taken from the header and said so.

## Real-weight slices

Three slices run on original weights. A bf16 word is the top 16 bits of an
f32, so `0x3f80` is 1 and `0x4049` is 3.140625. Each slice uses a fixed
input formula, so the Python tool and the Coax code see the same numbers:

- `proj_in`, rows 0 to 7, on the latent patch `u[i] = ((37 i) mod 17 − 8) / 8`;
- `proj_out`, rows 0 and 1, on the hidden state `h[j] = ((29 j) mod 23 − 11) / 11`;
- the VAE encoder's `conv_in`, filters 0 and 1, on one frame of a 3 × 4 × 4
  image.

The convolution needs two conventions from the VAE. First, Wan's patchify
turns a 3-colour image into 12 channels at half the resolution: pixel
(colour c, row 2r + a, column 2s + b) becomes channel 4c + 2b + a at (r,
s), column offset first. Second, its 3 × 3 × 3 convolutions are causal in
time: they pad two zero frames before the first, so on a single frame only
temporal tap 2 meets real data.

<!-- source: psionic/lib/cosmos.coax slices -->
```jai
// A bf16 word is the top half of an f32.
cs_bf16 :: (b:u16)->f64 {return f64(f32_from_bits(u32(b)<<16));}
// rows outputs of an affine map whose weights are bf16 words, summed from 0
// in input order, then the bias added.
cs_linear_bf16 :: (w:[]u16,b:[]u16,rows:int,cols:int,x:[]f64,out:[]f64) {
    for r:0..<rows {s:=0.0;for c:0..<cols {s+=cs_bf16(w[r*cols+c])*x[c];}out[r]=cs_bf16(b[r])+s;}
}
cs_input_in :: (i:int)->f64 {return f64((i*37)%17-8)/8;}
cs_input_out :: (j:int)->f64 {return f64((j*29)%23-11)/11;}
// The VAE encoder's conv_in on one frame of a 3 x 4 x 4 image: patchified
// to 12 x 2 x 2; the causal pad puts two zero frames first, so only temporal
// tap 2 meets the frame; zero spatial padding of 1. Filters 0 and 1, all 4
// positions.
cs_conv_in_slice :: (out:[]f64) {
    p:[12*4]f64;
    for c:0..<3 {for y:0..<4 {for x:0..<4 {p[(c*4+(x%2)*2+y%2)*4+(y/2)*2+x/2]=f64(((c*16+y*4+x)*7)%11-5)/5;}}}
    for o:0..<2 {for y:0..<2 {for x:0..<2 {
        s:=0.0;
        // Every 3 x 3 window holds the whole 2 x 2 grid: tap (py - y + 1, px - x + 1).
        for c:0..<12 {for py:0..<2 {for px:0..<2 {
            s+=cs_bf16(CS_CONV_IN_W[((o*12+c)*3+py-y+1)*3+px-x+1])*p[c*4+py*2+px];
        }}}
        out[(o*2+y)*2+x]=cs_bf16(CS_CONV_IN_B[o])+s;
    }}}
}
```

The tool computes each reference in Python floats (binary64) from the same
words, summing in the same order. The tolerance, 1e-12, was frozen before
the first comparison; all 18 values are within it (in fact equal). These
slices check that the weights are read correctly and that the conventions
above are applied as stated. They are not parity with NVIDIA's
implementation: that needs reference tensors from running it, which this
chapter does not do.

## The path on synthetic weights

The rest of the chapter runs the whole path with the same structure on
weights small enough to check by hand. The frames are chapter 165's 3 × 8 ×
8 images, and the shapes scale down accordingly:

<!-- source: psionic/lib/cosmos.coax shape -->
```jai
CS_SIDE :: 8;               // frames are chapter 165's 3 x 8 x 8
CS_GRID :: 4;               // the codec's latent: 1 channel of 4 x 4
CS_LATENT :: 16;
CS_CODEC_PARAMS :: 37;      // encoder 12 + 1 bias, decoder 12 + 12 biases
CS_MEAN :: 0.5;             // the toy latent's mean and standard deviation
CS_STD :: 0.5;
CS_TOKENS :: 4;             // 2 x 2 latent patches a frame
CS_WIDTH :: 4;              // a token: one latent patch, 1 x 2 x 2 values
CS_D :: 4;                  // the denoiser's hidden width
CS_F :: 8;                  // its MLP width
CS_EPS :: 0.00001;          // rms_norm_eps
CS_STEPS_MAX :: 8;
CS_WEIGHT_MAX :: 1000.0;    // admitted denoiser weights: |w| at most this, so no sum overflows
CS_TMAX :: 200.0/201.0;     // flow time at sigma_max: 200 / (1 + 200)
// Synthetic denoiser weights, one flat list. A tower: norm 4, q, k, v, o 16
// each, norm 4, up 8 x 4, down 4 x 8.
CS_EU :: 0;  CS_WI :: 16; CS_BI :: 32; CS_TA :: 36;
CS_TOWER :: 40; CS_TW :: 136;   // understanding tower at 40, generation at 176
CS_QN :: 312; CS_KN :: 316; CS_KU :: 320; CS_NF :: 324; CS_WO :: 328; CS_BO :: 344;
CS_PARAMS :: 348;
```

<!-- source: psionic/lib/cosmos.coax types -->
```jai
CsStatus :: enum u8 {ok;record;codec;weights;steps;}
// One generation: the walk of levels, the conditioning frame's latent, the
// starting noise, the understanding tower's output and the velocity at the
// first step, the final latent, its codes and the decoded frame.
CsRun :: struct {
    status:CsStatus;steps:int;walk:[CS_STEPS_MAX+1]int;
    cond:[CS_LATENT]f64;noise:[CS_LATENT]f64;context:[CS_LATENT]f64;first:[CS_LATENT]f64;
    latent:[CS_LATENT]f64;codes:[CS_LATENT]int;frame:[OB_PIXELS]f32;
}
```

### The codec

The codec patchifies the Wan way (8 × 8 becomes 12 channels of 4 × 4),
then maps the 12 channels at each position to one latent value, a 1 × 1
convolution where the real VAE has a deep network ending in 48 channels.
The latent is normalized as (z − mean) / std, the convention Wan pipelines
apply with the VAE's `latents_mean` and `latents_std`. Decoding inverts each
step.

<!-- source: psionic/lib/cosmos.coax codec -->
```jai
// Wan's patchify: pixel (colour c, row 2r + a, column 2s + b) is channel
// c x 4 + b x 2 + a at (r, s). Column offset first.
cs_patchify :: (x:[]f32,at:int,p:[]f64) {
    for c:0..<3 {for y:0..<CS_SIDE {for col:0..<CS_SIDE {
        p[((c*4+(col%2)*2+y%2)*CS_GRID+y/2)*CS_GRID+col/2]=f64(x[at+(c*CS_SIDE+y)*CS_SIDE+col]);
    }}}
}
cs_unpatchify :: (p:[]f64,x:[]f32,at:int) {
    for c:0..<3 {for y:0..<CS_SIDE {for col:0..<CS_SIDE {
        x[at+(c*CS_SIDE+y)*CS_SIDE+col]=f32(p[((c*4+(col%2)*2+y%2)*CS_GRID+y/2)*CS_GRID+col/2]);
    }}}
}
// Weights: encoder e[0..11] and bias e[12]; decoder d[13..24] and biases
// [25..36]. z = e . patch + e12, normalized as (z - mean) / std, the Wan
// pipelines' convention; decoding inverts the normalization first.
cs_codec_ok :: (w:[]f32)->bool {
    if w.count!=CS_CODEC_PARAMS {return false;}
    for v:w {if !ob_finite(v) {return false;}}
    return true;
}
// The frame at x[at..]: its normalized latent.
cs_latent :: (w:[]f32,x:[]f32,at:int,z:[]f64) {
    p:[CS_VAE_IN*CS_LATENT]f64;cs_patchify(x,at,p[..]);
    for i:0..<CS_LATENT {
        s:=f64(w[12]);for c:0..<CS_VAE_IN {s+=f64(w[c])*p[c*CS_LATENT+i];}
        z[i]=(s-CS_MEAN)/CS_STD;
    }
}
// The frame decoded from a normalized latent, into x[at..].
cs_frame :: (w:[]f32,z:[]f64,x:[]f32,at:int) {
    p:[CS_VAE_IN*CS_LATENT]f64;
    for i:0..<CS_LATENT {v:=z[i]*CS_STD+CS_MEAN;for c:0..<CS_VAE_IN {p[c*CS_LATENT+i]=f64(w[13+c])*v+f64(w[25+c]);}}
    cs_unpatchify(p[..],x,at);
}
// One admitted frame's latent; false when the weights or the frame are not.
cs_encode_latent :: (w:[]f32,x:[]f32,z:[]f64)->bool {
    if !cs_codec_ok(w) || x.count!=OB_PIXELS {return false;}
    cs_latent(w,x,0,z);
    return true;
}
cs_decode_latent :: (w:[]f32,z:[]f64,x:[]f32) {cs_frame(w,z,x,0);}
// Codes for frames first..first + n - 1: chapter 145's 15 levels of the
// normalized latent.
cs_encode :: (w:[]f32,x:[]f32,first:int,n:int,codes:[]int)->int {
    z:[CS_LATENT]f64;
    for f:0..<n {
        cs_latent(w,x,(first+f)*OB_PIXELS,z[..]);
        for i:0..<CS_LATENT {codes[f*CS_LATENT+i]=cz_code(z[i]);}
    }
    return n*CS_LATENT;
}
cs_decode :: (w:[]f32,codes:[]int,n:int,x:[]f32)->int {
    z:[CS_LATENT]f64;
    for f:0..<n {
        for i:0..<CS_LATENT {z[i]=f64(cz_level(codes[f*CS_LATENT+i]));}
        cs_frame(w,z[..],x,f*OB_PIXELS);
    }
    return n*OB_PIXELS;
}
```

**By hand.** Take frame 1 of chapter 165's hand record: a red square at
column 3, row 3. The probe weights average a block's four red pixels. At
latent position (1, 1) the block covers rows 2 and 3, columns 2 and 3:

| Channel | Offsets (row a, column b) | Pixel | Red |
| --- | --- | --- | --- |
| 0 | (0, 0) | (2, 2) | 0 |
| 1 | (1, 0) | (3, 2) | 0 |
| 2 | (0, 1) | (2, 3) | 0 |
| 3 | (1, 1) | (3, 3) | 1 |

z = (0 + 0 + 0 + 1) / 4 = 0.25. Normalized: (0.25 − 0.5) / 0.5 = −0.5. The
code is floor(7 × −0.5 + 0.5) = −3. The same holds at (1, 2), (2, 1) and
(2, 2), where the square covers one pixel of each block; every other block
is empty, z = 0, normalized −1, code −7. Decoding code −3 gives level −3/7,
z = −3/14 + 0.5 = 2/7 ≈ 0.2857, on all four red pixels of each of those
blocks.

Chapter 165's codec contract runs the codec over every frame of a record:
batch, one at a time, again, packed to bytes and back, and decoded.

<!-- source: psionic/lib/cosmos.coax run-codec -->
```jai
// Chapter 165's codec contract, filled from this codec.
cs_codec_run :: (w:[]f32,seq:ObSeq)->ObCodecRun {
    r:ObCodecRun;r.status=ob_admit(seq);if r.status!=ObStatus.ok {return r;}
    if !cs_codec_ok(w) {r.status=ObStatus.codec;return r;}
    s:=seq;n:=s.count;r.frames=n;
    r.written[0]=cs_encode(w,s.pixels[..],0,n,r.batch[..]);
    one:[CS_LATENT]int;
    for f:0..<n {
        r.written[1]+=cs_encode(w,s.pixels[..],f,1,one[..]);
        for j:0..<CS_LATENT {r.single[f*CS_LATENT+j]=one[j];}
    }
    r.written[2]=cs_encode(w,s.pixels[..],0,n,r.again[..]);
    bytes:[OB_CODES_MAX/2]u8;p:=cz_pack(r.batch[0..<n*CS_LATENT],bytes[..]);
    r.written[3]=cz_unpack(bytes[0..<p.written],r.unpacked[..]).written;
    r.pixels=cs_decode(w,r.batch[..],n,r.decoded[..]);
    return r;
}
```

It passes on the hand record and on random weights over a 6-frame scene.

### The flow schedule

The scheduler configuration names Karras sigmas from `sigma_min` 0.147 to
`sigma_max` 200 with flow prediction. A Karras schedule of n steps spaces
σ^(1/7) evenly:

σ_i = (σ_max^(1/7) + i/(n − 1) (σ_min^(1/7) − σ_max^(1/7)))^7.

A noise level σ in x = x0 + σ ε is the flow time t = σ / (1 + σ) in x_t =
(1 − t) x0 + t ε (divide by 1 + σ). Both have signal-to-noise ratio 1/σ²,
which a variance-preserving schedule writes as abar = 1 / (1 + σ²).

**By hand, n = 4.** σ_max^(1/7) = 2.13166, σ_min^(1/7) = 0.76041. The
sigmas are 200, 36.926, 3.9652 and 0.147; the flow times 0.99502, 0.97363,
0.79860 and 0.12816.

Chapter 165's scheduler contract is stated over integer levels, with betas
that never decrease. The schedule here has 1,000 levels, level k at flow
time k × t_max / 1000 with t_max = 200/201, and each Karras time is snapped
to its nearest level: 1000 × 0.97363 / 0.99502 = 978.50, level 979; then
803 and 129 (from 802.59 and 128.80). The walk is 1000, 979, 803, 129, 0,
and the sampler uses the snapped times, so what the contract checks is what
runs.

<!-- source: psionic/lib/cosmos.coax schedule -->
```jai
// Karras sigma i of n between sigma_max (i = 0) and sigma_min (i = n - 1).
cs_sigma :: (n:int,i:int)->f64 {
    if n==1 {return CS_SIGMA_MAX;}
    a:=pow(CS_SIGMA_MAX,1/CS_RHO);b:=pow(CS_SIGMA_MIN,1/CS_RHO);
    return pow(a+f64(i)/f64(n-1)*(b-a),CS_RHO);
}
// Level k of 1,000 is flow time k x TMAX / 1000.
cs_time :: (k:int)->f64 {return f64(k)*CS_TMAX/CS_TRAIN_LEVELS;}
// n + 1 levels: each Karras sigma as flow time sigma / (1 + sigma), snapped
// to the nearest level, then 0. Returns n + 1, or 0 when n is not in 1..8 or
// out is too short.
cs_walk :: (n:int,out:[]int)->int {
    if n<1 || n>CS_STEPS_MAX || out.count<n+1 {return 0;}
    for i:0..<n {sg:=cs_sigma(n,i);out[i]=int(floor(sg/(1+sg)*CS_TRAIN_LEVELS/CS_TMAX+0.5));}
    out[n]=0;
    return n+1;
}
// The same signal-to-noise ratio as a variance-preserving abar: x_t =
// (1 - t) x0 + t eps has abar = (1 - t)^2 / ((1 - t)^2 + t^2).
cs_abar :: (t:f64)->f64 {a:=(1-t)*(1-t);return a/(a+t*t);}
cs_schedule :: ()->DfSchedule {
    b:[CS_TRAIN_LEVELS]f64;
    for k:1..<CS_TRAIN_LEVELS+1 {b[k-1]=1-cs_abar(cs_time(k))/cs_abar(cs_time(k-1));}
    return df_betas(b[..]);
}
```

The snapping matters. The unsnapped Karras levels written as betas are
0.0212, 0.9389, 0.9877 and 0.9659: the last is smaller than the one before,
so chapter 146 refuses them and the contract fails. The contract's rule
assumes a variance-preserving schedule; a few Karras steps are not one.
Whether NVIDIA's pipeline, which sets `use_native_flow_schedule`, uses
exactly these times is not established here.

### The denoiser

One layer in the shape of the checkpoint's. Four conditioning tokens (the
last observed frame's latent, as 2 × 2 patches) go through the
understanding weights and attend causally among themselves. Four noisy
tokens go through the generation weights plus a time term, normalize their
queries and keys, and attend to all 8 tokens, seeing the understanding keys
through their own norm. Each tower then runs its own RMS-normalized MLP
with the squared ReLU (`relu2` in the configuration). The output norm and
`proj_out` give a velocity for each latent value. Positions are not encoded:
the real model uses a 3-axis rotary embedding (`mrope_section` 24, 20, 20),
which this toy leaves out.

<!-- source: psionic/lib/cosmos.coax denoiser -->
```jai
// Token k of a latent is the 2 x 2 patch at (k / 2, k % 2); latent value i
// is element cs_tok_elem(i) of token cs_tok(i).
cs_tok :: (i:int)->int {return ((i/CS_GRID)/2)*2+(i%CS_GRID)/2;}
cs_tok_elem :: (i:int)->int {return ((i/CS_GRID)%2)*2+i%2;}
// out = W x for the out.count x x.count matrix W stored row by row at at.
cs_mv :: (w:[]f64,at:int,x:[]f64,out:[]f64) {
    for r:0..<out.count {s:=0.0;for c:0..<x.count {s+=w[at+r*x.count+c]*x[c];}out[r]=s;}
}
cs_rms :: (x:[]f64,w:[]f64,at:int,out:[]f64) {
    s:=0.0;for v:x {s+=v*v;}
    k:=1/sqrt(s/f64(x.count)+CS_EPS);
    for i:0..<x.count {out[i]=x[i]*k*w[at+i];}
}
// Residual MLP of one tower: h += down(relu(up(rms(h)))^2).
cs_mlp :: (w:[]f64,tw:int,h:[]f64) {
    m:[CS_D]f64;cs_rms(h,w,tw+68,m[..]);u:[CS_F]f64;cs_mv(w,tw+72,m[..],u[..]);
    for j:0..<CS_F {r:=(u[j]+abs(u[j]))/2;u[j]=r*r;}
    d:[CS_D]f64;cs_mv(w,tw+104,u[..],d[..]);for j:0..<CS_D {h[j]+=d[j];}
}
// Scores to probabilities in place, shifted by the largest score first so
// no exponential overflows. The largest without a branch: max(a, b) = (a +
// b + |a - b|) / 2.
cs_softmax :: (s:[]f64) {
    top:=s[0];for j:1..<s.count {top=(top+s[j]+abs(top-s[j]))/2;}
    tot:=0.0;for j:0..<s.count {s[j]=exp(s[j]-top);tot+=s[j];}
    for j:0..<s.count {s[j]/=tot;}
}
// One layer, two towers. The 4 conditioning tokens use the understanding
// weights and attend causally among themselves; the 4 noisy tokens use the
// generation weights, normalize their queries and keys, and attend to every
// token, seeing the understanding keys through their own norm. Returns the
// velocity v and the understanding tower's output h, both as latents.
cs_forward :: (w:[]f64,cond:[]f64,x:[]f64,t:f64,v:[]f64,ctx:[]f64) {
    U:=CS_TOWER;G:=CS_TOWER+CS_TW;
    // Hidden states of 8 tokens: understanding 0..3, generation 4..7.
    h:[8*CS_D]f64;
    tc:[CS_TOKENS*CS_WIDTH]f64;tx:[CS_TOKENS*CS_WIDTH]f64;
    for i:0..<CS_LATENT {tc[cs_tok(i)*CS_WIDTH+cs_tok_elem(i)]=cond[i];tx[cs_tok(i)*CS_WIDTH+cs_tok_elem(i)]=x[i];}
    for k:0..<CS_TOKENS {
        cs_mv(w,CS_EU,tc[k*4..<k*4+4],h[k*4..<k*4+4]);
        g:=(k+4)*4;cs_mv(w,CS_WI,tx[k*4..<k*4+4],h[g..<g+4]);
        for j:0..<CS_D {h[g+j]+=w[CS_BI+j]+w[CS_TA+j]*t;}
    }
    // Queries, keys and values; kg holds the understanding keys as the
    // generation tokens see them.
    q:[8*CS_D]f64;kk:[8*CS_D]f64;vv:[8*CS_D]f64;kg:[4*CS_D]f64;a:[CS_D]f64;e:[CS_D]f64;
    for k:0..<8 {
        tw:=U;if k>=4 {tw=G;}
        cs_rms(h[k*4..<k*4+4],w,tw,a[..]);
        cs_mv(w,tw+4,a[..],q[k*4..<k*4+4]);cs_mv(w,tw+20,a[..],kk[k*4..<k*4+4]);cs_mv(w,tw+36,a[..],vv[k*4..<k*4+4]);
        if k>=4 {
            cs_rms(q[k*4..<k*4+4],w,CS_QN,e[..]);for j:0..<4 {q[k*4+j]=e[j];}
            cs_rms(kk[k*4..<k*4+4],w,CS_KN,e[..]);for j:0..<4 {kk[k*4+j]=e[j];}
        } else {
            cs_rms(kk[k*4..<k*4+4],w,CS_KU,kg[k*4..<k*4+4]);
        }
    }
    // Scaled dot products over the visible tokens, softmax, output matrix,
    // residual. All 8 outputs come from the inputs above.
    out:[8*CS_D]f64;
    for k:0..<8 {
        tw:=U;last:=k;if k>=4 {tw=G;last=7;}
        s:[8]f64;
        for j:0..<last+1 {
            d:=0.0;
            if k>=4 && j<4 {for i:0..<4 {d+=q[k*4+i]*kg[j*4+i];}} else {for i:0..<4 {d+=q[k*4+i]*kk[j*4+i];}}
            s[j]=d*0.5;
        }
        cs_softmax(s[0..<last+1]);
        mix:[CS_D]f64;for j:0..<last+1 {for i:0..<4 {mix[i]+=s[j]*vv[j*4+i];}}
        cs_mv(w,tw+52,mix[..],out[k*4..<k*4+4]);
    }
    for i:0..<8*CS_D {h[i]+=out[i];}
    for k:0..<CS_TOKENS {cs_mlp(w,U,h[k*4..<k*4+4]);cs_mlp(w,G,h[(k+4)*4..<(k+4)*4+4]);}
    for i:0..<CS_LATENT {
        k:=cs_tok(i)+4;cs_rms(h[k*4..<k*4+4],w,CS_NF,a[..]);
        r:=cs_tok_elem(i);s:=w[CS_BO+r];for j:0..<CS_D {s+=w[CS_WO+r*CS_D+j]*a[j];}
        v[i]=s;ctx[i]=h[cs_tok(i)*4+r];
    }
}
```

The softmax subtracts the largest score before exponentiating, so a score of
720 above the others neither overflows nor changes the result.

### Euler on the flow ODE

The velocity of x_t = (1 − t) x0 + t ε is ε − x0. Euler steps from the
noise along the walk add (t' − t) v each time.

<!-- source: psionic/lib/cosmos.coax sample -->
```jai
// Euler on the flow ODE from the noise along the walk: x += (t' - t) v. With
// exact set, v is the true velocity toward cond, (x - cond) / t.
cs_sample :: (w:[]f64,cond:[]f64,noise:[]f64,walk:[]int,exact:bool,x:[]f64,first:[]f64,ctx:[]f64) {
    for i:0..<CS_LATENT {x[i]=noise[i];}
    v:[CS_LATENT]f64;h:[CS_LATENT]f64;
    for s:0..<walk.count-1 {
        t:=cs_time(walk[s]);dt:=cs_time(walk[s+1])-t;
        if exact {for i:0..<CS_LATENT {v[i]=(x[i]-cond[i])/t;}} else {cs_forward(w,cond,x,t,v[..],h[..]);}
        if s==0 {for i:0..<CS_LATENT {first[i]=v[i];ctx[i]=h[i];}}
        for i:0..<CS_LATENT {x[i]+=dt*v[i];}
    }
}
```

**By hand.** If v is the exact velocity toward a target x0, v = (x − x0) /
t, then one step gives x' = x0 + (x − x0) t'/t, and the last step (t' = 0)
lands on x0 exactly, for any walk. The first velocity from noise ε at t_max
is (ε − x0) / 0.99502. If instead every weight is 0 except `proj_out`'s
bias b, the velocity is b at every step and the steps telescope: x = ε +
(0 − t_max) b = ε − 0.99502 b, whatever the number of steps. The checks
assert both.

### The whole path

<!-- source: psionic/lib/cosmos.coax generate -->
```jai
// The path: admit the record, encode its last frame, draw the noise, walk
// the levels, decode the latent.
cs_generate :: (cw:[]f32,dw:[]f64,seq:ObSeq,steps:int,seed:u64)->CsRun {
    r:CsRun;
    if ob_admit(seq)!=ObStatus.ok {r.status=CsStatus.record;return r;}
    s:=seq;n:=s.count;
    if !cs_encode_latent(cw,s.pixels[(n-1)*OB_PIXELS..<n*OB_PIXELS],r.cond[..]) {r.status=CsStatus.codec;return r;}
    if dw.count!=CS_PARAMS {r.status=CsStatus.weights;return r;}
    for v:dw {if !df_finite(v) || abs(v)>CS_WEIGHT_MAX {r.status=CsStatus.weights;return r;}}
    k:=cs_walk(steps,r.walk[..]);if k==0 {r.status=CsStatus.steps;return r;}
    r.steps=steps;
    for i:0..<CS_LATENT {r.noise[i]=df_normal(seed,u64(i));}
    cs_sample(dw,r.cond[..],r.noise[..],r.walk[0..<k],false,r.latent[..],r.first[..],r.context[..]);
    for i:0..<CS_LATENT {r.codes[i]=cz_code(r.latent[i]);}
    cs_decode_latent(cw,r.latent[..],r.frame[..]);
    return r;
}
```

**By hand.** With the probe codec and all-zero denoiser weights, one step:
the walk is 1000, 0, the velocity is 0, and the final latent is the noise
itself, drawn from chapter 53's counter generator with seed 7. The
condition is frame 1's latent from the codec trace above: −0.5 on the four
blocks the square touches, −1 elsewhere. Decoding puts noise value i × 0.5
+ 0.5 on the red pixels of block i.

With synthetic weights (normals scaled by 1/2, norms near 1) and 4 steps,
an oracle written separately (its own indexing, an 8 × 8 attention mask,
no shared helpers) recomputes every stage: the condition latent, the
understanding tower's output and the velocity at the first step, the final
latent and the decoded frame. All latents agree within 1e-12 and all
pixels within 1e-6, tolerances frozen before the comparison. A second run
with the same seed gives the same bits.

## Run it

<!-- source: psionic/examples/cosmos-path.coax example -->
```jai
main :: () {
    // 1. Manifest: 19 shapes from config.json against the safetensors headers.
    s:[95]int;cs_config_shapes(s[..]);same:=0;for i:0..<95 {if s[i]==CS_HEADER_SHAPES[i] {same+=1;}}
    print("manifest: % tensors, % parameters, % layers of % tensors; % of 95 shape numbers match the config\n",
        CS_HEADER_TENSORS,CS_HEADER_PARAMETERS,CS_HEADER_LAYERS,cs_per_layer(),same);

    // 2. Real-weight slices, tolerance 1e-12 frozen before comparison.
    u:[192]f64;for i:0..<192 {u[i]=cs_input_in(i);}
    wi:=CS_PROJ_IN_W;bi:=CS_PROJ_IN_B;y:[8]f64;cs_linear_bf16(wi[..],bi[..],8,192,u[..],y[..]);
    ok:=0;for r:0..<8 {if abs(y[r]-CS_REF_PROJ_IN[r])<=1e-12 {ok+=1;}}
    print("proj_in rows 0..7 on a fixed latent patch: % of 8 within 1e-12; row 0 = %\n",ok,show4(y[0]));
    h:[2048]f64;for j:0..<2048 {h[j]=cs_input_out(j);}
    wo:=CS_PROJ_OUT_W;bo:=CS_PROJ_OUT_B;o:[2]f64;cs_linear_bf16(wo[..],bo[..],2,2048,h[..],o[..]);
    ok=0;for r:0..<2 {if abs(o[r]-CS_REF_PROJ_OUT[r])<=1e-12 {ok+=1;}}
    print("proj_out rows 0..1 on a fixed hidden state: % of 2 within 1e-12; % %\n",ok,show4(o[0]),show4(o[1]));
    c:[8]f64;cs_conv_in_slice(c[..]);ok=0;for k:0..<8 {if abs(c[k]-CS_REF_CONV_IN[k])<=1e-12 {ok+=1;}}
    print("VAE encoder conv_in, filters 0 and 1, one frame: % of 8 within 1e-12; filter 0 at (0, 0) = %\n",ok,show4(c[0]));

    // 3. The codec on chapter 165's hand record, under its codec contract.
    cw:[CS_CODEC_PARAMS]f32;cs_probe(cw[..]);rec:=ob_two();
    run:=cs_codec_run(cw[..],rec);
    print("codec contract: %; frame 1 codes:",ob_codec_check(run));for i:16..<32 {print(" %",run.batch[i]);}print("\n");

    // 4. Flow levels, under the scheduler contract.
    walk:[CS_STEPS_MAX+1]int;n:=cs_walk(4,walk[..]);
    print("4 Karras steps, levels:");for i:0..<n {print(" %",walk[i]);}
    print("; flow times:");for i:0..<n {print(" %",show4(cs_time(walk[i])));}
    print("; scheduler contract: %\n",ob_schedule_check(cs_schedule(),walk[0..<n]));

    // 5. The whole path, synthetic two-tower weights, seed 7.
    dw:[CS_PARAMS]f64;cs_weights(dw[..],11);
    r:=cs_generate(cw[..],dw[..],rec,4,7);ora:=cs_oracle_generate(cw[..],dw[..],rec,4,7);
    worst:=0.0;
    for i:0..<16 {for d:[4]f64.[r.latent[i]-ora.latent[i],r.first[i]-ora.first[i],r.context[i]-ora.context[i],r.cond[i]-ora.cond[i]] {if abs(d)>worst {worst=abs(d);}}}
    px:=0;for i:0..<OB_PIXELS {if abs(r.frame[i]-ora.frame[i])<=0.000001 {px+=1;}}
    print("generate: %; every stage within 1e-12 of the oracle: %; frame pixels within 1e-6: % of 192\n",r.status,worst<=1e-12,px);
    print("  condition latent row 1:");for i:4..<8 {print(" %",show4(r.cond[i]));}print("\n");
    print("  first velocity row 1:");for i:4..<8 {print(" %",show4(r.first[i]));}print("\n");
    print("  final latent row 1:");for i:4..<8 {print(" %",show4(r.latent[i]));}print("\n");
    print("  final codes:");for i:0..<16 {print(" %",r.codes[i]);}print("\n");

    // 6. Task evaluation, apart from numerics: red mass within 25%.
    seen:=cs_red_mass(rec.pixels[192..<384]);got:=cs_red_mass(r.frame[..]);
    print("task: observed red mass %, generated % (untrained weights): plausible %\n",seen,show4(got),cs_plausible(seen,got));
    ctx:[CS_LATENT]f64;f:[CS_LATENT]f64;x:[CS_LATENT]f64;
    cs_sample(dw[..],r.cond[..],r.noise[..],walk[0..<n],true,x[..],f[..],ctx[..]);
    frame:[OB_PIXELS]f32;cs_decode_latent(cw[..],x[..],frame[..]);
    print("task: the exact flow toward the last frame's latent gives red mass %: plausible %\n",show4(cs_red_mass(frame[..])),cs_plausible(seen,cs_red_mass(frame[..])));
}
```

```text
manifest: 1188 tensors, 4563688396 parameters, 28 layers of 19 tensors; 95 of 95 shape numbers match the config
proj_in rows 0..7 on a fixed latent patch: 8 of 8 within 1e-12; row 0 = -0.0563
proj_out rows 0..1 on a fixed hidden state: 2 of 2 within 1e-12; -0.3058 -0.1299
VAE encoder conv_in, filters 0 and 1, one frame: 8 of 8 within 1e-12; filter 0 at (0, 0) = -0.2012
codec contract: ok; frame 1 codes: -7 -7 -7 -7 -7 -3 -3 -7 -7 -3 -3 -7 -7 -7 -7 -7
4 Karras steps, levels: 1000 979 803 129 0; flow times: 0.995 0.9741 0.799 0.1284 0.0; scheduler contract: ok
generate: ok; every stage within 1e-12 of the oracle: true; frame pixels within 1e-6: 192 of 192
  condition latent row 1: -1.0 -0.5 -0.5 -1.0
  first velocity row 1: 1.3165 1.4035 0.9908 1.3716
  final latent row 1: -1.3029 -0.9822 0.9797 -3.2472
  final codes: 7 -7 7 -7 -7 -7 7 -7 4 -5 7 -7 6 -7 -7 7
task: observed red mass 4.0, generated 23.5544 (untrained weights): plausible false
task: the exact flow toward the last frame's latent gives red mass 4.0: plausible true
```

## Task evaluation, apart from numbers

Numerical parity says the arithmetic is right. It says nothing about
whether the frames make sense. The task measure here is one stated before
running: a moving square neither appears nor vanishes, so the generated
frame's red mass (the sum of its red channel) must stay within 25% of the
observed frame's.

<!-- source: psionic/lib/cosmos.coax task -->
```jai
// Task evaluation, apart from numerics: the red mass (sum of the red
// channel) of a generated frame stays within 25% of the observed frame's.
// A moving square neither appears nor vanishes.
cs_red_mass :: (x:[]f32)->f64 {s:=0.0;for i:0..<64 {s+=f64(x[i]);}return s;}
cs_plausible :: (observed:f64,generated:f64)->bool {return observed>0 && abs(generated-observed)<=0.25*observed;}
```

The untrained synthetic denoiser passes every numerical check and fails
this one: red mass 23.55 against 4. The exact flow toward the observed
frame's latent passes with 4.0, but only because it was handed the answer.
Neither result says anything about Cosmos3-Edge's behaviour. And a frame
that passes such a check is plausible, not physically accurate: visually
plausible output does not prove accurate simulation.

## Refusals

<!-- source: psionic/examples/cosmos-path-refuse.coax refuse -->
```jai
main :: () {
    cw:[CS_CODEC_PARAMS]f32;cs_probe(cw[..]);dw:[CS_PARAMS]f64;cs_weights(dw[..],11);s:=ob_two();
    b:=s;b.ticks[1]=180;print("a record with a missing frame: %\n",cs_generate(cw[..],dw[..],b,4,7).status);
    print("36 codec weights: %\n",cs_generate(cw[0..<36],dw[..],s,4,7).status);
    c:=cw;c[3]=0.0/0.0;print("a NaN codec weight: % (codec contract: %)\n",cs_generate(c[..],dw[..],s,4,7).status,ob_codec_check(cs_codec_run(c[..],s)));
    print("347 denoiser weights: %\n",cs_generate(cw[..],dw[0..<347],s,4,7).status);
    d:=dw;d[200]=1001;print("a denoiser weight of 1001: %\n",cs_generate(cw[..],d[..],s,4,7).status);
    print("0 steps: %, 9 steps: %\n",cs_generate(cw[..],dw[..],s,0,7).status,cs_generate(cw[..],dw[..],s,9,7).status);
    // Karras sigmas 0.147 .. 200 as variance-preserving betas, unsnapped.
    betas:[4]f64;prev:=0.0;
    for i:0..<4 {sg:=cs_sigma(4,3-i);betas[i]=1-(1+prev*prev)/(1+sg*sg);prev=sg;}
    print("unsnapped Karras betas:");for v:betas {print(" %",floor(v*10000+0.5)/10000);}
    walk:=[5]int.[4,3,2,1,0];
    print("; chapter 146 says %, the contract says %\n",df_betas(betas[..]).status,ob_schedule_check(df_betas(betas[..]),walk[..]));
}
```

```text
a record with a missing frame: record
36 codec weights: codec
a NaN codec weight: codec (codec contract: status)
347 denoiser weights: weights
a denoiser weight of 1001: weights
0 steps: steps, 9 steps: steps
unsnapped Karras betas: 0.0212 0.9389 0.9877 0.9659; chapter 146 says order, the contract says status
```

A record with a missing frame, a codec weight list of the wrong length or
with a NaN, a denoiser weight list of the wrong length or with a weight
above 1,000 in magnitude (the bound that keeps every sum finite), and a
step count outside 1 to 8 are each refused with their status before any
arithmetic. The last line is the unsnapped schedule from above.

## What is not done yet

- **The transformer on original weights.** 28 layers of both towers,
  grouped-query attention (16 query and 8 key-value heads of 128), the
  3-axis rotary embedding, the time embedder's sinusoid and the action
  projections. Shard 1 (5.0 GB) was read for its header only.
- **The full Wan VAE.** Residual blocks with RMS norms, the middle
  attention, temporal down- and up-sampling with feature caches, and the
  48-channel latent with its per-channel mean and standard deviation. Only
  `conv_in` runs here, on one frame.
- **The vision encoder and projector** (27 layers of width 1,152, a 2 × 2
  merge into 2,048), on chapter 143's path, and the text tokenizer and
  chat template.
- **The pipeline's sampler.** The configuration names UniPC with the bh2
  order-2 corrector; this chapter uses first-order Euler steps, and the
  pipeline's native flow schedule is not established.
- **Reference tensors** from NVIDIA's implementation at each stage, which
  need that code run somewhere other than this machine, and a frozen
  tolerance against them.
- **A task evaluation on Aethyr worlds** (Aethyr chapter 31, a design
  draft) for a fixed scenario.

## Dependencies

- [Chapter 165](ch165-sequences-of-observations.md): observation records
  and the codec and scheduler contracts.
- [Chapter 146](ch146-noise-diffusion-and-flow.md): noise schedules and
  normal draws.
- [Chapter 145](ch145-compress-and-reconstruct-an-image.md): the 15-level
  codes and their byte packing.
- [Chapter 143](ch143-elephant-and-mammoth-see.md) and
  [chapter 144](ch144-see-with-c-radio.md): the vision paths the vision
  encoder will follow.
- Aethyr worlds for test scenarios
  ([Aethyr chapter 31](../../aethyr/book/ch031-many-worlds-in-one-step.md)).

## Sources

- [Chapter plan](PLAN.md), Part XVIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), Cosmos entries.
- The checkpoint's `config.json`, `transformer/config.json`,
  `vae/config.json` and `scheduler/scheduler_config.json` at the pinned
  revision, and the safetensors headers of its four weight files.
- The `patchify` and `WanCausalConv3d` definitions in diffusers'
  `autoencoder_kl_wan.py`, read, not run.
