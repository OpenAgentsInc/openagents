# 149. Execute MotionBricks

> **Status: partly implemented.** The pinned MotionBricks SOMA package is
> downloaded, hashed and checked against the architecture tensor by tensor:
> all 94 tensors of the pose VQ-VAE and all 210 of the pose predictor, with
> the root predictor's 174 tensors counted but not derived. The pose
> tokenizer runs on the real codebook. The VQ-VAE decoder, the masked
> pose-token transformer and its confidence schedule run on small synthetic
> weights, each matching a separately written reference, and decoded motion
> passes chapter 148's continuity check. Still to do: the root predictor
> (duration and trajectory), the decoder and predictor on the real weights,
> SOMA's feature layout and skeleton reconstruction, and comparison of full
> fixed-input trajectories with recorded reference outputs of the original
> PyTorch code.

MotionBricks is NVIDIA's real-time motion generator for humanoid robots and
human characters. You give it the character's recent poses and the poses it
should reach, and it generates the movement between them. Its SOMA package
works on the 30-joint SOMA skeleton at 30 frames a second and has three
networks:

- a **root predictor**, a transformer that picks how long the move takes (6
  to 16 tokens) and predicts the root's path over the ground;
- a **pose predictor**, a masked transformer that predicts motion tokens:
  each token stands for 4 frames and holds 5 codes, one for each of 5
  codebook heads of 16 codes;
- a **pose VQ-VAE**, whose decoder turns tokens back into per-frame features
  (root motion, joint positions, 6D rotations, velocities and foot
  contacts).

[Chapter 148](ch148-motion-tokens-become-poses.md) built a codebook, a
lookup decoder and masked filling with a counting model. This chapter
replaces those stand-ins with MotionBricks' own architecture, in
[`motionbricks.coax`](../lib/motionbricks.coax), in four parts:

- **the package**: the pinned checkpoints, their hashes, and a line-by-line
  check that the tensors are the ones the architecture needs;
- **the tokenizer**: packing, unpacking, lookup and re-encoding on the real
  codebook;
- **the networks**: the decoder and the pose predictor, at a small size with
  synthetic weights, each compared with a reference written a second way;
- **generation**: the confidence schedule, a generated token sequence, the
  decoded clip and its continuity check.

Everything is binary64 (profile `cpu-f64-motionbricks-v1`). The bounds admit
the real tokenizer (5 heads, 16 codes, 32 values a code) and 16 tokens of 4
frames; synthetic networks are at most 16 wide.

## Run it

```shell
coax/bin/coax run psionic/examples/motionbricks.coax
coax/bin/coax run psionic/examples/motionbricks-refuse.coax
coax/bin/coax test psionic/lib/motionbricks.coax
coax/bin/coax mutate psionic/lib/motionbricks.coax --root-only
```

The checkpoint fixtures are rebuilt and checked from a download of the
package's `soma/soma-v6-nova-4n8g-ea43dd8-fp32-v1` folder:

```shell
python3 -I psionic/tests/motionbricks_manifest.py SOMA_DIRECTORY
```

## The package

The pinned revision is
[`nvidia/MotionBricks` at `3cb362d9b95aa40462abcb4d8cfae9ffb6d97410`](https://huggingface.co/nvidia/MotionBricks/tree/3cb362d9b95aa40462abcb4d8cfae9ffb6d97410).
Its SOMA checkpoints are plain SafeTensors files, FP32, at training step
1,500,000, and none is gated:

| Component | File | Bytes | SHA-256 |
| --- | --- | ---: | --- |
| Pose VQ-VAE | `vqvae-v6-soma-nova-4n8g-ea43dd8-fp32-v1/version_0/checkpoints/model-step=1500000.safetensors` | 93,442,464 | `ba0d1db48ba7182ef5d1a72268bfaa1a196f1f1fbf00144c046d947b90684a1a` |
| Pose predictor | `mmm-v6-soma-pose-nova-4n8g-ea43dd8-fp32-v1/version_0/checkpoints/model-step=1500000.safetensors` | 546,109,841 | `8537d9d33a880ff1df6c16c28ed061d60c461a5918d70b1cf99251ff91a10a10` |
| Root predictor | `mmm-v6-soma-root-nova-4n8g-ea43dd8-fp32-v1/version_0/checkpoints/model-step=1500000.safetensors` | 161,695,084 | `a48d48c3d8ff98c825ca315797644789ae37f2db9069779a3118f42e84f4f29e` |

Each hash was computed over the downloaded file and equals the hash the
repository records for it. The terms are separate, and the
[package card](https://huggingface.co/nvidia/MotionBricks/blob/3cb362d9b95aa40462abcb4d8cfae9ffb6d97410/README.md)
states them: the weights are under the NVIDIA Open Model Agreement, the
MotionBricks software is Apache-2.0, and the BONES motion samples bundled
with the software are under the NVIDIA Sample Data Evaluation License, for
internal evaluation only. This chapter uses no sample motion: every pose it
runs is made up in the chapter. The architecture was read, not run, from the
Apache-2.0 exporter in
[NVlabs/motionbricks at `54fc563`](https://github.com/NVlabs/motionbricks/tree/54fc563aeaaf819a753d72cc9fdc6713989cdd43/tools/onnx/models)
(`layers.py`, `quantizer.py`, `vqvae.py`, `soma/networks.py` and
`soma/inference.py`); no downloaded code was executed.

The tooling script
[`motionbricks_manifest.py`](../tests/motionbricks_manifest.py) hashes each
file, reads only its JSON header, and writes one line a tensor (name, dtype,
dimensions) into
[`motionbricks-manifest.coax`](../fixtures/motionbricks-manifest.coax). It
also copies one real tensor, the tokenizer's 5 × 16 × 32 codebook, as F32 bit
patterns into
[`motionbricks-codebook.coax`](../fixtures/motionbricks-codebook.coax). The
library never reads the 800 MB of weights; the fixtures are what it checks.

The sizes come from the packages' `config.yaml` files and the model card:

<!-- source: psionic/lib/motionbricks.coax config -->
```jai
// The SOMA package's sizes, from its config.yaml files and model card.
MbConfig :: struct {
    heads:int;codes:int;head_dim:int;down:int;width:int;depth:int;
    pose_in:int;decoded:int;external:int;
    embd:int;layers:int;ffn:int;pose_feat:int;root_feat:int;length_feat:int;root_in:int;mlp_layers:int;
    min_tokens:int;max_tokens:int;
}
mb_soma :: ()->MbConfig {
    return MbConfig.{heads=5,codes=16,head_dim=32,down=2,width=512,depth=4,pose_in=268,decoded=365,external=2,
        embd=1024,layers=16,ffn=2048,pose_feat=640,root_feat=256,length_feat=128,root_in=4,mlp_layers=2,min_tokens=6,max_tokens=16};
}
```

Two places where the config file and the checkpoint disagree are settled by
the checkpoint: the config says the encoder reads 241 features and the
decoder writes 329, but the stored convolutions read 268 and write 365, as
the model card says.

### Counting the VQ-VAE's tensors by hand

The encoder is a convolution, two halving levels and a last convolution.
Each level is a strided convolution and a residual stack of 4 blocks, and
each block has two convolutions; each convolution has a weight and a bias:
2 + 2 × (2 + 4 × 2 × 2) + 2 = 40 tensors. The decoder mirrors it: a
convolution, two doubling levels of 4 blocks and one convolution each, and
two last convolutions: 2 + 2 × (4 × 2 × 2 + 2) + 2 + 2 = 42. Each doubling
level also has two condition projections, a weight and a bias each: 8. The
codebook has its values and three training statistics: 4. That is
40 + 42 + 8 + 4 = 94.

The shapes follow from the sizes. The decoder's first convolution reads the
token's latent, 5 heads × 32 = 160 values, so its weight is 512 × 160 × 3.
Before the first doubling a position holds 4 frames, so the target-pose
projection writes 512 / 4 = 128 values a frame and the velocity projection
reads 512 + 4 × 2 = 520; before the second, 256 and 516.

<!-- source: psionic/lib/motionbricks.coax vqvae-lines -->
```jai
// The pose VQ-VAE: a convolutional encoder that halves time twice, a
// decoder that doubles it twice with residual blocks of dilated
// convolutions, the two condition projections a doubling, and the
// multi-head codebook with its training statistics.
mb_vqvae_lines :: (c:MbConfig,out:[]u8)->int {
    s:[96]u8;w:=c.width;z:=c.heads*c.head_dim;
    n:=mb_conv_lines(out,0,"pose_net.encoder.model.0",w,c.pose_in,3);
    for i:0..<c.down {
        n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.encoder.model.",2+i,".0",-1,""),w,w,4);
        for d:0..<c.depth {
            n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.encoder.model.",2+i,".1.model.",d,".conv1"),w,w,3);
            n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.encoder.model.",2+i,".1.model.",d,".conv2"),w,w,1);
        }
    }
    n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.encoder.model.",2+c.down,"",-1,""),z,w,3);
    n=mb_conv_lines(out,n,"pose_net.decoder.model.0",w,z,3);
    for i:0..<c.down {
        for d:0..<c.depth {
            n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.decoder.model.",2+i,".0.model.",d,".conv1"),w,w,3);
            n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.decoder.model.",2+i,".0.model.",d,".conv2"),w,w,1);
        }
        n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.decoder.model.",2+i,".2",-1,""),w,w,3);
        per:=1<<(c.down-i);
        n=mb_linear_lines(out,n,mb_stem(s[..],"pose_net.decoder.external_cond_blocks.",2*i,"",-1,""),w,per*c.external+w);
        n=mb_linear_lines(out,n,mb_stem(s[..],"pose_net.decoder.target_cond_blocks.",2*i,"",-1,""),w/per,c.pose_in);
    }
    n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.decoder.model.",2+c.down,"",-1,""),w,w,3);
    n=mb_conv_lines(out,n,mb_stem(s[..],"pose_net.decoder.model.",4+c.down,"",-1,""),c.decoded,w,3);
    q:="pose_net.quantizer.vq._codebook.";
    n=mb_param(out,n,q,"cluster_size","F32",[3]int.[c.heads,c.codes,c.codes],2);
    n=mb_param(out,n,q,"embed","F32",[3]int.[c.heads,c.codes,c.head_dim],3);
    n=mb_param(out,n,q,"embed_avg","F32",[3]int.[c.heads,c.codes,c.head_dim],3);
    return mb_param(out,n,q,"initted","F32",[3]int.[1,c.heads,c.heads],1);
}
```

The pose predictor has 18 tensors outside its transformer and 12 in each of
its 16 encoder layers (the joint query-key-value projection, the output
projection, two feed-forward layers and two LayerNorms, each with weight and
bias): 18 + 16 × 12 = 210. Its token table has 5 × 17 = 85 rows of 32: 16
codes and the mask for each head.

<!-- source: psionic/lib/motionbricks.coax pose-lines -->
```jai
// The pose predictor: per-head token embeddings (16 codes and the mask), a
// two-layer MLP over them, projections of the pose constraints, the root
// trajectory and the length, a post-norm transformer encoder and one
// output layer of 16 logits a head.
mb_pose_lines :: (c:MbConfig,out:[]u8)->int {
    s:[96]u8;e:=c.embd;fpt:=1<<c.down;
    n:=mb_param(out,0,"mmm_net._pose_token_emb",".weight","F32",[3]int.[c.heads*(c.codes+1),c.head_dim,c.head_dim],2);
    n=mb_param(out,n,"mmm_net._position_emb",".embed","F32",[3]int.[c.max_tokens,1,e],3);
    n=mb_linear_lines(out,n,"mmm_net._proj_input.0",e,c.pose_feat+c.root_feat+c.length_feat);
    n=mb_linear_lines(out,n,"mmm_net._proj_local_pose",c.pose_feat/fpt,c.pose_in);
    n=mb_linear_lines(out,n,"mmm_net._proj_local_root_values",c.root_feat,c.root_in*fpt);
    n=mb_param(out,n,"mmm_net._proj_num_valid_positions",".weight","F32",[3]int.[c.max_tokens-c.min_tokens+1,c.length_feat,c.length_feat],2);
    n=mb_linear_lines(out,n,"mmm_net._proj_pose_output_logit",c.heads*c.codes,e);
    for k:0..<c.mlp_layers {
        n=mb_linear_lines(out,n,mb_stem(s[..],"mmm_net._proj_pose_token_emb.fc_layers.",k,"",-1,""),c.pose_feat,ifx k==0 then c.heads*c.head_dim else c.pose_feat);
    }
    n=mb_linear_lines(out,n,"mmm_net._proj_pose_token_emb.forward_projection",c.pose_feat,c.pose_feat);
    for l:0..<c.layers {
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".self_attn",-1,""),".in_proj_weight","F32",[3]int.[3*e,e,e],2);
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".self_attn",-1,""),".in_proj_bias","F32",[3]int.[3*e,e,e],1);
        n=mb_linear_lines(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".self_attn.out_proj",-1,""),e,e);
        n=mb_linear_lines(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".linear1",-1,""),c.ffn,e);
        n=mb_linear_lines(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".linear2",-1,""),e,c.ffn);
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".norm1",-1,""),".weight","F32",[3]int.[e,e,e],1);
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".norm1",-1,""),".bias","F32",[3]int.[e,e,e],1);
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".norm2",-1,""),".weight","F32",[3]int.[e,e,e],1);
        n=mb_param(out,n,mb_stem(s[..],"mmm_net._transformer_model.layers.",l,".norm2",-1,""),".bias","F32",[3]int.[e,e,e],1);
    }
    return mb_param(out,n,"mmm_net.initted","","BOOL",[3]int.[1,e,e],1);
}
```

The inventory compares the expected lines with the manifest's as sets, and
also adds up every F32 tensor's values:

<!-- source: psionic/lib/motionbricks.coax inventory -->
```jai
// A manifest matches the expected text when it has exactly the expected
// lines, each once, in any order. bad names the first expected line that
// is missing or repeats, or is -1 when every expected line is there once
// but the manifest has more. parameters counts the F32 values the
// manifest holds; it is computed for any manifest, so the root
// predictor's package, whose layout this chapter does not derive, gets a
// count too. More than MB_LINES_MAX lines are refused unread.
MbInventory :: struct {status:MbStatus;lines:int;expected:int;parameters:int;bad:int;}
MB_LINES_MAX :: 512;
MB_TEXT_MAX :: 32768;
mb_inventory :: (expected:string,manifest:string)->MbInventory {
    ends:[513]int;lines:=0;ends[0]= -1;r:MbInventory;r.bad= -1;
    for i:0..<manifest.count {if manifest[i]=='\n' {
        if lines==MB_LINES_MAX {r.status=MbStatus.checkpoint;r.lines=lines+1;return r;}
        lines+=1;ends[lines]=i;
    }}
    r.lines=lines;
    for k:0..<lines {
        line:=manifest[ends[k]+1..<ends[k+1]];
        // The dimensions follow the dtype " F32 ".
        at:= -1;for i:0..<line.count-4 {if at== -1 && cast(string)line[i..<i+5]==" F32 " {at=i+5;}}
        if at!= -1 {
            size:=1;v:=0;
            for i:at..<line.count {if line[i]==',' {size*=v;v=0;} else {v=v*10+cast(int)(line[i]-'0');}}
            if at<line.count {size*=v;}
            r.parameters+=size;
        }
    }
    start:=0;
    for i:0..<expected.count {if expected[i]=='\n' {
        want:=expected[start..<i];found:=0;
        for k:0..<lines {from:=ends[k]+1;if ends[k+1]-from==want.count && manifest[from..<ends[k+1]]==want {found+=1;}}
        if found!=1 && r.bad== -1 {r.bad=r.expected;}
        r.expected+=1;start=i+1;
    }}
    if r.bad!= -1 || lines!=r.expected {r.status=MbStatus.checkpoint;}
    return r;
}
```

The sums are 23,357,918, 136,520,976 and 40,418,385, the model card's
"23.4 million", "136.5 million" and "40.4 million". The root predictor is
counted only: its 174 tensor names are in the fixture, but this chapter does
not derive its layout.

## The tokenizer

A token holds one code per head. The real codebook is read from its bit
patterns, so every value is the exact number the checkpoint stores:

<!-- source: psionic/lib/motionbricks.coax codebook -->
```jai
// The real codebook, read exactly: each F32 bit pattern becomes the binary64
// number it stands for.
mb_soma_codebook :: ()->MbCodebook {
    b:MbCodebook;b.heads=5;b.codes=16;b.dim=32;
    for h:0..<5 {for e:0..<16 {for i:0..<32 {b.value[h][e][i]=f64(f32_from_bits(MB_SOMA_CODEBOOK_BITS[(h*16+e)*32+i]));}}}
    return b;
}
```

MotionBricks can also carry a token as one integer, with head 0 as the
least significant digit in base 16:

<!-- source: psionic/lib/motionbricks.coax pack -->
```jai
// A packed token is one integer: head h's code is digit h in base codes,
// head 0 the least significant, so 16^5 = 1,048,576 tokens in all.
mb_pack :: (b:MbCodebook,c:MbCode)->int {
    if !mb_book_ok(b) {return -1;}
    p:=0;for h:0..<b.heads {if c.code[h]<0 || c.code[h]>=b.codes {return -1;}p+=c.code[h]*mb_power(b.codes,h);}
    return p;
}
mb_unpack :: (b:MbCodebook,packed:int)->MbCode {
    out:MbCode;
    if !mb_book_ok(b) {out.status=MbStatus.shape;return out;}
    if packed<0 || packed>=mb_power(b.codes,b.heads) {out.status=MbStatus.token;return out;}
    for h:0..<b.heads {out.code[h]=packed%mb_power(b.codes,h+1)/mb_power(b.codes,h);}
    return out;
}
```

By hand, codes (1, 2, 3, 4, 5) pack to
1 + 2 × 16 + 3 × 256 + 4 × 4,096 + 5 × 65,536 = 1 + 32 + 768 + 16,384 + 327,680 = 344,865.
Unpacking head 2 takes 344,865 mod 16³ = 801 and divides by 16² to get 3.
Packed numbers run from 0 to 16⁵ − 1 = 1,048,575; 1,048,576 is refused.

Decoding starts with a lookup: each head's 32 values, laid end to end into
160. Encoding goes back head by head, choosing the nearest code:

<!-- source: psionic/lib/motionbricks.coax lookup -->
```jai
// Decoding starts by looking each head's code up and laying the heads end
// to end. Encoding picks, head by head, the code nearest the latent's slice
// in squared distance, the lower number on ties.
mb_dequantize :: (b:MbCodebook,c:MbCode)->MbLatent {
    out:MbLatent;
    if !mb_book_ok(b) {out.status=MbStatus.shape;return out;}
    for h:0..<b.heads {
        if c.code[h]<0 || c.code[h]>=b.codes {out.status=MbStatus.token;return out;}
        for i:0..<b.dim {out.value[h*b.dim+i]=b.value[h][c.code[h]][i];}
    }
    return out;
}
mb_quantize :: (b:MbCodebook,z:[MB_LATENT_MAX]f64)->MbQuantized {
    out:MbQuantized;
    if !mb_book_ok(b) {out.status=MbStatus.shape;return out;}
    for h:0..<b.heads {
        for e:0..<b.codes {
            s:=0.0;for i:0..<b.dim {d:=z[h*b.dim+i]-b.value[h][e][i];s+=d*d;}
            if e==0 || s<out.squared[h] {out.squared[h]=s;out.code[h]=e;}
        }
    }
    return out;
}
```

A codebook entry encodes to itself at distance 0. A latent nudged by less
than half the distance between the two closest codes of a head keeps that
head's code: the closest pair in any head of the real codebook is head 1's
codes 7 and 12, 4.906655 apart, so a nudge of 0.43 in each of 32 values
(0.43 √32 = 2.432, below 4.906655 / 2 = 2.453) changes no code.

## The decoder

The decoder is a stack of 1D convolutions over token positions:

<!-- source: psionic/lib/motionbricks.coax conv -->
```jai
// One 1D convolution over positions: x is [position][channel], zero beyond
// both ends. Output o at position p is bias o plus, for every input
// channel i and tap j, weight (o, i, j) times x at p + j dilation - pad.
mb_conv :: (w:[]f64,at:int,x:[]f64,positions:int,cin:int,cout:int,taps:int,dilation:int,out:[]f64) {
    pad:=dilation*(taps-1)/2;
    for p:0..<positions {for o:0..<cout {
        v:=w[at+cout*cin*taps+o];
        for i:0..<cin {for j:0..<taps {
            q:=p+j*dilation-pad;
            if q>=0 && q<positions {v+=w[at+(o*cin+i)*taps+j]*x[q*cin+i];}
        }}
        out[p*cout+o]=v;
    }}
}
```

Before each doubling it takes two kinds of condition. A frame with a target
pose overwrites its share of its position's channels: with 4 frames to a
position of 8 channels, frame t owns channels 2(t mod 4) and 2(t mod 4) + 1
of position t / 4. Then each position is re-projected together with the
ground velocities of its frames. The residual blocks use dilations 3, then 1
(SOMA: 27, 9, 3, 1), each position is repeated, and a convolution follows.

<!-- source: psionic/lib/motionbricks.coax decode -->
```jai
// n tokens become 4n frames of features. Before each doubling, every
// constrained frame overwrites its share of its position's channels with
// a projection of its pose (a position holds 4, then 2 frames), and every
// position is re-projected with the ground velocities of its frames; then
// the residual blocks, the doubling (each position repeated) and a
// convolution. Two last convolutions give the features.
mb_decode :: (s:MbShape,w:[]f64,b:MbCodebook,q:MbTokens,c:MbCond)->MbMotion {
    out:MbMotion;
    if !mb_shape_ok(s) || b.heads!=s.heads || b.codes!=s.codes || b.dim!=s.dim || w.count<mb_decoder_plan(s).count {out.status=MbStatus.shape;return out;}
    if q.count<1 || q.count>MB_TOKENS {out.status=MbStatus.length;return out;}
    p:=mb_decoder_plan(s);wd:=s.width;z:=s.heads*s.dim;
    x:[MB_FRAMES*MB_WIDTH_MAX]f64;
    for t:0..<q.count {
        code:MbCode;for h:0..<s.heads {code.code[h]=q.code[t][h];}
        l:=mb_dequantize(b,code);if l.status!=MbStatus.ok {out.status=l.status;return out;}
        for i:0..<z {x[t*z+i]=l.value[i];}
    }
    h:[MB_FRAMES*MB_WIDTH_MAX]f64;g:[MB_FRAMES*MB_WIDTH_MAX]f64;
    positions:=q.count;
    mb_conv(w,0,x[..],positions,z,wd,3,1,h[..]);mb_relu(h[..],positions*wd);
    for i:0..<2 {
        per:=1<<(2-i);slice:=wd/per;
        for t:0..<positions*per {if c.has[t] {
            at:=p.target[i];
            for o:0..<slice {
                v:=w[at+slice*s.pose_in+o];for k:0..<s.pose_in {v+=w[at+o*s.pose_in+k]*c.pose[t][k];}
                h[t*slice+o]=mb_relu1(v);
            }
        }}
        cin:=wd+2*per;at:=p.external[i];
        for r:0..<positions {
            for o:0..<wd {
                v:=w[at+wd*cin+o];
                for k:0..<wd {v+=w[at+o*cin+k]*h[r*wd+k];}
                for f:0..<per {for k:0..<2 {v+=w[at+o*cin+wd+2*f+k]*c.velocity[r*per+f][k];}}
                g[r*wd+o]=mb_relu1(v);
            }
        }
        for d:0..<s.depth {
            dilation:=mb_power(s.growth,s.depth-1-d);
            for k:0..<positions*wd {h[k]=mb_relu1(g[k]);}
            mb_conv(w,p.block[i][d][0],h[..],positions,wd,wd,3,dilation,x[..]);mb_relu(x[..],positions*wd);
            for r:0..<positions {mb_affine(w,p.block[i][d][1],wd,wd,x[r*wd..<MB_FRAMES*MB_WIDTH_MAX],h[r*wd..<MB_FRAMES*MB_WIDTH_MAX]);}
            for k:0..<positions*wd {g[k]+=h[k];}
        }
        for r:0..<positions {for k:0..<wd {h[2*r*wd+k]=g[r*wd+k];h[(2*r+1)*wd+k]=g[r*wd+k];}}
        positions*=2;
        mb_conv(w,p.up[i],h[..],positions,wd,wd,3,1,g[..]);
        for k:0..<positions*wd {h[k]=g[k];}
    }
    mb_conv(w,p.mid,h[..],positions,wd,wd,3,1,g[..]);mb_relu(g[..],positions*wd);
    mb_conv(w,p.out,g[..],positions,wd,s.decoded,3,1,h[..]);
    out.frames=positions;
    for t:0..<positions {for k:0..<s.decoded {out.value[t][k]=h[t*s.decoded+k];}}
    return out;
}
```

The checks run it at a small size, `mb_small`: 2 heads of 4 codes of 2
values, width 8, 2 blocks a level, 13 pose features in and 16 features out.
By hand, its weights take
104 + (2 × (200 + 72) + 200 + 136 + 28) + (2 × (200 + 72) + 200 + 104 + 56) + 200 + 400 = 2,516
values: the first convolution 8 × 4 × 3 + 8 = 104; a block 8 × 8 × 3 + 8 =
200 and 8 × 8 + 8 = 72; the velocity projections 8 × 16 + 8 = 136 and
8 × 12 + 8 = 104; the pose projections 2 × 13 + 2 = 28 and 4 × 13 + 4 = 56;
the last two convolutions 200 and 16 × 8 × 3 + 16 = 400. With every weight
0, the output is the last convolution's bias at every frame.

## The pose predictor

The pose predictor reads all 16 positions, of which the first n are real:

<!-- source: psionic/lib/motionbricks.coax predict -->
```jai
// The pose predictor reads max_tokens positions of which the first n are
// real. A position's input is its pose part (frame by frame: the projected
// constraint where a frame has one, otherwise that frame's quarter of the
// MLP of the position's token embeddings), its root part (the projected
// root trajectory of its 4 frames) and the length's embedding, projected
// with ReLU, plus the position code. Post-norm encoder layers follow, whose
// attention never looks at a padded position, then the logits.
mb_predict :: (s:MbShape,w:[]f64,q:MbTokens,c:MbCond)->MbLogits {
    out:MbLogits;
    if !mb_shape_ok(s) || w.count<mb_predictor_plan(s).count {out.status=MbStatus.shape;return out;}
    if q.count<s.min_tokens || q.count>s.max_tokens {out.status=MbStatus.length;return out;}
    for t:0..<s.max_tokens {for h:0..<s.heads {if q.code[t][h]<0 || q.code[t][h]>s.codes {out.status=MbStatus.token;return out;}}}
    p:=mb_predictor_plan(s);e:=s.embd;m:=s.max_tokens;n:=q.count;quarter:=s.pose_feat/MB_FPT;
    x:[MB_TOKENS][MB_WIDTH_MAX]f64;
    for t:0..<m {
        a:[MB_WIDTH_MAX]f64;b:[MB_WIDTH_MAX]f64;cat:[MB_FEATURE_MAX]f64;
        for h:0..<s.heads {for i:0..<s.dim {a[h*s.dim+i]=w[(h*(s.codes+1)+q.code[t][h])*s.dim+i];}}
        mb_affine(w,p.mlp[0],s.pose_feat,s.heads*s.dim,a[..],b[..]);mb_leaky(b[..],s.pose_feat);
        mb_affine(w,p.mlp[1],s.pose_feat,s.pose_feat,b[..],a[..]);mb_leaky(a[..],s.pose_feat);
        mb_affine(w,p.mlp[2],s.pose_feat,s.pose_feat,a[..],cat[..]);
        for f:0..<MB_FPT {if c.has[t*MB_FPT+f] {
            pose:[MB_WIDTH_MAX]f64;for k:0..<s.pose_in {pose[k]=c.pose[t*MB_FPT+f][k];}
            mb_affine(w,p.pose,quarter,s.pose_in,pose[..],b[..]);
            for k:0..<quarter {cat[f*quarter+k]=b[k];}
        }}
        root:[4*MB_FPT]f64;for f:0..<MB_FPT {for k:0..<4 {root[f*4+k]=c.root[t*MB_FPT+f][k];}}
        mb_affine(w,p.root,s.root_feat,4*MB_FPT,root[..],b[..]);
        for k:0..<s.root_feat {cat[s.pose_feat+k]=b[k];}
        for k:0..<s.length_feat {cat[s.pose_feat+s.root_feat+k]=w[p.length+(n-s.min_tokens)*s.length_feat+k];}
        mb_affine(w,p.input,e,s.pose_feat+s.root_feat+s.length_feat,cat[..],a[..]);
        for k:0..<e {x[t][k]=mb_relu1(a[k])+mb_position(t,k,e);}
    }
    hd:=e/s.attn;scale:=1/sqrt(f64(hd));
    for l:0..<s.layers {
        at:=p.layer[l];out_proj:=at+mb_conv_size(3*e,e,1);lin1:=out_proj+mb_conv_size(e,e,1);lin2:=lin1+mb_conv_size(s.ffn,e,1);norm:=lin2+mb_conv_size(e,s.ffn,1);
        qkv:[MB_TOKENS][3*MB_WIDTH_MAX]f64;next:[MB_TOKENS][MB_WIDTH_MAX]f64;
        for t:0..<m {mb_affine(w,at,3*e,e,x[t][..],qkv[t][..]);}
        for t:0..<m {
            mix:[MB_WIDTH_MAX]f64;
            for a:0..<s.attn {
                score:[MB_TOKENS]f64;top:=0.0;
                for k:0..<n {v:=0.0;for i:0..<hd {v+=qkv[t][a*hd+i]*qkv[k][e+a*hd+i];}score[k]=v*scale;top=ifx k==0 then score[k] else mb_larger(top,score[k]);}
                sum:=0.0;for k:0..<n {score[k]=exp(score[k]-top);sum+=score[k];}
                for i:0..<hd {v:=0.0;for k:0..<n {v+=score[k]*qkv[k][2*e+a*hd+i];}mix[a*hd+i]=v/sum;}
            }
            y:[MB_WIDTH_MAX]f64;mb_affine(w,out_proj,e,e,mix[..],y[..]);
            for k:0..<e {y[k]+=x[t][k];}
            mb_norm(w,norm,y[..],e);
            hidden:[MB_FFN_MAX]f64;mb_affine(w,lin1,s.ffn,e,y[..],hidden[..]);mb_relu(hidden[..],s.ffn);
            z:[MB_WIDTH_MAX]f64;mb_affine(w,lin2,e,s.ffn,hidden[..],z[..]);
            for k:0..<e {z[k]+=y[k];}
            mb_norm(w,norm+2*e,z[..],e);
            for k:0..<e {next[t][k]=z[k];}
        }
        x=next;
    }
    for t:0..<m {
        y:[MB_HEADS_MAX*MB_CODES_MAX]f64;mb_affine(w,p.logit,s.heads*s.codes,e,x[t][..],y[..]);
        for h:0..<s.heads {for k:0..<s.codes {out.value[t][h][k]=y[h*s.codes+k];}}
    }
    return out;
}
```

A padded position never changes a real one: its key is left out of every
attention, which a check confirms by changing everything at the padded
positions and comparing the real logits bit for bit.

## Generation

The schedule decides how many tokens to mask before each step:

<!-- source: psionic/lib/motionbricks.coax schedule -->
```jai
// Before step s of S, the masked count is max(1, floor(cos(s pi / 2S) n H))
// of the n H real tokens: all of them at step 0, then fewer.
mb_mask_count :: (step:int,steps:int,n:int,heads:int)->int {
    m:=cast(int)floor(cos(f64(step)/f64(steps)*POSE_PI*0.5)*f64(n*heads));
    return m+(ifx m==0 then 1 else 0);
}
```

By hand, for 6 tokens of 5 heads (30 codes) over 4 steps:
step 0 masks 30 cos 0 = 30; step 1, 30 cos(π/8) = 30 × 0.92388 = 27.7, so 27;
step 2, 30 cos(π/4) = 21.2, so 21; step 3, 30 cos(3π/8) = 11.5, so 11. A
final step's count is never below 1.

<!-- source: psionic/lib/motionbricks.coax generate -->
```jai
// Every token starts as the mask (code number codes); all confidences
// start equal, and step 0 masks every real token. Each step masks the m least confident real tokens (the lower
// position, then head, on ties), runs the predictor and takes every
// token's best code (the lower code on ties) and its softmax probability
// as the new confidence. Unmasked tokens are predicted again too.
mb_generate :: (s:MbShape,w:[]f64,n:int,c:MbCond,steps:int)->MbGenerate {
    out:MbGenerate;out.steps=steps;
    if !mb_shape_ok(s) || w.count<mb_predictor_plan(s).count {out.status=MbStatus.shape;return out;}
    if n<s.min_tokens || n>s.max_tokens || steps<1 || steps>MB_TOKENS {out.status=MbStatus.length;return out;}
    q:MbTokens;q.count=n;prob:[MB_TOKENS][MB_HEADS_MAX]f64;
    for t:0..<s.max_tokens {for h:0..<s.heads {q.code[t][h]=s.codes;}}
    for step:0..<steps {
        m:=mb_mask_count(step,steps,n,s.heads);out.masked[step]=m;
        before:=q;done:[MB_TOKENS][MB_HEADS_MAX]bool;
        for r:0..<m {
            pick:= -1;
            for t:0..<n {for h:0..<s.heads {if !done[t][h] && (pick== -1 || prob[t][h]<prob[pick/s.heads][pick%s.heads]) {pick=t*s.heads+h;}}}
            done[pick/s.heads][pick%s.heads]=true;q.code[pick/s.heads][pick%s.heads]=s.codes;out.last[step]=pick;
        }
        l:=mb_predict(s,w,q,c);
        for t:0..<s.max_tokens {for h:0..<s.heads {
            best:=0;for k:1..<s.codes {if l.value[t][h][k]>l.value[t][h][best] {best=k;}}
            sum:=0.0;for k:0..<s.codes {sum+=exp(l.value[t][h][k]-l.value[t][h][best]);}
            q.code[t][h]=best;prob[t][h]=1/sum;
            if t<n && best!=before.code[t][h] {out.changed[step]+=1;}
        }}
    }
    out.tokens=q;
    for t:0..<n {for h:0..<s.heads {out.confidence[t][h]=prob[t][h];}}
    return out;
}
```

Two details follow MotionBricks' exporter and differ from chapter 148's
filling. Every step predicts every token again, including the ones not
masked, so a token can change after it is first chosen. The confidence that
decides what to mask is the softmax probability of the chosen code. The
exporter starts every confidence at 1/5; any equal start gives the same
result, because step 0 masks every real token. Where confidences tie, this
chapter masks the lower position, then head, first; PyTorch's sort does not
promise an order for ties. The checks run a predictor whose weights are all
zero: every logit ties, code 0 wins with confidence 1/4, and step 1 masks the
first 8 of the 12 tokens in order.

Decoded features become a chapter 147 clip. This layout is the synthetic
decoder's own, not SOMA's 365 features:

<!-- source: psionic/lib/motionbricks.coax clip -->
```jai
// The synthetic decoder's features per frame: heading turn (radians),
// ground velocity along the heading's x and z, root height, then a 6D
// rotation per joint. The root's heading and position accumulate frame by
// frame (the first frame's turn and step from the origin included); each
// 6D pair becomes a rotation by chapter 147's Gram-Schmidt.
MbClip :: struct {status:MbStatus;detail:PoseStatus;clip:PoseClip;}
mb_clip :: (s:MbShape,motion:MbMotion)->MbClip {
    out:MbClip;out.clip.convention=pose_gltf();out.clip.frames=motion.frames;
    joints:=(s.decoded-4)/6;heading:=0.0;x:=0.0;z:=0.0;
    if !mb_shape_ok(s) || motion.status!=MbStatus.ok || motion.frames<1 || motion.frames>MB_FRAMES {out.status=MbStatus.shape;return out;}
    for t:0..<motion.frames {
        f:=motion.value[t];
        heading+=f[0];x+=cos(heading)*f[1]+sin(heading)*f[2];z+=cos(heading)*f[2]-sin(heading)*f[1];
        out.clip.frame[t].root=[3]f64.[x,f[3],z];
        for j:0..<joints {
            six:[6]f64;for k:0..<6 {six[k]=f[4+6*j+k];}
            m:=pose_six_matrix(six);
            if m.status!=PoseStatus.ok {out.status=MbStatus.pose;out.detail=m.status;return out;}
            out.clip.frame[t].rotation[j]=pose_matrix_quat(m.value);
        }
    }
    return out;
}
```

The synthetic weights are exact binary fractions, and the decoder's last
bias is set so that small weights give a standing pose:

<!-- source: psionic/lib/motionbricks.coax synthetic -->
```jai
// Weight k is ((37 k + 11 seed) mod 61 - 30) / 64 times scale, so every
// value is exact in binary64. The decoder's last convolution's biases are
// then set to the identity's 6D form, (1, 0, 0, 0, 1, 0) per joint, and a
// height of 0.9 m; with a small scale the motion stays near standing.
mb_synthetic :: (w:[]f64,count:int,seed:int,scale:f64) {
    for k:0..<count {w[k]=f64((37*k+11*seed)%61-30)/64*scale;}
}
mb_synthetic_decoder :: (s:MbShape,w:[]f64,seed:int,scale:f64) {
    p:=mb_decoder_plan(s);mb_synthetic(w,p.count,seed,scale);
    bias:=p.out+mb_conv_size(s.decoded,s.width,3)-s.decoded;
    w[bias+3]=0.9;
    for j:0..<(s.decoded-4)/6 {w[bias+4+6*j]=1;w[bias+4+6*j+4]=1;}
}
```

## Run the example

The [example](../examples/motionbricks.coax) checks the package, runs the
real tokenizer, then generates 8 tokens with a synthetic predictor over 4
steps and decodes them with a synthetic decoder whose weights are 1/16 of
the usual size.

<!-- source: psionic/examples/motionbricks.coax inventory -->
```jai
    soma:=mb_soma();buf:[MB_TEXT_MAX]u8;
    n:=mb_vqvae_lines(soma,buf[..]);r:=mb_inventory(cast(string)buf[0..<n],MB_VQVAE_MANIFEST);
    print("pose VQ-VAE: % of % expected tensors, % F32 values, %\n",r.lines,r.expected,r.parameters,r.status);
    n=mb_pose_lines(soma,buf[..]);r=mb_inventory(cast(string)buf[0..<n],MB_POSE_MANIFEST);
    print("pose predictor: % of % expected tensors, % F32 values, %\n",r.lines,r.expected,r.parameters,r.status);
    r=mb_inventory("",MB_ROOT_MANIFEST);
    print("root predictor: % tensors, % F32 values (layout not derived)\n",r.lines,r.parameters);
```

<!-- source: psionic/examples/motionbricks.coax tokens -->
```jai
    book:=mb_soma_codebook();code:MbCode;code.code=[5]int.[1,2,3,4,5];
    packed:=mb_pack(book,code);back:=mb_unpack(book,packed);
    print("codes 1 2 3 4 5 pack to %, unpack to % % % % %\n",packed,back.code[0],back.code[1],back.code[2],back.code[3],back.code[4]);
    latent:=mb_dequantize(book,back);again:=mb_quantize(book,latent.value);
    print("latent[0] (millionths) %, re-encoded % % % % %, distances % % % % %\n",mbx_micro(latent.value[0]),
        again.code[0],again.code[1],again.code[2],again.code[3],again.code[4],
        again.squared[0],again.squared[1],again.squared[2],again.squared[3],again.squared[4]);
    for h:0..<5 {p:=mb_separation(book,h);print("head %: closest codes % and %, distance (millionths) %\n",h,p.first,p.second,mbx_micro(sqrt(p.squared)));}
```

<!-- source: psionic/examples/motionbricks.coax generate -->
```jai
    s:=mb_small();
    print("mask counts for 6 tokens, 5 heads, 4 steps: % % % %\n",mb_mask_count(0,4,6,5),mb_mask_count(1,4,6,5),mb_mask_count(2,4,6,5),mb_mask_count(3,4,6,5));
    predictor:[MB_WEIGHTS_MAX]f64;mb_synthetic(predictor[..],mb_predictor_plan(s).count,1,1);
    walk:=mb_walk(s,8,0);
    g:=mb_generate(s,predictor[..],8,walk,4);
    print("generation: %, masked % % % %, changed % % % %\n",g.status,g.masked[0],g.masked[1],g.masked[2],g.masked[3],g.changed[0],g.changed[1],g.changed[2],g.changed[3]);
    mbx_tokens("tokens: ",g.tokens,s.heads);
```

<!-- source: psionic/examples/motionbricks.coax decode -->
```jai
    decoder:[MB_WEIGHTS_MAX]f64;mb_synthetic_decoder(s,decoder[..],3,0.0625);
    m:=mb_decode(s,decoder[..],mb_small_codebook(s),g.tokens,walk);
    k:=mb_clip(s,m);
    print("decoded: % frames, %, clip %\n",m.frames,m.status,k.status);
    print("last root (mm): % % %\n",mbx_milli(k.clip.frame[31].root[0]),mbx_milli(k.clip.frame[31].root[1]),mbx_milli(k.clip.frame[31].root[2]));
    j:=mt_continuity(k.clip,mb_chain(2),10*POSE_PI/180,0.1);
    print("continuity: %, worst turn (millidegrees) % at frame %, worst step (mm) % at frame %\n",j.status,mbx_milli(j.angle*180/POSE_PI),j.turn_frame,mbx_milli(j.step),j.step_frame);
```

```text
pose VQ-VAE: 94 of 94 expected tensors, 23357918 F32 values, ok
pose predictor: 210 of 210 expected tensors, 136520976 F32 values, ok
root predictor: 174 tensors, 40418385 F32 values (layout not derived)
codes 1 2 3 4 5 pack to 344865, unpack to 1 2 3 4 5
latent[0] (millionths) -5122517, re-encoded 1 2 3 4 5, distances 0.0 0.0 0.0 0.0 0.0
head 0: closest codes 11 and 12, distance (millionths) 5175329
head 1: closest codes 7 and 12, distance (millionths) 4906655
head 2: closest codes 3 and 4, distance (millionths) 5210055
head 3: closest codes 0 and 14, distance (millionths) 5521768
head 4: closest codes 8 and 11, distance (millionths) 5287972
mask counts for 6 tokens, 5 heads, 4 steps: 30 27 21 11
generation: ok, masked 16 14 11 6, changed 16 0 0 0
tokens: 12 12 12 12 12 32 12 12
decoded: 32 frames, ok, clip ok
last root (mm): -233 899 -601
continuity: ok, worst turn (millidegrees) 63 at frame 31, worst step (mm) 21 at frame 31
```

For 8 tokens of 2 heads the counts are 16, 16 × 0.92388 = 14.8, 16 × 0.70711
= 11.3 and 16 × 0.38268 = 6.1, so 16, 14, 11 and 6. The synthetic predictor
is not trained, so its tokens are almost all the same; they show the
mechanics, not a motion. Once chosen, no token changed in later steps. The
decoded clip's worst joint turn between frames is 0.063 degrees and its worst
root step 21 mm, inside the 10 degree and 100 mm bounds.

## Refusals

The [refusal example](../examples/motionbricks-refuse.coax) breaks one
input at a time.

<!-- source: psionic/examples/motionbricks-refuse.coax refuse -->
```jai
main :: () {
    wrong:=mb_soma();wrong.heads=4;buf:[MB_TEXT_MAX]u8;
    n:=mb_vqvae_lines(wrong,buf[..]);r:=mb_inventory(cast(string)buf[0..<n],MB_VQVAE_MANIFEST);
    print("4 heads: %, first missing line %\n",r.status,r.bad);
    s:=mb_small();w:[MB_WEIGHTS_MAX]f64;mb_synthetic(w[..],mb_predictor_plan(s).count,1,1);
    walk:=mb_walk(s,8,0);q:MbTokens;q.count=8;q.code[3][1]=5;
    print("code 5 with 4 codes and the mask: %\n",mb_predict(s,w[..],q,walk).status);
    print("5 tokens: %\n",mb_generate(s,w[..],5,walk,4).status);
    g:=mb_generate(s,w[..],8,walk,4);
    loud:[MB_WEIGHTS_MAX]f64;mb_synthetic_decoder(s,loud[..],3,1);
    k:=mb_clip(s,mb_decode(s,loud[..],mb_small_codebook(s),g.tokens,walk));
    j:=mt_continuity(k.clip,mb_chain(2),10*POSE_PI/180,0.1);
    print("decoder weights 16 times larger: %, first at frame % joint %\n",j.status,j.frame,j.joint);
}
```

```text
4 heads: checkpoint, first missing line 38
code 5 with 4 codes and the mask: token
5 tokens: length
decoder weights 16 times larger: jump, first at frame 1 joint 0
```

With 4 heads the latent is 128 values, so the first line that differs is
line 38, the encoder's last convolution (512 → 128 instead of 160): the
earlier 38 lines do not depend on the head count. Code 5 is past the mask
(4 codes, mask 4). Five tokens are fewer than the predictor's six. With
decoder weights 16 times larger, the first frame already turns the first
joint by more than 10 degrees. The program exits with status 0 after
printing each refusal.

## Checks

[motionbricks.coax](../lib/motionbricks.coax) holds 19 checks of its own,
which run in debug and release with the checks of chapters 147, 148 and 167.
Tolerances were fixed before the comparisons ran: a relative 10⁻¹² for the
decoder and generation confidences and 10⁻¹¹ for logits. The independent
ones are:

- **The checkpoints.** The two derived inventories match the pinned
  manifests line for line, with the parameter sums above; six wrong
  configurations each name the expected first bad line; a line given twice
  and a manifest of more than 512 lines are refused. Every expected line is
  distinct.
- **The real codebook.** Four values checked against their F32 numbers; the
  closest pair in each head and its squared distance equal those of a
  separate Python reading of the same bits; every code of every head
  re-encodes to itself at distance 0; 1,000 packed tokens round-trip through
  codes and latents; 200 nudged latents keep their codes.
- **The decoder.** 24 inputs, half with 3 values a head, compared with a reference that keeps tensors
  channel-first, loops taps outside channels and writes the conditions by
  frame; the all-zero case by hand.
- **The predictor.** 12 inputs, some with 3 values a head, compared with a reference that embeds
  tokens as one-hot products, divides softmax without subtracting the
  largest score and computes LayerNorm as E[x²] − E[x]².
- **Generation.** 12 runs compared with a reference that orders confidences
  by insertion sort: tokens, masked and changed counts and confidences.
- **By hand.** Packing, the schedule's counts, the plan sizes and every
  offset, the position code, ReLU on −0, the clip's heading and steps, a
  softmax over scores in the millions without NaN, and refusals at and past
  every bound.

`coax mutate --root-only` on the unit leaves zero survivors.

## What this chapter does not do

- **The root predictor** does not run. Its 174 tensors are counted, not
  derived, and duration and root trajectory prediction are not built.
- **The real weights** do not run through the decoder or the predictor. The
  tokenizer's codebook is the one real tensor this chapter executes.
- **SOMA's features** (268 in, 365 out) and the skeleton reconstruction to
  root translations and joint rotations are not implemented; the clip here
  uses a small layout of its own.
- **No reference outputs** of the original PyTorch code are recorded, so
  full fixed-input trajectories are not compared. Producing them means
  running NVIDIA's code, which this chapter did not do.
- Matching outputs at inference would not show that the original training
  could be reproduced: the package has no training pipeline, optimizer
  state or training data.

## Sources

- [Chapter plan](PLAN.md), Part XIV.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), MotionBricks entry and
  license notes.
- [Chapter 147](ch147-poses-have-conventions.md): the 6D form and clips.
- [Chapter 148](ch148-motion-tokens-become-poses.md): the continuity check
  and masked filling.
- T. Wang et al., *MotionBricks: Scalable Real-Time Motions with Modular
  Latent Generative Model and Smart Primitives*, 2026,
  [arXiv:2604.24833](https://arxiv.org/abs/2604.24833).
