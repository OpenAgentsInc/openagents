# 17. Qualify honestly

Chapter 14 compared Psionic with an independent reference and wrote down how
close they were. This chapter asks a harder question: what would it take for
that comparison to count as qualification, and what does the evidence we have
actually support?

The rules come from an earlier attempt to run DeepSeek V4.1, described in the
[DeepSeek target notes](../docs/targets/deepseek.md). Its numerical parity
never passed, and some of its evidence was wrong in ways that looked like
success. A command printed `passed: true` and exited 0 for a report that
failed. A checker accepted forged result lines. A missing profile was treated
as "qualified". Tolerances were changed after the data had been read. A router
check measured an F64 stand-in instead of the values the GPU computed. Each
rule below closes one of those holes:

- Compare like with like: decode with decode, a replayed token stream with a
  replay, one block with one block.
- Write the comparison policy before reading the data it judges, and give it
  an identity that changes when any threshold changes.
- Compare values that the named arithmetic profile produced, not a surrogate.
- Bind every result to the exact build: compiler, sources, profile and model
  files. A field that was not recorded is unknown, and unknown is never a pass.
- Make the process exit status the verdict, so that nothing can print one
  answer and return another.
- Recompute a result from its evidence; never trust a summary of it.
- State a failure with its units and the value that caused it.

Run the [example](../examples/qualify-elephant.coax). It needs no model files:

```shell
coax/bin/coax run psionic/examples/qualify-elephant.coax
```

Its first report is chapter 14's evidence as it was recorded:

```jai
chapter 14, layer 0 of token 32
  policy 1a20462a8028a3b97a261de3518793171d8da0057d897779096568300c6ab45a (one_block)
  rule: 4096 values; each |candidate - reference| <= 3e-06 hidden-state units
  measured: largest |difference| 1.430511474609375e-06 hidden-state units at value 3994 (candidate 7.504566192626953, reference 7.504567623138428); 0 of 4096 over the limit
  unknown: sources, profile (not recorded)
  verdict: unknown (claimed: agrees)
chapter 14, decision probabilities
  policy 2e1baf5507e6d111a6060b1dc1dbaafbce222f24b1e97b9158ef08ce299822d7 (replay)
  rule: 2 values; each |candidate - reference| <= 0.0005 probability; total variation <= 0.0005; top 1 must match unless the reference margin <= 0.0005 and the regret <= 0.0005
  measured: largest |difference| 3.7422332788139556e-08 probability at value 0 (candidate 0.757196285, reference 0.7571962475776672); 0 of 2 over the limit; total variation 2.98023224015731e-08; top 1: candidate [0], reference [0], margin 0.5143925547599792, regret 0.0
  unknown: sources, profile (not recorded)
  verdict: unknown (claimed: agrees)
chapter 14, decision slot mass
  policy ff89eed1a44a088e856d36bf292049ecf6d1bb0135207142b64ce68960a1998f (replay)
  rule: 1 value; each |candidate - reference| <= 0.0005 probability
  measured: largest |difference| 1.8926036071675867e-07 probability at value 0 (candidate 0.228232722, reference 0.22823253273963928); 0 of 1 over the limit
  unknown: sources, profile (not recorded)
  verdict: unknown (claimed: agrees)
exit status 2
```

Every number agrees within its policy. The verdict is still unknown, because
chapter 14 did not record which sources were built or which arithmetic that
build executed. The rest of this chapter explains how the library reaches that
answer, and what a complete record would change.

## A policy is a frozen record

A policy says what is compared and how closely. The library is
[`qualify.coax`](../lib/qualify.coax):

<!-- source: psionic/lib/qualify.coax policy -->
```jai
QyMode :: enum u8 {unknown;decode;replay;one_block;}
QyUnit :: enum u8 {unknown;hidden;probability;score;}
QyPolicy :: struct {
    mode:QyMode;            // like modes only: decode, replay or one block
    unit:QyUnit;            // what one compared value measures
    values:int;             // exact number of values compared
    value_abs:f64;          // largest |candidate - reference| for any value, in unit
    distribution:bool;      // the values are one probability distribution
    tv_max:f64;             // largest total variation; zero unless distribution
    top:int;                // 0, or k: the top-k sets must agree...
    near_tie:f64;           // ...unless the reference margin is at most this
    regret_max:f64;         // and the reference value lost is at most this
    profile:[32]u8;         // arithmetic the candidate must have executed
    reference_profile:[32]u8;
    dataset:[32]u8;         // SHA-256 of the inputs, fixed before outputs are read
}
```

There are three kinds of measurement:

- **Per-value tolerance.** Every value must be within `value_abs` of the
  reference, in the policy's unit. The report names the largest difference,
  where it occurred and how many values exceeded the limit.
- **Total variation.** For a probability distribution, half the sum of the
  absolute differences: the largest probability that the two answers can
  assign differently to any set of options.
- **Top-k agreement with near-tie regret.** The candidate's k largest values
  must select the same set as the reference's. A different set is accepted
  only when the reference itself nearly ties (its margin between the k-th and
  the next value is at most `near_tie`) and the choice costs little (the
  reference values lost, the regret, are at most `regret_max`). With k = 1
  this is top-1 agreement for a decision; with larger k it is the check for a
  mixture-of-experts router, where one-ulp differences can legitimately flip
  a near tie. That is what happened to DeepSeek from layer 1 on.

The policy has an identity: the SHA-256 of every field in a fixed byte order.
A threshold that is not used must be zero, and negative zero is refused, so
one meaning has one encoding:

<!-- source: psionic/lib/qualify.coax digest -->
```jai
qy_policy_valid :: (p:QyPolicy)->bool {
    if p.mode==QyMode.unknown || p.unit==QyUnit.unknown {return false;}
    if p.values<1 || p.values>QY_MAX_VALUES || !qy_threshold(p.value_abs) {return false;}
    if p.distribution {if !qy_threshold(p.tv_max) {return false;}} else if f64_bits(p.tv_max)!=0 {return false;}
    if p.top<0 || p.top>QY_MAX_TOP || p.top>=p.values {return false;}
    if p.top==0 {if f64_bits(p.near_tie)!=0 || f64_bits(p.regret_max)!=0 {return false;}}
    else if !qy_threshold(p.near_tie) || !qy_threshold(p.regret_max) {return false;}
    return qy_name_ok(p.profile) && qy_name_ok(p.reference_profile) && qy_known(p.dataset);
}
// Every field, in a fixed order: change any threshold and the identity changes.
qy_policy_digest :: (p:QyPolicy)->[32]u8 {
    out:[32]u8;if !qy_policy_valid(p) {return out;}
    buf:[172]u8;for i:0..<25 {buf[i]=QY_POLICY_TAG[i];}
    buf[25]=cast(u8)p.mode;buf[26]=cast(u8)p.unit;buf[27]=ifx p.distribution then 1 else 0;
    qy_put(buf[..],28,cast(u64)p.values);qy_put(buf[..],36,cast(u64)p.top);
    qy_put(buf[..],44,f64_bits(p.value_abs));qy_put(buf[..],52,f64_bits(p.tv_max));
    qy_put(buf[..],60,f64_bits(p.near_tie));qy_put(buf[..],68,f64_bits(p.regret_max));
    for i:0..<32 {buf[76+i]=p.profile[i];buf[108+i]=p.reference_profile[i];buf[140+i]=p.dataset[i];}
    status:=sha256_hash_checked(buf[..],out[..],false);#assert status==SHA_OK;
    return out;
}
```

A digest cannot prove when a policy was written. What it does is make a
change visible: the identity is recorded with the result, and a looser
threshold produces a different identity. The way to use it is to commit the
policy and its digest first, and only then read the outputs of the data set
that the policy names. The data set's identity is the digest of its inputs,
such as the token IDs, which can be fixed before any model output exists.

## Chapter 14's policies

[`qualify-elephant.coax`](../lib/qualify-elephant.coax) writes down three
policies for chapter 14's evidence and pins their digests:

<!-- source: psionic/lib/qualify-elephant.coax policies -->
```jai
ye_layer0_policy :: ()->QyPolicy {
    return QyPolicy.{mode=QyMode.one_block,unit=QyUnit.hidden,values=4096,value_abs=0.000003,
        profile=qy_name(YE_PROFILE),reference_profile=qy_name(YE_REFERENCE),dataset=qy_text_digest(YE_LAYER0_INPUT)};
}
ye_decision_policy :: ()->QyPolicy {
    return QyPolicy.{mode=QyMode.replay,unit=QyUnit.probability,values=2,value_abs=0.0005,distribution=true,tv_max=0.0005,
        top=1,near_tie=0.0005,regret_max=0.0005,profile=qy_name(YE_PROFILE),reference_profile=qy_name(YE_REFERENCE),dataset=qy_hex(YE_REQUEST_FILE)};
}
ye_mass_policy :: ()->QyPolicy {
    return QyPolicy.{mode=QyMode.replay,unit=QyUnit.probability,values=1,value_abs=0.0005,
        profile=qy_name(YE_PROFILE),reference_profile=qy_name(YE_REFERENCE),dataset=qy_hex(YE_REQUEST_FILE)};
}
// Frozen: a changed threshold changes the digest, and these checks fail.
YE_LAYER0_POLICY :: "1a20462a8028a3b97a261de3518793171d8da0057d897779096568300c6ab45a";
YE_DECISION_POLICY :: "2e1baf5507e6d111a6060b1dc1dbaafbce222f24b1e97b9158ef08ce299822d7";
YE_MASS_POLICY :: "ff89eed1a44a088e856d36bf292049ecf6d1bb0135207142b64ce68960a1998f";
```

The thresholds are chapter 14's own: 0.000003 for the layer-0 hidden state
(the [fixture checker](../tests/elephant_layer0_fixture.py)) and 0.0005 for
the decision statistics (the original-file harness). They were chosen by
chapter 14 after its numbers were seen. That makes chapter 14's request a
development set, not a confirmation set, and the library records it as one.

The modes are those that actually ran. The layer-0 comparison is one token in
one block. The decision is a replay: both implementations processed the same
86 given token IDs as a 49-token prefix and a 37-token suffix, and neither
generated tokens. The candidate profile is `cpu-f32-wide-v1`, the CPU
arithmetic of [chapter 4](ch004-numbers-and-matrices.md); the reference is
PyTorch 2.10.0 on the CPU in float32 with eager attention. Each data set's
identity is the SHA-256 of its inputs: the request's token file, and for layer
0 a line naming the model revision, token 32, position 0 and layer 0.

The evidence itself is chapter 14's checked-in files. A tooling script,
[`qualify_oracle.py`](../tests/qualify_oracle.py), copies them into a
generated Coax fixture: the layer-0 values as their exact f32 bits, the
decision numbers as the exact decimal text of the files. Coax then hashes the
bits and the text again and refuses to compare anything unless the digests
equal those of the files. The same script computes every pinned digest
independently, from the encoding described here rather than from the Coax
code.

## A result binds the build

A comparison alone does not say what was compared. The record names the
policy, the inputs, the model files, the build and the evidence:

<!-- source: psionic/lib/qualify.coax record -->
```jai
QyRecord :: struct {
    policy:[32]u8;          // digest of the policy this result claims
    dataset:[32]u8;         // inputs actually run
    model:[32]u8;           // SHA-256 over the model files' digests
    compiler:[32]u8;        // compiler name and version that built the candidate
    sources:[32]u8;         // SHA-256 over every input of that build
    profile:[32]u8;         // arithmetic profile the candidate executed
    mode:QyMode;            // how the candidate ran
    reference_profile:[32]u8;
    reference_mode:QyMode;
    candidate:[32]u8;       // digest of the candidate values compared
    reference:[32]u8;       // digest of the reference values compared
    role:QyRole;            // development, or confirmation under a policy frozen first
}
```

Every field starts as zero, and zero means unknown. An empty record is
therefore unknown in every field, not a pass by default. The model identity
for Elephant is the SHA-256 of the `sha256sum` listing of `config.json`,
`model.safetensors`, `readout.json` and `tokenizer.json`, using the pinned
digests of chapter 14.

This is what chapter 14 recorded about its own candidate:

<!-- source: psionic/lib/qualify-elephant.coax history -->
```jai
ye_record :: (i:int)->QyRecord {
    r:=QyRecord.{dataset=ye_policy(i).dataset,model=qy_hex(YE_MODEL),compiler=qy_name("coax0 0.16.0"),
        reference_profile=qy_name(YE_REFERENCE),role=QyRole.development};
    // Not recorded in chapter 14: the digest of the sources built, and the
    // arithmetic profile that build executed. They stay unknown.
    if i==0 {r.policy=qy_hex(YE_LAYER0_POLICY);r.mode=QyMode.one_block;r.candidate=qy_hex(YE_LAYER0_PSIONIC_VALUES);r.reference=qy_hex(YE_LAYER0_TRANSFORMERS_VALUES);}
    if i==1 {r.policy=qy_hex(YE_DECISION_POLICY);r.mode=QyMode.replay;r.candidate=qy_hex(YE_DECISION_PSIONIC_VALUES);r.reference=qy_hex(YE_DECISION_TRANSFORMERS_VALUES);}
    if i==2 {r.policy=qy_hex(YE_MASS_POLICY);r.mode=QyMode.replay;r.candidate=qy_hex(YE_MASS_PSIONIC_VALUES);r.reference=qy_hex(YE_MASS_TRANSFORMERS_VALUES);}
    r.reference_mode=r.mode;
    return r;
}
```

Chapter 14's verification record names the compiler, the model revision and
the reference library. It does not record a digest of the sources that
produced the checked-in candidate files, or the compiler flags and host of
that build. We do not fill those in after the fact.

## The verdict is the exit status

`qy_qualify` decides in a fixed order. A known field that contradicts the
policy or the evidence fails. A comparison outside the policy fails. Only
then do unknown fields make the verdict unknown. A complete record agrees on
a development set and qualifies only on a confirmation set:

<!-- source: psionic/lib/qualify.coax qualify -->
```jai
qy_qualify :: (p:QyPolicy,r:QyRecord,candidate:[]f64,reference:[]f64)->QyReport {
    if !qy_policy_valid(p) {return QyReport.{verdict=QyVerdict.failed,cause=QyCause.policy,worst=-1,first_over=-1};}
    x:=qy_compare(p,candidate,reference);x.policy=qy_policy_digest(p);x.unknown=qy_unknown(r);
    wrong:=QyCause.none;
    if qy_mismatch(r.policy,x.policy) {wrong=QyCause.policy_digest;}
    else if qy_mismatch(r.dataset,p.dataset) {wrong=QyCause.dataset;}
    else if qy_mismatch(r.profile,p.profile) {wrong=QyCause.profile;}
    else if r.mode!=QyMode.unknown && r.mode!=p.mode {wrong=QyCause.mode;}
    else if qy_mismatch(r.reference_profile,p.reference_profile) {wrong=QyCause.reference_profile;}
    else if r.reference_mode!=QyMode.unknown && r.reference_mode!=p.mode {wrong=QyCause.reference_mode;}
    else if qy_known(r.candidate) && !qy_same(r.candidate,qy_values_digest(candidate)) {wrong=QyCause.candidate_values;}
    else if qy_known(r.reference) && !qy_same(r.reference,qy_values_digest(reference)) {wrong=QyCause.reference_values;}
    if wrong!=QyCause.none {x.verdict=QyVerdict.failed;x.cause=wrong;return x;}
    if x.verdict==QyVerdict.failed {return x;}
    if x.unknown!=0 {
        x.verdict=QyVerdict.unknown;
        // The lowest unknown field names the cause; the mask is not zero here.
        m:=x.unknown;i:=0;while (m&1)==0 {m=m>>1;i+=1;}
        x.cause=cast(QyCause)i;
        return x;
    }
    x.verdict=ifx r.role==QyRole.confirmation then QyVerdict.qualified else QyVerdict.agrees;
    return x;
}
// The process exit status: 0 only when the verdict meets the claim.
qy_exit :: (x:QyReport,claim:QyVerdict)->int #export_c {
    if claim!=QyVerdict.agrees && claim!=QyVerdict.qualified {return 2;}
    if x.verdict==QyVerdict.failed {return 1;}
    if x.verdict==QyVerdict.qualified {return 0;}
    if x.verdict==QyVerdict.agrees {return ifx claim==QyVerdict.agrees then 0 else 3;}
    return 2;
}
```

The exit status is 0 only when the verdict meets what was claimed: 1 for a
failure, 2 for unknown, and 3 when a development agreement is presented as a
qualification. A profile that differs from the policy's, such as an F64
surrogate standing in for the device's arithmetic, is a known mismatch and
fails.

The gate in [`qualify-gate.coax`](../lib/qualify-gate.coax) prints one case's
report and returns its status. The example prints that status; the
[native host](../tests/qualify.c) returns it as the process exit status, with
no logic of its own:

<!-- source: psionic/lib/qualify-gate.coax gate -->
```jai
// Prints case k's report and returns the status the process exits with.
ye_gate :: (k:int)->int #export_c {
    c:[4096]f64;r:[4096]f64;
    if k==0 {
        names:=[3]string.["chapter 14, layer 0 of token 32","chapter 14, decision probabilities","chapter 14, decision slot mass"];
        worst:=0;
        for i:0..<YE_CASES {
            status:=qy_print(names[i],ye_policy(i),ye_case(i,c[..],r[..]),QyVerdict.agrees);
            if status>worst {worst=status;}
        }
        return worst;
    }
    // Chapter 14's decision values with a complete build record.
    p:=ye_decision_policy();record:=ye_record(1);n:=ye_evidence(1,c[..],r[..]);
    record.sources=qy_text_digest("an example sources digest");record.profile=qy_name(YE_PROFILE);
    bound:=qy_qualify(p,record,c[0..<n],r[0..<n]);
    if k==1 {return qy_print("complete record, development data",p,bound,QyVerdict.agrees);}
    if k==2 {return qy_print("complete record, claimed as qualified",p,bound,QyVerdict.qualified);}
    if k==5 {empty:QyRecord;return qy_print("no record",p,qy_qualify(p,empty,c[0..<n],r[0..<n]),QyVerdict.agrees);}
    if k==7 {bad:=p;bad.tv_max=-bad.tv_max;return qy_print("negative threshold",bad,qy_qualify(bad,record,c[0..<n],r[0..<n]),QyVerdict.agrees);}
    // A deliberately failing candidate: the first probability is 0.001 high.
    c[0]+=0.001;c[1]-=0.001;
    if k==6 {c[1]=f64_from_bits(cast(u64)k+0x7ff0000000000000);}
    record.candidate=qy_values_digest(c[0..<n]);
    if k==3 {return qy_print("deliberately failing candidate",p,qy_qualify(p,record,c[0..<n],r[0..<n]),QyVerdict.agrees);}
    if k==4 {return qy_print("summary edited to pass",p,qy_check_summary(bound,p,record,c[0..<n],r[0..<n]),QyVerdict.agrees);}
    if k==6 {return qy_print("not-a-number candidate",p,qy_qualify(p,record,c[0..<n],r[0..<n]),QyVerdict.agrees);}
    print("no gate case %\n",k);
    return 2;
}
```

The example runs every case:

<!-- source: psionic/examples/qualify-elephant.coax example -->
```jai
main :: () {
    for k:0..<YE_GATE_CASES {
        status:=ye_gate(k);
        print("exit status %\n\n",status);
    }
}
```

With the missing sources digest and profile filled in, chapter 14's decision
agrees, and the same values presented as a qualification do not. The two
reports end:

```jai
  verdict: agrees (claimed: agrees)
exit status 0
```

```jai
  verdict: agrees (claimed: qualified)
exit status 3
```

The sources digest in that case is a stand-in that the example writes; it
shows what the library does with a complete record, not that chapter 14's
build had one.

## Failures are stated precisely

A failing report states the policy, the unit, the counterexample and the
count. Moving the first probability up by 0.001 and the second down by 0.001
gives:

```jai
deliberately failing candidate
  policy 2e1baf5507e6d111a6060b1dc1dbaafbce222f24b1e97b9158ef08ce299822d7 (replay)
  rule: 2 values; each |candidate - reference| <= 0.0005 probability; total variation <= 0.0005; top 1 must match unless the reference margin <= 0.0005 and the regret <= 0.0005
  measured: largest |difference| 0.001000037422332789 probability at value 0 (candidate 0.758196285, reference 0.7571962475776672); 2 of 2 over the limit; total variation 0.0010000076200103875; top 1: candidate [0], reference [0], margin 0.5143925547599792, regret 0.0
  failed (value): 2 of 2 values differ by more than 0.0005 probability; the first is value 0
  verdict: failed (claimed: agrees)
exit status 1
```

The check that pins this behavior is written against the library, with a
complete record, so that a failure cannot be excused by a missing field:

<!-- source: psionic/lib/qualify.coax failing -->
```jai
#test "a failing report fails the command even when its build is complete" {
    p:=qy_test_policy();
    c:=[2]f64.[0.75,0.25];r:=[2]f64.[0.7490234375,0.2509765625];
    record:=qy_test_record(p,c[..],r[..]);
    x:=qy_qualify(p,record,c[..],r[..]);
    #assert x.verdict==QyVerdict.failed && x.cause==QyCause.value && x.unknown==0;
    #assert x.worst==0 && x.difference==0.0009765625 && x.over==2 && x.first_over==0;
    #assert qy_exit(x,QyVerdict.agrees)==1 && qy_exit(x,QyVerdict.qualified)==1;
}
```

A value that is not finite is reported by position with both values, before
any difference is computed, and a probability outside [0, 1] is reported the
same way. The example's last cases show a NaN candidate and a policy with a
negative threshold, which is refused before anything is compared.

## A summary is checked by recomputing it

A summary is a claim about evidence. `qy_check_summary` does not parse it or
trust any field of it: it recomputes the report from the policy, the record
and the values, and rejects the summary unless every field, including each
floating-point bit pattern, is identical. A summary that was edited to read
as a pass fails:

<!-- source: psionic/lib/qualify.coax forged -->
```jai
#test "an edited summary is rejected; the checker recomputes" {
    p:=qy_test_policy();c:=[2]f64.[0.75,0.25];r:=[2]f64.[0.7490234375,0.2509765625];
    record:=qy_test_record(p,c[..],r[..]);
    honest:=qy_qualify(p,record,c[..],r[..]);
    #assert qy_check_summary(honest,p,record,c[..],r[..]).verdict==QyVerdict.failed;
    #assert qy_check_summary(honest,p,record,c[..],r[..]).cause==QyCause.value;
    forged:=honest;forged.verdict=QyVerdict.agrees;forged.cause=QyCause.none;
    x:=qy_check_summary(forged,p,record,c[..],r[..]);
    #assert x.verdict==QyVerdict.failed && x.cause==QyCause.summary && qy_exit(x,QyVerdict.agrees)==1;
}
```

Another check edits each of the report's 19 fields in turn and requires every
edit to be rejected. The example presents the passing report of the complete
record for the failing evidence; it is rejected as `failed (summary)`.

## Run the checks

```shell
psionic/tests/qualify.sh
```

The script checks the generated fixtures and independent digests, runs the
Coax checks in debug and release, and runs `coax mutate` on both authority
files with zero survivors. It then builds the gate natively in debug and
release, with and without ASan and UBSan, runs each case as its own process,
requires each exit status, and requires the combined output to equal the
book's transcript. The [verification record](../../docs/verification/psionic-ch13-2026-10-29.md)
lists the results.

## What this establishes

Chapter 14's evidence agrees with its independent reference under policies
that are now written down and frozen, with the same thresholds chapter 10
used. It is not qualified. Two fields of its build were not recorded, so the
verdict is unknown; and its request was read before its policy was written, so
even a complete record would make it a development agreement. Qualification
needs a confirmation set whose inputs are fixed after the policy digest is
committed and whose outputs are read after that, with every field of the
record bound by the run that produced it. Chapter 16's held-out request corpus
is the natural first confirmation set.

The digest makes a policy change visible; it does not prove when the policy
was written, and it does not sign anything. The transcription of decimal
numbers into Coax is checked by tooling, and the layer-0 bits and the text of
the files are checked by Coax's own SHA-256. The library compares values that
the caller supplies; it cannot tell that they came from the device and profile
the record names. That remains the duty of the harness that runs the model.
