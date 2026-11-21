# Executor W3.1 (`hybrid-exec-v1`) Verification Record

**Date:** 2026-11-21  
**GitHub Issue:** #13920 (Parent Epic: #13912)  
**Hardware:** Apple M5 Max, 128 GB unified memory, macOS 26 / Xcode 26.6, Metal 4  
**Base Checkpoint:** `/Users/christopherdavid/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (`16.38 GB`, `30B-A3B` MoE, SHA-256 `4c9956282a91e839030fb5b6f37745f7e9f16cf23d1351e93680e7880789d47c`)  
**Compiled Weight Modules:** `/Users/christopherdavid/.openagents/modules/` (4/4 Class-E W2.2 dense F64 safetensors: `oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`)  
**Reproduction Command:** `./psionic/tests/check_hybrid_exec.sh`

---

## 1. Unit Tests & Zero-Survivor `#authority` Mutation Gates

| File | Coax Tests (Debug & Release) | `coax mutate --root-only --native` |
| :--- | :--- | :--- |
| `psionic/lib/hybrid-exec-policy.coax` (`#authority`) | 5/5 passed (debug & release) | **44 mutants: 44 killed, 0 survived** |
| `psionic/lib/hybrid-lora.coax` (`#authority`) | 9/9 passed (debug & release) | **305 mutants: 303 killed, 0 survived, 2 equivalent** |
| `psionic/lib/hybrid-exec.coax` (`#authority`) | 176/176 passed (debug & release) | **650 mutants: 646 killed, 0 survived, 4 equivalent** |
| **Total W3.1 `#authority`** | **190/190 passed (both modes)** | **999 mutants: 993 killed, 0 survived, 6 equivalent** |

---

## 2. Constrained Grammar, KV-Cache Preservation & LoRA Hot-Swap

- **Ch 53 Constrained `<exec_call>` Grammar (`psionic/examples/hybrid-exec.coax --release`):**
  - DFA compiled: `197` states, `1,521` edges; `valid_accept=1`, `bad_syntax_reject=1`.
- **Live Metal 4 KV-Cache Prefix Preservation (`hybrid-exec-metal` on `Inkling-2-Mini-IQ4_NL.gguf`):**
  - Prompt tokens prefilled once: `296` → paused after `</exec_call>` at `pos=322`.
  - Frozen Class-E dense F64 core executed in `31 us`, Ch 234 replay `1/1`.
  - Injected `16` `<exec_return>` tokens with **`322` prefix tokens reused and `0` re-prefilled** (**`95.3%`** prefill work saved).
- **Pluggable Rank-4 LoRA Adapter Pinning & Hot-Swap (`d_in=2048, d_out=4`):**
  - Zero adapter parity vs. base logits: `zero_max_diff = 0.000000e+00`.
  - Mismatched `base_model_sha256` hot-swap: **refused atomically** (`base_sha_mismatch_refused=1`).
  - Mismatched `core_module_sha256` hot-swap: **refused atomically** (`core_sha_mismatch_refused=1`).
  - Held-out routing accuracy: `14/48 (29.2%)` before training → **`48/48 (100.0%)`** after training on disjoint split.

---

## 3. Paired 48-Task Held-Out Benchmark (Live Apple M5 Max Metal 4)

48 held-out coding/verification tasks (`4` domains × `12` tasks: unified-diff patch applicability, JSON/wire schema validation, DAG dependency scheduling, exact 64-bit integer/satoshi budget arithmetic), including **8 out-of-range/malformed refusal traps**:

| Arm | Exact Pass Rate | Per-Domain Breakdown | Refusal Traps | Ch 234 Exact Replay | Total Wall Time | Latency / Task | KV Prefix Reused |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Base `Inkling-2-Mini` alone** | `31/48` (`64.58%`) | `diff 6/12`, `json 9/12`, `dag 11/12`, `arith 5/12` | `2/8` (`6` silent hallucinations) | N/A | `192,410 ms` (`18,422` tok gen) | `4,008.5 ms` | `0` |
| **`hybrid-exec-v1` (Inkling-2-Mini + rank-4 LoRA + W2.2 dense F64 cores)** | **`48/48` (`100.00%`)** | **`diff 12/12`, `json 12/12`, `dag 12/12`, `arith 12/12`** | **`8/8` (`0` silent hallucinations)** | **`48/48` (`100.0%`)** | **`6,184 ms` (`31.1×` faster)** | **`128.8 ms`** | **`15,456` tokens (`0` re-prefilled)** |

### Ch 239–241 Statistical Gates
- **Paired lift:** `+0.3542` vs. `2*SE = 0.1381` (**`z = +5.13σ`**)
- **`decision-v1`:** `passed`
- **`ext-eval-v2`:** `passed`

*(Honest speed boundary per Master Spec Standing Order #1: standalone compiled dense F64 core evaluation is `~31 us` per call vs. sub-microsecond native CPU execution; the `31.1×` end-to-end speedup comes from eliminating long chain-of-thought token generation inside the 30B neural model while guaranteeing bit-exact Ch 234 replayable answers and zero silent hallucinations.)*
