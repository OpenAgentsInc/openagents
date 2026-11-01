# 139. Learn from rollouts

> **Status: partly implemented.** On-policy learning from sampled rollouts
> on chapter 136's calculator task: seeded rollouts, a checker that grades
> each one from its tokens, group-relative advantages with a penalty on the
> log-probability ratio to a frozen reference, a policy-gradient step
> through chapter 127's masked gradient, refusal of rollouts sampled from
> another policy version, and a held-out acceptance rule frozen before the
> run. Still to come: rollouts drawn from chapter 242's environments, held-out
> comparisons against chapter 240's noise floor, and nanochat's own GSM8K
> task.

Supervised fine-tuning shows a model the right answer. Learning from
rollouts lets the model answer for itself, scores each answer with a
checker, and raises the probability of the answers that scored above the
others. nanochat's `chat_rl.py` does this on GSM8K: for each question it
samples several answers, rewards the ones whose final number is right, and
takes a policy-gradient step weighted by each answer's reward minus the
mean reward of its group.

This chapter builds that recipe in Coax on chapter 136's task, arithmetic
questions answered through a calculator tool, where the reward can be
checked exactly. Every decision is made by one `#authority` file,
[rollouts.coax](../lib/nanochat/rollouts.coax), whose checks leave no
mutant alive.

## The profile

This chapter's profile is `nanochat-rollouts-cpu-f64-v1`:

- Binary64 on the CPU, one thread, every sum in a fixed order. The short
  run's transcript is exact for the named build in debug and release; the
  recorded run's is exact in release.
- The model is chapter 136's: chapter 128's two blocks of width 32 over 329
  tokens, 53,290 parameters. Both runs rebuild chapter 136's base run step
  for step and its tuning run up to a cut, and the policy and the frozen
  reference both start from the weights at the cut.
- Rollouts are drawn by chapter 126's engine with chapter 53's sampler at
  temperature 1, from chapter 103's counter generator, so a seed names one
  rollout. The sampler reads binary32 logits; the step's log-probabilities
  are binary64, from the same logits before rounding.
- Gradients are chapter 127's masked gradients, through chapter 138's
  `pf_grad`; the schedule, clipping and AdamW are chapter 128's.

## The recipe

nanochat's recipe, pinned in the [target notes](../docs/targets/nanochat.md),
is plain REINFORCE with a group baseline:

- For each prompt, sample a group of `K` answers from the current policy.
- Reward each answer with a checker.
- The advantage of answer `i` is its reward minus the mean reward of its
  group.
- The loss is `−Σ_i A_i · log π(y_i)`, summed over every sampled token and
  divided by the number of sampled tokens.
- One optimizer step per batch of fresh samples, so every sample comes from
  the weights being updated. There is no ratio to an older policy and no
  clipping of that ratio, as PPO would have.

This chapter adds one thing to that recipe: a penalty on drifting from a
frozen reference, the starting weights. A rollout's reward is shaped to
`r_i − β·(log π(y_i) − log π_ref(y_i))`, where both log-probabilities are
summed over the rollout's sampled tokens. The mean of
`log π(y) − log π_ref(y)` over samples from `π` is the KL divergence from
`π_ref` to `π`, so the shaped reward's expected value is the reward minus `β`
times that divergence. With `β = 0` the recipe is nanochat's. An option
divides each advantage by its group's standard deviation, as GRPO does; the
runs here leave it off.

## The starting policy

Chapter 136's full tuning run answers all 256 held-out questions exactly,
which leaves nothing for rollouts to improve. A model that never earns a
reward gets no gradient either: every advantage in its groups is 0. The
runs here start where chapter 136 is imperfect: its tuning run stopped
after step 250 of 1,200, the same plan and the same draws, so the weights
are exactly chapter 136's at that step. At the cut the model answers about
a third of the held-out questions exactly; its sampled rollouts show the
typical miss, a right call followed by an answer that does not copy the
calculator's output.

## Rollouts

A rollout is one sampled exchange. It renders the question with chapter
126's `ne_prompt`, draws the first reply with `ne_generate`, and, when the
reply closes a call, runs the calculator, writes its output, and draws the
second reply. It differs from chapter 136's `sf_ask` in three ways: every
token is drawn by the sampler at temperature 1, the stop token the sampler
drew (`<|assistant_end|>` or `<|bos|>`) stays in the rollout, because the
policy chose it, and the rollout records a target for every token the model
drew. The prompt and the calculator's output are not the model's choices,
so they have no targets.

The two replies draw from two streams of the seed: `cr_bits(seed, 0)` and
`cr_bits(seed, 1)`. Reusing one stream for both would make the second
reply's draws depend on the first's, and the rollout would not be a sample
from the policy.

<!-- source: psionic/lib/nanochat/rollouts.coax rollout -->
```jai
// One sampled exchange in tok, its targets in tgt. Like chapter 136's
// sf_ask, but every token is drawn with options from seed's two streams, the
// stop token the sampler drew is kept, and every token the model drew is a
// target: the prompt and the calculator's output are not.
rl_rollout :: (c:NcConfig,w:[]f64,table:[]u8,specials:[]int,p:SfProblem,options:NxOptions,seed:u64,work:NeWork,tok:[]int,tgt:[]int)->RlRoll {
    if tok.count<c.context || tgt.count<c.context {return RlRoll.{status=RlStatus.shape};}
    q:[24]u8;m:=[2]NcMessage.[NcMessage.{role=NcRole.user,text=sf_text(q[..],sf_question(p,q[..]))},NcMessage.{role=NcRole.assistant}];none:[1]NcPart;
    pr:=ne_prompt(table,specials,c.vocab,m[..],none[..],tok,false);
    if pr.status!=NeStatus.ok {return RlRoll.{status=RlStatus.shape};}
    n:=pr.written;if n>=c.context {return RlRoll.{status=RlStatus.window};}
    base:=specials[0];
    g:=ne_generate(c,w,base,tok[0..<n],options,cr_bits(seed,0),c.context-n,tok[n..<c.context],work,false);
    if g.status!=NeStatus.ok {return RlRoll.{status=RlStatus.model};}
    end:=rl_close(base,g,n,tok);r:=RlRoll.{n=end,prompt=n,positions=g.positions};at:=end;
    if g.stop==NeStop.tool {
        s:=n;while tok[s]!=base+NC_PYTHON_START {s+=1;}
        d:=bt_decode(table,tok[s+1..<end-1]);k:=sf_calc(cast(string)d.bytes[0..<d.written]);
        o:[24]u8;out:=bt_encode(table,sf_text(o[..],sf_itoa(k.value,o[..])));at=end+out.written+2;
        if k.ok && at<c.context {
            tok[end]=base+NC_OUTPUT_START;for i:0..<out.written {tok[end+1+i]=out.ids[i];}tok[at-1]=base+NC_OUTPUT_END;
            h:=ne_generate(c,w,base,tok[0..<at],options,cr_bits(seed,1),c.context-at,tok[at..<c.context],work,false);
            r.n=rl_close(base,h,at,tok);r.positions+=h.positions;
        }
    }
    for i:0..<r.n {tgt[i]=-1;}
    for i:n-1..<end-1 {tgt[i]=tok[i+1];}
    for i:at-1..<r.n-1 {tgt[i]=tok[i+1];}
    return r;
}
// The reply's end: a drawn <|assistant_end|> or <|bos|> stays, as the token
// the sampler chose.
rl_close :: (base:int,g:NeReply,at:int,tok:[]int)->int {
    end:=at+g.written;
    if g.stop==NeStop.assistant_end {tok[end]=base+NC_ASSISTANT_END;end+=1;}
    if g.stop==NeStop.bos {tok[end]=base;end+=1;}
    return end;
}
```

For `1+2` answered the way chapter 136 trains, the rollout is the same 25
tokens as chapter 136's conversation for that problem, and its targets are
the same as chapter 136's mask:

```text
position  0 ... 13  14  15 16 17 18  19  20 21  22 23  24
token   320 ... 322 323 325 49 43 50 326 327 51 328 51 324
target    - ...   - 325  49 43 50 326   -   -  -  51 324  -
```

Positions 15 to 19 are the call the model drew. The calculator wrote 20 to
22, so the targets at 19 to 21, which would predict them, are `-`. The
model drew 23, the answer, and 24, `<|assistant_end|>`. Seven tokens are
sampled: `T = 7`.

## The checker

The checker grades a rollout from its tokens alone. `called` holds when the
first python block is exactly the question's expression. `answered` holds
when the exchange ends with `<|assistant_end|>` and the reply before it,
after the calculator's output when there was a call, is exactly the right
number. The reward is 1 when both hold and 0 otherwise. A rollout whose
recorded output is not what the calculator returns for its call is refused:
the checker never trusts the environment's side of the record.

<!-- source: psionic/lib/nanochat/rollouts.coax grade -->
```jai
// The checker, from the tokens alone. called: the first python block holds
// exactly the question's expression. answered: the exchange ends with
// <|assistant_end|> and the reply before it, after the calculator's output
// when there was a call, is exactly the right number. A rollout whose
// recorded output is not what the calculator returns for the call is refused.
rl_grade :: (table:[]u8,base:int,p:SfProblem,tok:[]int,prompt:int)->RlGrade {
    n:=tok.count;if prompt<1 || prompt>=n {return RlGrade.{status=RlStatus.shape};}
    e:[12]u8;v:[12]u8;expr:=sf_text(e[..],sf_expr(p,e[..]));want:=sf_text(v[..],sf_itoa(sf_value(p),v[..]));
    done:=tok[n-1]==base+NC_ASSISTANT_END;r:=RlGrade.{};
    s:=prompt;while s<n && tok[s]!=base+NC_PYTHON_START {s+=1;}
    f:=s;while f<n && tok[f]!=base+NC_PYTHON_END {f+=1;}
    if f==n {r.answered=done && sf_says(table,tok[prompt..<n-1],want);return r;}
    d:=bt_decode(table,tok[s+1..<f]);call:=cast(string)d.bytes[0..<d.written];r.called=call==expr;
    if f==n-1 {return r;}
    k:=sf_calc(call);o:[24]u8;out:=bt_encode(table,sf_text(o[..],sf_itoa(k.value,o[..])));at:=f+out.written+3;
    if !k.ok || at>n || tok[f+1]!=base+NC_OUTPUT_START || tok[at-1]!=base+NC_OUTPUT_END {return RlGrade.{status=RlStatus.tool};}
    for i:0..<out.written {if tok[f+2+i]!=out.ids[i] {return RlGrade.{status=RlStatus.tool};}}
    r.answered=done && sf_says(table,tok[at..<n-1],want);
    return r;
}
rl_reward :: (g:RlGrade)->f64 {return ifx g.called && g.answered then 1.0 else 0.0;}
```

`called` and `answered` mean what chapter 136's `sf_ask` means by them. The
checks confirm it: greedy rollouts from programmed models that call
correctly, call the wrong expression, answer without the tool, write text
before the call, make a call the calculator cannot run, or stop on
`<|bos|>` give `sf_ask`'s tokens and `sf_ask`'s verdict.

## Advantages, by hand

Take one group of four rollouts with rewards `1 0 0 1`, summed
log-probabilities `−2 −3 −1 −4` under the policy and `−2.5 −3 −2 −3` under
the reference, and `β = 0.1`. The log-ratios are `0.5 0 1 −1`, so the shaped
rewards are:

```text
s = r − β·(log π − log π_ref) = 1 − 0.05, 0 − 0, 0 − 0.1, 1 + 0.1
  = 0.95, 0, −0.1, 1.1          mean 0.4875
A = s − mean = 0.4625, −0.4875, −0.5875, 0.6125
```

Divided by the group's standard deviation,
`sqrt((0.4625² + 0.4875² + 0.5875² + 0.6125²)/4) = sqrt(0.29296875) =
0.54127`, the first advantage would be `0.85448`. A group whose shaped
rewards are all equal has advantages of 0, scaled or not: it carries no
signal.

<!-- source: psionic/lib/nanochat/rollouts.coax advantages -->
```jai
// Group-relative advantages. Rollout i's shaped reward is its reward less
// beta times its log-probability ratio to the reference; its advantage is
// that minus its group's mean, and over the group's standard deviation when
// scale is set (0 when the group is all alike).
rl_advantages :: (group:int,beta:f64,scale:bool,reward:[]f64,logp:[]f64,logr:[]f64,out:[]f64)->RlStatus {
    n:=reward.count;
    if group<1 || n%group!=0 || logp.count!=n || logr.count!=n || out.count!=n {return RlStatus.data;}
    if !(beta>=0) {return RlStatus.config;}
    for i:0..<n {out[i]=reward[i]-beta*(logp[i]-logr[i]);}
    for a:0..<n/group {at:=a*group;mean:=0.0;for i:at..<at+group {mean+=out[i];}mean/=f64(group);
        spread:=0.0;for i:at..<at+group {out[i]-=mean;spread+=out[i]*out[i];}sd:=sqrt(spread/f64(group));
        if scale {for i:at..<at+group {out[i]=ifx sd>0 then out[i]/sd else 0.0;}}}
    return RlStatus.ok;
}
```

## The policy gradient, by hand

The step minimizes the surrogate loss

```text
L = −(1/T) Σ_i A_i · log π(y_i)
```

where `log π(y_i)` is rollout `i`'s summed log-probability over its sampled
tokens and `T` is the batch's sampled tokens. Its gradient is
`−(1/T) Σ_i A_i ∇log π(y_i)`, and `∇log π(y_i)` is what chapter 138's
`pf_grad` returns: minus the rollout's kept targets times chapter 127's
masked gradient of its mean loss. Both are called in chapter 138's summed
mode, `PfMode.dpo`, which reads no verdict token; they are passed `c.vocab`,
an ID no token has. The
advantages are constants: the gradient does not flow through the group mean
or the penalty.

Take four stand-in rollouts over 12 tokens with 3, 3, 3 and 2 sampled
tokens, `T = 11`, and advantages `0.5 −0.25 −0.75 0.5`. With the output head
set to zero every logit is 0, so every sampled token has log-probability
`−ln 12`, and

```text
L = −(1/11)·(0.5·(−3 ln 12) − 0.25·(−3 ln 12) − 0.75·(−3 ln 12) + 0.5·(−2 ln 12))
  = (ln 12 / 11)·(1.5 − 0.75 − 2.25 + 1) = −0.5 · 2.48491 / 11 = −0.11295
```

The checks compare the step's gradient with the sum of chapter 127's
gradients weighted by `A_i · kept_i / T`, and with central differences of
the surrogate computed by chapter 125's forward pass in four tensors.

<!-- source: psionic/lib/nanochat/rollouts.coax collect -->
```jai
// One step's rollouts at policy version `version`: prompts problems, taken
// in turn, group rollouts each. Rollout k of prompt j at version s draws from
// the seed cr_bits(plan.seed, (s*prompts+j)*group+k). Each is graded, read by
// the policy and the frozen reference, and given its advantage.
rl_collect :: (plan:RlPlan,version:int,c:NcConfig,w:[]f64,ref:[]f64,table:[]u8,specials:[]int,problems:[]SfProblem,work:NeWork,k:PfWork,
               tok:[]int,tgt:[]int,start:[]int,reward:[]f64,logp:[]f64,logr:[]f64,adv:[]f64)->RlBatch {
    count:=plan.prompts*plan.group;b:=RlBatch.{version=version};
    if plan.prompts<1 || plan.group<1 || problems.count<1 || version<0 {b.status=RlStatus.config;return b;}
    if start.count<count+1 || reward.count<count || logp.count<count || logr.count<count || adv.count<count || tok.count<count*c.context || tgt.count<count*c.context {b.status=RlStatus.shape;return b;}
    options:=NxOptions.{};start[0]=0;
    for j:0..<plan.prompts {at:=version*plan.prompts+j;p:=problems[at%problems.count];
        for g:0..<plan.group {i:=j*plan.group+g;x:=start[i];
            r:=rl_rollout(c,w,table,specials,p,options,cr_bits(plan.seed,cast(u64)(at*plan.group+g)),work,tok[x..<x+c.context],tgt[x..<x+c.context]);
            if r.status!=RlStatus.ok {b.status=r.status;return b;}
            q:=rl_grade(table,specials[0],p,tok[x..<x+r.n],r.prompt);
            a:=pf_value(c,w,PfMode.dpo,c.vocab,tok[x..<x+r.n],tgt[x..<x+r.n],k);if a.status!=PfStatus.ok {b.status=RlStatus.shape;return b;}
            z:=pf_value(c,ref,PfMode.dpo,c.vocab,tok[x..<x+r.n],tgt[x..<x+r.n],k);if z.status!=PfStatus.ok {b.status=RlStatus.model;return b;}
            reward[i]=rl_reward(q);logp[i]=a.value;logr[i]=z.value;start[i+1]=x+r.n;
            b.kept+=sf_kept(tgt[x..<x+r.n]);b.positions+=r.positions;b.reward+=reward[i];b.kl+=a.value-z.value;if q.called && q.answered {b.rewarded+=1;}}}
    b.count=count;b.tokens=start[count];b.reward/=f64(count);b.kl/=f64(count);
    b.status=rl_advantages(plan.group,plan.beta,plan.scale,reward[0..<count],logp[0..<count],logr[0..<count],adv[0..<count]);
    return b;
}
```

<!-- source: psionic/lib/nanochat/rollouts.coax step -->
```jai
// One policy-gradient step on a batch. The surrogate loss is
// -sum_i A_i log pi(y_i) / T, T the batch's sampled tokens, so the step adds
// -A_i/T times rollout i's summed log-probability gradient; then clipping,
// the schedule and AdamW as in chapter 128. A batch sampled at another
// version, or whose recorded log-probabilities the current weights do not
// give, is refused before anything moves.
rl_step :: (p:NpPlan,run:NpRun,batch:RlBatch,tok:[]int,tgt:[]int,start:[]int,logp:[]f64,adv:[]f64,w:[]f64,m:[]f64,v:[]f64,total:[]f64,k:PfWork)->RlStep {
    c:=p.model;n:=w.count;fail:=RlStep.{run=run};
    if run.step>=p.steps {fail.status=RlStatus.config;return fail;}
    if batch.status!=RlStatus.ok || batch.count<1 || start.count<batch.count+1 || logp.count<batch.count || adv.count<batch.count || batch.kept<1 {fail.status=RlStatus.data;return fail;}
    if batch.version!=run.step {fail.status=RlStatus.stale;return fail;}
    for i:0..<n {total[i]=0;}loss:=0.0;T:=f64(batch.kept);
    for i:0..<batch.count {a:=start[i];b:=start[i+1];
        x:=pf_grad(c,w,PfMode.dpo,c.vocab,tok[a..<b],tgt[a..<b],k.g,k);
        if x.status!=PfStatus.ok {fail.status=RlStatus.model;return fail;}
        d:=x.value-logp[i];if !(d<=RL_SAME && d>=-RL_SAME) {fail.status=RlStatus.stale;return fail;}
        f:=adv[i]/T;for j:0..<n {total[j]-=f*k.g[j];}loss-=f*x.value;
    }
    sum:=0.0;for x:total {sum+=x*x;}norm:=sqrt(sum);
    if p.clip>0 && norm>p.clip {for i:0..<n {total[i]=total[i]*p.clip/norm;}}
    scale:=np_scale(p,run.step);groups:[5]AwGroup;np_groups(p,scale,groups[..]);none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,w,total,m,v,none[0..<0],false);
    if a.status!=AwStatus.ok {fail.status=RlStatus.config;return fail;}
    return RlStep.{run=NpRun.{step=run.step+1,state=a.state},loss=loss,norm=norm,scale=scale};
}
```

## On-policy means this version

The recipe is only right for rollouts drawn from the weights being updated.
A batch carries the step number it was sampled at, and the step refuses a
batch from any other step as `stale` before anything moves. That catches a
batch reused by mistake. It does not catch a batch relabelled with the right
number, so the step also compares each rollout's recorded log-probability
with the one it computes from the current weights. The two come from
different code, chapter 125's forward pass and chapter 127's tape, and agree
to rounding; a difference above `2^-30` means the rollout came from other
weights, and the batch is refused as `stale` too.

## Held-out success and the acceptance rule

The held-out questions are chapter 136's held-out problems, asked greedily,
each graded by the checker. Each question's outcome before and after
training is kept, so the comparison is paired: a question can be gained
(wrong before, right after) or lost. The rule was written before the first
run.

<!-- source: psionic/lib/nanochat/rollouts.coax accept -->
```jai
// The acceptance rule, frozen before the first run, on the same held-out
// questions asked greedily before and after (1 where the reward was 1). Of
// the questions whose outcome changed, the share that changed to right has
// a 95% Wilson interval whose lower end is above one half: more gained than
// lost, beyond chance.
rl_accept :: (before:RlRate,after:RlRate,was:[]u8,now:[]u8)->bool {
    if before.status!=RlStatus.ok || after.status!=RlStatus.ok || after.count!=before.count {return false;}
    return rl_above(rl_changes(was,now),0.5);
}
// right: the questions gained; count: those gained or lost.
rl_changes :: (was:[]u8,now:[]u8)->RlRate {
    r:=RlRate.{};if was.count!=now.count {r.status=RlStatus.data;return r;}
    for i:0..<was.count {if was[i]!=now[i] {r.count+=1;if now[i]==1 {r.right+=1;}}}
    return r;
}
// The lower end of r's interval is above bound.
rl_above :: (r:RlRate,bound:f64)->bool {return r.status==RlStatus.ok && pf_wilson(r.right,r.count).lo>bound;}
```

<!-- source: psionic/lib/nanochat/rollouts.coax rate -->
```jai
// Held-out success: each question answered greedily and graded by rl_grade,
// its outcome in out (1 when the reward is 1).
rl_rate :: (c:NcConfig,w:[]f64,table:[]u8,specials:[]int,problems:[]SfProblem,work:NeWork,out:[]u8)->RlRate {
    r:=RlRate.{};if problems.count==0 || out.count!=problems.count {r.status=RlStatus.data;return r;}
    tok:[NE_PROMPT_MAX]int;tgt:[NE_PROMPT_MAX]int;greedy:=NxOptions.{temperature=0};
    for p:problems {o:=rl_rollout(c,w,table,specials,p,greedy,cast(u64)r.count,work,tok[..],tgt[..]);if o.status!=RlStatus.ok {r.status=o.status;return r;}
        out[r.count]=cast(u8)rl_reward(rl_grade(table,specials[0],p,tok[0..<o.n],o.prompt));r.right+=cast(int)out[r.count];r.count+=1;}
    return r;
}
```

With 15 questions gained and 6 lost, the share gained is `15/21 = 0.714`,
and its 95% Wilson interval, worked as in chapter 138, is `0.5004` to
`0.8619`: its lower end is above one half, so the rule passes. With 14
gained and 6 lost the interval is `0.4810` to `0.8545`, and the rule fails.
Questions whose outcome did not change say nothing about the difference
and do not count.

## Run it

The driver holds the buffers, about 16 MiB, and prints. Every decision it
reports comes from `rollouts.coax` and the chapters it loads.

<!-- source: psionic/tests/rollouts-drive.coax drive -->
```jai
// Chapter 136's base run, rebuilt, and its tuning run stopped after step
// cut; the starting policy asked held-out questions greedily; greedy
// rollouts against chapter 136's sf_ask; the first batch drawn twice; the
// policy-gradient steps against the frozen start; the same questions asked
// again; the frozen rule; the reference checked; then the refusals.
rl_drive :: (base:NpPlan,tune:NpPlan,cut:int,rl:NpPlan,plan:RlPlan,trains:int,asks:int,report:int) {
    c:=tune.model;table:=NC_LEARNED;sp:[9]int;ne_specials(sp[..]);bos:=sp[0];L:=c.sequence_len;n:=nc_parameter_count(c);#assert n==RL_PARAMETERS;
    train:[RL_TRAIN_MAX]SfProblem;held:[RL_HELD_MAX]SfProblem;seen:[SF_KEYS]u8;
    #assert sf_problems(RL_SEED,false,train[0..<trains])==trains && sf_problems(RL_SEED,true,held[0..<asks])==asks;
    tr:=train[0..<trains];he:=held[0..<asks];a:=sf_audit(tr,he,seen[..]);
    print("problems: % training, % held out, shared: %, audit: %\n",a.train,a.held,a.shared,a.status);
    // Chapter 136's run, step for step, with tuning stopped after step cut.
    eq:[RL_TRAIN_MAX*12]int;val:[RL_HELD_MAX*12]int;d:=sf_stream(table[..],bos,tr,eq[..]);dv:=sf_stream(table[..],bos,he,val[..]);#assert d.status==SfStatus.ok && dv.status==SfStatus.ok;
    tok:[RL_TRAIN_MAX*40]int;tgt:[RL_TRAIN_MAX*40]int;start:[RL_TRAIN_MAX+1]int;e:=sf_encode(table[..],sp[..],c.vocab,L,tr,tok[..],tgt[..],start[..]);#assert e.status==SfStatus.ok;
    w:[RL_PARAMETERS]f64;m:[RL_PARAMETERS]f64;v:[RL_PARAMETERS]f64;g:[RL_PARAMETERS]f64;h:[RL_PARAMETERS]f64;total:[RL_PARAMETERS]f64;
    tape:[110000]f64;logits:[21056]f64;cache:[8192]f64;work:[16384]f64;order:[RL_TRAIN_MAX+1]int;seq:[64]int;target:[64]int;
    last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;ww:=ne_buffers(c,L,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);
    k:=PfWork.{cache=cache[..],work=work[..],logits=logits[..],tape=tape[..],g=g[..],h=h[..],seq=seq[..],target=target[..]};
    np_init(c,base.init,w[..]);order[d.tokens/L]=-1;run:=NpRun.{};
    while run.step<base.steps {r:=np_step(base,run,eq[0..<d.tokens],order[0..<d.tokens/L+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;}
    bp:=np_plan_id(base);bd:=np_data_id(eq[0..<d.tokens],val[0..<dv.tokens]);rec:[1279092]u8;#assert np_save(bp,bd,run,w[..],m[..],v[..],rec[..])==NpCheck.ok;
    tp:=sf_plan_id(tune,ck_get(rec[..],rec.count-32));td:=np_data_id(tok[0..<e.tokens],tgt[0..<e.tokens]);
    s:=sf_start(bp,bd,base.steps,rec[..],w[..],m[..],v[..]);#assert s.status==NpCheck.ok;
    for i:0..<e.count+1 {order[i]=0;}order[e.count]=-1;run=s.run;
    while run.step<cut {r:=sf_step(tune,run,tok[0..<e.tokens],tgt[0..<e.tokens],start[0..<e.count+1],order[0..<e.count+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;}
    tuned:[1279092]u8;#assert np_save(tp,td,run,w[..],m[..],v[..],tuned[..])==NpCheck.ok;
    print("start: chapter 136's run, % base steps and % of % tuning steps, record digest ",base.steps,cut,tune.steps);rl_hex(ck_get(tuned[..],tuned.count-32));print("...\n");
    // The reference is the start, frozen; the policy starts there with fresh moments.
    ref:=w;for i:0..<n {m[i]=0;v[i]=0;}
    was:[RL_HELD_MAX]u8;now:[RL_HELD_MAX]u8;before:=rl_rate(c,w[..],table[..],sp[..],he,ww,was[0..<asks]);#assert before.status==RlStatus.ok;
    rl_print("start, held-out questions answered greedily with reward 1",before);
    // Greedy rollouts are chapter 136's exchanges, and the checker is its verdict.
    x:[NE_PROMPT_MAX]int;y:[NE_PROMPT_MAX]int;z:[NE_PROMPT_MAX]int;agree:=0;greedy:=NxOptions.{temperature=0};
    for p:he {o:=rl_rollout(c,w[..],table[..],sp[..],p,greedy,0,ww,x[..],y[..]);q:=sf_ask(c,w[..],table[..],sp[..],p,ww,z[..]);
        gr:=rl_grade(table[..],bos,p,x[0..<o.n],o.prompt);same:=o.status==RlStatus.ok && q.status==SfStatus.ok && gr.called==q.called && gr.answered==q.answered && rl_same_batch(x[..],z[..],q.written);
        if same {agree+=1;}}
    print("greedy rollouts against chapter 136's sf_ask: % of % agree in tokens and verdict\n",agree,asks);
    o:=rl_rollout(c,w[..],table[..],sp[..],he[0],NxOptions.{},plan.seed,ww,x[..],y[..]);#assert o.status==RlStatus.ok;
    rl_show("a sampled rollout of the first held-out question, tokens",x[0..<o.n]);rl_show("its targets",y[0..<o.n]);
    // The batches. The first is drawn twice and kept for the refusals.
    count:=plan.prompts*plan.group;#assert count<=RL_BATCH_MAX;
    bt:[RL_BATCH_MAX*64]int;bg:[RL_BATCH_MAX*64]int;bs:[RL_BATCH_MAX+1]int;rw:[RL_BATCH_MAX]f64;lp:[RL_BATCH_MAX]f64;lr:[RL_BATCH_MAX]f64;ad:[RL_BATCH_MAX]f64;
    ft:[RL_BATCH_MAX*64]int;fg:[RL_BATCH_MAX*64]int;fs:[RL_BATCH_MAX+1]int;fw:[RL_BATCH_MAX]f64;fp:[RL_BATCH_MAX]f64;fr:[RL_BATCH_MAX]f64;fa:[RL_BATCH_MAX]f64;
    first:=rl_collect(plan,0,c,w[..],ref[..],table[..],sp[..],tr,ww,k,ft[..],fg[..],fs[..],fw[..],fp[..],fr[..],fa[..]);#assert first.status==RlStatus.ok;
    run=NpRun.{};rollouts:=0;positions:=0;sampled:=0;rewarded:=0;
    while run.step<rl.steps {b:=rl_collect(plan,run.step,c,w[..],ref[..],table[..],sp[..],tr,ww,k,bt[..],bg[..],bs[..],rw[..],lp[..],lr[..],ad[..]);#assert b.status==RlStatus.ok;
        if run.step==0 {print("the first batch drawn twice from its seeds: identical tokens: %, identical log-probabilities: %\n",rl_same_batch(bt[..],ft[..],b.tokens) && b.tokens==first.tokens,np_same(lp[0..<count],fp[0..<count]));}
        r:=rl_step(rl,run,b,bt[..],bg[..],bs[..],lp[..],ad[..],w[..],m[..],v[..],total[..],k);#assert r.status==RlStatus.ok;
        rollouts+=b.count;positions+=b.positions;sampled+=b.kept;rewarded+=b.rewarded;run=r.run;
        if run.step%report==0 {print("step %: mean reward %, mean log-ratio to the reference %, loss %, gradient norm %\n",run.step,b.reward,b.kl,r.loss,r.norm);}}
    print("budget: % rollouts, % graded, % with reward 1, % positions read while sampling, % sampled tokens, % gradient passes, % reference passes\n",rollouts,rollouts,rewarded,positions,sampled,rollouts,rollouts);
    after:=rl_rate(c,w[..],table[..],sp[..],he,ww,now[0..<asks]);#assert after.status==RlStatus.ok;
    rl_print("after, held-out questions answered greedily with reward 1",after);
    ch:=rl_changes(was[0..<asks],now[0..<asks]);ci:=pf_wilson(ch.right,ch.count);
    print("changed outcomes: % gained, % lost; share gained % to % (95%% interval)\n",ch.right,ch.count-ch.right,ci.lo,ci.hi);
    print("acceptance (frozen before the run): %\n",ifx rl_accept(before,after,was[0..<asks],now[0..<asks]) then "passed" else "failed");
    // The reference is the start's record, bit for bit.
    zw:=w;zm:=m;zv:=v;rd:=np_read(tp,td,cut,tuned[..],zw[..],zm[..],zv[..]);#assert rd.status==NpCheck.ok;
    print("reference after training: identical to the start's record: %\n",np_same(zw[..],ref[..]));
    // Refusals, each before anything moves.
    keep:=w;
    print("the first batch offered at step 1: %\n",rl_step(rl,NpRun.{step=1},first,ft[..],fg[..],fs[..],fp[..],fa[..],w[..],m[..],v[..],total[..],k).status);
    forged:=first;forged.version=run.step-1;
    print("the first batch relabelled as step %: %\n",run.step-1,rl_step(rl,NpRun.{step=run.step-1},forged,ft[..],fg[..],fs[..],fp[..],fa[..],w[..],m[..],v[..],total[..],k).status);
    print("weights unchanged by the refusals: %\n",np_same(keep[..],w[..]));
    // A rollout whose recorded calculator output was changed.
    p0:=SfProblem.{a=2,b=3};#assert sf_example(table[..],sp[..],c.vocab,L,p0,x[..],y[..]).n==25;
    print("the 2+3 exchange graded: reward %\n",rl_reward(rl_grade(table[..],bos,p0,x[0..<25],15)));
    x[21]=54;print("the same exchange with the calculator's output changed to 6: %\n",rl_grade(table[..],bos,p0,x[0..<25],15).status);
}
```

[The short run](../examples/rollouts.coax) rebuilds a briefly tuned model
and takes three steps:

<!-- source: psionic/examples/rollouts.coax example -->
```jai
// The short run: chapter 136's model rebuilt from 6 base steps and 10 of its
// tuning steps, then 3 policy-gradient steps of 2 prompts with 4 rollouts
// each, and 4 held-out questions asked before and after.
RL_BASE_DEMO :: NpPlan.{model=SF_MODEL,batch=2,steps=6,warmup=2,warmdown=2,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e11};
RL_TUNE_DEMO :: NpPlan.{model=SF_MODEL,batch=4,steps=10,warmup=0,warmdown=3,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e11};
RL_RUN_DEMO :: NpPlan.{model=SF_MODEL,batch=1,steps=3,warmup=0,warmdown=1,final=0,
    embed=0.01,matrix=0.003,head=0.003,scalar=0.003,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=19,init=0,budget=1e11};
RL_PLAN_DEMO :: RlPlan.{prompts=2,group=4,beta=0.02,seed=139};
main :: () {rl_drive(RL_BASE_DEMO,RL_TUNE_DEMO,10,RL_RUN_DEMO,RL_PLAN_DEMO,64,4,1);}
```

```shell
coax/bin/coax run psionic/examples/rollouts.coax
psionic/check.sh --only nanochat/rollouts
```

```text
problems: 64 training, 4 held out, shared: 0, audit: ok
start: chapter 136's run, 6 base steps and 10 of 10 tuning steps, record digest 723058b2318e1c94...
start, held-out questions answered greedily with reward 1: 0 of 4 right, 95% interval 0.0 to 0.48990002040399916
greedy rollouts against chapter 136's sf_ask: 4 of 4 agree in tokens and verdict
a sampled rollout of the first held-out question, tokens: 320 321 87 104 300 32 105 115 32 49 54 42 53 53 63 322 323 325 42 51 52 57 49 54 48 6 52 326
its targets: - - - - - - - - - - - - - - - - 325 42 51 52 57 49 54 48 6 52 326 -
the first batch drawn twice from its seeds: identical tokens: true, identical log-probabilities: true
step 1: mean reward 0.0, mean log-ratio to the reference 0.0, loss 0.0, gradient norm 0.0
step 2: mean reward 0.0, mean log-ratio to the reference 0.0, loss 0.0, gradient norm 0.0
step 3: mean reward 0.0, mean log-ratio to the reference 0.0, loss 0.0, gradient norm 0.0
budget: 24 rollouts, 24 graded, 0 with reward 1, 660 positions read while sampling, 288 sampled tokens, 24 gradient passes, 24 reference passes
after, held-out questions answered greedily with reward 1: 0 of 4 right, 95% interval 0.0 to 0.48990002040399916
changed outcomes: 0 gained, 0 lost; share gained 0.0 to 1.0 (95% interval)
acceptance (frozen before the run): failed
reference after training: identical to the start's record: true
the first batch offered at step 1: stale
the first batch relabelled as step 2: ok
weights unchanged by the refusals: true
the 2+3 exchange graded: reward 1.0
the same exchange with the calculator's output changed to 6: tool
```

The [transcript](../fixtures/rollouts.out) is exact for the named build in
debug and release. The short run is too short to mean anything about
quality. Its model, tuned for 10 steps, answers no held-out question
and earns no reward in any of its 24 rollouts. The policy starts equal to
the reference, so every log-ratio is 0, every advantage is 0, the gradient
is 0 and the weights never move. That is why the first batch, relabelled
as step 2, is accepted: its recorded log-probabilities are still exactly
what the current weights give, so it is still on-policy. The same batch
offered at step 1 is refused by its version. The run still shows every
piece working within the example time limit: greedy rollouts agree with
`sf_ask`, the first batch replays exactly from its seeds, the reference is
unchanged, and a changed calculator output is refused as `tool`.

## The recorded run

The recorded run rebuilds chapter 136's recorded base run, 600
steps, and its tuning run up to step 250 of 1,200. It then takes 150
policy-gradient steps, each on 8 training prompts with 8 rollouts per prompt,
and asks chapter 136's 256 held-out questions greedily before and after. It
runs in release only, in a few minutes on one core, and
[check_rollouts.sh](../tests/check_rollouts.sh) compares its transcript with
the [recorded one](../fixtures/rollouts/long.out).

The learning rate, the cut and `β` were chosen in development runs on the
same task before this recording; the acceptance rule was written before
any of them.

<!-- source: psionic/tests/rollouts-long.coax long -->
```jai
// Chapter 136's recorded plans (psionic/tests/sft-long.coax), unchanged.
SF_BASE_RUN :: NpPlan.{model=SF_MODEL,batch=8,steps=600,warmup=20,warmdown=120,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e12};
SF_TUNE_RUN :: NpPlan.{model=SF_MODEL,batch=16,steps=1200,warmup=0,warmdown=300,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e13};
RL_RUN :: NpPlan.{model=SF_MODEL,batch=1,steps=150,warmup=0,warmdown=40,final=0,
    embed=0.01,matrix=0.003,head=0.003,scalar=0.003,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=19,init=0,budget=1e13};
RL_PLAN :: RlPlan.{prompts=8,group=8,beta=0.02,scale=false,seed=139};
main :: () {rl_drive(SF_BASE_RUN,SF_TUNE_RUN,250,RL_RUN,RL_PLAN,4096,256,25);}
```

```shell
psionic/tests/check_rollouts.sh
```

```text
problems: 4096 training, 256 held out, shared: 0, audit: ok
start: chapter 136's run, 600 base steps and 250 of 1200 tuning steps, record digest aee203d11c6e6180...
start, held-out questions answered greedily with reward 1: 90 of 256 right, 95% interval 0.29566093701007257 to 0.4118531792823149
greedy rollouts against chapter 136's sf_ask: 256 of 256 agree in tokens and verdict
a sampled rollout of the first held-out question, tokens: 320 321 87 104 300 32 105 115 32 49 54 42 53 53 63 322 323 325 49 54 42 53 53 326 327 56 56 48 328 56 54 53 324
its targets: - - - - - - - - - - - - - - - - 325 49 54 42 53 53 326 - - - - - 56 54 53 324 -
the first batch drawn twice from its seeds: identical tokens: true, identical log-probabilities: true
step 25: mean reward 0.296875, mean log-ratio to the reference 0.7329176892347451, loss -0.03194355231140722, gradient norm 0.12837514840252118
step 50: mean reward 0.484375, mean log-ratio to the reference 0.5735990873880318, loss -0.02357372238443379, gradient norm 0.1890936454070852
step 75: mean reward 0.390625, mean log-ratio to the reference 1.1360229894552332, loss -0.017980284028969926, gradient norm 0.08555963071594658
step 100: mean reward 0.609375, mean log-ratio to the reference 2.189248355796322, loss -0.008984507406609887, gradient norm 0.32247618302646947
step 125: mean reward 0.578125, mean log-ratio to the reference 1.2400435439490123, loss -0.013302756518308444, gradient norm 0.0754518540284159
step 150: mean reward 0.75, mean log-ratio to the reference 1.6417507887822094, loss -0.029684066584589865, gradient norm 0.12846130774441092
budget: 9600 rollouts, 9600 graded, 4660 with reward 1, 505987 positions read while sampling, 99402 sampled tokens, 9600 gradient passes, 9600 reference passes
after, held-out questions answered greedily with reward 1: 170 of 256 right, 95% interval 0.6041561810850576 to 0.7191176903812512
changed outcomes: 81 gained, 1 lost; share gained 0.9341050148733268 to 0.9978440634290349 (95% interval)
acceptance (frozen before the run): passed
reference after training: identical to the start's record: true
the first batch offered at step 1: stale
the first batch relabelled as step 149: stale
weights unchanged by the refusals: true
the 2+3 exchange graded: reward 1.0
the same exchange with the calculator's output changed to 6: tool
```

The rule passes. At step 250 of chapter 136's tuning, the model
answers 90 of 256 held-out questions exactly, 95% interval 0.296 to 0.412.
After 150 steps of learning from its own rollouts it answers 170 of 256,
interval 0.604 to 0.719. Of the questions whose outcome changed, 81 were
gained and 1 was lost: the share gained has a 95% interval of 0.934 to
0.998, well above one half. The mean reward of the training rollouts rose
from about 0.3 to 0.75 over the run, and the mean log-ratio to the
reference stayed between 0.5 and 2.2 nats per rollout.

All 256 greedy rollouts at the start agree with chapter 136's `sf_ask` in
tokens and verdict. The first batch drawn twice from its seeds is identical.
After training, the reference reread from the start's record is
bit-identical. The first batch is refused at step 1 by its version, and
again when relabelled as step 149, because its recorded log-probabilities
are no longer what the weights give. The budget line records the run's
cost: 9,600 rollouts, each graded once and read once by the policy and once
by the reference, 505,987 positions read while sampling, and 99,402 sampled
tokens.

This is a measurement of one run of one small model on one generated task.
Chapter 136's own tuning reaches 256 of 256 by step 1,200; rollouts are not
shown here to be better than more supervised steps, only to improve the
model from its own samples and a checker.

## Checks

- **Advantages by hand:** the group above, scaled and unscaled, a group all
  alike, `β = 0`, groups of one and of eight, and every refusal before
  anything is written.
- **The checker against the tokens:** the `2+3` exchange right, with a wrong
  answer, cut off, ending at the call or the output, a wrong call, a direct
  answer right and wrong, an unclosed call, text before the call, a second
  call, two-token and negative outputs, and refusals of a changed or missing
  output.
- **Rollouts against chapter 136:** greedy rollouts from programmed models
  give `sf_ask`'s tokens and verdict, with the drawn stop token kept; the
  `1+2` exchange's tokens and targets by hand; replies that run to the
  window, calls the calculator cannot run, and the window's edges.
- **Seeds:** sampled rollouts replay exactly from their seeds; each reply
  equals chapter 126's engine run on its own stream; a programmed fork falls
  both ways across the seeds.
- **A batch from its definitions:** every rollout, reward, log-probability,
  reference value, advantage and budget of a batch equals the same thing
  computed alone, at two versions.
- **The step's gradient** equals the hand combination of chapter 127's
  gradients and agrees with central differences of the forward surrogate in
  four tensors; the zero-head loss is `−0.5 ln 12 / 11`; zero advantages
  give a zero gradient; clipping and the schedule behave as in chapter 128.
- **Refusals:** another version's batch, a log-probability off by `2^-29`
  either way or not a number, a finished run, short buffers, a broken model
  and an optimizer refusal, each before the weights move; `2^-30` either way
  passes.
- **Acceptance:** the rule just met and just missed, and every refusal.

Mutation of `rollouts.coax` leaves no survivors.

## What remains

- Rollouts drawn from [chapter 242](ch242-environments-with-checkers.md)'s
  environments. Chapter 242's environments hold integer tasks for a
  simulated agent; this chapter's environment is chapter 136's calculator
  task with its own checker.
- Held-out comparisons against [chapter 240](ch240-how-far-a-metric-moves-by-itself.md)'s
  noise floor, measured by rerunning the same recipe from other seeds. The
  comparison here is a paired interval on one run.
- nanochat's own task, GSM8K, which needs a model large enough to do
  arithmetic in words.

No general GRPO, PPO or RLHF support is claimed. These numbers measure one
small model on one generated task.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
- [Chapter 136](ch136-supervised-conversations-and-tools.md): the model,
  the task, the split and `sf_ask`.
- [Chapter 138](ch138-preferences-and-reward-models.md): `pf_value`,
  `pf_grad`, the frozen reference and the Wilson interval.
- [Chapter 126](ch126-execute-nanochat-exactly.md): the engine.
- [Chapter 53](ch053-choosing-the-next-token.md): seeded sampling.
- [Chapter 127](ch127-differentiate-nanochat.md): the masked gradient.
