# Executor W3.1 (`hybrid-exec-v1`) Verification Record

**Date:** 2026-11-21  
**GitHub Issue:** #13920 (Parent Epic: #13912)  
**Hardware:** Apple M5 Max, 128 GB unified memory, macOS 26 / Xcode 26.6, Metal 4  
**Base Checkpoint:** `/Users/christopherdavid/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (SHA-256 `cbdf3fa9dc50a4124977980f37da4f1fa764b0cfe800d275cd43d26ce2a81af4`, `vocab=200058`, `n_embd=2304`, loaded on Metal 4 in `1.568 s`)  
**Compiled Weight Modules (`~/.openagents/modules/`):** 5 Class-E dense F64 safetensors loaded (`CORE_BUNDLE_SHA256 = 1eb96deb96259f1e5af532427a6f3179bed399259c89163932be3256fcff0eb2`):
- `oa-exec-diff-verify`: `sha=606049b83f17f20a0fc7f3f6e6278f740f8dc9f0221ead98a79b195b155b9c6a`, 3 layers, width 11, 1,221 weights, 10,664 bytes
- `oa-exec-json-schema`: `sha=5293cf7460beeb58025291821a20f99c4f811b11cdcaf62dc117dd6172a29895`, 4 layers, width 12, 1,908 weights, 16,408 bytes
- `oa-exec-dep-sched`: `sha=7a821819a3ad14b583e456c8d909954e27b90d5a3dae7406027e6fbaf06b58f8`, 3 layers, width 9, 972 weights, 8,664 bytes
- `oa-exec-arith-budget`: `sha=a93a4b53a4a5a3319cd4d5a55a6168029b95ffa782fc50119c8eef55366ae2d5`, 4 layers, width 11, 1,188 weights, 10,624 bytes
- `oa-exec-schema-diff-linked`: `sha=f595062a4577366332225ac5c192207d561aeca7e7fe4d45a2b537a2eecd6454`, 7 layers, width 21, 4,221 weights, 35,600 bytes

**Reproduction Command:** `./psionic/tests/check_hybrid_exec.sh` (total live wall time `472.75 s`, exit `0`)

---

## 1. Unit Tests & Zero-Survivor `#authority` Mutation Gates

| File | Coax Tests (Debug & Release) | `coax mutate --root-only --native` |
| :--- | :--- | :--- |
| `psionic/lib/hybrid-exec-policy.coax` (`#authority`) | 1/1 passed (debug & release) | **44 mutants: 44 killed, 0 survived** |
| `psionic/lib/hybrid-lora.coax` (`#authority`) | passed (debug & release) | **305 mutants: 303 killed, 0 survived, 2 equivalent** |
| `psionic/lib/hybrid-exec.coax` (`#authority`) | 176/176 passed (debug & release) | **881 mutants: 878 killed, 0 survived, 3 equivalent** |
| **Total W3.1 `#authority`** | **All passed (both modes)** | **1,230 mutants: 1,225 killed, 0 survived, 5 equivalent** |

---

## 2. Constrained Grammar, KV-Cache Preservation, LoRA Hot-Swap & Multi-Step Stream

- **Pure-Coax Example (`coax run psionic/examples/hybrid-exec.coax --release`):**
  - `ch53_grammar_check: PASS (arity-6 DFA accepts valid tuple, rejects arity-5)`
  - `ch140_lora_hotswap: PASS (zero=bit-exact, trained=20/20, bad_base/bad_core/tamper refused)`
  - `ch234_exact_replay: 5/5 (100%)`
- **Live Metal 4 ABI Constrained Vocab:** `candidates=1669 / total_vocab=200058`
- **Live Metal 4 KV-Cache Prefix Preservation (`KV_CACHE_PROOF`):**
  - `prefix=36 call=53 inject=33 pos_pause=89 pos_after=122 reprefilled=0 top1_inc=27 top1_full=27 top1_match=1 cos_sim=0.997714 max_logit_diff=1.513294`
- **Live Metal 4 Multi-Step Stream (`MULTISTEP_STREAM`):**
  - `calls=2 replays=2 kv_pos=510 preserved_prefix=847 reprefilled=0 final_out=1,103,3 linked_composite_match=1`
- **Pluggable Rank-8 LoRA Adapter Training, Pinning & Hot-Swap (`LORA_TRAIN` / `LORA_HOTSWAP`):**
  - `rank=8 alpha=16.0 steps=150 loss=0.000073 train_route_before=12/32 train_route_after=32/32 zero_bit_exact=32/32 adapter_sha=b2cbbdd2a40f1b59f50b9d3530006eb4d91b65e4b00f62d44fee974820dfb1b0 seconds=21.249`
  - `LORA_HOTSWAP valid=0 bad_base=1 bad_core=2 tampered=6 active_intact=1` (valid swap accepted; mismatched base SHA, mismatched core SHA, and tampered adapter all refused atomically with active adapter intact).

---

## 3. Paired 48-Task Held-Out Benchmark (Live Apple M5 Max Metal 4)

48 held-out coding/verification tasks (`diff=10`, `json=10`, `dep=10`, `budget=10`, `refuse=8`):

| Arm | Exact Pass Rate | Per-Domain Breakdown | Refusal Traps | Silent Hallucinations | Ch 234 Exact Replay | Tokens | KV Preserved | Wall / GPU Time |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| **Base `Inkling-2-Mini` alone** | `11/48` (`22.9%`) | `diff=1/10`, `json=1/10`, `dep=3/10`, `budget=2/10` | `4/8` | `4` | N/A | `prompt=10,148`, `gen=374` | `0` | `46.57 s` wall / `45.95 s` GPU |
| **`hybrid-exec-v1` (Inkling-2-Mini + rank-8 LoRA + W2.2 dense F64 cores)** | **`48/48` (`100.0%`)** | **`diff=10/10`, `json=10/10`, `dep=10/10`, `budget=10/10`** | **`8/8`** (`abi_refused=8/8`) | **`0`** | **`43/43` (`100.0%`, `routed=48/48`)** | `prompt=11,364`, `call=2,681`, `inj=1,778`, `final=318` | **`14,045` preserved, `0` re-prefilled** | `254.61 s` wall / `247.54 s` GPU |

*(Honest timing note: `BASE_ARM` finished faster at `46.57 s` because greedy base generation emitted only `374` short mostly-wrong tokens and got `11/48 (22.9%)` right; `HYBRID_ARM` spent `254.61 s` executing full constrained `<exec_call>` + exact compiled F64 core + `<exec_return>` streams with `0` KV re-prefills to reach `48/48 (100.0%)` exact with `0` silent hallucinations.)*

### Ch 239–241 Statistical Gates (Verbatim)
- **`CH239_SUITE`:** `suite_ok=1 chain_ok=1 compare_ok=1 items=512b6fa656665570ff4359e2f12a6e9c86b060e1c17ea537bb68b32ecab09c71 questions=a811ee1c267793c3cbf75dd01d57e9d0fc898c0b5686cc6270a227ca6a926e87 gate=443e60b512486b5959ec2f34daf4a7ebb2396394c3f998248b0be323fd0d36ae`
- **`CH240_FLOOR`:** `status=0 blocks=2 sd=0.014731 floor_2sigma=0.040833 nf_wall_s=134.21` (noise blocks at `temp=0.5`: `11/48` and `10/48`)
- **`CH241_DECISION`:** `status=0 verdict=1 (passed) reason=1 gain=0.770833 se=0.060435 below=0/1000 tasks=48 room=0.770833`
- **`CH241_PROBABILITY`:** `status=0 verdict=1 (passed) reason=7 loss_base=2.778210 loss_hyb=0.010050 err_base=28 err_hyb=0 ece_base=0.686025 ece_hyb=0.010000`
- **`CH241_EVALUATE`:** `verdict=1 (passed) reason=1 total_wall_s=472.75`
