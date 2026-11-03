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
