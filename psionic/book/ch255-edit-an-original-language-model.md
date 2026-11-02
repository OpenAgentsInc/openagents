# 255. Edit an original language model

> **Status: implemented — original-window-v1, bounded CPU interface and recorded original GPT-OSS-20B Metal experiments.**

We can change authenticated original-model weights through an immutable,
versioned window interface, execute the replacement in a private Metal model,
and restore the original bytes. That establishes an editing mechanism. The
experiment below asks a separate question: does a targeted readout edit help
an original language model finish correct arithmetic within a small budget?

**No tested changed-weight condition improves the primary held-out result.**
The selected α1 edit scores **3/12**, versus the unchanged model's **5/12**,
while both retain **24/24** control answers. Earlier completion comes with
more wrong answers. A stronger edit creates repeated-token loops and
malformed framing; all three LLM proposers decline to edit. The finite repair
goal fails, and independent quality qualification is **unverifiable**.

The [weight-space programming article](https://rikiyat.github.io/blog/weight-space-programming/)
motivates direct parameter construction and language-model interventions.
This chapter uses the original model already supported by Psionic. It does
**not** reproduce the article's Qwen3.5-0.8B experiment: that model still
needs its own complete generation/admission path. Chapters
[253](ch253-read-and-repair-a-models-weights.md) and
[254](ch254-write-a-network-from-a-specification.md) establish the smaller
editing and network-construction experiments.

## Admit a window, preserve the original

Chapter 253's `weight-edit-f64-v1` remains a 4096-value binary64 contract.
The new [original-window.coax](../lib/original-window.coax) owns
`original-window-v1`, with a different identity and representation:

<!-- source: psionic/lib/original-window.coax types -->
```jai
OwType :: enum u8 {bf16;f32;unsupported;}
OwStatus :: enum u8 {ok;layout;number;identity;}
OwWindow :: struct {model:[32]u8;shard:[32]u8;name:string;rows:int;cols:int;first:int;count:int;dtype:OwType;}
OwIdentity :: struct {status:OwStatus;digest:[32]u8;}
OwResult :: struct {status:OwStatus;count:int;changed:int;base:[32]u8;digest:[32]u8;bytes:[65536]u8;}
```

A window names a flattened contiguous span of a row-major matrix. Rows and
columns are each 1–1,048,576; count is 1–16,384; `first` and exact byte
length must fit the matrix. Supported encodings are little-endian BF16 and
F32. Identity hashes the version, trusted model/shard pins, tensor name,
shape, offset, count, dtype and **every source bit**, including signed zero.
Authenticating the complete original shard and supplying trustworthy pins
are caller obligations. A digest alone cannot establish provenance.

`ow_apply` checks the source, expected identity, exact proposal count and
finite coefficients within ±2²⁰ before publishing owned bytes. Refusals
return zero count, change count and payload/digests; the source never changes.
An admitted original may contain larger finite values, but a proposed
replacement must satisfy the replacement range. BF16 replacement explicitly
rounds binary64 to F32, then F32 to BF16 with ties to even. This two-stage
rounding matters at boundaries; it is not direct binary64-to-BF16 rounding.
F32 stores the F32 conversion's own bits.

<!-- source: psionic/lib/original-window.coax apply -->
```jai
ow_apply :: (w:OwWindow,source:[]u8,pin:[32]u8,proposed:[]f64)->OwResult {
    r:OwResult;id:=ow_identity(w,source);if id.status!=OwStatus.ok {r.status=id.status;return r;}
    if !we_same(pin,id.digest) {r.status=OwStatus.identity;return r;}
    if proposed.count!=w.count {r.status=OwStatus.layout;return r;}
    for v:proposed {if !we_number(v) {r.status=OwStatus.number;return r;}}
    width:=ow_width(w.dtype);
    for i:0..<w.count {bits:=ow_encoded(proposed[i],w.dtype);different:=false;
        for j:0..<width {b:=cast(u8)((bits>>cast(u32)(8*j))&255);r.bytes[width*i+j]=b;if b!=source[width*i+j] {different=true;}}
        if different {r.changed+=1;}
    }
    r.count=source.count;r.base=id.digest;r.digest=ow_identity(w,r.bytes[0..<r.count]).digest;return r;
}
```

The returned `count` is **bytes**; `changed` counts coefficients with changed
encoded bits. Exact restoration proposes decoded original values against the
edited window's identity. A stale original pin against edited bytes refuses.
The public example flips one actual original coefficient's sign, restores
it exactly and demonstrates that stale-pin failure without loading a model.

The live experiment authenticates all original files through the existing
[gpt-oss binding](../lib/gptoss/weights.coax), maps them read-only, and copies
weights into a private GPU session. The C adapter performs mapping, byte
copies and device effects; the existing Coax model supplies all arithmetic.
A test-only patch hook restores both original rows, uploads both replacement
rows and stops before inference if an effect fails. It does not publish a
partially installed candidate. This is private-process installation, not a
live-serving atomic swap API. No original file is modified.

## Establish original-model parity

The target is `openai/gpt-oss-20b` at revision
`6cee5e81ee83917806bbde320786a8fb61efebee`, existing
`gpt-oss-metal-f32-ff-v1`: 24 layers, width 2880, vocabulary 201088, original
BF16/MXFP4 weights. The pinned original model shard is 13,761,300,984 bytes,
SHA-256 `3340a61d1a0391e8c5b5d3463d18d4c48129a84bbc04a554c762c99020aa06ed`.
The overlay's model identity also binds original configuration/dtype pins,
tokenizer table and numerical profile. Capacity is 512 with the existing
automatic 128-token prefill batch. The initial batch-eight protocol note was
corrected before observing development results.

Before editing, two fresh original GPU logit vectors agree with the retained
independent CPU NumPy reference: **201088 logits each**, maximum scaled
errors **9.98402877e-6** and **9.05275518e-6**, exact argmax IDs **7** and
**200005**. The frozen bound is `.0005 × (1 + |reference|)` per logit.
This is original-weight prefix parity, not independent full-generation
quality verification. No whole-original-model CPU rerun is needed.

## Freeze a failure that actually exists

The [protocol](../fixtures/original-edit/protocol.md) is an exploratory
working-tree protocol, frozen before the relevant observations, with its
amendments preserved. It is not independent preregistration. Prompts use the
pinned low-effort Harmony recipe. The main budget is **64 emitted tokens**,
including analysis/channel/framing tokens; greedy decoding selects the first
argmax on ties. Raw IDs, decoded output, parser status and milliseconds are
retained for every actual sequence.

The initial six nominal arithmetic questions all pass, taking 20–37 tokens.
There is no improvement headroom. They remain retained controls. Before
any edit evaluation or held-out generation, we freeze a harder six-question
development block and twelve-question held-out block. Final partitions are:

| Partition | Primary harder arithmetic | Retained nominal arithmetic/factual/format |
| --- | ---: | ---: |
| Development | 6 | 12 |
| Held-out | 12 | 24 |

All 54 questions and answers are in
[original-study.coax](../lib/original-study.coax). The independent oracle
recomputes arithmetic truths. The baseline gets **2/6** primary development
answers: `137*269` and `728*463` complete correctly in 57/56 tokens; the other
four reach 64 without a completed final. The original screen also exposed a
**scorer bug**: `H₂O` was valid but version 1 rejected it. Before candidate
evaluation, version 2 adds generic Unicode subscript-digit normalization.
The original raw output and false version-1 boolean remain in the ledger.
This is a scorer failure, not evidence of an incorrect model answer.

Reported accuracy requires a completed recipient-free assistant `final`
ending in return. Normalize ASCII case/punctuation/whitespace and subscript
digits, then compare the whole remaining alphanumeric/minus string with the
canonical answer. Analysis and incomplete output earn no credit. This is
strict requested-answer matching, not broad semantic grading. Format cases
are subject to that same case-insensitive rule; they do not prove obedience
to uppercase instructions.

A repetition diagnostic searches spans of 3–16 token IDs repeated three
consecutive times. The baseline development failures trigger none. We have
reproducible **budget/completion failures**, not the blog's demonstrated
looping problem. A 64-token limit alone is not a malformed answer or a loop.

## Orient the edit and work it by hand

For readout `z = W h`, rows are vocabulary tokens and columns are hidden
features. For unit direction `d`, removing it gives
`W' = W - (W d) dᵀ`; then `W'h = W(h-d(dᵀh))` before rounding.
For `W=[[1,2],[3,4]], d=[1,0], h=[5,6]`, the edited matrix is
`[[0,2],[0,4]]`, giving `[12,24]`, exactly the original matrix applied to
`[0,6]`. The admitted-input helpers support non-unit directions by dividing
by their squared length:

<!-- source: psionic/lib/original-study.coax rank -->
```jai
os_rank :: (p:[]f64,rows:int,cols:int,u:[]f64,v:[]f64,alpha:f64)->[16384]f64 {
    out:[16384]f64;for row:0..<rows {for col:0..<cols {out[row*cols+col]=p[row*cols+col]+alpha*u[row]*v[col];}}return out;
}
os_remove :: (p:[]f64,rows:int,cols:int,direction:[]f64)->[16384]f64 {
    u:[16384]f64;den:=0.0;for x:direction {den+=x*x;}#assert den>0;
    for row:0..<rows {for col:0..<cols {u[row]+=p[row*cols+col]*direction[col]/den;}}
    return os_rank(p,rows,cols,u[0..<rows],direction,-1);
}
```

The original-model recipe instead encourages an earlier final channel:
`ΔW = α (e_final - e_analysis) dᵀ`. The literal tokenizer resolves
`analysis` to row **35644**, `final` to **17196**. Each is a separate
2880-coefficient BF16 window in `unembedding.weight [201088,2880]`.
All other weights remain original. We average per-case L2-normalized final
RMS/readout activations at the six primary development channel prefixes,
then normalize the mean to length **1/16**, fixed before fitting.

This is rank one **before BF16 rounding**. Independently rounded two-row
updates can have rank two. It changes most coefficients in these two rows,
even though they are a small fraction of the 579,133,440-coefficient readout.
Neither small global fraction nor channel interpretation establishes locality
of behavior. Suppressing analysis may discard useful reasoning.
The selected α1 edit changes **5685/5760** window coefficients (98.7%),
with squared delta norm **0.007807305954730573**: about 20.3% of the original
two rows' joint L2 norm. Its 0.000982% share of the readout's coefficients
does not make it a small perturbation of these selected rows.

For each of the thirteen actual main weight/prompt conditions (IDs 0–12),
a fixed development prefix checks that all 2880 hidden activation bits stay
unchanged. The two physically edited GPU logits agree with independently
computed `base_logit + Σ(rounded_new-old)*h` under the same scaled bound.
No-op logits match exactly. This checks the real encoded edit against an
output/activation intervention at that prefix, not complete generated-trace
parity for arbitrary interventions. QK/OV products and edits deeper in the
network remain untested here.

## Compare the complete frozen matrix

IDs 0–15 are fixed before held-out evaluation. Sampling-only uses temperature
.6, top-k 20, top-p .95, seed `255+case`. It reads GPU logits and uses the
existing Coax host categorical sampler; this does not qualify a native Metal
sampling kernel. Forced final appends three channel/framing tokens to the
prompt without editing weights.

Three random controls permute the α4 replacements **among columns sharing
the same original BF16 codepoint**, separately per row. This preserves the
exact changed count and multiset of squared deltas, not just an approximate
norm. It is a constrained permutation, not a uniform random direction.
It preserves each source-codepoint group's mean update, so substantial
alignment can remain. Encoded delta cosines with the authored α4 edit are
**.40646, .48910, .39883**. All three random conditions' 54 answer-token
suffixes match forced-final prompting after their three emitted framing
tokens. Their results do not establish a uniquely useful learned direction.

The fitting baseline optimizes **one scalar α**, with fixed direction and a
two-class final-versus-analysis loss on six saved development activations:
200 full-batch AdamW steps, learning rate .05, betas .9/.99, epsilon 1e-8,
no decay, clamp [-4,4]. Saved α is **0.8624249821905133**. This is channel-only
rank-one fitting, not full LoRA, next-token SFT or a same-compute full-model
training comparison. The derivative and independent 200-step reconstruction
are checked. No hyperparameter retry occurs.

Three fresh text-only `claude-opus-5-5` low-effort calls see the selected
weights alone, weights plus supplied numerical row statistics, or those plus
supplied development scores and activation projections. None sees held-out observations.
They have no tools, MCP, hooks, browser or persistent session. Each must
return `[analysis_scale,final_scale,alpha]`, with scales 0–2 and integer α
in [-4,4]. The first two arms must use α0 because their prompts do not
provide the activation-derived direction. The author selected row labels,
mechanism and bounds; these are not raw uninterpreted full-model weights.

**All three return `[1,1,0]`**, declining to edit. They receive no repair
credit. Explicit same-digest receipts carry baseline greedy observations;
these are zero new model sequences. The no-op control still physically
executes all questions, and a separate negative-edit-then-restore control
physically executes one baseline development question.

Selection excludes unchanged encoded candidates. Development primary
accuracy, retained accuracy, then fewer emitted tokens rank the candidates;
earlier ID breaks ties. This chooses ID **6**,
α1: **1/6 primary, 12/12 retained**, 108 tokens across both groups. The
baseline already scores 2/6. This is the best changed diagnostic candidate,
not a development improvement. Held-out answers never change that selection. For α1 and α2, all 54
generated answer-token suffixes exactly match forced-final prompting after
removing the edit's three emitted channel/framing tokens. This observed
equivalence supports a channel-selection interpretation within these cases.

| ID / condition | Dev correct /6 | Held correct /12 | Held complete /12 | Retained held /24 | Primary held emitted tokens |
| --- | ---: | ---: | ---: | ---: | ---: |
| 0 unchanged greedy | 2 | 5 | 5 | 24 | 720 |
| 1 sampling only | 1 | 4 | 4 | 24 | 723 |
| 2 force-final prompt | 1 | 3 | 12 | 24 | 50 |
| 3 physical no-op | 2 | 5 | 5 | 24 | 720 |
| 4 zero analysis row | 1 | 2 | 2 | 24 | 762 |
| 5 zero final row | 0 | 0 | 0 | 0 | 768 |
| 6 authored α1, selected | 1 | 3 | 12 | 24 | 86 |
| 7 authored α2 | 1 | 3 | 12 | 24 | 86 |
| 8 authored α4 | 0 | 0 | 0 | 0 | 650 |
| 9 random seed255 | 1 | 3 | 12 | 24 | 86 |
| 10 random seed256 | 1 | 3 | 12 | 24 | 86 |
| 11 random seed257 | 1 | 3 | 12 | 24 | 86 |
| 12 channel-only AdamW | 1 | 3 | 12 | 24 | 86 |
| 13 LLM weights only, declined | 2 | 5 | 5 | 24 | 720† |
| 14 LLM + row statistics, declined | 2 | 5 | 5 | 24 | 720† |
| 15 LLM + dev/projections, declined | 2 | 5 | 5 | 24 | 720† |

† Carried baseline observations, **zero new sequences or emitted tokens**.
Baseline primary held-out tokens per completed answer are **144**; sampling
is **180.75**; the selected edit is **7.17**, with fewer correct answers.
Forced final uses **4.17 emitted tokens per completion**, plus its three extra
prompt tokens per question. These costs include failed traces in the numerator.

For held-out `314*159`, the baseline reasons and returns the correct `49926`
in 57 tokens. The selected edit immediately returns **`49866`** in six.
Forced-final prompting returns that same wrong number in three emitted
tokens, after its extra three prompt tokens. For `1653*743`, the baseline's
analysis already contains `1228179` but reaches the budget before finishing
the final channel; the edit completes the wrong **`122,5899`**. Its normalized
answer is `1225899`, not the required `1228179`. These
[raw baseline](../fixtures/original-edit/runs/44.json) and
[edited](../fixtures/original-edit/runs/644.json) traces distinguish a
budget-truncated correct calculation from a completed incorrect answer.

Increasing α to four is destructive: **0/12** primary held-out answers
complete. Ten traces contain a repeated 16-token span, while two terminate
after five tokens with malformed status 4. This edit induces loops on a
baseline that did not exhibit that detector's looping pattern. Stronger
channel pressure is not a monotone improvement.
The [trace](../fixtures/original-edit/runs/843.json) repeatedly emits the
literal token `final`. These readout rows affect every position, including
answer bodies, rather than only channel selection boundaries.

The primary finite goal is at least **two additional held-out correct
answers** and **zero retained loss** versus ID0 at the same 64-token budget.
The [full CSV](../fixtures/original-edit/matrix.csv) additionally records
completion, limits, repetition, parser status, tokens and loss proxies.
Parser-non-ok counts include expected incomplete status 6; they are not all
syntax failures. Tokens per completed answer divide all emitted tokens in
the group by completed answers, so failed traces still cost tokens.

Before opening held-out generation, we also froze two extra sampling streams
(seeds `511+case`, `767+case`) and an unchanged greedy **128-token** arm.
These compare sampler sensitivity and more generous compute. They neither
select an edit nor earn the 64-token finite goal. Three sampler streams are
not independent greedy-model baseline blocks.

| Unchanged-weight condition | Primary held correct /12 | Completed /12 | Retained held /24 | Primary held emitted tokens |
| --- | ---: | ---: | ---: | ---: |
| Sampling seed255+case | 4 | 4 | 24 | 723 |
| Sampling seed511+case | 4 | 4 | 24 | 696 |
| Sampling seed767+case | 4 | 4 | 24 | 747 |
| Greedy, 128 tokens | 9 | 9 | 24 | 1012 |

The 128-token arm also scores **5/6** on primary development. All three
remaining held-out failures hit 128 tokens; development `9876*5432` does too.
Increasing the budget improves this worksheet with unchanged weights, at
40.6% more primary held-out emitted tokens. It does not establish that every
failure disappears with more compute. Saved 128-token traces preserve the
64-token baseline's entire generated prefix on all questions.

## Measure loss without confusing it with repair

For all 54 questions, cache original full-vocabulary logits and final RMS
activations at a **forced-final reference prefix**, before the first answer
token. A readout-only edit permits an exact two-row counterfactual from those
vectors. The report computes temperature-one NLL of the canonical answer's
**first BPE token**. A confident-wrong proxy means another first token has
probability over .9 at that fixed prefix. It is neither whole-answer
confidence nor sequence language-model loss. The protocol's original
“first-channel” wording is corrected by the [measurement clarification](../fixtures/original-edit/measurement-notes.md); the captured prefix determines what is measured.
Forced-final prompting and
sampling changes share the unchanged-weight proxy; it does not model their
entire generation distributions.

| Condition | Primary held mean first-token NLL | Confident wrong first token /12 |
| --- | ---: | ---: |
| Unchanged weights | 0.974181565 | 0 |
| Selected α1 | 0.974182110 | 0 |
| Authored α4 | 1.769289712 | 1 |
| Channel-only AdamW | 0.974181839 | 0 |

The selected edit's proxy is almost unchanged despite worse completed-answer
accuracy. α4 has only one confident-wrong first token under this prefix,
although none of its ordinary-prefix held-out answers completes. This proxy
can miss framing failures and later arithmetic errors.

Relative NLL improvement alone cannot establish repair, as chapter 253's
untrained-model controls showed. The chapter-241 gate receives twelve held-out
task items but **zero independent baseline blocks**, and returns
**unverifiable (no_floor)**. No statistical superiority, retained general
capability, shadow promotion or serving adoption is authorized. Chapter
[224](ch224-shadow-a-new-model-then-promote-it.md) remains the integration
point after independent quality qualification.

## Count the work and keep the failures

There are **865 actual generated sequences**, **21,109 emitted tokens** and
**61,080 generation-prompt tokens**, plus **162 explicitly carried comparison
observations**. Thirteen main physical weight/prompt/sampler conditions each
run 54 questions, three supplementary conditions each run 54, and rollback
runs one. Carried LLM declines add no inference.

Capture, parity and edit equivalence require **141 additional prefills**
(10,054 token positions) and **85 additional single steps**. This includes
the answer capture's redundant first prefill; it is real work. Total logical
driver requests are **1006 prefills / 71,134 prefill token positions** and
**20,329 single steps**. Cache resets, uploads, authentication, report softmaxes,
compilation and audits are additional work; these counts are not GPU FLOPs.
The [ledger](../fixtures/original-edit/ledger.csv) is independently checked.

Per-generation measured wall time sums to **1914.827 seconds**. Nine later
phase commands, including repeated authentication and setup, total
**2768.841 seconds**, with a maximum of **540.354 seconds**. Initial
screen/stress total wall time is missing, so the latter is not the complete
experiment's elapsed time. Exact no-op output IDs match baseline on all 54
questions; negative-edit-then-restore matches all 57 generated IDs of its
baseline control.

The three proposer calls use **6 uncached input**, **155,477 cache-creation
input**, **zero cache-read**, and **139 output tokens**, including 112
provider-reported thinking tokens. Reported list cost is **$1.246620**;
actual payment and accelerator time are not inferred. Serial client wall
time totals **23.053 seconds**. Each finishes in one turn, without tools or
spawned subjects; the full responses retain usage and stop reasons.

Channel fitting performs **200 updates / 1200 scalar sample-gradient
evaluations**, taking 9 ms in the preparation observation. It reuses six
saved channel activations and performs no original-model forward during
fitting. The offline oracle's separate reconstruction is verification work,
not an additional candidate training run. There are no environment steps
or game/control rollouts in this original-model matrix.

Each live phase authenticates the original bytes and loads a private GPU
copy. Per-sequence time includes prefill, logit readback, token selection and
steps, but not startup. Shared-host times are observations, not portable
throughput claims. Initial screen/stress total command time and binary hashes
were not captured; later build hashes and exact historical driver snapshots
are retained. No later hash is attributed to the initial executable.

Engineering failures are retained in the verification record: initial
non-Coax foreign-ABI assumptions failed compilation, capturing giant generated
C lines through the process-output limit truncated them, and mutation found
missing boundary checks in the authored tests. Primitive-view effects/raw
compiler-output redirection and isolated boundary tests correct these issues.
They are setup/check failures, not model-quality trials. The existing tiny-Metal
harness also initially checks all 48 names against 45 active kernels; its
independent preflight now follows the published runtime count. The corrected
tiny regression passes, with the initial failure retained. No mutant was marked
equivalent to make a gate pass. The scorer failure and all model failures
remain visible rather than being replaced by successful reruns.

## Run the bounded example and audit offline

The [example](../examples/original-weight-edit.coax) uses only two captured
original rows and the frozen result ledger. It executes byte editing,
restoration, stale refusal and the worked projection, then prints the
recorded evaluation. It does not run the original model on CPU.

<!-- source: psionic/examples/original-weight-edit.coax example -->
```jai
main :: () {
    w:=OwWindow.{model=OE_MODEL,shard=OE_SHARD,name="unembedding.weight",rows=201088,cols=2880,first=35644*2880,count=2880};
    source:=OE_ROWS[0];pin:=ow_identity(w,source[..]);#assert pin.status==OwStatus.ok;
    p:[2880]f64;for i:0..<2880 {p[i]=ow_value(source[..],i,OwType.bf16);}p[0]=-p[0];
    edit:=ow_apply(w,source[..],pin.digest,p[..]);#assert edit.status==OwStatus.ok && edit.changed==1;
    p[0]=-p[0];back:=ow_apply(w,edit.bytes[0..<edit.count],edit.digest,p[..]);
    #assert back.status==OwStatus.ok && we_same(back.digest,pin.digest);
    for i:0..<5760 {#assert back.bytes[i]==source[i] && source[i]==OE_ROWS[0][i];}
    stale:=ow_apply(w,edit.bytes[0..<edit.count],pin.digest,p[..]);#assert stale.status==OwStatus.identity && stale.count==0;
    print("original window: % BF16 values, % bytes, % changed; exact restoration true\n",w.count,edit.count,edit.changed);
    print("stale base: identity refusal, published bytes %\n",stale.count);
    weights:=[4]f64.[1,2,3,4];direction:=[2]f64.[1,0];removed:=os_remove(weights[..],2,2,direction[..]);
    print("rank-one removal: [%,%;%,%], output [%,%]\n",removed[0],removed[1],removed[2],removed[3],removed[0]*5+removed[1]*6,removed[2]*5+removed[3]*6);
    for id:[5]int.[0,2,5,6,12] {r:=OE_RESULTS[id];
        print("recorded candidate %: primary dev %/%, held %/%; retained held %/%\n",id,r[1],r[0],r[7],r[6],r[10],r[9]);
    }
    print("quality: %\n",OE_QUALITY);
}
```

[Exact transcript](../fixtures/original-edit/example.out), checked in debug
and release. Focused phases run serially at low priority, each below fifteen
minutes:

```shell
nice -n 10 coax/bin/coax run psionic/examples/original-weight-edit.coax --run-steps 2000000
nice -n 10 coax/web/tool.sh psionic/tools/check-original-edit.coax checks
nice -n 10 coax/web/tool.sh psionic/tools/check-original-edit.coax mutate
nice -n 10 coax/web/tool.sh psionic/tools/check-original-edit.coax native
nice -n 10 coax/web/tool.sh psionic/tools/check-original-edit.coax book
```

The two admission authorities kill **123 + 45** entry-source mutants, one
worker. Native debug/release, plain and ASan/UBSan builds check nonfinite
proposals and all **65,280 finite BF16 codepoints**. The independent
[oracle](../tests/original_edit_oracle.py) reconstructs original parity,
encoded candidates, identities, random matching, AdamW, output framing,
accuracy, selection, loss proxies and accounting. The
[verification record](../../docs/verification/psionic-ch255-2026-11-02.md)
provides commands, provenance and limitations.

Live reproduction requires the already admitted original checkpoint and
Apple Metal. Build with `check-original-edit.coax build`; then use its
`run MODE ORIGINAL_DIR` phases. For a new capture, use a disposable checkout,
preserve its committed `psionic/fixtures/original-edit` elsewhere, and create
an empty directory at that path with the protocol copied back first. A fresh
checkout alone still contains the saved observations, which refuse replacement.
Offline checks never invoke a live
proposer, retrain the scalar, download a model or load full original weights.

The justified connections are immutable tensor windows, original-model
admission and Metal execution, pinned Harmony tokenization, existing sampling
and AdamW, and chapter 241's ability to say unverifiable. Arbitrary circuit
transplants, full LoRA comparisons, QK/OV repairs, genuine loop repair,
Qwen3.5 support, multiple original models and live shadow replacement remain
open. This chapter implements and measures the bounded experiment, including
its failures; it does not establish general weight-space programming ability.
