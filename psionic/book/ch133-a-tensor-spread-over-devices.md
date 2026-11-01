# 133. A tensor spread over devices

> **Status: partly implemented.** A sharded tensor is a global shape, a
> device count and a layout: replicated, sharded along rows or columns, or
> partial (each device holds an addend). Redistributions between layouts
> (slice, all-gather, all-to-all, reduce-scatter, all-reduce) sum in a
> fixed device order and move exactly the values a hand-worked cost model
> counts. Matrix products follow a rule table (data-, column- and
> row-parallel) and match one device; propagation gives every node's layout
> or names the node it refuses. The devices are logical blocks on the CPU.
> A named multi-axis mesh, layouts carried in chapter 38's tensor IR,
> transfers through chapter 6's device storage, threads and GPUs, and
> operations beyond the product, add and relu remain.

[Chapter 132](ch132-train-across-devices.md) split a training step's rows
over a group of devices, and every device held every parameter. That works
until a parameter matrix no longer fits on one device. Then the matrix
itself has to be split, and every operation that uses it has to know which
part of it each device holds.

This chapter gives a tensor a layout over a device group and decides, for
each operation, what layout its result has, which combinations are refused,
and what has to move between devices to change a layout. It is all in
[`shard.coax`](../lib/shard.coax) (`#authority`). The devices are logical:
device `d`'s data is one block of a buffer on this CPU. Nothing in the
layouts, the rules or the counts depends on what kind of device a block
lives on.

## Layouts

A sharded tensor here is a matrix of `rows` by `cols` binary64 values over
`devices` devices (1 to 8), in one of four layouts:

- **replicated**: every device holds the whole matrix;
- **sharded on rows** (axis 0): device `d` holds a contiguous run of rows;
- **sharded on columns** (axis 1): device `d` holds a contiguous run of
  columns;
- **partial**: every device holds a whole-shape addend, and the matrix is
  their sum, device 0's first.

A shard uses chapter 132's split: device `d`'s piece of an extent `n` runs
from `d * n / devices` up to, but not including, `(d + 1) * n / devices`.
Device `d`'s block of a buffer starts at `d * rows * cols` and is row-major
in the block's own shape. `sd_index` says where global element `(i, j)`
sits on device `d`, or `-1` when device `d` does not hold it.

<!-- source: psionic/lib/shard.coax layout -->
```jai
// Where each part of a tensor lives. axis is 0 (rows) or 1 (columns) and
// matters only for a shard.
SdLayout :: struct {kind:SdKind;axis:int;}
SdTensor :: struct {status:SdStatus;rows:int;cols:int;devices:int;layout:SdLayout;}
sd_replicated :: ()->SdLayout {return SdLayout.{kind=SdKind.replicated};}
sd_shard :: (axis:int)->SdLayout {return SdLayout.{kind=SdKind.shard,axis=axis};}
sd_partial :: ()->SdLayout {return SdLayout.{kind=SdKind.partial};}
sd_same :: (a:SdLayout,b:SdLayout)->bool {return a.kind==b.kind && (a.kind!=SdKind.shard || a.axis==b.axis);}
sd_tensor :: (rows:int,cols:int,devices:int,l:SdLayout)->SdTensor #export_c {
    t:=SdTensor.{rows=rows,cols=cols,devices=devices,layout=l};
    if devices<1 || devices>SD_MAX_DEVICES {t.status=SdStatus.devices;}
    else if rows<1 || rows>SD_MAX_EXTENT || cols<1 || cols>SD_MAX_EXTENT {t.status=SdStatus.shape;}
    else if l.kind==SdKind.shard && l.axis!=0 && l.axis!=1 {t.status=SdStatus.axis;}
    return t;
}
// Device d's piece of an extent runs from sd_first(d) up to sd_first(d+1):
// chapter 132's split. With more devices than the extent, some are empty.
sd_first :: (extent:int,devices:int,d:int)->int {return d*extent/devices;}
// Where global element (i, j) sits in device d's block, or -1 when device d
// does not hold it.
sd_index :: (t:SdTensor,d:int,i:int,j:int)->int #export_c {
    base:=d*t.rows*t.cols;
    if t.layout.kind!=SdKind.shard {return base+i*t.cols+j;}
    if t.layout.axis==0 {lo:=sd_first(t.rows,t.devices,d);
        if i<lo || i>=sd_first(t.rows,t.devices,d+1) {return -1;}
        return base+(i-lo)*t.cols+j;}
    lo:=sd_first(t.cols,t.devices,d);hi:=sd_first(t.cols,t.devices,d+1);
    if j<lo || j>=hi {return -1;}
    return base+i*(hi-lo)+(j-lo);
}
```

Work it by hand. Six rows over 4 devices: the piece starts are
`0 * 6 / 4 = 0`, `6 / 4 = 1`, `12 / 4 = 3`, `18 / 4 = 4` and `24 / 4 = 6`,
so device 0 holds row 0, device 1 rows 1 and 2, device 2 row 3, and device
3 rows 4 and 5. With more devices than rows, some pieces are empty: one row
over 2 devices gives device 0 nothing and device 1 row 0.

For a 5 by 3 matrix sharded on rows over 2 devices, device 1 holds rows 2
to 4. Element `(4, 2)` sits at `15 + (4 - 2) * 3 + 2 = 23`: device 1's block
starts at `1 * 5 * 3 = 15`, and the block's rows are 3 values long. Sharded
on columns, device 1 holds columns 1 and 2, its block is 5 by 2, and the
same element sits at `15 + 4 * 2 + (2 - 1) = 24`.

## Place and gather

`sd_place` spreads one device's row-major copy into every device's block.
`sd_gather` brings the blocks back into one copy: a shard's element from
its one holder, a replicated element from device 0, and a partial element
as device 0's addend plus device 1's, and so on in device order. A partial
cannot be placed. Only an operation makes one.

<!-- source: psionic/lib/shard.coax gather -->
```jai
// From one device's row-major copy to every device's block. A partial
// cannot be placed: only an operation makes one.
sd_place :: (t:SdTensor,global:[]f64,local:[]f64)->SdStatus #export_c {
    if t.status!=SdStatus.ok {return t.status;}
    if t.layout.kind==SdKind.partial {return SdStatus.make_partial;}
    if global.count<t.rows*t.cols || !sd_fits(t,local) {return SdStatus.storage;}
    for d:0..<t.devices {for i:0..<t.rows {for j:0..<t.cols {k:=sd_index(t,d,i,j);if k>=0 {local[k]=global[i*t.cols+j];}}}}
    return SdStatus.ok;
}
// Back to one row-major copy. A shard's element comes from its one holder,
// a replicated element from device 0, and a partial element is device 0's
// addend, plus device 1's, and so on in device order.
sd_gather :: (t:SdTensor,local:[]f64,out:[]f64)->SdStatus #export_c {
    if t.status!=SdStatus.ok {return t.status;}
    if out.count<t.rows*t.cols || !sd_fits(t,local) {return SdStatus.storage;}
    for i:0..<t.rows {for j:0..<t.cols {v:=0.0;held:=false;
        for d:0..<t.devices {k:=sd_index(t,d,i,j);
            if k>=0 {if !held {v=local[k];held=true;} else if t.layout.kind==SdKind.partial {v+=local[k];}}}
        out[i*t.cols+j]=v;}}
    return SdStatus.ok;
}
```

The device order matters for a partial. Three addends `1e16`, `1` and `1`
gather to `1e16`: between 2^53 and 2^54 neighbouring binary64 values are 2
apart, so `1e16 + 1` is a tie between `1e16` and `1e16 + 2`, and ties go to
the even neighbour, `1e16`; adding the second `1` ties again. The same
addends in the order `1`, `1`, `1e16` gather to `2 + 1e16 = 1e16 + 2`,
which is exact. A layout that is summed has to fix the order, and this one
fixes device order.

## Redistribution

An operation may need its input in another layout. `sd_redistribute` moves
a tensor into a new layout and counts the values that cross from one device
to another. The collective follows from the two layouts:

| From | To | Collective |
| --- | --- | --- |
| any layout | the same layout | none |
| replicated | a shard | slice (each device keeps its piece) |
| a shard | replicated | all-gather |
| a shard | the other shard | all-to-all |
| partial | a shard | reduce-scatter |
| partial | replicated | all-reduce |
| replicated or a shard | partial | refused |

A partial element is summed in device order on the first device that will
hold it, which receives the other devices' addends; every other holder then
receives the sum. Any other element comes from a device that holds it,
unless the receiving device holds it already. Because the sum is always in
device order, an all-reduce has the same bits as a reduce-scatter followed
by an all-gather.

<!-- source: psionic/lib/shard.coax redistribute -->
```jai
SdMove :: struct {status:SdStatus;collective:SdCollective;moved:int;tensor:SdTensor;}
sd_collective :: (from:SdLayout,to:SdLayout)->SdCollective {
    if sd_same(from,to) {return SdCollective.none;}
    if to.kind==SdKind.partial {return SdCollective.refused;}
    if from.kind==SdKind.replicated {return SdCollective.slice;}
    if from.kind==SdKind.shard && to.kind==SdKind.replicated {return SdCollective.all_gather;}
    if from.kind==SdKind.shard {return SdCollective.all_to_all;}
    if to.kind==SdKind.replicated {return SdCollective.all_reduce;}
    return SdCollective.reduce_scatter;
}
// Moves a tensor into another layout and counts the values that cross from
// one device to another. A partial element is summed in device order on the
// first device that will hold it, which receives the other devices' addends;
// then every other holder receives the sum. Other elements come from a
// device that holds them, unless the receiver holds one already.
sd_redistribute :: (t:SdTensor,local:[]f64,to:SdLayout,out:[]f64)->SdMove #export_c {
    u:=sd_tensor(t.rows,t.cols,t.devices,to);m:=SdMove.{status=t.status,collective=sd_collective(t.layout,to),tensor=u};
    if t.status!=SdStatus.ok {return m;}
    if u.status!=SdStatus.ok {m.status=u.status;return m;}
    if m.collective==SdCollective.refused {m.status=SdStatus.make_partial;return m;}
    if !sd_fits(t,local) || !sd_fits(t,out) {m.status=SdStatus.storage;return m;}
    if m.collective==SdCollective.none {for k:0..<t.devices*t.rows*t.cols {out[k]=local[k];}return m;}
    partial:=t.layout.kind==SdKind.partial;
    for i:0..<t.rows {for j:0..<t.cols {v:=0.0;held:=false;at:=-1;
        for d:0..<t.devices {k:=sd_index(t,d,i,j);if k>=0 {if !held {v=local[k];held=true;} else if partial {v+=local[k];}}}
        for d:0..<t.devices {k:=sd_index(u,d,i,j);if k<0 {continue;}
            out[k]=v;
            if partial && at<0 {at=d;m.moved+=t.devices-1;}
            else if partial || sd_index(t,d,i,j)<0 {m.moved+=1;}}}}
    return m;
}
```

Trace a 2 by 3 matrix `[1 2 3; 4 5 6]` over 2 devices from rows to
columns. Device 0 holds row 0 and needs column 0; device 1 holds row 1 and
needs columns 1 and 2. Device 0 already has `(0, 0)` and receives `(1, 0)`
= 4. Device 1 already has `(1, 1)` and `(1, 2)` and receives `(0, 1)` = 2
and `(0, 2)` = 3. Three values move: 24 bytes. Device 0's new block is
`[1; 4]` and device 1's is `[2 3; 5 6]`.

## What a redistribution costs

`sd_cost` gives the bytes a redistribution moves in closed form, so a plan
can be priced before anything runs. The checks compare it with what
`sd_redistribute` counts for every pair of layouts.

<!-- source: psionic/lib/shard.coax cost -->
```jai
// The bytes (8 a value) one redistribution moves between devices, in closed
// form; -1 for a move into a partial.
//   all-gather:     each device receives what it lacks: (D - 1) * n values.
//   reduce-scatter: each value's holder receives D - 1 addends: (D - 1) * n.
//   all-reduce:     a reduce-scatter, then an all-gather: 2 * (D - 1) * n.
//   all-to-all:     everything except what each device keeps, the cells in
//                   both its row piece and its column piece.
sd_cost :: (rows:int,cols:int,devices:int,from:SdLayout,to:SdLayout)->int #export_c {
    n:=rows*cols;c:=sd_collective(from,to);
    if c==SdCollective.refused {return -1;}
    if c==SdCollective.all_gather || c==SdCollective.reduce_scatter {return 8*(devices-1)*n;}
    if c==SdCollective.all_reduce {return 16*(devices-1)*n;}
    if c!=SdCollective.all_to_all {return 0;}
    kept:=0;
    for d:0..<devices {kept+=(sd_first(rows,devices,d+1)-sd_first(rows,devices,d))*(sd_first(cols,devices,d+1)-sd_first(cols,devices,d));}
    return 8*(n-kept);
}
```

Work it for a 6 by 4 matrix (24 values) over 4 devices:

- **All-gather** (rows or columns to replicated): each device lacks
  `24 - its piece`, and the pieces sum to 24, so `4 * 24 - 24 = 3 * 24 = 72`
  values, 576 bytes.
- **All-to-all** (rows to columns): the row pieces are 1, 2, 1 and 2 rows
  and the column pieces 1 column each, so device `d` keeps 1, 2, 1 and 2
  values. `24 - 6 = 18` values move, 144 bytes. Columns to rows is the same.
- **Reduce-scatter** (partial to a shard): each value's holder receives 3
  addends, 72 values, 576 bytes.
- **All-reduce** (partial to replicated): 72 for the reduce-scatter and 72
  for the all-gather, 1152 bytes.

A slice and a move to the same layout cost nothing; a move into a partial
is refused, and costs `-1`.

## The layout of a product

For `c = a @ b`, the contraction axis is `a`'s columns and `b`'s rows. The
rule table:

| `a` \ `b` | replicated | rows | columns | partial |
| --- | --- | --- | --- | --- |
| replicated | replicated | refused (contraction) | columns (column-parallel) | partial |
| rows | rows (data-parallel) | refused (contraction) | refused (2-D mesh) | refused (2-D mesh) |
| columns | refused (contraction) | partial (row-parallel) | refused (contraction) | refused (contraction) |
| partial | partial | refused (contraction) | refused (2-D mesh) | refused (both partial) |

- If the contraction axis is split on both operands (`a` on columns, `b`
  on rows), each device sums its own run of the contraction, and the result
  is partial. This is a row-parallel linear layer.
- If it is split on one operand only, the device holding part of it lacks
  the other operand's matching part, so the product is refused until one
  operand is redistributed.
- `a` partial and `b` partial would need the cross terms `a_0 b_1`, which no
  device has, so that is refused too.
- Otherwise a replicated operand takes the other's layout: `b` on columns
  gives a result on columns (column-parallel), `a` on rows a result on rows
  (data-parallel), and a partial times a replicated factor stays partial.
  Two split operands would split the result two ways, which needs a 2-D
  mesh, and this chapter's group is one axis.

Elementwise add needs both operands in the same layout and keeps it; two
partials add addend by addend. Relu keeps any layout except partial:
`relu(a_0) + relu(a_1)` is not `relu(a_0 + a_1)`.

<!-- source: psionic/lib/shard.coax rules -->
```jai
SdRule :: struct {status:SdStatus;layout:SdLayout;}
// The layout of a @ b, or why there is none. The contraction axis is a's
// columns and b's rows.
sd_matmul_rule :: (a:SdLayout,b:SdLayout)->SdRule #export_c {
    ak:=a.kind==SdKind.shard && a.axis==1;bk:=b.kind==SdKind.shard && b.axis==0;
    // Row-parallel: each device's run of the contraction makes an addend.
    if ak && bk {return SdRule.{layout=sd_partial()};}
    if ak || bk {return SdRule.{status=SdStatus.contract};}
    if a.kind==SdKind.partial && b.kind==SdKind.partial {return SdRule.{status=SdStatus.cross};}
    // Column-parallel (b on columns), data-parallel (a on rows), a partial
    // times a replicated factor, or both replicated.
    if a.kind==SdKind.replicated {return SdRule.{layout=b};}
    if b.kind==SdKind.replicated {return SdRule.{layout=a};}
    return SdRule.{status=SdStatus.mesh};
}
sd_add_rule :: (a:SdLayout,b:SdLayout)->SdRule {
    if !sd_same(a,b) {return SdRule.{status=SdStatus.layouts};}
    return SdRule.{layout=a};
}
sd_relu_rule :: (a:SdLayout)->SdRule {
    if a.kind==SdKind.partial {return SdRule.{status=SdStatus.nonlinear};}
    return SdRule.{layout=a};
}
// The output tensor of each operation, from its inputs' tensors alone.
sd_matmul_out :: (a:SdTensor,b:SdTensor)->SdTensor {
    if a.status!=SdStatus.ok {return a;}
    if b.status!=SdStatus.ok {return b;}
    if a.devices!=b.devices || a.cols!=b.rows {return SdTensor.{status=SdStatus.mismatch};}
    r:=sd_matmul_rule(a.layout,b.layout);c:=sd_tensor(a.rows,b.cols,a.devices,r.layout);c.status=r.status;return c;
}
sd_add_out :: (a:SdTensor,b:SdTensor)->SdTensor {
    if a.status!=SdStatus.ok {return a;}
    if b.status!=SdStatus.ok {return b;}
    if a.devices!=b.devices || a.rows!=b.rows || a.cols!=b.cols {return SdTensor.{status=SdStatus.mismatch};}
    c:=a;c.status=sd_add_rule(a.layout,b.layout).status;return c;
}
sd_relu_out :: (a:SdTensor)->SdTensor {c:=a;if a.status==SdStatus.ok {c.status=sd_relu_rule(a.layout).status;}return c;}
```

## Running the operations

`sd_matmul` computes, on each device, the elements its block of the result
holds: over the whole contraction, or over its own run of it when the
contraction is split. `sd_add` and `sd_relu` work block by block.
`sd_matmul_one` is the one-device product the checks compare with: each
element sums `a[i][p] * b[p][j]` from `p = 0` up.

<!-- source: psionic/lib/shard.coax ops -->
```jai
// The one-device product: c[i][j] sums a[i][p] * b[p][j] from p = 0 up.
sd_matmul_one :: (m:int,k:int,n:int,a:[]f64,b:[]f64,c:[]f64) {
    for i:0..<m {for j:0..<n {acc:=0.0;for p:0..<k {acc+=a[i*k+p]*b[p*n+j];}c[i*n+j]=acc;}}
}
// Every device computes the elements its block of c holds, over the whole
// contraction, or over its own run of it when the contraction is split.
sd_matmul :: (a:SdTensor,la:[]f64,b:SdTensor,lb:[]f64,lc:[]f64)->SdTensor #export_c {
    c:=sd_matmul_out(a,b);
    if c.status!=SdStatus.ok {return c;}
    if !sd_fits(a,la) || !sd_fits(b,lb) || !sd_fits(c,lc) {c.status=SdStatus.storage;return c;}
    split:=a.layout.kind==SdKind.shard && a.layout.axis==1;
    for d:0..<c.devices {for i:0..<c.rows {for j:0..<c.cols {k:=sd_index(c,d,i,j);if k<0 {continue;}
        lo:=0;hi:=a.cols;if split {lo=sd_first(a.cols,a.devices,d);hi=sd_first(a.cols,a.devices,d+1);}
        acc:=0.0;for p:lo..<hi {acc+=la[sd_index(a,d,i,p)]*lb[sd_index(b,d,p,j)];}
        lc[k]=acc;}}}
    return c;
}
sd_add :: (a:SdTensor,la:[]f64,b:SdTensor,lb:[]f64,lc:[]f64)->SdTensor {
    c:=sd_add_out(a,b);
    if c.status!=SdStatus.ok {return c;}
    if !sd_fits(a,la) || !sd_fits(b,lb) || !sd_fits(c,lc) {c.status=SdStatus.storage;return c;}
    for d:0..<c.devices {for i:0..<c.rows {for j:0..<c.cols {k:=sd_index(c,d,i,j);if k>=0 {lc[k]=la[k]+lb[k];}}}}
    return c;
}
sd_relu :: (a:SdTensor,la:[]f64,lc:[]f64)->SdTensor {
    c:=sd_relu_out(a);
    if c.status!=SdStatus.ok {return c;}
    if !sd_fits(a,la) || !sd_fits(c,lc) {c.status=SdStatus.storage;return c;}
    for d:0..<c.devices {for i:0..<c.rows {for j:0..<c.cols {k:=sd_index(c,d,i,j);if k>=0 {lc[k]=0;if la[k]>0 {lc[k]=la[k];}}}}}
    return c;
}
```

By hand: `x = [1 2]` replicated over 3 devices times
`w1 = [1 0 2; 0 1 1]` sharded on columns, one column a device. Device 0
computes `1 * 1 + 2 * 0 = 1`, device 1 `1 * 0 + 2 * 1 = 2`, and device 2
`1 * 2 + 2 * 1 = 4`. Gathered, `h = [1 2 4]`, the one-device product.

A column-parallel or data-parallel element is summed over the whole
contraction in the same order as on one device, so it has the same bits at
every device count. A row-parallel element is not. Take
`[1e16 1 -1e16 1] @ [1 1 1 1]^T`:

- **One device**: `0 + 1e16 = 1e16`; `+ 1` ties and rounds to `1e16`;
  `- 1e16 = 0`; `+ 1 = 1`.
- **Two devices**: device 0 sums `1e16 + 1`, which rounds to `1e16`; device
  1 sums `-1e16 + 1`, which rounds to `-1e16`; the partial gathers to `0`.
- **Four devices**: the addends are exact, `1e16`, `1`, `-1e16` and `1`,
  and they gather in device order to the one-device sum, `1`.

The exact answer is 2, so no order is exact. What changes with the device
count is where the partial sums end: the row-parallel result has the bits
of a blocked sum, in which each device sums its run from zero and the runs
are then added in device order. The checks compare it with that blocked sum
bit for bit and with the one-device sum within `1e-14`. With small integers
every sum is exact, and every legal pair of layouts gives the one-device
bits.

## Propagation

A program is a list of nodes. An input names its shape and layout;
`matmul` and `add` use two earlier nodes, `relu` one, and `move`
redistributes one into a new layout. `sd_propagate` gives every node's
shape and layout, from the inputs' layouts alone, with the same rules the
operations use. It stops at the first refused node and names it.

<!-- source: psionic/lib/shard.coax propagate -->
```jai
// A program of operations on sharded tensors. An input names its shape and
// layout; matmul and add use nodes a and b; relu uses a; move redistributes
// a into layout.
SdNode :: struct {op:SdOp;a:int;b:int;rows:int;cols:int;layout:SdLayout;}
SdCheck :: struct {status:SdStatus;node:int=-1;}
// Gives every node's tensor, or the first node refused and why.
sd_propagate :: (devices:int,nodes:[]SdNode,out:[]SdTensor)->SdCheck #export_c {
    if out.count<nodes.count {return SdCheck.{status=SdStatus.storage};}
    for x,n:nodes {t:SdTensor;
        if x.op==SdOp.input {t=sd_tensor(x.rows,x.cols,devices,x.layout);}
        else if x.a<0 || x.a>=n || (x.op==SdOp.matmul || x.op==SdOp.add) && (x.b<0 || x.b>=n) {t.status=SdStatus.order;}
        else if x.op==SdOp.matmul {t=sd_matmul_out(out[x.a],out[x.b]);}
        else if x.op==SdOp.add {t=sd_add_out(out[x.a],out[x.b]);}
        else if x.op==SdOp.relu {t=sd_relu_out(out[x.a]);}
        else {a:=out[x.a];t=sd_tensor(a.rows,a.cols,devices,x.layout);if t.status==SdStatus.ok && sd_collective(a.layout,x.layout)==SdCollective.refused {t.status=SdStatus.make_partial;}}
        if t.status!=SdStatus.ok {return SdCheck.{status=t.status,node=n};}
        out[n]=t;}
    return SdCheck.{};
}
```

## The program

The example, [`psionic/examples/shard.coax`](../examples/shard.coax),
splits a 6 by 4 matrix over 4 devices, runs every redistribution between
replicated, rows and columns, and then runs `y = relu(x w1) w2` the way a
tensor-parallel MLP does: `x` replicated, `w1` sharded on columns, `w2` on
rows, one all-reduce at the end. It compares the result with one device at
1 to 4 devices, and ends with the row-parallel trace above.

<!-- source: psionic/examples/shard.coax example -->
```jai
// Chapter 133: one matrix in every layout over a group of logical devices,
// every redistribution between them, and a two-layer network run
// column-parallel then row-parallel, compared with one device.
value :: (i:int,seed:int)->f64 {return f64((i*37+seed*11)%17)/10.0-1.0;}
main :: () {
    l:=[4]SdLayout.[sd_replicated(),sd_shard(0),sd_shard(1),sd_partial()];
    print("a 6 x 4 matrix over 4 devices:\n");
    for k:1..<3 {t:=sd_tensor(6,4,4,l[k]);print("  %:",sd_name(l[k]));
        for d:0..<4 {print(" device % holds % to %;",d,sd_first(sd_extent(t,k-1),4,d),sd_first(sd_extent(t,k-1),4,d+1)-1);}print("\n");}

    print("every redistribution of it, 8 bytes a value:\n");
    g:[24]f64;for i:0..<24 {g[i]=value(i,1);}
    for f:0..<3 {src:=sd_tensor(6,4,4,l[f]);a:[96]f64;#assert sd_place(src,g[..],a[..])==SdStatus.ok;
        for t:0..<3 {b:[96]f64;back:[96]f64;out:[24]f64;m:=sd_redistribute(src,a[..],l[t],b[..]);#assert sd_gather(m.tensor,b[..],out[..])==SdStatus.ok;
            r:=sd_redistribute(m.tensor,b[..],l[f],back[..]);
            print("  % to %: %, % values moved, % bytes (sd_cost %), gathers to the matrix and comes back: %\n",
                sd_name(l[f]),sd_name(l[t]),sd_collective_name(m.collective),m.moved,8*m.moved,sd_cost(6,4,4,l[f],l[t]),sd_equal(back[..],a[..]) && sd_equal(out[..],g[..]));}}

    // y = relu(x w1) w2: x is 2 x 4, w1 4 x 6, w2 6 x 3.
    x:[8]f64;w1:[24]f64;w2:[18]f64;for i:0..<8 {x[i]=value(i,2);}for i:0..<24 {w1[i]=value(i,3);}for i:0..<18 {w2[i]=value(i,4);}
    h:[12]f64;y:[6]f64;sd_matmul_one(2,4,6,x[..],w1[..],h[..]);for i:0..<12 {if !(h[i]>0) {h[i]=0;}}sd_matmul_one(2,6,3,h[..],w2[..],y[..]);
    nodes:=[7]SdNode.[SdNode.{op=SdOp.input,rows=2,cols=4,layout=sd_replicated()},SdNode.{op=SdOp.input,rows=4,cols=6,layout=sd_shard(1)},
        SdNode.{op=SdOp.matmul,a=0,b=1},SdNode.{op=SdOp.relu,a=2},SdNode.{op=SdOp.input,rows=6,cols=3,layout=sd_shard(0)},
        SdNode.{op=SdOp.matmul,a=3,b=4},SdNode.{op=SdOp.move,a=5,layout=sd_replicated()}];
    tensors:[7]SdTensor;c:=sd_propagate(4,nodes[..],tensors[..]);
    print("y = relu(x w1) w2 over 4 devices: %\n",sd_why(c.status));
    names:=[7]string.["x","w1","x w1","relu","w2","relu w2","y"];
    for n:0..<7 {print("  node % (%): % x %, %\n",n,names[n],tensors[n].rows,tensors[n].cols,sd_name(tensors[n].layout));}
    print("  one device: y = % % % / % % %\n",y[0],y[1],y[2],y[3],y[4],y[5]);
    for d:1..<5 {X:=sd_tensor(2,4,d,sd_replicated());W1:=sd_tensor(4,6,d,sd_shard(1));W2:=sd_tensor(6,3,d,sd_shard(0));
        lx:[32]f64;lw1:[96]f64;lw2:[72]f64;lh:[48]f64;lr:[48]f64;lp:[24]f64;ly:[24]f64;gh:[12]f64;gy:[6]f64;
        #assert sd_place(X,x[..],lx[..])==SdStatus.ok;#assert sd_place(W1,w1[..],lw1[..])==SdStatus.ok;#assert sd_place(W2,w2[..],lw2[..])==SdStatus.ok;
        H:=sd_matmul(X,lx[..],W1,lw1[..],lh[..]);Rl:=sd_relu(H,lh[..],lr[..]);P:=sd_matmul(Rl,lr[..],W2,lw2[..],lp[..]);
        m:=sd_redistribute(P,lp[..],sd_replicated(),ly[..]);#assert sd_gather(Rl,lr[..],gh[..])==SdStatus.ok;#assert sd_gather(m.tensor,ly[..],gy[..])==SdStatus.ok;
        far:=0.0;differ:=0;for i:0..<6 {if f64_bits(gy[i])!=f64_bits(y[i]) {differ+=1;}gap:=gy[i]-y[i];if gap<0 {gap=-gap;}if gap>far {far=gap;}}
        print("  % devices: hidden layer equal bit for bit: %; all-reduce moved % values; y differs in % of 6 values, largest gap %\n",d,sd_equal(gh[..],h[..]),m.moved,differ,far);}

    print("row-parallel [1e16 1 -1e16 1] . [1 1 1 1], one device: ");
    a:=[4]f64.[1e16,1,-1e16,1];ones:=[4]f64.[1,1,1,1];one:[1]f64;sd_matmul_one(1,4,1,a[..],ones[..],one[..]);print("%\n",one[0]);
    for d:[3]int.[1,2,4] {A:=sd_tensor(1,4,d,sd_shard(1));B:=sd_tensor(4,1,d,sd_shard(0));la:[16]f64;lb:[16]f64;lc:[4]f64;
        #assert sd_place(A,a[..],la[..])==SdStatus.ok;#assert sd_place(B,ones[..],lb[..])==SdStatus.ok;C:=sd_matmul(A,la[..],B,lb[..],lc[..]);#assert sd_gather(C,lc[..],one[..])==SdStatus.ok;
        print("  % devices: partial sums",d);for k:0..<d {print(" %",lc[k]);}print(", their sum %\n",one[0]);}
}
```

```text
a 6 x 4 matrix over 4 devices:
  sharded on rows: device 0 holds 0 to 0; device 1 holds 1 to 2; device 2 holds 3 to 3; device 3 holds 4 to 5;
  sharded on columns: device 0 holds 0 to 0; device 1 holds 1 to 1; device 2 holds 2 to 2; device 3 holds 3 to 3;
every redistribution of it, 8 bytes a value:
  replicated to replicated: none, 0 values moved, 0 bytes (sd_cost 0), gathers to the matrix and comes back: true
  replicated to sharded on rows: slice, 0 values moved, 0 bytes (sd_cost 0), gathers to the matrix and comes back: true
  replicated to sharded on columns: slice, 0 values moved, 0 bytes (sd_cost 0), gathers to the matrix and comes back: true
  sharded on rows to replicated: all-gather, 72 values moved, 576 bytes (sd_cost 576), gathers to the matrix and comes back: true
  sharded on rows to sharded on rows: none, 0 values moved, 0 bytes (sd_cost 0), gathers to the matrix and comes back: true
  sharded on rows to sharded on columns: all-to-all, 18 values moved, 144 bytes (sd_cost 144), gathers to the matrix and comes back: true
  sharded on columns to replicated: all-gather, 72 values moved, 576 bytes (sd_cost 576), gathers to the matrix and comes back: true
  sharded on columns to sharded on rows: all-to-all, 18 values moved, 144 bytes (sd_cost 144), gathers to the matrix and comes back: true
  sharded on columns to sharded on columns: none, 0 values moved, 0 bytes (sd_cost 0), gathers to the matrix and comes back: true
y = relu(x w1) w2 over 4 devices: ok
  node 0 (x): 2 x 4, replicated
  node 1 (w1): 4 x 6, sharded on columns
  node 2 (x w1): 2 x 6, sharded on columns
  node 3 (relu): 2 x 6, sharded on columns
  node 4 (w2): 6 x 3, sharded on rows
  node 5 (relu w2): 2 x 3, partial
  node 6 (y): 2 x 3, replicated
  one device: y = -0.29400000000000004 -0.06600000000000009 -0.3819999999999999 / -1.354 -0.36400000000000005 -0.938
  1 devices: hidden layer equal bit for bit: true; all-reduce moved 0 values; y differs in 0 of 6 values, largest gap 0.0
  2 devices: hidden layer equal bit for bit: true; all-reduce moved 12 values; y differs in 0 of 6 values, largest gap 0.0
  3 devices: hidden layer equal bit for bit: true; all-reduce moved 24 values; y differs in 1 of 6 values, largest gap 5.551115123125783e-17
  4 devices: hidden layer equal bit for bit: true; all-reduce moved 36 values; y differs in 1 of 6 values, largest gap 5.551115123125783e-17
row-parallel [1e16 1 -1e16 1] . [1 1 1 1], one device: 1.0
  1 devices: partial sums 1.0, their sum 1.0
  2 devices: partial sums 10000000000000000.0 -10000000000000000.0, their sum 0.0
  4 devices: partial sums 10000000000000000.0 1.0 -10000000000000000.0 1.0, their sum 1.0
```

The redistributions move what the hand count says: 72 values for an
all-gather and 18 for an all-to-all. In the network, the hidden layer is
column-parallel and has the one-device bits at every device count. The
all-reduce of the 2 by 3 partial moves `2 * (d - 1) * 6` values: 12, 24 and
36. The output is row-parallel: at 1 and 2 devices it happens to equal one
device's bits; at 3 and 4 devices one of its six values differs in the last
bit, by `5.55e-17`.

## Refusals

[`psionic/examples/shard-refuse.coax`](../examples/shard-refuse.coax) shows
the tensors, redistributions, products and programs that are refused.

<!-- source: psionic/examples/shard-refuse.coax refuse -->
```jai
// Chapter 133: tensors, redistributions, operations and programs that are
// refused, each with its reason. Nothing is computed for a refused one.
main :: () {
    print("9 devices: %\n",sd_why(sd_tensor(4,4,9,sd_replicated()).status));
    print("65 rows: %\n",sd_why(sd_tensor(65,4,2,sd_replicated()).status));
    print("a shard on axis 2: %\n",sd_why(sd_tensor(4,4,2,sd_shard(2)).status));
    g:=[16]f64.[1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16];R:=sd_tensor(4,4,2,sd_replicated());lr:[32]f64;out:[32]f64;
    #assert sd_place(R,g[..],lr[..])==SdStatus.ok;
    m:=sd_redistribute(R,lr[..],sd_partial(),out[..]);print("replicated to partial: %\n",sd_why(m.status));
    m=sd_redistribute(R,lr[..],sd_shard(0),out[0..<31]);print("into a 31-value buffer: %\n",sd_why(m.status));
    l:=[4]SdLayout.[sd_replicated(),sd_shard(0),sd_shard(1),sd_partial()];
    for a:0..<4 {for b:0..<4 {r:=sd_matmul_rule(l[a],l[b]);if r.status!=SdStatus.ok {print("% @ %: %\n",sd_name(l[a]),sd_name(l[b]),sd_why(r.status));}}}
    lc:[32]f64;print("a 4 x 4 @ a 2 x 4: %\n",sd_why(sd_matmul(R,lr[..],sd_tensor(2,4,2,sd_replicated()),lr[..],lc[..]).status));
    print("relu of a partial: %\n",sd_why(sd_relu(sd_tensor(4,4,2,sd_partial()),lr[..],lc[..]).status));
    print("replicated + sharded on rows: %\n",sd_why(sd_add(R,lr[..],sd_tensor(4,4,2,sd_shard(0)),lr[..],lc[..]).status));

    // The two-layer network with relu moved after the row-parallel product, and with a node that uses itself.
    nodes:=[7]SdNode.[SdNode.{op=SdOp.input,rows=2,cols=4,layout=sd_replicated()},SdNode.{op=SdOp.input,rows=4,cols=6,layout=sd_shard(1)},
        SdNode.{op=SdOp.matmul,a=0,b=1},SdNode.{op=SdOp.input,rows=6,cols=3,layout=sd_shard(0)},
        SdNode.{op=SdOp.matmul,a=2,b=3},SdNode.{op=SdOp.relu,a=4},SdNode.{op=SdOp.move,a=5,layout=sd_replicated()}];
    tensors:[7]SdTensor;c:=sd_propagate(4,nodes[..],tensors[..]);print("relu(x w1 w2): node %: %\n",c.node,sd_why(c.status));
    nodes[5]=SdNode.{op=SdOp.move,a=4,layout=sd_replicated()};nodes[6]=SdNode.{op=SdOp.relu,a=5};
    c=sd_propagate(4,nodes[..],tensors[..]);print("relu after the all-reduce: %, y %\n",sd_why(c.status),sd_name(tensors[6].layout));
    nodes[2].b=2;c=sd_propagate(4,nodes[..],tensors[..]);print("a product that uses its own output: node %: %\n",c.node,sd_why(c.status));
}
```

```text
9 devices: refused: from 1 to 8 devices
65 rows: refused: from 1 to 64 rows and columns
a shard on axis 2: refused: a shard axis is 0 (rows) or 1 (columns)
replicated to partial: refused: only an operation makes a partial sum
into a 31-value buffer: refused: a buffer is too small for every device's block
replicated @ sharded on rows: refused: the contraction axis is split on one operand only; redistribute one first
sharded on rows @ sharded on rows: refused: the contraction axis is split on one operand only; redistribute one first
sharded on rows @ sharded on columns: refused: the result would be split two ways, which needs a 2-D mesh
sharded on rows @ partial: refused: the result would be split two ways, which needs a 2-D mesh
sharded on columns @ replicated: refused: the contraction axis is split on one operand only; redistribute one first
sharded on columns @ sharded on columns: refused: the contraction axis is split on one operand only; redistribute one first
sharded on columns @ partial: refused: the contraction axis is split on one operand only; redistribute one first
partial @ sharded on rows: refused: the contraction axis is split on one operand only; redistribute one first
partial @ sharded on columns: refused: the result would be split two ways, which needs a 2-D mesh
partial @ partial: refused: both operands are partial sums, so the cross terms are missing
a 4 x 4 @ a 2 x 4: refused: the shapes or device counts do not fit
relu of a partial: refused: a partial sum through a non-linear operation; all-reduce it first
replicated + sharded on rows: refused: the operands have different layouts; redistribute one first
relu(x w1 w2): node 5: refused: a partial sum through a non-linear operation; all-reduce it first
relu after the all-reduce: ok, y replicated
a product that uses its own output: node 2: refused: a node uses a node that is not before it
```

Every refusal comes before any value is written. The program exits with
status 0; the refusals are its output.

## How it is checked

`psionic/lib/shard.coax` (`#authority`) has its checks before the code:

- layouts, pieces, block positions and every refusal by hand, with every
  diagnostic's exact text;
- for every shape up to 6 by 6 at 1 to 4 devices and every layout: each
  element has exactly one home under a shard and one on every device
  otherwise, and the blocks do not overlap;
- place and gather by hand, including the order of a partial's addends;
- redistributions and costs by hand, including the 6 by 4 table above;
- for every shape up to 5 by 5 at 1 to 4 devices and every pair of
  layouts: the redistribution gathers to the same value bit for bit and
  moves exactly the bytes `sd_cost` gives; unless it started from a
  partial, moving back gives the starting blocks bit for bit; a move into
  a partial is refused; an all-reduce equals a reduce-scatter then an
  all-gather bit for bit;
- the full product rule table, and add and relu's rules;
- column- and row-parallel by hand, including the `1e16` trace at 1, 2 and
  4 devices;
- for every product up to 3 by 5 by 3 at 1 to 4 devices and every legal
  pair of layouts: with small integers, the one-device bits; with other
  values, the one-device bits unless the contraction is split, and then an
  independently written blocked sum bit for bit and the one-device sum
  within `1e-14`;
- propagation through the two-layer network and a refusal at each node.

```shell
coax/bin/coax test psionic/lib/shard.coax
coax/bin/coax test psionic/lib/shard.coax --release
coax/bin/coax mutate psionic/lib/shard.coax --jobs 8
coax/bin/coax run psionic/examples/shard.coax
coax/bin/coax run psionic/examples/shard-refuse.coax
```

The mutation run kills all 450 mutants. Both examples give the same
transcript in debug and release.

## What remains

- **A named mesh.** The group here is one axis. A 2-D mesh (say data by
  tensor) would accept the products this chapter refuses as needing one.
- **The tensor IR.** The layouts are carried by this chapter's node list,
  not by [chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md)'s
  tensor IR and its execution plans.
- **Real devices.** The blocks live in one CPU buffer. Moving them through
  [chapter 6](ch006-one-tensor-several-devices-and-dtypes.md)'s owned
  device storage, on chapter 132's threads or a GPU, is not done.
- **More operations.** Only the product, add and relu have rules.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"
- [Chapter 132](ch132-train-across-devices.md): the split rule and the
  fixed-order all-reduce.
