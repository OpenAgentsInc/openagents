# 48. Integer matrix products

> **Status: partly implemented.** The block format, the quantizer, the row
> kernel, the tiled kernel and the error bound run on the CPU. The device
> kernels (a dp4a row kernel and an int8 tensor-core tile) and a throughput
> measurement on the RTX 4080 are not written yet.

Quantized weights save memory, but the products still need to be correct.
This chapter quantizes activations to 8-bit blocks, multiplies them with
8-bit weight codes, and adds each block's products exactly in a 32-bit
integer. Only then are the scales applied. Because the integer part is exact,
two different kernels must agree on it exactly, and only the quantization and
scaling steps need a tolerance.

The profile is `int8-block-i32-v1`. Everything in it runs on the CPU, written
in Coax.

```shell
coax/bin/coax run psionic/examples/int8-products.coax
coax/bin/coax test psionic/lib/int8-products.coax
coax/bin/coax mutate psionic/lib/int8-products.coax --root-only
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

## What remains

- A device row kernel using `dp4a`, and a tiled kernel using int8 tensor-core
  instructions, through the compute subset of chapter 37 and the admission of
  chapter 68, checked bit for bit against these CPU kernels on the integer
  sums.
- A throughput measurement on the RTX 4080 for both kernels. No speed is
  claimed here.
- A quantized profile never inherits the original weights' evidence: a model
  run through these products needs its own qualification.

## Dependencies

- [Chapter 4](ch004-numbers-and-matrices.md): the float reference product.
- [Chapter 37](ch037-a-coax-compute-subset.md): the device compute subset the
  remaining device kernels will use.
- [Chapter 50](ch050-quantized-weights-and-caches.md) uses this block format
  and these kernels for its weight formats; it builds on this chapter, not the
  other way round.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): 8-bit activation
  blocks, `mmvq` row kernels and `mmq` tiles.
