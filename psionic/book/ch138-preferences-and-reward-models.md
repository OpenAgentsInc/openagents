# 138. Preferences and reward models

> **Status: partly implemented.** Preference pairs with known ground truth
> on chapter 136's calculator task, a reward model trained with the
> Bradley-Terry loss, DPO against a frozen reference, held-out pairwise
> accuracy with 95% intervals, a measured limitation on a kind of pair the
> reward model never saw, and acceptance rules frozen before the run. Still
> to come: HelpSteer3's human preference pairs with recorded provenance and
> fixed splits, and the limitations that only human data shows, such as a
> score that rises with answer length.

Supervised fine-tuning shows a model one right answer. Preference training
shows it two answers and says which one is better. A reward model learns to
score answers so that the better one scores higher; DPO (direct preference
optimization) skips the separate score and moves the model itself towards
the better answer of each pair.

This chapter builds both in Coax on a task where the right answer is known:
chapter 136's arithmetic questions answered through a calculator tool. Each
pair has a chosen reply that is right by construction and a rejected reply
that is wrong by construction, so every accuracy reported here is measured
against ground truth, not against a judge. Every decision is made by one
`#authority` file, [prefer.coax](../lib/nanochat/prefer.coax), whose checks
leave no mutant alive.

## The profile

This chapter's profile is `nanochat-prefer-cpu-f64-v1`:

- Binary64 on the CPU, one thread, every sum in a fixed order. The short
  run's transcript is exact for the named build in debug and release; the
  recorded run's is exact in release.
- The model is chapter 136's: chapter 128's two blocks of width 32 over 329
  tokens, 53,290 parameters. Both runs rebuild chapter 136's base run and
  tuning step for step, and the reward model, the DPO policy and the DPO
  reference all start from the tuned weights.
- Gradients are chapter 127's masked gradients; the schedule, clipping and
  AdamW are chapter 128's.

## Pairs with known ground truth

A pair is two replies to one question. The chosen reply is chapter 136's
conversation: the exact call, the calculator's output and that number. The
rejected reply is one of three kinds:

| Kind | Rejected reply for `2+3` | What is wrong |
| --- | --- | --- |
| 0 | `<|python_start|>2+3<|python_end|>` output `5`, then `6` | the answer ignores the tool's output |
| 1 | `6` | no tool, and a wrong number |
| 2 | `<|python_start|>2+4<|python_end|>` output `6`, then `6` | the call has the wrong operand |

The rejected answer is the right value plus one; a kind 2 call adds one to
the second operand. Training uses kinds 0 and 1 in turn. Kind 2 is held
out entirely: it measures what the reward model does on a mistake it was
never shown.

The ground truth is one function. A reply is right when the user asked the
problem's question and the assistant answered in exactly the form chapter
136 trains:

<!-- source: psionic/lib/nanochat/prefer.coax truth -->
```jai
// The ground truth for a reply to problem p: the user asked p's question,
// and the assistant answered in exactly the form chapter 136 trains, the
// exact call, the calculator's output and that number.
pf_correct :: (p:SfProblem,messages:[]NcMessage,parts:[]NcPart)->bool {
    if messages.count!=2 || messages[0].role!=NcRole.user || messages[1].role!=NcRole.assistant || !messages[1].list || messages[1].parts!=3 {return false;}
    q:[24]u8;e:[12]u8;v:[12]u8;at:=messages[1].first;want:=sf_text(v[..],sf_itoa(sf_value(p),v[..]));
    return messages[0].text==sf_text(q[..],sf_question(p,q[..])) && parts[at].kind==NcPartKind.python && parts[at].text==sf_text(e[..],sf_expr(p,e[..]))
        && parts[at+1].kind==NcPartKind.output && parts[at+1].text==want && parts[at+2].kind==NcPartKind.text && parts[at+2].text==want;
}
```

Admission renders both replies with chapter 136's `sf_admit`, so a rejected
reply still has to be well formed: a tool output that is not what the
calculator returns is refused there. It then refuses a pair whose replies
are identical, whose prompts differ, or whose chosen reply is not right.
The prompt is every token up to and including the `<|assistant_start|>`
before the first supervised target. A right reply has one rendering, so a
rejected reply that is right is caught as the same reply.

<!-- source: psionic/lib/nanochat/prefer.coax admit -->
```jai
// One pair into tokens and targets, the chosen reply first, or a refusal that
// writes nothing. Each reply passes chapter 136's admission and leaves room
// for the verdict token; both share one prompt, the tokens up to and including
// the <|assistant_start|> before the first supervised target; they differ;
// and the ground truth says the chosen reply is right, which makes the
// rejected one wrong.
pf_admit :: (table:[]u8,specials:[]int,vocab:int,context:int,p:SfProblem,cm:[]NcMessage,cp:[]NcPart,rm:[]NcMessage,rp:[]NcPart,tok:[]int,tgt:[]int)->PfPair {
    ct:[NC_MAX_TOKENS]int;cg:[NC_MAX_TOKENS]int;rt:[NC_MAX_TOKENS]int;rg:[NC_MAX_TOKENS]int;
    a:=sf_admit(table,specials,vocab,context,cm,cp,ct[..],cg[..]);if a.status!=SfStatus.ok {return PfPair.{status=PfStatus.reply,reply=a.status};}
    b:=sf_admit(table,specials,vocab,context,rm,rp,rt[..],rg[..]);if b.status!=SfStatus.ok {return PfPair.{status=PfStatus.reply,reply=b.status};}
    if a.n>=context || b.n>=context {return PfPair.{status=PfStatus.window};}
    if tok.count<a.n+b.n || tgt.count<a.n+b.n {return PfPair.{status=PfStatus.shape};}
    // The chosen prompt holds <|assistant_start|> only at its end, so a
    // rejected prompt of another length differs from it within its length.
    f:=pf_prompt(cg[0..<a.n]);if !pf_equal(ct[0..<f],rt[0..<f]) {return PfPair.{status=PfStatus.prompt};}
    if pf_equal(ct[0..<a.n],rt[0..<b.n]) {return PfPair.{status=PfStatus.same};}
    // The right reply has one rendering, so a rejected reply that is right
    // was refused above as the same; only the chosen one needs grading.
    if !pf_correct(p,cm,cp) {return PfPair.{status=PfStatus.label};}
    for i:0..<a.n {tok[i]=ct[i];tgt[i]=cg[i];}for i:0..<b.n {tok[a.n+i]=rt[i];tgt[a.n+i]=rg[i];}
    return PfPair.{prompt=f,chosen=a.n,rejected=b.n};
}
```

<!-- source: psionic/lib/nanochat/prefer.coax pairs -->
```jai
// Problem p's pair of the given kind. The chosen reply is chapter 136's
// conversation. The rejected one answers the next number after the right
// output (kind 0), answers the next number with no tool (kind 1), or calls
// the calculator with the second operand one larger and answers its output
// (kind 2).
pf_example :: (table:[]u8,specials:[]int,vocab:int,context:int,p:SfProblem,kind:int,tok:[]int,tgt:[]int)->PfPair {
    if kind<0 || kind>2 {return PfPair.{status=PfStatus.kind};}
    q:[24]u8;e:[12]u8;v:[12]u8;x:[12]u8;y:[12]u8;z:[12]u8;o:=p;o.b+=1;
    expr:=sf_text(e[..],sf_expr(p,e[..]));want:=sf_text(v[..],sf_itoa(sf_value(p),v[..]));wrong:=sf_text(x[..],sf_itoa(sf_value(p)+1,x[..]));
    u:=NcMessage.{role=NcRole.user,text=sf_text(q[..],sf_question(p,q[..]))};
    cp:=[3]NcPart.[NcPart.{kind=NcPartKind.python,text=expr},NcPart.{kind=NcPartKind.output,text=want},NcPart.{text=want}];
    cm:=[2]NcMessage.[u,NcMessage.{role=NcRole.assistant,list=true,first=0,parts=3}];rp:=cp;rm:=cm;
    if kind==0 {rp[2].text=wrong;}
    if kind==1 {rm[1]=NcMessage.{role=NcRole.assistant,text=wrong};}
    if kind==2 {other:=sf_text(z[..],sf_itoa(sf_value(o),z[..]));rp[0].text=sf_text(y[..],sf_expr(o,y[..]));rp[1].text=other;rp[2].text=other;}
    return pf_admit(table,specials,vocab,context,p,cm[..],cp[..],rm[..],rp[..],tok,tgt);
}
// Every problem's pair, back to back, kinds taken in turn: pair j's chosen
// reply holds tok[start[2j]..<start[2j+1]] and its rejected reply
// tok[start[2j+1]..<start[2j+2]], with targets at the same positions of tgt.
pf_encode :: (table:[]u8,specials:[]int,vocab:int,context:int,problems:[]SfProblem,kinds:[]int,tok:[]int,tgt:[]int,start:[]int)->PfSet {
    r:=PfSet.{};if kinds.count==0 || start.count<2*problems.count+1 {r.status=PfStatus.data;return r;}
    start[0]=0;
    for j:0..<problems.count {at:=r.tokens;if at+2*context>tok.count || at+2*context>tgt.count {r.status=PfStatus.data;return r;}
        a:=pf_example(table,specials,vocab,context,problems[j],kinds[j%kinds.count],tok[at..<tok.count],tgt[at..<tgt.count]);
        if a.status!=PfStatus.ok {return PfSet.{status=a.status,reply=a.reply};}
        start[2*j+1]=at+a.chosen;r.tokens=at+a.chosen+a.rejected;start[2*j+2]=r.tokens;r.count+=1;}
    return r;
}
```

Pairs use chapter 136's split by problem: training pairs come from training
problems, held-out pairs from held-out problems, and chapter 136's audit
checks that no held-out problem was trained on.

## The tiny pair, by hand

Take `2+3` with a kind 0 rejection. Chapter 136 worked out the chosen
reply's 25 tokens and targets. The rejected reply is the same sequence with
one token changed: position 23, the answer, is `6` (54) instead of `5` (53).

```text
position   0 ...  13  14  15 16 17 18  19  20 21  22 23  24
chosen   320 ... 322 323 325 50 43 51 326 327 53 328 53 324
rejected 320 ... 322 323 325 50 43 51 326 327 53 328 54 324
target     - ...   - 325  50 43 51 326   -   -  - 53/54 324  -
```

The first supervised target sits at position 14, so the prompt is positions
0 to 14, 15 tokens, and both replies share it. The pair differs in one
target, at position 22. A kind 1 rejection is 17 tokens: the prompt, then
`6` and `<|assistant_end|>`.

## The reward model and the Bradley-Terry loss

A reward model gives each reply a number `r`. The Bradley-Terry model says
the chosen reply wins with probability `σ(r_c − r_r)`, where `σ` is the
logistic function, and the loss is its negative log:

```text
L = −log σ(r_c − r_r) = softplus(−(r_c − r_r)),   softplus(x) = log(1 + e^x)
dL/dr_c = −σ(−(r_c − r_r)),   dL/dr_r = +σ(−(r_c − r_r))
```

With `r_c = 1` and `r_r = 0`: `L = log(1 + e^−1) = 0.31326`, and
`dL/dr_c = −σ(−1) = −0.26894`. When the two rewards are equal, `L = ln 2 =
0.69315` whatever they are.

The reward here is read from the model through one token: the
log-probability of `Y` (token 89, `PF_VERDICT`) right after the reply's
`<|assistant_end|>`. Chapter 127's masked pass with a single kept target,
`Y` at the last position, returns exactly `−r` as its loss and `−∇r` as its
gradient, so the reward model needs no new backward pass. A separate linear
head on the last hidden state is the more common design; it would need a
backward pass seeded at that state, which chapter 127's pass does not take.
The checks confirm the reading: with a zero output head every logit is 0,
every reward is `−ln 12` on a 12-token stand-in, and the loss is `ln 2`.

## DPO

DPO trains a policy `π` against a frozen reference `π_ref`, here both
chapter 136's tuned model at the start. A reply's value is its summed
log-probability over its supervised targets, the same targets chapter 136
trained, so the prompt and the tool's output count for nothing. With `s`
for the policy's values and `s_ref` for the reference's:

```text
z = β·((s_c − s_ref,c) − (s_r − s_ref,r)),   L = softplus(−z)
dL/ds_c = −β·σ(−z),   dL/ds_r = +β·σ(−z)
```

With `β = 0.1`, a chosen reply that gained 1 nat over the reference
(`s_c = −2`, `s_ref,c = −3`) and a rejected one that lost 1 nat (`s_r = −4`,
`s_ref,r = −3`): `z = 0.1 × (1 − (−1)) = 0.2`, `L = log(1 + e^−0.2) =
0.59814`, and `dL/ds_c = −0.1 × σ(−0.2) = −0.045017`. At the first step the
policy is the reference, `z = 0` and `L = ln 2`.

A reply's summed log-probability is minus its kept targets times chapter
127's masked mean loss, so its gradient is minus the kept count times the
masked gradient. The reference's values are computed once, before training,
by chapter 125's forward pass; the reference weights are never handed to a
step.

<!-- source: psionic/lib/nanochat/prefer.coax value -->
```jai
// What a loss reads from one reply. For the reward model: the
// log-probability of the verdict token after the reply's last token. For
// DPO: the reply's summed log-probability over its supervised targets. Both
// come from chapter 125's forward pass; tgt has one target per token, the
// last always -1.
pf_value :: (c:NcConfig,w:[]f64,mode:PfMode,verdict:int,tok:[]int,tgt:[]int,k:PfWork)->PfValue {
    s:=pf_sequence(c,mode,verdict,tok,tgt,k);if s.status!=PfStatus.ok {return s;}
    n:=cast(int)s.value;seq:=k.seq[0..<n];none:[1]f64;
    f:=nc_forward(c,w,seq,k.cache,k.work,k.logits,none[0..<0],false);if f.status!=NcStatus.ok {return PfValue.{status=PfStatus.model};}
    // The softcap keeps every logit within 15, so exp needs no shift.
    total:=0.0;V:=c.vocab;
    for i:0..<n-1 {y:=k.target[i];if y>=0 {sum:=0.0;for x:0..<V {sum+=exp(k.logits[i*V+x]);}total+=k.logits[i*V+y]-log(sum);}}
    return PfValue.{value=total};
}
// The same value through chapter 127's masked pass, and its gradient in grad:
// minus the targets kept times the gradient of their mean loss.
pf_grad :: (c:NcConfig,w:[]f64,mode:PfMode,verdict:int,tok:[]int,tgt:[]int,grad:[]f64,k:PfWork)->PfValue {
    s:=pf_sequence(c,mode,verdict,tok,tgt,k);if s.status!=PfStatus.ok {return s;}
    n:=cast(int)s.value;t:=k.target[0..<n-1];
    r:=nb_masked(c,w,k.seq[0..<n],t,grad,k.tape,k.logits,false);if r.status!=NcStatus.ok {return PfValue.{status=PfStatus.model};}
    kept:=f64(sf_kept(t));for i:0..<grad.count {grad[i]=-kept*grad[i];}
    return PfValue.{value=-kept*r.loss};
}
```

<!-- source: psionic/lib/nanochat/prefer.coax margin -->
```jai
// Pair q's margin. Reward model: the chosen reward minus the rejected one.
// DPO: beta times the chosen reply's gain over the reference minus the
// rejected reply's, with the reference's values in refs[2q] and refs[2q+1].
// The loss is softplus(-margin), Bradley-Terry's -log sigma(margin).
pf_margin :: (c:NcConfig,w:[]f64,mode:PfMode,beta:f64,verdict:int,tok:[]int,tgt:[]int,start:[]int,refs:[]f64,q:int,k:PfWork)->f64 {
    a:=start[2*q];b:=start[2*q+1];e:=start[2*q+2];
    x:=pf_value(c,w,mode,verdict,tok[a..<b],tgt[a..<b],k);y:=pf_value(c,w,mode,verdict,tok[b..<e],tgt[b..<e],k);
    if x.status!=PfStatus.ok || y.status!=PfStatus.ok {return 0.0/0.0;}
    return pf_combine(mode,beta,x.value,y.value,refs,q);
}
pf_combine :: (mode:PfMode,beta:f64,chosen:f64,rejected:f64,refs:[]f64,q:int)->f64 {
    if mode==PfMode.reward {return chosen-rejected;}
    return beta*((chosen-refs[2*q])-(rejected-refs[2*q+1]));
}
```

A step averages the pair losses over a batch drawn in chapter 128's seeded
order, then clips, schedules and applies AdamW as chapter 128 does:

<!-- source: psionic/lib/nanochat/prefer.coax step -->
```jai
// One step over batch pairs drawn in chapter 128's seeded per-epoch order.
// With m the pair's margin, d(softplus(-m))/dm = -sigma(-m), so the pair adds
// -sigma(-m) f (grad chosen - grad rejected), f being 1 for the reward model
// and beta for DPO, averaged over the batch; then clipping, the schedule and
// AdamW as in chapter 128. The reference never enters: DPO reads its values
// from refs, computed once before training.
pf_step :: (p:NpPlan,run:NpRun,mode:PfMode,beta:f64,verdict:int,tok:[]int,tgt:[]int,start:[]int,refs:[]f64,order:[]int,w:[]f64,m:[]f64,v:[]f64,total:[]f64,k:PfWork)->NpStep {
    c:=p.model;pairs:=(start.count-1)/2;n:=w.count;fail:=NpStep.{run=run};
    if run.step>=p.steps || !(beta>0) {fail.status=NpStatus.config;return fail;}
    if pairs<p.batch || start.count!=2*pairs+1 || tgt.count!=tok.count || (mode==PfMode.dpo && refs.count<2*pairs) {fail.status=NpStatus.data;return fail;}
    f:=1.0;if mode==PfMode.dpo {f=beta;}
    for i:0..<n {total[i]=0;}loss:=0.0;B:=f64(p.batch);
    for j:0..<p.batch {q:=np_window(p,pairs,run.step*p.batch+j,order);a:=start[2*q];b:=start[2*q+1];e:=start[2*q+2];
        x:=pf_grad(c,w,mode,verdict,tok[a..<b],tgt[a..<b],k.g,k);y:=pf_grad(c,w,mode,verdict,tok[b..<e],tgt[b..<e],k.h,k);
        if x.status!=PfStatus.ok || y.status!=PfStatus.ok {fail.status=NpStatus.data;return fail;}
        margin:=pf_combine(mode,beta,x.value,y.value,refs,q);s:=nc_sigmoid(-margin)*f/B;
        for i:0..<n {total[i]-=s*(k.g[i]-k.h[i]);}loss+=pf_softplus(-margin)/B;
    }
    sum:=0.0;for x:total {sum+=x*x;}norm:=sqrt(sum);
    if p.clip>0 && norm>p.clip {for i:0..<n {total[i]=total[i]*p.clip/norm;}}
    scale:=np_scale(p,run.step);groups:[5]AwGroup;np_groups(p,scale,groups[..]);none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,w,total,m,v,none[0..<0],false);
    if a.status!=AwStatus.ok {fail.status=NpStatus.optimizer;return fail;}
    return NpStep.{run=NpRun.{step=run.step+1,state=a.state},loss=loss,norm=norm,scale=scale};
}
```

## Held-out accuracy and its interval

A held-out pair counts as right when its margin is above zero: the reward
model scores the chosen reply higher, or DPO's `z` is positive. A tie
counts against. The uncertainty is the 95% Wilson score interval for a
proportion. For 8 right of 10, with `z = 1.96` and `z² = 3.8416`:

```text
p = 0.8,  d = 1 + z²/n = 1.38416
centre = (p + z²/(2n)) / d = 0.99208 / 1.38416 = 0.71674
half   = z·sqrt(p(1−p)/n + z²/(4n²)) / d = 1.96 × 0.16001 / 1.38416 = 0.22658
interval: 0.4902 to 0.9433
```

<!-- source: psionic/lib/nanochat/prefer.coax math -->
```jai
// log(1 + e^x) without overflow, and the 95% Wilson score interval for
// right successes in n trials.
pf_softplus :: (x:f64)->f64 {return (x+abs(x))/2+log(1+exp(-abs(x)));}
pf_wilson :: (right:int,n:int)->PfInterval {
    if n<1 {return PfInterval.{lo=0,hi=1};}
    z:=1.96;k:=f64(n);p:=f64(right)/k;d:=1+z*z/k;
    centre:=(p+z*z/(2*k))/d;half:=z*sqrt(p*(1-p)/k+z*z/(4*k*k))/d;
    return PfInterval.{lo=centre-half,hi=centre+half};
}
```

<!-- source: psionic/lib/nanochat/prefer.coax score -->
```jai
// Held-out pairwise accuracy: a pair counts when its margin is above 0, so a
// tie counts against; mean is the mean margin.
pf_score :: (c:NcConfig,w:[]f64,mode:PfMode,beta:f64,verdict:int,tok:[]int,tgt:[]int,start:[]int,refs:[]f64,k:PfWork)->PfScore {
    pairs:=(start.count-1)/2;r:=PfScore.{};
    if pairs<1 || start.count!=2*pairs+1 || tgt.count!=tok.count || (mode==PfMode.dpo && refs.count<2*pairs) {r.status=PfStatus.data;return r;}
    for q:0..<pairs {x:=pf_margin(c,w,mode,beta,verdict,tok,tgt,start,refs,q,k);if !(x==x) {r.status=PfStatus.model;return r;}
        r.count+=1;r.mean+=x;if x>0 {r.right+=1;}}
    r.mean/=f64(r.count);return r;
}
```

## The acceptance rules

The rules were written before the first run. They hold the reward model and
DPO to the same accuracy bound, and DPO must not cost chapter 136's model
its skill with the tool.

<!-- source: psionic/lib/nanochat/prefer.coax accept -->
```jai
// The acceptance rules, frozen before the first run. The reward model passes
// when the lower end of the 95% Wilson interval of its held-out pairwise
// accuracy, on the kinds it was trained on, is at least 0.75. DPO passes
// when the same holds for its implicit reward on the same pairs, and, on the
// same held-out questions asked greedily with the tool, its exact calls and
// its exact answers each fall by at most 1 in 10 from the starting model's.
PF_RM_LOW :: 0.75;
PF_DPO_LOW :: 0.75;
PF_DROP_TENTHS :: 1;
pf_accept_reward :: (s:PfScore)->bool {return pf_clears(s,PF_RM_LOW);}
pf_accept_dpo :: (s:PfScore,before:SfRates,after:SfRates)->bool {
    if before.status!=SfStatus.ok || after.status!=SfStatus.ok || after.asked!=before.asked {return false;}
    n:=before.asked;
    return pf_clears(s,PF_DPO_LOW) && 10*(before.called-after.called)<=PF_DROP_TENTHS*n && 10*(before.answered-after.answered)<=PF_DROP_TENTHS*n;
}
// The score was measured, and the lower end of its interval reaches low.
pf_clears :: (s:PfScore,low:f64)->bool {return s.status==PfStatus.ok && pf_wilson(s.right,s.count).lo>=low;}
```

## Run it

The driver holds the buffers, about 16 MiB, and prints. Every decision it
reports comes from `prefer.coax` and chapter 136's `sft.coax`.

<!-- source: psionic/tests/prefer-drive.coax drive -->
```jai
// Chapter 136's base run and tuning, rebuilt; the pairs; the reward model
// trained from the tuned weights and scored on held-out pairs of the kinds it
// saw and of a kind it did not; DPO from the same weights against them,
// frozen; the starting and tuned policies asked the same held-out questions;
// the frozen rules; then the refusals.
pf_drive :: (base:NpPlan,tune:NpPlan,rm:NpPlan,dpo:NpPlan,trains:int,helds:int,pairs:int,asks:int,report:int) {
    c:=tune.model;table:=NC_LEARNED;sp:[9]int;ne_specials(sp[..]);bos:=sp[0];L:=c.sequence_len;n:=nc_parameter_count(c);#assert n==PF_PARAMETERS;
    train:[PF_TRAIN_MAX]SfProblem;held:[PF_HELD_MAX]SfProblem;seen:[SF_KEYS]u8;
    #assert sf_problems(PF_SEED,false,train[0..<trains])==trains && sf_problems(PF_SEED,true,held[0..<helds])==helds;
    tr:=train[0..<trains];he:=held[0..<helds];a:=sf_audit(tr,he,seen[..]);
    print("problems: % training, % held out, shared: %, audit: %\n",a.train,a.held,a.shared,a.status);
    // Chapter 136's run, step for step: equations, then conversations.
    eq:[PF_TRAIN_MAX*12]int;val:[PF_HELD_MAX*12]int;d:=sf_stream(table[..],bos,tr,eq[..]);dv:=sf_stream(table[..],bos,he,val[..]);#assert d.status==SfStatus.ok && dv.status==SfStatus.ok;
    tok:[PF_TRAIN_MAX*40]int;tgt:[PF_TRAIN_MAX*40]int;start:[2*PF_TRAIN_MAX+1]int;e:=sf_encode(table[..],sp[..],c.vocab,L,tr,tok[..],tgt[..],start[..]);#assert e.status==SfStatus.ok;
    w:[PF_PARAMETERS]f64;m:[PF_PARAMETERS]f64;v:[PF_PARAMETERS]f64;g:[PF_PARAMETERS]f64;h:[PF_PARAMETERS]f64;total:[PF_PARAMETERS]f64;
    tape:[110000]f64;logits:[21056]f64;cache:[8192]f64;work:[16384]f64;order:[2*PF_TRAIN_MAX+1]int;seq:[64]int;target:[64]int;
    last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;ww:=ne_buffers(c,L,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);
    k:=PfWork.{cache=cache[..],work=work[..],logits=logits[..],tape=tape[..],g=g[..],h=h[..],seq=seq[..],target=target[..]};
    np_init(c,base.init,w[..]);order[d.tokens/L]=-1;run:=NpRun.{};
    while run.step<base.steps {r:=np_step(base,run,eq[0..<d.tokens],order[0..<d.tokens/L+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;}
    bp:=np_plan_id(base);bd:=np_data_id(eq[0..<d.tokens],val[0..<dv.tokens]);rec:[1279092]u8;#assert np_save(bp,bd,run,w[..],m[..],v[..],rec[..])==NpCheck.ok;
    tp:=sf_plan_id(tune,ck_get(rec[..],rec.count-32));td:=np_data_id(tok[0..<e.tokens],tgt[0..<e.tokens]);
    s:=sf_start(bp,bd,base.steps,rec[..],w[..],m[..],v[..]);#assert s.status==NpCheck.ok;
    for i:0..<e.count+1 {order[i]=0;}order[e.count]=-1;run=s.run;
    while run.step<tune.steps {r:=sf_step(tune,run,tok[0..<e.tokens],tgt[0..<e.tokens],start[0..<e.count+1],order[0..<e.count+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;}
    tuned:[1279092]u8;#assert np_save(tp,td,run,w[..],m[..],v[..],tuned[..])==NpCheck.ok;
    print("chapter 136's tuned model: % base steps, % tuning steps, record digest ",base.steps,tune.steps);pf_hex(ck_get(tuned[..],tuned.count-32));print("...\n");
    before:=sf_rates(c,w[..],table[..],sp[..],he[0..<asks],ww);#assert before.status==SfStatus.ok;
    print("tuned model, % held-out questions: % exact calls, % exact answers\n",before.asked,before.called,before.answered);
    // The pairs: kinds 0 and 1 in turn for training and for the main held-out
    // set; kind 2, never trained on, held out apart.
    x:[128]int;y:[128]int;one:=pf_example(table[..],sp[..],c.vocab,L,SfProblem.{a=2,b=3},0,x[..],y[..]);#assert one.status==PfStatus.ok;
    pf_show("2+3 rejected (kind 0) tokens",x[one.chosen..<one.chosen+one.rejected]);pf_show("2+3 rejected (kind 0) targets",y[one.chosen..<one.chosen+one.rejected]);
    ks:=[2]int.[0,1];k2:=[1]int.[2];
    pt:[PF_TRAIN_MAX*72]int;pg:[PF_TRAIN_MAX*72]int;ps:[2*PF_TRAIN_MAX+1]int;q:=pf_encode(table[..],sp[..],c.vocab,L,tr[0..<pairs],ks[..],pt[..],pg[..],ps[..]);#assert q.status==PfStatus.ok;
    ht:[PF_HELD_MAX*128]int;hg:[PF_HELD_MAX*128]int;hs:[2*PF_HELD_MAX+1]int;hq:=pf_encode(table[..],sp[..],c.vocab,L,he,ks[..],ht[..],hg[..],hs[..]);#assert hq.status==PfStatus.ok;
    ut:[PF_HELD_MAX*128]int;ug:[PF_HELD_MAX*128]int;us:[2*PF_HELD_MAX+1]int;uq:=pf_encode(table[..],sp[..],c.vocab,L,he,k2[..],ut[..],ug[..],us[..]);#assert uq.status==PfStatus.ok;
    T:=pt[0..<q.tokens];G:=pg[0..<q.tokens];S:=ps[0..<2*q.count+1];H:=ht[0..<hq.tokens];HG:=hg[0..<hq.tokens];HS:=hs[0..<2*hq.count+1];U:=ut[0..<uq.tokens];UG:=ug[0..<uq.tokens];US:=us[0..<2*uq.count+1];
    print("pairs: % training (% tokens), % held out of kinds 0 and 1, % held out of kind 2\n",q.count,q.tokens,hq.count,uq.count);
    none:[1]f64;
    // The reward model starts from the tuned weights with fresh moments.
    rw:=w;rmm:[PF_PARAMETERS]f64;rmv:[PF_PARAMETERS]f64;for i:0..<q.count+1 {order[i]=0;}order[q.count]=-1;run=NpRun.{};
    pf_print("reward model before training, held out kinds 0 and 1",pf_score(c,rw[..],PfMode.reward,PF_BETA,PF_VERDICT,H,HG,HS,none[0..<0],k));
    while run.step<rm.steps {r:=pf_step(rm,run,PfMode.reward,PF_BETA,PF_VERDICT,T,G,S,none[0..<0],order[0..<q.count+1],rw[..],rmm[..],rmv[..],total[..],k);#assert r.status==NpStatus.ok;run=r.run;
        if run.step%report==0 {print("reward step %: loss %, gradient norm %\n",run.step,r.loss,r.norm);}}
    rs:=pf_score(c,rw[..],PfMode.reward,PF_BETA,PF_VERDICT,H,HG,HS,none[0..<0],k);
    pf_print("reward model, held out kinds 0 and 1",rs);
    pf_print("reward model, held out kind 2 (not trained on)",pf_score(c,rw[..],PfMode.reward,PF_BETA,PF_VERDICT,U,UG,US,none[0..<0],k));
    print("reward model acceptance (frozen before the run): %\n",ifx pf_accept_reward(rs) then "passed" else "failed");
    // DPO: the policy starts from the tuned weights; the tuned weights
    // themselves are the reference, read once for every reply.
    rt:[2*PF_TRAIN_MAX]f64;rh:[2*PF_HELD_MAX]f64;ru:[2*PF_HELD_MAX]f64;
    #assert pf_refs(c,w[..],PF_VERDICT,T,G,S,rt[..],k)==PfStatus.ok && pf_refs(c,w[..],PF_VERDICT,H,HG,HS,rh[..],k)==PfStatus.ok && pf_refs(c,w[..],PF_VERDICT,U,UG,US,ru[..],k)==PfStatus.ok;
    pw:=w;for i:0..<n {m[i]=0;v[i]=0;}for i:0..<q.count+1 {order[i]=0;}order[q.count]=-1;run=NpRun.{};
    while run.step<dpo.steps {r:=pf_step(dpo,run,PfMode.dpo,PF_BETA,PF_VERDICT,T,G,S,rt[..],order[0..<q.count+1],pw[..],m[..],v[..],total[..],k);#assert r.status==NpStatus.ok;run=r.run;
        if run.step%report==0 {print("DPO step %: loss %, gradient norm %\n",run.step,r.loss,r.norm);}}
    ds:=pf_score(c,pw[..],PfMode.dpo,PF_BETA,PF_VERDICT,H,HG,HS,rh[..],k);
    pf_print("DPO implicit reward, held out kinds 0 and 1",ds);
    pf_print("DPO implicit reward, held out kind 2 (not trained on)",pf_score(c,pw[..],PfMode.dpo,PF_BETA,PF_VERDICT,U,UG,US,ru[..],k));
    after:=sf_rates(c,pw[..],table[..],sp[..],he[0..<asks],ww);#assert after.status==SfStatus.ok;
    print("DPO model, % held-out questions: % exact calls, % exact answers\n",after.asked,after.called,after.answered);
    print("DPO acceptance (frozen before the run): %\n",ifx pf_accept_dpo(ds,before,after) then "passed" else "failed");
    // The reference is the tuned record, bit for bit, and gives the same values again.
    z:=pw;zm:=m;zv:=v;rd:=np_read(tp,td,tune.steps,tuned[..],z[..],zm[..],zv[..]);#assert rd.status==NpCheck.ok;again:[2*PF_HELD_MAX]f64;#assert pf_refs(c,w[..],PF_VERDICT,H,HG,HS,again[..],k)==PfStatus.ok;
    same:=0;for j:0..<2*hq.count {if f64_bits(again[j])==f64_bits(rh[j]) {same+=1;}}
    print("reference after DPO: identical to the tuned record: %; % of % reference values identical\n",np_same(z[..],w[..]),same,2*hq.count);
    // Broken pairs, each refused before anything is written.
    u:=NcMessage.{role=NcRole.user,text="What is 2+3?"};as:=NcMessage.{role=NcRole.assistant,list=true,first=0,parts=3};
    good:=[3]NcPart.[NcPart.{kind=NcPartKind.python,text="2+3"},NcPart.{kind=NcPartKind.output,text="5"},NcPart.{text="5"}];bad:=good;bad[2].text="6";
    names:=[5]string.["the same reply twice","a rejected reply to another question","the label swapped","a rejected reply with the wrong tool output","a chosen reply with no room for the verdict token"];
    for f:0..<5 {cp:=good;rp:=bad;cm:=[2]NcMessage.[u,as];rm2:=cm;ctx:=L;
        if f==0 {rp=good;}if f==1 {rm2[0].text="What is 2+4?";}if f==2 {cp=bad;rp=good;}if f==3 {rp[1].text="6";}if f==4 {ctx=25;}
        r:=pf_admit(table[..],sp[..],c.vocab,ctx,SfProblem.{a=2,b=3},cm[..],cp[..],rm2[..],rp[..],x[..],y[..]);
        print("%: %",names[f],r.status);if r.status==PfStatus.reply {print(" (%)",r.reply);}print("\n");}
}
```

[The short run](../examples/prefer.coax) rebuilds a briefly tuned model and
runs eight steps of each method:

<!-- source: psionic/examples/prefer.coax example -->
```jai
// The short run: chapter 136's model rebuilt from 6 base steps and 10
// tuning steps, then 6 reward-model steps and 6 DPO steps of 4 pairs each,
// scored on 12 held-out pairs of kinds 0 and 1 and 12 of kind 2, and 2
// held-out questions asked before and after DPO.
PF_BASE_DEMO :: NpPlan.{model=SF_MODEL,batch=2,steps=6,warmup=2,warmdown=2,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e11};
PF_TUNE_DEMO :: NpPlan.{model=SF_MODEL,batch=4,steps=10,warmup=0,warmdown=3,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e11};
PF_RM_DEMO :: NpPlan.{model=SF_MODEL,batch=4,steps=6,warmup=0,warmdown=2,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=13,init=0,budget=1e11};
PF_DPO_DEMO :: NpPlan.{model=SF_MODEL,batch=4,steps=6,warmup=0,warmdown=2,final=0,
    embed=0.003,matrix=0.001,head=0.001,scalar=0.001,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=17,init=0,budget=1e11};
main :: () {pf_drive(PF_BASE_DEMO,PF_TUNE_DEMO,PF_RM_DEMO,PF_DPO_DEMO,64,12,24,2,3);}
```

```shell
coax/bin/coax run psionic/examples/prefer.coax
psionic/check.sh --only nanochat/prefer
```

```text
problems: 64 training, 12 held out, shared: 0, audit: ok
chapter 136's tuned model: 6 base steps, 10 tuning steps, record digest e9821ce3a547cc66...
tuned model, 2 held-out questions: 0 exact calls, 0 exact answers
2+3 rejected (kind 0) tokens: 320 321 87 104 300 32 105 115 32 50 43 51 63 322 323 325 50 43 51 326 327 53 328 54 324
2+3 rejected (kind 0) targets: - - - - - - - - - - - - - - 325 50 43 51 326 - - - 54 324 -
pairs: 24 training (1400 tokens), 12 held out of kinds 0 and 1, 12 held out of kind 2
reward model before training, held out kinds 0 and 1: 3 of 12 pairs right, 95% interval 0.08894003962896138 to 0.53231033912066, mean margin -0.014422155585743853
reward step 3: loss 0.6011031486351626, gradient norm 0.42535707376780574
reward step 6: loss 0.18130376600492995, gradient norm 0.07260334036012564
reward model, held out kinds 0 and 1: 10 of 12 pairs right, 95% interval 0.5519636426153274 to 0.9530358523851776, mean margin 2.417072347301766
reward model, held out kind 2 (not trained on): 6 of 12 pairs right, 95% interval 0.2537781703934222 to 0.7462218296065778, mean margin 0.00011205161476938545
reward model acceptance (frozen before the run): failed
DPO step 3: loss 0.6927070326957935, gradient norm 0.2369501212113306
DPO step 6: loss 0.6783716391394925, gradient norm 0.3685643218699696
DPO implicit reward, held out kinds 0 and 1: 7 of 12 pairs right, 95% interval 0.3195073356553728 to 0.8067425380947535, mean margin 0.02292210883102731
DPO implicit reward, held out kind 2 (not trained on): 4 of 12 pairs right, 95% interval 0.13811783989736134 to 0.609382412602386, mean margin -0.005474638551966956
DPO model, 2 held-out questions: 0 exact calls, 0 exact answers
DPO acceptance (frozen before the run): failed
reference after DPO: identical to the tuned record: true; 24 of 24 reference values identical
the same reply twice: same
a rejected reply to another question: prompt
the label swapped: label
a rejected reply with the wrong tool output: reply (result)
a chosen reply with no room for the verdict token: window
```

The [transcript](../fixtures/prefer.out) is exact for the named build in
debug and release. The short run is too short to mean anything about
quality: its tuned model makes no exact call, and 12 held-out pairs give
intervals more than 0.3 wide. Both rules fail. It exists to show every piece
working within the example time limit.

Even so, it shows the shape of the result. Before training, the tuned
model's verdict-token reward prefers the chosen reply in 3 of 12 held-out
pairs, worse than a coin. After six reward steps it prefers it in 10 of 12
pairs of the kinds it trained on, with a mean margin of 2.4 nats, but in
only 6 of 12 pairs of kind 2, with a mean margin of 0.0001 nats: it has
learned nothing about a wrong call. DPO's loss starts at `ln 2 = 0.69315`,
as worked above, and its small learning rate moves it little in six steps.
After DPO, the reference weights reread from the tuned record are
bit-identical, and every reference value computed again matches the one
used in training. The last five lines are broken pairs, each refused before
anything is written.

## The recorded run

The recorded run rebuilds chapter 136's recorded run, 600 base steps and
1,200 tuning steps, and checks that the tuned model still answers all 256
held-out questions through the tool, as chapter 136's transcript says. It
then trains the reward model for 400 steps and DPO for 200 steps, each on
16 of the 4,096 training pairs per step, and scores both on 256 held-out
pairs of kinds 0 and 1 and 256 of kind 2. It runs in release only, in about
five minutes on one core, and [check_prefer.sh](../tests/check_prefer.sh)
compares its transcript with the [recorded one](../fixtures/prefer/long.out).

<!-- source: psionic/tests/prefer-long.coax long -->
```jai
// Chapter 136's recorded plans (psionic/tests/sft-long.coax), unchanged.
SF_BASE_RUN :: NpPlan.{model=SF_MODEL,batch=8,steps=600,warmup=20,warmdown=120,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e12};
SF_TUNE_RUN :: NpPlan.{model=SF_MODEL,batch=16,steps=1200,warmup=0,warmdown=300,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e13};
PF_RM_RUN :: NpPlan.{model=SF_MODEL,batch=16,steps=400,warmup=0,warmdown=100,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=13,init=0,budget=1e13};
PF_DPO_RUN :: NpPlan.{model=SF_MODEL,batch=16,steps=200,warmup=0,warmdown=50,final=0,
    embed=0.003,matrix=0.001,head=0.001,scalar=0.001,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=17,init=0,budget=1e13};
main :: () {pf_drive(SF_BASE_RUN,SF_TUNE_RUN,PF_RM_RUN,PF_DPO_RUN,4096,256,4096,256,100);}
```

```shell
psionic/tests/check_prefer.sh
```

```text
problems: 4096 training, 256 held out, shared: 0, audit: ok
chapter 136's tuned model: 600 base steps, 1200 tuning steps, record digest c9eead03f742d773...
tuned model, 256 held-out questions: 256 exact calls, 256 exact answers
2+3 rejected (kind 0) tokens: 320 321 87 104 300 32 105 115 32 50 43 51 63 322 323 325 50 43 51 326 327 53 328 54 324
2+3 rejected (kind 0) targets: - - - - - - - - - - - - - - 325 50 43 51 326 - - - 54 324 -
pairs: 4096 training (237230 tokens), 256 held out of kinds 0 and 1, 256 held out of kind 2
reward model before training, held out kinds 0 and 1: 110 of 256 pairs right, 95% interval 0.3705266391091252 to 0.4909274159767426, mean margin -0.48931780062807156
reward step 100: loss 0.3012867571223993, gradient norm 0.011011056523662624
reward step 200: loss 0.13474295811951076, gradient norm 0.18828734884886095
reward step 300: loss 0.21931018438748012, gradient norm 0.7429617953872659
reward step 400: loss 0.08546256487383179, gradient norm 0.5746960772010649
reward model, held out kinds 0 and 1: 252 of 256 pairs right, 95% interval 0.9605201201790405 to 0.9939075003405375, mean margin 12.915872188092182
reward model, held out kind 2 (not trained on): 152 of 256 pairs right, 95% interval 0.5326305715645441 to 0.6520973549876323, mean margin 2.045395387713313
reward model acceptance (frozen before the run): passed
DPO step 100: loss 0.21281227517614615, gradient norm 0.14043117963693266
DPO step 200: loss 0.22438003398350184, gradient norm 0.11225466789386762
DPO implicit reward, held out kinds 0 and 1: 256 of 256 pairs right, 95% interval 0.9852156082782743 to 1.0, mean margin 1.503696692254501
DPO implicit reward, held out kind 2 (not trained on): 244 of 256 pairs right, 95% interval 0.9198662271464744 to 0.9729854178557116, mean margin 0.6387934099060045
DPO model, 256 held-out questions: 256 exact calls, 253 exact answers
DPO acceptance (frozen before the run): passed
reference after DPO: identical to the tuned record: true; 512 of 512 reference values identical
the same reply twice: same
a rejected reply to another question: prompt
the label swapped: label
a rejected reply with the wrong tool output: reply (result)
a chosen reply with no room for the verdict token: window
```

Both rules pass. The reward model prefers the chosen reply in 252 of 256
held-out pairs of the kinds it trained on, 95% interval 0.961 to 0.994,
against 110 of 256 before training: the tuned model's verdict token starts
out slightly worse than a coin. DPO's implicit reward prefers the chosen
reply in all 256, interval 0.985 to 1.0.

**The measured limitation.** On kind 2, a wrong call whose output and
answer agree with each other, the reward model prefers the chosen reply in
only 152 of 256 pairs, interval 0.533 to 0.652. It learned to check that the
answer copies the tool's output and that a tool was called, the two
mistakes it was shown; it did not learn to check the call against the
question. Its score would rise for a confident, consistent, wrong reply.
DPO, trained on the same pairs, ranks kind 2 correctly in 244 of 256: it
moves the probability of every supervised token, including the call's, so
lowering the trained mistakes also lowered a call the question did not ask
for. These are measurements of this task and these runs, not a general
claim about either method.

DPO cost chapter 136's model little of its skill with the tool: on the same
256 held-out questions asked greedily, exact calls stay at 256 and exact
answers fall from 256 to 253, within the frozen bound of 1 in 10. After
DPO, the reference weights reread from the tuned record are bit-identical,
and all 512 held-out reference values computed again are identical to the
ones used for scoring.

## Checks

- **The losses by hand:** softplus, Bradley-Terry and DPO values and slopes
  as worked above, and the Wilson interval for 8 of 10, 15 of 20, 45 of 50,
  0 of 10 and 1 of 1.
- **The tiny pair by hand,** token by token for every kind, and chapter
  136's `sf_check` on both replies.
- **Two paths to one value:** the forward-pass log-probability of every
  stand-in reply equals minus its kept targets times chapter 127's masked
  loss, and the reward equals minus the masked loss of a sequence built by
  hand with `Y` appended.
- **The step's gradient** equals the hand combination of chapter 127's
  gradients, `σ(−m)` times their difference, for both losses, and agrees
  with central differences of the forward loss in four tensors, at the
  reference and away from it.
- **The frozen reference:** a DPO step leaves the reference values
  unchanged; both runs reread chapter 136's tuned record after DPO and find
  the reference weights and every held-out reference value bit-identical.
- **Admission:** identical replies, prompts of the same and of different
  lengths, a swapped label, two wrong replies, a problem that is not the
  prompt's, malformed replies, no room for the verdict token and short
  buffers, each refused before anything is written.
- **Scores and acceptance:** ties, mean margins, refusals, and each rule's
  thresholds just met and just missed.

Mutation of `prefer.coax` leaves no survivors.

## What remains

- HelpSteer3's pairwise human preferences, each pair traced to its dataset,
  split and licence, with fixed splits. The pairs here are generated in
  Coax and carry their provenance as chapter 136's seed and draw index.
- The limitations human data shows: a reward that rises with length or
  style while quality does not. The limitation measured here is a kind of
  mistake absent from training.
- A reward model with its own linear head on the last hidden state.

No broad RLHF parity is claimed. These numbers measure one small model on
one generated task.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
- [Chapter 136](ch136-supervised-conversations-and-tools.md): the tuned
  model, the task, the split and admission.
- [Chapter 127](ch127-differentiate-nanochat.md): the masked
  gradient.
- [Chapter 240](ch240-how-far-a-metric-moves-by-itself.md): noise floors
  for held-out comparisons.
