# 118. One trainer for every model

> **Status: partly implemented.** One trainer runs on the CPU and trains
> three existing models (chapter 103's tabular models, chapter 115's tiny
> decoder and chapter 117's module block) to the same bits as their own
> training loops. MNIST MLP and CNN consumers compose the public module,
> autodiff and trainer APIs on the CPU. GPU/compiled CNN training, SGD with
> momentum and Muon behind the same interface, and the trainers of chapters
> 104, 116 and 157 remain.

Each earlier chapter that trains something writes its own loop. Chapter
103's `tb_train` shuffles rows and runs minibatch SGD with weight decay.
Chapter 115's `tt_step` reads packed rows, averages four gradients and
takes an AdamW step on a linear schedule. Chapter 117's example calls
`mdl_sgd` 400 times. Each loop also decides, in its own way, when a batch
ends, what happens to a number that is not finite, and what a checkpoint
holds.

This chapter moves those decisions into one trainer. A model now supplies
only two things: the loss and the mean gradient of one micro-batch. The
trainer owns everything else:

- the **batch order**: rows shuffled once per epoch, or packed rows read
  from [chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md)'s
  stream through [chapter 121](ch121-feed-the-accelerator.md)'s feed;
- **gradient accumulation** over micro-batches and **clipping** of the
  gradient's global norm;
- the **learning-rate schedule** and the **optimizer**: SGD with weight
  decay on chosen spans, or
  [chapter 124](ch124-adamw-and-explicit-precision.md)'s AdamW;
- a **bound** on every new parameter, so a diverging run is refused before
  anything is written;
- the **evaluation cadence** and **early stopping**;
- the **checkpoint**: chapter 122's record plus a short trainer tail, and
  exact **resume**.

Coax has no callbacks yet, so the trainer cannot call the model. Instead
the caller's loop asks the trainer what to do next. `tr_next` names the next
batch, or says that an evaluation is due, or that the run is done.
`tr_add` adds one micro-batch's loss and gradient. `tr_apply` makes the
step. `tr_eval` records an evaluation. The loop has the same shape for every
model; only the two model calls inside it differ. Stopping the loop at any
step boundary leaves a run that `tr_save` can write and `tr_resume` can
restore.

The runnable profile is `trainer-cpu-f64-v1`: binary64 on the CPU, one
thread, every sum in a fixed order, at most 4,096 parameters (chapter 122's
record limit). The library is [trainer.coax](../lib/trainer.coax), which
decides admission and refusals and is marked `#authority`. The models are
driven from [trainer-models.coax](../lib/trainer-models.coax) and
[trainer-module.coax](../lib/trainer-module.coax). They are two files
because chapter 117's tensor stack and chapter 113's decoder both declare a
type named `TdStatus`, so they cannot load into one program. For the same
reason there are two example programs:
[one-trainer.coax](../examples/one-trainer.coax) (chapters 103 and 115) and
[one-trainer-module.coax](../examples/one-trainer-module.coax) (chapter
117).

## The interface

<!-- source: psionic/lib/trainer.coax types -->
```jai
TrSpan :: struct {first:int;count:int;}
// Everything the trainer decides. The optimizer: SGD (rate at most 16) or
// AdamW (rate at most 1); weight decay on the parameters in spans only. The
// schedule: constant, or linear from rate down by drop over the run. Storage
// binary32 rounds every SGD result to f32. clip caps the gradient's global
// norm (0: off); bound refuses any step that would write a parameter larger
// in magnitude. Batches: rows mode walks examples positions in batches,
// shuffled each epoch when shuffle is set; stream mode reads batch packed
// rows of length tokens from chapter 122's stream. A step adds accumulate
// micro-batches. every and patience set evaluation and early stopping.
TrConfig :: struct {
    optimizer:TrOptimizer;schedule:TrSchedule;storage:TrStorage;source:TrSource;
    rate:f64;drop:f64;beta1:f64;beta2:f64;eps:f64;decay:f64;spans:[4]TrSpan;span_count:int;
    clip:f64;accumulate:int=1;bound:f64;
    examples:int;batch:int=1;epochs:int;shuffle:bool;seed:u64;length:int;steps:int;
    every:int;patience:int;
}
// The whole run: data cursor, optimizer counters, steps taken, and the
// evaluation record (best loss, evaluations since it improved, the revision
// last evaluated, and whether early stopping fired).
TrRun :: struct {cursor:PkCursor;state:AwState;revision:int;evals:int;best:f64;stale:int;last:int=-1;stopped:bool;}
TrBatch :: struct {status:TrStatus;revision:int;micro:int;start:int;after:PkCursor;}
TrSum :: struct {status:TrStatus;micro:int;loss:f64;}
TrStep :: struct {status:TrStatus;run:TrRun;loss:f64;rate:f64;norm:f64;}
```

A run has a fixed number of steps. In rows mode it is
`epochs * ceil(ceil(examples / batch) / accumulate)`: every epoch is cut
into micro-batches of `batch` positions (the last one shorter), and a step
takes up to `accumulate` of them without crossing the epoch's end. In
stream mode it is `steps`, and every step reads `accumulate * batch` packed
rows.

`tr_admit` refuses a configuration before any work: a rate outside SGD's
`(0, 16]` or a set of AdamW hyperparameters chapter 124 would refuse, a
`drop` outside `[0, 1]`, a negative or infinite clip, a bound that is not
positive and finite, decay spans that overlap or leave the parameters, and
batch sizes outside the limits. In stream mode a step's rows must fit one
chapter 121 batch (at most 64 rows of 2 to 256 tokens) and the dataset must
pass chapter 122's admission.

## Batches

<!-- source: psionic/lib/trainer.coax next -->
```jai
tr_due :: (c:TrConfig,run:TrRun)->bool {return c.every>0 && run.revision%c.every==0 && run.last!=run.revision;}
// The next step's batch. Rows mode names positions in the epoch's order;
// stream mode fills rows with the step's packed rows through chapter 121's
// feed. A due evaluation comes first.
tr_next :: (c:TrConfig,run:TrRun,docs:[]PkDoc,tokens:[]u8,rows:[]u8)->TrBatch {
    if run.stopped {return TrBatch.{status=TrStatus.stopped};}
    if tr_due(c,run) {return TrBatch.{status=TrStatus.evaluate};}
    if run.revision>=tr_steps(c) {return TrBatch.{status=TrStatus.done};}
    if c.source==TrSource.rows {
        k:=tr_min(c.accumulate,tr_ceil(c.examples-run.cursor.order,c.batch));
        after:=run.cursor;after.order+=k*c.batch;if after.order>=c.examples {after.order=0;after.epoch+=1;}
        return TrBatch.{revision=run.revision,micro=k,start=run.cursor.order,after=after};
    }
    n:=c.accumulate*c.batch;p:=fd_plan(n,c.length,1,1,n*c.length);
    if rows.count!=n*c.length {return TrBatch.{status=TrStatus.shape};}
    // One worker owns every row, so its success is the whole batch.
    cursors:[65]PkCursor;done:[64]u8;f:=fd_cursors(docs,tokens,p,run.cursor,cursors[0..<n+1]);
    if f.status!=FdStatus.ok {return TrBatch.{status=TrStatus.data};}
    if fd_worker(docs,tokens,p,0,cursors[0..<n+1],rows,done[0..<n])!=FdStatus.ok {return TrBatch.{status=TrStatus.data};}
    return TrBatch.{revision=run.revision,micro=c.accumulate,after=f.cursor};
}
// Micro-batch k: positions order[first..<first+count] in rows mode, packed
// rows first..<first+count of the batch in stream mode.
tr_rows :: (c:TrConfig,b:TrBatch,k:int)->TrSpan {
    first:=b.start+k*c.batch;count:=c.batch;if c.source==TrSource.rows {count=tr_min(count,c.examples-first);}
    return TrSpan.{first=first,count=count};
}
// Adds one micro-batch's loss and mean gradient, each divided by the step's
// micro-batch count, in order. The first clears the sum.
tr_add :: (b:TrBatch,s:TrSum,loss:f64,g:[]f64,total:[]f64)->TrSum {
    if s.status!=TrStatus.ok || s.micro>=b.micro {return TrSum.{status=TrStatus.order};}
    if g.count!=total.count {return TrSum.{status=TrStatus.shape};}
    if s.micro==0 {for i:0..<total.count {total[i]=0;}}
    k:=f64(b.micro);for i:0..<total.count {total[i]+=g[i]/k;}
    return TrSum.{micro=s.micro+1,loss=s.loss+loss/k};
}
```

In rows mode the trainer shuffles **positions** `0..examples-1`, not the
rows themselves; the model maps a position to its row. The shuffle is
chapter 103's: Fisher-Yates from the last position down, one SplitMix64
draw per swap, the counter continuing across epochs, so epoch `e` starts at
counter `e * (examples - 1)`. Shuffling positions and then looking rows up
gives the same rows in the same order as shuffling the rows, because each
swap depends only on the draw, never on the values being swapped. The
order for epoch `e` follows from the seed alone (`tr_order`), and a step's
place in the epoch follows from the revision alone (`tr_cursor`), so a
checkpoint needs no copy of the order.

In stream mode the step's rows are chapter 121's batch: `fd_cursors` finds
every row's start from document lengths, and one worker fills the rows with
chapter 122's `pk_next`. The rows are the ones a sequential reader would
get, which the checks confirm row by row.

## The step

<!-- source: psionic/lib/trainer.coax apply -->
```jai
tr_rate :: (c:TrConfig,step:int)->f64 {
    if c.schedule==TrSchedule.constant {return c.rate;}
    return c.rate*(1-c.drop*f64(step)/f64(tr_steps(c)));
}
// One step from a complete sum. total is the trainer's scratch: clipping
// scales it in place. New values go to stage first; p, m and v change only
// when every one is admitted.
tr_apply :: (c:TrConfig,run:TrRun,b:TrBatch,s:TrSum,p:[]f64,m:[]f64,v:[]f64,total:[]f64,stage:[]f64,order:[]int)->TrStep {
    n:=p.count;fail:=TrStep.{run=run};
    if b.status!=TrStatus.ok || s.status!=TrStatus.ok || b.revision!=run.revision || b.micro<1 || s.micro!=b.micro {fail.status=TrStatus.order;return fail;}
    if total.count!=n || m.count!=n || v.count!=n || stage.count<3*n || (c.source==TrSource.rows && order.count<c.examples) {fail.status=TrStatus.shape;return fail;}
    // A non-finite gradient, or one whose squares overflow, makes the sum non-finite.
    sum:=0.0;for x:total {sum+=x*x;}
    if !tr_finite(s.loss) || !tr_finite(sum) {fail.status=TrStatus.number;return fail;}
    norm:=sqrt(sum);
    if c.clip>0 && norm>c.clip {for i:0..<n {total[i]=total[i]*c.clip/norm;}}
    rate:=tr_rate(c,run.state.step);sp:=stage[0..<n];sm:=stage[n..<2*n];sv:=stage[2*n..<3*n];state:=run.state;
    for i:0..<n {sp[i]=p[i];sm[i]=m[i];sv[i]=v[i];}
    if c.optimizer==TrOptimizer.sgd {
        for i:0..<n {
            step:=total[i];if tr_decayed(c,i) {step+=c.decay*p[i];}
            next:=p[i]-rate*step;
            if c.storage==TrStorage.binary32 {if !aw_narrow(next) {fail.status=TrStatus.bound;return fail;}next=f64(f32(next));}
            sp[i]=next;
        }
        state.step+=1;
    } else {
        groups:[9]AwGroup;g:=0;at:=0;none:[1]f32;
        for k:0..<c.span_count {
            sk:=c.spans[k];
            if sk.first>at {groups[g]=AwGroup.{first=at,count=sk.first-at,lr=rate,beta1=c.beta1,beta2=c.beta2,eps=c.eps};g+=1;}
            groups[g]=AwGroup.{first=sk.first,count=sk.count,lr=rate,beta1=c.beta1,beta2=c.beta2,eps=c.eps,decay=c.decay};g+=1;at=sk.first+sk.count;
        }
        if at<n {groups[g]=AwGroup.{first=at,count=n-at,lr=rate,beta1=c.beta1,beta2=c.beta2,eps=c.eps};g+=1;}
        a:=aw_step(groups[0..<g],AwPolicy.{},run.state,sp,total,sm,sv,none[0..<0],false);
        if a.status!=AwStatus.ok {fail.status=TrStatus.optimizer;return fail;}
        state=a.state;
    }
    for i:0..<n {if !(abs(sp[i])<=c.bound) {fail.status=TrStatus.bound;return fail;}}
    for i:0..<n {p[i]=sp[i];m[i]=sm[i];v[i]=sv[i];}
    r:=run;r.state=state;r.revision+=1;r.cursor=b.after;
    if c.source==TrSource.rows && c.shuffle && b.after.epoch>run.cursor.epoch {next:=tr_shuffle(order[0..<c.examples],c.seed,cast(u64)(b.after.epoch*(c.examples-1)));}
    return TrStep.{run=r,loss=s.loss,rate=rate,norm=norm};
}
```

A step is refused, with nothing written, when:

- the sum is incomplete or belongs to another revision (`order`);
- the loss or the sum of squared gradients is not finite (`number`);
- AdamW refuses its own step (`optimizer`);
- any new parameter would be larger in magnitude than `bound`, or, with
  binary32 storage, past f32's range (`bound`).

SGD's step is `p - rate * (g + decay * p)`, with the decay term only inside
the decay spans. AdamW gets one group per span and per gap between spans,
so decay again reaches only the spans. The schedule reads the optimizer's
step count, so a resumed run continues on the same line.

## One step by hand

Take the line `y = w * x + b` with loss half the squared error, two rows
`(x, y) = (1, 1)` and `(2, -1)`, one row per micro-batch, two micro-batches
a step, SGD at rate 1/2, and `w = b = 0`.

| | residual `e = w x + b - y` | gradient `(e x, e)` | loss `e^2 / 2` |
| --- | --- | --- | --- |
| row 0 | -1 | (-1, -1) | 1/2 |
| row 1 | 1 | (2, 1) | 1/2 |

`tr_add` divides each gradient and loss by the step's two micro-batches and
adds them in order: the sum is `(-1/2 + 1, -1/2 + 1/2) = (1/2, 0)` and the
step's loss is `1/4 + 1/4 = 1/2`. That is the mean over both rows, as one
micro-batch of two rows would give. The norm is `sqrt(1/4) = 1/2`.

- Without clipping, `w = 0 - 1/2 * 1/2 = -1/4` and `b` stays 0.
- With the norm capped at 1/4, each component is scaled by
  `(1/4) / (1/2)`: the sum becomes `(1/4, 0)` and `w = -1/8`.
- A gradient `(3, 4)` has norm 5; capped at 2.5 it becomes `(1.5, 2)`.
- With decay 1/2 on `w` only, from `w = b = 1`, `w`'s step is
  `1/2 + 1/2 * 1 = 1`, so `w = 1/2`; `b`'s step is 0, so `b = 1`.

On a linear schedule with rate `r`, drop `d` and `K` steps, step `k` uses
`r * (1 - d * k / K)`. Chapter 115's run is `r = 0.01`, `d = 0.9`,
`K = 400`: 0.01 at step 0, 0.0055 at step 200, 0.001 at the end. The checks
compare each of these values with what the trainer computes.

## Three models, one loop

Each model keeps its own gradient code. Chapter 103's tabular models become
a configuration and a micro-batch call:

<!-- source: psionic/lib/trainer-models.coax tabular -->
```jai
// Chapter 103's SGD as a trainer configuration: weight decay on the weight
// spans (W1 and w2, or w), never a bias; parameters bounded by its domain.
tm_tabular :: (m:TbModel,c:TbConfig,rows:int)->TrConfig {
    t:=TrConfig.{rate=c.rate,decay=c.decay,bound=TB_DOMAIN,examples=rows,batch=tr_min(c.batch,rows),epochs=c.epochs,shuffle=true,seed=c.shuffle_seed};
    hd:=m.hidden*m.features;t.span_count=1;t.spans[0]=TrSpan.{first=0,count=m.features};
    if m.hidden>0 {t.span_count=2;t.spans[0].count=hd;t.spans[1]=TrSpan.{first=hd+m.hidden,count=m.hidden};}
    return t;
}
// One micro-batch: the positions name rows of the training set.
tm_tabular_step :: (m:TbModel,p:[]f64,x:[]f64,y:[]f64,rows:[]int,pick:[]int,g:[]f64)->f64 {
    idx:[256]int;for k:0..<pick.count {idx[k]=rows[pick[k]];}
    tb_gradient(m,p,x,y,idx[0..<pick.count],g);return tb_loss(m,p,x,y,idx[0..<pick.count]);
}
```

The loop that drives it is the caller's, and every model's loop has this
shape:

<!-- source: psionic/tests/trainer-models-checks.coax drive -->
```jai
// The caller's loop, the same shape for every model: ask for a batch,
// evaluate when asked, add each micro-batch, apply. Only the model calls
// differ. Stops when the trainer says so or at revision stop. Evaluation
// is the loss on the held rows.
tm_tabular_drive :: (m:TbModel,c:TrConfig,x:[]f64,y:[]f64,rows:[]int,held:[]int,run:TrRun,p:[]f64,mo:[]f64,vo:[]f64,order:[]int,stop:int,losses:[]f64)->TrDrive {
    out:=TrDrive.{run=run};n:=p.count;g:[161]f64;total:[161]f64;stage:[483]f64;none:[1]PkDoc;nothing:[1]u8;
    while out.run.revision<stop {
        b:=tr_next(c,out.run,none[..],nothing[..],nothing[..]);
        if b.status==TrStatus.evaluate {e:=tr_eval(c,out.run,tb_loss(m,p,x,y,held));if e.status!=TrStatus.ok {out.status=e.status;return out;}out.run=e.run;out.evals+=1;continue;}
        if b.status!=TrStatus.ok {out.status=b.status;return out;}
        s:TrSum;
        for k:0..<b.micro {r:=tr_rows(c,b,k);s=tr_add(b,s,tm_tabular_step(m,p,x,y,rows,order[r.first..<r.first+r.count],g[0..<n]),g[0..<n],total[0..<n]);}
        a:=tr_apply(c,out.run,b,s,p,mo,vo,total[0..<n],stage[0..<3*n],order);if a.status!=TrStatus.ok {out.status=a.status;return out;}
        losses[out.steps]=a.loss;out.run=a.run;out.steps+=1;
    }
    return out;
}
```

Why the run is `tb_train`'s run, bit for bit: `tb_train` closes a batch when
it holds `batch` rows or the epoch runs out, which is `tr_next` with one
micro-batch a step. Its update is `p - rate * (g + decay * p)` on weights
only, which is SGD with the weight spans. With one micro-batch, `tr_add`'s
sum is `0 + g / 1 = g`. And it refuses an update that leaves `TB_DOMAIN`,
which is the trainer's bound.

Chapter 115's decoder reads packed rows. Its step averages four rows'
gradients, adding `grads[i] / 4` to a cleared sum in row order. That is
exactly `tr_add` with four micro-batches of one row:

<!-- source: psionic/lib/trainer-models.coax decoder -->
```jai
// Chapter 115's step as a trainer configuration: AdamW on a linear
// schedule, each step four packed rows of the context length, one row a
// micro-batch, read from the stream at seed TT_ORDER.
tm_decoder :: (s:TdShape,steps:int)->TrConfig {
    return TrConfig.{optimizer=TrOptimizer.adamw,schedule=TrSchedule.linear,source=TrSource.stream,rate=TT_RATE,drop=0.9,beta1=0.9,beta2=0.99,eps=0.00000001,
        bound=TD_NUMBER,batch=1,accumulate=TT_BATCH,length=s.context,steps=steps,seed=TT_ORDER};
}
tm_decoder_step :: (s:TdShape,p:[]f64,row:[]u8,w:[]f64,dw:[]f64,g:[]f64)->TdResult {
    ints:[256]int;for t:0..<row.count {ints[t]=cast(int)row[t];}return td_gradients(s,p,ints[0..<row.count],w,dw,g,false);
}
```

Chapter 117's block holds its values as f32 in one input buffer. The trainer
keeps the trained ones in binary64 and, with binary32 storage, rounds each
update to f32. With the rate given as chapter 117's f32 rate,
`f64(f32(p - rate * g))` is `mdl_sgd`'s
`f32(f64(w) - f64(rate) * f64(g))`:

<!-- source: psionic/lib/trainer-module.coax module -->
```jai
// Chapter 117's block: the trained values are the parameters the tape
// differentiates, in entry order, held as f32 in the model's input buffer.
// The trainer keeps them in binary64 and rounds each update to f32 (binary32
// storage), at chapter 117's f32 rate, so it makes mdl_sgd's update exactly.
tm_module :: (steps:int,every:int)->TrConfig {
    return TrConfig.{storage=TrStorage.binary32,rate=f64(f32(0.02)),bound=AW_F32_MAX,examples=1,epochs=steps,every=every};
}
tm_module_floats :: (f:MdlModel)->int {mode:=mdl_mode(f);n:=0;for i:0..<f.count {if mode.grad[f.entries[i].node] {n+=f.entries[i].size;}}return n;}
// Copies trained values between the trainer's vector and the input buffer.
tm_module_move :: (f:MdlModel,p:[]f64,inputs:[]f32,into:bool) {
    mode:=mdl_mode(f);at:=0;
    for i:0..<f.count {e:=f.entries[i];if !mode.grad[e.node] {continue;}
        for j:0..<e.size {if into {inputs[e.base+j]=f32(p[at+j]);} else {p[at+j]=f64(inputs[e.base+j]);}}at+=e.size;}
}
// One micro-batch: refresh the dropout mask for this step, run the tape,
// read the loss and every trained entry's gradient.
tm_module_step :: (f:MdlModel,t:AdTape,step:int,p:[]f64,inputs:[]f32,g:[]f64)->f64 {
    tm_module_move(f,p,inputs,true);#assert mdl_refresh(f,cast(u64)step+100,inputs)==MdlStatus.ok;
    values:[1024]f32;grads:[1024]f64;flags:[64]u8;out:[1]f32;g32:[64]f32;mode:=mdl_mode(f);
    run:=ad_run(t,AdCursor.{},inputs,values[..],grads[..],flags[..]);#assert run.status==AdStatus.ok;
    #assert ad_value(t,run.cursor,f.program.output,inputs,values[..],out[..])==AdStatus.ok;
    at:=0;
    for i:0..<f.count {e:=f.entries[i];if !mode.grad[e.node] {continue;}
        #assert ad_gradient(t,run.cursor,e.node,grads[..],g32[0..<e.size])==AdStatus.ok;for j:0..<e.size {g[at+j]=f64(g32[j]);}at+=e.size;}
    return f64(out[0]);
}
```

## Evaluation and early stopping

<!-- source: psionic/lib/trainer.coax eval -->
```jai
// Records a due evaluation's loss. patience evaluations in a row that do not
// improve on the best stop the run.
tr_eval :: (c:TrConfig,run:TrRun,loss:f64)->TrState {
    if !tr_due(c,run) {return TrState.{status=TrStatus.order,run=run};}
    if !tr_finite(loss) {return TrState.{status=TrStatus.number,run=run};}
    r:=run;r.last=run.revision;r.evals+=1;
    if run.evals==0 || loss<run.best {r.best=loss;r.stale=0;} else {r.stale+=1;}
    if c.patience>0 && r.stale>=c.patience {r.stopped=true;}
    return TrState.{run=r};
}
```

An evaluation is due at every revision that is a multiple of `every`,
including revision 0 and the last. `tr_next` returns `evaluate` until the
caller records it, so a loop cannot skip the cadence. The caller computes
the loss (on validation rows, a held-out stream, or in eval mode); the
trainer only keeps the record. With patience 2 and losses 3, 3, 2, 2, 5,
the best becomes 3, then 2; the run stops at the fifth evaluation, the
second in a row that does not beat 2.

## Checkpoints

A trainer checkpoint is chapter 122's record (parameters, both AdamW
moments, the data cursor and the optimizer's counters, signed) followed by
an 80-byte tail with the evaluation record. For `n` parameters it is
`228 + 24n + 32 + 80` bytes: 388 bytes for the line's two parameters, 5,620
for the small decoder's 220. The record's optimizer identity is the SHA-256
of every configuration field, so a checkpoint resumes only into the same
configuration.

<!-- source: psionic/lib/trainer.coax save -->
```jai
// The optimizer identity a checkpoint must match: every configuration field.
tr_identity :: (c:TrConfig)->[32]u8 {
    w:=[31]u64.[cast(u64)cast(int)c.optimizer,cast(u64)cast(int)c.schedule,cast(u64)cast(int)c.storage,cast(u64)cast(int)c.source,
        f64_bits(c.rate),f64_bits(c.drop),f64_bits(c.beta1),f64_bits(c.beta2),f64_bits(c.eps),f64_bits(c.decay),
        cast,trunc(u64)c.spans[0].first,cast,trunc(u64)c.spans[0].count,cast,trunc(u64)c.spans[1].first,cast,trunc(u64)c.spans[1].count,
        cast,trunc(u64)c.spans[2].first,cast,trunc(u64)c.spans[2].count,cast,trunc(u64)c.spans[3].first,cast,trunc(u64)c.spans[3].count,cast,trunc(u64)c.span_count,
        f64_bits(c.clip),cast,trunc(u64)c.accumulate,f64_bits(c.bound),cast,trunc(u64)c.examples,cast,trunc(u64)c.batch,cast,trunc(u64)c.epochs,
        cast(u64)(ifx c.shuffle then 1 else 0),c.seed,cast,trunc(u64)c.length,cast,trunc(u64)c.steps,cast,trunc(u64)c.every,cast,trunc(u64)c.patience];
    b:[248]u8;at:=0;for x:w {at=ck_put64(b[..],at,x);}
    return ck_digest("psionic.trainer-config.v1",b[0..<at]);
}
tr_ids :: (c:TrConfig,model:[32]u8,tokenizer:[32]u8,dataset:[32]u8)->CkIds {return CkIds.{model=model,tokenizer=tokenizer,dataset=dataset,optimizer=tr_identity(c)};}
tr_size :: (n:int)->int {return ck_size(n)+TR_TAIL;}
// The evaluation record a run can hold: a finite best, at most one
// evaluation per revision, no more stale evaluations than evaluations.
tr_record :: (r:TrRun)->bool {return tr_finite(r.best) && r.evals>=0 && r.evals<=r.revision+1 && r.stale>=0 && r.stale<=r.evals && r.last>=-1 && r.last<=r.revision;}
// The 80-byte tail: "PSTR" u16 version=1 u16 0; evaluations, evaluations
// since the best, last evaluated revision + 1, stopped (u64 each); the best
// loss (binary64); SHA-256 (tag psionic.trainer.v1) of the record's digest
// followed by the five fields (bytes 8 to 47).
tr_tail_digest :: (d:[32]u8,t:[]u8)->[32]u8 {
    b:[72]u8;for i:0..<32 {b[i]=d[i];}for i:8..<48 {b[24+i]=t[i];}
    return ck_digest("psionic.trainer.v1",b[..]);
}
tr_tail_write :: (d:[32]u8,run:TrRun,t:[]u8) {
    t[0]=80;t[1]=83;t[2]=84;t[3]=82;t[4]=1;t[5]=0;t[6]=0;t[7]=0;
    at:=ck_put64(t,8,cast,trunc(u64)run.evals);at=ck_put64(t,at,cast,trunc(u64)run.stale);
    at=ck_put64(t,at,cast,trunc(u64)(run.last+1));at=ck_put64(t,at,cast(u64)(ifx run.stopped then 1 else 0));at=ck_put64(t,at,f64_bits(run.best));
    at=ck_put(t,at,tr_tail_digest(d,t));
}
// Reads an 80-byte tail signed over digest d into the evaluation record of
// a run at revision.
tr_tail :: (t:[]u8,d:[32]u8,revision:int)->TrResume {
    if !ck_magic(t,84,82) {return TrResume.{status=CkStatus.magic};}
    if t[4]!=1 || t[5]!=0 || t[6]!=0 || t[7]!=0 {return TrResume.{status=CkStatus.version};}
    if !ck_equal(tr_tail_digest(d,t),ck_get(t,48)) {return TrResume.{status=CkStatus.corrupt};}
    evals:=ck_get64(t,8);stale:=ck_get64(t,16);last:=ck_get64(t,24);stopped:=ck_get64(t,32);best:=ck_get64(t,40);top:=cast(u64)(revision+1);
    if evals>top || stale>evals || last>top || stopped>1 || !ck_bits(best,false) {return TrResume.{status=CkStatus.corrupt};}
    return TrResume.{run=TrRun.{revision=revision,evals=cast(int)evals,stale=cast(int)stale,last=cast(int)last-1,stopped=stopped==1,best=f64_from_bits(best)}};
}
// Writes the run as chapter 122's record, the tail, and the pointer naming
// the record. Nothing is written for a run record that cannot be true.
tr_save :: (ids:CkIds,run:TrRun,p:[]f64,m:[]f64,v:[]f64,bytes:[]u8,pointer:[]u8)->CkStatus {
    n:=p.count;if bytes.count!=tr_size(n) || pointer.count!=CK_POINTER {return CkStatus.shape;}
    if !tr_record(run) {return CkStatus.corrupt;}
    body:=ck_size(n);s:=ck_write(CkMeta.{ids=ids,revision=run.revision,cursor=run.cursor,state=run.state},p,m,v,bytes[0..<body],false);if s!=CkStatus.ok {return s;}
    d:=ck_get(bytes,body-32);tr_tail_write(d,run,bytes[body..<bytes.count]);
    return ck_pointer_write(run.revision,d,pointer);
}
// Restores p, m, v, the batch order and the run from a record whose pointer
// is current, for the same configuration. Every check passes before
// anything is written.
tr_resume :: (c:TrConfig,ids:CkIds,pointer:[]u8,bytes:[]u8,docs:[]PkDoc,tokens:[]u8,p:[]f64,m:[]f64,v:[]f64,order:[]int)->TrResume {
    n:=p.count;rows:=c.source==TrSource.rows;
    if tr_admit(c,n,docs,tokens)!=TrStatus.ok || m.count!=n || v.count!=n || (rows && order.count<c.examples) {return TrResume.{status=CkStatus.shape};}
    if bytes.count<tr_size(n) {return TrResume.{status=CkStatus.truncated};}
    if bytes.count>tr_size(n) {return TrResume.{status=CkStatus.corrupt};}
    q:=ck_pointer_read(pointer);if q.status!=CkStatus.ok {return TrResume.{status=q.status};}
    body:=ck_size(n);sp:[4096]f64;sm:[4096]f64;sv:[4096]f64;
    // The optimizer identity is this configuration's, whatever ids says.
    want:=ids;want.optimizer=tr_identity(c);
    r:=ck_read(bytes[0..<body],want,sp[0..<n],sm[0..<n],sv[0..<n],false);s:=ck_current(q,r);if s!=CkStatus.ok {return TrResume.{status=s};}
    t:=tr_tail(bytes[body..<bytes.count],r.digest,r.meta.revision);if t.status!=CkStatus.ok {return t;}
    run:=t.run;run.cursor=r.meta.cursor;run.state=r.meta.state;
    if run.revision>tr_steps(c) || run.state.step!=run.revision || run.state.skipped!=0 {return TrResume.{status=CkStatus.corrupt};}
    if rows && !fd_same(run.cursor,tr_cursor(c,run.revision)) {return TrResume.{status=CkStatus.corrupt};}
    if !rows && (!pk_cursor(docs,run.cursor) || run.cursor.seed!=c.seed) {return TrResume.{status=CkStatus.corrupt};}
    for i:0..<n {p[i]=sp[i];m[i]=sm[i];v[i]=sv[i];}
    if rows {tr_order(c,run.cursor.epoch,order);}
    return TrResume.{run=run};
}
```

`tr_resume` checks everything before it writes anything: the pointer names
this record (`stale` otherwise), chapter 122 admits the record against the
model, tokenizer, dataset and this configuration's identity, the tail's
digest covers the record's digest, the evaluation record is one a run could
hold, the optimizer's step count equals the revision, and the cursor is the
one the revision gives (rows mode) or one the stream holds (stream mode).
It then restores the order of the cursor's epoch. Fixed parts of a model
that the trainer does not train, such as chapter 117's frozen norm and
scale buffer, come from the model's own initialization.

## The programs

<!-- source: psionic/examples/one-trainer.coax example -->
```jai
main :: () {
    // Chapter 103's deadline data, features scaled on the training rows.
    x:[9600]f64;xs:[9600]f64;y:[2400]f64;lat:[2400]f64;g:[2400]int;tr:[2400]int;va:[2400]int;mean:[4]f64;sd:[4]f64;none:[1]PkDoc;nothing:[1]u8;
    sim:=tb_simulate(70,300,x[..],y[..],lat[..],g[..],false);
    rows:=tr[0..<tb_rows(g[..],0,tr[..],false)];held:=va[0..<tb_rows(g[..],1,va[..],false)];
    #assert tb_fit_scale(x[..],4,rows,mean[..],sd[..],false)==TbStatus.ok && tb_apply_scale(x[..],4,mean[..],sd[..],xs[..],false)==TbStatus.ok;
    print("chapter 103: % requests, % training rows, % validation rows\n",sim.rows,rows.count,held.count);
    names:=[2]string.["logistic","MLP (8 hidden, decay 0.01)"];
    models:=[2]TbModel.[TbModel.{features=4,logistic=true},TbModel.{features=4,hidden=8,logistic=true}];
    configs:=[2]TbConfig.[TbConfig.{epochs=40,batch=16,rate=0.5,init_seed=1,shuffle_seed=2},TbConfig.{epochs=40,batch=16,rate=0.5,decay=0.01,init_seed=1,shuffle_seed=2}];
    for k:0..<2 {
        m:=models[k];c:=configs[k];n:=tb_param_count(m);ref:[161]f64;order:[2400]int;
        want:=tb_train(m,c,xs[..],y[..],rows,ref[0..<n],order[..],false);
        tc:=tm_tabular(m,c,rows.count);tc.every=500;p:[161]f64;mo:[161]f64;vo:[161]f64;o:[2400]int;losses:[8192]f64;
        tb_initialize(m,c.init_seed,p[0..<n]);run:=tr_begin(tc,n,none[..],nothing[..],o[..]).run;
        d:=tm_tabular_drive(m,tc,xs[..],y[..],rows,held,run,p[0..<n],mo[0..<n],vo[0..<n],o[..],1048576,losses[..]);
        print("  %: %, % steps, training loss %; % evaluations, best validation loss %\n",names[k],d.status,d.steps,r6(tb_loss(m,p[0..<n],xs[..],y[..],rows)),d.evals,r6(d.run.best));
        print("    tb_train: % steps; weights and loss the same bits: %\n",want.steps,tr_same(p[0..<n],ref[0..<n]) && tb_loss(m,p[0..<n],xs[..],y[..],rows)==want.loss);
    }
    // The MLP cut after step 1000, saved, and resumed in fresh buffers.
    m:=models[1];c:=configs[1];tc:=tm_tabular(m,c,rows.count);tc.every=500;n:=tb_param_count(m);
    p:[161]f64;mo:[161]f64;vo:[161]f64;o:[2400]int;losses:[8192]f64;tb_initialize(m,c.init_seed,p[0..<n]);
    part:=tm_tabular_drive(m,tc,xs[..],y[..],rows,held,tr_begin(tc,n,none[..],nothing[..],o[..]).run,p[0..<n],mo[0..<n],vo[0..<n],o[..],1000,losses[..]);
    ids:=tr_ids(tc,ck_fill(1),ck_fill(2),ck_fill(3));bytes:[4196]u8;pointer:[80]u8;
    #assert tr_save(ids,part.run,p[0..<n],mo[0..<n],vo[0..<n],bytes[0..<tr_size(n)],pointer[..])==CkStatus.ok;
    q:[161]f64;qm:[161]f64;qv:[161]f64;qo:[2400]int;
    back:=tr_resume(tc,ids,pointer[..],bytes[0..<tr_size(n)],none[..],nothing[..],q[0..<n],qm[0..<n],qv[0..<n],qo[..]);
    rest:=tm_tabular_drive(m,tc,xs[..],y[..],rows,held,back.run,q[0..<n],qm[0..<n],qv[0..<n],qo[..],1048576,losses[..]);
    whole:=tm_tabular_drive(m,tc,xs[..],y[..],rows,held,part.run,p[0..<n],mo[0..<n],vo[0..<n],o[..],1048576,losses[..]);
    print("  MLP resumed at step % (epoch %, row %) from % bytes: %; same weights as the whole run: %\n",back.run.revision,back.run.cursor.epoch,back.run.cursor.order,tr_size(n),back.status,tr_same(q[0..<n],p[0..<n]) && rest.steps==whole.steps);
    other:=tc;other.batch=8;print("  resumed as a run with batch 8: %\n",tr_resume(other,ids,pointer[..],bytes[0..<tr_size(n)],none[..],nothing[..],q[0..<n],qm[0..<n],qv[0..<n],qo[..]).status);
    // Unscaled features at rate 16 diverge: both trainers refuse.
    line:=TbModel.{features=4};wild:=TbConfig.{epochs=2,batch=16,rate=16,init_seed=1,shuffle_seed=2};ref:[5]f64;order:[2400]int;
    print("  unscaled latency line at rate 16: tb_train %, trainer %\n",tb_train(line,wild,x[..],lat[..],rows,ref[..],order[..],false).status,
        tm_tabular_run(line,wild,x[..],lat[..],rows,p[0..<5]).status);

    // Chapter 115's decoder, the small shape, 60 steps.
    f:=tt_fix();s:=TT_SMALL;docs:=f.docs[..];tokens:=f.tokens[0..<f.n];hb:[4096]int;nh:=tt_stream(docs,tokens,PkSplit.validation,hb[..]);
    rp:[220]f64;rm:[220]f64;rv:[220]f64;w:[600]f64;dw:[600]f64;gr:[220]f64;total:[220]f64;want:[60]f64;
    rr:=tt_start(rp[..]);for k:0..<60 {r:=tt_step(s,docs,tokens,rr,60,rp[..],rm[..],rv[..],w[..],dw[..],gr[..],total[..]);rr=r.run;want[k]=r.loss;}
    dc:=tm_decoder(s,60);dc.every=20;dp:[220]f64;dm:[220]f64;dv:[220]f64;got:[60]f64;one:[1]int;seed:=td_init(s,CrSeed.{seed=TT_INIT},dp[..]);
    d:=tm_decoder_drive(s,dc,docs,tokens,hb[0..<nh],tr_begin(dc,220,docs,tokens,one[..]).run,dp[..],dm[..],dv[..],30,got[..]);
    did:=tr_ids(dc,tt_model(s),ck_fill(2),ck_fill(3));db:[5620]u8;dpt:[80]u8;#assert tr_save(did,d.run,dp[..],dm[..],dv[..],db[..],dpt[..])==CkStatus.ok;
    e:=tm_decoder_drive(s,dc,docs,tokens,hb[0..<nh],d.run,dp[..],dm[..],dv[..],1048576,got[30..<60]);
    same:=0;for k:0..<60 {if f64_bits(got[k])==f64_bits(want[k]) {same+=1;}}
    print("\nchapter 115: % parameters, AdamW on a linear schedule, 4 rows of 8 a step: %, % steps, % evaluations\n",td_parameter_count(s),e.status,d.steps+e.steps,d.evals+e.evals);
    print("  loss at step 1 %, at step 60 %; best held-out loss %\n",r6(got[0]),r6(got[59]),r6(e.run.best));
    print("  tt_step: % of 60 losses the same bits; weights and moments the same bits: %\n",same,tr_same(dp[..],rp[..]) && tr_same(dm[..],rm[..]) && tr_same(dv[..],rv[..]));
    print("  weights: sha256 ");hex8(tt_digest(dp[..]));print("...\n");
    q2:[220]f64;qm2:[220]f64;qv2:[220]f64;b2:=tr_resume(dc,did,dpt[..],db[..],docs,tokens,q2[..],qm2[..],qv2[..],one[..]);again:[30]f64;
    a2:=tm_decoder_drive(s,dc,docs,tokens,hb[0..<nh],b2.run,q2[..],qm2[..],qv2[..],1048576,again[..]);
    print("  resumed at step % from % bytes: %; % steps, weights and moments the same bits: %\n",b2.run.revision,db.count,b2.status,a2.steps,tr_same(q2[..],dp[..]) && tr_same(qm2[..],dm[..]) && tr_same(qv2[..],dv[..]));
    bad:=db;bad[5600]^=1;print("  resume with one tail byte changed: %\n",tr_resume(dc,did,dpt[..],bad[..],docs,tokens,q2[..],qm2[..],qv2[..],one[..]).status);
    print("  resume as a 61-step run: %\n",tr_resume(tm_decoder(s,61),did,dpt[..],db[..],docs,tokens,q2[..],qm2[..],qv2[..],one[..]).status);
}
```

```shell
coax/bin/coax run psionic/examples/one-trainer.coax
coax/bin/coax run psionic/examples/one-trainer-module.coax
coax/bin/coax test psionic/lib/trainer.coax
coax/bin/coax test psionic/lib/trainer-models.coax
coax/bin/coax test psionic/lib/trainer-module.coax
```

```text
chapter 103: 2400 requests, 1440 training rows, 480 validation rows
  logistic: done, 3600 steps, training loss 0.10144; 8 evaluations, best validation loss 0.107442
    tb_train: 3600 steps; weights and loss the same bits: true
  MLP (8 hidden, decay 0.01): done, 3600 steps, training loss 0.112608; 8 evaluations, best validation loss 0.115894
    tb_train: 3600 steps; weights and loss the same bits: true
  MLP resumed at step 1000 (epoch 11, row 160) from 1516 bytes: ok; same weights as the whole run: true
  resumed as a run with batch 8: optimizer
  unscaled latency line at rate 16: tb_train update, trainer bound

chapter 115: 220 parameters, AdamW on a linear schedule, 4 rows of 8 a step: done, 60 steps, 4 evaluations
  loss at step 1 2.936081, at step 60 2.580775; best held-out loss 2.562791
  tt_step: 60 of 60 losses the same bits; weights and moments the same bits: true
  weights: sha256 938b9a3e2363dc1f...
  resumed at step 30 from 5620 bytes: ok; 30 steps, weights and moments the same bits: true
  resume with one tail byte changed: corrupt
  resume as a 61-step run: optimizer
```

<!-- source: psionic/examples/one-trainer-module.coax example -->
```jai
main :: () {
    // Chapter 117's block with norm frozen, dropout 128/256, its example's inputs.
    m:=mdl_test_block(128);f:=mdl_freeze(m,"norm",true);n:=tm_module_floats(f);none:[1]PkDoc;nothing:[1]u8;
    // The reference: chapter 117's own loop, mdl_sgd at rate 0.02.
    ref:[256]f32;tm_module_inputs(m,ref[..]);t:=ad_tape(f.program,mdl_mode(f),OrRegistry.{});
    values:[1024]f32;grads:[1024]f64;flags:[64]u8;g84:[84]f32;out:[1]f32;want:[400]f64;
    for step:0..<400 {
        #assert mdl_refresh(f,cast(u64)step+100,ref[..])==MdlStatus.ok;run:=ad_run(t,AdCursor.{},ref[..],values[..],grads[..],flags[..]);
        #assert ad_value(t,run.cursor,26,ref[..],values[..],out[..])==AdStatus.ok;want[step]=f64(out[0]);
        #assert mdl_sgd(f,t,run.cursor,grads[..],g84[..],ref[..],0.02)==MdlStatus.ok;
    }
    // The trainer: the same block, an evaluation every 100 steps, cut after step 200.
    c:=tm_module(400,100);inputs:[256]f32;tm_module_inputs(m,inputs[..]);p:[76]f64;pm:[76]f64;pv:[76]f64;order:[1]int;got:[400]f64;evals:[5]f32;
    tm_module_move(f,p[..],inputs[..],false);
    print("chapter 117: % trained floats (norm frozen), SGD at rate %, binary32 storage\n",n,c.rate);
    d:=tm_module_drive(f,c,tr_begin(c,n,none[..],nothing[..],order[..]).run,p[..],pm[..],pv[..],inputs[..],order[..],200,got[..],evals[..]);
    ids:=tr_ids(c,ck_fill(1),ck_fill(2),ck_fill(3));bytes:[2164]u8;pointer:[80]u8;#assert tr_save(ids,d.run,p[..],pm[..],pv[..],bytes[..],pointer[..])==CkStatus.ok;
    e:=tm_module_drive(f,c,d.run,p[..],pm[..],pv[..],inputs[..],order[..],1048576,got[200..<400],evals[2..<5]);
    for k:0..<5 {print("  step %: eval loss %\n",100*k,r6(f64(evals[k])));}
    same:=0;for k:0..<400 {if f64_bits(got[k])==f64_bits(want[k]) {same+=1;}}
    weights:=true;for i:0..<f.count {x:=f.entries[i];if mdl_saved(x) {for j:0..<x.size {weights=weights && f32_bits(inputs[x.base+j])==f32_bits(ref[x.base+j]);}}}
    print("  %, % steps; mdl_sgd's loop: % of 400 losses and every saved float the same bits: %\n",e.status,d.steps+e.steps,same,weights);
    // A fresh buffer: the model's own init rebuilds the fixed entries, the record the rest.
    fresh:[256]f32;tm_module_inputs(m,fresh[..]);r:[76]f64;rm:[76]f64;rv:[76]f64;more:[5]f32;again:[400]f64;
    back:=tr_resume(c,ids,pointer[..],bytes[..],none[..],nothing[..],r[..],rm[..],rv[..],order[..]);
    a:=tm_module_drive(f,c,back.run,r[..],rm[..],rv[..],fresh[..],order[..],1048576,again[..],more[..]);
    print("  resumed at step % from % bytes: %; % steps, weights the same bits: %\n",back.run.revision,bytes.count,back.status,a.steps,tr_same(r[..],p[..]));
    // Refusals.
    print("  resume as a 399-step run: %\n",tr_resume(tm_module(399,100),ids,pointer[..],bytes[..],none[..],nothing[..],r[..],rm[..],rv[..],order[..]).status);
    later:[2164]u8;lp:[80]u8;#assert tr_save(ids,a.run,r[..],rm[..],rv[..],later[..],lp[..])==CkStatus.ok;
    print("  the step-200 record with the finished run's pointer: %\n",tr_resume(c,ids,lp[..],bytes[..],none[..],nothing[..],r[..],rm[..],rv[..],order[..]).status);
    zero:=c;zero.rate=0;print("  a run at rate 0: %\n",tr_begin(zero,n,none[..],nothing[..],order[..]).status);
}
```

```text
chapter 117: 76 trained floats (norm frozen), SGD at rate 0.019999999552965164, binary32 storage
  step 0: eval loss 9.122487
  step 100: eval loss 1.054721
  step 200: eval loss 1.211376
  step 300: eval loss 0.850827
  step 400: eval loss 0.903775
  done, 400 steps; mdl_sgd's loop: 400 of 400 losses and every saved float the same bits: true
  resumed at step 200 from 2164 bytes: ok; 200 steps, weights the same bits: true
  resume as a 399-step run: optimizer
  the step-200 record with the finished run's pointer: stale
  a run at rate 0: config
```

What the output shows:

- Chapter 103's logistic model and MLP train through the trainer, with a
  validation evaluation every 500 steps, to weights and a final loss with
  the same bits as `tb_train`'s.
- The MLP, cut after step 1,000 in the middle of epoch 11 and resumed from
  a 1,516-byte checkpoint, finishes with the same weights as the
  uninterrupted run. A configuration with another batch size is refused
  (`optimizer`).
- Unscaled features at rate 16 diverge. `tb_train` refuses with `update`;
  the trainer refuses with `bound`, before writing the step.
- Chapter 115's small decoder: all 60 losses, the weights and both AdamW
  moments have the same bits as `tt_step`'s; the first loss, 2.936081, is
  chapter 115's pinned first loss. Resumed at step 30, it finishes with the
  same bits. A damaged tail byte gives `corrupt`; a 61-step configuration
  gives `optimizer`.
- Chapter 117's block: the five evaluation losses are the ones chapter
  117's example prints (9.122487 down to 0.903775), and all 400 losses and
  every saved float have the same bits as the `mdl_sgd` loop. A resume from
  step 200 finishes the same; a record that the pointer does not name gives
  `stale`, and a rate of 0 gives `config`.

## How it is checked

[trainer.coax](../lib/trainer.coax) holds the trainer's checks, written
before the code, with their models in
[trainer-checks.coax](../tests/trainer-checks.coax):

- **Admission:** every field just outside its range is refused and every
  edge admitted; AdamW hyperparameters through chapter 124's own admission;
  decay spans that overlap, are empty, or leave the parameters.
- **Order:** each epoch's order is a permutation; epoch `e` is epoch `e - 1`
  shuffled once more from counter `e * (n - 1)`; a two-position shuffle
  against the draw by hand; no shuffle keeps the order across epochs.
- **Batches:** micro-batch sizes and step counts for 5 rows in batches of
  2; the cursor a revision implies; stream rows against a sequential
  `pk_next` reader, row by row.
- **Accumulation, clipping, decay and storage:** the hand values above,
  including a cap equal to the norm where scaling would round.
- **AdamW:** the trainer's step against chapter 124's `aw_step` with the
  same three groups, bit for bit, and against the hand value.
- **Refusals:** every refusal leaves parameters and moments as they were.
- **Evaluation:** the cadence, the hand sequence above, patience 0 and 1.
- **Resume:** a rows run with shuffling, clipping, a linear schedule and
  evaluations, cut mid-epoch, resumes into scrambled buffers and finishes
  with the same losses and weights; likewise a stream run with AdamW. Every
  damaged byte, short or long buffer, other configuration, other model,
  stale pointer and impossible run record is refused with nothing written.
- **Identity:** every configuration field changes the optimizer identity,
  and the identity and the tail digest are pinned by their first bytes.
- **Mutation:** `coax mutate psionic/lib/trainer.coax --root-only` kills
  every mutant.

[trainer-models.coax](../lib/trainer-models.coax) and
[trainer-module.coax](../lib/trainer-module.coax) run each model's own
trainer beside the shared one: three tabular models (logistic, MLP with
decay, and a line with decay on a batch larger than the data), a tabular
run cut mid-epoch and resumed, the small decoder for four steps with a cut
after step 2, and chapter 117's block for four steps with evaluations, a
cut and a resume into a rebuilt buffer. Losses, weights and moments are
compared bit for bit. Both files' mutants are all killed as well.

## A new model: an MLP on real MNIST

The classifier is a new consumer of the framework. It uses chapter 117's
public module constructors, chapter 106's autodiff and this chapter's
trainer protocol; it does not add a model-specific tensor operator or change
the engine to recognize MNIST. Its caller-owned execution adapter is
[module-train.coax](../lib/module-train.coax). The older `tm_module_step`
helper remains a bounded adapter for chapter 117's small residual fixture.

### Data and the model

The [portable example](../examples/mnist-classifier.coax) exercises the complete
3,322-value gradient interface on a hand-worked zero network, and refuses an
unrepresentable compute weight.
It downloads no data and does not claim classifier accuracy.

At zero weights the hidden values and all scores are zero. For label 7,
the squared error is `(0 - 1)^2 = 1`; the derivative for `fc2.bias[7]` is
`2 * (0 - 1) = -2`. The other last-bias derivatives are zero, and the earlier
derivatives vanish because the second layer's weights are zero. That gives
an independent expected value for every one of the 3,322 gradients below.

<!-- source: psionic/examples/mnist-classifier.coax example -->
```jai
main :: () {
    m:=mc_model();q:=mtr_plan(m);c:=mc_config();
    #assert q.status==MtrStatus.ok;
    print("MNIST MLP: 196 -> 16 -> 10; % parameters, % nodes\n",q.parameters,m.program.count);
    print("caller arenas: % inputs, % values, % adjoints, % flags; largest entry %\n",q.inputs,q.values,q.adjoints,q.flags,q.largest);
    print("AdamW: % epochs, % rows, % rows per update, % updates\n",c.epochs,c.examples,c.accumulate,tr_steps(c));
    // This hand-worked zero model is a synthetic API example, not MNIST
    // accuracy. Its only nonzero derivative is bias[7]: -2 at zero.
    p:[3322]f64;w:[1]McWork;#assert mn_target(7,w[0].inputs[196..<206])==MnStatus.ok;
    r:=mtr_run(q,0,p[..],w[0].inputs[..],w[0].values[..],w[0].adjoints[..],w[0].flags[..],w[0].entry[..],w[0].staging[..],w[0].gradient[..]);
    #assert r.status==MtrStatus.ok && r.loss==1 && w[0].gradient[3319]== -2;
    print("synthetic zero model: loss %, bias[7] gradient %, master unchanged %\n",r.loss,w[0].gradient[3319],p[3319]==0);
    p[0]=1e100;w[0].gradient[0]=123;
    r=mtr_run(q,0,p[..],w[0].inputs[..],w[0].values[..],w[0].adjoints[..],w[0].flags[..],w[0].entry[..],w[0].staging[..],w[0].gradient[..]);
    print("unrepresentable compute weight: %, gradient unchanged %\n",r.status,w[0].gradient[0]==123);
    print("No dataset downloaded and no training experiment run.\n");
}
```

```shell
coax/bin/coax run psionic/examples/mnist-classifier.coax
```


[mnist.coax](../lib/data/mnist.coax) admits the original IDX image and label
records: the big-endian magic, matching counts, 28 by 28 shape, exact byte
extents and every label in 0–9. It averages each non-overlapping 2 by 2 block
into one feature as the integer byte sum divided by 1,020, rounded once from
binary64 to binary32. The result is 196 row-major values in `[0, 1]`.
This fixed preprocessing fits no statistics on validation or test data.

The model is two affine layers, `196 → 16 → 10`, with ReLU between them:

```jai
h = relu(x * fc1.weight + fc1.bias)
scores = h * fc2.weight + fc2.bias
loss = sum((scores - one_hot(label)) * (scores - one_hot(label)))
```

<!-- source: psionic/lib/mnist-classifier.coax model -->
```jai
mc_model :: ()->MdlModel {
    m:=mdl_model();m=mdl_data(m,"x",mdl_shape(2,1,196));x:=m.node;
    m=mdl_data(m,"target",mdl_shape(2,1,10));target:=m.node;
    m=mdl_linear(m,MDL_ROOT,"fc1",x,16);m=mdl_relu(m,m.node);
    m=mdl_linear(m,MDL_ROOT,"fc2",m.node,10);prediction:=m.node;
    return mdl_squared_error(m,MDL_ROOT,"loss",prediction,target);
}
mc_config :: ()->TrConfig {
    return TrConfig.{optimizer=TrOptimizer.adamw,storage=TrStorage.binary64,
        rate=0.003,beta1=0.9,beta2=0.999,eps=0.00000001,clip=1,bound=16,
        examples=4096,batch=1,accumulate=16,epochs=12,shuffle=true,seed=11802};
}
```

There are `196*16 + 16 + 16*10 + 10 = 3,322` trainable values, below the
trainer/checkpoint limit of 4,096. The graph uses existing matrix products,
broadcasts, ReLU, multiplication and reductions. Classification takes the
largest finite score, keeping the first class on a tie. It needs neither a
softmax nor a convolution. The eager tensor library already has those
operations and their derivatives. The CNN section below now wires convolution
into the shared module/IR path; broader operator integration remains.

One micro-batch is one image. `tr_add` averages sixteen row losses and
gradients before AdamW updates the binary64 master parameters and moments.
Each module evaluation narrows the parameters to binary32; its forward
products and sums use the existing tensor IR's binary32 rules. The reverse
pass accumulates in binary64 and publishes each parameter gradient rounded
to binary32. The loss is a sum over ten classes, averaged over the sixteen
images, not a mean over all 160 class scores. That normalization is part of
the experiment identity.

### Frozen experiment and qualification

The [dataset manifest](../fixtures/mnist/dataset.json) pins the four original
MNIST objects and their decompressed identities. Acquisition and the
independent numerical oracle are tooling; Python performs no optimizer
step and is not part of the runtime. Original images stay outside the
repository.

The [experiment manifest](../fixtures/mnist/experiment.json) fixes the split
before learning: 4,096 official training images, the next 1,024 for validation,
and the first 2,048 official test images. It also fixes initialization,
shuffle order, optimizer settings, twelve epochs and the acceptance rule.
The run must reach at least 80% test accuracy, beat the training-majority
baseline by at least 50 percentage points and reduce mean test squared
error. These gates were fixed before learning. The official test
slice never supplies gradients or chooses a model, seed or stopping point.

The independent oracle checks preprocessing, initial predictions and all
initial parameter gradients with its own dense formulas and declared
tolerances. It models binary32 products and sequential sums independently
of the tensor IR implementation. A small dyadic fixture also has an exact arithmetic
calculation. Saving the final module state dictionary must reproduce every
test prediction when loaded into fresh storage. A trainer checkpoint must
restore parameters, moments, cursor and shuffle order; comparing the next
update after the frozen interruption point checks resume without repeating
the completed training prefix. These numerical, persistence, resume and
quality claims require their own result records.

### The first original-data run

The [retained run](../fixtures/mnist/run-v1/result.json) passes every frozen
gate on the first canonical attempt. The
[independent audit](../fixtures/mnist/run-v1/audit.json) recomputes initial and
final metrics from the captured score words and the original official labels.

| Observation | Result |
| --- | --- |
| Initial held-out accuracy | 171 / 2,048 = 8.3496% |
| Training-majority baseline on held-out data | 242 / 2,048 = 11.8164% (class 1, chosen from training only) |
| Final held-out accuracy | 1,755 / 2,048 = **85.6934%** |
| Initial → final mean held-out squared error | 1.2761225913 → **0.2832042093** |
| Canonical learning | 3,072 AdamW updates; 49,152 training-row visits over twelve epochs |
| Native workflow time | 17.036464 seconds on the recorded CPU context, including admission, evaluations, replay and export/reload |
| Process maximum resident size | 58,425,344 bytes |
| One next-update replay from epoch six | All 80,068 checkpoint bytes and all 4,096 shuffle entries match |
| Named inference export | Four tensors, 3,322 F32 words, 13,632 bytes |
| Reloaded inference | All 20,480 held-out score words match in Coax |

These scores are regression outputs, not calibrated probabilities. This is a
bounded CPU experiment on 4,096 training images and a 2,048-image test slice,
not full-dataset MNIST, a CNN, a GPU throughput result or a PyTorch comparison.
The [complete validation trace](../fixtures/mnist/run-v1/validation.csv) retains
all thirteen scheduled observations; none selected a checkpoint or stopping
point. No seed, data split or setting was changed to get a pass.

The reusable adapter's [qualification](../fixtures/module-train/qualification.json)
retains its earlier mutation failures and final 160/160 killed sites. Together
with the data adapter's 147 sites and application policy's 117, the three new
authority roots have **424 killed mutants and zero survivors**. These are
entry-source scores; existing tensor, autodiff, trainer and optimizer mutations
were not repeated or represented as new proof. The application integration
is exercised by native checks and the recorded learning run, rather than a
claim of exhaustive mutation coverage of the whole training driver.

The numerical preflight checks all 3,322 initial gradients against separate
dense equations. Checked C, release C and ASan/UBSan captures are identical and
match the independent reference without observed numerical error. The full
learning run was performed once in release; it was not repeated under each
instrumented build. The independent audit checks checkpoint hashes, identities,
counts and replay bytes, and every exported F32 word. Inference-score equality
after reload is an additional Coax observation, not a Python model execution.

The data adapter's retained
[qualification](../fixtures/mnist/data-qualification.json) records seven
authored checks in debug and release and 147 killed root-source mutants,
with no survivors. It covers synthetic IDX boundaries and independently
calculated pooling, not the quality of a trained classifier or the full
image data-loader goal of chapter 121. Callers retain valid, disjoint spans
and immutable admitted label bytes; a plain `MnDataset` is not a certificate
for arbitrary bytes.

The focused native gate runs synthetic guards in debug, release and sanitized
builds. An explicitly supplied dataset adds only the numerical preflight:

```shell
psionic/tests/check_mnist.sh
psionic/tests/check_mnist.sh --data-dir /tmp/psionic-mnist-data --out /tmp/psionic-mnist-check
```

Neither command downloads data or trains. Canonical learning is an explicit
separate run under the frozen manifest. Passing a synthetic gate, a numerical
preflight or a saved-file round trip alone does not establish the quality gate.

The manifest is an identity input; it does not configure the executable.
The actual model and optimizer settings are the Coax constructors above.
Changing a JSON field alone cannot change that program. Checkpoints bind the
manifest and initial parameter bytes, preprocessing, all four original data
files and the trainer configuration. The experiment also records its exact
source closure, compiler and generated native artifacts before learning.

This first consumer checks one next-update resume and inference export/reload.
Its host writes the retained artifacts after the run; it does not demonstrate
incremental durable checkpoint publication or crash-and-restart recovery.

To acquire the pinned data and run this fixed experiment in a new directory:

```shell
python3 psionic/tests/mnist_reference.py acquire --data-dir /tmp/psionic-mnist-data
psionic/run-mnist.sh --train /tmp/psionic-mnist-data /tmp/my-mnist-run
python3 psionic/tests/mnist_reference.py verify-run /tmp/my-mnist-run/run --out /tmp/my-mnist-run/audit.json
```

Acquisition and the audit are tooling. The compiled training executable needs
neither Python nor a model SDK. Use the retained result for unchanged checks;
running the command above starts another explicit experiment and preserves the
existing one. The focused gate is `psionic/check.sh --only mnist` and never
downloads data or starts canonical learning.

## The same trainer for a CNN

The MLP exposed the next framework gap: the eager operator library could
convolve, but the shared IR and module tape could not compose that operation.
The `cnn-nchw-f32-v1` extension connects shape/work admission, forward
execution, both operand derivatives, named modules and the unchanged generic
training adapter. New model code calls public constructors:

<!-- source: psionic/lib/cnn-classifier.coax model -->
```jai
cn_model :: ()->MdlModel {
    m:=mdl_model();m=mdl_data(m,"x",TvShape.{rank=4,dims=[4]int.[1,1,14,14]});x:=m.node;
    m=mdl_data(m,"target",mdl_shape(2,1,10));target:=m.node;
    m=mdl_conv2d(m,MDL_ROOT,"conv",x,2,3,3);m=mdl_relu(m,m.node);
    m=mdl_flatten(m,m.node);m=mdl_linear(m,MDL_ROOT,"fc",m.node,10);prediction:=m.node;
    return mdl_squared_error(m,MDL_ROOT,"loss",prediction,target);
}
cn_config :: ()->TrConfig {
    return TrConfig.{optimizer=TrOptimizer.adamw,storage=TrStorage.binary64,
        rate=0.003,beta1=0.9,beta2=0.999,eps=0.00000001,clip=1,bound=16,
        examples=4096,batch=1,accumulate=16,epochs=12,shuffle=true,seed=11812};
}
```

The same fixed pooling produces `[1,1,14,14]` input. Two learned 3 by 3
filters produce `[1,2,12,12]`; ReLU and a contiguous flatten give 288
features, followed by ten linear scores. There are
`2*1*3*3 + 2 + 288*10 + 10 = 2,910` parameters, 18 nodes and 3,117 input
floats. Names are `conv.weight`, `conv.bias`, `fc.weight` and `fc.bias`.
The bias `[1,2,1,1]` broadcasts over batch and image coordinates. Flattening
keeps the batch and channel-major order; it does not transpose the image.

The planner still reports caller-owned capacities; the actual training tape
needs 606 value floats, 3,486 binary64 adjoints and 288 numerical flags.
The application supplies bounded arenas, and the generic adapter, AdamW,
trainer, checkpoint and named-tensor implementations need no CNN-specific
branches. Runtime model arithmetic and policy are Coax; C only supplies files,
allocation, clocks and logging. Python is an independent oracle and audit.

### Check the convolution before learning

A zero network alone cannot check the learned filters. Independent asymmetric
dyadic fixtures use multiple batches and channels, rectangular kernels and
nonzero upstream derivatives. They check every input, filter and bias
derivative, reversed strides, overlapping windows and the lifetime of saved
materialized operands. Freezing a filter must preserve its values for an
upstream trainable layer's derivative. Native checks also cover state reload,
nonfinite operands and preservation of public outputs on refusal.

The full classifier's original-training-image preflight checks all 2,910
parameters and gradients, all 196 features, ten scores and the loss against
[separate equations](../tests/cnn_reference.py). The reference rounds each
forward product and sequential sum to binary32 and computes the formal reverse
pass in binary64 before the same published-entry narrowing. Its fixed
comparison tolerance is `2e-6 + 2e-6*abs(reference)`; the observations have
zero error. Sixteen of the eighteen initial filter derivatives are nonzero.
Checked, release and checked ASan/UBSan captures agree byte for byte.
The [reference](../fixtures/cnn/reference.json) was fixed without an optimizer
step. Small exact-rational checks test the independent equations themselves.

A full-size zero-network check exceeded the compile-time evaluator's aggregate
traversal limit. The retained native check covers all 2,910 derivatives;
a 200-parameter composed CNN supplies the hand-derived compile-time check.
No evaluator budget was raised. The [portable example](../examples/cnn-classifier.coax) demonstrates the full
model and a refused unrepresentable compute weight without loading data:

<!-- source: psionic/examples/cnn-classifier.coax example -->
```jai
main :: () {
    m:=cn_model();q:=mtr_plan(m);w:[1]CnWork;p:[2910]f64;
    print("CNN: %, % nodes, % parameters, % inputs\n",q.status,m.program.count,q.parameters,q.inputs);
    print("input [1,1,14,14], conv [2,1,3,3], activation [1,2,12,12], flatten [1,288], scores [1,10]\n");
    #assert mn_target(7,w[0].inputs[196..<206])==MnStatus.ok;
    r:=mtr_run(q,0,p[..],w[0].inputs[..],w[0].values[..],w[0].adjoints[..],w[0].flags[..],w[0].entry[..],w[0].staging[..],w[0].gradient[..]);
    print("zero network: %, loss %, last-bias gradient %\n",r.status,r.loss,w[0].gradient[2907]);
    p[0]=1e300;w[0].gradient[0]=123;
    r=mtr_run(q,0,p[..],w[0].inputs[..],w[0].values[..],w[0].adjoints[..],w[0].flags[..],w[0].entry[..],w[0].staging[..],w[0].gradient[..]);
    print("unrepresentable compute weight: %, gradient unchanged %\n",r.status,w[0].gradient[0]==123);
    print("No dataset downloaded and no learning experiment run.\n");
}
```

```shell
coax/bin/coax run psionic/examples/cnn-classifier.coax
psionic/check.sh --only cnn
```

The focused gate never downloads images or trains. It checks the changed CNN
path and new admission roots. Full mutation runs for shared tensor/autodiff
engines are omitted under the owner's fast-check instruction; their focused
numerical, lifetime and refusal checks are recorded separately. Do not treat
new-module mutation scores as mutation qualification of those whole engines.

### A separately frozen CNN experiment

The [CNN manifest](../fixtures/cnn/experiment.json) keeps the MLP's 4,096/1,024/
2,048 split, twelve epochs, sixteen-row accumulation, AdamW settings and quality
thresholds. It freezes initialization seed 11811 and shuffle seed 11812 before
learning. This is a new regression experiment, not a controlled architecture
comparison: the model and seed differ. The earlier MLP artifacts remain intact.
Validation is observed only at the scheduled boundaries; test scores are
captured initially and finally, without selecting a checkpoint or tuning.

The [single retained run](../fixtures/cnn/run-v1/result.json) passes the
frozen gates. Its [independent audit](../fixtures/cnn/run-v1/audit.json)
recomputes quality from the score captures and official labels.

| Observation | Result |
| --- | --- |
| Initial held-out accuracy | 166 / 2,048 = 8.1055% |
| Training-majority baseline | 242 / 2,048 = 11.8164% |
| Final held-out accuracy | 1,814 / 2,048 = **88.5742%** |
| Initial → final mean held-out squared error | 1.1769798681 → **0.3471311970** |
| Learning | 3,072 updates; 49,152 training-row visits over twelve epochs |
| Native workflow time | 13.895613 seconds on the recorded shared Apple M5 Max CPU context |
| Process peak resident size | 58,359,808 bytes |
| Next-update replay | All 70,180 checkpoint bytes and 4,096 shuffle entries match |
| Named inference export | Four tensors, 2,910 F32 words, 11,976 bytes |
| Reloaded inference | All 20,480 score words match in Coax |

The [complete validation trace](../fixtures/cnn/run-v1/validation.csv) retains
all thirteen scheduled observations. The CNN has higher accuracy on this
slice than the earlier MLP, but also a larger final mean squared error
(0.3471 versus 0.2832). Seeds and models differ, so this is not an architecture
ablation or evidence of better calibration. Neither model's raw scores are
probabilities. Timing is one shared-host workflow observation, not a speed
comparison or throughput guarantee.

The [source freeze](../fixtures/cnn/source-freeze.json) binds the exact model,
54 source/context files, compiler image and three native preflight builds
before learning. The [constructor qualification](../fixtures/cnn-modules/qualification.json)
retains the incomplete initial mutation attempt, the 50/51 result and the
sharpened 51/51 result. Together with the application policy's 117 sites,
the two new roots have **168 killed mutants and zero survivors**. This is
entry-source mutation; shared-engine full mutation was not run. The
[core receipt](../fixtures/cnn-core/qualification.json) records focused debug/
release calculus, lifetime, numerical-refusal and unsupported-path checks.
The full learning run happened once in release, not once per sanitizer mode.
The new [MLP compatibility record](../fixtures/cnn/mlp-compatibility.json)
checks unchanged initial capture bytes in three fresh native builds without
repeating the old training experiment.

The checkpoint replay covers exactly the next update after epoch six,
including moments, cursor and shuffle order. Export reload compares every
held-out score word in Coax; the independent audit checks all exported
weights and recomputes quality from captured scores and official labels.
The host publishes artifacts after completion, so this does not qualify
crash/restart recovery or incremental durable publication.

To reproduce the explicit experiment in a new directory:

```shell
python3 psionic/tests/mnist_reference.py acquire --data-dir /tmp/psionic-mnist-data
psionic/run-cnn.sh --train /tmp/psionic-mnist-data /tmp/my-cnn-run
python3 psionic/tests/cnn_reference.py verify-run /tmp/my-cnn-run/run --data-dir /tmp/psionic-mnist-data --out /tmp/my-cnn-run/audit.json
```

Run the original-data numerical preflight before starting learning; the
manifest is an identity input, while model/settings are the Coax constructors.
For routine work use retained quality results and the focused gate. This CPU
slice supplies no convolution GPU or compiled-backward throughput claim,
probability calibration, full-MNIST/CIFAR result or PyTorch comparison.
[`cb_compile`](../lib/compiled-backward.coax) and higher-order function
transforms explicitly refuse convolution until their own derivative rules
exist. Broader completion stays tracked in
[#12626](https://github.com/OpenAgentsInc/openagents/issues/12626).

## What is not here yet

- **Broader CNN training.** The bounded CPU CNN above uses the shared
  module/IR/tape/trainer path. Larger datasets, pooling and convolution
  options, efficient GPU training and compiled convolution derivatives remain.
- **More optimizers behind the same interface.** SGD with momentum and
  chapter 124's Muon are not options yet; the trainer has plain SGD and
  AdamW.
- **The other trainers.** Chapter 104's tape trainer, chapter 116's
  budgeted step (micro-batches of different sizes and recomputation) and
  chapter 157's operator learner still run their own loops.
- **Callbacks.** Coax has no callbacks, so logging and checkpoints happen
  in the caller's loop between steps. Cancellation is the same: stop the
  loop at a step boundary and save.

## Public API

| Call | What it does |
| --- | --- |
| `tr_begin(c, n, docs, tokens, order)` | Admits a configuration for `n` parameters and starts a run; fills epoch 0's order. |
| `tr_next(c, run, docs, tokens, rows)` | The next step's batch, or `evaluate`, `done` or `stopped`. |
| `tr_rows(c, batch, k)` | Micro-batch `k`'s positions or packed rows. |
| `tr_add(batch, sum, loss, g, total)` | Adds one micro-batch's loss and mean gradient. |
| `tr_apply(c, run, batch, sum, p, m, v, total, stage, order)` | Clips, schedules, steps and bounds; writes only when admitted. |
| `tr_eval(c, run, loss)` | Records a due evaluation; applies early stopping. |
| `tr_steps(c)`, `tr_rate(c, step)`, `tr_cursor(c, revision)`, `tr_order(c, epoch, order)` | The run's length, schedule, cursor and order. |
| `tr_ids(c, model, tokenizer, dataset)`, `tr_size(n)` | A checkpoint's identities and size. |
| `tr_save(ids, run, p, m, v, bytes, pointer)`, `tr_resume(c, ids, pointer, bytes, docs, tokens, p, m, v, order)` | Writes and restores a run. |
| `tm_tabular`, `tm_decoder`, `tm_module` | Chapters 103, 115 and 117's training as configurations. |
| `tm_tabular_step`, `tm_decoder_step`, `tm_module_step` | Those models' micro-batch loss and gradient. |
| `mtr_plan(model)` | Checks the supported CPU scalar-loss module and reports parameter and caller-owned arena counts. |
| `mtr_run(plan, seed, master, inputs, values, adjoints, flags, entry_gradient, staging, gradient)` | Runs the built-in tape and publishes the complete gradient; compute and published entry derivatives are binary32, while master storage remains unchanged binary64. |
