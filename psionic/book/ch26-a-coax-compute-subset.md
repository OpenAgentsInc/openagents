# 26. A Coax compute subset

Chapter 10's decision takes about 35 minutes on one CPU core. Almost all of
that time goes into matrix products, which a GPU does well. But Psionic's rule
is that Coax owns the arithmetic: the order of every sum, the rounding of
every product and the shape of every loop. A hand-written CUDA kernel would
move that arithmetic into another language, and a BLAS library would hide it
completely. So before any model runs on a GPU, the compiler needs a small
language for device code.

This chapter builds that language for NVIDIA GPUs and proves it on one kernel,
a BF16 matrix times f32 vectors. The same source runs on the CPU, where
`coax test` checks it, and on an RTX 4080, where we compare the two bit for
bit.

## Kernels are procedures over work items

A kernel is an ordinary `#export_c` procedure whose first parameter is its
`int` work item and which returns nothing. The device runs it once for each
item, in no particular order, so each output must belong to exactly one item.
Here is the one-token product that chapter 30 uses for the vocabulary readout:

<!-- source: psionic/lib/device/project.coax matvec -->
```jai
// One token: item row*16+lane sums the groups lane, lane+16, ... of its row,
// so the 16 lanes of a row read 128 contiguous bytes at a time. It writes
// partial[lane*rows+row] for dv_reduce with 16 slices.
dv_matvec :: (item:int,rows:int,cols:int,w:[]u64,x:[]f32,partial:[]f32) #export_c {
    groups:=cols/4;row:=item/DV_LANES;lane:=item%DV_LANES;per:=groups/DV_LANES;
    sum:f32=0;total:f32=0;
    for k:0..<per {
        g:=k*DV_LANES+lane;bits:=w[row*groups+g];
        for j:0..<4 {sum+=dv_bf16(bits,j)*x[4*g+j];}
        if (k+1)%DV_GROUPS_PER_CHUNK==0 || k+1==per {total+=sum;sum=0;}
    }
    partial[lane*rows+row]=total;
}
```

The weights stay in their original BF16 bytes. Each `u64` holds four of them,
and `dv_bf16` turns one into an f32 by shifting its bits. Sixteen work items
share a row: neighbouring items read neighbouring `u64`s, which is how a GPU
wants memory to be read. Each one adds f32 products in chunks of 256 columns.
A second kernel adds the sixteen partial sums of each row in f64, in order:

<!-- source: psionic/lib/device/project.coax reduce -->
```jai
// out[i] for i = t*rows+r: the slice partials added in f64, in slice order.
dv_reduce :: (item:int,count:int,slices:int,partial:[]f32,out:[]f32) #export_c {
    sum:f64=0;
    for s:0..<slices {sum+=f64(partial[s*count+item]);}
    out[item]=f32(sum);
}
```

This is a profile of its own, DEVICE-F32-CHUNKED-v1, not chapter 3's wide CPU
profile. Its worst-case error is `(258 + cols/256) * 2^-24` of `sum |w x|`,
which is below `2^-15` for every Elephant shape. A result never depends on the
order in which the device runs work items, because no two items write the same
output.

## The same source on the CPU

Nothing in a kernel is GPU-specific, so the CPU can run it too: call it once
for every work item. The [example](../examples/device-matvec.coax) does
exactly that for four rows. It needs no GPU:

```shell
coax/bin/coax run psionic/examples/device-matvec.coax
```

```jai
rows: 896.0 448.0 896.0 448.0
lane 0 of row 0 added groups 0, 16, 32 and 48: 24.0
```

<!-- source: psionic/examples/device-matvec.coax example -->
```jai
// The chapter 26 kernels on the CPU: every work item of dv_matvec, then of
// dv_reduce, in any order. The GPU runs the same source.
main :: () {
    rows:=4;cols:=256;groups:=cols/4;
    w:[256]u64;x:[256]f32;partial:[64]f32;out:[4]f32;
    one:u64=0x3f80;half:u64=0x3f00;
    for r:0..<rows {
        value:=one;if r%2==1 {value=half;}
        for g:0..<groups {w[r*groups+g]=value|(value<<16)|(value<<32)|(value<<48);}
    }
    for c:0..<cols {x[c]=f32(c%8);}
    for i:0..<rows*16 {dv_matvec(rows*16-1-i,rows,cols,w[..],x[..],partial[..]);}
    for i:0..<rows {dv_reduce(i,rows,16,partial[..],out[..]);}
    print("rows: % % % %\n",out[0],out[1],out[2],out[3]);
    print("lane 0 of row 0 added groups 0, 16, 32 and 48: %\n",partial[0]);
}
```

The loop runs the items backwards on purpose: order must not matter. The
library's checks in `project.coax` go further. They pin each kernel's work-item
placement, show where the f32 chunks round (with a 2^24 value that swallows
the ones after it), and pass a zero-survivor mutation run.

## What `coax cuda` accepts

`coax cuda FILE` compiles a file's kernels, and every procedure they call, to
CUDA C. A kernel and everything it reaches may use only:

- `bool`, `f32`, `f64` and the sized integers;
- fixed arrays of at most 1,024 scalars, which become registers;
- views of numbers (`[]f32`, `[]u64` and so on), which become device pointers
  with a count;
- `if`, `else`, `for` loops over a range or a view, `break`, `continue`, and
  calls that do not recurse.

Everything else is refused with E0700 and the construct it found. Strings,
structs, enums, `while` loops, `defer`, `#assert`, `print` and `#foreign` stay
in CPU code. A kernel's parameters after the work item must be scalars or
views of numbers. Checks and the procedures that only checks call are not
part of the device module, so a kernel file can still test itself with
strings and helpers. The admission decisions are themselves Coax, in
[`compute.coax`](../../coax/lib/compiler/compute.coax), with its own
zero-survivor mutation gate.

A `while` loop is refused because it has no visible bound; a `for` loop is
bounded by its range or view. Integer overflow, bounds and cast checks keep
their CPU meaning. On the device a failed check executes a trap instruction,
which stops the whole launch:

```shell
$ coax/bin/coax cuda bad.coax
bad.coax:1:38: error[E0700]: a while loop is outside the CUDA compute subset
```

For each kernel the compiler emits a `__global__` entry that takes the number
of work items first and calls the procedure once per item:

```jai
extern "C" __global__ void dv_project(int64_t coax_items, int64_t l_rows, int64_t l_cols, int64_t l_tokens, int64_t l_slices, coax_v_u64 l_w, coax_v_f32 l_x, coax_v_f32 l_out) {
  int64_t coax_item = (int64_t)blockIdx.x * (int64_t)blockDim.x + (int64_t)threadIdx.x;
  if (coax_item >= coax_items) return;
  coax_k_dv_project(coax_item, l_rows, l_cols, l_tokens, l_slices, l_w, l_x, l_out);
}
```

The emitted file needs no headers. Loops over constant bounds of at most 128
steps are unrolled, so a fixed array indexed by the loop counter stays in
registers.

## The host does effects

[`cuda-host.c`](../platform/cuda-host.c) is the only C in this chapter. It
opens the driver library and NVRTC with `dlopen`, so a machine without a GPU
still builds and reports that it has none. It compiles the generated source
at launch, the way the iOS app compiles Coax-generated Metal, with
`--fmad=false`: every multiply and add rounds once, as in the C build, so +,
-, *, / and square root agree with the CPU bit for bit. It allocates, copies,
launches and waits, and it reports a failed launch at the next completion
point. It makes no arithmetic or scheduling decision.

## Run it

The harness qualifies the kernels on random BF16 weights at Elephant's shapes.
It needs an NVIDIA driver and NVRTC (CUDA 13.2 here); without them it prints
`SKIP` and exits.

```shell
psionic/tests/run_device_project.sh
```

Before we measured anything, we fixed two conditions. The device must match
the same kernels compiled to C exactly. And each output must be within
`2^-15 * sum |w x|` of the CPU reference profile from chapter 3. Every shape
met both: no output differed from the C build, and the largest reference error
was 1.2e-7 of `sum |w x|`. The 248,320-row vocabulary readout reads its
2.03 GB of weights in 3.49 ms, 583 GB/s of the card's 717 GB/s. The 86-token
products reach 5.4 to 6.4 TFLOP/s. The [verification
record](../../docs/verification/psionic-ch26-2026-10-29.md) has every shape,
the host and the tests that refuse each construct outside the subset.

## Limits

The subset has no shared memory, barriers, atomics, warp exchange or tensor
cores, so a work item cannot cooperate with its neighbours. That is why the
batched product runs at about a quarter of the card's f32 rate. The tested host
is one NixOS machine with one driver; that is evidence for this machine, not a
qualified Linux target. [Chapter 30](ch30-elephant-on-cuda.md) uses these
kernels, and more like them, for the whole Elephant decision.
