# 82. Original FP8 weights

> **Status: partly implemented.** The decoder, its file admission and the
> activation profile run on the CPU against constructed files. The first
> consumers' real headers are not pinned yet, and the activation profile has
> not been compared with the reference implementation's outputs.

Several Mistral releases ship their original weights as FP8 E4M3 bytes, with
one scale for each 128 × 128 block of the matrix. Mistral Large 4 uses
power-of-two (UE8M0) scales; other FP8 checkpoints use a 32-bit float scale.
This chapter decodes both forms exactly, admits the weight and its scale grid
from a safetensors file, and refuses every input it cannot decode exactly.

The decoder writes binary64 values. Every E4M3 code times every admitted scale
fits in binary64 without rounding, so the output is the original number, not
an approximation of it. Narrowing those values to BF16 or F32, or re-encoding
them as a smaller format, is a derived profile (chapter 50) and is not done
here. The decoder's profile is `fp8-e4m3-block128-f64-v1`.

```shell
coax/bin/coax run psionic/examples/original-fp8.coax
coax/bin/coax test psionic/lib/fp8.coax
coax/bin/coax test psionic/lib/fp8-file.coax
python3 -I psionic/tests/fp8_oracle.py
```

## Which bytes?

The [Mistral target survey](../docs/targets/mistral.md) lists
Ministral-3-14B-Instruct-2512 and Devstral-Small-2-24B-Instruct-2512 as FP8 E4M3
block-scaled weights, and Large 4 as FP8 with 128 × 128 blocks and UE8M0
scales. No header from those checkpoints is pinned in this repository yet.
The files in this chapter are constructed: owned bytes in the safetensors
layout, not extracted checkpoint tensors. Confirming each checkpoint's tensor
names, scale dtype and block size from its pinned header is the first open
item of this chapter.

A safetensors file starts with an 8-byte little-endian header length, then a
JSON header, then the payload. Each header entry names a tensor and gives its
dtype, shape and byte span in the payload. This chapter admits four dtypes:

| Dtype | Bytes per value | Meaning here |
| --- | --- | --- |
| `F8_E4M3` | 1 | The weight codes |
| `F32` | 4 | A float scale per block (a `weight_scale_inv` tensor) |
| `F8_E8M0` | 1 | A UE8M0 power-of-two scale per block |
| `BF16` | 2 | Other tensors that may share the file |

`F8_E5M2` and every other dtype are refused with `dtype`. The scale is a
multiplier: the stored weight is code value × scale, even when the tensor is
named `_scale_inv`.

## One byte, one number

E4M3 has one sign bit, four exponent bits with bias 7, and three fraction
bits. There is no infinity. The two codes with every exponent and fraction bit
set, 0x7F and 0xFF, are NaN. Every other code is finite:

| Code | Fields | Value |
| --- | --- | --- |
| 0x00, 0x80 | exponent 0, fraction 0 | +0 and −0 |
| 0x01 | exponent 0, fraction 1 | 1 × 2⁻⁹ = 0.001953125, the smallest subnormal |
| 0x07 | exponent 0, fraction 7 | 7 × 2⁻⁹, the largest subnormal |
| 0x08 | exponent 1, fraction 0 | 2⁻⁶, the smallest normal |
| 0x38 | exponent 7, fraction 0 | 1 |
| 0x3C | exponent 7, fraction 4 | 1.5 |
| 0x7E | exponent 15, fraction 6 | 448, the largest |
| 0xFE | exponent 15, fraction 6 | −448 |
| 0x7F, 0xFF | exponent 15, fraction 7 | NaN |

Both cases fit one formula. With exponent `e` and fraction `f`, a subnormal
(`e` = 0) is `f × 2⁻⁹`, and a normal value is `(8 + f) × 2^(e − 10)`. For
0xFE: `e` = 15, `f` = 6, so the magnitude is 14 × 2⁵ = 448 and the sign bit
makes it −448.

The decoder builds the power of two directly from binary64 exponent bits.
Binary64's bias is 1023, so `2^(e − 10)` has the exponent field `e + 1013`, and
2⁻⁹ has the field 1014. The sign is copied as a bit, so 0x80 decodes to −0, not
+0:

<!-- source: psionic/lib/fp8.coax unit -->
```jai
fp8_nan :: (code:u8)->bool {return (code&127)==127;}
fp8_unit :: (code:u8)->f64 {
    exponent:=cast(u64)(code>>3)&15;fraction:=f64(code&7);
    magnitude:=fraction*f64_from_bits(1014<<52);
    if exponent!=0 {magnitude=(8+fraction)*f64_from_bits((exponent+1013)<<52);}
    return f64_from_bits(f64_bits(magnitude)|((cast(u64)code&128)<<56));
}
```

## One scale per block

A UE8M0 scale byte `s` means `2^(s − 127)`. Byte 127 is 1, byte 131 is 16, and
byte 0 is 2⁻¹²⁷. Byte 0xFF is reserved for NaN, so it is refused. Its binary64
exponent field is `s + 896`. A float scale is four little-endian bytes. Only a
positive, finite float is a scale: zero, −0, negative numbers, infinities and
NaNs are refused. Subnormal float scales are admitted, because their products
are still exact.

<!-- source: psionic/lib/fp8.coax scale -->
```jai
fp8_power :: (scale:u8)->f64 {return f64_from_bits((cast(u64)scale+896)<<52);}
fp8_word :: (scales:[]u8,block:int)->u32 {
    bits:u32=0;
    for i:0..<4 {bits|=cast(u32)scales[4*block+i]<<(8*i);}
    return bits;
}
fp8_f32_scale_ok :: (bits:u32)->bool {return bits!=0 && bits<0x7f800000;}
fp8_scale_ok :: (kind:u8,scales:[]u8,block:int)->bool {
    if kind==FP8_SCALE_UE8M0 {return scales[block]!=255;}
    return fp8_f32_scale_ok(fp8_word(scales,block));
}
fp8_scale :: (kind:u8,scales:[]u8,block:int)->f64 {
    if kind==FP8_SCALE_UE8M0 {return fp8_power(scales[block]);}
    return f64(f32_from_bits(fp8_word(scales,block)));
}
```

Why is every product exact? A code value has at most 4 significant bits and a
float scale at most 24, so their product needs at most 28 of binary64's 53
bits. The smallest product, 2⁻⁹ × 2⁻¹⁴⁹, and the largest, 448 × the largest
float, are both far inside binary64's normal range. UE8M0 scales add only an
exponent, from −127 to 127.

The block grid is 128 × 128. A weight with `rows` × `columns` values has
`rows/128` × `columns/128` scales, stored row-major, so value `(r, c)` uses
scale `(r/128) × (columns/128) + c/128`. The profile refuses a partial block:
both dimensions must be positive multiples of 128. One decode window holds at
most 64 blocks (1,048,576 values, 8 MiB of output); the bound is checked by
division before any multiplication:

<!-- source: psionic/lib/fp8.coax plan -->
```jai
fp8_plan :: (rows:int,columns:int,kind:u8)->Fp8Plan #export_c {
    if kind>FP8_SCALE_UE8M0 {return Fp8Plan.{status=Fp8Status.shape};}
    if rows<FP8_BLOCK || columns<FP8_BLOCK || rows%FP8_BLOCK!=0 || columns%FP8_BLOCK!=0 {return Fp8Plan.{status=Fp8Status.shape};}
    if rows/FP8_BLOCK>FP8_BLOCKS_MAX/(columns/FP8_BLOCK) {return Fp8Plan.{status=Fp8Status.limit};}
    blocks:=(rows/FP8_BLOCK)*(columns/FP8_BLOCK);width:=4;
    if kind==FP8_SCALE_UE8M0 {width=1;}
    return Fp8Plan.{blocks=blocks,scale_bytes=blocks*width,values=rows*columns};
}
```

## Decode, then write

Here is a hand trace of the example's file. The weight is 128 × 256, so it has
two blocks side by side. Block 0's scale is the float 0.5 (bytes
`00 00 00 3F`) and block 1's is 3.0 (bytes `00 00 40 40`). Row 0 starts with
the same four codes in each block:

| Code | Value | × 0.5 (block 0) | × 3 (block 1) | × 2⁴ (UE8M0 byte 131) |
| --- | --- | --- | --- | --- |
| 0x38 | 1 | 0.5 | 3 | 16 |
| 0x3C | 1.5 | 0.75 | 4.5 | 24 |
| 0xFE | −448 | −224 | −1344 | −7168 |
| 0x01 | 2⁻⁹ | 2⁻¹⁰ = 0.0009765625 | 0.005859375 | 2⁻⁵ |

The public decoder checks the shape, the byte counts, the output capacity,
aliasing, every scale and every code before it writes anything. A refusal
leaves the whole destination unchanged:

<!-- source: psionic/lib/fp8.coax decode -->
```jai
fp8_decode :: (rows:int,columns:int,kind:u8,data:[]u8,scales:[]u8,out:[]f64,overlap:bool)->Fp8Result #export_c {
    p:=fp8_plan(rows,columns,kind);if p.status!=Fp8Status.ok {return Fp8Result.{status=p.status};}
    if data.count!=p.values || scales.count!=p.scale_bytes {return Fp8Result.{status=Fp8Status.storage};}
    if out.count<p.values {return Fp8Result.{status=Fp8Status.capacity};}
    if overlap {return Fp8Result.{status=Fp8Status.alias};}
    for b:0..<p.blocks {if !fp8_scale_ok(kind,scales,b) {return Fp8Result.{status=Fp8Status.scale};}}
    for code:data {if fp8_nan(code) {return Fp8Result.{status=Fp8Status.code};}}
    per_row:=columns/FP8_BLOCK;
    for i:0..<p.values {
        block:=(i/columns/FP8_BLOCK)*per_row+(i%columns)/FP8_BLOCK;
        out[i]=fp8_unit(data[i])*fp8_scale(kind,scales,block);
    }
    return Fp8Result.{written=p.values};
}
```

A NaN code is refused with `code`, even though the file format can store it:
a NaN weight is never a valid original value in this profile.

## Admit the pair from a file

`fp8_pair` reads a safetensors header and returns the byte spans of one named
F8_E4M3 weight and its named scale tensor. It refuses, with a typed status:

- `offsets`: the file ends before its length prefix or header, a span runs
  past the payload (a truncated file), spans overlap, or they leave payload
  bytes uncovered;
- `header`: malformed JSON, a length that needs more than 2 bytes (the header
  is at most 65,535 bytes), or a missing, extra or mistyped field;
- `dtype`: an unadmitted dtype, a weight that is not `F8_E4M3`, or a scale
  that is neither `F32` nor `F8_E8M0`;
- `shape`: a weight that is not two-dimensional or has a partial block;
- `grid`: a scale tensor whose shape is not `[rows/128, columns/128]`;
- `missing`, `duplicate` and `limit` (more than 16 tensors).

<!-- source: psionic/lib/fp8-file.coax pair -->
```jai
fp8_pair :: (file:[]u8,weight:string,scale:string)->Fp8Pair #export_c {
    if file.count<8 {return Fp8Pair.{status=Fp8Status.offsets};}
    for i:2..<8 {if file[i]!=0 {return Fp8Pair.{status=Fp8Status.header};}}
    length:=cast(int)file[0]|cast(int)file[1]<<8;
    if length>file.count-8 {return Fp8Pair.{status=Fp8Status.offsets};}
    payload:=file.count-8-length;entries:[FP8_FILE_TENSORS]Fp8Entry;count:=0;covered:=0;meta:=false;
    j:[1]Json;json_start_bounded(j[..],cast(string)file[8..<8+length],length,length);
    if json_next(j[..])!=JSON_OBJECT {return Fp8Pair.{status=Fp8Status.header};}
    t:=json_next(j[..]);
    while t!=JSON_OBJECT_END {
        if t!=JSON_STRING {return Fp8Pair.{status=Fp8Status.header};}
        name:=json_text(j[..]);
        if name=="__metadata__" {
            if meta {return Fp8Pair.{status=Fp8Status.duplicate};}meta=true;
            if fp8_metadata(j[..])!=Fp8Status.ok {return Fp8Pair.{status=Fp8Status.header};}
        } else {
            if count==FP8_FILE_TENSORS {return Fp8Pair.{status=Fp8Status.limit};}
            parsed:=fp8_parse(j[..],name);if parsed.status!=Fp8Status.ok {return Fp8Pair.{status=parsed.status};}
            e:=parsed.entry;size:=fp8_bytes(e,payload);
            if size==0 || e.begin>payload-size || e.end!=e.begin+size {return Fp8Pair.{status=Fp8Status.offsets};}
            for i:0..<count {
                if entries[i].name==name {return Fp8Pair.{status=Fp8Status.duplicate};}
                if e.begin<entries[i].end && entries[i].begin<e.end {return Fp8Pair.{status=Fp8Status.offsets};}
            }
            entries[count]=e;count+=1;covered+=size;
        }
        t=json_next(j[..]);
    }
    if json_next(j[..])!=JSON_END {return Fp8Pair.{status=Fp8Status.header};}
    if covered!=payload {return Fp8Pair.{status=Fp8Status.offsets};}
    w:=fp8_find(entries[0..<count],weight);s:=fp8_find(entries[0..<count],scale);
    if w<0 || s<0 {return Fp8Pair.{status=Fp8Status.missing};}
    if entries[w].dtype!=FP8_DTYPE_E4M3 {return Fp8Pair.{status=Fp8Status.dtype};}
    kind:u8=FP8_SCALE_F32;
    if entries[s].dtype==FP8_DTYPE_E8M0 {kind=FP8_SCALE_UE8M0;}
    else if entries[s].dtype!=FP8_DTYPE_F32 {return Fp8Pair.{status=Fp8Status.dtype};}
    if entries[w].rank!=2 {return Fp8Pair.{status=Fp8Status.shape};}
    rows:=entries[w].dims[0];columns:=entries[w].dims[1];
    p:=fp8_plan(rows,columns,kind);if p.status!=Fp8Status.ok {return Fp8Pair.{status=p.status};}
    g:=entries[s];
    if g.rank!=2 || g.dims[0]!=rows/FP8_BLOCK || g.dims[1]!=columns/FP8_BLOCK {return Fp8Pair.{status=Fp8Status.grid};}
    base:=8+length;
    return Fp8Pair.{rows=rows,columns=columns,kind=kind,data_begin=base+entries[w].begin,data_end=base+entries[w].end,
        scale_begin=base+g.begin,scale_end=base+g.end};
}
```

The pair only says where the bytes are. Scales and codes are checked by the
decoder, which reads exactly the two spans the pair returns. It does not
authenticate where the file came from.

## A separate, approximate activation profile

The reference quantizes activations to FP8 at run time. That is a different
job from decoding originals: it rounds, so it cannot be exact. It has its own
name, `fp8-e4m3-dynamic-activation-v1`, and its own stated tolerance.

The input is a row of BF16 activations, a multiple of 128 values, split into
groups of 128. For each group, the scale is the largest magnitude divided by
448, computed in binary64. Each value is divided by the scale and rounded to the
nearest finite code, with ties going to the code with an even last bit. A group
of zeros gets scale 0 and zero codes. Infinities and NaNs are refused with
`number`.

Trace one group whose first values are 896, 3, 2.09375 and −1:

1. The largest magnitude is 896, so the scale is 896 / 448 = 2.
2. 896 / 2 = 448: code 0x7E.
3. 3 / 2 = 1.5, exactly a code: 0x3C.
4. 2.09375 / 2 = 1.046875 lies between 1 (0x38) and 1.125 (0x39). It is
   0.046875 above 1 and 0.078125 below 1.125, so it rounds to 0x38, and the
   dequantized value is 2.
5. −1 / 2 = −0.5: code 0xB0.

The error in step 4 is 0.09375. The profile's tolerance is
`|dequantized − x| ≤ |x|/16 + scale/512`: rounding a normal code moves it by at
most half its spacing, which is at most 1/16 of its value, and a subnormal
code by at most 2⁻¹⁰ × scale. For step 4 the bound is 0.1309 + 0.0039.

<!-- source: psionic/lib/fp8.coax activation -->
```jai
// Nearest finite non-negative code to 0 <= magnitude < 464, ties to the even
// code. The walk stops at 126 (448): code 127's pattern would be 480.
fp8_closer :: (magnitude:f64,code:int)->bool {
    below:=magnitude-fp8_unit(cast(u8)code);above:=fp8_unit(cast(u8)(code+1))-magnitude;
    return above<below || (above==below && code%2==1);
}
fp8_round :: (magnitude:f64)->u8 {
    code:=0;
    while fp8_closer(magnitude,code) {code+=1;}
    return cast(u8)code;
}
fp8_quantize_limit :: (n:int)->bool {return n>=FP8_BLOCK && n/FP8_BLOCK<=FP8_GROUPS_MAX && n%FP8_BLOCK==0;}
fp8_bf16 :: (word:u16)->f64 {return f64(f32_from_bits(cast(u32)word<<16));}
fp8_quantize :: (x:[]u16,codes:[]u8,scales:[]f64,overlap:bool)->Fp8Result #export_c {
    if !fp8_quantize_limit(x.count) {return Fp8Result.{status=Fp8Status.shape};}
    groups:=x.count/FP8_BLOCK;
    if codes.count<x.count || scales.count<groups {return Fp8Result.{status=Fp8Status.capacity};}
    if overlap {return Fp8Result.{status=Fp8Status.alias};}
    for v:x {if (v&0x7fff)>=0x7f80 {return Fp8Result.{status=Fp8Status.number};}}
    for g:0..<groups {
        // Largest magnitude word; positive BF16 words order like their values.
        top:=0;
        for j:0..<FP8_BLOCK {w:=cast(int)(x[g*FP8_BLOCK+j]&0x7fff);top=cast(int)((f64(top+w)+abs(f64(top-w)))/2);}
        scale:=fp8_bf16(cast(u16)top)/448;scales[g]=scale;
        for j:0..<FP8_BLOCK {
            i:=g*FP8_BLOCK+j;q:=0.0;
            if top!=0 {q=abs(fp8_bf16(x[i]))/scale;}
            codes[i]=fp8_round(q)|cast(u8)(x[i]>>8)&128;
        }
    }
    return Fp8Result.{written=groups};
}
```

The bound is checked over 120 generated groups, from tiny to large magnitudes,
with signed zeros. It is a property of this procedure. Whether the reference
implementation computes the same scale and the same codes, bit for bit, has
not been checked; that comparison is the second open item.

## Run it

The [example](../examples/original-fp8.coax) builds the two constructed files,
decodes them, and then breaks them on purpose:

<!-- source: psionic/examples/original-fp8.coax example -->
```jai
main :: () {
    text:[256]u8;buffer:[33100]u8;out:[32768]f64;
    header:=join(text[..],F32_SCALES,WEIGHT);
    file:=fp8_build(buffer[..],header,32776);base:=8+header.count;
    put_codes(file,base);
    // Scale 0.5 for block 0 and 3.0 for block 1, little-endian F32.
    scales:=[8]u8.[0,0,0,0x3f,0,0,0x40,0x40];
    for i:0..<8 {file[base+32768+i]=scales[i];}
    p:=fp8_pair(file,"w.weight","w.weight_scale_inv");
    print("w.weight: % x % F8_E4M3, % blocks of 128 x 128, F32 scales\n",p.rows,p.columns,(p.rows/128)*(p.columns/128));
    r:=decode(file,out[..]);
    #assert r.status==Fp8Status.ok && r.written==32768;
    print("block 0 x 0.5: %, %, %, %\n",out[0],out[1],out[2],out[3]);
    print("block 1 x 3:   %, %, %, %\n",out[128],out[129],out[130],out[131]);

    header=join(text[..],E8M0_SCALES,WEIGHT);
    file=fp8_build(buffer[..],header,32770);base=8+header.count;
    put_codes(file,base);
    // UE8M0 bytes 126 and 131: 2^-1 and 2^4.
    file[base+32768]=126;file[base+32769]=131;
    r=decode(file,out[..]);#assert r.status==Fp8Status.ok;
    print("UE8M0 126, 131: %, %\n",out[2],out[130]);

    out[0]=-7;
    file[base+32768+1]=255;print("scale byte 0xFF: %\n",decode(file,out[..]).status);file[base+32768+1]=131;
    file[base+5]=0x7f;print("code 0x7F: %\n",decode(file,out[..]).status);file[base+5]=0;
    print("one byte short: %\n",decode(file[0..<file.count-1],out[..]).status);
    print("100 x 256, a partial block: %\n",fp8_plan(100,256,FP8_SCALE_F32).status);
    #assert out[0]==-7;

    // Dynamic activation profile: BF16 words 896, 3, 2.09375, -1 in one group.
    x:[128]u16;q:[128]u8;s:[1]f64;
    x[0]=0x4460;x[1]=0x4040;x[2]=0x4006;x[3]=0xbf80;
    a:=fp8_quantize(x[..],q[..],s[..],false);#assert a.status==Fp8Status.ok;
    print("activation scale %, codes",s[0]);
    for i:0..<4 {print(" ");hex(q[i]);}
    print(" -> %, %, %, %\n",fp8_unit(q[0])*s[0],fp8_unit(q[1])*s[0],fp8_unit(q[2])*s[0],fp8_unit(q[3])*s[0]);
}
```

It prints:

```text
w.weight: 128 x 256 F8_E4M3, 2 blocks of 128 x 128, F32 scales
block 0 x 0.5: 0.5, 0.75, -224.0, 0.0009765625
block 1 x 3:   3.0, 4.5, -1344.0, 0.005859375
UE8M0 126, 131: -224.0, -7168.0
scale byte 0xFF: scale
code 0x7F: code
one byte short: offsets
100 x 256, a partial block: shape
activation scale 2.0, codes 0x7e 0x3c 0x38 0xb0 -> 896.0, 3.0, 2.0, -1.0
```

The broken inputs leave the output untouched: a scale byte 0xFF is refused
with `scale`, a NaN code with `code`, a file one byte short with `offsets`,
and a 100 × 256 weight with `shape`.

## How the checks know

The [oracle](../tests/fp8_oracle.py) evaluates every code from the E4M3
definition with Python's exact `Fraction` type and writes
[fp8-vectors.coax](../fixtures/fp8-vectors.coax): the binary64 bits of all 256
codes (NaN marked), and 505 rounding inputs (every code, every midpoint
between neighbours, and points just either side) with their nearest codes. It
shares no code with the decoder. Running it again checks that the fixture has
not drifted.

The checks in [fp8.coax](../lib/fp8.coax) compare:

- the hand table above, including ±0, the smallest subnormal, 448 and both
  NaN codes;
- all 254 finite codes against the oracle's bits;
- every finite code at every one of the 255 admitted UE8M0 scales, 64,770
  products, against the oracle's bits with the exponent field moved by
  `s − 127` in integer arithmetic;
- float scales including the smallest subnormal, the largest float and 0.1;
- every value of two 256 × 256 decodes against its own block's scale;
- each refusal, with the destination unchanged afterwards;
- the activation rounding against the oracle's 505 inputs, and the tolerance
  as a `#forall` property.

The checks in [fp8-file.coax](../lib/fp8-file.coax) cover each refusal
listed above, including the 65,535-byte header limit and the 16-tensor limit.
Both files are `#authority` files. `coax mutate` leaves zero survivors in
`fp8.coax` (195 mutants) and in `fp8-file.coax` (297 mutants, with
`--root-only`, because the shared JSON tokenizer is checked by its own suite).

## What remains

- Pin the headers of Ministral-3-14B-Instruct-2512,
  Devstral-Small-2-24B-Instruct-2512 and Large 4, and confirm from them the
  scale tensor names, dtypes and block size this profile assumes.
- Compare the activation profile with the reference implementation's scales
  and codes on recorded inputs, and state the measured difference.
