# 30. Elephant on CUDA

[Chapter 10](ch010-execute-elephant.md) answered one typed question with the
original Elephant weights in about 35 minutes of CPU time. This chapter
answers the same question on an RTX 4080 in about a quarter of a second, with
the same probabilities to six decimal places. Every kernel is Coax from
[chapter 26](ch026-a-coax-compute-subset.md), and so is every decision about
what runs where: which weights stay on the device, which stream from host
memory, and the order of every copy and launch.

The plan's title promises Mammoth too. Mammoth's 51 GB of text weights need
the streaming of this chapter at a much larger scale and their own
qualification; [chapter 30b](ch030b-mammoth-on-cuda.md) does both.

## The decision as one pass

Chapter 10 runs the shared prefix and the question as two blocks, each through
all 32 layers. On a GPU we would rather read each weight once, so we run all
86 tokens through one layer before the next (layer-major). That is the same
calculation, because a prefix token never depends on a later token: the
DeltaNet layers are causal, and full attention follows chapter 8's block mask.
A token in the prefix sees the prefix; a later token sees everything:

<!-- source: psionic/lib/device/layer.coax mask -->
```jai
// Decision blocks: a token before `prefix` sees the prefix; a later token
// sees every token. Within a block attention is bidirectional.
dl_visible :: (t:int,s:int,prefix:int)->bool {return t>=prefix || s<prefix;}
```

Only the last token's readout matters, so the vocabulary product runs once,
for that token, and the CPU turns its 248,320 logits into option probabilities
with chapter 9's `qr_read`.

## Kernels for every step

`psionic/lib/device/layer.coax` holds the other sixteen steps: the offset RMS
norm (partial sums of squares, then a scaled row), residual adds, SwiGLU, the
causal convolution, DeltaNet's gates and normalized queries and keys, the
recurrence itself, the gated head norm, attention preparation with rotary
positions, scores, softmax and the attention sum. Each is a kernel over work
items with its own checks. Reductions over a row add in f64; values are
stored as f32, as on the CPU.

The DeltaNet recurrence is sequential in time but independent across value
heads and columns. Each work item owns one column of one head's 128 x 128
state, keeps it in registers and walks all 86 tokens:

<!-- source: psionic/lib/device/layer.coax scan -->
```jai
// Value head h, column c (item h*128+c) of the gated delta rule over every
// token, from a zero state. The state column lives in registers.
dl_scan :: (item:int,tokens:int,heads:int,key_heads:int,channels:int,conv:[]f32,q:[]f32,k:[]f32,beta:[]f32,decay:[]f32,out:[]f32) #export_c {
    h:=item/128;c:=item%128;kh:=h/(heads/key_heads);vbase:=key_heads*256+h*128+c;
    state:[128]f32;
    for t:0..<tokens {
        kb:=(t*key_heads+kh)*128;kr:=k[kb..<kb+128];qr:=q[kb..<kb+128];
        g:=decay[t*heads+h];memory:f32=0;
        for r:0..<128 {state[r]=state[r]*g;memory+=state[r]*kr[r];}
        delta:=(conv[t*channels+vbase]-memory)*beta[t*heads+h];
        result:f32=0;
        for r:0..<128 {state[r]+=kr[r]*delta;result+=qr[r]*state[r];}
        out[(t*heads+h)*128+c]=result;
    }
}
```

## The plan is data

`psionic/lib/device/plan.coax` turns the decision into a list of operations:
kernel launches with their arguments, copies into streaming slots, and the
waits and releases that order them. The host C executes the list as written.
The first decision is what fits. With the 4080's other users, about 14.2 GB
is free for us; Elephant's text weights are 15.9 GB. So some must stream:

<!-- source: psionic/lib/device/plan.coax residency -->
```jai
// Which weights stream from pinned host memory, through two slots that hold
// them in turn, each the size of one feed-forward matrix (96 MiB for
// Elephant, 170 MiB for Mammoth). The feed-forward matrices go first: all
// down projections, then up, then gate, in bit-reversed layer order (0, 16,
// 8, 24, ... for 32 layers) so any number of them is spread across the
// model. If that is not enough the head streams, in chunks of up to one
// feed-forward width of rows, and then the mixer matrices (the projections
// before each layer's attention or scan), a whole layer at a time in the
// same spread order. Only the small vectors and the workspace must fit:
// everything else can stream.
//
// Streaming indices, for L layers: 3*layer+k for feed-forward matrix k
// (0 gate, 1 up, 2 down); 3L+5*layer+k for mixer matrix k (linear: qkv, z,
// a, b, out; full: q, k, v, out, and k = 4 is no matrix); 8L+c for head
// chunk c.
dp_mixer_base :: (m:DpModel)->int {return 3*m.layers;}
dp_head_base :: (m:DpModel)->int {return 8*m.layers;}
dp_head_chunks :: (m:DpModel)->int #export_c {return m.chunks;}
dp_streamable :: (m:DpModel)->int #export_c {return dp_head_base(m)+dp_head_chunks(m);}
dp_residency_of :: (m:DpModel,budget:int,workspace:int,stream:[]bool)->DpPlan {
    if stream.count!=dp_streamable(m) {return DpPlan.{status=DpStatus.capacity};}
    total:=0;
    for w:0..<dp_weights(m) {total+=dp_align(dp_bytes(m,w));}
    slot:=dp_align(2*m.width*m.hidden);
    for i:0..<stream.count {stream[i]=false;}
    if workspace+total<=budget {return DpPlan.{workspace=workspace,resident=total,kind=m.kind};}
    over:=workspace+total+2*slot-budget;saved:=0;n:=0;l:=m.layers;
    kinds:=[3]int.[2,1,0];
    for i:0..<3*l {if saved<over {stream[3*dp_spread_of(l,i%l)+kinds[i/l]]=true;saved+=slot;n+=1;}}
    if saved<over {for c:0..<dp_head_chunks(m) {stream[dp_head_base(m)+c]=true;}saved+=dp_bytes(m,dp_head(m));n+=dp_head_chunks(m);}
    for i:0..<l {
        layer:=dp_spread_of(l,i);
        for k:0..<dp_mixers(layer) {
            x:=dp_mixer_base(m)+5*layer+k;
            if saved<over {stream[x]=true;saved+=dp_bytes(m,dp_stream_weight_of(m,x));n+=1;}
        }
    }
    if saved<over {return DpPlan.{status=DpStatus.memory};}
    return DpPlan.{workspace=workspace,resident=total-saved,streamed=saved,slot=slot,streamed_tensors=n,kind=m.kind};
}
```

Streamed matrices are spread across the layers on purpose, in bit-reversed
layer order. The copy of each
one starts as soon as its slot is released, two matrices ahead, so it arrives
over PCIe while the layers before it compute.

The [example](../examples/device-plan.coax) prints the plan for a 14 GiB
budget without a GPU:

```shell
coax/bin/coax run psionic/examples/device-plan.coax
```

```jai
operations: 1128, launches: 1092, copies: 12
workspace: 91447808 bytes
resident: 14667538432 bytes, streamed: 1207959552 bytes in 12 feed-forward matrices
streamed down projections, by layer: 0 2 4 8 10 12 16 18 20 24 26 28
first operation: copy weight 13 into slot 0
first launch: dl_squares over 1376 work items
```

<!-- source: psionic/examples/device-plan.coax example -->
```jai
// Chapter 30's schedule for the 86-token sample with a 14 GiB device budget.
// Nothing runs: the plan is data that the host executes.
main :: () {
    ops:[1500]DpOp;stream:[277]bool;
    budget:=14*1073741824;
    plan:=dp_plan(86,49,budget,stream[..],ops[..]);
    #assert plan.status==DpStatus.ok;
    launches:=0;copies:=0;for i:0..<plan.ops {if ops[i].kind==DpOpKind.launch {launches+=1;} if ops[i].kind==DpOpKind.copy {copies+=1;}}
    print("operations: %, launches: %, copies: %\n",plan.ops,launches,copies);
    print("workspace: % bytes\n",plan.workspace);
    print("resident: % bytes, streamed: % bytes in % feed-forward matrices\n",plan.resident,plan.streamed,plan.streamed_tensors);
    print("streamed down projections, by layer:");for i:0..<32 {if stream[3*i+2] {print(" %",i);}}print("\n");
    at:=0;while ops[at].kind!=DpOpKind.launch {at+=1;}
    print("first operation: copy weight % into slot %\n",ops[0].tensor,ops[0].slot);
    print("first launch: % over % work items\n",dp_kernel_name(cast(int)ops[at].kernel),ops[at].items);
}
```

## Run it

The harness opens and authenticates the session exactly as chapter 10 does,
then uploads the resident weights, pins the streamed ones, and runs the plan.
It needs the original files, an NVIDIA driver and NVRTC; without the last two
it prints `SKIP`.

```shell
psionic/tests/run_elephant_device.sh /path/to/elephant-directory
```

Before the first device run we froze the acceptance test: the choice must be
`yes`, and each of the four decision statistics must be within 0.0005 of both
chapter 10's independent reference and its recorded CPU result. The device
returned `yes` with probability 0.757196636 against the CPU's 0.757196285, a
difference of 3.5e-7, and the other statistics agree as closely.

With `PSIONIC_DEVICE_QUALIFY=1` the harness also repeats every launch of the
first linear layer, the first attention layer and the readout on the CPU, from
the device's own inputs, and compares the whole workspace. 54 of the 55
launches were identical bit for bit. The other was the softmax, whose f64
`exp` comes from the CUDA math library rather than the C library.

With chapter 26's work-item projections, a warm decision took about 237 ms
from the host's request to the probabilities, against 2,092 s on one CPU core;
three quarters of it was the projections, at 6 to 7 TFLOP/s. With the
tensor-core projections of chapter 26's warp kernels it takes about 147 ms,
with p(yes) 0.757202773, and the projections take about 95 ms of it. The 0.9 GB of streamed weights per decision arrive while earlier
layers compute, so lowering the budget to 12 GiB (3.3 GB streamed) did not
change the time; at 10 GiB (5.4 GB streamed) the copies begin to show, at
about 255 ms.
With only 6.25 GiB, 8.8 GB stream per decision and the answer takes about
415 ms, the speed of the PCIe link.

An opt-in comparator, `psionic/tests/bench_cublas.sh`, runs the same shapes
through cuBLAS, which no Psionic build loads. The one-token readout reaches
90% of cuBLAS's BF16 rate; the 86-token products reach 34–41% of its f32
SGEMM and 10–16% of its BF16 tensor-core GEMM.

A request's questions share one prefix, so the engine runs chapter 8's fork
(chapter 31 lists `dp_plan_prefix` and `dp_plan_suffix`): the prefix pass
keeps each linear layer's convolution history and delta-rule state and each
full layer's key and value rows in an 82 MiB fork buffer, and every
question's suffix starts from them. The forked decision is bit-identical to
the whole one (none of 496,640 logits differ); the 37-token suffix takes
about 120 ms, and the Lab's five questions went from 1.84 to 1.17 s with the
same answers.

Opening the session took about 100 seconds, nearly all of it hashing the
18.8 GB checkpoint on the CPU; the upload of 13.9 GB of resident weights takes
2 seconds. The [verification record](../../docs/verification/psionic-ch30-2026-10-29.md)
has the measurements, the per-kernel profile and the headroom runs.

## Limits

This is one request, one machine and one arithmetic profile. The tensor-core
projections share no tiles between warps yet; chapters 27 to 29 and 32 are where
they come from. When the feed-forward matrices are not enough, the head and the
mixer matrices stream too: [chapter 30a](ch030a-share-the-card.md) runs the
decision in a few hundred MiB beside another model. The session is still the CPU's, so a cold start is
dominated by authentication unless a verified-file record (chapter 15)
names the checkpoint. Questions run one after another, not batched.
