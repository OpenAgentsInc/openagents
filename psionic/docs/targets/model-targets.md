# Early targets: Elephant and Mammoth

Owner priority, 2026-10-28: implement both open-weight decision models natively
in Coax early. Elephant is the first complete real-weight target; Mammoth
follows on the same engine. Their inference delivery does not wait for the
book's autodiff, language-model training, or nanochat pipeline chapters.

Full model execution remains planned. Chapters 1–9 implement the bounded scalar
trainer, storage/numerics, original-weight artifact admission and the
`qwen35-text-v1` tokenizer/request profile. [The project queue](https://openagents.com/projects/psionic)
and [roadmap](../roadmap.md) define the actual dependencies and acceptance.

## Evidence and pins

The historical starting point is
[`0b401561a0^:docs/research/baselabs-decision-models/README.md`](https://github.com/OpenAgentsInc/openagents/blob/853417f290e7cae1df18c081b89b0818432e60af/docs/research/baselabs-decision-models/README.md).
It describes a Rust Psionic implementation and fleet that were removed in the
reset. Its native kernels, gateway, Pylon integration and measurements are
research inputs; they are not installed capabilities of the new Coax library.
Its API free window ended on October 27, and its effort/memory estimates do
not establish a budget or a working Coax port.

Public source files and original tokenizer data were rechecked on October
29. No weights were downloaded, no hosted inference was called, and no
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

## Pinned contracts

[`lib/model-pins.coax`](../../lib/model-pins.coax) freezes three targets, and
[`fixtures/models/pins/`](../../fixtures/models/pins/) holds its manifests,
`psionic.model-pin.v1`, one JSON file per target, byte for byte what the
library writes. Each binds the revision, every pinned file's size and SHA-256
and role, the model-specific bounds, the operations an implementation needs
(with the Psionic unit that owns each) and the refusal identities. Run
`psionic/check.sh --only model-pins`: debug/release checks, zero-survivor
mutation, a byte comparison of the committed manifests
(`coax/web/tool.sh psionic/tools/model-pins.coax check psionic/fixtures/models/pins`)
and an independent [oracle](../../tests/model_pins_oracle.py) that re-derives
layer kinds, BF16 storage, shard pins, vision tensor counts and the nanochat
source hashes from the separate tensor inventories and the bpe-train record.
It is offline: no weights, no network, no upstream code.

| Target | Revision | Files pinned |
| --- | --- | --- |
| `elephant-1` | `b3f8a93462fb1051a9de031e94a7ff380e254072` | 13: card, config, generation config, tokenizer and its config, chat template, `serve.patch`, `readout.json`, `engine.py`, `setup.sh`, `constraints.txt`, processor config, `model.safetensors` |
| `mammoth-1` | `61cec44c281c693992e3d6ebeb44dd3402dfa186` | 15: the same set with its own card, configs, shard index and two weight shards |
| `nanochat` | `92d63d4e8bb4df75c3b71618f31ddde2378b2bcd` | 13: `LICENSE`, the model, tokenizer, checkpoint, optimizer, loader, engine and evaluation sources, the training/SFT/RL/tokenizer scripts and `runs/speedrun.sh` |
| `psionic-pin-toy` | synthetic | one 47-byte readout file that admission hashes offline |

Metadata was rechecked on October 31: both Base Labs revisions are still
their repositories' heads (last modified October 22), and `92d63d4e` is still
nanochat's `master`. Only small text files were fetched, to hash them;
`.gitattributes` and the banner images are not pinned. Shared files
(tokenizer, readout, renderer, engine wrapper) are identical across the two
Base Labs revisions; the configs differ.

**Storage.** Every original tensor is BF16 (760 in Elephant, 1,184 in
Mammoth). The CPU reference decodes BF16 to f32 values with compensated f64
reductions (`cpu-f32-wide-v1`); a device or quantized profile is a separate
identity with its own evidence.

**Text first.** The weights include 333 vision tensors each. The text
profile admits them as inventory only: it loads no vision encoder or
projector and refuses any request with images (`psionic.pin.scope`).

**Bounds.** Requests carry 1–16,384 tokens (the published trained length,
not the base config's 262,144 positions), and either a choice of 1–255
options or a score of 1–10 levels. Readout has 256 slots over a 248,320-token
vocabulary. Over-limit work is refused, never truncated. nanochat's pinned
defaults are recorded (sequence 2,048, vocabulary 32,768 padded to a multiple
of 64, 12 layers, width 768, rotary base 100,000, logit softcap 15, nine
special tokens); its forward/training units are still planned.

**Refusals.** Admission reports the first mismatch: `psionic.pin.target`
(unknown target), `psionic.pin.path` (a file the pin does not name),
`psionic.pin.bytes`, `psionic.pin.digest`, `psionic.pin.scope` (images, or a
decision request to a non-decision target), `psionic.pin.length`,
`psionic.pin.options` and `psionic.pin.score`. `mp_admit` hashes the bytes
itself; `mp_check` compares a described size and digest.

**Licenses.** Metadata is not license text. Both Base Labs cards say
`license: mit` and the pinned trees contain no LICENSE file, so the manifests
record `license_metadata: "mit"` and `license_text: null`; redistributing
converted weights needs an actual license text or a recorded basis first.
nanochat's MIT `LICENSE` is pinned as a file (`license_text: "LICENSE"`).
Files with role `reference` (`engine.py`, `setup.sh`, `constraints.txt`, the
processor config and nanochat's Python) are read, never run on this Mac;
`serve.patch` and the chat template are read as renderer data.

**Reference capture.** Independent cases come from a reviewed, isolated
environment, never this Mac: fetch the pinned files, verify every size and
digest against the manifest before loading, run the pinned upstream runtime
in eager CPU f32, and record tokens, chosen intermediate layer tensors,
recurrent state, logits, probabilities and the final answer with the build
and library versions. Chapters [14](../../book/ch014-execute-elephant.md) and
[15](../../book/ch015-execute-mammoth.md) retain such captures in
`fixtures/models/elephant/` and `fixtures/models/mammoth/`.

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
255 options, and score levels 1–10, with exact fixture-defined edge behavior.
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

## Chapter 9 implementation scope

The [text chapter](../../book/ch009-text-becomes-a-model-request.md)
now implements frozen tagged rendering and original tokenizer semantics under
`qwen35-text-v1` budgets, using Unicode 17.0.0 and integer structured entries.
Both pinned tokenizer originals had SHA-256
`a5cd9732badce41de57e6efce8302930ded1c1188c5f81feb2bd6c24c4a1941f`.
The actual pinned schema permits 1–10 score levels. Independent owned
Unicode/regex/BPE fixtures qualify the stated corpus; no upstream model runtime
was executed. Full request-domain/reference parity and model execution remain
their separate acceptance gates.
