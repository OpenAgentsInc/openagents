# 124. AdamW and explicit precision

Chapter 112 trained with a fixed step size: every parameter moved by the same
rate times its gradient. AdamW gives each parameter its own scale. It keeps two
running averages per parameter, of the gradient and of its square, and divides
one by the root of the other, so a parameter whose gradient is consistently
small still moves at roughly the learning rate. Weight decay shrinks each
parameter directly instead of entering the gradient, which is what the "W"
(decoupled weight decay) names.

The runnable profile is `cpu-adamw-v1`. It is CPU only and stands alone: the
optimizer reads caller-owned views and writes them in place. This chapter does
not claim nanochat optimizer parity; that needs Muon and nanochat's parameter
grouping, which are separate work.

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
