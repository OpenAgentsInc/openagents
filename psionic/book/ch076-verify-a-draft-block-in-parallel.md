# 76. Verify a draft block in parallel

> **Status: partly implemented.** Greedy verification of a prompt n-gram
> draft block runs on the GPU in one batched prefill per block, with
> rollback of only the speculative cache rows, exact token parity and
> byte-equal caches. It is checked on the CPU with the tiny owned model and
> measured on the original gpt-oss-20b on one RTX 4080. There, speculation is
> 1.84 times slower than the target alone, so the rule leaves it off. Sampled
> (nonzero temperature) verification on the device, a cheaper verification
> pass and a better drafter are not built; the last section lists them.

[Chapter 75](ch075-let-the-small-model-guess.md) verifies draft tokens one
at a time on the CPU, and records that this serial path is slower than the
target alone. Speculation can only pay when the target checks a whole block
of drafts in one pass. This chapter does that on the device. It reuses
[chapter 67](ch067-prefill-together.md)'s batched prefill: verifying k drafts
is a (k + 1)-token prefill on the existing cache. It then measures the result
against [chapter 66](ch066-a-complete-cuda-decoder.md)'s decoder under
[chapter 34](ch034-benchmark-against-llama-cpp-and-cublas.md)'s statistics
and lets an `#authority` rule decide whether speculation is turned on.

The code is in [`device-verify.coax`](../lib/gptoss/device-verify.coax)
(drafts, commitment, the copy and readout programs) and
[`speculation-gate.coax`](../lib/speculation-gate.coax) (the rule). Both are
`#authority` files: the first decides which IDs are committed and what the
cache holds afterwards, the second decides whether speculation runs at all.
The harness is [`gptoss_verify.c`](../tests/gptoss_verify.c), run by
[`check_device_verify.sh`](../tests/check_device_verify.sh).

## One block, by hand

The drafter is chapter 75's prompt n-gram, which needs no second model. The
committed history is `5 6 7 8 9 5 6 7`. Its longest suffix that occurs
earlier, `5 6 7`, starts the history, and was followed by 8. Append 8 and
look again: the suffix `6 7 8` was followed by 9, then `7 8 9` by 5. With
k = 3 the drafts are `8 9 5`.

The last committed ID, 7, has not been fed to the model yet; it is the
pending token. The block is the pending token and the drafts, `7 8 9 5`, fed
at positions 6, 7, 8 and 9 in one prefill. Row i's logits give the target's
greedy choice after row i:

```text
row   fed   target picks   draft for the next position   match
0     7     8              8                             yes
1     8     9              9                             yes
2     9     4              5                             no: stop reading rows
3     5     (not read)
```

Two drafts are accepted, and the target's pick at the first mismatch, 4, is
the correction. The block commits `8 9 4`. Had all three drafts matched, row
3's pick would have been a fourth, bonus ID. Rows 0, 1 and 2 (`7 8 9`) are
now correctly in the cache; row 3 (`5`, at position 9) is speculative and has
to go, and 4 becomes the pending token at position 9.

Removing row 3 is not just moving the position back. Even layers keep a ring
of `window` slots. With a window of 4, position 9 sits in slot 9 mod 4 = 1,
where position 5's keys and values were. Here the next ID goes to position 9
and would overwrite that slot anyway. But had the second draft also been
wrong, rows 2 and 3 would both be rejected, the next ID would go to position
8, and its window (positions 5 to 8) would read a slot that row 3 had
overwritten. So the slots a block will write are saved before it runs and the
ones past the kept rows are put back after; the cache is then byte for byte
what it would be had the block never been drafted. For the full layer of 16
slots, rows 6 to 9 are slots 6 to 9; for the ring they are slots 2, 3, then 0
and 1, which wrap. That is three copies to save (one for the ring before the
wrap, one after, one for the full layer) and two to put back (ring slot 1 and
full slot 9).

## Admission

<!-- source: psionic/lib/gptoss/device-verify.coax admit -->
```jai
// A block of k drafts feeds k + 1 rows in one prefill chunk. Every row must
// still be in the sliding ring afterwards, so k + 1 may not exceed the window:
// otherwise an accepted row's keys would already be overwritten by a later,
// rejected row of the same chunk.
gv_admit :: (s:GShape,p:GDPlan,k:int)->GStatus #export_c {
    if p.status!=GStatus.ok || !g_shape(s) {return GStatus.shape;}
    if k<1 || k>GV_DRAFTS || k+1>s.window || k+1>p.batch {return GStatus.shape;}
    return GStatus.ok;
}
// The undo store: one slot of keys and values per layer per row, in upload
// slot 1. A refusal keeps the caller's plan unusable rather than half set.
gv_with_undo :: (p:GDPlan,s:GShape,k:int)->GDPlan #export_c {
    if p.status!=GStatus.ok {return GDPlan.{status=p.status};}
    st:=gv_admit(s,p,k);if st!=GStatus.ok {return GDPlan.{status=st};}
    r:=p;r.upload[1]=s.layers*(k+1)*2*s.kv_heads*64*4;return r;
}
```

For gpt-oss-20b (window 128) any k from 1 to 16 is admitted; the undo store
for k = 4 is 24 layers × 5 rows × 1,024 floats × 4 bytes = 491,520 bytes. The
tiny owned model has a window of 2, so it admits only k = 1.

## Draft, commit, put back

The block draft repeats chapter 75's `gx_ngram` on a scratch copy of the
history that grows by each guess:

<!-- source: psionic/lib/gptoss/device-verify.coax draft -->
```jai
// Chapter 75's prompt n-gram draft, extended to a block: guess one ID, append
// it to a scratch copy of the history, guess again. It stops at k IDs or when
// no suffix matches. Returns the number drafted, or -1 when refused.
gv_draft :: (history:[]int,width:int,k:int,scratch:[]int,out:[]int)->int #export_c {
    if !gx_history_size(history.count,width) || k<1 || k>GV_DRAFTS || out.count<k || scratch.count<history.count+k {return -1;}
    for i:0..<history.count {scratch[i]=history[i];}
    n:=0;while n<k {next:=gx_ngram(scratch[0..<history.count+n],width);if next<0 {break;}scratch[history.count+n]=next;out[n]=next;n+=1;}
    return n;
}
// Drafts that fit: the block's k + 1 rows must end inside the context.
gv_room :: (s:GShape,position:int,k:int)->int #export_c {
    r:=0;while r<k && position+r+1<s.capacity {r+=1;}return r;
}
```

When nothing matches, there is no block and the harness takes one ordinary
decode step, so a prompt with no repetition pays nothing for speculation.

Commitment follows gk_choose exactly, one ID at a time, including its stop
IDs (200002, 200012) and the generation limit, so a block that crosses a stop
commits no more than the target alone would have:

<!-- source: psionic/lib/gptoss/device-verify.coax commit -->
```jai
// Rows were read 0..rows-1: row i's pick is the target's greedy ID after the
// pending token and drafts 0..i-1. Drafts 0..a-1 matched; the committed IDs
// are those drafts and then pick a (a correction, or the bonus when all k
// matched). Commitment stops at the first stop ID or the limit, exactly as
// gk_choose decides one ID at a time. `count` IDs were committed before.
gv_commit :: (vocab:int,draft:[]int,picks:[]int,rows:int,count:int,limit:int,out:[]int)->GVBlock #export_c {
    k:=draft.count;if k<1 || k>GV_DRAFTS || vocab<1 || count<1 || count>=limit {return GVBlock.{status=GStatus.shape};}
    if picks.count<rows || out.count<k+1 {return GVBlock.{status=GStatus.capacity};}
    for d:draft {if d<0 || d>=vocab {return GVBlock.{status=GStatus.token};}}
    a:=0;while a<k && a<rows && draft[a]==picks[a] {a+=1;}
    if rows!=a+1 {return GVBlock.{status=GStatus.state};}
    for i:0..<rows {if picks[i]<0 || picks[i]>=vocab {return GVBlock.{status=GStatus.token};}}
    b:=GVBlock.{accepted=a};total:=count;
    for i:0..<a+1 {
        id:=ifx i<a then draft[i] else picks[a];
        out[i]=id;b.written+=1;total+=1;b.token=id;
        if id==200002 {b.stop=1;}else if id==200012 {b.stop=2;}else if total>=limit {b.stop=3;}
        if b.stop!=0 {break;}
    }
    b.kept=b.written;return b;
}
// More rows to read: row `row` matched its draft and a later row exists.
gv_more :: (draft:[]int,picks:[]int,row:int)->bool #export_c {return row>=0 && row<draft.count && draft[row]==picks[row];}
```

The kept rows are the pending token plus every committed ID except the last,
which becomes the new pending token, so `kept` equals `written`.

The save and restore programs are lists of chapter 66's `gk_copy` launches
between the cache (plan buffer 16) and the undo store, built by Coax and
executed by the host:

<!-- source: psionic/lib/gptoss/device-verify.coax undo -->
```jai
// Copy rows of keys and values between the cache (plan buffer 16) and the
// undo store (upload slot 1) with gk_copy (kind 39). Row i of the block sits
// at cache slot (position + i) mod capacity of each layer; the rows wrap at
// most once, so each layer needs at most two copies. `back` copies undo to
// cache for rows first..<last; otherwise cache to undo.
gv_copies :: (s:GShape,p:GDPlan,position:int,first:int,last:int,back:bool,ops:[]GDOp,args:[]GDArg)->GDProgram {
    u:=gv_rows(p,s);
    if p.status!=GStatus.ok || u<2 {return GDProgram.{status=GStatus.shape};}
    if position<0 || first<0 || first>last || last>u || position+last>s.capacity {return GDProgram.{status=GStatus.state};}
    w:=GDWriter.{plan=p,ops=ops,args=args};row:=2*s.kv_heads*64;
    for layer:0..<s.layers {
        cap:=g_layer_capacity(s,layer);i:=first;
        while i<last {
            slot:=(position+i)%cap;n:=1;while i+n<last && slot+n<cap {n+=1;}
            cache:=gd_slice(p,16,g_cache_base(s,layer)+slot*row,n*row);undo:=GDRef.{buffer=1,offset=(layer*u+i)*row*4,count=n*row,elem=4};
            if back {w=gd_emit(w,39,n*row,0,2,GDScalars.{},GDViews.{a=undo,b=cache});}else {w=gd_emit(w,39,n*row,0,2,GDScalars.{},GDViews.{a=cache,b=undo});}
            i+=n;
        }
    }
    return w.result;
}
// Before the block: save the slots its rows will overwrite.
gv_save :: (s:GShape,p:GDPlan,position:int,rows:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    if rows<1 {return GDProgram.{status=GStatus.state};}return gv_copies(s,p,position,0,rows,false,ops,args);
}
// After it: put back only the rows past the kept ones. Committed rows stay.
gv_restore :: (s:GShape,p:GDPlan,position:int,rows:int,kept:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    if kept<1 || kept>rows {return GDProgram.{status=GStatus.state};}return gv_copies(s,p,position,kept,rows,true,ops,args);
}
```

Chapter 75 copied the whole cache, 107 MB for gpt-oss-20b, to roll back. This
copies at most 24 × 17 rows of 4 KB.

## Reading every row

Chapter 67's prefill reads out only its last row. After a verification
prefill (with no choice), the final residual of every row is still in plan
buffer 28, so one short program per row reads it out:

<!-- source: psionic/lib/gptoss/device-verify.coax readout -->
```jai
// Row `row` of the last prefill chunk: final norm, unembedding, the guarded
// flags and the greedy choice into control[1], the same launches as the tail
// of gd_batch_program for its last row, without publishing or advancing.
gv_readout :: (s:GShape,p:GDPlan,params:[]GDParam,row:int,rows:int,limit:int,ops:[]GDOp,args:[]GDArg)->GDProgram #export_c {
    if p.status!=GStatus.ok || !g_shape(s) {return GDProgram.{status=GStatus.shape};}
    if rows<1 || rows>p.batch || row<0 || row>=rows || limit<1 || limit>s.capacity {return GDProgram.{status=GStatus.state};}
    if params.count!=3+15*s.layers || ops.count<p.max_ops || args.count<p.max_args {return GDProgram.{status=GStatus.capacity};}
    w:=GDWriter.{plan=p,ops=ops,args=args};d:=s.width;control:=gd_all(p,24);
    w=gd_emit(w,1,p.flags,0,1,GDScalars.{},GDViews.{a=gd_all(p,22)});
    w=gd_norm(w,d,gd_weight(params,1,2),gd_slice(p,28,row*d,d),gd_all(p,1));
    w=gd_bf(w,s.vocab,d,gd_weight(params,2,8),GDRef.{elem=2},gd_all(p,1),gd_all(p,17));
    summaries:=(w.result.flags+255)/256;
    w=gd_emit(w,18,summaries,0,2,GDScalars.{},GDViews.{a=gd_slice(p,22,0,w.result.flags),b=gd_slice(p,23,0,summaries)});
    w=gd_emit(w,19,1,0,2,GDScalars.{},GDViews.{a=gd_slice(p,23,0,summaries),b=control});
    w=gd_emit(w,22,p.counts[27],0,2,GDScalars.{},GDViews.{a=gd_all(p,17),b=gd_all(p,27)});
    return gd_emit(w,23,1,1,3,GDScalars.{a=limit},GDViews.{a=gd_all(p,17),b=gd_all(p,27),c=control}).result;
}
```

A check compares the last row's readout with `gd_batch_program`'s own tail,
launch by launch and argument by argument. Rows are read in order and the
harness stops at the first mismatch (`gv_more`), so a block whose first
draft is wrong reads one row.

After the block, Coax sets the control words to what a target-only run would
hold after committing the same IDs; the host uploads them:

<!-- source: psionic/lib/gptoss/device-verify.coax control -->
```jai
// After a block: the device's control words say what a target-only run
// would say after committing the same IDs. Refused while control reports a
// failure, which then stays visible.
gv_control :: (control:[]int,position:int,b:GVBlock,count:int)->GStatus #export_c {
    if control.count<8 || b.status!=GStatus.ok || b.written<1 || position<0 || count<1 {return GStatus.shape;}
    if control[0]!=0 {return GStatus.state;}
    control[1]=b.token;control[2]=position+b.kept;control[3]=count+b.written;control[4]=b.stop;return GStatus.ok;
}
```

The platform gains three effects in
[`gptoss-device.c`](../platform/gptoss-device.c): `pg_run` executes a program
Coax built, `pg_adopt` allocates the upload slot of a plan Coax derived, and
`pg_write` uploads words Coax decided.

## The rule

The threshold was frozen before any measurement:

<!-- source: psionic/lib/speculation-gate.coax threshold -->
```jai
// Frozen before any measurement: speculation must save at least 5% of the
// target-only time, beyond two standard errors of the difference.
SG_GAIN_PERCENT :: 5;
```

<!-- source: psionic/lib/speculation-gate.coax decide -->
```jai
// Refusals first: changed outputs, unpaired runs, refused timings. A spread
// over a tenth of either mean decides nothing (chapter 34's rule). Then on
// when saving - 2 se >= 5% of the base mean, off when saving + 2 se < 5% of
// it, and too close to call between the two.
sg_decide :: (same_outputs:bool,base:[]int,spec:[]int)->SgDecision #export_c {
    d:=SgDecision.{verdict=SgVerdict.refused,reason=SgReason.outputs};
    if !same_outputs {return d;}
    d.reason=SgReason.pairs;if base.count!=spec.count {return d;}
    d.base=cm_welford(base);d.spec=cm_welford(spec);d.reason=SgReason.stats;
    if d.base.status!=CmStatus.ok || d.spec.status!=CmStatus.ok {return d;}
    n:=cast(f64)d.base.n;d.se=sqrt((d.base.sd*d.base.sd+d.spec.sd*d.spec.sd)/n);
    saving:=d.base.mean-d.spec.mean;d.gain=saving/d.base.mean;
    d.verdict=SgVerdict.noisy;d.reason=SgReason.spread;
    if d.base.sd*10>d.base.mean || d.spec.sd*10>d.spec.mean {return d;}
    need:=d.base.mean*cast(f64)SG_GAIN_PERCENT/100;d.reason=SgReason.none;
    if saving-2*d.se>=need {d.verdict=SgVerdict.on;return d;}
    if saving+2*d.se<need {d.verdict=SgVerdict.off;return d;}
    d.reason=SgReason.close;return d;
}
// Only an "on" verdict turns speculation on.
sg_enabled :: (d:SgDecision)->bool #export_c {return d.verdict==SgVerdict.on;}
```

By hand: target-only runs of 100, 102, 98, 101 and 99 ns have mean 100 and
variance (0 + 4 + 4 + 1 + 1) / 4 = 2.5; speculative runs of 90, 92, 88, 91
and 89 have mean 90 and the same variance. The standard error of the
difference is sqrt((2.5 + 2.5) / 5) = 1. The saving is 10 and the threshold 5:
10 − 2 = 8 ≥ 5, so speculation is on. Shift the speculative runs up by 4
(saving 6): 6 − 2 = 4 is under 5 but 6 + 2 = 8 is not, so the rule says too
close to call, and speculation stays off.

## Run it on the CPU

The [example](../examples/device-verify.coax) works the block above, rolls
it back on host arrays the way the copy programs run, and applies the rule
to the RTX 4080 receipt:

```shell
coax/bin/coax run psionic/examples/device-verify.coax
```

```text
drafts after 5 6 7 8 9 5 6 7: 8 9 5 (3)
picks 8 9 4: accepted 2, committed 8 9 4, keep 3 of 4 rows, next pending ID 4
position 6, 4 rows: 3 copies saved, 2 put back; cache equals never having drafted: true
RTX 4080, k=4: target-only mean 4213698938.6 ns, speculative mean 7733227629.8 ns (280 IDs a run)
tokens/s: target-only 66.44993011603954, speculative 36.207391454639165
acceptance: 90 of 360 drafts; saving -0.8352586984701291 of the target-only time, se 6328556.535801797 ns
speculation: off (none); enabled: false
```

<!-- source: psionic/examples/device-verify.coax example -->
```jai
main :: () {
    // Draft: the history ends 5 6 7, which also starts it.
    history:=[8]int.[5,6,7,8,9,5,6,7];scratch:[12]int;draft:[3]int;
    n:=gv_draft(history[..],3,3,scratch[..],draft[..]);
    print("drafts after 5 6 7 8 9 5 6 7: % % % (%)\n",draft[0],draft[1],draft[2],n);
    // Verify: rows 0, 1 and 2 pick 8, 9, 4. The third draft (5) is wrong.
    picks:=[3]int.[8,9,4];out:[4]int;
    b:=gv_commit(201088,draft[..],picks[..],3,10,256,out[..]);
    print("picks 8 9 4: accepted %, committed % % %, keep % of 4 rows, next pending ID %\n",b.accepted,out[0],out[1],out[2],b.kept,b.token);
    // Roll back on host arrays: a ring of 4 slots and a full layer of 16.
    s:=GT_SHAPE;s.window=4;s.capacity=16;pins:=GOWN_ARTIFACTS;p:=gv_with_undo(gd_plan(s,pins[..],1000000000,1000000000),s,3);
    spec:[2560]f32;never:[2560]f32;undo:[1024]f32;ops:[192]GDOp;args:[4608]GDArg;
    before:=[6]int.[1,2,3,4,5,6];block:=[4]int.[7,8,9,5];
    gv_write_rows(s,spec[..],0,before[..]);gv_write_rows(s,never[..],0,before[..]);
    saved:=gv_save(s,p,6,4,ops[..],args[..]);gv_run_copies(ops[..],args[..],saved.ops,spec[..],undo[..]);
    gv_write_rows(s,spec[..],6,block[..]);
    put:=gv_restore(s,p,6,4,b.kept,ops[..],args[..]);gv_run_copies(ops[..],args[..],put.ops,spec[..],undo[..]);
    gv_write_rows(s,never[..],6,block[0..<b.kept]);
    same:=true;for i:0..<2560 {if spec[i]!=never[i] {same=false;}}
    print("position 6, 4 rows: % copies saved, % put back; cache equals never having drafted: %\n",saved.ops,put.ops,same);
    // The measured rule on the RTX 4080.
    base:=SG_R1_BASE;spec_runs:=SG_R1_SPEC;d:=sg_decide(SG_R1_SAME,base[..],spec_runs[..]);
    print("RTX 4080, k=%: target-only mean % ns, speculative mean % ns (% IDs a run)\n",SG_R1_K,d.base.mean,d.spec.mean,SG_R1_IDS);
    print("tokens/s: target-only %, speculative %\n",cast(f64)SG_R1_IDS*1000000000/d.base.mean,cast(f64)SG_R1_IDS*1000000000/d.spec.mean);
    print("acceptance: % of % drafts; saving % of the target-only time, se % ns\n",SG_R1_ACCEPTED,SG_R1_DRAFTED,d.gain,d.se);
    print("speculation: % (%); enabled: %\n",sg_verdict_name(d.verdict),sg_reason_name(d.reason),sg_enabled(d));
}
```

## Refusals

The [refusal example](../examples/device-verify-refuse.coax) breaks one
thing at a time:

```text
20b, k=4: ok
20b, k=17 drafts: shape
tiny model, k=2 with a window of 2: shape
rows 7 8 4 against drafts 7 8 9: ok
a third row read after the second mismatched: state
a draft ID outside the vocabulary: token
a pick outside the vocabulary: token
restore keeping no rows: state
save past the end of the context: state
control words after a flagged readout: state
measured 10% faster: on (none)
speculative outputs changed: refused (speculative outputs differ from the target-only outputs)
five target-only runs, four speculative: refused (run counts differ)
four runs each: refused (the timings were refused)
a speculative run of 0 ns: refused (the timings were refused)
```

## Checks

- Commitment agrees with chapter 75's `gx_block` in greedy mode on one-hot
  target rows, for k from 1 to 5 and 40 seeds each (an independent
  implementation of the same rule).
- On host arrays, save, write a whole block, restore: the cache equals a
  cache that only received the kept rows, byte for byte, for every position
  0 to 11, k from 1 to 3 and every kept count, on a ring of 4 and a full
  layer of 16. Without the restore, the ring differs whenever a rejected row
  wrapped onto a live slot.
- The rule agrees with chapter 34's exact integer variance at 12 shifts of
  randomly drawn runs.
- On the tiny owned model, executed by the device programs compiled to C
  (plain and under ASan/UBSan), three prompts with three drafters each: the
  n-gram drafter, an oracle that drafts the target-only answer (every draft
  accepted) and one that drafts it plus one (every draft rejected). All nine
  generations equal the target-only IDs, and after every rollback the whole
  cache equals the target-only run's cache at the same position, byte for
  byte (13 rollbacks). With the restore removed from the harness, the first
  comparison fails.
- `coax mutate` leaves no survivor in either file.

## On the RTX 4080

```shell
COAX_CC=gcc psionic/tests/check_device_verify.sh \
  ~/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee 5 4
```

The harness generates the two fixture prompts of
[chapter 71](ch071-throughput-is-a-gate.md) ("Say hi." and "List the
integers from 1 to 100, separated by commas.", greedy up to 256 IDs) with the
target alone and with speculation (prompt n-gram width 3, k = 4). Prefill
expert products are the f32 ones, so verification rows and decode steps are
the same arithmetic: chapter 67 showed the batched and serial caches are
bitwise equal on the original model.

- **Parity.** Speculative and target-only IDs are equal on both prompts, and
  the target-only output words are chapter 71's reference words
  (`447e9a1f919e90b3`, `7c6705e4ab014dfd`).
- **Rollback.** After the first six rollbacks past position 128, where the
  ring has wrapped, the whole 107 MB cache equals the target-only cache at
  the same position, byte for byte.
- **Acceptance.** Per run, 90 blocks propose 360 drafts and accept 90 (25%);
  98 positions have no n-gram match and take a plain decode step. The
  "list" prompt accounts for 87 of the blocks.

| Path | Mean per run (sd) | Tokens/s |
| --- | ---: | ---: |
| target only | 4.213699 s (5.2 ms) | 66.45 |
| speculative, k = 4 | 7.733228 s (13.2 ms) | 36.21 |

The saving is −83.5% of the target-only time (standard error of the
difference 6.3 ms), so the rule says **off**. Arithmetic on the means (not a
separate measurement): the target alone spends 15.2 ms per decode step (278
steps a run); the 98 plain steps account for about 1.49 s of the speculative
run, leaving about 69 ms for each block, and a block commits two IDs on
average. A block would have to cost under two decode steps to break
even at this acceptance, and the five-row f32 prefill with its readouts
costs more than four.

The [record](../../docs/verification/psionic-ch076-device-verify-2026-11-02.md)
has every run, the build words and the check transcripts.

## What is not built yet

- **Sampled verification on the device.** Only greedy blocks run on the GPU.
  Chapter 75's `gx_block` decides sampled blocks on the CPU from full
  probability rows; the device does not yet produce those rows per block row,
  so the tiny sampled-law checks run only on chapter 75's CPU path.
- **A cheaper verification pass.** A five-row prefill costs more than four
  decode steps here. The batched path's fixed costs (chapter 67's whole-cache
  shadow copy, per-row readouts each reading the unembedding, a host read per
  row) have not been profiled or reduced, and chapter 71's tensor-core expert
  products are not used, because they would break bitwise parity with decode.
- **A better drafter.** Prompt n-grams accept 25% of drafts on these
  prompts. A small draft model (chapter 75's 20b-drafts-for-120b pairing) on
  the device is not built.

Until a measurement passes the rule, speculation stays off.
