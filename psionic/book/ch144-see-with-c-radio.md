# 144. See with C-RADIO

> **Status: partly implemented.** The pinned C-RADIOv3-B encoder runs on the
> CPU: its 175-tensor inventory checked against the config, image admission
> and the input conditioner, patch rows, the patch embedding with the CPE
> position table cut to the image's grid, the cls and register tokens, the
> layer-scaled transformer block, and the summary and spatial features. On
> the original weights, the full 12-block encoder for a 64 by 48 image
> matches an independent oracle within a frozen tolerance. Visual-asset
> retrieval, the small trained head over frozen features, resizing images to
> an admitted size, and a comparison with a capture of the upstream
> runtime's own outputs are not built yet.

C-RADIOv3-B is an NVIDIA image encoder that produces general visual
features: one summary vector for the whole image and one feature vector per
16 by 16 patch. This chapter pins the checkpoint, reads its tensors, and
computes those features in Coax, checked piece by piece against an
independent oracle and then end to end on the original weights.

It builds four things, in [`radio.coax`](../lib/radio.coax), on chapter
143's helpers (row product, layer norm, erf GELU, attention scores) and
chapter 142's preprocessing:
- the **inventory**: the 175 tensors the config implies, their names, types
  and shapes, checked against the file's header;
- **admission and patches**: which images the encoder takes as they are, the
  input conditioner, and the patch rows;
- the **encoder**: the patch embedding with its interpolated positions, the
  prefix tokens, twelve blocks, and the split into summary and spatial
  features;
- an **original-weight check**, in `psionic/tests`, that runs the encoder on
  the real weights against an independent oracle.

```shell
coax/bin/coax run psionic/examples/radio.coax
coax/bin/coax run psionic/examples/radio-refuse.coax
psionic/check.sh --only radio
psionic/tests/check_radio_slices.sh ~/models/c-radiov3-b-44653a04
```

## The pinned checkpoint

[`psionic/fixtures/radio/MANIFEST`](../fixtures/radio/MANIFEST) pins the
files: `nvidia/C-RADIOv3-B` at revision
`44653a0482cf460bb4f12595fc3cc3dfecc403d1`, `model.safetensors` (393,038,784
bytes, SHA-256 `27881c65…190b9b`), `config.json` and
`preprocessor_config.json`. The repository also ships a pickle of the same
model (`c-radio_v3-b_half.pth.tar`); it is not fetched or unpickled.

The weights use the NVIDIA Open Model License. The repository's Python
modules describe the model; they were read as text and never run. The RADIO
source license limits that code and its derivatives to noncommercial research
and evaluation, so the Coax encoder and the Python oracle are written
separately from it, from the shapes and operations it describes.

The config names the model and its options; the model's own sizes come from
the model definition it names:

| Field | Value | Meaning |
| --- | --- | --- |
| `args.model` | `vit_base_patch16_v2_224` | 16-pixel patches, width 768 in 12 heads of 64, MLP 3072, 12 blocks, layer scale |
| `args.cpe_max_size` | 2048 | the position table covers 2048 / 16 = 128 by 128 patches |
| `args.cls_token_per_teacher` | true, 4 teachers | 4 cls tokens |
| `args.cpe_num_registers` | 4 | 4 register tokens after them |
| `args.teachers[*].use_summary` | true, true, true, false | `summary_idxs` = [0, 1, 2]: the summary is cls tokens 0 to 2 |
| `args.model_norm` | false | no final layer norm |
| `vitdet_window_size` | null | no windowed attention |
| `feature_normalizer_config` | null | features are not normalized afterwards |

`preprocessor_config.json` says `do_resize` false, `do_rescale` true (by
1/255) and `do_normalize` false; the model's own input conditioner then
subtracts OpenAI CLIP's mean and divides by its spread, which the checkpoint
stores as `input_conditioner.norm_mean` and `norm_std`.

<!-- source: psionic/lib/radio.coax config -->
```jai
// C-RADIOv3-B: vit_base_patch16_v2_224 (16-pixel patches, width 768 in 12
// heads, MLP 3072, 12 blocks), CPE up to 2048 pixels (a 128 by 128 table),
// one cls token per teacher (4) and 4 registers. summary_idxs is [0, 1, 2]:
// the teachers with use_summary. The conditioner's mean and spread are the
// stored f32 values of OpenAI CLIP's.
rd_c_radio_v3_b :: ()->RdConfig {
    return RdConfig.{patch=16,width=768,heads=12,inner=3072,depth=12,side=128,cls=4,registers=4,summary=3,epsilon=0.000001,
        mean=[3]f64.[0.48145467042922974,0.45782750844955444,0.40821072459220886],std=[3]f64.[0.2686295509338379,0.2613025903701782,0.27577710151672363]};
}
```

The tensors follow. Seven are shared: `summary_idxs` (I64, [3]), the
conditioner's mean and spread ([3, 1, 1] each), the patch embedding
`embedder.weight` [768, 768] (3 · 16 · 16 = 768 inputs, no bias), the
position table `pos_embed` [1, 16384, 768], the prefix tokens
`cls_token.token` [8, 768], and `reg_token` [1, 4, 768]. Each block has
fourteen: two layer norms with biases, `attn.qkv` [2304, 768], `attn.proj`
[768, 768], `mlp.fc1` [3072, 768] and `mlp.fc2` [768, 3072] with biases, and
the two layer scales `ls1.grandma` and `ls2.grandma` [768]. That is 7 + 14 ·
12 = 175 tensors, every one F32 except `summary_idxs`.

Counting values by hand:

| Part | Values |
| --- | --- |
| summary indices, mean, spread | 3 + 3 + 3 = 9 |
| patch embedding | 768 · 768 = 589,824 |
| position table | 16384 · 768 = 12,582,912 |
| cls and register tokens | 8 · 768 = 6,144 |
| `reg_token` | 4 · 768 = 3,072 |
| one block | 1,536 + 1,771,776 + 590,592 + 768 + 1,536 + 2,362,368 + 2,360,064 + 768 = 7,089,408 |
| 12 blocks | 85,072,896 |
| total | 98,254,857 |

The file agrees: 8 bytes of length, a 19,336-byte header, then 98,254,854
four-byte values and 3 eight-byte indices, 393,038,784 bytes.

`reg_token` is timm's own register table. The CPE forward replaces timm's
token handling with the cls-token table's four registers, so `reg_token` is
checked as inventory and never read.

<!-- source: psionic/lib/radio.coax inventory -->
```jai
// The tensors the checkpoint must hold, in a fixed order: seven shared ones,
// then fourteen per block. Each entry writes its name and returns its shape
// and type: every tensor is F32 except summary_idxs, I64. reg_token is
// timm's own register table; the CPE forward reads the cls_token table's
// four registers instead, so it is checked but never used.
RD_SHARED :: 7;
RD_PER_BLOCK :: 14;
RdDtype :: enum u8 {f32;i64;}
RdTensor :: struct {name:string;dtype:RdDtype;shape:P5Shape;elements:int;}
RdNeed :: struct {count:int;dtype:RdDtype;shape:P5Shape;}
rd_s3 :: (a:int,b:int,c:int)->P5Shape {s:=sv_s2(a,b);s.rank=3;s.dims[2]=c;return s;}
rd_tensors :: (c:RdConfig)->int {return RD_SHARED+RD_PER_BLOCK*c.depth;}
rd_need :: (c:RdConfig,i:int,name:[]u8)->RdNeed {
    d:=c.width;
    n:=json_raw(name,0,"radio_model.");
    if i<RD_SHARED {
        shared:=[7]string.["summary_idxs","input_conditioner.norm_mean","input_conditioner.norm_std","model.patch_generator.embedder.weight",
            "model.patch_generator.pos_embed","model.patch_generator.cls_token.token","model.reg_token"];
        shapes:=[7]P5Shape.[sv_s1(c.summary),rd_s3(3,1,1),rd_s3(3,1,1),sv_s2(d,3*c.patch*c.patch),rd_s3(1,c.side*c.side,d),
            sv_s2(c.cls+c.registers,d),rd_s3(1,c.registers,d)];
        need:=RdNeed.{count=json_raw(name,n,shared[i]),shape=shapes[i]};
        if i==0 {need.dtype=RdDtype.i64;}
        return need;
    }
    b:=(i-RD_SHARED)/RD_PER_BLOCK;j:=i-RD_SHARED-b*RD_PER_BLOCK;
    suffixes:=[14]string.["norm1.weight","norm1.bias","attn.qkv.weight","attn.qkv.bias","attn.proj.weight","attn.proj.bias","ls1.grandma",
        "norm2.weight","norm2.bias","mlp.fc1.weight","mlp.fc1.bias","mlp.fc2.weight","mlp.fc2.bias","ls2.grandma"];
    shapes:=[14]P5Shape.[sv_s1(d),sv_s1(d),sv_s2(3*d,d),sv_s1(3*d),sv_s2(d,d),sv_s1(d),sv_s1(d),
        sv_s1(d),sv_s1(d),sv_s2(c.inner,d),sv_s1(c.inner),sv_s2(d,c.inner),sv_s1(d),sv_s1(d)];
    n=json_raw(name,n,"model.blocks.");n=json_write_int(name,n,b);n=json_raw(name,n,".");
    return RdNeed.{count=json_raw(name,n,suffixes[j]),shape=shapes[j]};
}
// Every needed tensor present once, with its type and the config's shape,
// and nothing else in the file. Returns the element count.
rd_inventory :: (c:RdConfig,inventory:[]RdTensor)->RdResult {
    if rd_config_status(c)!=RdStatus.ok {return RdResult.{status=RdStatus.config};}
    total:=0;
    for i:0..<rd_tensors(c) {
        name:[128]u8;need:=rd_need(c,i,name[..]);key:=cast(string)name[0..<need.count];found:=-1;
        for t:0..<inventory.count {if inventory[t].name==key {if found>=0 {return RdResult.{status=RdStatus.extra,cause=i};}found=t;}}
        if found<0 {return RdResult.{status=RdStatus.missing,cause=i};}
        if inventory[found].dtype!=need.dtype {return RdResult.{status=RdStatus.dtype,cause=i};}
        if !pa_shape_equal(inventory[found].shape,need.shape) {return RdResult.{status=RdStatus.shape,cause=i};}
        total+=inventory[found].elements;
    }
    if inventory.count!=rd_tensors(c) {return RdResult.{status=RdStatus.extra,cause=inventory.count};}
    return RdResult.{written=total,cause=inventory.count};
}
```

Chapter 7's rank-5 header reader takes BF16 and F32 tensors only, so the
original-weight check scans this header with its own small reader in
`radio-slices-runtime.coax`: the same rules (each span inside the payload,
no overlaps, full coverage), with I64 added.

## Which images are admitted

The model accepts any size whose sides are multiples of 16, up to its
2048-pixel maximum; the pinned processor does not resize. This chapter
admits an image as it is: 8-bit RGB, both sides multiples of 16, at most 512
pixels a side (chapter 142's bound), and at most the position table's 128
patches a side. Anything else is refused with a typed reason before any value
is computed.

<!-- source: psionic/lib/radio.coax admit -->
```jai
rd_config_status :: (c:RdConfig)->RdStatus {
    sizes:=[7]int.[c.patch,c.width,c.heads,c.inner,c.depth,c.side,c.summary];
    for s:sizes {if s<1 || s>SV_EXTENT_MAX {return RdStatus.config;}}
    if c.summary>c.cls || c.registers<0 || c.cls+c.registers>SV_EXTENT_MAX || c.width%c.heads!=0 || !(c.epsilon>0) {return RdStatus.config;}
    if px_profile_status(rd_profile(c,PxImage.{width=PX_SIDE_MAX,height=PX_SIDE_MAX}))!=PxStatus.ok {return RdStatus.config;}
    return RdStatus.ok;
}
// An image is admitted as is: 8-bit RGB, both sides a multiple of the patch
// (the model's min_resolution_step without windowed attention), at most
// chapter 142's 512-pixel side and at most the position table's grid.
// The pinned processor does not resize, so neither does this.
rd_admit :: (c:RdConfig,image:PxImage)->RdGrid {
    if rd_config_status(c)!=RdStatus.ok {return RdGrid.{status=RdStatus.config};}
    s:=px_image_status(image);
    if s==PxStatus.shape {return RdGrid.{status=RdStatus.shape};}
    if s==PxStatus.limit {return RdGrid.{status=RdStatus.large};}
    if s==PxStatus.channels {return RdGrid.{status=RdStatus.channels};}
    if s==PxStatus.depth {return RdGrid.{status=RdStatus.depth};}
    if image.width%c.patch!=0 || image.height%c.patch!=0 {return RdGrid.{status=RdStatus.multiple};}
    g:=RdGrid.{rows=image.height/c.patch,cols=image.width/c.patch,features=3*c.patch*c.patch};
    if g.rows>c.side || g.cols>c.side {return RdGrid.{status=RdStatus.large};}
    g.patches=g.rows*g.cols;g.tokens=c.cls+c.registers+g.patches;
    return g;
}
```

By hand: a 64 by 48 image (width by height) is 48 / 16 = 3 rows by 64 / 16 =
4 columns, 12 patches; with 4 cls and 4 register tokens, the encoder runs on
20 tokens. A 512 by 512 image is 32 by 32 = 1024 patches and 1032 tokens.

## Bytes to patch rows

The conditioner turns byte b of channel c into (b / 255 − mean[c]) /
spread[c]. For red, mean 0.48145467 and spread 0.26862955: byte 0 becomes
−0.48145467 / 0.26862955 = −1.79226, and byte 255 becomes 0.51854533 /
0.26862955 = 1.93034. Chapter 142's preprocessing does this with one
rounding to f32 per stage.

Patches go left to right, then top to bottom. Inside a patch row the 768
values are (channel, row, column): value k is channel k / 256, row (k mod
256) / 16, column k mod 16. Value 300 is channel 1 (green), row 2, column 12.
That is the layout of `embedder.weight`'s 768 inputs.

<!-- source: psionic/lib/radio.coax pixels -->
```jai
// The input conditioner: bytes / 255, then (x - mean) / spread per channel;
// chapter 142's preprocessing with no resize.
rd_profile :: (c:RdConfig,image:PxImage)->PxProfile {
    return PxProfile.{scale=1.0/255,mean=c.mean,std=c.std,height=image.height,width=image.width,filter=PxFilter.nearest};
}
// Patch rows left to right, top to bottom; each holds (channel, row, column)
// values, the layout of the patch embedding's weight [768, 3 * 16 * 16].
rd_pixels :: (c:RdConfig,image:PxImage,bytes:[]u8,work:[]f32,out:[]f32,overlap:bool)->RdResult {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return RdResult.{status=g.status};}
    stage:=3*image.width*image.height;
    if bytes.count!=stage {return RdResult.{status=RdStatus.shape};}
    if work.count<2*stage || out.count<g.patches*g.features {return RdResult.{status=RdStatus.capacity};}
    if overlap {return RdResult.{status=RdStatus.alias};}
    #assert px_preprocess(image,bytes,rd_profile(c,image),work[0..<stage],work[stage..<2*stage],false).written==stage;
    pp:=c.patch*c.patch;
    for r:0..<g.patches {for k:0..<g.features {
        y:=(r/g.cols)*c.patch+(k%pp)/c.patch;x:=(r%g.cols)*c.patch+k%c.patch;
        out[r*g.features+k]=work[stage+((k/pp)*image.height+y)*image.width+x];
    }}
    return RdResult.{written=g.patches*g.features};
}
```

## The patch embedding and its positions

The embedding is one row product per patch, 768 inputs to 768 outputs,
without bias. Each patch then adds its learned position, and the prefix
tokens go first without one.

The position table is 128 by 128 patches, built for 2048-pixel images.
For a smaller grid the model resizes the whole table bilinearly, with
corners aligned, to a square whose side is the grid's longer side, then keeps
the grid's rows and columns from the top-left corner. For the 3 by 4 grid the
square is 4 by 4: grid row y reads table row y · 127 / 3, so row 1 reads
42.333, which is 2/3 of table row 42 plus 1/3 of row 43; column 3 reads
table column 127. The transposed 4 by 3 grid uses the same square: its row 3
reads table row 127, and its three columns read table columns 0, 42.333 and
84.667.
Positions therefore keep their scale: a patch's position depends on its
distance from the top-left corner relative to the longer side.

<!-- source: psionic/lib/radio.coax embed -->
```jai
// The patch embedding is a row product without bias (768 inputs, 768
// outputs). Positions come from the 128 by 128 CPE table: resized
// bilinearly, corners aligned, to a square whose side is the grid's longer
// side, then cut to the grid from the top-left corner. The cls and register
// tokens go first, without positions.
rd_embed_work :: (c:RdConfig,g:RdGrid)->int {return (g.patches+1)*c.width;}
rd_embed :: (c:RdConfig,image:PxImage,patches:[]f32,w:[]f32,table:[]f32,tokens:[]f32,out:[]f32,work:[]f32,overlap:bool)->RdResult {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return RdResult.{status=g.status};}
    n:=g.patches;d:=c.width;s:=c.cls+c.registers;
    if patches.count!=n*g.features || w.count!=d*g.features || table.count!=c.side*c.side*d || tokens.count!=s*d {return RdResult.{status=RdStatus.shape};}
    if out.count<g.tokens*d || work.count<rd_embed_work(c,g) {return RdResult.{status=RdStatus.capacity};}
    if overlap {return RdResult.{status=RdStatus.alias};}
    if !px_finite(patches) || !px_finite(w) || !px_finite(table) || !px_finite(tokens) {return RdResult.{status=RdStatus.number};}
    zero:=work[0..<d];y:=work[d..<work.count];for k:0..<d {zero[k]=0;}
    sv_linear(patches,n,g.features,w,zero,d,y);
    m:=(g.rows+g.cols+cast(int)abs(f64(g.rows-g.cols)))/2;
    bad:=false;
    for r:0..<n {
        a:=sv_point(r/g.cols,m,c.side);b:=sv_point(r%g.cols,m,c.side);low:=a.low*c.side;high:=a.high*c.side;
        for k:0..<d {
            top:=(1-b.fraction)*f64(table[(low+b.low)*d+k])+b.fraction*f64(table[(low+b.high)*d+k]);
            bottom:=(1-b.fraction)*f64(table[(high+b.low)*d+k])+b.fraction*f64(table[(high+b.high)*d+k]);
            y[r*d+k]=sv_f32(f64(y[r*d+k])+f64(sv_f32((1-a.fraction)*top+a.fraction*bottom)));bad=bad || !rd_finite(y[r*d+k]);
        }
    }
    if bad {return RdResult.{status=RdStatus.number};}
    for i:0..<s*d {out[i]=tokens[i];}
    for i:0..<n*d {out[s*d+i]=y[i];}
    return RdResult.{written=g.tokens*d};
}
```

## The block and the outputs

A block is `x + ls1 · proj(attention(norm1(x)))`, then `+ ls2 ·
fc2(gelu(fc1(norm2(x))))`: timm's pre-norm block with layer scale. The
layer norms use population variance and ε = 10^−6. GELU is the exact one,
with erf, and chapter 143's `sv_erf` (Abramowitz and Stegun 7.1.26, absolute
error below 1.5 · 10^−7) stands in for it. Attention is plain softmax
attention over all 20 tokens, 12 heads of 64, scaled by 1 / 8; there is no
rotary position.

<!-- source: psionic/lib/radio.coax block -->
```jai
// One block: x + ls1 * proj(attention(norm1(x))), then
// + ls2 * fc2(gelu(fc1(norm2(x)))). Attention is plain softmax attention
// over every token in both directions, 12 heads of 64; there is no rotary
// position: the positions were added once, at the embedding.
// The layer-scaled branch rounds to f32 before the residual sum; a value
// past f32 stays as it is, so the sum's rounding refuses it.
rd_finite :: (v:f32)->bool {return psionic_tensor_finite_bits(f32_bits(v));}
rd_round :: (v:f64)->f64 {if psionic_ops_fits_f32(v) {return f64(f32(v));}return v;}
rd_block_work :: (c:RdConfig,n:int)->int {return n*(6*c.width+c.inner);}
rd_block_status :: (c:RdConfig,w:RdBlock)->RdStatus {
    d:=c.width;i:=c.inner;
    parts:=[14][]f32.[w.norm1,w.norm1_bias,w.qkv,w.qkv_bias,w.proj,w.proj_bias,w.ls1,w.norm2,w.norm2_bias,w.fc1,w.fc1_bias,w.fc2,w.fc2_bias,w.ls2];
    want:=[14]int.[d,d,3*d*d,3*d,d*d,d,d,d,d,i*d,i,d*i,d,d];
    for k:0..<14 {if parts[k].count!=want[k] {return RdStatus.shape;}}
    for p:parts {if !px_finite(p) {return RdStatus.number;}}
    return RdStatus.ok;
}
rd_block :: (c:RdConfig,image:PxImage,x:[]f32,w:RdBlock,out:[]f32,work:[]f32,overlap:bool)->RdResult {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return RdResult.{status=g.status};}
    n:=g.tokens;d:=c.width;
    if x.count!=n*d {return RdResult.{status=RdStatus.shape};}
    s:=rd_block_status(c,w);if s!=RdStatus.ok {return RdResult.{status=s};}
    if out.count!=n*d || work.count<rd_block_work(c,n) {return RdResult.{status=RdStatus.capacity};}
    if overlap {return RdResult.{status=RdStatus.alias};}
    if !px_finite(x) {return RdResult.{status=RdStatus.number};}
    // Each region runs to the end of work; only its start matters.
    mid:=work;h:=work[n*d..<work.count];qkv:=work[2*n*d..<work.count];att:=work[5*n*d..<work.count];f:=work[6*n*d..<work.count];
    sv_norm(x,n,d,w.norm1,w.norm1_bias,c.epsilon,h);
    sv_linear(h,n,d,w.qkv,w.qkv_bias,3*d,qkv);
    rd_attend(qkv,n,d,c.heads,att);
    sv_linear(att,n,d,w.proj,w.proj_bias,d,mid);
    for i:0..<n*d {mid[i]=sv_f32(f64(x[i])+rd_round(f64(mid[i])*f64(w.ls1[i%d])));}
    sv_norm(mid,n,d,w.norm2,w.norm2_bias,c.epsilon,h);
    sv_linear(h,n,d,w.fc1,w.fc1_bias,c.inner,f);
    for i:0..<n*c.inner {f[i]=sv_f32(sv_gelu(f64(f[i])));}
    sv_linear(f,n,c.inner,w.fc2,w.fc2_bias,d,att);
    bad:=false;
    for i:0..<n*d {att[i]=sv_f32(f64(mid[i])+rd_round(f64(att[i])*f64(w.ls2[i%d])));bad=bad || !rd_finite(att[i]);}
    if bad {return RdResult.{status=RdStatus.number};}
    for i:0..<n*d {out[i]=att[i];}
    return RdResult.{written=n*d};
}
// Softmax attention per head over all n tokens. The largest score is
// subtracted before exponentiating, so scores past 709 do not overflow.
rd_attend :: (qkv:[]f32,n:int,d:int,heads:int,att:[]f32) {
    hd:=d/heads;scale:=1/sqrt(f64(hd));
    for head:0..<heads {for r:0..<n {
        q:=r*3*d+head*hd;top:f64;
        for j:0..<n {v:=sv_score(qkv,q,j*3*d+d+head*hd,hd,scale);top=ifx j==0 then v else (top+v+abs(top-v))/2;}
        total:=0.0;for j:0..<n {total+=exp(sv_score(qkv,q,j*3*d+d+head*hd,hd,scale)-top);}
        for e:0..<hd {
            acc:=0.0;
            for j:0..<n {acc+=exp(sv_score(qkv,q,j*3*d+d+head*hd,hd,scale)-top)*f64(qkv[j*3*d+2*d+head*hd+e]);}
            att[r*d+head*hd+e]=sv_f32(acc/total);
        }
    }}
}
```

After the last block the outputs split. The summary is the first three cls
tokens joined end to end: 3 · 768 = 2304 values, whatever the image size.
The spatial features are the patch tokens after the 8 prefix tokens: one row
of 768 per grid cell, so a 64 by 48 image gives 12 rows and a 224 by 224
image 14 · 14 = 196 rows. The feature grid is always the image's size
divided by 16.

<!-- source: psionic/lib/radio.coax outputs -->
```jai
// The model has no final norm (model_norm is false). The summary is the
// first `summary` cls tokens joined end to end (3 * 768 = 2304 values); the
// spatial features are every patch token, after the cls and register
// tokens, one row of 768 per grid cell, rows then columns.
rd_split :: (c:RdConfig,image:PxImage,y:[]f32,summary:[]f32,features:[]f32)->RdResult {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return RdResult.{status=g.status};}
    d:=c.width;s:=c.cls+c.registers;
    if y.count!=g.tokens*d {return RdResult.{status=RdStatus.shape};}
    if summary.count!=c.summary*d || features.count!=g.patches*d {return RdResult.{status=RdStatus.capacity};}
    for i:0..<c.summary*d {summary[i]=y[i];}
    for i:0..<g.patches*d {features[i]=y[s*d+i];}
    return RdResult.{written=(c.summary+g.patches)*d};
}
```

<!-- source: psionic/lib/radio.coax encode -->
```jai
// The whole encoder: bytes to patch rows, the embedding, every block, then
// the summary and the spatial features.
RdWeights :: struct {patch:[]f32;table:[]f32;tokens:[]f32;blocks:[]RdBlock;}
rd_encode_work :: (c:RdConfig,image:PxImage)->int {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return 0;}
    return rd_scratch(c,image,g)+g.patches*g.features+2*g.tokens*c.width;
}
// The scratch serves the pixels (two channel-major images) and then each
// block: the larger of the two.
rd_scratch :: (c:RdConfig,image:PxImage,g:RdGrid)->int {
    p:=6*image.width*image.height;b:=rd_block_work(c,g.tokens);return (p+b+cast(int)abs(f64(p-b)))/2;
}
rd_encode :: (c:RdConfig,image:PxImage,bytes:[]u8,w:RdWeights,summary:[]f32,features:[]f32,work:[]f32,overlap:bool)->RdResult {
    g:=rd_admit(c,image);if g.status!=RdStatus.ok {return RdResult.{status=g.status};}
    if w.blocks.count!=c.depth {return RdResult.{status=RdStatus.shape};}
    if work.count<rd_encode_work(c,image) {return RdResult.{status=RdStatus.capacity};}
    sc:=rd_scratch(c,image,g);np:=g.patches*g.features;nt:=g.tokens*c.width;
    scratch:=work[0..<sc];patches:=work[sc..<sc+np];a:=work[sc+np..<sc+np+nt];b:=work[sc+np+nt..<sc+np+2*nt];
    r:=rd_pixels(c,image,bytes,scratch,patches,overlap);if r.status!=RdStatus.ok {return r;}
    r=rd_embed(c,image,patches,w.patch,w.table,w.tokens,a,scratch,false);if r.status!=RdStatus.ok {return r;}
    for k:0..<c.depth {
        r=rd_block(c,image,a,w.blocks[k],b,scratch,false);if r.status!=RdStatus.ok {return RdResult.{status=r.status,cause=k};}
        for i:0..<nt {a[i]=b[i];}
    }
    r=rd_split(c,image,a,summary,features);
    return r;
}
```

## The program

The [example](../examples/radio.coax) prints the hand-derived numbers above,
then runs a tiny encoder end to end: 2-pixel patches, width 8 in 2 heads, 2
blocks, a 4 by 4 position table, 3 cls tokens of which 2 form the summary,
and 1 register.

<!-- source: psionic/examples/radio.coax example -->
```jai
main :: () {
    e:=rd_c_radio_v3_b();name:[128]u8;total:=0;
    for i:0..<rd_tensors(e) {total+=rd_size(rd_need(e,i,name[..]).shape);}
    print("C-RADIOv3-B: % tensors, % values; table % by %, % cls and % register tokens\n",rd_tensors(e),total,e.side,e.side,e.cls,e.registers);
    sizes:=[5]PxImage.[PxImage.{width=64,height=48},PxImage.{width=48,height=64},PxImage.{width=16,height=16},PxImage.{width=224,height=224},PxImage.{width=512,height=512}];
    for image:sizes {
        g:=rd_admit(e,image);
        print("%x%: % by % patches, % tokens; summary % values, features % by %\n",image.width,image.height,g.rows,g.cols,g.tokens,e.summary*e.width,g.patches,e.width);
    }
    // Bytes 0 and 255 in the red channel, through the conditioner.
    print("red 0 and 255 become % and %\n",f32((0-e.mean[0])/e.std[0]),f32((f64(f32(255*(1.0/255)))-e.mean[0])/e.std[0]));
    // A 3 by 4 grid reads the table resized to 4 by 4.
    a:=sv_point(1,4,e.side);b:=sv_point(3,4,e.side);
    print("grid row 1 reads table rows % and % (fraction %); column 3 reads %\n",a.low,a.high,a.fraction,b.low);

    // The tiny encoder: a 6 by 4 image, 6 patches plus 4 prefix tokens, 2 blocks.
    t:=rd_tiny();image:=PxImage.{width=6,height=4};bytes:[72]u8;sv_bytes(bytes[..]);w:=rd_tiny_weights();
    blocks:=[2]RdBlock.[rd_tiny_block(w.blocks[0..<RD_TINY_BLOCK]),rd_tiny_block(w.blocks[RD_TINY_BLOCK..<2*RD_TINY_BLOCK])];
    weights:=RdWeights.{patch=w.patch[..],table=w.table[..],tokens=w.tokens[..],blocks=blocks[..]};
    summary:[16]f32;features:[48]f32;work:[772]f32;
    r:=rd_encode(t,image,bytes[..],weights,summary[..],features[..],work[..],false);
    os:=RD_ORACLE_WIDE_SUMMARY;of:=RD_ORACLE_WIDE_FEATURES;
    print("tiny encoder: %, % values, within the oracle's tolerance: %\n",r.status,r.written,rd_near(summary[..],os[..]) && rd_near(features[..],of[..]));
    print("  summary:");for i:0..<4 {print(" %",summary[i]);}print(" ...\n");
}
```

```text
C-RADIOv3-B: 175 tensors, 98254857 values; table 128 by 128, 4 cls and 4 register tokens
64x48: 3 by 4 patches, 20 tokens; summary 2304 values, features 12 by 768
48x64: 4 by 3 patches, 20 tokens; summary 2304 values, features 12 by 768
16x16: 1 by 1 patches, 9 tokens; summary 2304 values, features 1 by 768
224x224: 14 by 14 patches, 204 tokens; summary 2304 values, features 196 by 768
512x512: 32 by 32 patches, 1032 tokens; summary 2304 values, features 1024 by 768
red 0 and 255 become -1.7922626 and 1.9303361
grid row 1 reads table rows 42 and 43 (fraction 0.3333333333333357); column 3 reads 127
tiny encoder: ok, 64 values, within the oracle's tolerance: true
  summary: 0.13536607 0.16132584 -0.28667647 -0.23252365 ...
```

## Refusals

The [refusal program](../examples/radio-refuse.coax) feeds the path one
broken input at a time.

<!-- source: psionic/examples/radio-refuse.coax refuse -->
```jai
main :: () {
    e:=rd_c_radio_v3_b();
    images:=[6]PxImage.[PxImage.{width=60,height=48},PxImage.{width=528,height=48},PxImage.{width=64,height=48,channels=4},
        PxImage.{width=64,height=48,depth=16},PxImage.{width=0,height=48},PxImage.{width=64,height=48}];
    labels:=[6]string.["60x48, not a multiple of 16","528x48, over 512 pixels a side","64x48 with 4 channels",
        "64x48 with 16-bit channels","0x48","64x48 under a config with 5 heads of a 768 width"];
    bad:=e;bad.heads=5;
    for i:0..<6 {c:=ifx i==5 then bad else e;print("%: %\n",labels[i],rd_admit(c,images[i]).status);}
    // An inventory missing its last block's ls2.grandma, then one with a stray final norm.
    t:=rd_tiny();names:[36][128]u8;inv:[36]RdTensor;
    for i:0..<rd_tensors(t) {need:=rd_need(t,i,names[i][..]);inv[i]=RdTensor.{name=cast(string)names[i][0..<need.count],dtype=need.dtype,shape=need.shape,elements=rd_size(need.shape)};}
    r:=rd_inventory(t,inv[0..<34]);print("inventory without tensor %: %\n",r.cause,r.status);
    inv[35]=RdTensor.{name="radio_model.model.norm.weight",shape=sv_s1(8),elements=8};r=rd_inventory(t,inv[0..<36]);print("inventory with % tensors: %\n",r.cause,r.status);
    // One block where the config says two; summary room one value short.
    w:=rd_tiny_weights();bytes:[72]u8;sv_bytes(bytes[..]);one:=[1]RdBlock.[rd_tiny_block(w.blocks[0..<RD_TINY_BLOCK])];
    weights:=RdWeights.{patch=w.patch[..],table=w.table[..],tokens=w.tokens[..],blocks=one[..]};
    summary:[16]f32;features:[48]f32;work:[772]f32;image:=PxImage.{width=6,height=4};
    print("one block of two: %\n",rd_encode(t,image,bytes[..],weights,summary[..],features[..],work[..],false).status);
    two:=[2]RdBlock.[one[0],rd_tiny_block(w.blocks[RD_TINY_BLOCK..<2*RD_TINY_BLOCK])];weights.blocks=two[..];
    print("summary one value short: %\n",rd_encode(t,image,bytes[..],weights,summary[0..<15],features[..],work[..],false).status);
}
```

```text
60x48, not a multiple of 16: multiple
528x48, over 512 pixels a side: large
64x48 with 4 channels: channels
64x48 with 16-bit channels: depth
0x48: shape
64x48 under a config with 5 heads of a 768 width: config
inventory without tensor 34: missing
inventory with 36 tensors: extra
one block of two: shape
summary one value short: capacity
```

The program exits with status 0 after printing each refusal. Every encoder
procedure also refuses short or long weights, aliasing, too little work
space, non-finite inputs or weights, and values that overflow f32
(`number`), before writing its output.

## On the original weights

`psionic/tests/check_radio_slices.sh` compiles `radio-slices-runtime.coax`
to C with a small host (`radio_slices.c`) that maps the file. Coax scans the
header, checks the inventory, checks that `summary_idxs` is [0, 1, 2] and
that the stored mean and spread are the config's values, decodes the 171
tensors the encoder reads, and runs: the patches, embedding and block 0 of a
64 by 48 image, then the whole encoder for it (all 12 blocks) to its summary
and spatial features; the embedding of the 48 by 64 transpose; and the
patches of a 224 by 224 image. The images' bytes are the same fixed pattern
as the tiny checks'.

`radio_slices.py` reads the header and tensors from the file itself, derives
the expected inventory from `config.json` and the model definition, and
recomputes everything in binary64 Python with exact sums (`math.fsum`) and
`math.erf`, following the reference's operations (the `rearrange` of
patches, `F.interpolate` with corners aligned, then the top-left cut, the
prepended tokens, timm's block, `summary_idxs`) rather than this chapter's
loops. The tolerance, frozen before the first comparison, is |Coax −
oracle| ≤ 10^−4 · (1 + |oracle|) for every value. The recorded run
([`psionic/fixtures/radio/slices.txt`](../fixtures/radio/slices.txt)) took
74 seconds, 1.2 of them in Coax:

```text
  patches 64x48: 9216 values, largest |coax - oracle| / (1 + |oracle|) = 1.06e-07
  embedding 64x48: 15360 values, largest |coax - oracle| / (1 + |oracle|) = 4.50e-08
  block 0: 15360 values, largest |coax - oracle| / (1 + |oracle|) = 2.86e-07
  summary (12 blocks): 2304 values, largest |coax - oracle| / (1 + |oracle|) = 1.51e-05
  spatial features (12 blocks): 9216 values, largest |coax - oracle| / (1 + |oracle|) = 1.54e-05
  embedding 48x64: 15360 values, largest |coax - oracle| / (1 + |oracle|) = 7.60e-08
  patches 224x224: 150528 values, largest |coax - oracle| / (1 + |oracle|) = 1.06e-07
summary 2304 values, spatial features 12 by 768
within the frozen tolerance 0.0001
```

The difference grows from about 3 · 10^−7 after one block to 1.5 · 10^−5
after twelve: Coax rounds every stored value to f32 and uses an approximate
erf, while the oracle keeps binary64 throughout. Both compute from the same
F32 weights; neither is the upstream PyTorch runtime, so this is agreement
on the computation, not a capture of the upstream model's outputs.

Features alone do not qualify any downstream task. A retrieval index, a
classifier or a segmentation head built on them needs its own data, split and
evaluation.

## Supported profile

Profile psionic-radio-v1, bounded CPU: f32 storage, binary64 sums, one
rounding to f32 per stored value; erf by Abramowitz and Stegun 7.1.26;
images 8-bit RGB, sides multiples of 16 up to 512, no resampling; one image
at a time.

## How it is checked

- `#test` checks in `radio.coax` derive admission, the patch layout, the
  position points, names, types and shapes, inventory defects, work sizes and
  every refusal by hand. A three-token attention check makes scores of
  2263, past exp's range, so a softmax that did not subtract each head's
  largest score would overflow.
- Two tiny encoders, a 6 by 4 image and its 4 by 6 transpose, match
  `psionic/tests/radio_oracle.py`, an independent binary64 oracle; its values
  are in `psionic/fixtures/radio/oracle.coax`, within 10^−6 · (1 + |value|):
  patches, embedding, block 0, summary and features.
- `coax mutate` on `radio.coax` leaves no surviving mutant.
- The original-weight run above.

## What this does not do yet

- Visual-asset retrieval: embedding a fixed image collection, querying by
  image, and recall at fixed cutoffs against exhaustive search (chapter 54).
- A small Psionic-trained head over frozen features, evaluated on held-out
  images against a constant predictor, with split and seeds recorded
  (chapters 103 and 104).
- Resizing: images that are not already at an admitted size are refused.
- A capture of the upstream runtime's summary and features for fixed images:
  that would mean running the reference's PyTorch code, which this chapter
  does not do.
- Images over 512 pixels a side, and a GPU path.

## Dependencies

- [Chapter 7](ch007-reading-original-weights.md): tensor shapes and the
  safetensors rules the header scan follows.
- [Chapter 142](ch142-pixels-convolutions-and-patches.md): image admission
  and preprocessing.
- [Chapter 143](ch143-elephant-and-mammoth-see.md): the row product, layer
  norm, erf GELU, attention scores and position points.
- [Chapter 54](ch054-search-the-coax-book.md): exhaustive vector search, the
  retrieval baseline still to come.
- [Chapters 103](ch103-learning-with-arrays.md) and
  [104](ch104-gradients-as-a-program.md): training the head still to come.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [NVIDIA survey](../docs/targets/nvidia-breadth.md), C-RADIO entry and
  license notes.
- The pinned checkpoint's `config.json`, `preprocessor_config.json` and
  model card, and its Python modules read as text.
