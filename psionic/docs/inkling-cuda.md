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
  router runs, the host reads the chosen ids, uploads the experts not in the
  cache over the least recently used slots, and gives the kernels slot numbers
  in place of expert ids. The arithmetic is the same either way: an 8 GB plan
  and a 16 GB plan produce bit-identical logits.
- A prefill batch is at most `slots / 8` tokens (and at most 32), so a batch's
  experts are always in the cache together.

The plan is made before any large allocation: `resident`, `streaming`, or
`refused` when even 16 slots a layer do not fit. A refusal names the VRAM
needed and the longest context that fits.

Environment switches for testing: `INK_VRAM_MB` plans as if that much VRAM were
free, `INK_EXPERT_SLOTS` caps every layer's cache, `INK_BATCH` caps the prefill
batch, `INK_PROFILE=1` prints time per kernel, and `INK_ORACLE_Q8=1` rounds
matvec inputs the way ggml does (a parity diagnostic, see below).

## Measured on an RTX 4080 (16 GB), Linux, driver 595.71, 2026-11-21

Checkpoint `Inkling-2-Mini-IQ4_NL.gguf`, SHA-256
`cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`. The card
cannot hold the model, so every Psionic number below is with experts streaming
from host memory (114 of 128 experts a layer cached at context 4,096; 106 at
32,768). The comparison is llama.cpp b12626 (CUDA 13.5 build) with 22 of 25
layers on the GPU, the most that fit.

| Check | Result |
| --- | --- |
| `--test-quant-kernels` | `nvfp4_max_abs=0.00e+00 mxfp8_max_abs=0.00e+00 iq4_max_abs=0.00e+00 ok=true` |
| `--verify-prefill-decode` (batched against one token at a time) | `max_abs=0.00000 cos=1.000000 ok=true` |
| Decode, short context | 30.6 tok/s (llama.cpp: 8.6) |
| Decode after a 1,356-token prompt | 26.5 tok/s (llama.cpp: 7.2) |
| Prefill, 1,356-token chat prompt | 56.7 tok/s (llama.cpp: 21.7 to 22.9, see the note) |
| Plan as an 8 GB card (51 of 128 experts cached) | prefill 29.5 tok/s, decode 19.7 tok/s, logits bit-identical to the 16 GB plan |
| Plan with 3,000 MB free | refused before allocation: `insufficient VRAM: 3000 MB free ... needs 3868 MB with the smallest expert cache (17050 MB with every expert in VRAM)` |
| `psionic-inkling-serve --ctx 32768` | loads, plans 15,441 MB, answers `/v1/chat/completions` (`391`, 29 prompt tokens, as on Metal) and `/v1/responses` |
| Driver hidden | `pc_open` returns `PC_ABSENT`; the CLI exits with `no NVIDIA GPU: no CUDA driver library (libcuda.so.1)` |
| MinGW cross-compile | `psionic-inkling-serve.exe` and `psionic-inkling-cli.exe` link. Not run: there is no Windows host here. |

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

ggml multiplies a quantized matrix by activations it has first rounded to 8
bits; this engine keeps activations in f32. With `INK_ORACLE_Q8=1` the engine
rounds them the same way, and its logits are then as close to each llama.cpp
backend as the two backends are to each other. llama.cpp rejects K=1 for this
checkpoint (`invalid loop core`), so there is no reference for K=1.

## Not done yet

- **Fully resident run on a 24 GB card.** Not possible on this machine. The
  planner tests cover it; no 24 GB card has run it.
- **The 50 tok/s decode and 500 tok/s prefill targets.** Not met: 30.6 and 56.7
  on a card that streams experts. The kernels do about 280 G multiply-adds a
  second. Known next steps, in order of measured value:
  1. `coax cuda` checks every index. Removing the checks by hand (an experiment,
     not shippable) took decode from 31.6 to 40 tok/s. The compiler's loop
     versioning, which proves a loop's indexes in range once, is switched off
     for device output (`versioned_for` in `coax/src/emit_c.cpp`).
  2. Batched prefill decodes each weight block once per token. A tensor-core
     GEMM (`cu_mma_bf16_16x8x16`) that decodes a block once per batch is the
     path to hundreds of tokens a second.
  3. CUDA Graphs (`pc_capture_begin` / `pc_replay`) for the roughly 1,900
     launches a token.
- **Windows.** The code cross-compiles. Nothing has run on Windows.
- **Tier 2 and the 27-task Session History suites.** They run through
  OpenAgents Desktop, which needs the Linux host work of Track B3 and B4.
