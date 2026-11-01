# 128. A complete pretraining run

> **Status: partly implemented.** A small nanochat-shaped model trains from
> scratch on the CPU on this book's own prose, with a leak-free split, a
> learning-rate schedule, periodic held-out evaluation, checkpoints, exact
> resume and an acceptance rule frozen before the run, which it passes. Still
> to come: nanochat's MuonAdamW optimizer in the run (this run uses AdamW
> only), BOS-aligned best-fit packing, the two-way compute-budget
> comparison, GPU-scale runs and any reproduction of nanochat's speedrun.

[Chapter 127](ch127-differentiate-nanochat.md) gave every parameter of the
pinned nanochat GPT a checked gradient. This chapter spends those gradients:
it trains the same architecture from random weights on a real text corpus,
end to end in Coax, and says beforehand what result counts as success.

A pretraining run is more than a loop of gradient steps. It needs a corpus
and a tokenizer, a split that keeps held-out text out of training, a
schedule, an evaluation on frozen data in a unit that does not depend on the
tokenizer, a checkpoint that a crash cannot corrupt, and a resume that
continues the run exactly. Each of those is a decision, and each decision
here is made by one `#authority` file,
[pretrain.coax](../lib/nanochat/pretrain.coax), whose checks leave no
mutant alive.

## The profile

This chapter's profile is `nanochat-pretrain-cpu-f64-v1`:

- Binary64 on the CPU, one thread, every sum in a fixed order. The short
  run's transcript is exact for the named build in debug and release; the
  recorded run's is exact in release.
- The model is chapter 125's GPT: two blocks of width 32, two query heads
  over one key-value head, a value embedding on the second block, windows of
  64 tokens over a vocabulary of 320. It has 48,170 parameters.
- The gradient is chapter 127's `nb_gradient`, one window at a time. A step
  averages it over a batch of windows and takes one step of
  [chapter 124's](ch124-adamw-and-explicit-precision.md) AdamW.
- The corpus is the prose of chapters 1 to 40 of this book, pinned at
  revision `cf2bddd202`: 118 documents, 339,273 bytes.

## The corpus, the split and the tokenizer

[pretrain_corpus.py](../tools/pretrain_corpus.py), a tooling script that is
never shipped, reads chapters 1 to 40 through `git show` at the pinned
revision, so later edits to the book never change the corpus. It keeps
headings and paragraphs, drops code, tables and comments, keeps printable
ASCII only, removes repeated paragraphs, and cuts each chapter into
documents of at most 3,600 bytes. Each document records its chapter as its
source. The result is [corpus.coax](../fixtures/pretrain/corpus.coax).

The split is by source, never by window. Every document of chapters 8, 16,
24, 32 and 40 is held out: 17 documents, 49,981 bytes. A window cut from a
held-out chapter therefore never shares a document with training text.
Duplicated text could still cross the split, so the audit hashes every
training paragraph (FNV-1a over the bytes between blank lines) and refuses
the split if any held-out paragraph has a training paragraph's hash. A hash
collision would refuse a clean split, never admit a leaky one. On this
corpus the audit finds 1,243 paragraphs and none shared.

<!-- source: psionic/lib/nanochat/pretrain.coax split -->
```jai
np_holdout :: (source:int)->bool {return source%NP_HOLDOUT==0;}
// FNV-1a over one paragraph.
np_fnv :: (s:string)->u64 {h:=cast(u64)0xcbf29ce484222325;for i:0..<s.count {h=mul_wrap(h^cast(u64)s[i],cast(u64)0x100000001b3);}return h;}
// Paragraphs are the non-empty runs between blank lines ("\n\n"). Training
// paragraphs' hashes are kept; a held-out paragraph with the same hash is a
// shared paragraph, and any shared paragraph refuses the split.
np_audit :: (docs:[]string,sources:[]int,hashes:[]u64)->NpData {
    r:=NpData.{};if docs.count!=sources.count {r.status=NpStatus.data;return r;}
    kept:=0;
    for pass:0..<2 {
        for d:0..<docs.count {if np_holdout(sources[d])!=(pass==1) {continue;}
            if pass==0 {r.train_docs+=1;} else {r.val_docs+=1;}
            s:=docs[d];at:=0;
            while at<s.count {
                if s[at]==10 {at+=1;continue;}
                end:=at;while end<s.count && !(s[end]==10 && end+1<s.count && s[end+1]==10) {end+=1;}
                h:=np_fnv(s[at..<end]);r.paragraphs+=1;at=end;
                if pass==0 {if kept==hashes.count {r.status=NpStatus.data;return r;}hashes[kept]=h;kept+=1;}
                else {for k:0..<kept {if hashes[k]==h {r.shared+=1;}}}
            }
        }
    }
    if r.train_docs==0 || r.val_docs==0 {r.status=NpStatus.data;return r;}
    if r.shared>0 {r.status=NpStatus.leak;}
    return r;
}
```

[Chapter 123's](ch123-learn-a-tokenizer.md) byte BPE learns 63 merges from
16 samples of training documents only: evenly spaced documents, the first
256 bytes of each. With the 256 byte tokens and a BOS token the vocabulary
is exactly 320, five rows of nanochat's padding to 64. Each document becomes
BOS followed by its tokens. Training text is 289,292 bytes in 199,039
tokens, about 1.45 bytes a token.

<!-- source: psionic/lib/nanochat/pretrain.coax tokenizer -->
```jai
// Chapter 123's byte BPE, trained on samples of training documents only: up
// to 16 evenly spaced training documents, the first 256 bytes of each.
np_samples :: (docs:[]string,sources:[]int,out:[]string)->int {
    train:=0;for s:sources {if !np_holdout(s) {train+=1;}}
    k:=np_min(train,NP_SAMPLES);
    for j:0..<k {want:=j*train/k;seen:=0;
        for d:0..<docs.count {if !np_holdout(sources[d]) {if seen==want {s:=docs[d];out[j]=s[0..<np_min(s.count,NP_SAMPLE_BYTES)];}seen+=1;}}}
    return k;
}
np_tokenizer :: (classes:[]u8,docs:[]string,sources:[]int,merges:int,packed:[]u8,scratch:[]u8)->NpTok {
    if docs.count!=sources.count {return NpTok.{status=NpStatus.data};}
    picks:[16]string;k:=np_samples(docs,sources,picks[..]);
    m:=bt_train(classes,picks[0..<k],merges);if m.status!=PtStatus.ok || m.merges!=merges {return NpTok.{status=NpStatus.tokenizer};}
    w:=bt_pack(m,classes,packed,scratch,false);if w.status!=PtStatus.ok {return NpTok.{status=NpStatus.tokenizer};}
    return NpTok.{written=w.written,merges=m.merges,vocab=257+m.merges};
}
// Bytes per token ID; BOS (and anything past it) has none.
np_lengths :: (packed:[]u8,bos:int,out:[]int)->NpStatus {
    if out.count<=bos {return NpStatus.data;}
    for i:bos..<out.count {out[i]=0;}
    one:[1]int;for i:0..<bos {one[0]=i;r:=bt_decode(packed,one[..]);if r.status!=PtStatus.ok {return NpStatus.tokenizer;}out[i]=r.written;}
    return NpStatus.ok;
}
// The two streams: each document is BOS then its tokens, training sources
// into train and held-out sources into val, after the audit admits the split.
np_data :: (packed:[]u8,docs:[]string,sources:[]int,bos:int,hashes:[]u64,train:[]int,val:[]int)->NpData {
    r:=np_audit(docs,sources,hashes);if r.status!=NpStatus.ok {return r;}
    for d:0..<docs.count {e:=bt_encode(packed,docs[d]);if e.status!=PtStatus.ok {r.status=NpStatus.tokenizer;return r;}
        if np_holdout(sources[d]) {
            if r.val+1+e.written>val.count {r.status=NpStatus.data;return r;}
            val[r.val]=bos;for i:0..<e.written {val[r.val+1+i]=e.ids[i];}r.val+=1+e.written;r.val_bytes+=docs[d].count;
        } else {
            if r.train+1+e.written>train.count {r.status=NpStatus.data;return r;}
            train[r.train]=bos;for i:0..<e.written {train[r.train+1+i]=e.ids[i];}r.train+=1+e.written;r.train_bytes+=docs[d].count;
        }
    }
    return r;
}
```

## Bits per byte, by hand

Loss in nats per token depends on the tokenizer: a tokenizer with longer
tokens has fewer, harder predictions. Bits per byte does not. Add the
negative log probability of every predicted token in nats, divide by ln 2,
and divide by the number of bytes those tokens decode to. BOS decodes to no
bytes, so a BOS target is skipped in both sums, as nanochat's evaluation
masks it.

Work one case by hand. Give a 40-token model a head of zeros. Every logit is
`15·tanh(0) = 0`, so every target costs `ln 40` nats. Give token `i` a
length of `1 + i mod 3` bytes and BOS (39) none, and evaluate two windows of
eight:

```text
window 1: 39 1 2 3 39 5 6 7      targets 1 2 3 5 6 7   bytes 2+3+1+3+1+2 = 12
window 2: 39 9 10 11 12 13 14 15  targets 9 ... 15      bytes 1+2+3+1+2+3+1 = 13
13 targets, 25 bytes:  bits per byte = 13 · log2(40) / 25 = 13 · 5.32193 / 25 = 2.76740
```

The first check in `pretrain.coax` builds exactly this model and requires
`np_eval` to report 13 targets, 25 bytes and `13·ln 40` nats. A second case
gives the head large weights and requires one window's nats to equal seven
times chapter 125's mean loss for that window.

<!-- source: psionic/lib/nanochat/pretrain.coax eval -->
```jai
// Held-out bits per byte over the first `windows` windows of val: the
// negative log probability of every target that has bytes (BOS has none),
// in nats, over ln 2 times those targets' bytes.
np_eval :: (c:NcConfig,w:[]f64,val:[]int,lengths:[]int,windows:int,cache:[]f64,work:[]f64,logits:[]f64)->NpEval {
    L:=c.sequence_len;V:=c.vocab;r:=NpEval.{};if windows<1 || windows*L>val.count {r.status=NpStatus.data;return r;}
    none:[1]f64;
    for k:0..<windows {t:=val[k*L..<k*L+L];
        f:=nc_forward(c,w,t,cache,work,logits,none[0..<0],false);if f.status!=NcStatus.ok {r.status=NpStatus.number;return r;}
        for i:0..<L-1 {y:=t[i+1];b:=lengths[y];
            // The softcap keeps every logit within 15, so exp needs no shift.
            if b>0 {z:=i*V;sum:=0.0;for x:0..<V {sum+=exp(logits[z+x]);}r.nats+=log(sum)-logits[z+y];r.bytes+=b;r.targets+=1;}
        }
    }
    r.bpb=r.nats/(log(2.0)*f64(r.bytes));return r;
}
// The same targets under counts from the training stream: unigram with
// add-one smoothing, and bigram with add-k for each k of NP_K, the best kept.
// counts holds V*V pair counts, V token counts and V row totals.
np_baselines :: (c:NcConfig,train:[]int,val:[]int,lengths:[]int,windows:int,counts:[]f64)->NpBase {
    V:=c.vocab;L:=c.sequence_len;r:=NpBase.{};
    if counts.count<V*V+2*V || windows*L>val.count {r.status=NpStatus.data;return r;}
    U:=V*V;R:=U+V;for i:0..<V*V+2*V {counts[i]=0;}
    for i:0..<train.count {counts[U+train[i]]+=1;if i+1<train.count {counts[train[i]*V+train[i+1]]+=1;counts[R+train[i]]+=1;}}
    uni:=0.0;bi:[6]f64;bytes:=0;
    for k:0..<windows {for i:0..<L-1 {x:=val[k*L+i];y:=val[k*L+i+1];
        if lengths[y]>0 {bytes+=lengths[y];uni-=log((counts[U+y]+1)/(f64(train.count)+f64(V)));
            for j:0..<6 {a:=NP_K[j];bi[j]-=log((counts[x*V+y]+a)/(counts[R+x]+a*f64(V)));}}
    }}
    // Ties keep the larger k.
    d:=log(2.0)*f64(bytes);r.unigram=uni/d;r.bigram=1e308;
    for j:0..<6 {if bi[j]/d<r.bigram {r.bigram=bi[j]/d;r.k=NP_K[j];}}
    return r;
}
```

The baselines score the same targets from training counts alone. The
unigram baseline uses add-one smoothing. The bigram baseline uses add-k
smoothing for each k in 1, 0.3, 0.1, 0.03, 0.01 and 0.003 and keeps the best
k on the held-out targets. That choice looks at held-out data, so it favours
the baseline and makes the bar harder to clear. The hand check uses three
symbols. Training text `2 0 1 0 1 2 0` has token counts 3, 2 and 2 of 7, and
each symbol starts two pairs. The held-out windows `2 0 1` and `0 1 2` give
the targets 0 after 2, 1 after 0 and 1 after 0, worth 1, 2 and 2 bytes. The
unigram baseline is `(−ln 0.4 − 2 ln 0.3) / (5 ln 2)`. The bigram baseline
for each k is `−3 ln((2+k)/(2+3k)) / (5 ln 2)`, and the smallest k wins.

## The plan and its envelope

A plan names everything the run decides. Admission refuses a plan before
any work: a model chapter 125 does not admit, a window that is not the
whole context, a batch or step count out of range, a schedule longer than
the run, a rate outside (0, 1], an AdamW setting chapter 124 refuses, a
negative or infinite clip, fewer training windows than a batch, or an
estimated cost above the budget.

<!-- source: psionic/lib/nanochat/pretrain.coax plan -->
```jai
// Everything a run decides. model.sequence_len is the window length; every
// window is a full context. Rates are AdamW's, one per parameter group, and
// are scaled by the schedule: a linear warmup over warmup steps, then
// constant, then a linear warmdown over the last warmdown steps towards
// final times the rate. budget caps the run's estimated FLOPs.
NpPlan :: struct {model:NcConfig;batch:int;steps:int;warmup:int;warmdown:int;final:f64;
    embed:f64;matrix:f64;head:f64;scalar:f64;beta1:f64;beta2:f64;eps:f64;clip:f64;seed:u64;init:u64;budget:f64;}
```

<!-- source: psionic/lib/nanochat/pretrain.coax admit -->
```jai
np_flops :: (p:NpPlan)->f64 {return 6.0*f64(nc_parameter_count(p.model))*f64(p.steps)*f64(p.batch)*f64(p.model.sequence_len-1);}
np_unit :: (x:f64)->bool {return x>0 && x<=1;}
// Every refusal of a plan, before any work: the model, the window, the batch,
// the schedule, the rates, the optimizer, clipping, the data and the budget.
np_admit :: (p:NpPlan,windows:int)->NpStatus {
    c:=p.model;if !nc_valid(c) || c.context!=c.sequence_len {return NpStatus.config;}
    if p.batch<1 || p.batch>NP_BATCH || p.steps<1 || p.steps>NP_STEPS || p.warmup<0 || p.warmdown<0 || p.warmup+p.warmdown>p.steps {return NpStatus.config;}
    if !(p.final>=0 && p.final<=1) || !np_unit(p.embed) || !np_unit(p.matrix) || !np_unit(p.head) || !np_unit(p.scalar) {return NpStatus.config;}
    if !aw_beta(p.beta1) || !aw_beta(p.beta2) || !aw_eps(p.eps) || !(p.clip>=0) || !aw_finite(p.clip) {return NpStatus.config;}
    if windows<p.batch {return NpStatus.data;}
    if np_flops(p)>p.budget {return NpStatus.budget;}
    return NpStatus.ok;
}
```

The estimate is the usual 6ND: six FLOPs per parameter per predicted token,
two for the forward pass and four for the backward pass. The short run
below predicts `24 steps × 4 windows × 63 targets = 6,048` tokens, so its
estimate is `6 × 48,170 × 6,048 = 1,747,992,960` FLOPs, under its budget of
10^10. The recorded run predicts 504,000 tokens, `1.457 × 10^11` FLOPs,
under its budget of `2 × 10^11`. Halve the budget and admission refuses.

The schedule is nanochat `base_train`'s multiplier: `(s+1)/warmup` during
warmup, then 1, then over the last `warmdown` steps `f + (1 − f)·final` with
`f = (steps − s)/warmdown`. With six steps, warmup 2, warmdown 2 and final
0.5, steps 0 to 6 get 1/2, 1, 1, 1, 1, 3/4 and 1/2. The constant phase
returns 1 without reading `final`, which a check pins by setting `final` to
NaN. Five AdamW groups share the multiplier: the scalars, `wte`, every
block's matrices and gates, `lm_head`, and `smear_gate` with the value
embeddings. Decay is zero. nanochat itself trains the matrices with Muon;
that is not done here.

<!-- source: psionic/lib/nanochat/pretrain.coax schedule -->
```jai
// nanochat base_train's multiplier: (s+1)/warmup during warmup, 1, then
// over the last warmdown steps f + (1 - f) final with f = (steps - s)/warmdown.
np_scale :: (p:NpPlan,s:int)->f64 {
    if s<p.warmup {return f64(s+1)/f64(p.warmup);}
    if s+p.warmdown<=p.steps {return 1;}
    f:=f64(p.steps-s)/f64(p.warmdown);return f+(1-f)*p.final;
}
// The smaller of two integers, without a branch.
np_min :: (a:int,b:int)->int {return (a+b-cast(int)abs(f64(a-b)))/2;}
// Five AdamW groups in parameter order: the scalars, wte, every block's
// matrices and gates, lm_head, then smear_gate and the value embeddings.
np_groups :: (p:NpPlan,scale:f64,out:[]AwGroup) {
    c:=p.model;at:=[6]int.[0,nc_wte(c),nc_block(c,0).q,nc_head(c),nc_smear_gate(c),nc_parameter_count(c)];
    lr:=[5]f64.[p.scalar,p.embed,p.matrix,p.head,p.embed];
    for k:0..<5 {out[k]=AwGroup.{first=at[k],count=at[k+1]-at[k],lr=lr[k]*scale,beta1=p.beta1,beta2=p.beta2,eps=p.eps};}
}
```

Initialization draws one uniform number per flat index from chapter 103's
counter generator. Input matrices and value embeddings get standard
deviation `1/sqrt(32)`, `wte` gets 1 and `lm_head` 0.001, so the first loss
is close to `ln 320 = 5.768`. nanochat zeroes its output projections; this
run starts them at a tenth of the input scale instead, so the first gradient
reaches every matrix and the oracle comparison below covers them all. The
rule is this chapter's own. It is not claimed to match nanochat's
`init_weights` or torch's random numbers.

<!-- source: psionic/lib/nanochat/pretrain.coax init -->
```jai
// Uniform draws from chapter 103's counter generator, one per flat index:
// p = offset + scale (2u - 1). Input matrices and value embeddings take
// scale sqrt(3/width) (standard deviation 1/sqrt(width)), wte sqrt(3)
// (deviation 1), lm_head 0.001 sqrt(3). Output projections and gates start
// at a tenth of the input scale, not at zero, so the first gradient reaches
// every matrix. resid 1, x0 0.1, smear 0.1, backout 0.2.
np_init :: (c:NcConfig,seed:u64,p:[]f64) {
    inv:[NC_TENSORS_MAX]NcTensor;n:=nc_inventory(c,inv[..]);s:=sqrt(3.0/f64(c.width));
    for k:0..<n {t:=inv[k];scale:=s;offset:=0.0;
        if t.kind==NcKind.resid {scale=0;offset=1;}if t.kind==NcKind.x0 || t.kind==NcKind.smear_lambda {scale=0;offset=0.1;}
        if t.kind==NcKind.backout {scale=0;offset=0.2;}if t.kind==NcKind.wte {scale=sqrt(3.0);}if t.kind==NcKind.head {scale=0.001*sqrt(3.0);}
        if t.kind==NcKind.proj || t.kind==NcKind.mlp || t.kind==NcKind.gate || t.kind==NcKind.smear_gate {scale=0.1*s;}
        for e:0..<nc_elements(t) {i:=t.offset+e;u:=f64(cr_bits(seed,cast(u64)i)>>11)/9007199254740992.0;p[i]=offset+scale*(2*u-1);}
    }
}
```

## A step, and the order of windows

The training stream is cut into consecutive windows of 64 tokens: 3,109 of
them. Each epoch visits them in a seeded order: a Fisher-Yates shuffle whose
draws are counters `epoch × windows` onward. The order is a function of the
seed, the epoch and nothing else, so a resumed run recomputes it rather
than storing it. A step takes the next `batch` windows of that order, adds
each window's gradient divided by the batch, clips the global norm, scales
the rates and takes one AdamW step.

<!-- source: psionic/lib/nanochat/pretrain.coax step -->
```jai
// Epoch e's order of the training windows: Fisher-Yates from the last
// position down, counters e*windows onwards. order[windows] remembers the epoch.
np_window :: (p:NpPlan,windows:int,g:int,order:[]int)->int {
    epoch:=g/windows;
    if order[windows]!=epoch {
        for i:0..<windows {order[i]=i;}
        for r:0..<windows-1 {i:=windows-1-r;j:=cast(int)(cr_bits(p.seed,cast(u64)(epoch*windows+r))%cast(u64)(i+1));t:=order[i];order[i]=order[j];order[j]=t;}
        order[windows]=epoch;
    }
    return order[g%windows];
}
// One optimizer step: the mean loss and gradient over batch windows, the
// global norm, clipping, the scheduled rates and AdamW.
np_step :: (p:NpPlan,run:NpRun,train:[]int,order:[]int,w:[]f64,m:[]f64,v:[]f64,g:[]f64,total:[]f64,tape:[]f64,logits:[]f64)->NpStep {
    c:=p.model;L:=c.sequence_len;windows:=train.count/L;n:=w.count;fail:=NpStep.{run=run};
    if run.step>=p.steps {fail.status=NpStatus.config;return fail;}
    if windows<p.batch {fail.status=NpStatus.data;return fail;}
    for i:0..<n {total[i]=0;}loss:=0.0;k:=f64(p.batch);
    for j:0..<p.batch {at:=np_window(p,windows,run.step*p.batch+j,order)*L;
        r:=nb_gradient(c,w,train[at..<at+L],g,tape[0..<nb_tape_count(c,L)],logits[0..<L*c.vocab],false);
        if r.status!=NcStatus.ok {fail.status=NpStatus.data;return fail;}
        for i:0..<n {total[i]+=g[i]/k;}loss+=r.loss/k;
    }
    sum:=0.0;for x:total {sum+=x*x;}norm:=sqrt(sum);
    if p.clip>0 && norm>p.clip {for i:0..<n {total[i]=total[i]*p.clip/norm;}}
    scale:=np_scale(p,run.step);groups:[5]AwGroup;np_groups(p,scale,groups[..]);none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,w,total,m,v,none[0..<0],false);
    if a.status!=AwStatus.ok {fail.status=NpStatus.optimizer;return fail;}
    return NpStep.{run=NpRun.{step=run.step+1,state=a.state},loss=loss,norm=norm,scale=scale};
}
```

## Checkpoint, resume and the acceptance rule

The checkpoint is a little-endian record: a magic word and version, the
plan identity (a digest of every plan field), the data identity (a digest of
both token streams), the step, AdamW's counters, the parameter count, every
parameter, first moment and second moment as binary64 bits, and a digest of
everything before it. SHA-256 takes at most 1 MiB in one call, so the digest
is folded over 32 KiB blocks. Resume admits the whole record against the
plan and data it must belong to before it writes anything, and refuses a
changed byte, another plan, other data, a step the plan cannot reach, or a
value that is not finite (or a negative second moment).

<!-- source: psionic/lib/nanochat/pretrain.coax checkpoint -->
```jai
// The record: "PSPT" u16 version=1 u16 0; the plan identity and the data
// identity (32 bytes each); the step, AdamW's step and skip counters (u64);
// n (u32); n parameters, n first moments, n second moments (binary64 bits);
// then a digest of everything before it, folded over 32 KiB blocks.
np_plan_id :: (p:NpPlan)->[32]u8 {
    c:=p.model;w:=[22]u64.[cast,trunc(u64)c.sequence_len,cast,trunc(u64)c.vocab,cast,trunc(u64)c.layers,cast,trunc(u64)c.heads,cast,trunc(u64)c.kv_heads,cast,trunc(u64)c.width,
        cast(u64)c.pattern,cast,trunc(u64)c.pattern_len,cast,trunc(u64)c.context,cast,trunc(u64)p.batch,cast,trunc(u64)p.steps,cast,trunc(u64)p.warmup,cast,trunc(u64)p.warmdown,
        f64_bits(p.final),f64_bits(p.embed),f64_bits(p.matrix),f64_bits(p.head),f64_bits(p.scalar),f64_bits(p.beta1),f64_bits(p.beta2),f64_bits(p.eps),f64_bits(p.clip)];
    b:[200]u8;at:=0;for x:w {at=ck_put64(b[..],at,x);}at=ck_put64(b[..],at,p.seed);at=ck_put64(b[..],at,p.init);at=ck_put64(b[..],at,f64_bits(p.budget));
    return ck_digest("psionic.pretrain-plan.v1",b[0..<at]);
}
// Folds bytes into a running digest, 32 KiB at a time.
np_fold :: (d:[32]u8,bytes:[]u8)->[32]u8 {
    out:=d;at:=0;buf:[32800]u8;
    while at<bytes.count {k:=np_min(bytes.count-at,32768);for i:0..<32 {buf[i]=out[i];}for i:0..<k {buf[32+i]=bytes[at+i];}
        out=ck_digest("psionic.pretrain-fold.v1",buf[0..<32+k]);at+=k;}
    return out;
}
// The data identity: both streams, as u32 values, with their lengths.
np_data_id :: (train:[]int,val:[]int)->[32]u8 {
    d:[32]u8;head:[16]u8;h:=ck_put64(head[..],0,cast,trunc(u64)train.count);h=ck_put64(head[..],h,cast,trunc(u64)val.count);d=np_fold(d,head[0..<h]);
    buf:[32768]u8;
    for part:0..<2 {s:=train;if part==1 {s=val;}at:=0;
        while at<s.count {k:=np_min(s.count-at,8192);q:=0;for i:0..<k {q=ck_put32(buf[..],q,s[at+i]);}d=np_fold(d,buf[0..<q]);at+=k;}}
    return d;
}
np_sign :: (b:[]u8) {d:[32]u8;d=np_fold(d,b[0..<b.count-32]);for i:0..<32 {b[b.count-32+i]=d[i];}}
np_save :: (plan:[32]u8,data:[32]u8,run:NpRun,w:[]f64,m:[]f64,v:[]f64,out:[]u8)->NpCheck {
    n:=w.count;if m.count!=n || v.count!=n || out.count!=np_size(n) {return NpCheck.shape;}
    if run.state.step!=run.step || run.state.skipped!=0 {return NpCheck.step;}
    for i:0..<n {if !aw_finite(w[i]) || !aw_finite(m[i]) || !aw_finite(v[i]) || v[i]<0 {return NpCheck.number;}}
    out[0]=80;out[1]=83;out[2]=80;out[3]=84;out[4]=1;out[5]=0;out[6]=0;out[7]=0;
    at:=ck_put(out,8,plan);at=ck_put(out,at,data);at=ck_put64(out,at,cast,trunc(u64)run.step);
    at=ck_put64(out,at,cast,trunc(u64)run.state.step);at=ck_put64(out,at,cast,trunc(u64)run.state.skipped);at=ck_put32(out,at,n);
    for i:0..<n {at=ck_put64(out,at,f64_bits(w[i]));}for i:0..<n {at=ck_put64(out,at,f64_bits(m[i]));}for i:0..<n {at=ck_put64(out,at,f64_bits(v[i]));}
    np_sign(out);return NpCheck.ok;
}
// Admits the whole record against the plan and data it must belong to, then
// writes the values. A refusal writes nothing.
np_resume :: (p:NpPlan,plan:[32]u8,data:[32]u8,bytes:[]u8,w:[]f64,m:[]f64,v:[]f64)->NpRead {
    if w.count!=nc_parameter_count(p.model) {return NpRead.{status=NpCheck.shape};}
    return np_read(plan,data,p.steps,bytes,w,m,v);
}
// The record itself, for n = w.count values and a run of at most steps.
np_read :: (plan:[32]u8,data:[32]u8,steps:int,bytes:[]u8,w:[]f64,m:[]f64,v:[]f64)->NpRead {
    n:=w.count;if m.count!=n || v.count!=n || bytes.count!=np_size(n) {return NpRead.{status=NpCheck.shape};}
    if !ck_magic(bytes,80,84) {return NpRead.{status=NpCheck.magic};}
    if bytes[4]!=1 || bytes[5]!=0 || bytes[6]!=0 || bytes[7]!=0 {return NpRead.{status=NpCheck.version};}
    d:[32]u8;d=np_fold(d,bytes[0..<bytes.count-32]);if !ck_equal(d,ck_get(bytes,bytes.count-32)) {return NpRead.{status=NpCheck.corrupt};}
    if !ck_equal(ck_get(bytes,8),plan) {return NpRead.{status=NpCheck.plan};}
    if !ck_equal(ck_get(bytes,40),data) {return NpRead.{status=NpCheck.data};}
    step:=ck_get64(bytes,72);t:=ck_get64(bytes,80);skipped:=ck_get64(bytes,88);
    if step>cast(u64)steps || t!=step || skipped!=0 {return NpRead.{status=NpCheck.step};}
    if ck_get32(bytes,96)!=n {return NpRead.{status=NpCheck.shape};}
    for i:0..<3*n {if !ck_bits(ck_get64(bytes,NP_HEAD+8*i),i>=2*n) {return NpRead.{status=NpCheck.number};}}
    for i:0..<n {w[i]=f64_from_bits(ck_get64(bytes,NP_HEAD+8*i));m[i]=f64_from_bits(ck_get64(bytes,NP_HEAD+8*(n+i)));v[i]=f64_from_bits(ck_get64(bytes,NP_HEAD+8*(2*n+i)));}
    s:=cast(int)step;return NpRead.{run=NpRun.{step=s,state=AwState.{step=s}}};
}
```

The acceptance rule was written into the source before the first run:

<!-- source: psionic/lib/nanochat/pretrain.coax accept -->
```jai
// The acceptance rule, frozen before the first run: the model's held-out
// bits per byte must be at least NP_MARGIN below the best smoothed bigram's,
// on the same targets.
NP_MARGIN :: 0.1;
np_accept :: (model:f64,base:NpBase)->bool {return base.status==NpStatus.ok && model+NP_MARGIN<=base.bigram;}
```

## Run it

The driver holds the buffers, about 8 MiB, and prints. Every decision it
reports comes from `pretrain.coax`.

<!-- source: psionic/tests/pretrain-drive.coax drive -->
```jai
// Tokenizer, split, baselines, the run with evaluations and a checkpoint
// after step cut, then a resume from that checkpoint replayed to the end.
np_drive :: (plan:NpPlan,windows:int,every:int,report:int,cut:int,refusals:bool) {
    c:=plan.model;docs:=NP_CORPUS;sources:=NP_CORPUS_SOURCES;classes:=PT_TOY;packed:[65536]u8;scratch:[65536]u8;
    t:=np_tokenizer(classes[..],docs[..],sources[..],NP_MERGES,packed[..],scratch[..]);#assert t.status==NpStatus.ok && t.vocab==c.vocab;
    bos:=t.vocab-1;lengths:[320]int;#assert np_lengths(packed[0..<t.written],bos,lengths[..])==NpStatus.ok;
    hashes:[NP_PARAGRAPHS]u64;train:[NP_TRAIN_MAX]int;val:[NP_VAL_MAX]int;
    d:=np_data(packed[0..<t.written],docs[..],sources[..],bos,hashes[..],train[..],val[..]);#assert d.status==NpStatus.ok;
    tr:=train[0..<d.train];va:=val[0..<d.val];L:=c.sequence_len;
    // Chapter 127's oracle took its step-0 gradient over these first 16 tokens.
    for i:0..<16 {#assert tr[i]==NP_ORACLE_TOKENS[i];}
    print("corpus: % documents, % paragraphs; held out: % documents (sources divisible by %), shared paragraphs: %\n",docs.count,d.paragraphs,d.val_docs,NP_HOLDOUT,d.shared);
    print("tokenizer: % merges on % training samples, vocabulary % (BOS %)\n",t.merges,NP_SAMPLES,t.vocab,bos);
    print("train: % bytes, % tokens, % windows of %; held out: % bytes, % tokens, % frozen windows\n",d.train_bytes,d.train,d.train/L,L,d.val_bytes,d.val,windows);
    #assert np_admit(plan,d.train/L)==NpStatus.ok;
    n:=nc_parameter_count(c);#assert n==NP_PARAMETERS;
    print("envelope: % parameters, % steps x % windows x % targets, % FLOPs estimated (6ND) of % budgeted\n",n,plan.steps,plan.batch,L-1,np_flops(plan),plan.budget);
    counts:[103040]f64;base:=np_baselines(c,tr,va,lengths[..],windows,counts[..]);#assert base.status==NpStatus.ok;
    print("baselines, bits per byte: unigram %, bigram % (add-k, k = %)\n",base.unigram,base.bigram,base.k);
    w:[NP_PARAMETERS]f64;m:[NP_PARAMETERS]f64;v:[NP_PARAMETERS]f64;g:[NP_PARAMETERS]f64;total:[NP_PARAMETERS]f64;
    tape:[101040]f64;logits:[20480]f64;cache:[4128]f64;work:[12544]f64;order:[NP_TRAIN_MAX/64+1]int;order[d.train/L]=-1;
    np_init(c,plan.init,w[..]);
    e:=np_eval(c,w[..],va,lengths[..],windows,cache[..],work[..],logits[..]);#assert e.status==NpStatus.ok;
    print("step 0: held-out % bits per byte over % targets, % bytes\n",e.bpb,e.targets,e.bytes);
    pid:=np_plan_id(plan);did:=np_data_id(tr,va);bytes:[1156212]u8;#assert np_size(n)==bytes.count;
    run:=NpRun.{};losses:[1000]f64;
    while run.step<plan.steps {
        r:=np_step(plan,run,tr,order[0..<d.train/L+1],w[..],m[..],v[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;
        losses[run.step]=r.loss;run=r.run;
        if run.step%report==0 {print("step %: loss %, gradient norm %, rate scale %\n",run.step,r.loss,r.norm,r.scale);}
        if run.step%every==0 && run.step<plan.steps {e=np_eval(c,w[..],va,lengths[..],windows,cache[..],work[..],logits[..]);print("step %: held-out % bits per byte\n",run.step,e.bpb);}
        if run.step==cut {#assert np_save(pid,did,run,w[..],m[..],v[..],bytes[..])==NpCheck.ok;print("step %: checkpoint, % bytes, digest ",cut,bytes.count);np_hex(ck_get(bytes[..],bytes.count-32));print("...\n");}
    }
    e=np_eval(c,w[..],va,lengths[..],windows,cache[..],work[..],logits[..]);
    print("final: held-out % bits per byte; bigram % - margin % = %\n",e.bpb,base.bigram,NP_MARGIN,base.bigram-NP_MARGIN);
    print("acceptance (frozen before the run): %\n",ifx np_accept(e.bpb,base) then "passed" else "failed");
    // A fresh set of buffers holds only the checkpoint.
    x:[NP_PARAMETERS]f64;xm:[NP_PARAMETERS]f64;xv:[NP_PARAMETERS]f64;o2:[NP_TRAIN_MAX/64+1]int;o2[d.train/L]=-1;
    rd:=np_resume(plan,pid,did,bytes[..],x[..],xm[..],xv[..]);#assert rd.status==NpCheck.ok;again:=rd.run;same:=0;
    while again.step<plan.steps {r:=np_step(plan,again,tr,o2[0..<d.train/L+1],x[..],xm[..],xv[..],g[..],total[..],tape[..],logits[..]);#assert r.status==NpStatus.ok;
        if f64_bits(r.loss)==f64_bits(losses[again.step]) {same+=1;}again=r.run;}
    print("resumed at step %: % of % losses identical; parameters and moments identical: %\n",rd.run.step,same,plan.steps-rd.run.step,np_same(x[..],w[..]) && np_same(xm[..],m[..]) && np_same(xv[..],v[..]));
    if !refusals {return;}
    // Broken inputs, each refused before anything is written.
    bytes[5000]^=1;print("checkpoint with one bit flipped: %\n",np_resume(plan,pid,did,bytes[..],x[..],xm[..],xv[..]).status);bytes[5000]^=1;
    q:=plan;q.steps=plan.steps+1;print("checkpoint resumed under a %-step plan: %\n",q.steps,np_resume(q,np_plan_id(q),did,bytes[..],x[..],xm[..],xv[..]).status);
    print("checkpoint against other data: %\n",np_resume(plan,pid,np_data_id(tr[0..<d.train-1],va),bytes[..],x[..],xm[..],xv[..]).status);
    q=plan;q.budget=np_flops(plan)/2;print("the plan with half its FLOPs budgeted: %\n",np_admit(q,d.train/L));
    leaky:=docs;for i:0..<docs.count {if np_holdout(sources[i]) {leaky[i]=docs[0];break;}}
    print("a held-out document that repeats a training document: %, % shared paragraphs\n",np_audit(leaky[..],sources[..],hashes[..]).status,np_audit(leaky[..],sources[..],hashes[..]).shared);
}
```

[The short run](../examples/pretrain.coax) trains for 24 steps of four windows and evaluates on the first
16 held-out windows:

<!-- source: psionic/examples/pretrain.coax example -->
```jai
// The short run: four windows a step for 24 steps, evaluated on the first
// 16 frozen held-out windows, with a checkpoint after step 12.
NP_DEMO :: NpPlan.{model=NP_MODEL,batch=4,steps=24,warmup=4,warmdown=8,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1281,budget=1e10};
main :: () {np_drive(NP_DEMO,16,12,4,12,true);}
```

```shell
coax/bin/coax run psionic/examples/pretrain.coax
psionic/check.sh --only nanochat/pretrain
```

```text
corpus: 118 documents, 1243 paragraphs; held out: 17 documents (sources divisible by 8), shared paragraphs: 0
tokenizer: 63 merges on 16 training samples, vocabulary 320 (BOS 319)
train: 289292 bytes, 199039 tokens, 3109 windows of 64; held out: 49981 bytes, 34564 tokens, 16 frozen windows
envelope: 48170 parameters, 24 steps x 4 windows x 63 targets, 1747992960.0 FLOPs estimated (6ND) of 10000000000.0 budgeted
baselines, bits per byte: unigram 4.456933833293177, bigram 3.1062932989428584 (add-k, k = 0.03)
step 0: held-out 5.773199928683723 bits per byte over 1008 targets, 1453 bytes
step 4: loss 5.4614144186952025, gradient norm 0.6689967848387486, rate scale 1.0
step 8: loss 4.663260642159237, gradient norm 0.4759126980122564, rate scale 1.0
step 12: loss 4.547918215372782, gradient norm 0.39890044740763503, rate scale 1.0
step 12: held-out 4.523625132160234 bits per byte
step 12: checkpoint, 1156212 bytes, digest 0fcc5d728deb307e...
step 16: loss 4.466083101037292, gradient norm 0.41770122519190284, rate scale 1.0
step 20: loss 4.354102946996971, gradient norm 0.4123463332232608, rate scale 0.625
step 24: loss 4.443679361036116, gradient norm 0.336281782618377, rate scale 0.125
final: held-out 4.355480036494545 bits per byte; bigram 3.1062932989428584 - margin 0.1 = 3.0062932989428583
acceptance (frozen before the run): failed
resumed at step 12: 12 of 12 losses identical; parameters and moments identical: true
checkpoint with one bit flipped: corrupt
checkpoint resumed under a 25-step plan: plan
checkpoint against other data: data
the plan with half its FLOPs budgeted: budget
a held-out document that repeats a training document: leak, 25 shared paragraphs
```

The [transcript](../fixtures/pretrain.out) is exact for the named build in
debug and release. The model starts at 5.77 bits per byte, the cost of a
uniform guess over 320 tokens at 1.44 bytes a token, and reaches 4.36 after
24 steps. That is better than the unigram baseline and far from the bigram
one, so this short run fails the acceptance rule; it exists to show every
piece working in seconds. The checkpoint after step 12 resumes in fresh
buffers and replays steps 13 to 24 with every loss, parameter and moment
identical. The last five lines are deliberately broken inputs, each refused
before anything is written: a checkpoint with one bit flipped, a checkpoint
offered to a plan of another length, a checkpoint offered with other data,
the plan with half its budget, and a held-out document that copies a
training document.

## The recorded run

The recorded run takes eight windows a step for 1,000 steps, about two and
a half passes over the training windows, and evaluates on 256 frozen
held-out windows (16,121 targets that have bytes) every 200 steps. It runs in release only,
in about a minute on one core, and
[check_pretrain.sh](../tests/check_pretrain.sh) compares its transcript with
the [recorded one](../fixtures/pretrain/long.out).

<!-- source: psionic/tests/pretrain-long.coax long -->
```jai
NP_RUN :: NpPlan.{model=NP_MODEL,batch=8,steps=1000,warmup=40,warmdown=300,final=0,
    embed=0.03,matrix=0.01,head=0.01,scalar=0.01,beta1=0.8,beta2=0.95,eps=1e-10,clip=1,seed=7,init=1281,budget=2e11};
main :: () {np_drive(NP_RUN,256,200,100,900,false);}
```

```text
corpus: 118 documents, 1243 paragraphs; held out: 17 documents (sources divisible by 8), shared paragraphs: 0
tokenizer: 63 merges on 16 training samples, vocabulary 320 (BOS 319)
train: 289292 bytes, 199039 tokens, 3109 windows of 64; held out: 49981 bytes, 34564 tokens, 256 frozen windows
envelope: 48170 parameters, 1000 steps x 8 windows x 63 targets, 145666080000.0 FLOPs estimated (6ND) of 200000000000.0 budgeted
baselines, bits per byte: unigram 4.46390609867792, bigram 3.292520570496483 (add-k, k = 0.1)
step 0: held-out 5.781912815417212 bits per byte over 16121 targets, 23203 bytes
step 100: loss 3.446071417071761, gradient norm 0.3175075938849577, rate scale 1.0
step 200: loss 3.169007659595982, gradient norm 0.43116245776627504, rate scale 1.0
step 200: held-out 3.1510515795058938 bits per byte
step 300: loss 2.7822600753420876, gradient norm 0.4035268461584631, rate scale 1.0
step 400: loss 2.75291023678843, gradient norm 0.45865091659973856, rate scale 1.0
step 400: held-out 2.8748616326164655 bits per byte
step 500: loss 2.523662105171549, gradient norm 0.4484257496031922, rate scale 1.0
step 600: loss 2.4624008366734627, gradient norm 0.41631784183091153, rate scale 1.0
step 600: held-out 2.731868292983047 bits per byte
step 700: loss 2.613147751902321, gradient norm 0.4352549852372503, rate scale 1.0
step 800: loss 2.3486906707705373, gradient norm 0.4697451180367985, rate scale 0.67
step 800: held-out 2.623236897415898 bits per byte
step 900: loss 2.2465191327990532, gradient norm 0.4508071789593838, rate scale 0.33666666666666667
step 900: checkpoint, 1156212 bytes, digest 0b0f643fd36d293e...
step 1000: loss 2.3011427639236044, gradient norm 0.4448154986461925, rate scale 0.0033333333333333335
final: held-out 2.533604422311366 bits per byte; bigram 3.292520570496483 - margin 0.1 = 3.192520570496483
acceptance (frozen before the run): passed
resumed at step 900: 100 of 100 losses identical; parameters and moments identical: true
```

The rule asked for at least 0.1 bits per byte below the best bigram on the
same targets. The run ends at 2.534 bits per byte against the bigram's
3.293, and passes. The checkpoint after step 900 resumes and replays the
last 100 steps bit for bit.

These numbers measure this experiment: a 48,170-parameter model, a
339 KB corpus of one book's prose, and a held-out set of five of its
chapters. They say nothing about general language quality.

## Checks

The checks in `pretrain.coax` are independent of the code they test where it
matters:

- **Step 0 against chapter 127's oracle.**
  [pretrain_oracle.py](../tests/pretrain_oracle.py) rebuilds the initial
  weights from the written rule in plain Python and takes the loss and
  gradient of the first 16 training tokens through chapter 127's scalar
  reverse-mode oracle, which shares no code with Coax. Coax matches the loss
  within `1e-13` relative, 63 per-tensor gradient digests and every 97th
  gradient value within `1e-10`, and 24 initial weights exactly. The example
  asserts that its first 16 training tokens are the oracle's.
- **Bits per byte and the baselines by hand,** as worked above.
- **Identities and order.** The plan identity, the data identity, the folded
  digest over 70,000 bytes, FNV-1a and the first two epoch orders are
  recomputed in Python from their definitions and must match exactly.
- **Resume.** A run cut after three of six steps and continued from a copy
  of its state matches the uninterrupted run bit for bit. A three-value
  record is checked byte by byte, and every field change refuses without
  writing. Both examples round-trip a whole model through the record.
- **Admission and refusals** for each plan field just outside its range,
  for the split, the tokenizer, the streams, a step and the record.

[The oracle values](../fixtures/nanochat/pretrain-oracle.coax) and
[the corpus](../fixtures/pretrain/corpus.coax) regenerate exactly from
their scripts. Mutation of `pretrain.coax` leaves no survivors.

## What remains

- nanochat's MuonAdamW. Chapter 124 implements Muon, but this run trains
  every group with AdamW.
- nanochat's tokenizer size and data. This run's 320-token BPE learns from 4
  KiB of samples; nanochat's is 65,536 tokens over FineWeb.
- BOS-aligned best-fit packing. Windows here are consecutive slices of the
  stream and may cross document boundaries.
- The two-way budget comparison: a smaller model trained longer against a
  larger one trained for fewer steps, with each one's inference cost.
- Runs beyond the CPU through [chapter 129](ch129-train-a-whole-model-on-one-gpu.md),
  and any reproduction of nanochat's speedrun numbers. A larger run needs an
  owner-approved compute budget.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
