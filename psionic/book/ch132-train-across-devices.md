# 132. Train across devices

> **Status: partly implemented.** A device group splits each training step
> of chapter 115's decoder across N logical devices. Each device computes
> its own rows' gradients with chapter 129's row kernel. A chain
> all-reduce then gives chapter 118's run bit for bit at every device
> count and arrival order, and a tree all-reduce gives the same bits at
> every power-of-two device count. A step with a late, missing, repeated,
> stale or failed device commits nothing. A run saved at one device count
> resumes at another. A communication cost model is worked by hand. The
> group runs as host threads (1 to 16 devices, with a measured receipt)
> and as one host thread plus the RTX 4080. Several identical GPUs or
> machines, a real interconnect, sharded optimizer state and the
> collectives shared with chapters 176 and 183 remain.

[Chapter 129](ch129-train-a-whole-model-on-one-gpu.md) trains chapter
115's tiny decoder with every step's arithmetic in device kernels. This
chapter splits each step across a group of devices. The split is data
parallel: every device holds the same parameters and moments, each device
computes the gradients of some of the step's rows, and an all-reduce
combines those gradients into the step's mean gradient. Every device then
applies the same AdamW update to its own copy.

The hard part is not the split. It is keeping the result the same. A
gradient summed in a different order rounds differently, and a group that
waits for a device that never answers must not write half a step. This
chapter makes both decisions in Coax, in
[`device-group.coax`](../lib/transformer/device-group.coax) (`#authority`):
who owns which rows, whether every device's report is admitted, the order
of every addition in the all-reduce, and whether a step commits.

This host has one GPU and a 28-thread CPU. The devices here are logical:
threads on the CPU, and the GPU as one member next to a thread. Nothing in
the plan, the gather or the all-reduce depends on what kind of device a
member is.

## Who owns which rows

A step has `rows` rows (chapter 118's micro-batches, one row each). Device
`d` owns the contiguous rows from `d * rows / devices` up to, but not
including, `(d + 1) * rows / devices`. Every device owns at least one row.

<!-- source: psionic/lib/transformer/device-group.coax plan -->
```jai
// Rows of a step over devices: device d owns rows dg_first(d) up to
// dg_first(d+1), a contiguous run, and every device owns at least one.
DgPlan :: struct {status:DgStatus;devices:int;rows:int;collective:DgCollective;}
dg_power :: (n:int)->bool {return n>0 && (n&(n-1))==0;}
// The levels of a pairwise tree over n leaves: the least l with 2^l >= n.
dg_log2 :: (n:int)->int {l:=0;while (1<<l)<n {l+=1;}return l;}
dg_plan :: (devices:int,rows:int,collective:DgCollective)->DgPlan #export_c {
    p:=DgPlan.{devices=devices,rows=rows,collective=collective};
    if devices<1 || devices>DG_MAX_DEVICES {p.status=DgStatus.devices;return p;}
    if rows<1 || rows>DG_MAX_ROWS || devices>rows {p.status=DgStatus.rows;return p;}
    // The tree adds rows in pairs, then pairs of pairs. Each device's rows
    // must be one whole subtree, or the device count would change the tree.
    if collective==DgCollective.ring || (collective==DgCollective.tree && !(dg_power(devices) && dg_power(rows))) {p.status=DgStatus.collective;}
    return p;
}
dg_first :: (p:DgPlan,d:int)->int #export_c {return d*p.rows/p.devices;}
dg_owner :: (p:DgPlan,row:int)->int {
    if row<0 || row>=p.rows {return -1;}
    d:=row*p.devices/p.rows;while dg_first(p,d+1)<=row {d+=1;}
    return d;
}
```

Work it by hand for 4 rows over 3 devices. `dg_first` gives
`0 * 4 / 3 = 0`, `1 * 4 / 3 = 1`, `2 * 4 / 3 = 2` and `3 * 4 / 3 = 4`, so
device 0 owns row 0, device 1 owns row 1, and device 2 owns rows 2 and 3.
Sixteen rows over 16 devices give one row each. Five devices for 4 rows
would leave one device idle, so the plan refuses that group.

## The order of the all-reduce

Chapter 129's `gk_mean` sums the rows' gradients divided by the row count,
row 0 first: `(((0 + g0/4) + g1/4) + g2/4) + g3/4`. Binary64 addition is
not associative, so any other order can give other bits.

Take one parameter whose four row gradients are `1e16`, `1`, `-1e16` and
`1`. Divided by 4 they are `2.5e15`, `0.25`, `-2.5e15` and `0.25`. Between
2^51 and 2^52 (about 2.25e15 to 4.5e15) neighbouring binary64 values are
0.5 apart, so `2.5e15 + 0.25` is a tie between `2.5e15` and `2.5e15 + 0.5`,
and ties go to the even neighbour, `2.5e15`.

- **Row order** (chapter 129): `0 + 2.5e15 = 2.5e15`; `+ 0.25` rounds to
  `2.5e15`; `- 2.5e15 = 0`; `+ 0.25 = 0.25`.
- **Pairs, then pairs of pairs**: `2.5e15 + 0.25` rounds to `2.5e15`;
  `-2.5e15 + 0.25` rounds to `-2.5e15`; their sum is `0`.

The exact mean is 0.5, so neither answer is exact. What matters is that one
order is fixed and every device count uses it.

The chain keeps chapter 129's order. Device 0 starts the sum at zero and
adds its rows in order; device 1 receives that running sum and continues
with its rows; and so on. The last device holds chapter 129's mean, bit for
bit, at any device count, because continuing a left-to-right sum over the
next contiguous rows is the same sum. The cost is that the devices add one
after another.

The tree adds rows in pairs, then pairs of pairs. Each device first adds
its own rows that way, then the devices' sums combine the same way: at
stride `s`, device `d` adds the sum of device `d + s`. When both the device
count and the row count are powers of two, each device's rows are one whole
subtree, so this is one fixed tree over the rows at any device count. Its
bits differ from chapter 129's, as the trace above shows, so the tree is
checked against chapter 129 within chapter 129's frozen tolerance, and
against itself bit for bit.

<!-- source: psionic/lib/transformer/device-group.coax reduce -->
```jai
// The chain all-reduce. Device d receives the running sum from device d-1
// (device 0 starts at zero), adds its rows' gradients divided by the row
// count in row order, and sends the sum on in its slot of msgs. The last
// slot is the step's gradient, which goes back to every device. The
// additions are chapter 129's gk_mean, in the same order, at any device
// count.
dg_chain :: (p:DgPlan,d:int,n:int,grads:[]f64,msgs:[]f64) {
    k:=f64(p.rows);
    for i:0..<n {acc:=0.0;if d>0 {acc=msgs[(d-1)*n+i];}
        for r:dg_first(p,d)..<dg_first(p,d+1) {acc+=grads[r*n+i]/k;}
        msgs[d*n+i]=acc;}
}
// The tree all-reduce. Device d adds its rows in pairs, then pairs of pairs,
// into its slot; then the devices' slots combine the same way: at stride s,
// device d adds the slot of device d+s. With power-of-two counts this is one
// fixed tree over the rows, whatever the device count.
dg_tree :: (p:DgPlan,d:int,n:int,grads:[]f64,part:[]f64) {
    k:=f64(p.rows);lo:=dg_first(p,d);count:=dg_first(p,d+1)-lo;
    for i:0..<n {x:[64]f64;for r:0..<count {x[r]=grads[(lo+r)*n+i]/k;}
        for level:0..<dg_log2(count) {s:=1<<level;for j:0..<count/(2*s) {x[2*s*j]+=x[2*s*j+s];}}
        part[d*n+i]=x[0];}
}
dg_reduce :: (p:DgPlan,n:int,grads:[]f64,slots:[]f64,total:[]f64) {
    if p.collective==DgCollective.chain {
        for d:0..<p.devices {dg_chain(p,d,n,grads,slots);}
        for i:0..<n {total[i]=slots[(p.devices-1)*n+i];}
        return;
    }
    for d:0..<p.devices {dg_tree(p,d,n,grads,slots);}
    for level:0..<dg_log2(p.devices) {s:=1<<level;for j:0..<p.devices/(2*s) {d:=2*s*j;for i:0..<n {slots[d*n+i]+=slots[(d+s)*n+i];}}}
    for i:0..<n {total[i]=slots[i];}
}
```

A ring all-reduce, the usual choice on GPU clusters, splits the gradient
into one piece per device and passes the pieces around a ring. Each piece
is summed in an order that starts at a different device, and that order
changes with the device count. The plan refuses it; it appears below only
in the cost model.

## Every device reports, once, in time

A device that finishes its rows sends a report: who it is, the run
revision and the rows it computed, when it finished and whether it failed.
The step reaches the all-reduce only when every device has reported exactly
once, for this revision and its own rows, by the deadline, without a
failure. Reports can arrive in any order. A refusal names the lowest device
at fault, so the answer does not depend on the arrival order either.

<!-- source: psionic/lib/transformer/device-group.coax gather -->
```jai
// What a device says when its rows' gradients are written: who it is, the
// run revision and rows it computed, when it finished (in the clock the
// deadline uses) and whether it failed.
DgReport :: struct {device:int;revision:int;first:int;count:int;at:int;failed:bool;}
DgGather :: struct {status:DgStatus;device:int=-1;}
dg_report :: (p:DgPlan,run:TrRun,d:int,at:int)->DgReport {
    lo:=dg_first(p,d);return DgReport.{device=d,revision=run.revision,first=lo,count=dg_first(p,d+1)-lo,at=at};
}
// Admits a step's reports, which may arrive in any order. Every device must
// report exactly once, for this revision and its own rows, by the deadline,
// without a failure. A refusal names the lowest device at fault, so the
// answer does not depend on the arrival order either.
dg_gather :: (p:DgPlan,run:TrRun,reports:[]DgReport,deadline:int)->DgGather {
    if p.status!=DgStatus.ok {return DgGather.{status=p.status};}
    seen:[64]int;g:DgGather;
    for r:reports {
        if r.device<0 || r.device>=p.devices {return DgGather.{status=DgStatus.stale,device=r.device};}
        seen[r.device]+=1;
    }
    for d:0..<p.devices {g.device=d;
        if seen[d]==0 {g.status=DgStatus.missing;return g;}
        if seen[d]>1 {g.status=DgStatus.twice;return g;}
        for r:reports {if r.device==d {
            if r.failed {g.status=DgStatus.failed;return g;}
            if r.revision!=run.revision || r.first!=dg_first(p,d) || r.count!=dg_first(p,d+1)-dg_first(p,d) {g.status=DgStatus.stale;return g;}
            if r.at>deadline {g.status=DgStatus.late;return g;}}}
    }
    g.device=-1;return g;
}
```

## The step

`dg_rows` is one device's share: chapter 129's `gk_rows` on each of its
rows, each writing only that row's slices of the work, gradient and loss
buffers. `dg_finish` gathers the reports, runs the all-reduce, and then
runs the rest of chapter 129's step unchanged: the norm, `gt_decide`, the
clip, AdamW into the stage, the admission flags, `gt_commit` and the
commit. A refused gather returns before the all-reduce, so no stage,
parameter or moment is written, and the run keeps its revision. Retrying
the same batch is then safe.

<!-- source: psionic/lib/transformer/device-group.coax step -->
```jai
DgStep :: struct {status:DgStatus;device:int=-1;step:TrStep;}
// Device d's part of a step: chapter 129's row kernel on each of its rows,
// writing only those rows' slices of work, dwork, grads and losses. The
// caller (a thread, a GPU's host, or the CPU loop below) stamps the report
// with the time it finished. A batch the plan does not cover is not computed.
dg_rows :: (s:TdShape,p:DgPlan,d:int,b:TrBatch,rows:[]u8,rope:[]f64,pm:[]f64,work:[]f64,dwork:[]f64,grads:[]f64,losses:[]f64)->DgReport #export_c {
    lo:=dg_first(p,d);hi:=dg_first(p,d+1);r:=DgReport.{device=d,revision=b.revision,first=lo,count=hi-lo};
    if p.status!=DgStatus.ok || b.status!=TrStatus.ok || b.micro!=p.rows || d<0 || d>=p.devices {r.failed=true;return r;}
    sh:[8]int;gt_shape(s,sh[..]);
    for k:lo..<hi {gk_rows(k,sh[..],pm,rows,rope,work,dwork,grads,losses);}
    return r;
}
// The rest of the step, once reports are in. Only a gathered step reaches
// the all-reduce; then chapter 129's step runs on every device's copy, and
// only an admitted step is committed.
dg_finish :: (c:TrConfig,p:DgPlan,run:TrRun,b:TrBatch,reports:[]DgReport,deadline:int,pm:[]f64,m:[]f64,v:[]f64,stage:[]f64,total:[]f64,grads:[]f64,losses:[]f64,scalars:[]f64,slots:[]f64)->DgStep #export_c {
    fail:=DgStep.{status=p.status,step=TrStep.{status=TrStatus.order,run=run}};
    if p.status!=DgStatus.ok {return fail;}
    if b.micro!=p.rows {fail.status=DgStatus.rows;return fail;}
    if b.status!=TrStatus.ok {return fail;}
    g:=dg_gather(p,run,reports,deadline);if g.status!=DgStatus.ok {fail.status=g.status;fail.device=g.device;return fail;}
    n:=pm.count;dg_reduce(p,n,grads,slots,total);
    gk_squares(0,n,total,scalars);
    d:=gt_decide(c,run,b,losses[0..<p.rows],scalars[0]);if d.status!=TrStatus.ok {fail.step.status=d.status;return fail;}
    if d.clip {for i:0..<n {gk_clip(i,c.clip,d.norm,total);}}
    for i:0..<n {gk_adamw(i,n,d.rate,c.beta1,c.beta2,c.eps,0,d.c1,d.c2,pm,total,m,v,stage);}
    gk_admit(0,n,c.bound,stage,scalars[1..<3]);
    st:=gt_commit(c,run,b,d,scalars[1..<3]);
    if st.status==TrStatus.ok {for i:0..<n {gk_commit(i,n,stage,pm,m,v);}}
    return DgStep.{step=st};
}
// One step with every device on this CPU, run one after another in `order`
// and stamped with the matching time in `at`. reports holds order.count.
dg_step_cpu :: (s:TdShape,c:TrConfig,p:DgPlan,run:TrRun,b:TrBatch,rows:[]u8,rope:[]f64,pm:[]f64,m:[]f64,v:[]f64,stage:[]f64,total:[]f64,grads:[]f64,work:[]f64,dwork:[]f64,losses:[]f64,scalars:[]f64,slots:[]f64,
        order:[]int,at:[]int,deadline:int,reports:[]DgReport)->DgStep {
    for k:0..<order.count {reports[k]=dg_rows(s,p,order[k],b,rows,rope,pm,work,dwork,grads,losses);reports[k].at=at[k];}
    return dg_finish(c,p,run,b,reports[0..<order.count],deadline,pm,m,v,stage,total,grads,losses,scalars,slots);
}
```

Because every device ends a step with the same parameters and moments, a
checkpoint is chapter 129's checkpoint (`gt_save`). The checks save a run
after two steps at 4 devices and resume it at 3 devices; the resumed run
ends with the same bits as the run that was never interrupted.

## What the all-reduce sends

The cost model counts what one all-reduce of `n` binary64 values sends
between `devices` devices, when each hop costs a fixed latency and each
link carries a fixed number of bytes a microsecond. It reports the hops in
a row on the longest path, the bytes along that path, all bytes sent, and
the time `hops * latency + path / bandwidth`.

<!-- source: psionic/lib/transformer/device-group.coax cost -->
```jai
// What one all-reduce of n binary64 values sends, at `devices` devices with
// `latency` nanoseconds a hop and `bandwidth` bytes a microsecond on every
// link. hops is the hops in a row on the longest path; path the bytes along
// it; bytes all bytes sent; ns = hops * latency + path / bandwidth.
//   chain: d-1 hops to sum, d-1 to send the sum back, n values each.
//   tree:  log2(d) levels to sum, log2(d) to send back, n values each.
//   ring:  d-1 steps to sum, d-1 to share, each moving one d-th of the values
//          (rounded up) between every neighbour pair at once.
DgCost :: struct {hops:int;path:int;bytes:int;ns:int;}
dg_cost :: (k:DgCollective,devices:int,n:int,latency:int,bandwidth:int)->DgCost #export_c {
    c:DgCost;if devices<2 {return c;}
    if bandwidth<1 {c.ns=-1;return c;}
    if k==DgCollective.chain {c.hops=2*(devices-1);c.path=c.hops*n*8;c.bytes=c.path;}
    else if k==DgCollective.tree {c.hops=2*dg_log2(devices);c.path=c.hops*n*8;c.bytes=2*(devices-1)*n*8;}
    else {piece:=(n+devices-1)/devices;c.hops=2*(devices-1);c.path=c.hops*piece*8;c.bytes=c.path*devices;}
    c.ns=c.hops*latency+c.path*1000/bandwidth;
    return c;
}
```

Work it by hand for chapter 115's decoder (`n = 3152`, so 25216 bytes a
gradient) on 4 devices, 5000 ns a hop and 25000 bytes a microsecond
(25 GB/s):

- **Chain**: 3 hops to sum and 3 to send the sum back, 6 hops of 25216
  bytes. Path 151296 bytes; time `6 * 5000 + 151296 * 1000 / 25000 =
  30000 + 6051 = 36051` ns (the division rounds down).
- **Tree**: 2 levels to sum and 2 to send back, 4 hops of 25216 bytes. Path
  100864 bytes; time `20000 + 4034 = 24034` ns. All bytes sent: 3 sends
  up and 3 down, 151296.
- **Ring**: 6 steps, each moving a piece of `ceil(3152 / 4) = 788` values.
  Path `6 * 788 * 8 = 37824` bytes; time `30000 + 1512 = 31512` ns.

At this size latency dominates and the tree wins. At a billion values on 8
devices the bytes dominate: the chain takes about 4.48 s, the tree 1.92 s
and the ring 0.56 s. That is why large runs use a ring, and why this
chapter does not yet: the ring's bits would depend on the device count.

## The program

The example, [`psionic/examples/device-group.coax`](../examples/device-group.coax),
plans a group, prints the cost table, and runs three steps at
the checks' small shape (220 parameters, four rows a step) with each device
count from 1 to 4. The devices run one after another on this CPU, last
device first.

<!-- source: psionic/examples/device-group.coax example -->
```jai
// Chapter 132: a step's rows split over a group of devices, the all-reduce
// in a fixed order, and what it costs to send. The devices run one after
// another on this CPU, last device first; the bits do not depend on that.
// psionic/tests/check_device_group.sh runs them as threads and with the GPU.
main :: () {
    p:=dg_plan(3,4,DgCollective.chain);print("4 rows over 3 devices: %\n",dg_status(p.status));
    for d:0..<3 {print("  device % owns rows % to %\n",d,dg_first(p,d),dg_first(p,d+1)-1);}

    print("one all-reduce of 3152 binary64 values, 5000 ns a hop, 25000 bytes a microsecond:\n");
    names:=[3]string.["chain","tree","ring"];kinds:=[3]DgCollective.[DgCollective.chain,DgCollective.tree,DgCollective.ring];
    for d:[3]int.[2,4,8] {for k:0..<3 {x:=dg_cost(kinds[k],d,3152,5000,25000);print("  % devices, %: % hops, % bytes on the longest path, % bytes in all, % ns\n",d,names[k],x.hops,x.path,x.bytes,x.ns);}}
    print("the same at a billion values, 8 devices:\n");
    for k:0..<3 {x:=dg_cost(kinds[k],8,1000000000,5000,25000);print("  %: % ns\n",names[k],x.ns);}

    f:=tt_fix();c:=gt_config(TT_SMALL,4,3);run:=gt_start(c,f,220);one:=gt_small_run(c,f,run,3,gt_small());
    print("three steps at the checks' small shape, 4 rows a step; chapter 129's single-device losses: % % %\n",one.x.losses[0],one.x.losses[1],one.x.losses[2]);
    for d:1..<5 {r:=dg_small_run(c,f,dg_plan(d,4,DgCollective.chain),run,3,dg_small(),true);
        same:=tr_same(r.x.losses[0..<3],one.x.losses[0..<3]) && tr_same(r.x.p[..],one.x.p[..]) && tr_same(r.x.m[..],one.x.m[..]) && tr_same(r.x.v[..],one.x.v[..]);
        print("  chain, % devices: %, losses, parameters and moments equal chapter 129's bit for bit: %\n",d,dg_status(r.step.status),same);}
    t1:=dg_small_run(c,f,dg_plan(1,4,DgCollective.tree),run,3,dg_small(),true);
    for d:[3]int.[1,2,4] {t:=dg_small_run(c,f,dg_plan(d,4,DgCollective.tree),run,3,dg_small(),true);g:=gt_gap(t.x.p[..],one.x.p[..],GT_TOLERANCE);
        print("  tree, % devices: parameters equal the 1-device tree's bit for bit: %; against chapter 129's, % of 220 differ in bits, % past the tolerance\n",d,tr_same(t.x.p[..],t1.x.p[..]),g.differ,g.far);}

    // Two steps at 4 devices, saved; resumed into fresh buffers at 3 devices.
    whole:=dg_small_run(c,f,dg_plan(4,4,DgCollective.chain),run,3,dg_small(),false);half:=dg_small_run(c,f,dg_plan(4,4,DgCollective.chain),run,2,dg_small(),false);
    ids:=gt_ids(TT_SMALL,c,f.docs[..],f.tokens[0..<f.n]);bytes:[5620]u8;pointer:[80]u8;none:[1]int;
    saved:=gt_save(ids,half.step.step.run,half.x.p[..],half.x.m[..],half.x.v[..],bytes[..],pointer[..]);
    y:=dg_small();back:=gt_resume(c,ids,pointer[..],bytes[..],f.docs[..],f.tokens[0..<f.n],y.p[..],y.m[..],y.v[..],none[0..<0]);
    rest:=dg_small_run(c,f,dg_plan(3,4,DgCollective.chain),back.run,1,y,true);
    print("saved at 4 devices after step 2 (%), resumed at 3 devices: step 3 loss %, weights equal the uninterrupted run: %\n",
        saved,rest.x.losses[0],tr_same(rest.x.p[..],whole.x.p[..]) && tr_same(rest.x.v[..],whole.x.v[..]));
}
```

```text
4 rows over 3 devices: admitted
  device 0 owns rows 0 to 0
  device 1 owns rows 1 to 1
  device 2 owns rows 2 to 3
one all-reduce of 3152 binary64 values, 5000 ns a hop, 25000 bytes a microsecond:
  2 devices, chain: 2 hops, 50432 bytes on the longest path, 50432 bytes in all, 12017 ns
  2 devices, tree: 2 hops, 50432 bytes on the longest path, 50432 bytes in all, 12017 ns
  2 devices, ring: 2 hops, 25216 bytes on the longest path, 50432 bytes in all, 11008 ns
  4 devices, chain: 6 hops, 151296 bytes on the longest path, 151296 bytes in all, 36051 ns
  4 devices, tree: 4 hops, 100864 bytes on the longest path, 151296 bytes in all, 24034 ns
  4 devices, ring: 6 hops, 37824 bytes on the longest path, 151296 bytes in all, 31512 ns
  8 devices, chain: 14 hops, 353024 bytes on the longest path, 353024 bytes in all, 84120 ns
  8 devices, tree: 6 hops, 151296 bytes on the longest path, 353024 bytes in all, 36051 ns
  8 devices, ring: 14 hops, 44128 bytes on the longest path, 353024 bytes in all, 71765 ns
the same at a billion values, 8 devices:
  chain: 4480070000 ns
  tree: 1920030000 ns
  ring: 560070000 ns
three steps at the checks' small shape, 4 rows a step; chapter 129's single-device losses: 2.9360807937186078 2.761001189796394 2.7893926020018047
  chain, 1 devices: admitted, losses, parameters and moments equal chapter 129's bit for bit: true
  chain, 2 devices: admitted, losses, parameters and moments equal chapter 129's bit for bit: true
  chain, 3 devices: admitted, losses, parameters and moments equal chapter 129's bit for bit: true
  chain, 4 devices: admitted, losses, parameters and moments equal chapter 129's bit for bit: true
  tree, 1 devices: parameters equal the 1-device tree's bit for bit: true; against chapter 129's, 7 of 220 differ in bits, 0 past the tolerance
  tree, 2 devices: parameters equal the 1-device tree's bit for bit: true; against chapter 129's, 7 of 220 differ in bits, 0 past the tolerance
  tree, 4 devices: parameters equal the 1-device tree's bit for bit: true; against chapter 129's, 7 of 220 differ in bits, 0 past the tolerance
saved at 4 devices after step 2 (ok), resumed at 3 devices: step 3 loss 2.7893926020018047, weights equal the uninterrupted run: true
```

The chain matches chapter 129's single-device run bit for bit at every
device count. The tree matches itself at 1, 2 and 4 devices; 7 of its 220
parameters differ from chapter 129's in their last bits, and none past the
tolerance.

## Refusals

[`psionic/examples/device-group-refuse.coax`](../examples/device-group-refuse.coax)
shows the groups the plan refuses and one refused step for each fault.

<!-- source: psionic/examples/device-group-refuse.coax refuse -->
```jai
// Chapter 132: groups the plan refuses, and steps the gather refuses. A
// refused step writes no parameter or moment and leaves the run where it
// was; the same batch retried with every device in time is the step that
// was never interrupted.
main :: () {
    print("0 devices: %\n",dg_status(dg_plan(0,4,DgCollective.chain).status));
    print("5 devices for 4 rows: %\n",dg_status(dg_plan(5,4,DgCollective.chain).status));
    print("a tree over 3 devices: %\n",dg_status(dg_plan(3,4,DgCollective.tree).status));
    print("a ring over 4 devices: %\n",dg_status(dg_plan(4,4,DgCollective.ring).status));

    f:=tt_fix();c:=gt_config(TT_SMALL,4,3);run:=gt_start(c,f,220);x:=dg_small();p:=dg_plan(4,4,DgCollective.chain);
    whole:=dg_small_run(c,f,p,run,1,x,false);rows:[32]u8;b:=gt_next(c,run,f.docs[..],f.tokens[0..<f.n],rows[..]);
    what:=[4]string.["device 2's report is lost","device 1 reports 100 ticks late","device 3 reports twice","device 0 reports a failure"];
    faults:=[4]DgFault.[DgFault.{device=2,lost=true},DgFault.{device=1,delay=100},DgFault.{device=3,again=true},DgFault.{device=0,failed=true}];
    for k:0..<4 {s:=dg_small_step(c,p,run,b,rows[..],x,faults[k]);
        print("%: %, device %; revision %, parameters and moments unchanged: %\n",what[k],dg_status(s.step.status),s.step.device,s.step.step.run.revision,
            tr_same(s.x.p[..],x.p[..]) && tr_same(s.x.m[..],x.m[..]) && tr_same(s.x.v[..],x.v[..]));
        r:=dg_small_step(c,p,run,b,rows[..],s.x,DgFault.{});
        print("  retried with every device in time: %, revision %, equal to the uninterrupted step: %\n",dg_status(r.step.status),r.step.step.run.revision,tr_same(r.x.p[..],whole.x.p[..]) && tr_same(r.x.v[..],whole.x.v[..]));}
    old:=b;old.revision=1;s:=dg_small_step(c,p,run,old,rows[..],x,DgFault.{});
    print("every device computed a batch of revision 1 while the run is at 0: %, device %\n",dg_status(s.step.status),s.step.device);
}
```

```text
0 devices: refused: from 1 to 64 devices
5 devices for 4 rows: refused: from 1 to 64 rows a step, at least one a device
a tree over 3 devices: refused: this all-reduce's additions would depend on the device count
a ring over 4 devices: refused: this all-reduce's additions would depend on the device count
device 2's report is lost: refused: a device did not report, device 2; revision 0, parameters and moments unchanged: true
  retried with every device in time: admitted, revision 1, equal to the uninterrupted step: true
device 1 reports 100 ticks late: refused: a device reported after the deadline, device 1; revision 0, parameters and moments unchanged: true
  retried with every device in time: admitted, revision 1, equal to the uninterrupted step: true
device 3 reports twice: refused: a device reported twice, device 3; revision 0, parameters and moments unchanged: true
  retried with every device in time: admitted, revision 1, equal to the uninterrupted step: true
device 0 reports a failure: refused: a device reported a failure, device 0; revision 0, parameters and moments unchanged: true
  retried with every device in time: admitted, revision 1, equal to the uninterrupted step: true
every device computed a batch of revision 1 while the run is at 0: refused: a device reported for another step or other rows, device 0
```

Each refused step leaves the run at revision 0 and every parameter and
moment unchanged. The same batch retried with every device in time is the
step that was never interrupted. Both programs exit with status 0; the
refusals are their output.

## On threads and on the GPU

`psionic/tests/check_device_group.sh` runs the checks and both examples in
debug and release, then two native harnesses built from the same Coax
compiled to C. Each harness only starts threads or launches kernels,
stamps reports with the time they finished, and times steps.

```shell
COAX_CC=gcc psionic/tests/check_device_group.sh
```

`psionic/tests/device_group_threads.c` runs every device as a POSIX thread,
first under ThreadSanitizer (2 steps, 4 devices), then at 1, 2, 4, 8 and 16
devices with 16 rows a step of chapter 115's decoder (3152 parameters,
rows of 32 tokens). `psionic/tests/device_group_gpu.c` makes a group of two
devices: a host thread on rows 0 to 7, and the RTX 4080 running the
Coax-generated `gk_rows` on rows 8 to 15 at the same time. The GPU's rows'
gradients are copied back and the chain runs on the host. Without a CUDA
device it prints `SKIP`.

The run recorded for this chapter (Intel Core i7-14700K, 28 threads, GCC
15.2.0 with `-O2 -ffp-contract=off`, RTX 4080 sm_89, load average about 9
from other jobs, 2026-11-01). The correctness lines come from the check's
6-step run (the 8-device lines elided); the receipts and
the GPU group's lines from a 40-step run of the same binaries:

```text
6 steps of 16 rows of chapter 115's decoder (3152 parameters), each device a host thread
chain all-reduce:
   1 devices: equal to chapter 118's run bit for bit: yes; equal to 1 device: yes; parameters 0 differ in bits, 0 past the tolerance
   2 devices: equal to chapter 118's run bit for bit: yes; equal to 1 device: yes; parameters 0 differ in bits, 0 past the tolerance
   4 devices: equal to chapter 118's run bit for bit: yes; equal to 1 device: yes; parameters 0 differ in bits, 0 past the tolerance
  16 devices: equal to chapter 118's run bit for bit: yes; equal to 1 device: yes; parameters 0 differ in bits, 0 past the tolerance
tree all-reduce:
   1 devices: equal to chapter 118's run bit for bit: no; equal to 1 device: yes; parameters 1101 differ in bits, 0 past the tolerance
   2 devices: equal to chapter 118's run bit for bit: no; equal to 1 device: yes; parameters 1101 differ in bits, 0 past the tolerance
   4 devices: equal to chapter 118's run bit for bit: no; equal to 1 device: yes; parameters 1101 differ in bits, 0 past the tolerance
  16 devices: equal to chapter 118's run bit for bit: no; equal to 1 device: yes; parameters 1101 differ in bits, 0 past the tolerance
a step with every report past the deadline at 8 devices: refused as late, device 0 named, parameters and run unchanged: yes; retried, the run ends with chapter 118's bits: yes
receipt: chapter 118's CPU run 4.00 ms/step (127872 tokens/s)
receipt:  1 devices, chain 4.82 ms/step (106192 tokens/s, 1.00x one device), tree 4.55 ms/step
receipt:  2 devices, chain 2.79 ms/step (183677 tokens/s, 1.73x one device), tree 2.96 ms/step
receipt:  4 devices, chain 1.53 ms/step (335272 tokens/s, 3.16x one device), tree 1.66 ms/step
receipt:  8 devices, chain 0.99 ms/step (518109 tokens/s, 4.88x one device), tree 1.12 ms/step
receipt: 16 devices, chain 0.80 ms/step (638612 tokens/s, 6.01x one device), tree 1.06 ms/step
device 1: NVIDIA GeForce RTX 4080 (sm_89); device 0: a host thread
a step whose GPU report comes after the deadline: refused as late, device 1 named, nothing written: yes
rows 8 to 15 of the first step, GPU against the host's C build:
  row gradients    25216 values, 22390 differ in bits, 0 past the tolerance, worst relative gap 2.48e-11
  row losses           8 values,     3 differ in bits, 0 past the tolerance, worst relative gap 7.63e-17
40 steps of 16 rows, host thread and GPU, against chapter 118's CPU run (tolerance 1e-09):
  losses              40 values,    11 differ in bits, 0 past the tolerance, worst relative gap 2.5e-16
  parameters        3152 values,  2463 differ in bits, 0 past the tolerance, worst relative gap 3.9e-13
  first moments     3152 values,  3052 differ in bits, 0 past the tolerance, worst relative gap 2.89e-12
  second moments    3152 values,  2876 differ in bits, 0 past the tolerance, worst relative gap 2.15e-15
receipt: host thread and GPU, 35.33 ms/step (14490 tokens/s)
```

What it shows:

- **The chain is exact.** At every device count the losses, parameters and
  both moments equal chapter 118's CPU run bit for bit, with threads
  finishing in whatever order the scheduler chose.
- **The tree is stable.** Its bits are the same at every device count;
  1101 of 3152 parameters differ from chapter 118's in their last bits,
  none past the tolerance.
- **A late step writes nothing.** The run, parameters and moments stay as
  they were, and the retried run ends with chapter 118's bits.
- **The mixed group is within the tolerance.** The GPU's rows differ from
  the host's in their last bits (device `exp` and `log`, as in chapter
  129), and after 40 steps every value is within the frozen tolerance.

The speed lines are a receipt, not a check: they change from run to run.
Sixteen threads make a step 6 times faster than one, not 16, because each
step still runs the all-reduce, the norm and AdamW on one thread, and the
threads start and join every step. The chain beats the tree here because
both reductions run in shared memory, where the chain's sequential hops
cost nothing; the cost model's hops are what a real interconnect would
add. The mixed group is slower than one thread, because chapter 129's row
kernel runs a whole row on one GPU thread, so the GPU's eight rows run on
eight threads.

## How it is checked

- `psionic/lib/transformer/device-group.coax` (`#authority`): plans by
  hand and every refusal; row owners against a brute-force search for
  every row and device count below 20; the chain and the tree by hand,
  including the order trace above; the chain against `gk_mean` and the
  tree against itself for every row count up to 16 and every device count;
  every refusal of the gather, with reports in any order and several
  faults at once; the group step against chapter 129's step bit for bit
  at 1 to 4 devices, plain and clipped, devices in either order; the tree
  step at 1, 2 and 4 devices; refused steps (lost, late, repeated, failed,
  stale, a rate AdamW refuses, a bound crossed) write nothing, and their
  retries are the uninterrupted step; resume at another device count; the
  cost model by hand. Mutation testing (`--root-only`) kills all 309
  mutants.
- The examples' transcripts, compared exactly in debug and release.
- The thread harness under ThreadSanitizer, and both native harnesses
  above.

## What is not here yet

- Several identical GPUs, or several machines: this host has one GPU.
  Nothing here has run on a real interconnect, so the cost model's
  latencies and bandwidths are inputs, not measurements.
- A deterministic ring all-reduce, and the collectives shared with serving
  in chapters 176 and 183.
- Sharded optimizer state (each device holding part of the moments) and a
  sharded checkpoint: every device here holds every value.
- A step that goes on without a failed device. A refused step is retried
  with the whole group; dropping a device would change the row split,
  which the run's identity does not record yet.
- A GPU member fast enough to help: that needs chapter 129's row kernel
  split across GPU threads.

## Public API

- `psionic/lib/transformer/device-group.coax`: `dg_plan`, `dg_first`,
  `dg_owner`, `dg_report`, `dg_gather`, `dg_chain`, `dg_tree`, `dg_reduce`,
  `dg_rows`, `dg_finish`, `dg_step_cpu`, `dg_cost`, `dg_status`, and the
  types `DgPlan`, `DgReport`, `DgGather`, `DgStep`, `DgCost`, `DgStatus`,
  `DgCollective`.
- `psionic/tests/device_group_threads.c`, `psionic/tests/device_group_gpu.c`
  and `psionic/tests/check_device_group.sh`: the native harnesses.

## Sources

- [Chapter plan](PLAN.md)
- [Chapter 35](ch035-use-every-core.md): fixed partitions and tree-ordered
  merges on host threads.
- [Chapter 118](ch118-one-trainer-for-every-model.md) and
  [chapter 129](ch129-train-a-whole-model-on-one-gpu.md): the trainer and
  the device step this chapter splits.
- [nanochat target notes](../docs/targets/nanochat.md)
