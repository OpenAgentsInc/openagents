# Exact-input llama.cpp diagnostic

This diagnostic compares a third runtime on the fixed text-029 input. It does
not replace the frozen Torch reference, change its all-104-top requirement,
change the 0.02 probability bound, or close M4. No build or model has run from
these sources. Root and peer review plus an explicit workload window are
required before execution.

## Version and arithmetic

The available complete public source is llama.cpp commit
`62a6f74bab748b1755f4cd8dfc0871caf865c6c8`, tree
`04358a29d219e5bf7e758cd5f5c013c60874d1bd`. This is not the M1 baseline.
Root resolved public tag `b11538` to
`79e2e74eb11022c1ba2e438df7f0ca2d4c10f8b6`. Its commit object exists
locally, but reading its tree failed on missing object
`7c274c03b7f1546cfd665ad7f60f01b80f47f592`. No source download or
version substitution is part of this plan.

The prepared build links static CPU llama/ggml libraries. It disables Metal,
CUDA, other device backends, loadable backends, BLAS, Accelerate, KleidiAI,
weight repacking, and llamafile. It fixes the CPU architecture to
`armv8.2-a+dotprod`, two threads, and the existing upstream Release math flags.
The driver verifies exactly one CPU device and registry, the Q4_K CPU
vector-dot RHS type Q8_K, 48 fused CPU Gated Delta Net graph operators, and no
Flash Attention operator. A mismatch fails the diagnostic; there is no fallback.

This CPU path quantizes matrix-multiply RHS inputs to Q8_K for Q4_K weights.
It is different from both native f16 TensorOps and the independent Torch f32
oracle. Its fused CPU Delta recurrence uses its own reduction and output order.
A match or mismatch therefore describes another implementation; neither
proves that all residual differences are harmless nor identifies one cause by
itself. All outputs and distances remain visible.

## Existing artifacts

The existing `lldump.cpp` establishes the layer callback mechanism, but defaults
to GPU offload and silently skips unsupported captures. It is not executed.
The cached helper at `q4-down-q6-regeneration/helper-build-2/build` contains
only `libggml-base.0.26.0.dylib` and the Q6 bridge. Its CMake configuration has
`GGML_CPU=OFF`; it has no built libllama or CPU backend. The new wrapper uses
its own build output directory and never changes that helper or the source
checkout. No target in another agent's slot is touched.

The exact original GGUF remains 19,232,219,200 bytes, previously verified as
SHA-256 `9997675e4c9e9016273372b5fdedbdda7ad14285bc722949e4b9daca62bf3822`.
The guard compares its saved size/inode/mtime/ctime before and after each
stage; it does not reread 19 GB just to hash the model again.

## Fixed input and captures

`common.frozen_input()` verifies the independent CPU record and the later
exact-source native encoder export agree on all 359 IDs, three questions,
six options, types, spans, order, and no truncation. The input TSV contains
position, token, and decision-span marker for every token, including explicit
zero markers between spans. Its SHA-256 is
`d0c32090dcef916602b5955e748c0790836a8f3f6671e7e1721e39f36fbd741d`.
Native Stage6 HTTP did not itself export IDs; the later source reconstruction
does not erase that historical limitation.

The driver creates one sequence, one 359-token encode operation, no persistent
cache, context/batch/ubatch limits of 512, and no generation. It writes all 64
`l_out-N` arrays and `result_norm`, requiring contiguous finite f32
`[359,5120]` rows, exactly one file per capture, and exclusive output creation.
It holds only one 7,352,320-byte callback copy at a time. Captured rows total
477,900,800 bytes. A callback failure aborts computation; callback state outlives
context cleanup on every exit. Missing or duplicate files fail the run.

The graph always executes its embedded head. That head's 160 tensors include
59 Q8_0 weights, one BF16 matrix, and 100 f32 tensors. Its six logits are kept
under a separate label and excluded from every head-equivalent comparison.

## Unchanged external head

After the model process and its mappings have exited, a separate CPU process
compares every saved layer to the independent f32 CPU and native Stage6
observer arrays. It retains whole-row and disputed question/option-span
distances for all layers, without selecting a favorable layer.

The replay loads official BF16 head weights as f32, the same GGUF lexical rows,
and exactly the same IDs/spans/options. It applies the head directly to
`result_norm`: it does not apply the backbone RMSNorm twice. It first requires
bitwise reproduction of the saved CPU A head logits. It separately checks
native observer replay against the existing 1e-3 raw-logit/all-top screen.
Execution success and this screen's result remain separate.

All three questions' logits, probabilities, tops, hidden rows, pooled rows,
lexical rows, and byte identities remain available. Parameter and input hashes
must remain unchanged. The public report includes compact summaries and hashes;
raw arrays and model weights remain in scratch.

## Proposed bounds and sequence

These bounds are a proposal for review, not permission to launch.

1. Build: build + memory 8 GiB + disk 2 GB leases, 20 GiB available at start,
   12 GiB available-memory floor, two compile jobs, 600-second deadline,
   2 GiB output cap, and 25 GiB free-disk floor. Compile only the static wrapper
   target from the pinned local source. No downloads or model execution.
2. Capture: memory 32 GiB + disk 2 GB leases, 44 GiB available at start,
   12 GiB floor, two CPU threads, 600-second deadline, 2 GiB output cap,
   and 25 GiB free-disk floor. No build/GPU/quiet lease. The packed model is
   17.91 GiB; the remaining approximately 14 GiB covers graph buffers, mapped
   pages, allocator overhead, and the embedded head. This is a conservative
   admission cap, not a measured peak. Mmap plus disabled repacking avoids a
   second full weight copy. The scalar/quantized CPU path may exceed six to
   ten minutes; if it crosses 600 seconds the guard preserves failure and
   stops only its owned group, without retry or cap increase.
3. Saved-row replay: memory 4 GiB + disk 2 GB leases, start 16 GiB/floor 12 GiB,
   two CPU threads, 120-second deadline, 2 GiB output cap, 25 GiB disk floor.
   The head checkpoint is about 256 MB; rows are compared one layer at a time.
   No second trunk is loaded, and the packed model's lexical reader uses
   bounded reads rather than another full mapping.

Each stage uses a new directory, a frozen source descriptor, an owned child
process group, fail-closed memory monitoring (including close-time errors),
and source/artifact checks before and after. Build and model groups never
overlap. All failed attempts remain intact. The guard strips injected dynamic
library and GGML/llama environment overrides. No desktop app, system service,
permission, power setting, or existing process is modified.

The only changed historical guard dependency is `mac_quiet_bench.py`: root's
later cache-response validator/self-test edit has hash
`7018be065710be9ffe5dfc2d289770015ca617e2fdd3da253a98e07ba1bc997b`.
Its process and memory guard code is unchanged. This file is explicitly frozen
at its current hash; other original reference dependencies must still match.

## Causal predictions

If llama's full trunk and unchanged external head reproduce native's disputed
top, that supports a precision-path-sensitive answer across independent
implementations; it does not satisfy the existing Torch all-top gate.
If they reproduce the Torch top, native remains the outlier in that comparison.
In both cases, the full layer trajectory can show whether agreement arises
early or only after accumulated drift. Q8_K RHS rounding, distinct Delta
reductions, non-Flash softmax, and this newer upstream revision prevent a
single-result claim that one operator is the cause. No extra variant, case,
threshold, or artifact change follows automatically.
