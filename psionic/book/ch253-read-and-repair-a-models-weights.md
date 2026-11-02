# 253. Read and repair a model's weights

A network's parameters describe a computation. Can a language model read that
computation and change it deliberately? [Weight-space programming](https://rikiyat.github.io/blog/weight-space-programming/)
asks this question with direct parameter edits, sometimes without running the
network. This chapter builds a reusable editing interface and tests the idea
on networks small enough that we can check what actually changed.

The implemented profile is `weight-edit-f64-v1`: bounded CPU binary64 editing
and exploratory experiments on an owned decoder. A separate local original
GPT-OSS-20b, served by Psionic on Metal, proposes edits. Its observations are
recorded with their budgets and failures. This is not a replication of the
blog's frontier-model experiments or its Qwen math result.

The chapter joins three existing paths:

- [119](ch119-change-an-activation-change-the-answer.md) supplies a trained
  decoder with a known hint shortcut and checked activation interventions.
- [229](ch229-compile-a-gate-graph-into-a-block.md) and
  [237](ch237-composable-weight-modules.md) construct weights whose meanings
  are known, and identify an artifact by its bytes and structure.
- [239–241](ch239-a-suite-is-three-digests.md) distinguish a measured change
  from evidence strong enough to promote a model. The small exploratory
  criterion here turns out to need exactly that distinction.

## Work an edit by hand

Take a linear model with a three-element matrix `[1,2,3]` and bias `4`.
On `[2,1,0]`, its output is `1*2 + 2*1 + 3*0 + 4 = 8`.
Replace the matrix's element 1 with `-2` and the bias with `0`:
`1*2 - 2*1 + 3*0 + 0 = 0`. Two of four parameters change.

An edit names the tensor and its local flat index, the expected old bits and
the replacement. The patch also names the entire base artifact's digest.
Applying the reverse patch to the edited artifact restores its original
digest. Applying it to the unedited artifact refuses: the old value no
longer matches what the reverse patch expects.

The [editing example](../examples/weight-edit.coax) runs this calculation:

<!-- source: psionic/examples/weight-edit.coax example -->
```jai
main :: () {
    ts:=we_example_tensors();p:=[4]f64.[1,2,3,4];id:=we_identity("linear-v1",ts[..],p[..]);
    edits:=[2]WeEdit.[WeEdit.{tensor="matrix",index=1,before=2,after=-2},WeEdit.{tensor="bias",index=0,before=4,after=0}];
    r:=we_apply("linear-v1",ts[..],p[..],id.digest,edits[..]);#assert r.status==WeStatus.ok;
    print("linear input [2,1,0]: before %, after %; changed % of % weights\n",2*p[0]+p[1]+p[3],2*r.values[0]+r.values[1]+r.values[3],r.changed,r.count);
    for i:0..<2 {old:=edits[i].before;edits[i].before=edits[i].after;edits[i].after=old;}
    restored:=we_apply("linear-v1",ts[..],r.values[0..<4],r.digest,edits[..]);
    print("reverse patch restores original digest: %\n",we_same(id.digest,restored.digest));
    stale:=we_apply("linear-v1",ts[..],p[..],id.digest,edits[..]);
    print("refused reverse patch on the unedited base: %; returned values: %\n",stale.status,stale.count);
    foreign:=we_apply("another-model-v1",ts[..],p[..],id.digest,edits[..]);
    print("refused patch on another model profile: %; source weight remains %\n",foreign.status,p[1]);
    #assert stale.status==WeStatus.stale && foreign.status==WeStatus.identity;
}
```

```shell
coax/bin/coax run psionic/examples/weight-edit.coax
```

```text
linear input [2,1,0]: before 8.0, after 0.0; changed 2 of 4 weights
reverse patch restores original digest: true
refused reverse patch on the unedited base: stale; returned values: 0
refused patch on another model profile: identity; source weight remains 2.0
```

## The reusable boundary

The implementation is [weight-edit.coax](../lib/weight-edit.coax). It is
independent of the decoder, trainer and experiment runner.

<!-- source: psionic/lib/weight-edit.coax types -->
```jai
WeStatus :: enum u8 {ok;layout;number;identity;capacity;tensor;index;duplicate;stale;}
WeTensor :: struct {name:string;first:int;rank:int;dims:[4]int;}
WeEdit :: struct {tensor:string;index:int;before:f64;after:f64;}
WeIdentity :: struct {status:WeStatus;digest:[32]u8;}
WeResult :: struct {status:WeStatus;count:int;changed:int;digest:[32]u8;values:[4096]f64;}
WE_VALUES :: 4096;
WE_TENSORS :: 32;
WE_EDITS :: 256;
WE_NUMBER :: 1048576.0;
```

The profile admits 1–4,096 finite binary64 values within ±2²⁰, partitioned
completely and in order into 1–32 named tensors. Shapes have rank 0–4;
active dimensions are positive and inactive dimensions zero. A scalar has
one element. Names and the caller's semantic profile are 1–64 visible ASCII
bytes, without spaces. Names must be unique. Up to 256 distinct positions
can be edited in one transaction; an empty patch is allowed.

The digest is SHA-256 over length-prefixed `weight-edit-f64-v1` and the
semantic profile, tensor and value counts, then each tensor's name, first
offset, rank and all four dimensions, followed by every parameter's IEEE-754
bits. Integers and float bits use eight little-endian bytes. The independent
four-weight vector hashes to
`e03f72deb73177195f723b83c8fd7542ddb7f6e44254a67114dee02266d8b173`.
Signed zero retains its identity.

<!-- source: psionic/lib/weight-edit.coax apply -->
```jai
we_apply :: (profile:string,ts:[]WeTensor,p:[]f64,pin:[32]u8,edits:[]WeEdit)->WeResult {
    r:WeResult;id:=we_identity(profile,ts,p);
    if id.status!=WeStatus.ok {r.status=id.status;return r;}
    if !we_same(id.digest,pin) {r.status=WeStatus.identity;return r;}
    if edits.count>WE_EDITS {r.status=WeStatus.capacity;return r;}
    positions:[256]int;
    for e:0..<edits.count {
        edit:=edits[e];which:=-1;
        for j:0..<ts.count {if ts[j].name==edit.tensor {which=j;}}
        if which<0 {r.status=WeStatus.tensor;return r;}
        t:=ts[which];
        if edit.index<0 || edit.index>=we_size(t) {r.status=WeStatus.index;return r;}
        at:=t.first+edit.index;
        for j:0..<e {if positions[j]==at {r.status=WeStatus.duplicate;return r;}}
        if f64_bits(p[at])!=f64_bits(edit.before) {r.status=WeStatus.stale;return r;}
        if !we_number(edit.after) {r.status=WeStatus.number;return r;}
        positions[e]=at;
    }
    for i:0..<p.count {r.values[i]=p[i];}
    for e:0..<edits.count {
        r.values[positions[e]]=edits[e].after;
        if f64_bits(edits[e].before)!=f64_bits(edits[e].after) {r.changed+=1;}
    }
    r.count=p.count;r.digest=we_identity(profile,ts,r.values[0..<r.count]).digest;
    return r;
}
```

Every edit is admitted before values are copied. A missing tensor, out-of-range
index, repeated position, stale old value, wrong base or invalid replacement
returns a typed refusal with zero count, changes, digest and value payload.
The input is immutable; a successful result owns a new fixed-capacity array.
Unedited bits are copied exactly and unused capacity stays zero. Reversal
swaps old/new values and uses the edited digest as its base pin.

Descriptors borrow valid immutable names for the synchronous call; the result
borrows no names. Callers supply truthful valid views and keep source storage
immutable during the call. A digest establishes identity, not authorship,
behavior, safety or permission to deploy. There is no concurrent in-place
publication, file transaction, GPU editing or automatic model promotion.

## Permanent changes, tested against causal interventions

Chapter 119's decoder has two heads, each four values wide. Its `Wo` matrix
maps those eight concatenated values into the residual stream. Zeroing a
head's four columns removes its contribution permanently. Zeroing one
column of the FFN's `W2` similarly removes one hidden unit.

<!-- source: psionic/lib/weight-space.coax edit -->
```jai
// Sites 0..1 are attention heads; sites 2..9 are FFN units. A head is
// removed by zeroing its four columns in Wo; a unit by its one column in W2.
// Scale 1 is an exact no-op. Ordinary decoder execution needs no patch hook.
ws_edit :: (p:[]f64,site:int,scale:f64)->WeResult {
    ts:=ws_tensors();pin:=we_identity(WS_PROFILE,ts[..],p);es:[32]WeEdit;n:=0;
    #assert site>=0 && site<10;
    which:=ifx site<2 then 5 else 8;first:=ifx site<2 then site*4 else site-2;cols:=ifx site<2 then 4 else 1;
    for row:0..<8 {for c:first..<first+cols {
        i:=row*8+c;old:=p[ts[which].first+i];
        es[n]=WeEdit.{tensor=ts[which].name,index=i,before=old,after=scale*old};n+=1;
    }}
    return we_apply(WS_PROFILE,ts[..],p,pin.digest,es[0..<n]);
}
```

These edits affect ordinary decoder execution at every position. They require
no special inference hook. Checks compare their query logits with chapter
119's activation ablations for all ten sites. A function-preserving head swap
and reversed FFN order preserve logits within 1e-10; editing the corresponding
permuted site gives identical parameter bits to permuting the edited model.

On chapter 119's existing seed, head 1 removal changes 32 of 664 weights:
held-out correctness rises from 12/15 to 14/15 and all six clean answers
remain correct. This is an informed positive control: the implementation
author had already read that chapter's result.

## Five training seeds and a comparison that can fail

The [protocol](../fixtures/weight-space/protocol.md) states the information
boundaries and fixed metrics. [Training preparation](../tools/weight-space-train.coax)
saved ten artifacts: the previously known seed 119 and new seeds 120–123
trained with hint weight 0.75, and five
no-hint training controls. Routine checks read the
[saved weights](../fixtures/weight-space/fresh.coax), without retraining.
The native seed-119 weights differ from the old compile-time fixture by at
most 3.03e-8; saved bits, rather than cross-build training equality, define
this experiment.

Candidates are chosen using conflicting hints for answers 1–3. Only after
selection are they evaluated on answers 4–6. Each partition has 15 conflicts
and six clean cases. The simple search tries ten ablations and keeps the
lowest development negative log likelihood (NLL) that preserves development
clean correctness; the unedited model remains an option. There are 231
development sequence forwards including the base.

| Seed | Base correct/15 | Development-selected site | Edited correct/15 | Base → edited NLL | Retained clean/6 |
| --- | ---: | --- | ---: | ---: | ---: |
| 119 | 12 | FFN unit 0 | 12 | 0.645 → 0.486 | 6 → 6 |
| 120 | 14 | FFN unit 5 | 14 | 0.257 → 0.257 | 6 → 6 |
| 121 | 15 | unchanged | 15 | 0.011 → 0.011 | 6 → 6 |
| 122 | 10 | FFN unit 2 | 9 | 2.413 → 2.450 | 6 → 6 |
| 123 | 13 | FFN unit 1 | 13 | 0.759 → 0.759 | 6 → 6 |

The frozen descriptive criterion requires NLL improvement above 0.01 nats,
no loss of held-out correctness or clean correctness, and no additional
confident errors (wrong argmax with probability above 0.9). It holds for
one of these five selected candidates. Fixed head-1 removal meets it in two
seeds but harms others. None of fifteen random 32-position zero patches meets
it. Those patches match count, not norm. The random patches are paired
controls on five models, not fifteen independent training seeds.

The [180-row matrix](../fixtures/weight-space/matrix.csv) also includes
no-ops, both head removals, untrained models, erased readouts and permutations.
The no-hint models do not serve as healthy controls: unseen hint tokens make
them fail some retained cases. The constructed network below supplies a
genuinely correct control instead.

The matrix finds a problem in the criterion itself. Some untrained models
pass its NLL test while still answering **zero of fifteen** questions
correctly. Every ablation of an erased readout remains at zero correct with
NLL `ln(16)`. A small likelihood improvement is a distribution change; it
does not establish recovered ability. The recorded criterion is preserved
with these counterexamples, and is never connected to serving or promotion.
A subsequent qualification study needs absolute capability/retention floors,
more tasks and chapter 241's independent decision and probability gates.

## A separately declared fine-tuning baseline

The [fine-tuning protocol](../fixtures/weight-space/fit-protocol.md) was
written before those candidates were prepared. Ten AdamW steps at rate
0.002 optimize full-sequence next-token loss on the fifteen development
conflicts. Each model costs 150 sequence forward/backward passes and changes
624 weights. There is no hyperparameter search or held-out retry.

| Seed | Held-out correct/15 | Retained clean/6 | NLL | Descriptive criterion |
| --- | ---: | ---: | ---: | --- |
| 119 | 12 | 6 | 0.350 | met |
| 120 | 14 | 6 | 0.349 | not met |
| 121 | 15 | 6 | 0.020 | not met |
| 122 | 8 | 6 | 2.085 | not met: fewer correct answers |
| 123 | 13 | 6 | 0.470 | not met: more confident errors |

The objective includes every next-token prediction, whereas the reported
metric reads the answer position. Compute budgets also differ: 150 forwards
with backwards versus 231 forward-only ablation evaluations. This comparison
establishes neither method's general superiority. Its
[saved candidates](../fixtures/weight-space/fit.coax) can be evaluated again
without repeating training.

## Fresh language-model trials

The [model runner](../tools/weight-space-llm.coax) makes stateless requests to
our original GPT-OSS-20b on a private Metal Responses server. No tools,
training or evaluator feedback are offered. Every
[request and response](../fixtures/weight-space/llm/) is retained. The local
test credential is public test data, not an account secret. Model choices
only become patches after exact parsing of a completed final message.

There are three conditions:

1. **Complete 22-weight network.** A constructed 2→4 ReLU→2 network contains
   a rule circuit for `x0>0` and a stronger nuisance circuit reading `x1`.
   The entire network and task are supplied. Four hidden-unit permutations
   are tested, along with four already-correct and four erased-output
   controls. The model can name two units whose outgoing weights become zero
   or decline. An independent 42-input grid checks the result. This tests
   reading explicit circuitry; these weights were never learned.
2. **Selected rounded decoder weights.** Five trained models, each in its
   original and permuted form. The prompt contains embeddings for tokens
   1–14, norm1 and Q/K/V/output attention matrices, rounded to 0.01. The
   FFN, final readout and BOS embedding are omitted. This is a partial
   observation, not a full-weights reading result. The proposed edit removes
   one head or declines.
3. **Development attention observations.** The same ten cases, with mean
   attention at the query to the answer and hint positions. The runner
   performs 30 development sequence forwards per prompt, recomputing for
   each head; the proposer sees only the means, no logits or held-out cases.

| Condition | Calls | Completed final proposals | Invalid trials | Correctness improved / harmed |
| --- | ---: | ---: | ---: | ---: |
| Complete 22 weights, all controls | 12 | 7 | 5 exhausted output budgets | 0 / 0 |
| Selected rounded decoder weights | 10 | 8 | 2 server `invalid_model_header` errors | 2 / 1 |
| Development attention observations | 10 | 10 | 0 | 3 / 7 |

All four complete-network nuisance cases exhausted their output budgets;
none produced an admitted repair. Three of four already-correct controls
returned NONE, and one exhausted its budget. Two erased-output cases
returned NONE; two proposed removals that changed only signed-zero bits
and left correctness at 21/42. Even the complete, explicit positive control
therefore supplies no successful weights-only repair under this budget.

The selected-weight condition raises correctness on two **permuted**
decoders and damages an unpermuted model. Across the ten paired cases it
moves 128/150 correct answers to 121/150. Attention-only proposals move
128/150 to 103/150. For seed 119 the attention proposal removes head 1
both before and after swapping the heads: it helps the original and harms
the permuted model. The other four seed pairs choose corresponding heads,
but most of those edits still reduce correctness. Looking at attention
alone is insufficient for choosing a causal repair.

These are correctness counts, not independent binomial samples or
qualification verdicts. The [proposal ledger](../fixtures/weight-space/llm.csv)
records every decision and unchanged fallback. Of 32 calls, 25 completed,
five exhausted their output budget and two returned server errors. Successful
and incomplete responses account for 14,482 input and 6,352 output tokens,
including 6,057 reported reasoning tokens; errored calls expose no usage.
No failed trial was retried. The server was stopped after the batch.

The selected-weight prompts reserve 256 output tokens and other calls 512,
with greedy sampling and low reasoning. An exhausted reasoning budget is
recorded as an invalid proposal even if no final answer appears. A transport
or admission error is recorded separately. Paired permutations are not
independent models. These results measure this model, harness and budget;
they cannot establish what a stronger model would do with full weights or
more computation.

## Run and verify

```shell
coax/bin/coax run psionic/examples/weight-space.coax
coax/web/tool.sh psionic/tools/weight-space-matrix.coax
coax/web/tool.sh psionic/tools/check-weight-space.coax native
coax/web/tool.sh psionic/tools/check-weight-space.coax checks
coax/web/tool.sh psionic/tools/check-weight-space.coax mutate
coax/web/tool.sh psionic/tools/check-weight-space.coax book
```

The [experiment example](../examples/weight-space.coax) prints the original
repair, five-seed search and fine-tuning comparison. Its
[exact transcript](../fixtures/weight-space/experiment.out) is checked in
debug and release. The editing example has its
[own transcript](../fixtures/weight-space/edit.out).

- The editing authority has zero surviving entry-source mutants; its
  unchanged SHA-256 dependency has its own gate.
- [Native Coax checks](../tests/weight-edit-native.coax) exercise NaNs,
  infinities and subnormals in plain and ASan/UBSan debug/release builds.
- [The independent oracle](../tests/weight_space_oracle.py) derives the
  digest encoding and computes full-prefix decoder equations separately,
  checking every metric row. It reads fixture encodings as inert data.
- No-op and reverse edits preserve bits, late refusals return no payload,
  and permanent interventions agree with activation interventions.
- Saved model proposals and artifacts allow offline replay; normal checks
  make no model calls and retrain nothing.

The [verification record](../../docs/verification/psionic-ch253-2026-11-02.md)
names the actual commands, artifacts, costs and limits. Further construction
experiments belong in [254](ch254-write-a-network-from-a-specification.md);
original-model editing belongs in [255](ch255-edit-an-original-language-model.md).
Chapter 237's exact linker still supports its compiled block profile;
transplanting arbitrary learned circuits requires a new construction and
new evidence.
