# Chapter 71: throughput is a gate, 2026-11-01

## What was measured

The tg-gptoss20b-v1 gate (`psionic/lib/throughput-gate.coax`) on the original
gpt-oss-20b (`model.safetensors` at revision
`6cee5e81ee83917806bbde320786a8fb61efebee`), resident on one RTX 4080:
1,024 input IDs and 128 forced output IDs (`tg_id`), 2,048-position context,
concurrency 1, one untimed warm-up and five timed runs. Prefill is one
`pg_prefill` of 1,024 IDs; decode is 128 `pg_step` calls followed by one
wait, timed with the host's monotonic clock. Goals: prefill at most 2 s,
decode at least 25 tokens/s (128 tokens in at most 5.12 s). Verdict rule:
chapter 34's Welford mean and sample standard deviation; pass when
mean + 2 se is within the limit, fail when mean - 2 se is beyond it, too
noisy otherwise or when sd is over a tenth of the mean.

Greedy outputs on two fixture prompts ("Say hi." and "List the integers from
1 to 100, separated by commas.", up to 256 IDs) are taken before and after
the timed runs and must equal the reference words `447e9a1f919e90b3` (24
IDs) and `7c6705e4ab014dfd` (256 IDs). Those words come from the f32 path's
first run (unchecked, below); every later run, both paths, reproduced them.

## Host

| Item | Value |
| --- | --- |
| Machine | coderos-4080, NixOS, Linux 6.18.49 x86-64, GCC 15.2.0 |
| GPU | NVIDIA GeForce RTX 4080, 16 GB, sm_89, driver 595.71.05, max clocks 3105 MHz SM / 11201 MHz memory (not pinned) |
| NVRTC | CUDA 13.2 through `psionic/platform/cuda-host.c` |
| Compiler | cached coax release build `8925679b5e19…` (binary sha256 `18b90dec5d4e…`) |
| Load | shared host; one-minute load average 14 to 42 during the runs, recorded per run |

## Runs

1. f32 prefill, no reference words (to record them): prefill 3.097 to
   3.113 s, decode 56.3 tokens/s, greedy outputs repeated exactly; gate
   refused (outputs not checked), as designed.
2. f32 prefill, reference words given. Build
   `coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-host-8047c5dc9facccfb`, commit
   `fd72815cfa`. Load average 34.5 to 34.9.

   | Run | Prefill ns | Decode ns |
   | --- | ---: | ---: |
   | warm-up | 3,102,570,958 | 2,273,361,874 |
   | 0 | 3,103,625,482 | 2,274,467,422 |
   | 1 | 3,112,139,217 | 2,275,193,802 |
   | 2 | 3,105,208,106 | 2,276,227,004 |
   | 3 | 3,107,413,133 | 2,281,197,384 |
   | 4 | 3,109,107,931 | 2,277,115,570 |

   Prefill mean 3.107499 s, sd 3.33 ms, se 1.49 ms: **fail**. Decode mean
   2.276840 s (56.22 tokens/s), sd 2.64 ms: pass. Gate: **fail**.
3. Kernel profile of one f32 prefill (test build, CUDA events per launch):
   `gk_batch_mx_part` 2.047 s of 3.130 s device time (384 launches),
   `gk_batch_bf_part` 0.277 s, `gk_batch_scores` 0.164 s, flags 0.208 s,
   `gk_mx_finish` 0.110 s.
4. Tensor-core prefill, MMA module emitted without `--release` (checked
   arithmetic in the inner loop): prefill 4.834 s mean, fail. Not used.
5. Kernel profile with the `--release` MMA module, 4 slices: `gk_mma_part`
   0.513 s, `gk_mma_finish` 0.043 s, `gk_mma_split` 0.007 s; total device
   time 1.519 s.
6. Tensor-core prefill, `--release` MMA module, 4 slices, before the last
   check and plan-buffer changes (kernels identical): prefill mean
   1.516531 s, decode 2.274546 s; gate pass.
7. Final receipt: see below.

Final receipt, `psionic/tests/check_throughput_gate.sh ORIGINAL 5
447e9a1f919e90b3 7c6705e4ab014dfd 4` on the committed code (the commit word
reads `fd72815cfa+uncommitted` because it ran before the commit; the build
words hash the compiler, both CUDA modules and the host C):

```text
12 checks, 12 passed, 0 failed
12 checks, 12 passed, 0 failed
load average before: 18.22 22.32 25.25
device: NVIDIA GeForce RTX 4080 sm_89 16711352320 bytes
original 20b authenticated and resident in 46.681 s
prefill expert products: MXFP4-BF16-MMA-v1, 4 slices
greedy prompt=0 ids=24 word=447e9a1f919e90b3
greedy prompt=1 ids=256 word=7c6705e4ab014dfd
run=-1 prefill_ns=1507942394 decode_ns=2273047714 decode_tok_s=56.312 load1=14.99
run=0 prefill_ns=1509731025 decode_ns=2273655612 decode_tok_s=56.297 load1=14.99
run=1 prefill_ns=1508299997 decode_ns=2274044147 decode_tok_s=56.287 load1=14.99
run=2 prefill_ns=1509091290 decode_ns=2279963061 decode_tok_s=56.141 load1=15.07
run=3 prefill_ns=1513465083 decode_ns=2279978867 decode_tok_s=56.141 load1=15.54
run=4 prefill_ns=1512092657 decode_ns=2279540283 decode_tok_s=56.152 load1=16.22
greedy again prompt=0 ids=24 word=447e9a1f919e90b3
greedy again prompt=1 ids=256 word=7c6705e4ab014dfd
greedy outputs equal the reference words
device bytes in use=14783086592 of 16711352320
bound: build=coax-18b90dec5d4e-cuda-72df8e2ef7ee4f53-mma-858d3a0112b4f54f-host-c3cd797e1230584e commit=fd72815cfa+uncommitted machine=coderos-4080 Linux 6.18.49 driver 595.71.05
goal prefill: n=5 mean_ns=1510536010 sd_ns=2164347 se_ns=967925 limit_ns=2000000000 verdict=pass (none)
goal decode: n=5 mean_ns=2277436394 sd_ns=3281624 se_ns=1467587 limit_ns=5120000000 verdict=pass (none)
gate: pass (none)
load average after: 16.52 20.82 24.49
exit=0
```

Prefill mean 1.510536 s, sd 2.16 ms, se 0.97 ms: **pass**. Decode mean
2.277436 s (56.20 tokens/s): **pass**. Gate: **pass**.

## Tolerance

Decode is unchanged (gpt-oss-cuda-f32-v1; the f32 module's sha256 prefix
`72df8e2ef7ee4f53` is identical in every run). Prefill expert products use
MXFP4-BF16-MMA-v1 with chapter 69's frozen per-output bound, checked on the
CPU against the binary64 reference and chapter 66's rule against chapter
67's f32 kernels. On the original model the gate's own check holds: greedy
outputs on both fixture prompts are identical to the f32 path's.

## Checks

| Check | Result |
| --- | --- |
| `coax test psionic/lib/throughput-gate.coax` (debug, release) | 12 of 12 |
| `coax mutate psionic/lib/throughput-gate.coax --root-only` | 116 mutants, 116 killed |
| `coax test psionic/lib/gptoss/device-mma.coax` | 32 of 32 |
| `coax mutate psionic/lib/gptoss/device-mma.coax --root-only` | 288 mutants, 288 killed |
| `coax test` device-plan / device-prefill | 133 of 133 / 135 of 135 |
| `coax mutate` device-plan / device-prefill `--root-only` | no survivor on any changed line; 441 (plan) and 345 (prefill) survivors remain on unchanged pre-existing lines (the f32 planner, gd_emit bounds, gd_rope, gd_packed_fits), which this chapter did not touch |
| `psionic/tests/check_gptoss_device.sh` (tiny model, CPU, plain and ASan/UBSan, debug and release) | pass, including the tensor-core tiny prefill |
| examples `throughput-gate`, `throughput-gate-refuse` (debug, release) | equal to their transcripts |

## Limits

- Five runs per receipt; no P95, per-phase peak memory or transfer counts.
- One host, shared and loaded; clocks not pinned.
- No packaged reference engine; concurrency 1 only (continuous batching is
  chapter 175's).
- Chapter 79's Responses server does not turn the tensor-core prefill on.
