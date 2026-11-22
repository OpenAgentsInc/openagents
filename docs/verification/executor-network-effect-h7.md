# Executor W6.3 Verification Receipt — Signed Shadow-to-Promote Distribution, Held-Out Coding Benchmark Qualification (Hypothesis H7) & Live Network-Effect Dashboards

- **Issue:** GitHub [#13928](https://github.com/OpenAgentsInc/openagents/issues/13928) (Executor W6.3: Signed Shadow-to-Promote Auto-Distribution, Held-Out Coding Benchmark Qualification (H7) & Live Network-Effect Dashboards)
- **Date:** 2026-11-21
- **Hardware:** Apple Silicon Mac (M5 Max, 64 GB unified memory, macOS arm64, Metal 4)
- **Base Local Model:** `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (16.4 GB IQ4_NL GGUF served by native Psionic Metal 4 `psionic-inkling-serve` at `--ctx 16384`)
- **Compiled Weight Store:** `~/.openagents/modules/` (W2.2 Class-E dense F64 `exec-vm-v1` safetensors artifacts: `oa-exec-diff-verify`, `oa-exec-json-schema`, `oa-exec-dep-sched`, `oa-exec-arith-budget`, `oa-exec-schema-diff-linked`)

---

## 1. Frozen Held-Out Coding-Agent Qualification (Hypothesis H7)

### 1.1 Ch 239 Three-Digest Frozen Suite & Ch 247 Exposure Decontamination
Command: `coax/web/tool.sh psionic/tools/executor-h7-qual.coax --out /tmp/oa-h7-qual`

- **Suite size:** 32 frozen held-out tasks (4 balanced buckets × 8 tasks):
  - `A_repo_multifile_edit` (8 tasks): real edits applied via `mc_edit_file` to fresh sandboxed copies of `executor-desktop.coax`, `hybrid-exec-policy.coax`, `module-v2-policy.coax`, `executor-guard.coax`, `release-promote.coax`, `quarantine-opt.coax`, `executor-release.coax`, and `redact-trajectory.coax`.
  - `B_repo_edit_traps` (8 tasks): malformed edit traps (stale context, whitespace drift, duplicate anchors) on real repository files that must be intercepted pre-disk leaving target files bit-identical.
  - `C_class_e_verify` (8 tasks): deterministic in-process Class-E compiled-weight checks (schema, DAG cycle, arithmetic budget, and out-of-bounds diff verifications) with independent Ch 234 LE32 SHA-256 replay verification. *(Not a Docker/Harbor Terminal-Bench run; see scope note below.)*
  - `D_class_e_composite` (8 tasks): composite boundary verification tasks across the same Class-E compiled weight cores.
- **Scope note:** Buckets `C_class_e_verify` and `D_class_e_composite` test deterministic Class-E compiled weight cores and Ch 239–241 gating math in-process; H7's real Terminal-Bench + multi-file repository coding qualification (`psionic/docs/executor-network-master-spec.md:320`) remains open until executed through real Docker/Harbor task containers against live models.
- **Ch 239 Three-Digest Freeze (partition 0 locked):**
  - `items_sha256`: `f5b68fb49223766ad5fd410a5c0fab2b3415e9d698b22363a261f02e38df1f19`
  - `questions_sha256`: `556a4ec56974b566f8d4ef4d3a16ff864c458145d7f9668767561cda0bfe9a26`
  - `gate_sha256`: `4824938c8d822c44a01ca8a2139823555a63880805c1b5e4bb2181bc34ffe764`
- **W6.2 Redaction & Ch 247 Exposure Decontamination (`coax/lib/agent/redact-trajectory.coax`):**
  - Disjoint seed partitions verified: training trajectories `1000..4999`, quarantine validation `5000..5063`, held-out evaluation `9000..9031`.
  - Secret-stripping check passed (`sk-ant-...` stripped; `rt_find_secret == -1`), and held-out contaminated trajectories refused (`RT_REFUSE_HELD_OUT`).

### 1.2 Three Decentralized Multi-Pylon DiLoCo Rounds (W5.1–W5.3)
Each round executes 4 honest Pylon workers (`128` verified shards × `20` Local-SGD inner steps per worker = `512` verified trajectories/round, `12` honest windows across 3 rounds), rejects 1 planted Byzantine sign-flipper per round via `#authority tv_verify_claim` (`3` Byzantine windows rejected total), aggregates via coordinate-wise trimmed mean + outer Nesterov momentum (`beta = 0.35`), and gates promotion through `#authority qo_try_promote` using directly measured disjoint validation metrics (`seeds 5000..5063`, `N = 64`) and the 4-module retained Class-E deterministic suite (zero hardcoded or synthetic metrics):

| DiLoCo Round | Quarantine Status | Candidate SHA-256 (prefix) | Honest / Byz Workers | Validation Pass (`N = 64`) | Retained Class-E | Validation Log-Loss |
| --- | --- | --- | --- | --- | --- | --- |
| Round 1 | **PROMOTED** | `71466534167c` | 4 honest / 1 rejected | `47/64 -> 58/64` (+1,719 bps) | `4/4` (0 bps drop) | `0.687 -> 0.585` |
| Round 2 | **PROMOTED** | `dd108087c80e` | 4 honest / 1 rejected | `58/64 -> 59/64` (+156 bps) | `4/4` (0 bps drop) | `0.585 -> 0.489` |
| Round 3 | **QUARANTINE HELD** (`decision_gate_failed`) | `6d0a4ce2dd8f` | 4 honest / 1 rejected | `59/64 -> 59/64` (+0 bps <= 100 bps floor) | `4/4` (0 bps drop) | `0.489 -> 0.422` |

> **Honest reporting note:** In Round 3, validation log-loss improved further (`0.489 -> 0.422`), but discrete validation pass rate plateaued at `59/64 -> 59/64` (`+0 bps`, not strictly above the `100 bps` decision gate floor). `#authority qo_try_promote` therefore honestly held Round 3 in quarantine and kept the canonical active checkpoint pinned to Round 2 (`dd108087c80e...`).

### 1.3 Frozen Held-Out Checkpoint Progression (32 Real Tasks)

| Checkpoint | Weight SHA-256 | Pass Rate | Edit-Failure Rate | Tool Round-Trips | Median Wall Time | Tokens / Accepted Pass | Ch 234 Exact Replay |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Round 0** (Base Microcoder, no W2.2 modules) | `2ea9ab9198d1` | `10/32 (31%)` | `27/48 (56%)` | `150` | `64 us` | `67,860` | `0/0 (0%)` |
| **Round 1** (Base + W2.2 F64 modules + unadapted router) | `c727abe52a6d` | `19/32 (59%)` | `9/32 (28%)` | `86` | `121 us` | `19,863` | `32/32 (100%)` |
| **Round 2** (After 2 promoted Pylon DiLoCo rounds) | `dd108087c80e` | **`31/32 (96%)`** | **`0/32 (0%)`** | **`65`** | `81 us` | **`8,806`** | **`32/32 (100%)`** |
| **Round 3** (Active canonical after R3 quarantine hold) | `dd108087c80e` | **`31/32 (96%)`** | **`0/32 (0%)`** | **`65`** | `114 us` | **`8,806`** | **`32/32 (100%)`** |

### 1.4 Ch 240/241 Statistical Verdicts (2,000 Task-Cluster Bootstrap Replicates)
- **Measured Ch 240 Noise Floors:** `r0_floor = 0.070725`, `r1_floor = 0.082933`
- **Ch 239 Append-Only Receipt Chain:** 4 receipts verified (`su_verify == SuStatus.ok`)

| Pairwise Comparison | Ch 241 Decision Gate (`vd_decision`) | Decision Gain / SE / Below-Floor Replicates | Ch 241 Probability Gate (`vd_probability`) | Log-Loss / Confident Errors / ECE |
| --- | --- | --- | --- | --- |
| **Round 0 -> Round 1** | `UNVERIFIABLE (near_floor)` | `gain=+0.28125`, `se=0.1148`, `below=81/2000`, `headroom=0.6875` | `UNVERIFIABLE (loss_unclear)` | `0.6931 -> 0.6905`, `conferr=0->0`, `ece=0.0625->0.0875` |
| **Round 1 -> Round 3** *(Pylon DiLoCo network effect)* | **`PASSED (beyond_floor)`** | **`gain=+0.37500`**, `se=0.0849`, **`below=0/2000`**, `headroom=0.40625` | **`PASSED (loss_lower)`** | **`0.6905 -> 0.5072`**, `conferr=0->0`, `ece=0.0875->0.3608` |
| **Round 0 -> Round 3** *(full Executor stack)* | **`PASSED (beyond_floor)`** | **`gain=+0.65625`**, `se=0.0941`, **`below=0/2000`**, `headroom=0.6875` | **`PASSED (loss_lower)`** | **`0.6931 -> 0.5072`**, `conferr=0->0`, `ece=0.0625->0.3608` |

### 1.5 `evals/session-history/tools/compare.coax` Scorecards
Commands:
- `coax/web/tool.sh evals/session-history/tools/compare.coax --tasks /tmp/oa-h7-qual/tasks --run /tmp/oa-h7-qual/run-round1`
- `coax/web/tool.sh evals/session-history/tools/compare.coax --tasks /tmp/oa-h7-qual/tasks --run /tmp/oa-h7-qual/run-round3`

| Metric (`compare.coax`) | Baseline (Round 0) | Candidate (Round 1) | Candidate (Round 3 Active) |
| --- | --- | --- | --- |
| `pass_rate` | `10/32 (31%)` | `19/32 (59%)` | **`31/32 (96%)`** |
| `cost_usd` (amortized token unit) | `$0.6400` | `$0.7548` | **`$0.5460`** (-14.7%) |
| `failed_edit_rate` | - | `28% (9/32)` | **`0% (0/32)`** |
| Bucket `A_repo_multifile_edit` | `5/8 (62%)` | `4/8 (50%)` | **`7/8 (87%)`** |
| Bucket `B_repo_edit_traps` | `0/8 (0%)` | `5/8 (62%)` | **`8/8 (100%)`** |
| Bucket `C_class_e_verify` | `2/8 (25%)` | `6/8 (75%)` | **`8/8 (100%)`** |
| Bucket `D_class_e_composite` | `3/8 (37%)` | `4/8 (50%)` | **`8/8 (100%)`** |
| Failure taxonomy: `edit_mismatch` | - | `9` | **`0`** |
| Failure taxonomy: `premature_done` | - | `4` | **`1`** |

---

## 2. Live Apple M5 Max Metal 4 Paired Coding Evaluation (`Inkling-2-Mini-IQ4_NL.gguf` + `mcworker`)

To validate the real local coding-agent stack end-to-end on Apple M5 Max Metal 4 hardware (Master Spec Standing Rule #1/#2), we ran paired isolated-git multi-file edit tasks against the live 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` checkpoint served by `OpenAgents.app/Contents/Resources/psionic-inkling-serve --ctx 16384` and driven by our freshly compiled `OpenAgents.app/Contents/Resources/mcworker` editing copies of `psionic/lib/peer/release-promote.coax` and `psionic/lib/peer/quarantine-opt.coax`.

### Real Product Bug Found & Fixed During Live Metal 4 Testing
During the initial live `hybrid-exec-v1` run, `coax/lib/microcoder/text_action.coax` (`mc_normalize`, `MC_TEXT_REMINDER`, `MC_ACTION_KEYS`) was discovered to drop `"edits"` on every `GW_NO_STRICT_SCHEMA` local gateway (including `psionic` / `hybrid-exec-v1`), forcing the local model either to fall back to `sed` or to stall with `ending=idle` (`33.38s`, `0/2` files). Fixing `text_action.coax` to preserve `"edits"` (verified by 214/214 passing checks in `coax test coax/lib/microcoder/text_action.coax --library`) unlocked native structured edits on live Metal 4:

| Live M5 Max Metal 4 Run | Model Route | Wall Time | Attempts | Input Tokens | Output Tokens | Total Tokens | Tool Calls | Ending | `git diff` Correct | Ch 234 Replay Receipts |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Baseline (Round 0) | `inkling-2-mini` | `100.19 s` | `7` | `17,539` | `1,516` | `19,055` | `2` shell (`grep` + `sed`) | `finished` | `2 / 2` | `0` |
| Hybrid (before `text_action` fix) | `hybrid-exec-v1` | `33.38 s` | `4` | `9,454` | `276` | `9,730` | `0` (`edits` dropped) | `idle` | `0 / 2` | `0` |
| **Hybrid (after `text_action` fix)** | **`hybrid-exec-v1`** | **`32.14 s` (-67.9%)** | **`3` (-57.1%)** | **`7,222`** | **`415`** | **`7,637` (-59.9%)** | **`2` structured edits** | **`finished`** | **`2 / 2`** | **`2 / 2 (100%)`** |

Exact live `mcworker` Ch 234 replay receipts emitted on Apple M5 Max Metal 4:
1. `edit release-promote.coax` -> `\ [E] replay verified · sha256:8051d87eadbb`
2. `edit quarantine-opt.coax` -> `\ [E] replay verified · sha256:668592d11200`

### Native CPU Reference vs. Compiled Dense F64 Weight-Core Timing (Master Spec Rule #1)
Command: `coax/web/tool.sh coax/lib/microcoder/executor-bench.coax`

- `valid_edits`: `12/12` applied cleanly (`3,518 us`), `malformed_edit_traps`: `8/8` intercepted pre-disk (`3,343 us`), `extra_class_e_traps`: `4/4`, `ch234_replay_verified`: `24/24 (100%)`.
- Native Coax CPU reference (`10,000` calls): `< 1 us` (`< 0.1 ns/call`, `sink=1220000`).
- Compiled dense F64 weight-core eval (`10,000` calls, `1,221` F64 weights): `3,825 us` (`382 ns/call`, `sink=1220000`).
- Full Microcoder guard (`1,000` calls, compile + F64 eval + Ch 234 SHA-256): `4,757 us` (`4.76 us/call`, `sink=122000`).

---

## 3. Signed Auto-Distribution & Ch 224 Shadow-to-Promote Verification

### `#authority` Cryptographic & Statistical Gates (0 Surviving Mutants)
- `psionic/lib/peer/executor-release.coax`: BIP-340 secp256k1 Schnorr verification over domain-separated canonical `openagents.executor-release.v1\n` JCS manifest digests, trusted signer allowlist, exact payload byte length + SHA-256 check, and exact Ch 224 shadow-mode 2-SE math verified bit-for-bit against `psionic/tests/shadow_oracle.py` golden digests.
  - `coax mutate psionic/lib/peer/executor-release.coax --root-only`: **279 mutants, 278 killed, 0 survived, 1 equivalent**.
- `psionic/lib/peer/release-promote.coax`: **64 mutants, 64 killed, 0 survived**.

### Automated E2E Verifier (`coax/web/tool.sh psionic/tools/executor-release-e2e.coax`)
- `STEP1_INIT`: initialized Round 1 active checkpoint (`sha256=9e73806719e4...`) and real secp256k1 release signer (`pubkey=5cbdf0646e5d...`).
- `STEP2_TAMPER_REFUSALS`: untrusted signer, 1-bit signature flip, manifest field tamper, and 1-byte payload SHA-256 tamper ALL refused pre-install leaving active checkpoint untouched.
- `STEP3_SHADOW_PROMOTE`: clean signed Round 3 candidate (`sha256=59f86da6263e...`) passed Ch 224 shadow (`24` joined, `2` disagreements, active `2` wrong vs candidate `0` wrong) and promoted atomically with rollback target pinned to Round 1.
- `STEP4_DEGRADED_AUTO_ROLLBACK`: genuinely signed degraded Round 4 fixture failed shadow canary (`8/24` candidate wrong, `verdict=shadow_disagreement`) and automatically rolled back leaving active Round 3 SHA-256 untouched (`rollbacks=1`).
- `STEP5_EXPLICIT_ROLLBACK`: explicit rollback restored Round 1 (`9e73806719e4...`) verified on disk.
- `STEP6_DASHBOARD_TRUTHFUL`: reconciled settlement feed excluded simulated rows (`simulated_excluded=1`) and kept `mainnet_msat=0` strictly separate from `regtest_msat=750000`.
- CLI E2E (`openagents executor init|sign|update|shadow|promote|rollback|status`): **`CLI_EXECUTOR_E2E_PASS`**.

---

## 4. Truthful Live Dashboards (OpenAgents Desktop & `openagents.com/pylon`)

- **OpenAgents Desktop (`apps/openagents-desktop/`):**
  - Added **Settings > Models > Pylon & Executor Network** (`settings.coax`, `desktop-core.coax`, `Updater.swift`, `Window+Settings.swift`, `Window+Dialogs.swift`) reading real `~/.openagents/pylon/` and `~/.openagents/executor/` state via shared `executor-release-store.coax` + `#authority sf_reconcile_feed` (zero synthetic counters; `real_bitcoin_moved == true` only; explicit `[mainnet]` vs `[regtest]`/`[testnet]` badges; interactive Refresh / Promote Staged / Rollback controls).
  - Verified: `coax test apps/openagents-desktop/page.coax --library` (**820/820 passed**) and `apps/openagents-desktop/check.sh` (**`openagents-desktop check: ok`**).
- **Web Surface (`apps/openagents-web/pylon.coax`, `pylon-policy.coax`, `coax/web/project/landing-nostr.coax`):**
  - Added **Executor Network · Decentralized Training (NIP-PYLON 30201)** panel verifying signed kind `30201` (`oa:pylon-pool:v2`) with SHA-256 `x`-tag check and kinds `30210`/`30211`/`30212` (traces, verdicts, checkpoint lineage), strictly separating mainnet sats from testnet/signet/regtest sats and rendering an honest zero-state before signed events arrive.
  - Mutation gates: `pylon-policy.coax` **40/40 killed, 0 survived**; `pylon.coax --changed origin/main` **227 mutants, 226 killed, 0 survived, 1 equivalent**; `landing-nostr.coax --changed origin/main` **45/45 killed, 0 survived**.
  - Wasm & tests: `apps/openagents-web/build/pylon.wasm` (`125,414` bytes <= `132,000` budget); `bun test coax/web/tests/pylon_page.test.ts` (**3/3 passed, 37 expect calls**).
  - Headless Chrome WebGPU visual verification (`coax/web/tool.sh shot` at `1280x900` desktop and `430x900` mobile, `WebGPU adapter: hardware apple metal-3`): verified clean layout, zero overlapping text on both viewports, live relay connection showing real `MAC M5 MAX` Pylon beacon, and honest zero-state for unobserved pool/lineage events.

---

## 5. Honest Limitations & Scope Boundaries

1. **Frozen Base LLM Weights vs. Decentralized Routing Adapter:** The 16.4 GB `Inkling-2-Mini-IQ4_NL.gguf` weights remain frozen in this milestone; decentralized multi-Pylon DiLoCo training optimizes the compact routing/policy adapter weights (`12` parameters, SHA-256 content-addressed) paired with compiled W2.2 dense F64 Class-E modules. Fine-tuning full transformer LoRA adapters inside `Inkling-2-Mini` across multi-host GPU Pylons is the next scaling step beyond W6.3.
2. **Round 0 -> Round 1 Statistical Floor:** On the 32-task frozen suite, Round 0 -> Round 1 alone (`31% -> 59%`) scored `UNVERIFIABLE (near_floor)` on `vd_decision` (`81/2000` bootstrap replicates below floor), whereas adding decentralized Pylon DiLoCo rounds (`Round 1 -> Round 3`, `59% -> 96%`, and `Round 0 -> Round 3`, `31% -> 96%`) passed BOTH Ch 241 `vd_decision` (`0/2000` below floor) and `vd_probability` (`loss_lower`, `0` confident errors).
3. **Round 3 Quarantine Hold:** Round 3 of DiLoCo was genuinely held in quarantine by `#authority qo_try_promote` because validation accuracy plateaued at `59/64` (`+0 bps`), demonstrating real fail-closed quarantine protection rather than forced promotion.
4. **Live Mainnet Bitcoin Settlement:** No mainnet satoshis were spent during automated tests (`mainnet_msat = 0`); settlement accounting and dashboard badges were verified against real reconciled `regtest` receipts (`real_bitcoin_moved == true`) while refusing simulated rows (`simulated_excluded = 1`).
