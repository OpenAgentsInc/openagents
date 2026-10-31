# Mistral models as inference targets

Read-only survey, 2026-10-29. Metadata, configs, model cards and licences were
read from Hugging Face. No weights were downloaded and no upstream code was
run. Part VIII of the [book plan](../../book/PLAN.md) gives the ladder,
and Part XX runs the largest models across a mesh. Nothing here blocks
[Elephant and Mammoth](model-targets.md).

## Why a ladder

[Mistral Large 4](https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B/tree/395730aefd2f928daaf7b69fbd9fefab0ee69b64)
("Le Chonk", released October 29, 2026) combines nearly every feature the
plan has to qualify:

- **Mixture of experts.** About 1T total and 52B active parameters, with 128
  routed experts (4 per token) plus a shared expert.
- **Two attention mechanisms.** Latent attention, and sparse attention with
  an indexer.
- **Layer sharing.** KV and indexer state are shared across layers.
- **FP8 weights** with power-of-two block scales.
- **An embedded draft head** for speculative decoding.
- **Long context and images.** 1M positions and a vision encoder.

Its root safetensors total 1,060.6 GB. Mistral's smaller releases add those
features one or two at a time, so each can be checked on the smallest model
that has it:

| Model (revision) | Licence | Original files | Adds |
| --- | --- | --- | --- |
| [Ministral-3-3B-Instruct-2512-BF16](https://huggingface.co/mistralai/Ministral-3-3B-Instruct-2512-BF16/tree/b6d637bef239) | Apache-2.0 | 15.4 GB | Dense GQA decoder (26 layers, width 3072, 32 query / 8 KV heads), Tekken tokenizer (131,072 entries), Mistral chat/reasoning format |
| [Ministral-3-14B-Instruct-2512](https://huggingface.co/mistralai/Ministral-3-14B-Instruct-2512/tree/29439f81c2be), [Devstral-Small-2-24B-Instruct-2512](https://huggingface.co/mistralai/Devstral-Small-2-24B-Instruct-2512/tree/55c5b41e98c2) | Apache-2.0 | 31.5 / 51.6 GB | FP8 E4M3 block-scaled weights; Devstral is the coding/tool-calling model |
| [Mistral-Small-4-119B-2603](https://huggingface.co/mistralai/Mistral-Small-4-119B-2603/tree/a11f36bebf70) | Apache-2.0 | 241.9 GB | Latent attention (KV rank 256, query rank 1024) with 128 routed + 1 shared expert, top 4 |
| [Mistral-Large-3-675B-Instruct-2512](https://huggingface.co/mistralai/Mistral-Large-3-675B-Instruct-2512/tree/383ffea2c7d6) | Apache-2.0 | 681.5 GB | The same family at 61 layers and width 7168 (KV rank 512, query rank 1536); the first mesh-scale target |
| [Mistral-Large-4-1T-A52B](https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B/tree/395730aefd2f928daaf7b69fbd9fefab0ee69b64) | Mistral Ecosystem Open-Weight License | 1,060.6 GB | Sparse attention, cross-layer sharing, NoPE layers, logit soft-capping, UE8M0 FP8 scales, embedded draft head |

File totals are root-level safetensors from the Hub listing. Some
repositories ship two weight layouts, so the totals are not residency. The
abbreviated revisions are prefixes of the full SHAs that the Hub reports.
Admission pins full file hashes.

## Mistral Large 4 as configured

From `params.json` and `config.json` at the pinned revision:

- **Shape.** 72 layers, width 6144, 64 heads, vocabulary 204,800, untied
  embeddings, and the first layer dense.
- **Latent attention.** KV latent rank 512, with no query compression. Each
  query/key head has 64 rotary and 64 non-rotary dimensions; values have 128.
- **Sparse attention** (`dsa`). An indexer with 32 heads of 128 selects the
  top 2,048 keys. The indexer and the KV cache are FP8. A per-layer `full` or
  `shared` pattern and a `kv_sharing` source map let later layers reuse
  earlier state. Layer 35, for example, is reused by many later layers.
- **Positions.** `nope` repeats "rope, rope, rope, nope", so every fourth
  layer has no rotary positions. YaRN uses factor 2048 over 512 original
  positions, up to 1,048,576.
- **Logits.** Tanh soft-capping at 30.
- **Weights.** FP8 E4M3 in 128×128 blocks with UE8M0 (power-of-two) scales
  and dynamic activation quantization. The vision tower, projector, LM head
  and some attention projections stay unconverted.
- **Draft head** (`mtp`). Three layers, with an "eagle" normalization
  strategy and a DSpark recurrent head.
- **Vision.** A Pixtral-style encoder: 30 layers, patch 14, images up to
  1540 pixels.

An [NVFP4 variant](https://huggingface.co/mistralai/Mistral-Large-4-1T-A52B-NVFP4/tree/d5fdac874617651e8dac18fe8609075700ce8c93)
targets Blackwell GPUs. It is a derived checkpoint, not the original.

## Licence boundary

Large 4's licence is a modified MIT licence. It permits use, modification
and redistribution. A business that offers the model as a service, meaning
it gives third parties meaningful control over inputs or parameters through
an API or hosted endpoint, must obtain a separate commercial licence before
any commercial use. Internal use, end-user products with the model embedded
in specific features, and relaying to models hosted by others are excluded
from that requirement. Running Large 4 for OpenAgents' own agents is internal.
Offering it as an endpoint to others is an owner decision that needs that
licence. The smaller ladder models are Apache-2.0.

Speech candidates: [Voxtral-Mini-4B-Realtime-2602](https://huggingface.co/mistralai/Voxtral-Mini-4B-Realtime-2602/tree/2769294da956)
is Apache-2.0 and suits the streaming-transcription chapter.
[Voxtral-4B-TTS-2603](https://huggingface.co/mistralai/Voxtral-4B-TTS-2603/tree/b81be46c3777)
is CC BY-NC 4.0, which excludes commercial use.

## What each chapter must establish

Each model needs its own evidence: pinned file hashes and complete tensor
inventory; tokenizer and chat-format fixtures; FP8 decode vectors; and
references for selected layers and tokens covering latent attention, indexer
selections, shared-layer reuse, router choices, logits and draft acceptance.
Text comes first. Ministral 3 does not qualify Small 4, and Small 4 does not
qualify Large 4. A layer that matches its reference does not qualify a split
of that model across machines.
