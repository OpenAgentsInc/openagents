# 33. Measure the work and plan the memory

> **Status: partly implemented.** The exact accounting of weight, state and
> scratch bytes, FLOPs and bytes moved for Elephant, Mammoth and gpt-oss, the
> residency plan with typed refusals, and a single-thread roofline measured
> on one named host run. Timed phases of a real model run, measured peak
> memory compared against the plan, and a repeat on a quiet host within a
> stated spread are not built yet.

Before you make anything faster, you need to know where the time and the
memory go. This chapter writes that down as exact integer arithmetic on each
model's pinned shape: how many bytes each model holds, how many multiply-adds
one step does, and how many bytes that step reads. It then measures two
numbers on the machine that runs the model, its memory bandwidth and its
arithmetic rate, and places each phase between them. Last, it turns the byte
counts into a residency plan that admits a request, admits it with some
weights streamed, or refuses it with a typed reason before anything is
allocated.

The library is [`residency.coax`](../lib/residency.coax), an `#authority`
file because it decides admission, and
[`roofline.coax`](../lib/roofline.coax), which holds the timed kernels and
the placement arithmetic. Chapters 34, 35 and the rest of Part V name the
bottleneck they target against these numbers.

## The counting rules

The profile is `rp-plan-v1`. Every count is an integer, so two programs that
follow the rules agree exactly.

- **Weights** are counted as stored: BF16 for Elephant and Mammoth, and for
  gpt-oss BF16 attention, router, embeddings and biases with MXFP4 experts
  (half a byte per value plus one scale byte per 32 values).
- **State** is the model's memory between steps, as `f32`: a linear layer's
  convolution history and delta-rule matrices, and a full or sliding
  attention layer's keys and values.
- **Scratch** is one step's workspace, as `f32`: the residual and its normed
  copy for each token in the step, the widest intermediate, one row of
  attention scores per head, and the last token's logits.
- **A FLOP** is half a multiply-add. Counted: every weight product, the
  depthwise convolution, attention scores and values, and three
  multiply-adds per delta-rule state entry (read, update and readout). Not
  counted: norms, activations, gates, softmax, rotary positions and routing
  choices.
- **Bytes moved** are the ideal-reuse traffic of one step: each weight it
  uses is read once, the live state is read once and the new state written
  once, and activations stay in cache. A real run moves at least this much.

## Count the weights

A model's shape is a handful of integers, taken from chapter 15's
model pins and chapter 64's original gpt-oss profiles:

| Field | Elephant | Mammoth | gpt-oss-20b |
| --- | --- | --- | --- |
| Layers | 24 linear, 8 full | 48 linear, 16 full | 12 full, 12 sliding (window 128) |
| Width, feed-forward | 4,096, 12,288 | 5,120, 17,408 | 2,880, 2,880 per expert |
| Heads, key-value heads, head size | 16, 4, 256 | 24, 4, 256 | 64, 8, 64 |
| Delta key heads, value heads, size | 16, 32, 128 | 16, 48, 128 | — |
| Experts, selected | — | — | 32, 4 |
| Vocabulary | 248,320 | 248,320 | 201,088 |

Work through one Elephant linear layer by hand. Its convolved channels are
q and k for 16 key heads and v for 32 value heads, each 128 wide:
(2 × 16 + 32) × 128 = 8,192. Its matrices, all 4,096 columns wide, have
8,192 rows (`in_proj_qkv`), 4,096 (`in_proj_z`), 32 and 32 (`in_proj_a`,
`in_proj_b`), 4,096 (`out_proj`, read as its transpose) and 3 × 12,288 for
the feed-forward gate, up and down projections:

```text
4,096 × (8,192 + 4,096 + 64 + 4,096 + 36,864) = 4,096 × 53,312 = 218,365,952 values
```

A full layer's q projection carries its output gate, so it has 2 × 16 × 256
= 8,192 rows; k and v have 4 × 256 = 1,024 each, and the output projection
4,096:

```text
4,096 × (8,192 + 2,048 + 4,096 + 36,864) = 4,096 × 51,200 = 209,715,200 values
```

The head is 248,320 × 4,096 = 1,017,118,720 values, and the embedding table
the same again. With 24 linear and 8 full layers:

```text
24 × 218,365,952 + 8 × 209,715,200 + 1,017,118,720 = 7,935,623,168 values
```

That is every rank-2 text tensor in the
[pinned manifest](../fixtures/models/elephant/manifest.json) except the
embedding table, summed independently from the manifest's shapes. The
remaining values are vectors: per linear layer the 8,192 × 4 convolution, 32
`A_log`, 32 `dt_bias`, the 128-wide delta norm and two 4,096-wide layer
norms (41,152 values), per full layer two norms and two 256-wide q and k
norms (8,704), and the final norm (4,096): 24 × 41,152 + 8 × 8,704 + 4,096 =
1,061,376 values. At two bytes a value:

```text
2 × (7,935,623,168 + 1,017,118,720 + 1,061,376) = 17,907,606,528 bytes
```

The manifest's file is 18,819,722,392 bytes. The difference is the 333
vision tensors and the file header; a text-only plan keeps neither resident.

`rp_weights` computes the same sums from the shape fields:

<!-- source: psionic/lib/residency.coax weights -->
```jai
rp_weights :: (m:RpModel)->RpWeights #export_c {
    if !rp_valid(m) {return RpWeights.{};}
    w:=m.width;table:=2*m.vocab*w;x:=RpWeights.{embedding=table,expert=rp_expert(m)};
    qkv:=(m.heads+2*m.kv_heads)*m.head_dim;out:=m.heads*m.head_dim;
    if m.arch==RpArch.hybrid {
        lin:=rp_channels(m)*m.conv+2*m.value_heads+m.delta_dim+2*w;full:=2*m.head_dim+2*w;
        x.vectors=2*(m.linear*lin+m.full*full+w);
        x.total=2*(m.linear*rp_dense(m,true)+m.full*rp_dense(m,false))+x.vectors+2*table;
        x.slot=2*w*rp_max(m.ffn,rp_max(rp_channels(m),2*out));
    } else {
        layer:=qkv+3*w+m.heads+m.experts;   // q, k, v and out biases, two norms, sinks, router bias
        x.vectors=2*(rp_layers(m)*layer+w);
        x.total=rp_layers(m)*(2*rp_moe_dense(m)+m.experts*x.expert)+x.vectors+2*table;
        x.slot=rp_max(2*w*qkv,rp_max(2*w*m.experts,x.expert));
    }
    return x;
}
```

The checks compare its totals with independent sums of every pinned tensor:
17,907,606,528 bytes for Elephant and 53,791,996,928 for Mammoth from their
manifests, and 13,761,264,768 for gpt-oss-20b and 65,248,815,744 for
gpt-oss-120b from chapter 64's original descriptor tables. `slot` is the
largest unit that can stream: one feed-forward matrix for Elephant (96 MiB)
and Mammoth (170 MiB), the same slots chapter 45 uses on the card, and the
q, k and v projection for gpt-oss-20b.

## Count the state and the scratch

For a request reserved at 86 tokens, chapter 14's recorded Elephant request,
each linear layer keeps the last three convolution inputs and one 128 × 128
delta matrix per value head:

```text
3 × 8,192 + 32 × 128 × 128 = 24,576 + 524,288 = 548,864 values per linear layer
```

Each full layer keeps a key and a value of 4 × 256 values for each position:
86 × 2 × 1,024 = 176,128 values. In all:

```text
24 × 548,864 + 8 × 176,128 = 13,172,736 + 1,409,024 = 14,581,760 values = 58,327,040 bytes
```

That is the state chapter 15's `ps_plan` reports for the same request, a
second independent check. A gpt-oss sliding layer keeps at most 128
positions, so its state stops growing at the window while a full layer's
grows with the capacity.

The scratch for a step of 86 tokens holds each token's residual and normed
copy (2 × 4,096), the widest intermediate (gate and up, 2 × 12,288), one score
row per head over 86 positions and 248,320 logits:

```text
4 × (86 × (8,192 + 24,576) + 16 × 86 + 248,320) = 12,270,976 bytes
```

<!-- source: psionic/lib/residency.coax state -->
```jai
// f32 state for capacity tokens: each linear layer's convolution history
// (conv - 1 inputs) and delta state (value_heads matrices of delta_dim
// squared), each full layer's keys and values for every position, each
// sliding layer's for at most window positions.
rp_state :: (m:RpModel,capacity:int)->int #export_c {
    kv:=2*m.kv_heads*m.head_dim;
    return rp_recurrent(m)+4*(m.full*capacity*kv+m.sliding*rp_min(capacity,m.window)*kv);
}
rp_recurrent :: (m:RpModel)->int {return 4*m.linear*((m.conv-1)*rp_channels(m)+rp_values(m)*m.delta_dim);}
// f32 scratch for chunk tokens at once: the residual and its normed copy,
// the widest intermediate, one attention score row per head over the
// capacity, and the last token's logits.
rp_scratch :: (m:RpModel,chunk:int,capacity:int)->int #export_c {
    widest:=rp_max((m.heads+2*m.kv_heads)*m.head_dim,m.experts+2*m.ffn);
    if m.arch==RpArch.hybrid {widest=rp_max(rp_max(2*m.ffn,2*(m.heads+m.kv_heads)*m.head_dim),rp_channels(m)+rp_values(m)+2*m.value_heads);}
    return 4*(chunk*(2*m.width+widest)+m.heads*capacity+m.vocab);
}
```

## Count the work of a step

Decoding one token multiplies every matrix except the embedding table once,
which is one multiply-add per matrix value: 7,935,623,168 for Elephant. Add
the convolution (24 × 8,192 × 4 = 786,432), the delta rule (24 × 3 × 32 ×
128 × 128 = 37,748,736) and attention over n positions in 8 full layers
(8 × 2 × 16 × 256 × n). At n = 86 that is 5,636,096, so:

```text
2 × (7,935,623,168 + 786,432 + 37,748,736 + 5,636,096) = 15,959,588,864 FLOPs
```

The same step reads all weights but the embedding table, plus one 8,192-byte
embedding row; reads and writes the recurrent state (2 × 24 × 548,864 × 4 =
105,381,888 bytes); reads 86 positions of keys and values and writes one more
in each full layer (8 × 87 × 8,192 = 5,701,632 bytes):

```text
15,873,369,088 + 8,192 + 105,381,888 + 5,701,632 = 15,984,460,800 bytes
```

The arithmetic intensity is FLOPs per byte: 15,959,588,864 / 15,984,460,800
= 0.998. With BF16 weights each two-byte value feeds one multiply-add, two
FLOPs, so a decode step does about one FLOP per byte whatever the model's
size. Prefill of the same 86 tokens reads the weights once and uses each value
86 times, so its intensity is 75.3.

gpt-oss-20b changes the picture. A token uses 4 of 32 experts, so its active
matrix values are the 1,218,355,200 dense values (attention, router and head)
plus one eighth of the experts' 19,110,297,600: 3,607,142,400, which is the
"3.6B active" the upstream card states. Its experts are stored in 4.25 bits a
value, so a decode step at 86 does 1.95 FLOPs per byte.

<!-- source: psionic/lib/residency.coax work -->
```jai
// Multiply-adds per token outside attention over positions: every layer's
// dense matrices, the selected experts, the convolution and the delta rule.
rp_token_macs :: (m:RpModel)->int {
    if m.arch==RpArch.moe {return rp_layers(m)*(rp_moe_dense(m)+m.selected*3*m.ffn*m.width);}
    delta:=rp_channels(m)*m.conv+3*rp_values(m)*m.delta_dim;
    return m.linear*(rp_dense(m,true)+delta)+m.full*rp_dense(m,false);
}
// Attention multiply-adds for one query over n positions (scores, values).
rp_attention_macs :: (m:RpModel,n:int)->int {
    return 2*m.heads*m.head_dim*(m.full*n+m.sliding*rp_min(n,m.window));
}
rp_milli :: (flops:int,bytes:int)->int {
    if bytes<=0 {return 0;}
    return flops/bytes*1000+flops%bytes*1000/bytes;
}
rp_check :: (m:RpModel,n:int)->RpStatus {
    if !rp_valid(m) {return RpStatus.model;}
    if n<1 || n>m.max_tokens {return RpStatus.request;}
    return RpStatus.ok;
}
// One decode step at position n (n positions including its own).
rp_decode :: (m:RpModel,n:int)->RpWork #export_c {
    s:=rp_check(m,n);if s!=RpStatus.ok {return RpWork.{status=s};}
    w:=rp_weights(m);head:=m.vocab*m.width;
    flops:=2*(rp_token_macs(m)+head+rp_attention_macs(m,n));
    kv:=4*2*m.kv_heads*m.head_dim;
    unused:=rp_layers(m)*(m.experts-m.selected)*w.expert;
    state:=2*rp_recurrent(m)+m.full*(n*kv+kv)+m.sliding*(rp_min(n,m.window)*kv+kv);
    bytes:=w.total-w.embedding+2*m.width-unused+state;
    return RpWork.{flops=flops,bytes=bytes,milli=rp_milli(flops,bytes)};
}
// Prefill of t tokens from an empty state, logits for the last token only.
rp_prefill :: (m:RpModel,t:int)->RpWork #export_c {
    s:=rp_check(m,t);if s!=RpStatus.ok {return RpWork.{status=s};}
    w:=rp_weights(m);head:=m.vocab*m.width;
    k:=rp_min(t,m.window);sliding:=k*(k+1)/2+(t-k)*m.window;   // 1 + 2 + ... + k, then the window
    attention:=2*m.heads*m.head_dim*(m.full*(t*(t+1)/2)+m.sliding*sliding);
    flops:=2*(t*rp_token_macs(m)+head+attention);
    unused:=rp_layers(m)*rp_max(0,m.experts-t*m.selected)*w.expert;
    bytes:=w.total-w.embedding+t*2*m.width-unused+rp_recurrent(m)+(m.full+m.sliding)*t*4*2*m.kv_heads*m.head_dim;
    return RpWork.{flops=flops,bytes=bytes,milli=rp_milli(flops,bytes)};
}
```

## Measure the host

A phase with intensity I can go no faster than the smaller of the host's
arithmetic rate P and I times its memory bandwidth B. The intensity where the
two meet, P / B, is the ridge: below it a phase is bound by memory, above it
by arithmetic. To place phases you need P and B for the code you actually
run, so the two benchmarks are Coax kernels compiled the way the release
checks compile Psionic:

<!-- source: psionic/lib/roofline.coax kernels -->
```jai
// a = b + s * c: reads 8 bytes and writes 4 per element, two FLOPs.
rf_triad :: (a:[]f32,b:[]f32,c:[]f32,s:f32)->int #export_c {
    if b.count!=a.count || c.count!=a.count {return -1;}
    for i:0..<a.count {a[i]=b[i]+s*c[i];}
    return a.count;
}
rf_triad_bytes :: (n:int)->int #export_c {return 12*n;}
// x = x * m + k, rounds times over a buffer small enough to stay in cache:
// two FLOPs per value per round, and the values are independent, so the
// compiler may keep several in flight at once.
rf_madd :: (x:[]f32,rounds:int,m:f32,k:f32)->int #export_c {
    if rounds<0 {return -1;}
    for r:0..<rounds {for i:0..<x.count {x[i]=x[i]*m+k;}}
    return 2*rounds*x.count;
}
```

The triad streams three 64 MiB arrays, far larger than the cache, and moves
12 bytes per element. The multiply-add loop runs 100,000 rounds over 4,096
values that stay in the first-level cache, so memory does not limit it. The
[native harness](../platform/roofline.c) owns the effects: it allocates the
arrays, reads the monotonic clock, runs each kernel once to warm up and seven
more times, and keeps the best run. Coax turns the times into rates and places
each phase:

<!-- source: psionic/lib/roofline.coax place -->
```jai
// Millions of units per second from units (at least 0) done in ns nanoseconds.
rf_rate :: (units:int,ns:int)->int #export_c {
    if ns<=0 {return 0;}
    return units/ns*1000+units%ns*1000/ns;
}
// Place a phase with intensity milli (chapter 33's rp_* accounting) and
// flops FLOPs against the host's two roofs.
rf_place :: (h:RfHost,milli:int,flops:int)->RfPlace #export_c {
    if h.mflops<=0 || h.mbytes<=0 || milli<0 || flops<0 {return RfPlace.{};}
    p:=RfPlace.{ridge=h.mflops*1000/h.mbytes};
    memory:=h.mbytes*milli/1000;
    if milli<p.ridge {p.bound=RfBound.memory;p.attainable=memory;}
    else {p.bound=RfBound.compute;p.attainable=h.mflops;}
    if p.attainable>0 {p.ns=flops*1000/p.attainable;}
    return p;
}
```

Run it on a host and record the receipt:

```shell
psionic/tests/roofline.sh --record
```

The [receipt for coderos-4080](../fixtures/roofline/coderos-4080.json), an
Intel Core i7-14700K under Linux 6.18 with GCC 15.2 at `-O2`, one thread,
recorded 2026-11-01:

| Measure | Best of 7 | Median |
| --- | --- | --- |
| Triad bandwidth | 23,361 MB/s | 21,018 MB/s |
| Multiply-add rate | 35,909 MFLOP/s | 35,120 MFLOP/s |
| Ridge | 1.537 FLOPs per byte | |

| Phase | FLOPs per byte | Bound | Shortest time on this roof |
| --- | --- | --- | --- |
| Elephant decode at 86 | 0.998 | memory | 0.68 s |
| Elephant prefill of 86 | 75.3 | compute | 33.4 s |
| Mammoth decode at 86 | 0.998 | memory | 2.21 s |
| gpt-oss-20b decode at 86 | 1.950 | compute | 0.20 s |
| gpt-oss-20b prefill of 86 | 41.5 | compute | 14.6 s |

Read these as floors for single-threaded Coax code on this machine, not as
the chip's limits: one thread uses one of its 28 hardware threads. A decode
step of a BF16 model sits below the ridge, so making its arithmetic faster
does nothing until it reads fewer bytes; prefill sits far above, so it waits
on arithmetic and gains from more cores ([chapter 35](ch035-use-every-core.md)).
gpt-oss-20b's MXFP4 experts move its decode just above the ridge. The rate
counts only the multiply-adds; decoding BF16 or MXFP4 values to `f32` is
further work this roof does not include.

The numbers are not stable on a shared machine. The receipt records the load
average (12.5 on 28 hardware threads). Three runs on the same day measured
triad bandwidth of 26,902, 7,347 and 23,361 MB/s and multiply-add rates of
37,039, 16,825 and 35,909 MFLOP/s; the slow run was taken at a load average of
20. That is why the receipt keeps the best, median and worst runs and names
the machine, compiler, flags and load, and why no measured number appears in
an exact transcript. The check that the same workload run twice on a quiet
host agrees within a stated spread is still open.

## Plan the memory

`rp_plan` takes a shape, a request (the token capacity to reserve, the tokens
processed together in a step, and whether weights may stream from a slower
tier) and a budget in bytes:

<!-- source: psionic/lib/residency.coax plan -->
```jai
// Admit a request as resident, as resident with streamed weights, or refuse
// it with the first reason that applies. Nothing is allocated here: the
// caller allocates the plan's bytes only after status ok.
rp_plan :: (m:RpModel,r:RpRequest,budget:int)->RpPlan #export_c {
    if !rp_valid(m) {return RpPlan.{status=RpStatus.model};}
    if r.capacity<1 || r.capacity>m.max_tokens || r.chunk<1 || r.chunk>r.capacity {return RpPlan.{status=RpStatus.request};}
    if budget<0 {return RpPlan.{status=RpStatus.budget};}
    w:=rp_weights(m);
    p:=RpPlan.{weights=w.total,state=rp_state(m,r.capacity),scratch=rp_scratch(m,r.chunk,r.capacity),slot=w.slot};
    live:=p.state+p.scratch;all:=w.total+live;least:=w.vectors+live+2*w.slot;
    p.need=ifx r.stream then rp_min(all,least) else all;
    if all<=budget {p.mode=RpMode.resident;p.resident=w.total;return p;}
    if live>budget {p.status=RpStatus.state;return p;}
    if !r.stream {p.status=RpStatus.weights;return p;}
    if least>budget {p.status=RpStatus.stream;return p;}
    p.mode=RpMode.streamed;p.resident=budget-live-2*w.slot;p.streamed=w.total-p.resident;
    return p;
}
```

The order of the checks is the order of the remedies. A request whose state
and scratch alone exceed the budget is refused for its `state`: only a
shorter request helps. If the whole plan does not fit and streaming is off,
the reason is `weights`: allow streaming or add memory. With streaming on, the
vectors, state, scratch and two slots must fit, as on the card in chapter 45;
otherwise the reason is `stream`. A streamed plan keeps as many weight bytes
resident as the budget allows and reports how many must stream on every step.
`need` is always the smallest budget that admits the same request.

The plan is arithmetic. The caller allocates only after `status` is `ok`, so a
refusal leaves nothing half-allocated. The `#forall` check confirms that for
40 budgets around gpt-oss-20b's need, a plan is admitted exactly when the
budget reaches `need` and its bytes then fit inside the budget.

The [example](../examples/residency-plan.coax) prints the counts and a few
plans:

<!-- source: psionic/examples/residency-plan.coax example -->
```jai
main :: () {
    show("elephant-1",rp_elephant());
    show("mammoth-1",rp_mammoth());
    show("gpt-oss-20b",rp_gptoss_20b());
    gib:=1<<30;
    plan("elephant-1",rp_elephant(),86,true,64*gib);
    plan("elephant-1",rp_elephant(),86,true,16*gib);
    plan("mammoth-1",rp_mammoth(),86,false,48*gib);
    plan("gpt-oss-20b",rp_gptoss_20b(),4096,false,16000000000);
    plan("gpt-oss-20b",rp_gptoss_20b(),131072,false,16000000000);
    plan("gpt-oss-20b",rp_gptoss_20b(),131072,true,16000000000);
}
```

```text
elephant-1: weights 17907606528 bytes (vectors 2122752, slot 100663296), state for 86 tokens 58327040 bytes, scratch 12270976 bytes
  decode at 86: 15959588864 FLOPs, 15984460800 bytes, 998 mFLOP/byte
  prefill of 86: 1199135391744 FLOPs, 15932400640 bytes, 75263 mFLOP/byte
mammoth-1: weights 53791996928 bytes (vectors 5291008, slot 178257920), state for 86 tokens 168165376 bytes, scratch 16500800 bytes
  decode at 86: 51508150272 FLOPs, 51574400000 bytes, 998 mFLOP/byte
  prefill of 86: 4212125990912 FLOPs, 51418246144 bytes, 81918 mFLOP/byte
gpt-oss-20b: weights 13761264768 bytes (vectors 670848, slot 29491200), state for 86 tokens 8454144 bytes, scratch 4800256 bytes
  decode at 86: 7248101376 FLOPs, 3716641536 bytes, 1950 mFLOP/byte
  prefill of 86: 523446829056 FLOPs, 12611947392 bytes, 41504 mFLOP/byte
elephant-1 at 86 tokens, budget 68719476736 bytes, streaming on: resident (ok), need 274047360 bytes
elephant-1 at 86 tokens, budget 17179869184 bytes, streaming on: streamed (ok), need 274047360 bytes, 16907944576 resident and 999661952 streamed
mammoth-1 at 86 tokens, budget 51539607552 bytes, streaming off: refused (weights: the weights, state and scratch exceed the budget and streaming is off), need 53976663104 bytes
gpt-oss-20b at 4096 tokens, budget 16000000000 bytes, streaming off: resident (ok), need 13994394240 bytes
gpt-oss-20b at 131072 tokens, budget 16000000000 bytes, streaming off: refused (weights: the weights, state and scratch exceed the budget and streaming is off), need 20268024448 bytes
gpt-oss-20b at 131072 tokens, budget 16000000000 bytes, streaming on: streamed (ok), need 6566412928 bytes, 9434257920 resident and 4327006848 streamed
```

gpt-oss-20b's card says the model runs "within 16GB of memory". Under this
profile, with `f32` keys and values, that holds for a 4,096-token reservation
(13,994,394,240 bytes) and not for the full 131,072-token context without
streaming (20,268,024,448 bytes). The upstream claim stays an upstream claim;
the plan states what fits under its own profile.

## Refuse what does not fit

The [refusal example](../examples/residency-refuse.coax) sends requests that
the plan refuses, one for each reason:

<!-- source: psionic/examples/residency-refuse.coax refuse -->
```jai
main :: () {
    e:=rp_elephant();r:=RpRequest.{capacity=86,chunk=86,stream=true};
    say("capacity 16385 for a 16384-token model",rp_plan(e,RpRequest.{capacity=16385,chunk=86,stream=true},1<<40));
    say("a chunk of 87 in a capacity of 86",rp_plan(e,RpRequest.{capacity=86,chunk=87,stream=true},1<<40));
    say("a budget of -1 bytes",rp_plan(e,r,-1));
    x:=e;x.kv_heads=3;
    say("16 heads over 3 key-value heads",rp_plan(x,r,1<<40));
    least:=rp_plan(e,r,0).need;
    say("Elephant at the least budget that streams",rp_plan(e,r,least));
    say("one byte less",rp_plan(e,r,least-1));
    r.stream=false;
    say("the same budget with streaming off",rp_plan(e,r,least));
    long:=RpRequest.{capacity=16384,chunk=512,stream=true};
    say("16384 tokens in 1 GiB",rp_plan(e,long,1<<30));
}
```

```text
capacity 16385 for a 16384-token model: refused (request: capacity or chunk outside 1..max tokens), need 0 bytes
a chunk of 87 in a capacity of 86: refused (request: capacity or chunk outside 1..max tokens), need 0 bytes
a budget of -1 bytes: refused (budget: a budget is never negative), need 0 bytes
16 heads over 3 key-value heads: refused (model: the shape is outside the planner's bounds), need 0 bytes
Elephant at the least budget that streams: streamed (ok), need 274047360 bytes
one byte less: refused (stream: the vectors, state, scratch and two stream slots exceed the budget), need 274047360 bytes
the same budget with streaming off: refused (weights: the weights, state and scratch exceed the budget and streaming is off), need 17978204544 bytes
16384 tokens in 1 GiB: refused (state: the state and scratch alone exceed the budget), need 1399032832 bytes
```

Malformed requests, budgets and shapes report a `need` of 0 because no plan
was sized. The shape bounds (at most 128 layers, width 8,192, 262,144 tokens
and so on) keep every count below 2^63; a check sizes the largest admitted
shapes of both architectures to show it.

## Check it

```shell
coax/bin/coax test psionic/lib/residency.coax
coax/bin/coax mutate psionic/lib/residency.coax
coax/bin/coax test psionic/lib/roofline.coax
coax/bin/coax run psionic/examples/residency-plan.coax
psionic/tests/roofline.sh
```

`coax mutate` leaves no survivors in `residency.coax` (509 mutants) or in
`roofline.coax`'s own procedures (66 mutants). The checks' expected values
come from the pinned manifests and descriptor tables, chapter 15's recorded
state, and the hand counts above.

## What is not built yet

- **Timed phases of a real run.** Chapter 18's phase timer around a pinned
  Elephant or gpt-oss workload, with warm-up runs and latency percentiles per
  phase, measured beside this chapter's predicted floors.
- **Measured allocation against the plan.** Peak memory of a real run
  compared with the predicted weight, state and scratch bytes within a stated
  margin. The scratch rule here is a planning rule, not yet any executor's
  exact workspace.
- **A quiet-host repeat.** The same workload twice on a quiet host, with its
  spread stated.
- **Computing from mapped BF16 rows without expanding them to `f32`.** The
  plan counts weights as stored; whether each executor does so is checked in
  its own chapter.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [gpt-oss target](../docs/targets/gpt-oss.md) for the upstream sizing claims.
- [Making Psionic serving faster](../docs/serving/performance.md).
- [Chapter 15](ch015-execute-mammoth.md)'s `ps_plan` and
  [chapter 45](ch045-share-the-card.md)'s streamed device plan.
