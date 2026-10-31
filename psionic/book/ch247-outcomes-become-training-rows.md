# 247. Outcomes become training rows

Chapters 215, 223, 235 and 243 learn from what already happened: decisions
and whether they turned out right, and traces of programs that ran. This
chapter decides which of those records may become training rows, and for
which split.

One pre-reset audit found that all 41 replayed traces used for training came
from groups reserved for calibration or development. Another found a
retraining script that trained on its own evaluation cases. Each set of rows
here is built for one artifact, such as a router, a calibration refit or a
student, and every admission is checked against an **exposure ledger** that
all artifacts share.

```shell
coax/bin/coax run psionic/examples/training-rows.coax
coax/bin/coax test psionic/lib/training-rows.coax
python3 -I psionic/tests/rows_oracle.py
```

```jai
artifact 0, cuts 22/30/38: admitted 32, unlabelled 3, summary 9, crosses 3, duplicate 1; rows: train 16, calibration 4, development 5, heldout 7; digest 1fbc842157b4dd08
artifact 1, cuts 14/18/26: admitted 17, unlabelled 3, summary 9, reserved 19; rows: train 10, calibration 0, development 0, heldout 7; digest 09b055b532b1e9c3
artifact 2, traces:
  squares in group 20: admitted
  squares again in group 21: duplicate
  squares, one value changed in group 22: failed
  squares, no rows, budget 10 in group 23: unverifiable
  squares from 3 in group 24: admitted
  squares from 3 in group 9: reserved
artifact 2: 2 rows, digest 462297b4ba74a74e
ledger: 14 groups, 6 shared by artifacts 0 and 1 in one role; group 9: development, artifact mask 1
```

The [example](../examples/training-rows.coax) feeds 48 decision outcomes, in
12 groups of four consecutive times, to two artifacts with different time
cuts. A third artifact, a student trained on traces, files six of chapter
201's claimed runs.

## Splits by time

<!-- source: psionic/lib/training-rows.coax split -->
```jai
tr_cuts_ok :: (c:TrCuts)->bool {
    return c.calibration>0 && c.calibration<=c.development && c.development<=c.heldout;
}
tr_role :: (c:TrCuts,time:int)->TrRole {
    if time<c.calibration {return TrRole.train;}
    if time<c.development {return TrRole.calibration;}
    if time<c.heldout {return TrRole.development;}
    return TrRole.heldout;
}
```

Each artifact names three cuts. Rows before the first cut are for training;
then come calibration and development, and from the last cut on, rows are
held out. A row's split comes from its time, never from a random draw, so
nothing the artifact trains on happened after what it is evaluated on.

Every row belongs to a **group**, such as a task or a conversation. A group
gets one split for good, from its first admitted row. A later row of the
same group that falls past a cut is refused as `crosses`, so one group never
appears in two splits. In the example, groups 5 and 7 straddle cuts of the
first artifact, and 3 of their rows are refused that way.

## What a row may be

<!-- source: psionic/lib/training-rows.coax admit -->
```jai
// Checks the set and the row's place in the feed. A row that passes moves
// the feed's time forward, even if a later check refuses it.
tr_front :: (s:TrSet,group:int,time:int,ok:bool)->TrAdd {
    a:=TrAdd.{set=s};
    if !ok || s.artifact<0 || s.artifact>=TR_ARTIFACTS || !tr_cuts_ok(s.cuts) || group<0 || time<0 ||
        s.count<0 || s.count>s.row.count || s.entries<0 || s.entries>s.entry.count {
        a.reason=TrReason.shape;return a;
    }
    if time<s.last {a.reason=TrReason.order;return a;}
    a.set.last=time;
    return a;
}
tr_find :: (s:TrSet,group:int)->int {
    for i:0..<s.entries {if s.entry[i].group==group {return i;}}
    return -1;
}
// The ledger and the duplicate check, then the row is appended.
tr_admit :: (s:TrSet,row:TrRow)->TrAdd {
    a:=TrAdd.{set=s};r:=row;r.role=tr_role(s.cuts,r.time);
    bit:=cast(u64)1<<cast(u64)s.artifact;e:=tr_find(s,r.group);
    if e>=0 && s.entry[e].role!=r.role {
        a.reason=ifx s.entry[e].artifacts!=bit then TrReason.reserved else TrReason.crosses;
        return a;
    }
    for i:0..<s.count {if rx_same(s.row[i].key,r.key) {a.reason=TrReason.duplicate;return a;}}
    if s.count==s.row.count || (e<0 && s.entries==s.entry.count) {a.reason=TrReason.capacity;return a;}
    a.set.row[s.count]=r;a.set.count+=1;
    if e<0 {e=s.entries;a.set.entry[e]=TrEntry.{group=r.group,role=r.role};a.set.entries+=1;}
    a.set.entry[e].artifacts|=bit;
    return a;
}
tr_add_outcome :: (s:TrSet,o:TrOutcome)->TrAdd {
    a:=tr_front(s,o.group,o.time,o.label>=0 && o.label<=1 && o.from!=TrLabel.replay);
    if a.reason!=TrReason.admitted {return a;}
    if o.from==TrLabel.none {a.reason=TrReason.unlabelled;return a;}
    if o.from==TrLabel.summary {a.reason=TrReason.summary;return a;}
    return tr_admit(a.set,TrRow.{source=TrSource.decision,group=o.group,time=o.time,label=o.label,from=o.from,key=tr_outcome_key(o)});
}
// The trace is replayed here, whole, within budget steps; chapter 234
// refuses a budget below 1 by shape.
tr_add_trace :: (s:TrSet,program:[]TmOp,claim:RxClaim,rows:[]int,group:int,time:int,budget:int)->TrAdd {
    a:=tr_front(s,group,time,true);
    if a.reason!=TrReason.admitted {return a;}
    r:=rx_check(program,claim,rows,0,claim.steps,budget);
    if r.status!=RxStatus.ok {a.reason=TrReason.shape;return a;}
    if r.verdict==RxVerdict.unverifiable {a.reason=TrReason.unverifiable;return a;}
    if r.verdict==RxVerdict.failed {a.reason=TrReason.failed;return a;}
    return tr_admit(a.set,TrRow.{source=TrSource.trace,group=group,time=time,label=1,from=TrLabel.replay,key=tr_trace_key(claim)});
}
```

The feed must run forward in time. Each candidate is then checked in this
order:
1. **A decision outcome** needs a label from a **check**, such as a test that
   passed, or a **diff**, such as a change that was merged. A missing label
   is `unlabelled`. A label taken from the agent's own summary of what it did
   is refused as `summary`: an agent's account of its work isn't evidence
   that the work was right.
2. **A trace** is replayed with chapter 234's checker, whole, within a step
   budget. Only an `exact` verdict is admitted; its label is 1 and its
   label source is `replay`. A trace that fails is refused as `failed`, and
   one that can't be checked in the budget as `unverifiable`.
3. **The ledger.** If the group already has another role, the row is
   refused: `crosses` if only this artifact used the group, `reserved` if any
   other artifact did.
4. **Duplicates.** The same decision in the same group at the same time is
   one outcome, whatever its label says. The same run is one trace, whatever
   group it was filed under.

Only an admitted row exposes its group. A refused row leaves the ledger as
it was.

## The ledger

The ledger has one entry per group: its role, and one bit for each artifact
that used it. It is passed from one artifact's set to the next. In the
example, the second artifact cuts time earlier than the first. Of its 36
labelled outcomes, 19 fall in groups the first artifact already uses in
another role, and all 19 are refused as `reserved`. Six groups are shared, each in one role: four that both train
on, and two that both hold out.

The trace student trains on everything it is given. Its last trace is filed
under group 9, which the first artifact uses for development, so it is
refused even though it replays exactly. This is the audit's failure, caught
at admission.

## Keys and the set digest

<!-- source: psionic/lib/training-rows.coax keys -->
```jai
tr_put :: (buf:[]u8,n:int,v:int)->int {
    w:=cast(u64)v;
    for i:0..<8 {buf[n+i]=cast(u8)((w>>cast(u64)(i*8))&0xff);}
    return n+8;
}
tr_hash :: (buf:[]u8,n:int)->[32]u8 {
    d:[32]u8;#assert sha256_hash_checked(buf[0..<n],d[..],false)==SHA_OK;
    return d;
}
// The same decision at the same time in the same group is one outcome,
// whatever its label says.
tr_outcome_key :: (o:TrOutcome)->[32]u8 {
    b:[32]u8;n:=tr_put(b[..],0,cast(int)TrSource.decision);
    n=tr_put(b[..],n,o.group);n=tr_put(b[..],n,o.time);n=tr_put(b[..],n,o.decision);
    return tr_hash(b[..],n);
}
// A trace is the run its claim names, whichever group it was filed under.
tr_trace_key :: (c:RxClaim)->[32]u8 {
    b:[112]u8;n:=tr_put(b[..],0,cast(int)TrSource.trace);
    for i:0..<32 {b[n+i]=c.program[i];b[n+32+i]=c.trace[i];b[n+64+i]=c.outputs[i];}
    n=tr_put(b[..],n+96,c.steps);
    return tr_hash(b[..],n);
}
```

<!-- source: psionic/lib/training-rows.coax digest -->
```jai
// The set's digest: its row count, then each row's source, group, time,
// role, label and label source as words, and its key.
tr_digest :: (s:TrSet)->[32]u8 {
    sha:[112]u8;d:[32]u8;b:[80]u8;#assert sha256_init(sha[..])==SHA_OK;
    n:=tr_put(b[..],0,s.count);#assert sha256_update_checked(sha[..],b[0..<n],false)==SHA_OK;
    for i:0..<s.count {
        r:=s.row[i];
        n=tr_put(b[..],0,cast(int)r.source);n=tr_put(b[..],n,r.group);n=tr_put(b[..],n,r.time);
        n=tr_put(b[..],n,cast(int)r.role);n=tr_put(b[..],n,r.label);n=tr_put(b[..],n,cast(int)r.from);
        for k:0..<32 {b[n+k]=r.key[k];}
        #assert sha256_update_checked(sha[..],b[..],false)==SHA_OK;
    }
    #assert sha256_final_checked(sha[..],d[..],false)==SHA_OK;
    return d;
}
// Rows per role, indexed by the role's value.
tr_counts :: (s:TrSet)->[5]int {
    c:[5]int;
    for i:0..<s.count {c[cast(int)s.row[i].role]+=1;}
    return c;
}
```

A set's digest covers each admitted row in order, so two artifacts built
from the same rows can be compared by digest, and a row that changes split
changes it.

## Supported profile

`rows-v1`:
- at most 64 artifacts, each one bit in the ledger;
- binary labels;
- the set and the ledger in storage the caller provides;
- traces from chapter 226's machine, checked by chapter 234's `replay-v1`.

## How it is checked

- **Coax checks:** the chapter's 14 checks, with the 119 from chapters 226 and 234 that `training-rows.coax` loads, all 133 passing in debug and release. They cover:
  - the cuts and each split's edges;
  - each refusal reason, and that an admitted row claims its group;
  - the feed's order, including rows that were refused;
  - `crosses` against `reserved`, across two artifacts;
  - duplicates, for outcomes and for traces;
  - capacity of the set and of the ledger;
  - keys and digests against vectors computed with Python's `hashlib`;
  - the example's feed, for both artifacts.
- **An independent oracle,** [rows_oracle.py](../tests/rows_oracle.py),
  rebuilds the feed and the rules. It derives both artifacts' tallies, rows
  per split, digests and the shared ledger, and finds each line in the
  example's output. It doesn't replay traces; it takes their verdicts from
  chapter 234 and checks the ledger's refusal of group 9.
- **Mutation testing** of `training-rows.coax`: 1,570 of 1,570 mutants killed.

The [verification record](../../docs/verification/psionic-ch186-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Real outcomes.** The feed is synthetic. Part XXII's outcome joins are
  the source this is for.
- **More than binary labels.** Chapters 141 and 223 need a teacher's answer
  distributions, which a `rows-v1` row can't carry. They use the separate
  contract below.
- **A durable ledger.** It lives in memory for one run. Chapter 239's
  append-only store with its receipt chain is where it belongs.

## A distillation target is a different record

> **Design draft.** Not implemented. `rows-v1` holds binary labels from a
> check or a diff and traces verified by replay. It holds no teacher
> predictions.

Distillation trains a student to match a teacher's answer distribution. That
distribution is a prediction, not an observed outcome: a teacher can be
confident and wrong. If a target row and an outcome row were the same record,
a student could be trained on the teacher's mistakes and then scored as if
they were facts. The design keeps them apart with a separate record,
`distill-v1`.

### The record

| Field | What it holds |
| --- | --- |
| `teacher` | The teacher's identity: model ID, weights digest, tokenizer digest, prompt rendering profile, readout profile (temperature and calibration map, if any) and the numerical profile and host that produced the distribution. |
| `request` | The digest of the exact rendered tokens, the request's group and its time. |
| `options` | The option IDs of the decision, in order. |
| `distribution` | The teacher's probability for each option at temperature 1, and the probability mass outside the options (chapter 13). |
| `split` | The split from the same time cuts as `rows-v1`, and the exposure-ledger artifact that assigned it. |
| `teacher_exposure` | Whether the request's group was in the teacher's training or calibration data, when known, and `unknown` otherwise. |

The rules:

1. **Targets hold predictions only.** A `distill-v1` record never holds an
   outcome label, and a `rows-v1` outcome row never holds a teacher
   probability. A student's training set and its evaluation join them only by
   request digest.
2. **One teacher per target set.** A target set names one teacher identity.
   Targets from a different teacher, or from the same model with a different
   readout or numerical profile, form a new set. Two sets are never mixed in
   one loss.
3. **A distribution is admitted only when it's valid.** Every probability is
   finite and nonnegative, the options and the outside mass sum to 1 within
   `1e-6`, and the request was rendered with the teacher's tokenizer and
   prompt profile.
4. **Splits share the exposure ledger.** A request group gets one role across
   outcome rows and target sets, so a group held out for the student's
   outcome evaluation is never in its distillation training set.
5. **Teacher exposure limits evaluation.** A group the teacher trained or
   calibrated on can be used for training the student, but it isn't used to
   measure the student's accuracy against outcomes, and a result on it says
   so. An `unknown` exposure is reported as unknown.
6. **The distillation temperature is a training setting.** Targets are stored
   at temperature 1. A temperature used to soften them belongs to the
   student's training record.
7. **Agreement and correctness are reported separately.** Agreement with the
   teacher is computed from targets. Accuracy, Brier score and calibration
   error are computed from outcome rows. A report never presents agreement as
   accuracy.

### Checks for distillation targets

- A target whose probabilities sum to `1 + 1e-5` is refused, and so is one
  with a negative or non-finite probability.
- A target from a second teacher identity in the same set is refused.
- A request group held out in the outcome ledger is refused from a training
  target set.
- A report built from a set where the teacher is wrong on 20 percent of
  held-out outcomes shows teacher agreement and outcome accuracy as separate
  numbers, and the student's accuracy is computed only from outcome rows.
- A target set's digest changes when the teacher's readout profile changes,
  even when the weights don't.
