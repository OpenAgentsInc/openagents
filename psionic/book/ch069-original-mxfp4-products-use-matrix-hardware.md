# 69. Original MXFP4 products use matrix hardware

> **Status: partly implemented.** The MXFP4-BF16-MMA-v1 expert products, their
> frozen bound, Coax admission and the comparison with chapter 66's f32
> kernels run here, on the CPU and on an RTX 4080, with synthetic weights and
> the original layer 0 expert tensors. No full-model step uses the profile
> yet; the last section lists what remains.

The original gpt-oss experts are stored as MXFP4: four-bit codes with one
power-of-two scale for every 32 values ([chapter 60](ch060-original-mxfp4-weights.md)).
[Chapter 66](ch066-a-complete-cuda-decoder.md) multiplies them on the GPU
with ordinary f32 arithmetic, one product and one add at a time. The GPU's
tensor cores multiply whole 16 x 16 tiles in one instruction, but only BF16
tiles, and they add in an order the hardware chooses. This chapter runs the
expert products on the tensor cores under a separately named numerical
profile, MXFP4-BF16-MMA-v1, writes its error bound down before any
comparison, and measures whether the change is worth it.

The f32 profile, gpt-oss-cuda-f32-v1, stays exactly as it is. Nothing here
replaces it; a caller has to plan the new profile by name, and a device
without the instruction refuses it.

## Coefficients are staged exactly

The tensor-core primitive from [chapter 37](ch037-a-coax-compute-subset.md),
`cu_mma_bf16_16x8x16`, takes its 16 x 16 matrix as BF16 pairs, two to a
`u32`: the even column in the low half. One packed MXFP4 byte also holds two
columns, the even one in its low nibble. So one byte becomes one fragment
word.

Take the byte `0xA3` with scale byte 128. The scale means 2^(128-127) = 2.
The low nibble 3 is the E2M1 value 1.5, so the even column is 1.5 · 2 = 3.
The high nibble `0xA` has the sign bit set and code 2, the value 1.0, so the
odd column is -2. As f32, 3 is `0x40400000` and -2 is `0xC0000000`. Their top
halves, `0x4040` and `0xC000`, are exactly BF16 3 and -2, so the word is
`0xC0004040`.

The top half is exact for every byte and every scale chapter 60 admits. An
E2M1 value has at most two significant bits, and multiplying by a power of
two only moves them, so the f32 product of a code and its scale is exact and
its low 16 bits are zero. Even the smallest, 0.5 · 2^-127 = 2^-128, is the
f32 bit pattern `0x00200000`. The first check compares all 16 codes at all
253 admitted scales with chapter 60's independent gold bits.

<!-- source: psionic/lib/device/mxfp4-mma.coax decode -->
```jai
// One original coefficient as BF16 bits. The f32 product of an E2M1 value
// (at most two significant bits) and a power of two is exact, and its low 16
// bits are zero, so dropping them loses nothing.
mm_coefficient :: (code:u8,power:f32)->u32 {
    return f32_bits(gk_fp4(cast(u64)code)*power)>>16;
}
// One packed byte, two columns: the low nibble is the even column and goes
// in the low half of the fragment word, as cu_mma_bf16_16x8x16 expects.
mm_pair :: (byte:u8,power:f32)->u32 {
    return mm_coefficient(byte&15,power)|(mm_coefficient(byte>>4,power)<<16);
}
```

## Activations become two BF16 limbs

The activations are f32, and rounding them to BF16 would keep only 8
significant bits. [Chapter 37](ch037-a-coax-compute-subset.md) splits each
one into two BF16 limbs, `hi = bf16(x)` and `lo = bf16(x - hi)`, and
multiplies by both. For x = 1.1 (f32 `0x3F8CCCCD`), hi is `0x3F8D`, which is
1.1015625. The remainder, x - hi = -0.0015625 approximately, rounds to the
BF16 value -0.0015640259. Their sum is 1.5 · 10^-6 from x, below 2^-17 of x,
where hi alone was 1.6 · 10^-3 away.

<!-- source: psionic/lib/device/mxfp4-mma.coax split -->
```jai
// Item (slot*padded+token)*(cols/2)+p: columns 2p and 2p+1 of one token's
// input as high and low BF16 limbs. Slot s reads x from s*stride (stride 0:
// every slot shares one input, as the first expert projection does).
// `padded` is tokens rounded up to 8; tokens from `tokens` to `padded` are
// zero.
mm_split :: (item:int,cols:int,tokens:int,padded:int,stride:int,x:[]f32,hi:[]u32,lo:[]u32) #export_c {
    half:=cols/2;p:=item%half;row:=item/half;token:=row%padded;slot:=row/padded;h:u32=0;l:u32=0;
    if token<tokens {
        at:=slot*stride+token*cols+2*p;a:=x[at];b:=x[at+1];
        ha:=dw_bf16(a);hb:=dw_bf16(b);
        la:=dw_bf16(a-f32_from_bits(ha<<16));lb:=dw_bf16(b-f32_from_bits(hb<<16));
        h=ha|(hb<<16);l=la|(lb<<16);
    }
    hi[item]=h;lo[item]=l;
}
```

`mm_split` reuses chapter 37's rounding, `dw_bf16`. A stride of 0 lets the
four selected experts of the first projection share one input; the second
projection gives each expert its own.

## One warp, sixteen rows, thirty-two tokens

`mm_part` is a warp kernel: each work item is one warp of 32 lanes. A warp
owns 16 rows of one selected expert and up to 32 tokens of that expert's
inputs. For every 16 columns it decodes one coefficient fragment straight
from the packed bytes and scales, then, for each group of 8 tokens, runs the
instruction twice, once with the high limbs and once with the low ones, into
the same f32 accumulator.

Lane l holds rows g = l/4 and g + 8 and columns 2t, 2t + 1, 2t + 8 and
2t + 9 of each step, with t = l % 4. Those four columns are the bytes `t` and
`t + 4` of the step's eight bytes, and both lie in one 32-value block, so each
lane reads two scales per step.

<!-- source: psionic/lib/device/mxfp4-mma.coax part -->
```jai
// One warp: rows r0..r0+15 of expert ids[slot] times tokens t0..t0+31 of that
// slot, over one slice of the columns. Work item
// ((slot*groups+group)*blocks+block)*slices+slice. Each 16-column step
// decodes a 16x16 coefficient fragment from the packed bytes and adds A hi
// and A lo into f32 fragments. Writes
// partial[((slice*slots+slot)*tokens+token)*rows+row] for mm_finish, and
// marks there 1 when the lane read a scale above 252 (chapter 60 refuses
// them; chapter 66's kernels make such outputs non-finite instead).
mm_part :: (item:int,rows:int,cols:int,tokens:int,slots:int,slices:int,packed:[]u8,scales:[]u8,ids:[]int,hi:[]u32,lo:[]u32,partial:[]f32,marks:[]u8) #export_c {
    slice:=item%slices;rest:=item/slices;blocks:=rows/16;block:=rest%blocks;rest=rest/blocks;
    groups:=(tokens+MM_TOKENS-1)/MM_TOKENS;group:=rest%groups;slot:=rest/groups;
    expert:=ids[slot];r0:=block*16;t0:=group*MM_TOKENS;nb:=cols/32;half:=cols/2;per:=cols/16/slices;
    padded:=(tokens+7)/8*8;used:=(tokens-t0+7)/8;
    fa:[128]u32;fb:[64]u32;fl:[64]u32;fc:[128]f32;acc:[512]f32;bad:[32]u8;
    for s:0..<per {
        c0:=(slice*per+s)*16;
        for lane:0..<32 {
            g:=lane/4;t:=lane%4;flag:=bad[lane*1];
            for h:0..<2 {
                blk:=(expert*rows+r0+g+8*h)*nb+c0/32;scale:=scales[blk];
                if scale>252 {flag=1;}
                power:=gk_power(scale);at:=blk*16+c0%32/2+t;
                fa[lane*4+h]=mm_pair(packed[at],power);fa[lane*4+2+h]=mm_pair(packed[at+4],power);
            }
            bad[lane*1]=flag;
        }
        for tile:0..<MM_TILES {
            if tile<used {
                for lane:0..<32 {
                    xb:=(slot*padded+t0+tile*8+lane/4)*half+c0/2+lane%4;
                    fb[lane*2]=hi[xb];fb[lane*2+1]=hi[xb+4];fl[lane*2]=lo[xb];fl[lane*2+1]=lo[xb+4];
                    for j:0..<4 {fc[lane*4+j]=acc[lane*16+tile*4+j];}
                }
                cu_mma_bf16_16x8x16(fa[..],fb[..],fc[..]);
                cu_mma_bf16_16x8x16(fa[..],fl[..],fc[..]);
                for lane:0..<32 {for j:0..<4 {acc[lane*16+tile*4+j]=fc[lane*4+j];}}
            }
        }
    }
    for lane:0..<32 {
        for tile:0..<MM_TILES {
            for j:0..<4 {
                row:=r0+lane/4+8*(j/2);token:=t0+tile*8+2*(lane%4)+j%2;
                if token<tokens {
                    at:=((slice*slots+slot)*tokens+token)*rows+row;partial[at]=acc[lane*16+tile*4+j];marks[at]=bad[lane*1];
                }
            }
        }
    }
}
```

The columns can be split into slices, each with its own accumulator, so one
token's product has enough warps to fill the GPU. `mm_finish` adds the slices
in binary64 in slice order and adds the bias, the same final step as chapter
66's `gk_mx_finish`.

<!-- source: psionic/lib/device/mxfp4-mma.coax finish -->
```jai
// Item (slot*tokens+token)*rows+row: the slice partials added in f64 in
// slice order, rounded to f32, plus the expert's BF16 bias, as gk_mx_finish.
// A slice that read a scale above 252 marks the output bad.
mm_finish :: (item:int,rows:int,tokens:int,slots:int,slices:int,ids:[]int,partial:[]f32,marks:[]u8,bias:[]u16,out:[]f32,flags:[]u8) #export_c {
    row:=item%rows;slot:=item/rows/tokens;sum:f64=0;
    bad:u8=0;
    for s:0..<slices {at:=s*slots*tokens*rows+item;sum+=f64(partial[at]);bad|=marks[at];}
    value:=f32(sum)+gk_bf(bias[ids[slot]*rows+row]);out[item]=value;flags[item]=gk_bad(value)|bad;
}
```

A scale byte above 252 is refused by chapter 60. On the device a kernel
cannot refuse, so a lane that reads one marks every output it writes, and
`mm_finish` flags them. A check sets row 9's scale to 253 and shows that
exactly rows 1 and 9, the two rows of the lanes that read it, are flagged.

Here is the smallest whole product. One expert has 16 rows by 32 columns,
every byte `0x22` (both codes 1.0) at scale 127 (power 1), and one token's
inputs are 0, 1, ..., 31. There are two 16-column steps and two limbs, so
each output takes four instructions. The high limbs of these integers are
exact and the low limbs are zero, so every row is 0 + 1 + ... + 31 = 496, and
row 3, with a bias of 1, is 497. The check asserts these values exactly.

## The bound, written before comparing

<!-- source: psionic/lib/device/mxfp4-mma.coax profile -->
```jai
MM_PROFILE :: "MXFP4-BF16-MMA-v1";
// The bound, frozen before any comparison: with n tensor-core
// instructions adding into one f32 accumulator, every output y satisfies
//   |y - y64| <= (2^-16 + (n+2) * 2^-20) * (sum |w x| + |bias|)
// where y64 is the binary64 sum of the exactly decoded coefficients times
// the f32 activations, plus the bias. 2^-17 covers the two-limb split of x
// (chapter 37), 2^-20 per instruction covers the tensor core's own f32
// accumulation (chapter 37's frozen conformance bound), and the rest covers
// the f64 slice sum, the final f32 rounding and the bias add.
mm_bound :: (instructions:int,magnitude:f64)->f64 {
    return (1.0/65536.0+f64(instructions+2)/1048576.0)*magnitude;
}
// Two instructions (high and low limb) per 16-column step of one slice.
mm_instructions :: (cols:int,slices:int)->int {return 2*(cols/16/slices);}
```

Each term comes from something already measured or proved. The split costs
at most 2^-17 of each product. Chapter 37 froze the tensor core's own error
at 2^-20 of `|C| + sum |A B|` per instruction, and the accumulator never
exceeds the running `sum |w x|`. The f64 slice sum, the final rounding and
the bias add are each below 2^-24 of the magnitude. For the 16 x 32 product
above, n = 4 and the bound is (16 + 6) · 2^-20 = 2.1 · 10^-5 of
`sum |w x| + |bias|`. For 2,880 columns in 4 slices, n = 90 and the bound is
1.03 · 10^-4.

The reference y64 does not share code with the kernels: `mm_reference` reads
each coefficient from chapter 60's gold bit table and adds in binary64. The
checks also run chapter 66's `gk_mx_part` and `gk_mx_finish` on the same
inputs and require its comparison rule,
`|candidate - reference| / (1 + |reference|) < 0.0005`, on every output.

## Planning refuses devices without the instruction

<!-- source: psionic/lib/device/mxfp4-mma-plan.coax plan -->
```jai
MmStatus :: enum u8 {ok;shape;slices;tokens;device;}
// A launch of the profile: shape refusals first, then the device.
MmPlan :: struct {status:MmStatus;admission:DaStatus;split_items:int;part_items:int;finish_items:int;instructions:int;}
// What mm_part needs: one warp, registers only, and the tensor-core
// instruction.
MM_NEEDS :: DaNeeds.{threads=32,shared=0,mma=true};
mm_plan :: (rows:int,cols:int,tokens:int,slots:int,slices:int,dev:DaDevice)->MmPlan {
    if rows<16 || rows%16!=0 || cols<32 || cols%32!=0 || slots<1 {return MmPlan.{status=MmStatus.shape};}
    if slices<1 || cols%(16*slices)!=0 {return MmPlan.{status=MmStatus.slices};}
    if tokens<1 || tokens>4096 {return MmPlan.{status=MmStatus.tokens};}
    admitted:=da_admit(MM_NEEDS,dev);
    if admitted!=DaStatus.ok {return MmPlan.{status=MmStatus.device,admission=admitted};}
    groups:=(tokens+MM_TOKENS-1)/MM_TOKENS;
    return MmPlan.{split_items=slots*((tokens+7)/8*8)*cols/2,part_items=slots*groups*rows/16*slices,
        finish_items=slots*tokens*rows,instructions=mm_instructions(cols,slices)};
}
mm_text :: (p:MmPlan)->string {
    if p.status==MmStatus.ok {return "admitted";}
    if p.status==MmStatus.shape {return "refused: rows must be a multiple of 16 and columns a multiple of 32";}
    if p.status==MmStatus.slices {return "refused: the slices must divide the 16-column steps";}
    if p.status==MmStatus.tokens {return "refused: tokens must be 1 to 4096";}
    return da_text(p.admission);
}
```

`mm_plan` refuses malformed shapes and slice counts first, then asks
[chapter 68](ch068-coax-kernels-cooperate.md)'s `da_admit` about the device.
`mm_part` needs one warp, no shared memory and the tensor-core instruction,
so a device of compute capability 7.5 is refused. The host reports the
device's numbers and obeys the answer; `mxfp4-mma-plan.coax` is an
`#authority` file.

## Run it on the CPU

The [example](../examples/mxfp4-mma.coax) stages the byte from the first
section, splits 1.1, runs a 16 x 32 product with a fixed byte pattern through
both profiles, and plans launches at gpt-oss-20b's shapes. It needs no GPU:

```shell
coax/bin/coax run psionic/examples/mxfp4-mma.coax
```

```text
byte 0xa3 at scale 128: fragment word 0xc0004040, columns 3.0 and -2.0
f32 1.1 splits into 1.1015625 + -0.0015640259, 1.5497207641601562e-06 from it
row 0: MMA -0.3434628, f32 -0.3434627, binary64 -0.3434626960006426
16 of 16 rows within the frozen bound (0.0000209808349609375 of sum |w x| for 4 instructions); worst MMA error 1.7906476621127123e-06, worst f32 error 7.077312477137007e-08
chapter 66's rule holds for 16 of 16 rows
profile MXFP4-BF16-MMA-v1
first expert projection, 1 token, 4 slots, 4 slices on compute capability 8.9: admitted, 5760 warps, 90 instructions per accumulator
second expert projection, 32 tokens per slot, 1 slice: admitted, 720 warps, 360 instructions per accumulator
compute capability 7.5: refused: cu_mma_bf16_16x8x16 needs compute capability 8.0 or later
2880 columns in 7 slices: refused: the slices must divide the 16-column steps
5761 rows: refused: rows must be a multiple of 16 and columns a multiple of 32
```

The last three lines are refusals: a device without the instruction, 2,880
columns in 7 slices (180 steps do not divide by 7), and 5,761 rows.

<!-- source: psionic/examples/mxfp4-mma.coax example -->
```jai
// Chapter 69 on the CPU: stage one packed byte, split one activation, run a
// small expert product through the three MXFP4-BF16-MMA-v1 kernels and
// chapter 66's f32 kernels, compare both with the binary64 reference, then
// plan launches at gpt-oss-20b's shape.
main :: () {
    word:=mm_pair(0xa3,gk_power(128));
    print("byte 0xa3 at scale 128: fragment word 0x");
    digits:="0123456789abcdef";
    for i:0..<8 {d:=cast(int)((word>>cast(u32)(28-4*i))&15);print("%",digits[d..<d+1]);}
    print(", columns % and %\n",f32_from_bits(word<<16),f32_from_bits((word>>16)<<16));
    x:=[2]f32.[1.1,0];hi:[1]u32;lo:[1]u32;
    mm_split(0,2,1,1,0,x[..],hi[..],lo[..]);
    h:=f32_from_bits(hi[0]<<16);l:=f32_from_bits(lo[0]<<16);
    print("f32 1.1 splits into % + %, % from it\n",h,l,f64(x[0])-f64(h)-f64(l));

    // One expert of 16 rows by 32 columns from a fixed byte pattern, one
    // token: 4 tensor-core instructions per output.
    rows:=16;cols:=32;
    packed:[256]u8;scales:[16]u8;bias:[16]u16;ids:=[1]int.[0];input:[32]f32;
    mm_fill(packed[..],scales[..],bias[..]);mm_inputs(input[..]);
    limbs_hi:[128]u32;limbs_lo:[128]u32;partial:[16]f32;marks:[16]u8;out:[16]f32;flags:[16]u8;
    n:=mm_run(rows,cols,1,1,1,0,packed[..],scales[..],ids[..],bias[..],input[..],limbs_hi[..],limbs_lo[..],partial[..],marks[..],out[..],flags[..]);
    words:[32]u64;for i:0..<32 {w:u64=0;for b:0..<8 {w|=cast(u64)packed[i*8+b]<<cast(u64)(8*b);}words[i]=w;}
    gp:[256]f32;f32_out:[16]f32;f32_flags:[16]u8;
    for i:0..<rows*16 {gk_mx_part(i,rows,cols,16,0,words[..],scales[..],ids[..],input[..],gp[..]);}
    for i:0..<rows {gk_mx_finish(i,rows,16,ids[..],gp[..],bias[..],f32_out[..],f32_flags[..]);}
    worst:f64=0;worst_f32:f64=0;within:=0;agree:=0;
    for r:0..<rows {
        ref:=mm_reference(rows,cols,0,0,r,0,0,packed[..],scales[..],bias[..],input[..]);
        e:=abs(f64(out[r])-ref.value)/ref.magnitude;if e>worst {worst=e;}
        g:=abs(f64(f32_out[r])-ref.value)/ref.magnitude;if g>worst_f32 {worst_f32=g;}
        if abs(f64(out[r])-ref.value)<=mm_bound(n,ref.magnitude) {within+=1;}
        if mm_rule_66(out[r],f32_out[r]) {agree+=1;}
    }
    print("row 0: MMA %, f32 %, binary64 %\n",out[0],f32_out[0],mm_reference(rows,cols,0,0,0,0,0,packed[..],scales[..],bias[..],input[..]).value);
    print("% of % rows within the frozen bound (% of sum |w x| for % instructions); worst MMA error %, worst f32 error %\n",
        within,rows,mm_bound(n,1),n,worst,worst_f32);
    print("chapter 66's rule holds for % of % rows\n",agree,rows);

    print("profile %\n",MM_PROFILE);
    rtx4080:=DaDevice.{major=8,minor=9,max_threads=1024,max_shared=49152};
    sm75:=DaDevice.{major=7,minor=5,max_threads=1024,max_shared=49152};
    p:=mm_plan(5760,2880,1,4,4,rtx4080);
    print("first expert projection, 1 token, 4 slots, 4 slices on compute capability 8.9: %, % warps, % instructions per accumulator\n",
        mm_text(p),p.part_items,p.instructions);
    p=mm_plan(2880,2880,32,4,1,rtx4080);
    print("second expert projection, 32 tokens per slot, 1 slice: %, % warps, % instructions per accumulator\n",
        mm_text(p),p.part_items,p.instructions);
    print("compute capability 7.5: %\n",mm_text(mm_plan(5760,2880,1,4,4,sm75)));
    print("2880 columns in 7 slices: %\n",mm_text(mm_plan(5760,2880,1,4,7,rtx4080)));
    print("5761 rows: %\n",mm_text(mm_plan(5761,2880,1,4,4,rtx4080)));
}
```

Both files pass a zero-survivor mutation run. Checks that need
bit-for-bit answers use integers: three experts at scales 1, 2 and 1/2, with
31, 32 and 33 tokens per slot and one or two slices, give exactly
16t and 64(64 + t) for every token t. A check that zeroes the low limbs shows
the bound fails without them.

## On the GPU

`psionic/tests/check_mxfp4_mma.sh` runs the checks in debug and release
builds, emits CUDA, compares the example transcript, and runs the products
on the device. Given the original gpt-oss-20b directory it also reads layer
0's two expert tensors and their biases.

```shell
COAX_CC=gcc psionic/tests/check_mxfp4_mma.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee
```

The harness uses four selected experts of 32 at full gpt-oss-20b shape,
random activations from a fixed seed, and both profiles' kernels compiled
into one module. On the RTX 4080 every checked output met the frozen bound,
no output was flagged, and chapter 66's rule held for every one-token output.
On the original layer 0 tensors the worst tensor-core error was 3.3 · 10^-7
of `sum |w x|` with 4 slices, against a bound of 1.03 · 10^-4; the f32
profile's worst was 3.0 · 10^-8.

The speed answer depends on how often each coefficient is reused. For one
token, both profiles read 133 MB of packed coefficients at about 680 GB/s
and take the same 0.195 ms; tensor cores do not help a product that is
limited by memory. With 32 tokens per expert, the tensor-core product of the
first projection takes 0.29 ms against 1.79 ms for chapter 67's batched f32
kernel, about six times faster. The
[verification record](../../docs/verification/psionic-ch069-mxfp4-mma-2026-11-01.md)
has every shape, slice count and time.

## What is not built yet

- **The model.** No decode or prefill step uses MXFP4-BF16-MMA-v1. Expert
  routes and top tokens on chapter 66's fixture prompts, and the retained
  original vectors of chapter 64, are not yet compared under this profile.
- **Selected-expert time in a full step.** The times above are the product
  kernels alone; the record does not yet show a whole decode or prefill
  step before and after.
- **Shared tiles.** Each warp reads its coefficients from device memory.
  Sharing decoded tiles across warps needs tensor cores inside group kernels,
  which `coax cuda` refuses (E0700, reason 34; see chapter 68).
- **Other devices.** One RTX 4080 under NixOS.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 5.
- [Making Psionic serving faster](../docs/serving/performance.md):
  matrix-hardware products from original MXFP4 (P2).
