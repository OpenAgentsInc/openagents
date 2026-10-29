# 58. Execute DeepSeek V2-Lite

A complete decoder joins latent attention, learned normalization and a mixture
of routed and shared experts. This chapter implements
`deepseek-v2-lite-cpu-wide-v1` for the original V2-Lite-Chat BF16 checkpoint.
It carries every layer through prefill and continuation, then reads all
102,400 vocabulary logits. It is a CPU equation reference with explicit F32
activation boundaries and compensated binary64 products and reductions.

The pinned original is
[DeepSeek-V2-Lite-Chat, revision 85864749](https://huggingface.co/deepseek-ai/DeepSeek-V2-Lite-Chat/tree/85864749cd611b4353ce1decdb286193298f64c7).
Its configuration, recipe and reviewed tensor inventory are retained as
[inert data](../fixtures/deepseek/v2-original/README.md). Remote model Python
was read as a reference and never executed. No framework, converted checkpoint
or external kernel library is a runtime dependency.

## The complete stack

| Component | Original dimensions and rule |
| --- | --- |
| Decoder | 27 layers, width 2,048; learned RMS scales, epsilon 1e-6 |
| Attention | 16 heads; non-rotary query/key 128, rotary 64, value 128; normalized latent 512 |
| Rotary | Original adjacent-pair permutation into split halves; theta 10,000; YaRN factor 40, original context 4,096, beta 32/1, both mscale factors 0.707 |
| Layer 0 | Dense gated SiLU FFN, hidden width 10,944 |
| Layers 1–26 | 64 routed experts, six selected, hidden width 1,408; two shared experts concatenated to width 2,816 |
| Router | Full-expert softmax; selected probabilities retain discarded mass; no correction bias or selected-only renormalization |
| Combination | Apply each route weight after its expert's down projection; sum routed outputs, add the shared branch once, then residual |
| Readout | Independent embedding and vocabulary head; learned final RMS scale |

[Chapter 59](ch059-routers-that-balance-themselves.md) supplies routing.
The [latent attention implementation](../lib/deepseek/mla.coax) offers two
cache forms. A compressed row contains the normalized latent and one shared
rotary key: 576 F32 values per layer and position. An expanded row contains
each head's non-rotary key/value plus the shared rotary key: 4,160 values.
The two forms have separate numerical references because their F32 projection
boundaries differ. Both use causal attention over exactly the supplied prefix.

The [plan](../lib/deepseek/v2-plan.coax) reserves every scratch interval and
all pending cache rows before execution. The [decoder](../lib/deepseek/v2-model.coax)
stages all layer rows, residual diagnostics, routes and full logits. It publishes
them together after final readout succeeds. A bad last head coefficient preserves
all previously committed cache and public outputs. Private work may change.

## Run a complete owned model

The [example](../examples/deepseek-v2.coax) uses an independently authored
small BF16 checkpoint with one dense layer and one MoE layer. It runs five
causal steps in each cache form, including the last context position. Its
shared branch, router and readout are active.

```shell
coax/bin/coax run psionic/examples/deepseek-v2.coax
coax/bin/coax run psionic/examples/deepseek-v2.coax --release
psionic/tests/check_deepseek_v2.sh
psionic/check.sh --only deepseek/v2 --jobs 8
```

<!-- source: psionic/examples/deepseek-v2.coax example -->
```jai
main :: () {
    for mode:0..<2 {
        data:=DVT_DATA;blocks:[2]DvBlock;experts:[3]DvExpert;
        model:=dv_tiny_model(data[..],blocks[..],experts[..]);form:=cast(MlMode)mode;p:=dv_plan(model.shape,form);
        cache:[100]f32;logits:[9]f32;trace:[16]f32;routes:[2]DrRoute;
        floats:[4096]f32;scores:[128]f64;acc:[4]PsionicAccumulator;draft:[2]DrRoute;
        for pos:0..<DVT_TOKENS.count {
            r:=dv_step(model,form,DVT_TOKENS[pos],pos,cache[0..<p.cache],
                DvOutput.{logits=logits[..],trace=trace[..],routes=routes[..]},
                DvWork.{floats=floats[0..<p.floats],scores=scores[0..<p.scores],accumulators=acc[..],routes=draft[..]},false);
            #assert r.status==DsStatus.ok && r.next==pos+1;
        }
        next:=dv_greedy(logits[..]);#assert next.status==DsStatus.ok && routes[0].count==0 && routes[1].count==2;
        print("%: % cache floats; five tokens through both layers\n",ifx mode==0 then "compressed" else "expanded",p.cache);
        print("next token %; routed experts %, %; shared expert branch included\n",next.next,routes[1].ids[0],routes[1].ids[1]);
    }
}
```

A second owned model has three layers, three attention heads, four experts and
unequal dimensions. Both models compare every step's logits, residuals,
ordered routes, weights and complete cache prefixes against independent
cache-free scalar references. Native checks cover real nested aliases,
metadata ownership after drafts are cleared, untouched tails and late failure.

## Admit original bytes before binding tensors

The [binder](../lib/deepseek/v2-weights.coax) authenticates all four original
shards and three metadata files with complete SHA-256. The shard files total
31,413,626,576 bytes. Its fixed profile describes all 5,291 BF16 tensors.
Every payload is covered exactly once, with no gaps, overlap or trailing bytes;
every required name and shape must bind before any model metadata publishes.
The exact inventory count plus lookup of every distinct canonical name also
refuses missing or duplicate tensor names.

The published model borrows immutable file bytes and public block/expert
storage. It never borrows temporary metadata drafts. The caller keeps those
allocations alive, supplies truthful typed extents, exclusive disjoint writable
spans and a sequential cache for the same model, shape and cache form. Native
C observes addresses, maps files, allocates memory and performs effects.
Admission and all numerical and stopping decisions remain in Coax.

## Original generation and its boundary

The original tokenizer has no normalization pass. Its recipe inserts BOS
100000 once, followed by `User: Hi\n\nAssistant:`. The exact eight IDs and
independent ASCII BPE authoring are retained in
[prompt-hi.json](../fixtures/deepseek/v2-original/prompt-hi.json).
This chapter provides that fixed original prompt and a SHA-256-admitted
original-derived token-byte decoder. It does not provide a general V2 text
encoder. Tokenizer max length 16,384 and model rotary capacity 163,840 are
distinct metadata.

The [generation controller](../lib/deepseek/v2-generate.coax) chooses the lowest
ID at a greedy tie. EOS, token limit, context limit and cancellation are
separate typed stops. The [answer decoder](../lib/deepseek/v2-text.coax) removes
only a final typed EOS, refuses BOS and unmapped IDs, preserves ordinary marker
spellings and publishes bytes atomically. Byte-level pieces can split UTF-8;
the consumer interprets complete assembled output. There is no reasoning-tag
stripping for this original V2 chat recipe.

An explicit original qualification, with the four original shards and pinned
metadata already present, runs both forms under one admission. NumPy is
independent oracle tooling only; put its installation on the tooling Python's
path first. The output directory must be new.

```shell
psionic/tests/qualify_deepseek_v2.sh /path/to/original-v2 /tmp/v2-qualification
```

The [frozen policy](../tests/v2-qualification-policy.json) requires every logit
and residual within `0.0002 + 0.00002 * abs(reference)`, route weights within
`0.000002 + 0.00002 * abs(reference)`, exact ordered router IDs, exact greedy
tokens and normal EOS. The independent NumPy implementation evaluates the
whole causal prefix without an incremental cache, using the original BF16
coefficients and explicit F32 boundaries. It executes no upstream model code.

Original results and build identities are recorded in the
[chapter verification record](../../docs/verification/psionic-ch58-2026-10-29.md).
Routine gates replay lossless retained values and use owned fixtures; they
never rerun the complete original model on CPU. This chapter establishes a
bounded CPU reference for the fixed greeting, with separately checked cache
forms. GPU execution, Torch/BF16 arithmetic parity, general tokenizer coverage,
sampling, tools, a quality corpus and useful serving throughput need separate
evidence.
