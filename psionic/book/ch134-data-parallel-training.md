# 134. Data-parallel training

> **Status: partly implemented.** Chapter 118's trainer runs data parallel
> for any model it drives: the ranks split a step's micro-batches, report
> at their accumulation boundary, and all-reduce the gradient bucket by
> bucket in a fixed order that gives chapter 118's sum bit for bit at every
> rank count and bucket size. Chapter 103's two-layer model and chapter
> 115's decoder are checked that way. A killed rank stops the step without
> writing anything, and the run restarts from its last checkpoint on fewer
> ranks with the uninterrupted bits. Overlapping the all-reduce with
> backward is a worked timing model. Measured throughput with and without
> overlap, which needs a backward pass that hands out each bucket as it
> finishes it and ranks on real threads or devices, remains.

In data-parallel training each rank holds a full copy of the model and
trains on different examples. The ranks combine their gradients before
every update, so each one makes the same update to its own copy.

[Chapter 132](ch132-train-across-devices.md) already does this for one
model: chapter 115's decoder, split by rows, with chapter 129's device
kernels. It owns the hard cases of a device group: a chain all-reduce
that keeps the single-device bits, a tree all-reduce, deadlines, resume
at another device count, a communication cost model, and measured runs on
host threads and the GPU. This chapter does not repeat those. It adds
four things:

- **Any model.** The split is over chapter 118's micro-batches, and the
  result is chapter 118's `tr_add` sum, so the step is chapter 118's
  `tr_apply`, unchanged. Each model keeps its own gradient code.
- **Buckets.** The gradient is cut into buckets of a fixed number of
  values, and each bucket is all-reduced on its own.
- **Accumulation boundaries.** A rank sends nothing until it has finished
  every micro-batch it owns in the step.
- **Overlap.** Backward finishes the last bucket first. A bucket's
  all-reduce can start while backward is still producing the buckets
  before it. A timing model says when that helps and when it does not.

The decisions are in Coax, in
[`data-parallel.coax`](../lib/data-parallel.coax) (`#authority`): who owns
which micro-batches, how the gradient is cut, whether every rank's report
is admitted, the order of every addition, and the overlap timeline.

## Who owns what

A step has `micro` micro-batches (chapter 118's `accumulate`). Rank `r`
owns the micro-batches from `r * micro / ranks` up to, but not including,
`(r + 1) * micro / ranks`. Every rank owns at least one. The gradient's
`n` values are cut into buckets of `bucket` values; the last bucket is
shorter when `bucket` does not divide `n`.

<!-- source: psionic/lib/data-parallel.coax plan -->
```jai
// Micro-batches of a step over ranks: rank r owns micro-batches dp_first(r)
// up to dp_first(r+1), a contiguous run, and every rank owns at least one.
// The gradient's n values are cut into buckets of `bucket` values, the last
// one shorter when bucket does not divide n.
DpPlan :: struct {status:DpStatus;ranks:int;micro:int;n:int;bucket:int;}
dp_plan :: (ranks:int,micro:int,n:int,bucket:int)->DpPlan {
    p:=DpPlan.{ranks=ranks,micro=micro,n=n,bucket=bucket};
    if ranks<1 || ranks>DP_RANKS {p.status=DpStatus.ranks;return p;}
    if micro<ranks || micro>TR_ACCUMULATE {p.status=DpStatus.micro;return p;}
    if n<1 || n>CK_VALUES || bucket<1 || bucket>n {p.status=DpStatus.bucket;}
    return p;
}
dp_first :: (p:DpPlan,r:int)->int {return r*p.micro/p.ranks;}
dp_buckets :: (p:DpPlan)->int {return tr_ceil(p.n,p.bucket);}
dp_span :: (p:DpPlan,k:int)->TrSpan {first:=k*p.bucket;return TrSpan.{first=first,count=tr_min(p.bucket,p.n-first)};}
```

For 4 micro-batches over 3 ranks, `dp_first` gives `0`, `4 / 3 = 1`,
`8 / 3 = 2` and `12 / 3 = 4`: rank 0 owns micro-batch 0, rank 1 owns
micro-batch 1, and rank 2 owns micro-batches 2 and 3. Ten values in
buckets of 4 make `ceil(10 / 4) = 3` buckets: values 0 to 3, 4 to 7, and
8 and 9.

## Reports at the accumulation boundary

A rank that has finished its micro-batches sends a report: who it is, the
step (the run's revision), how many micro-batches it finished, and whether
it failed. A rank never sends a report, or any gradient, in the middle of
its micro-batches. That is the accumulation boundary: with 8 micro-batches
on 2 ranks, each rank runs 4 forward and backward passes and then
communicates once.

The ranks' sum starts only when every rank has reported exactly once, for
this step, with all of its micro-batches done and no failure. Reports can
arrive in any order. A refusal names the lowest rank at fault, so the
answer does not depend on the arrival order either.

<!-- source: psionic/lib/data-parallel.coax gather -->
```jai
// What a rank says at its accumulation boundary: who it is, the step it
// computed, how many micro-batches it finished, and whether it failed.
DpReport :: struct {rank:int;revision:int;done:int;failed:bool;}
DpGather :: struct {status:DpStatus;rank:int=-1;}
// Admits a step's reports, in any arrival order. Every rank must report
// exactly once, for this step, with all its micro-batches done and no
// failure. A refusal names the lowest rank at fault.
dp_gather :: (p:DpPlan,b:TrBatch,reports:[]DpReport)->DpGather {
    if p.status!=DpStatus.ok {return DpGather.{status=p.status};}
    if b.status!=TrStatus.ok {return DpGather.{status=DpStatus.batch};}
    if b.micro!=p.micro {return DpGather.{status=DpStatus.micro};}
    seen:[64]int;
    for x:reports {if x.rank<0 || x.rank>=p.ranks {return DpGather.{status=DpStatus.stale,rank=x.rank};}seen[x.rank]+=1;}
    for r:0..<p.ranks {
        g:=DpGather.{rank=r};
        if seen[r]==0 {g.status=DpStatus.missing;return g;}
        if seen[r]>1 {g.status=DpStatus.twice;return g;}
        for x:reports {if x.rank==r {
            if x.failed {g.status=DpStatus.failed;return g;}
            if x.revision!=b.revision {g.status=DpStatus.stale;return g;}
            if x.done!=dp_first(p,r+1)-dp_first(p,r) {g.status=DpStatus.boundary;return g;}}}
    }
    return DpGather.{};
}
```

A rank that is killed never reports. Here that is a missing report; in a
real group it is a report that has not arrived by a deadline, which is
chapter 132's `dg_gather`.

## The sum, bucket by bucket

Chapter 118's `tr_add` makes a step's gradient by adding each
micro-batch's gradient divided by the micro-batch count, micro-batch 0
first, starting from zero. Binary64 addition is not associative, so any
other order can round differently. The ranks keep that order with a
chain: rank 0 starts at zero and adds its micro-batches; rank 1 receives
that running sum and adds its own; and so on. The last rank holds
`tr_add`'s sum, bit for bit. Chapter 132 uses the same chain over rows.

Each value's chain is independent of every other value's, so cutting the
gradient into buckets changes which values travel together, not any
addition. `dp_sum` runs the buckets last first, the order backward
finishes them, and returns a `TrSum` that goes to `tr_apply` unchanged.
The loss is summed the same way.

<!-- source: psionic/lib/data-parallel.coax reduce -->
```jai
// Rank r's part of bucket k: take the running sum from rank r-1 (rank 0
// starts at zero), add this rank's micro-batch gradients divided by the
// step's micro-batch count, in order, and pass the sum on in rank r's slot
// of msgs. grads holds micro-batch m's gradient at m*n.
dp_chain :: (p:DpPlan,r:int,k:int,grads:[]f64,msgs:[]f64) {
    s:=dp_span(p,k);count:=f64(p.micro);
    for i:s.first..<s.first+s.count {
        acc:=0.0;if r>0 {acc=msgs[(r-1)*p.n+i];}
        for m:dp_first(p,r)..<dp_first(p,r+1) {acc+=grads[m*p.n+i]/count;}
        msgs[r*p.n+i]=acc;
    }
}
DpSum :: struct {status:DpStatus;rank:int=-1;sum:TrSum;}
// The step's sum, once every report is in: each bucket's chain, last bucket
// first (the order backward finishes them), and the loss in micro-batch
// order. The result is chapter 118's tr_add sum, bit for bit, for any rank
// count and bucket size, and goes to tr_apply unchanged. A refusal writes
// nothing and returns a sum tr_apply refuses.
dp_sum :: (p:DpPlan,b:TrBatch,reports:[]DpReport,grads:[]f64,losses:[]f64,msgs:[]f64,total:[]f64)->DpSum {
    g:=dp_gather(p,b,reports);if g.status!=DpStatus.ok {return DpSum.{status=g.status,rank=g.rank,sum=TrSum.{status=TrStatus.order}};}
    if grads.count!=p.micro*p.n || losses.count!=p.micro || msgs.count!=p.ranks*p.n || total.count!=p.n {return DpSum.{status=DpStatus.shape,sum=TrSum.{status=TrStatus.order}};}
    nb:=dp_buckets(p);
    for j:0..<nb {k:=nb-1-j;
        for r:0..<p.ranks {dp_chain(p,r,k,grads,msgs);}
        s:=dp_span(p,k);for i:s.first..<s.first+s.count {total[i]=msgs[(p.ranks-1)*p.n+i];}
    }
    count:=f64(p.micro);loss:=0.0;for m:0..<p.micro {loss+=losses[m]/count;}
    return DpSum.{sum=TrSum.{micro=p.micro,loss=loss}};
}
```

Work one step by hand: 4 micro-batches of a 3-value gradient on 2 ranks,
buckets of 2 values. The micro-batch gradients are `(1, 2, 3)`,
`(0.5, -4, 6)`, `(-1, 8, 0)` and `(3, 10, -2)`, and the losses are 4, 8,
2 and 6. Every term is divided by 4.

- **Value 0.** Rank 0 owns micro-batches 0 and 1: `0 + 0.25 + 0.125 =
  0.375`, and it sends 0.375. Rank 1 continues with micro-batches 2 and 3:
  `0.375 - 0.25 + 0.75 = 0.875`.
- **Value 1.** Rank 0: `0.5 - 1 = -0.5`. Rank 1: `-0.5 + 2 + 2.5 = 4`.
- **Value 2.** Rank 0: `0.75 + 1.5 = 2.25`. Rank 1: `2.25 + 0 - 0.5 = 1.75`.
- **Loss.** `1 + 2 + 0.5 + 1.5 = 5`.

Bucket 1 (value 2) is reduced first, then bucket 0 (values 0 and 1). The
sum is `(0.875, 4, 1.75)` with loss 5, which is what `tr_add` gives for the
same four micro-batches.

Rounding is where a different order would show. Take one value whose four
micro-batch gradients are `1e16`, `1`, `-1e16` and `1`. Divided by 4 they
are `2.5e15`, `0.25`, `-2.5e15` and `0.25`. Near `2.5e15` neighbouring
binary64 values are 0.5 apart, so `2.5e15 + 0.25` is a tie and rounds to
the even neighbour, `2.5e15`. In `tr_add`'s order the sum is
`2.5e15 - 2.5e15 + 0.25 = 0.25`. The chain makes exactly those additions
at 1, 2, 3 and 4 ranks, so every rank count gives 0.25. (Adding each
rank's micro-batches from zero and then adding the ranks' sums would give
`(2.5e15 + 0.25) + (-2.5e15 + 0.25) = 0` at 2 ranks.)

## The caller's loop

The loop is chapter 118's loop with the micro-batches split over ranks.
Each rank runs the model's own step on its micro-batches, writing each
gradient into that micro-batch's slot, and reports. `dp_sum` makes the
sum and `tr_apply` makes the step. Here are chapter 103's tabular models;
the decoder's loop in the same file differs only in the model call and in
writing a checkpoint every few steps.

<!-- source: psionic/tests/data-parallel-checks.coax drive -->
```jai
// Chapter 118's loop for chapter 103's models, data parallel. Rank r runs
// the model on its own micro-batches, each gradient in its own slot, and
// reports at its accumulation boundary; dp_sum makes the step's sum and
// chapter 118's tr_apply makes the step. A step with fewer micro-batches
// than ranks (the end of an epoch) runs on as many ranks as it has.
dp_tabular_drive :: (m:TbModel,c:TrConfig,x:[]f64,y:[]f64,rows:[]int,run:TrRun,p:[]f64,mo:[]f64,vo:[]f64,order:[]int,stop:int,ranks:int,bucket:int,fault:DpFault,losses:[]f64)->DpDrive {
    out:=DpDrive.{run=run};n:=p.count;grads:[1288]f64;ml:[8]f64;msgs:[1288]f64;total:[161]f64;stage:[483]f64;none:[1]PkDoc;nothing:[1]u8;
    while out.run.revision<stop {
        b:=tr_next(c,out.run,none[..],nothing[..],nothing[..]);if b.status!=TrStatus.ok {out.status=b.status;return out;}
        k:=tr_min(ranks,b.micro);plan:=dp_plan(k,b.micro,n,tr_min(bucket,n));reports:[16]DpReport;sent:=0;
        for j:0..<k {r:=k-1-j;report:=DpReport.{rank=r,revision=b.revision};
            for mb:dp_first(plan,r)..<dp_first(plan,r+1) {s:=tr_rows(c,b,mb);
                ml[mb]=tm_tabular_step(m,p,x,y,rows,order[s.first..<s.first+s.count],grads[mb*n..<(mb+1)*n]);report.done+=1;}
            sent=dp_send(fault,b.revision,report,reports[..],sent);}
        s:=dp_sum(plan,b,reports[0..<sent],grads[0..<b.micro*n],ml[0..<b.micro],msgs[0..<k*n],total[0..<n]);
        if s.status!=DpStatus.ok {out.status=TrStatus.order;out.refused=s.status;out.rank=s.rank;return out;}
        a:=tr_apply(c,out.run,b,s.sum,p,mo,vo,total[0..<n],stage[0..<3*n],order);if a.status!=TrStatus.ok {out.status=a.status;return out;}
        losses[out.steps]=a.loss;out.run=a.run;out.steps+=1;
    }
    return out;
}
```

In rows mode chapter 118 ends each epoch with a shorter step when the
micro-batches do not divide evenly. The example's tabular run has 120 rows
in micro-batches of 7, so each epoch is 18 micro-batches: four steps of 4
and one of 2. The short step runs on 2 ranks; the bits do not depend on
the rank count, so the run is still chapter 118's.

## When the all-reduce can overlap backward

Backward computes the last layers' gradients first. When the gradient is
one bucket, the all-reduce waits for the whole backward pass. With
several buckets, the last bucket can travel while backward computes the
others. Only the last micro-batch's backward overlaps: earlier
micro-batches stay on the rank until its accumulation boundary.

`dp_schedule` models one step from the start of that last backward pass.
Backward takes `per` nanoseconds a value. A bucket's all-reduce is
chapter 132's chain: `2 * (ranks - 1)` hops in a row, each costing
`latency` nanoseconds plus the bucket's bytes at `bandwidth` bytes a
microsecond. There is one link, so a bucket starts when backward has
produced it and the bucket before it has finished.

<!-- source: psionic/lib/data-parallel.coax schedule -->
```jai
// The overlap model, in nanoseconds from the start of the step's last
// backward pass (earlier micro-batches send nothing: their gradients stay
// on the rank until its accumulation boundary). Backward takes `per` ns a
// value and finishes the last bucket first. Bucket k's chain all-reduce is
// 2(ranks-1) hops in a row, each `latency` ns plus the bucket's bytes at
// `bandwidth` bytes a microsecond. One link: a bucket starts when backward
// has produced it and the bucket before it has finished.
DpTime :: struct {status:DpStatus;backward:int;serial:int;overlap:int;}
dp_max :: (a:int,b:int)->int {return (a+b+cast(int)abs(f64(a-b)))/2;}
dp_comm :: (p:DpPlan,k:int,latency:int,bandwidth:int)->int {
    hops:=2*(p.ranks-1);return hops*latency+hops*dp_span(p,k).count*8*1000/bandwidth;
}
// start and end receive each bucket's all-reduce times. serial is the step
// when every all-reduce waits for the whole backward; overlap when each
// starts as soon as it can.
dp_schedule :: (p:DpPlan,per:int,latency:int,bandwidth:int,start:[]int,end:[]int)->DpTime {
    t:=DpTime.{status=p.status};if p.status!=DpStatus.ok {return t;}
    nb:=dp_buckets(p);
    if per<0 || latency<0 || bandwidth<1 || start.count!=nb || end.count!=nb {t.status=DpStatus.shape;return t;}
    ready:=0;free:=0;comm:=0;
    for j:0..<nb {k:=nb-1-j;c:=dp_comm(p,k,latency,bandwidth);
        ready+=dp_span(p,k).count*per;start[k]=dp_max(ready,free);end[k]=start[k]+c;free=end[k];comm+=c;}
    t.backward=ready;t.serial=ready+comm;t.overlap=free;
    return t;
}
```

Work it by hand for chapter 115's full decoder (`n = 3152`) on 4 ranks,
buckets of 1024, 20 ns of backward a value, 500 ns a hop and 25000 bytes
a microsecond. The buckets hold 1024, 1024, 1024 and 80 values. Each
all-reduce is 6 hops.

- **A full bucket** sends 8192 bytes a hop:
  `6 * 500 + 6 * 8192 * 1000 / 25000 = 3000 + 1966 = 4966` ns (the
  division rounds down).
- **Bucket 3** (80 values, 640 bytes): `3000 + 3840000 / 25000 = 3000 +
  153 = 3153` ns.
- **Backward** takes `3152 * 20 = 63040` ns. It finishes bucket 3 at
  `80 * 20 = 1600` ns, bucket 2 at `1600 + 20480 = 22080`, bucket 1 at
  42560 and bucket 0 at 63040.
- **The timeline.** Bucket 3 runs from 1600 to 4753 ns. Bucket 2 is ready
  at 22080, after the link is free, so it runs from 22080 to 27046.
  Bucket 1 runs from 42560 to 47526, and bucket 0 from 63040 to 68006.
- **Without overlap** the all-reduces run after backward:
  `63040 + 3153 + 3 * 4966 = 81091` ns. **With overlap** the step's
  communication ends at 68006 ns, 13085 ns sooner.

One bucket of 3152 values costs `3000 + 6051 = 9051` ns, so a step with
one bucket ends at `63040 + 9051 = 72091` ns. Four buckets beat one here.
At 5000 ns a hop the hops dominate: each full bucket costs 31966 ns,
longer than backward takes to produce the next one, and four buckets end
at 127651 ns against one bucket's 99091. Smaller buckets overlap more but
pay the latency more often; the right size depends on the link.

## The program

The example, [`psionic/examples/data-parallel.coax`](../examples/data-parallel.coax),
prints the plan, the hand example, the overlap model over several bucket
sizes, and then trains two of chapter 118's models data parallel. Chapter
103's two-layer model (19 parameters) runs 2 epochs at 1 to 4 ranks with
buckets of 1, 5 and 19 values. Chapter 115's decoder at the checks' small
shape (220 parameters, 4 rows a step) runs 4 steps at 1 to 4 ranks. Last,
a run with a checkpoint every 2 steps loses rank 1 in step 4 and restarts
from its checkpoint on 3 ranks.

<!-- source: psionic/examples/data-parallel.coax example -->
```jai
// Chapter 134: chapter 118's trainer, data parallel. The ranks run one after
// another on this CPU, last rank first; the bits do not depend on that.
main :: () {
    p:=dp_plan(3,4,10,4);print("4 micro-batches over 3 ranks, 10 values in buckets of 4: %\n",dp_status(p.status));
    for r:0..<3 {print("  rank % owns micro-batches % to %\n",r,dp_first(p,r),dp_first(p,r+1)-1);}
    for k:0..<dp_buckets(p) {s:=dp_span(p,k);print("  bucket % holds values % to %\n",k,s.first,s.first+s.count-1);}

    // The hand example: 4 micro-batches of 3 values over 2 ranks, buckets of 2.
    grads:=[12]f64.[1,2,3, 0.5,-4,6, -1,8,0, 3,10,-2];losses:=[4]f64.[4,8,2,6];msgs:[6]f64;total:[3]f64;b:=TrBatch.{micro=4};
    reports:=[2]DpReport.[DpReport.{rank=1,done=2},DpReport.{rank=0,done=2}];
    s:=dp_sum(dp_plan(2,4,3,2),b,reports[..],grads[..],losses[..],msgs[..],total[..]);
    want:[3]f64;t:TrSum;for k:0..<4 {t=tr_add(b,t,losses[k],grads[3*k..<3*k+3],want[..]);}
    print("hand example, 2 ranks: rank 0 sends % % %; the sum is % % %, loss %; equal to tr_add bit for bit: %\n",
        msgs[0],msgs[1],msgs[2],total[0],total[1],total[2],s.sum.loss,tr_same(total[..],want[..]) && f64_bits(t.loss)==f64_bits(s.sum.loss));

    print("overlap model, 3152 values on 4 ranks, 20 ns of backward a value, 25000 bytes a microsecond:\n");
    for latency:[2]int.[500,5000] {
        for bucket:[5]int.[3152,1576,1024,512,256] {q:=dp_plan(4,4,3152,bucket);start:[13]int;end:[13]int;nb:=dp_buckets(q);
            x:=dp_schedule(q,20,latency,25000,start[0..<nb],end[0..<nb]);
            print("  % ns a hop, buckets of %: % buckets, backward % ns, all-reduce after backward % ns, overlapped % ns\n",latency,bucket,nb,x.backward,x.serial,x.overlap);}}
    q:=dp_plan(4,4,3152,1024);start:[4]int;end:[4]int;x:=dp_schedule(q,20,500,25000,start[..],end[..]);
    print("  the timeline at 500 ns a hop, buckets of 1024:\n");
    ready:=0;for j:0..<4 {k:=3-j;ready+=dp_span(q,k).count*20;print("    bucket % (% values): backward done at % ns, all-reduce from % to % ns\n",k,dp_span(q,k).count,ready,start[k],end[k]);}

    // Chapter 103's two-layer model, 4 micro-batches of 7 rows a step, 2 epochs.
    tt:=tm_table();xs:=tt.xs[0..<4*tt.all];y:=tt.y[0..<tt.all];rows:=tt.rows[0..<tt.n];
    m:=TbModel.{features=4,hidden=3,logistic=true};tc:=tm_tabular(m,TbConfig.{epochs=2,batch=7,rate=0.5,decay=0.01,init_seed=3,shuffle_seed=4},tt.n);tc.accumulate=4;
    ref:[19]f64;mo:[19]f64;vo:[19]f64;order:[192]int;rl:[64]f64;none:[1]PkDoc;nothing:[1]u8;tb_initialize(m,3,ref[..]);
    o:=tm_tabular_drive(m,tc,xs,y,rows,rows,tr_begin(tc,19,none[0..<0],nothing[0..<0],order[..]).run,ref[..],mo[..],vo[..],order[..],1048576,rl[..]);
    print("chapter 103's model, % rows: chapter 118's loop takes % steps, last loss %\n",tt.n,o.steps,rl[o.steps-1]);
    for ranks:1..<5 {same:=true;for bucket:[3]int.[1,5,19] {r:=dp_tabular_run(m,3,tc,xs,y,rows,ranks,bucket,DpFault.{});same=same && r.drive.status==TrStatus.done && tr_same(r.p[0..<19],ref[..]) && tr_same(r.losses[0..<o.steps],rl[0..<o.steps]);}
        print("  % ranks, buckets of 1, 5 and 19: parameters and losses equal chapter 118's bit for bit: %\n",ranks,same);}

    // Chapter 115's decoder at the checks' small shape, 4 rows a step.
    f:=tt_fix();c:=tm_decoder(TT_SMALL,4);one:=dp_decoder_run(f,c,1,220,4,DpFault.{},0);
    print("chapter 115's decoder, 220 parameters, 4 steps: losses % % % %\n",one.losses[0],one.losses[1],one.losses[2],one.losses[3]);
    for ranks:2..<5 {r:=dp_decoder_run(f,c,ranks,64,4,DpFault.{},0);
        print("  % ranks, buckets of 64: parameters, moments and losses equal 1 rank's bit for bit: %\n",ranks,tr_same(r.p[..],one.p[..]) && tr_same(r.m[..],one.m[..]) && tr_same(r.v[..],one.v[..]) && tr_same(r.losses[0..<4],one.losses[0..<4]));}

    // A checkpoint every 2 steps; rank 1 is killed during step 4.
    k:=dp_decoder_run(f,c,4,64,4,DpFault.{step=3,rank=1,lost=true},2);
    print("4 ranks, rank 1 killed in step 4: %, rank %; the run stopped at revision %, last checkpoint at %\n",dp_status(k.drive.refused),k.drive.rank,k.drive.run.revision,k.drive.saved);
    back:=dp_decoder_resume(f,c,k,3,64,4);
    print("  restarted from the checkpoint on 3 ranks: % steps, losses % %; equal to the uninterrupted run: %\n",back.drive.steps,back.losses[0],back.losses[1],
        tr_same(back.p[..],one.p[..]) && tr_same(back.m[..],one.m[..]) && tr_same(back.v[..],one.v[..]));
}
```

```text
4 micro-batches over 3 ranks, 10 values in buckets of 4: admitted
  rank 0 owns micro-batches 0 to 0
  rank 1 owns micro-batches 1 to 1
  rank 2 owns micro-batches 2 to 3
  bucket 0 holds values 0 to 3
  bucket 1 holds values 4 to 7
  bucket 2 holds values 8 to 9
hand example, 2 ranks: rank 0 sends 0.375 -0.5 2.25; the sum is 0.875 4.0 1.75, loss 5.0; equal to tr_add bit for bit: true
overlap model, 3152 values on 4 ranks, 20 ns of backward a value, 25000 bytes a microsecond:
  500 ns a hop, buckets of 3152: 1 buckets, backward 63040 ns, all-reduce after backward 72091 ns, overlapped 72091 ns
  500 ns a hop, buckets of 1576: 2 buckets, backward 63040 ns, all-reduce after backward 75090 ns, overlapped 69065 ns
  500 ns a hop, buckets of 1024: 4 buckets, backward 63040 ns, all-reduce after backward 81091 ns, overlapped 68006 ns
  500 ns a hop, buckets of 512: 7 buckets, backward 63040 ns, all-reduce after backward 90091 ns, overlapped 67023 ns
  500 ns a hop, buckets of 256: 13 buckets, backward 63040 ns, all-reduce after backward 108085 ns, overlapped 66531 ns
  5000 ns a hop, buckets of 3152: 1 buckets, backward 63040 ns, all-reduce after backward 99091 ns, overlapped 99091 ns
  5000 ns a hop, buckets of 1576: 2 buckets, backward 63040 ns, all-reduce after backward 129090 ns, overlapped 97570 ns
  5000 ns a hop, buckets of 1024: 4 buckets, backward 63040 ns, all-reduce after backward 189091 ns, overlapped 127651 ns
  5000 ns a hop, buckets of 512: 7 buckets, backward 63040 ns, all-reduce after backward 279091 ns, overlapped 217651 ns
  5000 ns a hop, buckets of 256: 13 buckets, backward 63040 ns, all-reduce after backward 459085 ns, overlapped 397645 ns
  the timeline at 500 ns a hop, buckets of 1024:
    bucket 3 (80 values): backward done at 1600 ns, all-reduce from 1600 to 4753 ns
    bucket 2 (1024 values): backward done at 22080 ns, all-reduce from 22080 to 27046 ns
    bucket 1 (1024 values): backward done at 42560 ns, all-reduce from 42560 to 47526 ns
    bucket 0 (1024 values): backward done at 63040 ns, all-reduce from 63040 to 68006 ns
chapter 103's model, 120 rows: chapter 118's loop takes 10 steps, last loss 0.60276473860105
  1 ranks, buckets of 1, 5 and 19: parameters and losses equal chapter 118's bit for bit: true
  2 ranks, buckets of 1, 5 and 19: parameters and losses equal chapter 118's bit for bit: true
  3 ranks, buckets of 1, 5 and 19: parameters and losses equal chapter 118's bit for bit: true
  4 ranks, buckets of 1, 5 and 19: parameters and losses equal chapter 118's bit for bit: true
chapter 115's decoder, 220 parameters, 4 steps: losses 2.9360807937186078 2.761001189796394 2.7895427091516334 2.733675497919888
  2 ranks, buckets of 64: parameters, moments and losses equal 1 rank's bit for bit: true
  3 ranks, buckets of 64: parameters, moments and losses equal 1 rank's bit for bit: true
  4 ranks, buckets of 64: parameters, moments and losses equal 1 rank's bit for bit: true
4 ranks, rank 1 killed in step 4: refused: a rank did not report, rank 1; the run stopped at revision 3, last checkpoint at 2
  restarted from the checkpoint on 3 ranks: 2 steps, losses 2.7895427091516334 2.733675497919888; equal to the uninterrupted run: true
```

Every rank count and bucket size gives chapter 118's parameters, moments
and losses bit for bit, for both models. The killed rank's step is
refused; the run stays at revision 3 with the parameters of step 3. A
fresh process reads the checkpoint written after step 2 and runs steps 3
and 4 on 3 ranks, and ends with the uninterrupted run's bits.

## Refusals

[`psionic/examples/data-parallel-refuse.coax`](../examples/data-parallel-refuse.coax)
shows the plans that are refused and one refused step for each fault.

<!-- source: psionic/examples/data-parallel-refuse.coax refuse -->
```jai
// Chapter 134: plans and steps that are refused. A refused sum writes
// nothing, and the sum it returns is one chapter 118's tr_apply refuses too.
main :: () {
    print("0 ranks: %\n",dp_status(dp_plan(0,4,10,4).status));
    print("5 ranks for 4 micro-batches: %\n",dp_status(dp_plan(5,4,10,4).status));
    print("buckets of 11 values for a gradient of 10: %\n",dp_status(dp_plan(2,4,10,11).status));
    start:[3]int;end:[3]int;
    print("an overlap model with no bandwidth: %\n",dp_status(dp_schedule(dp_plan(2,4,10,4),20,500,0,start[..],end[..]).status));

    // One step of 4 micro-batches over 3 ranks, revision 5.
    p:=dp_plan(3,4,2,1);b:=TrBatch.{revision=5,micro=4};grads:[8]f64;losses:[4]f64;msgs:[6]f64;total:[2]f64;
    for k:0..<8 {grads[k]=f64(k);}
    good:=[3]DpReport.[DpReport.{rank=2,revision=5,done=2},DpReport.{rank=0,revision=5,done=1},DpReport.{rank=1,revision=5,done=1}];
    what:=[5]string.["rank 1's report is lost","rank 0 reports a failure","rank 2 reports after 1 of its 2 micro-batches","rank 1 reports twice","rank 1 reports for revision 4"];
    for k:0..<5 {
        r:[4]DpReport;for j:0..<3 {r[j]=good[j];}n:=3;
        if k==0 {n=2;}
        if k==1 {r[1].failed=true;}
        if k==2 {r[0].done=1;}
        if k==3 {r[3]=good[2];n=4;}
        if k==4 {r[2].revision=4;}
        for j:0..<6 {msgs[j]=-7;}total[0]=-7;total[1]=-7;
        s:=dp_sum(p,b,r[0..<n],grads[..],losses[..],msgs[..],total[..]);
        written:=false;for j:0..<6 {written=written || msgs[j]!=-7;}written=written || total[0]!=-7 || total[1]!=-7;
        print("%: %, rank %; anything written: %\n",what[k],dp_status(s.status),s.rank,written);
    }
    // tr_apply refuses the refused sum, so a loop that ignores the refusal still changes nothing.
    c:=TrConfig.{rate=0.1,bound=10,examples=4,epochs=1};w:=[2]f64.[1,2];m:[2]f64;v:[2]f64;stage:[6]f64;order:[4]int;
    s:=dp_sum(p,b,good[0..<2],grads[..],losses[..],msgs[..],total[..]);
    a:=tr_apply(c,TrRun.{revision=5},b,s.sum,w[..],m[..],v[..],total[..],stage[..],order[..]);
    print("tr_apply given the refused sum: status %, revision %, weights % %\n",a.status,a.run.revision,w[0],w[1]);
}
```

```text
0 ranks: refused: from 1 to 64 ranks
5 ranks for 4 micro-batches: refused: from 1 to 64 micro-batches a step, at least one a rank
buckets of 11 values for a gradient of 10: refused: a bucket holds from 1 value to the whole gradient
an overlap model with no bandwidth: refused: a buffer or timing input does not fit the plan
rank 1's report is lost: refused: a rank did not report, rank 1; anything written: false
rank 0 reports a failure: refused: a rank reported a failure, rank 0; anything written: false
rank 2 reports after 1 of its 2 micro-batches: refused: a rank reported before its accumulation boundary, rank 2; anything written: false
rank 1 reports twice: refused: a rank reported twice, rank 1; anything written: false
rank 1 reports for revision 4: refused: a rank reported for another step, rank 1; anything written: false
tr_apply given the refused sum: status order, revision 5, weights 1.0 2.0
```

No refused step writes a message or a sum. The sum a refusal returns has
status `order`, so `tr_apply` refuses it too: a loop that ignored the
refusal would still leave the weights and the run as they were. Both
programs exit with status 0; the refusals are their output.

## How it is checked

- `psionic/lib/data-parallel.coax` (`#authority`): plans by hand and every
  refusal; ownership for every rank count below 13 and micro-batch count
  below 17; the chain by hand against `tr_add`, and the rounding trace at
  1 to 4 ranks; for every rank count up to 6 and bucket size up to 9, the
  sum of eight micro-batches equals `tr_add` bit for bit, with the ranks
  reporting last first; every refusal of the gather, alone and with
  several faults; refused sums write nothing, and buffers that do not fit
  are refused; the overlap timeline by hand at 500 and 5000 ns a hop, a
  link slower than backward, and one rank.
- Chapter 103's two-layer model through the data-parallel loop at 1 to 4
  ranks and buckets of 1, 5 and 19 values equals chapter 118's own loop
  (`tm_tabular_drive`) bit for bit over two epochs, including each epoch's
  short last step.
- An independent check of the gradient's meaning: for four micro-batches
  of 5 rows each, the ranks' sum equals chapter 103's `tb_gradient` over
  all 20 rows within a relative 1e-12 (the bits differ, because a mean of
  means adds in another order), and the loss likewise.
- Chapter 115's decoder through the data-parallel loop at 1 to 4 ranks and
  buckets of 7, 64 and 220 values equals chapter 118's own loop
  (`tm_decoder_drive`) bit for bit; a killed rank refuses its step and
  writes nothing, and the run restarted from the checkpoint on 3 ranks
  ends with the uninterrupted bits; a record that was never written does
  not resume; failed, early and repeated reports are refused at the step
  they happen.
- Mutation testing (`--root-only`) kills all 185 mutants.
- The examples' transcripts, compared exactly in debug and release.

## What is not here yet

- **Measured throughput.** The draft asked for throughput at one and two
  devices, with and without overlap. Here the ranks run one after another
  on one CPU thread, and the models' backward passes return the whole
  gradient at once, so nothing can overlap and nothing is timed. Chapter
  132's harness measures ranks as host threads without buckets. A
  measurement of overlap needs a backward pass that hands out each bucket
  as it finishes it.
- **Real devices and links.** The overlap model's latency and bandwidth
  are inputs, not measurements, as in chapter 132.
- **Deadlines.** A killed rank shows up here as a missing report. Chapter
  132's gather adds the deadline; this chapter's restart path does not
  use it yet.
- **Chapter 117's module block** trains through chapter 118 in its own
  file and is not checked here.
- **Chapter 133's collectives.** The all-reduce here is chapter 132's
  chain written for buckets, not chapter 133's sharded-tensor collectives.

## Public API

- `psionic/lib/data-parallel.coax`: `dp_plan`, `dp_first`, `dp_buckets`,
  `dp_span`, `dp_gather`, `dp_chain`, `dp_sum`, `dp_comm`, `dp_schedule`,
  `dp_max`, `dp_status`, and the types `DpPlan`, `DpReport`, `DpGather`,
  `DpSum`, `DpTime` and `DpStatus`.
- `psionic/tests/data-parallel-checks.coax`: the data-parallel loops for
  chapter 103's models and chapter 115's decoder (`dp_tabular_drive`,
  `dp_decoder_drive`, `dp_decoder_resume`).

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
- [Chapter 118](ch118-one-trainer-for-every-model.md): the trainer whose
  sum and step this chapter keeps.
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the
  checkpoint the restart reads.
- [Chapter 132](ch132-train-across-devices.md): the chain all-reduce,
  deadlines and measured device groups.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md): meshes, layouts
  and collectives.
