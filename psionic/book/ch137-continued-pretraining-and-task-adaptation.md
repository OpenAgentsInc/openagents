# 137. Continued pretraining and task adaptation

> **Status: partly implemented.** Chapter 128's model continues training on
> a new domain, Coax source code, from its pinned final checkpoint. Each
> step mixes domain windows with rehearsal of the original text in a stated
> ratio, one audit covers both corpora, and a promotion rule frozen before
> the runs reports domain gain and general forgetting separately. Still to
> come: promotion through chapter 241's decision gate with a noise floor
> measured over seeds, and adaptation to tasks other than predicting the
> domain's text.

Continued pretraining trains a base model further on a chosen domain.
nanochat's speedrun has no such stage, so this chapter is a Psionic
extension, not a stage claimed from the pinned recipe. It needs its own
workload, data and promotion rules, and this chapter writes them down before
any step runs.

[Chapter 128](ch128-a-complete-pretraining-run.md) trained a small nanochat
GPT on the prose of this book. Here the same model continues on the Coax
source that the book's first 40 chapters describe. Prose and code share a
vocabulary of words but little else: code has its own punctuation, layout
and names. That makes the trade visible. Training on code improves the
model's predictions of code it has not seen; it can also make the model
worse at the prose it already knew. The first is the **gain**, the second
is **forgetting**, and this chapter measures both on frozen held-out text.

Every decision is made by one `#authority` file,
[continue.coax](../lib/nanochat/continue.coax), built on chapter 128's
[pretrain.coax](../lib/nanochat/pretrain.coax). Its checks leave no mutant
alive.

## The profile

This chapter's profile is `nanochat-continue-cpu-f64-v1`:

- Everything of chapter 128's profile: binary64 on the CPU, one thread,
  every sum in a fixed order, chapter 125's 48,170-parameter model, chapter
  127's gradient and chapter 124's AdamW. The short run's transcript is
  exact for the named build in debug and release; the recorded runs' is
  exact in release.
- The base is a checkpoint of chapter 128, named by its plan and by the
  digest of its record. The program trains the base again, bit for bit, and
  the continued plans refuse it unless the digest matches.
- The domain is the Coax source of chapters 1 to 40, pinned at the same
  revision as chapter 128's corpus, `cf2bddd202`, and tokenized with chapter
  128's tokenizer unchanged.

## The domain workload

[continue_corpus.py](../tools/continue_corpus.py), a tooling script that is
never shipped, reads `chapters.json` at the pinned revision through `git
show` and walks chapters 1 to 40 in order. For each `psionic/lib` Coax file
a chapter lists, and that no earlier chapter listed, it makes one document:
the file's leading paragraphs, up to 3,600 bytes, with printable ASCII
only. A paragraph that already appears in chapter 128's corpus, or earlier
in this one, is dropped. The result is
[domain.coax](../fixtures/continue/domain.coax): 50 files, 174,652 bytes.

Each document's source is the chapter that lists its file, so the domain
takes chapter 128's split rule unchanged: sources divisible by 8 are held
out. The held-out domain is seven files, 24,689 bytes:
`interchange.coax` (chapter 8), the three decision files of chapter 16, the
two Pylon files of chapter 24 and `operator-registry.coax` (chapter 40).
Chapter 32 lists no library file.

The model's embedding table has one row per token, so the domain cannot
have a tokenizer of its own. Chapter 128's tokenizer, trained on prose,
encodes the 43 training files in 125,080 tokens, about 1.20 bytes a token
against prose's 1.45. Bits per byte is still the measure: it does not
depend on how many tokens the bytes became.

## One audit over both corpora

Chapter 128's audit refuses a split if any held-out paragraph is also a
training paragraph. With two corpora there are four ways for text to
cross: domain training text in either held-out set, and general training
text in either. One audit over the union of both corpora catches all four,
because the union is still one split by source:

<!-- source: psionic/lib/nanochat/continue.coax audit -->
```jai
// Chapter 128's audit over both corpora as one split: every training
// paragraph, general or domain, against every held-out paragraph. Held-out
// text may repeat held-out text. The domain must hold out at least one
// document and no more than it trains on.
ct_audit :: (general:[]string,gs:[]int,domain:[]string,ds:[]int,docs:[]string,sources:[]int,hashes:[]u64)->NpData {
    n:=general.count+domain.count;
    if gs.count!=general.count || ds.count!=domain.count || docs.count<n || sources.count<n {return NpData.{status=NpStatus.data};}
    held:=0;for s:ds {if np_holdout(s) {held+=1;}}
    if held==0 || 2*held>ds.count {return NpData.{status=NpStatus.data};}
    for i:0..<general.count {docs[i]=general[i];sources[i]=gs[i];}
    for i:0..<domain.count {docs[general.count+i]=domain[i];sources[general.count+i]=ds[i];}
    return np_audit(docs[0..<n],sources[0..<n],hashes);
}
```

The domain must also hold out at least one file and no more files than it
trains on. On these corpora the audit reads 168 documents, 24 of them held out, and
1,370 paragraphs, and finds none shared. The corpus script already removes
repeated paragraphs, so the audit is an independent second check of the
same property, not the only one.

## The plan names its base

A continued plan holds two chapter 128 plans and two more fields:

<!-- source: psionic/lib/nanochat/continue.coax plan -->
```jai
// A continued run names its base completely: base is the plan chapter 128
// trained under and digest the checkpoint that plan ended with. run is a
// chapter 128 plan for the new steps over the same model; its init must be
// 0, since every weight comes from the base. Of run.batch windows a step,
// domain come from the domain's training stream and the rest rehearse the
// base's training stream.
CtPlan :: struct {base:NpPlan;digest:[32]u8;run:NpPlan;domain:int;}
```

The mixing ratio is `domain` out of `run.batch`. With a batch of eight,
`domain = 8` trains on code only, and `domain = 6` rehearses two windows of
the original prose in every step.

Admission refuses a plan before any work. The run's model must be the
base's: the check swaps the run's model into a copy of the base plan and
asks whether the base plan's identity changed. Both plans must pass chapter
128's own admission, and each stream must hold enough windows for its share
of a step. The plan's identity digests the base plan, the base checkpoint's
digest, the run plan, the ratio and the domain data together, so a
transcript that prints it names the whole workload.

<!-- source: psionic/lib/nanochat/continue.coax admit -->
```jai
// The continued run's refusals, before any work: the same model as the base,
// no initialization of its own, at least one domain window a step, both
// plans admitted by chapter 128's rules, and enough windows in each stream.
ct_admit :: (p:CtPlan,domain:int,general:int)->CtStatus {
    same:=p.base;same.model=p.run.model;
    if !ck_equal(np_plan_id(same),np_plan_id(p.base)) || p.run.init!=0 || p.domain<1 || p.domain>p.run.batch {return CtStatus.config;}
    if np_admit(p.base,p.base.batch)!=NpStatus.ok {return CtStatus.config;}
    s:=np_admit(p.run,p.run.batch);if s==NpStatus.budget {return CtStatus.budget;}if s!=NpStatus.ok {return CtStatus.config;}
    if domain<p.domain || general<p.run.batch-p.domain {return CtStatus.data;}
    return CtStatus.ok;
}
// The continued plan's identity: the base plan, the base checkpoint, the
// run's plan, the domain count and the domain data, digested together.
ct_plan_id :: (p:CtPlan,domain:[32]u8)->[32]u8 {
    b:[136]u8;at:=ck_put(b[..],0,np_plan_id(p.base));at=ck_put(b[..],at,p.digest);at=ck_put(b[..],at,np_plan_id(p.run));
    at=ck_put64(b[..],at,cast,trunc(u64)p.domain);at=ck_put(b[..],at,domain);
    return ck_digest("psionic.continue-plan.v1",b[0..<at]);
}
```

The base is admitted only as the exact checkpoint the plan names. The
record's last 32 bytes are its digest, a fold of everything before it, so
comparing them with the plan's `digest` names one record. That record must
then hold the base plan's last step, and chapter 128's resume must accept
it under the base plan and the base data. The weights are kept. The
optimizer starts fresh, its moments zero and its step counter at 0, as
nanochat's later training stages build a new optimizer over a loaded model.

<!-- source: psionic/lib/nanochat/continue.coax base -->
```jai
// The base: the record's trailing digest must be the one the plan names,
// chapter 128's resume must admit the whole record under the base plan and
// data, and the base run must have finished. The weights are kept; the
// optimizer starts fresh, its moments zero. A refusal writes nothing.
ct_base :: (p:CtPlan,data:[32]u8,bytes:[]u8,w:[]f64,m:[]f64,v:[]f64)->CtBase {
    if bytes.count<32 || !ck_equal(ck_get(bytes,bytes.count-32),p.digest) {return CtBase.{status=CtStatus.digest};}
    if bytes.count>=NP_HEAD && ck_get64(bytes,72)!=cast,trunc(u64)p.base.steps {return CtBase.{status=CtStatus.unfinished};}
    r:=np_resume(p.base,np_plan_id(p.base),data,bytes,w,m,v);if r.status!=NpCheck.ok {return CtBase.{status=CtStatus.base,check=r.status};}
    for i:0..<m.count {m[i]=0;v[i]=0;}
    return CtBase.{};
}
```

## The mixture, by hand

A step reads `domain` windows from the domain's training stream, then
`batch − domain` from chapter 128's training stream. Each stream has its
own seeded order of windows, reshuffled every epoch by chapter 128's
`np_window`; the general order uses the run's seed plus one, so the two
orders are not the same permutation. Each stream's position advances by
its own count per step.

Trace the short example's second run: a batch of four, `domain = 3`, seed
7, 8 steps, over 1,954 domain windows and 3,109 general ones.

```text
step s reads   domain positions 3s, 3s+1, 3s+2   (order under seed 7)
               general position s                (order under seed 8)
8 steps read   domain positions 0 to 23, general positions 0 to 7
               24 + 8 = 32 windows of 64 tokens, 32 · 63 = 2,016 predicted tokens
rehearsal      8 of 32 windows = 1/4 of the step's gradient weight
```

Neither stream reaches the end of its first epoch. Every window weighs
`1/4` in the step's mean gradient, so rehearsal carries a quarter of each
step. With `domain = 4` the general stream is never read.

The rest of a step is chapter 128's: the mean gradient, its norm, clipping,
the warmup and warmdown schedule, and AdamW.

<!-- source: psionic/lib/nanochat/continue.coax step -->
```jai
// One continued step: domain windows from the domain stream in the run's
// order, then rehearsal windows from the general stream in the order under
// seed + 1, each stream's position advancing by its own count per step. The
// rest is chapter 128's step: mean, norm, clipping, schedule and AdamW.
ct_step :: (p:CtPlan,run:NpRun,domain:[]int,general:[]int,dorder:[]int,gorder:[]int,w:[]f64,m:[]f64,v:[]f64,g:[]f64,total:[]f64,tape:[]f64,logits:[]f64)->NpStep {
    q:=p.run;c:=q.model;L:=c.sequence_len;dw:=domain.count/L;gw:=general.count/L;rehearse:=q.batch-p.domain;n:=w.count;fail:=NpStep.{run=run};
    if run.step>=q.steps {fail.status=NpStatus.config;return fail;}
    if dw<p.domain || gw<rehearse {fail.status=NpStatus.data;return fail;}
    gp:=q;gp.seed=q.seed+1;for i:0..<n {total[i]=0;}loss:=0.0;k:=f64(q.batch);
    for j:0..<q.batch {s:=ifx j<p.domain then domain else general;
        at:=L*(ifx j<p.domain then np_window(q,dw,run.step*p.domain+j,dorder) else np_window(gp,gw,run.step*rehearse+j-p.domain,gorder));
        r:=nb_gradient(c,w,s[at..<at+L],g,tape[0..<nb_tape_count(c,L)],logits[0..<L*c.vocab],false);
        if r.status!=NcStatus.ok {fail.status=NpStatus.data;return fail;}
        for i:0..<n {total[i]+=g[i]/k;}loss+=r.loss/k;
    }
    sum:=0.0;for x:total {sum+=x*x;}norm:=sqrt(sum);
    if q.clip>0 && norm>q.clip {for i:0..<n {total[i]=total[i]*q.clip/norm;}}
    scale:=np_scale(q,run.step);groups:[5]AwGroup;np_groups(q,scale,groups[..]);none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,w,total,m,v,none[0..<0],false);
    if a.status!=AwStatus.ok {fail.status=NpStatus.optimizer;return fail;}
    return NpStep.{run=NpRun.{step=run.step+1,state=a.state},loss=loss,norm=norm,scale=scale};
}
```

## The promotion rule, by hand

The rule is written before the first run and never changes with its
results. The base and the continued model are evaluated on the same frozen
targets of both held-out sets, in chapter 128's bits per byte. The gain is
the base's domain bits per byte minus the continued model's. The forgetting
is the continued model's general bits per byte minus the base's. A model is
promoted when the gain is at least 0.2 and the forgetting at most 0.05.

Work three cases by hand, with the base at 3 bits per byte on general text
and 4 on the domain:

```text
continued general 3.05, domain 3.75:  gain 4 − 3.75 = 0.25 ≥ 0.2,  forgetting 3.05 − 3 = 0.05 ≤ 0.05   passed
continued general 3.00, domain 3.80:  gain 0.2, exactly the line,  forgetting 0                      passed
continued general 3.05, domain 4.50:  gain −0.5 < 0.2,  forgetting 0.0500001 > 0.05                  failed (both)
```

The check "verdicts by hand" in `continue.coax` builds exactly these
evaluations and requires these verdicts.

A rule that compares two numbers must also say when the numbers cannot be
compared. Following [chapter 241](ch241-gates-that-can-say-unverifiable.md),
the verdict is one of three: **passed**, **failed** or **unverifiable**. It
is unverifiable when an evaluation failed or gave a non-finite value, when
the two models were scored on different targets or bytes, or when either
held-out set has fewer than 500 targets.

<!-- source: psionic/lib/nanochat/continue.coax accept -->
```jai
// The promotion rule, frozen before the first run. On the same targets as
// the base, the continued model must gain at least CT_GAIN bits per byte on
// the domain's held-out text and lose at most CT_FORGET on chapter 128's
// held-out text. Each evaluation needs CT_TARGETS targets; fewer, different
// targets or a failed evaluation make the verdict unverifiable.
CT_GAIN :: 0.2;
CT_FORGET :: 0.05;
CT_TARGETS :: 500;
ct_judge :: (base_general:NpEval,base_domain:NpEval,general:NpEval,domain:NpEval)->CtJudge {
    r:=CtJudge.{};all:=[4]NpEval.[base_general,base_domain,general,domain];
    for e:all {if e.status!=NpStatus.ok || !aw_finite(e.bpb) {r.reason=CtReason.number;return r;}}
    if general.targets!=base_general.targets || general.bytes!=base_general.bytes || domain.targets!=base_domain.targets || domain.bytes!=base_domain.bytes {r.reason=CtReason.mismatch;return r;}
    if base_general.targets<CT_TARGETS || base_domain.targets<CT_TARGETS {r.reason=CtReason.few_targets;return r;}
    r.gain=base_domain.bpb-domain.bpb;r.forget=general.bpb-base_general.bpb;
    small:=r.gain<CT_GAIN;forgot:=r.forget>CT_FORGET;r.verdict=CtVerdict.failed;
    if small && forgot {r.reason=CtReason.both;} else if small {r.reason=CtReason.small_gain;} else if forgot {r.reason=CtReason.forgot;}
    else {r.verdict=CtVerdict.passed;}
    return r;
}
```

## Run it

The driver trains chapter 128's base from its plan, saves its final
checkpoint, resumes it into fresh buffers through `ct_base`, evaluates it,
then trains and judges each continued plan from that same base. It holds
about 16 MiB of buffers and prints; every decision it reports comes from
`continue.coax`.

<!-- source: psionic/tests/continue-drive.coax drive -->
```jai
// The base run of chapter 128 to its last step and its final checkpoint; the
// continued plans resumed from that checkpoint, each trained and judged
// against the base on the same targets: gwin windows of chapter 128's
// held-out text and dwin windows of the domain's.
ct_drive :: (plans:[]CtPlan,gwin:int,dwin:int,report:int,refusals:bool) {
    base:=plans[0].base;c:=base.model;L:=c.sequence_len;
    docs:=NP_CORPUS;sources:=NP_CORPUS_SOURCES;dd:=CT_DOMAIN;ds:=CT_DOMAIN_SOURCES;classes:=PT_TOY;packed:[65536]u8;scratch:[65536]u8;
    t:=np_tokenizer(classes[..],docs[..],sources[..],NP_MERGES,packed[..],scratch[..]);#assert t.status==NpStatus.ok && t.vocab==c.vocab;
    bos:=t.vocab-1;lengths:[320]int;#assert np_lengths(packed[0..<t.written],bos,lengths[..])==NpStatus.ok;
    hashes:[NP_PARAGRAPHS]u64;both:[256]string;bs:[256]int;
    a:=ct_audit(docs[..],sources[..],dd[..],ds[..],both[..],bs[..],hashes[..]);#assert a.status==NpStatus.ok;
    print("audit over both corpora: % documents, % held out (sources divisible by %), % paragraphs, shared paragraphs: %\n",a.train_docs+a.val_docs,a.val_docs,NP_HOLDOUT,a.paragraphs,a.shared);
    train:[NP_TRAIN_MAX]int;val:[NP_VAL_MAX]int;dtrain:[CT_DOMAIN_MAX]int;dval:[CT_HELD_MAX]int;
    d:=np_data(packed[0..<t.written],docs[..],sources[..],bos,hashes[..],train[..],val[..]);#assert d.status==NpStatus.ok;
    e:=np_data(packed[0..<t.written],dd[..],ds[..],bos,hashes[..],dtrain[..],dval[..]);#assert e.status==NpStatus.ok;
    tr:=train[0..<d.train];va:=val[0..<d.val];dtr:=dtrain[0..<e.train];dva:=dval[0..<e.val];
    print("general (chapter 128): % training windows; held out: % tokens, % frozen windows\n",d.train/L,d.val,gwin);
    print("domain (Coax source, chapter 128's tokenizer): % files, % bytes, % tokens, % windows; held out: % files, % bytes, % tokens, % frozen windows\n",
        e.train_docs,e.train_bytes,e.train,e.train/L,e.val_docs,e.val_bytes,e.val,dwin);
    // The base, exactly as chapter 128 trains it.
    #assert np_admit(base,d.train/L)==NpStatus.ok;n:=nc_parameter_count(c);#assert n==NP_PARAMETERS;
    w:[NP_PARAMETERS]f64;m:[NP_PARAMETERS]f64;v:[NP_PARAMETERS]f64;g:[NP_PARAMETERS]f64;total:[NP_PARAMETERS]f64;
    tape:[101040]f64;logits:[20480]f64;order:[NP_TRAIN_MAX/64+1]int;order[d.train/L]=-1;
    np_init(c,base.init,w[..]);pid:=np_plan_id(base);did:=np_data_id(tr,va);fin:[1156212]u8;mid:[1156212]u8;run:=NpRun.{};
    while run.step<base.steps {r:=np_step(base,run,tr,order[0..<d.train/L+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;run=r.run;
        if run.step==base.steps/2 {#assert np_save(pid,did,run,w[..],m[..],v[..],mid[..])==NpCheck.ok;}}
    #assert np_save(pid,did,run,w[..],m[..],v[..],fin[..])==NpCheck.ok;
    print("base: chapter 128's plan, % steps x % windows; final checkpoint digest ",base.steps,base.batch);np_hex(ck_get(fin[..],fin.count-32));
    print("..., the one the plans name: %\n",ck_equal(ck_get(fin[..],fin.count-32),plans[0].digest));
    // Fresh buffers hold only what the checkpoint gives.
    x:[NP_PARAMETERS]f64;xm:[NP_PARAMETERS]f64;xv:[NP_PARAMETERS]f64;
    b:=ct_base(plans[0],did,fin[..],x[..],xm[..],xv[..]);#assert b.status==CtStatus.ok;
    bg:=ct_eval(c,x[..],va,lengths[..],gwin);bd:=ct_eval(c,x[..],dva,lengths[..],dwin);
    print("base model: general held-out % bits per byte (% targets), domain held-out % bits per byte (% targets)\n",bg.bpb,bg.targets,bd.bpb,bd.targets);
    print("promotion rule (frozen before the runs): domain gain at least %, general forgetting at most % bits per byte\n",CT_GAIN,CT_FORGET);
    did2:=np_data_id(dtr,dva);
    for k:0..<plans.count {p:=plans[k];
        #assert ct_admit(p,e.train/L,d.train/L)==CtStatus.ok && ck_equal(p.digest,plans[0].digest);
        print("run %: % steps, % domain + % rehearsal windows a step, plan ",k+1,p.run.steps,p.domain,p.run.batch-p.domain);np_hex(ct_plan_id(p,did2));print("...\n");
        y:=x;ym:=xm;yv:=xv;dor:[CT_DOMAIN_MAX/64+1]int;dor[e.train/L]=-1;gor:[NP_TRAIN_MAX/64+1]int;gor[d.train/L]=-1;cr:=NpRun.{};
        while cr.step<p.run.steps {r:=ct_step(p,cr,dtr,tr,dor[0..<e.train/L+1],gor[0..<d.train/L+1],y[..],ym[..],yv[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;cr=r.run;
            if cr.step%report==0 {print("  step %: loss %, gradient norm %, rate scale %\n",cr.step,r.loss,r.norm,r.scale);}}
        rg:=ct_eval(c,y[..],va,lengths[..],gwin);rd:=ct_eval(c,y[..],dva,lengths[..],dwin);j:=ct_judge(bg,bd,rg,rd);
        print("  general % (forgetting %), domain % (gain %): % (%)\n",rg.bpb,j.forget,rd.bpb,j.gain,j.verdict,j.reason);
        if k==plans.count-1 && refusals {
            // The same model judged on too few domain targets.
            sd:=ct_eval(c,x[..],dva,lengths[..],4);sy:=ct_eval(c,y[..],dva,lengths[..],4);j=ct_judge(bg,sd,rg,sy);
            print("run %, judged on 4 domain windows (% targets): % (%)\n",k+1,sd.targets,j.verdict,j.reason);
        }
    }
    if !refusals {return;}
    // Broken inputs, each refused before a step.
    q:=plans[0];q.digest[0]^=1;print("a plan naming another checkpoint: %\n",ct_base(q,did,fin[..],x[..],xm[..],xv[..]).status);
    q=plans[0];q.digest=ck_get(mid[..],mid.count-32);print("the base's checkpoint after step %, named by its digest: %\n",base.steps/2,ct_base(q,did,mid[..],x[..],xm[..],xv[..]).status);
    q=plans[0];q.base.seed+=1;b=ct_base(q,did,fin[..],x[..],xm[..],xv[..]);print("the right checkpoint under a base plan with another seed: % (%)\n",b.status,b.check);
    q=plans[0];q.run.init=base.init;print("a continued run that draws its own initialization: %\n",ct_admit(q,e.train/L,d.train/L));
    leaky:=dd;for i:0..<dd.count {if np_holdout(ds[i]) {leaky[i]=docs[0];break;}}a=ct_audit(docs[..],sources[..],leaky[..],ds[..],both[..],bs[..],hashes[..]);
    print("a held-out domain file that repeats a general training document: %, % shared paragraphs\n",a.status,a.shared);
}
```

[The short run](../examples/continue.coax) takes chapter 128's example plan
as the base, and runs the same 8-step continuation twice, with no
rehearsal and with one rehearsal window in four:

<!-- source: psionic/examples/continue.coax example -->
```jai
// Chapter 128's short run as the base, named by its final checkpoint's
// digest; then two continued runs of 8 steps over four windows each, all
// from the domain or three domain and one rehearsal, judged on the first 16
// frozen windows of each held-out stream.
// Chapter 128's example plan (psionic/examples/pretrain.coax), unchanged.
CT_BASE :: NpPlan.{model=NP_MODEL,batch=4,steps=24,warmup=4,warmdown=8,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1281,budget=1e10};
CT_DEMO_BASE :: [32]u8.[76,25,188,79,245,21,219,87,186,196,126,234,146,229,237,37,7,31,66,20,220,235,230,208,230,249,132,131,248,229,134,186];
CT_DEMO_RUN :: NpPlan.{model=NP_MODEL,batch=4,steps=8,warmup=2,warmdown=4,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=0,budget=1e10};
CT_DEMO :: [2]CtPlan.[CtPlan.{base=CT_BASE,digest=CT_DEMO_BASE,run=CT_DEMO_RUN,domain=4},CtPlan.{base=CT_BASE,digest=CT_DEMO_BASE,run=CT_DEMO_RUN,domain=3}];
main :: () {plans:=CT_DEMO;ct_drive(plans[..],16,16,4,true);}
```

```shell
coax/bin/coax run psionic/examples/continue.coax
psionic/check.sh --only nanochat/continue
```

```text
audit over both corpora: 168 documents, 24 held out (sources divisible by 8), 1370 paragraphs, shared paragraphs: 0
general (chapter 128): 3109 training windows; held out: 34564 tokens, 16 frozen windows
domain (Coax source, chapter 128's tokenizer): 43 files, 149963 bytes, 125080 tokens, 1954 windows; held out: 7 files, 24689 bytes, 20448 tokens, 16 frozen windows
base: chapter 128's plan, 24 steps x 4 windows; final checkpoint digest 4c19bc4ff515db57..., the one the plans name: true
base model: general held-out 4.355480036494545 bits per byte (1008 targets), domain held-out 5.731033721859297 bits per byte (1008 targets)
promotion rule (frozen before the runs): domain gain at least 0.2, general forgetting at most 0.05 bits per byte
run 1: 8 steps, 4 domain + 0 rehearsal windows a step, plan 806f7ca82e486b4f...
  step 4: loss 4.674891828797806, gradient norm 0.6441695051676039, rate scale 1.0
  step 8: loss 4.4771814800154655, gradient norm 0.5514312812826376, rate scale 0.25
  general 4.494086169903292 (forgetting 0.1386061334087465), domain 5.248352728546574 (gain 0.48268099331272296): failed (forgot)
run 2: 8 steps, 3 domain + 1 rehearsal windows a step, plan bb954a9a91ab33cb...
  step 4: loss 4.606333427104724, gradient norm 0.4934549405404174, rate scale 1.0
  step 8: loss 4.176463844871427, gradient norm 0.4930053370674742, rate scale 0.25
  general 4.3829080393042155 (forgetting 0.027428002809670282), domain 5.203338591928625 (gain 0.527695129930672): passed (cleared)
run 2, judged on 4 domain windows (252 targets): unverifiable (few_targets)
a plan naming another checkpoint: digest
the base's checkpoint after step 12, named by its digest: unfinished
the right checkpoint under a base plan with another seed: base (plan)
a continued run that draws its own initialization: config
a held-out domain file that repeats a general training document: leak, 25 shared paragraphs
```

The [transcript](../fixtures/continue.out) is exact for the named build in
debug and release. The base line is the first cross-check: the program
trained chapter 128's example plan again, its final checkpoint has the
digest the plans name, and the resumed model scores 4.355480036494545 bits
per byte on the general held-out windows, the same value chapter 128's
example prints as its final line. On the domain the base scores 5.73 bits
per byte, near a uniform guess: it has seen no code.

Eight steps on code alone gain 0.48 bits per byte on held-out code and lose
0.14 on held-out prose, so the rule fails it for forgetting. The same eight
steps with one window in four rehearsing prose gain 0.53 and lose 0.03, and
pass. That is one short run on 16 windows of each held-out set; it shows
every piece working, not that rehearsal always pays. Judged on only 4
domain windows, the same comparison has 252 targets and is unverifiable.
The last five lines are deliberately broken inputs, each refused before a
step: a plan naming another checkpoint, the base's checkpoint after step 12
(a real record of the right plan, but unfinished), the right checkpoint
under a base plan with another seed, a continued plan with its own
initialization, and a held-out domain file that copies a general training
document.

## The recorded runs

The recorded runs take chapter 128's recorded 1,000-step plan as the base,
then continue it three times for 300 steps of eight windows, with 8, 6 and
4 of them from the domain. Each run reads 2,400 windows, about 1.2 passes
over the domain's training windows when it takes all eight. The base and
each continued model are judged on 256 frozen windows of the general
held-out text (16,121 targets) and all 319 whole windows of the domain's
(20,091 targets). The runs are in release only, a few minutes on one core,
and [check_continue.sh](../tests/check_continue.sh) compares the transcript
with the [recorded one](../fixtures/continue/long.out).

<!-- source: psionic/tests/continue-long.coax long -->
```jai
// Chapter 128's recorded plan (psionic/tests/pretrain-long.coax), unchanged.
CT_BASE :: NpPlan.{model=NP_MODEL,batch=8,steps=1000,warmup=40,warmdown=300,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1281,budget=2e11};
CT_LONG_BASE :: [32]u8.[65,222,176,108,163,30,136,242,155,229,186,160,220,183,165,76,126,212,16,134,190,239,117,174,79,246,248,12,63,193,12,61];
CT_LONG_RUN :: NpPlan.{model=NP_MODEL,batch=8,steps=300,warmup=30,warmdown=100,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=0,budget=1e11};
CT_LONG :: [3]CtPlan.[CtPlan.{base=CT_BASE,digest=CT_LONG_BASE,run=CT_LONG_RUN,domain=8},CtPlan.{base=CT_BASE,digest=CT_LONG_BASE,run=CT_LONG_RUN,domain=6},
    CtPlan.{base=CT_BASE,digest=CT_LONG_BASE,run=CT_LONG_RUN,domain=4}];
main :: () {plans:=CT_LONG;ct_drive(plans[..],256,319,100,false);}
```

```text
audit over both corpora: 168 documents, 24 held out (sources divisible by 8), 1370 paragraphs, shared paragraphs: 0
general (chapter 128): 3109 training windows; held out: 34564 tokens, 256 frozen windows
domain (Coax source, chapter 128's tokenizer): 43 files, 149963 bytes, 125080 tokens, 1954 windows; held out: 7 files, 24689 bytes, 20448 tokens, 319 frozen windows
base: chapter 128's plan, 1000 steps x 8 windows; final checkpoint digest 41deb06ca31e88f2..., the one the plans name: true
base model: general held-out 2.533604422311366 bits per byte (16121 targets), domain held-out 5.846832642738594 bits per byte (20091 targets)
promotion rule (frozen before the runs): domain gain at least 0.2, general forgetting at most 0.05 bits per byte
run 1: 300 steps, 8 domain + 0 rehearsal windows a step, plan b1ab359307dbc12f...
  step 100: loss 2.7986807652315777, gradient norm 0.5675973046616911, rate scale 1.0
  step 200: loss 2.378771619529852, gradient norm 0.5638495231064203, rate scale 1.0
  step 300: loss 2.2563547458804676, gradient norm 0.45846232504308354, rate scale 0.01
  general 3.4647864701230495 (forgetting 0.9311820478116837), domain 3.162510574076871 (gain 2.684322068661723): failed (forgot)
run 2: 300 steps, 6 domain + 2 rehearsal windows a step, plan f9b313b21d539504...
  step 100: loss 2.260640691198524, gradient norm 0.5106953892450838, rate scale 1.0
  step 200: loss 2.5114921239448185, gradient norm 0.5052495464491579, rate scale 1.0
  step 300: loss 2.269970270442076, gradient norm 0.4758145214543546, rate scale 0.01
  general 2.861932479517296 (forgetting 0.32832805720593017), domain 3.1368671308267952 (gain 2.7099655119117987): failed (forgot)
run 3: 300 steps, 4 domain + 4 rehearsal windows a step, plan 82c6102661526e41...
  step 100: loss 2.5565292603721748, gradient norm 0.5677504449289382, rate scale 1.0
  step 200: loss 2.5760981770547353, gradient norm 0.5067604015324989, rate scale 1.0
  step 300: loss 2.6201400738516543, gradient norm 0.443649281742207, rate scale 0.01
  general 2.689961217134595 (forgetting 0.15635679482322917), domain 3.2631918302225467 (gain 2.5836408125160473): failed (forgot)
```

The base scores 2.533604422311366 bits per byte on the general held-out
windows, exactly chapter 128's recorded final value: the same checkpoint,
reached again bit for bit and named by its digest.

| Domain windows of 8 | Domain bits per byte | Gain | General bits per byte | Forgetting | Verdict |
| --- | --- | --- | --- | --- | --- |
| base | 5.847 | — | 2.534 | — | — |
| 8 | 3.163 | 2.684 | 3.465 | 0.931 | failed: forgot |
| 6 | 3.137 | 2.710 | 2.862 | 0.328 | failed: forgot |
| 4 | 3.263 | 2.584 | 2.690 | 0.156 | failed: forgot |

Every run gains far more than the 0.2 bits per byte the rule asks for on
held-out code. Rehearsal is a strong knob for forgetting: two windows in
eight cut it from 0.93 to 0.33 bits per byte, four cut it to 0.16, and the
gain barely moves. But no ratio tried meets the frozen limit of 0.05, so
**no continued model is promoted**. The rule stays as it was written; a
later run with more rehearsal, a lower rate or fewer steps would be a new
plan with its own identity, judged by the same rule.

These numbers measure this experiment: a 48,170-parameter model, 150 KB
of one library's source and seven held-out files. They say nothing about
code quality in general.

## Checks

The checks in `continue.coax` are independent of the code they test where
it matters:

- **The base identity.** A six-step base is trained with chapter 128's
  step and saved after steps 3 and 6. The plan naming the final record gets
  its weights bit for bit and zero moments. Each wrong base is refused and
  writes nothing: another digest, the step-3 record named by its own digest
  (unfinished), the final record under a seven-step plan, a flipped bit
  inside the record with its digest intact (corrupt), another base seed
  (plan), other data (data) and other buffer sizes (shape). The examples go
  further: the base they train must reproduce chapter 128's checkpoint
  digest exactly, or every continued plan refuses it.
- **The mixed step by hand.** At steps 0 and 3, the step's mean gradient
  must equal half of `nb_gradient` on the domain window `np_window` names
  plus half on the general window under seed + 1, bit for bit, with the
  loss, the norm and the schedule's scale. A step with no rehearsal must
  match domain windows 2 and 3 by hand and never read a general stream
  filled with out-of-vocabulary tokens.
- **Reproducibility.** Six steps run twice from the same base give
  identical parameters; another seed does not.
- **The audit** by hand over six small documents: held-out text that
  repeats held-out text is admitted; domain training text that is general
  held-out text, and the reverse, are refused.
- **The verdicts by hand,** as worked above, with each line exactly at the
  threshold, every way to be unverifiable, and the identity of the plan
  recomputed from its written layout.
- **Admission** refuses each field just outside its range.

[The domain corpus](../fixtures/continue/domain.coax) regenerates exactly
from its script. Mutation of `continue.coax` leaves no survivors.

## What remains

- Chapter 241's decision gate. This rule compares point estimates against
  fixed lines. The gate would pass a gain only beyond a noise floor
  measured over several seeds of the base and of each continued run, which
  needs repeated base runs this CPU slice does not make.
- Adaptation to tasks other than the domain's own text: instruction data
  ([chapter 136](ch136-supervised-conversations-and-tools.md)), adapters
  ([chapter 140](ch140-post-train-a-decision-model.md)) and task metrics beyond
  bits per byte.
- A domain tokenizer. Growing the vocabulary needs new embedding and output
  rows and a rule for initializing them.
- Larger domains and runs beyond the CPU.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
- [Chapter 128](ch128-a-complete-pretraining-run.md): the base model and its
  pretraining pipeline.
- [Chapter 120](ch120-data-and-resumable-state.md): provenance and splits.
- [Chapter 241](ch241-gates-that-can-say-unverifiable.md): verdicts that can
  say unverifiable.
