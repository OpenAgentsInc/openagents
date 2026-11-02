# 143. Elephant and Mammoth see

> **Status: partly implemented.** The vision tower's inventory check, image
> admission, the processor's patch layout, the encoder's patch embedding with
> its interpolated positions, one transformer block with 2-D rotary
> positions, the patch merger and the multimodal request layout run on the
> CPU. On the original weights, the inventory and a slice (embedding, block
> 0 and the merger on a 2 by 4 patch grid) match an independent oracle.
> Resizing images to an admitted size, the full 27-block encoder against a
> reference, splicing the vision tokens into the text model with M-RoPE, and
> image-bearing decisions on Elephant and Mammoth are not built yet.

Elephant and Mammoth are Qwen3.5 models, and their pinned checkpoints carry
a vision tower. Chapters [14](ch014-execute-elephant.md) and
[15](ch015-execute-mammoth.md) admit its tensors as inventory only and
refuse any request with an image. This chapter builds the parts of the image
path that can be checked exactly: which tensors the tower must have, how an
image becomes patch rows, what the encoder computes from them, and where the
resulting tokens sit in a request.

It builds five things, in [`see.coax`](../lib/see.coax):
- the **inventory**: the 333 vision tensors the config implies, their names
  and shapes, checked against a checkpoint's header;
- **admission and patches**: which images the path takes as they are, and
  the processor's layout of 16 by 16 by 2 patches, block by block;
- the **encoder's parts**: the patch embedding with a 48 by 48 position
  table resized to the image's grid, a transformer block whose attention
  turns queries and keys by each patch's row and column, and the merger that
  joins 2 by 2 patches into one token of the text model's width;
- the **request layout**: image tokens spliced between the vision-start and
  vision-end tokens, with three rows of positions (time, height, width);
- an **original-weight slice check**, in `psionic/tests`, that runs the
  encoder's parts on the real weights against an independent oracle.

```shell
coax/bin/coax run psionic/examples/see.coax
coax/bin/coax run psionic/examples/see-refuse.coax
psionic/check.sh --only see
psionic/tests/check_see_slices.sh ~/models/elephant-1-b3f8a934 ~/models/mammoth-1-61cec44c
```

## The vision tower in the checkpoints

Both checkpoints' `config.json` carry the same `vision_config`, apart from
the merger's output width:

| Field | Value | Meaning |
| --- | --- | --- |
| `patch_size` | 16 | a patch is 16 by 16 pixels |
| `temporal_patch_size` | 2 | and 2 frames; a still image fills both |
| `in_channels` | 3 | RGB |
| `hidden_size` | 1152 | encoder width, in `num_heads` 16 heads of 72 |
| `intermediate_size` | 4304 | the block's feed-forward width |
| `depth` | 27 | transformer blocks |
| `num_position_embeddings` | 2304 | a learned 48 by 48 position table |
| `spatial_merge_size` | 2 | 2 by 2 patches become one token |
| `out_hidden_size` | 4096 (Elephant), 5120 (Mammoth) | the text model's width |
| `hidden_act` | `gelu_pytorch_tanh` | the block's activation |
| `deepstack_visual_indexes` | `[]` | no intermediate features feed the text model |

The text side names the tokens: `vision_start_token_id` 248053,
`vision_end_token_id` 248054 and `image_token_id` 248056. Its
`rope_parameters` say `mrope_interleaved: true` with `mrope_section`
`[11, 11, 10]`.

<!-- source: psionic/lib/see.coax configs -->
```jai
// Both checkpoints' vision_config: 16-pixel patches over 2 frames, 2 by 2
// merging, width 1152 in 16 heads, 27 blocks, a 48 by 48 position table.
// Only the merger's output width differs: the text model's hidden size.
// The processor's smallest area is 65536 pixels (256 by 256).
sv_elephant :: ()->SvConfig {
    return SvConfig.{patch=16,temporal=2,merge=2,width=1152,heads=16,inner=4304,depth=27,positions=2304,out=4096,least=65536,epsilon=0.000001,theta=10000.0};
}
sv_mammoth :: ()->SvConfig {c:=sv_elephant();c.out=5120;return c;}
```

The tensors follow from these numbers. Nine are shared: the patch
embedding's weight `[1152, 3, 2, 16, 16]` and bias, the position table
`[2304, 1152]`, and the merger's layer norm, `linear_fc1` `[4608, 4608]`
(4608 = 2 · 2 · 1152) and `linear_fc2` `[out, 4608]` with their biases. Each
block has twelve: two layer norms with biases, `attn.qkv` `[3456, 1152]`,
`attn.proj` `[1152, 1152]`, `mlp.linear_fc1` `[4304, 1152]` and
`mlp.linear_fc2` `[1152, 4304]`, with biases. That is 9 + 12 · 27 = 333
tensors, all BF16.

Counting parameters by hand for Elephant:

| Part | Parameters |
| --- | --- |
| patch embedding | 1152 · 1536 + 1152 = 1,770,624 |
| position table | 2304 · 1152 = 2,654,208 |
| merger norm | 2 · 1152 = 2,304 |
| merger `linear_fc1` | 4608 · 4608 + 4608 = 21,238,272 |
| merger `linear_fc2` | 4096 · 4608 + 4096 = 18,878,464 |
| one block | 4 · 1152 + 3,984,768 + 1,328,256 + 4,962,512 + 4,959,360 = 15,239,504 |
| 27 blocks | 411,466,608 |
| total | 456,010,480 |

Mammoth's `linear_fc2` is 5120 · 4608 + 5120 = 23,598,080, which makes
460,730,096.

`sv_need` writes the name and shape of tensor `i` in that fixed order, and
`sv_inventory` checks a header against it: every needed tensor present once,
BF16, with its shape, and no other tensor under `model.visual.`.

<!-- source: psionic/lib/see.coax inventory -->
```jai
// The vision tensors a checkpoint must hold, in a fixed order: nine shared
// tensors, then twelve per block. Each entry writes its name and returns its
// shape; every one is BF16.
SV_SHARED :: 9;
SV_PER_BLOCK :: 12;
sv_tensors :: (c:SvConfig)->int {return SV_SHARED+SV_PER_BLOCK*c.depth;}
sv_s1 :: (a:int)->P5Shape {return P5Shape.{rank=1,dims=[5]int.[a,0,0,0,0]};}
sv_s2 :: (a:int,b:int)->P5Shape {return P5Shape.{rank=2,dims=[5]int.[a,b,0,0,0]};}
sv_need :: (c:SvConfig,i:int,name:[]u8)->SvNeed {
    d:=c.width;k:=c.merge*c.merge*d;
    n:=json_raw(name,0,"model.visual.");
    if i<SV_SHARED {
        shared:=[9]string.["patch_embed.proj.weight","patch_embed.proj.bias","pos_embed.weight","merger.norm.weight","merger.norm.bias",
            "merger.linear_fc1.weight","merger.linear_fc1.bias","merger.linear_fc2.weight","merger.linear_fc2.bias"];
        shapes:=[9]P5Shape.[P5Shape.{rank=5,dims=[5]int.[d,3,c.temporal,c.patch,c.patch]},sv_s1(d),sv_s2(c.positions,d),sv_s1(d),sv_s1(d),
            sv_s2(k,k),sv_s1(k),sv_s2(c.out,k),sv_s1(c.out)];
        return SvNeed.{count=json_raw(name,n,shared[i]),shape=shapes[i]};
    }
    b:=(i-SV_SHARED)/SV_PER_BLOCK;j:=(i-SV_SHARED)%SV_PER_BLOCK;
    suffixes:=[12]string.["norm1.weight","norm1.bias","attn.qkv.weight","attn.qkv.bias","attn.proj.weight","attn.proj.bias",
        "norm2.weight","norm2.bias","mlp.linear_fc1.weight","mlp.linear_fc1.bias","mlp.linear_fc2.weight","mlp.linear_fc2.bias"];
    shapes:=[12]P5Shape.[sv_s1(d),sv_s1(d),sv_s2(3*d,d),sv_s1(3*d),sv_s2(d,d),sv_s1(d),sv_s1(d),sv_s1(d),sv_s2(c.inner,d),sv_s1(c.inner),sv_s2(d,c.inner),sv_s1(d)];
    n=json_raw(name,n,"blocks.");n=json_write_int(name,n,b);n=json_raw(name,n,".");
    return SvNeed.{count=json_raw(name,n,suffixes[j]),shape=shapes[j]};
}
sv_visual :: (name:string)->bool {
    prefix:="model.visual.";if name.count<prefix.count {return false;}
    for i:0..<prefix.count {if name[i]!=prefix[i] {return false;}}
    return true;
}
// Every needed tensor present once, BF16, with the config's shape, and no
// other tensor under model.visual. Returns the parameter count.
sv_inventory :: (c:SvConfig,inventory:[]P5Tensor)->SvResult {
    if sv_config_status(c)!=SvStatus.ok {return SvResult.{status=SvStatus.config};}
    total:=0;
    for i:0..<sv_tensors(c) {
        name:[128]u8;need:=sv_need(c,i,name[..]);key:=cast(string)name[0..<need.count];found:=-1;
        for t:0..<inventory.count {if inventory[t].name==key {if found>=0 {return SvResult.{status=SvStatus.extra,cause=i};}found=t;}}
        if found<0 {return SvResult.{status=SvStatus.missing,cause=i};}
        if inventory[found].dtype!=PwDtype.bf16 {return SvResult.{status=SvStatus.dtype,cause=i};}
        if !pa_shape_equal(inventory[found].shape,need.shape) {return SvResult.{status=SvStatus.shape,cause=i};}
        total+=inventory[found].elements;
    }
    visual:=0;for t:inventory {if sv_visual(t.name) {visual+=1;}}
    if visual!=sv_tensors(c) {return SvResult.{status=SvStatus.extra,cause=visual};}
    return SvResult.{written=total,cause=visual};
}
```

The slice check reads each checkpoint's header with chapter 7's rank-5
inventory (`p5_inspect_checked`), without reading any weight bytes, and
finds exactly these tensors:

```text
elephant: 760 tensors in the file, 333 under model.visual, inventory status 0 (entry 333), 456010480 vision parameters
mammoth: 400 tensors in the file, 333 under model.visual, inventory status 0 (entry 333), 460730096 vision parameters
```

Mammoth's 333 vision tensors are all in the second of its two shards.

## Which images are admitted

The Qwen3-VL image processor resizes an image so that each side is a
multiple of `patch_size · spatial_merge_size` = 32 and the area stays within
its bounds (at least 65,536 pixels), rescales bytes by 1/255 and normalizes
each channel with mean 0.5 and spread 0.5. The pinned model directories hold
no processor configuration, so this chapter takes those values from the
upstream Qwen3-VL processor defaults and states them as its profile.

This chapter does not resample. It admits an image only when the processor
would leave its size unchanged: 8-bit RGB, both sides multiples of 32, an
area of at least 65,536 pixels, and sides of at most 512 pixels (chapter
142's bound, which keeps the patch grid within 32 by 32). Anything else is
refused with a typed reason, before any value is computed.

<!-- source: psionic/lib/see.coax admit -->
```jai
sv_side :: (n:int)->int {s:=1;while (s+1)*(s+1)<=n {s+=1;}return s;}
sv_config_status :: (c:SvConfig)->SvStatus {
    sizes:=[8]int.[c.patch,c.temporal,c.merge,c.width,c.heads,c.inner,c.positions,c.out];
    for s:sizes {if s<1 || s>SV_EXTENT_MAX {return SvStatus.config;}}
    if c.depth<1 || c.least<1 || !(c.epsilon>0) || !(c.theta>1) {return SvStatus.config;}
    // Each head splits in half for rotation, and each half into a row part
    // and a column part; the position table is a square grid.
    if c.width%c.heads!=0 || (c.width/c.heads)%4!=0 {return SvStatus.config;}
    s:=sv_side(c.positions);if s*s!=c.positions {return SvStatus.config;}
    return SvStatus.ok;
}
// An image is admitted as is: 8-bit RGB, both sides a multiple of patch times
// merge (32 pixels), at least the processor's smallest area, at most
// chapter 142's 512-pixel side. For such an image the processor's resize
// rule (round each side to a multiple of 32, then keep the area within its
// bounds) leaves the size unchanged, so no resampling happens here.
sv_admit :: (c:SvConfig,image:PxImage)->SvGrid {
    if sv_config_status(c)!=SvStatus.ok {return SvGrid.{status=SvStatus.config};}
    s:=px_image_status(image);
    if s==PxStatus.shape {return SvGrid.{status=SvStatus.shape};}
    if s==PxStatus.limit {return SvGrid.{status=SvStatus.large};}
    if s==PxStatus.channels {return SvGrid.{status=SvStatus.channels};}
    if s==PxStatus.depth {return SvGrid.{status=SvStatus.depth};}
    unit:=c.patch*c.merge;
    if image.width%unit!=0 || image.height%unit!=0 {return SvGrid.{status=SvStatus.multiple};}
    if image.width*image.height<c.least {return SvGrid.{status=SvStatus.small};}
    g:=SvGrid.{grid_h=image.height/c.patch,grid_w=image.width/c.patch,features=3*c.temporal*c.patch*c.patch};
    if g.grid_h>SV_GRID_MAX || g.grid_w>SV_GRID_MAX {return SvGrid.{status=SvStatus.large};}
    g.patches=g.grid_h*g.grid_w;g.tokens=g.patches/(c.merge*c.merge);
    return g;
}
```

By hand, for a 320 by 256 image (width by height): the grid is 256 / 16 = 16
rows by 320 / 16 = 20 columns, 320 patches. Each patch row holds 3 · 2 · 16 ·
16 = 1536 values. Merging 2 by 2 leaves 8 by 10 = 80 image tokens.

## Bytes to patch rows

The processor's values come from chapter 142's preprocessing: byte b becomes
(b / 255 − 0.5) / 0.5, so 0 becomes −1, 255 becomes 1 and 51 becomes
(0.2 − 0.5) / 0.5 = −0.6. Each stage rounds to f32 once, as chapter 142
states.

The patch order is not row by row. The processor reshapes the image into
(merged row, merged column, row within the block, column within the block)
and lists patches in that order, so the four patches of one merged token are
adjacent. In a grid 20 patches wide, patches 0 to 3 are cells (0, 0), (0, 1),
(1, 0) and (1, 1); patch 4 starts the next block at (0, 2). Inside a patch
row the values are (channel, frame, row, column): the layout of the patch
embedding's weight `[1152, 3, 2, 16, 16]`.

<!-- source: psionic/lib/see.coax pixels -->
```jai
// Patch r of the processor's order: merged blocks left to right and top to
// bottom, and inside each block its merge by merge patches in the same order.
sv_cell :: (c:SvConfig,g:SvGrid,r:int)->[2]int {
    per:=c.merge*c.merge;across:=g.grid_w/c.merge;b:=r/per;i:=r%per;
    return [2]int.[(b/across)*c.merge+i/c.merge,(b%across)*c.merge+i%c.merge];
}
// Bytes to patch rows. Chapter 142's preprocessing with the processor's
// profile (scale 1/255, mean 0.5, spread 0.5, no resize) gives a
// channel-major image; each patch row then holds (channel, frame, row,
// column) values, the layout of the patch embedding's weights. A still image
// repeats in every frame of the temporal patch.
sv_profile :: (image:PxImage)->PxProfile {
    return PxProfile.{scale=1.0/255,mean=[3]f64.[0.5,0.5,0.5],std=[3]f64.[0.5,0.5,0.5],height=image.height,width=image.width,filter=PxFilter.nearest};
}
sv_pixels :: (c:SvConfig,image:PxImage,bytes:[]u8,work:[]f32,out:[]f32,overlap:bool)->SvResult {
    g:=sv_admit(c,image);if g.status!=SvStatus.ok {return SvResult.{status=g.status};}
    area:=image.width*image.height;stage:=3*area;
    if bytes.count!=stage {return SvResult.{status=SvStatus.shape};}
    if work.count<2*stage || out.count<g.patches*g.features {return SvResult.{status=SvStatus.capacity};}
    if overlap {return SvResult.{status=SvStatus.alias};}
    #assert px_preprocess(image,bytes,sv_profile(image),work[0..<stage],work[stage..<2*stage],false).written==stage;
    pp:=c.patch*c.patch;
    for r:0..<g.patches {
        cell:=sv_cell(c,g,r);
        for k:0..<g.features {
            ch:=k/(c.temporal*pp);y:=cell[0]*c.patch+(k/c.patch)%c.patch;x:=cell[1]*c.patch+k%c.patch;
            out[r*g.features+k]=work[stage+(ch*image.height+y)*image.width+x];
        }
    }
    return SvResult.{written=g.patches*g.features};
}
```

## The patch embedding and its positions

The patch embedding is a 3-D convolution whose stride equals its kernel, so
it is one row product per patch: 1536 inputs, 1152 outputs, plus the bias.
Chapter 142's `px_patch_embed` bounds its product at 2^20 multiply-adds,
which a single patch here exceeds (1536 · 1152 = 1,769,472), so this chapter
uses its own row product: binary64 sums, one rounding per stored value.

Each patch then adds its learned position. The table has 48 by 48 rows, and
the grid is usually smaller. The model resizes it bilinearly with the
corners aligned: grid row y reads table row y · 47 / (rows − 1) and blends
the two table rows around it, and the same for columns. On a 16-row grid,
row 1 reads 47 / 15 = 3.1333, so it is 0.8667 of table row 3 plus 0.1333 of
table row 4. A one-row grid reads table row 0. This differs from chapter
142's `px_positions`, which uses half-pixel centres.

<!-- source: psionic/lib/see.coax embed -->
```jai
// The patch embedding, a convolution whose stride equals its kernel, is one
// row product per patch (3 * 2 * 16 * 16 = 1536 inputs), plus the learned
// position of its cell. The 48 by 48 table is resized to the patch grid
// bilinearly with corners aligned: grid row y reads table row
// y * (side - 1) / (rows - 1) and blends the two rows around it.
sv_embed_work :: (c:SvConfig,g:SvGrid)->int {return g.patches*c.width;}
sv_embed :: (c:SvConfig,image:PxImage,patches:[]f32,w:[]f32,bias:[]f32,table:[]f32,out:[]f32,work:[]f32,overlap:bool)->SvResult {
    g:=sv_admit(c,image);if g.status!=SvStatus.ok {return SvResult.{status=g.status};}
    n:=g.patches;f:=g.features;d:=c.width;
    if patches.count!=n*f || w.count!=d*f || bias.count!=d || table.count!=c.positions*d {return SvResult.{status=SvStatus.shape};}
    if out.count<n*d || work.count<n*d {return SvResult.{status=SvStatus.capacity};}
    if overlap {return SvResult.{status=SvStatus.alias};}
    if !px_finite(patches) || !px_finite(w) || !px_finite(bias) || !px_finite(table) {return SvResult.{status=SvStatus.number};}
    sv_linear(patches,n,f,w,bias,d,work);
    side:=sv_side(c.positions);
    for r:0..<n {
        cell:=sv_cell(c,g,r);a:=sv_point(cell[0],g.grid_h,side);b:=sv_point(cell[1],g.grid_w,side);
        low:=a.low*side;high:=a.high*side;
        for k:0..<d {
            top:=(1-b.fraction)*f64(table[(low+b.low)*d+k])+b.fraction*f64(table[(low+b.high)*d+k]);
            bottom:=(1-b.fraction)*f64(table[(high+b.low)*d+k])+b.fraction*f64(table[(high+b.high)*d+k]);
            work[r*d+k]=sv_f32(f64(work[r*d+k])+f64(sv_f32((1-a.fraction)*top+a.fraction*bottom)));
        }
    }
    if !px_finite(work[0..<n*d]) {return SvResult.{status=SvStatus.number};}
    for i:0..<n*d {out[i]=work[i];}
    return SvResult.{written=n*d};
}
SvPoint :: struct {low:int;high:int;fraction:f64;}
sv_point :: (i:int,count:int,side:int)->SvPoint {
    if count==1 {return SvPoint.{}; }
    p:=f64(i*(side-1))/f64(count-1);low:=cast(int)floor(p);
    return SvPoint.{low=low,high=ifx low<side-1 then low+1 else low,fraction=p-f64(low)};
}
```

## One block, with 2-D rotary positions

A block is `x + proj(attention(norm1(x)))`, then `+ fc2(gelu(fc1(norm2(x))))`,
with layer norms (population variance, ε = 10^−6) and GELU in its tanh form.
Attention runs over all of the image's patches in both directions, 16 heads
of 72.

Queries and keys turn by each patch's position. A head of 72 values splits
into halves of 36 that rotate together; of the 36 angles, the first 18 use
the patch's row and the last 18 its column, each times a frequency
10000^(−4j/72) for j from 0 to 17. Patch 3 of a 16 by 16 grid is cell
(1, 1), so its angle 1 is 1 · 10000^(−1/18) = 0.5995 and its angle 18 is
1 · 10000^0 = 1. Patch 0 at (0, 0) does not turn.

<!-- source: psionic/lib/see.coax rotary -->
```jai
// The 2-D rotary angles of patch r: a head of d values rotates its first and
// second halves together; the first quarter of frequencies turns with the
// patch's row, the second with its column. Frequency j of d/4 is
// theta^(-4j/d), the table of a rotary embedding of width d/2.
sv_angle :: (c:SvConfig,g:SvGrid,r:int,i:int)->f64 {
    d:=c.width/c.heads;quarter:=d/4;j:=i%(d/2);cell:=sv_cell(c,g,r);
    p:=cell[0];if j>=quarter {p=cell[1];j-=quarter;}
    return f64(p)*exp(-log(c.theta)*f64(4*j)/f64(d));
}
```

The largest score is subtracted before exponentiating, so scores in the
thousands do not overflow; a check builds scores of 3600 to prove it.

<!-- source: psionic/lib/see.coax block -->
```jai
// One encoder block: x + attention(norm1(x)), then + fc2(gelu(fc1(norm2(x)))).
// Attention is bidirectional over the image's patches, per head, with the
// rotary angles above applied to queries and keys.
sv_block_work :: (c:SvConfig,n:int)->int {return n*(6*c.width+c.inner);}
sv_block_status :: (c:SvConfig,w:SvBlock)->SvStatus {
    d:=c.width;i:=c.inner;
    counts:=[12]int.[w.norm1.count,w.norm1_bias.count,w.qkv.count,w.qkv_bias.count,w.proj.count,w.proj_bias.count,
        w.norm2.count,w.norm2_bias.count,w.fc1.count,w.fc1_bias.count,w.fc2.count,w.fc2_bias.count];
    want:=[12]int.[d,d,3*d*d,3*d,d*d,d,d,d,i*d,i,d*i,d];
    for k:0..<12 {if counts[k]!=want[k] {return SvStatus.shape;}}
    parts:=[12][]f32.[w.norm1,w.norm1_bias,w.qkv,w.qkv_bias,w.proj,w.proj_bias,w.norm2,w.norm2_bias,w.fc1,w.fc1_bias,w.fc2,w.fc2_bias];
    for p:parts {if !px_finite(p) {return SvStatus.number;}}
    return SvStatus.ok;
}
sv_block :: (c:SvConfig,image:PxImage,x:[]f32,w:SvBlock,out:[]f32,work:[]f32,overlap:bool)->SvResult {
    g:=sv_admit(c,image);if g.status!=SvStatus.ok {return SvResult.{status=g.status};}
    n:=g.patches;d:=c.width;heads:=c.heads;hd:=d/heads;half:=hd/2;
    if x.count!=n*d {return SvResult.{status=SvStatus.shape};}
    s:=sv_block_status(c,w);if s!=SvStatus.ok {return SvResult.{status=s};}
    if out.count<n*d || work.count<sv_block_work(c,n) {return SvResult.{status=SvStatus.capacity};}
    if overlap {return SvResult.{status=SvStatus.alias};}
    if !px_finite(x) {return SvResult.{status=SvStatus.number};}
    q0:=n*d;a0:=4*n*d;m0:=5*n*d;f0:=6*n*d;
    h:=work[0..<q0];qkv:=work[q0..<a0];att:=work[a0..<m0];mid:=work[m0..<f0];f:=work[f0..<sv_block_work(c,n)];
    sv_norm(x,n,d,w.norm1,w.norm1_bias,c.epsilon,h);
    sv_linear(h,n,d,w.qkv,w.qkv_bias,3*d,qkv);
    // Rotate queries (part 0) and keys (part 1) in place, half against half.
    // Patch 0 sits at (0, 0) and does not turn; rows go last to first.
    for back:0..<n {r:=n-1-back;for part:0..<2 {for head:0..<heads {for i:0..<half {
        at:=r*3*d+part*d+head*hd;angle:=sv_angle(c,g,r,i);a:=f64(qkv[at+i]);b:=f64(qkv[at+i+half]);
        qkv[at+i]=sv_f32(a*cos(angle)-b*sin(angle));qkv[at+i+half]=sv_f32(b*cos(angle)+a*sin(angle));
    }}}}
    scale:=1/sqrt(f64(hd));
    for head:0..<heads {for r:0..<n {
        q:=r*3*d+head*hd;top:=sv_score(qkv,q,d+head*hd,hd,scale);
        for j:1..<n {v:=sv_score(qkv,q,j*3*d+d+head*hd,hd,scale);top=(top+v+abs(top-v))/2;}
        total:=0.0;for j:0..<n {total+=exp(sv_score(qkv,q,j*3*d+d+head*hd,hd,scale)-top);}
        for e:0..<hd {
            acc:=0.0;
            for j:0..<n {acc+=exp(sv_score(qkv,q,j*3*d+d+head*hd,hd,scale)-top)*f64(qkv[j*3*d+2*d+head*hd+e]);}
            att[r*d+head*hd+e]=sv_f32(acc/total);
        }
    }}
    sv_linear(att,n,d,w.proj,w.proj_bias,d,mid);
    for i:0..<n*d {mid[i]=sv_f32(f64(mid[i])+f64(x[i]));}
    sv_norm(mid,n,d,w.norm2,w.norm2_bias,c.epsilon,h);
    sv_linear(h,n,d,w.fc1,w.fc1_bias,c.inner,f);
    for i:0..<n*c.inner {f[i]=sv_f32(sv_gelu_tanh(f64(f[i])));}
    sv_linear(f,n,c.inner,w.fc2,w.fc2_bias,d,att);
    for i:0..<n*d {att[i]=sv_f32(f64(att[i])+f64(mid[i]));}
    if !px_finite(att) {return SvResult.{status=SvStatus.number};}
    for i:0..<n*d {out[i]=att[i];}
    return SvResult.{written=n*d};
}
sv_score :: (qkv:[]f32,q:int,k:int,width:int,scale:f64)->f64 {
    acc:=0.0;for e:0..<width {acc+=f64(qkv[q+e])*f64(qkv[k+e]);}return acc*scale;
}
```

## The merger

The merger layer-normalizes each patch, joins each block's four adjacent
patch rows into one row of 4608 values, and applies `linear_fc1`, exact GELU
and `linear_fc2`. The result is one token of the text model's width per
merged block: 4096 values for Elephant, 5120 for Mammoth.

GELU here uses erf. Coax has no erf, so `sv_erf` uses Abramowitz and
Stegun's formula 7.1.26, whose absolute error is below 1.5 · 10^−7. That is
an approximation the oracle does not share; the tolerances below cover it.

<!-- source: psionic/lib/see.coax linear -->
```jai
// A value too large for f32 becomes NaN, which the finiteness check after
// each stage refuses.
sv_f32 :: (v:f64)->f32 {if psionic_ops_fits_f32(v) {return f32(v);}return f32(0.0/0.0);}
// y = x W^T + b over rows, binary64 sums, one rounding per stored value.
sv_linear :: (x:[]f32,rows:int,inputs:int,w:[]f32,bias:[]f32,outputs:int,y:[]f32) {
    for r:0..<rows {for o:0..<outputs {
        acc:=f64(bias[o]);
        for k:0..<inputs {acc+=f64(x[r*inputs+k])*f64(w[o*inputs+k]);}
        y[r*outputs+o]=sv_f32(acc);
    }}
}
// Layer normalization over each row: population variance, epsilon inside
// the root, then scale and shift.
sv_norm :: (x:[]f32,rows:int,width:int,gain:[]f32,bias:[]f32,epsilon:f64,y:[]f32) {
    for r:0..<rows {
        mean:=0.0;for k:0..<width {mean+=f64(x[r*width+k]);}mean/=f64(width);
        spread:=0.0;for k:0..<width {d:=f64(x[r*width+k])-mean;spread+=d*d;}
        inverse:=1/sqrt(spread/f64(width)+epsilon);
        for k:0..<width {y[r*width+k]=sv_f32((f64(x[r*width+k])-mean)*inverse*f64(gain[k])+f64(bias[k]));}
    }
}
// tanh as 1 - 2 / (e^(2a) + 1); past |a| = 10 it is within 5e-9 of 1.
sv_tanh :: (a:f64)->f64 {if a>10.0 {return 1.0;}if a<-10.0 {return -1.0;}return 1-2/(exp(2*a)+1);}
// The block's activation, GELU with the tanh approximation.
sv_gelu_tanh :: (z:f64)->f64 {return 0.5*z*(1+sv_tanh(0.7978845608028654*(z+0.044715*z*z*z)));}
// The merger's activation, GELU with erf. erf by Abramowitz and Stegun
// 7.1.26, absolute error below 1.5e-7, odd in x; past |x| = 8 it is 1 to
// that accuracy.
sv_erf :: (x:f64)->f64 {
    z:=abs(x);t:=1/(1+0.3275911*z);
    p:=((((1.061405429*t-1.453152027)*t+1.421413741)*t-0.284496736)*t+0.254829592)*t;
    v:=1-p*exp(-z*z);
    return ifx x<0 then -v else v;
}
sv_gelu :: (z:f64)->f64 {return 0.5*z*(1+sv_erf(z*0.7071067811865476));}
```

<!-- source: psionic/lib/see.coax merge -->
```jai
// The merger: layer-normalize each patch, join each block's merge by merge
// patches into one row (4 * 1152 = 4608 values), then fc2(gelu(fc1(row))):
// one token of the text model's width per block.
sv_merge_work :: (c:SvConfig,g:SvGrid)->int {k:=c.merge*c.merge*c.width;return g.patches*c.width+g.tokens*(k+c.out);}
sv_merge :: (c:SvConfig,image:PxImage,x:[]f32,m:SvMerger,out:[]f32,work:[]f32,overlap:bool)->SvResult {
    g:=sv_admit(c,image);if g.status!=SvStatus.ok {return SvResult.{status=g.status};}
    n:=g.patches;t:=g.tokens;d:=c.width;k:=c.merge*c.merge*d;
    if x.count!=n*d || m.norm.count!=d || m.norm_bias.count!=d || m.fc1.count!=k*k || m.fc1_bias.count!=k || m.fc2.count!=c.out*k || m.fc2_bias.count!=c.out {
        return SvResult.{status=SvStatus.shape};
    }
    if out.count<t*c.out || work.count<sv_merge_work(c,g) {return SvResult.{status=SvStatus.capacity};}
    if overlap {return SvResult.{status=SvStatus.alias};}
    parts:=[7][]f32.[x,m.norm,m.norm_bias,m.fc1,m.fc1_bias,m.fc2,m.fc2_bias];
    for p:parts {if !px_finite(p) {return SvResult.{status=SvStatus.number};}}
    ty:=t*c.out;zk:=t*k;y:=work[0..<ty];z:=work[ty..<ty+zk];h:=work[ty+zk..<sv_merge_work(c,g)];
    sv_norm(x,n,d,m.norm,m.norm_bias,c.epsilon,h);
    sv_linear(h,t,k,m.fc1,m.fc1_bias,k,z);
    for i:0..<t*k {z[i]=sv_f32(sv_gelu(f64(z[i])));}
    sv_linear(z,t,k,m.fc2,m.fc2_bias,c.out,y);
    if !px_finite(y) {return SvResult.{status=SvStatus.number};}
    for i:0..<t*c.out {out[i]=y[i];}
    return SvResult.{written=t*c.out};
}
```

## Where image tokens sit in a request

A rendered request with one image is the text before it (ending with the
role header), `<|vision_start|>`, one `<|image_pad|>` token per merged
block, `<|vision_end|>`, and the text after it. Every token carries three
positions: time, height and width. Text tokens carry their index on all
three rows. Image tokens carry the image's start s on the time row and s
plus their merged block's row and column on the other two. Text after the
image continues from the largest position used plus one.

By hand, for three text tokens, the 320 by 256 image (8 by 10 merged
blocks) and two text tokens after it:

| Token | Id | Time | Height | Width |
| --- | --- | --- | --- | --- |
| 0 | text | 0 | 0 | 0 |
| 3 | vision start | 3 | 3 | 3 |
| 4 | image, block (0, 0) | 4 | 4 | 4 |
| 5 | image, block (0, 1) | 4 | 4 | 5 |
| 14 | image, block (1, 0) | 4 | 5 | 4 |
| 83 | image, block (7, 9) | 4 | 11 | 13 |
| 84 | vision end | 4 + max(8, 10) = 14 | 14 | 14 |
| 86 | text | 16 | 16 | 16 |

The request has 3 + 1 + 80 + 1 + 2 = 87 tokens. The text model's rotary
width is 64 of each 256-value head (`partial_rotary_factor` 0.25), so 32
frequencies. Interleaved sections (11, 11, 10) assign them in turn: j mod 3
= 1 reads height for the first 11 such j, j mod 3 = 2 reads width for the
first 10, and every other j reads time. For 32 frequencies that is exactly
j mod 3. A text-only request has equal rows, so it turns exactly as the
text profile does today.

Client text cannot contain the reserved vision tokens (248053 to 248057);
such a request is refused, so only the layout places image tokens.

<!-- source: psionic/lib/see.coax layout -->
```jai
// The rendered request with an image: the text before it (ending with the
// role header), the vision-start token, one image token per merged block,
// the vision-end token, then the text after. Positions are three rows,
// (time, height, width). Text tokens carry their index on all three; image
// tokens carry the image's start s on the time row and s plus their block's
// row and column; text after the image continues from the largest
// position used plus one.
sv_layout_length :: (g:SvGrid,before:int,after:int)->int {return before+after+g.tokens+2;}
sv_layout :: (c:SvConfig,image:PxImage,before:[]int,after:[]int,ids:[]int,pos:[]int)->SvResult {
    g:=sv_admit(c,image);if g.status!=SvStatus.ok {return SvResult.{status=g.status};}
    for t:before {if !sv_text_token(t) {return SvResult.{status=SvStatus.layout,cause=t};}}
    for t:after {if !sv_text_token(t) {return SvResult.{status=SvStatus.layout,cause=t};}}
    total:=sv_layout_length(g,before.count,after.count);
    if ids.count<total || pos.count<3*total {return SvResult.{status=SvStatus.capacity};}
    at:=0;
    for t:before {sv_put(ids,pos,total,at,t,at,at,at);at+=1;}
    sv_put(ids,pos,total,at,SV_VISION_START,at,at,at);at+=1;
    start:=at;across:=g.grid_w/c.merge;
    for k:0..<g.tokens {sv_put(ids,pos,total,at,SV_IMAGE,start,start+k/across,start+k%across);at+=1;}
    down:=g.grid_h/c.merge;next:=start+(down+across+cast(int)abs(f64(down-across)))/2;
    sv_put(ids,pos,total,at,SV_VISION_END,next,next,next);at+=1;
    for t:after {next+=1;sv_put(ids,pos,total,at,t,next,next,next);at+=1;}
    return SvResult.{written=total};
}
// Vision start and end, the vision pad, image and video tokens are reserved:
// a client's text cannot place them.
sv_text_token :: (t:int)->bool {return t>=0 && t<SV_VOCAB && (t<SV_VISION_START || t>SV_VIDEO);}
sv_put :: (ids:[]int,pos:[]int,total:int,at:int,id:int,t:int,h:int,w:int) {ids[at]=id;pos[at]=t;pos[total+at]=h;pos[2*total+at]=w;}
// The text model's rotary frequency j (of 32: rotary width 64 of head width
// 256) reads one of the three rows. Interleaved sections (11, 11, 10): j mod
// 3 = 1 reads height for the first 11 such j, j mod 3 = 2 reads width for
// the first 10,
// every other j reads time.
sv_mrope_axis :: (sections:[3]int,j:int)->int {
    if j%3==1 && j/3<sections[1] {return 1;}
    if j%3==2 && j/3<sections[2] {return 2;}
    return 0;
}
```

## The program

The [example](../examples/see.coax) prints each value derived by hand above,
then runs the tiny encoder end to end in well under a second.

<!-- source: psionic/examples/see.coax example -->
```jai
main :: () {
    e:=sv_elephant();m:=sv_mammoth();
    print("vision tensors: Elephant %, Mammoth %; merger output width % and %\n",sv_tensors(e),sv_tensors(m),e.out,m.out);
    image:=PxImage.{width=320,height=256};g:=sv_admit(e,image);
    print("320x256 image: % patches in a % by % grid, % values each, % image tokens\n",g.patches,g.grid_h,g.grid_w,g.features,g.tokens);
    // Bytes 0, 51 and 255 under the processor's profile.
    print("bytes 0, 51, 255 become % % %\n",(0.0/255-0.5)/0.5,f32((f64(f32(51*(1.0/255)))-0.5)/0.5),(255.0/255-0.5)/0.5);
    for r:0..<6 {cell:=sv_cell(e,g,r);print("patch % is grid cell (%, %)\n",r,cell[0],cell[1]);}

    before:=[3]int.[10,11,12];after:=[2]int.[20,21];ids:[87]int;pos:[261]int;
    r:=sv_layout(e,image,before[..],after[..],ids[..],pos[..]);
    print("request: % tokens\n",r.written);
    show:=[8]int.[0,3,4,5,14,83,84,86];
    for i:show {print("  token % id %: time % height % width %\n",i,ids[i],pos[i],pos[87+i],pos[174+i]);}
    sections:=[3]int.[11,11,10];print("M-RoPE rows of the 32 text frequencies:");
    for j:0..<32 {print(" %",sv_mrope_axis(sections,j));}print("\n");

    // The tiny encoder: an 8 by 4 image, 8 patches, 2 tokens of width 5.
    t:=sv_tiny();small:=PxImage.{width=8,height=4};bytes:[96]u8;sv_bytes(bytes[..]);
    w:=sv_tiny_weights();work:[432]f32;patches:[192]f32;embedded:[64]f32;blocked:[64]f32;merged:[10]f32;
    r=sv_pixels(t,small,bytes[..],work[0..<192],patches[..],false);
    r=sv_embed(t,small,patches[..],w.patch_w[..],w.patch_b[..],w.table[..],embedded[..],work[..],false);
    r=sv_block(t,small,embedded[..],sv_tiny_block(w.block[..]),blocked[..],work[..],false);
    r=sv_merge(t,small,blocked[..],sv_tiny_merger(w.merger[..]),merged[..],work[0..<138],false);
    oracle:=SV_ORACLE_MERGE;
    print("tiny encoder: %, % values, within the oracle's tolerance: %\n",r.status,r.written,sv_near(merged[..],oracle[..]));
    print("  token 0:");for i:0..<5 {print(" %",merged[i]);}print("\n");
}
```

```text
vision tensors: Elephant 333, Mammoth 333; merger output width 4096 and 5120
320x256 image: 320 patches in a 16 by 20 grid, 1536 values each, 80 image tokens
bytes 0, 51, 255 become -1.0 -0.6 1.0
patch 0 is grid cell (0, 0)
patch 1 is grid cell (0, 1)
patch 2 is grid cell (1, 0)
patch 3 is grid cell (1, 1)
patch 4 is grid cell (0, 2)
patch 5 is grid cell (0, 3)
request: 87 tokens
  token 0 id 10: time 0 height 0 width 0
  token 3 id 248053: time 3 height 3 width 3
  token 4 id 248056: time 4 height 4 width 4
  token 5 id 248056: time 4 height 4 width 5
  token 14 id 248056: time 4 height 5 width 4
  token 83 id 248056: time 4 height 11 width 13
  token 84 id 248054: time 14 height 14 width 14
  token 86 id 21: time 16 height 16 width 16
M-RoPE rows of the 32 text frequencies: 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1 2 0 1
tiny encoder: ok, 10 values, within the oracle's tolerance: true
  token 0: -2.559846 0.58792496 -1.0654418 0.2055582 1.7459991
```

Every number above the tiny encoder is the hand calculation of the earlier
sections. The tiny encoder is a configuration small enough to read: 2-pixel
patches over 2 frames, width 8 in 2 heads, a 3 by 3 position table, an 8 by
4 image, and weights that are multiples of 1/16.

## Refusals

The [refusal program](../examples/see-refuse.coax) feeds the path one
broken input at a time.

<!-- source: psionic/examples/see-refuse.coax refuse -->
```jai
main :: () {
    e:=sv_elephant();
    images:=[6]PxImage.[PxImage.{width=250,height=256},PxImage.{width=128,height=128},PxImage.{width=544,height=256},
        PxImage.{width=256,height=256,channels=4},PxImage.{width=256,height=256,depth=16},PxImage.{width=0,height=256}];
    labels:=[6]string.["250x256, not a multiple of 32","128x128, below 65536 pixels","544x256, over 512 pixels a side",
        "256x256 with 4 channels","256x256 with 16-bit channels","0x256"];
    for i:0..<6 {print("%: %\n",labels[i],sv_admit(e,images[i]).status);}
    // A client's text that already holds an image token is refused.
    image:=PxImage.{width=256,height=256};before:=[2]int.[10,SV_IMAGE];after:=[1]int.[20];ids:[70]int;pos:[210]int;
    r:=sv_layout(e,image,before[..],after[..],ids[..],pos[..]);
    print("text holding token %: %\n",r.cause,r.status);
    // Weights one value short, and too little output room.
    t:=sv_tiny();small:=PxImage.{width=8,height=4};w:=sv_tiny_weights();x:[192]f32;sv_fill(x[..],3);work:[64]f32;out:[64]f32;
    r=sv_embed(t,small,x[..],w.patch_w[0..<191],w.patch_b[..],w.table[..],out[..],work[..],false);
    print("patch weights one short: %\n",r.status);
    r=sv_embed(t,small,x[..],w.patch_w[..],w.patch_b[..],w.table[..],out[0..<63],work[..],false);
    print("output one value short: %\n",r.status);
}
```

```text
250x256, not a multiple of 32: multiple
128x128, below 65536 pixels: small
544x256, over 512 pixels a side: large
256x256 with 4 channels: channels
256x256 with 16-bit channels: depth
0x256: shape
text holding token 248056: layout
patch weights one short: shape
output one value short: capacity
```

The program exits with status 0 after printing each refusal. Every
encoder procedure also refuses aliasing, non-finite weights or inputs, and
values that overflow f32 (`number`), before writing its output.

## On the original weights

`psionic/tests/check_see_slices.sh` compiles `see-slices-runtime.coax` to C
with a small host (`see_slices.c`) that maps the checkpoint. Coax checks the
inventory, decodes the 21 tensors it needs from BF16, and runs the slice:
the processor's patches for a 64 by 32 image (a 2 by 4 patch grid; the
processor's smallest area is lifted for the slice only), the patch
embedding with positions, block 0 and the merger, then the processor's
patches for a 320 by 256 image. `see_slices.py` reads the same header and
tensors from the file itself and recomputes everything in binary64 Python,
following the upstream code's reshapes, `linspace` interpolation, rotary
table and `rotate_half` rather than this chapter's loops. The tolerance,
frozen before the first comparison, is |Coax − oracle| ≤ 10^−4 · (1 +
|oracle|) for every value. The recorded run
([`psionic/fixtures/see/slices.txt`](../fixtures/see/slices.txt)) took 24
seconds for both models:

```text
  patches: 12288 values, largest |coax - oracle| / (1 + |oracle|) = 5.91e-08
  embedding: 9216 values, largest |coax - oracle| / (1 + |oracle|) = 6.04e-08
  block 0: 9216 values, largest |coax - oracle| / (1 + |oracle|) = 3.14e-07
  merger: 8192 values, largest |coax - oracle| / (1 + |oracle|) = 3.13e-07
  320x256 patches: 491520 values, largest |coax - oracle| / (1 + |oracle|) = 5.91e-08
within the frozen tolerance 0.0001
```

Those are Elephant's; Mammoth's are within 5.61 · 10^−7. Both sides
compute from the BF16 weights decoded exactly; neither is the upstream
runtime's BF16 arithmetic, so this is agreement on the computation, not a
capture of the upstream model's outputs.

## Supported profile

Profile psionic-see-v1, bounded CPU: f32 storage, binary64 sums, one
rounding to f32 per stored value; erf by Abramowitz and Stegun 7.1.26;
images 8-bit RGB, sides multiples of 32 up to 512, at least 65,536 pixels,
no resampling; one image per request.

## How it is checked

- `#test` checks in `see.coax` derive admission, cell order, rotary angles,
  position points, layer norm, activations, names and shapes, inventory
  defects and the request layout by hand.
- Two tiny encoders, one with 2-pixel patches merged 2 by 2 and one with
  3-pixel patches over one frame merged 3 by 3, match
  `psionic/tests/see_oracle.py`, an independent binary64 oracle that follows
  the upstream processor and model code; its values are in
  `psionic/fixtures/see/oracle.coax`, within 10^−6 · (1 + |value|).
- `coax mutate` on `see.coax` leaves no surviving mutant.
- The original-weight slices above, for Elephant and Mammoth.

## What this does not do yet

- Resizing: images that are not already at an admitted size are refused;
  the processor's bicubic `smart_resize` is not built.
- The full encoder: blocks 1 to 26 run through the same `sv_block`, but only
  block 0 is checked on original weights, and no full 27-block output is
  compared with a reference.
- Splicing the merger's tokens into the text model's embeddings, and
  applying M-RoPE inside the text attention of chapters 10 and 12 (which
  use one position row today). Chapter 9's rendering does not emit the
  vision tokens yet; this chapter's layout takes rendered text token ids.
- Image-bearing decisions on Elephant and then Mammoth, with their own
  reference captures under chapter 17's rules, and a GPU path for them.
- A pinned processor configuration: the profile's processor values come
  from the upstream Qwen3-VL defaults, not from a file in the pinned
  revision.

## Dependencies

- [Chapter 142](ch142-pixels-convolutions-and-patches.md): image admission
  and preprocessing.
- [Chapter 7](ch007-reading-original-weights.md): the rank-5 header inventory
  and tensor views.
- [Chapters 14](ch014-execute-elephant.md) and
  [15](ch015-execute-mammoth.md): the text decision path that the image
  tokens will join.
- [Chapter 9](ch009-text-becomes-a-model-request.md): request rendering and
  special tokens.
- [Chapter 16](ch016-are-the-probabilities-any-good.md) and
  [chapter 17](ch017-qualify-honestly.md): quality measurement and
  qualification rules, for the image-bearing decisions still to come.

## Sources

- [Chapter plan](PLAN.md), Part XIII.
- [Model targets](../docs/targets/model-targets.md).
- The pinned checkpoints' `config.json` files, and the upstream Qwen3-VL
  image processor and vision model, which the oracle follows.
