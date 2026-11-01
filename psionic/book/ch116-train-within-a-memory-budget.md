# 116. Train within a memory budget

> **Status: partly implemented.**

Training holds far more memory than inference. Inference keeps the weights
and a cache. Training also keeps a gradient for every weight, the
optimizer's moments, and every value the backward pass will read. This
chapter counts those bytes exactly for chapter 115's tiny decoder, then fits
its training step under a byte cap in two ways that change only where values
live: gradient accumulation and layer recomputation. Both reproduce chapter
115's run bit for bit. A third way, a reduced-precision gradient policy with
a dynamic loss scale, changes the numbers, and the chapter measures by how
much.

The runnable profiles are `mb-plan-v1` (the byte account and the planner)
and `mb-train-cpu-f64-v1` (the step that runs a plan): binary64 on the CPU,
one thread, every sum in a fixed order. The library is
[budget.coax](../lib/transformer/budget.coax), which decides admission and is
marked `#authority`, and
[budget-train.coax](../lib/transformer/budget-train.coax). They build on:

- [Chapter 115](ch115-train-the-whole-tiny-decoder.md): the model, its data
  stream and its step (`tt_step`), the reference every plan must reproduce.
- [Chapter 113](ch113-differentiate-a-decoder.md): the decoder, its work
  layout and its gradients (`td_gradients`).
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW (`aw_step`)
  and its binary64 state.
- [Chapter 114](ch114-compile-the-backward-pass.md): the general form of
  the recomputation choice, on compiled graphs (`cb_choose`). Here the choice
  is made by hand for one model, one layer at a time.

Three parts of the plan remain, and nothing below claims them:

- A packed reduced-precision store. The binary16 policy rounds gradients to
  binary16 values but holds them in binary64 slots, so it changes numbers,
  not bytes, and the planner does not count it as a saving. Reduced-precision
  forward and backward arithmetic also remains.
- Random-number replay in recomputed blocks. The decoder has no random
  operation (no dropout), so there is nothing to replay yet.
- Wall-clock time. Extra time is reported as counted layer evaluations, which
  are exact and repeatable, not as measured seconds.

## Where the bytes go

One training step of chapter 115 holds five kinds of values, each with its
own lifetime:

- **Parameters** (binary64): the whole run.
- **Optimizer state**: AdamW's two moments, binary64, the whole run.
- **Gradients**: the step's gradient sum lives for one step. Each row in
  flight also has its own gradient until it is added in.
- **Saved values**: what the forward pass keeps for the backward pass, for
  each row in flight.
- **Scratch**: the backward pass's gradients of those saved values, for each
  row in flight.

Count them by hand for chapter 115's shape: width 16, two layers, context 32,
16 symbols, 3,152 parameters. Every value is 8 bytes.

- Parameters: `8 * 3152 = 25216`. Moments: `16 * 3152 = 50432`.
- One layer saves, at each of 32 positions, a slot of
  `4 * 16 + 2 + 2 * 16 + 16 + 2 * 32 = 178` floats plus a key and a value of
  8 each: `32 * (178 + 16) = 6208` floats. The head saves
  `32 * (2 * 16 + 1 + 16) = 1568`. One row saves `2 * 6208 + 1568 = 13984`
  floats, chapter 113's work count, and its scratch is the same size.
- Holding all four rows at once:
  `25216 + 50432 + 8 * 3152 * (1 + 4) + 2 * 4 * 8 * 13984 = 1096704` bytes.
  The saved values and their scratch are 82% of that.

<!-- source: psionic/lib/transformer/budget.coax bytes -->
```jai
// The floats of one layer's saved values plus the final head's: the region a
// recomputed layer is rebuilt in, laid out as a one-layer decoder's.
mb_region :: (s:TdShape)->int {one:=s;one.layers=1;return td_work_count(one);}
// Floats one row saves for the backward pass. Recomputation keeps the input
// of every layer but the last (context x width each) and one region.
mb_saved :: (s:TdShape,recompute:bool)->int {
    if recompute {return (s.layers-1)*s.context*s.width+mb_region(s);}
    return td_work_count(s);
}
// Floats of the backward pass's own scratch (the gradients of the saved
// values) for one row.
mb_scratch :: (s:TdShape,recompute:bool)->int {if recompute {return mb_region(s);}return td_work_count(s);}
// Bytes of one step that holds `micro` rows at once. Parameters and both
// AdamW moments live for the whole run; the step's gradient sum lives for
// one step; each held row has its own gradient, saved values and scratch,
// which live for one micro-batch. Every value is binary64: 8 bytes.
mb_bytes :: (s:TdShape,micro:int,recompute:bool)->MbBytes {
    p:=td_parameter_count(s);b:MbBytes;
    b.parameters=8*p;b.moments=16*p;b.gradients=8*p*(1+micro);
    b.activations=8*micro*mb_saved(s,recompute);b.workspace=8*micro*mb_scratch(s,recompute);
    b.total=b.parameters+b.moments+b.gradients+b.activations+b.workspace;
    return b;
}
// Layer evaluations per step: every row runs every layer at every position
// once, and recomputation runs every layer but the last a second time.
mb_evaluations :: (s:TdShape,batch:int,recompute:bool)->int {
    e:=s.layers;if recompute {e+=s.layers-1;}return batch*s.context*e;
}
```

## Two ways to hold less

**Gradient accumulation.** A step does not need all four rows at once. Run
the rows in micro-batches of 1 or 2, add each row's gradient into the sum,
and update once after the last micro-batch. Two rules keep this identical to
one big batch. First, each row's gradient is divided by the whole batch (4),
not by the micro-batch: dividing by 1 would make the gradient four times too
large. Second, AdamW steps once per batch, so the step count and the
learning-rate schedule do not move between micro-batches. If the rows are
added in the same order, the sum has the same bits. With one row at a time
the step holds `25216 + 50432 + 8 * 3152 * 2 + 2 * 8 * 13984 = 349824`
bytes.

**Recomputation.** Within one row, the backward pass reads a layer's saved
values only while it differentiates that layer. So the forward pass can keep
only each layer's input (32 x 16 floats) and drop the rest. Just before the
backward pass reaches a layer, it rebuilds that layer from its input. The
last layer is differentiated first, while its values are still there, so it
is never rebuilt. For two layers that means one region of
`6208 + 1568 = 7776` floats for saved values, the same for scratch, and 512
floats of saved inputs, against 13,984 and 13,984. One row at a time with
recomputation needs
`25216 + 50432 + 50432 + 8 * (512 + 7776) + 8 * 7776 = 254592` bytes. The
cost is arithmetic: each row runs layer 0 a second time, so a step of 4 rows
evaluates a layer at a position `4 * 32 * 3 = 384` times instead of 256, 50%
more.

A rebuilt layer equals the original only if it repeats the same arithmetic
in the same order. `mb_layer` is chapter 113's per-layer forward code with
three changes. It addresses a one-layer region, so it reads its input from
the region's slot and writes its output where the head reads. It takes the
largest attention score with chapter 113's `td_argmax` instead of a running
maximum; both pick the same value. And it writes the feed-forward ReLU as
`(z + |z|) / 2` instead of a branch: above zero that is `z` exactly, and at
or below zero it adds a zero to a sum that is never negative zero, so every
bit is unchanged. The last two remove comparisons where `>` and `>=` give the
same bits, a change no check could see, so the mutation check stays exact.

<!-- source: psionic/lib/transformer/budget-train.coax layer -->
```jai
// Layer l at position t, computed in the one-layer region: the same values,
// in the same order, as chapter 113's td_advance, reading the layer's input
// from its slot and writing its output where the head reads.
mb_layer :: (s:TdShape,p:[]f64,w:[]f64,l:int,t:int) {
    one:=mb_one(s);d:=s.width;e:=s.head_width;q:=s.heads*e;k:=s.kv_heads*e;group:=s.heads/s.kv_heads;scale:=1/sqrt(f64(e));
    b:=td_block(s,l);o:=td_slot(one,0,t);n1:=o+d;r:=o+2*d;qs:=r+2;cs:=qs+q;m:=cs+q;n2:=m+d;u:=n2+d;a:=u+s.hidden;
    key:=td_keys(one,0)+t*k;value:=td_values(one,0)+t*k;
    w[r]=td_norm(w,o,p,b.norm1,d,w,n1);
    td_matvec(p,b.wq,q,d,w,n1,w,qs);td_matvec(p,b.wk,k,d,w,n1,w,key);td_matvec(p,b.wv,k,d,w,n1,w,value);
    td_rope(w,qs,s.heads,e,f64(t));td_rope(w,key,s.kv_heads,e,f64(t));
    for h:0..<s.heads {g:=h/group;row:=a+h*s.context;top:=0.0;
        for j:0..<t+1 {score:=0.0;for x:0..<e {score+=w[qs+h*e+x]*w[td_keys(one,0)+j*k+g*e+x];}w[row+j]=score*scale;}
        // The largest score, the value td_advance's running maximum finds.
        top=w[row+td_argmax(w[row..<row+t+1])];
        total:=0.0;for j:0..<t+1 {w[row+j]=exp(w[row+j]-top);total+=w[row+j];}for j:0..<t+1 {w[row+j]/=total;}
        for x:0..<e {acc:=0.0;for j:0..<t+1 {acc+=w[row+j]*w[td_values(one,0)+j*k+g*e+x];}w[cs+h*e+x]=acc;}
    }
    td_matvec(p,b.wo,d,q,w,cs,w,m);for i:0..<d {w[m+i]+=w[o+i];}
    w[r+1]=td_norm(w,m,p,b.norm2,d,w,n2);td_matvec(p,b.w1,s.hidden,d,w,n2,w,u);
    next:=td_final(one,t);
    // (z + |z|) / 2 is the ReLU without a branch: z itself above zero, else
    // zero, whose square adds nothing to the sum.
    for i:0..<d {acc:=0.0;for f:0..<s.hidden {z:=(w[u+f]+abs(w[u+f]))/2;acc+=p[b.w2+i*s.hidden+f]*z*z;}w[next+i]=w[m+i]+acc;}
}
```

The gradient driver runs the forward pass layer by layer, saving the input of
each layer but the last. Then it runs the head's backward pass. Then, for
each layer from the last, it rebuilds the layer if needed, clears its scratch
and runs its backward pass. The gradient at a layer's input becomes the
gradient at the output of the layer below it.

<!-- source: psionic/lib/transformer/budget-train.coax recompute -->
```jai
// One row's loss and gradients with recomputation: w and dw hold one region,
// saves holds the input of every layer but the last. The forward pass saves
// those inputs and keeps only the last layer; the backward pass rebuilds
// each earlier layer from its input just before its gradients.
mb_gradients :: (s:TdShape,p:[]f64,tokens:[]int,saves:[]f64,w:[]f64,dw:[]f64,grads:[]f64)->MbRow {
    if !td_valid(s) || p.count!=td_parameter_count(s) || grads.count!=p.count {return MbRow.{status=TdStatus.shape};}
    for x:p {if !(x>=-TD_NUMBER && x<=TD_NUMBER) {return MbRow.{status=TdStatus.number};}}
    status:=td_tokens(s,tokens);if status!=TdStatus.ok {return MbRow.{status=status};}if tokens.count<2 {return MbRow.{status=TdStatus.shape};}
    region:=mb_region(s);if w.count<region || dw.count<region || saves.count<(s.layers-1)*s.context*s.width {return MbRow.{status=TdStatus.capacity};}
    n:=tokens.count;d:=s.width;one:=mb_one(s);evaluations:=0;
    for t:0..<n {o:=td_slot(one,0,t);for i:0..<d {w[o+i]=p[tokens[t]*d+i];}}
    for l:0..<s.layers {
        if l>0 {mb_carry(s,w,n,true);}if l+1<s.layers {mb_inputs(s,w,saves,l,n,true);}
        for t:0..<n {mb_layer(s,p,w,l,t);evaluations+=1;}
    }
    for t:0..<n {mb_head(s,p,w,t);}loss:=td_loss(one,w,tokens);
    for i:0..<grads.count {grads[i]=0;}for i:td_layer_size(one)..<region {dw[i]=0;}
    mb_head_back(s,p,w,dw,tokens,grads);
    l:=s.layers;
    while l>0 {l-=1;
        if l+1<s.layers {mb_inputs(s,w,saves,l,n,false);for t:0..<n {mb_layer(s,p,w,l,t);evaluations+=1;}}
        for i:0..<td_layer_size(one) {dw[i]=0;}
        mb_layer_back(s,p,w,dw,l,n,grads);
        // This layer's input gradient is the next (earlier) layer's output gradient.
        mb_carry(s,dw,n,false);
    }
    for t:0..<n {f:=td_final(one,t);for i:0..<d {grads[tokens[t]*d+i]+=dw[f+i];}}
    return MbRow.{status=TdStatus.ok,loss=loss,evaluations=evaluations};
}
```

A check runs this against `td_gradients` on chapter 113's test shape, on a
two-layer small shape with a short row, on chapter 115's full shape, and on a
three-layer shape (two layers rebuilt). In every case the loss and every
gradient have the same bits, and the counted evaluations are
`n * (2 * layers - 1)` for a row of `n` symbols. Refusals (a bad shape, a
parameter out of range, a bad symbol, a short row, a buffer one float short)
come before any write.

## The planner

The planner tries plans in a fixed order and takes the first that fits:
first without recomputation, then with it, each from the largest micro-batch
that divides the batch down to one row. Accumulation costs no arithmetic on
this path and recomputation does, so the first plan that fits is the fastest.
If none fits, it refuses with the status `budget` and names the smallest
budget that would fit: one row at a time with recomputation, since bytes grow
with the micro-batch and recomputation never costs bytes. A check compares
that number with every candidate.

<!-- source: psionic/lib/transformer/budget.coax plan -->
```jai
// The fastest plan that fits: no recomputation before recomputation, then
// the largest micro-batch that divides the batch. Accumulation costs no
// arithmetic on this path, recomputation does. A refusal names the smallest
// budget any plan needs: one row with recomputation, since bytes grow with
// the micro-batch and recomputation never saves more than it keeps.
mb_plan :: (s:TdShape,batch:int,budget:int)->MbPlan {
    if !td_valid(s) {return MbPlan.{status=MbStatus.shape};}
    if batch<1 || batch>MB_BATCH {return MbPlan.{status=MbStatus.batch};}
    for recompute:[2]bool.[false,true] {
        for j:0..<batch {micro:=batch-j;
            if batch%micro==0 {b:=mb_bytes(s,micro,recompute);
                if b.total<=budget {return MbPlan.{batch=batch,micro=micro,recompute=recompute,bytes=b,evaluations=mb_evaluations(s,batch,recompute)};}}}}
    return MbPlan.{status=MbStatus.budget,batch=batch,need=mb_bytes(s,1,true).total};
}
```

Work through four caps by hand with the totals above:

| Cap (bytes) | Plan | Bytes | Layer evaluations |
| --- | --- | --- | --- |
| 1,100,000 | 4 rows at a time | 1,096,704 | 256 |
| 400,000 | 1 row at a time (2 rows need 598,784) | 349,824 | 256 |
| 300,000 | 1 row at a time, recomputed | 254,592 | 384 |
| 250,000 | refused: `budget`, smallest budget 254,592 | | |

Checks pin each of these, the boundary one byte either side, batches of 3 and
6 (micro-batches must divide the batch), and the refusals for a batch outside
1 to 64 and an invalid shape. The planner is `#authority`, and `coax mutate`
leaves no survivors.

## Low-precision gradients and the loss scale

The third policy keeps parameters, moments and the sum in binary64 but rounds
each row's gradient to binary16, as a mixed-precision trainer stores
gradients. Binary16 has 11 significant bits and spans 2^-24 to 65,504. Small
gradients would flush to zero, so each gradient is multiplied by a loss scale
before rounding and divided by it after. The scale is a power of two from 1
to 2^24, so both operations are exact.

The scale follows a fixed rule. If any scaled gradient overflows (rounds to
infinity), the step is skipped: the rows are consumed, AdamW's skipped count
goes up, nothing else changes, and the scale halves. If nothing overflows but
some nonzero gradient rounds to zero (underflow), the update happens and the
scale doubles. Otherwise the scale stays.

<!-- source: psionic/lib/transformer/budget-train.coax half -->
```jai
// Rounds to the nearest binary16 value, ties to even. Magnitudes from 65520
// up round to infinity, as a hardware cast does; below 2^-14 the spacing is
// the subnormal step 2^-24, and below 2^-25 values flush to zero.
mb_f16 :: (x:f64)->f64 {
    if !(x<65520 && x>-65520) {return x/0.0;}
    // The binade's exponent from the bits, never below -14: under 2^-14 the
    // spacing stays 2^-24.
    a:=abs(x);e:=-nx_min(1023-cast(int)((f64_bits(a)>>52)&2047),14);
    // Adding and removing 1.5 x 2^52 rounds a value below 2^51 to an integer, ties to even.
    ulp:=pow(2.0,f64(e-10));r:=(a/ulp+6755399441055744.0)-6755399441055744.0;
    if x<0 {return -r*ulp;}return r*ulp;
}
// A loss scale is a power of two from 1 to 2^24, so unscaling is exact.
mb_scale_ok :: (x:f64)->bool {return x>=1 && x<=MB_SCALE_MAX && (f64_bits(x)&0xfffffffffffff)==0;}
// Adds one row's gradient, scaled, rounded to binary16 and unscaled, divided
// by the batch, into total. Overflow stops at the first infinite value; an
// underflow is a nonzero gradient that rounds to zero.
mb_half :: (g:[]f64,scale:f64,batch:int,total:[]f64)->MbHalf {
    h:MbHalf;
    for i:0..<g.count {x:=mb_f16(g[i]*scale);if !(x<=MB_HALF_MAX && x>=-MB_HALF_MAX) {h.overflow=true;return h;}if x==0 && g[i]!=0 {h.underflow+=1;}total[i]+=x/scale/f64(batch);}
    return h;
}
// The scale's rule after a step: overflow halves it (not below 1) and the
// step is skipped; any underflow doubles it (not above 2^24); otherwise it stays.
mb_next_scale :: (scale:f64,h:MbHalf)->f64 {
    if h.overflow {if scale>1 {return scale/2;}return scale;}
    if h.underflow>0 && scale<MB_SCALE_MAX {return scale*2;}
    return scale;
}
```

Round two values by hand. `1/3` lies between 0.25 and 0.5, where binary16
values are `2^-12` apart. `1/3 * 4096 = 1365.33`, so it rounds to
`1365 * 2^-12`. `2^-25` lies exactly halfway between 0 and the smallest
positive value, `2^-24`, and ties go to the even neighbour, 0. An independent
check builds every finite binary16 value from its bits and confirms that each
one rounds to itself, that each midpoint rounds to the neighbour with the
even bit pattern, and that values just off the midpoint go to the nearer
side.

The fixtures run the rule on the gradient `[0.01, -1e-9, 0.5]`. At scale
2^24, `0.01 * 2^24 = 167772` overflows: skip, and halve. At 2^16 the largest
scaled value is `0.5 * 65536 = 32768`, which fits, and `-1e-9 * 65536` is
kept, so the scale stays. At scale 1, `1e-9` is below `2^-25` and flushes to
zero: one underflow, and the scale doubles.

## The planned step

The step reads the batch's rows from the cursor and runs them a micro-batch
at a time in the caller's arena. It adds each row's gradient into the sum in
row order (through the binary16 policy if it is on) and makes one AdamW
update at chapter 115's rate and settings. The arena is the plan made
physical: each row in flight gets its gradient, its saved values and its
scratch. A check confirms that the arena's bytes plus the parameters, moments
and sum are the plan's total. An arena one float short is refused.

<!-- source: psionic/lib/transformer/budget-train.coax step -->
```jai
// Floats one step under the plan needs from the caller's arena: each held
// row's gradient, saved values (with the saved layer inputs) and scratch.
mb_floats :: (s:TdShape,plan:MbPlan)->int {return plan.micro*(td_parameter_count(s)+mb_saved(s,plan.recompute)+mb_scratch(s,plan.recompute));}
// Chapter 115's step under a plan: the batch's rows in micro-batches of
// plan.micro, each row's gradient divided by the whole batch and added in
// row order, one AdamW update after the last micro-batch. Under the half
// policy each row's gradient goes through mb_half, an overflow skips the
// update (the rows are still consumed) and the scale follows mb_next_scale.
mb_step :: (s:TdShape,plan:MbPlan,policy:MbPolicy,docs:[]PkDoc,tokens:[]u8,run:RsRun,steps:int,p:[]f64,m:[]f64,v:[]f64,arena:[]f64,total:[]f64)->MbStep {
    refused:=MbStep.{run=run,scale=policy.scale};
    if plan.status!=MbStatus.ok || plan.batch%plan.micro!=0 || p.count!=td_parameter_count(s) || total.count!=p.count {refused.status=MbTrain.plan;return refused;}
    if policy.half && !mb_scale_ok(policy.scale) {refused.status=MbTrain.scale;return refused;}
    floats:=mb_floats(s,plan);if arena.count<floats {refused.status=MbTrain.capacity;return refused;}
    c:=run.cursor;n:=s.context;rows:[4096]int;bytes:[64]u8;
    for b:0..<plan.batch {next:=pk_next(docs,tokens,c,bytes[0..<n],false);if next.status!=PkStatus.ok {refused.status=MbTrain.data;return refused;}c=next.cursor;for t:0..<n {rows[b*n+t]=cast(int)bytes[t];}}
    np:=p.count;saved:=mb_saved(s,plan.recompute);scratch:=mb_scratch(s,plan.recompute);each:=np+saved+scratch;
    loss:=0.0;evaluations:=0;half:MbHalf;losses:[64]f64;
    for i:0..<np {total[i]=0;}
    for first:0..<plan.batch/plan.micro {
        for j:0..<plan.micro {b:=first*plan.micro+j;at:=j*each;g:=arena[at..<at+np];w:=arena[at+np..<at+np+saved];dw:=arena[at+np+saved..<at+each];row:=rows[b*n..<b*n+n];
            if plan.recompute {
                region:=mb_region(s);r:=mb_gradients(s,p,row,w[region..<saved],w[0..<region],dw,g);if r.status!=TdStatus.ok {refused.status=MbTrain.model;return refused;}
                losses[j]=r.loss;evaluations+=r.evaluations;
            } else {
                r:=td_gradients(s,p,row,w,dw,g,false);if r.status!=TdStatus.ok {refused.status=MbTrain.model;return refused;}
                losses[j]=r.loss;evaluations+=n*s.layers;
            }
        }
        for j:0..<plan.micro {at:=j*each;g:=arena[at..<at+np];loss+=losses[j]/f64(plan.batch);
            if policy.half {h:=mb_half(g,policy.scale,plan.batch,total);half.underflow+=h.underflow;if h.overflow {half.overflow=true;}}
            else {for i:0..<np {total[i]+=g[i]/f64(plan.batch);}}}
    }
    next:=RsRun.{cursor=c,state=run.state,revision=run.revision+1};scale:=mb_next_scale(policy.scale,half);
    if half.overflow {next.state.skipped+=1;return MbStep.{run=next,loss=loss,evaluations=evaluations,floats=floats,skipped=true,underflow=half.underflow,scale=scale};}
    groups:=[1]AwGroup.[AwGroup.{first=0,count=np,lr=tt_rate(run.state.step,steps),beta1=0.9,beta2=0.99,eps=0.00000001,decay=0}];none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,p,total,m,v,none[0..<0],false);if a.status!=AwStatus.ok {refused.status=MbTrain.optimizer;return refused;}
    next.state=a.state;
    return MbStep.{run=next,loss=loss,evaluations=evaluations,floats=floats,underflow=half.underflow,scale=scale};
}
```

A check runs three steps of every plan (one, two or four rows at a time, with
and without recomputation) on a two-layer small model and compares them with
three steps of chapter 115's `tt_step`. Every loss, parameter and moment has
the same bits, and the cursor and step count agree. Other checks show that a
refused step changes nothing and that a binary16 first moment stays within
half a binary16 step of the reference's.

## The run

[The example](../examples/train-budget.coax) prints the account for every
plan, trains chapter 115's model for 12 steps with `tt_step` as the
reference, then trains it again under three caps. It shows two broken inputs:
a cap below every plan and an arena one float short. Last, it runs the
binary16 policy from a scale of 2^24, retrying the first step's rows until
the scale stops overflowing. It compares the rest of the run with the
reference against a bound frozen before the run: no skipped step after the
first, and every loss within 0.001.

<!-- source: psionic/examples/train-budget.coax example -->
```jai
main :: () {
    s:=TT_SHAPE;rows:[47]SpRow;tokens:[4096]u8;n:=tt_corpus(rows[..],tokens[..]);docs:[47]PkDoc;
    #assert tt_docs(rows[..],tokens[0..<n],docs[..])==SpStatus.ok;
    print("chapter 115's decoder: % parameters, batch of % rows of %\n",td_parameter_count(s),TT_BATCH,s.context);
    micros:=[3]int.[4,2,1];
    for recompute:[2]bool.[false,true] {for micro:micros {mb_way(micro,recompute);print(": ");mb_say(mb_bytes(s,micro,recompute));print(" bytes; % layer evaluations\n",mb_evaluations(s,TT_BATCH,recompute));}}
    // The reference: chapter 115's step, unchanged.
    rp:[3152]f64;rm:[3152]f64;rv:[3152]f64;w:[13984]f64;dw:[13984]f64;g:[3152]f64;total:[3152]f64;losses:[12]f64;
    seed:=td_init(s,CrSeed.{seed=TT_INIT},rp[..]);start:=rp;run:=RsRun.{cursor=PkCursor.{seed=TT_ORDER}};
    for k:0..<MB_STEPS {r:=tt_step(s,docs[..],tokens[0..<n],run,TT_STEPS,rp[..],rm[..],rv[..],w[..],dw[..],g[..],total[..]);#assert r.status==TtStatus.ok;run=r.run;losses[k]=r.loss;}
    print("reference: % steps, loss % then %\n",MB_STEPS,losses[0],losses[MB_STEPS-1]);
    arena:[124480]f64;full:=mb_bytes(s,TT_BATCH,false);
    for cap:[3]int.[1100000,400000,300000] {
        plan:=mb_plan(s,TT_BATCH,cap);#assert plan.status==MbStatus.ok;
        print("cap % bytes: ",cap);mb_way(plan.micro,plan.recompute);print(", % bytes (% saved), % layer evaluations per step (% extra)\n",plan.bytes.total,full.total-plan.bytes.total,plan.evaluations,plan.evaluations-mb_evaluations(s,TT_BATCH,false));
        p:=start;m:[3152]f64;v:[3152]f64;q:=RsRun.{cursor=PkCursor.{seed=TT_ORDER}};same:=0;floats:=0;counted:=0;
        for k:0..<MB_STEPS {r:=mb_step(s,plan,MbPolicy.{},docs[..],tokens[0..<n],q,TT_STEPS,p[..],m[..],v[..],arena[..],total[..]);#assert r.status==MbTrain.ok;q=r.run;floats=r.floats;counted=r.evaluations;if r.loss==losses[k] {same+=1;}}
        print("  arena % bytes, % evaluations counted; % of % losses identical, weights and moments identical: %\n",8*floats,counted,same,MB_STEPS,tt_same(p[..],rp[..]) && tt_same(m[..],rm[..]) && tt_same(v[..],rv[..]));
    }
    // Broken inputs: a cap below every plan, and an arena one float short.
    low:=mb_plan(s,TT_BATCH,250000);print("cap 250000 bytes: %, smallest budget that fits % bytes\n",low.status,low.need);
    plan:=mb_plan(s,TT_BATCH,300000);short:=mb_step(s,plan,MbPolicy.{},docs[..],tokens[0..<n],run,TT_STEPS,rp[..],rm[..],rv[..],arena[0..<mb_floats(s,plan)-1],total[..]);
    print("arena of % floats for a plan that needs %: %\n",mb_floats(s,plan)-1,mb_floats(s,plan),short.status);
    // The half policy: each row's gradient rounded to binary16 under a loss
    // scale. The first step starts too high and retries its rows until it applies.
    plan=mb_plan(s,TT_BATCH,1100000);p:=start;m:[3152]f64;v:[3152]f64;q:=RsRun.{cursor=PkCursor.{seed=TT_ORDER}};policy:=MbPolicy.{half=true,scale=MB_SCALE_MAX};
    first:=mb_step(s,plan,policy,docs[..],tokens[0..<n],q,TT_STEPS,p[..],m[..],v[..],arena[..],total[..]);print("half policy, step 1: scale %",policy.scale);
    while first.skipped {policy.scale=first.scale;print(" overflows, %",policy.scale);first=mb_step(s,plan,policy,docs[..],tokens[0..<n],q,TT_STEPS,p[..],m[..],v[..],arena[..],total[..]);}
    print(" applies\n");q=first.run;policy.scale=first.scale;worst:=abs(first.loss-losses[0]);skipped:=0;
    for k:1..<MB_STEPS {r:=mb_step(s,plan,policy,docs[..],tokens[0..<n],q,TT_STEPS,p[..],m[..],v[..],arena[..],total[..]);#assert r.status==MbTrain.ok;
        if r.skipped {skipped+=1;}q=r.run;policy.scale=r.scale;worst=nx_maxf(worst,abs(r.loss-losses[k]));}
    far:=0.0;for i:0..<3152 {far=nx_maxf(far,abs(p[i]-rp[i]));}
    print("  % more steps, % skipped, scale now %; largest loss difference from the reference %, largest weight difference %\n",MB_STEPS-1,skipped,policy.scale,worst,far);
    #assert skipped==0 && worst<0.001;
}
```

```shell
coax/bin/coax run psionic/examples/train-budget.coax
psionic/check.sh --only transformer/budget transformer/budget-train
```

The [transcript](../fixtures/train-budget.out) is exact, in debug and in
release:

```text
chapter 115's decoder: 3152 parameters, batch of 4 rows of 32
4 row(s) at a time: parameters 25216, moments 50432, gradients 126080, saved 447488, scratch 447488: total 1096704 bytes; 256 layer evaluations
2 row(s) at a time: parameters 25216, moments 50432, gradients 75648, saved 223744, scratch 223744: total 598784 bytes; 256 layer evaluations
1 row(s) at a time: parameters 25216, moments 50432, gradients 50432, saved 111872, scratch 111872: total 349824 bytes; 256 layer evaluations
4 row(s) at a time, recomputed: parameters 25216, moments 50432, gradients 126080, saved 265216, scratch 248832: total 715776 bytes; 384 layer evaluations
2 row(s) at a time, recomputed: parameters 25216, moments 50432, gradients 75648, saved 132608, scratch 124416: total 408320 bytes; 384 layer evaluations
1 row(s) at a time, recomputed: parameters 25216, moments 50432, gradients 50432, saved 66304, scratch 62208: total 254592 bytes; 384 layer evaluations
reference: 12 steps, loss 2.9069731002090147 then 2.5653537089008
cap 1100000 bytes: 4 row(s) at a time, 1096704 bytes (0 saved), 256 layer evaluations per step (0 extra)
  arena 995840 bytes, 256 evaluations counted; 12 of 12 losses identical, weights and moments identical: true
cap 400000 bytes: 1 row(s) at a time, 349824 bytes (746880 saved), 256 layer evaluations per step (0 extra)
  arena 248960 bytes, 256 evaluations counted; 12 of 12 losses identical, weights and moments identical: true
cap 300000 bytes: 1 row(s) at a time, recomputed, 254592 bytes (842112 saved), 384 layer evaluations per step (128 extra)
  arena 153728 bytes, 384 evaluations counted; 12 of 12 losses identical, weights and moments identical: true
cap 250000 bytes: budget, smallest budget that fits 254592 bytes
arena of 19215 floats for a plan that needs 19216: capacity
half policy, step 1: scale 16777216.0 overflows, 8388608.0 overflows, 4194304.0 overflows, 2097152.0 overflows, 1048576.0 overflows, 524288.0 overflows, 262144.0 overflows, 131072.0 applies
  11 more steps, 0 skipped, scale now 131072.0; largest loss difference from the reference 5.193791967617045e-06, largest weight difference 0.00005972980462903843
```

Each plan's arena is its total less the parameters, moments and gradient sum
(100,864 bytes). Under all three caps the 12 losses, the weights and the
moments are identical to the reference: accumulation and recomputation
changed where values lived, not what was computed. One row at a time cut the
step's memory from 1,096,704 to 349,824 bytes at no arithmetic cost.
Recomputation cut it to 254,592 bytes, 23% of the unconstrained step, for 50%
more layer evaluations.

The binary16 policy overflowed seven times before the first step applied at
2^17, so the largest initial gradient of any row lies between
`65520 / 2^18 = 0.25` and `65520 / 2^17 = 0.5`. After that no step
overflowed or underflowed. Its losses differ from the reference by at most
5.2e-6 over the 12 steps, well within the frozen 0.001, and its weights by at
most 6.0e-5.
