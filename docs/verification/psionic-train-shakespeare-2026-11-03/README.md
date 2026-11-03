# Useful single-GPU training: the frozen workload, 2026-11-03

This record serves [#12627](https://github.com/OpenAgentsInc/openagents/issues/12627).
The section "Frozen workload" was committed before any Coax device kernel
for it was written, before any baseline ran and before any tuning. Later
sections add evidence; they never edit the frozen section. A change to it
needs a new workload name.

## Frozen workload: `char-gpt-shakespeare-v1`

**Corpus.** Tiny Shakespeare, `data/tinyshakespeare/input.txt` of
[karpathy/char-rnn](https://github.com/karpathy/char-rnn) at commit
`370cbcd448eb7daf32f21a6be560b70e0b33c4e3` (repository MIT; the text is
Shakespeare, public domain). 1,115,394 bytes, SHA-256
`86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed`.
Fetched once to `~/.cache/psionic/tinyshakespeare/input.txt`; it is never
committed.

**Tokens.** One token per byte. The vocabulary is the 65 distinct bytes of
the file in ascending order, ids 0–64:
``"\n !$&',-.3:;?A…Za…z"``. Every one occurs in the training split.

**Split.** Training is bytes `[0, 1003854)` (⌊0.9·n⌋), SHA-256
`a9e24e23a1ec77744dad26844bfd5a09b6e041954e1eef0000e7f24cba6db735`.
Held out is bytes `[1003854, 1115394)`, 111,540 bytes, SHA-256
`c54f3753a4e6e3c3d1759212815a7caf826e68a33021b25312984400bed40a1f`.
The held-out text is the end of the file (the last plays' scenes); no
training row reads it. This is the nanoGPT `shakespeare_char` split.

**Rows and order.** A step reads B = 64 rows of T = 256 inputs and their
256 next-byte targets. Row r of step s starts at
`cr_bits(7, 64·s + r) mod (1003854 − 256)` in the training split, with
`cr_bits` the splitmix64 counter map of `psionic/lib/counter-rng.coax`
(`z = seed + 0x9e3779b97f4a7c15·(counter+1)`, then the two splitmix64
multiply-xorshift rounds, all modulo 2^64). Rows are sampled with
replacement; there are no epochs.

**Model.** A pre-norm decoder: vocabulary 65, width d = 384, 6 layers,
6 heads of 64, context 256, MLP hidden 1536.

- `x = tok[id] + pos[t]` (learned token and position tables).
- Each layer: `h = rms(x)`; `[q k v] = h·Wqkv`; causal softmax attention
  per head with scale exactly 1/8; `x += o·Wo`; `h = rms(x)`;
  `x += relu(h·Wup)² · Wdown`.
- `logits = rms(x)·Whead`; loss is the mean natural-log cross-entropy of
  all B·T targets.
- `rms(x) = x / sqrt(mean(x²) + 1e-5)`, without a gain. No biases, no
  dropout, no weight tying. Every weight multiplies on the right
  (`x·W`, `W[d_in, d_out]`).

Parameters, in this flat order: `tok[65,384]`, `pos[256,384]`, then per
layer `Wqkv[384,1152]` (columns q, k, v; head h uses columns 64h…64h+63 of
each), `Wo[384,384]`, `Wup[384,1536]`, `Wdown[1536,384]`, then
`Whead[384,65]`. Total **10,765,056** parameters.

**Initialization.** Tensor k (0-based, in the order above) element i is
`std · z`, rounded once from binary64 to FP32, with
`z = sqrt(−2 ln u1)·cos(2π u2)`, `u1 = ((cr_bits(1000+k, 2i) >> 11) + 1)/2^53`,
`u2 = (cr_bits(1000+k, 2i+1) >> 11)/2^53`. std is 0.02, and 0.02/√12 for
every `Wo` and `Wdown`.

**Optimizer.** AdamW with β1 = 0.9, β2 = 0.99, ε = 1e-8, decoupled weight
decay 0.1 on every parameter (all are matrices), PyTorch's update order
(decay, moments, bias-corrected step). Peak rate 1e-3, linear warmup over
the first 100 steps (`1e-3·(s+1)/100`), then cosine to 1e-4 at step 2000.
The global gradient norm is clipped to 1.0 (`scale = min(1, 1/(norm+1e-6))`).
**2000 steps**, 32,768,000 training tokens. A step's gradient is the mean
over its 64 rows; it may be accumulated from micro-batches.

**Quality.** Held-out loss is the mean cross-entropy (nats per byte) over
the 435 non-overlapping windows of 256 inputs at held-out offsets
0, 256, …, 111,104: 111,360 predictions, all of the held-out split but the
last 180 bytes. Measured at step 0, every 250 steps and at step 2000.

- **Threshold:** the final held-out loss is at most **1.70** nats/byte.
- **Comparator:** it is within **0.03** nats/byte of the independent
  PyTorch baseline's final held-out loss on this exact workload.

**Numerical policy, FP32 profile (`fp32`).** Parameters, gradients, both
AdamW moments and activations are FP32 on the device. Products and sums
round once per operation (NVRTC `--fmad=false`); every reduction has a
fixed order, without atomics, so a run is deterministic. `exp`, `log` and
`sqrt` come from the CUDA device library. Learning rate and bias
corrections are computed in binary64 on the host and passed as FP32.

**Numerical policy, BF16 profile (`bf16`).** Matrix-product operands are
rounded to BF16 (round to nearest even) and multiplied on tensor cores with
FP32 accumulation; FP32 master parameters, gradients, moments, norms,
softmax and loss. The loss scale is static 1 (BF16 has FP32's exponent
range). Overflow policy for both profiles: a step whose gradient norm is
not finite is skipped — parameters, moments and step count unchanged — and
counted; more than 10 skipped steps in a run is a failure.

**Resume.** Within one profile, a run saved after any step and resumed in a
new process must end with exactly the bits of the uninterrupted run.

**Independent baseline.** `psionic/tests/char_gpt_torch.py` implements this
specification in PyTorch (CPU, FP32), written from this text, not from the
Coax code. It is opt-in verification tooling, never shipped. The same file,
in binary64, is the independent oracle for gradients and AdamW updates.

**Hardware and resource goals.** One NVIDIA GeForce RTX 4080 (16 GiB,
sm_89), shared with other work. Goals, frozen now and reported whether or
not they are met:

- End-to-end training throughput of at least **50,000 tokens/s** in the
  `fp32` profile (steps including data upload and every host decision;
  evaluation excluded and reported separately).
- Peak device memory of at most **8 GiB**.
- Compile and startup (NVRTC plus allocation and initialization) at most
  **60 s**.

**Fallback rule.** Every step runs only device kernels; the step reads back
only the loss and the squared gradient norm. A step that would need any
host arithmetic on parameters, activations or gradients is refused, never
performed, and the run's transfer ledger is checked after every step.

**Gates.** Each command finishes within 15 minutes; the full job resumes
across commands from checkpoints. Every attempt, failure and the total
tuning time are kept below.

## Amendment: `char-gpt-shakespeare-v2`, frozen before any full-size Coax run

The independent PyTorch baseline of v1 (CUDA, FP32, TF32 off) showed that
v1's 2000-step budget overfits this 1 MB corpus without dropout: held-out
loss 4.236 (step 0), 1.794 (250), 1.565 (500), **1.530 (750)**, 1.619
(1000), 1.844 (1250), rising after, while the training loss kept falling
(0.50 at step 1500). The baseline itself would fail v1's 1.70 threshold, so
v1 is retired with that evidence kept (`baseline/v1-cuda.jsonl`). A first
CPU attempt of the baseline is kept too (`baseline/cpu-attempt-1.jsonl`):
on this shared 28-core host under a load average of about 75 it took
400–490 s a step and was stopped after 5 steps.

v2 is v1 with one change: **750 steps** (warmup 100, cosine to 1e-4 at step
750), 12,288,000 training tokens. Held-out loss is measured at steps 0, 250,
500 and 750. Threshold (1.70), comparator margin (0.03), numerical policies,
resume rule, resource goals and everything else are unchanged. This choice
used only the baseline's curve; no Coax full-size training had run. The
Coax device path had run only the tiny oracle shape (3 steps) at this point.

## Evidence (added after the freeze)

All runs: one RTX 4080 (sm_89, driver 595.71.05, NVRTC 13.2), idle at
146 MiB / 0% before and after (`v2/gpu-before.txt`, `v2/gpu-after.txt`);
no other job was stopped. Build: `psionic/tests/char_gpt_build.sh`; kernel
and library hashes in `v2/build.sha256`. Runs:
`psionic/tests/char_gpt_acceptance.sh BUILD CORPUS OUT`.

### What runs

`psionic/lib/transformer/char-gpt-kernels.coax` holds 16 Coax kernels that
`coax cuda` emits for NVRTC (`--fmad=false`): cooperative 64×64-tile FP32
products (256 threads, register prefetch, three orientations; causal, inner-
limited and head-sliced batched modes for attention), causal softmax and its
backward, RMS norm and its backward, ids, embedding and both table
gradients, ReLU², cross-entropy, a fixed-order two-pass sum, and AdamW. Every
reduction has one written order and no atomics. `char-gpt.coax` writes a
step as data (336 launches: two micro-batches of 32 rows, forward and
backward each, then four reductions); `char_gpt_device.c` only replays them.
Parameters, gradients, both moments and activations live in one device
arena (1,738,416,198 bytes planned); each step uploads the 64 row offsets
(512 B) and downloads two FP32 words (loss sum, squared gradient norm).
`char-gpt-policy.coax` (`#authority`) decides admission, the schedule, the
update constants, clipping, skips, the ledger and the checkpoint header;
`coax mutate --root-only`: **244 of 244 mutants killed**. There is no CPU
path in the device driver: a step that moved any other bytes or launched any
other kernel fails `cgp_ledger`, checked after every step.

### Independent numerical checks

- **PyTorch float64 oracle, tiny shape** (`oracle/`): initial values of all
  418,048 parameters are bit-identical; step-1 loss and gradient norm agree
  to 3.5e-7 relative; every step-1 gradient value is within 4.1e-5 of the
  oracle (scaled by |a|+|b|+1e-3·max; cosine 0.99999999999993); parameters
  after three AdamW steps with clipping within 3.9e-5. The CPU (C build of
  the same launches) and the device both pass.
- **Native self-checks** (`char-gpt selfcheck`, 0 failures): every tensor's
  first, middle and last gradient value against central differences of the
  step's own loss; two micro-batches against one batch of four rows; the
  AdamW launch against its formula; held-out windows; an infinite parameter
  makes the step's numbers non-finite and the decision skips the update with
  every parameter bit unchanged; the BF16 profile's gradient within cosine
  0.9999 and 1% norm of FP32's.
- **Coax checks**: 20 checks over the kernels (each product orientation and
  mode against its definition bit for bit on ragged tiles, BF16 rounding by
  hand, softmax/RMS/CE against binary64), the plan, init bits and row starts
  computed independently in Python.
- **Full shape against the baseline**: step losses 1–10 equal PyTorch's to
  7–8 significant digits (`v2/fp32/log.jsonl` against
  `baseline/v2-cuda.jsonl`); held-out at step 0 is 4.23626826 against
  PyTorch's 4.23626824.
- **Resume**: a run saved after 375 steps and resumed in a new process ends
  with a checkpoint byte-identical to the uninterrupted run
  (`v2/checkpoints.sha256`, `v2/resume.txt`): exact resume, no tolerance.

### Acceptance: `char-gpt-shakespeare-v2` on the RTX 4080

| | held-out 0 | 250 | 500 | **750** | tokens/s | step (median) |
| --- | --- | --- | --- | --- | --- | --- |
| PyTorch 2.12 CUDA FP32 (TF32 off), baseline | 4.2363 | 1.7911 | 1.5451 | **1.5283** | ≈150,000 (cuBLAS) | ≈0.11 s |
| Coax, `fp32` | 4.2363 | 1.7860 | 1.5411 | **1.5171** | **108,122** | 0.150 s |
| Coax, `fp32` resumed at 375 | — | 1.7860 | 1.5411 | **1.5171** | 107,633 / 108,191 | — |
| Coax, `bf16` | 4.2362 | 1.7754 | 1.5362 | **1.5154** | 108,748 | 0.150 s |

- **Quality:** 1.5171 ≤ 1.70, and |1.5171 − 1.5283| = 0.011 ≤ 0.03. The BF16
  profile reaches 1.5154 (0.002 from FP32). No step was skipped; 51 (FP32)
  and 47 (BF16) steps were clipped.
- **Throughput:** 108,122 tokens/s end to end (every launch, upload,
  download, decision and the per-step ledger; evaluation excluded) against
  the 50,000 goal: met. 12,288,000 tokens in 113.6 s of steps, 119.4 s wall
  including startup and four held-out passes (~0.33 s each). PyTorch with
  cuBLAS is about 1.4× faster; the Coax products run at roughly 8 TFLOPS of
  the card's FP32 rate without FMA.
- **Memory:** the plan is 1,738,416,198 bytes; the driver's free memory
  fell by 1,740,636,160 bytes; nvidia-smi peaked at 2,052 MiB for the whole
  card (1,898 MiB for the process). Goal 8 GiB: met. PyTorch peaked at
  3.50 GB allocated.
- **Compile and startup:** NVRTC 0.03–2.4 s (driver cache warm or cold),
  startup including corpus admission, tokenization, initialization and
  upload 0.46–3.4 s. Goal 60 s: met.

The BF16 profile is qualified numerically only: operands are rounded to
BF16 (nearest even) as they enter shared memory and multiplied in FP32 SIMT
arithmetic, not on tensor cores as the frozen text said, so it is not faster
and stores nothing in BF16. That part of the frozen BF16 policy is **not
met**. Loss scaling stays static 1, as frozen.

### Profile (Nsight Systems, 10 steps and one held-out pass)

`profile/kernels-10-steps.csv`: products take 93% of kernel time (nn 38%,
tn 32%, nt 23%); ReLU² 2.7%, softmax 1.9%, RMS 1.3%, the token-table
gradient 0.6%, AdamW 0.2%. One bottleneck was found and fixed during the
work: rounding BF16 operands right after their global load (`failures/`)
made the products wait on the load the register prefetch was meant to hide,
and the **FP32** run fell to 77,957 tokens/s (0.209 s a step). Moving the
rounding to the shared-memory store restored 108,122 tokens/s (0.150 s).

### Failures and cost

- v1 retired (baseline overfit), and the first CPU baseline attempt
  abandoned under host load: see the amendment above.
- A first full-size Coax run used a build still carrying v1's 2000-step
  schedule; it is kept as the Coax side of v1 (`v1-coax/`): held-out 1.5344
  at 750 and 2.8750 at 2000 against PyTorch's 1.5295 and 2.9090, 112,197
  tokens/s, the same overfitting.
- The BF16 rounding regression above.
- Evaluator budget: the whole-step finite-difference check ran out of the
  checker's 2·10⁹ steps, so it moved to the native `selfcheck`.
- Total: about one hour of wall time from freeze to these results; GPU time
  about 15 minutes (Coax runs about 12, PyTorch about 5).

### What remains

- BF16 on tensor cores (`cu_mma_bf16_16x8x16`), BF16 storage of weights and
  activations, and a dynamic loss scale.
- Faster products (larger register tiles, split-K for small outputs); the
  baseline's cuBLAS step is 1.4× faster.
- Distributed training (chapters 132–135) was not started.

## Evidence for #12701: the `bf16` profile on tensor cores

This section serves [#12701](https://github.com/OpenAgentsInc/openagents/issues/12701).
It runs the frozen `bf16` profile as written: products on the tensor cores
with BF16 operands and FP32 accumulation; FP32 master parameters, gradients,
moments, norms, softmax and loss; static loss scale 1. Nothing above is
edited. Runs: `psionic/tests/char_gpt_acceptance.sh BUILD CORPUS OUT mma`
(files in `v3-mma/`), one RTX 4080 idle at 146 MiB / 0% before and after;
no other job was stopped. The driver names this profile `full-mma`
(profile 2); `full-bf16` (profile 1) is #12627's operand-rounding profile,
kept unchanged for comparison.

### What runs

- **Tensor cores.** `ck_mma` in `char-gpt-kernels.coax` is one Coax warp
  kernel for every product orientation and causal mode. It calls the
  existing primitive `cu_mma_bf16_16x8x16` (`coax/lib/compute/mma.coax`),
  which `coax cuda` emits as one `mma.sync.aligned.m16n8k16` BF16
  instruction with FP32 accumulation. Each warp owns one 64×64 output tile
  as 4×8 fragments. No cuBLAS, cuDNN or hand-written CUDA is involved.
- **BF16 storage.** BF16 values are packed four to a u64 word. The kernel
  permutes fragment rows, columns and inner values (the same permutation on
  both operands, so every sum has the same terms). With that permutation, a
  lane loads one word per four values in every orientation (k-major,
  m-major or n-major) and writes BF16 outputs as whole words.
  Stored in BF16: a working copy of the parameters, rewritten from the FP32
  masters at the start of each step and each held-out pass; Q, K and V; the
  attention output; the normed activations' operand copies; ReLU²'s output;
  and the gradients that are only product operands (logits, residual, ReLU²
  input, attention output, dQ/dK/dV, dS). Kept in FP32: the masters,
  gradients and moments, the residual stream, the norms' inputs and
  outputs, scores and probabilities (softmax), the MLP pre-activation, the
  logits and loss, and every gradient a norm, the softmax or ReLU²'s
  backward reads.
- **No stored attention.** The backward pass recomputes each layer's scores
  and softmax with the same launches, so it gets the same bits, instead of
  keeping 50 MB of probabilities per layer.
- **Parameter gradients** are split along the 8,192 tokens into 10–64
  slices (`cg_split`), written to a scratch buffer and added in slice order
  by `ck_ksum`. Every sum still has a fixed order and there are no atomics,
  so a run is deterministic.
- **Policy** (`char-gpt-policy.coax`, `#authority`): profiles 0–2,
  `cgp_mma`, and `cgp_profile`, which refuses the tensor-core profile below
  compute capability 8.0. The header admits profile 2.
  `coax mutate --root-only` killed **265 of 265** mutants.
- **Loss scale.** Static 1, as frozen. No step was skipped, so no dynamic
  scale is needed.

### Independent numerical checks

The tolerance was written into `char_gpt_torch.py compare --bf16` before
the first comparison ran: step losses within 2e-3 relative, gradient norms
within 1e-2, step-1 gradient cosine at least 0.999 with a relative L2 error
of at most 0.05, parameters after three AdamW steps within 2× the sum of
the step rates (1.2e-4), and an update cosine of at least 0.9.

- **PyTorch float64 oracle, tiny shape** (`v3-mma/oracle/`): passes on the
  CPU and on the RTX 4080. Initial bits are identical. Step losses are
  within 1.2e-5 relative and norms within 3.3e-4. The step-1 gradient has
  cosine 0.9999945 and relative L2 error 0.0033. Parameters after three
  steps are within 5.3e-5, with update cosine 0.99985.
- **Coax kernel checks** (`char-gpt-kernels-checks.coax`): `ck_mma` against
  its binary64 definition, within 2⁻²⁰·(steps+2)·Σ|ab| plus one FP32
  rounding. Covered: quad, m/n-major and value-by-value loads; ragged
  shapes; acc 0, 1 and 2 (BF16 out); a 3-slice split; batched causal modes
  1–3 with head offsets and skipped tiles. Also covered: BF16 packing
  (ties to even), ReLU² into BF16 and the ordered slice sum.
- **Plan check** (`char-gpt-checks.coax`): no per-layer scores or FP32
  operand copies, every BF16 buffer word-aligned, every BF16 product output
  whole words of a row, 234 tensor-core launches a step, and a smaller plan
  than FP32's.
- **Native self-check** (`char-gpt selfcheck`, 0 failures; new
  `mma_failures`). The tensor-core step stays within 1% of the FP32 step's
  loss and within 2% of its gradient norm, with cosine above 0.999. Two
  runs of the same launches give the same bits. The held-out pass writes
  only its windows. An infinite parameter still skips the step. Infinities
  and every NaN survive BF16 storage.
- **Resume**: a run saved after 375 steps and resumed in a new process ends
  with a checkpoint byte-identical to the uninterrupted run
  (`v3-mma/checkpoints.sha256`, `v3-mma/resume.txt`). Resume is exact,
  with no tolerance.

### Acceptance: before and after on the RTX 4080

| | held-out 0 | 250 | 500 | **750** | tokens/s | plan | card peak (nvidia-smi) |
| --- | --- | --- | --- | --- | --- | --- | --- |
| PyTorch CUDA FP32 baseline (above) | 4.2363 | 1.7911 | 1.5451 | **1.5283** | ≈150,000 (cuBLAS) | — | 3.50 GB allocated |
| Coax `fp32`, same build and day (before) | 4.2363 | 1.7860 | 1.5411 | **1.5171** | 102,828 | 1,738,416,198 B | 2,052 MiB |
| Coax `fp32`, #12627's run | 4.2363 | 1.7860 | 1.5411 | **1.5171** | 108,122 | 1,738,416,198 B | 2,052 MiB |
| Coax `bf16` on tensor cores (after) | 4.2362 | 1.7924 | 1.5383 | **1.5204** | **236,998** | **1,370,547,798 B** | **1,702 MiB** |
| the same, saved at 375 and resumed | — | 1.7924 | 1.5383 | **1.5204**, checkpoint identical | 240,100 / 241,007 | 1,370,547,798 B | 1,702 MiB |

- **Quality:** 1.5204 ≤ 1.70, and |1.5204 − 1.5283| = 0.008 ≤ 0.03. The
  frozen gate is met; the result is 0.003 from FP32's 1.5171. No step was
  skipped, and 57 were clipped (51 for FP32).
- **Throughput:** 236,998 tokens/s end to end (12,288,000 tokens in 51.8 s
  of steps), 2.3× the same-day FP32 run and about 1.6× the PyTorch FP32
  baseline's cuBLAS step. The FP32 run's held-out losses repeat #12627's to
  every printed digit, so the FP32 path is unchanged.
- **Memory:** the plan falls by 21% (1.74 GB to 1.37 GB), and the card's
  peak falls from 2,052 MiB to 1,702 MiB. The driver's free memory fell by
  1,373,634,560 bytes.
- **Startup:** 0.7–5.0 s, including NVRTC.

### Profile (Nsight Systems, 10 steps and one held-out pass)

The 10-step profile (`v3-mma/profile/`) ran at 203,017 tokens/s, slowed by
the profiler. Kernel time splits as follows:

- Tensor-core products: 78% of kernel time. Of that, the attention
  products take 54 points: the 256 × 64 batched ones (P·V, Pᵀ·dO, dS·K,
  dSᵀ·Q) 28 and the causal score and dP tiles 26. The split parameter
  gradients take 18, and the dense forward and dX products the rest.
- BF16 copies: 5.1%. Softmax: 3.5%. ReLU²'s backward: 3.5%. RMS backward:
  2.6%.

The score tiles have k = 64, so their FP32 stores, one 4-byte value per
sector, dominate. They are the next bottleneck.

### Failures and cost

- **A naive tensor-core kernel was slower than FP32.** The first `ck_mma`
  read BF16 as u16, two loads per fragment word, and ran at 72,489
  tokens/s, 225 ms a step (`v3-mma/failures/`). The parameter-gradient
  products (36–144 warps for k = 8,192) and the attention products were
  the slowest. In a throwaway experiment that removed the generated code's
  bounds checks on those loads (never shipped), the same kernel ran at
  134,043 tokens/s. In a warp kernel every operand load comes straight from
  device memory under a bounds check, so the fix had to load fewer, wider
  words. Packing BF16 four to a word with the fragment permutation, plus
  the token split for parameter gradients, gives 237k without touching the
  checks.
- **Toolchain.** Built with the cached checked compiler `727901f6a2be`
  (`v3-mma/build.sha256`), because this worktree's own bootstrap had not
  finished when the runs started. The compiler sources were not changed.
- **Cost:** about 50 minutes of wall time from the first kernel to these
  results; about 6 minutes of GPU time.

### What remains

- Faster attention products: FP32 score stores as whole words, or a
  smaller tile for the 64-wide head products.
- A tensor-core path inside group kernels (chapter 68), so operands can be
  staged in shared memory. It needs compiler support in `coax cuda`.
- Distributed training (chapters 132–135).
