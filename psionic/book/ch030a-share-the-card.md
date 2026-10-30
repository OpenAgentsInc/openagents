# 30a. Share the card

[Chapter 30](ch030-elephant-on-cuda.md) answers an Elephant decision on the
RTX 4080 in about 147 ms, with 14 GiB of the card to itself. On the machine
that serves us that is rarely true. [Chapter 48b](ch048b-serve-responses.md)'s
gpt-oss-20b server keeps all 13.8 GB of its original MXFP4 weights on the
same 16 GB card, and after the desktop's share about 1.4 GB is left. Chapter
30's plan streams only the feed-forward matrices, so it refused anything
below about 6.1 GiB. One model or the other could run, not both.

This chapter lets every Elephant matrix stream. The plan keeps only the
workspace, the small f32 vectors and two copy slots on the device: about
283 MiB for the 86-token sample. The rest comes from pinned host memory over
PCIe for every decision. The decision becomes a question of bandwidth rather
than memory: about 0.66 s at the link's speed, against 147 ms with the card to
itself, and both models stay ready.

## What streams, in what order

The plan still prefers to keep weights resident, and it streams only as much
as the budget requires. The order is the same idea as chapter 30's, extended:
first the feed-forward matrices, then the head, then the mixer matrices (the
projections in front of each layer's scan or attention), a whole layer at a
time and spread across the model:

<!-- source: psionic/lib/device/plan.coax residency -->
```jai
// Which weights stream from pinned host memory, through two slots of 96 MiB
// that hold them in turn. The feed-forward matrices go first: all down
// projections, then up, then gate, in bit-reversed layer order (0, 16, 8,
// 24, ...) so any number of them is spread across the model. If that is not
// enough the head streams, in 21 chunks of up to 12,288 rows, and then the
// mixer matrices (the projections before each layer's attention or scan),
// a whole layer at a time in the same spread order. Only the small vectors
// and the workspace must fit: everything else can stream.
//
// Streaming indices: 3*layer+k for feed-forward matrix k (0 gate, 1 up,
// 2 down); 96+5*layer+k for mixer matrix k (linear: qkv, z, a, b, out; full:
// q, k, v, out, and k = 4 is no matrix); 256+c for head chunk c.
DP_FFN_STREAMS :: 96;
DP_MIXER_BASE :: 96;
DP_HEAD_BASE :: 256;
DP_HEAD_ROWS :: 12288;
DP_HEAD_CHUNKS :: 21;
DP_STREAMABLE :: 277;
dp_residency :: (budget:int,workspace:int,stream:[]bool)->DpPlan #export_c {
    if stream.count!=DP_STREAMABLE {return DpPlan.{status=DpStatus.capacity};}
    total:=0;
    for w:0..<DP_WEIGHTS {n:=dp_weight_bytes(w);if n<0 {return DpPlan.{status=DpStatus.capacity};}total+=dp_align(n);}
    slot:=dp_align(2*DP_WIDTH*DP_HIDDEN);
    for i:0..<DP_STREAMABLE {stream[i]=false;}
    if workspace+total<=budget {return DpPlan.{workspace=workspace,resident=total};}
    over:=workspace+total+2*slot-budget;saved:=0;n:=0;
    kinds:=[3]int.[2,1,0];
    for i:0..<DP_FFN_STREAMS {if saved<over {stream[3*dp_spread(i%DP_LAYERS)+kinds[i/DP_LAYERS]]=true;saved+=slot;n+=1;}}
    if saved<over {for c:0..<DP_HEAD_CHUNKS {stream[DP_HEAD_BASE+c]=true;}saved+=dp_weight_bytes(DP_HEAD);n+=DP_HEAD_CHUNKS;}
    for i:0..<DP_LAYERS {
        layer:=dp_spread(i);
        for k:0..<dp_mixers(layer) {
            x:=DP_MIXER_BASE+5*layer+k;
            if saved<over {stream[x]=true;saved+=dp_weight_bytes(dp_stream_weight(x));n+=1;}
        }
    }
    if saved<over {return DpPlan.{status=DpStatus.memory};}
    return DpPlan.{workspace=workspace,resident=total-saved,streamed=saved,slot=slot,streamed_tensors=n};
}
```

Every streamed matrix fits one 96 MiB slot except the head: 248,320 rows of
4,096 BF16 values are 1.9 GiB. It streams as 21 chunks of up to 12,288 rows,
each exactly one slot. So that a copy can name part of a weight, it now
carries one argument naming the host bytes:

<!-- source: psionic/lib/device/plan.coax copy -->
```jai
// The copy that fills a slot with streaming index x. Its one argument names
// the host bytes: weight 3+w, a byte offset into it and a byte count.
dp_copy :: (x:int,slot:int)->DpOp {
    w:=dp_stream_weight(x);offset:=0;bytes:=dp_weight_bytes(w);
    if x>=DP_HEAD_BASE {c:=x-DP_HEAD_BASE;offset=2*c*DP_HEAD_ROWS*DP_WIDTH;bytes=2*dp_head_rows(c)*DP_WIDTH;}
    op:=DpOp.{kind=DpOpKind.copy,tensor=w,slot=slot,count=1};
    op.args[0]=DpArg.{kind=4,buffer=3+w,offset=offset,count=bytes};
    return op;
}
```

A streamed matrix is used the way chapter 30 used a streamed feed-forward
matrix: the compute stream waits for the slot's copy, uses it, releases it,
and the copy of the streamed matrix after next starts at once. Two slots keep
one copy in flight while the other is being read:

<!-- source: psionic/lib/device/plan.coax take -->
```jai
// A streamed matrix is read from slot at%2: wait for its copy, use it, then
// release the slot and start the copy of the streamed matrix after next.
dp_take :: (b:DpBuilder)->DpBuilder {return dp_push(b,DpOp.{kind=DpOpKind.wait,slot=b.at%2});}
dp_give :: (b:DpBuilder)->DpBuilder {
    slot:=b.at%2;
    r:=dp_push(b,DpOp.{kind=DpOpKind.release,slot=slot});
    if r.at+2<r.count {r=dp_push(r,dp_copy(r.order[r.at+2],slot));}
    r.at+=1;return r;
}
// A matrix projection whose weight may stream.
dp_project_weight :: (b:DpBuilder,p:DpLayout,w:int,rows:int,cols:int,tokens:int,input:int,output:int)->DpBuilder {
    if !dp_streamed(w,b.stream) {return dp_project(b,p,dp_matrix(w),rows,cols,tokens,input,output);}
    r:=dp_take(b);
    r=dp_project(r,p,DpArg.{kind=4,buffer=1+b.at%2},rows,cols,tokens,input,output);
    return dp_give(r);
}
```

The readout runs per chunk. The one-token product writes 16 lane partials
for the chunk's rows and the reduce writes those rows of the logits. The
arithmetic of every row is the same as in the resident readout: the same
lanes, chunks and f64 sum, only fewer rows per launch:

<!-- source: psionic/lib/device/plan.coax readout -->
```jai
// The head's rows first..first+rows: one-token products into 16 lane
// partials, then their sums into those logits. A streamed chunk's slot is
// released between the two.
dp_readout :: (b:DpBuilder,p:DpLayout,weight:DpArg,first:int,rows:int,streamed:bool)->DpBuilder {
    a:[12]DpArg;w:=weight;w.count=rows*DP_WIDTH/4;
    a[0]=dp_int(rows);a[1]=dp_int(DP_WIDTH);a[2]=w;a[3]=dp_ws(p.final,DP_WIDTH);a[4]=dp_ws(p.lanes,16*rows);
    r:=dp_push(b,dp_launch(DpKernel.matvec,16*rows,a,5));
    if streamed {r=dp_give(r);}
    e:[12]DpArg;e[0]=dp_int(rows);e[1]=dp_int(16);e[2]=dp_ws(p.lanes,16*rows);e[3]=dp_ws(p.logits+4*first,rows);
    return dp_push(r,dp_launch(DpKernel.reduce,rows,e,4));
}
```

## The same calculation

Streaming must change where a weight is read from and nothing else. The plan's
checks show that for every budget they try. A small checker walks a streamed
plan, keeps track of what each slot holds, refuses a copy into a full slot, a
wait before its copy or a read outside its wait and release, and rewrites
each slot view as the host bytes it holds. For a plan that streams 42
feed-forward matrices, the rewritten launches are the resident plan's,
digest for digest. For a plan that streams everything, they are the resident
plan's up to the readout, which reads the head in 21 chunks that cover its
248,320 rows exactly once.

The [example](../examples/share-the-card.coax) prints the plan for 800 MiB,
the room left beside gpt-oss-20b, without a GPU:

```shell
coax/bin/coax run psionic/examples/share-the-card.coax
```

```jai
operations: 1882, launches: 1132, copies: 250
resident: 524345344 bytes, slots: 2 of 100663296 bytes, workspace: 91447808 bytes
copied per decision: 15351152640 bytes
first copies: weights 6 and 7; last: weight 512, bytes 2013265920 to 2034237440
smallest budget: 297026048 bytes; one byte less: memory
```

At 800 MiB a few mixer matrices still fit, so 15.4 of the 15.9 GB are copied
per decision. The smallest budget is the workspace, the vectors and two slots.

## Run it beside gpt-oss

With gpt-oss-20b's server resident on the card and 1.16 GiB free, chapter 30's
harness ran the sample decision with an 800 MiB budget, which the host lowers
to what is free less its reserve (0.65 GiB):

```shell
PSIONIC_DEVICE_BUDGET_MIB=800 psionic/tests/run_elephant_device.sh /path/to/elephant-directory
```

```jai
plan: 1897 operations; workspace 87.2 MiB, resident 0.37 GiB, streamed 14.41 GiB in 255 copies through 2 slots of 96.0 MiB (budget 0.65 GiB)
session: authenticated in 72.7 s; uploaded 0.37 GiB resident in 3.68 s (0.1 GB/s); pinned 14.41 GiB for streaming
run 1: time to response 663.2 ms (device 656.9 ms, input upload 4.27 ms)
run 2: time to response 662.9 ms (device 660.8 ms, input upload 0.08 ms)
run 3: time to response 663.1 ms (device 661.1 ms, input upload 0.07 ms)
device memory in use by this process: about 0.67 GiB; 0.48 GiB left free
choice=yes p_yes=0.757202773 p_no=0.242797227 slot_mass=0.228238784 confidence=0.514405545
```

The acceptance test is chapter 30's, frozen before its first device run: the
choice `yes` and every statistic within 0.0005 of chapter 10's independent
reference and its recorded CPU result. Both pass. The probabilities are
chapter 30's resident tensor-core result to every printed digit, as the plan
checker says they must be.

A warm decision takes 663 ms against chapter 30's 147 ms. 15.5 GB in 0.66 s
is 23 GB/s, the practical rate of the 4080's PCIe 4.0 x16 link: the kernels'
146 ms now hide under the copies.

The `/v1/systemone` server of [chapter 16](ch016-serve-systemone.md) uses the
same engine. Started with `--elephant-gpu` and `--capacity 512` beside the
gpt-oss server, it placed for 512-token decisions in the 0.90 GiB the card
had left after its 256 MiB reserve: 0.22 GiB resident and 14.56 GiB pinned
in host memory for streaming. Both servers then held the card at once
(gpt-oss 13,874 MiB, Elephant 1,140 MiB, 249 MiB free). Six requests of
chapter 10's sample over loopback took 673 to 690 ms each, every one `yes`
with p_yes 0.757202773.

## Limits

Each decision copies every streamed byte again, so a decision is bounded by
PCIe, not by the kernels. When gpt-oss is generating at the same moment the
two share the link and the compute units, and neither runs at its own speed;
the measurements here were taken with the gpt-oss server resident and idle. Each server start still hashes the 18.8 GB checkpoint on the CPU.
The streamed weights are pinned in host memory, 15.9 GB of it per process.
Quantized weights (chapter 34) would halve the bytes; a scheduler that hands
the whole card to whichever model is busy is a separate service.
