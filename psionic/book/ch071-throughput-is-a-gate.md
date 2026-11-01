# 71. Throughput is a gate

> **Status: partly implemented.** The gate (profile tg-gptoss20b-v1) runs on
> the CPU, and an opt-in harness applies it to the original gpt-oss-20b on
> one RTX 4080. With chapter 69's tensor-core expert products wired into
> chapter 67's prefill, both goals pass; with the f32 prefill, prefill fails.
> P95 and per-phase memory and transfer reports, a packaged reference engine
> on the same device, and continuous batching at concurrency 4 and 8 are not
> built; the last section lists them.

A correct decoder that is too slow is not useful inference. This chapter
turns gpt-oss-20b speed on the RTX 4080 into a verdict: one fixed workload,
two goals frozen before any measurement, and a rule that says pass, fail or
too noisy. It then makes prefill fast enough to pass, by running chapter 67's
grouped expert products on the tensor cores with chapter 69's profile.

The gate is [`throughput-gate.coax`](../lib/throughput-gate.coax), an
`#authority` file because its verdict decides which speed claims the book may
make. It reuses chapter 34's warm-run statistics
([`comparator.coax`](../lib/comparator.coax)). The measuring harness is
[`gptoss_gate.c`](../tests/gptoss_gate.c), run by
[`check_throughput_gate.sh`](../tests/check_throughput_gate.sh). The
tensor-core prefill is [`device-mma.coax`](../lib/gptoss/device-mma.coax).

## The workload is frozen

The workload is 1,024 input IDs and 128 forced output IDs on the original
gpt-oss-20b, with a 2,048-position context and one request at a time. Coax
owns the IDs, so the harness cannot feed different ones:

<!-- source: psionic/lib/throughput-gate.coax workload -->
```jai
// Input ID i is 1000 + (i * 7919) mod 190000; the 128 forced output IDs
// repeat inputs 0 to 127. Any other index has no ID.
tg_id :: (i:int)->int #export_c {
    if i<0 || i>=TG_INPUT+TG_OUTPUT {return -1;}
    j:=i;if j>=TG_INPUT {j-=TG_INPUT;}
    return 1000+(j*7919)%190000;
}
TG_FNV_BASIS :: cast(u64)0xcbf29ce484222325;
// FNV-1a over bytes, and over the IDs first..<last as 8 little-endian bytes each.
tg_word :: (s:string)->u64 #export_c {
    h:=TG_FNV_BASIS;
    for i:0..<s.count {h=mul_wrap(h^cast(u64)s[i],cast(u64)0x100000001b3);}
    return h;
}
tg_ids_word :: (first:int,last:int)->u64 {
    h:=TG_FNV_BASIS;
    for i:first..<last {v:=cast(u64)tg_id(i);for b:0..<8 {h=mul_wrap(h^((v>>cast(u64)(8*b))&255),cast(u64)0x100000001b3);}}
    return h;
}
tg_corpus_word :: ()->u64 #export_c {return tg_ids_word(0,TG_INPUT+TG_OUTPUT);}
tg_work :: ()->TgWork #export_c {
    return TgWork.{model=tg_word(TG_MODEL),corpus=tg_corpus_word(),input=TG_INPUT,output=TG_OUTPUT,context=TG_CONTEXT,concurrency=1};
}
```

The IDs are the same corpus chapter 67 measured: 1000, 8919, 16838, and at
i = 24, 24 × 7,919 = 190,056 wraps to 56, giving 1056. Forced output IDs
measure the decoder's work, not the quality of sampled text.

A run names what it did as identity words: FNV-1a 64 of the model name and
revision, and of the 1,152 IDs actually fed in. The checks compare both words
with values computed outside Coax by a separate Python one-liner over the same
bytes (`0x65a9dd2191394d40` and `0xfddb0726d56ca5de`).

## Two goals, frozen first

<!-- source: psionic/lib/throughput-gate.coax goals -->
```jai
// The goals, frozen before any measurement, in nanoseconds per run.
TG_PREFILL_LIMIT :: 2000000000;  // 1,024 input IDs in at most 2 s
TG_DECODE_LIMIT :: 5120000000;   // 128 output IDs at 25 tokens/s or faster
```

Decode at 25 tokens/s or faster means 128 tokens in at most 128 / 25 =
5.12 s. Both goals are times, so both are "lower is better" and one rule
decides them.

## Work it by hand

Take five prefill runs of 1.90, 1.92, 1.94, 1.96 and 1.98 s.

```text
mean       = 9.70 / 5 = 1.94 s
deviations = -0.04, -0.02, 0, 0.02, 0.04
squares    = 0.0016 + 0.0004 + 0 + 0.0004 + 0.0016 = 0.0040
variance   = 0.0040 / (5 - 1) = 0.0010
sd         = sqrt(0.0010) = 0.0316228 s
se         = sd / sqrt(5) = 0.0141421 s
mean + 2 se = 1.94 + 0.0282843 = 1.9682843 s   <= 2 s  -> pass
```

The spread is 0.0316 s, under a tenth of the mean (0.194 s), so the runs are
quiet enough to decide anything. Shift every run up by 0.05 s: the mean is
1.99 s, mean + 2 se = 2.018 s is over the limit and mean - 2 se = 1.962 s is
under it, so the gate cannot tell and says too noisy, within two standard
errors of the limit. Shift by another 0.05 s: mean - 2 se = 2.0117 s is over
2 s, and the goal fails.

## Deciding one goal

A goal reuses chapter 34's Welford statistics, with its admission rule: 5 to
1,024 runs, each a positive number of nanoseconds.

<!-- source: psionic/lib/throughput-gate.coax one-goal -->
```jai
// Pass when the mean plus two standard errors is within the limit; fail when
// the mean minus two standard errors is beyond it; otherwise too close to
// call. A spread over a tenth of the mean decides nothing (chapter 34's rule).
tg_goal :: (runs:[]int,limit:int)->TgGoal #export_c {
    g:=TgGoal.{verdict=TgVerdict.refused,reason=TgReason.stats,stats=cm_welford(runs),limit=limit};
    if g.stats.status!=CmStatus.ok {return g;}
    g.se=g.stats.sd/sqrt(cast(f64)g.stats.n);
    g.verdict=TgVerdict.noisy;g.reason=TgReason.spread;
    if g.stats.sd*10>g.stats.mean {return g;}
    g.reason=TgReason.close;
    if g.stats.mean+2*g.se<=cast(f64)limit {g.verdict=TgVerdict.pass;g.reason=TgReason.none;}
    if g.stats.mean-2*g.se>cast(f64)limit {g.verdict=TgVerdict.fail;g.reason=TgReason.none;}
    return g;
}
```

The design draft asked for a median. The gate uses the mean and its standard
error instead, because chapter 34 already qualifies those two statistics two
ways and the standard error says how sure the verdict is; with five quiet runs
the two summaries differ by far less than either margin below.

## The whole gate

<!-- source: psionic/lib/throughput-gate.coax decide -->
```jai
// The whole gate. Refusals come first, in this order; then both goals.
// A goal that fails decides the gate even when the other is too noisy.
tg_decide :: (work:TgWork,bind:TgBind,same_outputs:bool,prefill:[]int,decode:[]int)->TgGate #export_c {
    gate:=TgGate.{verdict=TgVerdict.refused};f:=tg_work();
    if work.model!=f.model || work.corpus!=f.corpus || work.input!=f.input || work.output!=f.output || work.context!=f.context || work.concurrency!=f.concurrency {gate.reason=TgReason.workload;return gate;}
    if bind.build==0 || bind.commit==0 || bind.machine==0 || bind.device==0 {gate.reason=TgReason.unbound;return gate;}
    if bind.warmup<1 {gate.reason=TgReason.warmup;return gate;}
    if !same_outputs {gate.reason=TgReason.outputs;return gate;}
    if prefill.count!=decode.count {gate.reason=TgReason.pairs;return gate;}
    gate.prefill=tg_goal(prefill,TG_PREFILL_LIMIT);gate.decode=tg_goal(decode,TG_DECODE_LIMIT);
    a:=gate.prefill.verdict;b:=gate.decode.verdict;
    if a==TgVerdict.refused || b==TgVerdict.refused {gate.reason=TgReason.stats;return gate;}
    gate.verdict=TgVerdict.noisy;
    if a==TgVerdict.pass && b==TgVerdict.pass {gate.verdict=TgVerdict.pass;}
    if a==TgVerdict.fail || b==TgVerdict.fail {gate.verdict=TgVerdict.fail;}
    return gate;
}
```

`same_outputs` is how speed stays tied to correctness. The harness runs two
fixture prompts greedily ("Say hi." and "List the integers from 1 to 100,
separated by commas.", up to 256 IDs each) before and after the timed runs,
and passes true only when both runs' output words equal the reference path's
words given on its command line. A faster path that changes a greedy token is
refused, not timed.

## Run it on the CPU

The [example](../examples/throughput-gate.coax) prints the workload, decides
the hand-worked case and then decides the two receipts recorded on the RTX
4080, first with the f32 prefill and then with the tensor-core prefill (the
next sections show where they come from):

```shell
coax/bin/coax run psionic/examples/throughput-gate.coax
```

```text
profile tg-gptoss20b-v1: 1024 input IDs, 128 forced output IDs, context 2048, concurrency 1
first IDs 1000 8919 16838 1056; output ID 0 is 1000; ID 1152 is -1
model word 7325629405134933312; corpus word 18292222174866220510
hand prefill goal: n=5 mean=1940000000.0 ns sd=31622776.60168379 ns se=14142135.623730948 ns limit=2000000000 ns: pass (none)
RTX 4080, f32 prefill: prefill goal: n=5 mean=3107498773.8 ns sd=3330915.139690083 ns se=1489630.5359260465 ns limit=2000000000 ns: fail (none)
RTX 4080, f32 prefill: decode goal: n=5 mean=2276840236.4 ns sd=2635222.580079108 ns se=1178507.3649798536 ns limit=5120000000 ns: pass (none)
RTX 4080, f32 prefill: decode rate at the mean: 56.21826158623485 tokens/s
RTX 4080, f32 prefill: gate: fail (none)
RTX 4080, tensor-core prefill: prefill goal: n=5 mean=1510536010.4 ns sd=2164346.863763451 ns se=967925.3428527105 ns limit=2000000000 ns: pass (none)
RTX 4080, tensor-core prefill: decode goal: n=5 mean=2277436394.0 ns sd=3281623.692986197 ns se=1467586.7308182071 ns limit=5120000000 ns: pass (none)
RTX 4080, tensor-core prefill: decode rate at the mean: 56.20354550283875 tokens/s
RTX 4080, tensor-core prefill: gate: pass (none)
```

<!-- source: psionic/examples/throughput-gate.coax example -->
```jai
main :: () {
    w:=tg_work();
    print("profile %: % input IDs, % forced output IDs, context %, concurrency %\n",TG_PROFILE,w.input,w.output,w.context,w.concurrency);
    print("first IDs % % % %; output ID 0 is %; ID 1152 is %\n",tg_id(0),tg_id(1),tg_id(2),tg_id(24),tg_id(1024),tg_id(1152));
    print("model word %; corpus word %\n",w.model,w.corpus);
    // The hand-worked case: five prefill runs from 1.90 s to 1.98 s.
    hand:=[5]int.[1900000000,1920000000,1940000000,1960000000,1980000000];
    show("hand","prefill",tg_goal(hand[..],TG_PREFILL_LIMIT));
    // The RTX 4080 receipts.
    p:=TG_R1_PREFILL;d:=TG_R1_DECODE;
    receipt("RTX 4080, f32 prefill:",TG_R1_BUILD,TG_R1_COMMIT,TG_R1_MACHINE,TG_R1_DEVICE,p[..],d[..]);
    q:=TG_R2_PREFILL;e:=TG_R2_DECODE;
    receipt("RTX 4080, tensor-core prefill:",TG_R2_BUILD,TG_R2_COMMIT,TG_R2_MACHINE,TG_R2_DEVICE,q[..],e[..]);
}
```

## Refusals

The [refusal example](../examples/throughput-gate-refuse.coax) breaks one
thing at a time. Each line is refused with its typed reason before any goal is
decided:

```text
the frozen workload: pass (none)
1,023 input IDs: refused (not the frozen workload)
other input IDs: refused (not the frozen workload)
four requests at once: refused (not the frozen workload)
no commit recorded: refused (a build, commit, machine or device is not recorded)
no warm-up run: refused (no warm-up run)
greedy outputs changed: refused (greedy outputs differ from the reference path)
five prefill runs, four decode runs: refused (prefill and decode run counts differ)
four runs: refused (the timings were refused)
a decode run of 0 ns: refused (the timings were refused)
```

## Where the prefill time goes

The gate's first verdict on the GPU was a fail. Chapter 66's decoder with
chapter 70's replay runs 128 forced tokens in 2.277 s, 56.2 tokens/s, more
than twice the goal. Chapter 67's prefill takes 3.107 s. A test build of the
same harness times every launch of one 1,024-token prefill with CUDA events:

| Kernel | Launches | Device time |
| --- | ---: | ---: |
| `gk_batch_mx_part`, f32 grouped expert products | 384 | 2.047 s |
| `gk_batch_bf_part`, BF16 attention and router products | 576 | 0.277 s |
| `gk_batch_scores`, attention scores | 192 | 0.164 s |
| `gk_flags_finish` and `gk_flags_part`, numerical guards | 5,408 | 0.208 s |
| `gk_mx_finish`, expert lane sums and bias | 384 | 0.110 s |
| everything else | | 0.324 s |
| total | | 3.130 s |

Two thirds of prefill is the expert products: 24 layers × 8 chunks × 2
projections = 384 launches. Chapter 69 measured its tensor-core product about
six times faster than `gk_batch_mx_part` at 32 tokens per expert. A 128-token
chunk gives 512 token-expert pairs over 32 experts, about 16 per expert, so
the gain should be smaller, but even a fourfold gain on 2.047 s brings the
prefill near 3.13 - 2.05 + 0.51 = 1.6 s.

## Expert products on the tensor cores

Chapter 69's `mm_part` multiplies one expert by 32 contiguous tokens per warp.
Prefill's tokens are not contiguous per expert: chapter 67's `gk_group` lists
each expert's token-slot pairs in `map`, and `gk_group_prefix` turns the
counts into 8-row tiles. Expert e owns grouped rows from `8 * counts[E + e]`;
row `8 * counts[E + e] + o` holds pair `map[e * tokens + o]` while
`o < counts[e]`. Three kernels follow that layout.

The split copies each grouped row's input into two BF16 limbs, as chapter 69's
`mm_split` does, reading token `i / selected` for the first projection and
row `i` for the second:

<!-- source: psionic/lib/gptoss/device-mma.coax mma-split -->
```jai
// Item g*(cols/2)+p: columns 2p and 2p+1 of grouped row g. Pair i reads
// token i/selected (the first projection, slot_input 0) or row i of x (the
// second projection, slot_input 1). Rows past an expert's pairs, and past
// the last tile (gk_expert_tile gives `experts`, which owns no pairs), are
// left as they are: they reach only accumulator columns that gk_mma_part
// never stores.
gk_mma_split :: (item:int,cols:int,tokens:int,experts:int,selected:int,slot_input:int,map:[]int,counts:[]int,x:[]f32,hi:[]u32,lo:[]u32) #export_c {
    half:=cols/2;p:=item%half;g:=item/half;
    e:=gk_expert_tile(g/8,experts,counts);o:=g-8*counts[experts+e];
    if o>=counts[e] {return;}
    i:=map[e*tokens+o];t:=i/selected;if slot_input!=0 {t=i;}
    at:=t*cols+2*p;a:=x[at];b:=x[at+1];
    ha:=dw_bf16(a);hb:=dw_bf16(b);
    la:=dw_bf16(a-f32_from_bits(ha<<16));lb:=dw_bf16(b-f32_from_bits(hb<<16));
    hi[item]=ha|(hb<<16);lo[item]=la|(lb<<16);
}
```

One warp multiplies 16 rows of one expert by up to 32 of its grouped rows.
Warps are numbered over 32-row groups in expert order, so a warp finds its
expert by counting `ceil(counts[e] / 32)` groups per expert. The fragment
decode, the two-limb products and the scale marks are chapter 69's:

<!-- source: psionic/lib/gptoss/device-mma.coax mma-part -->
```jai
// The expert and group of one warp's 32 grouped rows, as
// expert*65536+group, or -1 past the last group: expert e owns
// ceil(counts[e] / 32) groups, in expert order.
gk_mma_group :: (quad:int,experts:int,counts:[]int)->int {
    r:=-1;seen:=0;
    for e:0..<experts {q:=(counts[e]+31)/32;if r<0 && quad<seen+q {r=e*65536+quad-seen;}seen+=q;}
    return r;
}
// One warp: rows r0..r0+15 of one expert times up to 32 of its grouped rows,
// over one slice of the columns. Work item (quad*blocks+block)*slices+slice.
// Writes partial[(slice*grouped+g)*rows+row] for each valid grouped row g,
// and marks there 1 when the lane read a scale above 252 (gk_mma_finish then
// flags the output, as gk_batch_mx_part's non-finite partials do).
gk_mma_part :: (item:int,rows:int,cols:int,experts:int,slices:int,grouped:int,packed:[]u8,scales:[]u8,counts:[]int,hi:[]u32,lo:[]u32,partial:[]f32,marks:[]u8) #export_c {
    slice:=item%slices;rest:=item/slices;blocks:=rows/16;block:=rest%blocks;quad:=rest/blocks;
    q:=gk_mma_group(quad,experts,counts);
    if q<0 {return;}
    expert:=q/65536;group:=q%65536;r0:=block*16;g0:=8*counts[experts+expert]+32*group;valid:=counts[expert]-32*group;
    nb:=cols/32;half:=cols/2;per:=cols/16/slices;used:=(valid+7)/8;
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
            for lane:0..<32 {for j:0..<4 {fc[lane*4+j]=acc[lane*16+tile*4+j];}}
            if tile<used {
                for lane:0..<32 {
                    xb:=(g0+tile*8+lane/4)*half+c0/2+lane%4;
                    fb[lane*2]=hi[xb];fb[lane*2+1]=hi[xb+4];fl[lane*2]=lo[xb];fl[lane*2+1]=lo[xb+4];
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
                row:=r0+lane/4+8*(j/2);k:=tile*8+2*(lane%4)+j%2;
                v:=acc[lane*16+tile*4+j];
                if k<valid {at:=(slice*grouped+g0+k)*rows+row;partial[at]=v;marks[at]=bad[lane*1];}
            }
        }
    }
}
```

The finish adds the slices in f64 in slice order, adds the bias and stores
each pair's output where `gk_mx_finish` would have stored it, so activation,
the second projection and the mixture read the same buffers as before:

<!-- source: psionic/lib/gptoss/device-mma.coax mma-finish -->
```jai
// Item g*rows+row: the slices added in f64 in slice order, rounded to f32,
// plus the expert's BF16 bias, stored at pair map[...]'s output row, as
// gk_mx_finish stores it. A marked slice flags the output.
gk_mma_finish :: (item:int,rows:int,tokens:int,experts:int,slices:int,grouped:int,map:[]int,counts:[]int,partial:[]f32,marks:[]u8,bias:[]u16,out:[]f32,flags:[]u8) #export_c {
    row:=item%rows;g:=item/rows;
    e:=gk_expert_tile(g/8,experts,counts);o:=g-8*counts[experts+e];
    if o>=counts[e] {return;}
    i:=map[e*tokens+o];sum:f64=0;bad:u8=0;
    for s:0..<slices {at:=(s*grouped+g)*rows+row;sum+=f64(partial[at]);bad|=marks[at];}
    value:=f32(sum)+gk_bf(bias[e*rows+row]);out[i*rows+row]=value;flags[i*rows+row]=gk_bad(value)|bad;
}
```

The prefill planner emits these three kernels in place of
`gk_batch_mx_part` and `gk_mx_finish` only when the session turned the
profile on. `gd_with_mma` admits the number of slices against the plan: both
projections must be whole 16 × 32 fragments, the slices must divide each
projection's 16-column steps, and the plan's partial buffer must hold every
slice. The limbs and marks have their own plan buffers, 65 MB in all
(chapter 65's workspace grows from 527,933,440 to 592,814,080 bytes).

<!-- source: psionic/lib/gptoss/device-prefill.coax batch-mma -->
```jai
// Chapter 71: the same grouped products on the tensor cores (kinds 45 to
// 47), when the session turned MXFP4-BF16-MMA-v1 on. The packed weights are
// read as bytes; grouped rows, limbs and partials live in plan buffers 47,
// 48, 40 and 49.
gd_batch_mma :: (w:GDWriter,rows:int,cols:int,n:int,e:int,z:int,slot_input:int,weight:GDRef,scale:GDRef,bias:GDRef,x:GDRef,out:GDRef)->GDWriter {
    p:=w.plan;s:=GShape.{experts=e,selected=z};grouped:=gd_grouped(n,s);quads:=(n*z+31)/32+e;
    bytes:=GDRef.{buffer=weight.buffer,offset=weight.offset,count=weight.count*weight.elem,elem=1};
    r:=gd_emit(w,45,grouped*(cols/2),5,5,GDScalars.{a=cols,b=n,c=e,d=z,e=slot_input},GDViews.{a=gd_all(p,38),b=gd_all(p,39),c=x,d=gd_all(p,47),e=gd_all(p,48)});
    r=gd_emit(r,46,quads*(rows/16)*p.mma,5,7,GDScalars.{a=rows,b=cols,c=e,d=p.mma,e=grouped},GDViews.{a=bytes,b=scale,c=gd_all(p,39),d=gd_all(p,47),e=gd_all(p,48),f=gd_all(p,40),g=gd_all(p,49)});
    r=gd_emit(r,47,grouped*rows,5,7,GDScalars.{a=rows,b=n,c=e,d=p.mma,e=grouped},GDViews.{a=gd_all(p,38),b=gd_all(p,39),c=gd_all(p,40),d=gd_all(p,49),e=bias,f=out,g=gd_all(p,37)});
    return gd_guard(r,n*z*rows);
}
```

Decode does not change. With one token per expert the tensor cores do not
help (chapter 69), and decode already passes. Turning the profile on is a
session call, `pg_mma`, which compiles a second module from `coax cuda
--release` of `device-mma.coax`; every session that does not call it runs
exactly the f32 program and module as before.

## Checks

The grouped kernels are checked on the CPU, where Coax runs the warp kernel's
lanes one after another and the tensor-core instruction by its definition:

- Integer coefficients and inputs give exact sums at every pair's output row.
- Four experts of 32 × 64, 37 tokens, two selected, one expert with 37 pairs
  (two warp groups) and two ending exactly on a tile, 1 and 2 slices, both
  projections' input kinds: every output is within chapter 69's frozen bound
  of the binary64 reference and within chapter 66's rule of chapter 67's f32
  kernels.
- A scale above 252 flags exactly the outputs of the lane that read it;
  a scale of 252 flags nothing.
- The planner emits the three kernels four times for a two-layer model, with
  the item counts worked by hand, and four fewer operations when the profile
  is off; `gd_with_mma` refuses slices that do not divide or do not fit.

On the tiny owned model (chapter 66's fixture), a CPU session with two slices
prefills four tokens and its logits, trace and mixture weights stay within
0.0005 of the reference with identical routes; turning the profile off again
gives back chapter 67's exact cache bytes.

## On the RTX 4080

```shell
COAX_CC=gcc psionic/tests/check_throughput_gate.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee \
  5 447e9a1f919e90b3 7c6705e4ab014dfd 4
```

The two hexadecimal words are the f32 path's greedy outputs on the fixture
prompts (24 and 256 IDs), recorded in a first run and identical in every run
since. The last argument turns on four slices.

| Receipt | Prefill mean (sd) | Prefill verdict | Decode mean (sd) | Decode rate | Decode verdict | Gate |
| --- | ---: | --- | ---: | ---: | --- | --- |
| f32 prefill (gpt-oss-cuda-f32-v1) | 3.107499 s (3.3 ms) | fail | 2.276840 s (2.6 ms) | 56.22 tokens/s | pass | **fail** |
| tensor-core prefill (MXFP4-BF16-MMA-v1, 4 slices) | 1.510536 s (2.2 ms) | pass | 2.277436 s (3.3 ms) | 56.20 tokens/s | pass | **pass** |

The tensor-core prefill is 2.06 times faster, 0.49 s inside the limit, and
its greedy outputs on both fixture prompts are identical to the f32 path's.
Decode is the same program in both receipts. A kernel profile of one
tensor-core prefill gives `gk_mma_part` 0.513 s, `gk_mma_finish` 0.043 s and
`gk_mma_split` 0.007 s, against 2.047 s and 0.110 s for the f32 kernels they
replace: the product is four times faster at about 16 pairs per expert,
against six times at 32 in chapter 69.

The MMA module is emitted with `coax cuda --release` (wrapping arithmetic;
bounds traps stay). Emitted without it, its checked arithmetic in the inner
loop made prefill slower than the f32 path, 4.83 s, so that build is not
used.

The final receipt's runs:

| Run | Prefill ns | Decode ns | Load average |
| --- | ---: | ---: | ---: |
| warm-up | 1,507,942,394 | 2,273,047,714 | 14.99 |
| 0 | 1,509,731,025 | 2,273,655,612 | 14.99 |
| 1 | 1,508,299,997 | 2,274,044,147 | 14.99 |
| 2 | 1,509,091,290 | 2,279,963,061 | 15.07 |
| 3 | 1,513,465,083 | 2,279,978,867 | 15.54 |
| 4 | 1,512,092,657 | 2,279,540,283 | 16.22 |

The host is shared: the one-minute load average was between 14 and 42 while
these runs were taken, and is recorded with every run. The spread of five
runs stayed under 0.4% of the mean. The
[record](../../docs/verification/psionic-ch071-throughput-gate-2026-11-01.md)
has every run, the build words and the kernel profiles.

## What is not built yet

- **Tail and memory reports.** The gate decides from the mean of five runs;
  it does not yet report P95, peak device memory per phase or host-device
  transfers per run. The harness records device bytes in use once.
- **A packaged reference engine** on the same device and file. None is
  approved and installed.
- **Continuous batching.** Bounded continuous batching, KV allocation per
  request, and aggregate and per-request rates at concurrency 1, 4 and 8
  move to [chapter 175](ch175-continuous-batching-and-a-fair-scheduler.md).
  The gate refuses any concurrency but 1.
- **Serving.** Chapter 79's Responses server does not call `pg_mma` yet.
- **Other devices and clocks.** One RTX 4080; clocks are not pinned.

## Sources

- [Chapter plan](PLAN.md), GPT-OSS device delivery order, step 8.
- [Decode qualification](../../docs/verification/psionic-decode-speed-2026-10-29.md).
- [Chapter 69's record](../../docs/verification/psionic-ch069-mxfp4-mma-2026-11-01.md).
- [Making Psionic serving faster](../docs/serving/performance.md).
