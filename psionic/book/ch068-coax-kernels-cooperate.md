# 68. Coax kernels cooperate

> **Status: partly implemented.** Three cooperative group kernels, their CPU
> oracles, the refusals, and Coax capability admission run here, on the CPU
> and on an RTX 4080. Shuffle instructions, FP16 matrix primitives, tensor
> cores inside group kernels and any full-model use are not built yet; the
> last section lists them.

The kernels of [chapter 37](ch037-a-coax-compute-subset.md) work alone. Each
work item, or each 32-lane warp, reads its inputs from device memory and
writes outputs that no other item touches. Faster products need threads that
share work: one thread loads a value into fast on-chip memory and many
threads read it, or threads pass partial sums to each other instead of
writing them out and reading them back. This chapter writes three kernels
that cooperate, checks them against the CPU, shows the kernels the compiler
refuses, and moves the decision "may this kernel run on this device" into
Coax.

## Group kernels

`coax cuda` already has the forms this needs (see the
[compiler README](../../coax/README.md)). A kernel that contains loops
`for thread: 0..<T`, with T a multiple of 32 up to 1024, is a group kernel:
the device runs one block of T threads for each work item. Each thread loop
is a phase. Thread t runs the loop body with `thread = t`, and a barrier
(`__syncthreads()` in CUDA) fences each phase on both sides, so every write
of one phase is visible to every thread in the next. A fixed array declared
at the top of the kernel and indexed in a thread loop lives in threadgroup
memory (CUDA `__shared__`), unless every index there is the thread's own
`thread*k + c`, in which case each thread keeps its slice in registers.

On the CPU a thread loop is an ordinary loop, so the same source compiled to
C runs every thread of a phase one after another. That is the oracle: as long
as no thread reads what another thread writes in the same phase, running the
threads in sequence gives the same result as running them at once. The
compiler checks that rule, as the section on refusals shows.

## A tile reduction, by hand

The first kernel adds 256 values with 128 threads. In phase 0, thread t adds
values t and t+128. Then seven phases halve the number of threads that add:
with `half` = 64, 32, 16, 8, 4, 2 and 1, thread t < half adds the element
`half` places on. The total ends in element 0.

The order of these additions is part of the kernel's meaning, and it changes
the answer. Take 2^24 = 16,777,216 followed by 255 ones. Near 2^24, f32 values
are 2 apart, so 2^24 + 1 lies exactly between two of them and rounds to the
even one, 2^24. A plain loop that adds the values in order loses every one of
the ones and returns 16,777,216. The tree does better. Follow thread 0:

| Phase | half | Thread 0 adds | Thread 0 holds | Other adding threads hold |
| --- | --- | --- | --- | --- |
| 0 | — | value 128, a 1 | 2^24 + 1, rounded to 16,777,216 | 1 + 1 = 2 |
| 1 | 64 | 2 | 16,777,218 | 4 |
| 2 | 32 | 4 | 16,777,222 | 8 |
| 3 | 16 | 8 | 16,777,230 | 16 |
| 4 | 8 | 16 | 16,777,246 | 32 |
| 5 | 4 | 32 | 16,777,278 | 64 |
| 6 | 2 | 64 | 16,777,342 | 128 |
| 7 | 1 | 128 | 16,777,470 | — |

Every value after phase 0 is even and below 2^25, so it is exact. The tree
loses only the one that met 2^24 in phase 0: it returns 16,777,470 where the
exact sum is 16,777,471. The first check in `cooperate.coax` asserts all
three numbers.

<!-- source: psionic/lib/device/cooperate.coax tile -->
```jai
DC_THREADS :: 128;
DC_TILE :: 256;
// The threads that add in a phase: the first half.
dc_live :: (thread:int,half:int)->bool {return thread<half;}
// Group `item` adds x[item*256 ..< item*256+256] (zero past count) into
// sums[item]. Phase 0: thread t adds the values t and t+128 into a[t]. Then
// seven phases with half = 64, 32, ..., 1: every thread reads its own
// element, and threads t < half add the element half places on. Each phase
// reads one array and writes the other, so no phase writes an element
// another thread reads in that phase.
dc_tile_sum :: (item:int,count:int,x:[]f32,sums:[]f32) #export_c {
    a:[128]f32;b:[128]f32;
    for thread:0..<DC_THREADS {
        at:=item*DC_TILE+thread;lo:f32=0;hi:f32=0;
        if at<count {lo=x[at];}
        if at+DC_THREADS<count {hi=x[at+DC_THREADS];}
        a[thread]=lo+hi;
    }
    for step:0..<7 {
        half:=1<<(6-step);
        if step%2==0 {
            for thread:0..<DC_THREADS {s:=a[thread];if dc_live(thread,half) {b[thread]=s+a[thread+half];}}
        } else {
            for thread:0..<DC_THREADS {s:=b[thread];if dc_live(thread,half) {a[thread]=s+b[thread+half];}}
        }
    }
    // Step 6 is even, so the total is b[0].
    for thread:0..<DC_THREADS {s:=b[thread];if thread==0 {sums[item]=s;}}
}
```

Two arrays take turns. An even step reads `a` and writes `b`; an odd step
reads `b` and writes `a`. Within one phase, a thread writes only its own
element, and no thread reads the array being written, so threads never race.
Every thread reads its own element before it tests whether it adds. That read
is not needed for the result, but it makes a group of the wrong size fail on
the CPU instead of passing silently, and it keeps every line of the kernel
visible to the mutation check.

## Every thread reaches every barrier

A barrier is only safe if every thread of the group reaches it. If one thread
returns early, the others wait for it at the next `__syncthreads()` and the
result is undefined. Coax rules this out by construction:

- `thread` exists only inside thread loops. Code between them is the group's
  shared control flow, which runs identically in every thread, so a branch
  there is taken by all threads or by none.
- A thread loop may not leave early with `break`, `continue` or `return`.
- A thread loop may not assign a variable that the shared code sees, so the
  shared code cannot branch on a value that differs between threads.
- A thread loop that writes a threadgroup array may touch only the elements
  its own thread owns.

Here is a kernel that breaks the second rule:

```shell
coax/bin/coax cuda psionic/fixtures/cooperate-refused/early-return.coax
```

```text
psionic/fixtures/cooperate-refused/early-return.coax:4:41: error[E0700]: break, continue or return directly in a thread loop is outside the CUDA compute subset
      for thread:0..<128 {if x[thread]<0 {return;} a[thread]=x[thread];}
                                          ^
  fix: coax cuda accepts bool, f32, f64 and sized integers, fixed arrays of at most 4096 scalars, views of numbers, if/else, for loops, break/continue and acyclic calls; every #export_c procedure is a kernel with an int work item first and no result
1 error
```

The exit status is 1. The other recorded refusals in
`psionic/fixtures/cooperate-refused/` are a phase that reads the element its
neighbour writes (`race.coax`):

```text
psionic/fixtures/cooperate-refused/race.coax:6:46: error[E0700]: a thread loop that writes a threadgroup array and touches elements another thread owns is outside the CUDA compute subset
      for thread:0..<128 {a[thread]=a[thread]+a[thread+1];}
                                               ^
  fix: coax cuda accepts bool, f32, f64 and sized integers, fixed arrays of at most 4096 scalars, views of numbers, if/else, for loops, break/continue and acyclic calls; every #export_c procedure is a kernel with an int work item first and no result
1 error
```

a thread loop that adds into the group's one total (`shared-value.coax`,
"a thread loop assigning a value the group shares"), and a group of 100
threads (`size.coax`, not a multiple of 32). `check_cooperate.sh` runs all
four and compares each diagnostic and exit status with the recorded one.

## Lane exchange

The second kernel lets each of 32 threads end with the total of all 32
values, without a single thread doing all the work. In the step with mask
`m`, thread t adds the value of thread `t ^ m`, the thread whose index
differs from t in one bit. With masks 16, 8, 4, 2 and 1, each value reaches
every thread exactly once. This is the butterfly pattern that GPU shuffle
instructions are built for.

Start with thread t holding t. After mask 16, thread t < 16 holds
t + (t + 16) = 2t + 16, and thread t + 16 holds the same sum. After all five
masks every thread holds 0 + 1 + ... + 31 = 496. With 2^24 in thread 0 and
ones elsewhere, threads 0 and 16 both compute 2^24 + 1 first and lose the
one; then they add 2, 4, 8 and 16 and end at 16,777,246. Because f32 addition
is commutative, a thread and its partner always compute the same sum, so all
32 threads end with the same bits. The checks assert this for every lane.

<!-- source: psionic/lib/device/cooperate.coax butterfly -->
```jai
DC_WARP :: 32;
// Group `item` of 32 threads: thread t starts with x[item*32+t] (zero past
// count). In the step with mask m it adds the value of thread t^m. After
// masks 16, 8, 4, 2 and 1 every thread holds the same total, which it writes
// to out[item*32+t] (inside count).
dc_butterfly :: (item:int,count:int,x:[]f32,out:[]f32) #export_c {
    a:[32]f32;b:[32]f32;
    for thread:0..<DC_WARP {
        at:=item*DC_WARP+thread;v:f32=0;
        if at<count {v=x[at];}
        a[thread]=v;
    }
    for step:0..<5 {
        mask:=1<<(4-step);
        if step%2==0 {
            for thread:0..<DC_WARP {b[thread]=a[thread]+a[thread^mask];}
        } else {
            for thread:0..<DC_WARP {a[thread]=b[thread]+b[thread^mask];}
        }
    }
    for thread:0..<DC_WARP {
        at:=item*DC_WARP+thread;
        if at<count {out[at]=b[thread];}
    }
}
```

On the device the exchange goes through threadgroup memory and a barrier per
step. A shuffle instruction (`__shfl_xor_sync`) would exchange registers
directly, without the barrier. The subset has no shuffle form yet.

## A shared tile for a packed BF16 product

The third kernel multiplies a matrix of BF16 weights, two to a `u32` word, by
an f32 vector. Each thread computes one row. Without cooperation, all 128
threads of a group would read the same vector from device memory. Here the
group loads 256 vector values into a shared tile, two per thread, and then
every thread reads the tile.

<!-- source: psionic/lib/device/cooperate.coax rows -->
```jai
// Groups for a launch, and tiles of a row: n items in groups of per.
dc_groups :: (n:int,per:int)->int {return (n+per-1)/per;}
// Two BF16 values in one u32: column 2p in the low half, 2p+1 in the high.
dc_bf16 :: (word:u32,high:bool)->f32 {
    if high {return f32_from_bits(word&0xffff0000);}
    return f32_from_bits(word<<16);
}
// Group `item` computes out[r] = sum over c of w[r][c] * x[c] for the rows
// r = item*128 + t, one per thread. w is rows by cols BF16 (cols even),
// packed two to a u32, cols/2 words per row. The vector is read in tiles of
// 256: every thread loads two of its values into the shared tile, then
// every thread reads the whole tile. Each thread adds its row's products in
// f32 in column order.
dc_bf16_rows :: (item:int,rows:int,cols:int,w:[]u32,x:[]f32,out:[]f32) #export_c {
    tile:[256]f32;acc:[128]f32;
    words:=cols/2;
    for s:0..<dc_groups(cols,DC_TILE) {
        c0:=s*DC_TILE;
        for thread:0..<DC_THREADS {
            for i:0..<2 {
                c:=c0+thread+DC_THREADS*i;v:f32=0;
                if c<cols {v=x[c];}
                tile[thread+DC_THREADS*i]=v;
            }
        }
        for thread:0..<DC_THREADS {
            row:=item*DC_THREADS+thread;
            if row<rows {
                sum:=acc[thread];
                for p:0..<DC_TILE/2 {
                    c:=c0+2*p;
                    if c<cols {
                        word:=w[row*words+c/2];
                        sum+=dc_bf16(word,false)*tile[2*p];
                        sum+=dc_bf16(word,true)*tile[2*p+1];
                    }
                }
                acc[thread]=sum;
            }
        }
    }
    for thread:0..<DC_THREADS {
        row:=item*DC_THREADS+thread;
        if row<rows {out[row]=acc[thread];}
    }
}
```

Each thread adds its row's products in f32 in column order, so the device
must match the C build bit for bit. Before running it, we also froze an
accuracy bound against an f64 sum: at most `cols * 2^-24` of `sum |w x|`.

## Capability admission

A kernel can be correct and still fail on a device that lacks what it needs.
Chapter 37's tensor-core primitive `cu_mma_bf16_16x8x16` lowers to an
`mma.sync` instruction that needs compute capability 8.0 or later. A group of
1,024 threads needs a device that allows 1,024 threads per block. So each
kernel states its needs, the host reports what the device has, and Coax
decides:

<!-- source: psionic/lib/device/admit.coax admit -->
```jai
DaStatus :: enum u8 {ok;report;kernel;threads;shared;mma;}
// What a kernel needs. threads: threads per group (a warp kernel's group is
// one warp, 32). shared: bytes of threadgroup arrays. mma: it calls
// cu_mma_bf16_16x8x16, which lowers to mma.sync.m16n8k16 with BF16 inputs,
// an instruction of compute capability 8.0 and later.
DaNeeds :: struct {threads:int;shared:int;mma:bool;}
// What the device reports (CUDA device attributes 75, 76, 1 and 8).
DaDevice :: struct {major:int;minor:int;max_threads:int;max_shared:int;}

da_admit :: (need:DaNeeds,dev:DaDevice)->DaStatus {
    if dev.major<1 || dev.minor<0 || dev.minor>9 || dev.max_threads<32 || dev.max_shared<0 {return DaStatus.report;}
    if need.threads<32 || need.threads/32>32 || need.threads%32!=0 || need.shared<0 {return DaStatus.kernel;}
    if need.threads>dev.max_threads {return DaStatus.threads;}
    if need.shared>dev.max_shared {return DaStatus.shared;}
    if need.mma && dev.major<8 {return DaStatus.mma;}
    return DaStatus.ok;
}
da_text :: (s:DaStatus)->string {
    if s==DaStatus.ok {return "admitted";}
    if s==DaStatus.report {return "refused: the device report is malformed";}
    if s==DaStatus.kernel {return "refused: the kernel's needs are malformed";}
    if s==DaStatus.threads {return "refused: the group has more threads than the device allows in one block";}
    if s==DaStatus.shared {return "refused: the kernel needs more shared memory than one block may use";}
    return "refused: cu_mma_bf16_16x8x16 needs compute capability 8.0 or later";
}
```

<!-- source: psionic/lib/device/admit.coax needs -->
```jai
// The needs of chapter 68's group kernels and chapter 37's tensor-core
// conformance kernel, stated next to their sources: 0 dc_tile_sum (two
// [128]f32 arrays), 1 dc_butterfly (two [32]f32), 2 dc_bf16_rows (one
// [256]f32 tile; its [128]f32 accumulator is per-thread registers),
// 3 dw_mma_tile (one warp, registers only).
DA_KERNELS :: 4;
da_needs :: (kernel:int)->DaNeeds {
    if kernel==0 {return DaNeeds.{threads=128,shared=1024,mma=false};}
    if kernel==1 {return DaNeeds.{threads=32,shared=256,mma=false};}
    if kernel==2 {return DaNeeds.{threads=128,shared=1024,mma=false};}
    if kernel==3 {return DaNeeds.{threads=32,shared=0,mma=true};}
    return DaNeeds.{threads=0,shared=0,mma=false};
}
da_name :: (kernel:int)->string {
    if kernel==0 {return "dc_tile_sum";}
    if kernel==1 {return "dc_butterfly";}
    if kernel==2 {return "dc_bf16_rows";}
    if kernel==3 {return "dw_mma_tile";}
    return "unknown";
}
```

The host, `psionic/platform/cuda-host.c`, now reports two more device
attributes, the most threads (attribute 1) and shared bytes (attribute 8)
per block, next to the compute capability it already read. It makes no
admission decision. `admit.coax` is an `#authority` file: its checks cover
every boundary, and its mutation run has no survivors. The needs are written
by hand next to the kernels, and a check pins each one; the compiler does not
derive them yet.

## Run it

The [example](../examples/cooperate.coax) runs every kernel on the CPU and
asks admission about two devices: the RTX 4080 on this machine, and a device
of compute capability 7.5 described by its numbers.

```shell
coax/bin/coax run psionic/examples/cooperate.coax
```

```text
2^24 and 255 ones: tree 16777470.0, plain loop 16777216.0, exact 16777471
butterfly of 2^24 and 31 ones: lane 0 16777246.0, lane 31 16777246.0, all lanes equal: true
packed BF16 rows: 10.0 5.5
compute capability 8.9:
  dc_tile_sum: admitted
  dc_butterfly: admitted
  dc_bf16_rows: admitted
  dw_mma_tile: admitted
compute capability 7.5:
  dc_tile_sum: admitted
  dc_butterfly: admitted
  dc_bf16_rows: admitted
  dw_mma_tile: refused: cu_mma_bf16_16x8x16 needs compute capability 8.0 or later
a 100-thread group: refused: the kernel's needs are malformed
```

<!-- source: psionic/examples/cooperate.coax example -->
```jai
// Chapter 68 on the CPU: each group kernel runs as ordinary loops, one work
// item after another, then admission decides where each kernel may run.
main :: () {
    x:[256]f32;sums:[1]f32;
    x[0]=16777216;for i:1..<256 {x[i]=1;}
    dc_tile_sum(0,256,x[..],sums[..]);
    plain:f32=0;for i:0..<256 {plain+=x[i];}
    print("2^24 and 255 ones: tree %, plain loop %, exact 16777471\n",sums[0],plain);

    lanes:[32]f32;out:[32]f32;same:=true;
    lanes[0]=16777216;for i:1..<32 {lanes[i]=1;}
    dc_butterfly(0,32,lanes[..],out[..]);
    for i:1..<32 {if out[i]!=out[0] {same=false;}}
    print("butterfly of 2^24 and 31 ones: lane 0 %, lane 31 %, all lanes equal: %\n",out[0],out[31],same);

    // Two rows of four BF16 weights against x = 1, 2, 3, 4.
    w:[4]u32;v:[4]f32;rows:[2]f32;
    w[0]=dc_pack(1,1);w[1]=dc_pack(1,1);w[2]=dc_pack(0.5,-1);w[3]=dc_pack(2,0.25);
    for c:0..<4 {v[c]=f32(c+1);}
    dc_bf16_rows(0,2,4,w[..],v[..],rows[..]);
    print("packed BF16 rows: % %\n",rows[0],rows[1]);

    devices:=[2] DaDevice.[DaDevice.{major=8,minor=9,max_threads=1024,max_shared=49152},
        DaDevice.{major=7,minor=5,max_threads=1024,max_shared=49152}];
    for d:0..<2 {
        print("compute capability %.%:\n",devices[d].major,devices[d].minor);
        for k:0..<DA_KERNELS {print("  %: %\n",da_name(k),da_text(da_admit(da_needs(k),devices[d])));}
    }
    print("a 100-thread group: %\n",da_text(da_admit(DaNeeds.{threads=100,shared=0,mma=false},devices[0])));
}
```

The second row of the product is 0.5·1 − 1·2 + 2·3 + 0.25·4 = 5.5, and the
compute capability 7.5 device refuses only the tensor-core kernel.

The checks in `cooperate.coax` compare each kernel with an oracle written
separately: the tree and butterfly orders as plain loops over a copy of the
values, the BF16 product as a column-order f32 loop and an f64 sum. Groups
run in reverse order to show that order does not matter. Both files pass a
zero-survivor mutation run.

## On the GPU

`psionic/tests/check_cooperate.sh` runs the checks in debug and release
builds, emits the CUDA source, checks the four refusals, compares the
example transcript, and then runs the kernels on the device. It needs an
NVIDIA driver and NVRTC; without them the device step prints `SKIP`.

```shell
psionic/tests/check_cooperate.sh
```

Before running on the device we fixed the conditions: every output must
equal the C build bit for bit, every butterfly lane must equal lane 0 of its
group, and every BF16 row must be within `cols * 2^-24` of `sum |w x|` of an
f64 sum. On the RTX 4080 (compute capability 8.9), admission
admitted all four kernels, and every check held:

```text
device: NVIDIA GeForce RTX 4080 (sm_89, 1024 threads and 49152 shared bytes per block)
dc_tile_sum   4194304 values  16384 groups: 0 differ from the CPU (0.034 ms)
dc_butterfly  1048576 values  32768 groups: 0 differ from the CPU, 0 lanes differ from lane 0 (0.024 ms)
dc_bf16_rows  4096x4096    32 groups: 0 differ from the CPU, worst error 1.45e-07 of sum |w x| (bound 2.44e-04) (0.243 ms)
dc_bf16_rows  2880x2880    23 groups: 0 differ from the CPU, worst error 2.13e-07 of sum |w x| (bound 1.72e-04) (0.189 ms)
chapter 68 group kernels on CUDA: admitted, and bit-identical to the CPU
```

The harness also runs ragged sizes (1, 255, 256 and 257 values; 1, 31, 32
and 33 lanes; 1 × 2, 130 × 262 and 1000 × 1002 matrices), all with zero
differences. The times are single launches and are not a performance claim.
These kernels show that cooperation is correct; they are not tuned.

## What is not built yet

- **Shuffles.** Lane exchange goes through threadgroup memory and a barrier.
  A shuffle form that lowers to `__shfl_xor_sync` (and `simd_shuffle` on
  Metal) would need a compiler change.
- **FP16 matrix primitives.** The only tensor-core primitive is chapter 37's
  BF16 `cu_mma_bf16_16x8x16`. There is no FP16 primitive.
- **Tensor cores in group kernels.** A group kernel that calls the MMA
  primitive is refused (E0700, reason 34), so shared tiles and tensor cores
  cannot yet meet in one kernel. That combination is what fast expert
  products for [chapter 66](ch066-a-complete-cuda-decoder.md) and batched
  prefill in [chapter 67](ch067-prefill-together.md) would need.
- **Derived needs.** The compiler does not yet compute a kernel's threads and
  shared bytes for admission; they are stated by hand and pinned by checks.
- **Other devices.** These kernels ran on one RTX 4080 under NixOS. The same
  source should also compile with `coax metal-compute`, but it has not been
  run on an Apple GPU for this chapter.
- **Models.** No full-model measurement uses these kernels.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 4.
- [Making Psionic serving faster](../docs/serving/performance.md): cooperative
  BF16/FP16 matrix operations (P2).
- [gpt-oss device record](../../docs/verification/psionic-gptoss-device-2026-10-29.md).
