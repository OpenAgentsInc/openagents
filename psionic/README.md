# Psionic

Psionic is a machine-learning library written in Coax, built one runnable book
chapter at a time. The first milestone trains a one-parameter model on
`[1, 2] → [2, 4]`, then predicts for an unseen input `3`. Chapter 2 adds
bounded shapes, row-major indexing and a staged copy into caller-owned storage.
[Chapter 3](book/ch003-a-tensor-is-a-view-of-memory.md) adds strided CPU views,
empty tensors, broadcasting and storage-version checks for saved values.
Chapter 4 adds checked CPU numerical operations over that storage.
[Chapter 5](book/ch005-a-standard-library-of-tensor-operators.md) adds 27
view-aware CPU operators, support metadata and explicit analytical value VJPs.
[Chapter 6](book/ch006-one-tensor-several-devices-and-dtypes.md) adds owned
CPU/CUDA/Metal placement, f32/f16/bf16 storage, explicit synchronization and
seeded uniform/dropout through shared Coax kernels. Chapter 7
reads original BF16/F32 weight files and complete pinned shard bundles.
[Chapter 8](book/ch008-models-travel-between-frameworks.md) adds named
Safetensors export, a versioned C interface and leased CPU DLPack exchange,
with a NumPy-to-Coax training and portable inference round trip.
Chapter 9 renders typed text decisions and tokenizes original Qwen data.
Chapter 10 executes a small causal dense attention block with a KV cache.
Chapter 11 composes Qwen-style linear and full-attention layers with separate states.
Chapter 12 adds decision blocks and forks their complete live hybrid state.
Chapter 13 converts vocabulary logits to option probabilities and typed statistics.
[Chapters 18–23](book/ch018-time-a-decision.md) now put that model behind a
measured resident decision service: bounded host/device memory ownership,
JSON/protobuf `/v1/systemone`, golden-gated readiness, content-free usage,
Coax restart supervision and exact CPU prefix reuse. The first managed
deployment is private loopback; [operations](book/ch022-run-it-live.md)
describe its install and stop commands. Run the focused gate with
`psionic/check.sh --only live-decision`.
[Chapter 25](book/ch025-the-api-fans-out.md) adds an opt-in loopback fleet adapter:
fresh exact-identity Pylons, encrypted correlated jobs, checked answers, and
same-identity local fallback. It uses throwaway buyer keys and is not public ingress.
[Chapter 26](book/ch026-check-the-mesh.md) adds opt-in free remote receipts,
memory-only sampled checking on a pinned different provider, and public
pool totals independently recomputed under explicit trust. Its first replay
scope is the toy CPU profile; claims are not execution attestations.
[Chapter 60](book/ch060-original-mxfp4-weights.md) starts the independent gpt-oss
branch with an exact bounded decoder for its original MXFP4 byte groups.
[Chapter 61](book/ch061-a-few-experts-then-a-router.md) adds sorted top-k routing
and a complete tiny affine mixture with explicit resident budgets.
[Chapter 62](book/ch062-attention-with-sinks-and-a-window.md) adds sink/window
attention, split-half YaRN, biased projection and the clamped expert activation.
[Chapter 63](book/ch063-the-harmony-format.md) adds the original byte-rank
tokenizer and typed Harmony rendering/parsing with exact completion stops.

[Chapters 64–77](book/ch064-execute-gpt-oss-20b.md) execute both original gpt-oss models,
add seeded generation, standard speculative verification and a constrained tool
round trip. The supported CPU and finite-schema profiles state their limits.

[Chapter 88](book/ch088-execute-a-deepseek-r1-distill.md) executes the original
DeepSeek R1 Qwen distill as a dense CPU reference. [Chapter 89](book/ch089-deepseeks-front-door.md)
adds typed recipe boundaries, independent plain-run tokenization and a bounded
UTF-8/DSML streaming protocol with explicit finish handling. Its separate
V4.1 text adapter uses the original vocabulary/merges, Unicode 16 and no
normalization, checked through actual token streams entirely on CPU.
[Chapter 90](book/ch090-execute-deepseek-v2-lite.md) executes the complete original
V2-Lite BF16 stack in compressed and expanded cache forms, with original
generation checked against independent full-prefix equations.
[Chapter 91](book/ch091-routers-that-balance-themselves.md) supplies DeepSeek’s
selection-only correction bias, grouped/global routing and complete margin
comparison records used by chapter 90’s complete V2-Lite decoder.
[Chapter 93](book/ch093-many-residual-streams.md) adds V4.1's four residual
streams, fixed Sinkhorn mixing and the previous pre-mix carried between
sub-blocks and into final readout.
[Chapter 94](book/ch094-memory-by-lookup.md) adds Engram's exact original
token map and n-gram hashes, three-token continuation, file range planning
and bounded mapped/direct FP8 row reads at original table sizes.
[Chapter 95](book/ch095-v41s-expert-weights.md) adds exact FP4/FP8 expert
formats, activation grids and pre-down route weighting, including an
independently qualified original 384/6 MoE component.
[Chapter 96](book/ch096-hot-experts-in-fast-memory.md) adds the exact CPU
expert-cache core: profile heat, between-step transactions, private prefetch
and host/file/resident coefficient parity, including an original expert.
Device residency and execution remain integration work.
[Chapter 97](book/ch097-prefill-layer-by-layer.md) adds layer-major CPU
chunks, stable routed groups and grouped block-scaled products, with exact
owned causal-stack and original MoE-batch parity. Generated CUDA products match
CPU words exactly on the RTX 4080, including original routed projection shapes;
complete GPU prefill remains integration work.
[Chapter 98](book/ch098-pack-the-cache-exactly.md) retains the original KV/index
grids as packed codes/scales, reconstructs their BF16 bits and admits persistent
cache payload and chunk codec work against separate limits.
[Chapter 92](book/ch092-compressed-and-shared-attention.md) combines projected
compression, causal window/sparse attention, pinned cross-layer sharing and
candidate reuse, with independent prefix and chunk-state checks.

[Chapter 38](book/ch038-a-tensor-program-becomes-an-execution-plan.md) adds a
typed tensor IR, root-lifetime planning, reusable workspace and one feed-forward
block executed by CPU reference procedures or shared CPU/CUDA/Metal kernels.

[Chapters 65–67](book/ch065-original-weights-stay-on-the-gpu.md) add resident
original 20b CUDA execution, complete greedy Harmony generation and batched
prefill on the RTX 4080. [Run a prompt](book/ch066-a-complete-cuda-decoder.md)
with `psionic/run-gptoss.sh`; [focused checks](tests/check_gptoss_device.sh)
use owned tiny weights by default. Device throughput has separate measured
gates in the plan.

[Chapter 79](book/ch079-serve-responses.md) adds a Responses-native loopback
API over the resident original 20b worker: typed reasoning/function replay,
incremental SSE and SDK checks. [Serve it](docs/serving/responses.md) with
`psionic/serve-gptoss.sh`; run `psionic/tests/check_responses.sh` for focused
owned HTTP checks.

Start with [the book](book/README.md) and
[Chapter 1: Learning a number](book/ch001-learning-a-number.md), then
[Chapter 2: Shapes and storage](book/ch002-shapes-and-storage.md) and
[Chapter 4: Numbers and matrices](book/ch004-numbers-and-matrices.md), then
[Chapter 7: Reading original weights](book/ch007-reading-original-weights.md), then
[Chapter 9: Text becomes a model request](book/ch009-text-becomes-a-model-request.md), then
[Chapter 10: Attention without training](book/ch010-attention-without-training.md), then
[Chapter 11: A hybrid sequence model](book/ch011-a-hybrid-sequence-model.md), then
[Chapter 12: Decisions have a different mask](book/ch012-decisions-have-a-different-mask.md), then
[Chapter 13: Read the options](book/ch013-read-the-options.md).
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
a C11 compiler and Python 3 for book tooling and independent Decimal/owned-artifact fixtures. Chapter 9 fixture authoring and
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
| [lib/interchange.coax](lib/interchange.coax), [platform/interchange.h](platform/interchange.h) | Bounded named F32/BF16 parameter files, stable ABI v1 and zero-copy leased CPU f32 DLPack. |
| [lib/learn.coax](lib/learn.coax) | Checks first, admission, loss/gradient, fixed-rate SGD, prediction. |
| [lib/tensor.coax](lib/tensor.coax) | Checked rank/shape/byte/stride metadata, indexing and staged finite-f32 copy. |
| [lib/qwen-model.coax](lib/qwen-model.coax) | Ordered hybrid layers with staged complete-block state and activations; original-weight qualification is recorded in chapter 14. |
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
values, preserves unused capacity, and refuses before any write. Chapter 4
implements the named `CPU-f32-wide-v1` profile for bounded rows/matrices, with
a 1,048,576 multiply-add budget per matmul. It validates finite inputs before
staging; arithmetic overflow may dirty scratch while preserving destination.
[Its chapter](book/ch004-numbers-and-matrices.md) defines exact shapes, masks,
normalization, precision and error contracts. Chapter 7 adds the separate `original-weights-v1` artifact profile: rank 0–4
BF16/F32 originals, 64-GiB files, 128-GiB weight bundles, 16 shards, 4,096
tensors, 1-MiB headers/indexes and finite decode windows of at most 1,048,576
values. Metadata may describe larger tensors without expanding them. Caller
pins bind original sidecar/shard hashes and complete name/dtype/shape inventory;
labels alone do not authenticate provenance. Metadata drafts may change on
refusal; public inventory and both decoded float spans have explicit atomic
publication contracts. See [chapter 7](book/ch007-reading-original-weights.md)
for exact profile and lifetime obligations. Chapter 9 adds `qwen35-text-v1`:
original Qwen tokenizer data and frozen tagged Base Labs rendering, with
Unicode-17 NFC, explicit byte/token/work budgets, integer structured JSON
entries and complete token-prefix matching across 1–32 branches. Its
[chapter](book/ch009-text-becomes-a-model-request.md) and
[verification](../docs/verification/psionic-ch05-2026-10-29.md) distinguish
owned reference evidence from upstream-runtime/model qualification.
Chapter 10 adds a small causal dense block with caller-supplied interleaved rotary
tables, grouped-query attention and a key/value cache; its dimensions are
bounded and do not qualify an actual model. See [chapter 10](book/ch010-attention-without-training.md).
Chapter 11 implements single-token Qwen-style layers and a small hybrid example;
its bounded checks do not themselves qualify complete checkpoint execution or
model behavior. See
[chapter 11](book/ch011-a-hybrid-sequence-model.md).
Chapter 12 evaluates a bidirectional full-attention block and forks convolution,
recurrent and live KV state. The fork compares a caller-bound model identity;
see chapter 14 for the Elephant original-checkpoint qualification.
Chapter 13 provides CPU slot readout given full-vocabulary logits and explicit
temperature; it does not load calibration artifacts or serialize responses.
Chapter 14 authenticates and executes a typed Elephant text decision on the CPU;
see [execution progress](docs/model-execution-progress.md). Chapter 11 runs
Mammoth's on the CPU, and [chapter 46](book/ch046-mammoth-on-cuda.md) runs it
on the RTX 4080 in about 1.6 s, streaming most of its weights over PCIe.
[Chapter 36](book/ch036-many-questions-at-once.md) runs a request's question
suffixes as one pass over the weights, bit-identical to forking them one at a
time: Mammoth's five-question Lab request on the 4080 drops from 9.3 s to 3.3 s.
<!-- capability-claim: psionic.autodiff.current -->
Autodiff is implemented in two bounded CPU profiles: the binary64 scalar tape
in [chapter 104](book/ch104-gradients-as-a-program.md) and first derivatives over
32-node f32 tensor programs in
[chapter 106](book/ch106-differentiate-any-tensor-program.md). General function
transforms, higher derivatives and device backward execution remain planned.
Original gpt-oss 20b GPU execution and a bounded Responses edge are qualified
separately above; neither extends the autodiff profile.
<!-- /capability-claim -->

Chapter [88: Execute a DeepSeek R1 distill](book/ch088-execute-a-deepseek-r1-distill.md)
adds a bounded original BF16 Qwen2 CPU reference with its own pinned tokenizer,
greedy continuation and separate reasoning/final intervals. Run
`psionic/tests/check_deepseek.sh` for the offline debug/release, native/sanitized
and book checks, or `psionic/check.sh --only deepseek --jobs 8` to include the
zero-survivor authority mutation gates. The original full-model capture is
retained; routine checks do not rerun it.

[Chapter 49](book/ch049-read-a-gguf-file.md) reads bounded little-endian GGUF v3 metadata and tensor ranges, checked against original upstream headers without model execution. Run `psionic/check.sh --only gguf`.

[Chapter 52](book/ch052-tokenizers-and-chat-templates.md): WordPiece, SentencePiece-style Unigram and bounded Jinja, checked against original BERT/T5 tables and TinyLlama templates. Run `psionic/check.sh --only tokenizer-families`.

[Chapter 112](book/ch112-bytes-and-a-small-language-model.md): Train a complete 256-byte bigram model on fixed CC0 synthetic documents, evaluate disjoint held-out splits and sample with a named counter RNG. Run `psionic/check.sh --only byte-lm`.

[Chapter 122](book/ch122-a-checkpoint-holds-the-whole-experiment.md): Deterministic document packing, versioned checkpoint and pointer records with identities, atomic publication and exact resume of an interrupted run. Run `psionic/check.sh --only resume`.

[Chapter 123](book/ch123-learn-a-tokenizer.md): Deterministic Coax byte-BPE training, pinned nanochat two-digit splitting, independent merges and persisted chapter-9 PST5 round trips. Run `psionic/check.sh --only bpe-train`.

[Chapter 124](book/ch124-adamw-and-explicit-precision.md): AdamW over a binary64 master copy with parameter groups, declared compute precision, all-or-nothing admission, a skip policy and exact resume, checked against an independent oracle. Run `psionic/check.sh --only adamw`.
