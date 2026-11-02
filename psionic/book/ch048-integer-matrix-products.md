# 48. Integer matrix products

> **Status: partly implemented.** The block format, the quantizer, the row
> kernel, the tiled kernel and the error bound run on the CPU. Their device
> versions run on the RTX 4080 and equal the CPU kernels bit for bit, with a
> throughput receipt. The `dp4a` and int8 tensor-core instructions, which
> `coax cuda` cannot emit yet, and the speed they would bring, remain.

Quantized weights save memory, but the products still need to be correct.
This chapter quantizes activations to 8-bit blocks, multiplies them with
8-bit weight codes, and adds each block's products exactly in a 32-bit
integer. Only then are the scales applied. Because the integer part is exact,
two different kernels must agree on it exactly, and only the quantization and
scaling steps need a tolerance.

The profile is `int8-block-i32-v1`. It runs on the CPU and, through chapter
37's compute subset, on an NVIDIA GPU, written in Coax.

```shell
coax/bin/coax run psionic/examples/int8-products.coax
coax/bin/coax test psionic/lib/int8-products.coax
coax/bin/coax mutate psionic/lib/int8-products.coax --root-only
coax/bin/coax run psionic/examples/int8-device.coax
COAX_CC=gcc psionic/tests/check_int8_device.sh
```

## The block format

A matrix row of width `n` is split into blocks of `block` values. Chapter 50's
weight formats build on this one, so this chapter defines it:

- **Activations.** Each block has codes in [−127, 127], one f32 scale `d`, and
  the stored sum of its codes. The value a code stands for is `d × q`.
- **Weights.** Each block has codes in [−127, 127], one f32 scale `s`, and one
  f32 offset `m`. The weight a code stands for is `s × q + m`. GGUF's Q8_0
  (chapter 49) has the same shape with `m` = 0, 32 codes per block and an f16
  scale; a format with a minimum per block uses `m`.

The code −128 is never allowed, so the largest product of two codes is
127 × 127 = 16,129 in magnitude.

<!-- source: psionic/lib/int8-products.coax types -->
```jai
Int8Status :: enum u8 {ok;shape;overflow;limit;storage;capacity;code;scale;sum;number;tile;}
Int8Request :: struct {rows:int;width:int;tokens:int;block:int;}
Int8Plan :: struct {status:Int8Status;blocks:int;weight_codes:int;weight_scales:int;activation_codes:int;activation_scales:int;outputs:int;dots:int;}
Int8Result :: struct {status:Int8Status;written:int;}
// Weight value = scale * code + offset, one scale and offset per row block.
Int8Weights :: struct {codes:[]s8;scales:[]f32;offsets:[]f32;}
// Activation value ~ scale * code; sums[b] is the sum of block b's codes.
Int8Activations :: struct {codes:[]s8;scales:[]f32;sums:[]s32;}
```

## Why 32 bits are enough

One block's sum has `block` products of at most 16,129 each. For the usual
block of 32:

```text
32 × 127 × 127 = 516,128 < 2,147,483,647 = 2³¹ − 1
```

so a 32-value block cannot overflow an `s32`, whatever its codes. The same
arithmetic gives the largest safe block: 2,147,483,647 / 16,129 = 133,144.97,
so a block of 133,144 values can reach ±2,147,479,576 and still fits, and a
block of 133,145 is not admitted. The plan refuses any block over that with
`overflow`, before it looks at any data. Across blocks the scales differ, so
blocks are combined in binary64, never in the integer.

The plan also requires a block that is a positive multiple of 4 (the kernels
add products four at a time), a width that is a whole number of blocks, and at
most 1,048,576 multiply-adds, the same work limit as chapter 4.

<!-- source: psionic/lib/int8-products.coax plan -->
```jai
int8_block_fits :: (block:int)->bool {return block<=INT8_S32_MAX/(INT8_CODE_MAX*INT8_CODE_MAX);}
// Work in binary64: exact below 2^53, and far above the limit beyond it.
int8_work_fits :: (rows:int,width:int,tokens:int)->bool {return f64(rows)*f64(width)*f64(tokens)<=f64(INT8_MAC_LIMIT);}
int8_plan :: (r:Int8Request)->Int8Plan #export_c {
    if r.rows<1 || r.tokens<1 || r.block<4 || r.block%4!=0 {return Int8Plan.{status=Int8Status.shape};}
    if !int8_block_fits(r.block) {return Int8Plan.{status=Int8Status.overflow};}
    if r.width<r.block || r.width%r.block!=0 {return Int8Plan.{status=Int8Status.shape};}
    if !int8_work_fits(r.rows,r.width,r.tokens) {return Int8Plan.{status=Int8Status.limit};}
    nb:=r.width/r.block;
    return Int8Plan.{blocks=nb,weight_codes=r.rows*r.width,weight_scales=r.rows*nb,activation_codes=r.tokens*r.width,
        activation_scales=r.tokens*nb,outputs=r.rows*r.tokens,dots=r.rows*r.tokens*nb};
}
```

## Quantizing one block by hand

Take one block of four activations:

```text
x = [1.984375, −0.5, 0.1, 0.0078125]
```

The largest magnitude is 1.984375 = 127/64. The scale is the smallest f32 at
or above amax/127, here exactly `d` = 1/64 = 0.015625. Each code is `x/d`
rounded to the nearest integer, with halves rounded up:

| x | x / d | code |
| --- | --- | --- |
| 1.984375 | 127 | 127 |
| −0.5 | −32 | −32 |
| 0.1 | 6.4 | 6 |
| 0.0078125 | 0.5 | 1 |

The stored sum is 127 − 32 + 6 + 1 = 102. Each code is within half a step of
its value: 0.1 becomes 6/64 = 0.09375, off by 0.00625, less than
`d/2` = 0.0078125.

Rounding the scale up, not to nearest, matters. If `d` were rounded down,
amax/`d` could be a little over 127.5 for a tiny (subnormal) block and round to
128, which is not a code. Rounded up, amax/`d` is at most 127.

<!-- source: psionic/lib/int8-products.coax quantize -->
```jai
int8_finite :: (v:f32)->bool {return (f32_bits(v)&0x7fffffff)<0x7f800000;}
// The smallest f32 at or above amax/127, so amax/scale never exceeds 127.
int8_scale :: (amax:f32)->f32 {
    want:=f64(amax)/127;d:=f32(want);
    if f64(d)<want {d=f32_from_bits(f32_bits(d)+1);}
    return d;
}
// Nearest integer, ties upward.
int8_nearest :: (t:f64)->int {return int(floor(t+0.5));}
int8_quantize :: (r:Int8Request,x:[]f32,out:Int8Activations)->Int8Result #export_c {
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    if x.count!=p.activation_codes {return Int8Result.{status=Int8Status.storage};}
    if out.codes.count<p.activation_codes || out.scales.count<p.activation_scales || out.sums.count<p.activation_scales {return Int8Result.{status=Int8Status.capacity};}
    for v:x {if !int8_finite(v) {return Int8Result.{status=Int8Status.number};}}
    for g:0..<p.activation_scales {
        // Non-negative finite f32 bits order like their values.
        top:=0;
        for k:0..<r.block {v:=cast(int)(f32_bits(x[g*r.block+k])&0x7fffffff);top=cast(int)((f64(top+v)+abs(f64(top-v)))/2);}
        d:=int8_scale(f32_from_bits(cast(u32)top));sum:s32=0;
        for k:0..<r.block {
            q:=0;
            if top!=0 {q=int8_nearest(f64(x[g*r.block+k])/f64(d));}
            out.codes[g*r.block+k]=cast(s8)q;sum+=cast(s32)q;
        }
        out.scales[g]=d;out.sums[g]=sum;
    }
    return Int8Result.{written=p.activation_scales};
}
```

## One block's product by hand

Take a weight block with codes [2, −1, 0, 3], scale `s` = 0.5 and offset
`m` = 0.25. Its weights are [1.25, −0.25, 0.25, 1.75].

The integer sum is

```text
S = 2·127 + (−1)(−32) + 0·6 + 3·1 = 254 + 32 + 0 + 3 = 289
```

The block's contribution is the scaled sum plus the offset times the stored
code sum:

```text
s·d·S + m·d·102 = 0.5 · (1/64) · 289 + 0.25 · (1/64) · 102
                = 2.2578125 + 0.3984375 = 2.65625
```

That is exactly Σ wᵢ · d·qᵢ = 1.25·1.984375 + (−0.25)(−0.5) +
0.25·0.09375 + 1.75·0.015625. The stored sum is what lets the offset be
applied once per block instead of once per value.

Chapter 4's float product of the same weights with the original `x` is
2.6441407 (to f32), so the quantized result is off by about 0.0121. The next
section bounds that before it is measured.

<!-- source: psionic/lib/int8-products.coax dot -->
```jai
// Groups of four products, as one dp4a instruction would add them.
int8_dot4 :: (a:[]s8,at:int,b:[]s8,bt:int)->s32 {
    return cast(s32)a[at]*cast(s32)b[bt]+cast(s32)a[at+1]*cast(s32)b[bt+1]
        +cast(s32)a[at+2]*cast(s32)b[bt+2]+cast(s32)a[at+3]*cast(s32)b[bt+3];
}
int8_dot :: (a:[]s8,ai:int,b:[]s8,bi:int,n:int)->s32 {
    acc:s32=0;
    for g:0..<n/4 {acc+=int8_dot4(a,ai+4*g,b,bi+4*g);}
    return acc;
}
// One block's contribution: d_w d_x S + m d_x sum(x codes), in binary64.
int8_term :: (w:Int8Weights,wb:int,x:Int8Activations,xb:int,dot:s32)->f64 {
    dx:=f64(x.scales[xb]);
    return f64(w.scales[wb])*dx*f64(dot)+f64(w.offsets[wb])*dx*f64(x.sums[xb]);
}
```

`int8_dot4` adds four products the way one `dp4a` instruction would. In Coax
the `s32` additions are checked, so an overflow would trap instead of wrapping;
the bound above says it never happens for an admitted block.

## The error bound, frozen first

The integer sums are exact, so the difference from chapter 4 comes from three
places: quantizing the activations, rounding the weights to f32 for chapter 4,
and the floating-point steps on both sides. For one output,

```text
bound = (Σⱼ |wⱼ| · dⱼ / 2) · (1 + 2⁻⁴⁰) + 2⁻²² · Σⱼ |wⱼ xⱼ|
```

The first term is half a quantization step per activation, weighted by its
weight; the factor `1 + 2⁻⁴⁰` covers the binary64 division `x/d`. The second
term covers the f32 weights, the binary64 combine, and chapter 4's single
narrowing to f32, each at most a few units of 2⁻²⁴ of Σ|w x|.

For the hand block, Σ|w| = 3.5 and `d/2` = 1/128, so the first term is
0.02734375, and Σ|w x| ≈ 2.644 adds about 6.3 × 10⁻⁷. The bound is
0.0273444; the measured error, 0.0121, is under half of it.

`int8_bound` computes this from the f32 weights, the original activations and
the activation scales. The checks and the example call it before running
either product, so the bound cannot be fitted to the result.

<!-- source: psionic/lib/int8-products.coax reference -->
```jai
// Weights as f32 for chapter 4's float product. Refuses values f32 cannot hold.
int8_dequantize :: (r:Int8Request,w:Int8Weights,out:[]f32)->Int8Result #export_c {
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    if w.codes.count!=p.weight_codes || w.scales.count!=p.weight_scales || w.offsets.count!=p.weight_scales {return Int8Result.{status=Int8Status.storage};}
    if out.count<p.weight_codes {return Int8Result.{status=Int8Status.capacity};}
    for c:w.codes {if c<-INT8_CODE_MAX {return Int8Result.{status=Int8Status.code};}}
    for i:0..<p.weight_scales {if !int8_scale_ok(w.scales[i]) || !int8_finite(w.offsets[i]) {return Int8Result.{status=Int8Status.scale};}}
    for i:0..<p.weight_codes {
        b:=i/r.block;
        if !psionic_ops_fits_f32(f64(w.scales[b])*f64(w.codes[i])+f64(w.offsets[b])) {return Int8Result.{status=Int8Status.number};}
    }
    for i:0..<p.weight_codes {b:=i/r.block;out[i]=f32(f64(w.scales[b])*f64(w.codes[i])+f64(w.offsets[b]));}
    return Int8Result.{written=p.weight_codes};
}
// The frozen bound for |quantized product - chapter 4's product|, per output:
// half a step per activation times |w| (quantization), times (1 + 2^-40) for
// the binary64 division, plus 2^-22 sum |w x| for f32 weights, binary64
// combining and chapter 4's one narrowing to f32.
int8_bound :: (r:Int8Request,wf:[]f32,x:[]f32,a:Int8Activations,out:[]f64)->Int8Result #export_c {
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    if wf.count!=p.weight_codes || x.count!=p.activation_codes || a.scales.count!=p.activation_scales {return Int8Result.{status=Int8Status.storage};}
    if out.count<p.outputs {return Int8Result.{status=Int8Status.capacity};}
    for t:0..<r.tokens {for row:0..<r.rows {
        step:=0.0;size:=0.0;
        for k:0..<r.width {
            w:=abs(f64(wf[row*r.width+k]));
            step+=w*f64(a.scales[t*p.blocks+k/r.block])/2;
            size+=w*abs(f64(x[t*r.width+k]));
        }
        out[t*r.rows+row]=step*(1+1.0/1099511627776)+size/4194304;
    }}
    return Int8Result.{written=p.outputs};
}
```

## Admission

Both kernels admit their inputs the same way before writing anything: the
plan, the exact sizes of every input, the capacity of both outputs, no code
equal to −128, every scale finite and not negative, every offset finite, and
every stored sum equal to the sum of its block's codes. A refused call leaves
both outputs unchanged.

<!-- source: psionic/lib/int8-products.coax admit -->
```jai
int8_scale_ok :: (v:f32)->bool {return int8_finite(v) && (f32_bits(v)&0x80000000)==0;}
int8_admit :: (r:Int8Request,w:Int8Weights,x:Int8Activations,dots:[]s32,y:[]f64,tiled:bool)->Int8Plan {
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return p;}
    if tiled && (r.rows%INT8_TILE!=0 || r.tokens%INT8_TILE!=0) {return Int8Plan.{status=Int8Status.tile};}
    if w.codes.count!=p.weight_codes || w.scales.count!=p.weight_scales || w.offsets.count!=p.weight_scales {return Int8Plan.{status=Int8Status.storage};}
    if x.codes.count!=p.activation_codes || x.scales.count!=p.activation_scales || x.sums.count!=p.activation_scales {return Int8Plan.{status=Int8Status.storage};}
    if dots.count<p.dots || y.count<p.outputs {return Int8Plan.{status=Int8Status.capacity};}
    for c:w.codes {if c<-INT8_CODE_MAX {return Int8Plan.{status=Int8Status.code};}}
    for c:x.codes {if c<-INT8_CODE_MAX {return Int8Plan.{status=Int8Status.code};}}
    for i:0..<p.weight_scales {if !int8_scale_ok(w.scales[i]) || !int8_finite(w.offsets[i]) {return Int8Plan.{status=Int8Status.scale};}}
    for i:0..<p.activation_scales {
        if !int8_scale_ok(x.scales[i]) {return Int8Plan.{status=Int8Status.scale};}
        s:=0;
        for k:0..<r.block {s+=cast(int)x.codes[i*r.block+k];}
        if s!=cast(int)x.sums[i] {return Int8Plan.{status=Int8Status.sum};}
    }
    return p;
}
```

## The row kernel

The row kernel is the decode shape: few tokens, so it walks one output row at
a time. For each token and row it computes each block's `s32` sum, records it
in `dots`, and adds that block's term to a binary64 total, in block order.

<!-- source: psionic/lib/int8-products.coax row -->
```jai
// Decode shape: for each token and each output row, walk the row's blocks.
int8_row :: (r:Int8Request,w:Int8Weights,x:Int8Activations,dots:[]s32,y:[]f64)->Int8Result #export_c {
    p:=int8_admit(r,w,x,dots,y,false);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    nb:=p.blocks;
    for t:0..<r.tokens {for row:0..<r.rows {
        acc:f64=0;
        for b:0..<nb {
            s:=int8_dot(w.codes,(row*nb+b)*r.block,x.codes,(t*nb+b)*r.block,r.block);
            dots[(t*r.rows+row)*nb+b]=s;
            acc+=int8_term(w,row*nb+b,x,t*nb+b,s);
        }
        y[t*r.rows+row]=acc;
    }}
    return Int8Result.{written=p.outputs};
}
```

## The tiled kernel

For a whole prompt there are many tokens, and each weight row is used once
per token. The tiled kernel computes a 4 × 4 tile of outputs (four rows times
four tokens) at a time, with sixteen `s32` accumulators per block, so each
group of weight codes serves four tokens before the kernel moves on.

The weights are interleaved first so the reads are contiguous. Within a tile
of four rows and one block, group `g` (four codes) of row `i` sits at offset
16·g + 4·i: one 16-byte read gives all four rows the codes that meet the same
four activation codes. For four rows of eight codes numbered 0–31 by row, the
interleaved order starts 0 1 2 3, 8 9 10 11, 16 17 18 19, 24 25 26 27.

<!-- source: psionic/lib/int8-products.coax tiled -->
```jai
// Interleaved weights: for each tile of four rows and each block, group g of
// row i sits at ((tile*nb + b)*(block/4) + g)*16 + 4*i, so one 16-byte read
// gives four rows the same four activation codes.
int8_at :: (r:Int8Request,row:int,k:int)->int {
    nb:=r.width/r.block;b:=k/r.block;g:=(k%r.block)/4;
    return (((row/INT8_TILE)*nb+b)*(r.block/4)+g)*16+4*(row%INT8_TILE)+k%4;
}
int8_interleave :: (r:Int8Request,codes:[]s8,out:[]s8)->Int8Result #export_c {
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    if r.rows%INT8_TILE!=0 {return Int8Result.{status=Int8Status.tile};}
    if codes.count!=p.weight_codes {return Int8Result.{status=Int8Status.storage};}
    if out.count<p.weight_codes {return Int8Result.{status=Int8Status.capacity};}
    for row:0..<r.rows {for k:0..<r.width {out[int8_at(r,row,k)]=codes[row*r.width+k];}}
    return Int8Result.{written=p.weight_codes};
}
// Prompt shape: a 4 x 4 tile of s32 accumulators per block; the same
// blocks are added to each output in the same order as the row kernel.
int8_tiled :: (r:Int8Request,w:Int8Weights,x:Int8Activations,dots:[]s32,y:[]f64)->Int8Result #export_c {
    p:=int8_admit(r,w,x,dots,y,true);
    if p.status!=Int8Status.ok {return Int8Result.{status=p.status};}
    nb:=p.blocks;
    for i:0..<p.outputs {y[i]=0;}
    for rt:0..<r.rows/INT8_TILE {for tt:0..<r.tokens/INT8_TILE {for b:0..<nb {
        tile:[16]s32;
        for g:0..<r.block/4 {
            base:=((rt*nb+b)*(r.block/4)+g)*16;
            for j:0..<INT8_TILE {
                xa:=((tt*INT8_TILE+j)*nb+b)*r.block+4*g;
                for i:0..<INT8_TILE {tile[4*i+j]+=int8_dot4(w.codes,base+4*i,x.codes,xa);}
            }
        }
        for i:0..<INT8_TILE {for j:0..<INT8_TILE {
            row:=rt*INT8_TILE+i;t:=tt*INT8_TILE+j;s:=tile[4*i+j];
            dots[(t*r.rows+row)*nb+b]=s;
            y[t*r.rows+row]+=int8_term(w,row*nb+b,x,t*nb+b,s);
        }}
    }}}
    return Int8Result.{written=p.outputs};
}
```

The tiled kernel adds the same blocks to each output in the same order as the
row kernel, so the two agree bit for bit on the integer sums and on the
binary64 outputs. It refuses a row or token count that is not a multiple of
4 with `tile`; a caller pads the prompt or uses the row kernel for the tail.

## Run it

The example, [int8-products.coax](../examples/int8-products.coax), runs the
hand block, then an 8 × 64 weight matrix (two blocks of
32 per row) against four tokens through both kernels, then each refusal.

<!-- source: psionic/examples/int8-products.coax example -->
```jai
main :: () {
    // One block of four: the hand trace in the chapter.
    r:=Int8Request.{rows=1,width=4,tokens=1,block=4};
    x:=[4]f32.[1.984375,-0.5,0.1,0.0078125];
    xc:[4]s8;xs:[1]f32;xm:[1]s32;
    a:=Int8Activations.{codes=xc[..],scales=xs[..],sums=xm[..]};
    #assert int8_quantize(r,x[..],a).status==Int8Status.ok;
    print("activation codes % % % %, scale %, sum %\n",xc[0],xc[1],xc[2],xc[3],xs[0],xm[0]);
    wc:=[4]s8.[2,-1,0,3];ws:=[1]f32.[0.5];wo:=[1]f32.[0.25];
    w:=Int8Weights.{codes=wc[..],scales=ws[..],offsets=wo[..]};
    wf:[4]f32;dots:[1]s32;y:[1]f64;bound:[1]f64;ref:[1]f32;
    #assert int8_dequantize(r,w,wf[..]).status==Int8Status.ok;
    print("weights % % % %\n",wf[0],wf[1],wf[2],wf[3]);
    #assert int8_bound(r,wf[..],x[..],a,bound[..]).status==Int8Status.ok;
    #assert int8_row(r,w,a,dots[..],y[..]).status==Int8Status.ok;
    print("integer sum %, quantized product %\n",dots[0],y[0]);
    // Chapter 4 needs two columns; the second token repeats the first.
    two:=Int8Request.{rows=1,width=4,tokens=2,block=4};
    pair:[8]f32;ref2:[2]f32;
    for k:0..<4 {pair[k]=x[k];pair[4+k]=x[k];}
    reference(two,wf[..],pair[..],ref2[..]);ref[0]=ref2[0];
    print("chapter 4 product %, error %, bound %\n",ref[0],abs(y[0]-f64(ref[0])),bound[0]);

    // Eight rows, four tokens, two blocks of 32: row kernel against tiled kernel.
    big:=Int8Request.{rows=8,width=64,tokens=4,block=32};
    bc:[512]s8;bi:[512]s8;bs:[16]f32;bo:[16]f32;bf:[512]f32;bx:[256]f32;
    qc:[256]s8;qs:[8]f32;qm:[8]s32;d1:[64]s32;d2:[64]s32;y1:[32]f64;y2:[32]f64;bb:[32]f64;br:[32]f32;
    for i:0..<512 {bc[i]=cast(s8)((i*37)%255-127);}
    for i:0..<16 {bs[i]=f32(f64(i+1)/512);bo[i]=f32(f64(i%5-2)/64);}
    for i:0..<256 {bx[i]=f32(f64((i*53)%201-100)/37);}
    bw:=Int8Weights.{codes=bc[..],scales=bs[..],offsets=bo[..]};
    q:=Int8Activations.{codes=qc[..],scales=qs[..],sums=qm[..]};
    #assert int8_quantize(big,bx[..],q).status==Int8Status.ok;
    #assert int8_dequantize(big,bw,bf[..]).status==Int8Status.ok;
    #assert int8_bound(big,bf[..],bx[..],q,bb[..]).status==Int8Status.ok;
    #assert int8_row(big,bw,q,d1[..],y1[..]).status==Int8Status.ok;
    #assert int8_interleave(big,bc[..],bi[..]).status==Int8Status.ok;
    #assert int8_tiled(big,Int8Weights.{codes=bi[..],scales=bs[..],offsets=bo[..]},q,d2[..],y2[..]).status==Int8Status.ok;
    same:=true;
    for i:0..<64 {same=same && d1[i]==d2[i];}
    for i:0..<32 {same=same && f64_bits(y1[i])==f64_bits(y2[i]);}
    print("8 x 64 times 64 x 4: 64 block sums, tiled equals row bit for bit: %\n",same);
    reference(big,bf[..],bx[..],br[..]);
    worst:=0.0;within:=true;
    for t:0..<4 {for row:0..<8 {
        e:=abs(y1[t*8+row]-f64(br[row*4+t]));
        if e/bb[t*8+row]>worst {worst=e/bb[t*8+row];}
        within=within && e<=bb[t*8+row];
    }}
    print("every output within its bound: %, largest error / bound: %\n",within,worst);

    // Refusals, each with its typed reason.
    print("block of 6: %\n",int8_plan(Int8Request.{rows=1,width=96,tokens=1,block=6}).status);
    print("block of 133148: %\n",int8_plan(Int8Request.{rows=1,width=133148,tokens=1,block=133148}).status);
    print("1025 x 1024 weights: %\n",int8_plan(Int8Request.{rows=1025,width=1024,tokens=1,block=32}).status);
    wc[1]=-128;print("weight code -128: %\n",int8_row(r,w,a,dots[..],y[..]).status);wc[1]=-1;
    xm[0]=101;print("stored sum 101 for codes summing to 102: %\n",int8_row(r,w,a,dots[..],y[..]).status);xm[0]=102;
    ws[0]=-0.5;print("weight scale -0.5: %\n",int8_row(r,w,a,dots[..],y[..]).status);ws[0]=0.5;
    print("tiled kernel with 1 row: %\n",int8_tiled(r,w,a,dots[..],y[..]).status);
    print("quantized product left unchanged: %\n",y[0]);
}
```

Debug and release builds print the same transcript:

```text
activation codes 127 -32 6 1, scale 0.015625, sum 102
weights 1.25 -0.25 0.25 1.75
integer sum 289, quantized product 2.65625
chapter 4 product 2.6441407, error 0.01210927963256836, bound 0.027344380412275715
8 x 64 times 64 x 4: 64 block sums, tiled equals row bit for bit: true
every output within its bound: true, largest error / bound: 0.2557947414250603
block of 6: shape
block of 133148: overflow
1025 x 1024 weights: limit
weight code -128: code
stored sum 101 for codes summing to 102: sum
weight scale -0.5: scale
tiled kernel with 1 row: tile
quantized product left unchanged: 2.65625
```

The first four lines are the hand calculation above. On the larger product
every output is within its bound; the largest error is about a quarter of its
bound. The refusals each name their reason, and the last line shows that none
of them touched the earlier output.

## On the device

The device kernels are `coax cuda` kernels (chapter 37): each `#export_c`
procedure runs once per work item, and its integer, cast and bounds checks
keep their meaning on the device. They are in
[int8-kernels.coax](../lib/device/int8-kernels.coax). The launch plan, the
admission and the comparisons with the CPU kernels are in
[int8-device.coax](../lib/device/int8-device.coax).

### Four codes to a word

On the device, codes travel four to a `u32`: byte k of word i is code
4i + k. That is how the bytes of an `s8` array already lie in memory, so the
host uploads the same bytes it holds. The hand block's activation codes 127,
−32, 6 and 1 are the bytes 0x7f, 0xe0, 0x06 and 0x01, so their word is
0x0106e07f. The weight codes 2, −1, 0 and 3 make the word 0x0300ff02.

`i8_dp4a` computes what one `dp4a` instruction computes: it sign-extends the
four bytes of each word, multiplies them in pairs and adds the four products
to an accumulator. For the two hand words that is

```text
0 + 2·127 + (−1)(−32) + 0·6 + 3·1 = 289
```

the same sum as the CPU's.

<!-- source: psionic/lib/device/int8-kernels.coax bytes -->
```jai
// Code k (0..3) of a word, sign-extended.
i8_code :: (word:u32,k:int)->s32 {return cast(s32)cast,trunc(s8)(word>>cast(u32)(8*k));}
// What one dp4a instruction computes: acc plus four products of codes.
i8_dp4a :: (a:u32,b:u32,acc:s32)->s32 {
    return acc+i8_code(a,0)*i8_code(b,0)+i8_code(a,1)*i8_code(b,1)+i8_code(a,2)*i8_code(b,2)+i8_code(a,3)*i8_code(b,3);
}
```

`coax cuda` has no `dp4a` primitive yet, so the device compiler sees the
shifts, multiplies and adds of this body, not the single instruction. The sums
are the same; the speed is not, as the receipt below shows.

### The quantizer, the row kernels and the combine

`i8_quantize` quantizes one activation block per work item with the
arithmetic of `int8_quantize`: the same largest-magnitude search over the
bits, the same scale rounded up, the same rounding of each code, written four
codes to a word. A block of zeros gets scale 0 and codes 0 without a
division.

<!-- source: psionic/lib/device/int8-kernels.coax quantize -->
```jai
// int8_scale and int8_nearest of int8-products.coax, for the device.
i8_scale :: (amax:f32)->f32 {
    want:=f64(amax)/127;d:=f32(want);
    if f64(d)<want {d=f32_from_bits(f32_bits(d)+1);}
    return d;
}
// Item i: activation block i (token i/nb, block i%nb), quantized as
// int8_quantize does, four codes to a word.
i8_quantize :: (item:int,block:int,x:[]f32,codes:[]u32,scales:[]f32,sums:[]s32) #export_c {
    at:=item*block;top:=0;
    for k:0..<block {v:=cast(int)(f32_bits(x[at+k])&0x7fffffff);top=cast(int)((f64(top+v)+abs(f64(top-v)))/2);}
    d:=i8_scale(f32_from_bits(cast(u32)top));sum:s32=0;
    for g:0..<block/4 {
        word:u32=0;
        for k:0..<4 {
            q:=0;
            if top!=0 {q=int(floor(f64(x[at+4*g+k])/f64(d)+0.5));}
            word|=(cast,trunc(u32)q&0xff)<<cast(u32)(8*k);sum+=cast(s32)q;
        }
        codes[at/4+g]=word;
    }
    scales[item]=d;sums[item]=sum;
}
```

The decode shape uses two kernels. `i8_dots` computes one block's `s32` sum
per work item, so neighbouring items read neighbouring blocks of one row.
`i8_finish` then adds each output's block terms in binary64, in block order,
with the expression `int8_term` uses. The device rounds each `f64` operation
once, as the C build does (chapter 37), so the outputs equal `int8_row`'s bit
for bit.

<!-- source: psionic/lib/device/int8-kernels.coax dots -->
```jai
// Decode shape. Item (t*rows+row)*nb+b: block b of weight row `row` times
// block b of token t, one word (four codes) at a time. Neighbouring items
// read neighbouring blocks of one row.
i8_dots :: (item:int,rows:int,width:int,block:int,w:[]u32,x:[]u32,dots:[]s32) #export_c {
    nb:=width/block;b:=item%nb;row:=item/nb%rows;t:=item/nb/rows;
    wa:=(row*width+b*block)/4;xa:=(t*width+b*block)/4;acc:s32=0;
    for g:0..<block/4 {acc=i8_dp4a(w[wa+g],x[xa+g],acc);}
    dots[item]=acc;
}
```

<!-- source: psionic/lib/device/int8-kernels.coax finish -->
```jai
// Item t*rows+row: the block terms of int8_term added in binary64 in block
// order, exactly as int8_row adds them.
i8_finish :: (item:int,rows:int,nb:int,ws:[]f32,wo:[]f32,xs:[]f32,xm:[]s32,dots:[]s32,y:[]f64) #export_c {
    row:=item%rows;t:=item/rows;acc:f64=0;
    for b:0..<nb {
        dx:=f64(xs[t*nb+b]);
        acc+=f64(ws[row*nb+b])*dx*f64(dots[item*nb+b])+f64(wo[row*nb+b])*dx*f64(xm[t*nb+b]);
    }
    y[item]=acc;
}
```

### The tensor-core tile

Chapter 37's one tensor-core primitive, `cu_mma_bf16_16x8x16`, multiplies
BF16 values and adds in f32. A code in [−127, 127] has at most 7 significant
bits, so it is exact in BF16, and every product of two codes is an integer of
magnitude at most 16,129. Every partial sum of one block is then an integer of
magnitude at most `block` × 16,129. An f32 holds every integer up to
2²⁴ = 16,777,216, so as long as

```text
block × 16,129 ≤ 16,777,216
```

every addition the tensor core makes is exact, in whatever order it makes
them. The tile steps through a block 16 codes at a time, so the plan admits
blocks that are a multiple of 16 and at most 65 steps long:

```text
1040 × 16,129 = 16,774,160 ≤ 16,777,216    admitted
1056 × 16,129 = 17,032,224 > 16,777,216    refused
```

This is the difference from chapter 69's BF16 products, which round in the
tensor core and need a tolerance: here nothing rounds before the block sum
converts to `s32`, so the tile's block sums equal the row kernel's exactly.

One warp computes 16 rows by 32 tokens (four tiles of 8 tokens). For each
16-code step it converts the weight codes to a BF16 fragment once and uses it
for all four token tiles. After each block it converts the f32 sums to `s32`,
adds the block's terms to binary64 totals in block order, as `i8_finish`
does, and with `record` set writes the block sums too. The activations are
read for the tokens rounded up to a multiple of 8; a ragged tile reads the
padding and writes nothing for it.

<!-- source: psionic/lib/device/int8-kernels.coax mma -->
```jai
I8_TILES :: 4;   // token tiles of 8 per warp
I8_TOKENS :: 32; // tokens per warp
// Two codes of a word (half h: bytes 2h and 2h+1) as a BF16 pair. Every code
// in [-127, 127] is exact in BF16.
i8_bf16 :: (code:s32)->u32 {return f32_bits(f32(code))>>16;}
i8_pair :: (word:u32,h:int)->u32 {return i8_bf16(i8_code(word,2*h))|(i8_bf16(i8_code(word,2*h+1))<<16);}
// One warp: rows r0..r0+15 times tokens t0..t0+31, every block in order.
// Work item group*(rows/16)+rowblock. x holds the tokens rounded up to a
// multiple of 8: a ragged tile reads the padding, whatever it holds, and
// writes nothing for it.
// Every product of two codes and every partial sum of one block is an
// integer of magnitude at most block*127*127, which the plan keeps at or
// below 2^24, so the f32 accumulation is exact in any order and each block's
// sum converts to s32 without loss. After each block the warp adds its terms
// to binary64 totals, in block order, as i8_finish and int8_row do, and
// writes y at the end. With record 1 it also writes every block sum to dots.
i8_mma :: (item:int,rows:int,width:int,tokens:int,block:int,w:[]u32,x:[]u32,ws:[]f32,wo:[]f32,xs:[]f32,xm:[]s32,record:int,dots:[]s32,y:[]f64) #export_c {
    blocks:=rows/16;r0:=item%blocks*16;t0:=item/blocks*I8_TOKENS;
    nb:=width/block;q:=width/4;used:=(tokens-t0+7)/8;
    fa:[128]u32;fb:[64]u32;fc:[128]f32;acc:[512]f32;total:[512]f64;
    for b:0..<nb {
        for lane:0..<32 {for j:0..<16 {acc[lane*16+j]=0;}}
        for s:0..<block/16 {
            c:=(b*block+s*16)/4; // the step's first word
            for lane:0..<32 {
                at:=(r0+lane/4)*q+c+lane%4/2;h:=lane%2;
                fa[lane*4]=i8_pair(w[at],h);fa[lane*4+1]=i8_pair(w[at+8*q],h);
                fa[lane*4+2]=i8_pair(w[at+2],h);fa[lane*4+3]=i8_pair(w[at+8*q+2],h);
            }
            for tile:0..<I8_TILES {
                if tile<used {
                    for lane:0..<32 {
                        xa:=(t0+tile*8+lane/4)*q+c+lane%4/2;
                        fb[lane*2]=i8_pair(x[xa],lane%2);fb[lane*2+1]=i8_pair(x[xa+2],lane%2);
                        for j:0..<4 {fc[lane*4+j]=acc[lane*16+tile*4+j];}
                    }
                    cu_mma_bf16_16x8x16(fa[..],fb[..],fc[..]);
                    for lane:0..<32 {for j:0..<4 {acc[lane*16+tile*4+j]=fc[lane*4+j];}}
                }
            }
        }
        for lane:0..<32 {
            for tile:0..<I8_TILES {
                for j:0..<4 {
                    row:=r0+lane/4+8*(j/2);token:=t0+tile*8+2*(lane%4)+j%2;
                    if token<tokens {
                        dot:=cast(s32)acc[lane*16+tile*4+j];dx:=f64(xs[token*nb+b]);
                        total[lane*16+tile*4+j]+=f64(ws[row*nb+b])*dx*f64(dot)+f64(wo[row*nb+b])*dx*f64(xm[token*nb+b]);
                        if record!=0 {dots[(token*rows+row)*nb+b]=dot;}
                    }
                }
            }
        }
    }
    for lane:0..<32 {
        for tile:0..<I8_TILES {
            for j:0..<4 {
                row:=r0+lane/4+8*(j/2);token:=t0+tile*8+2*(lane%4)+j%2;
                if token<tokens {y[token*rows+row]=total[lane*16+tile*4+j];}
            }
        }
    }
}
```

### The plan

The row kernels are plain threads, launched 256 to a block, and need no
special instruction. The tile is one warp that calls the BF16 tensor-core
instruction, so chapter 68's `da_admit` refuses it on a device before compute
capability 8.0. Shape refusals come before the device, and the work limit is
2³⁶ multiply-adds, enough for the 8192 × 8192 × 128 product below.

For 8192 × 8192 weights, 128 tokens and blocks of 32, each row has 256
blocks. The quantizer has 128 × 256 = 32,768 work items; the row kernels have
8192 × 128 × 256 = 268,435,456 block sums and 8192 × 128 = 1,048,576 outputs;
the tile has 8192 / 16 = 512 row blocks times 128 / 32 = 4 token groups, 2,048
warps, and reads 128 × 8192 / 4 = 262,144 activation words.

<!-- source: psionic/lib/device/int8-device.coax plan -->
```jai
I8Status :: enum u8 {ok;shape;overflow;limit;tile;device;}
I8Kernel :: enum u8 {row;mma;}
// x_words: the activation words the kernels read; the tile reads tokens
// rounded up to a multiple of 8.
I8Plan :: struct {status:I8Status;admission:DaStatus;blocks:int;quantize_items:int;dot_items:int;mma_items:int;finish_items:int;dots:int;x_words:int;}
I8_WORK_LIMIT :: 68719476736; // 2^36 multiply-adds in one product
I8_MMA_STEPS_MAX :: 65;       // 65 steps of 16 codes: 1040 * 127 * 127 = 16,774,160 <= 2^24
// The row kernels are plain threads (launched 256 to a block) with no
// special instruction; i8_mma is one warp on the BF16 tensor cores.
I8_ROW_NEEDS :: DaNeeds.{threads=256,shared=0,mma=false};
I8_MMA_NEEDS :: DaNeeds.{threads=32,shared=0,mma=true};
i8_plan :: (kernel:I8Kernel,rows:int,width:int,tokens:int,block:int,dev:DaDevice)->I8Plan {
    if rows<1 || tokens<1 || block<4 || block%4!=0 {return I8Plan.{status=I8Status.shape};}
    if !int8_block_fits(block) {return I8Plan.{status=I8Status.overflow};}
    if width<block || width%block!=0 {return I8Plan.{status=I8Status.shape};}
    if f64(rows)*f64(width)*f64(tokens)>f64(I8_WORK_LIMIT) {return I8Plan.{status=I8Status.limit};}
    nb:=width/block;needs:=I8_ROW_NEEDS;
    if kernel==I8Kernel.mma {
        if rows%16!=0 || block%16!=0 || block/16>I8_MMA_STEPS_MAX {return I8Plan.{status=I8Status.tile};}
        needs=I8_MMA_NEEDS;
    }
    admitted:=da_admit(needs,dev);
    if admitted!=DaStatus.ok {return I8Plan.{status=I8Status.device,admission=admitted};}
    p:=I8Plan.{blocks=nb,quantize_items=tokens*nb,dot_items=rows*tokens*nb,finish_items=rows*tokens,dots=rows*tokens*nb,x_words=tokens*width/4};
    if kernel==I8Kernel.mma {p.dot_items=0;p.mma_items=(tokens+I8_TOKENS-1)/I8_TOKENS*(rows/16);p.finish_items=0;p.x_words=(tokens+7)/8*8*width/4;}
    return p;
}
i8_text :: (p:I8Plan)->string {
    if p.status==I8Status.ok {return "admitted";}
    if p.status==I8Status.shape {return "refused: the block must be a positive multiple of 4 that divides the width";}
    if p.status==I8Status.overflow {return "refused: a block that long could overflow s32";}
    if p.status==I8Status.limit {return "refused: more than 2^36 multiply-adds";}
    if p.status==I8Status.tile {return "refused: the tensor-core tile needs rows and the block in multiples of 16 and a block of at most 1040";}
    return da_text(p.admission);
}
```

The CPU kernels check their inputs in `int8_admit`, under the CPU plan's work
limit of 1,048,576 multiply-adds. `i8_inputs` makes the same checks for
buffers of device size, so the harness can admit the inputs of the large
products before it uploads them.

<!-- source: psionic/lib/device/int8-device.coax inputs -->
```jai
// What int8_admit checks of the codes, scales and sums, for buffers too large
// for the CPU plan's work limit: no code -128, every scale finite and not
// negative, every offset finite, every stored sum equal to its codes' sum.
i8_inputs :: (rows:int,width:int,tokens:int,block:int,w:Int8Weights,x:Int8Activations)->Int8Status {
    nb:=width/block;
    if w.codes.count!=rows*width || w.scales.count!=rows*nb || w.offsets.count!=rows*nb {return Int8Status.storage;}
    if x.codes.count!=tokens*width || x.scales.count!=tokens*nb || x.sums.count!=tokens*nb {return Int8Status.storage;}
    for c:w.codes {if c<-INT8_CODE_MAX {return Int8Status.code;}}
    for c:x.codes {if c<-INT8_CODE_MAX {return Int8Status.code;}}
    for i:0..<rows*nb {if !int8_scale_ok(w.scales[i]) || !int8_finite(w.offsets[i]) {return Int8Status.scale;}}
    for i:0..<tokens*nb {
        if !int8_scale_ok(x.scales[i]) {return Int8Status.scale;}
        s:=0;
        for k:0..<block {s+=cast(int)x.codes[i*block+k];}
        if s!=cast(int)x.sums[i] {return Int8Status.sum;}
    }
    return Int8Status.ok;
}
```

## Run the device kernels on the CPU

The example, [int8-device.coax](../examples/int8-device.coax), runs the device
kernels' Coax bodies on the CPU: the hand block, then four shapes against
`int8_quantize`, `int8_row` and `int8_tiled`, then the plan's admissions and
refusals, and a stored sum that does not match its codes.

<!-- source: psionic/examples/int8-device.coax example -->
```jai
main :: () {
    // The hand block of chapter 48 through i8_quantize, i8_dots and i8_finish.
    x:=[4]f32.[1.984375,-0.5,0.1,0.0078125];xw:[1]u32;xs:[1]f32;xm:[1]s32;
    i8_quantize(0,4,x[..],xw[..],xs[..],xm[..]);
    print("activation codes % % % %, scale %, sum %\n",i8_code(xw[0],0),i8_code(xw[0],1),i8_code(xw[0],2),i8_code(xw[0],3),xs[0],xm[0]);
    ww:=[1]u32.[0x0300ff02]; // weight codes 2, -1, 0, 3
    print("one dp4a step: %\n",i8_dp4a(ww[0],xw[0],0));
    ws:=[1]f32.[0.5];wo:=[1]f32.[0.25];dots:[1]s32;y:[1]f64;
    i8_dots(0,1,4,4,ww[..],xw[..],dots[..]);
    i8_finish(0,1,1,ws[..],wo[..],xs[..],xm[..],dots[..],y[..]);
    print("block sum %, product %\n",dots[0],y[0]);

    // Every kernel against int8_quantize, int8_row and int8_tiled.
    shapes:=[16]int.[16,64,4,32, 32,64,8,16, 16,48,9,48, 8,96,3,48];
    for n:0..<4 {
        rows:=shapes[4*n];width:=shapes[4*n+1];tokens:=shapes[4*n+2];block:=shapes[4*n+3];
        print("% x % times % tokens, block %: device kernels equal the CPU kernels bit for bit: %\n",rows,width,tokens,block,i8_same_as_cpu(rows,width,tokens,block,n));
    }

    // The plan: shapes first, then the device.
    rtx:=DaDevice.{major=8,minor=9,max_threads=1024,max_shared=49152};
    old:=DaDevice.{major=7,minor=5,max_threads=1024,max_shared=49152};
    print("tile, 8192 x 8192 times 128 tokens on sm_89: %\n",i8_text(i8_plan(I8Kernel.mma,8192,8192,128,32,rtx)));
    print("tile on sm_75: %\n",i8_text(i8_plan(I8Kernel.mma,8192,8192,128,32,old)));
    print("row kernels on sm_75: %\n",i8_text(i8_plan(I8Kernel.row,8192,8192,128,32,old)));
    print("tile, block of 1056: %\n",i8_text(i8_plan(I8Kernel.mma,16,1056,1,1056,rtx)));
    print("row kernels, block of 6: %\n",i8_text(i8_plan(I8Kernel.row,1,96,1,6,rtx)));
    print("row kernels, 4097 x 4096 times 4096 tokens: %\n",i8_text(i8_plan(I8Kernel.row,4097,4096,4096,32,rtx)));

    // A stored sum that does not match its codes.
    codes:=[4]s8.[127,-32,6,1];sums:=[1]s32.[101];wc:=[4]s8.[2,-1,0,3];
    print("stored sum 101 for codes summing to 102: %\n",i8_inputs(1,4,1,4,Int8Weights.{codes=wc[..],scales=ws[..],offsets=wo[..]},Int8Activations.{codes=codes[..],scales=xs[..],sums=sums[..]}));
}
```

Debug and release builds print the same transcript:

```text
activation codes 127 -32 6 1, scale 0.015625, sum 102
one dp4a step: 289
block sum 289, product 2.65625
16 x 64 times 4 tokens, block 32: device kernels equal the CPU kernels bit for bit: true
32 x 64 times 8 tokens, block 16: device kernels equal the CPU kernels bit for bit: true
16 x 48 times 9 tokens, block 48: device kernels equal the CPU kernels bit for bit: true
8 x 96 times 3 tokens, block 48: device kernels equal the CPU kernels bit for bit: true
tile, 8192 x 8192 times 128 tokens on sm_89: admitted
tile on sm_75: refused: cu_mma_bf16_16x8x16 needs compute capability 8.0 or later
row kernels on sm_75: admitted
tile, block of 1056: refused: the tensor-core tile needs rows and the block in multiples of 16 and a block of at most 1040
row kernels, block of 6: refused: the block must be a positive multiple of 4 that divides the width
row kernels, 4097 x 4096 times 4096 tokens: refused: more than 2^36 multiply-adds
stored sum 101 for codes summing to 102: sum
```

The first three lines are the hand trace above. "Bit for bit" means every
code, every scale's bits, every stored sum, every block sum and every output's
bits; for the shapes with 4 rows and 4 tokens it includes `int8_tiled`, and
for rows and blocks in sixteens it includes the tile. The refusals name their
reasons: the tile on a compute capability 7.5 device, a block too long for
exact f32 sums, a block that is not a multiple of 4, too much work, and a
wrong stored sum.

## Run it on the RTX 4080

[check_int8_device.sh](../tests/check_int8_device.sh) runs the checks in
debug and release, emits the CUDA, compares the example transcript, and runs
the [C harness](../tests/int8_device.c) on the device. The harness reports the
device, fills buffers, moves bytes, launches and times; the plan, the
admission, the naive oracle and the statistics are Coax entries of
`int8-device.coax`.

```shell
COAX_CC=gcc psionic/tests/check_int8_device.sh
```

The first part compares the device with the CPU kernels at seven shapes within
the CPU's work limit, byte for byte with `memcmp`: the quantized codes, scales
and sums, the block sums and the binary64 outputs. The second part times
8192 × 8192 weights at decode (1 token) and prefill (32 and 128 tokens): the
two int8 paths, including the quantization of the activations, against
chapter 37's f32 kernels on BF16 weights (`dv_matvec` and `dv_reduce` at
decode, `dv_project` at prefill, the kernels chapter 44 runs Elephant with)
and chapter 37's BF16 tensor-core projection `dw_project`, the instruction
chapter 69 uses. At the large shapes the two int8 paths are compared with each
other bit for bit, the activations pass `i8_inputs`, and every 997th block sum
is compared with the naive oracle.

Each side runs once untimed, then 20 times, each timed alone with CUDA events.
Chapter 34's `cm_two_pass` turns the times into a mean and a standard
deviation. Chapter 34's `cm_compare` refuses to rank sides that compute in
different number formats, and these do on purpose, so `i8_rank` applies the
rest of its rule to the times alone: a side is faster only by more than two
standard errors, and a spread over a tenth of either mean ranks nothing.

<!-- source: psionic/lib/device/int8-device.coax rank -->
```jai
// Chapter 34's rule for two engines' warm run times on one shape: side a is
// faster when its mean is lower by more than two standard errors, and a
// spread over a tenth of either mean ranks nothing. cm_compare also demands
// the same number formats; here the formats differ on purpose (int8 against
// f32 or BF16), so the rule is applied to the times alone.
i8_rank :: (a:CmStats,b:CmStats)->CmCall {
    call:=CmCall.{verdict=CmVerdict.refused};
    if a.status!=CmStatus.ok || b.status!=CmStatus.ok {call.reason=CmReason.stats;return call;}
    call.ratio=b.mean/a.mean;
    call.se=sqrt(a.sd*a.sd/f64(a.n)+b.sd*b.sd/f64(b.n));
    if a.sd*10>a.mean || b.sd*10>b.mean {call.verdict=CmVerdict.noisy;return call;}
    diff:=a.mean-b.mean;call.verdict=CmVerdict.same;
    if diff< -2*call.se {call.verdict=CmVerdict.faster;}
    if diff>2*call.se {call.verdict=CmVerdict.slower;}
    return call;
}
```

The receipt, [int8-device-receipt.txt](../fixtures/int8-device-receipt.txt),
from one run on 2026-11-02:

```text
device: NVIDIA GeForce RTX 4080 (sm_89): row kernels admitted, tensor-core tile admitted
a reported sm_75: tile status 85 (5 + 16*5: needs compute capability 8.0), row kernels status 0
against chapter 48's CPU kernels (int8_quantize, int8_row, int8_tiled), bit for bit:
   256 x 4096 times   1 tokens, block   32: quantize equal; int8_tiled not applicable; i8_dots+i8_finish equal; i8_mma equal
  1024 x 1024 times   1 tokens, block   32: quantize equal; int8_tiled not applicable; i8_dots+i8_finish equal; i8_mma equal
    64 x  512 times  32 tokens, block   32: quantize equal; int8_tiled equals int8_row; i8_dots+i8_finish equal; i8_mma equal
   128 x 1024 times   8 tokens, block   32: quantize equal; int8_tiled equals int8_row; i8_dots+i8_finish equal; i8_mma equal
    32 x  512 times  37 tokens, block   64: quantize equal; int8_tiled not applicable; i8_dots+i8_finish equal; i8_mma equal
    16 x 1040 times  12 tokens, block 1040: quantize equal; int8_tiled equals int8_row; i8_dots+i8_finish equal; i8_mma equal
    12 x  960 times   5 tokens, block   48: quantize equal; int8_tiled not applicable; i8_dots+i8_finish equal; i8_mma not admitted
throughput on the device (chapter 34 statistics: one warm-up, 20 timed runs per side):
  decode: 8192 x 8192 weights, 1 token, block 32
    int8 outputs: i8_mma equals i8_dots+i8_finish bit for bit: yes; activations pass input admission: yes; 2104 sampled block sums equal the naive oracle: yes
    int8 i8_dots+i8_finish (dp4a-shaped)     331.9 us  sd    2.9  (n=20)     202 GB/s of weights      202 G multiply-adds/s
    int8 i8_mma (BF16 tensor cores)        704.0 us  sd    3.9  (n=20)      95 GB/s of weights       95 G multiply-adds/s
    f32 dv_matvec+dv_reduce (ch. 37/44)     243.5 us  sd    1.2  (n=20)     551 GB/s of weights      276 G multiply-adds/s
    BF16-weight MMA dw_project (ch. 37)    1716.0 us  sd    7.5  (n=20)      78 GB/s of weights       39 G multiply-adds/s
    int8 i8_dots+i8_finish (dp4a-shaped) against f32 dv_matvec+dv_reduce (ch. 37/44): 0.73x, slower
    int8 i8_dots+i8_finish (dp4a-shaped) against BF16-weight MMA dw_project (ch. 37): 5.17x, faster
    int8 i8_mma (BF16 tensor cores) against int8 i8_dots+i8_finish (dp4a-shaped): 0.47x, slower
  prefill: 8192 x 8192 weights, 32 tokens, block 32
    int8 outputs: i8_mma equals i8_dots+i8_finish bit for bit: yes; activations pass input admission: yes; 67311 sampled block sums equal the naive oracle: yes
    int8 i8_dots+i8_finish (dp4a-shaped)    6674.5 us  sd   30.0  (n=20)      10 GB/s of weights      322 G multiply-adds/s
    int8 i8_mma (BF16 tensor cores)       3625.5 us  sd    3.5  (n=20)      19 GB/s of weights      592 G multiply-adds/s
    f32 dv_project (ch. 37/44)            3390.6 us  sd   45.2  (n=20)      40 GB/s of weights      633 G multiply-adds/s
    BF16-weight MMA dw_project (ch. 37)    1652.4 us  sd    9.0  (n=20)      81 GB/s of weights     1300 G multiply-adds/s
    int8 i8_mma (BF16 tensor cores) against f32 dv_project (ch. 37/44): 0.94x, slower
    int8 i8_mma (BF16 tensor cores) against BF16-weight MMA dw_project (ch. 37): 0.46x, slower
    int8 i8_mma (BF16 tensor cores) against int8 i8_dots+i8_finish (dp4a-shaped): 1.84x, faster
  prefill: 8192 x 8192 weights, 128 tokens, block 32
    int8 outputs: i8_mma equals i8_dots+i8_finish bit for bit: yes; activations pass input admission: yes; 269244 sampled block sums equal the naive oracle: yes
    int8 i8_dots+i8_finish (dp4a-shaped)   25114.5 us  sd  119.7  (n=20)       3 GB/s of weights      342 G multiply-adds/s
    int8 i8_mma (BF16 tensor cores)      13143.6 us  sd  237.3  (n=20)       5 GB/s of weights      654 G multiply-adds/s
    f32 dv_project (ch. 37/44)            4204.4 us  sd  123.0  (n=20)      32 GB/s of weights     2043 G multiply-adds/s
    BF16-weight MMA dw_project (ch. 37)    1974.6 us  sd    6.8  (n=20)      68 GB/s of weights     4350 G multiply-adds/s
    int8 i8_mma (BF16 tensor cores) against f32 dv_project (ch. 37/44): 0.32x, slower
    int8 i8_mma (BF16 tensor cores) against BF16-weight MMA dw_project (ch. 37): 0.15x, slower
    int8 i8_mma (BF16 tensor cores) against int8 i8_dots+i8_finish (dp4a-shaped): 1.91x, faster
chapter 48 on CUDA: every device block sum and output equals the CPU kernels bit for bit
```

The integer contract holds on the device: every block sum and every output
equals the CPU kernels, and the tile is refused on a compute capability 7.5
report. The speed does not yet justify the format:

- At decode, the int8 row kernels read 64 MB of weights in 332 µs (202 GB/s);
  chapter 37's f32 kernels read 128 MB of BF16 weights in 244 µs (551 GB/s).
  int8 is 0.73 times as fast, though it moves half the bytes.
- At prefill, the tile is 1.84 times the row kernels at 32 tokens and 1.91
  times at 128, but 0.94 and 0.32 times chapter 37's f32 `dv_project`, and
  0.46 and 0.15 times its BF16 tensor-core `dw_project`. The 32-token
  comparison with `dv_project` is close: a second run of the same check
  measured 3,341 µs against 3,356 µs, the same within two standard errors.

Three costs explain it. Without `dp4a`, each code is extracted, sign-extended
and multiplied by ordinary instructions. Without an int8 tensor-core
instruction, the tile converts every code to BF16 in its inner loop, and each
BF16 instruction covers 16 codes of a row where the `s8` instruction
(m16n8k32) covers 32. And both paths add every block's term in binary64, which the RTX 4080 runs at
a small fraction of its f32 rate. These are measurements of these kernels on
one card, not limits of the format.

## What the checks compare

The checks in [int8-products.coax](../lib/int8-products.coax) were written
before the kernels. They compare:

- the overflow arithmetic above, and a block of 133,144 codes that reaches
  ±2,147,479,576 without trapping;
- each plan's buffer counts and each shape, `overflow` and `limit` refusal,
  including the work limit at exactly 1,048,576 and 1,048,577;
- the hand quantization, and the scale rounding on subnormal, normal and
  largest-f32 blocks;
- the half-step error of every value, as a `#forall` over 64 random blocks;
- the grouped `s32` dot against a naive loop that adds one 64-bit product at a
  time, as a `#forall` over 384 cases;
- the row kernel's block sums against the naive loop, and its outputs, with
  power-of-two scales, against a plain binary64 sum that is exact for those
  values;
- the interleaved layout against a hand table;
- the tiled kernel against the row kernel, bit for bit on sums and outputs,
  over 18 shapes and seeds;
- the quantized product against chapter 4's float product, within the bound
  computed first, over 8 seeds, and the bound against a hand value;
- every refusal, with both outputs unchanged afterwards.

The naive oracle and the fixtures live in
[int8-products-checks.coax](../tests/int8-products-checks.coax), apart from
the code they check. The library is an `#authority` file. `coax mutate
--root-only` (chapter 4's product checks itself) leaves zero survivors of 369
mutants.

The device kernels' checks in
[int8-kernels.coax](../lib/device/int8-kernels.coax) compare byte extraction,
one `dp4a` step and the BF16 pairs with hand values; two hand-quantized blocks
of eight; the row kernels' block sums against a naive 64-bit loop and their
outputs against an exact binary64 sum over five shapes; the tile's block sums
and outputs against the same oracle over seven shapes, including ragged token
tiles and two token groups, with views exactly as long as the plan says so a
tile too many traps; and the largest admitted block, 1040 codes of ±127.
The checks in [int8-device.coax](../lib/device/int8-device.coax) run every
device kernel's Coax body against `int8_quantize`, `int8_row` and
`int8_tiled`, bit for bit, over 8 shapes and 2 seeds each; every plan count
and refusal; `i8_inputs` against `int8_admit` and each of its refusals; and
the statistics and rank against chapter 34's hand values. The fixtures and the
comparison live in [int8-device-checks.coax](../tests/int8-device-checks.coax)
and [int8-device-plan-checks.coax](../tests/int8-device-plan-checks.coax).
Both files are `#authority` files: `coax mutate --root-only` leaves zero
survivors of 358 mutants in `int8-kernels.coax` and of 207 in
`int8-device.coax`.

## What remains

- `dp4a` (compute capability 6.1) and an int8 tensor-core instruction
  (`mma.sync` m16n8k32 with `s8` inputs and `s32` accumulation, compute
  capability 8.0) as device primitives of `coax cuda`, with their CPU meaning, in the way chapter 37 added
  `cu_mma_bf16_16x8x16`. That is a compiler change. The row kernel and the
  tile would then call them, and chapter 68's admission would state the
  capability each needs.
- Kernels that beat chapter 37's f32 kernels at decode and prefill on the
  RTX 4080. No speedup is claimed here.
- A quantized profile never inherits the original weights' evidence: a model
  run through these products needs its own qualification.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the float reference product.
- [Chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md): the warm-run
  statistics and the rule for ranking two sides.
- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset,
  the BF16 tensor-core primitive and the f32 and BF16 kernels timed here.
- [Chapter 68](ch068-coax-kernels-cooperate.md): device capability admission.
- [Chapter 50](ch050-quantized-weights-and-caches.md) uses this block format
  and these kernels for its weight formats; it builds on this chapter, not the
  other way round.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): 8-bit activation
  blocks, `mmvq` row kernels and `mmq` tiles.
