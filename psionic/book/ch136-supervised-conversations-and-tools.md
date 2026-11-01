# 136. Supervised conversations and tools

> **Status: partly implemented.** A base model pretrained with chapter 128's
> step is fine-tuned on rendered conversations that call a calculator tool,
> with the loss on assistant and tool-call tokens only, a split by problem
> and its audit, admission that refuses malformed conversations, exact
> resume and an acceptance rule frozen before the run. The recorded run
> answers all 256 held-out questions through the tool and fails the rule's
> bound on the base model's own skill. Still to come: a recipe that keeps
> that skill within the bound, the bounded Nemotron-Agentic-v1 and
> OpenCodeReasoning subsets with recorded provenance and licences, and
> nanochat's own SFT mixture.

A base model continues text. Supervised fine-tuning (SFT) teaches it to
answer in conversations and to call tools. nanochat runs SFT as a separate
stage after pretraining: it renders each conversation with the special
tokens of [chapter 123](ch123-learn-a-tokenizer.md), sets the target of every
token the assistant did not write to -1, and trains on the rest. This
chapter builds that stage in Coax on a task small enough to check by hand:
arithmetic questions answered through a calculator tool.

The stage needs four things beyond chapter 128's pretraining loop: a loss
that skips chosen positions, a mask that says which positions those are,
data that refuses what it cannot learn from, and an evaluation that runs
the tool the model calls. Every decision is made by one `#authority` file,
[sft.coax](../lib/nanochat/sft.coax), whose checks leave no mutant alive.

## The profile

This chapter's profile is `nanochat-sft-cpu-f64-v1`:

- Binary64 on the CPU, one thread, every sum in a fixed order. The short
  run's transcript is exact for the named build in debug and release; the
  recorded run's is exact in release.
- The model is chapter 128's: two blocks of width 32, two query heads over
  one key-value head, windows of 64 tokens. Its vocabulary is chapter 123's
  320 ordinary tokens plus nanochat's nine special tokens, 329 in all, so it
  has 53,290 parameters.
- The base model is pretrained with chapter 128's `np_step` on equations
  written as plain text. Tuning starts from its checkpoint record with fresh
  AdamW moments, as nanochat's `chat_sft` builds a new optimizer.
- The gradient is chapter 127's, extended here with chosen targets.

## The task and the split

A problem is `a op b` with `a` and `b` from 0 to 99 and `op` one of `+`, `-`
and `*`. Problem `j` is one draw `x` of chapter 103's counter generator:
`a = x mod 100`, `b = (x / 100) mod 100`, `op = (x / 10000) mod 3`. Each
problem becomes one conversation:

```text
user:       What is 37+48?
assistant:  <|python_start|>37+48<|python_end|>
            <|output_start|>85<|output_end|>
            85
```

The base model never sees a conversation. Its documents are the same
training problems written as equations, `37+48=85`, one per document after
`<|bos|>`. A model this small cannot add two-digit numbers; it can learn to
hand the question to the calculator and copy the result back.

The split is by problem, never by conversation. A problem's key is
`op·10000 + 100·first + second`, with the operands sorted for `+` and `*`,
so `37+48` and `48+37` are one problem. One key in eight, chosen by a draw
of the counter generator, is held out. The audit marks every training key
and refuses the split if any held-out key is marked. Held-out problems
appear in neither the base model's equations nor the tuning conversations.

<!-- source: psionic/lib/nanochat/sft.coax problems -->
```jai
// Problem j is one draw of chapter 103's counter generator.
sf_problem :: (seed:u64,j:int)->SfProblem {
    x:=cr_bits(seed,cast(u64)j);return SfProblem.{a=cast(int)(x%100),b=cast(int)((x/100)%100),op=cast(int)((x/10000)%3)};
}
sf_value :: (p:SfProblem)->int {if p.op==0 {return p.a+p.b;}if p.op==1 {return p.a-p.b;}return p.a*p.b;}
// A problem's identity: the same operation on the same operands, in either
// order for + and *.
sf_key :: (p:SfProblem)->int {
    lo:=p.a;hi:=p.b;if p.op!=1 {lo=np_min(p.a,p.b);hi=p.a+p.b-lo;}
    return p.op*10000+lo*100+hi;
}
// The split is by problem: one in eight keys, chosen by a draw, is held out.
sf_held :: (key:int)->bool {return cr_bits(SF_SPLIT,cast(u64)key)%8==0;}
// The first out.count problems on one side of the split, in draw order.
sf_problems :: (seed:u64,held:bool,out:[]SfProblem)->int {
    k:=0;j:=0;while k<out.count {p:=sf_problem(seed,j);if sf_held(sf_key(p))==held {out[k]=p;k+=1;}j+=1;}
    return k;
}
// Every held-out problem against every training problem.
sf_audit :: (train:[]SfProblem,held:[]SfProblem,seen:[]u8)->SfSplit {
    r:=SfSplit.{train=train.count,held=held.count};
    if seen.count<SF_KEYS || train.count==0 || held.count==0 {r.status=SfStatus.data;return r;}
    for i:0..<SF_KEYS {seen[i]=0;}for p:train {seen[sf_key(p)]=1;}
    for p:held {if seen[sf_key(p)]==1 {r.shared+=1;}}
    if r.shared>0 {r.status=SfStatus.leak;}
    return r;
}
```

The calculator is the tool. It runs exactly one form, one to four decimal
digits, an operator, one to four decimal digits, and refuses anything else:

<!-- source: psionic/lib/nanochat/sft.coax calculator -->
```jai
// The tool: "a op b" with a and b of one to four decimal digits and op one
// of + - *. Anything else is not a call it can run.
sf_digits :: (s:string,at:int)->int {e:=at;while e<s.count && s[e]>='0' && s[e]<='9' {e+=1;}return e-at;}
sf_number :: (s:string,at:int,n:int)->int {x:=0;for i:at..<at+n {x=x*10+cast(int)(s[i]-'0');}return x;}
sf_calc :: (s:string)->SfCalc {
    da:=sf_digits(s,0);if da<1 || da>4 {return SfCalc.{};}
    db:=sf_digits(s,da+1);if db<1 || db>4 || da+1+db!=s.count {return SfCalc.{};}
    a:=sf_number(s,0,da);b:=sf_number(s,da+1,db);op:=s[da];
    if op=='+' {return SfCalc.{ok=true,value=a+b};}
    if op=='-' {return SfCalc.{ok=true,value=a-b};}
    if op=='*' {return SfCalc.{ok=true,value=a*b};}
    return SfCalc.{};
}
// Decimal text of x, with a sign when negative.
sf_itoa :: (x:int,out:[]u8)->int {
    n:=0;v:=x;if x<0 {out[0]='-';n=1;v=-x;}
    digits:[20]u8;k:=0;while true {digits[k]='0'+cast(u8)(v%10);k+=1;v/=10;if v==0 {break;}}
    for i:0..<k {out[n+i]=digits[k-1-i];}return n+k;
}
```

## The mask, by hand

Chapter 123's `nc_render` already marks each token it writes: 1 for
assistant text, for a python call with its `<|python_start|>` and
`<|python_end|>`, and for `<|assistant_end|>`; 0 for everything else,
including `<|assistant_start|>` and the tool's output. The model at
position `i` predicts token `i+1`, so position `i`'s target is token `i+1`
when that token is marked, and -1 when it is not. The last position has no
target.

Work the smallest conversation, `2+3`, by hand. Bytes are their own token
IDs (`2` is 50, `+` is 43, `5` is 53), chapter 123's merge 300 is `at`, and
the specials start at 320: `<|bos|>` 320, `<|user_start|>` 321,
`<|user_end|>` 322, `<|assistant_start|>` 323, `<|assistant_end|>` 324,
`<|python_start|>` 325, `<|python_end|>` 326, `<|output_start|>` 327,
`<|output_end|>` 328.

```text
position  0   1   2   3   4   5  6   7   8   9 10 11 12  13  14  15 16 17 18  19  20 21  22 23  24
token   320 321  87 104 300  32 105 115 32 50 43 51 63 322 323 325 50 43 51 326 327 53 328 53 324
          bos u<   W   h  at   _   i   s  _  2  +  3  ?  u>  a<  p<  2  +  3  p>  o<  5  o>  5  a>
mask      0   0   0   0   0   0  0   0   0  0  0  0  0   0   0   1  1  1  1   1   0  0   0  1   1
target    -   -   -   -   -   -  -   -   -  -  -  -  -   - 325  50 43 51 326  -   -  -  53 324   -
```

Seven targets survive: the four tokens of the call, its closing
`<|python_end|>`, the answer `5` and `<|assistant_end|>`. The question, the
`<|assistant_start|>` that the prompt supplies at inference, and the tool's
output `5` cost nothing. A model rewarded for predicting the output would
learn to guess the calculator instead of reading it.

With a head of zeros every logit is `15·tanh(0) = 0`, so each of the seven
targets costs `ln 329 = 5.796` nats and the masked mean is `ln 329` whatever
the count. A check builds that model and requires a step's loss to be
`ln 329` within `1e-14`.

Admission turns a conversation into tokens and targets, or refuses it
before writing anything. It refuses what `nc_render` refuses, a
conversation that ends on the user's turn, one longer than the window
(nanochat truncates; this stage never trains on a cut conversation), a call
that is not followed by its output or an output that does not follow a
call, a call the calculator cannot run, and an output that is not exactly
what the calculator returns:

<!-- source: psionic/lib/nanochat/sft.coax admit -->
```jai
// One conversation into tokens and targets, or a refusal that writes nothing.
// Position i's target is token i+1 when the render supervises it, else -1;
// the last position has none.
sf_admit :: (table:[]u8,specials:[]int,vocab:int,context:int,messages:[]NcMessage,parts:[]NcPart,ids:[]int,targets:[]int)->SfAdmit {
    if context<2 || context>NC_MAX_TOKENS || ids.count<context || targets.count<context {return SfAdmit.{status=SfStatus.shape};}
    staged:[NC_MAX_TOKENS]int;mask:[NC_MAX_TOKENS]u8;
    r:=nc_render(table,specials,vocab,messages,parts,NC_MAX_TOKENS,staged[..],mask[..],false);
    if r.status!=NcChatStatus.ok {return SfAdmit.{status=SfStatus.render,chat=r.status};}
    if messages[messages.count-1].role!=NcRole.assistant {return SfAdmit.{status=SfStatus.shape};}
    if r.total>context {return SfAdmit.{status=SfStatus.window};}
    // Every call is followed by its output, every output follows a call, and
    // the output is exactly what the calculator returns for the call.
    for m:messages {if !m.list {continue;}
        for k:m.first..<m.first+m.parts {p:=parts[k];
            if p.kind==NcPartKind.output && (k==m.first || parts[k-1].kind!=NcPartKind.python) {return SfAdmit.{status=SfStatus.tool};}
            if p.kind!=NcPartKind.python {continue;}
            if k+1==m.first+m.parts || parts[k+1].kind!=NcPartKind.output {return SfAdmit.{status=SfStatus.tool};}
            c:=sf_calc(p.text);if !c.ok {return SfAdmit.{status=SfStatus.tool};}
            b:[24]u8;if parts[k+1].text!=sf_text(b[..],sf_itoa(c.value,b[..])) {return SfAdmit.{status=SfStatus.result};}
        }
    }
    // <|bos|> is never supervised, so a supervised token always has a position before it.
    n:=r.written;kept:=0;
    for i:0..<n {ids[i]=staged[i];targets[i]=-1;if mask[i]==1 {targets[i-1]=staged[i];kept+=1;}}
    return SfAdmit.{n=n,kept=kept};
}
```

A mask that covers the wrong tokens fails `sf_check`, which derives the
mask again from the token IDs alone, with no access to the messages, and
compares the targets with it. The checks run it on generated conversations
of every operation, on two-turn conversations and on targets broken by
hand: a target on the tool's output, a lost answer target, a target after
the last token, a target for `<|assistant_start|>`.

<!-- source: psionic/lib/nanochat/sft.coax check -->
```jai
// The mask again, from the tokens alone: inside an assistant turn every token
// is supervised except a tool output (<|output_start|> to <|output_end|>);
// <|assistant_start|> itself and everything outside are not. targets must
// name exactly the supervised next tokens.
sf_check :: (ids:[]int,targets:[]int,base:int)->bool {
    if targets.count!=ids.count {return false;}
    assistant:=false;output:=false;
    for i:0..<ids.count {x:=ids[i]-base;supervised:=false;
        if x==NC_ASSISTANT_START {assistant=true;}
        else if x==NC_ASSISTANT_END {supervised=true;assistant=false;}
        else if x==NC_OUTPUT_START {output=true;}
        else if x==NC_OUTPUT_END {output=false;}
        else {supervised=assistant && !output;}
        if i>0 && targets[i-1]!=(ifx supervised then ids[i] else -1) {return false;}
    }
    return targets[ids.count-1]==-1;
}
```

## A loss on chosen targets

Chapter 127's `nb_gradient` averaged the loss over every next token. This
chapter generalizes it without changing a bit of its result:
`nb_masked` takes a target per position, skips a position whose target is
-1 in both the forward sum and the head's backward pass, and divides by the
number of targets kept. `nb_gradient` is now `nb_masked` with targets
`tokens[1..]`, so the division is still by `n-1` and chapter 128's recorded
runs reproduce exactly. A sequence with no target to keep is refused.

The checks in `backward.coax` compare `nb_masked` with three independent
facts: its loss with the mean of `log Σ exp(z) − z_target` over the kept
positions of chapter 125's forward logits, its gradient with central
differences of that loss in four tensors, and its gradient for two kept
targets with the mean of the gradients for each one alone. A kept target
may be any token, not only the next one.

A step draws conversations in chapter 128's seeded per-epoch order and
weights each conversation's mean gradient by its share of the batch's
targets, so the step's loss is the mean over every supervised token in the
batch, as nanochat's `cross_entropy(..., ignore_index=-1)` takes it. The
rest is chapter 128's step: clipping, the schedule and AdamW.

<!-- source: psionic/lib/nanochat/sft.coax step -->
```jai
// One step: conversations drawn in chapter 128's seeded per-epoch order,
// chapter 127's masked gradient for each, weighted by its supervised targets
// over the batch's (the mean over every supervised token), then clipping,
// the schedule and AdamW, as in chapter 128.
sf_step :: (p:NpPlan,run:NpRun,tok:[]int,tgt:[]int,start:[]int,order:[]int,w:[]f64,m:[]f64,v:[]f64,g:[]f64,total:[]f64,tape:[]f64,logits:[]f64)->NpStep {
    c:=p.model;count:=start.count-1;n:=w.count;fail:=NpStep.{run=run};
    if run.step>=p.steps {fail.status=NpStatus.config;return fail;}
    if count<p.batch {fail.status=NpStatus.data;return fail;}
    picks:[NP_BATCH]int;all:=0;
    for j:0..<p.batch {k:=np_window(p,count,run.step*p.batch+j,order);picks[j]=k;all+=sf_kept(tgt[start[k]..<start[k+1]-1]);}
    for i:0..<n {total[i]=0;}loss:=0.0;
    for j:0..<p.batch {k:=picks[j];a:=start[k];b:=start[k+1];t:=tgt[a..<b-1];
        r:=nb_masked(c,w,tok[a..<b],t,g,tape,logits,false);
        if r.status!=NcStatus.ok {fail.status=NpStatus.data;return fail;}
        share:=f64(sf_kept(t))/f64(all);for i:0..<n {total[i]+=g[i]*share;}loss+=r.loss*share;
    }
    sum:=0.0;for x:total {sum+=x*x;}norm:=sqrt(sum);
    if p.clip>0 && norm>p.clip {for i:0..<n {total[i]=total[i]*p.clip/norm;}}
    scale:=np_scale(p,run.step);groups:[5]AwGroup;np_groups(p,scale,groups[..]);none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,w,total,m,v,none[0..<0],false);
    if a.status!=AwStatus.ok {fail.status=NpStatus.optimizer;return fail;}
    return NpStep.{run=NpRun.{step=run.step+1,state=a.state},loss=loss,norm=norm,scale=scale};
}
```

Tuning starts from the base checkpoint. The tuning plan's identity covers
the base record's digest, so a tuning checkpoint offered with another base
refuses, and so does the base record offered as a tuning checkpoint.

<!-- source: psionic/lib/nanochat/sft.coax start -->
```jai
// The tuning plan's identity covers the base record it starts from.
sf_plan_id :: (p:NpPlan,base:[32]u8)->[32]u8 {
    id:=np_plan_id(p);b:[64]u8;for i:0..<32 {b[i]=id[i];b[32+i]=base[i];}
    return ck_digest("psionic.sft-plan.v1",b[..]);
}
// Tuning starts from the base record's weights with fresh moments at step 0,
// as nanochat's chat_sft builds a new optimizer.
sf_start :: (plan:[32]u8,data:[32]u8,steps:int,bytes:[]u8,w:[]f64,m:[]f64,v:[]f64)->NpRead {
    r:=np_read(plan,data,steps,bytes,w,m,v);if r.status!=NpCheck.ok {return r;}
    for i:0..<m.count {m[i]=0;v[i]=0;}
    return NpRead.{};
}
```

## Asking with the tool

Evaluation asks each held-out question through chapter 126's engine,
greedy, and writes the whole exchange to a buffer. When a reply stops as
`tool`, the harness decodes the call between `<|python_start|>` and
`<|python_end|>`, runs the calculator, appends
`<|output_start|>`, the result and `<|output_end|>`, and lets the model
reply again. A question scores two things: whether the call was exactly the
question's expression, and whether the final reply was exactly the right
number followed by `<|assistant_end|>`. It also counts the positions the
model read, the cost of the answer.

<!-- source: psionic/lib/nanochat/sft.coax ask -->
```jai
// One held-out question through chapter 126's engine, greedy, with the
// whole exchange written to ids. A reply that stops as `tool` has its call
// (the text after its <|python_start|>) run by the calculator; the output
// goes back between <|output_start|> and <|output_end|> and the model
// replies again. called: the call was exactly the question's expression.
// answered: the final reply ended with <|assistant_end|> and was exactly the
// right number. positions: the positions the model read, for the cost.
sf_ask :: (c:NcConfig,w:[]f64,table:[]u8,specials:[]int,p:SfProblem,work:NeWork,ids:[]int)->SfAsk {
    if ids.count<c.context {return SfAsk.{status=SfStatus.shape};}
    q:[24]u8;e:[12]u8;v:[12]u8;nq:=sf_question(p,q[..]);expr:=sf_text(e[..],sf_expr(p,e[..]));want:=sf_text(v[..],sf_itoa(sf_value(p),v[..]));
    m:=[2]NcMessage.[NcMessage.{role=NcRole.user,text=sf_text(q[..],nq)},NcMessage.{role=NcRole.assistant}];none:[1]NcPart;
    pr:=ne_prompt(table,specials,c.vocab,m[..],none[..],ids,false);
    if pr.status!=NeStatus.ok {return SfAsk.{status=SfStatus.render};}
    n:=pr.written;if n>=c.context {return SfAsk.{status=SfStatus.window};}
    // Greedy decoding draws nothing, so any seed names the same reply.
    greedy:=NxOptions.{temperature=0};base:=specials[0];seed:=cast(u64)n;
    g:=ne_generate(c,w,base,ids[0..<n],greedy,seed,c.context-n,ids[n..<c.context],work,false);
    if g.status!=NeStatus.ok {return SfAsk.{status=SfStatus.model};}
    at:=n+g.written;r:=SfAsk.{first=g.stop,written=at,positions=g.positions};reply:=ids[n..<at];
    if g.stop==NeStop.assistant_end {r.answered=sf_says(table,reply,want);return r;}
    if g.stop!=NeStop.tool {return r;}
    s:=0;while reply[s]!=base+NC_PYTHON_START {s+=1;}
    // A call holding a special token does not decode: it is empty text.
    d:=bt_decode(table,reply[s+1..<g.written-1]);call:=cast(string)d.bytes[0..<d.written];r.called=call==expr;
    k:=sf_calc(call);if !k.ok {return r;}
    o:[24]u8;out:=bt_encode(table,sf_text(o[..],sf_itoa(k.value,o[..])));
    room:=c.context-at-out.written-2;if room<1 {return r;}
    ids[at]=base+NC_OUTPUT_START;for i:0..<out.written {ids[at+1+i]=out.ids[i];}at+=out.written+2;ids[at-1]=base+NC_OUTPUT_END;
    h:=ne_generate(c,w,base,ids[0..<at],greedy,seed,room,ids[at..<c.context],work,false);
    r.last=h.stop;r.written=at+h.written;r.positions+=h.positions;r.answered=h.stop==NeStop.assistant_end && sf_says(table,ids[at..<r.written],want);
    return r;
}
```

The checks drive `sf_ask` with programmed models whose answers are known in
advance. With every block matrix zero, each block passes its input through
and the logits depend only on the current token, so a list of pairs `(a, b)`
makes greedy decoding emit `b` after `a`. One program calls `1+2`, reads the
output `3` and answers it; asked `1*2`, the same program makes the wrong
call and still answers with the calculator's `3`, so the call and the answer
are scored apart. Others answer directly with no tool, call something the
calculator cannot run, put a special token inside the call, or run to the
end of the window.

## The acceptance rule

The rule was written before the first run. Both models answer the same
held-out questions.

<!-- source: psionic/lib/nanochat/sft.coax accept -->
```jai
// The acceptance rule, frozen before the first run, on the same held-out
// questions for both models, in tenths: the tuned model answers at least 6 in
// 10 exactly, at least 5 in 10 more than the base model does, and makes the
// exact calculator call for at least 6 in 10; and its held-out bits per byte
// on plain equations, the base model's own skill, rises by at most 0.5.
SF_ANSWER_TENTHS :: 6;
SF_GAIN_TENTHS :: 5;
SF_CALL_TENTHS :: 6;
SF_DRIFT_MAX :: 0.5;
sf_accept :: (base:SfRates,tuned:SfRates,before:f64,after:f64)->bool {
    if base.status!=SfStatus.ok || tuned.status!=SfStatus.ok || tuned.asked!=base.asked {return false;}
    n:=tuned.asked;
    return 10*tuned.answered>=SF_ANSWER_TENTHS*n && 10*(tuned.answered-base.answered)>=SF_GAIN_TENTHS*n && 10*tuned.called>=SF_CALL_TENTHS*n && after-before<=SF_DRIFT_MAX;
}
```

## Run it

The driver holds the buffers, about 10 MiB, and prints. Every decision it
reports comes from `sft.coax`.

<!-- source: psionic/tests/sft-drive.coax drive -->
```jai
// The split and its audit, the two streams, the base run and its record, the
// base model's answers, tuning from the record with a checkpoint after step
// cut, the tuned model's answers, the frozen rule, then a resume replayed to
// the end and the refusals.
sf_drive :: (base:NpPlan,tune:NpPlan,trains:int,helds:int,asks:int,report:int,cut:int) {
    c:=tune.model;table:=NC_LEARNED;sp:[9]int;ne_specials(sp[..]);bos:=sp[0];L:=c.sequence_len;
    train:[SF_TRAIN_MAX]SfProblem;held:[SF_HELD_MAX]SfProblem;seen:[SF_KEYS]u8;
    #assert sf_problems(SF_SEED,false,train[0..<trains])==trains && sf_problems(SF_SEED,true,held[0..<helds])==helds;
    tr:=train[0..<trains];he:=held[0..<helds];a:=sf_audit(tr,he,seen[..]);
    print("problems: % training, % held out (one key in eight), shared: %, audit: %\n",a.train,a.held,a.shared,a.status);
    // The tiny conversation, by hand in the chapter.
    ids:[64]int;t:[64]int;x:=sf_example(table[..],sp[..],c.vocab,L,SfProblem.{a=2,b=3},ids[..],t[..]);
    sf_show("2+3 tokens",ids[0..<x.n]);sf_show("2+3 targets",t[0..<x.n]);
    // Base documents: BOS and one equation each.
    eq:[SF_TRAIN_MAX*12]int;val:[SF_HELD_MAX*12]int;d:=sf_stream(table[..],bos,tr,eq[..]);dv:=sf_stream(table[..],bos,he,val[..]);#assert d.status==SfStatus.ok && dv.status==SfStatus.ok;
    windows:=dv.tokens/L;lengths:[329]int;#assert np_lengths(table[..],bos,lengths[..])==NpStatus.ok;
    print("equations: % training tokens, % windows of %; held out: % tokens, % windows\n",d.tokens,d.tokens/L,L,dv.tokens,windows);
    // Conversations, and their targets checked against the tokens alone.
    tok:[SF_TRAIN_MAX*40]int;tgt:[SF_TRAIN_MAX*40]int;start:[SF_TRAIN_MAX+1]int;e:=sf_encode(table[..],sp[..],c.vocab,L,tr,tok[..],tgt[..],start[..]);#assert e.status==SfStatus.ok;
    agree:=0;for j:0..<e.count {if sf_check(tok[start[j]..<start[j+1]],tgt[start[j]..<start[j+1]],bos) {agree+=1;}}
    print("conversations: %, % tokens, % supervised targets; targets agree with the mask from tokens alone: % of %\n",e.count,e.tokens,e.kept,agree,e.count);
    #assert np_admit(base,d.tokens/L)==NpStatus.ok && np_admit(tune,e.count)==NpStatus.ok;
    n:=nc_parameter_count(c);#assert n==SF_PARAMETERS;
    // The base run: chapter 128's step on equation windows.
    w:[SF_PARAMETERS]f64;m:[SF_PARAMETERS]f64;v:[SF_PARAMETERS]f64;g:[SF_PARAMETERS]f64;total:[SF_PARAMETERS]f64;
    tape:[110000]f64;logits:[21056]f64;cache:[8192]f64;work:[16384]f64;order:[SF_TRAIN_MAX+1]int;
    last:[329]f32;pr:[329]f64;st:[329]f64;or:[329]int;te:[329]int;ww:=ne_buffers(c,L,cache[..],work[..],logits[..],last[..],pr[..],st[..],or[..],te[..]);
    np_init(c,base.init,w[..]);order[d.tokens/L]=-1;run:=NpRun.{};
    while run.step<base.steps {r:=np_step(base,run,eq[0..<d.tokens],order[0..<d.tokens/L+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;
        if run.step%(base.steps/2)==0 {print("base step %: loss %\n",run.step,r.loss);}}
    bp:=np_plan_id(base);bd:=np_data_id(eq[0..<d.tokens],val[0..<dv.tokens]);rec:[1279092]u8;#assert np_size(n)==rec.count && np_save(bp,bd,run,w[..],m[..],v[..],rec[..])==NpCheck.ok;
    print("base record: % bytes, digest ",rec.count);sf_hex(ck_get(rec[..],rec.count-32));print("...\n");
    before:=np_eval(c,w[..],val[0..<dv.tokens],lengths[..],windows,cache[..],work[..],logits[..]);#assert before.status==NpStatus.ok;
    br:=sf_rates(c,w[..],table[..],sp[..],he[0..<asks],ww);#assert br.status==SfStatus.ok;
    print("base: held-out equations % bits per byte; % questions: % exact calls, % exact answers, % positions read\n",before.bpb,br.asked,br.called,br.answered,br.positions);
    // Tuning starts from the record, with fresh moments.
    tp:=sf_plan_id(tune,ck_get(rec[..],rec.count-32));td:=np_data_id(tok[0..<e.tokens],tgt[0..<e.tokens]);
    s:=sf_start(bp,bd,base.steps,rec[..],w[..],m[..],v[..]);#assert s.status==NpCheck.ok;
    for i:0..<e.count+1 {order[i]=0;}order[e.count]=-1;run=s.run;losses:[4000]f64;saved:[1279092]u8;
    while run.step<tune.steps {r:=sf_step(tune,run,tok[0..<e.tokens],tgt[0..<e.tokens],start[0..<e.count+1],order[0..<e.count+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);
        #assert r.status==NpStatus.ok;losses[run.step]=r.loss;run=r.run;
        if run.step%report==0 {print("tune step %: loss %, gradient norm %, rate scale %\n",run.step,r.loss,r.norm,r.scale);}
        if run.step==cut {#assert np_save(tp,td,run,w[..],m[..],v[..],saved[..])==NpCheck.ok;print("tune step %: checkpoint, digest ",cut);sf_hex(ck_get(saved[..],saved.count-32));print("...\n");}}
    after:=np_eval(c,w[..],val[0..<dv.tokens],lengths[..],windows,cache[..],work[..],logits[..]);#assert after.status==NpStatus.ok;
    tr2:=sf_rates(c,w[..],table[..],sp[..],he[0..<asks],ww);#assert tr2.status==SfStatus.ok;
    print("tuned: held-out equations % bits per byte; % questions: % exact calls, % exact answers, % positions read\n",after.bpb,tr2.asked,tr2.called,tr2.answered,tr2.positions);
    // The first held-out exchange as text.
    ex:[64]int;xa:=sf_ask(c,w[..],table[..],sp[..],he[0],ww,ex[..]);text:[1024]u8;tx:=ne_text(table[..],ex[0..<xa.written],text[..]);#assert xa.status==SfStatus.ok && tx.status==NeStatus.ok;
    print("first held-out exchange: %\n",cast(string)text[0..<tx.written]);
    print("acceptance (frozen before the run): %\n",ifx sf_accept(br,tr2,before.bpb,after.bpb) then "passed" else "failed");
    // A fresh set of buffers holds only the checkpoint.
    y:[SF_PARAMETERS]f64;ym:[SF_PARAMETERS]f64;yv:[SF_PARAMETERS]f64;o2:[SF_TRAIN_MAX+1]int;o2[e.count]=-1;
    rd:=np_resume(tune,tp,td,saved[..],y[..],ym[..],yv[..]);#assert rd.status==NpCheck.ok;again:=rd.run;same:=0;
    while again.step<tune.steps {r:=sf_step(tune,again,tok[0..<e.tokens],tgt[0..<e.tokens],start[0..<e.count+1],o2[0..<e.count+1],y[..],ym[..],yv[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;
        if f64_bits(r.loss)==f64_bits(losses[again.step]) {same+=1;}again=r.run;}
    print("resumed at step %: % of % losses identical; parameters and moments identical: %\n",rd.run.step,same,tune.steps-rd.run.step,np_same(y[..],w[..]) && np_same(ym[..],m[..]) && np_same(yv[..],v[..]));
    // Broken inputs, each refused before anything is written.
    print("the base record resumed as the tuning run: %\n",np_resume(tune,tp,td,rec[..],y[..],ym[..],yv[..]).status);
    print("the tuning checkpoint against another base: %\n",np_resume(tune,sf_plan_id(tune,bp),td,saved[..],y[..],ym[..],yv[..]).status);
    leaky:=he;leaky[0]=tr[0];print("a held-out question that repeats a training problem: %\n",sf_audit(tr,leaky,seen[..]).status);
    u:=NcMessage.{role=NcRole.user,text="What is 2+3?"};as:=NcMessage.{role=NcRole.assistant,list=true,first=0,parts=3};
    good:=[3]NcPart.[NcPart.{kind=NcPartKind.python,text="2+3"},NcPart.{kind=NcPartKind.output,text="5"},NcPart.{text="5"}];
    names:=[6]string.["a tool output with no call before it","a recorded output the calculator would not return","a call the calculator cannot run","a call with no output after it","a conversation that ends on the user's turn","two user turns in a row"];
    for f:0..<6 {p:=good;mm:=[2]NcMessage.[u,as];k:=2;
        if f==0 {p[0].kind=NcPartKind.text;}if f==1 {p[1].text="6";}if f==2 {p[0].text="2+";}if f==3 {mm[1].parts=1;}if f==4 {k=1;}if f==5 {mm[1]=u;}
        r:=sf_admit(table[..],sp[..],c.vocab,L,mm[0..<k],p[..],ids[..],t[..]);print("%: %\n",names[f],r.status);}
    long:=[2]NcMessage.[NcMessage.{role=NcRole.user,text="What is 2+3? Please write the call for the calculator first, wait for its output, and then reply with the number alone."},as];
    print("a conversation longer than the %-token window: %\n",L,sf_admit(table[..],sp[..],c.vocab,L,long[..],good[..],ids[..],t[..]).status);
}
```

[The short run](../examples/sft.coax) pretrains for 16 steps, tunes for 24
and asks 8 held-out questions:

<!-- source: psionic/examples/sft.coax example -->
```jai
// The short run: 16 base steps of 4 equation windows, then 24 tuning steps
// of 8 conversations from the base record, checkpointed after step 12, and
// 8 held-out questions asked of each model.
SF_BASE_DEMO :: NpPlan.{model=SF_MODEL,batch=4,steps=16,warmup=2,warmdown=4,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e11};
SF_TUNE_DEMO :: NpPlan.{model=SF_MODEL,batch=8,steps=24,warmup=0,warmdown=6,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e11};
main :: () {sf_drive(SF_BASE_DEMO,SF_TUNE_DEMO,256,64,8,6,12);}
```

```shell
coax/bin/coax run psionic/examples/sft.coax
psionic/check.sh --only nanochat/sft
```

```text
problems: 256 training, 64 held out (one key in eight), shared: 0, audit: ok
2+3 tokens: 320 321 87 104 300 32 105 115 32 50 43 51 63 322 323 325 50 43 51 326 327 53 328 53 324
2+3 targets: - - - - - - - - - - - - - - 325 50 43 51 326 - - - 53 324 -
equations: 2394 training tokens, 37 windows of 64; held out: 601 tokens, 9 windows
conversations: 256, 8116 tokens, 2650 supervised targets; targets agree with the mask from tokens alone: 256 of 256
base step 8: loss 3.3308819203681033
base step 16: loss 2.7833560952587995
base record: 1279092 bytes, digest 0fa71fbf0e27843f...
base: held-out equations 4.04010301064654 bits per byte; 8 questions: 0 exact calls, 0 exact answers, 504 positions read
tune step 6: loss 3.6748412267622856, gradient norm 1.0993966818246024, rate scale 1.0
tune step 12: loss 2.978963921750017, gradient norm 0.9517926710514433, rate scale 1.0
tune step 12: checkpoint, digest 4e3593bae3463fea...
tune step 18: loss 2.589846376385457, gradient norm 0.5425937531765223, rate scale 1.0
tune step 24: loss 2.4513006758845246, gradient norm 0.6890203245428601, rate scale 0.16666666666666666
tuned: held-out equations 4.604901682363932 bits per byte; 8 questions: 0 exact calls, 0 exact answers, 152 positions read
first held-out exchange: <|bos|><|user_start|>What is 16*55?<|user_end|><|assistant_start|><|python_start|>1
acceptance (frozen before the run): failed
resumed at step 12: 12 of 12 losses identical; parameters and moments identical: true
the base record resumed as the tuning run: plan
the tuning checkpoint against another base: plan
a held-out question that repeats a training problem: leak
a tool output with no call before it: tool
a recorded output the calculator would not return: result
a call the calculator cannot run: tool
a call with no output after it: tool
a conversation that ends on the user's turn: shape
two user turns in a row: render
a conversation longer than the 64-token window: window
```

The [transcript](../fixtures/sft.out) is exact for the named build in debug
and release. The short run is too short to learn the task: after 24 tuning
steps the loss on supervised tokens is still 2.45 nats, no held-out question
gets an exact call or answer, and the rule fails. It exists to show every
piece working within the example time limit. The first held-out exchange
shows what the model has picked up so far: it opens a python block and
writes a digit. Every one of the 256 training conversations has targets that
agree with the mask derived from its tokens alone. The checkpoint after step
12 resumes in fresh buffers and replays steps 13 to 24 with every loss,
parameter and moment identical. The last ten lines are deliberately
broken inputs, each refused before anything is written.

## The recorded run

The recorded run pretrains for 600 steps of eight equation windows, tunes
for 1,200 steps of 16 conversations, about 4.7 passes over the 4,096
training conversations, and asks all 256 held-out questions of both models.
It runs in release only, in about two minutes on one core, and
[check_sft.sh](../tests/check_sft.sh) compares its transcript with the
[recorded one](../fixtures/sft/long.out).

<!-- source: psionic/tests/sft-long.coax long -->
```jai
SF_BASE_RUN :: NpPlan.{model=SF_MODEL,batch=8,steps=600,warmup=20,warmdown=120,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1361,budget=1e12};
SF_TUNE_RUN :: NpPlan.{model=SF_MODEL,batch=16,steps=1200,warmup=0,warmdown=300,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=11,init=0,budget=1e13};
main :: () {sf_drive(SF_BASE_RUN,SF_TUNE_RUN,4096,256,256,200,1100);}
```

```shell
psionic/tests/check_sft.sh
```

```text
problems: 4096 training, 256 held out (one key in eight), shared: 0, audit: ok
2+3 tokens: 320 321 87 104 300 32 105 115 32 50 43 51 63 322 323 325 50 43 51 326 327 53 328 53 324
2+3 targets: - - - - - - - - - - - - - - 325 50 43 51 326 - - - 53 324 -
equations: 38507 training tokens, 601 windows of 64; held out: 2414 tokens, 37 windows
conversations: 4096, 130262 tokens, 42603 supervised targets; targets agree with the mask from tokens alone: 4096 of 4096
base step 300: loss 1.8273938321629386
base step 600: loss 1.715941086050738
base record: 1279092 bytes, digest ccf18d6cc7c4118b...
base: held-out equations 2.676034251495632 bits per byte; 256 questions: 0 exact calls, 0 exact answers, 5436 positions read
tune step 200: loss 0.39774023562687155, gradient norm 0.2948535326427079, rate scale 1.0
tune step 400: loss 0.009525585840467296, gradient norm 0.08247045846316757, rate scale 1.0
tune step 600: loss 0.007173103179035303, gradient norm 0.12458899302515761, rate scale 1.0
tune step 800: loss 0.00031492155862624575, gradient norm 0.002355723668432103, rate scale 1.0
tune step 1000: loss 0.0001145616325124049, gradient norm 0.00033332075386028416, rate scale 0.67
tune step 1100: checkpoint, digest 17a7b047d9192202...
tune step 1200: loss 0.00002175982000802627, gradient norm 0.0000844417885613393, rate scale 0.0033333333333333335
tuned: held-out equations 18.564109611527613 bits per byte; 256 questions: 256 exact calls, 256 exact answers, 13628 positions read
first held-out exchange: <|bos|><|user_start|>What is 16*55?<|user_end|><|assistant_start|><|python_start|>16*55<|python_end|><|output_start|>880<|output_end|>880
acceptance (frozen before the run): failed
resumed at step 1100: 100 of 100 losses identical; parameters and moments identical: true
the base record resumed as the tuning run: plan
the tuning checkpoint against another base: plan
a held-out question that repeats a training problem: leak
a tool output with no call before it: tool
a recorded output the calculator would not return: result
a call the calculator cannot run: tool
a call with no output after it: tool
a conversation that ends on the user's turn: shape
two user turns in a row: render
a conversation longer than the 64-token window: window
```

The tuned model makes the exact call and gives the exact answer for all 256
held-out questions, against none for the base model. The first exchange
shows the whole loop: the model writes `16*55` as a call, the harness runs
the calculator, and the model copies `880` back and ends its turn. No
held-out problem, in either order of its operands, was in the training data
of either stage.

The run fails its acceptance rule anyway, and the rule is right to fail it.
On held-out equations, the base model's own skill, bits per byte rise from
2.68 to 18.56, far past the bound of 0.5 stated before training. Tuning on
conversations alone taught the model that every document starts with
`<|user_start|>`, and it now gives almost no probability to the plain
equation text it was pretrained on. This is catastrophic forgetting, and the
drift bound exists to catch it. nanochat mixes other data into its SFT stage
for this reason. Keeping the base skill within the bound, for example by
mixing equation windows into each tuning batch, is a change to the recipe
that a future run must make and record, under the same frozen rule.

The checkpoint after step 1,100 resumes and replays the last 100 steps bit
for bit. These numbers measure this experiment: a 53,290-parameter model, a
single generated task and one tool. They say nothing about conversational
ability in general.

## Checks

- **The mask by hand,** as worked above, token by token, and `sf_check`'s
  independent derivation on generated, two-turn and broken targets.
- **The masked loss** in `backward.coax`: against chapter 125's forward
  logits, central differences, linearity in the kept targets, and bit
  equality with `nb_gradient` when every next token is kept. Mutation of
  `backward.coax` leaves no survivors.
- **The step by hand:** a zero head's loss is `ln 12` on a 12-token
  stand-in, and the step's gradient equals the two sequences' masked
  gradients weighted by their kept targets.
- **Resume:** a run cut after one of three steps and resumed from its
  checkpoint record matches the uninterrupted run bit for bit; both examples
  resume a whole model from its record.
- **Asking:** programmed models whose replies are known exercise every
  branch of `sf_ask`.
- **Admission and refusals** for every malformed shape above, the split, the
  streams, a step and the start from a base record.

Mutation of `sft.coax` leaves no survivors.

## What remains

- The bounded Nemotron-Agentic-v1 and OpenCodeReasoning subsets, each
  example traced to its dataset, subset and licence. The conversations here
  are generated in Coax and carry their provenance as a seed and a draw
  index.
- nanochat's own SFT mixture and its multi-turn chat evaluations. This
  chapter's evaluation is one task, exact answers through one tool.
- A tuning recipe that keeps the base model's skill within the frozen bound,
  such as mixing equation windows into each tuning batch, recorded under the
  same rule.
- Packing several conversations into one window; here each conversation is
  its own sequence.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
