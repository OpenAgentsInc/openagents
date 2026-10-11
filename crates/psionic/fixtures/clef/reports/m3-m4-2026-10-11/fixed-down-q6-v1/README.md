# Fixed FFN-down Q6 candidate

Status: candidate generation and independent sampled-payload checks passed.
No model run has been performed by this utility. This candidate has not passed
the 104-answer BF16 gate. The artifact SHA-256 is
`a8f44a683f241da62b5cc0c355e556957fdfb87f74567d8596f1923ff3825e9b`.

The existing artifact contains Q4_K in all 192 FFN matrices. The candidate
replaces 32 of the 64 down projections with Q6_K generated directly from the
original BF16 checkpoint. The selection is the `use_more_bits` predicate in
[llama.cpp at 62a6f74](https://github.com/ggml-org/llama.cpp/blob/62a6f74bab748b1755f4cd8dfc0871caf865c6c8/src/llama-quant.cpp#L452):
the first and last eighth of the layers and every third layer between them.
No selection uses acceptance cases, logits, or their errors. This applies the
upstream FFN-down policy to the 64 backbone layers; it does not claim to
reproduce all other choices of the complete upstream Q4_K_M preset.

The exact layer list is:
`0,1,2,3,4,5,6,7,10,13,16,19,22,25,28,31,34,37,40,43,46,49,52,55,56,57,58,59,60,61,62,63`.

`q6_bridge.cpp` calls upstream `ggml_quantize_chunk(GGML_TYPE_Q6_K)` without an
importance matrix. It contains no replacement quantization algorithm.
`upstream.json` pins primary source snapshots and their hashes. Building needs
a clean checkout at that exact revision. The isolated scratch checkout was
restored from the pinned public source archive; all 3,767 regular files and
symlinks match the pinned Git tree blob identities. The original llama.cpp
checkout is unchanged. The initial source fetch and SDK configuration failure
remain in separate attempt directories. The bridge links
only the CPU `ggml-base` library; the build disables Metal, CUDA, other GPU
backends, BLAS, Accelerate, OpenMP, and native CPU tuning. The evidence records
the compiler, commands, bridge, and library hashes. The guard clears dynamic
library injection/search overrides. Before quantization, `dladdr` verifies that
the resolved bridge and `ggml_quantize_chunk` symbols belong to those exact
hashed images. The build checks the pinned source revision and hashes again
after compilation.

`regenerate.py plan` reads GGUF descriptors, safetensor headers, and prior
verification records. It does not read tensor payloads. It checks original
checkpoint file identity against the clean CPU-head reference and original
GGUF identity against the independent same-weight reference. It preserves the
known limitation that historical full-checkpoint hashes are not recomputed.

`regenerate.py generate` requires the guard. It streams the original GGUF once,
computes and checks its full SHA-256 during that pass, and generates selected
Q6 payloads from 64-row BF16 slices. FFN-down weights need no head permutation
or normalization adjustment. All other tensor bytes, all inter-tensor padding,
all metadata, and descriptor order remain unchanged. Only selected type codes
and tensor offsets change. Artifact identity and the exact recipe are in a
sidecar, so tokenizer and architecture metadata remain byte-identical.
Every copied tensor and every replaced source/output receives a payload hash.
The destination remains `.partial` until final source identity, full original
hash, size, and new header checks succeed. Output files are never overwritten.
An interrupted partial file is retained for inspection and must not be served.

Expected candidate size is **19,967,533,120 bytes** (18.596 GiB), an increase of
**735,313,920 bytes** (0.685 GiB). Selected BF16 payload reads total
5,704,253,440 bytes. No 54 GB BF16 GGUF or full-model allocation is needed.
The largest f32 block is 4,456,448 bytes; BF16 input is 2,228,224 bytes and Q6
output is 913,920 bytes. Including conversion temporaries, headers, and Python,
The measured generation peak was 129,449,984 bytes under the 2 GiB
process-group cap. Generation took 72.07 seconds and left more than 102 GB
available throughout sampled observations.

The prepared guard uses a 12 GiB available-memory floor and stops only its own
child process group. Build: 4 GiB memory, start with 16 GiB available, two build
jobs, 600-second deadline, and a 2 GiB disk reservation. Generation: 2 GiB
memory, start with 14 GiB available, one quantizer thread, 1,800-second deadline,
and a 20 GiB disk reservation. Generation also requires 45 GiB free disk to
leave its 25 GiB free-space floor, matching the broker's observed floor.
The helper build requires 27 GiB free disk. No GPU or quiet lease is allowed. Run neither
phase during the parent's latency window.

After the explicit handoff, build with `guard.py build --source CHECKOUT
--run-dir NEW_DIR` under build, memory, and disk leases. Generate with
`guard.py generate --bridge BUILD/q6_bridge.dylib --bridge-manifest
BUILD/bridge.json --run-dir NEW_DIR` under memory and disk leases. The guard
provides the full commands and hashes in `guard.json`. Paths must be absolute.

The built helper passed five synthetic cases covering 2,367,488 decoded
values. Packed bytes match the direct upstream quantizer and separate row
calls; every decoded f32 bit matches the independent NumPy decoder. Five
invalid requests leave output bytes unchanged. The successful build took
5.16 seconds and stayed below 410 MB of sampled process-group memory.

`check_candidate.py` passed checks on four fixed complete rows
(0, 1706, 3413, and 5119) in every replacement matrix. It compares exact
re-quantization from original BF16 inputs and all 2,228,224 decoded values
between C and NumPy, reading about 6.3 MB of payloads. This verifies sampled
generated rows and complements the streaming hashes; it does not claim a
second full-file hash pass. All 128 rows re-quantized to the exact generated
bytes, and every decoded f32 bit matched. The checker read 6,284,288 payload
bytes and stayed below 100 MB of sampled process-group memory.

Next, run native resident/on-demand equivalence if the
placement changes. Retain the original BF16 goldens unchanged. Run all 104
answers, images, chunk/prefix/batch checks, and the speed gate before claiming
acceptance. A single corrected case is not enough.

## Runtime follow-up

The current `q4k-on-demand` mode leaves Q6 FFN weights resident as f16. For this
candidate that adds 4,099,932,160 bytes (3.818 GiB) of Metal buffers compared
with the old packed Q4 weights, plus the larger mapped artifact. Check the
existing 52 GiB process cap first; do not raise it automatically.

If the candidate does not fit, add a separate opt-in packed-FFN mode for
Q4_K/Q6_K/Q8_0. The kernels and `linear_with_workspace` already accept those
formats. Extend only placement selection and its serial/batch tests, retaining
the 170 MiB conversion slot, ordered dispatches, allocation estimates, and
default resident behavior. Packed Q6 adds only 0.685 GiB of Metal weight
buffers for this candidate. This placement change does not improve unchanged
Q4 values; accuracy depends on the newly generated artifact.
