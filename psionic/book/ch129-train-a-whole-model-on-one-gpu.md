# 129. Train a whole model on one GPU

> **Status: partly implemented.** Chapter 115's tiny decoder trains on the
> RTX 4080 under chapter 118's trainer: each row's forward and backward
> pass, the step's mean gradient, its norm, AdamW and the commit run in
> Coax-generated kernels, all trainable state stays on the device between
> steps, the run matches chapter 118's CPU run within a tolerance frozen
> before the first device run, and it resumes bit for bit. An MLP and a CNN
> on the device, a 10–50M-parameter decoder, BF16 storage, convolution
> gradients, kernels that split one row's work across threads (this
> chapter's device step is slower than the CPU) and a PyTorch baseline
> remain.

A separate [checked dense component](../lib/device/training-dense.coax),
`training-dense-f32-v1`, now supplies cooperative `Y = XW`, `dX = dY Wᵀ`
and `dW = Xᵀ dY`. Its 128-thread groups own disjoint 32×32 output tiles,
use a bounded reduction tile and fixed-order FP32 sums, and pass independent
CPU and actual RTX 4080 numerical checks. The
[public example](../examples/training-dense.coax) computes a squared loss
and both operand gradients through the checked API. Dimensions are 1–128;
this component has no optimizer or decoder integration. The
[qualification record](../../docs/verification/psionic-training-dense-2026-11-02/README.md)
keeps the earlier failures, exact scope and remaining work. The whole-step
measurements below remain unchanged.

Inference needs only forward kernels. Training needs the gradients of
every one of them, an optimizer that runs where the parameters live, and
a rule for every number a step could produce. This chapter moves the whole
training step of [chapter 115](ch115-train-the-whole-tiny-decoder.md)'s
decoder onto one GPU, with no PyTorch and no hand-written kernel: every
kernel is Coax, emitted by `coax cuda`
([chapter 37](ch037-a-coax-compute-subset.md)) and compiled by NVRTC with
`--fmad=false`, as in [chapter 44](ch044-elephant-on-cuda.md).

The decisions do not move. [Chapter 118](ch118-one-trainer-for-every-model.md)'s
trainer still names every batch, owns the schedule, and writes and reads
the checkpoint. The device does arithmetic; Coax on the host decides
whether the run fits the device at all, whether a step's numbers are
admitted, and whether its staged values are written.

## What runs where

One step is seven kernels, launched in this order:

| Kernel | Work items | What it does |
| --- | --- | --- |
| `gk_rows` | one per row | chapter 113's forward and backward pass on one packed row: its loss and gradient |
| `gk_mean` | one per parameter | chapter 118's `tr_add`: the rows' gradients divided by the row count, summed in row order |
| `gk_squares` | one | the squared global norm, summed in parameter order |
| `gk_clip` | one per parameter | scales the gradient, launched only when the norm is past the clip |
| `gk_adamw` | one per parameter | [chapter 124](ch124-adamw-and-explicit-precision.md)'s `aw_cell`, written to a stage |
| `gk_admit` | one | two flags: a staged value AdamW refuses, a parameter outside the bound |
| `gk_commit` | one per parameter | copies the stage into the parameters and moments, launched only for an admitted step |

Between the third and fifth launches the host reads back the row losses
and the squared norm (a few values, not the gradient), and `gt_decide`
makes chapter 118's decisions. Between the sixth and seventh it reads the
two flags, and `gt_commit` decides whether the step happens. Parameters,
moments, gradients and work buffers never leave the device during a run;
they come back only for a checkpoint.

The decoder's own code (`td_advance`, `td_gradients`) uses structs and
`while` loops, which the compute subset refuses. The kernels restate the
same arithmetic over a shape view of eight integers and plain offsets,
one operation for one operation in the same order. Two changes keep the
bits while making the code checkable: the softmax shift comes from
`gk_first`, the first largest value, and the FFN's square ReLU always
multiplies by `gk_relu(z)` instead of skipping `z <= 0` (adding a zero to
a sum that is never `-0` leaves it unchanged). RoPE's cosines and sines
are not computed on the device at all: the host fills a table once.

<!-- source: psionic/lib/transformer/train-kernels.coax rope -->
```jai
// The host's table: for position t and pair i, the cosine and sine of
// t * theta^(-2i/E), at (t*E/2 + i)*2 and the element after it.
gk_rope_table :: (e:int,context:int,out:[]f64) {
    for t:0..<context {for i:0..<e/2 {angle:=f64(t)*exp(-log(GK_THETA)*f64(2*i)/f64(e));at:=(t*(e/2)+i)*2;out[at]=cos(angle);out[at+1]=sin(angle);}}
}
// Rotates each interleaved pair of every head; back applies the transpose
// (the sine negated), which the backward pass needs.
gk_rope :: (v:[]f64,vo:int,heads:int,e:int,rope:[]f64,t:int,back:bool) {
    for h:0..<heads {for i:0..<e/2 {at:=(t*(e/2)+i)*2;c:=rope[at];sn:=rope[at+1];if back {sn=-sn;}
        x:=vo+h*e+2*i;a:=v[x];b:=v[x+1];v[x]=a*c-b*sn;v[x+1]=a*sn+b*c;}}
}
```

## The step kernels

<!-- source: psionic/lib/transformer/train-kernels.coax step -->
```jai
// Kernel 1, one item per parameter: chapter 118's tr_add over the step's
// micro-batches, in their order: the sum starts at zero and adds each row's
// gradient divided by the micro-batch count.
gk_mean :: (item:int,micro:int,n:int,grads:[]f64,total:[]f64) #export_c {
    total[item]=0;for k:0..<micro {total[item]+=grads[k*n+item]/f64(micro);}
}
// Kernel 2, one item: the squared global norm, summed in parameter order as
// tr_apply sums it.
gk_squares :: (item:int,n:int,total:[]f64,out:[]f64) #export_c {
    s:=0.0;for i:0..<n {s+=total[i]*total[i];}out[item]=s;
}
// Kernel 3, one item per parameter, launched only when the norm is over the
// clip: tr_apply's scaling.
gk_clip :: (item:int,clip:f64,norm:f64,total:[]f64) #export_c {
    total[item]=total[item]*clip/norm;
}
// Kernel 4, one item per parameter: chapter 124's aw_cell at step t, with
// c1 = 1 - beta1^t and c2 = 1 - beta2^t from the host. The new parameter,
// moments go to the stage (p, m, v as three spans of n); nothing live changes.
gk_adamw :: (item:int,n:int,lr:f64,beta1:f64,beta2:f64,eps:f64,decay:f64,c1:f64,c2:f64,p:[]f64,total:[]f64,m:[]f64,v:[]f64,stage:[]f64) #export_c {
    grad:=total[item];
    decayed:=p[item]*(1-lr*decay);
    m2:=beta1*m[item]+(1-beta1)*grad;
    v2:=beta2*v[item]+(1-beta2)*grad*grad;
    stage[item]=decayed-(lr/c1)*(m2/(sqrt(v2)/sqrt(c2)+eps));stage[n+item]=m2;stage[2*n+item]=v2;
}
GK_MAX :: 1.7976931348623157e308;
gk_finite :: (x:f64)->bool {return x>=-GK_MAX && x<=GK_MAX;}
// Kernel 5, one item: what the host needs to admit the staged step. out[0]
// is 1 when a staged value is not finite (AdamW refuses the step), out[1]
// is 1 when a staged parameter is outside the bound.
gk_admit :: (item:int,n:int,bound:f64,stage:[]f64,out:[]f64) #export_c {
    bad:=0.0;far:=0.0;
    for i:0..<n {if !gk_finite(stage[i]) || !gk_finite(stage[n+i]) || !gk_finite(stage[2*n+i]) {bad=1;}if !(stage[i]<=bound && stage[i]>=-bound) {far=1;}}
    out[2*item]=bad;out[2*item+1]=far;
}
// Kernel 6, one item per parameter, launched only for an admitted step.
gk_commit :: (item:int,n:int,stage:[]f64,p:[]f64,m:[]f64,v:[]f64) #export_c {
    p[item]=stage[item];m[item]=stage[n+item];v[item]=stage[2*n+item];
}
```

On the CPU every one of these is an ordinary Coax procedure. On the device
each work item is one thread. Addition, subtraction, multiplication,
division and square root round the same way on both, so `gk_mean`,
`gk_squares`, `gk_clip`, `gk_adamw`, `gk_admit` and `gk_commit` give the C
build's bits when they are given the same inputs. `gk_rows` also calls
`exp` and `log`, which come from the CUDA device library: its results may
differ from the C library's in the last bits.

## Will it fit

Before anything is allocated, `gt_plan` asks the trainer to admit the run
(`tr_admit`), checks that the device has a kernel for every part of the
step, and adds up the buffers:

<!-- source: psionic/lib/transformer/gpu-train.coax plan -->
```jai
// Device buffers, in this order. Every one is binary64 except the rows (one
// byte a token) and the shape (eight 64-bit integers).
GT_BUFFERS :: 13;
GT_NAMES :: [13]string.["parameters","first moments","second moments","stage (p, m, v)","step gradient","row gradients","row work","row work gradients","rows","rope table","row losses","norm and flags","shape"];
GtPlan :: struct {status:GtStatus;parameters:int;work:int;rows:int;bytes:int;sizes:[13]int;}
// What one run of chapter 115's decoder needs on the device, in bytes, and
// whether the device has a kernel for every part of the step. Nothing is
// allocated unless the whole plan fits the budget.
gt_plan :: (s:TdShape,c:TrConfig,docs:[]PkDoc,tokens:[]u8,budget:int)->GtPlan #export_c {
    if !td_valid(s) {return GtPlan.{status=GtStatus.shape};}
    n:=td_parameter_count(s);
    // The trainer admits the run first: its checkpoint (chapter 122's
    // record) holds at most CK_VALUES parameters.
    t:=tr_admit(c,n,docs,tokens);
    if t==TrStatus.shape || c.source!=TrSource.stream || c.length!=s.context {return GtPlan.{status=GtStatus.shape,parameters=n};}
    if t!=TrStatus.ok {return GtPlan.{status=GtStatus.trainer,parameters=n};}
    // The device step is AdamW (binary64: the trainer admits no other
    // storage for it) without decay spans, one row a
    // micro-batch. Anything else would need a kernel this chapter does not
    // have, and a missing kernel is a refusal, never a CPU fallback.
    if c.optimizer!=TrOptimizer.adamw || c.span_count!=0 || c.batch!=1 {return GtPlan.{status=GtStatus.kernel,parameters=n};}
    rows:=c.accumulate;w:=td_work_count(s);p:=GtPlan.{parameters=n,work=w,rows=rows};
    each:=[13]int.[8*n,8*n,8*n,24*n,8*n,8*rows*n,8*rows*w,8*rows*w,rows*s.context,8*s.context*s.head_width,8*rows,24,64];
    for k:0..<GT_BUFFERS {p.sizes[k]=each[k];p.bytes+=each[k];}
    if p.bytes>budget {p.status=GtStatus.budget;}
    return p;
}
```

### The plan by hand

Chapter 115's decoder (`TT_SHAPE`) has n = 3152 parameters and needs
13984 work values for one row of 32 tokens. With four rows a step:

| Buffer | Values | Bytes |
| --- | --- | --- |
| parameters, first moments, second moments | 3 × 3152 | 75648 |
| stage (new p, m, v) | 3 × 3152 | 75648 |
| step gradient | 3152 | 25216 |
| row gradients | 4 × 3152 | 100864 |
| row work and its gradients | 2 × 4 × 13984 | 894976 |
| rope table | 32 × 8 | 2048 |
| row losses, then norm and two flags | 4 + 3 | 56 |
| rows (one byte a token) | 4 × 32 | 128 |
| shape (eight 64-bit integers) | 8 | 64 |
| **total** | | **1174648** |

The work buffers dominate: each row keeps every saved activation of the
forward pass for its backward pass. A budget of exactly 1174648 bytes is
admitted; one byte less is refused before anything is allocated.

## Decisions on the host

<!-- source: psionic/lib/transformer/gpu-train.coax decide -->
```jai
GtDecide :: struct {status:TrStatus;revision:int;loss:f64;norm:f64;rate:f64;clip:bool;c1:f64;c2:f64;}
// After the device has written each row's loss and the squared norm of the
// step's mean gradient: chapter 118's tr_add and the first half of
// tr_apply. The loss sums the rows' losses over the micro-batch count in
// order; the norm decides clipping; the schedule gives the rate; AdamW's
// bias corrections for the next step are computed here, once.
gt_decide :: (c:TrConfig,run:TrRun,b:TrBatch,losses:[]f64,squares:f64)->GtDecide #export_c {
    fail:=GtDecide.{status=TrStatus.order,revision=run.revision};
    if b.status!=TrStatus.ok || b.revision!=run.revision || b.micro<1 || losses.count!=b.micro {return fail;}
    k:=f64(b.micro);loss:=0.0;for x:losses {loss+=x/k;}
    if !tr_finite(loss) || !tr_finite(squares) {fail.status=TrStatus.number;return fail;}
    norm:=sqrt(squares);rate:=tr_rate(c,run.state.step);
    if !aw_state(run.state) || !aw_hyper(AwGroup.{lr=rate,beta1=c.beta1,beta2=c.beta2,eps=c.eps}) {fail.status=TrStatus.optimizer;return fail;}
    t:=f64(run.state.step+1);
    return GtDecide.{revision=run.revision,loss=loss,norm=norm,rate=rate,clip=c.clip>0 && norm>c.clip,c1=1-pow(c.beta1,t),c2=1-pow(c.beta2,t)};
}
// After the device has staged the step and flagged it (flags[0]: a value
// AdamW refuses; flags[1]: a parameter outside the bound): the rest of
// tr_apply. Only an admitted step advances the run; the caller then
// launches the commit.
gt_commit :: (c:TrConfig,run:TrRun,b:TrBatch,d:GtDecide,flags:[]f64)->TrStep #export_c {
    fail:=TrStep.{run=run,status=d.status};
    if d.status!=TrStatus.ok {return fail;}
    if d.revision!=run.revision || b.revision!=run.revision || b.status!=TrStatus.ok {fail.status=TrStatus.order;return fail;}
    if flags.count!=2 {fail.status=TrStatus.shape;return fail;}
    if flags[0]!=0 {fail.status=TrStatus.optimizer;return fail;}
    if flags[1]!=0 {fail.status=TrStatus.bound;return fail;}
    r:=run;r.state.step+=1;r.revision+=1;r.cursor=b.after;
    return TrStep.{run=r,loss=d.loss,rate=d.rate,norm=d.norm};
}
```

### One decision by hand

Take a step at revision 4 with three rows whose losses are 1, 2 and 3, and
a squared norm of 25 from `gk_squares`, with a clip of 2. The loss is
1/3 + 2/3 + 3/3 = 2, added in that order as `tr_add` adds it. The norm is
√25 = 5, past the clip of 2, so `gk_clip` is launched. The rate is
`tr_rate(c, 4)`. AdamW's next step is t = 5, so c1 = 1 − 0.9⁵ and
c2 = 1 − 0.99⁵. If `gk_admit` then reports (0, 0), the run moves to
revision 5 and optimizer step 5; (1, 0) is an `optimizer` refusal and
(0, 1) a `bound` refusal, and the run stays at revision 4. The check
"decisions by hand, and every refusal" asserts each of these values.

And one AdamW cell by hand, as the kernel check computes it: rate 0.25,
β1 = 0.5, β2 = 0.75, ε = 0.5, p = 1, g = 2, zero moments, step 1. Then
m = 0.5·0 + 0.5·2 = 1, v = 0.75·0 + 0.25·4 = 1, c1 = 0.5, c2 = 0.25, and
p = 1 − (0.25/0.5) · (1/(√1/√0.25 + 0.5)) = 1 − 0.5 · (1/2.5) = 0.8.

## The same launches on the CPU

`gt_step_cpu` runs the device's launches in the device's order, each
kernel once for every work item. It is what a machine without a GPU runs,
and it is how the checks know the kernels are right:

<!-- source: psionic/lib/transformer/gpu-train.coax cpu -->
```jai
// The shape as the kernels read it.
gt_shape :: (s:TdShape,sh:[]int) #export_c {x:=[8]int.[s.vocab,s.width,s.heads,s.kv_heads,s.head_width,s.hidden,s.layers,s.context];for i:0..<8 {sh[i]=x[i];}}
// One step on the CPU: the device's launches, in the device's order, each
// kernel run once for every work item. rows holds the batch tr_next filled.
// flags is scalars[1..<3]: the norm is scalars[0].
gt_step_cpu :: (s:TdShape,c:TrConfig,run:TrRun,b:TrBatch,rows:[]u8,rope:[]f64,p:[]f64,m:[]f64,v:[]f64,stage:[]f64,total:[]f64,grads:[]f64,work:[]f64,dwork:[]f64,losses:[]f64,scalars:[]f64)->TrStep #export_c {
    sh:[8]int;gt_shape(s,sh[..]);n:=p.count;micro:=b.micro;
    if b.status!=TrStatus.ok || micro<1 || micro>losses.count {return TrStep.{status=TrStatus.order,run=run};}
    for i:0..<micro {gk_rows(i,sh[..],p,rows,rope,work,dwork,grads,losses);}
    for i:0..<n {gk_mean(i,micro,n,grads,total);}
    gk_squares(0,n,total,scalars);
    d:=gt_decide(c,run,b,losses[0..<micro],scalars[0]);if d.status!=TrStatus.ok {return TrStep.{status=d.status,run=run};}
    if d.clip {for i:0..<n {gk_clip(i,c.clip,d.norm,total);}}
    for i:0..<n {gk_adamw(i,n,d.rate,c.beta1,c.beta2,c.eps,0,d.c1,d.c2,p,total,m,v,stage);}
    gk_admit(0,n,c.bound,stage,scalars[1..<3]);
    st:=gt_commit(c,run,b,d,scalars[1..<3]);
    if st.status==TrStatus.ok {for i:0..<n {gk_commit(i,n,stage,p,m,v);}}
    return st;
}
```

It is compared with chapter 118's own run, which calls chapter 113's
`td_gradients` on each row and the trainer's `tr_add` and `tr_apply`:

<!-- source: psionic/lib/transformer/gpu-train.coax reference -->
```jai
// Chapter 118's own CPU run, which the device is compared with: tr_next,
// chapter 113's td_gradients on each row, tr_add and tr_apply. rows holds a
// step's rows, w and dw one row's work, g one row's gradient, stage 3 * n
// values; losses gets each step's loss.
gt_reference :: (s:TdShape,c:TrConfig,docs:[]PkDoc,tokens:[]u8,order:[]int,run:TrRun,count:int,rows:[]u8,w:[]f64,dw:[]f64,g:[]f64,p:[]f64,m:[]f64,v:[]f64,total:[]f64,stage:[]f64,losses:[]f64)->TrStep #export_c {
    n:=p.count;ints:[64]int;r:=run;nw:=td_work_count(s);
    if !td_valid(s) || n!=td_parameter_count(s) || w.count<nw || dw.count<nw || g.count<n || rows.count<c.accumulate*c.length || c.length>64 {return TrStep.{status=TrStatus.shape,run=run};}
    for k:0..<count {
        b:=tr_next(c,r,docs,tokens,rows[0..<c.accumulate*c.length]);if b.status!=TrStatus.ok {return TrStep.{status=b.status,run=r};}
        sum:TrSum;
        for j:0..<b.micro {span:=tr_rows(c,b,j);for t:0..<c.length {ints[t]=cast(int)rows[span.first*c.length+t];}
            res:=td_gradients(s,p,ints[0..<c.length],w,dw,g[0..<n],false);if res.status!=TdStatus.ok {return TrStep.{status=TrStatus.number,run=r};}
            sum=tr_add(b,sum,res.loss,g[0..<n],total);}
        a:=tr_apply(c,r,b,sum,p,m,v,total,stage,order);if a.status!=TrStatus.ok {return a;}
        r=a.run;losses[k]=a.loss;
    }
    return TrStep.{run=r};
}
```

On the CPU the two agree bit for bit: losses, parameters and both moments,
over six steps, with clipping, and with two rows a step. The kernels also
reproduce `td_gradients`' bits on two rows of a shape with no two fields
alike (seven symbols, width 16, four heads sharing two key heads, head
width 4, hidden 5, two layers, context 5), and match central differences
on chapter 113's small shape.

## The tolerance, frozen first

On the device, `exp` and `log` may round differently, and AdamW carries
any difference forward. The comparison rule was written into
`gpu-train.coax` before the first device run and has not changed:

<!-- source: psionic/lib/transformer/gpu-train.coax config -->
```jai
// Chapter 118's tm_decoder: AdamW on chapter 115's linear schedule, each
// step `rows` packed rows of the context length, one row a micro-batch.
gt_config :: (s:TdShape,rows:int,steps:int)->TrConfig #export_c {
    return TrConfig.{optimizer=TrOptimizer.adamw,schedule=TrSchedule.linear,source=TrSource.stream,rate=TT_RATE,drop=0.9,beta1=0.9,beta2=0.99,eps=0.00000001,
        bound=TD_NUMBER,batch=1,accumulate=rows,length=s.context,steps=steps,seed=TT_ORDER};
}
// Frozen before the first device run: a device value passes when its bits
// equal the CPU's or its gap is at most this fraction of their magnitudes.
GT_TOLERANCE :: 0.000000001;
```

A value passes when its bits equal the CPU's or when
|a − b| ≤ 10⁻⁹ · (|a| + |b|). The fixed-order pieces get no tolerance at
all: the device harness gives `gk_mean`, `gk_squares`, `gk_adamw` and
`gk_admit` the C build's row gradients and requires every bit to match.

## The program

The example, [`psionic/examples/gpu-train.coax`](../examples/gpu-train.coax),
prints the plan, the launches, and three steps of the device's launches on
the CPU against chapter 118's run, then saves after two steps and resumes.
It needs no GPU.

<!-- source: psionic/examples/gpu-train.coax example -->
```jai
// Chapter 129: the device plan for chapter 115's decoder, the step's
// launches, and three steps of those launches run on the CPU, against
// chapter 118's own run and through a checkpoint. No GPU is needed: the
// device harness (psionic/tests/check_gpu_train.sh) runs the same kernels
// on one.
main :: () {
    c:=gt_config(TT_SHAPE,4,400);f:=tt_fix();p:=gt_plan(TT_SHAPE,c,f.docs[..],f.tokens[0..<f.n],16*1073741824);
    print("plan for chapter 115's decoder, 4 rows a step: %, % parameters, % work values a row\n",gt_status(p.status),p.parameters,p.work);
    for k:0..<GT_BUFFERS {print("  %: % bytes\n",GT_NAMES[k],p.sizes[k]);}
    print("  total: % bytes (hand count %)\n",p.bytes,GT_HAND_BYTES);
    print("launches a step: gk_rows (4 items), gk_mean (3152), gk_squares (1), gk_clip (3152, only past the clip),\n");
    print("  gk_adamw (3152), gk_admit (1), gk_commit (3152, only when admitted)\n");

    s:=gt_config(TT_SMALL,4,3);run:=gt_start(s,f,220);x:=gt_small();
    a:=gt_small_run(s,f,run,3,x);b:=gt_small_reference(s,f,run,3,x);
    print("three steps at the checks' small shape, the device's launches on the CPU:\n");
    for k:0..<3 {print("  step %: loss % (chapter 118: %)\n",k+1,a.x.losses[k],b.x.losses[k]);}
    same:=gt_bits(a.x.losses[0..<3],b.x.losses[0..<3]) && gt_bits(a.x.p[..],b.x.p[..]) && gt_bits(a.x.m[..],b.x.m[..]) && gt_bits(a.x.v[..],b.x.v[..]);
    print("  losses, parameters and moments equal chapter 118's run bit for bit: %\n",same);

    // Saved after two steps, resumed into fresh buffers, one step more.
    two:=gt_small_run(s,f,run,2,x);ids:=gt_ids(TT_SMALL,s,f.docs[..],f.tokens[0..<f.n]);bytes:[5620]u8;pointer:[80]u8;
    saved:=gt_save(ids,two.step.run,two.x.p[..],two.x.m[..],two.x.v[..],bytes[..],pointer[..]);
    y:=gt_small();back:=gt_resume(s,ids,pointer[..],bytes[..],f.docs[..],f.tokens[0..<f.n],y.p[..],y.m[..],y.v[..],y.none[0..<0]);
    rest:=gt_small_run(s,f,back.run,1,y);
    print("saved after step 2 (% bytes), resumed at revision %: step 3 loss %, weights equal the uninterrupted run: %\n",
        gt_size(220),back.run.revision,rest.x.losses[0],gt_bits(rest.x.p[..],a.x.p[..]) && gt_bits(rest.x.v[..],a.x.v[..]) && saved==CkStatus.ok);
}
```

```text
plan for chapter 115's decoder, 4 rows a step: admitted, 3152 parameters, 13984 work values a row
  parameters: 25216 bytes
  first moments: 25216 bytes
  second moments: 25216 bytes
  stage (p, m, v): 75648 bytes
  step gradient: 25216 bytes
  row gradients: 100864 bytes
  row work: 447488 bytes
  row work gradients: 447488 bytes
  rows: 128 bytes
  rope table: 2048 bytes
  row losses: 32 bytes
  norm and flags: 24 bytes
  shape: 64 bytes
  total: 1174648 bytes (hand count 1174648)
launches a step: gk_rows (4 items), gk_mean (3152), gk_squares (1), gk_clip (3152, only past the clip),
  gk_adamw (3152), gk_admit (1), gk_commit (3152, only when admitted)
three steps at the checks' small shape, the device's launches on the CPU:
  step 1: loss 2.9360807937186078 (chapter 118: 2.9360807937186078)
  step 2: loss 2.761001189796394 (chapter 118: 2.761001189796394)
  step 3: loss 2.7893926020018047 (chapter 118: 2.7893926020018047)
  losses, parameters and moments equal chapter 118's run bit for bit: true
saved after step 2 (5620 bytes), resumed at revision 2: step 3 loss 2.7893926020018047, weights equal the uninterrupted run: true
```

The three losses are chapter 115's own (`TT_SHORT_LOSSES`): the device's
launches, run on the CPU, are chapter 115's training.

## Refusals

Runs the device refuses before it allocates anything, and a step it
refuses after staging
([`psionic/examples/gpu-train-refuse.coax`](../examples/gpu-train-refuse.coax)):

<!-- source: psionic/examples/gpu-train-refuse.coax refuse -->
```jai
// Chapter 129: runs the device refuses before it allocates anything, and a
// step it refuses after staging, which changes nothing.
main :: () {
    c:=gt_config(TT_SHAPE,4,400);f:=tt_fix();d:=f.docs[..];t:=f.tokens[0..<f.n];
    p:=gt_plan(TT_SHAPE,c,d,t,1048576);print("a 1 MiB budget: %; the plan needs % bytes\n",gt_status(p.status),p.bytes);
    p=gt_plan(TT_SHAPE,c,d,t,GT_HAND_BYTES);print("a budget of exactly % bytes: %\n",GT_HAND_BYTES,gt_status(p.status));
    big:=TdShape.{vocab=256,width=64,heads=8,kv_heads=2,head_width=8,hidden=256,layers=4,context=32};
    p=gt_plan(big,c,d,t,1<<40);print("a decoder of % parameters: %\n",p.parameters,gt_status(p.status));
    sgd:=c;sgd.optimizer=TrOptimizer.sgd;sgd.rate=0.1;p=gt_plan(TT_SHAPE,sgd,d,t,1<<40);print("SGD: %\n",gt_status(p.status));
    wide:=c;wide.batch=2;p=gt_plan(TT_SHAPE,wide,d,t,1<<40);print("two rows a micro-batch: %\n",gt_status(p.status));

    // A bound of 0.5 that the first step's AdamW move crosses.
    s:=gt_config(TT_SMALL,4,3);s.bound=0.5;run:=gt_start(s,f,220);x:=gt_small();
    r:=gt_small_run(s,f,run,1,x);
    print("a step past a bound of 0.5: status %, revision %, flags (% %), parameters unchanged: %\n",
        r.step.status,r.step.run.revision,r.x.scalars[1],r.x.scalars[2],gt_bits(r.x.p[..],x.p[..]) && gt_bits(r.x.m[..],x.m[..]));
}
```

```text
a 1 MiB budget: refused: the buffers do not fit the device budget; the plan needs 1174648 bytes
a budget of exactly 1174648 bytes: admitted
a decoder of 205376 parameters: refused: the shape is not a chapter 113 decoder the trainer can checkpoint, or rows are not the context long
SGD: refused: no device kernel for this optimizer, decay or micro-batch
two rows a micro-batch: refused: no device kernel for this optimizer, decay or micro-batch
a step past a bound of 0.5: status bound, revision 0, flags (0.0 1.0), parameters unchanged: true
```

A 1 MiB budget is 1048576 bytes, 126072 short of the plan. The larger
decoder has 205376 parameters, past the 4096 values chapter 122's record
holds, so the trainer refuses it before the device is asked. SGD and
two-row micro-batches have no device kernel here, and a missing kernel is
a refusal, never a fallback to the CPU. The bound refusal leaves the
parameters and moments as they were: the step was staged, flagged by
`gk_admit` (the second flag is 1) and never committed.

## On the device

`psionic/tests/check_gpu_train.sh` runs the checks, the examples in debug
and release, and then, when a CUDA driver and NVRTC are present, the
device harness `psionic/tests/gpu_train_device.c`. That file only
allocates, copies, launches and times; the plan, batches, decisions,
checkpoints and comparisons are the Coax above, compiled to C. Without a
device it prints `SKIP`.

```shell
COAX_CC=gcc psionic/tests/check_gpu_train.sh
```

The run recorded for this chapter (RTX 4080, sm_89, 20 steps of four
rows, 2026-11-01):

```text
device: NVIDIA GeForce RTX 4080 (sm_89), 15546 MiB free of 15937
plan: 1174648 bytes in 13 buffers (hand count 1174648): admitted; with a budget of 1174647 bytes: refused, the buffers do not fit
allocated: 1174648 bytes asked in 13 allocations; the driver's free memory fell by 2097152 bytes
one step's rows, device against the C build of the same kernel:
  row gradients           12608 values,  10999 differ in bits, 0 past the tolerance, worst relative gap 5.62e-12
  row losses                  4 values,      1 differ in bits, 0 past the tolerance, worst relative gap 7.52e-17
the fixed-order pieces on the same row gradients:
  mean gradient            3152 values,      0 differ in bits, 0 past the tolerance, worst relative gap 0
  norm and flags              3 values,      0 differ in bits, 0 past the tolerance, worst relative gap 0
  AdamW stage              9456 values,      0 differ in bits, 0 past the tolerance, worst relative gap 0
20 steps of 4 rows, device against chapter 118's CPU run (tolerance 1e-09, frozen before the first run):
  losses                     20 values,      4 differ in bits, 0 past the tolerance, worst relative gap 9.04e-17
  parameters               3152 values,   2154 differ in bits, 0 past the tolerance, worst relative gap 5.13e-14
  first moments            3152 values,   2989 differ in bits, 0 past the tolerance, worst relative gap 2.18e-12
  second moments           3152 values,   2861 differ in bits, 0 past the tolerance, worst relative gap 2.15e-15
  loss: step 1 2.906973, step 20 2.534578
saved after 10 steps (75988 bytes), resumed into buffers filled with NaN: ok; losses, parameters and moments differ in 0 bits from the uninterrupted run
receipt, 4 rows a step: device 33.05 ms/step (3872 tokens/s), C build of chapter 118's CPU run 1.27 ms/step (101064 tokens/s)
receipt, 64 rows a step (16114648 bytes on the device): device 54.88 ms/step (37320 tokens/s), CPU 17.14 ms/step (119462 tokens/s); parameters 0 past the tolerance
chapter 129 on CUDA: the decoder trains on the device within the frozen tolerance of chapter 118's CPU run, and resumes bit for bit
```

What it shows:

- **Memory.** The plan's 1174648 bytes equal the hand count, the thirteen
  allocations ask for exactly that, and the driver's free memory fell by
  2 MiB, its allocation granularity.
- **Fixed-order pieces are exact.** Given the same row gradients, the
  device's mean gradient, norm, flags and all 9456 staged values match
  the C build bit for bit.
- **The row kernel is close, not exact.** Most row-gradient values differ
  in their last bits (device `exp` and `log`); the worst relative gap is
  about 6 × 10⁻¹², far inside the tolerance.
- **Training matches.** After 20 steps the losses, parameters and both
  moments are all within the frozen tolerance of chapter 118's CPU run.
- **Resume is exact.** A run saved after 10 steps through chapter 118's
  checkpoint and resumed into device buffers filled with NaN ends with the
  same bits as the run that was never interrupted.

The speed lines are a receipt, not a check. They are kept out of every
transcript because they change from run to run. They also say plainly
what this chapter has not done: one thread computes a whole row, so with
four rows the GPU uses four threads, and the device step is slower than
the CPU. With 64 rows a step the device does more useful work per launch
but is still slower. Making the GPU pay off means splitting a row's
matrix products, attention and norms across threads and blocks, as
chapters 66–70 do for inference; that is the next step, not this one.

## How it is checked

- `psionic/lib/transformer/train-kernels.coax`: the layouts by hand;
  the rope table; the loss against chapter 113's independent Python
  definition; every gradient against central differences; both rows of
  two shapes against `td_gradients`' bits; the step kernels by hand,
  including every refusal flag. Mutation testing kills all 723 mutants.
- `psionic/lib/transformer/gpu-train.coax` (`#authority`): the plan
  against the hand count and every refusal at its edge; every decision by
  hand and every refusal; the CPU launches against chapter 118's run, bit
  for bit, plain, clipped and with two rows a step; save and resume bit
  for bit, and resume refusals; a refused step changes nothing; the gap
  rule at its edge. Mutation testing kills all 232 mutants.
- The examples' transcripts, compared exactly in debug and release.
- On the device: the harness above.

## What is not here yet

- An MLP and a CNN trained on the device, and convolution gradients.
- A 10–50M-parameter decoder: the trainer's checkpoint holds at most 4096
  parameters, and one thread per row is far too slow at that size.
- BF16 storage and mixed precision; this chapter is binary64 throughout.
- Kernels that split a row's work across threads, so that the device is
  faster than the CPU, and a tokens-per-second comparison with an
  independent PyTorch baseline.
- Decay spans, SGD and multi-row micro-batches on the device: each is a
  refusal today.

## Public API

- `psionic/lib/transformer/train-kernels.coax`: `gk_rows`, `gk_mean`,
  `gk_squares`, `gk_clip`, `gk_adamw`, `gk_admit`, `gk_commit` (kernels);
  `gk_rope_table`, `gk_parameters`, `gk_work`.
- `psionic/lib/transformer/gpu-train.coax`: `gt_plan`, `gt_decide`,
  `gt_commit`, `gt_step_cpu`, `gt_run_cpu`, `gt_reference`, `gt_gap`,
  `gt_config`, `GT_TOLERANCE`, and the C host's entry points (`gt_fix`,
  `gt_begin`, `gt_next`, `gt_ids`, `gt_save`, `gt_resume`, `gt_rope`,
  `gt_shape`, `gt_init`, `gt_tolerance`).
- `psionic/tests/gpu_train_device.c` and `psionic/tests/check_gpu_train.sh`:
  the device harness.

## Sources

- [Chapter plan](PLAN.md), section "A framework, not only engines"

The [bounded dense example](../examples/training-dense.coax) is registered
separately from whole-model training. `psionic/check.sh --only training-dense`
checks only these new units and native fixtures; it starts no dataset, optimizer
experiment or GPU job.
