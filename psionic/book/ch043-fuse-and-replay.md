# 43. Fuse and replay

> **Status: partly implemented.** A fusion pass over two elementwise chains
> (the RMS norm with its scale, and a bias, ReLU and residual add), its
> overlap and liveness checks, and record-once replay of decode and prefill
> steps run here, on the CPU and on an RTX 4080. Gate and up projections
> with their activation, expert routing, a recurrent step with its cache
> write, replay transaction boundaries and any model use are not built yet;
> the last section lists them.

Each device launch has a fixed cost on the host, about a microsecond on this
machine, whatever arithmetic the kernel does. A decode step for one token
does little arithmetic per launch, so a step made of many small launches can
spend as long submitting work as doing it. There are two remedies. Fusion
merges several kernels into one, so the step has fewer launches and its
intermediates never go to device memory. Replay records a step's launches
once as a device graph and submits the whole graph with one call.

Fusion is only safe under conditions the unfused program does not need. This
chapter writes a fusion pass that states those conditions in Coax, refuses
every chain that breaks one, and checks that each fused step writes exactly
the bits of the unfused step, on the CPU and on the GPU. It then records the
fused step and replays it with new arguments for decode and prefill chunks of
different sizes.

## A step as launches over spans

A step works on `rows` rows of `cols` values: one row per token, so a decode
step has one row and a prefill chunk has many. Five kernels make the step:

1. `rms`: the inverse RMS of each row, `1 / sqrt(mean of squares + eps)`.
2. `scale`: `y = x * inv[row] * gain[col]`.
3. `bias`: `h = y + bias[col]`.
4. `relu`: `r = max(h, 0)`.
5. `add`: `out = r + residual`.

The plan does not see arrays. It sees spans: a buffer number, a first element
and a count. Each launch names the spans it reads and the span it writes, and
whether its output is needed after the step:

<!-- source: psionic/lib/device/fuse.coax types -->
```jai
// count f32 values of buffer `buffer`, from element `offset`.
FuSpan :: struct {buffer:int;offset:int;count:int;}
// One launch of kernel `kind` (the numbers in fuse-kernels.coax), reading
// the spans a, b and c it uses and writing out. keep: out is read after the
// step, so it may not be removed.
FuOp :: struct {kind:int;a:FuSpan;b:FuSpan;c:FuSpan;out:FuSpan;keep:bool;}
FU_RMS :: 0;FU_SCALE :: 1;FU_BIAS :: 2;FU_RELU :: 3;FU_ADD :: 4;
FU_NORM_SCALE :: 5;FU_BIAS_RELU_ADD :: 6;
FU_LIMIT :: 65536;
FuStatus :: enum u8 {ok;shape;kind;span;alias;capacity;}
FuWhy :: enum u8 {fused;chain;overlap;live;}
// One merge the pass considered: the ops first ..< first+ops, the fused
// kernel it would become, and, when it was refused, the two spans that
// decided it.
FuDecision :: struct {first:int;ops:int;fused:int;why:FuWhy;p:FuSpan;q:FuSpan;}
FuPlan :: struct {status:FuStatus;op:int;launches:int;decisions:int;}
```

The chapter's step puts x and the residual in buffer 0, the gain and bias in
buffer 1, the inverse RMS in buffer 2, and y, h, r and the output one after
another in buffer 3. Only the output is kept.

## The step by hand

Take two rows of four values, `eps` = 0 so every value is exact:

| | Row 0 | Row 1 |
| --- | --- | --- |
| x | 2, 2, 2, 2 | 1, −1, 1, −1 |
| mean square | 16 / 4 = 4 | 4 / 4 = 1 |
| inverse RMS | 1/2 | 1 |
| y = x · inv · gain (gain 1, 2, 0.5, −1) | 1, 2, 0.5, −1 | 1, −2, 0.5, 1 |
| h = y + bias (bias 0.5, 0.5, −1, 0) | 1.5, 2.5, −0.5, −1 | 1.5, −1.5, −0.5, 1 |
| r = max(h, 0) | 1.5, 2.5, 0, 0 | 1.5, 0, 0, 1 |
| out = r + residual (all 1) | 2.5, 3.5, 1, 1 | 2.5, 1, 1, 2 |

Unfused, this is five launches, and y, h, r and the inverse RMS all go to
device memory between them. Fused, the norm and scale become one launch that
keeps the inverse RMS in registers, and bias, ReLU and add become one launch
that reads y and writes the output. Two launches, and h, r and the inverse
RMS are never written.

## The kernels

Every kernel has the same parameters, so one launch descriptor fits all of
them. The norms are group kernels in the style of
[chapter 68](ch068-coax-kernels-cooperate.md): one group of 128 threads per
row, each thread adding the squares of every 128th column, then a tree over
the 128 partial sums. The other kernels give one element to each work item.

<!-- source: psionic/lib/device/fuse-kernels.coax unfused -->
```jai
// The norms are group kernels (chapter 68): one group of 128 threads per
// row. Thread t adds the squares of columns t, t+128, t+256, ... in that
// order; then seven phases add the 128 partial sums as a tree, the first
// half adding the element half places on, as dc_tile_sum does. The RMS norm
// and the fused norm both reduce this way, so they agree bit for bit.
FK_THREADS :: 128;
// Columns thread t visits: t, t+128, ... below cols (none when t >= cols).
fk_count :: (thread:int,cols:int)->int {return (cols-thread+FK_THREADS-1)/FK_THREADS;}
fk_live :: (thread:int,half:int)->bool {return thread<half;}
// 1 / sqrt(mean square + eps), from the row's sum of squares.
fk_inv :: (sum:f32,cols:int,eps:f32)->f32 {return 1/sqrt(sum/f32(cols)+eps);}
// max(v, 0), with -0 going to +0.
fk_relu1 :: (v:f32)->f32 {if v>0 {return v;}return 0;}

// 0: item = row. out[row] = the inverse RMS of row `row` of a.
fk_rms :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    p:[128]f32;q:[128]f32;
    for thread:0..<FK_THREADS {
        s:f32=0;
        for k:0..<fk_count(thread,cols) {v:=a[item*cols+thread+k*FK_THREADS];s+=v*v;}
        p[thread]=s;
    }
    for step:0..<7 {
        half:=1<<(6-step);
        if step%2==0 {
            for thread:0..<FK_THREADS {s:=p[thread];if fk_live(thread,half) {q[thread]=s+p[thread+half];}}
        } else {
            for thread:0..<FK_THREADS {s:=q[thread];if fk_live(thread,half) {p[thread]=s+q[thread+half];}}
        }
    }
    for thread:0..<FK_THREADS {s:=q[thread];if thread==0 {out[item]=fk_inv(s,cols,eps);}}
}
// 1: item = element. out = a * b[row] * c[col]: the row's inverse RMS, then
// the column's gain.
fk_scale :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    out[item]=a[item]*b[item/cols]*c[item%cols];
}
// 2: out = a + b[col], the bias of the element's column.
fk_bias :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    out[item]=a[item]+b[item%cols];
}
// 3: out = max(a, 0).
fk_relu :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    out[item]=fk_relu1(a[item]);
}
// 4: out = a + b, the residual add.
fk_add :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    out[item]=a[item]+b[item];
}
```

The fused kernels do the same f32 operations in the same order as the
kernels they replace. `fk_norm_scale` repeats `fk_rms`'s reduction exactly,
and `fk_bias_relu_add` computes `relu(x + bias) + residual` with the same two
additions. Each operation rounds once, on the CPU and on the device
(`--fmad=false`), so there is no tolerance to state: fused and unfused
outputs must be equal bit for bit.

<!-- source: psionic/lib/device/fuse-kernels.coax fused -->
```jai
// 5 = 0 then 1: item = row. The same reduction as fk_rms; then every thread
// reads the row's total and scales its own columns. The inverse RMS stays
// in registers instead of a buffer. a is x, b is the gain.
fk_norm_scale :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    p:[128]f32;q:[128]f32;
    for thread:0..<FK_THREADS {
        s:f32=0;
        for k:0..<fk_count(thread,cols) {v:=a[item*cols+thread+k*FK_THREADS];s+=v*v;}
        p[thread]=s;
    }
    for step:0..<7 {
        half:=1<<(6-step);
        if step%2==0 {
            for thread:0..<FK_THREADS {s:=p[thread];if fk_live(thread,half) {q[thread]=s+p[thread+half];}}
        } else {
            for thread:0..<FK_THREADS {s:=q[thread];if fk_live(thread,half) {p[thread]=s+q[thread+half];}}
        }
    }
    // Each thread copies the row's total into its own element, then scales
    // its own columns.
    for thread:0..<FK_THREADS {p[thread]=q[0];}
    for thread:0..<FK_THREADS {
        inv:=fk_inv(p[thread],cols,eps);
        for k:0..<fk_count(thread,cols) {col:=thread+k*FK_THREADS;at:=item*cols+col;out[at]=a[at]*inv*b[col];}
    }
}
// 6 = 2, 3 then 4: item = element. a is x, b the bias, c the residual.
fk_bias_relu_add :: (item:int,cols:int,eps:f32,a:[]f32,b:[]f32,c:[]f32,out:[]f32) #export_c {
    out[item]=fk_relu1(a[item]+b[item%cols])+c[item];
}
```

## When a chain may fuse

The unfused step runs one launch to completion before the next starts. A
fused launch runs every item of the chain at once. Three conditions make the
two agree.

**The ops form a chain.** Each op reads what the one before it wrote: the
scale reads the norm's x and its inverse RMS, the ReLU reads the bias's
output, the add reads the ReLU's.

**No span the chain writes overlaps another span of the fused kernel.**
Suppose the output starts one element after y, so `out` is elements 1 to 8
of buffer 3 and y is elements 0 to 7. Each unfused launch is fine on its own:
the add reads r and the residual and writes over y only after the bias has
finished reading it. Fused, item i writes element i+1, which is the y value
item i+1 reads. On the CPU, where items run in order, item 0 writes 2.5 where
y[1] = 2 was, so item 1 computes `max(2.5 + 0.5, 0) + 1 = 4` instead of 3.5,
and the error carries along the row:

| Element | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Unfused | 2.5 | 3.5 | 1 | 1 | 2.5 | 1 | 1 | 2 |
| One launch on the CPU | 2.5 | 4 | 4 | 5 | 6.5 | 8 | 8 | 9 |

On a GPU the items run concurrently, so the result would depend on timing.
The pass checks the output and every intermediate against every span the
fused kernel reads, and the output against the intermediates.

**Nothing needs an intermediate.** The fused kernel never writes h, r or the
inverse RMS. If the caller keeps one, or a later op reads a span that
overlaps one, the chain stays unfused.

Before any of that, every unfused op is admitted on its own: its kernel is
one of the five, each span lies inside its buffer with the size the kernel
needs, and it does not write over one of its own inputs. A step that fails
this is refused whole; nothing is planned.

<!-- source: psionic/lib/device/fuse.coax admit -->
```jai
fu_inputs :: (kind:int)->int {
    if kind==FU_RMS || kind==FU_RELU {return 1;}
    if kind==FU_SCALE || kind==FU_BIAS_RELU_ADD {return 3;}
    return 2;
}
fu_input :: (op:FuOp,j:int)->FuSpan {if j==0 {return op.a;}if j==1 {return op.b;}return op.c;}
// The values span j of a kernel holds; j = 3 is the output.
fu_want :: (kind:int,j:int,rows:int,cols:int)->int {
    if j==3 {if kind==FU_RMS {return rows;}return rows*cols;}
    if j==0 || kind==FU_ADD {return rows*cols;}
    if kind==FU_SCALE && j==1 {return rows;}
    return cols;
}
// Work items: one per row for the norms, one per element otherwise.
fu_items :: (kind:int,rows:int,cols:int)->int #export_c {
    if kind==FU_RMS || kind==FU_NORM_SCALE {return rows;}
    return rows*cols;
}
// On the device: threads per block, and threads in all. A norm runs one
// group of FK_THREADS threads per row; the others one thread per element.
fu_block :: (kind:int)->int #export_c {
    if kind==FU_RMS || kind==FU_NORM_SCALE {return FK_THREADS;}
    return 256;
}
fu_threads :: (kind:int,rows:int,cols:int)->int #export_c {
    if kind==FU_RMS || kind==FU_NORM_SCALE {return rows*FK_THREADS;}
    return rows*cols;
}
fu_inside :: (s:FuSpan,want:int,buffers:[]int)->bool {
    return s.buffer>=0 && s.buffer<buffers.count && s.offset>=0 && s.count==want && s.offset<=buffers[s.buffer]-s.count;
}
fu_overlap :: (p:FuSpan,q:FuSpan)->bool {
    return p.buffer==q.buffer && p.offset<q.offset+q.count && q.offset<p.offset+p.count;
}
fu_same :: (p:FuSpan,q:FuSpan)->bool {return p.buffer==q.buffer && p.offset==q.offset && p.count==q.count;}
// An unfused launch is admitted when its spans lie inside their buffers with
// the right sizes and it writes none of its own inputs.
fu_check :: (op:FuOp,rows:int,cols:int,buffers:[]int)->FuStatus {
    if op.kind<FU_RMS || op.kind>FU_ADD {return FuStatus.kind;}
    if !fu_inside(op.out,fu_want(op.kind,3,rows,cols),buffers) {return FuStatus.span;}
    for j:0..<fu_inputs(op.kind) {if !fu_inside(fu_input(op,j),fu_want(op.kind,j,rows,cols),buffers) {return FuStatus.span;}}
    for j:0..<fu_inputs(op.kind) {if fu_overlap(op.out,fu_input(op,j)) {return FuStatus.alias;}}
    return FuStatus.ok;
}
```

<!-- source: psionic/lib/device/fuse.coax decide -->
```jai
// The chain that starts at op i, by kernel: 2 for norm then scale, 3 for
// bias, ReLU and add, 0 for none.
fu_pattern :: (ops:[]FuOp,i:int)->int {
    if i+1<ops.count && ops[i].kind==FU_RMS && ops[i+1].kind==FU_SCALE {return 2;}
    if i+2<ops.count && ops[i].kind==FU_BIAS && ops[i+1].kind==FU_RELU && ops[i+2].kind==FU_ADD {return 3;}
    return 0;
}
// The one launch that replaces the chain.
fu_fuse :: (ops:[]FuOp,i:int,n:int)->FuOp {
    if n==2 {return FuOp.{kind=FU_NORM_SCALE,a=ops[i].a,b=ops[i+1].c,out=ops[i+1].out,keep=ops[i+1].keep};}
    return FuOp.{kind=FU_BIAS_RELU_ADD,a=ops[i].a,b=ops[i].b,c=ops[i+2].b,out=ops[i+2].out,keep=ops[i+2].keep};
}
fu_refuse :: (i:int,n:int,f:FuOp,why:FuWhy,p:FuSpan,q:FuSpan)->FuDecision {
    return FuDecision.{first=i,ops=n,fused=f.kind,why=why,p=p,q=q};
}
// The spans the chain writes: its output, then its intermediates.
fu_written :: (ops:[]FuOp,i:int,n:int,k:int)->FuSpan {
    if k==0 {return ops[i+n-1].out;}
    return ops[i+k-1].out;
}
fu_decide :: (ops:[]FuOp,i:int,n:int)->FuDecision {
    f:=fu_fuse(ops,i,n);
    // Each op must read what the one before it wrote: the scale reads the
    // norm's x and its inverse RMS; the ReLU reads the bias's output and the
    // add reads the ReLU's.
    if n==2 {
        if !fu_same(ops[i+1].b,ops[i].out) {return fu_refuse(i,n,f,FuWhy.chain,ops[i].out,ops[i+1].b);}
        if !fu_same(ops[i+1].a,ops[i].a) {return fu_refuse(i,n,f,FuWhy.chain,ops[i].a,ops[i+1].a);}
    } else {
        for k:1..<n {if !fu_same(ops[i+k].a,ops[i+k-1].out) {return fu_refuse(i,n,f,FuWhy.chain,ops[i+k-1].out,ops[i+k].a);}}
    }
    // No span the chain writes may overlap a span the fused kernel reads, or
    // another span the chain writes: one launch runs every item at once.
    for k:0..<n {
        w:=fu_written(ops,i,n,k);
        for j:0..<fu_inputs(f.kind) {if fu_overlap(w,fu_input(f,j)) {return fu_refuse(i,n,f,FuWhy.overlap,w,fu_input(f,j));}}
        for m:k+1..<n {v:=fu_written(ops,i,n,m);if fu_overlap(w,v) {return fu_refuse(i,n,f,FuWhy.overlap,w,v);}}
    }
    // The fused kernel never writes the intermediates, so nothing may need
    // them: not the caller, and no later op.
    for k:0..<n-1 {
        w:=ops[i+k].out;
        if ops[i+k].keep {return fu_refuse(i,n,f,FuWhy.live,w,w);}
        for later:i+n..<ops.count {
            for j:0..<fu_inputs(ops[later].kind) {if fu_overlap(w,fu_input(ops[later],j)) {return fu_refuse(i,n,f,FuWhy.live,w,fu_input(ops[later],j));}}
        }
    }
    return FuDecision.{first=i,ops=n,fused=f.kind,why=FuWhy.fused};
}
```

A refused chain is not an error. The step still runs, unfused, and the
decision records which rule kept it apart and the two spans that decided it.

## The plan

`fu_plan` admits every op, then walks the step. Where a chain starts, it
records a decision; if the decision is `fused`, the chain becomes one launch,
otherwise its ops are copied unchanged.

<!-- source: psionic/lib/device/fuse.coax plan -->
```jai
// Admit every op, then copy the ops to `out`, replacing each chain the
// decision admits by its fused launch. Every chain considered gets a
// decision. Nothing is planned unless every op is admitted.
fu_plan :: (ops:[]FuOp,rows:int,cols:int,buffers:[]int,out:[]FuOp,decisions:[]FuDecision)->FuPlan #export_c {
    if rows<1 || rows>FU_LIMIT || cols<1 || cols>FU_LIMIT {return FuPlan.{status=FuStatus.shape,op=-1};}
    for i:0..<ops.count {s:=fu_check(ops[i],rows,cols,buffers);if s!=FuStatus.ok {return FuPlan.{status=s,op=i};}}
    i:=0;n:=0;d:=0;
    while i<ops.count {
        next:=ops[i];step:=1;chain:=fu_pattern(ops,i);
        if chain==2 || chain==3 {
            if d>=decisions.count {return FuPlan.{status=FuStatus.capacity,op=i};}
            decisions[d]=fu_decide(ops,i,chain);d+=1;
            if decisions[d-1].why==FuWhy.fused {next=fu_fuse(ops,i,chain);step=chain;}
        }
        if n>=out.count {return FuPlan.{status=FuStatus.capacity,op=i};}
        out[n]=next;n+=1;i+=step;
    }
    return FuPlan.{status=FuStatus.ok,op=-1,launches=n,decisions=d};
}
// A recorded step can be replayed with new arguments when the new plan has
// the same kernels in the same order; otherwise it is recorded again.
fu_same_kernels :: (a:[]FuOp,b:[]FuOp)->bool #export_c {
    if a.count!=b.count {return false;}
    for i:0..<a.count {if a[i].kind!=b[i].kind {return false;}}
    return true;
}
```

`fu_same_kernels` is the decision a recorded step needs: if the new plan has
the same kernels in the same order, the recorded graph is replayed with new
arguments; otherwise it is recorded again.

## Run it

The [example](../examples/fuse.coax) plans the hand-traced step, runs it unfused and fused on the CPU,
and plans a seven-row prefill chunk:

```shell
coax/bin/coax run psionic/examples/fuse.coax
```

```text
plan: admitted, 2 launches for 5 ops
  ops 0-1: fused into one launch (norm_scale)
  ops 2-4: fused into one launch (bias_relu_add)
  launch norm_scale over 2 items
  launch bias_relu_add over 8 items
inverse RMS (unfused only): 0.5 1.0
unfused out: 2.5 3.5 1.0 1.0 | 2.5 1.0 1.0 2.0
fused out:   2.5 3.5 1.0 1.0 | 2.5 1.0 1.0 2.0
bit-identical outputs: 8 of 8
7 rows: 2 launches, same kernels as 2 rows: true, items 7 and 28
```

<!-- source: psionic/examples/fuse.coax example -->
```jai
// Chapter 43 on the CPU: plan the step of two rows of four values, run it
// unfused and fused, and plan a 7-row prefill chunk that reuses the
// recorded kernels.
fu_name :: (kind:int)->string {
    names:=[7]string.["rms","scale","bias","relu","add","norm_scale","bias_relu_add"];
    return names[kind];
}
main :: () {
    ops:[5]FuOp;buffers:[4]int;plan:[5]FuOp;dec:[2]FuDecision;
    fu_step(2,4,ops[..]);fu_sizes(2,4,buffers[..]);
    p:=fu_plan(ops[..],2,4,buffers[..],plan[..],dec[..]);
    print("plan: %, % launches for 5 ops\n",fu_status(p.status),p.launches);
    for d:dec[0..<p.decisions] {print("  ops %-%: % (%)\n",d.first,d.first+d.ops-1,fu_why(d.why),fu_name(d.fused));}
    for op:plan[0..<p.launches] {print("  launch % over % items\n",fu_name(op.kind),fu_items(op.kind,2,4));}

    // x = 2,2,2,2 and 1,-1,1,-1; residual all ones; gain 1,2,0.5,-1; bias
    // 0.5,0.5,-1,0. eps 0 keeps every value exact.
    b0:=[16]f32.[2,2,2,2,1,-1,1,-1,1,1,1,1,1,1,1,1];b1:=[8]f32.[1,2,0.5,-1,0.5,0.5,-1,0];
    u2:[2]f32;u3:[32]f32;f2:[2]f32;f3:[32]f32;
    fu_run(ops[..],2,4,0,b0[..],b1[..],u2[..],u3[..]);
    fu_run(plan[0..<p.launches],2,4,0,b0[..],b1[..],f2[..],f3[..]);
    print("inverse RMS (unfused only): % %\n",u2[0],u2[1]);
    print("unfused out: % % % % | % % % %\n",u3[24],u3[25],u3[26],u3[27],u3[28],u3[29],u3[30],u3[31]);
    print("fused out:   % % % % | % % % %\n",f3[24],f3[25],f3[26],f3[27],f3[28],f3[29],f3[30],f3[31]);
    same:=0;for i:0..<8 {if f32_bits(u3[24+i])==f32_bits(f3[24+i]) {same+=1;}}
    print("bit-identical outputs: % of 8\n",same);

    // A prefill chunk of 7 rows: the same two kernels, so a recorded step
    // is replayed with new item counts and spans instead of recorded again.
    more:[5]FuOp;plan7:[5]FuOp;fu_step(7,4,more[..]);fu_sizes(7,4,buffers[..]);
    q:=fu_plan(more[..],7,4,buffers[..],plan7[..],dec[..]);
    print("7 rows: % launches, same kernels as 2 rows: %, items % and %\n",q.launches,
        fu_same_kernels(plan[0..<p.launches],plan7[0..<q.launches]),fu_items(plan7[0].kind,7,4),fu_items(plan7[1].kind,7,4));
}
```

The outputs match the table: 2.5, 3.5, 1, 1 and 2.5, 1, 1, 2. The fused plan
writes no inverse RMS buffer, which is why the example reads that value only
from the unfused run.

## Refusals

The [second example](../examples/fuse-refuse.coax) breaks the rules one at a time:

```shell
coax/bin/coax run psionic/examples/fuse-refuse.coax
```

```text
output over y: admitted, 4 launches
  ops 2-4: kept apart: a span the chain writes overlaps another span of the fused kernel
    [buffer 3, 1..<9) and [buffer 3, 0..<8)
  kept apart: 2.5 3.5 1.0 1.0 2.5 1.0 1.0 2.0
  one launch: 2.5 4.0 4.0 5.0 6.5 8.0 8.0 9.0
r kept after the step: 4 launches
  ops 2-4: kept apart: an intermediate the fused kernel would not write is still read
    [buffer 3, 16..<24) and [buffer 3, 16..<24)
scale reads the residual instead of x: 3 launches
  ops 0-1: kept apart: an op does not read what the op before it wrote
    [buffer 0, 0..<8) and [buffer 0, 8..<16)
relu writes over h: refused: an op writes over one of its own inputs, op 3, 0 launches
output at 25..<33 of 32: refused: a span lies outside its buffer or has the wrong size, op 4
0 rows: refused: rows and cols must be 1 to 65536
```

<!-- source: psionic/examples/fuse-refuse.coax refuse -->
```jai
// Chapter 43: chains the pass keeps apart, the result one launch would have
// given anyway, and steps refused before anything is planned.
fu_show :: (s:FuSpan) {print("[buffer %, %..<%)",s.buffer,s.offset,s.offset+s.count);}
fu_say :: (d:FuDecision) {
    print("  ops %-%: %\n    ",d.first,d.first+d.ops-1,fu_why(d.why));
    fu_show(d.p);print(" and ");fu_show(d.q);print("\n");
}
main :: () {
    ops:[5]FuOp;buffers:[4]int;plan:[5]FuOp;dec:[2]FuDecision;
    fu_step(2,4,ops[..]);fu_sizes(2,4,buffers[..]);

    // The output one element along from y, which the add chain reads.
    bad:=ops;bad[4].out=fu_span(3,1,8);
    p:=fu_plan(bad[..],2,4,buffers[..],plan[..],dec[..]);
    print("output over y: %, % launches\n",fu_status(p.status),p.launches);fu_say(dec[1]);
    b0:=[16]f32.[2,2,2,2,1,-1,1,-1,1,1,1,1,1,1,1,1];b1:=[8]f32.[1,2,0.5,-1,0.5,0.5,-1,0];
    kept:[2]f32;k3:[32]f32;forced:[2]f32;f3:[32]f32;
    fu_run(plan[0..<p.launches],2,4,0,b0[..],b1[..],kept[..],k3[..]);
    // One launch anyway: items run in order on the CPU, so item i reads the
    // y value that item i-1 has just overwritten.
    anyway:=[2]FuOp.[plan[0],fu_fuse(bad[..],2,3)];
    fu_run(anyway[..],2,4,0,b0[..],b1[..],forced[..],f3[..]);
    print("  kept apart: % % % % % % % %\n",k3[1],k3[2],k3[3],k3[4],k3[5],k3[6],k3[7],k3[8]);
    print("  one launch: % % % % % % % %\n",f3[1],f3[2],f3[3],f3[4],f3[5],f3[6],f3[7],f3[8]);

    bad=ops;bad[3].keep=true;p=fu_plan(bad[..],2,4,buffers[..],plan[..],dec[..]);
    print("r kept after the step: % launches\n",p.launches);fu_say(dec[1]);
    bad=ops;bad[1].a=fu_span(0,8,8);p=fu_plan(bad[..],2,4,buffers[..],plan[..],dec[..]);
    print("scale reads the residual instead of x: % launches\n",p.launches);fu_say(dec[0]);

    bad=ops;bad[3].out=bad[3].a;p=fu_plan(bad[..],2,4,buffers[..],plan[..],dec[..]);
    print("relu writes over h: %, op %, % launches\n",fu_status(p.status),p.op,p.launches);
    bad=ops;bad[4].out.offset=25;p=fu_plan(bad[..],2,4,buffers[..],plan[..],dec[..]);
    print("output at 25..<33 of 32: %, op %\n",fu_status(p.status),p.op);
    p=fu_plan(ops[..],0,4,buffers[..],plan[..],dec[..]);
    print("0 rows: %\n",fu_status(p.status));
}
```

The first three steps are admitted and run, with the chain kept apart; the
"one launch" line is the table above, computed by forcing the fusion the pass
refused. The last three are refused before planning, with the op that failed.
Both examples exit with status 0, and `psionic/book/examples.json` checks
each transcript in debug and release builds.

## Checks

The checks in `fuse-kernels.coax` compare every kernel with an oracle written
as plain loops in the check itself, not by calling the kernels: thread t's
squares in column order, the same tree in place, then the scale, bias, ReLU
and add. They run for shapes up to 6 rows of 308 columns, which covers three
strides of 128 with the last one partial, and run the fused rows and elements
in reverse order to show that order does not matter. The hand trace above is
a check of its own.

The checks in `fuse.coax` pin the step's layout, plan it, and run the plan on
the CPU against the unfused step bit for bit; they cover every admission
bound, each of the three rules with each span that can trigger it, chains
that do not link, a later op reading an intermediate through any of its
three inputs, and every capacity limit. `fuse.coax` is an `#authority` file:
its mutation run, which includes the kernels it loads, has no survivors
(495 mutants), and neither does the kernel file's own run (133 mutants).

```shell
coax/bin/coax test psionic/lib/device/fuse.coax
coax/bin/coax mutate psionic/lib/device/fuse.coax
```

## On the GPU, recorded once

`psionic/tests/check_fuse.sh` runs the checks in debug and release builds,
emits the CUDA source, compares both transcripts, and runs
`psionic/tests/fuse_device.c` on the device. The host compiles the step with
NVRTC and asks Coax for everything else: the step's spans, the plan, each
launch's thread count and block size, and whether a recorded graph still
fits. It needs an NVIDIA driver and NVRTC; without them the device step
prints `SKIP`.

```shell
psionic/tests/check_fuse.sh
```

For each size, the harness runs the step four ways: five unfused launches,
two fused launches, and both lists recorded as a device graph (chapter 70's
`pc_program`, one node per launch, in order) and replayed. Before each run it
fills the outputs with a NaN pattern, so stale values cannot pass. The graph
is recorded at 1 row and then patched for 7, 128, 512 and 1 row: Coax finds
the same kernels each time, so the host gives every node its new item count
and spans and replays. Before running we fixed the conditions: every output
and every y value must equal the CPU's unfused step bit for bit, in every
one of the four ways. On the RTX 4080, with `cols` = 2880 (gpt-oss's hidden
size), every condition held:

```text
device: NVIDIA GeForce RTX 4080 (sm_89)
   1 rows x 2880: differ from the CPU: unfused 5 launches 0, fused 2 launches 0, graph recorded: fused 0, unfused 0
   7 rows x 2880: differ from the CPU: unfused 5 launches 0, fused 2 launches 0, graph patched: fused 0, unfused 0
 128 rows x 2880: differ from the CPU: unfused 5 launches 0, fused 2 launches 0, graph patched: fused 0, unfused 0
 512 rows x 2880: differ from the CPU: unfused 5 launches 0, fused 2 launches 0, graph patched: fused 0, unfused 0
   1 rows x 2880: differ from the CPU: unfused 5 launches 0, fused 2 launches 0, graph patched: fused 0, unfused 0
output over y: 4 launches (decision 2 on ops 2-4), 0 differ from the CPU
chapter 43 on CUDA: fused, unfused and replayed steps are bit-identical to the CPU
```

The last correctness line is the shifted output: the plan keeps the add chain
apart (decision 2, `overlap`) and the four remaining launches match the CPU.

The harness also times each way over 2,000 steps. Host enqueue is the time
to submit one step to an idle device; device time is steps run back to back
between two events, including the gaps between kernels:

```text
time    1 rows unfused, 5 launches   host enqueue  8.96 us/step, device  10.62 us/step
time    1 rows fused, 2 launches     host enqueue  3.54 us/step, device   9.60 us/step
time    1 rows unfused graph replay  host enqueue  1.62 us/step, device   8.50 us/step
time    1 rows fused graph replay    host enqueue  1.61 us/step, device   9.04 us/step
time  128 rows unfused, 5 launches   host enqueue  8.87 us/step, device  15.28 us/step
time  128 rows fused, 2 launches     host enqueue  3.52 us/step, device  11.16 us/step
time  128 rows unfused graph replay  host enqueue  1.62 us/step, device  13.30 us/step
time  128 rows fused graph replay    host enqueue  1.62 us/step, device  10.33 us/step
```

Submitting the step costs about 1.8 µs per launch on this machine. Fusion
cuts the step's host cost from five launches to two, and replay cuts either
list to one graph launch, saving about 7.3 µs per step against unfused
launches. On the device, fusion saves about 4 µs per 128-row step: three
fewer kernels, and h, r and the inverse RMS are never written and read back.
At one row it saves about 1 µs; there the norm's single group of 128 threads
sets most of the step's time. These are single runs on a shared machine; an
earlier run of the same harness measured host enqueue times about half as
large, with the same ratios. They are not a performance claim for any model.

Replay here patches every node, because a new row count changes every
node's item count and spans. [Chapter 70](ch070-replay-the-device-program.md)
does the finer step for gpt-oss decode: Coax compares each node's admitted
descriptor with the last one and patches only the nodes that changed, and
refuses replay when the step is not a completed synchronous GPU step.

## What is not built yet

- **Other fusions.** Gate and up projections with their activation, expert
  routing (top-k selection and its weights) and a recurrent step with its
  cache write are not fused. Each needs the same three rules over more
  spans, and the projections need matrix kernels that fuse with an
  activation.
- **Plans from the tensor IR.** The step's spans are written by hand in
  `fu_step`. [Chapter 38](ch038-a-tensor-program-becomes-an-execution-plan.md)
  infers aliases and lifetimes for tensor programs; deriving these spans and
  `keep` flags from it is not done.
- **Replay transactions.** This chapter's replay has no failure path: a
  nonfinite value, a driver error or a cancelled step is not handled here.
  Chapter 70 keeps those boundaries for gpt-oss decode; a general replay
  that never publishes a partial step is not built.
- **Models.** No model step uses these kernels or this pass, so there is no
  end-to-end cache, logits and stop-behavior comparison between replayed and
  unreplayed decoding outside chapter 70's gpt-oss path.

## Sources

- [Chapter plan](PLAN.md), Part V.
- [What Psionic takes from llama.cpp](../docs/llama-cpp.md): fusion with
  overlap guards and captured device graphs.
- [Making Psionic serving faster](../docs/serving/performance.md).
- [Chapter 68](ch068-coax-kernels-cooperate.md): group kernels and their
  tree reduction.
- [Chapter 70](ch070-replay-the-device-program.md): device program replay.
