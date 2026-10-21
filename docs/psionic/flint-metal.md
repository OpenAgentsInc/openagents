# Native Flint on Apple silicon

Issue: [#12422](https://github.com/OpenAgentsInc/openagents/issues/12422).
The CUDA counterpart is [Native Flint on GCP](flint-native.md) (#12419).

Psionic runs Flint by Recursive on the Mac's GPU from Recursive's MLX INT4
checkpoint, read directly from its safetensors shards: no GGUF conversion, and
no `rec`, MLX, llama.cpp, or Python at run time. The model definition is the
one `psionic-flint` already has: `Config`, the chat template, the tokenizer,
`Sampling`, the stop tokens, and the reasoning split. The `metal` feature adds
an MLX INT4 loader and an executor over Metal kernels in
`psionic-backend-metal`.

| Item | Value |
| --- | --- |
| Checkpoint | `RecursiveSI/flint-int4` at `3b93f9053b530488405ae9fca51d4900d6ce21f0` (Apache-2.0) |
| Format | MLX affine INT4, group 64, BF16 scales and biases; INT8 embeddings; BF16 norms, sinks, and router |
| Pins | [`mlx-int4-artifact.json`](../../crates/psionic/crates/psionic-flint/testdata/mlx-int4-artifact.json): size and SHA-256 of every file; the shards and tokenizer match Hugging Face's LFS records |
| Artifact | `sha256:83be6d422b14b0e19070133ac4bfcffdfcd4265847305a68503193e06a3cd8da` (the manifest's SHA-256) |
| Numerical profile | `mlx_int4_metal_v1`: INT4 weights, BF16 activations and KV cache, FP32 accumulation, with `rec`'s BF16 rounding points |
| Pylon profile | `pylon serve --text-profile flint-metal`, advertised as `flint@sha256:83be6d42…` |
| Catalog row | `flint-int4-mlx` in [`models.json`](../../scripts/bench/pylon-models/models.json) |

## Credit: adapted from `rec`

The decode kernels adapt the design of Recursive's
[`rec`](https://github.com/recursive-oss/rec) engine (Apache-2.0, release
v0.1.0, commit `e53663b`), reimplemented as standalone Metal kernels
in Rust. [`kernels/flint.metal`](../../crates/psionic/crates/psionic-backend-metal/src/kernels/flint.metal)
carries the Apache-2.0 notice. Adapted from `rec`:

- The INT4 group matrix-vector product: each SIMD group reduces four rows;
  each lane reads sixteen activations prescaled by 1, 1/16, 1/256, and
  1/4096, so masked nibbles need no shifts, and the group bias enters as
  `bias * sum(x)`.
- The 256-thread RMSNorm, Q and K head norms, and NeoX RoPE.
- Split-K decode attention: the learned sink logit seeds partition 0, and
  partitions are merged in a second pass. The partition counts are `rec`'s,
  including BF16 partials past 1,024 keys on full-attention layers.
- Sigmoid top-6 routing with lower-index ties and renormalized weights.
- The BF16 rounding points of the expert combine and the residual, which
  `rec` matched to MLX's own graph.

`rec` runs these inside one persistent MLX kernel per token, or per eight
tokens with sampling on the GPU, synchronized by device-wide atomic barriers.
Psionic issues them as about 400 ordinary dispatches per token, but keeps
`rec`'s eight-token blocks:

- Each command buffer holds eight decode steps, and two are in flight.
- After each step, a one-thread kernel picks the next token and writes it
  where the next step's embedding reads it. The host reads tokens once per
  block and drops steps that ran past a stop.
- Greedy decoding takes the highest logit, with the lower ID on ties.
- Sampled decoding applies `Sampling::choose_ranked`'s rule in FP32: top-k
  over candidates ranked on the GPU, then temperature, then top-p. Each step
  takes one uniform draw from the host's seeded `StdRng`, in the same order
  `choose` takes them.

What makes `rec` faster than llama.cpp on this Mac, as read in its source:

- The persistent megakernel: no per-kernel launch gaps, and team-local
  barriers so each KV head and each expert waits only for its own team.
- Routing computed redundantly in every threadgroup, which removes a global
  barrier per layer.
- BF16 KV cache with a fixed 513-slot ring on sliding layers, and one
  append-only buffer for the 14 full-attention layers.
- Eight tokens per dispatch, with sampling on the GPU.
- MLX's INT4 `qmm` with matrix units for prefill, in 4,096-token chunks.

## What Psionic adds

- **Prefill** in chunks of up to 2,048 tokens:
  - an INT4 matrix product that stages 64x64 BF16 tiles and multiplies them
    with Metal 4 tensor operations where the compiler has them, falling back
    to BF16 simdgroup matrices;
  - a GPU expert plan (count, offsets, 64-row tiles, scatter), so routed
    experts run as grouped products without a host round trip;
  - flash attention. Up to 4,096 keys, a threadgroup owns one KV head and
    eight positions and shares each K and V block across that head's eight
    query heads. Past 4,096 keys, a Metal 4 tensor-operation kernel takes
    over: a threadgroup owns one query head and 64 positions and runs
    S = QKᵀ and O += PV on the matrix units. It is 3.6 times faster at 16K,
    and the shorter contexts keep the arithmetic that tracks `rec` most
    closely.
- **Sampling** ranks the top 20 candidates on the GPU in two stages, which
  take about 0.18 ms. A test checks the GPU choice against
  `Sampling::choose_ranked`, shared with the CUDA path. Another test checks
  `choose_ranked` against `choose` over the whole vocabulary.
- **Serving** through `psionic-flint serve --backend metal`. `/v1/models`
  reports `RecursiveSI/flint-int4`, its revision, artifact, profile, and
  `backend: "metal"`. Answers split reasoning from content, and each one
  reports its timing.

## Build and run

```sh
cd crates/psionic
export CARGO_TARGET_DIR="$HOME/work/openagents-target-1"
. ../../scripts/dev/cargo-build-env.sh
cargo build --release -p psionic-flint --features metal --bins
"$CARGO_TARGET_DIR/release/psionic-flint" serve --backend metal \
  --weights DIR --context 32768 --listen 127.0.0.1:18431
```

`DIR` holds the pinned files; startup checks every file's size and SHA-256.
The weights take 22.6 GB of GPU memory. A 32,768-token session adds about
0.9 GB of KV cache. Then serve it through a Pylon with its own home:

```sh
OPENAGENTS_PYLON_HOME=PATH pylon serve --engine http://127.0.0.1:18431/ \
  --text-profile flint-metal --pylon mac-m5max-flint \
  --label "Mac M5 Max Flint" --allow-any --dedicated
```

The profile serves one slot with a 32,768-token context and 16,384-token
answers. That's room for Flint's 4.3K-token median reasoning and its long
tail. It also sets 64 KiB of conversation text, a 900-second deadline, and
Flint's card sampling (temperature 1, top-p 0.95, top-k 20).

## Correctness

### Kernels against a CPU reference

`cargo test -p psionic-backend-metal flint::` covers every kernel on random
data:

- the INT4 matrix-vector product with the residual, including `rec`'s
  BF16-rounded activation sums;
- the INT4 matrix product in each mode (plain, residual, SwiGLU, and the
  BF16 router with its sigmoid), on both the tensor and simdgroup kernels;
- the INT8 embedding;
- head norm with and without RoPE;
- decode attention at 300, 1,024, and 1,500 keys;
- chunk attention across the cache and the chunk, on sliding and full layers;
- sigmoid top-6 routing with ties;
- the expert plan;
- GPU top-k with ties and a non-finite flag.

### Greedy agreement with `rec`

`rec` v0.1.0 ran on the same Mac at temperature 0 with `logprobs`; its token
IDs are the reference ([results](verification/flint-metal-2026-10-21/greedy-vs-rec.json)).

| Prompt | Free-running match | Where it diverged | Teacher-forced top-1 | Mean \|Δ log-prob\| |
| --- | --- | --- | --- | --- |
| 19 tokens | 82 of 128 | `rec` p = 0.43 | 127 of 128 | 0.009 |
| 1,985 tokens | 87 of 128 | `rec` p = 0.28 | 126 of 128 | 0.022 |
| 16,427 tokens | 75 of 128 | `rec` p = 0.36 | 126 of 128 | 0.029 |

Every free-running divergence falls where `rec` itself was nearly undecided.
Teacher-forced, the Metal path gives `rec`'s own tokens nearly the same
log-probability. The first version, whose prefill kept FP32 weights, matched
all 128 tokens at 19 tokens. Prefill now rounds dequantized weights to BF16,
as MLX does, and divergence comes from near-ties in summation order.
Exact equality would need MLX's own kernels.

### Logits against the BF16 reference

INT4 limits were declared before measuring: top-1 equal, total variation at
most 0.10, and centered-logit error at most 1.5 at every position.

On #12419's six positions (`<|user|>2+2…`, reference logits from eager FP32
computation on the original BF16 weights), the INT4 Metal logits **fail**
those limits ([report](verification/flint-metal-2026-10-21/int4-vs-bf16-six-positions.json)):

- Top-1 agrees at four of six positions.
- Position 5, where the assistant turn starts, matches exactly: `<think>` at
  probability 1.0.
- Positions 0 to 4 predict the user's own text. There the INT4 checkpoint is
  much flatter than BF16: entropy 5.0 against 2.3 at position 2, and total
  variation 0.42 to 0.97.

This is a property of the INT4 checkpoint, not of the kernels. The Metal path
agrees with `rec` on the same weights (the teacher-forced table above), and
Recursive distilled those weights from the BF16 model, which evidently didn't
keep predictions of user text. The limits stay as declared; this result is
recorded as a failure on these positions.

Serving depends on assistant positions. The request
[`int4-reference-request.json`](../../crates/psionic/crates/psionic-flint/testdata/int4-reference-request.json)
holds a prompt and the 64 assistant tokens both engines produce. Comparing it
needs BF16 reference logits from the CUDA host, so it's listed in the
after-close checks with the same limits.

## Speed on the M5 Max

The method is the research note's
([Local measurements](../research/flint/README.md#local-measurements-on-this-mac)):

- card sampling: temperature 1, top-p 0.95, top-k 20;
- up to 512 output tokens;
- one warm-up and three measured runs per prompt size;
- a unique tag at the start of each prompt, so no prefix cache helps;
- decode rate as output tokens minus one, divided by the time after the
  first token.

[`flint-rec-compare.py`](../../scripts/psionic/flint-rec-compare.py) is the
client. The Metal numbers come from the server's own timers, which use those
definitions. The `rec` column is the research note's recorded run on the same
Mac. The owner had `rec` stopped, so the two ran under **different
conditions**. The Metal run (source `70013c3fa4`) had a load average of 7 to
8 ([rows](verification/flint-metal-2026-10-21/speed.json)).

| Prompt | Engine | Time to first token | Prompt tokens/s | Decode tokens/s |
| --- | --- | --- | --- | --- |
| Short (18; 32 for `rec`) | Psionic Metal | **0.14 s** | 130 | **179** |
| | `rec` | 0.35 s | 92 | 125 |
| Medium (1,610; 1,641 for `rec`) | Psionic Metal | **0.92 s** | 1,755 | **183** |
| | `rec` | 1.11 s | 1,481 | 140 |
| Long (15,880; 14,950 for `rec`) | Psionic Metal | 9.69 s | 1,639 | **137** |
| | `rec` | 8.82 s | 1,695 | 109 |

- **Decode beats `rec`'s numbers** by 25 to 43%. Sampling runs on the GPU, so
  wall time per token equals GPU time.
- **Prefill beats `rec` up to 2K tokens and matches it at 16K.** At 16K, the
  prompt rate is within 3% of `rec`'s, and separate runs gave 1,667 to 1,755
  tokens per second. The long prompt here is also 930 tokens longer. The
  INT4 matrix product and full-attention layers dominate: about 0.5 s and up
  to 0.7 s per 2,048-token chunk.
- **Peak memory:** a 23 GB physical footprint, against `rec`'s 28 GB.
- A same-conditions run against `rec` on a quiet Mac is listed in the
  after-close checks.

## Pylon and the Grid terminal

A temporary Pylon, **Mac M5 Max Flint** (`mac-m5max-flint`, provider
`695678189c858a46…`), served the profile from this Mac on
`wss://relay.openagents.com` through the Metal worker:

- `pylon ask --model flint` aimed at it returned a stopped answer with a
  receipt.
- Under `openagents browser run`,
  [`check-grid-flint.py`](../../scripts/dev/check-grid-flint.py) walked to the
  Grid's shared terminal on the local site (signed out), clicked **Flint**,
  and sent one message. The [reply card](verification/flint-metal-2026-10-21/grid-turn.png)
  reads **Mac M5 Max Flint · Flint** and shows only the final answer. The
  site's [receipt](verification/flint-metal-2026-10-21/grid-turn-receipt.json)
  names `mac-m5max-flint`, outcome accepted, in 3.1 seconds.

With **Flint on GCP** also online and the same tier, the router chose GCP for
an unpinned turn. The check pins the Mac Pylon: it rewrites the turn's
`X-OpenAgents-Pylon-Model` header from `flint` to the door's exact-artifact
form, `flint@sha256:83be6d42…`. The page's client can't send that value,
because it drops model names over 64 characters (#12436).

The Pylon and worker ran as temporary processes for the check and were then
stopped. A permanent Mac service is the owner's decision.
