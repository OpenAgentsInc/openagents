# gpt-oss as an inference target

Survey updated 2026-10-29. [Chapter 41](../../book/ch041-original-mxfp4-weights.md)
now implements the bounded original-format CPU decoder;
[chapter 42](../../book/ch042-a-few-experts-then-a-router.md) adds a bounded
top-k router and tiny affine mixture.
[Chapter 43](../../book/ch043-attention-with-sinks-and-a-window.md) adds bounded
sink/window attention, split-half YaRN, biased projection and clamped SwiGLU.
[Chapter 44](../../book/ch044-the-harmony-format.md) adds bounded original-rank
text tokenization, typed Harmony framing and strict completion parsing.
[Chapters 45–46](../../book/ch045-execute-gpt-oss-20b.md) now execute both complete
original checkpoints in Coax under `gpt-oss-cpu-wide-v1`, with independent dense
references and seeded generation. [Chapter 47](../../book/ch047-let-the-small-model-guess.md)
adds serial speculative verification; [chapter 48](../../book/ch048-call-a-tool.md)
adds a finite JSON-schema enum profile and granted tool round trip. Full weights
were downloaded as pinned data; upstream setup, inference and tool code were never
executed. Python remains reference/export tooling, never a runtime dependency. Part VII of the [book plan](../../book/PLAN.md) gives the
chapter sequence. The remaining sequence is proposed work, and it does not block
[Elephant and Mammoth](model-targets.md).

## Pins

| Source | Revision read | Terms |
| --- | --- | --- |
| [openai/gpt-oss](https://github.com/openai/gpt-oss/tree/5681a6b6552635e7b29cde94a61ca6a7aee90ebd) | `5681a6b6552635e7b29cde94a61ca6a7aee90ebd` | Apache-2.0 |
| [openai/harmony](https://github.com/openai/harmony/tree/abd677f7ac962629c808197caa1feb9e3e95d2b0) | `abd677f7ac962629c808197caa1feb9e3e95d2b0` | Apache-2.0 |
| [openai/gpt-oss-20b](https://huggingface.co/openai/gpt-oss-20b/tree/6cee5e81ee83917806bbde320786a8fb61efebee) | `6cee5e81ee83917806bbde320786a8fb61efebee` | Apache-2.0 card metadata |
| [openai/gpt-oss-120b](https://huggingface.co/openai/gpt-oss-120b/tree/b5c939de8f754692c1647ca79fbf85e8c1e70f8a) | `b5c939de8f754692c1647ca79fbf85e8c1e70f8a` | Apache-2.0 card metadata |

The [original manifest](../../fixtures/gptoss/original/pins.json) pins each complete
file's SHA-256 and size. Retained source headers derive all 363/543 tensor
descriptors; the upstream notices are retained alongside them.

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

Chapter 42 implements the selected-only top-k formula for 1–128 logits and
1–4 selected experts, including synthetic 32/128-expert top-four fixtures.
Its `cpu-topk-affine-v1` profile has explicit lowest-ID ties, f32 routing
weights with wide accumulation and width-16 affine teaching experts. It does
not qualify the learned gate, nonlinear expert projections, BF16 arithmetic
or a complete original MLP.

## Weights are MXFP4 in the original files

The pinned README distinguishes `original/` files used by its Torch/Triton
loader, converted Hugging Face files at the repository root, and its Metal
conversion. The table's HF-root sizes are not original-file inventory pins.
Chapter 45 authenticates those original files and binds their complete tensor
pairs. Chapter 41's synthetic windows remain decoder evidence only.


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
  become explicit parameters under chapter 37's named RNG, not hidden defaults.

## Evidence for each model

For each model, capture these independently and freeze tolerances before
running the candidate: tokens, MXFP4 decode vectors, per-layer intermediates
(including sink and window behavior and the expert choices at each layer),
logits, full versus cached results, harmony render/parse round trips, and
schema-constrained tool calls. The 20b model does not qualify the 120b
model. Text is the only modality.

## Chapter 43 implementation boundary

[Attention with sinks and a window](../../book/ch043-attention-with-sinks-and-a-window.md)
now provides `gpt-oss-attention-cpu-wide-v1` in
[`gptoss-attention.coax`](../../lib/gptoss-attention.coax) and
[`gptoss-math.coax`](../../lib/gptoss-math.coax). Projected f32 Q/K/V execution
uses 64-wide heads, 1–64 query heads, 1–8 KV heads, per-query-head sink logits,
window-bounded circular KV storage and split-half YaRN at absolute positions
0–131,071. Sliding capacity equals the window, at most 128; full causal capacity
is separately limited to 128. The caller supplies the alternating layer shapes.
Biased projection uses the shared kernel's bounded matrix budget; clamped
interleaved SwiGLU accepts up to 16,384 pairs. Projection/expert weight binding,
normalization and residual composition are implemented in chapter 45.

The [owned scalar oracle](../../tests/gptoss_oracle.py) compares complete
prefixes against cached execution, plus direct-power rotary and Decimal
activation results. Native debug/release and sanitizer gates cover 64/8 heads,
128-token expiry, aliases and atomic refusals. See the
[verification record](../../../docs/verification/psionic-ch36-2026-10-29.md).
The arithmetic is wide CPU with f32 storage; it does not claim original Torch
float/BF16 parity, original checkpoint admission or complete generation.
Chapter 44 now supplies the bounded Harmony text gate below; chapter 45
now supplies complete original-weight execution and intermediate/logit references.

## Chapter 44 implementation boundary

[The harmony format](../../book/ch044-the-harmony-format.md) implements
`gpt-oss-harmony-text-v1`. Coax admits an immutable, SHA-256-pinned HST7 table
built deterministically from the original o200k rank bytes; the separate split
pattern uses Unicode-17 category tables and preserves bytes without NFC. The
shared chapter 5 BPE loop uses every original concatenated rank pair, plus a
whole-piece byte lookup. Ordinary text never scans for special literals.

Typed system/developer/user/assistant/tool messages preserve supplied history,
with a pinned default low/medium/high reasoning template. Parsing supports
analysis/commentary/final, recipients before or after channels, complete
transcripts and open-assistant completions, stopping exactly on return/call.
Control IDs enter through structured framing, not text fields. Public tokens,
decoded bytes and record intervals publish only after success. Bounds include
64 messages, 16,384 IDs, 65,536 decoded bytes and 64-byte ASCII identifiers.

The [retained fixtures](../../fixtures/harmony/README.md) pin all 199,998
ordinary ranks and the model-specific special range through ID 201,087;
generic Harmony's extra ID 201,088 is outside this model profile. The
[owned oracle](../../tests/harmony_oracle.py) and native sanitizer gate cover
original IDs, exact framing, terminal consumption, UTF-8 joining and atomic
refusals. See the [chapter 44 record](../../../docs/verification/psionic-ch44-2026-10-29.md).
This does not qualify complete upstream renderer/Unicode-version parity,
multimodal or optional header extensions, incremental stream parsing, tool
execution, original-weight model execution or sampling. Chapter 45 joins these
primitives with generation; chapter 48 owns tool-call integration.
