# Inkling 2 Mini on CUDA

The native Psionic CUDA runtime for Inkling 2 Mini (`inkling2_loop` GGUF) on
NVIDIA GPUs, for Linux and Windows. It is Track B of
[`docs/desktop/openagents-v1-spec.md`](../../docs/desktop/openagents-v1-spec.md)
(sections 5.3 and 5.4, issue #13966). The Metal runtime is described in book
chapter 269.

## Files

| File | What it is |
| --- | --- |
| `psionic/platform/inkling-engine.h` | The engine contract. The server and the CLI include only this. `inkling-metal.h` now includes it. |
| `psionic/platform/inkling-cuda.c` | The CUDA engine: memory plan, weight upload, expert cache, layer dispatch. No model arithmetic. |
| `psionic/lib/inkling/kernels-cuda.coax` | Every kernel, in Coax. `coax cuda` emits the CUDA source that NVRTC compiles at startup. |
| `psionic/lib/inkling-budget.coax` | The VRAM planner (`#authority`) and its tests. The engine makes the same plan with the same integer arithmetic. |
| `psionic/platform/cuda-host.c` | Loads the driver and NVRTC at run time: `libcuda.so.1` / `libnvrtc.so.*` on Linux, `nvcuda.dll` / `nvrtc64_*.dll` on Windows. |
| `psionic/platform/inkling-gguf.c` | Maps the GGUF read-only: `mmap` on POSIX, `CreateFileMappingA` / `MapViewOfFile` on Windows. |
| `psionic/platform/inkling-serve.c`, `inkling-cli.c` | One source for every engine. Sockets, the clock and the stop signal are the only code that differs by system. |
| `psionic/platform/inkling-cuda-build.sh` | Builds the kernels, the server and the CLI. |
| `psionic/tests/inkling_llama_oracle.c`, `inkling_logits_compare.coax` | The llama.cpp comparator and the logit comparison. Never linked into anything shipped. |

Nothing links cuBLAS or cuDNN, and the build needs no CUDA toolkit.

## Build and run

```shell
coax/build.sh
psionic/platform/inkling-cuda-build.sh             # into psionic/build
psionic/build/psionic-inkling-cli --kernels psionic/build/inkling-kernels-cuda.cu --test-quant-kernels
psionic/build/psionic-inkling-serve --model ~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf \
    --kernels psionic/build/inkling-kernels-cuda.cu --ctx 32768 --port 18794
```

To cross-compile for Windows with MinGW:

```shell
TARGET=windows CC=x86_64-w64-mingw32-gcc psionic/platform/inkling-cuda-build.sh OUT_DIR
```

`--kernels` names the engine's generated kernel source. `--metal` is the same
option, kept for the macOS scripts. If NVRTC is not on the loader's path, set
`PSIONIC_NVRTC` to the library.

## Memory: the expert cache

A discrete GPU holds weights in VRAM, apart from host memory. Of the 16.4 GiB
checkpoint, 15.07 GiB is the MoE experts, and a token uses 8 of each layer's
128.

- Every tensor except the experts and the token embedding is copied to VRAM
  once (about 1.25 GiB).
- The token embedding stays in host memory. A batch uploads its own rows.
- Each layer has an expert cache of `slots` experts. With room for all 128 the
  layer is resident and a step never waits on the host. With less, the
  remaining experts stay in host memory (the mapped GGUF). After a layer's
  router runs, the host reads the chosen ids and the router's logits, uploads
  the experts not in the cache, and gives the kernels slot numbers in place of
  expert ids. The expert replaced is the one that has waited longest; among
  experts that have waited equally long (at the start, all of them), it is the
  one the router has lately scored lowest. That cut the misses of a 128-token
  reply from 551 to 422. The arithmetic is the same either way: an 8 GB plan
  and a 16 GB plan produce bit-identical logits.
- A prefill batch is up to 128 tokens (`INK_MAX_BATCH = 128` on GPUs with >= 12 GiB
  free, 64 on 8–12 GiB, 16 below 8 GiB). With grouped-expert Tensor Core MoE
  streaming, in-cache hits execute in Pass 1 and missed experts stream in
  chunks of at most `slots` experts without thrashing the cache. (The legacy
  scalar/table fallback path with `INK_NO_MMA=1` or `INK_NO_TABLES=1` still
  caps the batch at `min(32, slots / 8)`.)

The plan is made before any large allocation: `resident`, `streaming`, or
`refused` when even 16 slots a layer do not fit. A refusal names the VRAM
needed and the longest context that fits.

Environment switches for testing and diagnostics:
- `INK_VRAM_MB` plans as if that much VRAM were free.
- `INK_EXPERT_SLOTS` caps every layer's expert cache.
- `INK_BATCH` caps the prefill batch size.
- `INK_NO_MMA=1` disables Tensor Core (`cu_mma_bf16_16x8x16`) prefill kernels.
- `INK_NO_GRAPH=1` disables CUDA Graph decode capture/replay.
- `INK_NO_TABLES=1` runs direct scalar/fallback kernels (also disables online-softmax prefill attention).
- `INK_PROFILE=1` synchronizes and prints per-kernel time (disables CUDA Graphs while profiling).
- `INK_ORACLE_Q8=1` rounds matvec inputs the way ggml does (parity diagnostic, see below).
- `PSIONIC_PTX_CACHE` overrides the PTX disk cache directory (default `~/.openagents/cache/psionic`, or `%LOCALAPPDATA%\.openagents\cache\psionic` on Windows); `PSIONIC_NO_PTX_CACHE=1` forces fresh NVRTC compilation.

## Measured on an RTX 4080 (16 GB), Linux, driver 595.71, 2026-11-22

See [inkling-cuda-hillclimb-report-2026-11-22.md](inkling-cuda-hillclimb-report-2026-11-22.md)
for the full optimization breakdown, register/spill audit, and bug postmortems.

Checkpoint `Inkling-2-Mini-IQ4_NL.gguf`, SHA-256
`cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`. The card
cannot hold the model, so every Psionic number below is with experts streaming
from host memory (114 of 128 experts a layer cached at context 4,096; 106 at
32,768). The comparison is llama.cpp b12626 (CUDA 13.5 build) with 22 of 25
layers on the GPU, the most that fit.

| Check | Result |
| --- | --- |
| PTX disk cache (`--test-quant-kernels` startup compile) | cold `3.77s` → warm **`0.00s`** (server load `1.43s` at ctx 32,768) |
| `--test-quant-kernels` | `nvfp4_max_abs=0.00e+00 mxfp8_max_abs=0.00e+00 iq4_max_abs=0.00e+00 ok=true` |
| `ptxas --gpu-name=sm_89 -v` spill audit | **0 bytes stack frame, 0 spill loads/stores** across all 47 kernels |
| `--verify-prefill-decode` (MMA batched prefill vs CUDA Graph 1-token decode) | `top1_diff=0.01818 max_abs=0.62459 rmse=0.124458 cos=0.999957 ok=true` |
| Decode, short context (128 tokens, 28-token prompt) | **`78.19 tok/s`** with segmented CUDA Graphs (`75.49 tok/s` with `INK_NO_GRAPH=1`, bit-identical `max_abs=0.00e+00`; llama.cpp: `8.6 tok/s`) |
| Decode after a 900-token prompt (64 tokens) | **`63.87 tok/s`** |
| Prefill, 900-token prompt | **`634.3 tok/s`** (`1.419s`, **4.06×** vs `INK_NO_MMA=1` at `156.2 tok/s`, **3.52×** vs prior `180 tok/s`; `cos=0.999777`, exact top-5 match; target `>= 500 tok/s` exceeded) |
| Prefill, 1,219-token prompt | **`632.7 tok/s`** (`1.927s`, **5.90×** vs scalar `107.3 tok/s`; run-to-run determinism `max_abs=0.00e+00`) |
| Prefill, 1,993-token prompt | **`598.0 tok/s`** (`3.333s`) |
| Plan as an 8 GB card (`INK_VRAM_MB=8192`, 1,219-token prompt) | prefill **`93.1 tok/s`** (up from `38.7 tok/s`), decode `35.89 tok/s`, logits **bit-identical** (`max_abs=0.00e+00`) to 16 GB plan |
| Plan with 3,000 MB free (`INK_VRAM_MB=3000`) | refused before allocation: `insufficient VRAM: 3000 MB free ... needs 3845 MB with the smallest expert cache (17028 MB with every expert in VRAM); largest context that fits: 0` |
| `psionic-inkling-serve --ctx 32768` (CUDA Graphs ON) | loads in `1.43s`, plans `15,422 MB` (`106/128` experts cached), answers `/v1/chat/completions` (`17 * 23 = 391`, `74.3 tok/s`) and `/v1/responses` (`23/23` prefix-cached in `0.001s`, **`83.1 tok/s`** warm decode) |
| Driver hidden | `pc_open` returns `PC_ABSENT`; the CLI exits with `no NVIDIA GPU: no CUDA driver library (libcuda.so.1)` |
| MinGW Windows cross-compile | `psionic-inkling-serve.exe` (295 KB) and `psionic-inkling-cli.exe` (200 KB) link with 0 warnings. Not run: no Windows host here. |

Note on the llama.cpp prefill number. When b12626 is given 32 or more tokens in
one batch on this card, it returns NaN logits for this model (at 64, 128, 256,
500, 900 and 1,356 tokens, with 0 or 22 layers on the GPU). Its batched speed
(about 440 to 570 tok/s) is therefore not a valid reference. In chunks of 16 or
24 tokens it is correct, and that is the number in the table.

### Parity with llama.cpp

Top-1 is the same in every comparison of a chat prompt. The cosine of the
logits is below 0.99999, and so is llama.cpp's cosine against itself:

| Prompt | Psionic vs llama.cpp CPU | Psionic vs llama.cpp CUDA | llama.cpp CUDA vs CPU |
| --- | --- | --- | --- |
| 23 tokens, K=3 | 0.9999481 | 0.9999504 | 0.9999825 |
| 23 tokens, K=5 | 0.9998894 | 0.9998531 | 0.9998976 |
| 23 tokens, K=3, `INK_ORACLE_Q8=1` | 0.9999790 | 0.9999880 | |
| 23 tokens, K=5, `INK_ORACLE_Q8=1` | 0.9999073 | 0.9999119 | |
| 1,356 tokens, K=3 | | 0.9998453 | 0.9998851 (chunks of 24 vs 16) |

The table is from 2026-11-21. On 2026-11-22 the 75.7 tok/s engine was compared
with the llama.cpp logits saved then: 0.9999481 at K=3, 0.9998894 at K=5 and
0.9999818 at K=3 with `INK_ORACLE_Q8=1`, top-1 the same in each.

ggml multiplies a quantized matrix by activations it has first rounded to 8
bits; this engine keeps activations in f32. With `INK_ORACLE_Q8=1` the engine
rounds them the same way, and its logits are then as close to each llama.cpp
backend as the two backends are to each other. llama.cpp rejects K=1 for this
checkpoint (`invalid loop core`), so there is no reference for K=1.

## How decode reached 75 tok/s

The first engine decoded at 30.6 tok/s. A token reads about 3 GB of weights
(48 logical layers, 64 MB each), and the card moves about 520 GB/s through
these kernels, so the weights alone take about 6 ms of a 13.2 ms token. What
was learned on the way, each measured on this card:

1. **A warp must read one contiguous run.** Quantized weights are stored
   block-major on the device (`ink_relayout`), so the 32 threads of a warp that
   hold 32 neighbouring rows read neighbouring words. In GGUF order the same
   kernels were no faster than the first engine.
2. **IQ4_NL through tables.** A byte holds two 4-bit codes. A group of 256
   threads builds 16 tables of 256 entries from 32 input values, and a row's
   block is then 16 lookups (`ink_moe_gu_t`, `ink_moe_down_t`).
3. **Q8_0 without a multiply per sign.** `byte ^ 128` is unsigned, and the block
   sum is `sum(u * x) - 128 * sum(x)`, with `sum(x)` shared by four rows
   (`ink_mvq_p`, `ink_proj_p`).
4. **A thread alone is slow.** A thread takes about 10 ns an instruction, so a
   kernel takes as long as its longest thread. Work that one thread did in a
   loop (a 64-term dot product, a 128-term key score, a sum of 256 parts) is
   split over eight or more threads that each do a few terms, and sums add up
   in two or three levels.
5. **Loads asked for together overlap.** A thread that loads everything it
   needs into locals before using any of it waits once, not once a load. The
   local arrays need constant loop bounds, or the compiler keeps them in
   memory and the kernel gets slower.
6. **Fewer launches.** A layer's eight attention projections are one launch
   over a table of matrices (`ink_proj_p`) and one for their sums. The three
   recurrence gates of a layer are one launch each for the gate and its
   output. K and V go from projection to KV cache in one launch (`ink_kv1`).
   The sum of a projection's shares, the post convolution and the residual
   add are one launch (`ink_sum_post`). A token is now about 750 launches,
   down from about 1,900.
7. **64-bit checked arithmetic is slow on the device.** Index arithmetic in hot
   loops is 32-bit wrapping (`add_wrap`, `mul_wrap`).

Decode rate after each step: 30.6, 49 (layout, tables, grouped loads), 60
(fused gates, norms and projections), 66.6 (mix and attention), 69.2 (K and V
path), 71.1 (gate outputs), 73.6 (sums and router), 75.7 (expert replacement).
Every step kept the logits: the cosine against the step before is 1.0000000,
and against llama.cpp it is what the table above shows.

## How prefill reached 634+ tok/s and decode reached 78–83 tok/s

1. **Cross-platform PTX disk cache (`cuda-host.c`).** Compiled PTX is cached by
   a 128-bit hash of the generated kernel source, byte length, SM capability,
   and driver version (atomic write-temp-and-rename on POSIX and Win32), taking
   warm startup compile time from `3.77s` to `0.00s`.
2. **Zero-spill Tensor Core (`cu_mma_bf16_16x8x16`) dense and grouped-MoE
   kernels (`kernels-cuda.coax`).** Per-lane fragments use unrolled scalar
   arrays so NVVM keeps every tile in registers (`0 B` stack frame across all
   47 kernels). `ink_proj_mma` (`16×32`) fuses all 6–8 attention Q8\_0
   projections into one launch; `ink_q8_mma` (`16×16`) handles `attn_output`;
   `ink_moe_{gu,down}_{iq4,q8}_mma` group tokens by active expert (up to 16
   tokens/task) with branchless BF16 dequantization and two-pass streaming
   (cached hits first, then chunked misses) so batches scale to `INK_MAX_BATCH
   = 128`.
3. **Batched Tensor Core cross-pass recurrence control gate.** `ink_ctrl_pack_bf16`,
   `ink_ctrl_gate_mma`, and `ink_ctrl_out_mma` cut recurrence control gate cost
   from `0.322 ms` to `0.078 ms/step`.
4. **Online-softmax GQA prefill attention (`ink_attn_online`).** Processes up
   to 8 Q vectors sharing one KV head (`t_step = 8 / min(gqa, 8)`, reusing each
   8-key KV tile in shared memory), handles exact sink logit `0` via initial
   `m = 0, s = 1`, and eliminates the `attn_scores` DRAM buffer for prefill (so
   `INK_MAX_BATCH = 128` fits at both 4K and 32K context while keeping
   `114/128` experts cached at 4K).
5. **CUDA Graph decode (`dev_pos` + `decode_graph[2]` / `stream_seg[]`).**
   `ink_kv1`, `ink_clip`, `ink_kv_store`, and `ink_attn` read the token
   position from a 4-byte device buffer `dev_pos` when `pos < 0`. Resident GPUs
   capture the entire forward pass into one graph; streaming GPUs capture one
   graph per inter-sync segment (collapsing ~750 launches into 49 graph replays
   per token). `cuda-host.c` binds the primary context per host thread via TLS
   so multi-threaded HTTP servers (`psionic-inkling-serve`) capture and replay
   cleanly.

## Not done yet

- **Fully resident run on a 24 GB+ card.** Not possible on this 16 GB machine.
  Both the planner tests and the single-graph `decode_graph[2]` path are wired;
  no 24 GB+ card has executed the resident path yet.
- **Native Windows execution.** Both `.exe` binaries cross-compile cleanly via
  MinGW with zero warnings; runtime verification requires a Windows NVIDIA host.
- **Tier 2 and the 27-task Session History suites.** They run through
  OpenAgents Desktop against the Linux/Windows local model host (Track B3/B4).
