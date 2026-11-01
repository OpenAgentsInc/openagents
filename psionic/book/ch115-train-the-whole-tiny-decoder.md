# 115. Train the whole tiny decoder

Chapter 113 gives you a small causal decoder with checked gradients. This
chapter trains it end to end: a fixed corpus, a leak-free split, a packed and
seeded data stream, AdamW on a recorded schedule, held-out likelihood against
a baseline, checkpoints with exact resume, and seeded generation. The pinned
run stays in the repository as a regression for every later training change:
its transcript, two pinned steps and the identities a checkpoint carries are
all checked.

The runnable profile is `tiny-train-cpu-f64-v1`: binary64 on the CPU, one
thread, every sum in a fixed order, no pretrained weights and no GPU. The
library is [train.coax](../lib/transformer/train.coax). It reuses the
pieces other chapters built:

- [Chapter 113](ch113-differentiate-a-decoder.md): the decoder, its
  initialization and its gradients (`td_init`, `td_forward`, `td_decode`,
  `td_gradients`).
- [Chapter 120](ch120-data-and-resumable-state.md): the leak-free split
  (`sp_assign`, `sp_docs`).
- [Chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md): the packed
  data stream and its cursor (`pk_next`), the checkpoint record and its
  pointer (`ck_write`, `ck_read`, `ck_pointer_write`, `ck_current`).
- [Chapter 124](ch124-adamw-and-explicit-precision.md): AdamW (`aw_step`).
- [Chapter 53](ch053-choosing-the-next-token.md): the seeded sampler
  (`nx_step`).

[Chapter 119](ch119-change-an-activation-change-the-answer.md), still a
design draft, plans to start from this trained model.

## The corpus and its symbols

The corpus is 47 sentences of plain prose from chapters 112, 113 and 122 of
this book, in book order, kept in
[corpus.coax](../fixtures/tiny-train/corpus.coax). Sentences with code,
digits, symbols or non-ASCII characters were left out. It is the
repository's own text, so its licence is known.

Chapter 122's packer takes 15 content symbols and a boundary, so the text is
folded to 15 symbols, not 256 bytes. Letters fold to lower case. The 13 most
common English letters, `etaoinshrdlcu`, are symbols 1 to 13 in that order.
Every other letter is 14. Any run of other bytes (spaces, punctuation) is one
separator, 0. Symbol 15 is the boundary (BOS) that only the packer writes.

<!-- source: psionic/lib/transformer/train.coax symbol -->
```jai
// Letters fold to lower case. The 13 most common English letters are
// symbols 1 to 13 in order, every other letter is 14, and any run of other
// bytes (spaces, punctuation) is one separator, 0. Symbol 15 is chapter
// 122's BOS, which only the packer writes.
tt_symbol :: (c:int)->int {
    // Setting bit 5 lowers A-Z to a-z and sends no other byte into a-z.
    x:=c|32;if x<97 || x>122 {return 0;}
    for i:0..<13 {if byte_at(TT_LETTERS,i)==x {return i+1;}}
    return 14;
}
// Appends the symbols of text at tokens[at..]; a separator run collapses to
// one and a leading separator is dropped. Returns the new end.
tt_fold :: (text:string,tokens:[]u8,at:int)->int {
    end:=at;prev:=0;
    for i:0..<text.count {t:=tt_symbol(byte_at(text,i));if t!=0 || prev!=0 {tokens[end]=cast(u8)t;end+=1;}prev=t;}
    return end;
}
```

Fold ` The cat,  ran. ` by hand. The leading space is dropped. `T` and `t` are
2, `h` is 8, `e` is 1. The space after `The` is 0. `c` is 12, `a` is 3, `t`
is 2. The comma and two spaces are one 0. `r` is 9, `a` is 3, `n` is 6. The
final period and space are one 0:

```text
t h e _ c  a t _ r a n _
2 8 1 0 12 3 2 0 9 3 6 0
```

That is twelve symbols, which a check confirms. The whole corpus folds to
4,080 symbols, under the packer's limit of 4,096.

## A split that cannot leak

Each sentence is one row. Its time is its place in the book, and it is its
own task and session. Chapter 120's split joins rows that share a task, a
session or at least half their bigrams, and gives each group one split from
its latest time. Here six pairs of sentences share at least half their
bigrams. They join rows 0 and 6 into one group and rows 8, 24, 27, 30, 32 and
36 into another. Both groups end before time 38, so neither crosses the
boundary: rows with time up to 37 train and rows 38 to 46 are held out, 38
sentences and 9. The audit counts zero cross-split pairs of every kind.

<!-- source: psionic/lib/transformer/train.coax corpus -->
```jai
// One row per sentence. Each sentence is its own task and session, its time
// is its place in the book, and its licence is the repository's own (1).
tt_corpus :: (rows:[]SpRow,tokens:[]u8)->int {
    at:=0;
    for i:0..<TT_ROWS {end:=tt_fold(TT_CORPUS[i],tokens,at);rows[i]=SpRow.{id=i+1,licence=1,task=i,session=i,time=i,outcome=i,first=at,count=end-at};at=end;}
    return at;
}
// Chapter 120's split by time with whole groups, audited, as packed documents.
tt_docs :: (rows:[]SpRow,tokens:[]u8,docs:[]PkDoc)->SpStatus {
    splits:[64]PkSplit;s:=sp_assign(rows,tokens,TT_PLAN,splits[0..<rows.count]);if s!=SpStatus.ok {return s;}
    return sp_docs(rows,tokens,splits[0..<rows.count],docs);
}
// The documents of one split, each as BOS then its symbols, in ID order.
tt_stream :: (docs:[]PkDoc,tokens:[]u8,split:PkSplit,out:[]int)->int {
    n:=0;for d:docs {if d.split==split {out[n]=PK_BOS;n+=1;for i:0..<d.count {out[n]=cast(int)tokens[d.first+i];n+=1;}}}
    return n;
}
```

Each split becomes one stream: every document as BOS then its symbols. The
training stream has 3,399 symbols and the held-out stream 728.

## The baseline, frozen first

The decoder has to beat something simple. The baseline is a bigram counted
on the training stream with add-one smoothing:
`P(b|a) = (C[a,b] + 1) / (C[a] + 16)`, where `C[a,b]` counts `a` followed by
`b` and `C[a]` is the row sum.

<!-- source: psionic/lib/transformer/train.coax bigram -->
```jai
// The baseline: add-one bigram counts over the training stream, scored as
// mean negative log probability of every held-out transition.
tt_bigram :: (train:[]int,counts:[]int) {
    for i:0..<256 {counts[i]=0;}for t:1..<train.count {counts[train[t-1]*16+train[t]]+=1;}
}
tt_bigram_loss :: (counts:[]int,held:[]int)->f64 {
    total:=0.0;
    for t:1..<held.count {a:=held[t-1];row:=0;for j:0..<16 {row+=counts[a*16+j];}total-=log(f64(counts[a*16+held[t]]+1)/f64(row+16));}
    return total/f64(held.count-1);
}
```

One transition by hand: `t` is followed by `h` 52 times in training and `t`
is followed by something 281 times. So `P(h|t) = 53 / 297 = 0.17845`, and
that transition costs `-ln 0.17845 = 1.7234` nats. A check computes the same
number through `tt_bigram_loss`. Averaged over all 727 held-out transitions,
the bigram scores 2.245995 nats per symbol (3.2403 bits). Predicting all 16
symbols equally would cost `ln 16 = 2.7726` nats.

The acceptance criterion was written into the library before the first run
and is not tuned: the decoder's held-out cross entropy must be at least 0.05
nats per symbol below the bigram's.

<!-- source: psionic/lib/transformer/train.coax config -->
```jai
TT_SHAPE :: TdShape.{vocab=16,width=16,heads=2,kv_heads=1,head_width=8,hidden=16,layers=2,context=32};
TT_PARAMETERS :: 3152;
TT_WORK :: 13984;
TT_BATCH :: 4;
TT_STEPS :: 400;
TT_RATE :: 0.01;
TT_INIT :: 115;
TT_ORDER :: 7;
TT_PLAN :: SpPlan.{train_end=37,valid_end=46};
// Frozen before the first run: the decoder's held-out cross entropy must be
// at least this many nats per token below the add-one bigram's.
TT_MARGIN :: 0.05;
```

## The model and the held-out score

The model is chapter 113's decoder at a width of 16: two blocks, two query
heads of width 8 sharing one key/value head, a squared-ReLU feed-forward
network of 16 units and a context of 32. Count the parameters. The embedding
and the head are `2 * 16 * 16 = 512`. One block has two norms (32), `wq`
(`16 * 16 = 256`), `wk` and `wv` (`8 * 16` each, 256), `wo` (256), and `w1`
and `w2` (`16 * 16` each, 512): 1,312. Two blocks and the final norm give
`512 + 2 * 1312 + 16 = 3152`.

The decoder sees at most 32 symbols, and the held-out stream is longer. It is
scored in windows of 32 that overlap by one symbol, so every transition the
bigram scores is predicted exactly once and the two numbers are comparable.
Each window predicts 31 transitions. For 727 transitions, 23 full windows
cover 713 and a 24th window of 15 symbols covers the last 14.

<!-- source: psionic/lib/transformer/train.coax held -->
```jai
// The decoder's held-out loss: windows of the context length that overlap by
// one token, so every transition the bigram scores is predicted exactly once.
tt_held_loss :: (s:TdShape,p:[]f64,held:[]int,w:[]f64)->f64 {
    total:=0.0;first:=0;logits:[512]f64;
    while first+1<held.count {n:=nx_min(held.count-first,s.context);
        r:=td_forward(s,p,held[first..<first+n],w,logits[0..<n*s.vocab],false);#assert r.status==TdStatus.ok;
        total+=r.loss*f64(n-1);first+=n-1;}
    return total/f64(held.count-1);
}
```

A second route computes the same number one token at a time through the
key/value cache, with the log-sum-exp written out separately. On the checks'
small model (context 8), the two agree within 1e-12 on the first 9 and 20
held-out symbols, which end in a partial window, and on all 728.

## One training step

A step reads four packed rows of the context length (32 symbols) from the
cursor, averages their
decoder gradients and applies one AdamW update. The rate falls linearly from
0.01 at step 0 to a tenth of that at the last step: step 200 of 400 uses
`0.01 * (1 - 0.9 * 200 / 400) = 0.0055`, and step 399 uses 0.0010225. The rate
is a function of the step count in the optimizer state, so a resumed run
picks it up without storing anything new.

<!-- source: psionic/lib/transformer/train.coax step -->
```jai
// The rate falls linearly from TT_RATE at step 0 to a tenth of it at `steps`.
tt_rate :: (step:int,steps:int)->f64 {return TT_RATE*(1-0.9*f64(step)/f64(steps));}
// One step: TT_BATCH packed rows of the context length from the cursor, the
// mean of their decoder gradients, one AdamW update at the scheduled rate.
// Nothing is updated unless every row and the optimizer are admitted.
tt_step :: (s:TdShape,docs:[]PkDoc,tokens:[]u8,run:RsRun,steps:int,p:[]f64,m:[]f64,v:[]f64,w:[]f64,dw:[]f64,grads:[]f64,total:[]f64)->TtStep {
    c:=run.cursor;loss:=0.0;row:[64]u8;ints:[64]int;n:=s.context;
    for i:0..<p.count {total[i]=0;}
    for b:0..<TT_BATCH {
        next:=pk_next(docs,tokens,c,row[0..<n],false);if next.status!=PkStatus.ok {return TtStep.{status=TtStatus.data,run=run};}c=next.cursor;
        for t:0..<n {ints[t]=cast(int)row[t];}
        r:=td_gradients(s,p,ints[0..<n],w,dw,grads,false);if r.status!=TdStatus.ok {return TtStep.{status=TtStatus.model,run=run};}
        loss+=r.loss/TT_BATCH;for i:0..<p.count {total[i]+=grads[i]/TT_BATCH;}
    }
    groups:=[1]AwGroup.[AwGroup.{first=0,count=p.count,lr=tt_rate(run.state.step,steps),beta1=0.9,beta2=0.99,eps=0.00000001,decay=0}];none:[1]f32;
    a:=aw_step(groups[..],AwPolicy.{},run.state,p,total,m,v,none[0..<0],false);if a.status!=AwStatus.ok {return TtStep.{status=TtStatus.optimizer,run=run};}
    return TtStep.{run=RsRun.{cursor=c,state=a.state,revision=run.revision+1},loss=loss};
}
```

A step that cannot run changes nothing. A cursor past the split refuses as
`data` before any row is read. A parameter outside the decoder's admitted
range refuses as `model`. An optimizer state AdamW cannot hold refuses as
`optimizer`. In each case the parameters and both moments stay as they were.

## Checkpoints and resume

A checkpoint is chapter 122's record: identities, the step count, the data
cursor, the optimizer state, then the parameters and both moments as
binary64. For 3,152 parameters it is `228 + 24 * 3152 + 32 = 75908` bytes.
The identities name what the run is, so a record resumes only into the same
experiment: the model's shape and initialization seed, the tokenizer, the
packed dataset, and the optimizer with its schedule, batch and number of
steps. Rows are always the model's context long.

<!-- source: psionic/lib/transformer/train.coax ids -->
```jai
tt_text :: (tag:string,s:string,extra:int)->[32]u8 {
    b:[256]u8;for i:0..<s.count {b[i]=cast(u8)byte_at(s,i);}at:=ck_put64(b[..],s.count,cast(u64)extra);return ck_digest(tag,b[0..<at]);
}
// The model: every shape field and the initialization seed, as u64 words.
tt_model :: (s:TdShape)->[32]u8 {
    b:[72]u8;at:=0;for x:[9]int.[s.vocab,s.width,s.heads,s.kv_heads,s.head_width,s.hidden,s.layers,s.context,TT_INIT] {at=ck_put64(b[..],at,cast(u64)x);}
    return ck_digest("psionic.model.v1",b[0..<at]);
}
// What a checkpoint must match to resume: the model and its initialization,
// the tokenizer, the packed dataset, and the optimizer with its schedule and
// batch for a run of `steps` steps (rows are the model's context long).
tt_ids :: (s:TdShape,docs:[]PkDoc,tokens:[]u8,steps:int)->CkIds {
    d:[32]u8;k:=pk_identity(docs,tokens,d[..]);#assert k==PkStatus.ok;
    return CkIds.{model=tt_model(s),tokenizer=tt_text("psionic.tokenizer.v1","fold etaoinshrdlcu=1..13 other letters=14 separator runs=0 BOS=15",16),dataset=d,
        optimizer=tt_text("psionic.optimizer.v1","adamw beta1=0.9 beta2=0.99 eps=1e-8 decay=0 rate=0.01 linear to 0.001 batch=4 rows=context steps",steps)};
}
```

The record ends with its own SHA-256, and that digest is what the pointer
names. Resume checks that the buffers fit a record, reads the pointer, admits
the whole record against the
expected identities, checks that the pointer is current and that the cursor
names a document of this split, and only then writes the values.

<!-- source: psionic/lib/transformer/train.coax save -->
```jai
// Writes the whole run as one checkpoint and the pointer that names it.
tt_save :: (ids:CkIds,run:RsRun,p:[]f64,m:[]f64,v:[]f64,bytes:[]u8,pointer:[]u8)->CkStatus {
    s:=ck_write(CkMeta.{ids=ids,revision=run.revision,cursor=run.cursor,state=run.state},p,m,v,bytes,false);if s!=CkStatus.ok {return s;}
    // The record ends with its own digest, which is what the pointer names.
    return ck_pointer_write(run.revision,ck_get(bytes,bytes.count-32),pointer);
}
// Restores a run into p, m and v from a checkpoint whose pointer is current.
// Every check passes before anything is written.
tt_resume :: (pointer:[]u8,bytes:[]u8,ids:CkIds,docs:[]PkDoc,p:[]f64,m:[]f64,v:[]f64)->CkRead {
    if p.count>CK_VALUES || m.count>CK_VALUES || v.count>CK_VALUES {return CkRead.{status=CkStatus.shape};}
    q:=ck_pointer_read(pointer);if q.status!=CkStatus.ok {return CkRead.{status=q.status};}
    sp:[4096]f64;sm:[4096]f64;sv:[4096]f64;r:=ck_read(bytes,ids,sp[0..<p.count],sm[0..<m.count],sv[0..<v.count],false);
    s:=ck_current(q,r);if s!=CkStatus.ok {return CkRead.{status=s};}
    if !pk_cursor(docs,r.meta.cursor) {return CkRead.{status=CkStatus.corrupt};}
    for i:0..<p.count {p[i]=sp[i];m[i]=sm[i];v[i]=sv[i];}return r;
}
```

## Seeded generation

Generation starts from BOS and feeds one symbol at a time through the
key/value cache. Chapter 53's sampler draws each symbol from the logits at a
temperature, with one counter of the named RNG per draw. It stops at the
first BOS or after 31 symbols, when the context is full. The same seed gives
the same symbols.

<!-- source: psionic/lib/transformer/train.coax generate -->
```jai
// Seeded generation: BOS, then up to context-1 symbols drawn by chapter 53's
// sampler at the given temperature, stopping at the first BOS. Returns the
// number of symbols written to out (the BOS is not counted).
tt_generate :: (s:TdShape,p:[]f64,cache:[]f64,seed:u64,temperature:f64,out:[]int)->int {
    logits:[16]f64;narrow:[16]f32;probs:[16]f64;st:[16]f64;o:[16]int;tm:[16]int;none:[1]u8;stops:=[1]int.[PK_BOS];
    run:=NxRun.{random=CrSeed.{seed=seed},limit=s.context-1};token:=PK_BOS;n:=0;
    while run.stop==NxStop.running {
        r:=td_decode(s,p,cache,n,token,logits[..],false);#assert r.status==TdStatus.ok;
        for v:0..<16 {narrow[v]=cast(f32)logits[v];}
        e:=nx_step(narrow[..],none[0..<0],NxOptions.{temperature=temperature},run,stops[..],false,probs[..],nx_work(st[..],o[..],tm[..]),false);#assert e.status==NxStatus.ok;
        run=e.run;token=e.token;if token!=PK_BOS {out[n]=token;n+=1;}
    }
    return n;
}
```

## The run

[The example](../examples/tiny-train.coax) runs the pinned 400 steps. It
writes a checkpoint after step 350, then reads it back into fresh buffers,
as a new process would, and runs steps 351 to 400 again. It then shows two
broken inputs: the record resumed as a 399-step run, and a pointer with one
byte changed.

<!-- source: psionic/examples/tiny-train.coax example -->
```jai
main :: () {
    s:=TT_SHAPE;rows:[47]SpRow;tokens:[4096]u8;n:=tt_corpus(rows[..],tokens[..]);docs:[47]PkDoc;
    #assert tt_docs(rows[..],tokens[0..<n],docs[..])==SpStatus.ok;
    train:[4096]int;held:[4096]int;nt:=tt_stream(docs[..],tokens[0..<n],PkSplit.train,train[..]);nh:=tt_stream(docs[..],tokens[0..<n],PkSplit.validation,held[..]);
    print("corpus: % sentences, % symbols; split by time: % train, % held out\n",TT_ROWS,n,pk_trains(docs[..]),TT_ROWS-pk_trains(docs[..]));
    print("streams: train % symbols, held-out % (% transitions)\n",nt,nh,nh-1);
    counts:[256]int;tt_bigram(train[0..<nt],counts[..]);bigram:=tt_bigram_loss(counts[..],held[0..<nh]);
    print("bigram baseline: held-out % nats per symbol\n",bigram);
    // The pinned run, with a checkpoint written after step 350.
    p:[3152]f64;m:[3152]f64;v:[3152]f64;w:[13984]f64;dw:[13984]f64;g:[3152]f64;total:[3152]f64;
    seed:=td_init(s,CrSeed.{seed=TT_INIT},p[..]);ids:=tt_ids(s,docs[..],tokens[0..<n],TT_STEPS);
    print("model: % parameters; held-out at initialization %\n",td_parameter_count(s),tt_held_loss(s,p[..],held[0..<nh],w[..]));
    run:=RsRun.{cursor=PkCursor.{seed=TT_ORDER}};losses:[400]f64;bytes:[75908]u8;pointer:[80]u8;
    for k:0..<TT_STEPS {
        r:=tt_step(s,docs[..],tokens[0..<n],run,TT_STEPS,p[..],m[..],v[..],w[..],dw[..],g[..],total[..]);#assert r.status==TtStatus.ok;run=r.run;losses[k]=r.loss;
        if k==349 {#assert tt_save(ids,run,p[..],m[..],v[..],bytes[..],pointer[..])==CkStatus.ok;}
        if (k+1)%50==0 {print("step %: train loss %",k+1,r.loss);if (k+1)%100==0 {print(", held-out %",tt_held_loss(s,p[..],held[0..<nh],w[..]));}print("\n");}
    }
    decoder:=tt_held_loss(s,p[..],held[0..<nh],w[..]);
    print("held-out: decoder %, bigram %, decoder better by % (frozen margin %)\n",decoder,bigram,bigram-decoder,TT_MARGIN);
    #assert bigram-decoder>=TT_MARGIN;
    print("weights: sha256 ");tt_hex(tt_digest(p[..]));print("...\n");
    // A fresh process would hold only the record and its pointer.
    rp:[3152]f64;rm:[3152]f64;rv:[3152]f64;q:=tt_resume(pointer[..],bytes[..],ids,docs[..],rp[..],rm[..],rv[..]);#assert q.status==CkStatus.ok;
    again:=RsRun.{cursor=q.meta.cursor,state=q.meta.state,revision=q.meta.revision};same:=0;
    for k:350..<TT_STEPS {r:=tt_step(s,docs[..],tokens[0..<n],again,TT_STEPS,rp[..],rm[..],rv[..],w[..],dw[..],g[..],total[..]);#assert r.status==TtStatus.ok;again=r.run;if r.loss==losses[k] {same+=1;}}
    print("resumed from step % (% bytes): % of 50 losses identical, weights and moments identical: %\n",q.meta.revision,bytes.count,same,tt_same(rp[..],p[..]) && tt_same(rm[..],m[..]) && tt_same(rv[..],v[..]));
    // Broken inputs: a record from a run of another length, and a damaged pointer.
    other:=tt_ids(s,docs[..],tokens[0..<n],TT_STEPS-1);print("resume as a 399-step run: %\n",tt_resume(pointer[..],bytes[..],other,docs[..],rp[..],rm[..],rv[..]).status);
    pointer[8]^=1;print("resume with one pointer byte changed: %\n",tt_resume(pointer[..],bytes[..],ids,docs[..],rp[..],rm[..],rv[..]).status);
    out:[32]int;repeat:[32]int;
    for x:1..<4 {c:=tt_generate(s,p[..],w[..],cast(u64)x,0.8,out[..]);print("seed % at 0.8: ",x);tt_show(out[..],c);print("\n");}
    a:=tt_generate(s,p[..],w[..],1,0.8,out[..]);b:=tt_generate(s,p[..],w[..],1,0.8,repeat[..]);ok:=a==b;for i:0..<a {ok=ok && out[i]==repeat[i];}
    print("seed 1 again: identical %\n",ok);
}
```

```shell
coax/bin/coax run psionic/examples/tiny-train.coax
psionic/check.sh --only transformer/train
```

The [transcript](../fixtures/tiny-train.out) is exact, in debug and in
release:

```text
corpus: 47 sentences, 4080 symbols; split by time: 38 train, 9 held out
streams: train 3399 symbols, held-out 728 (727 transitions)
bigram baseline: held-out 2.245995271590946 nats per symbol
model: 3152 parameters; held-out at initialization 2.912052180237498
step 50: train loss 2.3691303849935768
step 100: train loss 2.215522050643217, held-out 2.2383164539522182
step 150: train loss 2.1182495317068373
step 200: train loss 2.00755062928158, held-out 2.181159394918942
step 250: train loss 1.9534208094123406
step 300: train loss 1.862617216160598, held-out 2.136476928998962
step 350: train loss 1.8467996869585566
step 400: train loss 1.8767169022663621, held-out 2.1320579221135967
held-out: decoder 2.1320579221135967, bigram 2.245995271590946, decoder better by 0.11393734947734924 (frozen margin 0.05)
weights: sha256 f166553e852669ed...
resumed from step 350 (75908 bytes): 50 of 50 losses identical, weights and moments identical: true
resume as a 399-step run: optimizer
resume with one pointer byte changed: corrupt
seed 1 at 0.8: iducosee_*ad_resersit_rer_ra_
seed 2 at 0.8: id_*and_aliniont_non_re**s_cer*
seed 3 at 0.8: the_arecoderaiset_alllitil_thal
seed 1 again: identical true
```

Symbol `_` is the separator and `*` is any letter outside the thirteen.

The decoder ends at 2.132058 nats per symbol on held-out text (3.0759 bits),
0.1139 below the bigram's 2.245995: more than twice the frozen margin. It
fits the training rows better than the held-out ones (1.88 against 2.13 at
the end), as expected with 3,152 parameters and 3,399 training symbols. The
resumed run's 50 losses and its final weights and moments are identical to
the uninterrupted run, bit for bit. The 399-step identity refuses as
`optimizer` and the changed pointer as `corrupt`, and neither writes
anything. The samples are not English, but they use its pieces: `the`,
`and`, `re`, `er`, separators between short words.

## Checks

The checks in [train.coax](../lib/transformer/train.coax) cover:

- **Independent reference.** [tiny_train_oracle.py](../tests/tiny_train_oracle.py)
  reads the same sentences, folds them, groups and splits them by chapter
  120's rules, builds the streams and scores the add-one bigram, written
  apart from the Coax. Coax matches its symbol counts, row lengths, first
  symbols, groups, split, stream lengths, the counts `C[t,h] = 52` and `C[t] = 281`,
  and its held-out bigram loss within 1e-12.
- **Hand values.** The folding of ` The cat,  ran. `, the class of every
  byte 0 to 255, the single transition `-ln(53/297)`, the rate at steps 0,
  200 and 400, and the parameter, workspace and record sizes.
- **Two routes to held-out loss.** The windowed full-sequence score equals
  the one-token-at-a-time score with an unshifted log-sum-exp.
- **Reproducibility.** The training checks use a 220-parameter shape
  (width 4, one block, context 8) so that they train, save and resume in
  under a second; the example covers the full shape. Three steps of a
  three-step run give pinned losses, a pinned weight digest and a pinned
  cursor. The gradient accumulator starts dirty, so a step that forgot to
  clear what it sums would fail.
- **Exact resume.** A run stopped after two steps writes its record and
  pointer, resumes into fresh buffers, and finishes with every parameter and
  moment identical to the run that never stopped.
- **Refusals.** Resume refuses another run length (`optimizer`), a changed
  corpus (`dataset`), another shape (`model`), buffers of the wrong size
  (`shape`), a truncated or changed pointer, and a signed record whose cursor
  is outside the split (`corrupt`), each leaving the buffers untouched. Steps
  refuse bad data, parameters and optimizer state without writing.
- **Generation.** Seed 3 from the initialization gives a pinned 21 symbols
  that end at BOS. The same seed repeats, another seed differs, and greedy
  decoding from the initialization runs to the 31-symbol limit.
- **Pinned identities.** Changing any one of the eight shape fields changes
  the model identity. The model, tokenizer and optimizer identities of the
  pinned run match recorded bytes, so a change to any of them is seen.

`coax mutate --root-only` on the library leaves no surviving mutant.

## Limits

- The vocabulary is a 15-symbol fold of the text, not chapter 112's 256
  bytes or chapter 123's learned tokenizer, because chapter 122's packer and
  split are written for 16 symbols.
- The corpus is 47 sentences. These numbers measure this experiment, not
  language modelling in general.
- One CPU thread and binary64. The profile promises the same bits for the
  same build of this program, not across other orders of summation.
