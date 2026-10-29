# Psionic

Psionic is a machine-learning library written in Coax, built one runnable book
chapter at a time. The first milestone trains a one-parameter model on
`[1, 2] → [2, 4]`, then predicts for an unseen input `3`. Chapter 2 adds
bounded shapes, row-major indexing and a staged copy into caller-owned storage.
Chapter 3 adds checked CPU numerical operations over that storage. Chapter 4
reads original BF16/F32 weight files and complete pinned shard bundles.
Chapter 5 renders typed text decisions and tokenizes original Qwen data.
Chapter 6 executes a small causal dense attention block with a KV cache.
Chapter 7 composes Qwen-style linear and full-attention layers with separate states.
Chapter 8 adds decision blocks and forks their complete live hybrid state.
Chapter 9 converts vocabulary logits to option probabilities and typed statistics.
[Chapter 41](book/ch041-original-mxfp4-weights.md) starts the independent gpt-oss
branch with an exact bounded decoder for its original MXFP4 byte groups.
[Chapter 42](book/ch042-a-few-experts-then-a-router.md) adds sorted top-k routing
and a complete tiny affine mixture with explicit resident budgets.
[Chapter 43](book/ch043-attention-with-sinks-and-a-window.md) adds sink/window
attention, split-half YaRN, biased projection and the clamped expert activation.
[Chapter 44](book/ch044-the-harmony-format.md) adds the original byte-rank
tokenizer and typed Harmony rendering/parsing with exact completion stops.

[Chapters 45–48](book/ch045-execute-gpt-oss-20b.md) execute both original gpt-oss models,
add seeded generation, standard speculative verification and a constrained tool
round trip. The supported CPU and finite-schema profiles state their limits.

[Chapter 56](book/ch056-execute-a-deepseek-r1-distill.md) executes the original
DeepSeek R1 Qwen distill as a dense CPU reference. [Chapter 57](book/ch057-deepseeks-front-door.md)
adds typed recipe boundaries, independent plain-run tokenization and a bounded
UTF-8/DSML streaming protocol with explicit finish handling.
[Chapter 59](book/ch059-routers-that-balance-themselves.md) supplies DeepSeek’s
selection-only correction bias, grouped/global routing and complete margin
comparison records, ahead of chapter 58’s complete V2-Lite decoder.

[Chapters 45a–45c](book/ch045a-original-weights-stay-on-the-gpu.md) add resident
original 20b CUDA execution, complete greedy Harmony generation and batched
prefill on the RTX 4080. [Run a prompt](book/ch045b-a-complete-cuda-decoder.md)
with `psionic/run-gptoss.sh`; [focused checks](tests/check_gptoss_device.sh)
use owned tiny weights by default. Device throughput has separate measured
gates in the plan.

[Chapter 48b](book/ch048b-serve-responses.md) adds a Responses-native loopback
API over the resident original 20b worker: typed reasoning/function replay,
incremental SSE and SDK checks. [Serve it](docs/serving/responses.md) with
`psionic/serve-gptoss.sh`; run `psionic/tests/check_responses.sh` for focused
owned HTTP checks.

Start with [the book](book/README.md) and
[Chapter 1: Learning a number](book/ch001-learning-a-number.md), then
[Chapter 2: Shapes and storage](book/ch002-shapes-and-storage.md) and
[Chapter 3: Numbers and matrices](book/ch003-numbers-and-matrices.md), then
[Chapter 4: Reading original weights](book/ch004-reading-original-weights.md), then
[Chapter 5: Text becomes a model request](book/ch005-text-becomes-a-model-request.md), then
[Chapter 6: Attention without training](book/ch006-attention-without-training.md), then
[Chapter 7: A hybrid sequence model](book/ch007-a-hybrid-sequence-model.md), then
[Chapter 8: Decisions have a different mask](book/ch008-decisions-have-a-different-mask.md), then
[Chapter 9: Read the options](book/ch009-read-the-options.md).
[The design notes](docs/README.md) explain the larger plan.

From the repository root:

```shell
psionic/check.sh
coax/bin/coax run psionic/examples/learn-number.coax
coax/bin/coax run psionic/examples/shape-storage.coax
coax/bin/coax run psionic/examples/numbers-matrices.coax
coax/bin/coax run psionic/examples/read-weights.coax
coax/bin/coax run psionic/examples/text-request.coax
coax/bin/coax run psionic/examples/attention.coax
coax/bin/coax run psionic/examples/hybrid-sequence.coax
coax/bin/coax run psionic/examples/decision-forks.coax
coax/bin/coax run psionic/examples/read-options.coax
coax/bin/coax run psionic/examples/original-mxfp4.coax
psionic/tests/check_mxfp4.sh
coax/bin/coax run psionic/examples/few-experts.coax
psionic/tests/check_moe.sh
coax/bin/coax run psionic/examples/attention-sinks.coax
psionic/tests/check_gptoss_attention.sh
coax/bin/coax run psionic/examples/harmony.coax
psionic/tests/check_harmony.sh
psionic/tests/check_gptoss.sh
```

The check command builds the compiler if it is absent, runs authored Coax
checks in debug/release and the authority mutation gates, compares independent
native numerical oracles in debug/release with and without ASan/UBSan, and
verifies book listings, local links, example output and exit status. It uses
a C11 compiler and Python 3 for book tooling and independent Decimal/owned-artifact fixtures. Chapter 5 fixture authoring and
verification require Python with Unicode database 17.0.0 (tested Python 3.15.1).
Neither the library nor its examples require Python, models, services,
credentials, or downloads at run time.

| Path | Role |
| --- | --- |
| [lib/gptoss.coax](lib/gptoss.coax), [lib/gptoss/](lib/gptoss/) | Original 20b/120b admission and execution, seeded generation, speculative verification and constrained tools. |
| [lib/harmony.coax](lib/harmony.coax) | Original-rank gpt-oss text tokenization, immutable table admission, typed message framing and strict full/completion parsing with atomic public output. |
| [lib/gptoss-attention.coax](lib/gptoss-attention.coax), [lib/gptoss-math.coax](lib/gptoss-math.coax) | Bounded sink/window attention, split-half YaRN, biased projection and interleaved clamped SwiGLU under an explicit wide CPU profile. |
| [lib/moe.coax](lib/moe.coax) | Bounded top-k selection, selected-only normalization and tiny resident affine expert mixtures with atomic output. |
| [lib/mxfp4.coax](lib/mxfp4.coax) | Original gpt-oss MXFP4 byte decoding: exact finite CPU values, shape/count/scale admission and unchanged output on refusal; separate from complete artifact admission. |
| [lib/learn.coax](lib/learn.coax) | Checks first, admission, loss/gradient, fixed-rate SGD, prediction. |
| [lib/tensor.coax](lib/tensor.coax) | Checked rank/shape/byte/stride metadata, indexing and staged finite-f32 copy. |
| [lib/qwen-model.coax](lib/qwen-model.coax) | Ordered hybrid layers with staged complete-block state and activations; original-weight qualification is recorded in chapter 10. |
| [lib/model-io/rank5-bundle.coax](lib/model-io/rank5-bundle.coax) | Separate full rank-five inventory and bundle binding, including all original tensor axes. |
| [lib/readout.coax](lib/readout.coax), [tests/readout.c](tests/readout.c) | Slot mass, explicit-temperature probabilities and typed statistics, with original slot IDs and native buffer checks. |
| [lib/qwen-decision.coax](lib/qwen-decision.coax), [lib/qwen-prefix.coax](lib/qwen-prefix.coax) | Bidirectional full-attention blocks and isolated snapshots of complete live hybrid state. |
| [lib/qwen-linear-layer.coax](lib/qwen-linear-layer.coax), [lib/qwen-full-layer.coax](lib/qwen-full-layer.coax) | Bounded causal Qwen-style linear and full-attention layers with staged model state. |
| [lib/attention.coax](lib/attention.coax) | Bounded dense causal block, grouped-query attention, rotary positions and cache execution. |
| [platform/attention-memory.h](platform/attention-memory.h), [tests/attention.c](tests/attention.c) | Address observations and native independent forward/cache checks. |
| [lib/ops-contract.coax](lib/ops-contract.coax) | Operation shapes, typed view counts, epsilon rules and matrix-work admission. |
| [lib/text.coax](lib/text.coax), [lib/text/](lib/text/) | qwen35-text-v1: frozen tagged requests, Unicode-17 NFC, original-table ranked BPE and validated prefix/suffix layouts. |
| [platform/text-memory.h](platform/text-memory.h) | Typed text/token native interface and nested span observations. |
| [tests/text.c](tests/text.c), [tests/text_oracle.py](tests/text_oracle.py) | Independent Unicode/regex/BPE and rendered-request references, actual original data and native refusals. |
| [lib/model-io/](lib/model-io/) | original-weights-v1 prefix/shape admission, JSON inventory, streaming SHA-256 identities, shard-index/manifest checks and finite window decoding. |
| [platform/weights-memory.h](platform/weights-memory.h) | Native artifact interface; observes all writable/nested read-only spans. |
| [tests/weights.c](tests/weights.c), [tests/weights_fixtures.py](tests/weights_fixtures.py) | Actual original single/sharded files, independent hashes/encodings, malformed inputs, aliases and atomic float refusal. |
| [lib/ops.coax](lib/ops.coax) | Add, row sums/dots, matmul/matvec, gather, masked softmax, unscaled RMSNorm/LayerNorm; f32 storage with compensated f64 reductions. |
| [platform/ops-memory.h](platform/ops-memory.h) | Native operations wrapper; typed span observations include integer indices and byte masks. |
| [platform/tensor-memory.h](platform/tensor-memory.h) | Native tensor caller interface, alongside generated `tensor.h`; its C adapter observes addresses through shared [spans.h](platform/spans.h). |
| [platform/memory.c](platform/memory.c) | Observes native span overlap and calls Coax; no numerical implementation. |
| [platform/memory.h](platform/memory.h) | Native caller interface, alongside the generated `learn.h`. |
| [examples/](examples/) | Training, storage and numerical examples using the shared library. |
| [tests/native.c](tests/native.c) | Independent closed-form/quadratic oracles and actual storage/representation cases. |
| [tests/tensor.c](tests/tensor.c) | Independent layouts/indices, actual aliases/short allocations, float representations and a maximum-size copy. |
| [tests/ops.c](tests/ops.c), [tests/ops_oracle.py](tests/ops_oracle.py) | Independent Decimal fixtures, analytic magnitude/cancellation cases and real native storage. |
| [book/examples.json](book/examples.json) | Complete runnable example inventory and expected statuses. |
| [fixtures/](fixtures/) | Stable teaching transcripts. |

Current scope is synchronous CPU execution on the tested Apple LP64 host:
1–16 paired samples, 1–64 updates, a fixed learning rate `1/8`, and inputs,
targets, initial and candidate weights in `[-16, 16]`. Prediction accepts a
weight and input in that range. The book documents capacities, failure
publication, arithmetic, and lifetime obligations. Chapter 2 admits contiguous
`f32` shapes of rank 0–4 with positive active extents and at most 1,048,576
elements (4 MiB of logical data). The input view is exactly the logical tensor;
destination/scratch may have spare capacity. Copy accepts all finite binary32
values, preserves unused capacity, and refuses before any write. Chapter 3
implements the named `CPU-f32-wide-v1` profile for bounded rows/matrices, with
a 1,048,576 multiply-add budget per matmul. It validates finite inputs before
staging; arithmetic overflow may dirty scratch while preserving destination.
[Its chapter](book/ch003-numbers-and-matrices.md) defines exact shapes, masks,
normalization, precision and error contracts. Chapter 4 adds the separate `original-weights-v1` artifact profile: rank 0–4
BF16/F32 originals, 64-GiB files, 128-GiB weight bundles, 16 shards, 4,096
tensors, 1-MiB headers/indexes and finite decode windows of at most 1,048,576
values. Metadata may describe larger tensors without expanding them. Caller
pins bind original sidecar/shard hashes and complete name/dtype/shape inventory;
labels alone do not authenticate provenance. Metadata drafts may change on
refusal; public inventory and both decoded float spans have explicit atomic
publication contracts. See [chapter 4](book/ch004-reading-original-weights.md)
for exact profile and lifetime obligations. Chapter 5 adds `qwen35-text-v1`:
original Qwen tokenizer data and frozen tagged Base Labs rendering, with
Unicode-17 NFC, explicit byte/token/work budgets, integer structured JSON
entries and complete token-prefix matching across 1–32 branches. Its
[chapter](book/ch005-text-becomes-a-model-request.md) and
[verification](../docs/verification/psionic-ch05-2026-10-29.md) distinguish
owned reference evidence from upstream-runtime/model qualification.
Chapter 6 adds a small causal dense block with caller-supplied interleaved rotary
tables, grouped-query attention and a key/value cache; its dimensions are
bounded and do not qualify an actual model. See [chapter 6](book/ch006-attention-without-training.md).
Chapter 7 implements single-token Qwen-style layers and a small hybrid example;
complete checkpoint execution and model behavior remain unqualified. See
[chapter 7](book/ch007-a-hybrid-sequence-model.md).
Chapter 8 evaluates a bidirectional full-attention block and forks convolution,
recurrent and live KV state. The fork compares a caller-bound model identity;
see chapter 10 for the Elephant original-checkpoint qualification.
Chapter 9 provides CPU slot readout given full-vocabulary logits and explicit
temperature; it does not load calibration artifacts or serialize responses.
Chapter 10 authenticates and executes a typed Elephant text decision on the CPU;
see [execution progress](docs/model-execution-progress.md). Mammoth
end-to-end qualification and autodiff remain planned. Original gpt-oss 20b
GPU execution and a bounded Responses edge are qualified separately above.

Chapter [56: Execute a DeepSeek R1 distill](book/ch056-execute-a-deepseek-r1-distill.md)
adds a bounded original BF16 Qwen2 CPU reference with its own pinned tokenizer,
greedy continuation and separate reasoning/final intervals. Run
`psionic/tests/check_deepseek.sh` for the offline debug/release, native/sanitized
and book checks, or `psionic/check.sh --only deepseek --jobs 8` to include the
zero-survivor authority mutation gates. The original full-model capture is
retained; routine checks do not rerun it.
