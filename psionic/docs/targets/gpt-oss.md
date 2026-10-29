# gpt-oss as an inference target

Survey updated 2026-10-29. [Chapter 34](../../book/ch34-original-mxfp4-weights.md)
now implements the bounded original-format CPU decoder. Full-model execution
and Harmony remain planned. No weights were downloaded, and no upstream
setup, inference or tool code was run. Psionic would implement the behavior in
Coax; Python stays reviewed reference and export tooling, never a runtime
dependency. Part VII of the [book plan](../../book/PLAN.md) gives the
chapter sequence. The remaining sequence is proposed work, and it does not block
[Elephant and Mammoth](model-targets.md).

## Pins

| Source | Revision read | Terms |
| --- | --- | --- |
| [openai/gpt-oss](https://github.com/openai/gpt-oss/tree/5681a6b6552635e7b29cde94a61ca6a7aee90ebd) | `5681a6b6552635e7b29cde94a61ca6a7aee90ebd` | Apache-2.0 |
| [openai/harmony](https://github.com/openai/harmony/tree/abd677f7ac962629c808197caa1feb9e3e95d2b0) | `abd677f7ac962629c808197caa1feb9e3e95d2b0` | Apache-2.0 |
| [openai/gpt-oss-20b](https://huggingface.co/openai/gpt-oss-20b/tree/6cee5e81ee83917806bbde320786a8fb61efebee) | `6cee5e81ee83917806bbde320786a8fb61efebee` | Apache-2.0 card metadata |
| [openai/gpt-oss-120b](https://huggingface.co/openai/gpt-oss-120b/tree/b5c939de8f754692c1647ca79fbf85e8c1e70f8a) | `b5c939de8f754692c1647ca79fbf85e8c1e70f8a` | Apache-2.0 card metadata |

Admission must still pin each file's hash, the complete tensor inventory and
the exact notices, as chapter 4 does for the decision models. A model card's
license field alone is not artifact provenance.

## What the pinned sources specify

| Property | gpt-oss-20b | gpt-oss-120b | Source |
| --- | --- | --- | --- |
| Parameters, total / active (upstream claim) | 21B / 3.6B | 117B / 5.1B | repository README |
| Layers | 24 | 36 | `config.json` |
| Experts / selected per token | 32 / 4 | 128 / 4 | `config.json` |
| Hidden, expert intermediate | 2880, 2880 | 2880, 2880 | `config.json` |
| Query / KV heads, head dimension | 64 / 8, 64 | 64 / 8, 64 | `config.json` |
| HF-root Safetensors bytes (converted layout) | 13.8 GB | 65.2 GB | Hugging Face file listing |

Both use a vocabulary of 201,088, RMSNorm epsilon `1e-5`, untied embeddings,
and attention biases. `layer_types` alternates sliding attention (window 128)
with full attention, starting with sliding. Rotary positions use YaRN with
base 150,000, factor 32, original length 4,096, `beta_fast` 32 and
`beta_slow` 1, up to 131,072 positions.

The reference model ([`gpt_oss/torch/model.py`](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/torch/model.py))
also shows:

- **Attention sinks.** Each head has a learned sink logit that joins the
  softmax denominator and is then discarded, so a query can put attention
  mass on it without reading any value.
- **Routing.** The sorted top four router logits are softmaxed over just the
  selected experts.
- **A clamped SwiGLU.** The gate is clamped above at 7 and the linear term to
  ±7. The output is `gate * sigmoid(1.702 * gate) * (linear + 1)`. Expert
  projections have biases.

These are exactly the details where a generic MoE or attention block would be
silently wrong.

## Weights are MXFP4 in the original files

The pinned README distinguishes `original/` files used by its Torch/Triton
loader, converted Hugging Face files at the repository root, and its Metal
conversion. The table's HF-root sizes are not original-file inventory pins.
Original checkpoint admission must establish the `original/` files' own sizes,
hashes and complete tensor-pair metadata. The chapter's synthetic original-format
windows establish decoder behavior, not admission of a complete checkpoint.


`quantization_config` names `mxfp4`. Attention, router, embeddings and the LM
head are not converted and stay BF16. Each MoE projection is stored as two
tensors ([`gpt_oss/torch/weights.py`](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/torch/weights.py)):

- **`.blocks`** holds 32 FP4 (E2M1) values per 16 bytes, two per byte.
- **`.scales`** holds one byte per block, read as the exponent `scale - 127`.

The torch reference upcasts these to BF16. For Psionic, MXFP4 is the
**original** artifact format, so it belongs with chapter 4's original-weight
admission. It is not a derived quantization profile with its own quality gate.
The implemented finite-all-codes profile admits scales 0–252, preserves signed
zero and subnormals, and refuses 253–255 before publication. Each admitted
product is exactly representable in `f32`. Original checkpoint scale coverage
is still a separate gate. A BF16
conversion would be a separate, unoriginal artifact.

## The harmony format is required

The models "should only be used with this format". The
[harmony format document](https://github.com/openai/harmony/blob/abd677f7ac962629c808197caa1feb9e3e95d2b0/docs/format.md)
defines:

- **Roles:** system, developer, user, assistant and tool.
- **Reasoning effort** (low, medium, high), set in the system message.
- **Three assistant channels.** `final` is shown to users. `analysis` is chain
  of thought, which the document says not to show to end users. `commentary`
  carries function calls and preambles.
- **Stop tokens.** Generation stops on `<|return|>` (a finished answer) or
  `<|call|>` (a tool call).

The [tokenizer](https://github.com/openai/gpt-oss/blob/5681a6b6552635e7b29cde94a61ca6a7aee90ebd/gpt_oss/tokenizer.py)
is o200k_base's pattern and ranks plus harmony special tokens through ID
201,087. For Psionic this means:

- Chapter 5's byte-BPE engine can be reused, with a separate pattern, rank
  table and special-token identity.
- The Qwen tokenizer qualification does not carry over.
- The tiktoken ranks must be pinned and hashed before use.

## What Psionic does not adopt

- **The upstream tools.** The repository's browser and Python tools are
  described upstream as educational, and its Python container as permissive.
  Psionic only renders and parses tool messages. Executing a tool belongs to
  the host's own authority.
- **The reference runtimes.** The torch, Triton and Metal implementations are
  read as untrusted source and never executed on this Mac. Upstream calls its
  Metal path "not production-ready", and a reference runtime does not establish
  Psionic execution in any case.
- **Upstream sizing.** The 20b model's "within 16GB of memory" and the 120b
  model's single-80 GB-GPU target are upstream claims. Chapter 22's residency
  plan measures each model on the machine it runs on.
- **Upstream sampling defaults.** The recommended temperature 1.0 and top-p 1.0
  become explicit parameters under chapter 30's named RNG, not hidden defaults.

## Evidence for each model

For each model, capture these independently and freeze tolerances before
running the candidate: tokens, MXFP4 decode vectors, per-layer intermediates
(including sink and window behavior and the expert choices at each layer),
logits, full versus cached results, harmony render/parse round trips, and
schema-constrained tool calls. The 20b model does not qualify the 120b
model. Text is the only modality.
