# Executor W3.2 Verification Record: GPU Verified-Trace Distillation & Learned-Exactness Research Harness (`trace-distill-v1`, Falsifiable H1/H3/H5 Track)

**Date:** 2026-11-21  
**Issue:** [#13918](https://github.com/OpenAgentsInc/openagents/issues/13918) (Parent Epic [#13912](https://github.com/OpenAgentsInc/openagents/issues/13912))  
**Master Spec:** [psionic/docs/executor-network-master-spec.md](../../psionic/docs/executor-network-master-spec.md) (Section W3.2)  
**Machine (Live Metal Run):** Apple M5 Max (`Apple GPU family 10`, unified memory, `107.5 GiB` recommended working set, macOS 26 / Xcode 26.6 / Metal 4, `MTLMathModeSafe` / fast math OFF)  
**CUDA Hardware Status:** **Emitted (`coax cuda` and `coax cuda --release`) and thin C11 host built cleanly (`-Wall -Wextra -Werror`) against `psionic/platform/cuda-host.c`, verified clean `PC_ABSENT` refusal on macOS.** Live NVIDIA CUDA execution requires an NVIDIA GPU host (`libcuda.so.1` + NVRTC); no CUDA execution was faked or simulated.

---

## 1. Executive Summary & W5 Recommendation

We trained and evaluated a **real 10,715,136-parameter 6-layer Psionic causal transformer decoder** (plus a `73,728`-parameter 4-head auxiliary state probe on Variant B, `10,788,864` total active parameters) directly on the Apple M5 Max Metal 4 GPU across all three pre-registered experimental arms under an **identical training token budget of `1,638,400` tokens per arm (`4,915,200` GPU training tokens total)**, fed strictly by Ch 234 exact-replayed and Ch 247 exposure-ledger-admitted traces with zero split leakage.

Every final checkpoint was evaluated strictly by **standalone autoregressive KV-cached self-rollout replayed by Ch 234 (`psionic/lib/replay.coax`)** across **1×, 2×, 4×, and 8×** training trace lengths on **3 seen** (`countdown`, `prefix_sum`, `mem_accum`) and **3 unseen** (`stride_sub`, `squares_mem`, `tri_accum`) program families (`96` held-out programs total, `0` held-out leaks). Training cross-entropy and teacher-forced token accuracy are reported strictly as diagnostics and never counted as passes.

| Pre-Registered Arm | Hypothesis Tested | Active Params | Budget (tokens) | Live M5 Max Throughput | Train CE (start → end) | `[DIAGNOSTIC ONLY]` Train TF-Acc / 1× Train Ch 234 Rollout | Held-Out Ch 234 Pass@1 (`1× / 2× / 4× / 8×`) | Pre-Registered Verdict |
| --- | --- | ---: | ---: | ---: | :---: | :---: | :---: | :---: |
| **Baseline A** | **H1** (Standard next-token SFT on verified exact traces) | `10,715,136` | `1,638,400` | `11,801 tok/s` (`138.84 s`) | `5.0729 → 0.0947` | `97.4%` / `4/12 (33.3%)` | `0.0% / 0.0% / 0.0% / 0.0%` | **FALSIFIED** |
| **Variant B** | **H5** (Aux intermediate-state supervision + Ch 139 on-policy RL with binary Ch 234 reward) | `10,788,864` | `1,638,400` | `3,926 tok/s` (`417.29 s`) | `5.0729 → 0.1628` *(aux `4.1291 → 0.1751`)* | `96.1%` / `1/12 (8.3%)` | `0.0% / 0.0% / 0.0% / 0.0%` | **FALSIFIED** |
| **Variant C** | **H3** (Analytic `(2k, -k²)` parabolic-key attention init + max-margin lookup loss + `β` annealing `1 → 10`) | `10,715,136` | `1,638,400` | `12,826 tok/s` (`127.74 s`) | `4.9968 → 0.0852` *(margin `0.6798 → 0.4747`)* | `98.0%` / `2/12 (16.7%)` | `0.0% / 0.0% / 0.0% / 0.0%` | **FALSIFIED** |

> **Evidence-Backed Recommendation for W5 (Pylon Decentralized Training):** **`H2_ROUTING_ONLY`** (`tdp_w5_recommend(falsified, falsified, falsified) == TdW5Lane.h2_routing_only`).  
> Keep Pylon decentralized training focused strictly on **H2 (routing, invocation, and LoRA/interface adapters around frozen digest-pinned compiled exact cores)**. Do **not** open a pure neural student-backbone distillation lane in W5.

---

## 2. Deliverables & Authority / Verification Coverage

| File | Role | Verification / Mutation Status |
| --- | --- | --- |
| `psionic/lib/trace-distill-policy.coax` | `#authority` verified-trace generator (6 Ch 226 program families across `1×..8×`), lossless 128-token trace codec, Ch 136 loss mask, Variant B 4-head intermediate-state targets (`rx_fold`), Variant C write-to-read `src_pos` supervision, standalone Ch 234 `rx_check` rollout scorer, Ch 247 exposure-ledger admission (`training-rows.coax`), AdamW/LR/`β` schedules, Ch 139 group-relative RL advantages, checkpoint SHA-256 digest, and H1/H3/H5 verdict & W5 recommendation rules | `#authority`; `138/138` debug & release checks; **`755` mutants: `753` killed, `0` survived, `2` marked equivalent** |
| `psionic/lib/trace-distill-kernels.coax` | 24 single-source Coax GPU compute kernels (`tdk_mm_nn/nt/tn` 64×64×16 tiled FP32 GEMMs, `tdk_embed/_back`, `tdk_rms/_back`, `tdk_rope_qkv`, `tdk_relu2/_back`, `tdk_attn_scores`, `tdk_margin_loss`, `tdk_add_margin_ds`, `tdk_softmax/_back`, `tdk_attn_out`, `tdk_attn_dp`, `tdk_attn_dqkv`, `tdk_ce_weighted`, `tdk_aux_ce`, `tdk_kv_write_rope`, `tdk_decode_attn` up to `T=2048`, `tdk_sum`, `tdk_adamw`), zero atomics, `<= 12` args/kernel | `2/2` debug & release checks; `coax metal-compute --release` + `xcrun -sdk macosx metal -c` clean; `coax cuda` (debug & `--release`) clean |
| `psionic/lib/trace-distill.coax` | `10,715,136`-parameter decoder layout (`d=384, L=6, H=6, hd=64, d_ff=1536, V=128, T_train=320, T_eval_max=2048`), deterministic Box-Muller init + Variant C exact analytic `(2k, -k²)` parabolic-key attention init on Layers 0–1 Heads 0–1 (`argmax_{k'} q(k)·k(k') == k` for all 64 keys), W1.2 `wasm-exec-v1` parity bridge, Ch 234 + Ch 247 verified corpus builder, CPU reference forward/backward + central finite-difference gradcheck for all 3 arms, and `#export_c` bridge | `244/244` debug & release checks |
| `psionic/platform/trace-distill-gpu-host.c` | Thin C11 platform host compiling `-Wall -Wextra -Werror` cleanly for BOTH Metal 4 (`-DTD_GPU_METAL` + `metal-host.m`) and CUDA (`-DTD_GPU_CUDA` + `cuda-host.c`), zero CPU fallback (`PM_ABSENT` / `PC_ABSENT`), live GPU vs CPU parity, 10.72M exact save/resume parity, 3-arm training, and KV-cached autoregressive Ch 234 evaluation | Built `-Werror` clean for Metal 4 & CUDA; executed live on Apple M5 Max; verified `PC_ABSENT` on macOS |
| `psionic/tests/check_trace_distill.sh` | End-to-end W3.2 gate script (`Firm reason: 5`) executing debug/release tests, zero-survivor mutation check, Metal 4 & CUDA kernel emission/compilation, live M5 Max 3-arm run, and CUDA host check | Exit code `0` (`tools-check` & `wire-check` passed) |

---

## 3. Experimental Setup (Pre-Registered, Zero Held-Out Tuning)

### 3.1 Model Architecture & Parameter Count
- **Backbone:** 6-layer Psionic causal decoder (`d_model = 384`, `n_layers = 6`, `n_heads = 6`, `head_dim = 64`, `d_ff = 1536`, RMSNorm, ReLU² FFN, RoPE `θ = 10000`, vocab `V = 128`, `T_train = 320`, `T_eval_max = 2048`).
- **Exact Parameter Count:** `10,715,136` backbone parameters (`>= 10,000,000` verified at compile & runtime: `tok = 49,152`, `6 × 1,769,472 = 10,616,832` transformer block weights, `head = 49,152`). Variant B adds `W_aux[384, 192] = 73,728` parameters (`32` IP + `32` stack-depth + `64` top-of-stack mod 64 + `64` `loc0` mod 64), totaling `10,788,864` active parameters.
- **Variant C Analytic Parabolic-Key Initialization:** On Layers `0` and `1`, Heads `0` and `1`, RoPE is disabled (`td_para_heads(variant_c, l) == 2`) and the first two query/key lanes are zeroed and analytically initialized to the parabolic exact-lookup geometry `(2k, -k²)` over token value keys `k ∈ [0..63]`, verified by unit test to yield exact `argmax_{k'} q(k)·k(k') == k` for every key `k`.

### 3.2 Verified Corpus, W1.2 Parity & Ch 247 Exposure-Free Split
- **6 Distinct Program Families (Ch 226 / `wasm-exec-v1`):**
  - **Seen (training & held-out eval):** `countdown` (15 ops), `prefix_sum` (17 ops), `mem_accum` (20 ops).
  - **Unseen (held-out eval ONLY, never admitted to training):** `stride_sub` (11 ops), `squares_mem` (22 ops), `tri_accum` (23 ops).
- **W1.2 `wasm-exec-v1` Parity Bridge:** Loaded the compiled F64 safetensors core (`sha256: b1b6bbceedeb3c44...`) and executed all 6 families through `wx_run` (`hull-v2`) + `wx_replay_verify`: **`122` dense weight calls across 6 families, `6/6` EXACT**.
- **Length Buckets (`1×`, `2×`, `4×`, `8×`):**
  - `1×`: `35–42` execution steps (`<= 270` tokens, fits inside `T_train = 320`)
  - `2×`: `65–75` steps (`~450` tokens)
  - `4×`: `125–141` steps (`~860` tokens)
  - `8×`: `245–273` steps (`<= 1,702 < 2,048` tokens)
- **Ch 247 Exposure Ledger Receipt (zero train/eval leakage):**
  - **Training split:** `252` verified traces (`3` seen families × `3` training lengths `n ∈ {1, 2, 3}` × `28` inputs `p ∈ [0..27]`, `timestamp < 10,000`).
  - **Held-out eval split:** `96` verified traces (`6` families × `4` buckets `1×, 2×, 4×, 8×` × `4` disjoint inputs `p ∈ [28..31]`, `timestamp >= 10,000`).
  - **Leakage enforcement:** `groups = 27`, `leakage_probes_blocked = 4` (verified cross-artifact duplicate/unseen/length-leak refusals on the shared Ch 247 ledger).
  - **Exposure Ledger SHA-256:** `84db4d130d8260568f806e82a9926d05f37cedebb853bba125c785176be41b8b`.

### 3.3 Identical Training Budget & Hyperparameters
- **Total Budget per Arm:** `320` optimizer steps × `16` sequences/batch × `320` tokens/sequence = **`1,638,400` tokens/arm** (identical across Arms A, B, and C).
- **Optimizer (via `#authority` `tdp_decide`):** AdamW (`β₁ = 0.9`, `β₂ = 0.99`, `ε = 1e-8`, weight decay `0.01`, gradient clip `1.0`, linear warmup `32` steps, cosine decay `6e-4 → 6e-5` for SFT; `2e-4 → 2e-5` for Variant B RL phase).
- **Arm-Specific Objectives:**
  - **Baseline A (H1):** `320` SFT steps of Ch 136 assistant/trace-only masked next-token cross-entropy (`β = 1.0`).
  - **Variant B (H5):** `240` SFT steps with 4-head auxiliary intermediate-state CE (`aux_weight = 0.25`) + `80` Ch 139 on-policy group-relative RL steps (`4` train-split prompts/step × `G = 4` KV-cached temperature `0.7` GPU rollouts = `16` rollouts/step, `1,280` total rollouts; binary Ch 234 exact-replay reward `r ∈ {0, 1}`, KL penalty `β_KL = 0.02` against frozen step-240 SFT reference policy).
  - **Variant C (H3):** `320` SFT steps from exact analytic `(2k, -k²)` parabolic-key initialization + parabolic-head max-margin lookup loss (`margin = 2.0`, `margin_weight = 0.25`) over exact write-to-read `src_pos` supervision + linear sharpness annealing `β: 1.0 → 10.0`.
- **Pre-Registered Falsification Criteria (`tdp_eval_verdict`):**
  - `CONFIRMED`: `all_pass(4×) >= 0.50` AND `all_pass(8×) >= 0.25` AND `unseen_pass(1×) >= 0.25`.
  - `PARTIAL`: fails `CONFIRMED`, but `all_pass(2×) >= 0.25`.
  - `FALSIFIED`: `all_pass(2×) < 0.25`.

---

## 4. Live GPU Parity & Exact Save/Resume Verification Receipts

Compiled all 24 Coax GPU kernels on Apple M5 Max (`Metal 4`, fast math OFF) in **`0.383 s`**.

### 4.1 Live Metal 4 GPU vs. CPU Reference Gradient & KV-Decode Parity (`td_tiny_shape`)
| Arm | `max \|grad_gpu - grad_cpu\|` | `max \|kv_decode - full_forward\|` | GPU Time |
| --- | ---: | ---: | ---: |
| Baseline A (SFT) | `2.235e-08` | `0.000e+00` | `1.65 ms` |
| Variant B (Aux + RL) | `7.451e-09` | `0.000e+00` | `1.79 ms` |
| Variant C (Parabolic + Max-Margin + `β`) | `8.941e-08` | `0.000e+00` | `1.48 ms` |

### 4.2 Live 10,715,136-Parameter Exact Checkpoint Save/Resume Bit-Parity
Ran 4 live GPU AdamW steps uninterrupted vs. 2 steps → checkpoint save (params `p`, Adam moments `m`, `v`) → fresh GPU reload → 2 steps:
- **Uninterrupted step 4 SHA-256:** `24cd56642ab44abf5f74dc75874c3935f8f09f63583530616c9a3408576b5b7c`
- **Resumed step 4 SHA-256:** `24cd56642ab44abf5f74dc75874c3935f8f09f63583530616c9a3408576b5b7c`
- **Parameter bit differences across all `10,715,136` FP32 parameters:** **`0`**

---

## 5. Unedited Training Curves, Throughput & Checkpoint Digests (Apple M5 Max)

### 5.1 Baseline A — H1 (Standard Masked Next-Token SFT)
- **Active params:** `10,715,136` | **Steps:** `320` (`sft=320, rl=0`) | **Tokens:** `1,638,400`
- **Throughput:** **`11,801 tok/s`** (`138.84 s` wall / `138.13 s` GPU)
- **Final Checkpoint SHA-256:** `7c943c5c685c3142724d0e4cf7b0ee979ac6dc9d27367ce685881112aa6caa7d`
- **Diagnostics ONLY (never counted as passes):** train TF-acc = `97.4%`; train-split `1×` Ch 234 autoregressive rollout = `4/12 (33.3%)`

| Step | Train CE | Grad Norm | LR | `β` |
| ---: | ---: | ---: | ---: | ---: |
| `1 / 320` | `5.0729` | `33.156` | `0.00002` | `1.00` |
| `80 / 320` | `0.2277` | `0.356` | `0.00057` | `1.00` |
| `160 / 320` | `0.1882` | `0.315` | `0.00038` | `1.00` |
| `240 / 320` | `0.1378` | `0.242` | `0.00016` | `1.00` |
| `320 / 320` | `0.0947` | `0.276` | `0.00006` | `1.00` |

### 5.2 Variant B — H5 (Aux Intermediate-State Supervision + Ch 139 On-Policy RL)
- **Active params:** `10,788,864` (`backbone=10,715,136`, `aux_head=73,728`) | **Steps:** `320` (`sft=240, rl=80`) | **Tokens:** `1,638,400`
- **Throughput:** **`3,926 tok/s`** (`417.29 s` wall / `401.94 s` GPU; includes live GPU KV-cached autoregressive sampling + frozen-reference KL passes during RL)
- **Ch 139 RL Rollout Totals:** `6 / 1,280` (`0.005`) binary Ch 234 exact-replay rewards across `4` mixed-reward groups; cumulative mean KL = `0.3263`
- **Final Checkpoint SHA-256:** `f15b2e20e81d90897d3cc9fb2366852b1b852890fb5cc9156f7a52cdf8d25e14`
- **Diagnostics ONLY (never counted as passes):** train TF-acc = `96.1%`; train-split `1×` Ch 234 autoregressive rollout = `1/12 (8.3%)`

| Phase / Step | Train CE / RL Reward | Aux CE / Mean KL | Grad Norm | LR |
| --- | ---: | ---: | ---: | ---: |
| SFT `1 / 320` | `ce = 5.0729` | `aux = 4.1291` | `34.166` | `0.00002` |
| SFT `80 / 320` | `ce = 0.2321` | `aux = 0.3677` | `1.018` | `0.00057` |
| SFT `160 / 320` | `ce = 0.1984` | `aux = 0.1969` | `0.321` | `0.00038` |
| SFT `240 / 320` | `ce = 0.1628` | `aux = 0.1751` | `0.209` | `0.00016` |
| RL `241 / 320` | `reward = 0.000 (0/16)` | `mean_kl = 0.0000` | `0.0000` | — |
| RL `280 / 320` | `reward = 0.000 (0/16)` | `mean_kl = 0.3435` | `0.0143` | — |
| RL `320 / 320` | `reward = 0.000 (0/16)` | `mean_kl = 0.3263` | `0.0022` | — |

### 5.3 Variant C — H3 (Analytic `(2k, -k²)` Parabolic-Key Init + Max-Margin Loss + `β` Annealing)
- **Active params:** `10,715,136` | **Steps:** `320` (`sft=320, rl=0`) | **Tokens:** `1,638,400`
- **Throughput:** **`12,826 tok/s`** (`127.74 s` wall / `127.52 s` GPU)
- **Final Checkpoint SHA-256:** `2c5a64a4ef36771c50ca543304881707b6c5c4ccbfdc83f9ed6f1561d8b4455e`
- **Diagnostics ONLY (never counted as passes):** train TF-acc = `98.0%` *(highest of all three arms)*; train-split `1×` Ch 234 autoregressive rollout = `2/12 (16.7%)`

| Step | Train CE | Margin Loss | Grad Norm | LR | `β` |
| ---: | ---: | ---: | ---: | ---: | ---: |
| `1 / 320` | `4.9968` | `0.6798` | `29.571` | `0.00002` | `1.00` |
| `80 / 320` | `0.2509` | `0.4963` | `5.257` | `0.00057` | `3.22` |
| `160 / 320` | `0.1610` | `0.4977` | `3.866` | `0.00038` | `5.47` |
| `240 / 320` | `0.1093` | `0.4820` | `17.889` | `0.00016` | `7.72` |
| `320 / 320` | `0.0852` | `0.4747` | `6.560` | `0.00006` | `9.97` |

---

## 6. Unedited Standalone Autoregressive Ch 234 Exact-Replay Results (`1×`, `2×`, `4×`, `8×`)

Every entry below reflects live Metal 4 GPU greedy KV-cached self-rollout from the program prompt alone, decoded and verified strictly by `psionic/lib/replay.coax` (`tdp_score_rollout`).

### 6.1 Baseline A (H1: Standard Next-Token SFT)
*Eval compute: `17,059` generated tokens in `8.31 s` wall (`6.55 s` GPU).*

| Family | Split | Len | Exact Pass@1 (Ch 234) | Output-Digest Match | Syntax Errors | First Divergent Step (min / med / max) |
| --- | :---: | :---: | :---: | :---: | :---: | :---: |
| `countdown` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `countdown` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `prefix_sum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `4 / 4 / 4` |
| `mem_accum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `3 / 4 / 4` |
| `squares_mem` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `tri_accum` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |

- **Summary:** Seen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Unseen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Output-Digest `0.0%` across all buckets → **H1 Verdict: `FALSIFIED`**.

### 6.2 Variant B (H5: Aux Intermediate-State Supervision + Ch 139 RL)
*Eval compute: `18,971` generated tokens in `24.79 s` wall (`21.17 s` GPU).*

| Family | Split | Len | Exact Pass@1 (Ch 234) | Output-Digest Match | Syntax Errors | First Divergent Step (min / med / max) |
| --- | :---: | :---: | :---: | :---: | :---: | :---: |
| `countdown` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `countdown` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `prefix_sum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `4 / 4 / 4` |
| `mem_accum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `1/4` | `1 / 1 / 1` |
| `tri_accum` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |

- **Summary:** Seen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Unseen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Output-Digest `0.0%` across all buckets → **H5 Verdict: `FALSIFIED`**.

### 6.3 Variant C (H3: Parabolic Init + Max-Margin Loss + `β` Annealing)
*Eval compute: `16,525` generated tokens in `20.46 s` wall (`16.93 s` GPU).*

| Family | Split | Len | Exact Pass@1 (Ch 234) | Output-Digest Match | Syntax Errors | First Divergent Step (min / med / max) |
| --- | :---: | :---: | :---: | :---: | :---: | :---: |
| `countdown` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `countdown` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `countdown` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `2 / 2 / 2` |
| `prefix_sum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `prefix_sum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `4 / 4 / 4` |
| `mem_accum` | seen | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `mem_accum` | seen | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `stride_sub` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `4/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `3 / 3 / 3` |
| `squares_mem` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `squares_mem` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `1×` | `0/4 (0.0%)` | `0/4` | `0/4` | `1 / 1 / 1` |
| `tri_accum` | UNSEEN | `2×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `4×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |
| `tri_accum` | UNSEEN | `8×` | `0/4 (0.0%)` | `0/4` | `0/4` | `0 / 0 / 0` |

- **Summary:** Seen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Unseen pass@1 `0.0% / 0.0% / 0.0% / 0.0%`, Output-Digest `0.0%` across all buckets → **H3 Verdict: `FALSIFIED`**.

---

## 7. Scientific Analysis & Decision for W5

1. **Optimization converged cleanly — this is NOT a training bug.** All three 10.72M-parameter arms converged sharply from CE `~5.0` to `0.085–0.163`, reaching **`96.1%–98.0%` teacher-forced accuracy** on the verified corpus and achieving **`8.3%–33.3%` full exact Ch 234 autoregressive pass@1 on training-split programs** (`4/12` on Baseline A, `2/12` on Variant C, `1/12` on Variant B). GPU gradients match the CPU finite-difference reference to `< 9e-8`, and save/resume matches to `0` bit differences across all `10,715,136` parameters.
2. **Why H1, H3, and H5 all failed out-of-distribution exactness:**
   - **Disjoint unseen-value generalization fails immediately at step 2–4 (`1×`) and step 0 (`2×/4×/8×`):** Even on *seen* program families (`countdown`, `prefix_sum`, `mem_accum`), switching from training inputs `p ∈ [0..27]` to held-out inputs `p ∈ [28..31]` causes divergence on step 2–4 as soon as the model must push/add an unseen literal (`p` or `n`). At `2×`, `4×`, and `8×`, the initial loop counter `n` itself sits outside the training range `{1, 2, 3}`, triggering divergence on **step 0** (`first_div_step = 0/0/0`) across every family in every arm.
   - **Teacher-forced accuracy is a mirage for exact execution:** `98.0%` per-token accuracy (Variant C) still compounds exponentially across `40`–`270` autoregressive steps (`0.98^240 = 0.008`, and deterministic arithmetic errors compound to `0%` once the first carry/literal misses).
   - **Ch 139 RL (Variant B / H5) faces extreme sparse-reward collapse:** Under `0.7` temperature sampling after 240 SFT+aux steps, only `6 / 1,280` rollouts (`0.5%`, `4` mixed groups) passed Ch 234 exact replay; spending 25% of the fixed token budget on sparse-reward RL degraded both `1×` train exactness (`33.3% → 8.3%`) and held-out step survival (`squares_mem 1×`: `3–4 → 0`) while cutting wall throughput by `3×`.
   - **Parabolic init + max-margin + `β` annealing (Variant C / H3) cannot prevent gradient drift:** Although Variant C achieved the lowest training CE (`0.0852`) and highest teacher-forced accuracy (`98.0%`), dense AdamW updates + RMSNorm + softmax drift across all 6 layers still fail to preserve exact algorithmic lookup or arithmetic composition on unseen operands and unseen lengths.
3. **Binding W5 Architecture Decision (`H2_ROUTING_ONLY`):**
   - **Do NOT open a decentralized pure-neural student-backbone distillation lane in W5.** Gradient-trained neural backbones cannot match compiled exact executors on length extrapolation (`2×/4×/8×`) or unseen program families.
   - **Focus Pylon decentralized training strictly on H2:** training lightweight routing, tool-invocation, and interface LoRA adapters that learn *when and how to invoke* frozen, digest-pinned compiled exact cores (`wasm-exec-v1` / `exec-gpu-v1`) verified by Ch 234.

---

## 8. Exact Reproduction Commands

Reproduce all debug & release Coax checks, zero-survivor mutation checking on `#authority`, Metal 4 and CUDA kernel emission/compilation, live GPU parity, exact 10.72M save/resume verification, and the full 320-step (`1,638,400` tokens/arm) 3-arm Apple M5 Max Metal 4 experiment:

```sh
./psionic/tests/check_trace_distill.sh 320
```

Run standalone policy/model checks and mutation verification:

```sh
coax/bin/coax test psionic/lib/trace-distill-policy.coax --release
coax/bin/coax test psionic/lib/trace-distill-kernels.coax --release
coax/bin/coax test psionic/lib/trace-distill.coax --release
coax/bin/coax mutate psionic/lib/trace-distill-policy.coax --root-only --native --jobs 8
coax/web/tool.sh tools-check
coax/web/tool.sh wire-check
```

On an NVIDIA CUDA host (SM 6.0+ with `libcuda.so.1` + NVRTC), `./psionic/tests/check_trace_distill.sh 320` automatically compiles `trace-distill-kernels.cu` via NVRTC and runs the live CUDA `trace-distill-gpu-cuda` binary with the exact same Coax kernels and Ch 234/247 evaluation harness.
