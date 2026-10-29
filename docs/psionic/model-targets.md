# Early targets: Elephant and Mammoth

Owner priority, 2026-10-28: implement both open-weight decision models natively
in Coax early. Elephant is the first complete real-weight target; Mammoth
follows on the same engine. Their inference delivery does not wait for the
book's autodiff, language-model training, or nanochat pipeline chapters.

These are planned capabilities. Current Psionic implements the bounded scalar
trainer only. [The project queue](https://openagents.com/projects/psionic)
and [roadmap](roadmap.md) define the actual dependencies and acceptance.

## Evidence and pins

The historical starting point is
[`0b401561a0^:docs/research/baselabs-decision-models/README.md`](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/research/baselabs-decision-models/README.md).
It describes a Rust Psionic implementation and fleet that were removed in the
reset. Its native kernels, gateway, Pylon integration and measurements are
research inputs; they are not installed capabilities of the new Coax library.
Its API free window ended on October 27, and its effort/memory estimates do
not establish a budget or a working Coax port.

Small public source files and repository metadata were rechecked on October
28. No weights were downloaded, no hosted inference was called, and no
external setup or training script was executed.

| Target | Revision read | Base / text configuration |
| --- | --- | --- |
| [Elephant](https://huggingface.co/baselabs/elephant-1/tree/b3f8a93462fb1051a9de031e94a7ff380e254072) | `b3f8a93462fb1051a9de031e94a7ff380e254072` | Qwen3.5-9B; 32 layers, hidden 4096, FFN 12288, 16 full-attention query heads, 4 KV heads. |
| [Mammoth](https://huggingface.co/baselabs/mammoth-1/tree/61cec44c281c693992e3d6ebeb44dd3402dfa186) | `61cec44c281c693992e3d6ebeb44dd3402dfa186` | Qwen3.5-27B; 64 layers, hidden 5120, FFN 17408, 24 full-attention query heads, 4 KV heads. |

The pinned configs specify three linear-attention layers followed by one
full-attention layer, full-attention head dimension 256 and vocabulary 248320.
The DeltaNet value-head counts differ: 32 for Elephant, 48 for Mammoth;
both have 16 key heads, key/value head dimension 128 and convolution width 4.
Sources: the actual [Elephant config](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/config.json)
and [Mammoth config](https://huggingface.co/baselabs/mammoth-1/blob/61cec44c281c693992e3d6ebeb44dd3402dfa186/config.json).

Each config's `modeling_overrides` specifies block-bidirectional full attention,
causal linear attention, and `slot_lm` readout with 256 slots and head dimension
256. The pinned [readout record](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/readout.json),
[serving patch](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/serve.patch)
and [engine wrapper](https://huggingface.co/baselabs/elephant-1/blob/b3f8a93462fb1051a9de031e94a7ff380e254072/engine.py)
are the implementation references. The artifact-contract task must bind these
files, the pinned underlying OneJev runtime, tokenizer and prompt data together.
A moving model name or a model card alone is insufficient.

The current cards/metadata say `license: mit`, whereas the historical note
recorded `license: other` with an MIT name. The file inventories still contain
no standalone LICENSE file. Retain the exact weight/card/code notices and
provenance when admitting artifacts; record the basis for any redistribution
before publishing converted weights. This planning work redistributes none.

## What the first implementation must do

A decision request contains shared state and typed questions. It produces
probabilities and typed answers, without an autoregressive text-generation
loop. The native engine needs:

1. **Bounded artifact and numerical admission.** Parse safetensors/shard
   metadata and BF16 values in Coax. Bind names, dimensions, tokenizer and
   readout to the requested profile. Host code supplies bytes and storage.
2. **Exact rendering and tokenization.** Preserve state/question/option order,
   special tokens, shared-prefix cut, suffix tokens and answer position from
   the pinned patched renderer. Choice, `noul` and score are distinct types.
3. **The complete hybrid backbone.** Implement embeddings, projections,
   normalization, positions, FFN, full attention and causal Gated DeltaNet.
   Both architectures require all their layers; testing the final head alone
   does not qualify the model.
4. **The correct masks.** Prefix tokens see their prefix block; a question's
   suffix sees that prefix and its own suffix block. Sibling questions cannot
   influence one another. DeltaNet and its convolution remain causal.
5. **Complete prefix snapshots.** Fork full-attention KV, DeltaNet recurrence,
   convolution state, positions and identities. Start with sequential forks;
   batch only after proving the same scoped results. A KV-only copy is wrong.
6. **The trained readout.** Gather option-letter LM logits at the correct
   answer position and normalize the option set. Preserve the published
   choice/`noul`/score semantics and any actually supplied calibration. Do not
   invent a head, a calibration artifact, or a generated-text interpretation.

The initial profile is **text-only**. Published models also accept images,
but admitting vision tensors does not implement preprocessing, a vision
encoder/projector, or image token layout. A separate issue qualifies those.
The renderer must handle one-option choice and the published upper bound of
255 options, and score levels 2–10, with exact fixture-defined edge behavior.
The published trained-length ceiling is 16384; derive actual request/token
limits from the complete pinned serving bundle rather than its much larger
base-config positional capacity. Smaller first execution envelopes must be
named explicitly, with over-limit refusal and no truncation.

## Short delivery path

```text
pinned contracts ── safetensors/BF16 ─┐
shapes ── dense ops ── Qwen layers ──┼─ full attention + causal DeltaNet ─┐
exact tokenizer ── exact prompts ────┼─ hybrid prefix forks ─────────────┼─ Elephant ── Mammoth
                         slot readout + independent reference cases ───┘

shapes ── Coax compute contract ── generated Metal kernels ── Elephant device ── Mammoth device
```

This is a dependency graph, not an instruction to launch agents or allocate
hardware. The same operations support later learning work. A CPU reference is
required for numerical debugging, but full-model CPU throughput need not be
good before the compute branch starts. Metal is the first proposed device;
other backends require their own native compiler/kernel and target evidence.

The original BF16 files alone are roughly 18.8 GB and 54.7 GB according to the
historical inventory. Those figures exclude activations, caches, scratch,
headers, vision data handling and any f32 expansion. Measure a real residency
plan before claiming a model fits a machine. Bounded mapping/staging can be a
separate execution profile. No device rental or purchase follows from this plan.

Qualify original-weight execution before a separately identified quantized
profile. Quantized formats need independent block vectors, malformed-row
refusal, held-out answer/calibration comparison and a measured memory benefit.
A low-memory approximation must not inherit the original profile's evidence.

## Completion evidence

Capture independently produced tokens, intermediate layer tensors, recurrent
state, logits, probabilities and final answers. Freeze tolerances and datasets
before candidate runs. Tiny synthetic fixtures cover masks, branch independence,
metadata and state transitions offline; each full model also needs actual
pinned-weight execution. A reference capture must run only in a reviewed,
explicitly admitted isolated environment, or arrive as retained independently
supplied data. The reference runtime remains tooling.

For each model/backend/profile, record the complete request corpus, actual
peak memory, cold/warm latency, refused requests and numerical results. Keep
numerical conformance, useful decision quality and performance separate.
Do not inherit benchmark scores or old Rust/device qualification.

After native support, connect the existing root System One protocol/codecs
where applicable (`coax/lib/systemone/` and root `proto/`). Version any public
contract extension first. Hosted serving and wallet/provider policy remain
separate consumer work; calling Baseten is not native Psionic execution.
