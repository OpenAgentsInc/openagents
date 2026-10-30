# 159. A Pylon checks itself before serving

A Pylon advertises a decision slot under a model identity: the name and
checkpoint digest its engine reports. A buyer pays for that model's answers.
The identity alone doesn't say that the answers are right. A broken driver,
a changed kernel, a miscompiled build or an engine pointed at the wrong
files can still report the right digest and answer wrongly. So before it
advertises anything, a Pylon asks its own engine questions whose answers are
already known.

This chapter adds three things:
- **a shipped reference set:** a few decision requests and the answers one
  identity gave, in a small text format;
- **the self-check:** each answer must give the same choice, and every
  probability must be within the set's tolerance;
- **the identity gate:** a Pylon that served one identity won't serve
  another unless the operator declares it.

A read-only **doctor** reports the same steps without publishing or recording
anything.

```shell
coax/bin/coax run psionic/examples/pylon-self-check.coax
coax/bin/coax test psionic/lib/pylon/selfcheck.coax
```

```jai
6 cases for elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba, tolerance 500000 billionths
  Elephant's own answers: passed, largest error 0 billionths
  nudged by 0.0004: passed, largest error 400000 billionths
  nudged by 0.0006: failed at wet: out of tolerance, error 600000 billionths
  one choice changed: failed at tone: other choice, error 0 billionths
  an engine with other weights: the set is for another model
the identity gate:
  nothing served before: first start
  served the same: same as last served
  served other weights, nothing declared: an undeclared change
  served other weights, new ones declared: a declared change
the doctor, after an undeclared change: identity
the doctor, once it is declared: healthy: may serve
```

The [example](../examples/pylon-self-check.coax) uses Elephant's own
recorded answers and runs in under a second.

## The reference set

The set for Elephant is
[elephant-1.selfcheck](../fixtures/pylon/elephant-1.selfcheck). Its first
case:

```text
psionic.selfcheck.v1
identity elephant-1@sha256:682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba
tolerance 500000
case wet
request {"model":"elephant-1","state":"Water is wet.","questions":{"wet":{"type":"choice",...}}}
expect choice yes yes=757202773 no=242797227
```

Values are whole billionths, so reading a set involves no floating point.
The six cases cover two noul questions and four choices of two or three
options: a refund, a support team, a review's tone, urgency and a language.
Their answers were recorded from `psionic-serve --elephant-gpu` on the
RTX 4080. Asked twice, the engine gave identical answers.

<!-- source: psionic/lib/pylon/selfcheck.coax set -->
```jai
// The set is lines: the version, "identity ID", "tolerance BILLIONTHS", then
// for each case "case QUESTION", "request JSON" and "expect noul N" or
// "expect choice CHOICE OPTION=N ...". The first bad line refuses the set.
ps_read_set :: (text:string,cases:[]PsCase)->PsSet #export_c {
    s:PsSet;at:=0;n:=0;
    while true {
        end:=at;while end<text.count && text[end]!='\n' {end+=1;}
        if at==text.count {break;}
        line:=text[at..<end];at=ifx end<text.count then end+1 else end;s.line+=1;
        if s.line==1 {if line!=PS_VERSION {s.status=PsStatus.header;return s;}continue;}
        if s.line==2 {
            if !ps_word(line,"identity ") {s.status=PsStatus.identity;return s;}
            s.identity=line[9..<line.count];continue;
        }
        if s.line==3 {
            v:=-1;if ps_word(line,"tolerance ") {v=ps_count(line[10..<line.count]);}
            if v<0 {s.status=PsStatus.tolerance;return s;}
            s.tolerance=v;continue;
        }
        step:=(s.line-4)%3;
        if step==0 {
            if n==cases.count {s.status=PsStatus.cases;return s;}
            if !ps_word(line,"case ") {s.status=PsStatus.line;return s;}
            cases[n]=PsCase.{question=line[5..<line.count]};continue;
        }
        if step==1 {
            if !ps_word(line,"request ") {s.status=PsStatus.line;return s;}
            cases[n].request=line[8..<line.count];continue;
        }
        if !ps_expect(line,cases[n..<n+1]) {s.status=PsStatus.expect;return s;}
        n+=1;
    }
    next:=s.line+1;
    if next==1 {s.line=next;s.status=PsStatus.header;return s;}
    if next==2 {s.line=next;s.status=PsStatus.identity;return s;}
    if next==3 {s.line=next;s.status=PsStatus.tolerance;return s;}
    step:=(next-4)%3;
    if step==1 {s.line=next;s.status=PsStatus.line;return s;}
    if step==2 {s.line=next;s.status=PsStatus.expect;return s;}
    if n==0 {s.line=next;s.status=PsStatus.cases;return s;}
    s.count=n;return s;
}
```

The first bad line refuses the whole set, and the refusal names its line.

## Reading an answer

<!-- source: psionic/lib/pylon/selfcheck.coax nanos -->
```jai
// An answer's probability as billionths, exactly from its decimal text: 0 to
// 1 with at most nine places; anything else is -1.
ps_nanos :: (s:string)->int #export_c {
    if s.count==0 || (s[0]!='0' && s[0]!='1') {return -1;}
    whole:=cast(int)(s[0]-'0');
    if s.count==1 {return whole*PS_NANO;}
    if s[1]!='.' || s.count<3 || s.count>11 {return -1;}
    // Nine places, the missing ones zero.
    v:=0;
    for k:2..<11 {
        d:=0;
        if k<s.count {if !json_digit(s[k]) {return -1;}d=cast(int)(s[k]-'0');}
        v=10*v+d;
    }
    if whole==1 && v!=0 {return -1;}
    return whole*PS_NANO+v;
}
```

An engine prints probabilities with nine places. `ps_nanos` turns that text
into billionths exactly, digit by digit. A number that isn't between 0 and 1
with at most nine places is malformed.

<!-- source: psionic/lib/pylon/selfcheck.coax answer -->
```jai
// The one question's answer and the engine's identity, from the JSON answer.
// Anything malformed returns at once, still marked shape.
ps_answer :: (body:string,question:string)->PsAnswer #export_c {
    a:PsAnswer;a.status=PsCode.shape;got:PsAnswer;found:=false;identity:="";
    j:[1]Json;json_start(j[..],body);
    if json_next(j[..])!=JSON_OBJECT {return a;}
    while true {
        t:=json_next(j[..]);
        if t==JSON_OBJECT_END {break;}
        if t!=JSON_STRING {return a;}
        key:=json_text(j[..]);v:=json_next(j[..]);
        if json_is(key,"answers") {
            if v!=JSON_OBJECT {return a;}
            while true {
                t=json_next(j[..]);
                if t==JSON_OBJECT_END {break;}
                if t!=JSON_STRING {return a;}
                q:=json_text(j[..]);v=json_next(j[..]);
                if json_is(q,question) {
                    if v!=JSON_OBJECT {return a;}
                    got=ps_fields(j[..]);if got.status==PsCode.shape {return a;}
                    found=true;
                } else if !json_skip(j[..]) {return a;}
            }
        } else if json_is(key,"service") {
            if v!=JSON_OBJECT {return a;}
            while true {
                t=json_next(j[..]);
                if t==JSON_OBJECT_END {break;}
                if t!=JSON_STRING {return a;}
                f:=json_text(j[..]);v=json_next(j[..]);
                if json_is(f,"identity") {if v!=JSON_STRING {return a;}identity=json_text(j[..]);}
                else if !json_skip(j[..]) {return a;}
            }
        } else if !json_skip(j[..]) {return a;}
    }
    if json_next(j[..])!=JSON_END {return a;}
    if !found {a.status=PsCode.missing;return a;}
    got.identity=identity;return got;
}
```

The answer is read with the Coax Nostr modules' bounded JSON parser. Fields
may come in any order, and unknown fields are skipped. Anything malformed
returns at once.

## The comparison

<!-- source: psionic/lib/pylon/selfcheck.coax compare -->
```jai
// Same kind, same choice, the same options, and each value within the
// tolerance. A pass carries the largest error; a failure names its option.
ps_compare :: (c:PsCase,a:PsAnswer,tolerance:int)->PsItem #export_c {
    r:PsItem;r.option=-1;
    if a.status!=PsCode.pass {r.code=a.status;return r;}
    if a.kind!=c.kind {r.code=PsCode.kind;return r;}
    if c.kind==PsKind.choice {
        if !json_is(a.choice,c.choice) {r.code=PsCode.choice;return r;}
        if a.options!=c.options {r.code=PsCode.options;return r;}
    }
    errors:[PS_OPTIONS]int;
    for k:0..<c.options {
        at:=0;
        if c.kind==PsKind.choice {
            at=-1;for m:0..<a.options {if json_is(a.names[m],c.names[k]) {at=m;}}
            if at<0 {r.code=PsCode.missing;r.option=k;return r;}
        }
        pair:=[2]int.[a.nanos[at],c.nanos[k]];hi:=ps_largest(pair[..]);
        e:=pair[hi]-pair[1-hi];errors[k]=e;
        if e>tolerance {r.code=PsCode.value;r.option=k;r.error=e;return r;}
    }
    r.error=errors[ps_largest(errors[0..<c.options])];
    return r;
}
```

<!-- source: psionic/lib/pylon/selfcheck.coax check -->
```jai
// Every case against the engine's answer to its request, in order. The set
// must be for the engine's identity, and each answer must name no other.
ps_check :: (s:PsSet,cases:[]PsCase,identity:string,answers:[]string)->PsCheck #export_c {
    r:PsCheck;
    if s.status!=PsStatus.ok || s.count==0 {r.verdict=PsVerdict.no_references;return r;}
    if s.identity!=identity {r.verdict=PsVerdict.other_model;return r;}
    errors:[PS_CASES]int;
    for i:0..<s.count {
        item:=PsItem.{code=PsCode.missing,option=-1};
        if i<answers.count {
            a:=ps_answer(answers[i],cases[i].question);
            if a.status==PsCode.pass && a.identity.count>0 && !json_is(a.identity,identity) {a.status=PsCode.shape;}
            item=ps_compare(cases[i],a,s.tolerance);
        }
        if item.code!=PsCode.pass {r.verdict=PsVerdict.failed;r.failed=i;r.item=item;return r;}
        errors[i]=item.error;
    }
    r.worst=errors[ps_largest(errors[0..<s.count])];return r;
}
```

The tolerance is chapter 30's frozen rule, 0.0005. Honest backends are much
closer than that:
- the RTX 4080 against itself: 0;
- the L4 against the 4080: 0;
- the CPU against the 4080: at most 1.3e-5.

A copy whose probabilities drift by 0.0004 passes. One that drifts by
0.0006, or changes a single choice, fails.

The set must name the engine's identity. If an answer names an identity
itself, that must be the same one too.

Chapter 160 checks *another* machine's answers on fresh items, without an
answer key, and has to allow for honest differences between backends. This
chapter checks a machine against itself, with an answer key, so its
tolerance can be much tighter.

## The identity gate

<!-- source: psionic/lib/pylon/selfcheck.coax gate -->
```jai
// last: the identity this Pylon served before ("" on its first start);
// declared: the identity named on the command line ("" when none).
ps_gate :: (last:string,declared:string,engine:string)->PsGate #export_c {
    if engine.count==0 {return PsGate.unnamed;}
    if declared.count>0 && declared!=engine {return PsGate.other;}
    if last.count==0 {return PsGate.first;}
    if last==engine {return PsGate.same;}
    if declared.count>0 {return PsGate.declared;}
    return PsGate.undeclared;
}
ps_gate_ok :: (g:PsGate)->bool #export_c {return g==PsGate.first || g==PsGate.same || g==PsGate.declared;}
```

The Pylon keeps the identity it last served in a one-line state file. A
change of weights then refuses to serve until the operator names the new
identity with `--declare-identity`. Without the gate, a model update could
go out under the same Pylon, which buyers have already chosen. A declared
identity that differs from the engine's refuses too.

## The doctor

<!-- source: psionic/lib/pylon/selfcheck.coax doctor -->
```jai
ps_doctor :: (f:PsFacts)->PsStep #export_c {
    if !f.config {return PsStep.config;}
    if !f.engine {return PsStep.engine;}
    if !ps_gate_ok(f.gate) {return PsStep.identity;}
    if f.set!=PsStatus.ok {return PsStep.references;}
    if f.verdict!=PsVerdict.passed {return PsStep.check;}
    return PsStep.healthy;
}
ps_may_serve :: (f:PsFacts)->bool #export_c {return ps_doctor(f)==PsStep.healthy;}
```

`pylon --doctor` runs only the read-only steps:
- the configuration;
- the engine's probe;
- the identity gate;
- the reference set;
- the self-check.

It then prints the first failing step and exits with 0 when healthy, or 10
plus the step's number. It takes no key and opens no relay connection.

Without `--doctor`, the Pylon runs the same steps at start. If any step
fails, it stops with status 6 before its relay list or beacon exists.
Otherwise it records the identity, then goes online as in chapter 19.

## On the RTX 4080

The doctor against Elephant on coderos-4080 (`psionic-serve --elephant-gpu`,
started through the GPU switch):

| Scenario | Doctor's verdict | Exit |
| --- | --- | ---: |
| First start, the shipped set | healthy; 6 cases in about 1.4 s, largest error 0 | 0 |
| Last served other weights, nothing declared | identity: an undeclared change | 13 |
| The same, with the new identity declared | healthy | 0 |
| A reference moved by 0.001 | check: case `wet`, out of tolerance, error 1,000,000 billionths | 15 |
| A set for another identity | check: the set is for another model | 15 |
| A declared identity that isn't the engine's | identity: not the declared identity | 13 |

The doctor wrote no state in any of them.

Chapter 19's end-to-end test, run on the same machine with the toy engine,
now passes the shipped `psionic-toy.selfcheck` and a fresh state file. Its
Pylon log shows the self-check passing (2 cases, largest error 0) before
the relay list and beacon. All 40 checks pass, ASan and UBSan stay silent,
and the state file holds the toy's identity afterwards.

## Supported profile

`selfcheck-v1`:
- sets of up to 16 cases;
- choices of up to 8 options, and noul answers;
- values and tolerance in whole billionths from 0 to 10⁹;
- answers' probabilities as decimals with at most nine places;
- one identity per set, and one state file per Pylon.

## How it is checked

- **Coax checks:** 33 checks in debug and release. They cover:
  - every refusal of a set, at its line;
  - answers in any field order, with skipped fields and each malformed
    shape;
  - comparisons at the tolerance and one past it;
  - the largest error by option;
  - every gate case, including one-character identities;
  - the doctor's order.
- **An independent oracle,** [selfcheck_oracle.py](../tests/selfcheck_oracle.py),
  parses the set and the answers with Python's `json` and `Decimal`. It gives
  the same verdicts for the recorded answers and for each of the example's
  changed copies.
- **Mutation testing** of `selfcheck.coax` kills all 1,527 mutants.
- **The doctor on a real engine:** the six scenarios above, on the 4080.
- **Chapter 19's end-to-end test**, with the self-check in front.

The [verification record](../../docs/verification/psionic-ch159-2026-10-30.md)
has the commands and results.

## What this does not do yet

- **Only at start.** The Pylon doesn't repeat the check while it serves, so
  a fault that starts mid-run goes unnoticed until the next start. A periodic
  check that drains the Pylon on failure is next.
- **Sets are written by hand from recorded answers.** A set should be
  produced, and signed, by whoever publishes the identity's weights, so that
  a Pylon operator can't simply record their own engine's wrong answers.
- **Short prompts only.** Every case is under 128 tokens. Longer requests,
  where reduction order matters more, need their own cases.
- **No Mammoth set yet.** Its serving path is chapter 30b's harness, not the
  server.
