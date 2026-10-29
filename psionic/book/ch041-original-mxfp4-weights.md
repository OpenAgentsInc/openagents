# 41. Original MXFP4 weights

The first gpt-oss chapter starts with its expert weights. A reader that only
understands BF16 would misread them: the original expert projections contain
packed four-bit numbers and a separate scale for each group. We will decode
those bytes exactly into a bounded window of `f32` storage.

This chapter starts an independent branch. Chapters 11–40 remain planned; you
need the storage and number representations of chapters 2–4, not their future
model implementations. There is no model download or upstream runtime in the
example or checks.

```shell
coax/build.sh
coax/bin/coax run psionic/examples/original-mxfp4.coax
psionic/tests/check_mxfp4.sh
```

```jai
32 packed bytes + 2 scale bytes -> 64 f32 values
first block: 0.0, 0.5, 4.0, 6.0
second block: 0.0, 1.0, 8.0, 12.0
out-of-profile scale refused before publication
```

## Which original bytes?

We read OpenAI's [gpt-oss source at revision
5681a6b](https://github.com/openai/gpt-oss/tree/5681a6b6552635e7b29cde94a61ca6a7aee90ebd)
and [Harmony at revision
abd677f](https://github.com/openai/harmony/tree/abd677f7ac962629c808197caa1feb9e3e95d2b0).
The [gpt-oss README](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/README.md#download-the-model)
distinguishes the `original/` checkpoints used by its Torch/Triton references
from the Hugging Face converted layout at the repository root and the Metal
conversion. This chapter follows the original loader's byte ordering.

Its [weight loader](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/torch/weights.py)+pairs names such as `block.0.mlp.mlp1_weight.blocks` and
`block.0.mlp.mlp1_weight.scales`. The packed tensor's last dimension is 16
bytes; the corresponding scale tensor omits that dimension. One group
becomes 32 consecutive values along the decoded tensor's final dimension.

The fixtures here are owned synthetic bytes in that layout. They are not
extracted checkpoint tensors. The existing chapter 4 BF16/F32 file profile
does not admit these U8 pairs. Complete original-file inventory, authenticated
binding of each pair and actual checkpoint scale coverage are later model
qualification gates. The decoder receives an already selected pair of byte
windows; it does not authenticate their provenance.

## Four bits hold a small signed number

E2M1 has one sign bit, two exponent bits and one mantissa bit. Its positive
magnitudes are small enough to write out:

| Lower three bits | Magnitude | Code with sign bit set |
| --- | --- | --- |
| 0 | 0 | 8: negative zero |
| 1 | 0.5 | 9: -0.5 |
| 2 | 1 | 10: -1 |
| 3 | 1.5 | 11: -1.5 |
| 4 | 2 | 12: -2 |
| 5 | 3 | 13: -3 |
| 6 | 4 | 14: -4 |
| 7 | 6 | 15: -6 |

The first value occupies the byte's low nibble. For example, `0x91` means
positive 0.5 followed by negative 0.5. Decoding all low nibbles and then all
high nibbles would silently reorder a row.

One unsigned scale byte `s` multiplies every value in a group by a power of
two:

```jai
value = E2M1(code) * 2^(s - 127)
```

Scale 127 leaves the values unchanged. Scale 128 doubles them. The same
packed bytes with different scales therefore represent different weights.
The input windows must contain exactly 16 packed bytes and one scale byte
per group.

## Admit a finite decoding window

The named profile is `gpt-oss-mxfp4-f32-v1`. It accepts complete rows with
positive row count, a positive column count divisible by 32, and at most
1,048,576 output values. A window can flatten an expert/row prefix, but its
original shape and group alignment must have been checked by its caller.

<!-- source: psionic/lib/mxfp4.coax plan -->
```jai
mx_plan :: (rows:int,columns:int)->MxPlan #export_c {
    if rows<1 || columns<32 || columns%32!=0 {return MxPlan.{status=MxStatus.shape};}
    per_row:=columns/32;
    if rows>32768/per_row {return MxPlan.{status=MxStatus.limit};}
    blocks:=rows*per_row;values:=blocks*32;
    return MxPlan.{blocks=blocks,packed_bytes=blocks*16,values=values};
}
```

The division checks the bound before multiplication. One group requires
17 encoded bytes and expands to 128 output bytes. The maximum window needs
4 MiB of output; it does not require expanding a complete expert tensor or
model into memory.

This profile admits scale bytes 0–252. At scale 252, the largest magnitude
is exactly `6 * 2^125`, still finite in `f32`. Scale 253 can already produce
overflow. We conservatively refuse an entire group at scales 253–255, even
if some of its codes would individually remain finite. This is an explicit
profile restriction, not evidence that every original checkpoint fits it.

At the other end, scale zero and code one produce `2^-128`, an exactly
representable binary32 subnormal. Losing it to zero would be a decoder error
on this CPU profile. Negative zero must also preserve its sign bit.

## Decode without a rounding approximation

All admitted magnitudes and powers are exact dyadic values. Binary64's
exponent bias is 1023 and its fraction occupies 52 bits. Setting its exponent
to `s + 896`, where `896 = 1023 - 127`, constructs exactly the power of two we
need. The product is exact, and the admitted range makes the narrowing to
binary32 exact as well. We restore the sign bit explicitly, including for zero.

<!-- source: psionic/lib/mxfp4.coax bits -->
```jai
mx_bits :: (code:u8,scale:u8)->u32 {
    power:=f64_from_bits((cast(u64)scale+896)<<52);
    magnitude:=f32(MX_E2M1[cast(int)(code&7)]*power);
    return ((cast(u32)code&8)<<28)|f32_bits(magnitude);
}
```

`mx_bits` is internal: the caller has already restricted its arguments to a
nibble and an admitted scale. The public decoder first validates the whole
window, including every scale. Only then can it publish any output:

<!-- source: psionic/lib/mxfp4.coax decode -->
```jai
mx_decode :: (rows:int,columns:int,packed:[]u8,scales:[]u8,out:[]f32,overlap:bool)->MxResult #export_c {
    p:=mx_plan(rows,columns);if p.status!=MxStatus.ok {return MxResult.{status=p.status};}
    if packed.count!=p.packed_bytes || scales.count!=p.blocks {return MxResult.{status=MxStatus.storage};}
    if out.count<p.values {return MxResult.{status=MxStatus.capacity};}
    if overlap {return MxResult.{status=MxStatus.alias};}
    for scale:scales {if scale>252 {return MxResult.{status=MxStatus.scale};}}
    for i:0..<packed.count {
        byte:=packed[i];scale:=scales[i/16];
        out[2*i]=f32_from_bits(mx_bits(byte&15,scale));
        out[2*i+1]=f32_from_bits(mx_bits(byte>>4,scale));
    }
    return MxResult.{written=p.values};
}
```

No scratch array is necessary here. After admission, every decode is finite
and exact, so there is no later arithmetic refusal requiring staged output.
A malformed shape, wrong byte count, insufficient capacity, observed overlap
or out-of-profile scale leaves the entire destination unchanged. Successful
decoding leaves unused output capacity untouched.

The [native adapter](../platform/mxfp4-memory.c) observes overlap between
output and both input spans. The caller still owns truthful extents, valid
alignment for the float output, immutable inputs, lifetimes and exclusive
access. Byte input need not have float alignment. This is a CPU profile with
gradual underflow; it does not qualify a GPU backend.

## Use the shared implementation

The [example](../examples/original-mxfp4.coax) decodes two groups with different
scales and checks refusal of a bad final scale:

<!-- source: psionic/examples/original-mxfp4.coax example -->
```jai
main :: () {
    packed:=[32]u8.[0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe,0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe,0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe,0x10,0x32,0x54,0x76,0x98,0xba,0xdc,0xfe];
    scales:=[2]u8.[127,128];out:[64]f32;
    result:=mx_decode(1,64,packed[..],scales[..],out[..],false);
    #assert result.status==MxStatus.ok;
    print("32 packed bytes + 2 scale bytes -> % f32 values\n",result.written);
    print("first block: %, %, %, %\n",out[0],out[1],out[6],out[7]);
    print("second block: %, %, %, %\n",out[32],out[33],out[38],out[39]);
    #assert f32_bits(out[8])==0x80000000;
    scales[1]=253;out[0]=-7;
    refused:=mx_decode(1,64,packed[..],scales[..],out[..],false);
    #assert refused.status==MxStatus.scale && refused.written==0 && out[0]==-7;
    print("out-of-profile scale refused before publication\n");
}
```

The [independent oracle](../tests/mxfp4_oracle.py) uses scalar `math.ldexp`
and binary32 packing, rather than the decoder's exponent-bit construction.
All 16 codes at all 253 admitted scales have expected output bits. Native
checks also cover every one of the 256 packed bytes at every admitted scale,
the maximum window, genuine overlapping buffers and destination preservation
when the final scale is refused. The [verification record](../../docs/verification/psionic-ch34-2026-10-29.md)
identifies the compiler, host, sanitizer and mutation results.

This is format compatibility, not gpt-oss generation. The
[next chapter in this branch](PLAN.md) builds the expert router. Attention,
complete original-file admission and full-model evidence follow separately.
[Harmony's format](https://github.com/openai/harmony/blob/abd677f7ac962629c808197caa1feb9e3e95d2b0/docs/format.md)
is required for eventual conversations; its tokenizer and streaming parser
belong to chapter 44. Nothing in this byte decoder establishes that behavior.
