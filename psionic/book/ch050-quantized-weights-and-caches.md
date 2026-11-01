# 50. Quantized weights and caches

> **Status: partly implemented.** Q8_0 and Q4_0 weight blocks, an
> importance-weighted Q4_0 quantizer, an 8-bit KV cache and the quality card
> run on the CPU, measured on a small stand-in decision model. K-quant
> profiles, MXFP4 as a profile, perplexity on fixed text, a product evaluation
> and cards measured on Elephant's original weights are not done yet.

Elephant's BF16 weights are 18.8 GB, and the RTX 4080 has 16 GB. A format
with fewer bits per weight lets the whole model stay on the GPU, but every bit
removed changes the answers a little. This chapter adds two block formats from
GGUF, a quantizer for them, an 8-bit cache for attention keys and values, and
a quality card that measures each derived profile against the original before
it is used.

Each derived format is its own profile with its own evidence. A quantized
model never inherits the original model's measurements: the card measures it
again, and it is promoted only when the measured loss is within a bound
declared before the measurement.

Everything here runs on the CPU, written in Coax.

```shell
coax/bin/coax run psionic/examples/quantized.coax
coax/bin/coax test psionic/lib/quantized.coax
coax/bin/coax mutate psionic/lib/quantized.coax --root-only
```

## What the bits buy

Elephant's 18.8 GB of BF16 weights are 2 bytes each, so about 9.4 billion
weights. The two formats here store 32 weights per block with one 2-byte
scale:

| Format | Bytes per block of 32 | Bits per weight | 9.4 billion weights |
| --- | --- | --- | --- |
| BF16 | 64 | 16 | 18.8 GB |
| Q8_0 | 2 + 32 = 34 | 8.5 | 9.4 × 34 / 32 ≈ 9.99 GB |
| Q4_0 | 2 + 16 = 18 | 4.5 | 9.4 × 18 / 32 ≈ 5.29 GB |

If every tensor were stored as Q8_0, the weights would fit in 16 GB with about
6 GB left for the cache and activations. That is arithmetic on the sizes, not
a measured run: this chapter does not load Elephant.

The library's results are typed. `QzBound` is the declared loss and `QzCard`
the measured one:

<!-- source: psionic/lib/quantized.coax types -->
```jai
QzStatus :: enum u8 {ok;shape;partial;limit;kind;storage;capacity;scale;number;code;measure;}
QzPlan :: struct {status:QzStatus;blocks:int;bytes:int;values:int;block_bytes:int;}
QzResult :: struct {status:QzStatus;written:int;}
// The loss a derived profile may cost, fixed before anything is measured.
QzBound :: struct {kl:f64;agreement:f64;}
QzCard :: struct {status:QzStatus;items:int;bytes:int;agreement:f64;agreement_low:f64;kl:f64;kl_high:f64;
    perplexity_original:f64;perplexity_quantized:f64;original:PqReport;quantized:PqReport;promoted:bool;}
```

## Half precision by hand

Both formats store the block scale as an IEEE half: 1 sign bit, 5 exponent
bits and 10 fraction bits. Take `0x2c00`:

```text
0x2c00 = 0 01011 0000000000
sign 0, exponent 11, fraction 0
value = (1024 + 0) × 2^(11 − 25) = 2^−4 = 0.0625
```

Exponent 0 is subnormal (`fraction × 2⁻²⁴`), and exponent 31 is infinity or
NaN. `qz_half` decodes the bits exactly in binary64, and `qz_half_ok` refuses
exponent 31.

The quantizer needs the opposite direction: the smallest half at or above a
wanted scale. Non-negative halves order like their bit patterns, so a binary
search over the bits finds it without any rounding question. Above 65,504,
the largest finite half, it returns `0x7c00`, infinity, and the quantizer
refuses such blocks first.

<!-- source: psionic/lib/quantized.coax half -->
```jai
qz_larger :: (a:f64,b:f64)->f64 {return ifx b>a then b else a;}
// 2^e for e >= -25, by exact doublings.
qz_pow2 :: (e:int)->f64 {v:=1.0/33554432;for i:0..<e+25 {v*=2;}return v;}
// IEEE half precision: 1 sign bit, 5 exponent bits, 10 fraction bits.
qz_half_ok :: (bits:int)->bool {return (bits&0x7c00)!=0x7c00;}
qz_half :: (bits:int)->f64 {
    e:=(bits>>10)&31;m:=bits&1023;
    v:=ifx e==0 then f64(m)*qz_pow2(-24) else f64(1024+m)*qz_pow2(e-25);
    return ifx (bits&0x8000)!=0 then -v else v;
}
// The smallest non-negative half at or above want, or 0x7c00 (infinity)
// above 65504. Non-negative halves order like their bits: a binary search.
qz_half_up :: (want:f64)->int {
    lo:=0;hi:=0x7c00;
    while lo<hi {mid:=(lo+hi)/2;if qz_half(mid)>=want {hi=mid;} else {lo=mid+1;}}
    return lo;
}
```

## The block layouts

A row of `width` weights is cut into blocks of 32. A width that is not a
whole number of blocks is refused with `partial`: GGUF never splits a block
across rows, and neither does the decoder. The plan also refuses a format
other than GGUF types 8 (Q8_0) and 2 (Q4_0) with `kind`, and more than 32,768
blocks (1,048,576 values) per call with `limit`.

<!-- source: psionic/lib/quantized.coax plan -->
```jai
qz_plan :: (kind:int,rows:int,width:int)->QzPlan #export_c {
    if kind!=QZ_Q8_0 && kind!=QZ_Q4_0 {return QzPlan.{status=QzStatus.kind};}
    if rows<1 || width<1 {return QzPlan.{status=QzStatus.shape};}
    if width%QZ_BLOCK!=0 {return QzPlan.{status=QzStatus.partial};}
    if rows>QZ_BLOCKS_MAX/(width/QZ_BLOCK) {return QzPlan.{status=QzStatus.limit};}
    blocks:=rows*width/QZ_BLOCK;size:=ifx kind==QZ_Q8_0 then 34 else 18;
    return QzPlan.{blocks=blocks,bytes=blocks*size,values=rows*width,block_bytes=size};
}
```

**Q8_0** is 34 bytes: the half scale `d`, then 32 signed bytes. Element `k`
is `d × code[k]`. Here is a hand-built block with scale `0x2c00`:

| Byte | Code | Value |
| --- | --- | --- |
| 2: 127 | 127 | 127 / 16 = 7.9375 |
| 3: 129 | −127 | −7.9375 |
| 4: 1 | 1 | 0.0625 |
| 5: 128 | −128 | −8 |
| 33: 16 | 16 | 1 |

GGUF allows the code −128, so the decoder accepts it. Chapter 48's kernels do
not, and the conversion to them refuses it below.

**Q4_0** is 18 bytes: the half scale `d`, then 16 bytes of 4-bit codes. Byte
`j` holds element `j` in its low nibble and element `j + 16` in its high
nibble, and element `k` is `d × (code − 8)`.

A half times a code needs at most 11 + 8 = 19 significant bits, so every
decoded value is exact in f32. `qz_tensor` decodes a tensor described by
chapter 49's reader, using the reader's type, shape and byte range, so a
tensor of any other type is refused with `kind`.

<!-- source: psionic/lib/quantized.coax decode -->
```jai
qz_signed :: (b:u8)->int {v:=cast(int)b;return ifx v>127 then v-256 else v;}
qz_scale_bits :: (bytes:[]u8,block:int,size:int)->int {return cast(int)bytes[block*size]|cast(int)bytes[block*size+1]<<8;}
// Q8_0 element k: d * code; Q4_0 element k: d * (nibble - 8), elements
// 0..15 in the low nibbles and 16..31 in the high nibbles.
qz_value :: (kind:int,bytes:[]u8,block:int,k:int)->f64 {
    d:=qz_half(qz_scale_bits(bytes,block,ifx kind==QZ_Q8_0 then 34 else 18));
    if kind==QZ_Q8_0 {return d*f64(qz_signed(bytes[block*34+2+k]));}
    packed:=cast(int)bytes[block*18+2+k%16];
    return d*f64(((packed>>(4*(k/16)))&15)-8);
}
qz_decode :: (kind:int,rows:int,width:int,bytes:[]u8,out:[]f32)->QzResult #export_c {
    p:=qz_plan(kind,rows,width);
    if p.status!=QzStatus.ok {return QzResult.{status=p.status};}
    if bytes.count!=p.bytes {return QzResult.{status=QzStatus.storage};}
    if out.count<p.values {return QzResult.{status=QzStatus.capacity};}
    for b:0..<p.blocks {if !qz_half_ok(qz_scale_bits(bytes,b,p.block_bytes)) {return QzResult.{status=QzStatus.scale};}}
    // A half times a code needs at most 19 significant bits: f32 holds it exactly.
    for i:0..<p.values {out[i]=f32(qz_value(kind,bytes,i/QZ_BLOCK,i%QZ_BLOCK));}
    return QzResult.{written=p.values};
}
// One tensor described by chapter 49's reader, decoded from the file bytes.
qz_tensor :: (t:PgTensor,file:[]u8,out:[]f32)->QzResult #export_c {
    if t.dims[0]<1 {return QzResult.{status=QzStatus.shape};}
    if t.begin<0 || t.end>file.count {return QzResult.{status=QzStatus.storage};}
    return qz_decode(t.kind,t.elements/t.dims[0],t.dims[0],file[t.begin..<t.end],out);
}
```

## Quantizing one Q4_0 block by hand

Take a block whose nonzero values are

```text
x[0] = −4, x[1] = 1, x[2] = 3.9, x[3] = 0.24, x[4] = 0.25, x[16] = 3.6
```

The value of largest magnitude is −4. Q4_0 maps it to code 0, the value −8 ×
`d`, so `d` = −4 / −8 = 0.5, a half with bits `0x3800`. Its sign is the
opposite of the extreme value's sign. Each code is `x/d` rounded to nearest,
halves upward, plus 8, clamped to 0..15:

| x | x / d | code | decoded | error |
| --- | --- | --- | --- | --- |
| −4 | −8 | 0 | −4 | 0 |
| 1 | 2 | 10 | 1 | 0 |
| 3.9 | 7.8 → 8 | 15 (clamped) | 3.5 | 0.4 |
| 0.24 | 0.48 → 0 | 8 | 0 | 0.24 |
| 0.25 | 0.5 → 1 | 9 | 0.5 | 0.25 |
| 3.6 | 7.2 → 7 | 15 | 3.5 | 0.1 |

The first packed byte holds element 0 (code 0) low and element 16 (code 15)
high: 0 + 15 × 16 = 240. The next is 10 + 8 × 16 = 138, then 15 + 8 × 16 =
143. Element 2 shows why Q4_0 is coarse: the positive side only reaches
7 × `d`, so 3.9 loses 0.4, almost a whole step.

The scale is rounded up to a half, never down, so `|x / d|` never passes 8 and
the extreme value always gets code 0. For the same reason the Q8_0 scale is
the smallest half at or above amax / 127, and no Q8_0 code passes ±127.

## Importance-weighted quantization

Plain Q4_0 always divides the extreme value by 8. That is not always the best
scale: a smaller step fits the common values better at the cost of clipping
the extreme one. Which matters more depends on the inputs the weights meet.

An importance matrix records that. For each input column `j` it holds the
mean of `x_j²` over calibration inputs; a weight that meets large inputs costs
more when it is wrong. With importances, the quantizer tries plain Q4_0 first
and then 17 more divisors, 7, 7.125, ..., 9, and keeps the scale with the
least weighted squared error Σ importance × (x − decoded)². Plain Q4_0 is
first and wins ties, so the weighted error is never worse than plain Q4_0's,
and with all importances zero the result is plain Q4_0 exactly.

<!-- source: psionic/lib/quantized.coax quantize -->
```jai
qz_bad :: (v:f32)->bool {return !int8_finite(v);}
// Q4_0 for one block at scale d (d != 0): the weighted squared error, and the
// codes nearest x/d + 8, clamped to 0..15 (max and min by |t| identities).
qz_q4_try :: (x:[]f32,at:int,imp:[]f32,d:f64,codes:[]int)->f64 {
    err:=0.0;
    for k:0..<QZ_BLOCK {
        t:=f64(int8_nearest(f64(x[at+k])/d));
        t=(t+8+abs(t+8))/2-8;t=7-(7-t+abs(7-t))/2;
        codes[k]=int(t)+8;e:=f64(x[at+k])-d*t;
        err+=(ifx imp.count==0 then 1.0 else f64(imp[(at+k)%imp.count]))*e*e;
    }
    return err;
}
// The signed half scale: |top| / divisor rounded up, its sign opposite to top's.
qz_q4_scale :: (top:f32,divisor:f64)->int {return qz_half_up(abs(f64(top))/divisor)|(cast(int)(f32_bits(top)>>16)&0x8000^0x8000);}
qz_put :: (out:[]u8,at:int,bits:int) {out[at]=cast(u8)(bits&255);out[at+1]=cast(u8)(bits>>8);}
qz_block :: (kind:int,x:[]f32,at:int,imp:[]f32,out:[]u8,base:int) {
    // top: the value of largest magnitude, the first one on a tie.
    top:f32=0;
    for k:0..<QZ_BLOCK {if abs(x[at+k])>abs(top) {top=x[at+k];}}
    if top==0 {
        // Scale 0; Q8_0 codes 0, Q4_0 nibbles 8.
        qz_put(out,base,0);
        for k:2..<ifx kind==QZ_Q8_0 then 34 else 18 {out[base+k]=ifx kind==QZ_Q8_0 then 0 else 0x88;}
        return;
    }
    if kind==QZ_Q8_0 {
        bits:=qz_half_up(abs(f64(top))/127);d:=qz_half(bits);qz_put(out,base,bits);
        for k:0..<QZ_BLOCK {out[base+2+k]=cast(u8)(int8_nearest(f64(x[at+k])/d)&255);}
        return;
    }
    // Plain Q4_0 divides by 8. With importances, also try divisors 7, 7.125,
    // ..., 9, and keep the first with the least weighted error, so plain wins ties.
    codes:[32]int;best:[32]int;least:=-1.0;
    for i:0..<ifx imp.count==0 then 1 else QZ_CANDIDATES+1 {
        b:=qz_q4_scale(top,ifx i==0 then 8.0 else 7+f64(i-1)/8);
        err:=qz_q4_try(x,at,imp,qz_half(b),codes[..]);
        if least<0 || err<least {least=err;qz_put(out,base,b);for k:0..<QZ_BLOCK {best[k]=codes[k];}}
    }
    for j:0..<16 {out[base+2+j]=cast(u8)(best[j]|best[j+16]<<4);}
}
// importance: empty, or one non-negative weight per column (an imatrix row).
qz_quantize :: (kind:int,rows:int,width:int,x:[]f32,importance:[]f32,out:[]u8)->QzResult #export_c {
    p:=qz_plan(kind,rows,width);
    if p.status!=QzStatus.ok {return QzResult.{status=p.status};}
    if x.count!=p.values || (importance.count!=0 && importance.count!=width) {return QzResult.{status=QzStatus.storage};}
    if out.count<p.bytes {return QzResult.{status=QzStatus.capacity};}
    for v:importance {if qz_bad(v) || v<0 {return QzResult.{status=QzStatus.number};}}
    // The largest scale is |x| / 127 for Q8_0 and |x| / 7 for Q4_0's search.
    limit:=f64(QZ_HALF_MAX)*(ifx kind==QZ_Q8_0 then 127.0 else ifx importance.count==0 then 8.0 else 7.0);
    for v:x {if qz_bad(v) || abs(f64(v))>limit {return QzResult.{status=QzStatus.number};}}
    for b:0..<p.blocks {qz_block(kind,x,b*QZ_BLOCK,importance,out,b*p.block_bytes);}
    return QzResult.{written=p.blocks};
}
```

The clamp to 0..15 uses `max(t, −8) = (t + 8 + |t + 8|) / 2 − 8` and the
matching identity for the minimum, so the codes come from arithmetic, not from
branches. A refused call (a non-finite value, a negative importance, or a
scale above the largest half) writes nothing.

## Into chapter 48's kernels

A Q8_0 block is chapter 48's weight format with offset 0 and an f16 scale, so
`qz_int8` copies the codes and widens the scale to f32. It refuses a negative
or non-finite scale with `scale` and the code −128 with `code`, the two things
chapter 48 does not admit, before it writes anything.

<!-- source: psionic/lib/quantized.coax int8 -->
```jai
// Q8_0 as chapter 48's weights: the same codes, the f16 scale as f32, offset 0.
qz_int8 :: (rows:int,width:int,bytes:[]u8,w:Int8Weights)->QzResult #export_c {
    p:=qz_plan(QZ_Q8_0,rows,width);
    if p.status!=QzStatus.ok {return QzResult.{status=p.status};}
    if bytes.count!=p.bytes {return QzResult.{status=QzStatus.storage};}
    if w.codes.count<p.values || w.scales.count<p.blocks || w.offsets.count<p.blocks {return QzResult.{status=QzStatus.capacity};}
    for b:0..<p.blocks {
        s:=qz_scale_bits(bytes,b,34);
        if !qz_half_ok(s) || s>=0x8000 {return QzResult.{status=QzStatus.scale};}
        for k:0..<QZ_BLOCK {if bytes[b*34+2+k]==128 {return QzResult.{status=QzStatus.code};}}
    }
    for b:0..<p.blocks {
        w.scales[b]=f32(qz_half(qz_scale_bits(bytes,b,34)));w.offsets[b]=0;
        for k:0..<QZ_BLOCK {w.codes[b*QZ_BLOCK+k]=cast(s8)qz_signed(bytes[b*34+2+k]);}
    }
    return QzResult.{written=p.blocks};
}
```

## An 8-bit KV cache

During decoding, every earlier position's key and value rows stay in memory.
The `int8-kv-v1` profile stores each row of head width `d` as one chapter 48
block: codes in [−127, 127], an f32 scale rounded up from amax / 127, and the
code sum. For 40 positions of width 32:

```text
f32:  2 × 40 × 32 × 4        = 10,240 bytes
int8: 2 × 40 × (32 + 4)      =  2,880 bytes (plus chapter 48's code sums, unused here)
```

`qz_kv_attend` runs one decode query against the whole cache, decoding each
code as it reads it, with chapter 41's scores scaled by 1/√d.

### The bound, frozen first

Quantizing moves each key element by at most half its row's step, `dk/2`, so
each score moves by at most

```text
e = max over positions of Σᵢ |qᵢ| · dk / 2 / √d
```

Shifting every logit by at most `e` changes each softmax probability by a
factor between e^(−2e) and e^(2e), so the probabilities move by at most
e^(2e) − 1 in total. Each value moves by at most `dv/2`. So each output moves
by at most

```text
(e^(2e) − 1) · max|v'| + max dv / 2
```

plus 2⁻²⁰ · max|v'| for the two f32 narrowings and the binary64 sums.
`qz_kv_bound` computes this from the query, the original values and the
stored scales before either attention runs.

<!-- source: psionic/lib/quantized.coax kv -->
```jai
// int8-kv-v1: each cached key and value row (head width d) is one chapter 48
// block: codes in [-127, 127] and an f32 scale rounded up from amax/127.
qz_kv_request :: (d:int,t:int)->Int8Request {return Int8Request.{rows=1,width=d,tokens=t,block=d};}
qz_kv_store :: (d:int,t:int,k:[]f32,v:[]f32,keys:Int8Activations,values:Int8Activations)->QzResult #export_c {
    r:=qz_kv_request(d,t);
    p:=int8_plan(r);
    if p.status!=Int8Status.ok {return QzResult.{status=QzStatus.shape};}
    if k.count!=p.activation_codes || v.count!=p.activation_codes {return QzResult.{status=QzStatus.storage};}
    for x:k {if qz_bad(x) {return QzResult.{status=QzStatus.number};}}
    for x:v {if qz_bad(x) {return QzResult.{status=QzStatus.number};}}
    for a:[2]Int8Activations.[keys,values] {
        if a.codes.count<p.activation_codes || a.scales.count<t || a.sums.count<t {return QzResult.{status=QzStatus.capacity};}
    }
    a:=int8_quantize(r,k,keys);b:=int8_quantize(r,v,values);
    return QzResult.{written=a.written+b.written};
}
qz_kv_admit :: (d:int,t:int,q:[]f32,keys:Int8Activations,values:Int8Activations,scores:[]f64,out:[]f32)->QzStatus {
    if int8_plan(qz_kv_request(d,t)).status!=Int8Status.ok {return QzStatus.shape;}
    if q.count!=d || keys.codes.count!=t*d || values.codes.count!=t*d || keys.scales.count!=t || values.scales.count!=t {return QzStatus.storage;}
    if scores.count<t || out.count<d {return QzStatus.capacity;}
    for x:q {if qz_bad(x) {return QzStatus.number;}}
    for c:keys.codes {if c==-128 {return QzStatus.code;}}
    for c:values.codes {if c==-128 {return QzStatus.code;}}
    for i:0..<t {if !int8_scale_ok(keys.scales[i]) || !int8_scale_ok(values.scales[i]) {return QzStatus.scale;}}
    return QzStatus.ok;
}
qz_kv_at :: (a:Int8Activations,d:int,i:int)->f64 {return f64(a.scales[i/d])*f64(a.codes[i]);}
// One decode query against every cached position, scores scaled by 1/sqrt(d)
// as in chapter 41, in binary64, each cached code decoded as it is read.
qz_kv_attend :: (d:int,t:int,q:[]f32,keys:Int8Activations,values:Int8Activations,scores:[]f64,out:[]f32)->QzResult #export_c {
    status:=qz_kv_admit(d,t,q,keys,values,scores,out);
    if status!=QzStatus.ok {return QzResult.{status=status};}
    for j:0..<t {
        s:=0.0;
        for i:0..<d {s+=f64(q[i])*qz_kv_at(keys,d,j*d+i);}
        scores[j]=s/sqrt(f64(d));
    }
    m:=scores[0];for j:1..<t {m=qz_larger(m,scores[j]);}
    total:=0.0;
    for j:0..<t {scores[j]=exp(scores[j]-m);total+=scores[j];}
    for i:0..<d {
        o:=0.0;
        for j:0..<t {o+=scores[j]*qz_kv_at(values,d,j*d+i);}
        out[i]=f32(o/total);
    }
    return QzResult.{written=d};
}
// The bound on |int8 attention - f32 attention| for every output, frozen
// from the inputs before either runs. Each score moves by at most
// e = max_j sum_i |q_i| dk_j / 2 / sqrt(d), so each probability moves by a
// factor within exp(+-2e); each value moves by at most dv_j / 2. Then
// |o' - o| <= (exp(2e) - 1) max|v'| + max_j dv_j / 2, plus 2^-20 max|v'|
// for two f32 narrowings and the binary64 sums.
qz_kv_bound :: (d:int,t:int,q:[]f32,v:[]f32,keys:Int8Activations,values:Int8Activations)->f64 #export_c {
    norm:=0.0;for x:q {norm+=abs(f64(x));}
    e:=0.0;step:=0.0;top:=0.0;
    for j:0..<t {
        e=qz_larger(e,norm*f64(keys.scales[j])/2/sqrt(f64(d)));
        step=qz_larger(step,f64(values.scales[j])/2);
        for i:0..<d {top=qz_larger(top,abs(f64(v[j*d+i]))+f64(values.scales[j])/2);}
    }
    slack:=1+1.0/1099511627776;
    return (exp(2*e*slack)-1)*top+step*slack+top/1048576;
}
```

For a zero query no score moves, so a single cached position gives back its
decoded value, and the bound is half a value step plus the slack. With the
value row [1.984375, −0.5, 0.1, 0.0078125] the step is 1/64, so the output
is [1.984375, −0.5, 0.09375, 0.015625] and the bound is 1/128 + 1.9921875 ×
2⁻²⁰ ≈ 0.0078144.

## The quality card

A card compares a derived profile with the original on the same items. For
each item it takes both models' probabilities over the options and records:

- the KL divergence KL(original ‖ derived) = Σ p log(p / q);
- whether the top option (the first, on ties) agrees;
- the negative log-probability of the item's gold answer under each model,
  which gives a perplexity, exp(mean);
- chapter 16's decision metrics (correct answers, Brier score, calibration
  error) for both models.

A deterministic run has no noise from one run to the next, but the items are
still a sample of the requests the model will see. So the card reports the
mean KL with two standard errors above it, and the agreement with two
standard errors below it, and promotes the profile only if

```text
mean KL + 2 SE ≤ declared KL bound   and   agreement − 2 SE ≥ declared agreement
```

The bound is a constant fixed before any card runs. A card measured on fewer
than two items, or on no bytes, is refused with `measure` and is never
promoted; an invalid distribution, or a gold answer the original gives
probability 0, is refused with `number`.

### A card by hand

Four items with two options each. The original gives [0.5, 0.5] to items 0,
2 and 3, and [0.25, 0.75] to item 1. The derived model gives [0.5, 0.5] to
all four. The gold answers are options 0, 1, 0, 0.

- KL is 0 for items 0, 2 and 3. For item 1 it is 0.25 log(0.5) +
  0.75 log(1.5) ≈ 0.13081. The mean is 0.13081 / 4 ≈ 0.03270.
- The per-item KLs are 0, 0.13081, 0, 0: their sample variance is
  0.13081² / 4, so the standard error is 0.13081 / 4, and the pessimistic KL
  is 3 × 0.13081 / 4 ≈ 0.09811.
- On item 1 the derived model ties, and a tie picks option 0, so 3 of 4 top
  options agree: agreement 0.75, standard error √(0.75 × 0.25 / 4) ≈ 0.2165,
  pessimistic agreement 0.75 − 0.4330 = 0.3170.
- The derived model gives every gold answer 0.5, so its perplexity is 2. The
  original's is exp((3 log 2 + log(4/3)) / 4) ≈ 1.8072.

Against a declared bound of KL 0.0981 or more and agreement 0.317 or less,
this profile is promoted; a bound a millionth tighter on either refuses it.
The checks compare all of these.

<!-- source: psionic/lib/quantized.coax card -->
```jai
// Measures a derived profile against the original on the same items: mean
// KL(original || derived) and top-1 agreement, each with two standard errors,
// perplexity on the items' gold answers, and chapter 16's metrics for both.
// Promotes only when the pessimistic ends are within the declared bound.
qz_card :: (bound:QzBound,items:[]PqItem,original:[]f64,derived:[]f64,bytes:int)->QzCard #export_c {
    if !(bound.kl>=0 && bound.kl<1e300) || !(bound.agreement>=0 && bound.agreement<=1) {return QzCard.{status=QzStatus.number};}
    if items.count<2 || bytes<1 {return QzCard.{status=QzStatus.measure};}
    if original.count!=derived.count {return QzCard.{status=QzStatus.storage};}
    a:=pq_measure(items,original);b:=pq_measure(items,derived);
    if a.status==PqStatus.shape {return QzCard.{status=QzStatus.shape};}
    if a.status!=PqStatus.ok || b.status!=PqStatus.ok {return QzCard.{status=QzStatus.number};}
    n:=f64(items.count);kl:=0.0;kl2:=0.0;agree:=0.0;nll_a:=0.0;nll_b:=0.0;
    for item:items {
        p:=original[item.start..<item.start+item.count];q:=derived[item.start..<item.start+item.count];
        one:=0.0;
        for i:0..<item.count {
            if p[i]>0 {
                if q[i]<=0 {return QzCard.{status=QzStatus.number};}
                one+=p[i]*log(p[i]/q[i]);
            }
        }
        // q[gold] > 0 already: p[gold] > 0 is checked here, so the KL term refused q[gold] = 0.
        if p[item.gold]<=0 {return QzCard.{status=QzStatus.number};}
        kl+=one;kl2+=one*one;nll_a-=log(p[item.gold]);nll_b-=log(q[item.gold]);
        if pq_first_max(p)==pq_first_max(q) {agree+=1;}
    }
    c:=QzCard.{items=items.count,bytes=bytes,original=a,quantized=b};
    c.kl=kl/n;c.agreement=agree/n;
    // Sample variance of the per-item KL, and the binomial variance of agreement.
    c.kl_high=c.kl+2*sqrt(qz_larger(0.0,kl2-n*c.kl*c.kl)/(n-1)/n);
    c.agreement_low=c.agreement-2*sqrt(c.agreement*(1-c.agreement)/n);
    c.perplexity_original=exp(nll_a/n);c.perplexity_quantized=exp(nll_b/n);
    c.promoted=c.kl_high<=bound.kl && c.agreement_low>=bound.agreement;
    return c;
}
```

## Run it

The example, [quantized.coax](../examples/quantized.coax), decodes the hand
blocks, decodes a Q4_0 tensor found by chapter 49's reader in a GGUF file,
runs round trips, compares the KV cache with chapter 41, prints a card for
four profiles of the stand-in model, and then shows each refusal.

The stand-in, in [quantized-checks.coax](../tests/quantized-checks.coax), is
a small decision model: 64 inputs, a ReLU layer of 32 and a readout over 16
options, with seeded weights in which one weight in thirteen is six times
larger, and inputs in which four columns in sixteen are four times louder. Its
importance matrix comes from 64 calibration inputs, and its card from 64
other items whose gold answers are drawn from the original model's own
probabilities. It stands in for Elephant, whose original weights this chapter
does not run.

<!-- source: psionic/examples/quantized.coax example -->
```jai
main :: () {
    // A hand-built Q8_0 block: scale 0x2c00 = 1/16, codes 127, -127, 1, -128.
    b8:[34]u8;b8[1]=0x2c;b8[2]=127;b8[3]=129;b8[4]=1;b8[5]=128;b8[33]=16;y:[32]f32;
    #assert qz_decode(QZ_Q8_0,1,32,b8[..],y[..]).status==QzStatus.ok;
    print("Q8_0 block: % % % % ... %\n",y[0],y[1],y[2],y[3],y[31]);

    // The hand Q4_0 block: -4 has the largest magnitude, so d = -4 / -8 = 0.5.
    x:[32]f32;x[0]=-4;x[1]=1;x[2]=3.9;x[3]=0.24;x[4]=0.25;x[16]=3.6;b4:[18]u8;
    #assert qz_quantize(QZ_Q4_0,1,32,x[..],x[0..<0],b4[..]).status==QzStatus.ok;
    #assert qz_decode(QZ_Q4_0,1,32,b4[..],y[..]).status==QzStatus.ok;
    print("Q4_0 scale %, packed bytes % % %, values % % % % % and %\n",
        qz_half(qz_scale_bits(b4[..],0,18)),b4[2],b4[3],b4[4],y[0],y[1],y[2],y[3],y[4],y[16]);

    // Chapter 49 reads the file's descriptors; this chapter decodes the payload.
    file:=PG_OWNED;m:[15]PgMeta;ms:[15]PgMeta;t:[2]PgTensor;ts:[2]PgTensor;
    #assert pg_read(file[..],file.count,m[..],ms[..],t[..],ts[..],false).status==PgStatus.ok;
    for i:0..<18 {file[t[1].begin+i]=b4[i];}
    w:[64]f32;
    r:=qz_tensor(t[1],file[..],w[..]);
    print("GGUF tensor '%' type % (% x %): %, first values % % %\n",t[1].name,t[1].kind,t[1].dims[0],t[1].dims[1],r.status,w[0],w[1],w[2]);
    print("GGUF tensor '%' type % read as a quantized block: %\n",t[0].name,t[0].kind,qz_tensor(t[0],file[..],w[..]).status);

    // Round trips of a 4 x 64 matrix against bounds fixed before decoding.
    big:[256]f32;back:[256]f32;bytes:[272]u8;qz_fill(big[..],5,0);
    for kind:[2]int.[QZ_Q8_0,QZ_Q4_0] {
        p:=qz_plan(kind,4,64);
        #assert qz_quantize(kind,4,64,big[..],big[0..<0],bytes[0..<p.bytes]).status==QzStatus.ok;
        #assert qz_decode(kind,4,64,bytes[0..<p.bytes],back[..]).status==QzStatus.ok;
        worst:=0.0;
        for i:0..<256 {
            d:=abs(qz_half(qz_scale_bits(bytes[0..<p.bytes],i/32,p.block_bytes)));
            step:=ifx kind==QZ_Q8_0 then d/2 else d;
            worst=qz_larger(worst,abs(f64(big[i])-f64(back[i]))/step);
        }
        print("% values in % bytes (% bits each): largest error / bound %\n",p.values,p.bytes,f64(p.bytes*8)/f64(p.values),worst);
    }

    // The int8 KV cache: 40 positions of width 32, against chapter 41's f32 attention.
    q:[32]f32;k:[1280]f32;v:[1280]f32;qz_fill(q[..],1,0);qz_fill(k[..],2,0);qz_fill(v[..],3,0);
    kc:[1280]s8;ks:[40]f32;km:[40]s32;vc:[1280]s8;vs:[40]f32;vm:[40]s32;
    keys:=Int8Activations.{codes=kc[..],scales=ks[..],sums=km[..]};values:=Int8Activations.{codes=vc[..],scales=vs[..],sums=vm[..]};
    #assert qz_kv_store(32,40,k[..],v[..],keys,values).status==QzStatus.ok;
    bound:=qz_kv_bound(32,40,q[..],v[..],keys,values);
    out:[32]f32;ref:[32]f32;scores:[40]f64;
    #assert qz_kv_attend(32,40,q[..],keys,values,scores[..],out[..]).status==QzStatus.ok;
    qz_reference(32,40,q[..],k[..],v[..],ref[..]);
    worst:=0.0;for i:0..<32 {worst=qz_larger(worst,abs(f64(out[i])-f64(ref[i])));}
    print("KV cache: % bytes as f32, % as int8; bound %, largest error %\n",2*40*32*4,2*40*(32+4),bound,worst);

    // The quality card: each profile against the original, bound declared first.
    bound_kl:=qz_declared().kl;bound_agree:=qz_declared().agreement;
    print("declared: KL + 2 SE <= %, agreement - 2 SE >= %; original f32 weights: % bytes\n",bound_kl,bound_agree,(QZ_HIDDEN*QZ_IN+QZ_OPTIONS*QZ_HIDDEN)*4);
    names:=[4]string.["Q8_0","Q4_0","Q4_0 + imatrix","mix"];
    for profile:0..<4 {
        c:=qz_standin_card(profile);
        print("%: % bytes, agreement % (low %), KL % (high %), perplexity % -> %, Brier % -> %, correct % -> %, promoted %\n",
            names[profile],c.bytes,c.agreement,c.agreement_low,c.kl,c.kl_high,c.perplexity_original,c.perplexity_quantized,
            c.original.brier,c.quantized.brier,c.original.correct,c.quantized.correct,c.promoted);
    }

    // Refusals, each with its typed reason; none of them writes its output.
    y[0]=-7;
    print("a row of 48 values: %\n",qz_decode(QZ_Q8_0,1,48,b8[..],y[..]).status);
    print("GGUF type 3 (Q4_1): %\n",qz_decode(3,1,32,b4[..],y[..]).status);
    print("a Q8_0 block one byte short: %\n",qz_decode(QZ_Q8_0,1,32,b8[0..<33],y[..]).status);
    b8[0]=0x00;b8[1]=0x7e;print("a Q8_0 block whose scale is NaN: %\n",qz_decode(QZ_Q8_0,1,32,b8[..],y[..]).status);b8[1]=0x2c;
    wc:[32]s8;wsc:[1]f32;wo:[1]f32;
    print("code -128 into chapter 48's kernels: %\n",qz_int8(1,32,b8[..],Int8Weights.{codes=wc[..],scales=wsc[..],offsets=wo[..]}).status);
    items:[1]PqItem;items[0]=PqItem.{kind=QrKind.choice,count=2,start=0,gold=0,abstain=-1};one:=[2]f64.[0.5,0.5];
    print("a card measured on one item: %\n",qz_card(qz_declared(),items[..],one[..],one[..],34).status);
    print("output left unchanged: %\n",y[0]);
}
```

Debug and release builds print the same transcript:

```text
Q8_0 block: 7.9375 -7.9375 0.0625 -8.0 ... 1.0
Q4_0 scale 0.5, packed bytes 240 138 143, values -4.0 1.0 3.5 0.0 0.5 and 3.5
GGUF tensor 'quants' type 2 (32 x 2): ok, first values -4.0 1.0 3.5
GGUF tensor 'matrix' type 0 read as a quantized block: kind
256 values in 272 bytes (8.5 bits each): largest error / bound 0.9947713414634146
256 values in 144 bytes (4.5 bits each): largest error / bound 0.918127669462881
KV cache: 10240 bytes as f32, 2880 as int8; bound 0.024005489764143365, largest error 0.0006980597972869873
declared: KL + 2 SE <= 0.01, agreement - 2 SE >= 0.9; original f32 weights: 10240 bytes
Q8_0: 2720 bytes, agreement 1.0 (low 1.0), KL 0.00024416445920068925 (high 0.0002952508403386442), perplexity 3.395443037495561 -> 3.4032991132398447, Brier 0.5552702201875571 -> 0.5568834103015776, correct 37 -> 37, promoted true
Q4_0: 1440 bytes, agreement 0.859375 (low 0.7724664239793477), KL 0.057507185525802876 (high 0.06984262922602616), perplexity 3.395443037495561 -> 3.5089490854629446, Brier 0.5552702201875571 -> 0.5728581294841917, correct 37 -> 36, promoted false
Q4_0 + imatrix: 1440 bytes, agreement 0.84375 (low 0.7529769528232637), KL 0.04507884708212467 (high 0.05592216045609964), perplexity 3.395443037495561 -> 3.4046271721331363, Brier 0.5552702201875571 -> 0.5515279400343417, correct 37 -> 41, promoted false
mix: 1696 bytes, agreement 0.875 (low 0.7923202715292316), KL 0.03906635474139014 (high 0.049837036414141764), perplexity 3.395443037495561 -> 3.4483737640093772, Brier 0.5552702201875571 -> 0.5648069763836394, correct 37 -> 39, promoted false
a row of 48 values: partial
GGUF type 3 (Q4_1): kind
a Q8_0 block one byte short: storage
a Q8_0 block whose scale is NaN: scale
code -128 into chapter 48's kernels: code
a card measured on one item: measure
output left unchanged: -7.0
```

Reading it:

- The first four lines are the hand calculations above, and the reader's
  `quants` tensor decodes to the hand block that was written into its payload.
  The `matrix` tensor is F32 (type 0), so it is refused with `kind`.
- In the round trips the largest Q8_0 error is 0.995 of half a step, and the
  largest Q4_0 error 0.918 of one step, each within its bound.
- The KV cache is 3.6 times smaller. Its largest error, 0.0007, is far below
  the bound, 0.024: the bound assumes every score moves the worst way at once.
- Q8_0 keeps every top answer on this stand-in and costs a mean KL of 0.00024,
  so it is promoted under the declared bound (KL 0.01, agreement 0.9).
- The 4-bit profiles are not promoted: their pessimistic KL is 0.050 to 0.070,
  above 0.01. The importance matrix lowers plain Q4_0's mean KL from 0.0575
  to 0.0451 at the same size, and the mix (first layer Q4_0 with importances,
  readout Q8_0) lowers it to 0.0391 for 256 more bytes. Neither is close to
  the bound. The perplexity, Brier score and correct count each move only a
  little for every profile; on 64 items the extra correct answers of the
  importance profile are noise, not a gain.
- Each refusal names its reason, and the last line shows that none wrote the
  output.

## What the checks compare

The checks in [quantized.coax](../lib/quantized.coax) were written before the
code. They compare:

- half-precision values against hand values, including subnormals and the
  largest finite half, and rounding up as a `#forall` over 124 bit patterns;
- each plan's sizes and each refusal, including the limit at exactly 32,768
  blocks;
- the hand Q8_0 and Q4_0 blocks against values worked out from the GGUF
  layout, and a tensor read by chapter 49 from a GGUF file;
- Q8_0 round trips within half a step and Q4_0 within one step (two with
  importances), as `#forall`s over 48 blocks each, with the scale against
  amax / 127 or amax / 8;
- that importances never make the weighted error worse, that all-zero
  importances give plain Q4_0, and that the chosen scale has the least error
  among the 17 divisors, enumerated independently;
- one Q4_0 trial by hand, with both clamps;
- Q8_0 converted to chapter 48's weights against the decoded values;
- the int8 KV cache against chapter 41's f32 attention within the bound
  computed first, over 24 shapes and seeds, plus two hand cases: one cached
  position, and a dominant score of 1,024 that would overflow `exp` if the
  largest score were not subtracted;
- the quality card against the hand card above, and each card refusal;
- the stand-in's four cards: Q8_0 promoted, the 4-bit profiles not, and the
  importance matrix better than plain Q4_0.

The library is an `#authority` file, because it decides which profiles are
promoted. `coax mutate --root-only` (the units it loads check themselves)
leaves zero survivors of 495 mutants.

## What remains

- K-quant profiles (Q4_K, Q6_K): their quantizers, with importances and
  per-tensor mixes. Chapter 51 adds their decoders.
- Chapter 60's MXFP4 as a candidate profile on the same card.
- Perplexity on fixed text with a language model, and a product evaluation,
  on the card next to the decision metrics.
- Cards measured on Elephant's original weights, which needs the GPU path;
  the CPU stand-in here only shows the method.
- The 8-bit KV cache inside a full model's multi-head attention, and the
  device kernels that would read it.

## Dependencies

- [Chapter 16](ch016-are-the-probabilities-any-good.md): decision quality
  metrics.
- [Chapter 41](ch041-attention-in-tiles.md): the f32 attention the KV cache is
  compared with.
- [Chapter 48](ch048-integer-matrix-products.md): the 8-bit blocks, their
  rounding, and the kernels Q8_0 feeds.
- [Chapter 49](ch049-read-a-gguf-file.md): reading published quantized files.
- [Chapter 60](ch060-original-mxfp4-weights.md): the MXFP4 decoder, a profile
  still to add.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): Q8_0, K-quants,
  `imatrix` and the quality card. The survey cites an upstream Q8_0 figure of
  mean KL about 0.0014 on Llama-3-8B; that is an upstream claim, not measured
  here.
- The [GGUF specification](https://github.com/ggml-org/ggml/blob/facdc44a67da17df4433adcc468b424f48e9ded2/docs/gguf.md),
  pinned by chapter 49, for the type numbers and block sizes.
