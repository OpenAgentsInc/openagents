# 164a. Learn which model to call

OpenAgents calls other models: a small one that is cheap and fast, a coding
model, and a large one that is good at nearly everything and costs the most.
Choosing per task is a decision with a price. This chapter learns that choice
twice:
- **With full information:** every model ran on every training task, so a
  small model per candidate can learn when that candidate succeeds.
- **From logs:** only the chosen model's outcome was ever seen. Here the
  question changes from "which model is best?" to "how good would a new router
  have been, judged from an old router's logs?"

It uses [chapter 70](ch070-learning-with-arrays.md)'s logistic regression and
trainer unchanged. The tasks come from a simulator that knows each model's true
success probability, so every estimate can be compared with the truth.

```shell
coax/bin/coax run psionic/examples/model-router.coax
psionic/check.sh --only route
```

```jai
1000 test tasks, expected reward per task (success minus price):
  always small 0.412, always coder 0.6335, always large 0.6979
  old rule (code to the coder, the rest to small) 0.6675
  learned router 0.7663 (small 272, coder 324, large 404)
  best possible, from the true probabilities 0.7692; the router agrees on 942 tasks
logs of the old policy exploring 20 percent, 1000 tasks:
  the router's true value 0.777; the old policy earned 0.655
  naive 0.8542 (544 matching rows), direct 0.7898, inverse propensity 0.8103, doubly robust 0.7991
  with per-model averages as the reward model: direct 0.6874, doubly robust 0.7987
20 more logs, mean error and root mean square error:
  naive: 0.0798 and 0.0811
  direct: 0.0106 and 0.0348
  inverse propensity: 0.0099 and 0.0627
  doubly robust: 0.0075 and 0.0336
  direct, averages: -0.0797 and 0.0812
  doubly robust, averages: 0.0032 and 0.0308
logs of an old policy that never calls the large model: estimate status support
  a world with a worse large model logs the same: true; the router's true value there 0.5446, not 0.777
  logs that do explore it see the difference: inverse propensity 0.5603
```

The [example](../examples/model-router.coax) runs in about a second.

## Tasks, three models and a price

A task has four features:
- its length, from 0 to 1;
- whether it is code (two draws in five);
- its difficulty, from 0 to 1;
- one feature that matters to no model, from −1 to 1.

Each model's chance of success is a logistic function of those features. The
example's world has these weights on length, code and difficulty, then the bias:

```jai
small  -1.0  -3.0  -6.0   bias 4.0    good at short, easy, non-code tasks
coder  -0.5   2.0  -3.0   bias 2.0    good at code
large  -0.5   0.5  -2.5   bias 3.5    good at most things, hurt least by difficulty
```

Only the simulator reads those numbers. It also keeps each true probability, so
the chapter can measure a policy exactly, but no learner ever sees one. Each
model's outcome on a task is drawn at its own counter, so the three outcomes are
independent and the same in every run:

<!-- source: psionic/lib/route.coax simulate -->
```jai
// route-sim-v1. Task i draws length, code (40 percent), difficulty and an
// irrelevant feature at counters 8i to 8i+3 (code when the draw is 0 or 1
// mod 5), then model k's outcome at
// 8i+4+k. `truth` holds each model's bias and four weights; only the
// simulator reads it, and `prob` is kept for measuring, never for learning.
rt_simulate :: (seed:u64,truth:[]f64,x:[]f64,success:[]f64,prob:[]f64,overlap:bool)->RtStatus #export_c {
    n:=rt_tasks(x);
    if truth.count!=5*RT_MODELS || n==-1 {return RtStatus.shape;}
    if success.count<RT_MODELS*n || prob.count<RT_MODELS*n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    for c:truth {if !tb_in(c,-RT_COEF_MAX,RT_COEF_MAX) {return RtStatus.number;}}
    for i:0..<n {
        c:=cast(u64)(8*i);v:=x[4*i..<4*i+4];
        v[0]=tb_uniform(seed,c);v[1]=0;if tb_bits(seed,c+1)%5<2 {v[1]=1;}
        v[2]=tb_uniform(seed,c+2);v[3]=2*tb_uniform(seed,c+3)-1;
        for k:0..<RT_MODELS {
            p:=tb_sigmoid(tb_output(TbModel.{features=4},truth[5*k..<5*k+5],v));
            prob[RT_MODELS*i+k]=p;success[RT_MODELS*i+k]=0;
            if tb_uniform(seed,c+cast(u64)(4+k))<p {success[RT_MODELS*i+k]=1;}
        }
    }
    return RtStatus.ok;
}
```

Prices are published, not learned: dollars per call, plus 2 cents per second of
latency, which grows with length. The large model's price is four to five
times the coder's:

<!-- source: psionic/lib/route.coax price -->
```jai
// Dollars per call, and seconds of latency at length 0 and per unit of length.
rt_dollars :: (k:int)->f64 {
    if k==0 {return 0.001;}
    if k==1 {return 0.01;}
    return 0.1;
}
rt_seconds :: (k:int,length:f64)->f64 {
    if k==0 {return 0.3+0.5*length;}
    if k==1 {return 0.8+1.5*length;}
    return 2.0+4.0*length;
}
// Every model's price on every task: dollars plus 0.02 per second.
rt_price :: (x:[]f64,out:[]f64,overlap:bool)->RtStatus #export_c {
    n:=rt_tasks(x);
    if n==-1 {return RtStatus.shape;}
    if out.count<RT_MODELS*n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    if !rt_finite(x) {return RtStatus.number;}
    for i:0..<n {for k:0..<RT_MODELS {out[RT_MODELS*i+k]=rt_dollars(k)+RT_PER_SECOND*rt_seconds(k,x[4*i]);}}
    return RtStatus.ok;
}
```

A call's reward is its outcome (1 for success, 0 for failure) minus its price.
A policy's **value** is its expected reward per task: the true probability of
success of each chosen model, minus its price, averaged over tasks.

## With full information

Every model ran on all 1,500 training tasks. For each model we train chapter
70's logistic regression on its outcomes: 40 epochs, batches of 16, rate 0.5.
The router then sends each task to the model with the largest predicted success
probability minus price:

<!-- source: psionic/lib/route.coax route -->
```jai
// Success probabilities from three logistic models, five parameters each,
// laid out as chapter 70's: four weights, then the bias.
rt_scores :: (params:[]f64,x:[]f64,out:[]f64,overlap:bool)->RtStatus #export_c {
    n:=rt_tasks(x);
    if params.count!=5*RT_MODELS || n==-1 {return RtStatus.shape;}
    if out.count<RT_MODELS*n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    if !rt_finite(params) || !rt_finite(x) {return RtStatus.number;}
    for i:0..<n {for k:0..<RT_MODELS {
        out[RT_MODELS*i+k]=tb_sigmoid(tb_output(TbModel.{features=4},params[5*k..<5*k+5],x[4*i..<4*i+4]));
    }}
    return RtStatus.ok;
}
// Each task goes to the model with the largest probability minus price.
// A tie keeps the lower index, which is the cheaper model.
rt_choose :: (prob:[]f64,price:[]f64,out:[]int,overlap:bool)->RtStatus #export_c {
    n:=rt_triples(prob);
    if n==-1 || price.count!=prob.count {return RtStatus.shape;}
    if out.count<n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    if !rt_unit(prob) || !rt_finite(price) {return RtStatus.number;}
    for i:0..<n {
        best:=0;
        for k:1..<RT_MODELS {
            if prob[RT_MODELS*i+k]-price[RT_MODELS*i+k]>prob[RT_MODELS*i+best]-price[RT_MODELS*i+best] {best=k;}
        }
        out[i]=best;
    }
    return RtStatus.ok;
}
// The expected reward of the choices: mean of probability minus price.
rt_value :: (actions:[]int,prob:[]f64,price:[]f64)->RtValue #export_c {
    n:=rt_triples(prob);
    // A bad table makes n -1, which no count of actions matches.
    if price.count!=prob.count || actions.count!=n {return RtValue.{status=RtStatus.shape};}
    for k:actions {if !rt_in3(k) {return RtValue.{status=RtStatus.shape};}}
    if !rt_unit(prob) || !rt_finite(price) {return RtValue.{status=RtStatus.number};}
    a:PsionicAccumulator;
    for i:0..<n {k:=RT_MODELS*i+actions[i];a=psionic_ops_accumulate(a,prob[k]-price[k]);}
    return RtValue.{value=psionic_ops_total(a)/f64(n)};
}
```

On 1,000 new tasks:

| Policy | Value per task |
| --- | --- |
| Always small | 0.4120 |
| Always coder | 0.6335 |
| Always large | 0.6979 |
| Old rule: code to the coder, the rest to small | 0.6675 |
| Learned router (272 small, 324 coder, 404 large) | 0.7663 |
| Best possible, from the true probabilities | 0.7692 |

The learned router recovers 96 percent of the gap between always calling the
large model and the best possible choice. It agrees with the best choice on 942
tasks, and its 58 different choices cost 0.003 per task in all. The old hand-written rule does worse than calling the large
model every time, because it never uses the large model for hard tasks.

This setting is easy on purpose. The true success curves are logistic, the same
family the learner fits, and the data cover every model on every task.

## Logs show one outcome

Production has no full information. Each request goes to one model, and the log
records only that model's outcome. Suppose the old rule was running, with
**exploration**: with probability 0.8 it took its usual choice, and otherwise it
picked one of the other two models at random. It writes down the probability of
the action it took, its **propensity**. [Chapter 158](PLAN.md)'s decision
records keep exactly this field:

<!-- source: psionic/lib/route.coax log -->
```jai
// The old router: code to the coder, everything else to the small model,
// with probability `explore` shared among the other models it allows.
rt_old :: (x:[]f64,explore:f64,large:bool,out:[]f64,overlap:bool)->RtStatus #export_c {
    n:=rt_tasks(x);
    if n==-1 || !tb_in(explore,0,1) {return RtStatus.shape;}
    if out.count<RT_MODELS*n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    for i:0..<n {if x[4*i+1]!=0 && x[4*i+1]!=1 {return RtStatus.number;}}
    share:=explore;if large {share=explore/2;}
    for i:0..<n {
        o:=out[RT_MODELS*i..<RT_MODELS*i+RT_MODELS];
        for k:0..<RT_MODELS {o[k]=share;}
        o[cast(int)x[4*i+1]]=1-explore;
        if !large {o[2]=0;}
    }
    return RtStatus.ok;
}
// Each task's propensities lie in [0, 1] and sum to exactly 1, added in order.
rt_props_ok :: (props:[]f64)->bool {
    if !rt_unit(props) {return false;}
    for i:0..<props.count/RT_MODELS {if props[3*i]+props[3*i+1]+props[3*i+2]!=1 {return false;}}
    return true;
}
// Draw one action per task at counter i, and keep only its outcome and reward.
rt_log :: (seed:u64,props:[]f64,success:[]f64,price:[]f64,actions:[]int,outcome:[]f64,reward:[]f64,overlap:bool)->RtStatus #export_c {
    n:=rt_triples(props);
    if n==-1 || success.count!=props.count || price.count!=props.count {return RtStatus.shape;}
    if actions.count<n || outcome.count<n || reward.count<n {return RtStatus.capacity;}
    if overlap {return RtStatus.alias;}
    if !rt_props_ok(props) || !rt_binary(success) || !rt_finite(price) {return RtStatus.number;}
    for i:0..<n {
        u:=tb_uniform(seed,cast(u64)i);p:=props[3*i..<3*i+3];
        a:=2;if u<p[0] {a=0;} else if u<p[0]+p[1] {a=1;}
        actions[i]=a;outcome[i]=success[3*i+a];reward[i]=success[3*i+a]-price[3*i+a];
    }
    return RtStatus.ok;
}
```

The propensities must sum to exactly 1, so a model with propensity 0 is never
drawn. A draw that lands exactly on a boundary goes to the next model; a check
builds the propensities from the draw itself to prove it.

We now want the value of the learned router from a log of 1,000 tasks it never
ran on. On this log its true value is 0.7770. The old policy earned 0.6550.

## Four estimates

<!-- source: psionic/lib/route.coax estimate -->
```jai
// A new deterministic policy `target`, judged from logs of the old one.
// `model` is a reward model's success probability for every task and action.
// naive: the mean reward of logged rows where the old choice matches the new.
// direct: the reward model's mean for the new choices.
// ips: matching rewards weighted by 1 / propensity, over all tasks.
// dr: direct, corrected by the weighted residual of matching rows.
// The new policy must choose only actions the old one could have taken.
rt_estimate :: (target:[]int,actions:[]int,reward:[]f64,props:[]f64,model:[]f64,price:[]f64)->RtEstimate #export_c {
    n:=rt_triples(props);
    if target.count!=n || actions.count!=n || reward.count!=n || model.count!=props.count || price.count!=props.count {return RtEstimate.{status=RtStatus.shape};}
    for i:0..<n {if !rt_in3(target[i]) || !rt_in3(actions[i]) {return RtEstimate.{status=RtStatus.shape};}}
    if !rt_props_ok(props) || !rt_finite(reward) || !rt_unit(model) || !rt_finite(price) {return RtEstimate.{status=RtStatus.number};}
    for i:0..<n {if props[3*i+actions[i]]==0 {return RtEstimate.{status=RtStatus.shape};}}
    for i:0..<n {if props[3*i+target[i]]==0 {return RtEstimate.{status=RtStatus.support};}}
    matched:=0;naive:PsionicAccumulator;direct:PsionicAccumulator;ips:PsionicAccumulator;dr:PsionicAccumulator;
    for i:0..<n {
        t:=3*i+target[i];q:=model[t]-price[t];
        direct=psionic_ops_accumulate(direct,q);dr=psionic_ops_accumulate(dr,q);
        if actions[i]!=target[i] {continue;}
        matched+=1;naive=psionic_ops_accumulate(naive,reward[i]);
        ips=psionic_ops_accumulate(ips,reward[i]/props[t]);
        dr=psionic_ops_accumulate(dr,(reward[i]-q)/props[t]);
    }
    if matched==0 {return RtEstimate.{status=RtStatus.support};}
    return RtEstimate.{matched=matched,naive=psionic_ops_total(naive)/f64(matched),direct=psionic_ops_total(direct)/f64(n),
        ips=psionic_ops_total(ips)/f64(n),dr=psionic_ops_total(dr)/f64(n)};
}
```

With n tasks, a target choice t, a logged choice a with propensity π, a reward
r and a reward model's estimate q:

```jai
naive  = mean of r over rows where a = t
direct = (1/n) Σ q(t)
ips    = (1/n) Σ [a = t] r / π(a)
dr     = (1/n) Σ ( q(t) + [a = t] (r − q(a)) / π(a) )
```

The reward model here is chapter 70's logistic regression again, one per model,
trained only on the rows where the old policy chose that model. A second,
deliberately poor reward model ignores the task and uses each model's average
logged outcome. On the log above:

| Estimate | Value | Error |
| --- | --- | --- |
| Naive, over 544 matching rows | 0.8542 | +0.0772 |
| Direct, logistic reward model | 0.7898 | +0.0128 |
| Inverse propensity | 0.8103 | +0.0334 |
| Doubly robust, logistic reward model | 0.7991 | +0.0222 |
| Direct, per-model averages | 0.6874 | −0.0896 |
| Doubly robust, per-model averages | 0.7987 | +0.0218 |

One log is one draw. Over 20 more logs, each with fresh tasks and fresh draws:

| Estimate | Mean error | Root mean square error |
| --- | --- | --- |
| Naive | +0.0798 | 0.0811 |
| Direct, logistic | +0.0106 | 0.0348 |
| Inverse propensity | +0.0099 | 0.0627 |
| Doubly robust, logistic | +0.0075 | 0.0336 |
| Direct, averages | −0.0797 | 0.0812 |
| Doubly robust, averages | +0.0032 | 0.0308 |

What the table shows:
- **Naive is biased.** A row is kept when the old policy happened to choose what
  the router would. That happens with probability 0.8 on tasks where the router
  agrees with the old rule and 0.1 elsewhere, so the matching rows overweight
  one kind of task eight to one. Their mean describes a different mix of tasks.
  Its errors vary little from log to log, so the error is bias, not noise,
  and more data would not help.
- **Inverse propensity removes that bias but is noisy.** Dividing by the
  propensity counts each matching row as many tasks as it stands for. An
  explored large-model row stands for ten. Its spread is about twice the others'.
- **Direct is only as good as its reward model.** With a reward model of the
  right shape it does well. With averages that ignore the task it is as wrong as
  naive, in the other direction.
- **Doubly robust is good with either reward model.** The weighted residual
  corrects a poor reward model, and a good reward model leaves little residual
  to weight, so it is less noisy than inverse propensity.

Twenty logs cannot rank the small biases of direct, inverse propensity and doubly
robust: their mean errors are within about two standard errors of zero. They do
show naive's and the poor direct estimate's biases, more than 20 standard
errors from zero, and inverse propensity's larger spread.

## What the logs cannot tell

Now suppose the old policy never called the large model: 0.8 to its rule and 0.2
to the other cheap model. The learned router sends 375 of this log's 1,000
tasks to the large model. The logs say nothing about how those calls would have gone, so
`rt_estimate` refuses with `support` before computing anything.

The refusal is not caution for its own sake. The example builds a second world
whose large model is much worse (bias 0.5 instead of 3.5). The features, the
small and coder outcomes and every logging draw are unchanged. The logs of the old policy are identical in
both worlds, row for row. The router's true value is 0.7770 in the first world
and 0.5446 in the second. No estimator can tell the two apart from those logs,
and any number it reported for the router would be wrong in one of them.

Logs from the exploring policy do tell them apart. In the second world, inverse
propensity estimates 0.5603. A policy can be judged from logs only over actions
the old policy sometimes took, so exploration has to cover every model a future
router might call.

## Supported profile

`cpu-f64-route-v1`:
- binary64, with chapter 3's compensated accumulator for every sum;
- three models, 1 to 4,096 tasks of four features;
- simulator weights within ±64, features, prices and rewards within ±2²⁰,
  and probabilities and propensities within [0, 1];
- propensities that sum to exactly 1 per task, added in order, and outcomes of
  exactly 0 or 1;
- deterministic target policies only.

A logged action with propensity 0 is refused as `shape`. A target action with
propensity 0, or no matching row at all, is refused as `support`. Every exported
procedure that writes caller storage takes an `overlap` observation, which the
native adapter computes from real addresses.

## How it is checked

- **Coax checks:** 72 checks in debug and release, counting those of the units
  it loads. They cover:
  - prices, scores, choices and values by hand, with ties keeping the cheaper
    model and the incumbent's price counting;
  - the simulator's features, probabilities and outcomes at their counters,
    with outcome and code frequencies within four standard deviations over
    4,096 tasks;
  - a draw exactly equal to a success probability, which is a failure, and
    a draw exactly equal to a cumulative propensity, which goes to the next
    model;
  - every refusal at its bound;
  - the four estimates on a two-task log worked by hand.
- **An independent oracle,** [route_oracle.py](../tests/route_oracle.py),
  written from the specification. It runs the whole experiment with
  exactly rounded sums and chapter 70's oracle trainer.
- **The native harness** [route.c](../tests/route.c) runs the same experiment
  through the span adapter, plain and under ASan/UBSan, in debug and release.
  Tolerances were frozen before comparison. Every draw, logged action, choice,
  count and refusal must be equal. Values from true probabilities must agree
  within 10⁻¹², and trained parameters and estimates within 10⁻⁹.
  The router's parameters came out equal to the oracle's. Values agree within
  3.3 × 10⁻¹⁶, estimates and the 20-log errors within 1.4 × 10⁻¹⁷. The harness
  also reproduces the identical logs of the two worlds, and checks that NaN,
  infinity and overlapping storage are refused.
- **Mutation testing** of `route.coax` kills all 1,390 mutants. The first run
  left 24 survivors. All were fixed in the code or the checks:
  - draws compared with `<` whose equal case no check reached. The outcome and
    logging draws now have checks that force equality, and the code draw
    became an integer comparison;
  - output buffers that the checks had zeroed in advance;
  - a model flag that `tb_output` never reads, now removed;
  - a row bound that could never be hit because counts are multiples of four,
    now stated on the row count;
  - two `n == −1` tests already implied by a count comparison, now removed;
  - missing checks at 1 and 4,096 tasks, at the first task of a loop, and for
    a choice that depends on the incumbent's price.

The [verification record](../../docs/verification/psionic-ch164a-2026-10-29.md)
has the commands and results.

## What this does not do yet

- **Simulated tasks and models.** The real version learns from chapter 158's
  logged decisions, with real outcomes that arrive late or never.
- **The learner fits the true family.** The true success curves are logistic in
  the same features. A misspecified world, where the reward model is wrong in
  shape and not just in data, is where doubly robust matters most.
- **No confidence intervals** on any estimate, and one exploration rate.
- **Deterministic routers.** A router that randomizes its own choices needs the
  same estimators weighted by its probabilities.
- **No promotion.** Chapter 164 decides when a new router may replace the old
  one. This chapter only measures it.
