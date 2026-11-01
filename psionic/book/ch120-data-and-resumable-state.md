# 120. Data and resumable state

A held-out score means nothing if the test data leaked into training. Leaks
rarely come from copying a test file into the training set. They come from
splitting rows when the unit that matters is bigger than a row: two turns of
one episode, the same session seen twice, a paraphrase of a test question, a
near-copy scraped from a second site, or a feature that was only recorded
after the outcome it is meant to predict.

This chapter makes the split itself a checked object. Every row carries its
provenance, rows that belong together are joined into groups, each group
takes exactly one split, an audit counts every pair that crosses a split,
and preprocessing statistics are fitted on training rows only. A clean split
then becomes [chapter 122](ch122-a-checkpoint-holds-the-whole-experiment.md)'s
packed dataset, whose identity every checkpoint names, so a run interrupted at
a chosen step resumes bit for bit, and only on the split it was trained on.

The profile is `psionic-split-v1` on top of chapter 122's `psionic-resume-v1`:
CPU only, at most 64 rows, token values 0–14, and the 16 by 16 bigram model
that chapter trains with AdamW.

```shell
coax/bin/coax run psionic/examples/split.coax
coax/bin/coax test psionic/lib/data/split.coax
```

## Rows and their provenance

A row is one document with:

- an ID, ascending across the dataset;
- a source and a licence code (1 to 3; 0, unknown, is refused);
- a task, a session, a time and an outcome time;
- `derived`, 1 + the index of the earlier row it was derived from (a
  paraphrase or a translation), or 0 for an original;
- its span of tokens.

Its content identity is SHA-256 (tag `psionic.content.v1`) of its tokens.
Admission refuses a negative source, an unknown licence, an outcome earlier
than the row's time, a derived link to itself or a later row, spans that do
not cover the tokens exactly, and tokens at or above the boundary token 15.

The fixture has fourteen rows with five planted leaks. Times run from 1 to 10;
each row's outcome is one tick after its time.

<!-- source: psionic/lib/data/split.coax fixture -->
```jai
// Fourteen rows with five planted leaks: a task (rows 0, 1), a session
// (2, 3), near-duplicate content (4, 5), a paraphrase declared by provenance
// (6, 7) and an exact copy (8, 9). Rows 10 to 13 are independent. Feature 0
// is known with its row; feature 1 is known one tick after the outcome.
sp_fixture :: (rows:[]SpRow,tokens:[]u8,cells:[]SpCell) {
    text:=[51]u8.[1,2,3,4, 5,6,7, 8,9,10,11, 0,1, 12,13,14,12, 12,13,14,12,0, 3,5,7,9, 9,7,5,3, 2,4,6, 2,4,6, 11,10,9,8, 6,5,4, 13,12,1,0, 14,0,13,1];
    lens:=[14]int.[4,3,4,2,4,5,4,4,3,3,4,3,4,4];
    tasks:=[14]int.[1,1,2,3,4,5,6,7,8,9,10,11,12,13];
    sessions:=[14]int.[10,11,12,12,13,14,15,16,17,18,19,20,21,22];
    times:=[14]int.[1,7,3,6,4,9,2,10,3,8,1,2,4,5];
    values:=[14]f64.[50,60,70,80,90,100,110,120,130,140,1,2,3,6];
    for i:0..<51 {tokens[i]=text[i];}
    at:=0;
    for i:0..<14 {
        rows[i]=SpRow.{id=i+1,source=i%3,licence=cast(u8)(1+i%2),task=tasks[i],session=sessions[i],time=times[i],outcome=times[i]+1,first=at,count=lens[i]};at+=lens[i];
        cells[2*i]=SpCell.{value=values[i],known=times[i]};cells[2*i+1]=SpCell.{value=f64(i%2),known=times[i]+2};
    }
    rows[7].derived=7;
}
```

## Which rows belong together

Two rows are linked when any of these holds:

- they share a task (repeated episodes);
- they share a session;
- one is declared as derived from the other;
- their token bigram sets, with the boundary token before the first token,
  have a Jaccard index of at least 1/2 (near-duplicates and exact copies).

The Jaccard test runs in integers: `2*both >= either`. Rows 4 and 5 are
`12 13 14 12` and `12 13 14 12 0`. Their bigrams are

```text
row 4: (15,12) (12,13) (13,14) (14,12)
row 5: (15,12) (12,13) (13,14) (14,12) (12,0)
both 4, either 5: 2*4 = 8 >= 5, linked
```

Row 7, `9 7 5 3`, is row 6 reversed. They share no bigram, so content alone
cannot join them; the declared link (`rows[7].derived=7`, row 6) does. This
chapter detects paraphrases only through such declared provenance or through
bigram overlap; it has no semantic similarity model.

Groups are the transitive closure of the links. Each row is labelled with the
smallest index in its group. When two linked rows have different labels, the
smaller label is found by counting up from 0, and the group with the larger
one is renamed. Its rows all have indexes at or above its label, so the
renaming starts there:

<!-- source: psionic/lib/data/split.coax groups -->
```jai
// Labels every row with the smallest index in its group: rows are joined by a
// shared task or session, a derived link, or near-duplicate content, and the
// joining is transitive. A label is never larger than its rows' indexes, so
// the group being renamed starts at its own label.
sp_groups :: (rows:[]SpRow,tokens:[]u8,label:[]int) {
    for i:0..<rows.count {label[i]=i;}
    for i:0..<rows.count {for j:i+1..<rows.count {
        a:=label[i];b:=label[j];
        if a!=b && sp_linked(rows,tokens,i,j) {
            keep:=0;while keep!=a && keep!=b {keep+=1;}gone:=a+b-keep;
            for k:gone..<rows.count {if label[k]==gone {label[k]=keep;}}
        }
    }}
}
```

## One split per group

A group takes one split from its latest time. With the plan used here,
`train_end=5` and `valid_end=8`, a latest time at or before 5 is training, at
or before 8 is validation, and anything later is test. The code does not take
a maximum: a group is late when any of its rows is later than `train_end`,
and later when any is later than `valid_end`.

<!-- source: psionic/lib/data/split.coax assign -->
```jai
// Gives each group one split from its latest time: at or before train_end is
// training, at or before valid_end is validation, anything later is test. A
// group is late when any of its rows is.
sp_assign :: (rows:[]SpRow,tokens:[]u8,plan:SpPlan,splits:[]PkSplit)->SpStatus #export_c {
    s:=sp_rows(rows,tokens);if s!=SpStatus.ok {return s;}
    if splits.count!=rows.count || plan.train_end<0 || plan.valid_end<plan.train_end {return SpStatus.shape;}
    label:[64]int;sp_groups(rows,tokens,label[..]);late:[64]bool;later:[64]bool;
    for i:0..<rows.count {g:=label[i];t:=rows[i].time;late[g]=late[g] || t>plan.train_end;later[g]=later[g] || t>plan.valid_end;}
    train:=false;
    for i:0..<rows.count {g:=label[i];
        if later[g] {splits[i]=PkSplit.test;}
        else if late[g] {splits[i]=PkSplit.validation;}
        else {splits[i]=PkSplit.train;train=true;}
    }
    if !train {return SpStatus.split;}
    return SpStatus.ok;
}
```

By hand, on the fixture:

```text
group   rows    times   latest  split
0       0 1     1 7     7       validation   (shared task 1)
2       2 3     3 6     6       validation   (shared session 12)
4       4 5     4 9     9       test         (near-duplicate)
6       6 7     2 10    10      test         (row 7 derived from row 6)
8       8 9     3 8     8       validation   (exact copy)
10..13  one each 1 2 4 5 <= 5   training
```

Rows 0, 2, 4, 6 and 8 would be training rows by their own times. Their
partners pull them out. That costs training data; it is the price of a
held-out set that is held out.

Splitting by each row's own time puts row 0 in training and row 1 in
validation, row 2 in training and row 3 in validation, and so on: one leak of
each kind.

## The audit

The audit counts every pair in different splits that has the same content
identity (`exact`), near-duplicate content that is not an exact copy
(`near`), a shared task, a shared session or a derived link, and every feature
known after its row's outcome (`post`). A dataset is admitted for training
only when the five pair counts are all zero.

<!-- source: psionic/lib/data/split.coax audit -->
```jai
// Counts every pair of rows in different splits that shares content, a task,
// a session, a derived link or near-duplicate content, and every feature known
// after its row's outcome. A clean split has all six counts zero.
sp_audit :: (rows:[]SpRow,tokens:[]u8,splits:[]PkSplit,cells:[]SpCell,features:int)->SpAudit #export_c {
    s:=sp_rows(rows,tokens);if s!=SpStatus.ok {return SpAudit.{status=s};}
    if splits.count!=rows.count || features<0 || features>16 || cells.count!=rows.count*features {return SpAudit.{status=SpStatus.shape};}
    a:=SpAudit.{};ids:[64][32]u8;for i:0..<rows.count {ids[i]=sp_content(rows[i],tokens);}
    for i:0..<rows.count {for j:i+1..<rows.count {
        if splits[i]==splits[j] {continue;}
        x:=rows[i];y:=rows[j];
        if ck_equal(ids[i],ids[j]) {a.exact+=1;} else if sp_near(x,y,tokens) {a.near+=1;}
        if x.task==y.task {a.task+=1;}
        if x.session==y.session {a.session+=1;}
        if y.derived==i+1 {a.derived+=1;}
    }}
    a.post=sp_post(rows,cells,features);
    return a;
}
```

<!-- source: psionic/lib/data/split.coax docs -->
```jai
// Turns an admitted, leak-free split into chapter 122's packed documents.
// A split with any cross-split pair is refused and nothing is written.
sp_docs :: (rows:[]SpRow,tokens:[]u8,splits:[]PkSplit,docs:[]PkDoc)->SpStatus #export_c {
    none:[1]SpCell;a:=sp_audit(rows,tokens,splits,none[0..<0],0);if a.status!=SpStatus.ok {return a.status;}
    if docs.count!=rows.count {return SpStatus.shape;}
    if a.exact!=0 || a.task!=0 || a.session!=0 || a.derived!=0 || a.near!=0 {return SpStatus.leak;}
    train:=false;for s:splits {if s==PkSplit.train {train=true;}}if !train {return SpStatus.split;}
    for i:0..<rows.count {docs[i]=PkDoc.{id=rows[i].id,split=splits[i],first=rows[i].first,count=rows[i].count};}
    return SpStatus.ok;
}
```

## A feature known after the outcome

Each feature value carries the time it became known. Feature 0 is known at the
row's time. Feature 1 is known two ticks after the row's time, one tick after
the outcome: a model trained on it would learn from the answer. The audit
counts it whatever the split is, and fitting refuses it.

## Preprocessing from training rows only

Normalising a feature uses its mean and standard deviation. Computed over all
rows, the validation and test values (50 to 140) would move them. Fitting
reads training rows only. The training values of feature 0 are 1, 2, 3 and 6:

```text
mean     (1+2+3+6)/4                    = 3
variance ((1-3)^2+(2-3)^2+(3-3)^2+(6-3)^2)/4 = (4+1+0+9)/4 = 3.5
scale    sqrt(3.5)                      = 1.8708286933869707
```

<!-- source: psionic/lib/data/split.coax fit -->
```jai
// Mean and population standard deviation of one feature over training rows
// only. A feature known after the outcome is refused, not fitted.
sp_fit :: (rows:[]SpRow,splits:[]PkSplit,cells:[]SpCell,features:int,f:int)->SpFit #export_c {
    if splits.count!=rows.count || rows.count>SP_ROWS || features<1 || f<0 || f>=features || cells.count!=rows.count*features {return SpFit.{status=SpStatus.shape};}
    sum:=0.0;n:=0;
    for i:0..<rows.count {c:=cells[i*features+f];
        if c.known>rows[i].outcome {return SpFit.{status=SpStatus.time};}
        if splits[i]==PkSplit.train {sum+=c.value;n+=1;}
    }
    if n==0 {return SpFit.{status=SpStatus.split};}
    mean:=sum/f64(n);sq:=0.0;
    for i:0..<rows.count {if splits[i]==PkSplit.train {d:=cells[i*features+f].value-mean;sq+=d*d;}}
    return SpFit.{mean=mean,scale=sqrt(sq/f64(n)),rows=n};
}
```

## Run the example

The example audits the row-by-row split, refuses it as a dataset, makes the
grouped split, fits feature 0, refuses feature 1, and then trains chapter
122's bigram model on the grouped split's four training rows. One run takes
ten steps straight. A second takes four steps, writes a checkpoint and its
pointer, resumes into fresh buffers from those bytes alone, and takes the
other six. Finally it shows two refusals: the record with one flipped bit, and
the same record offered to a run whose split was made with another plan
(`train_end=8`), which is another dataset identity.

[Run the example](../examples/split.coax):

<!-- source: psionic/examples/split.coax example -->
```jai
main :: () {
    rows:[14]SpRow;tokens:[51]u8;cells:[28]SpCell;sp_fixture(rows[..],tokens[..],cells[..]);
    // The leaky way: each row takes the split of its own time.
    naive:[14]PkSplit;sp_by_row(rows[..],SP_PLAN,naive[..]);a:=sp_audit(rows[..],tokens[..],naive[..],cells[..],2);
    print("by row: exact % task % session % derived % near % post-outcome features %\n",a.exact,a.task,a.session,a.derived,a.near,a.post);
    docs:[14]PkDoc;print("by row as a dataset: %\n",sp_docs(rows[..],tokens[..],naive[..],docs[..]));
    // The grouped way: one split per group, from the group's latest time.
    splits:[14]PkSplit;#assert sp_assign(rows[..],tokens[..],SP_PLAN,splits[..])==SpStatus.ok;
    label:[14]int;sp_groups(rows[..],tokens[..],label[..]);
    for i:0..<14 {print("% ",label[i]);}print("<- group of each row\n");
    for i:0..<14 {print("% ",cast(int)splits[i]);}print("<- split (0 train, 1 validation, 2 test)\n");
    a=sp_audit(rows[..],tokens[..],splits[..],cells[..],2);
    print("grouped: exact % task % session % derived % near % post-outcome features %\n",a.exact,a.task,a.session,a.derived,a.near,a.post);
    f:=sp_fit(rows[..],splits[..],cells[..],2,0);print("feature 0 from % training rows: mean % scale %\n",f.rows,f.mean,f.scale);
    print("feature 1: %\n",sp_fit(rows[..],splits[..],cells[..],2,1).status);
    // Train ten steps straight; separately, four steps, a checkpoint, a fresh resume, six more.
    #assert sp_docs(rows[..],tokens[..],splits[..],docs[..])==SpStatus.ok;
    groups:=rs_group();P:=AwPolicy.{};ids:=rs_ids(docs[..],tokens[..],groups[..],P);
    w:[256]f64;m:[256]f64;v:[256]f64;run:=RsRun.{cursor=PkCursor.{seed=3}};straight:[10]f64;
    for k:0..<10 {s:=rs_step(docs[..],tokens[..],groups[..],P,run,w[..],m[..],v[..]);#assert s.status==RsStatus.ok;run=s.run;straight[k]=s.loss;}
    x:[256]f64;xm:[256]f64;xv:[256]f64;r:=RsRun.{cursor=PkCursor.{seed=3}};
    for k:0..<4 {s:=rs_step(docs[..],tokens[..],groups[..],P,r,x[..],xm[..],xv[..]);r=s.run;}
    bytes:[6404]u8;#assert ck_write(rs_meta(ids,r),x[..],xm[..],xv[..],bytes[..],false)==CkStatus.ok;
    pointer:[80]u8;#assert ck_pointer_write(r.revision,ck_digest("psionic.checkpoint.v1",bytes[0..<6372]),pointer[..])==CkStatus.ok;
    b:[256]f64;bm:[256]f64;bv:[256]f64;q:=rs_resume(pointer[..],bytes[..],ids,docs[..],b[..],bm[..],bv[..]);#assert q.status==CkStatus.ok;
    resumed:=RsRun.{cursor=q.meta.cursor,state=q.meta.state,revision=q.meta.revision};same:=0;
    for k:4..<10 {s:=rs_step(docs[..],tokens[..],groups[..],P,resumed,b[..],bm[..],bv[..]);resumed=s.run;if s.loss==straight[k] {same+=1;}}
    equal:=0;for i:0..<256 {if b[i]==w[i] && bm[i]==m[i] && bv[i]==v[i] {equal+=1;}}
    print("loss % -> %; resumed at 4: % of 6 losses and % of 256 parameter triples identical\n",straight[0],straight[9],same,equal);
    // The same checkpoint against a damaged record and against another split.
    bytes[300]^=1;print("one flipped bit: %\n",rs_resume(pointer[..],bytes[..],ids,docs[..],b[..],bm[..],bv[..]).status);bytes[300]^=1;
    #assert sp_assign(rows[..],tokens[..],SpPlan.{train_end=8,valid_end=9},splits[..])==SpStatus.ok;
    #assert sp_docs(rows[..],tokens[..],splits[..],docs[..])==SpStatus.ok;
    print("another split: %\n",rs_resume(pointer[..],bytes[..],rs_ids(docs[..],tokens[..],groups[..],P),docs[..],b[..],bm[..],bv[..]).status);
}
```

The [transcript](../fixtures/split.out) is exact in debug and release builds:

```text
by row: exact 1 task 1 session 1 derived 1 near 1 post-outcome features 1
by row as a dataset: leak
0 0 2 2 4 4 6 6 8 8 10 11 12 13 <- group of each row
1 1 1 1 2 2 2 2 1 1 0 0 0 0 <- split (0 train, 1 validation, 2 test)
grouped: exact 0 task 0 session 0 derived 0 near 0 post-outcome features 1
feature 0 from 4 training rows: mean 3.0 scale 1.8708286933869707
feature 1: time
loss 2.772588722239781 -> 1.993527110149979; resumed at 4: 6 of 6 losses and 256 of 256 parameter triples identical
one flipped bit: corrupt
another split: dataset
```

The split codes print as numbers: 0 training, 1 validation, 2 test. The
checkpoint record and the pointer are chapter 122's; its record digest catches
the flipped bit (`corrupt`), and its dataset identity covers every row's ID,
split, length and tokens, so moving rows between splits changes it
(`dataset`).

## Checks

The checks in [`split.coax`](../lib/data/split.coax) compare against values
derived by hand and against an independent reference:

- the group labels and splits in the tables above, the five planted pairs
  counted by the audit, and the mean and scale computed above;
- the Jaccard boundary: `1 2 3` against `1 2 4` shares 2 of 4 bigrams and is
  near; `1 2 3` against `1 4 3` shares 1 of 5 and is not;
- a `#forall` over 24 random datasets of 12 rows that builds the link
  relation, closes it with Warshall's algorithm, and requires the groups to
  equal the closure, every split to equal the split of the latest time each
  row reaches, and the audit of the result to be clean;
- every admission refusal, each with its first passing value beside it;
- each planted kind alone (rows 1, 3, 5 and 9 moved one at a time) counts
  exactly one pair of its own kind, and the dataset is refused;
- a feature known exactly at its row's outcome is not leaked; one tick later
  is;
- changing validation and test values leaves the fitted statistics unchanged;
  changing a training value changes them to the hand-computed mean 4 and
  scale `sqrt(12.5)`;
- the interrupted run on the grouped split equals the straight run in every
  loss, parameter and moment, and refuses the checkpoint under the other
  plan's dataset identity.

The file is `#authority`: every procedure runs in a check, and
`coax mutate psionic/lib/data/split.coax --root-only` leaves no survivor.

## What this does not cover

- Paraphrases are found only when declared by provenance or when their
  bigrams overlap; there is no semantic similarity model.
- Splits are by time over groups. A random split by group hash, and
  [chapter 247](ch247-outcomes-become-training-rows.md)'s exposure ledger for
  held-out groups, are not used.
- The resume profile is chapter 122's: one optimizer group, no scheduler and
  no shard layouts.

## Sources

- [Chapter plan](PLAN.md)
- [nanochat target notes](../docs/targets/nanochat.md)
