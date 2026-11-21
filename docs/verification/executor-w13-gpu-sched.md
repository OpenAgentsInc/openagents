# Executor W1.3 Verification Record: Metal & CUDA Exact Execution Kernels (`exec-gpu-v1`) + Optimal Liveness & Slot-Reuse Scheduler

**Date:** 2026-11-21  
**Issue:** [#13916](https://github.com/OpenAgentsInc/openagents/issues/13916) (Parent Epic [#13912](https://github.com/OpenAgentsInc/openagents/issues/13912))  
**Master Spec:** [psionic/docs/executor-network-master-spec.md](../../psionic/docs/executor-network-master-spec.md)  
**Machine (Live Metal Run):** Apple M5 Max (`Apple GPU family 10`, unified memory, `107.5 GiB` recommended working set, macOS 26 / Xcode 26.6 / Metal 4)  
**Machine (Live CUDA Run):** `coderos-4080` — `NVIDIA GeForce RTX 4080` (`SM 8.9`, `76 SMs`, `15.2 GiB free / 15.6 GiB total`, NVIDIA driver `595.71.05`, CUDA/NVRTC `13.2.51`, NixOS x86_64)  
**Status:** **Both live Apple Silicon Metal 4 and live NVIDIA CUDA hardware verification COMPLETE (`54/54` Ch 226 programs bit-exact `0` divergences, `9/9` device-level Ch 233 precision sweeps EXACT, zero-survivor `#authority` mutation gates on both macOS ARM64 and Linux x86_64).**

---

## 1. Deliverables & Architecture

| File | Role | Authority / Mutants |
| --- | --- | --- |
| `psionic/lib/exec-vm-sched.coax` | Optimal liveness + level-coalesced Attention wave packer + exact branch-and-bound / coordinate-descent layer packer + pin-coalesced interval coloring + reuse-driven clearing + formal schedule verifier (`evs_verify_schedule`) for `exec-vm-v1` and Ch 229 `gate-graph.coax` | `#authority` (`707` mutants: `682` killed, `0` survived, `25` marked equivalent) |
| `psionic/lib/exec-vm-opt-compile.coax` | Dense-weight compiler drivers (`evs_compile_ev`, `evs_compile_ev_planted`, `evs_compile_gg`, `evs_compile_gg_planted`, `evs_compile_ch226`) emitting exact `binary64` weights & safetensors from `#authority` `evs_schedule_ev` schedules | Tested (`98/98` debug & release; 54/54 Ch 226 + 100 Ch 229 graphs + 100% planted-bug detection) |
| `psionic/lib/exec-vm-kernels.coax` | Single-source Coax compute kernels admitted unchanged by `coax test`, `coax c`, `coax metal-compute --release` (fast math OFF), and `coax cuda --release` (`--fmad=false`): per-layer dense `i64` & `f64` FFN/Attention/readout, persistent batched multi-step rollout (`i64` & `f64`), arithmetic envelope probe, and parallel device-level Ch 233 softmax sweep (`f64`, `f32`, `bf16`) | Pure compute subset (CPU + Metal 4 + CUDA verified) |
| `psionic/lib/exec-vm-gpu.coax` | Device capability admission (Ch 37/68), exact arithmetic-envelope preflight (`metal_i64`, `metal_ff`, `cuda_f64`, `cuda_i64`), device memory planning, typed pre-launch refusals (non-integer, OOM, operand envelope), and Ch 226 SHA-256 trace digest | `#authority` (`182` mutants: `181` killed, `0` survived, `1` marked equivalent) |
| `psionic/platform/exec-vm-gpu-bridge.coax` | Thin Coax C-export bridge exposing the 54-program suite, prompt encoders, and Ch 233 rounding so the C host contains zero program/crypto/arithmetic logic | Host bridge |
| `psionic/platform/exec-vm-gpu-host.c` | Thin C11 platform host compiling cleanly (`-Wall -Wextra -Werror`) for BOTH Metal (`-DEXEC_GPU_METAL` + `metal-host.m`) and CUDA (`-DEXEC_GPU_CUDA` + `cuda-host.c`), with zero CPU fallback (`PM_ABSENT` / `PC_ABSENT`) | Platform effects only |
| `psionic/examples/exec-vm-gpu.coax` | Runnable Coax example comparing greedy vs. optimal schedules, 54/54 Ch 226 parity, 100% planted-bug detection, and CPU throughput | Runnable example |
| `psionic/tests/check_exec_vm_gpu.sh` | End-to-end W1.3 verification gate (debug/release tests, zero-survivor mutation checks, Metal 4 & CUDA emission, live Metal M5 Max run, CUDA host build & check) | Verification script (`Firm reason: 5`) |

---

## 2. Optimal Liveness & Slot-Reuse Scheduler: Before vs. After

### 2.1 Full Ch 226 Stack+Memory VM (`exec-vm.coax`)

| Metric | W1.1 Greedy Baseline (`ev_ch226_compile`) | W1.3 Optimal Scheduler (`evs_compile_ch226`) | Delta |
| --- | ---: | ---: | ---: |
| Residual width (`D`) | `49` | **`37`** (exact theoretical floor `1 + 36 kept`) | **-24.5%** (`-12` slots) |
| Transformer layers | `16` | **`15`** (Attention Wave 2 heads merged) | **-1 layer** |
| Attention heads | `7` | `8` (`cur_outputs` deferred to Wave 2) | `+1` head |
| Total FFN units | `259` | **`252`** (`56` reuse clears, `18` writeback) | **-7 units** |
| Writeback layer units | `25` | **`18`** (11 pinned nodes coalesced + clean-zero elision + 4 unpinned KV clears) | **-28.0%** |
| Total dense F64 weights | `41,013` | **`30,192`** | **-26.4%** (`-10,821` weights) |
| Safetensors artifact size | `333,264` B | **`246,800` B** (`sha256: 50c9e06a11a79e6b...`) | **-25.9%** |
| CPU dense-weight speed (same load) | `28,809` – `51,340` steps/s | **`41,028` – `73,408` steps/s** | **1.42× – 1.43× faster** |
| Divergences across 54 programs (3,692 steps) | `0` | **`0`** (bit-exact SHA-256) | `0` |

### 2.2 Ch 229 / Ch 230 Straight-Line Gate Graphs (100 Generated Graphs, Seeds 1–5)

| Metric | Ch 229 Greedy Baseline (`gg_compile`) | W1.3 Optimal B&B (`evs_compile_gg`) | Delta |
| --- | ---: | ---: | ---: |
| Aggregate residual width (100 graphs) | `618` | **`513`** (strictly narrower on `64/100`) | **-17.0%** |
| Aggregate dense weight count | `35,022` | **`29,040`** (strictly lighter on `73/100`) | **-17.1%** |
| Differential harness divergences | `0` | **`0`** | `0` |
| Planted-bug detection (10 seeds) | `4/4` Ch 229, `7/7` Attention+FFN | **`4/4` Ch 229 (10/10 seeds), `7/7` Attention+FFN (10/10 seeds)** | **100%** |

---

## 3. Live GPU Exactness & Arithmetic Envelope Verification (Apple M5 Max Metal 4 & NVIDIA RTX 4080 CUDA)

- **Apple M5 Max Metal 4:** Compiled with `coax metal-compute psionic/lib/exec-vm-kernels.coax --release` and loaded with **`MTLMathModeSafe` / `MTLMathFloatingPointFunctionsPrecise` (fast math OFF)** in `0.621 s` on `Apple M5 Max`.
- **NVIDIA GeForce RTX 4080 CUDA (`coderos-4080`):** Compiled with `coax cuda psionic/lib/exec-vm-kernels.coax --release` and JIT-compiled via NVRTC 13.2 (`--gpu-architecture=compute_89 --fmad=false --prec-div=true --prec-sqrt=true --ftz=false --std=c++17`) in `2.044 s` on `NVIDIA GeForce RTX 4080 (SM 8.9, 76 SMs, 15.2 GiB free / 15.6 GiB total)`.

### 3.1 Live GPU Exact Arithmetic Envelope Probe (`0.020 ms` M5 Max Metal 4 | `0.174 ms` RTX 4080 CUDA)

| Product | Exact Integer Result | `f32` (Metal & CUDA) | Metal `f32×2` (`coax_ff`) | CUDA native `f64` (`--fmad=false`) | `i64` (Metal & CUDA) |
| :--- | ---: | :---: | :---: | :---: | :---: |
| `4096 × 4096` (`2^24`) | `16,777,216` | EXACT | EXACT | EXACT | EXACT |
| `4097 × 4097` | `16,785,409` | **FAIL** | EXACT | **EXACT** | **EXACT** |
| `16,777,216 × 16,777,216` (`2^48`) | `281,474,976,710,656` | EXACT | EXACT | **EXACT** | **EXACT** |
| `16,777,217 × 16,777,217` (`> 2^48`) | `281,475,010,265,089` | **FAIL** | **FAIL** *(refused pre-launch by `metal_ff`)* | **EXACT** | **EXACT** |
| `67,108,863 × 67,108,863` (`(2^26-1)^2 < 2^53`) | `4,503,599,493,152,769` | **FAIL** | **FAIL** *(refused pre-launch by `metal_ff`)* | **EXACT** | **EXACT** |
| `67,108,864 × 67,108,864` (`2^52`) | `4,503,599,627,370,496` | EXACT | EXACT | **EXACT** | **EXACT** |

Preflight admission (`evg_admit_model`, `evg_admit_program`) verified typed pre-launch refusals (`non_integer`, `memory`, `envelope`) with zero GPU launches on invalid inputs on both hosts (`required 8.93 MiB`).

### 3.2 Full 54-Program Ch 226 Suite (3,692 Autoregressive Steps) on Live Metal 4 & CUDA GPUs

Every execution loaded the SHA-256-verified optimal `30,192`-weight F64 safetensors artifact (`sha256: 50c9e06a11a79e6b...`) and matched the CPU `tm_run` reference SHA-256 trace digest bit-for-bit across all 54 programs (4 named: `squares(4)`, `sum(10)`, `fact(10)`, `memfill(5,8)` + 50 seeded programs):

| Live GPU Execution Path | Device | Programs Verified | Total Steps | Divergences | Wall Time | GPU Time |
| --- | --- | :---: | ---: | :---: | ---: | ---: |
| Per-layer dense `i64` (`B=54`, 1 batch/step) | Apple M5 Max (Metal 4) | **54 / 54** | `3,692` | **0** | `924.12 ms` | `884.04 ms` |
| Persistent batched dense rollout `i64` (`B=54`) | Apple M5 Max (Metal 4) | **54 / 54** | `3,692` | **0** | `1,207.49 ms` | `1,198.35 ms` |
| Persistent batched dense rollout `f32×2` (`B=54`) | Apple M5 Max (Metal 4) | **54 / 54** | `3,692` | **0** | `1,846.17 ms` | `1,845.26 ms` |
| Per-layer dense `i64` (`B=54`, 1 batch/step) | NVIDIA RTX 4080 (CUDA) | **54 / 54** | `3,692` | **0** | `652.94 ms` | `645.73 ms` |
| Persistent batched dense rollout `i64` (`B=54`) | NVIDIA RTX 4080 (CUDA) | **54 / 54** | `3,692` | **0** | `777.69 ms` | `777.68 ms` |
| Persistent batched dense rollout native `f64` (`B=54`) | NVIDIA RTX 4080 (CUDA) | **54 / 54** | `3,692` | **0** | `1,126.08 ms` | `1,126.06 ms` |

---

## 4. Honest Four-Way Throughput & Latency Table

> **Standing honesty rule:** A bare CPU interpreter is faster for standalone serial loops (`~56×`–`80×` faster than single-stream CPU compiled weights). Compiled weights provide zero-cost replay verification, composable weight-space modules, and batched exact GPU execution native to neural systems.

Measured directly during `./psionic/tests/check_exec_vm_gpu.sh` gate runs on Apple M5 Max (macOS ARM64 / Metal 4) and `coderos-4080` (x86_64 Linux / NVIDIA GeForce RTX 4080 CUDA):

| Execution Path | Steps | Throughput (steps/s) | Latency (µs/step) | GPU Time (ms) | Status |
| --- | ---: | ---: | ---: | ---: | --- |
| Native CPU interpreter (`tm_run`, Apple M5 Max) | `166,800` | `2,469,574` *(idle: `3,541,326`)* | `0.40` | — | Verified (oracle) |
| Native CPU interpreter (`tm_run`, `coderos-4080` CPU) | `166,800` | `1,162,538` *(example: `2,032,634`)* | `0.86` | — | Verified (oracle) |
| CPU compiled dense weights — W1.1 greedy (`41,013` w, M5 Max) | `4,170` | `30,278` *(idle: `44,519–51,340`)* | `33.03` | — | Verified |
| CPU compiled dense weights — W1.3 optimal (`30,192` w, M5 Max) | `4,170` | `42,277` *(idle: `62,452–73,408`)* | `23.65` | — | Verified (`1.40×` vs greedy) |
| CPU compiled dense weights — W1.1 greedy (`41,013` w, `coderos-4080`) | `4,170` | `15,759` *(example: `33,527`)* | `63.46` | — | Verified |
| CPU compiled dense weights — W1.3 optimal (`30,192` w, `coderos-4080`) | `4,170` | `20,325` *(example: `42,652`)* | `49.20` | — | Verified (`1.29×` vs greedy) |
| Metal M5 Max per-layer dense `i64` (`B=1`, 1 batch/step) | `417` | `356` | `2,808.50` | `1,048.71` | Verified live on M5 Max |
| Metal M5 Max per-layer dense `i64` (`B=54` batched) | `3,692` | `3,995` | `250.30` | `884.04` | Verified live on M5 Max |
| Metal M5 Max persistent rollout `i64` (`B=54`) | `3,692` | `3,058` | `327.06` | `1,198.35` | Verified live on M5 Max |
| Metal M5 Max persistent rollout `i64` (`B=216`) | `14,768` | `12,284` | `81.41` | `1,197.78` | Verified live on M5 Max |
| Metal M5 Max persistent rollout `f32×2` (`B=216`) | `14,768` | `7,913` | `126.37` | `1,865.33` | Verified live on M5 Max |
| Metal M5 Max persistent rollout `i64` (`B=1080`) | `73,840` | **`56,715`** | **`17.63`** | `1,296.30` | Verified live on M5 Max |
| Metal M5 Max persistent rollout `f32×2` (`B=1080`) | `73,840` | `37,826` | `26.44` | `1,947.48` | Verified live on M5 Max |
| CUDA RTX 4080 per-layer dense `i64` (`B=1`, 1 batch/step) | `417` | `487` | `2,055.31` | `842.51` | Verified live on RTX 4080 |
| CUDA RTX 4080 per-layer dense `i64` (`B=54` batched) | `3,692` | `5,654` | `176.85` | `645.73` | Verified live on RTX 4080 |
| CUDA RTX 4080 persistent rollout `i64` (`B=54`) | `3,692` | `4,747` | `210.64` | `777.68` | Verified live on RTX 4080 |
| CUDA RTX 4080 persistent rollout `i64` (`B=216`) | `14,768` | `18,406` | `54.33` | `802.34` | Verified live on RTX 4080 |
| CUDA RTX 4080 persistent rollout native `f64` (`B=216`) | `14,768` | `12,779` | `78.25` | `1,155.60` | Verified live on RTX 4080 |
| CUDA RTX 4080 persistent rollout `i64` (`B=1080`) | `73,840` | **`91,948`** | **`10.88`** | `803.04` | Verified live on RTX 4080 (`1.62×` vs M5 Max) |
| CUDA RTX 4080 persistent rollout native `f64` (`B=1080`) | `73,840` | **`63,811`** | **`15.67`** | `1,157.14` | Verified live on RTX 4080 (`1.69×` vs M5 Max `f32×2`) |

---

## 5. Device-Level Ch 233 Softmax vs. Hard-Max Precision-Cliff Sweep (Live on Metal 4 & CUDA GPUs)

Executed doubling trace lengths `N = 1, 2, 4, ..., 65,536` directly on both Apple M5 Max (Metal 4) and NVIDIA GeForce RTX 4080 (CUDA) via Coax-emitted parallel reduction kernels (`evk_soft_top_logit → evk_soft_z_chunk → evk_soft_reduce_f64 → evk_soft_y_chunk → evk_soft_reduce_f64`), verified against CPU `soft-memory.coax` (`sm_limit`, analytic bound `ln(4000) = 8.294050`):

| Sweep Case | Same-Key (`until / fail`) | Near-Key (`until / fail`) | CPU Ch 233 Oracle | M5 Max Metal 4 GPU Time | RTX 4080 CUDA GPU Time |
| :--- | :---: | :---: | :---: | ---: | ---: |
| `f64` (CUDA native / Metal `f32×2`), `k=16`, `V=1000`, `β = ln(4V) + 1` | `65536 / 0` | `65536 / 0` | **EXACT** | `4.237 ms` | `3.219 ms` |
| `f32`, `k=16`, `V=1000`, `β = ln(4V) + 1` | `8192 / 16384` | `65536 / 0` | **EXACT** | `3.493 ms` | `2.797 ms` |
| `bf16`, `k=16`, `V=1000`, `β = ln(4V) + 1` | `1 / 2` | `1 / 2` | **EXACT** | `0.225 ms` | `0.052 ms` |
| `f64` (CUDA native / Metal `f32×2`), `k=256`, `V=1000`, `β = ln(4V) + 1` | `65536 / 0` | `65536 / 0` | **EXACT** | `3.983 ms` | `3.196 ms` |
| `f32`, `k=256`, `V=1000`, `β = ln(4V) + 1` | `32 / 64` | `256 / 512` | **EXACT** | `0.988 ms` | `0.407 ms` |
| `bf16`, `k=256`, `V=1000`, `β = ln(4V) + 1` | `1 / 2` | `1 / 2` | **EXACT** | `0.138 ms` | `0.048 ms` |
| `f64` (CUDA native / Metal `f32×2`), `k=16`, `V=1000`, `β = ln(4V) + 0.01` | `65536 / 0` | `65536 / 0` | **EXACT** | `4.045 ms` | `3.193 ms` |
| `f64` (CUDA native / Metal `f32×2`), `k=16`, `V=1000`, `β = ln(4V) - 1` | `1 / 2` | `65536 / 0` | **EXACT** | `2.043 ms` | `1.624 ms` |
| `f64` (CUDA native / Metal `f32×2`), `k=16`, `V=1`, `β = 0.40` | `1 / 2` | `1 / 2` | **EXACT** | `0.203 ms` | `0.051 ms` |

9/9 device-level GPU cases matched the CPU Ch 233 oracle identically on both Apple M5 Max Metal 4 and NVIDIA GeForce RTX 4080 CUDA across both `same` and `near` adversarial streams.

---

## 6. Reproducing

To reproduce all checks on macOS (Apple Silicon Metal 4) or Linux with an NVIDIA CUDA GPU (SM 6.0+ with NVRTC):

```sh
./psionic/tests/check_exec_vm_gpu.sh
```

On macOS, `check_exec_vm_gpu.sh` runs all unit and `#authority` mutation gates, compiles and executes Metal 4 (`MTLMathModeSafe`), builds the CUDA host `-Wall -Wextra -Werror`, and confirms clean `PC_ABSENT` refusal. On an NVIDIA CUDA host (`coderos-4080`), it runs all unit and `#authority` mutation gates, compiles `exec-vm-kernels.cu` via NVRTC (`--fmad=false`), executes all 54 Ch 226 programs (`i64` and native IEEE `binary64`), executes the live GPU precision probe and Ch 233 `N=65,536` sweep, and prints the complete NVIDIA GPU throughput table.
