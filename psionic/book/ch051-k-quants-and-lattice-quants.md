# 51. K-quants and lattice quants

> **Status: partly implemented.** Decoders for the five K-quant formats
> (Q2_K to Q6_K), the two IQ4 formats and the five IQ3 and IQ2 lattice
> formats run on the CPU, match an independent oracle bit for bit, read
> tensors through chapter 49's descriptors, and each has a quality card on a
> small stand-in model, written by a simple reference quantizer. llama.cpp's
> own quantizers, cards measured on a named model's original weights, and
> perplexity on fixed text are not done yet.

Most published quantized checkpoints are not Q8_0 or Q4_0. They use the
K-quant formats, which group 256 weights into a super-block with small packed
scales inside it, and the IQ formats, which replace evenly spaced levels with
a table, or a codebook of whole groups of values. This chapter adds decoders
for twelve of these block formats, so Psionic can read the files, and
measures each format with chapter 50's quality card.

Everything here runs on the CPU, written in Coax.

```shell
coax/bin/coax run psionic/examples/kquant.coax
coax/bin/coax test psionic/lib/kquant.coax
coax/bin/coax mutate psionic/lib/kquant.coax --root-only
python3 -I psionic/tests/kquant_oracle.py
```

## Two levels of scales

Chapter 50's Q4_0 stores one 16-bit scale for every 32 weights: half a bit per
weight goes to the scale. K-quants spend less on scales by stacking them. A
super-block of 256 weights has one or two half-precision numbers, `d` and
sometimes `dmin`. Each sub-block of 16 or 32 weights has a small integer
scale, 4, 6 or 8 bits wide, that multiplies `d`. One weight decodes as:

```text
asymmetric (Q2_K, Q4_K, Q5_K):  x = d × scale × q − dmin × min
symmetric  (Q3_K, Q6_K):        x = d × scale × q,  scale and q signed
```

The byte layouts are ggml's block structs in `ggml-common.h`. Chapter 49's
table already knows each GGUF type's block size:

| Format | GGUF type | Values per block | Bytes per block | Bits per weight | Sub-block scales |
| --- | --- | --- | --- | --- | --- |
| Q2_K | 10 | 256 | 16 + 64 + 2 + 2 = 84 | 2.625 | 16 × (4-bit scale, 4-bit min) |
| Q3_K | 11 | 256 | 32 + 64 + 12 + 2 = 110 | 3.4375 | 16 × 6-bit, minus 32 |
| Q4_K | 12 | 256 | 2 + 2 + 12 + 128 = 144 | 4.5 | 8 × (6-bit scale, 6-bit min) |
| Q5_K | 13 | 256 | 2 + 2 + 12 + 32 + 128 = 176 | 5.5 | 8 × (6-bit scale, 6-bit min) |
| Q6_K | 14 | 256 | 128 + 64 + 16 + 2 = 210 | 6.5625 | 16 × signed 8-bit |
| IQ4_NL | 20 | 32 | 2 + 16 = 18 | 4.5 | none; a 16-level table |
| IQ4_XS | 23 | 256 | 2 + 2 + 4 + 128 = 136 | 4.25 | 8 × 6-bit, minus 32; the table |

The decoder works in exact integers. A finite half is an integer number of
2⁻²⁴ units: a subnormal is `fraction` units and a normal number is
`(1024 + fraction) × 2^(exponent − 1)` units. The largest half, 65,504, is
just under 2⁴⁰ units, and the largest code product, Q6_K's 128 × 32, adds 12
bits, so every decoded value is an integer below 2⁵³. That converts to binary64
exactly and rounds to f32 once. ggml's float loops also round each value at
most once, so the two agree; the oracle below checks it.

<!-- source: psionic/lib/kquant.coax units -->
```jai
// A finite half as a signed integer number of 2^-24 units: exact for every
// half, and products of a half with small codes stay below 2^53.
kq_units :: (bits:int)->int {
    e:=(bits>>10)&31;m:=bits&1023;
    v:=ifx e==0 then m else (1024+m)<<(e-1);
    return ifx (bits&0x8000)!=0 then -v else v;
}
kq_byte :: (b:[]u8,at:int)->int {return cast(int)b[at];}
kq_half_bits :: (b:[]u8,at:int)->int {return kq_byte(b,at)|(kq_byte(b,at+1)<<8);}
kq_half_units :: (b:[]u8,at:int)->int {return kq_units(kq_half_bits(b,at));}
```

## Q2_K by hand

A Q2_K block starts with 16 scale bytes, one per sub-block of 16: the low
nibble is the scale and the high nibble the min. Then come 64 bytes of 2-bit
codes and the two halves, `d` at byte 80 and `dmin` at byte 82. ggml's loop
walks the 256 values in two halves of 128. In each half, byte `l` of the
32-byte code run holds four codes, one for each run of 32 values, at shifts
0, 2, 4 and 6.

Take `d = 0x3800 = 0.5`, `dmin = 0x3c00 = 1`, and scale bytes 0, 2, 4 and 6
(the first sub-block of each run of 32) set to `0x21`, `0x13`, `0x05` and
`0xf2`. Code byte 16 is `0xe4 = 0b11 10 01 00`:

```text
element  sub-block  scale byte  scale  min  code (shift)  d·scale·q − dmin·min
0        0          0x21        1      2    0  (0)        0.5·1·0 − 1·2  = −2
32       2          0x13        3      1    1  (2)        0.5·3·1 − 1·1  = 0.5
64       4          0x05        5      0    2  (4)        0.5·5·2 − 0    = 5
96       6          0xf2        2      15   3  (6)        0.5·2·3 − 15   = −12
16       1          0x00        0      0    0  (0)        0
```

## Q3_K by hand: 6 bits from two places

Q3_K is symmetric. Each value has a 2-bit low part in `qs` (at byte 32) and a
high bit in `hmask` (bytes 0 to 31); without the high bit the code is 4 lower,
so codes run from −4 to 3. The sixteen 6-bit scales are packed into 12 bytes
at offset 96: the low four bits of scale `s` are a nibble of byte
`96 + s mod 8` (the low nibble for `s < 8`), and its top two bits come from
byte `104 + s mod 4`, at shift `2 × (s div 4)`. The stored scale is 32 more
than the real one.

With `d = 1`:

```text
scale 0: byte 96 = 0x01 -> low 1; byte 104 = 0x02 -> top 2; 1 + 2·16 = 33 -> 1
element 0: qs byte 32 = 0x03 -> 3; hmask byte 0 bit 0 set -> code 3;   1·3     = 3
element 1: qs byte 33 = 0x02 -> 2; hmask byte 1 bit 0 clear -> 2 − 4; 1·(−2)  = −2
element 32: scale 2 has no bits -> 0 − 32; code 0 − 4;               −32·−4   = 128
element 144: scale 9: byte 97 = 0x50 -> high nibble 5; byte 105 = 0x10, shift 4 -> 1;
             5 + 16 = 21 -> −11; code 1 with hmask byte 16 bit 4 set;  −11·1    = −11
```

## Q4_K and Q5_K: 12 bytes for 16 six-bit numbers

Q4_K and Q5_K have eight sub-blocks of 32, each with a 6-bit scale and a 6-bit
min, 96 bits in the 12 bytes from offset 4. Sub-blocks 0 to 3 use the low six
bits of bytes 4 to 7 (scales) and 8 to 11 (mins). Sub-blocks 4 to 7 take their
low four bits from the nibbles of bytes 12 to 15 and their top two bits from
the spare top bits of bytes 4 to 11. This is ggml's `get_scale_min_k4`:

<!-- source: psionic/lib/kquant.coax kquant -->
```jai
// Q4_K and Q5_K: eight 6-bit scales (min = 0) or mins (min = 1) in the 12
// bytes from offset 4, ggml's get_scale_min_k4.
kq_scale_min :: (b:[]u8,j:int,min:int)->int {
    if j<4 {return kq_byte(b,4+j+4*min)&63;}
    return ((kq_byte(b,4+j+4)>>(4*min))&15)|((kq_byte(b,j+4*min)>>6)<<4);
}
// Element k of one block, in 2^-24 units.
kq_value :: (kind:int,b:[]u8,k:int)->int {
    if kind==KQ_Q6_K {
        // 128 values per half: four runs of 32, low nibbles in ql, two high bits in qh.
        n:=k/128;part:=(k%128)/32;l:=k%32;
        q:=((kq_byte(b,64*n+l+32*(part%2))>>(4*(part/2)))&15)|(((kq_byte(b,128+32*n+l)>>(2*part))&3)<<4);
        return kq_half_units(b,208)*qz_signed(b[192+k/16])*(q-32);
    }
    if kind==KQ_Q4_K || kind==KQ_Q5_K {
        // Four runs of 64: low nibbles are sub-block 2g, high nibbles 2g + 1.
        g:=k/64;part:=(k%64)/32;l:=k%32;j:=2*g+part;
        q:=(kq_byte(b,ifx kind==KQ_Q5_K then 48+32*g+l else 16+32*g+l)>>(4*part))&15;
        if kind==KQ_Q5_K {q+=((kq_byte(b,16+l)>>j)&1)<<4;}
        return kq_half_units(b,0)*kq_scale_min(b,j,0)*q-kq_half_units(b,2)*kq_scale_min(b,j,1);
    }
    // Q2_K and Q3_K: two halves of 128, each byte of qs holds four 2-bit
    // codes, the shift picks the run of 32, and h picks the sub-block of 16.
    n:=k/128;j:=(k%128)/32;h:=(k%32)/16;l:=k%16;s:=8*n+2*j+h;
    if kind==KQ_Q2_K {
        sc:=kq_byte(b,s);
        return kq_half_units(b,80)*(sc&15)*((kq_byte(b,16+32*n+16*h+l)>>(2*j))&3)-kq_half_units(b,82)*(sc>>4);
    }
    low:=(kq_byte(b,32+32*n+16*h+l)>>(2*j))&3;high:=(kq_byte(b,16*h+l)>>(4*n+j))&1;
    sc:=((kq_byte(b,96+s%8)>>(4*(s/8)))&15)|(((kq_byte(b,104+s%4)>>(2*(s/4)))&3)<<4);
    return kq_half_units(b,108)*(sc-32)*(low-4+4*high);
}
```

By hand, with `d = 0.5` and `dmin = 1`:

```text
byte 4 = 0xc3 = 0b11 000011: scale 0 = 3, top bits of scale 4 = 3
byte 8 = 0x41 = 0b01 000001: min 0 = 1,   top bits of min 4 = 1
byte 12 = 0x25:             scale 4 = 5 + 3·16 = 53, min 4 = 2 + 1·16 = 18
Q4_K element 0: low nibble of byte 16 (0x9a) = 10:  0.5·3·10 − 1·1   = 14
Q4_K element 128: run 2, low nibble of byte 80 = 7:  0.5·53·7 − 1·18  = 167.5
```

Q5_K stores the same nibbles at offset 48 and adds a fifth bit from the 32
bytes at offset 16: bit `j` of byte `16 + l` belongs to element `l` of
sub-block `j`. With byte 16 = `0x11`, element 0 gets bit 0 (code
10 + 16 = 26, so 0.5·3·26 − 1 = 38) and element 128 gets bit 4 (code
7 + 16 = 23, so 0.5·53·23 − 18 = 591.5).

Q6_K keeps the low four bits of each code in 128 bytes, the top two bits in
64 more, and one signed 8-bit scale per sub-block of 16 at byte 192 + k / 16.
Codes are stored 32 higher than their value. With `d = 0.5`: element 0 has low
nibble 1 and top bits 2, so 33 − 32 = 1, times scale 2: 1. Element 64 has the
high nibble of the same byte, 3, so 3 − 32 = −29, times scale `0xfd` = −3:
0.5·−3·−29 = 43.5.

## IQ4: a table instead of evenly spaced levels

Weights are not spread evenly: there are many near zero and a few large
ones. IQ4_NL keeps chapter 50's Q4_0 layout, a half scale and 16 bytes of
nibbles for 32 values, but each nibble indexes a fixed table of 16 levels
that are closer together near zero. The table is copied from ggml:

<!-- source: psionic/lib/kquant.coax table -->
```jai
// kvalues_iq4nl from ggml-common.h (MIT, Copyright (c) 2023-2026 The ggml authors).
KQ_IQ4 :: [16]int.[-127,-104,-83,-65,-49,-35,-22,-10,1,13,25,38,53,69,89,113];
```

IQ4_XS puts the same table inside a super-block of 256: one half `d`, then
eight 6-bit sub-block scales (stored 32 higher) split into four low bits in
`scales_l` and two high bits in the 16-bit `scales_h`. That is 136 bytes for
256 weights, 4.25 bits each.

<!-- source: psionic/lib/kquant.coax iq4 -->
```jai
kq_iq4 :: (kind:int,b:[]u8,k:int)->int {
    if kind==KQ_IQ4_NL {return kq_half_units(b,0)*KQ_IQ4[(kq_byte(b,2+k%16)>>(4*(k/16)))&15];}
    // IQ4_XS: eight sub-blocks of 32, each a 6-bit scale minus 32: four low
    // bits in scales_l (byte 4 + ib/2), two high bits in the 16-bit scales_h.
    ib:=k/32;r:=k%32;
    ls:=((kq_byte(b,4+ib/2)>>(4*(ib%2)))&15)|(((kq_half_bits(b,2)>>(2*ib))&3)<<4);
    return kq_half_units(b,0)*(ls-32)*KQ_IQ4[(kq_byte(b,8+16*ib+r%16)>>(4*(r/16)))&15];
}
```

By hand: an IQ4_NL block with `d = 0.5` and byte 2 = `0xf0` gives element 0
(low nibble 0) 0.5·−127 = −63.5 and element 16 (high nibble 15) 0.5·113 =
56.5. An IQ4_XS block with `d = 1`, `scales_h` = 2 and `scales_l[0]` = 1 has
sub-block 0's scale 1 + 2·16 − 32 = 1 and sub-block 1's 0 − 32 = −32; byte 8 =
`0x8f` makes element 0 113 and element 16 1, and element 32 (nibble 0) is
−32·−127 = 4064.

## IQ3 and IQ2: codes that name a point in a grid

The IQ3 and IQ2 formats spend about 2 to 3.4 bits a weight. At that rate a
code per weight leaves four to eight levels for each one. These formats code
a group of 4 or 8 weights at once instead: one index
names a point in a fixed grid, a codebook ggml ships as a table. The grid
holds magnitudes only. Signs are stored separately, one bit per weight, and
a 4-bit scale sets the size of each sub-block of 16 or 32 weights.

| Format | GGUF type | Bytes per 256 | Bits per weight | Grid | Scale | Signs per group of 8 |
| --- | --- | --- | --- | --- | --- | --- |
| IQ2_XXS | 16 | 2 + 64 = 66 | 2.0625 | 256 points of 8, bytes 8, 25, 43 | d (0.5 + s) / 4, per 32 | 7 bits |
| IQ2_XS | 17 | 2 + 64 + 8 = 74 | 2.3125 | 512 points of 8, the same bytes | d (0.5 + s) / 4, per 16 | 7 bits |
| IQ2_S | 22 | 2 + 64 + 8 + 8 = 82 | 2.5625 | 1,024 points of 8, the same bytes | d (0.5 + s) / 4, per 16 | 8 bits |
| IQ3_XXS | 18 | 2 + 64 + 32 = 98 | 3.0625 | 256 points of 4, bytes 4 to 62 | d (0.5 + s) / 2, per 32 | 7 bits |
| IQ3_S | 21 | 2 + 64 + 8 + 32 + 4 = 110 | 3.4375 | 512 points of 4, odd bytes 1 to 15 | d (1 + 2 s), per 32 | 8 bits |

A grid entry is a 64-bit word (IQ2) or a 32-bit word (IQ3). ggml reads it
through a byte pointer, so value `j` of the group is byte `j` of the word on
a little-endian host. The IQ3 formats use two 4-value entries per group of 8.

Seven sign bits per group save one bit in eight. ggml's `ksigns_iq2xs` table
maps the 7 stored bits to 8: the eighth bit is set when the other seven have
an odd count, so every group has an even number of minus signs. A quantizer
for IQ2_XXS, IQ2_XS or IQ3_XXS therefore flips one sign when a group has an
odd count. IQ2_S and IQ3_S store all 8 bits. `kmask_iq2xs` picks bit `j`.

The tables are data copied from ggml. `kquant_oracle.py --grids` reads them
as text from `ggml/src/ggml-common.h` in a local llama.cpp checkout (commit
`9b05354ec6fb58b4e665e9a39ebc40285c015638`), without building or running
anything, and writes [iq-grids.coax](../fixtures/iq-grids.coax): each table,
and the SHA-256 of its entries as little-endian bytes. The same digests are
pinned in the oracle. A `#test` hashes each table with Coax's SHA-256 and
compares it with the recorded digest, so a changed entry fails both sides.
The checks also derive what the tables must be without them: `kmask` is
`1 << j`, every `ksigns` entry is its index plus the parity bit, and every
grid is sorted, has no repeated points and uses only its format's bytes.

Each sub-block `ib` of 32 weights is laid out differently per format:

- **IQ2_XXS**: 8 bytes from offset 2 + 8 ib. Four grid indices, one byte each, then a 32-bit word
  holding four 7-bit sign indices (bits 0 to 27) and the scale (bits 28 to
  31).
- **IQ2_XS**: four 16-bit codes from offset 2 + 8 ib, each a 9-bit grid
  index and a 7-bit sign index, and scale byte 66 + ib with a nibble for each
  half of 16.
- **IQ2_S**: four index bytes at 2 + 4 ib, four sign bytes at 34 + 4 ib, two
  more index bits per group in `qh` byte 66 + ib, and scale byte 74 + ib.
- **IQ3_XXS**: eight index bytes (two per group) at 2 + 8 ib, and a 32-bit
  word of sign indices and scale at 66 + 4 ib.
- **IQ3_S**: eight index bytes at 2 + 8 ib, a ninth index bit for each half
  group in `qh` byte 66 + ib, four full sign bytes at 74 + 4 ib, and a scale
  nibble in byte 106 + ib / 2.

The decoder takes element `k` as sub-block `k / 32`, group `(k mod 32) / 8`
and position `k mod 8`:

<!-- source: psionic/lib/kquant.coax lattice -->
```jai
// Byte j of a grid entry, in memory order on a little-endian host, as ggml
// reads it, and the sign of position j in an 8-bit sign mask.
kq_grid_byte :: (entry:u64,j:int)->int {return cast(int)((entry>>cast(u64)(8*j))&255);}
kq_sign :: (signs:int,j:int)->int {return ifx (signs&cast(int)KQ_KMASK[j])!=0 then -1 else 1;}
kq_word :: (b:[]u8,at:int)->int {return kq_half_bits(b,at)|(kq_half_bits(b,at+2)<<16);}
// Element k of a lattice block in 2^-27 units: sub-block ib of 32, group l
// of 8, position j. ggml's scale is d (0.5 + s) / 4 for IQ2, d (0.5 + s) / 2
// for IQ3_XXS and d (1 + 2 s) for IQ3_S, so d (2 s + 1) times 1, 2 or 8.
kq_lattice :: (kind:int,b:[]u8,k:int)->int {
    ib:=k/32;l:=(k%32)/8;j:=k%8;d:=kq_half_units(b,0);
    if kind==KQ_IQ2_XXS {
        // Eight bytes a sub-block: four grid indices, then 4 x 7 sign bits and a 4-bit scale.
        aux:=kq_word(b,6+8*ib);
        g:=kq_grid_byte(KQ_IQ2XXS_GRID[kq_byte(b,2+8*ib+l)],j);
        return kq_sign(cast(int)KQ_KSIGNS[(aux>>(7*l))&127],j)*d*(2*(aux>>28)+1)*g;
    }
    if kind==KQ_IQ3_XXS {
        // Two 4-value grid indices a group; the signs and scale words follow all 64 indices.
        aux:=kq_word(b,66+4*ib);
        g:=kq_grid_byte(cast(u64)KQ_IQ3XXS_GRID[kq_byte(b,2+8*ib+2*l+j/4)],j%4);
        return kq_sign(cast(int)KQ_KSIGNS[(aux>>(7*l))&127],j)*d*(2*(aux>>28)+1)*2*g;
    }
    if kind==KQ_IQ3_S {
        // A ninth index bit from qh for each half group, a full sign byte, two scales a byte.
        e:=kq_byte(b,2+8*ib+2*l+j/4)|(((kq_byte(b,66+ib)>>(2*l+j/4))&1)<<8);
        s:=(kq_byte(b,106+ib/2)>>(4*(ib%2)))&15;
        return kq_sign(kq_byte(b,74+4*ib+l),j)*d*(2*s+1)*8*kq_grid_byte(cast(u64)KQ_IQ3S_GRID[e],j%4);
    }
    // IQ2_XS and IQ2_S: a 4-bit scale for each 16 values.
    if kind==KQ_IQ2_XS {
        // A 16-bit code a group: a 9-bit grid index and 7 sign bits.
        q:=kq_half_bits(b,2+8*ib+2*l);s:=(kq_byte(b,66+ib)>>(4*(l/2)))&15;
        return kq_sign(cast(int)KQ_KSIGNS[q>>9],j)*d*(2*s+1)*kq_grid_byte(KQ_IQ2XS_GRID[q&511],j);
    }
    // IQ2_S: a byte of index, two more bits from qh, and a full sign byte.
    e:=kq_byte(b,2+4*ib+l)|(((kq_byte(b,66+ib)>>(2*l))&3)<<8);s:=(kq_byte(b,74+ib)>>(4*(l/2)))&15;
    return kq_sign(kq_byte(b,34+4*ib+l),j)*d*(2*s+1)*kq_grid_byte(KQ_IQ2S_GRID[e],j);
}
```

ggml computes the scale `db` in float and multiplies by the grid byte. Every
step is exact: a half has 11 significant bits, `2 s + 1` at most 5, and a
grid byte at most 6, so the product has at most 22, within f32's 24. The
decoder keeps the same value as an integer in 2⁻²⁷ units, `d × (2 s + 1)`
times 1, 2 or 8 times the grid byte. The largest, IQ3_XXS's 65,504 · 2²⁴ ×
31 × 2 × 62 units, is below 2⁵², so it converts to f64 exactly and to f32
once, with the same result.

### IQ2_XXS by hand

One block with `d = 0x3c00 = 1`. Sub-block 0 is bytes 2 to 9: indices
`01 00 00 00`, then the word `0x30000083`.

```text
scale: 0x30000083 >> 28 = 3          -> 1 · (0.5 + 3) / 4 = 0.875
group 0: index 1 -> iq2xxs_grid[1] = 0x080808080808082b -> bytes 43, 8, 8, 8, 8, 8, 8, 8
         sign index 0x83 & 127 = 3   -> ksigns[3] = 0x03: elements 0 and 1 negative
         y[0] = −0.875 · 43 = −37.625, y[1] = −0.875 · 8 = −7, y[2] = 7
group 1: index 0 -> eight 8s
         sign index (0x83 >> 7) & 127 = 1 -> ksigns[1] = 0x81: one stored minus
         sign is odd, so the parity bit adds element 15
         y[8] = −7, y[9] = 7, y[15] = −7
sub-block 1: all zero bytes -> scale (0.5 + 0) / 4 = 0.125, index 0
         y[32] = 0.125 · 8 = 1
```

### IQ3_S by hand

One block with `d = 0x3800 = 0.5` and scale byte 106 = `0x21`.

```text
sub-block 0 scale: low nibble 1     -> 0.5 · (1 + 2 · 1) = 1.5
sub-block 1 scale: high nibble 2    -> 0.5 · (1 + 2 · 2) = 2.5
group 0, values 0-3: qs byte 2 = 0x01, qh byte 66 bit 0 set -> index 1 + 256 = 257
         iq3s_grid[257] = 0x05090705 -> bytes 5, 7, 9, 5
group 0, values 4-7: qs byte 3 = 0x00, qh bit 1 clear      -> index 0 = 1, 1, 1, 1
sign byte 74 = 0x92 = 0b10010010     -> elements 1, 4 and 7 negative
y[0..7] = 7.5, −10.5, 13.5, 7.5, −1.5, 1.5, 1.5, −1.5
y[32]: sub-block 1, index 0          -> 2.5 · 1 = 2.5
```

These are the hand checks, compared exactly by `#test`s:

<!-- source: psionic/lib/kquant.coax hand-tests -->
```jai
#test "Q2_K by hand: 4-bit scale and min in one byte, 2-bit codes" {
    // d = 0.5 and dmin = 1; scale bytes 0, 2, 4, 6 hold (scale | min << 4).
    b:[84]u8;b[0]=0x21;b[2]=0x13;b[4]=0x05;b[6]=0xf2;b[16]=0xe4;b[80]=0x00;b[81]=0x38;b[82]=0x00;b[83]=0x3c;
    y:[256]f32;
    #assert kq_decode(KQ_Q2_K,1,256,b[..],y[..]).status==QzStatus.ok;
    // Byte 16 = 0b11100100: codes 0, 1, 2, 3 for elements 0, 32, 64, 96.
    #assert y[0]==-2 && y[32]==0.5 && y[64]==5 && y[96]==-12 && y[1]==-2 && y[16]==0;
}
#test "Q3_K by hand: a 6-bit scale minus 32, a 2-bit code and a high bit" {
    b:[110]u8;b[108]=0x00;b[109]=0x3c;
    b[96]=0x01;b[104]=0x02;b[32]=0x03;b[0]=0x01;b[33]=0x02;
    b[97]=0x50;b[105]=0x10;b[80]=0x01;b[16]=0x10;
    y:[256]f32;
    #assert kq_decode(KQ_Q3_K,1,256,b[..],y[..]).status==QzStatus.ok;
    // Scale 0 = 1 | 2 << 4 = 33 -> 1. Element 0: code 3, high bit set -> 3; element 1: 2 - 4.
    #assert y[0]==3 && y[1]==-2;
    // Scale 2 is all zero bits: 0 - 32; element 32's code 0 without its high bit: -4.
    #assert y[32]==128;
    // Scale 9 = 5 | 1 << 4 = 21 -> -11; element 144: code 1, high bit 4 of byte 16 set.
    #assert y[144]==-11;
}
#test "Q4_K and Q5_K by hand: 6-bit scales and mins packed into 12 bytes" {
    b:[176]u8;b[0]=0x00;b[1]=0x38;b[2]=0x00;b[3]=0x3c;b[4]=0xc3;b[8]=0x41;b[12]=0x25;
    // Sub-block 0: scale 3, min 1. Sub-block 4: scale 5 | 3 << 4 = 53, min 2 | 1 << 4 = 18.
    four:=b;four[16]=0x9a;four[80]=0x07;
    y:[256]f32;
    #assert kq_decode(KQ_Q4_K,1,256,four[0..<144],y[..]).status==QzStatus.ok;
    #assert y[0]==14 && y[32]==0 && y[128]==167.5;
    // Q5_K: codes at 48, the fifth bit of element l in byte 16 + l, bit = sub-block.
    b[48]=0x9a;b[112]=0x07;b[16]=0x11;
    #assert kq_decode(KQ_Q5_K,1,256,b[..],y[..]).status==QzStatus.ok;
    #assert y[0]==38 && y[128]==591.5 && y[1]==-1;
}
#test "Q6_K and IQ4 by hand: signed 8-bit scales and the non-uniform table" {
    b:[210]u8;b[208]=0x00;b[209]=0x38;b[0]=0x31;b[128]=0x02;b[192]=2;b[196]=0xfd;
    y:[256]f32;
    #assert kq_decode(KQ_Q6_K,1,256,b[..],y[..]).status==QzStatus.ok;
    // Element 0: 1 | 2 << 4 = 33 -> 1, scale 2. Element 64: 3 -> -29, scale -3.
    #assert y[0]==1 && y[64]==43.5 && y[32]==0;
    n:[18]u8;n[0]=0x00;n[1]=0x38;n[2]=0xf0;
    #assert kq_decode(KQ_IQ4_NL,1,32,n[..],y[..]).status==QzStatus.ok;
    #assert y[0]==-63.5 && y[16]==56.5 && y[1]==-63.5 && y[17]==-63.5;
    x:[136]u8;x[0]=0x00;x[1]=0x3c;x[2]=0x02;x[4]=0x01;x[8]=0x8f;
    #assert kq_decode(KQ_IQ4_XS,1,256,x[..],y[..]).status==QzStatus.ok;
    // Sub-block 0: 1 | 2 << 4 = 33 -> 1; sub-block 1: 0 -> -32.
    #assert y[0]==113 && y[16]==1 && y[32]==4064;
}
```

<!-- source: psionic/lib/kquant.coax grid-tests -->
```jai
#test "IQ2_XXS and IQ3_S by hand: grid entries, sign masks and 4-bit scales" {
    // IQ2_XXS, d = 1. Sub-block 0: index bytes 1, 0, 0, 0; then 0x83 0x00 0x00 0x30.
    b:[66]u8;b[0]=0x00;b[1]=0x3c;b[2]=1;b[6]=0x83;b[9]=0x30;
    y:[256]f32;
    #assert kq_decode(KQ_IQ2_XXS,1,256,b[..],y[..]).status==QzStatus.ok;
    // Scale 3: (0.5 + 3) / 4 = 7/8. Group 0: entry 1 = 43, 8, ..., 8; signs index 3 -> 0x03.
    #assert y[0]==-37.625 && y[1]==-7 && y[2]==7 && y[7]==7;
    // Group 1: entry 0 = eight 8s; signs index 1 -> 0x81, the parity bit flips element 15.
    #assert y[8]==-7 && y[9]==7 && y[14]==7 && y[15]==-7;
    // Sub-block 1 is all zero bytes: scale 0, (0.5 + 0) / 4 times 8.
    #assert y[32]==1;
    // IQ3_S, d = 0.5. Index byte 1 with qh bit 0 -> entry 257 = 5, 7, 9, 5; index 0 -> 1, 1, 1, 1.
    c:[110]u8;c[0]=0x00;c[1]=0x38;c[2]=0x01;c[66]=0x01;c[74]=0x92;c[106]=0x21;
    #assert kq_decode(KQ_IQ3_S,1,256,c[..],y[..]).status==QzStatus.ok;
    // Scale nibble 1: 0.5 (1 + 2) = 1.5. Sign byte 0x92 flips elements 1, 4 and 7.
    #assert y[0]==7.5 && y[1]==-10.5 && y[2]==13.5 && y[3]==7.5 && y[4]==-1.5 && y[5]==1.5 && y[7]==-1.5;
    // Sub-block 1 takes the high nibble, 2: 0.5 (1 + 4) = 2.5 times entry 0.
    #assert y[32]==2.5;
}
```

## Decoding a tensor

`kq_plan` uses chapter 49's table for the block sizes and refuses anything
that is not one of the twelve types (GGUF type 19, IQ1_S, is one it refuses). `kq_decode` checks every block's half
scales before it writes anything, so a refused tensor leaves the output as it
was. `kq_tensor` takes a tensor descriptor from chapter 49's reader and sends
chapter 50's Q8_0 and Q4_0 to chapter 50, so one entry decodes every format
Psionic reads.

<!-- source: psionic/lib/kquant.coax plan -->
```jai
kq_lattice_kind :: (kind:int)->bool {return (kind>=KQ_IQ2_XXS && kind<=KQ_IQ3_XXS) || kind==KQ_IQ3_S || kind==KQ_IQ2_S;}
kq_kind :: (kind:int)->bool {return (kind>=KQ_Q2_K && kind<=KQ_Q6_K) || kind==KQ_IQ4_NL || kind==KQ_IQ4_XS || kq_lattice_kind(kind);}
// Chapter 49's table gives each type's values and bytes per block.
kq_plan :: (kind:int,rows:int,width:int)->QzPlan #export_c {
    if !kq_kind(kind) {return QzPlan.{status=QzStatus.kind};}
    if rows<1 || width<1 {return QzPlan.{status=QzStatus.shape};}
    if width%PG_BLOCKS[kind]!=0 {return QzPlan.{status=QzStatus.partial};}
    if rows>KQ_GROUPS_MAX/(width/32) {return QzPlan.{status=QzStatus.limit};}
    blocks:=rows*width/PG_BLOCKS[kind];
    return QzPlan.{blocks=blocks,bytes=blocks*PG_WIDTHS[kind],values=rows*width,block_bytes=PG_WIDTHS[kind]};
}
// Where each block keeps its half scales: d, and dmin (d again when there is none).
kq_d_at :: (kind:int)->int {return ifx kind==KQ_Q2_K then 80 else ifx kind==KQ_Q3_K then 108 else ifx kind==KQ_Q6_K then 208 else 0;}
kq_m_at :: (kind:int)->int {return ifx kind==KQ_Q2_K then 82 else ifx kind==KQ_Q4_K || kind==KQ_Q5_K then 2 else kq_d_at(kind);}
```

<!-- source: psionic/lib/kquant.coax decode -->
```jai
kq_decode :: (kind:int,rows:int,width:int,bytes:[]u8,out:[]f32)->QzResult #export_c {
    p:=kq_plan(kind,rows,width);
    if p.status!=QzStatus.ok {return QzResult.{status=p.status};}
    if bytes.count!=p.bytes {return QzResult.{status=QzStatus.storage};}
    if out.count<p.values {return QzResult.{status=QzStatus.capacity};}
    size:=p.block_bytes;n:=PG_BLOCKS[kind];
    for blk:0..<p.blocks {
        b:=bytes[blk*size..<(blk+1)*size];
        if !qz_half_ok(kq_half_bits(b,kq_d_at(kind))) || !qz_half_ok(kq_half_bits(b,kq_m_at(kind))) {return QzResult.{status=QzStatus.scale};}
    }
    for i:0..<p.values {
        b:=bytes[(i/n)*size..<(i/n+1)*size];
        // |units| < 2^53, so the f64 is exact and f32 rounds once.
        if kq_lattice_kind(kind) {out[i]=f32(f64(kq_lattice(kind,b,i%n))/134217728);}
        else {out[i]=f32(f64(ifx kind==KQ_IQ4_NL || kind==KQ_IQ4_XS then kq_iq4(kind,b,i%n) else kq_value(kind,b,i%n))/16777216);}
    }
    return QzResult.{written=p.values};
}
// One tensor from chapter 49's reader: chapter 50's types go to chapter 50.
kq_tensor :: (t:PgTensor,file:[]u8,out:[]f32)->QzResult #export_c {
    if t.kind==QZ_Q8_0 || t.kind==QZ_Q4_0 {return qz_tensor(t,file,out);}
    if t.dims[0]<1 {return QzResult.{status=QzStatus.shape};}
    if t.begin<0 || t.end>file.count {return QzResult.{status=QzStatus.storage};}
    return kq_decode(t.kind,t.elements/t.dims[0],t.dims[0],file[t.begin..<t.end],out);
}
```

A plan holds at most 32,768 groups of 32 values, 1,048,576 values, the same
limit as chapter 50.

## An independent oracle

[kquant_oracle.py](../tests/kquant_oracle.py) decodes the same formats a
different way. It walks each block in ggml's loop order (for the K-quants,
two halves of 128, four shifts, two runs of 16; for the lattice formats,
`dequantize_row_iq*`'s loops over sub-blocks, groups and grid bytes, with
the sign table lookups as ggml does them) instead of computing each element's
position, keeps values as Python integers in 2⁻²⁴ units (2⁻²⁷ for the lattice
formats), and rounds them to binary32 with integer arithmetic, nearest with
ties to even. It runs no upstream code and no Coax. It reads the grids back
from the fixture and refuses them unless each table's SHA-256 matches the
digest it pins. It writes two pseudo-random blocks per format (eight for
IQ4_NL), with half scales that include subnormals, negative scales and the
largest finite half, and the 5,888 decoded values as f32 bit patterns, to
[kquant-vectors.coax](../fixtures/kquant-vectors.coax). Run without
arguments, it checks that neither fixture has drifted. A `#test` decodes
every block and compares every bit pattern.

## Quality cards

To measure a format, something has to write it. The checks file
[kquant-checks.coax](../tests/kquant-checks.coax) has a simple reference
quantizer for each format. It picks each sub-block's min (rounded up) and
then the scale that reaches the sub-block's top from it (also rounded up), so
the codes cover every value, and then rounds each value to its nearest code.
For the symmetric formats the value of largest magnitude maps to the most
negative code, as in chapter 50's Q4_0. A `#forall` over 126 cases of two
blocks checks the round trip: every error is within the format's step (for
example, half of a range of up to 2 × amax over Q2_K's 3 steps, or IQ4's 14
table units at the end of the table).

For the lattice formats the quantizer is a nearest grid point search. It
rounds `d` up so that scale code 15 reaches the block's largest magnitude at
the grid's top byte, and gives each sub-block the smallest scale code that
reaches its own largest magnitude. Each group of 8 takes the signs of its
values, flipping the sign of its smallest value when the format needs an even
count, and then the grid entry (or two 4-value entries for IQ3) with the
least squared distance to the magnitudes, by trying every entry. A `#forall`
over 15 cases of two blocks checks that every block decodes to exactly the
points the quantizer chose.

None of these is llama.cpp's quantizer. llama.cpp searches for better scales
and, for the lattice formats, weights the search by importance and handles
points off the grid with its own neighbour tables. A card here measures a
format as written by this simple quantizer, not the files people publish.

The stand-in model is chapter 50's with a wider first layer, because a
K-quant row must hold whole super-blocks: 256 inputs, a ReLU layer of 32 and
a readout over 16 options. Only the 32 × 256 first layer is quantized; the
readout stays f32. Each card uses chapter 50's `qz_card` with the same bound,
declared before measuring: KL plus two standard errors at most 0.01, and
top-1 agreement minus two standard errors at least 0.9. The example also
prints the layer's weight error: the squared error over the squared weights.

## Run it

The example, [kquant.coax](../examples/kquant.coax), decodes the hand blocks,
checks the tables' digests, compares the oracle's vectors, reads a tensor,
prints the fourteen cards and shows six refusals:

```shell
coax/bin/coax run psionic/examples/kquant.coax
```

<!-- source: psionic/examples/kquant.coax example -->
```jai
main :: () {
    // The hand Q2_K block: d = 0.5, dmin = 1, four (scale | min << 4) bytes.
    q2:[84]u8;q2[0]=0x21;q2[2]=0x13;q2[4]=0x05;q2[6]=0xf2;q2[16]=0xe4;q2[81]=0x38;q2[83]=0x3c;y:[512]f32;
    #assert kq_decode(KQ_Q2_K,1,256,q2[..],y[..]).status==QzStatus.ok;
    print("Q2_K elements 0, 32, 64, 96: % % % %\n",y[0],y[32],y[64],y[96]);

    // The hand Q4_K block: sub-block 4's 6-bit scale and min are split across three bytes.
    q4:[144]u8;q4[1]=0x38;q4[3]=0x3c;q4[4]=0xc3;q4[8]=0x41;q4[12]=0x25;q4[16]=0x9a;q4[80]=0x07;
    #assert kq_decode(KQ_Q4_K,1,256,q4[..],y[..]).status==QzStatus.ok;
    print("Q4_K scale/min 0: %/%, 4: %/%; elements 0 and 128: % %\n",
        kq_scale_min(q4[..],0,0),kq_scale_min(q4[..],0,1),kq_scale_min(q4[..],4,0),kq_scale_min(q4[..],4,1),y[0],y[128]);

    // IQ4_NL: one half scale and a 16-entry table of non-uniform levels.
    nl:[18]u8;nl[1]=0x38;nl[2]=0xf0;
    #assert kq_decode(KQ_IQ4_NL,1,32,nl[..],y[..]).status==QzStatus.ok;
    print("IQ4 levels % ... %; IQ4_NL elements 0 and 16: % %\n",KQ_IQ4[0],KQ_IQ4[15],y[0],y[16]);

    // IQ2_XXS: grid entry 1 = 43, 8, ..., 8 at scale (0.5 + 3) / 4; signs index 3 flips elements 0 and 1.
    x2:[66]u8;x2[1]=0x3c;x2[2]=1;x2[6]=0x83;x2[9]=0x30;
    #assert kq_decode(KQ_IQ2_XXS,1,256,x2[..],y[..]).status==QzStatus.ok;
    print("IQ2_XXS elements 0 to 2, 8, 15 and 32: % % % % % %\n",y[0],y[1],y[2],y[8],y[15],y[32]);
    // IQ3_S: entry 257 = 5, 7, 9, 5 at 0.5 (1 + 2); sign byte 0x92 flips elements 1, 4 and 7.
    x3:[110]u8;x3[1]=0x38;x3[2]=0x01;x3[66]=0x01;x3[74]=0x92;x3[106]=0x21;
    #assert kq_decode(KQ_IQ3_S,1,256,x3[..],y[..]).status==QzStatus.ok;
    print("IQ3_S elements 0 to 7 and 32: % % % % % % % % %\n",y[0],y[1],y[2],y[3],y[4],y[5],y[6],y[7],y[32]);
    print("ggml's sign and grid tables match the SHA-256 recorded at extraction: %\n",kq_tables_ok());

    // The independent oracle's blocks, compared bit for bit.
    names:=[12]string.["Q2_K","Q3_K","Q4_K","Q5_K","Q6_K","IQ4_NL","IQ4_XS","IQ2_XXS","IQ2_XS","IQ2_S","IQ3_XXS","IQ3_S"];
    kinds:=[12]int.[10,11,12,13,14,20,23,16,17,22,18,21];
    for i:0..<12 {
        p:=kq_plan(kinds[i],1,256);
        print("% (GGUF type %): % bytes per 256 values (% bits each), oracle vectors match: %\n",
            names[i],kinds[i],p.bytes,f64(p.bytes*8)/256,kq_oracle(i,y[..]));
    }

    // Chapter 49's descriptor of a 256 x 2 Q4_K tensor over the oracle's bytes.
    file:=PG_OWNED;m:[15]PgMeta;ms:[15]PgMeta;t:[2]PgTensor;ts:[2]PgTensor;
    #assert pg_read(file[..],file.count,m[..],ms[..],t[..],ts[..],false).status==PgStatus.ok;
    k:=t[1];k.kind=KQ_Q4_K;k.dims[0]=256;k.dims[1]=2;k.elements=512;k.begin=0;k.end=288;
    payload:=KQV_Q4K_BYTES;
    r:=kq_tensor(k,payload[..],y[..]);
    print("tensor type % (% x %): %, % values; chapter 50's '%' (type %): %\n",
        k.kind,k.dims[0],k.dims[1],r.status,r.written,t[1].name,t[1].kind,kq_tensor(t[1],file[..],y[..]).status);

    // Quality cards: the stand-in's 32 x 256 first layer in each format.
    print("declared: KL + 2 SE <= %, agreement - 2 SE >= %; the f32 layer: % bytes\n",qz_declared().kl,qz_declared().agreement,KQ_HIDDEN*KQ_IN*4);
    for i:0..<KQ_PROFILES {
        error:[1]f64;c:=kq_standin_measure(i,error[..]);
        print("%: % bytes, weight error %, agreement % (low %), KL % (high %), perplexity % -> %, promoted %\n",
            KQ_PROFILE_NAMES[i],c.bytes,error[0],c.agreement,c.agreement_low,c.kl,c.kl_high,c.perplexity_original,c.perplexity_quantized,c.promoted);
    }

    // Refusals, each typed; none writes its output.
    y[0]=-7;
    print("GGUF type 19 (IQ1_S): %\n",kq_decode(19,1,256,q2[0..<50],y[..]).status);
    print("a Q2_K block one byte short: %\n",kq_decode(KQ_Q2_K,1,256,q2[0..<83],y[..]).status);
    print("a Q4_K row of 128 values: %\n",kq_decode(KQ_Q4_K,1,128,q4[..],y[..]).status);
    q2[82]=0x00;q2[83]=0x7e;print("a Q2_K block whose dmin is NaN: %\n",kq_decode(KQ_Q2_K,1,256,q2[..],y[..]).status);
    print("an IQ2_XXS block one byte short: %\n",kq_decode(KQ_IQ2_XXS,1,256,x2[0..<65],y[..]).status);
    x3[1]=0x7c;print("an IQ3_S block whose d is infinite: %\n",kq_decode(KQ_IQ3_S,1,256,x3[..],y[..]).status);
    print("output left unchanged: %\n",y[0]);
}
```

It prints:

```text
Q2_K elements 0, 32, 64, 96: -2.0 0.5 5.0 -12.0
Q4_K scale/min 0: 3/1, 4: 53/18; elements 0 and 128: 14.0 167.5
IQ4 levels -127 ... 113; IQ4_NL elements 0 and 16: -63.5 56.5
IQ2_XXS elements 0 to 2, 8, 15 and 32: -37.625 -7.0 7.0 -7.0 -7.0 1.0
IQ3_S elements 0 to 7 and 32: 7.5 -10.5 13.5 7.5 -1.5 1.5 1.5 -1.5 2.5
ggml's sign and grid tables match the SHA-256 recorded at extraction: true
Q2_K (GGUF type 10): 84 bytes per 256 values (2.625 bits each), oracle vectors match: true
Q3_K (GGUF type 11): 110 bytes per 256 values (3.4375 bits each), oracle vectors match: true
Q4_K (GGUF type 12): 144 bytes per 256 values (4.5 bits each), oracle vectors match: true
Q5_K (GGUF type 13): 176 bytes per 256 values (5.5 bits each), oracle vectors match: true
Q6_K (GGUF type 14): 210 bytes per 256 values (6.5625 bits each), oracle vectors match: true
IQ4_NL (GGUF type 20): 144 bytes per 256 values (4.5 bits each), oracle vectors match: true
IQ4_XS (GGUF type 23): 136 bytes per 256 values (4.25 bits each), oracle vectors match: true
IQ2_XXS (GGUF type 16): 66 bytes per 256 values (2.0625 bits each), oracle vectors match: true
IQ2_XS (GGUF type 17): 74 bytes per 256 values (2.3125 bits each), oracle vectors match: true
IQ2_S (GGUF type 22): 82 bytes per 256 values (2.5625 bits each), oracle vectors match: true
IQ3_XXS (GGUF type 18): 98 bytes per 256 values (3.0625 bits each), oracle vectors match: true
IQ3_S (GGUF type 21): 110 bytes per 256 values (3.4375 bits each), oracle vectors match: true
tensor type 12 (256 x 2): ok, 512 values; chapter 50's 'quants' (type 2): ok
declared: KL + 2 SE <= 0.01, agreement - 2 SE >= 0.9; the f32 layer: 32768 bytes
Q8_0: 8704 bytes, weight error 0.00007846212150888891, agreement 1.0 (low 1.0), KL 0.00022960896719454973 (high 0.00028397003336496973), perplexity 2.3511811667797677 -> 2.35421236686352, promoted true
Q4_0: 4608 bytes, weight error 0.02001822009472524, agreement 0.8125 (low 0.714921906275025), KL 0.050112980281734595 (high 0.06036053356438464), perplexity 2.3511811667797677 -> 2.5021304257374326, promoted false
Q2_K: 2688 bytes, weight error 0.19218370894487455, agreement 0.578125 (low 0.454660307583757), KL 0.4218520012100292 (high 0.5204050973854987), perplexity 2.3511811667797677 -> 3.5890874882700423, promoted false
Q3_K: 3520 bytes, weight error 0.06496858073598444, agreement 0.8125 (low 0.714921906275025), KL 0.12899541365876996 (high 0.1644972986082705), perplexity 2.3511811667797677 -> 2.599784901002744, promoted false
Q4_K: 4608 bytes, weight error 0.012416521485529507, agreement 0.9375 (low 0.8769846352155091), KL 0.02808012551034966 (high 0.0347936353432874), perplexity 2.3511811667797677 -> 2.4562710262184893, promoted false
Q5_K: 5632 bytes, weight error 0.0029277597754099323, agreement 0.953125 (low 0.9002822294591067), KL 0.006410056397020745 (high 0.007941411979327073), perplexity 2.3511811667797677 -> 2.420125305386027, promoted true
Q6_K: 6720 bytes, weight error 0.0008721908771155072, agreement 0.96875 (low 0.9252518409153905), KL 0.002086438434421452 (high 0.0026257607518881446), perplexity 2.3511811667797677 -> 2.339389045465552, promoted true
IQ4_NL: 4608 bytes, weight error 0.012081581594098985, agreement 0.890625 (low 0.8125977173615696), KL 0.029766057060308375 (high 0.03668478416335541), perplexity 2.3511811667797677 -> 2.416809699701994, promoted false
IQ4_XS: 4352 bytes, weight error 0.012725717998271931, agreement 0.890625 (low 0.8125977173615696), KL 0.027503436237209685 (high 0.033781521455899015), perplexity 2.3511811667797677 -> 2.424072909713312, promoted false
IQ2_XXS: 2112 bytes, weight error 0.23191023687935494, agreement 0.625 (low 0.5039692704310182), KL 0.44785453935220393 (high 0.5665987293326521), perplexity 2.3511811667797677 -> 3.9575460154241617, promoted false
IQ2_XS: 2368 bytes, weight error 0.14996201148842014, agreement 0.671875 (low 0.5544923499897131), KL 0.2871906681991752 (high 0.36445193094847705), perplexity 2.3511811667797677 -> 3.3749839656066194, promoted false
IQ2_S: 2624 bytes, weight error 0.13463573439810855, agreement 0.5625 (low 0.4384804072938473), KL 0.3590652493940362 (high 0.4584122407740423), perplexity 2.3511811667797677 -> 3.7998738463627, promoted false
IQ3_XXS: 3136 bytes, weight error 0.05509053273919221, agreement 0.765625 (low 0.6597231116271409), KL 0.1281880105343791 (high 0.15547034577675153), perplexity 2.3511811667797677 -> 2.525603003200908, promoted false
IQ3_S: 3520 bytes, weight error 0.032627594238591295, agreement 0.703125 (low 0.58890475377177), KL 0.10044651668717734 (high 0.12224005832735196), perplexity 2.3511811667797677 -> 2.6613454382266672, promoted false
GGUF type 19 (IQ1_S): kind
a Q2_K block one byte short: storage
a Q4_K row of 128 values: partial
a Q2_K block whose dmin is NaN: scale
an IQ2_XXS block one byte short: storage
an IQ3_S block whose d is infinite: scale
output left unchanged: -7.0
```

Reading it:

- The first five lines are the hand calculations above, and the sixth says
  the embedded ggml tables hash to the digests recorded when they were
  extracted.
- All twelve formats match the oracle's 5,888 values bit for bit, and a Q4_K
  tensor described by chapter 49 decodes through `kq_tensor`, as does chapter
  50's Q4_0 tensor.
- The K-quant ladder is in order: each extra bit lowers the mean KL, from
  0.42 for Q2_K to 0.0021 for Q6_K. Under the declared bound, Q8_0, Q6_K and
  Q5_K are promoted; Q5_K only just, with its agreement's lower end at 0.9003.
- No format of about 4 bits is promoted on this stand-in. Among them, Q4_K
  (KL 0.028) and the two IQ4 formats (0.030 and 0.028) lose about half as much
  as Q4_0 (0.050), and IQ4_XS does it in 256 fewer bytes than Q4_K.
- No lattice format is promoted either; at 2 to 3.4 bits none was expected
  to be. Their weight errors fall with each extra bit, from 0.232 for IQ2_XXS
  to 0.033 for IQ3_S. Against the K-quants: IQ3_S has half Q3_K's weight
  error (0.033 against 0.065) in the same 110 bytes a block, and lower KL
  (0.100 against 0.129); IQ2_XS has less weight error and KL than Q2_K
  (0.150 and 0.287 against 0.192 and 0.422) in 74 bytes instead of 84.
- KL follows the weight error only roughly. IQ2_S has less weight error than
  IQ2_XS but higher KL (0.359 against 0.287), and IQ3_S's agreement is below
  IQ3_XXS's. With 64 items, the KL's upper ends (0.458 and 0.364) overlap:
  the stand-in cannot order formats this close.
- Each refusal names its reason: GGUF type 19 (IQ1_S, not decoded) is
  `kind`, a block one byte short is `storage` (in a K-quant or a lattice
  format), a row of 128 values cannot hold a super-block (`partial`), and a
  NaN `dmin` or an infinite `d` is `scale`. The last line shows that none
  wrote the output.

## What the checks compare

The checks in [kquant.coax](../lib/kquant.coax) were written before the code.
They compare:

- each half's integer units against chapter 50's `qz_half`, as a `#forall`
  over 4,096 bit patterns and both signs;
- each plan's sizes against the block structs, the limit at exactly 32,768
  groups, and each refusal;
- the hand blocks above, value by value;
- each ggml table's SHA-256 against the digest recorded at extraction, and
  the sign tables against the rule they follow;
- every value of the oracle's blocks, bit for bit;
- wrong types, partial rows, truncated blocks, short outputs, and an infinite
  or NaN `d` or `dmin` in every format, each refused before any output is
  written;
- the reference quantizers' round trips, within each format's step, and
  for the lattice formats exactly the grid points chosen;
- tensors from chapter 49's descriptors, including chapter 50's types and
  each descriptor refusal;
- the fourteen stand-in cards: their sizes, which are promoted, that the
  K-quant KL and the lattice weight error fall with each extra bit, and that
  IQ3_S and IQ2_XS lose less than Q3_K and Q2_K.

The library is an `#authority` file, because it admits or refuses blocks read
from files. `coax mutate --root-only` (the units it loads check themselves)
leaves zero survivors of 629 mutants.

## What remains

- llama.cpp's quantizers for these formats (`make_qkx2_quants`, the
  importance-weighted searches and the lattice formats' neighbour search), so
  cards measure the files people publish. The cards here use the simple
  reference quantizers above.
- Cards measured on a named model's original weights, and perplexity on fixed
  text; the stand-in here only shows the method.
- IQ1_S and IQ1_M (GGUF types 19 and 29), and the other GGUF types chapter 49
  lists, are refused.

## License

The IQ4 table above and the K-quant and IQ4 block layouts follow ggml's
`ggml-common.h` at llama.cpp `0893ec98f5eafad36a43a72cd1c3723d72b0d03a`. The
IQ2 and IQ3 grids, the sign tables and the lattice block layouts are copied
from `ggml-common.h` at `9b05354ec6fb58b4e665e9a39ebc40285c015638`, and the
lattice decoders follow `dequantize_row_iq*` in `ggml-quants.c` at the same
commit. All are MIT licensed, "Copyright (c) 2023-2026 The ggml authors". The
notice is in [psionic/NOTICE](../NOTICE).

## Dependencies

- [Chapter 49](ch049-read-a-gguf-file.md): the GGUF container and its block
  size table.
- [Chapter 50](ch050-quantized-weights-and-caches.md): half precision, Q8_0
  and Q4_0, and the quality card.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): format sources
  and licensing.
