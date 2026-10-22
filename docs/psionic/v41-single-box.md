# DeepSeek-V4.1 on one machine

Status: built and measured on 2026-10-22 under the Maya harvest epic
([#12462](https://github.com/OpenAgentsInc/openagents/issues/12462)). Nothing
here changes the qualified profile or the pinned bundles: every feature is
behind a new flag, and a stage without them loads and runs as before.

The designs follow [Project Maya](../research/deepseek/2026-10-21-project-maya.md)
(`mw00/project-maya` at `451f9cb6a2`, MIT), reimplemented in Rust over
Psionic's own kernels. DFlash2 (CC BY-NC-ND) is not used.

## What a stage can do now

| Flag | Issue | What it does | Exact? |
| --- | --- | --- | --- |
| `--experts file` | #12463 | Reads routed experts in place from the mapped checkpoint shards; the page cache keeps the warm ones. Needs `--expert-cache`. | Yes |
| `--expert-cache auto\|GB\|slots:N` | #12463 | Keeps each layer's hottest routed experts in device slots. `auto` fills the device memory left after everything else, less 3 GiB. | Yes |
| `--miss-lane auto\|bus\|host` | #12463 | Where cache misses run: read over the bus from page-locked memory (`--experts host` only), or on host CPU threads. `auto` measures one expert both ways at load and keeps the bus only when it's at least 3% faster. | Yes |
| `--expert-profile FILE` | #12463 | Seeds each layer's slots with a routing profile's hottest experts. `chain --save-expert-profile FILE` writes one. | Yes |
| `--host-threads N`, `--frozen-cache` | #12463 | Host-lane threads (default: one per core); no swaps after load. | Yes |
| `--engram file` | #12466 | Reads Engram rows from the mapped shards: the host hashes each step's tokens and stages their rows before the step, so neither 101 GB table is pinned. | Yes |
| `--prefill layer-major` | #12464 | Runs a prompt step layer by layer: projections and routed experts batched over the step's tokens, uncached experts streamed once per layer through a device ring. | Yes |
| `--experts file --consumer-q DIR` | #12465 | Serves a requantized 2- or 3-bit expert artifact under its own profile name. | **No: lossy, unqualified** |

"Exact" means the tokens, logits, and per-layer observations equal the
ordinary stage's bit for bit. The CUDA tests check this on the synthetic
model (`cached_experts_equal_device_experts`,
`staged_engram_rows_equal_pinned_tables`,
`layer_major_prefill_equals_the_ordinary_step`, and the chain tests in
`psionic-v41-stage/tests/cuda.rs`); the host lane's scalar, AVX2, and
AVX-512 paths equal the device kernels' host model.

### How the cache works

- Each layer gets the same number of device slots. A slot map on the
  device gives every expert a slot, the bus lane, or the host lane, so the
  matrix-vector kernel picks each selected expert's weights itself and a
  decode step stays one captured graph.
- After routing, a kernel posts the token's FP8 activation and the
  selected ids to a mailbox in page-locked memory. A poller thread computes
  the host-lane experts on the host lane's threads, in the GPU kernel's
  summation order, while the GPU runs the cached ones; a second kernel
  waits for the answer before the expert sum.
- Heat counts each expert's uses and decays by 0.9 every 256 tokens. A host
  expert replaces the coldest cached one when its heat passes that
  expert's heat × 1.25 + 1. A thread stages the incoming expert during a
  step; the copy into its slot happens between steps.

## Measurements

One spot `g4-standard-48` in `asia-northeast1-b`: one RTX PRO 6000
(97,887 MiB), 48 vCPUs (AMD EPYC, AVX-512), 176 GiB of RAM, and a 900 GB
Hyperdisk Balanced at 2,400 MiB/s and 40,000 IOPS. Checkpoint copied from
`gs://openagents-psionic-models-ane1/` (476 GB in 5 minutes). All runs are
`psionic-v41-stage chain --split 0-40 --gpus 0` unless noted, greedy, with
`--experts file --expert-cache auto --engram file`.

The cache took 116 slots per layer (30% of the routed experts, 87 GB).
The other 70% (about 204 GB) don't fit the 176 GiB of RAM beside the
system, so some host-lane misses read from disk.

| Run | Prompt | Decode | Hit rate | Notes |
| --- | --- | ---: | ---: | --- |
| a1: no profile, third answer | 51 tokens | 19.8 tok/s | 70% (all rounds) | First answer after load: 4.6 tok/s |
| a2: profile from a1, third answer | 51 tokens | 24.3 tok/s (p50 40.8 ms, min 33.6 ms) | 89% | |
| a3: profile from a2 | 2,129 tokens | 14.3 tok/s (p50 68 ms) | 62% | A document prompt routes differently from the profile's chat |

- **Lanes.** At load one expert took 38 µs from a device slot and
  370–460 µs on the host lane (48 threads, pages partly on disk). A host
  request (a layer with any host-lane expert) averaged 2.9 ms, mostly page
  faults. The bus lane isn't available here: it needs every expert
  page-locked.
- **Engram from the shards.** 48 rows a decode token; with parallel reads
  and random-access advice, 3.1 s across 921 tokens (a2), down from 13.1 s
  with one read after another (a1).
- **Prefill.** Token by token, the single box prefilled the 2,129-token
  prompt in 131 s warm (16 tok/s). Layer-major with uncached experts
  streamed through the ring took 66.7 s warm (32 tok/s), of which the host
  copies of 23,364 streamed experts took 52 s a round.
- **One 10-layer stage** (layers 0–9, experts on the device, 2,129-token
  prompt, `--first-stage-only`): 121.6 tok/s token by token, 148 tok/s
  layer-major with only the projections batched. The token-independent work
  is now batched too; its measurement is pending.
- **Exact profile ceiling.** A decode token with every expert hit takes
  about 33 ms on this card (four 10-layer stages' worth of the device
  graph), so the exact profile can't pass about 30 tok/s here; Maya's 35
  tok/s used a 2–3-bit quant.

Answers were the same text as the device-expert runtime's ("The sky
appears blue because sunlight is scattered by gas molecules…").

## `consumer-q` (lossy, unqualified)

`psionic-v41-stage quantize --weights DIR --out DIR --recipe q2q3|q3
[--importance FILE,...]` writes an artifact: per layer one file of 384
experts, each `[w1, w2, w3]` as code bytes, a log-scale byte per 32 values,
and an `f32` scale per row; `manifest.json` records the recipe, formats,
source checkpoint, importance digest, and each file's SHA-256.

- Values are Lloyd–Max levels of a unit normal: 4 levels at 2 bits, 8 at
  3 bits. `q2q3` keeps gate and up at 2 bits and down at 3 (about 2.6 bits a
  weight, 175 GB); `q3` keeps all three at 3 (about 3.3 bits, 221 GB).
- The quantizer picks each block's scale by importance-weighted squared
  error over the columns' calibration mean squares
  (`chain --prefill layer-major --prompts-jsonl FILE --calibrate-out
  PREFIX`). There is no GPTQ error feedback yet.
- A stage serving it announces the profile `consumer-q-cuda-device-sm_NN`,
  so a group expecting the exact profile refuses it.
- The quality card (`scripts/bench/deepseek-v41/consumer_q_card.py`)
  teacher-forces the oracle's own tokens (`chain --prompt-ids --force-ids
  --capture`) and scores KL, total variation, top-1 agreement, and the
  oracle token's log-probability error against the oracle's decode logits;
  G2 suite 2 runs through `g2_check.py` beside the oracle's and the exact
  runtime's answers.

Calibration, the artifact, and the card are pending.

## Cost

The `g4-standard-48` ran from 04:01Z; see #12463 and #12464 for each run's
cost. The spend guard's `deepseek-v41` budget carries every machine.
