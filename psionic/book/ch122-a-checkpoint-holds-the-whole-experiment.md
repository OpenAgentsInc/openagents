# 122. A checkpoint holds the whole experiment

A training run that stops must be able to continue as if it had not. That
needs more than the parameters: the optimizer's moments and step count, where
the data stream was, and the random choices that ordered it. It also needs to
know it is resuming the same experiment, so a checkpoint names the model,
tokenizer, dataset and optimizer settings it belongs to and refuses any other.

The runnable profile is `psionic-resume-v1`, CPU only and deliberately small:
a 16 by 16 bigram table trained with [chapter 124's AdamW](ch124-adamw-and-explicit-precision.md)
on packed rows of eight tokens. Schedulers, compilation identity and restoring
into a different shard layout (chapter 135) are not part of it yet.

## Documents, splits and packing

A dataset is a list of documents, each with an ID, a split (train, validation
or test) and its tokens. Admission requires IDs in ascending order, spans that
cover the tokens exactly, content tokens 0–14 and at least one training
document. Only training documents are packed. Each document starts with the
boundary token 15, and the trainer never predicts a boundary, so no target
crosses from one document into the next.

Each epoch orders the training documents with a Fisher–Yates shuffle whose
draws are `splitmix64-top53-v1` at counters `epoch*64+i` for the run's seed.
The order is a function of (seed, epoch) alone, so the whole data position is
a four-field cursor: seed, epoch, document within the epoch, and offset within
the document. Rows can be cut at any length; the stream is the same.

<!-- source: psionic/lib/data/pack.coax next -->
```jai
// Fills row with the next tokens of the packed stream and returns the cursor
// after them. Nothing is written unless the dataset and cursor are admitted.
pk_next :: (docs:[]PkDoc,tokens:[]u8,cursor:PkCursor,row:[]u8,overlap:bool)->PkNext #export_c {
    if overlap {return PkNext.{status=PkStatus.alias,cursor=cursor};}
    status:=pk_dataset(docs,tokens);if status!=PkStatus.ok {return PkNext.{status=status,cursor=cursor};}
    if row.count<2 || row.count>PK_ROW {return PkNext.{status=PkStatus.shape,cursor=cursor};}
    if !pk_cursor(docs,cursor) {return PkNext.{status=PkStatus.cursor,cursor=cursor};}
    order:[64]int;n:=pk_trains(docs);pk_order(docs,cursor.seed,cursor.epoch,order[..]);stage:[256]u8;c:=cursor;bos:=0;
    for k:0..<row.count {
        d:=docs[order[c.order]];
        if c.offset==0 {stage[k]=PK_BOS;bos+=1;} else {stage[k]=tokens[d.first+c.offset-1];}
        c.offset+=1;
        if c.offset>d.count {c.offset=0;c.order+=1;if c.order==n {c.order=0;c.epoch+=1;if c.epoch==PK_EPOCHS {return PkNext.{status=PkStatus.cursor,cursor=cursor};}pk_order(docs,c.seed,c.epoch,order[..]);}}
    }
    for k:0..<row.count {row[k]=stage[k];}
    return PkNext.{cursor=c,boundaries=bos};
}
```

The dataset's identity is SHA-256 over every document's ID, split, length and
tokens. Changing any of them changes the identity.

## The record

A checkpoint is an explicit little-endian byte record, never a C struct: the
magic `PSCK`, a version, the four identities and a parent digest, the revision
(training steps taken), the data cursor, the optimizer's step and skip counts,
the count of values, then every parameter and both moments as binary64 bits,
and a SHA-256 of all of it. A second 80-byte record, the pointer, names the
checkpoint to resume from by revision and digest and carries its own digest.

Reading admits the whole record before writing any value: length, magic,
version, count, digest, then each identity against the run's (a wrong model,
tokenizer, dataset or optimizer each has its own refusal), the integer fields,
and finite values with non-negative second moments.

<!-- source: psionic/lib/checkpoint/record.coax read -->
```jai
// Admits a whole record against the identities the run expects, then writes
// the values. Refusals write nothing.
ck_read :: (bytes:[]u8,expect:CkIds,params:[]f64,m:[]f64,v:[]f64,overlap:bool)->CkRead #export_c {
    if overlap {return CkRead.{status=CkStatus.alias};}
    if bytes.count<ck_size(0) {return CkRead.{status=CkStatus.truncated};}
    if !ck_magic(bytes,67,75) {return CkRead.{status=CkStatus.magic};}
    if bytes[4]!=1 || bytes[5]!=0 || bytes[6]!=4 || bytes[7]!=0 {return CkRead.{status=CkStatus.version};}
    n:=ck_get32(bytes,CK_HEAD-4);if n<1 || n>CK_VALUES {return CkRead.{status=CkStatus.corrupt};}
    if bytes.count<ck_size(n) {return CkRead.{status=CkStatus.truncated};}
    if bytes.count>ck_size(n) {return CkRead.{status=CkStatus.corrupt};}
    body:=ck_size(n)-32;d:=ck_digest("psionic.checkpoint.v1",bytes[0..<body]);if !ck_equal(d,ck_get(bytes,body)) {return CkRead.{status=CkStatus.corrupt};}
    meta:=CkMeta.{ids=CkIds.{model=ck_get(bytes,8),tokenizer=ck_get(bytes,40),dataset=ck_get(bytes,72),optimizer=ck_get(bytes,104)},parent=ck_get(bytes,136)};
    if !ck_equal(meta.ids.model,expect.model) {return CkRead.{status=CkStatus.model};}
    if !ck_equal(meta.ids.tokenizer,expect.tokenizer) {return CkRead.{status=CkStatus.tokenizer};}
    if !ck_equal(meta.ids.dataset,expect.dataset) {return CkRead.{status=CkStatus.dataset};}
    if !ck_equal(meta.ids.optimizer,expect.optimizer) {return CkRead.{status=CkStatus.optimizer};}
    if params.count!=n || m.count!=n || v.count!=n {return CkRead.{status=CkStatus.shape};}
    // Every field must fit the run's integers before it is believed.
    top:=cast(u64)1099511627775;words:[7]u64;for i:0..<7 {words[i]=ck_get64(bytes,168+8*i);}
    for i:0..<7 {if i!=1 && words[i]>top {return CkRead.{status=CkStatus.corrupt};}}
    meta.revision=cast(int)words[0];meta.cursor=PkCursor.{seed=words[1],epoch=cast(int)words[2],order=cast(int)words[3],offset=cast(int)words[4]};meta.state=AwState.{step=cast(int)words[5],skipped=cast(int)words[6]};
    if meta.revision<meta.state.step || !aw_state(meta.state) {return CkRead.{status=CkStatus.corrupt};}
    at:=CK_HEAD;
    for i:0..<3*n {if !ck_bits(ck_get64(bytes,at+8*i),i>=2*n) {return CkRead.{status=CkStatus.number};}}
    for i:0..<n {params[i]=f64_from_bits(ck_get64(bytes,at+8*i));m[i]=f64_from_bits(ck_get64(bytes,at+8*(n+i)));v[i]=f64_from_bits(ck_get64(bytes,at+8*(2*n+i)));}
    return CkRead.{meta=meta,digest=d};
}
```

A pointer is current only for the record it names. If a newer record has
replaced the file under the old name, or the pointer names another revision,
the resume refuses as stale.

## Publishing atomically

The C host does only file effects: it writes the bytes to a temporary file in
the same directory, syncs it, renames it over the name and syncs the
directory. The record is published before the pointer. A crash between the
two leaves the old pointer naming the old, still-whole record, so a restart
resumes from it. Coax decides every byte and every name.

## The run

One step reads the next row, takes the mean cross-entropy over the transitions
whose target is not a boundary, computes its exact gradient and applies one
AdamW update. The row's maximum logit is subtracted before exponentiation, so
logits of 800 stay finite.

<!-- source: psionic/lib/checkpoint/resume.coax step -->
```jai
// One step: the next packed row, mean cross-entropy over the transitions
// whose target is not a BOS, its exact gradient, one AdamW update.
rs_step :: (docs:[]PkDoc,tokens:[]u8,groups:[]AwGroup,policy:AwPolicy,run:RsRun,w:[]f64,m:[]f64,v:[]f64)->RsStep #export_c {
    row:[8]u8;next:=pk_next(docs,tokens,run.cursor,row[..],false);if next.status!=PkStatus.ok {return RsStep.{status=RsStatus.data,run=run};}
    g:[256]f64;loss:=0.0;pairs:=0;
    for i:1..<RS_ROW {if row[i]!=PK_BOS {pairs+=1;}}
    for i:1..<RS_ROW {a:=cast(int)row[i-1];b:=cast(int)row[i];if b==PK_BOS {continue;}
        top:=w[a*16];for j:1..<16 {x:=w[a*16+j];top=(top+x+abs(top-x))/2;}z:=0.0;for j:0..<16 {z+=exp(w[a*16+j]-top);}
        loss+=log(z)+top-w[a*16+b];
        for j:0..<16 {hit:=0.0;if j==b {hit=1;}g[a*16+j]+=(exp(w[a*16+j]-top)/z-hit)/f64(pairs);}
    }
    none:[1]f32;r:=aw_step(groups,policy,run.state,w,g[..],m,v,none[0..<0],false);
    if r.status!=AwStatus.ok {return RsStep.{status=RsStatus.optimizer,run=run};}
    return RsStep.{run=RsRun.{cursor=next.cursor,state=r.state,revision=run.revision+1},loss=loss/f64(pairs)};
}
```

## Checks

An independent [oracle](../tests/resume_oracle.py) recomputes the epoch
orders, the packed stream, the dataset identity, a fixed checkpoint and
pointer packed with `struct.pack`, and the whole 12-step loss stream and final
weights. Coax matches the stream and the record byte for byte and the losses
within 1e-13. Runs interrupted after steps 1, 5 and 11 write a checkpoint,
resume in fresh buffers from those bytes alone, and finish with every
parameter and moment identical to the run that never stopped. The native
build does the same through real files: it publishes step 5 with its pointer,
crashes after writing step 8's record but before its pointer, restarts from
step 5 and reproduces the uninterrupted run bit for bit. Stale, truncated and
corrupt files refuse, and no temporary file is left behind.

[Run the example](../examples/resume.coax):

<!-- source: psionic/examples/resume.coax example -->
```jai
main :: () {
    docs:[5]PkDoc;tokens:[14]u8;pk_fixture(docs[..],tokens[..]);groups:=rs_group();P:=AwPolicy.{};ids:=rs_ids(docs[..],tokens[..],groups[..],P);
    // Twelve steps without stopping.
    w:[256]f64;m:[256]f64;v:[256]f64;run:=RsRun.{cursor=PkCursor.{seed=7}};straight:[12]f64;
    for k:0..<12 {s:=rs_step(docs[..],tokens[..],groups[..],P,run,w[..],m[..],v[..]);#assert s.status==RsStatus.ok;run=s.run;straight[k]=s.loss;}
    // Five steps, a checkpoint and its pointer, then a fresh start from those bytes.
    a:[256]f64;am:[256]f64;av:[256]f64;r:=RsRun.{cursor=PkCursor.{seed=7}};
    for k:0..<5 {s:=rs_step(docs[..],tokens[..],groups[..],P,r,a[..],am[..],av[..]);r=s.run;}
    bytes:[6404]u8;#assert ck_write(rs_meta(ids,r),a[..],am[..],av[..],bytes[..],false)==CkStatus.ok;
    pointer:[80]u8;#assert ck_pointer_write(r.revision,ck_digest("psionic.checkpoint.v1",bytes[0..<6372]),pointer[..])==CkStatus.ok;
    b:[256]f64;bm:[256]f64;bv:[256]f64;q:=rs_resume(pointer[..],bytes[..],ids,docs[..],b[..],bm[..],bv[..]);#assert q.status==CkStatus.ok;
    print("record: PSCK version %, % bytes, % values each for parameters and both moments\n",bytes[4],bytes.count,ck_get32(bytes[..],CK_HEAD-4));
    print("revision %, optimizer step %, cursor epoch % order % offset %, seed %\n",q.meta.revision,q.meta.state.step,q.meta.cursor.epoch,q.meta.cursor.order,q.meta.cursor.offset,q.meta.cursor.seed);
    print("dataset % % % %..., pointer names revision %\n",q.meta.ids.dataset[0],q.meta.ids.dataset[1],q.meta.ids.dataset[2],q.meta.ids.dataset[3],ck_pointer_read(pointer[..]).revision);
    resumed:=RsRun.{cursor=q.meta.cursor,state=q.meta.state,revision=q.meta.revision};same:=0;
    for k:5..<12 {s:=rs_step(docs[..],tokens[..],groups[..],P,resumed,b[..],bm[..],bv[..]);resumed=s.run;if s.loss==straight[k] {same+=1;}}
    for i:0..<256 {#assert b[i]==w[i] && bm[i]==m[i] && bv[i]==v[i];}
    print("loss % -> %; after resuming at 5, % of 7 losses and all 768 values are identical\n",straight[0],straight[11],same);
}
```

```shell
coax/bin/coax run psionic/examples/resume.coax
psionic/check.sh --only resume
```

The [transcript](../fixtures/resume.out) is exact for the named CPU build.
[The verification record](../../docs/verification/psionic-ch122-resume-2026-10-31.md)
records the checks, mutation and native evidence.
