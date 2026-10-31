# 124. AdamW, Muon and explicit precision

Chapter 112 trained with a fixed step size: every parameter moved by the same
rate times its gradient. AdamW gives each parameter its own scale. It keeps two
running averages per parameter, of the gradient and of its square, and divides
one by the root of the other, so a parameter whose gradient is consistently
small still moves at roughly the learning rate. Weight decay shrinks each
parameter directly instead of entering the gradient, which is what the "W"
(decoupled weight decay) names.

The first half's profile is `cpu-adamw-v1`. It is CPU only and stands alone:
the optimizer reads caller-owned views and writes them in place. The second
half, [Muon](#muon-nanochats-matrix-optimizer), adds the optimizer nanochat
pairs with AdamW, its parameter grouping and its update rules, as profile
`cpu-muon-v1`.

## One update

For step t (counting from 1), gradient g and group settings lr, beta1, beta2,
eps and decay:

```jai
p  = p * (1 - lr*decay)                       // decoupled decay first
m  = beta1*m + (1-beta1)*g                    // first moment
v  = beta2*v + (1-beta2)*g*g                  // second moment
p  = p - (lr / (1-beta1^t)) * m / (sqrt(v)/sqrt(1-beta2^t) + eps)
```

The moments start at zero, so early averages are biased toward zero; dividing
by `1-beta^t` corrects that. On the first step the correction makes the move
`lr*g/(|g|+eps)`, close to the learning rate whatever the gradient's size. The
decay multiplies the incoming value, before the moment step. Applying it after
gives a different number, and a check pins the order: with lr 0.5, decay 0.5,
beta1 0.5, beta2 0.75, eps 0.5 and g = 2, one step from 1 gives 0.75 - 0.4 =
0.35, where decaying afterwards would give 0.45.

<!-- source: psionic/lib/optim/adamw.coax update -->
```jai
// One element at step t (1-based): decoupled decay of the incoming value,
// then the moments, then the bias-corrected step with eps outside the root.
// ok is false when a moment is not representable; the step is then refused.
aw_cell :: (g:AwGroup,t:int,p:f64,grad:f64,m:f64,v:f64)->AwCell {
    decayed:=p*(1-g.lr*g.decay);
    m2:=g.beta1*m+(1-g.beta1)*grad;
    v2:=g.beta2*v+(1-g.beta2)*grad*grad;
    if !aw_finite(m2) || !aw_finite(v2) {return AwCell.{p=decayed,m=m2,v=v2};}
    c1:=1-pow(g.beta1,f64(t));c2:=1-pow(g.beta2,f64(t));
    return AwCell.{p=decayed-(g.lr/c1)*(m2/(sqrt(v2)/sqrt(c2)+g.eps)),m=m2,v=v2,ok=true};
}
```

## Groups, precision and admission

Parameters form one flat binary64 view. Groups cover it exactly once, in order
and without gaps, each with its own learning rate, betas, eps and decay. Up to
16 groups and 2^24 parameters are admitted. A typical split, used in the
example, decays weights and not biases.

The precision state is explicit. The master copy, the parameters and both
moments, is always binary64. The compute precision, the precision the forward
and backward passes ran at, is declared in the policy: binary64, or binary32.
With binary32 compute every gradient must be a binary32 value (carried in a
binary64 slot), and the step also writes a binary32 compute copy of the
updated parameters for the next forward pass. The master copy never loses the
small updates that binary32 would round away.

Nothing is written until the whole step is admitted. The step refuses, in this
order: overlapping views; mismatched shapes; groups that do not cover the
parameters; settings outside lr in (0,1], betas in [0,1), eps in (0,1] and
decay in [0,1] with lr*decay below 1; missing or non-finite moments or a step
counter at its limit (insufficient state); a gradient that is not at the
declared compute precision. Then every element is computed once without
writing. A non-finite gradient is `nonfinite`; a new value that is not
representable (a second moment past binary64, or a compute copy past
binary32) is `overflow`. The overflow policy decides what happens next:
`refuse` returns that status, and `skip` returns `skipped` and counts the skip,
as loss-scaled mixed-precision training does. Either way every view and the
step counter are unchanged.

<!-- source: psionic/lib/optim/adamw.coax step -->
```jai
aw_step :: (groups:[]AwGroup,policy:AwPolicy,state:AwState,params:[]f64,grads:[]f64,m:[]f64,v:[]f64,compute:[]f32,overlap:bool)->AwResult #export_c {
    if overlap {return AwResult.{status=AwStatus.alias,state=state};}
    n:=params.count;wide:=policy.compute==AwCompute.binary64;
    if !aw_parameters(n) || grads.count!=n || (wide && compute.count!=0) || (!wide && compute.count!=n) {return AwResult.{status=AwStatus.shape,state=state};}
    if !aw_groups(groups.count) || !aw_cover(groups,n) {return AwResult.{status=AwStatus.group,state=state};}
    for g:groups {if !aw_hyper(g) {return AwResult.{status=AwStatus.number,state=state};}}
    if m.count!=n || v.count!=n || !aw_state(state) {return AwResult.{status=AwStatus.state,state=state};}
    for i:0..<n {if !aw_finite(params[i]) || !aw_finite(m[i]) || !aw_finite(v[i]) || v[i]<0 {return AwResult.{status=AwStatus.state,state=state};}}
    for x:grads {if aw_finite(x) && !aw_precise(policy.compute,x) {return AwResult.{status=AwStatus.precision,state=state};}}
    // Admission: every gradient finite and every new value representable.
    bad:=AwStatus.ok;t:=state.step+1;
    for g:groups {for i:g.first..<g.first+g.count {
        if !aw_finite(grads[i]) {bad=AwStatus.nonfinite;break;}
        c:=aw_cell(g,t,params[i],grads[i],m[i],v[i]);
        if !c.ok || !aw_finite(c.p) || (!wide && !aw_narrow(c.p)) {bad=AwStatus.overflow;}
    }if bad==AwStatus.nonfinite {break;}}
    if bad!=AwStatus.ok {
        if policy.overflow==AwOverflow.skip {return AwResult.{status=AwStatus.skipped,state=AwState.{step=state.step,skipped=state.skipped+1}};}
        return AwResult.{status=bad,state=state};
    }
    for g:groups {for i:g.first..<g.first+g.count {
        c:=aw_cell(g,t,params[i],grads[i],m[i],v[i]);params[i]=c.p;m[i]=c.m;v[i]=c.v;if !wide {compute[i]=f32(c.p);}
    }}
    return AwResult.{state=AwState.{step=t,skipped=state.skipped},written=n};
}
```

## Resume

The optimizer's whole state is the step counter, the skip counter and the two
moment views, beside the parameters. A run that stops after any step and
continues from a copy of those, in fresh buffers, produces the same values as
the run that never stopped. The checks resume the two-group case at every step
and confirm the old buffers still hold the state of the cut. Resetting only
the step counter repeats the bias correction and diverges. Storing that state
in a versioned checkpoint is the next task's job.

## Checks

An independent [oracle](../tests/adamw_oracle.py) recomputes three cases with
per-group dictionaries and Python floats: two groups over a 3x4 matrix for five
steps, a binary32-compute vector for three, and a scalar with beta1 zero for
four. Its values are committed in
[reference.coax](../fixtures/adamw/reference.coax) and reproduce from the
oracle on every check. Coax compares every step within 1e-15 relative; a
single group with the first group's settings, or the second group's rate
alone, diverges. The native build compares the same cases, resumes at every
step, refuses actual aliases and native NaN, and runs at -O0 and -O3, plain and
under ASan/UBSan.

[Run the example](../examples/adamw.coax):

<!-- source: psionic/examples/adamw.coax example -->
```jai
// Fit y = 2x + 1 on eight points. The weight decays; the bias does not.
loss_and_gradient :: (p:[]f64,g:[]f64)->f64 {
    loss:=0.0;g[0]=0;g[1]=0;
    for i:0..<8 {x:=f64(i)/4-1;e:=p[0]*x+p[1]-(2*x+1);loss+=e*e/8;g[0]+=2*e*x/8;g[1]+=2*e/8;}
    return loss;
}
main :: () {
    groups:=[2]AwGroup.[AwGroup.{first=0,count=1,lr=0.05,beta1=0.9,beta2=0.999,eps=1e-8,decay=0.01},AwGroup.{first=1,count=1,lr=0.05,beta1=0.9,beta2=0.999,eps=1e-8,decay=0}];
    policy:=AwPolicy.{overflow=AwOverflow.skip};p:[2]f64;g:[2]f64;m:[2]f64;v:[2]f64;none:[1]f32;state:=AwState.{};
    first:=loss_and_gradient(p[..],g[..]);last:=first;
    for k:0..<400 {
        last=loss_and_gradient(p[..],g[..]);if k==200 {g[0]=0.0/0.0;}
        r:=aw_step(groups[..],policy,state,p[..],g[..],m[..],v[..],none[0..<0],false);
        #assert r.status==AwStatus.ok || (k==200 && r.status==AwStatus.skipped);state=r.state;
    }
    last=loss_and_gradient(p[..],g[..]);#assert last<first/1000;
    print("cpu-adamw-v1: binary64 master and compute; 2 groups\n");
    print("loss: % -> %\n",first,last);
    print("weight %, bias %\n",p[0],p[1]);
    print("steps applied %, skipped %\n",state.step,state.skipped);
}
```

```shell
coax/bin/coax run psionic/examples/adamw.coax
psionic/check.sh --only adamw
```

The [transcript](../fixtures/adamw.out) is exact for the named CPU build.
The weight settles slightly below 2 because it decays; the bias, undecayed,
reaches 1. Step 200's gradient is replaced with NaN, and the skip policy drops
that step: 399 applied, 1 skipped.
[The verification record](../../docs/verification/psionic-ch78-adamw-2026-10-31.md)
records the checks, mutation and native evidence.

## Muon: nanochat's matrix optimizer

nanochat trains with one combined optimizer, MuonAdamW. AdamW, as above,
updates the embeddings, the output head and the per-layer scalars. Every
matrix inside the transformer blocks goes to Muon instead. Muon keeps a
momentum, as SGD with momentum does, but it does not step along the momentum.
It replaces the momentum matrix with the nearest semi-orthogonal matrix: the
same singular vectors, with every singular value close to 1. Each direction the
matrix can move in then moves by about the same amount, however unbalanced the
gradient was.

This half reproduces the pinned code:
[`nanochat/optim.py`](https://github.com/karpathy/nanochat/blob/92d63d4e8bb4df75c3b71618f31ddde2378b2bcd/nanochat/optim.py)
and `GPT.setup_optimizer` in `nanochat/gpt.py`, at revision `92d63d4e`, the
revision chapter 123's tokenizer pins. The files' hashes are in
[the model pin](../fixtures/models/pins/nanochat.json). The profile is
`cpu-muon-v1`: one rank, CPU only, binary64 throughout.

### Which parameters go where

`setup_optimizer` makes six AdamW groups, each with its own rate, betas and
decay: the output head, the token embedding, the value embeddings, the
residual scalars, the x0 scalars, and a last group holding the smear gate, the
smear and the backout scalars, in that order. Learning rates for the first
three scale by `(model_dim/768)^-0.5`. Then it makes one Muon group per
distinct matrix shape, in sorted order, with momentum 0.95, five Polar Express
steps, beta2 0.9 and the caller's weight decay. A group holds its tensors in
model order. The planner returns the groups and where each tensor starts in
one flat view, so a group is always a contiguous range. A Muon group holds
whole matrices back to back. The planner refuses an inventory that does not
have exactly one of each singleton role and at least one matrix. nanochat
keeps an empty value-embedding group when a model has none; here that group is
left out, since it would update nothing.

<!-- source: psionic/lib/optim/muon.coax plan -->
```jai
// GPT.setup_optimizer: six AdamW groups (lm_head, wte, value embeddings,
// resid scalars, x0 scalars, then smear gate, smear and backout in that
// order), then one Muon group per distinct matrix shape in sorted order. A
// group lists its tensors in model order and occupies a contiguous range;
// places[i] is where tensor i starts. Empty groups (no value embeddings) are
// left out: nanochat's update nothing.
mc_plan :: (tensors:[]McTensor,h:McHyper,groups:[]McGroup,places:[]int)->McPlan #export_c {
    T:=tensors.count;
    if T<1 || T>MC_TENSORS || places.count!=T {return McPlan.{status=McStatus.shape};}
    counts:[9]int;total:=0;
    for t:tensors {if !mc_tensor(t) {return McPlan.{status=McStatus.role};}counts[cast(int) t.role]+=1;total+=mc_numel(t);}
    for ri:0..<9 {if ri!=2 && ri!=8 && counts[ri]!=1 {return McPlan.{status=McStatus.role};}}
    if counts[8]<1 {return McPlan.{status=McStatus.role};}
    if !aw_parameters(total) {return McPlan.{status=McStatus.shape};}
    if h.model_dim<1 || h.model_dim>AW_PARAMETERS || !mc_rate(h.unembedding_lr) || !mc_rate(h.embedding_lr) || !mc_rate(h.matrix_lr) || !mc_rate(h.scalar_lr) || !mc_unit(h.weight_decay) {return McPlan.{status=McStatus.number};}
    shapes:=0;at:=mc_first_shape(tensors);while at.rows!=0 {shapes+=1;at=mc_next_shape(tensors,at);}
    used:=5+shapes;if counts[2]>0 {used+=1;}
    if groups.count<used {return McPlan.{status=McStatus.group};}
    next:=0;g:=0;
    for gi:0..<6 {
        // Group 5 holds the smear gate, the smear and the backout, in that order.
        start:=next;last:=gi+1;if gi==5 {last=8;}
        for ri:gi..<last {for i:0..<T {if (cast(int) tensors[i].role)==ri {places[i]=next;next+=mc_numel(tensors[i]);}}}
        if next>start {groups[g]=mc_adamw_group(gi,h,start,next-start);g+=1;}
    }
    muon:=g;at=mc_first_shape(tensors);
    while at.rows!=0 {
        start:=next;
        for i:0..<T {t:=tensors[i];if t.role==McRole.matrix && t.rows==at.rows && t.cols==at.cols {places[i]=next;next+=mc_numel(t);}}
        groups[g]=McGroup.{kind=McKind.muon,first=start,count=next-start,rows=at.rows,cols=at.cols,lr=h.matrix_lr,beta2=0.9,decay=h.weight_decay,momentum=0.95,ns=5};
        g+=1;at=mc_next_shape(tensors,at);
    }
    return McPlan.{groups=g,elements=next,second=mc_seconds(groups[0..<g]),work=mc_scratch(groups[muon..<g])};
}
```

### One Muon update

For one matrix G (r x c) with momentum buffer M, the update runs in five steps:

1. **Nesterov momentum.** M moves toward G by 1 - momentum, and the update
   starts from G moved toward the new M by the momentum.
2. **MuonEq.** Every row is rescaled to the mean row norm, and then the whole
   matrix is divided by 1.01 times its Frobenius norm, so all its singular
   values are below 1.
3. **Polar Express.** Up to five quintic steps, X <- aX + (bA + cA^2)X with
   A = XX^T (or X(bA + cA^2) with A = X^TX for a tall matrix, so A is the
   smaller square). The pinned coefficients change at every step. Each step
   acts on each singular value s as as + bs^3 + cs^5, pushing it toward 1.
4. **Muon+.** The Frobenius norm is set to sqrt(min(r, c)), the norm of an
   exactly semi-orthogonal matrix.
5. **NorMuon.** A running mean square is kept per row (when r >= c) or per
   column, and each row or column is rescaled by its inverse root. The total
   norm stays unchanged.

The parameter then moves by the learning rate times that update. For a tall
matrix the rate is multiplied by sqrt(r/c). Cautious weight decay shrinks a
parameter only where the update and the parameter agree in sign. The floors
copy nanochat's clamps (1e-6 on norms, 1e-10 on the second moment), so a zero
row or a zero gradient stays finite.

<!-- source: psionic/lib/optim/muon.coax polar -->
```jai
// Polar Express on X (rows x cols, row-major) in place: ns quintic steps
// X <- a X + (b A + c A A) X with A = X X^T, or X (b A + c A A) with
// A = X^T X when X is tall, so A is always the smaller square. y is scratch
// for the next X; a and b hold k x k.
mc_polar :: (x:[]f64,r:int,c:int,ns:int,y:[]f64,a:[]f64,b:[]f64) {
    tall:=r>c;k:=mc_min(r,c);
    for step:0..<ns {
        ca:=MC_POLAR[3*step];cb:=MC_POLAR[3*step+1];cc:=MC_POLAR[3*step+2];
        for p:0..<k {for q:0..<k {
            s:=0.0;
            if tall {for i:0..<r {s+=x[i*c+p]*x[i*c+q];}} else {for j:0..<c {s+=x[p*c+j]*x[q*c+j];}}
            a[p*k+q]=s;
        }}
        for p:0..<k {for q:0..<k {s:=0.0;for t:0..<k {s+=a[p*k+t]*a[t*k+q];}b[p*k+q]=cb*a[p*k+q]+cc*s;}}
        for i:0..<r {for j:0..<c {
            s:=0.0;
            if tall {for t:0..<k {s+=x[i*c+t]*b[t*k+j];}} else {for t:0..<k {s+=b[i*k+t]*x[t*c+j];}}
            y[i*c+j]=ca*x[i*c+j]+s;
        }}
        for i:0..<r*c {x[i]=y[i];}
    }
}
```

<!-- source: psionic/lib/optim/muon.coax matrix -->
```jai
// One matrix of a Muon group: momentum m and second moment s in, the new
// values computed in w. Returns false when the step would overflow; writes p,
// m and s only when write is set.
mc_matrix :: (g:McGroup,p:[]f64,grad:[]f64,m:[]f64,s:[]f64,w:[]f64,write:bool)->bool {
    r:=g.rows;c:=g.cols;rc:=r*c;k:=mc_min(r,c);red:=mc_max(r,c);
    oa:=3*rc;ob:=oa+k*k;os:=ob+k*k;ov:=os+red;
    m2:=w[0..<rc];x:=w[rc..<2*rc];y:=w[2*rc..<oa];a:=w[oa..<ob];b:=w[ob..<os];s2:=w[os..<ov];vm:=w[ov..<ov+red];
    // Nesterov momentum: the buffer moves toward the gradient, the update
    // toward the buffer.
    mu:=g.momentum;total:=0.0;
    for i:0..<rc {m2[i]=m[i]+(1-mu)*(grad[i]-m[i]);x[i]=grad[i]+mu*(m2[i]-grad[i]);total+=x[i]*x[i];}
    // The only place a finite step can overflow: past it every value is
    // normalized. A finite x also means a finite momentum.
    if !aw_finite(total) {return false;}
    if !write {return true;}
    // MuonEq: every row to the mean row norm, then the whole below norm 1.
    target:=sqrt(total)/sqrt(f64(r));
    for i:0..<r {row:=x[i*c..<i*c+c];q:=target/mc_floor(mc_norm(row),0.000001);for j:0..<c {row[j]=row[j]*q;}}
    d:=mc_norm(x)*1.01+0.000001;for i:0..<rc {x[i]=x[i]/d;}
    mc_polar(x,r,c,g.ns,y,a,b);
    // Muon+: the Frobenius norm of an exactly semi-orthogonal matrix.
    f:=sqrt(f64(k))/mc_floor(mc_norm(x),0.000001);for i:0..<rc {x[i]=x[i]*f;}
    // NorMuon: a running mean square per row (rows >= cols) or per column,
    // each rescaled by its inverse root, with the total norm kept.
    rows:=r>=c;size:=r;if rows {size=c;}
    for i:0..<red {vm[i]=0;}
    for i:0..<r {for j:0..<c {e:=x[i*c+j]*x[i*c+j];if rows {vm[i]+=e;} else {vm[j]+=e;}}}
    mean:=0.0;for i:0..<red {vm[i]=vm[i]/f64(size);mean+=vm[i];}
    vnorm:=sqrt(mean*f64(size));scaled:=0.0;
    for i:0..<red {s2[i]=s[i]+(1-g.beta2)*(vm[i]-s[i]);st:=1/sqrt(mc_floor(s2[i],0.0000000001));scaled+=vm[i]*f64(size)*st*st;}
    vnew:=mc_floor(sqrt(scaled),0.0000000001);
    // Cautious decay: only where the update and the parameter agree in sign.
    lr:=mc_lr(g);
    for i:0..<r {for j:0..<c {
        at:=j;if rows {at=i;}
        u:=x[i*c+j]*((1/sqrt(mc_floor(s2[at],0.0000000001)))*(vnorm/vnew));
        pv:=p[i*c+j];mask:=0.0;if u*pv>=0 {mask=1;}
        p[i*c+j]=pv-(lr*u+lr*g.decay*pv*mask);
    }}
    for i:0..<rc {m[i]=m2[i];}for i:0..<red {s[i]=s2[i];}
    return true;
}
```

### One combined step

The step takes every group at once over one flat binary64 view. Parameters,
gradients and the first moment (AdamW's m, Muon's momentum buffer) all have
one entry per parameter. The second-moment view holds one entry per element
for an AdamW group and one per row or column of each matrix for a Muon group,
in group order. The caller also provides scratch memory for one matrix; the
planner reports how much. AdamW groups use the AdamW update above
and share one step counter.

Admission follows the AdamW half: overlapping views, then shapes, then groups,
then settings, then state, then scratch, then gradients. A group setting is
refused if it is outside its range. Settings a group's kind does not use
(Muon's beta1 and eps, AdamW's momentum and step count) must be zero, and
more than five Polar Express steps are refused rather than silently capped. A
non-finite gradient is `nonfinite`. A Muon matrix whose sum of squares
overflows binary64 is `overflow`, caught before any square root. After that
check every value is normalized and stays finite. The skip policy again turns
either into `skipped` and counts it. Nothing is written until the whole step
is admitted.

<!-- source: psionic/lib/optim/muon.coax step -->
```jai
mc_step :: (groups:[]McGroup,policy:McPolicy,state:McState,params:[]f64,grads:[]f64,m:[]f64,second:[]f64,work:[]f64,overlap:bool)->McResult #export_c {
    if overlap {return McResult.{status=McStatus.alias,state=state};}
    n:=params.count;
    if !aw_parameters(n) || grads.count!=n {return McResult.{status=McStatus.shape,state=state};}
    if groups.count<1 || groups.count>MC_GROUPS || !mc_cover(groups,n) {return McResult.{status=McStatus.group,state=state};}
    for g:groups {if !mc_hyper(g) {return McResult.{status=McStatus.number,state=state};}}
    if m.count!=n || second.count!=mc_seconds(groups) || !aw_state(AwState.{step=state.step,skipped=state.skipped}) {return McResult.{status=McStatus.state,state=state};}
    for i:0..<n {if !aw_finite(params[i]) || !aw_finite(m[i]) {return McResult.{status=McStatus.state,state=state};}}
    for x:second {if !aw_finite(x) || x<0 {return McResult.{status=McStatus.state,state=state};}}
    if work.count<mc_scratch(groups) {return McResult.{status=McStatus.work,state=state};}
    // Admission: every gradient finite, then every new value representable.
    bad:=McStatus.ok;t:=state.step+1;
    for x:grads {if !aw_finite(x) {bad=McStatus.nonfinite;}}
    if bad==McStatus.ok && !mc_apply(groups,t,params,grads,m,second,work,false) {bad=McStatus.overflow;}
    if bad!=McStatus.ok {
        if policy.overflow==McOverflow.skip {return McResult.{status=McStatus.skipped,state=McState.{step=state.step,skipped=state.skipped+1}};}
        return McResult.{status=bad,state=state};
    }
    applied:=mc_apply(groups,t,params,grads,m,second,work,true);
    return McResult.{state=McState.{step=t,skipped=state.skipped},written=n};
}
```

### Precision, resume and what is not covered

nanochat runs this in fp32, with bf16 for the Polar Express loop on GPUs that
have it, and passes its hyperparameters through fp32 tensors. `cpu-muon-v1` is
binary64 everywhere, including the hyperparameters. It matches the pinned code
as a calculation, within the tolerance below. It is not bit-identical to an
fp32 or bf16 run, and it does not claim to be.

The optimizer state is the step and skip counters, the first-moment view and
the second-moment view. A run that resumes from a copy of those in fresh
buffers, after any step, produces exactly the values the uninterrupted run
produces. Zeroing the Muon buffers, or resetting the step counter, diverges.

Not supported: more than one rank (nanochat's ZeRO-2 sharding of AdamW state,
and its chunking of Muon stacks across ranks with reduce-scatter and
all-gather); bf16 or fp16 compute and the fp16 GradScaler; `torch.compile`;
and `base_train`'s schedules. The schedules are a linear warmup and warmdown
of the rates, a momentum warmup from 0.85 to 0.97 then down to 0.90, and a
cosine decay of the Muon weight decay. The caller can still apply them by
setting a group's `lr`, `momentum` and `decay` before each step, as
`base_train` does to its param_groups, and the example decays its rate that way.

### Checks

An independent [oracle](../tests/muon_oracle.py) is written from the pinned
files in nanochat's own structure. It keys parameters and optimizer state by
name, builds setup_optimizer's group dictionaries, and spells out torch's
semantics, including the two-branch `lerp_` and left-to-right sums. It
computes two cases. The first is a one-layer nanochat-shaped model: width 4,
vocabulary 6, square 4x4 attention matrices, a wide 2x12 value gate, a tall
16x4 and a wide 4x16 MLP, and embedding, head, scalar and smear groups, under
the planner's own settings with base_train's 0.28 Muon decay, for four steps.
In its first gradient, one row is zero and one has norm 1e-9. The second case
is a Muon-heavy model with other momentum, beta2, decay and three Polar
Express steps, for five steps. The oracle also runs Polar Express alone on a
square, a tall and a wide input. Its values are committed in
[reference.coax](../fixtures/muon/reference.coax) and reproduce from the oracle
on every check.

Coax matches every parameter, momentum and second-moment value at every step
within 1e-12 relative. The largest difference measured is 1.5e-14, from the
two lerp forms and divisions taken in another order. Polar Express alone
matches exactly. The planner reproduces both cases' groups, settings and
offsets exactly. A change to any Muon setting, a 1% change to any group's rate,
or putting the square matrices under AdamW diverges. Hand checks pin a 1x1
update (the step is the rate, plus decay only when the signs agree), Polar
Express on diagonal input against the scalar polynomial, and the update's
Frobenius norm of lr times sqrt(min(r, c)), with equal row or column norms.
The native build compares both cases, resumes at every step, refuses actual
aliases, native NaN and overflow, and runs at -O0 and -O3, plain and under
ASan/UBSan. Mutation leaves no survivors.

[Run the example](../examples/muon.coax):

<!-- source: psionic/examples/muon.coax example -->
```jai
// A two-layer nanochat-shaped inventory (width 8, vocabulary 16): the planner
// sorts it into setup_optimizer's groups. Then a 2x3 linear map and its bias
// learn y = A x + b, the matrix with Muon and the bias with AdamW.
loss_and_gradient :: (p:[]f64,g:[]f64)->f64 {
    A:=[6]f64.[0.5,-1,0.25,1,0.5,-0.75];b:=[2]f64.[0.3,-0.2];loss:=0.0;
    for i:0..<8 {g[i]=0;}
    for k:0..<12 {
        x:=[3]f64.[sin(f64(k)),cos(1.7*f64(k)),f64(k%3)-1];
        for r:0..<2 {
            y:=p[6+r];want:=b[r];for c:0..<3 {y+=p[3*r+c]*x[c];want+=A[3*r+c]*x[c];}
            e:=y-want;loss+=e*e/12;g[6+r]+=2*e/12;for c:0..<3 {g[3*r+c]+=2*e*x[c]/12;}
        }
    }
    return loss;
}
main :: () {
    T:[22]McTensor;T[0]=McTensor.{role=McRole.embedding,rank=2,rows=16,cols=8};
    n:=1;for layer:0..<2 {
        for q:0..<4 {T[n]=McTensor.{role=McRole.matrix,rank=2,rows=8,cols=8};n+=1;}
        T[n]=McTensor.{role=McRole.matrix,rank=2,rows=2,cols=12};T[n+1]=McTensor.{role=McRole.matrix,rank=2,rows=32,cols=8};T[n+2]=McTensor.{role=McRole.matrix,rank=2,rows=8,cols=32};n+=3;
    }
    T[n]=McTensor.{role=McRole.lm_head,rank=2,rows=16,cols=8};T[n+1]=McTensor.{role=McRole.resid,rank=1,rows=1,cols=2};T[n+2]=McTensor.{role=McRole.x0,rank=1,rows=1,cols=2};
    T[n+3]=McTensor.{role=McRole.smear_gate,rank=2,rows=1,cols=24};T[n+4]=McTensor.{role=McRole.smear_lambda,rank=1,rows=1,cols=1};T[n+5]=McTensor.{role=McRole.backout,rank=1,rows=1,cols=1};
    T[n+6]=McTensor.{role=McRole.value_embed,rank=2,rows=16,cols=8};n+=7;
    groups:[16]McGroup;places:[22]int;plan:=mc_plan(T[0..<n],McHyper.{model_dim=8,weight_decay=0.28},groups[..],places[0..<n]);
    #assert plan.status==McStatus.ok;
    print("cpu-muon-v1: % tensors, % elements, % groups, % second-moment entries, % scratch\n",n,plan.elements,plan.groups,plan.second,plan.work);
    for i:0..<plan.groups {
        g:=groups[i];
        if g.kind==McKind.adamw {print("  adamw  % elements  lr % betas % % decay %\n",g.count,g.lr,g.beta1,g.beta2,g.decay);}
        else {print("  muon   %x% x %  lr % (applied %) momentum % beta2 % decay % ns %\n",g.rows,g.cols,g.count/(g.rows*g.cols),g.lr,mc_lr(g),g.momentum,g.beta2,g.decay,g.ns);}
    }
    fit:=[2]McGroup.[McGroup.{kind=McKind.muon,count=6,rows=2,cols=3,lr=0.05,beta2=0.9,momentum=0.95,ns=5},McGroup.{first=6,count=2,lr=0.05,beta1=0.8,beta2=0.95,eps=0.0000000001}];
    p:[8]f64;g:[8]f64;m:[8]f64;s:[5]f64;w:[32]f64;state:=McState.{};
    first:=loss_and_gradient(p[..],g[..]);
    for k:0..<300 {
        loss:=loss_and_gradient(p[..],g[..]);#assert loss>=0;
        // The rate decays linearly to a tenth, as base_train's warmdown does.
        fit[0].lr=0.05*(1-0.9*f64(k)/300);fit[1].lr=fit[0].lr;
        r:=mc_step(fit[..],McPolicy.{},state,p[..],g[..],m[..],s[..],w[..],false);#assert r.status==McStatus.ok;state=r.state;
    }
    last:=loss_and_gradient(p[..],g[..]);#assert last<first/10000;
    print("fit: loss % -> % in % steps\n",first,last,state.step);
}
```

```shell
coax/bin/coax run psionic/examples/muon.coax
psionic/check.sh --only muon
```

The [transcript](../fixtures/muon.out) is exact for the named CPU build. The
planner sorts a two-layer inventory into six AdamW and four Muon groups; the
tall 32x8 group's rate doubles to 0.04. The fit's loss falls from 1.80 to
8.8e-6 in 300 steps.
[The verification record](../../docs/verification/psionic-ch124-muon-2026-10-31.md)
records the checks, mutation and native evidence.
