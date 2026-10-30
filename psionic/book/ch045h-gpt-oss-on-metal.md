# 45h. gpt-oss-20b on Metal

[Chapter 45b](ch045b-a-complete-cuda-decoder.md) keeps the original 20b
decoder on an RTX 4080; [chapter 31](ch031-elephant-on-metal.md) moved
Elephant to the GPU of a MacBook Pro, an Apple M5 Max with 128 GB of unified
memory. This chapter moves gpt-oss-20b there too, so either model can live on
either machine and the Mac can serve both without the 4080.

Nothing about the model changes. The launch programs are chapter 45b's and 45c's
(`gd_program`, `gd_batch_program`), the kernels are the same 45 Coax
functions, and the engine has the same `pg_*` interface, so the Responses
server and the chat shell link either engine.

## The kernels lower unchanged

`coax metal-compute psionic/lib/gptoss/device-kernels.coax --release` admits
every kernel the plan names: MXFP4 decode, BF16 products, YaRN rotation from
the host-built table, sink/window attention and its two-stage softmax, the
clamped SwiGLU, routing, expert grouping, publication and greedy choice.
`device-prefill.coax` is not a kernel file. It is host planning (prompt
admission, chunking and the batched launch program) and compiles to C with the
rest of `device-host.coax`, so there was nothing to split out of it.
Every `f64` in the kernels is a sum, product, quotient, `exp` or `sqrt`, which
APPLE-FF-v1 represents as a float-float pair (chapter 31). No kernel needs an
`f64` bit pattern, an `f64` trigonometric function or an `f64` converted to an
integer. The one place the model needs trigonometry, the YaRN table, is built
once on the host in binary64 by chapter 43's code and copied to the GPU.

## The engine

[`psionic/platform/gptoss-metal.c`](../platform/gptoss-metal.c) performs the
plan through [`metal-host.m`](../platform/metal-host.m) and decides nothing.
The original bytes are authenticated by Coax at admission and copied once into
one shared buffer of 13.76 GB: with unified memory the GPU reads them where the
CPU wrote them, nothing streams, and admission checks the plan against the
GPU's recommended working set (107.5 GiB here) less what is already allocated.
Each step's program is one command buffer with launches in order. Its wait
reports a failed command buffer, or a Coax check that failed inside a kernel
(`PM_CHECK`). Either one poisons the session. There is no CPU fallback:
without a Metal device, `pg_open` returns `PC_ABSENT`.

Two CUDA features have no counterpart here yet. CUDA graph replay
([chapter 45f](ch045f-replay-the-device-program.md)) is replaced by plain
encoding: about 1,950 launches per token, and the CPU spends a small part of a
step encoding them. Uploads become `memcpy` into shared memory.

## Qualify against the CPU

Three checks, all frozen before the Mac was measured:

1. Chapter 45b's rule against chapter 45's recorded CPU vectors: every logit,
   residual trace and mixture weight within `|x - ref| / (1 + |ref|) < 0.0005`,
   expert IDs and the top token exact. Both prefixes pass. The largest errors
   are 1.0e-5 for logits and 4.9e-4 for the residual trace, about the same as
   on the 4080, whose largest trace error is 4.8e-4. The top tokens are 7 and
   200005, as on the CPU.
2. Greedy token identity: chapter 45's CPU engine, run greedily on the same
   rendered prompts, emits the same token IDs as the GPU for "Say hi." (24 IDs)
   and "What is 2 + 2?" (23 IDs). Both turns end with the return token and show
   the same final text as the 4080: "Hello! 👋 How can I help you today?" and
   "2 + 2 = 4."
3. A per-launch CPU repeat (`PSIONIC_DEVICE_QUALIFY=1`): every launch of
   layer 0 (windowed), layer 1 (full attention) and the readout runs again on
   the CPU from the GPU's own inputs. For an 8-token prompt chunk, 112 of 121
   launches are identical; for a decode step, 52 of 61. All the differences
   come from two sources. The f32 `exp` in the activation and the softmax
   differs from the C library by at most 3.2e-7 relative, a few ulps. The
   float-float sums of squares differ from binary64 by at most 2e-14.

The tiny owned model passes the same checks as on CUDA: logits, traces and
mixtures against its dense reference, every batch split byte-identical to
serial steps, cancellation, refused IDs, and an injected infinity that leaves
the public cache and logits unchanged. On the original model, a 129-token
history gives bitwise-identical caches and logits whether it runs as serial
steps or as batched prefill.

```shell
psionic/tests/run_gptoss_metal.sh                       # tiny model only
PSIONIC_DEVICE_QUALIFY=1 PSIONIC_CPU_GREEDY=1 \
  psionic/tests/run_gptoss_metal.sh "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee"
```

## How fast

The workload is the fixed one from chapter 45g: 1,024 forced prompt IDs then
128 forced decode steps, one warm-up and five measured runs. The machine was
shared with other work throughout, so the spread is wide.

| | Apple M5 Max (Metal) | RTX 4080 (CUDA, current) |
| --- | ---: | ---: |
| Decode, median | **34.8 tokens/s** (range 27.1–36.0) | 53.2 tokens/s |
| 1,024-token prefill, median | **6.44 s** (159 tokens/s) | 3.21 s |
| "Say hi.": 59-ID prefill / 24-ID decode | 0.35 s / 0.63 s | 0.21 s / 0.61 s (first build) |
| Resident GPU memory | 13.3 GiB of 107.5 GiB | 15.2 GiB of 16 GiB card |

Decode is bandwidth-bound, as the issue predicted, and runs at about two
thirds of the 4080 with graph replay. A launch-by-launch profile of 16 decode
steps puts 34% of GPU time in the selected-expert MXFP4 products, 24% in the
BF16 projections (mostly the 201,088-row head) and 12% in the softmax
denominators. Prefill spends 57% of its time in the batched MXFP4 products,
which are scalar loops here, and 10% in single-thread flag reductions. Faster
products need threadgroup tiles and Apple's matrix units in the compute
subset, as in chapter 31. That work is chapters 45d and 45e.

## Serve it

```shell
PSIONIC_API_KEY=... psionic/serve-gptoss.sh --metal \
  "$HOME/.cache/psionic/gptoss-original/20b/6cee5e81ee83917806bbde320786a8fb61efebee" 8794
```

`--metal` builds `responses-server` against this engine and serves
[chapter 48b's](ch048b-serve-responses.md) Responses API on its own loopback
port. Port 8080 stays the 4080's tunnel and 8793 stays Elephant on Metal.
The server is ready in about two minutes, nearly all of it spent hashing and
copying the 13.8 GB of weights. Each request runs alone. With the GPU shared,
three low-effort requests completed in 1.9 to 2.7 s, and streamed text started
1.2 to 1.9 s after a request was sent.

To use it from the desktop app, set `PSIONIC_URL=http://127.0.0.1:8794` and
`PSIONIC_GPTOSS_HOST=local` in the environment or in
`~/work/.secrets/psionic-responses.env`. Psionic turns then go to this Mac,
with no tunnel and no GPU switch, and the model picker says This Mac. Without
these settings, the app still points at the 4080 (`http://127.0.0.1:8080`
through its tunnel).

## Limits

One stream at a time, one machine, greedy decoding qualified. Prefill is about
half as fast as on the 4080. Sampled generation, tools and speculation on the
device keep the gates they have on CUDA. The [verification
record](../../docs/verification/psionic-gptoss-metal-2026-10-29.md) has the
runs, the profile and the build identities.
