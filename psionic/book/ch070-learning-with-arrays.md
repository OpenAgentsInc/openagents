# 70. Learning with arrays

[Chapter 1](ch001-learning-a-number.md) learned one number. This chapter learns
from a table: a few thousand rows, four measured features per row, and a yes/no
question about each one. Will this request finish before its 9-second deadline?

Five predictors answer it on the same held-out rows:
- a constant, the training on-time rate;
- an operator's handwritten rule;
- a line that predicts the latency itself;
- logistic regression;
- a network with one hidden layer of eight tanh units.

Everything is written out: the gradients, the minibatch loop, the scaling, the
split and the costs. Nothing comes from a framework.

```shell
coax/bin/coax run psionic/examples/deadline-predictor.coax
psionic/check.sh --only tabular
```

```jai
2400 requests from 300 sessions, 2047 on time (deadline 9 s)
split by session: 1440 training, 480 validation, 480 test rows
validation log loss, logistic: raw features 1.5078 at rate 1e-6, scaled 0.1086
latency line: test squared error 3.8812 s^2
test log loss: constant 0.3807, logistic 0.1593, MLP 0.1434
test decisions at threshold 0.5 (a missed late request costs 5, a needless flag 1):
  constant accuracy 0.875, cost 0.625, caught 0 of 60 late
  rule     accuracy 0.8146, cost 0.2021, caught 58 of 60 late
  line     accuracy 0.9292, cost 0.1792, caught 47 of 60 late
  logistic accuracy 0.9375, cost 0.1625, caught 48 of 60 late
  MLP      accuracy 0.9396, cost 0.1438, caught 50 of 60 late
  logistic threshold 0.9 chosen on validation: test cost 0.1167
  MLP      threshold 0.95 chosen on validation: test cost 0.1125
192 training rows: test log loss logistic 0.1771, MLP 0.2812, MLP with decay 0.01 0.1592
```

The [example](../examples/deadline-predictor.coax) runs in about a second.

## A simulator whose truth we know

The data comes from a simulator, so we know what generates it. Chapter 14's real
measurements can replace it later without changing anything else.

Each session is one machine for a while. It draws:
- its free memory, `headroom`, from 0.5 to 8 GiB;
- a `slowness` factor from 0.7 to 1.3.

Each of its eight requests then draws:
- a prompt of 64 to 8,192 tokens;
- a queue depth of 0 to 15;
- a small or large model;
- some noise.

Its latency in seconds is

```jai
0.2 + 0.15 · queue + prompt · cost(model) · slowness · (1 + 1.5 / headroom) + 0.6 · noise
```

with `cost` 0.0003 s per token for the small model and 0.0009 s for the large
one, and noise uniform in ±0.3 s. A request is on time when that latency is at
most 9 s. Memory matters through `1.5 / headroom`: a machine short on memory is
slower for every request, and nothing about the request itself shows that.

<!-- source: psionic/lib/tabular.coax simulate -->
```jai
tb_on_time :: (seconds:f64)->bool {return seconds<=TB_DEADLINE;}
// deadline-sim-v1. Session g draws its memory headroom and machine slowness;
// request i draws its prompt length, queue depth, model and noise.
tb_simulate :: (seed:u64,sessions:int,x:[]f64,y:[]f64,latency:[]f64,groups:[]int,overlap:bool)->TbSim #export_c {
    if sessions<1 || sessions>TB_SESSIONS_MAX {return TbSim.{status=TbStatus.shape};}
    n:=sessions*TB_PER_SESSION;
    if x.count<4*n || y.count<n || latency.count<n || groups.count<n {return TbSim.{status=TbStatus.capacity};}
    if overlap {return TbSim.{status=TbStatus.alias};}
    on:=0;
    for g:0..<sessions {
        headroom:=0.5+7.5*tb_uniform(seed,TB_SESSION_COUNTER+cast(u64)(2*g));
        slowness:=0.7+0.6*tb_uniform(seed,TB_SESSION_COUNTER+cast(u64)(2*g+1));
        for r:0..<TB_PER_SESSION {
            i:=g*TB_PER_SESSION+r;c:=cast(u64)(4*i);
            prompt:=64+cast(int)(tb_bits(seed,c)%8129);
            queue:=cast(int)(tb_bits(seed,c+1)%16);
            large:=cast(int)(tb_bits(seed,c+2)%2);
            cost:=0.0003;if large==1 {cost=0.0009;}
            seconds:=0.2+0.15*f64(queue)+f64(prompt)*cost*slowness*(1+1.5/headroom)+0.6*(tb_uniform(seed,c+3)-0.5);
            x[4*i]=f64(prompt);x[4*i+1]=f64(queue);x[4*i+2]=f64(large);x[4*i+3]=headroom;
            latency[i]=seconds;groups[i]=g;y[i]=0;
            if tb_on_time(seconds) {y[i]=1;on+=1;}
        }
    }
    return TbSim.{rows=n,on_time=on};
}
```

Every draw is SplitMix64 at a counter, the generator chapter 45's sampler uses.
Integer fields reduce the 64-bit word modulo their range. Request `i` uses
counters `4i` to `4i + 3`, and session `g` uses `2²⁰ + 2g` and `2²⁰ + 2g + 1`,
so a run is fixed by its seed and its size. Seed 70 with 300 sessions gives
2,400 requests, 2,047 of them on time.

## Split by session

Rows from one session share its headroom and slowness. A random row split would
put some of a session's rows in training and others in the test, and the test
would partly measure memory of that machine. So whole sessions go to one part:
three in five to training, one to validation, one to test.

<!-- source: psionic/lib/tabular.coax split -->
```jai
// Whole sessions go to one part: 0 training (three in five), 1 validation, 2 test.
tb_part :: (group:int)->int {
    k:=group%5;
    if k<3 {return 0;}
    if k==3 {return 1;}
    return 2;
}
// The rows of one part, in row order. -1 for a bad part, group, short or overlapping output.
tb_rows :: (groups:[]int,part:int,out:[]int,overlap:bool)->int #export_c {
    if part<0 || part>2 || overlap {return -1;}
    n:=0;
    for g:groups {
        if g<0 {return -1;}
        if tb_part(g)==part {n+=1;}
    }
    if out.count<n {return -1;}
    n=0;
    for i:0..<groups.count {if tb_part(groups[i])==part {out[n]=i;n+=1;}}
    return n;
}
```

We also tried splitting by row. At seed 70 the test loss looked better, but
across seeds 72 to 76 the difference was within noise, so this chapter claims
nothing about it. Planting a leak and catching it is chapter 76's work.

## Scale with training statistics

Prompt lengths run to 8,192 and the model flag is 0 or 1. One learning rate
can't suit both: a step that moves the flag's weight sensibly throws the
prompt's weight far off. Scaling each feature to mean 0 and standard deviation
1 fixes that. The statistics come from the training rows only, so validation
and test rows never shape the model.

<!-- source: psionic/lib/tabular.coax scale -->
```jai
// Mean and population standard deviation of each feature over the training rows only.
tb_fit_scale :: (x:[]f64,d:int,rows:[]int,mean:[]f64,sd:[]f64,overlap:bool)->TbStatus #export_c {
    if !tb_table_ok(x,d) || !tb_row_ok(rows,x.count/d) {return TbStatus.shape;}
    if mean.count<d || sd.count<d {return TbStatus.capacity;}
    if overlap {return TbStatus.alias;}
    for r:rows {for j:0..<d {if !tb_in(x[r*d+j],-TB_DOMAIN,TB_DOMAIN) {return TbStatus.number;}}}
    m:[8]f64;s:[8]f64;
    for j:0..<d {
        a:PsionicAccumulator;
        for r:rows {a=psionic_ops_accumulate(a,x[r*d+j]);}
        m[j]=psionic_ops_total(a)/f64(rows.count);
        v:PsionicAccumulator;
        for r:rows {e:=x[r*d+j]-m[j];v=psionic_ops_accumulate(v,e*e);}
        s[j]=sqrt(psionic_ops_total(v)/f64(rows.count));
        if s[j]==0 {return TbStatus.number;}
    }
    for j:0..<d {mean[j]=m[j];sd[j]=s[j];}
    return TbStatus.ok;
}
tb_apply_scale :: (x:[]f64,d:int,mean:[]f64,sd:[]f64,out:[]f64,overlap:bool)->TbStatus #export_c {
    if !tb_table_ok(x,d) || mean.count<d || sd.count<d {return TbStatus.shape;}
    if out.count<x.count {return TbStatus.capacity;}
    if overlap {return TbStatus.alias;}
    for j:0..<d {if !tb_in(mean[j],-TB_DOMAIN,TB_DOMAIN) || !(sd[j]>0 && sd[j]<=TB_DOMAIN) {return TbStatus.number;}}
    for v:x {if !tb_in(v,-TB_DOMAIN,TB_DOMAIN) {return TbStatus.number;}}
    for i:0..<x.count {j:=i%d;out[i]=(x[i]-mean[j])/sd[j];}
    return TbStatus.ok;
}
```

The difference is large. Logistic regression on raw features, 40 epochs at
three learning rates, gives these validation log losses at seed 70:

| Rate | Raw features | Scaled features |
| --- | --- | --- |
| 0.5 | 777,038 | 0.1086 |
| 0.001 | 2,985 | |
| 0.000001 | 1.508 | |

The best raw run is still 14 times worse than the scaled one, and the same
holds at every seed checked. A constant feature has no scale, so it's refused
as `number`.

## Three models

A model is an array of parameters plus a layout:
- **The line and logistic regression** use `[w₁ … w_d, b]`. Their output is
  `b + Σ wⱼxⱼ`. Logistic regression reads it as a logit, the log-odds of being
  on time; the line reads it as seconds.
- **The hidden-layer network** uses `W₁ [h × d]`, `b₁ [h]`, `w₂ [h]`, `b₂`.
  Its output is `b₂ + Σ w₂ₖ tanh(b₁ₖ + Σ W₁ₖⱼxⱼ)`, again a logit.

<!-- source: psionic/lib/tabular.coax model -->
```jai
// hidden 0: output = b + sum w_j x_j. hidden h: W1 [h,d], b1 [h], w2 [h], b2.
tb_param_count :: (m:TbModel)->int #export_c {
    if !tb_model_ok(m) {return 0;}
    if m.hidden==0 {return m.features+1;}
    return m.hidden*m.features+2*m.hidden+1;
}
tb_tanh :: (a:f64)->f64 {
    t:=exp(-2*abs(a));v:=(1-t)/(1+t);
    if a<0 {return -v;}
    return v;
}
// exp(min(z, 0)) / (1 + exp(-|z|)): no exponential of a large positive number.
tb_sigmoid :: (z:f64)->f64 {return exp((z-abs(z))/2)/(1+exp(-abs(z)));}
tb_softplus :: (z:f64)->f64 {return (z+abs(z))/2+log(1+exp(-abs(z)));}
tb_hidden :: (m:TbModel,p:[]f64,v:[]f64,h:[]f64) {
    d:=m.features;
    for k:0..<m.hidden {
        a:PsionicAccumulator;a=psionic_ops_accumulate(a,p[m.hidden*d+k]);
        for j:0..<d {a=psionic_ops_accumulate(a,p[k*d+j]*v[j]);}
        h[k]=tb_tanh(psionic_ops_total(a));
    }
}
tb_output :: (m:TbModel,p:[]f64,v:[]f64)->f64 {
    d:=m.features;a:PsionicAccumulator;
    if m.hidden==0 {
        a=psionic_ops_accumulate(a,p[d]);
        for j:0..<d {a=psionic_ops_accumulate(a,p[j]*v[j]);}
        return psionic_ops_total(a);
    }
    h:[16]f64;tb_hidden(m,p,v,h[..]);base:=m.hidden*d+m.hidden;
    a=psionic_ops_accumulate(a,p[base+m.hidden]);
    for k:0..<m.hidden {a=psionic_ops_accumulate(a,p[base+k]*h[k]);}
    return psionic_ops_total(a);
}
```

Every sum uses chapter 3's compensated accumulator. `tanh`, the sigmoid and
`softplus(z) = log(1 + eᶻ)` are written so they never take the exponential of a
large positive number: for |z| = 800 they return exact limits, not overflow.

## Gradients by hand

For log loss on a logit, and for half the squared error of a value, the
derivative of the loss with respect to the output is the same thing, a
residual: `σ(z) − y` for logistic regression and `z − y` for the line. Every
other gradient is that residual times what the parameter multiplied. Through a
hidden unit it is also scaled by `w₂ₖ` and by tanh's slope, `1 − hₖ²`.

<!-- source: psionic/lib/tabular.coax gradient -->
```jai
// Mean gradient of the per-row loss over `rows`: log loss on a logit, or
// half the squared error of a value. Both give d loss / d output = residual.
tb_gradient :: (m:TbModel,p:[]f64,x:[]f64,y:[]f64,rows:[]int,g:[]f64) {
    d:=m.features;n:=tb_param_count(m);acc:[161]PsionicAccumulator;h:[16]f64;
    for r:rows {
        v:=x[r*d..<r*d+d];z:=tb_output(m,p,v);
        residual:=z-y[r];if m.logistic {residual=tb_sigmoid(z)-y[r];}
        if m.hidden==0 {
            for j:0..<d {acc[j]=psionic_ops_accumulate(acc[j],residual*v[j]);}
            acc[d]=psionic_ops_accumulate(acc[d],residual);
        } else {
            tb_hidden(m,p,v,h[..]);base:=m.hidden*d+m.hidden;
            acc[n-1]=psionic_ops_accumulate(acc[n-1],residual);
            for k:0..<m.hidden {
                acc[base+k]=psionic_ops_accumulate(acc[base+k],residual*h[k]);
                back:=residual*p[base+k]*(1-h[k]*h[k]);
                acc[m.hidden*d+k]=psionic_ops_accumulate(acc[m.hidden*d+k],back);
                for j:0..<d {acc[k*d+j]=psionic_ops_accumulate(acc[k*d+j],back*v[j]);}
            }
        }
    }
    for s:0..<n {g[s]=psionic_ops_total(acc[s])/f64(rows.count);}
}
```

The checks compare every one of these gradients with a central difference,
`(L(θ + ε) − L(θ − ε)) / 2ε` with ε = 10⁻⁶, for both the network and the line.
They agree within 10⁻⁸. Chapter 71 derives the same rules mechanically.

## Minibatch SGD

Training starts from a stated initialization:
- each weight draws uniformly from ±1/√(fan-in) at the counter of its slot;
- every bias starts at zero;
- the affine models start at all zeros.

Each epoch shuffles the training rows with Fisher–Yates, on a counter that
carries over from epoch to epoch. Batches are consecutive runs of 16 rows,
and the last batch of an epoch may be short. Each step averages the batch's
gradient and adds `decay · θ` for weights, never for biases, then moves
`θ ← θ − rate · step`.

<!-- source: psionic/lib/tabular.coax train -->
```jai
// Every refusal, before any work. Chapter 71's trainer shares it.
tb_admit_train :: (m:TbModel,c:TbConfig,x:[]f64,y:[]f64,rows:[]int,params:[]f64,order:[]int,overlap:bool)->TbStatus {
    n:=tb_param_count(m);d:=m.features;
    if !tb_budget(m,c,rows.count) || c.batch<1 || c.batch>TB_BATCH_MAX || !(c.rate>0 && c.rate<=16) || !tb_in(c.decay,0,1) {return TbStatus.shape;}
    if x.count!=y.count*d || !tb_row_ok(rows,y.count) {return TbStatus.shape;}
    if params.count<n || order.count<rows.count {return TbStatus.capacity;}
    if overlap {return TbStatus.alias;}
    for r:rows {
        for j:0..<d {if !tb_in(x[r*d+j],-TB_DOMAIN,TB_DOMAIN) {return TbStatus.number;}}
        if m.logistic && y[r]!=0 && y[r]!=1 {return TbStatus.number;}
        if !tb_in(y[r],-TB_DOMAIN,TB_DOMAIN) {return TbStatus.number;}
    }
    return TbStatus.ok;
}
tb_train :: (m:TbModel,c:TbConfig,x:[]f64,y:[]f64,rows:[]int,params:[]f64,order:[]int,overlap:bool)->TbTrain #export_c {
    admitted:=tb_admit_train(m,c,x,y,rows,params,order,overlap);
    if admitted!=TbStatus.ok {return TbTrain.{status=admitted};}
    n:=tb_param_count(m);
    // Train on staged copies; the caller's parameters change only on success.
    p:[161]f64;g:[161]f64;tb_initialize(m,c.init_seed,p[..]);
    o:=order[0..<rows.count];for i:0..<rows.count {o[i]=rows[i];}
    counter:=(cast(u64)0);steps:=0;
    for epoch:0..<c.epochs {
        counter=tb_shuffle(o,c.shuffle_seed,counter);
        start:=0;
        for i:0..<o.count {
            // A batch closes when it holds `batch` rows or the epoch runs out of rows.
            if i+1-start!=c.batch && i+1!=o.count {continue;}
            tb_gradient(m,p[0..<n],x,y,o[start..<i+1],g[0..<n]);
            for s:0..<n {
                step:=g[s];if tb_is_weight(m,s) {step+=c.decay*p[s];}
                next:=p[s]-c.rate*step;
                if !tb_in(next,-TB_DOMAIN,TB_DOMAIN) {return TbTrain.{status=TbStatus.update};}
                p[s]=next;
            }
            steps+=1;start=i+1;
        }
    }
    loss:=tb_loss(m,p[0..<n],x,y,rows);
    for s:0..<n {params[s]=p[s];}
    return TbTrain.{steps=steps,loss=loss};
}
```

Training refuses before it reads data:
- `shape`: bad sizes, rates or decay, row indices out of range, or more work
  than the budget of 2²⁴ row-parameter visits;
- `capacity`: parameter or order storage too short;
- `alias`: overlapping storage;
- `number`: features or targets outside ±2²⁰, or a logistic label that is not
  0 or 1.

It trains on staged copies. If any step would take a parameter outside ±2²⁰,
it stops with `update` and the caller's parameters stay as they were.

## What "better" means

Accuracy isn't enough here. 87.5% of test requests are on time, so a predictor
that always says "on time" is 87.5% accurate and catches none of the 60 late
ones. We score the five predictors three ways:
- **Log loss** scores the probabilities.
- **The line** gets squared error on latency.
- **A written cost** scores the decisions. Missing a late request costs 5, and
  flagging one that would have finished costs 1. A request is flagged when its
  on-time probability is below a threshold.

The rule is what an operator might write from the documentation. It uses the
per-token and queue costs, doubles the estimate for safety, and ignores memory:

<!-- source: psionic/lib/tabular.coax rule -->
```jai
// An operator's rule: the documented per-token and queue costs, doubled for
// safety, and nothing about memory. 1 means "on time", 0 means "flag it".
tb_rule_seconds :: (prompt:f64,queue:f64,large:f64)->f64 {
    cost:=0.0003;if large==1 {cost=0.0009;}
    return 2*(0.2+0.15*queue+prompt*cost);
}
tb_rule :: (x:[]f64,out:[]f64,overlap:bool)->TbStatus #export_c {
    if !tb_table_ok(x,4) {return TbStatus.shape;}
    if out.count<x.count/4 {return TbStatus.capacity;}
    if overlap {return TbStatus.alias;}
    for i:0..<x.count/4 {
        out[i]=0;if tb_on_time(tb_rule_seconds(x[4*i],x[4*i+1],x[4*i+2])) {out[i]=1;}
    }
    return TbStatus.ok;
}
```

At seed 70, on the 480 test requests:

| Predictor | Test log loss | Accuracy | Cost per request | Late requests caught |
| --- | --- | --- | --- | --- |
| Constant | 0.3807 | 0.875 | 0.625 | 0 of 60 |
| Rule | — | 0.8146 | 0.2021 | 58 of 60 |
| Line (latency ≤ 9 s) | — | 0.9292 | 0.1792 | 47 of 60 |
| Logistic regression | 0.1593 | 0.9375 | 0.1625 | 48 of 60 |
| Hidden layer | 0.1434 | 0.9396 | 0.1438 | 50 of 60 |

The rule catches almost every late request, because doubling is cautious, but
it flags many that would have finished. The line's test squared error is 3.88
s², against 12.64 s² for predicting the training mean. Turned into a decision,
though, it is worse than logistic regression, because it was trained for
seconds, not for the deadline.

A threshold of 0.5 ignores the costs. Choosing the cheapest threshold on
validation, from 0.05 to 0.95, gives 0.9 for logistic regression and 0.95 for
the hidden layer. Their test costs drop to 0.1167 and 0.1125. Across the five
seeds, the validation choice lowered the test cost in 9 of 10 cases. In the
tenth (the hidden layer, seed 74) it rose from 0.1146 to 0.1187.

## When the larger model loses

With all 1,440 training rows, the hidden layer has lower test log loss than
logistic regression at 4 of 5 seeds. The exception is seed 76: 0.1327 against
0.1247. Now train both on the first 40 sessions' training rows only, 192 rows,
scaled on those rows, for 200 epochs:

| Seed | Logistic | Hidden layer | Hidden layer, decay 0.01 |
| --- | --- | --- | --- |
| 70 | 0.1771 | 0.2812 | 0.1592 |
| 72 | 0.1522 | 0.2059 | 0.1576 |
| 74 | 0.1659 | 0.2429 | 0.1659 |
| 76 | 0.1216 | 0.1308 | 0.1118 |
| 78 | 0.0974 | 0.1816 | 0.1088 |

At every seed, the 49-parameter network fits its 192 rows and loses to the
5-parameter one; at seed 76 only narrowly. Weight decay of 0.01 helps the
network at every seed. With it, the network beats logistic regression at seeds
70 and 76, ties at 74, and still loses at 72 and 78. Five seeds are not a
benchmark. They show that the ordering depends on the data you have, and that
the cases where it flips are measured, not assumed.

## Supported profile

`cpu-f64-sgd-v1`:
- binary64 storage and arithmetic, chapter 3's compensated sums, no FMA or
  reassociation;
- at most 4,096 rows, 1 to 8 features, 0 to 16 hidden units (161 parameters),
  512 epochs, batches of 1 to 256;
- learning rates in (0, 16], decay in [0, 1];
- features, targets and parameters within ±2²⁰;
- the simulator up to 512 sessions.

Every exported procedure that writes caller storage also takes an `overlap`
observation, which the native adapter computes from real addresses. Each
procedure refuses with a status before it writes anything.

## How it is checked

- **Coax checks:** 55 checks in debug and release. They cover:
  - the simulator against its formula, over two sessions;
  - the split, scaling and admission bounds at their edges;
  - hand-derived SGD steps for logistic regression, the line (with decay)
    and the hidden layer;
  - gradients against central differences;
  - the shuffle, the work budget and every refusal;
  - the metrics, the threshold grid's ends and the rule.

  One check trains a hidden layer for two epochs with a short last batch,
  decay and a reshuffle. It compares the result with the oracle's
  parameters to within 10⁻¹⁴.
- **An independent oracle,** [tabular_oracle.py](../tests/tabular_oracle.py),
  shares no code with the library. Its sums are exactly rounded (`math.fsum`),
  and its sigmoid and softplus are written separately. It runs the whole
  experiment for seeds 70, 72, 74, 76 and 78.
- **The native harness** [tabular.c](../tests/tabular.c) runs the generated C
  through the span adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before the comparison:
  - exact: every feature, label, group, split, chosen threshold, count, accuracy
    and cost;
  - 10⁻¹² relative: latencies;
  - 10⁻⁹ relative: parameters, losses and squared errors.

  The worst observed relative difference is 5.4 × 10⁻¹⁴. The harness also
  checks that NaN, infinity and overlapping storage are refused.
- **Mutation testing** of `tabular.coax` kills all 923 mutants. The
  first run left 104 survivors. Most were boundaries no check reached, or
  branches whose two sides agree at the boundary, like a sigmoid's two
  formulas at zero. The fixes were boundary checks and code without those
  branches:
  - a branch-free sigmoid;
  - shared admission helpers for rows, tables and models;
  - a batch loop with no clamp;
  - no gradient overflow test, because the ±2²⁰ domain makes overflow
    impossible.

The [verification record](../../docs/verification/psionic-ch70-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Simulated data only.** Chapter 14's measured latencies replace the
  simulator later.
- **Handwritten gradients.** Chapter 71 builds reverse-mode differentiation and
  checks it against these.
- **No diagnosis of a failed run.** That is chapter 71a.
- **No calibration measurement or leakage test.** The probabilities aren't
  checked for calibration the way chapter 12 checks Elephant's, and planted
  leakage is chapter 76.
- **One CPU thread, small tables.** The bounds above are the whole profile.
