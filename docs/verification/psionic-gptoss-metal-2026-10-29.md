# gpt-oss-20b on Metal, 2026-10-29

Scope: issue #12607, [chapter 45h](../../psionic/book/ch045h-gpt-oss-on-metal.md).
This record covers the original gpt-oss-20b served by Psionic from an Apple
GPU, using chapter 45b/45c's Coax launch programs and kernels, and qualified
against chapter 45's CPU reference under rules frozen before the device was
measured.

## What was run

- Machine: MacBook Pro, Apple M5 Max (Apple GPU family 10), 128 GB unified
  memory, recommended working set 107.5 GiB, largest buffer 80.6 GiB, macOS
  26.4. The machine was shared with other agents' work throughout. GPU
  utilization from other processes was 54–70% when sampled between requests.
- Model: original `openai/gpt-oss-20b`, revision
  `6cee5e81ee83917806bbde320786a8fb61efebee`, in
  `~/.cache/psionic/gptoss-original/20b/`. Coax authenticates every artifact at
  admission. The weights stay outside Git.
- Kernels: `psionic/lib/gptoss/device-kernels.coax`, unchanged, emitted by
  `coax metal-compute --release`. All 45 kernels that the plan names are
  admitted. `device-prefill.coax` is host planning compiled to C, not a kernel
  file, so nothing was split out of it.
- Engine: `psionic/platform/gptoss-metal.c` on `psionic/platform/metal-host.m`.
  All 13,761,300,984 weight bytes are resident in one shared buffer, the
  workspace takes 527,933,440 bytes, and nothing streams. There is no CPU
  fallback.
- Harness: `psionic/tests/run_gptoss_metal.sh` with `PSIONIC_DEVICE_QUALIFY=1
  PSIONIC_DEVICE_PROFILE=1` and five repeats, plus `gptoss_cpu_greedy.c` for
  the CPU repeat. [Build identities](psionic-gptoss-metal-2026-10-29/build.json).
- Numerical profile `gpt-oss-metal-f32-ff-v1`: the CUDA profile's f32 storage
  and lane sums, with its ordered f64 reductions computed as APPLE-FF-v1
  float-float pairs (chapter 31).

Commits: `f937335f58` (engine and harness), `f575a47b93` (Responses
server `--metal` and desktop endpoint setting).

## Qualification

1. **Chapter 45's recorded CPU vectors** (`gptoss_device_oracle.py`, bound
   `|x - ref| / (1 + |ref|) < 0.0005` frozen in chapter 45b, routes and top
   token exact). [Result](psionic-gptoss-metal-2026-10-29/qualification.json):

   | Prefix | Logits | Residual trace | Mixture weights | Routes | Top token |
   | --- | ---: | ---: | ---: | --- | ---: |
   | `[200006]` | 9.98e-6 | 4.887e-4 | 3.29e-7 | exact | 7 |
   | `[200006, 173781]` | 9.05e-6 | 3.923e-4 | 1.21e-7 | exact | 200005 |

   The 4080's largest trace error under the same rule is 4.805e-4.
2. **Greedy token identity.** Chapter 45's CPU engine (`psionic_gptoss_forward`,
   `gpt-oss-cpu-wide-v1`, temperature 0) was run on the prompt IDs the GPU
   harness rendered. Both runs generated
   [identical token IDs](psionic-gptoss-metal-2026-10-29/):
   - "Say hi.": 59 prompt IDs, 24 generated, return stop.
     Text: "Hello! 👋 How can I help you today?" The CPU run took 556.6 s,
     after 102.6 s of binding.
   - "What is 2 + 2?": 64 prompt IDs, 23 generated, return stop.
     Text: "2 + 2 = 4." The CPU run took 338.6 s.
   Both final texts and generated lengths also equal the 4080's recorded runs.
3. **Per-launch CPU repeat.** Every launch of layer 0 (windowed), layer 1 (full
   attention) and the readout ran again on the CPU, using the same kernels
   compiled to C and starting from the GPU's own inputs. The CPU run widens
   float-float views to binary64, and every byte of every workspace buffer the
   launch touched was compared.
   - Prompt chunk (8 tokens at position 9): 112 of 121 launches identical.
   - Decode step (position 17): 52 of 61 launches identical.
   - The differences are f32 `exp` in `gk_activate` and the softmax, at most
     3.16e-7 relative (a few ulps: Metal's precise `exp` against the C
     library), and float-float sums of squares at most 2e-14 against binary64.
     The decode softmax denominators differ by at most 3.7e-8 because they sum
     those `exp` values. No other kernel differs.
4. **Owned and structural checks.** The tiny owned model passes the same checks
   as on CUDA: logits, traces and mixtures against its dense reference; every
   batch split byte-identical to serial steps; cancellation; refused IDs; and
   an injected infinity that leaves the public state unchanged. On the original
   model, a 129-token history gives a bitwise-identical complete cache and
   logits whether it runs as serial steps or as batched prefill.

The [run log](psionic-gptoss-metal-2026-10-29/run.log) has every line.

## Throughput

The workload is fixed: 1,024 forced prompt IDs, then 128 forced decode steps,
with one warm-up and five measured runs. The 4080's current numbers are from
the [decode pass 2 record](psionic-decode-pass2-2026-10-29.md).

| | M5 Max, Metal | RTX 4080, CUDA |
| --- | ---: | ---: |
| Decode, median | **34.752 tokens/s** (3.683 s; range 27.1–36.0) | 53.157 tokens/s |
| 1,024-token prefill, median | **6.435 s** (159.1 tokens/s; range 5.97–6.99 s) | 3.21 s |
| "Say hi." prefill / decode | 0.352 s / 0.629 s (36.6 tokens/s) | 0.209 s / 0.608 s (tuning 1) |
| Admission (authenticate and copy) | 136.2 s | 60–85 s |
| GPU memory | 13.31 GiB allocated | 15.2 GB of 16 GiB |

Profile, one command buffer per launch:

- 16 decode steps at position 1,024: MXFP4 expert products (`gk_mx_part`)
  34.1%, BF16 projections and head (`gk_bf_part`) 23.7%, softmax denominators
  (`gk_softmax_prepare`) 11.9%, routing 5.2%.
- 1,024-token prefill: batched MXFP4 products (`gk_batch_mx_part`) 57.2%,
  batched BF16 products 11.9%, single-thread flag reductions
  (`gk_flags_finish`) 10.5%, attention 8.5%.

Decode is about two thirds of the 4080's rate, which runs with graph replay,
and prefill is about half. The products are scalar loops on Metal. Threadgroup
tiles and Apple's matrix units in the compute subset (planned chapters 45d and
45e) are the next step.

## Serving

`PSIONIC_API_KEY=… psionic/serve-gptoss.sh --metal ORIGINAL_CACHE 8794` built
and served from a temporary build of the commits above. It answered
`/v1/models`, and three non-streamed low-effort requests completed in 2.37 s,
2.66 s and 1.94 s: "Hello! 👋 How can I help you today?" (59 in, 24 out),
"2 + 2 = 4." (64 in, 23 out) and "Red, blue, and yellow." (61 in, 20 out).
Three streamed "Say hi." requests showed their first text after 1.23–1.91 s
and finished in 1.68–2.97 s. While a request ran, `sample` showed the request
thread waiting on its command buffer most of the time, so the requests were
GPU-bound on a shared GPU. The listener and key were removed afterward.

The desktop app reaches this server with `PSIONIC_URL=http://127.0.0.1:8794`
and `PSIONIC_GPTOSS_HOST=local`, which skips the tunnel and the GPU switch
for gpt-oss. The app's default is unchanged (the 4080 through
`127.0.0.1:8080`). `coax test apps/openagents-desktop/zeron/screen.coax`
passed 134 of 134 checks, including the new check that a local gpt-oss sends
its message at once with no switch and no tunnel. The app itself was not
rebuilt or relaunched in this pass.

## Not established

- Concurrency above one stream.
- Sampled generation, tools and speculation on the device.
- 120b on Metal.
- Prefill at the 4080's rate.
- Behaviour on Apple GPUs other than this M5 Max.
- Timing on an otherwise idle GPU.
