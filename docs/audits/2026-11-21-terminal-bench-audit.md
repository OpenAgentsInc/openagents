# Terminal-Bench Historical & Current-State Audit

**Date:** 2026-11-21  
**HEAD Audited:** `424345b388` (`origin/main`, post-Nuke Coax era) + complete reachable git history from `0aa2d953dd` (`2023-11-07`) through pre-Nuke `853417f290` (`0b401561a0^`, `2026-10-27`)  
**Scope:** Forensic inventory of every way the `openagents` repository has ever integrated, executed, reported, standardized on, or referenced **Terminal-Bench** (1.0 / 2.0 / 2.1 / 4.0), its official **Harbor** harness (`harbor-framework/harbor`), and its **ATIF** (Agent Trajectory Interchange Format) trajectory standard — including all historical eras, measured results (both wins and negative/inconclusive findings), integrity reckonings, deletions, and the exact current state and gaps on `origin/main`.

---

## 1. Executive Summary

### High-Level Findings

1. **Four independent Terminal-Bench / Harbor stacks were built and deleted in 11 months.** Between `2025-12-03` and `2026-10-20`, this repository accumulated **534 commits** matching Terminal-Bench / Harbor / `tbench` / `tb2`–`tb4` across `--all` (excluding nautical and unrelated false positives) and **19,904 distinct non-nautical historical file paths** (19,376 under `bench/terminal-bench/` alone). Each generation was wiped by a major language/architecture reset — including three full repository Nukes (`d6b8ce1bec` on `2025-12-01`, `dabc08102f` on `2026-09-18`, and `0b401561a0` on `2026-10-27`).
2. **Today on `origin/main` (`424345b388`), zero Terminal-Bench harness, adapter, or task suite exists.** All active coding evals on `origin/main` (`evals/openagents-desktop/tier2/`, `tier4/`, `evals/session-history/`, `GymBenchmarkView.swift`) run small custom local coding tasks or session replays — none execute Terminal-Bench tasks or invoke Harbor for Terminal-Bench.
3. **What genuinely endured across every reset is Harbor's `ATIF-v1.8` trajectory standard and out-of-process Harbor tooling.** On `origin/main`, canonical Harbor `ATIF-v1.8` is implemented in pure Coax (`coax/lib/nostr/atif/{log,digest,chunks}.coax`, `coax/lib/lean/atif.coax`), specified in `nostr/nips/openagents/NIP-ATIF.md` and `docs/protocol/openagents-protocol-spec.md` (`trace.v1`), and used by `evals/session-history/`, Desktop's Trace Drawer (`2d32e69d87`), and Lean Gym distillation (`e6d894bc0c`). Separately, OpenAgents Desktop's Gym wraps external Harbor runs for **PLE (`ple-kit`)** (`apps/openagents-desktop/ple-run-kit.coax`, `GymDiscovery.swift`), not Terminal-Bench.
4. **There are actionable stale references and one critical integrity gap on `origin/main`:**
   - **Critical — Synthetic "Terminal-Bench-style" stand-ins in `psionic/tools/executor-h7-qual.coax` (`#13928`):** While `psionic/docs/executor-network-master-spec.md:320` calls for sealing a frozen held-out benchmark combining real **Terminal-Bench** tasks and real multi-file repository tasks, `executor-h7-qual.coax:312-369,440-455` implements buckets `C_tbench_verify` and `D_tbench_composite` as in-process synthetic checks over 4 hardcoded constant inputs (`ti % 4`) with a seeded linear classifier, `passed = route_ok && (ti % 2 == 0)` for the baseline, and synthetic token accounting (`roundtrips * 4200 + e_fail * 1800`). `docs/verification/executor-network-effect-h7.md:19-20,71-72` reports `8/8 (100%)` on those buckets. Under `AGENTS.md` ("Done means the real thing works"), these must not be conflated with real Terminal-Bench evaluation.
   - **Stale specs and paths:** `docs/coder/desktop-evals-spec.md` (lines 33, 49, 104, 157) still cites deleted `bench/terminal-bench/` as an active harness and calls for a 25-task slice drawn from it (whereas `evals/openagents-desktop/tier4/slice.json` contains 0 Terminal-Bench tasks); `.gitignore:66` still ignores `/bench/terminal-bench/experiments/**/*.tar.gz`; `nostr/nips/openagents/{NIP-ATIF,NIP-EVAL,NIP-XP}.md` still point to deleted `bench/terminal-bench/` and Rust `crates/{atif,atif-nostr,gym-leaderboard}` paths; and `docs/transcripts/README.md:321,845` carries dead links to `../terminal-bench/README.md` and `../gym.md`.
5. **The empirical record from 11 months of Terminal-Bench work is remarkably consistent:**
   - **Configuration, lean tool menus, short prompt caches, and deterministic read-only briefings (`Jev` probes) produced every reproducible cost win** — validated out of sample on Terminal-Bench 2.1 (`#9683`: 30/65 confirmed cost wins at median `$0.0075` = 2.9% of Fable 5 xhigh, though 48% vs 92% reliability and zero-reasoning caveat; `#10356`: lean Claude `0.58×` cost on 6 unseen TB2.1 tasks at `18/18` parity).
   - **Heavy controller machinery, learned self-checks, escalation, and knowledge transfer repeatedly failed to hold up out of sample on Terminal-Bench 4.0** — matched controller v8 (`18/30` vs `15/30`, McNemar `p = 0.51`, `+68%` cost, `2.2×` time), `v9-escalate` (`0/12` rescues for `$20.40`), `microluna-v18` (`0/9` confirmation), and TB4 out-of-sample Rounds 2–4 (`0/24` held-out even after fixing Codex reasoning to `xhigh`).

---

## 2. Audit Methodology & Git History Coverage

### How History Was Inspected

The audit inspected the full object graph reachable across all local and remote refs in `/Users/christopherdavid/work/openagents`:

- **Pre-Nuke history (`2023-11-07` → `2026-10-27`):** Reachable from `0b401561a0^` (`853417f290`) and historical refs (`backup/*`, `main-eg`, `tags/backup/local-099294143e`), covering every commit back to `0aa2d953dd` ("Initial commit").
- **Post-Nuke history (`2026-11-01` → `2026-11-21`):** Reachable from `origin/main` (`424345b388`, 5,258 commits). Note that on this workstation `.git/shallow` grafts `origin/main` at `2c18d4ccdf` (`2026-11-01`), while local refs cover `2026-10-27`–`2026-11-01` (`c395df94fb`, `099294143e`), leaving no material blind spot in the audit window.
- **Queries run:** Commit-message regex scans across `--all` (`terminal.bench|tbench|t-bench|terminalbench|harbor|tb-2|tb2|tb3|tb4|tb 2`), historical path inventories across `--all`, broader `bench|atif|sandbox|gym|eval` scans over the `2025-11`–`2026-05` dormancy window, creation/deletion lifecycle traces (`--diff-filter=AD`), `git show` reads of primary reports/JSONs/scripts at each era's peak, and a full `git grep` of `origin/main`.

### Commit Volume by Month (Across `--all`)

| Month | Matching Commits | Active Era / Notes |
| --- | ---: | --- |
| `2023-11` – `2025-11` | 0 | No Terminal-Bench or Harbor usage prior to Nuke #1 (`d6b8ce1bec`, `2025-12-01`). |
| `2025-12` | 176 | **Era 1:** MechaCoder TS/Bun -> Rust GPUI -> Python `harbor/` (TB 2.0, ATIF v1.4). |
| `2026-01` – `2026-02` | 0 | Dormant (DSPy/DSRS, sandbox, and DSE evals did not use Terminal-Bench). |
| `2026-03` | 1 | **False positive:** `46adab4382` ("safe-harbor snapshot metrics" in economy). |
| `2026-04` – `2026-05` | 0 | Dormant (ARC, Apple FM, Gemma/GPT-OSS benches did not use Terminal-Bench). |
| `2026-06` | 39 | **Era 2:** Probe SHC smoke, Gym v1, Hydralisk Harbor `0.15.0`, Khala TB 2.0 (`#6253`). |
| `2026-07` | 3 | **Era 2 tail:** Gym/evals migration from D1 to Postgres + GCS (`#8355`, `#8523`, `#8515`). |
| `2026-08` | 54 | **Era 3:** Rust `openagents gym` + `docker/harbor-runner` + `tb2-cross-section` + CoderBench. |
| `2026-09` | 243 | **Era 4:** `bench/terminal-bench/` (TB 4.0, TB 2.1, Coder One, Microluna, Microcoder, ATIF v1.8). |
| `2026-10` | 16 | **Era 4 tail:** `#10356` frozen validation, GCS offload (`#11110`), agent-page Gym results. |
| `2026-11` | 2 | **Era 5 (post-Nuke):** Coax `ATIF-v1.8` history evals spec & Desktop Gym Benchmarks (non-TB). |
| **Total** | **534** | **533 genuine + 1 false positive (`46adab4382`)** |

### Explicit False Positives Excluded

To avoid polluting the audit with homonyms, the following matches in git history and `origin/main` were verified and excluded as non-Terminal-Bench:

- **Nautical "harbor" assets and prose:** 3D coastal meshes (`assets/verse/coast/source/harbor/*`, `assets/verse/generated/coast/harbor/*`, commit `4c752bc73e` "Add the tidal coast zone and harbor shelter"), SLAB ocean/port docs (`metaverse/slab-build.coax`, `metaverse/slab-where.coax`, `docs/research/ocean/*`), transcript `docs/transcripts/010.md` ("floating harbors"), and test fixture slug `"harbor"` in `metaverse-engine/editor/doc.coax:1054-1056`.
- **Legal/economic "safe-harbor":** Commit `46adab4382` (`2026-03-05`, economy certification gating).
- **Unrelated `testbench` / `bench` / wordlists:** Laravel vendor `testbench.yaml`, `bip39` English wordlist (`"harbor"`), `coax/web/fast-edit-bench.coax`, `psionic/tests/bench_elephant.sh`, `crates/autopilot`'s internal `B-001`..`B-013` benchmark system deleted in `839a317bba` (`2025-12-25`).

---

## 3. Master Timeline: The Five Eras and Three Nukes

| Era | Dates | Stack & Primary Paths | TB / Harbor / ATIF Versions | Peak Scale | Headline Measured Outcomes | How It Ended |
| --- | --- | --- | --- | --- | --- | --- |
| **Nuke #1** | `2025-12-01` | `d6b8ce1bec` (1,942 files, -274k LOC) | — | — | Cleared pre-Dec 2025 Tauri/web repo. | Followed 2 days later by Era 1. |
| **Era 1a: TS/Bun MechaCoder + TB HUD** | `2025-12-03` – `2025-12-10` | TypeScript/Bun (`src/harbor/`, `src/bench/`, `src/tbench-hud/`, `src/cli/tbench*.ts`, `.github/workflows/tbench-smoke.yml`) | TB 2.0 (`88–90` tasks), Harbor `MechaCoderAgent`, **ATIF v1.4** | ~188 TB/ATIF/gym files | Apple FM mini-suite 100%; `regex-log` 17/19 (89.5%) -> **hardcoded solution caught & removed (`893538db6a`)**. | `7cfe9ba491` & `3bd3998c06` ("Delete all TypeScript, go 100% Rust"). |
| **Era 1b: Rust GPUI + Python `harbor/`** | `2025-12-10` – `2025-12-19` | Rust (`crates/gym`, `crates/terminalbench`, `crates/harbor`, `crates/atif`) + Python `harbor/` (`ClaudeCodeAgent` + TestGen) | TB 2.0, Harbor `ClaudeCode` subclass, ATIF | ~140 files | TestGen v2 + Haiku 4.5 passed `regex-log` (via Harbor ATIF, 9:56), `log-summary-date-ranges`, `overfull-hbox`; failed 4 harder tasks. | `f33e72a55c` (`2025-12-13`, "thx zed bye4now") & `714ea04717` (`2025-12-19`, `#1515`). |
| **Dormancy** | `2026-01` – `2026-05` | DSPy/DSRS, DSE, ARC, Apple FM, Gemma/GPT-OSS | None | 0 | No Terminal-Bench runs. | Ended `2026-06-08`. |
| **Era 2: Gym v1 + Probe + Khala Fleet** | `2026-06-08` – `2026-07-07` | Rust `benchmark-cloud`, Bun/Effect Cloudflare Workers (`apps/openagents.com/workers/api/src/inference/gym/*`), Hydralisk shell runner (`docs/inference/terminal-bench-2/`) | TB 2.0 (`89` tasks), **Harbor `0.15.0`** (`environment_mode="separate"`), **ATIF v1.7** | 42 commits; 17 Worker/script files + 7 Probe fixtures | SHC Probe+Codex smoke `0/1` (`db-wal-recovery`); Khala black-box `2/3` (0.667, 0 errs) localized full-89 Khala failures to `#6310` tool-calling outage; fake `/gym` fixtures purged (`95a5854268`). | `d613b8ea22` & `a098b3e29f` (`2026-08-28`, deleting TS/Node/Vite product roots). |
| **Era 3: Rust `openagents gym` + CoderBench** | `2026-08-24` – `2026-09-01` | Rust CLI (`crates/openagents-cli/src/gym/`), `docker/harbor-runner`, `bench/adapters/openagents_coder.py`, `bench/optimize/` (GEPA), Phoenix live Gym | TB 2.0 (`tb2-cross-section` 12 tasks, `tb2-quick` 2 tasks), Harbor container, ATIF v1.7 | 54 commits; hash-chained `bench-results/tb2-cross-section.jsonl` | `tb2-cross-section`: `gpt-5.6-luna` **9/12 (75%)**, `glm-5.3-flash` **7/12 (58.3%)**, local `qwen3.8:27b` **7/12 (58.3%)**. Codified `terminal-bench-lessons.md`. | **Nuke #2** (`dabc08102f`, `2026-09-18`, 3,537 files, -1.34M LOC). |
| **Era 4: `bench/terminal-bench/` Research Program** | `2026-09-19` – `2026-10-20` | Python `tbench` (48 modules) + Rust `crates/{coder-one,microcoder,gym,gym-leaderboard,verse-gym,atif,atif-nostr}` + Nostr `NIP-ATIF`/`EVAL`/`XP` + GCS bucket | **TB 4.0** (`v4.0.0`, 66 tasks), **TB 2.1** (65+15 tasks), **Harbor `0.22.0`**, **ATIF v1.8** | **259 commits; 19,085 files** (14,744 traces, 3,370 experiment files, 186 published, 114 docs; 3.02 GB offloaded to GCS) | **TB2.1 OOS:** 30/65 confirmed cost wins (median `$0.0075` = 2.9% of Fable 5 xhigh, 48% vs 92% pass, zero-reasoning caveat). **#10356 (6 TB2.1):** lean Claude `0.58×` cost (`18/18`); routed Codex `2.29×` time. **TB4 OOS R2–R4:** `0/24` held-out. **Matched TB4 v8:** `18/30` vs `15/30` (`p=0.51`, `+68%` cost). | **Nuke #3** (`0b401561a0`, `2026-10-27`, 49,021 files, -24.7M LOC -> Coax reset). |
| **Era 5: Post-Nuke Coax Era (Current)** | `2026-10-27` – `2026-11-21` | Pure Coax `coax/lib/nostr/atif/`, `coax/lib/lean/atif.coax`, `evals/session-history/`, `evals/openagents-desktop/`, `ple-run-kit.coax`, `psionic/tools/executor-h7-qual.coax` | **ATIF v1.8** (Coax); Harbor referenced for external `ple-kit` & Lean; **no TB runner** | 0 TB tasks; 41 files mentioning TB/Harbor | No real Terminal-Bench runs. `executor-h7-qual.coax` runs synthetic `C_tbench_verify`/`D_tbench_composite` micro-checks (`8/8` each). | **Active (`origin/main` `424345b388`)**. |

---

## 4. Era-by-Era Forensic Breakdown

### 4.1 Pre-December 2025 (`2023-11-07` → `2025-12-02`)

A full search across all refs prior to `2025-12-03` returns **0 commits** mentioning Terminal-Bench, `tbench`, Harbor, or ATIF. On `2025-12-01`, **Nuke #1** (`d6b8ce1bec`, 1,942 files, 274,834 deletions) cleared the earlier Tauri/web stack, setting the stage for MechaCoder and the first Terminal-Bench build two days later.

---

### 4.2 Era 1 (`2025-12-03` → `2025-12-19`): MechaCoder, Harbor v1, Apple FM, TestGen, and Rust GPUI

#### Sub-Phase 1a: TypeScript / Bun Stack (`2025-12-03` – `2025-12-10`)

Terminal-Bench entered the repository alongside ATIF right after Nuke #1:

- **ATIF adoption (`2025-12-03`):** `038d10540f` ("ATIF plan") and `8cdb1588e1` implemented **ATIF v1.4** for trajectory capture, followed by HUD streaming JSONL persistence (`35108ef0c9`, `e789ff8af7`, `62a9fb8160`) and Healer stuck-detection integration (`5a42ad4a47`, `fbb0fdd3c8`).
- **Research & Harbor adapter (`2025-12-04`):** `4f3e52e768` (`docs/research/terminal-bench.md`) documented Terminal-Bench 1.0 (80 tasks), Terminal-Bench 2.0 (88–89 tasks, announced Nov 7, 2025), Terminus, and Harbor. The same day, `faa9dabaee` (`docs/claude/plans/terminalbench.md`) planned the integration and shipped:
  - `src/harbor/openagents_harbor/` — Python Harbor package with `MechaCoderAgent` (`b90aa57887`, `be5b540a41`, `acf8814982`, `647838019e`).
  - `src/cli/tbench.ts` and `src/cli/tbench-local.ts` — CLI wrapper for Harbor and internal local evaluation (`be789f32c3`, `c898bacb94`, `4f10cc125d`).
  - Three-mode execution (`local` / `sandbox` / `harbor`) with hybrid Docker container verification and auto-installed `pytest` (`2718118dc6`, `41b02c7680`, `8f290e56d9`, `5827f362ed`, `b123791795`).
  - **TB HUD & Desktop UI (`src/tbench-hud/`, `src/tbench-view/`):** Full-screen TB dashboard, category tree, output viewer, ATIF step display, and live subprocess streaming (`8e4ea57351`, `b96d01fd9a`, `1ddc9d9eaa`, `083fb4075f`, `52511d04c4`).
  - **GitHub Actions CI (the only era with TB CI):** `.github/workflows/tbench-smoke.yml` ran `bun test src/cli/tbench.harbor.test.ts` on PRs touching `src/bench/**`, `src/cli/tbench*.ts`, or `src/tbench-hud/**`.
- **Apple Foundation Models (FM) + Skill Library + Hillclimber / MAP / TestGen (`2025-12-05` – `2025-12-08`):** Built an overnight iteration runner (`b3caa2627e`), Effect-native Apple FM integration (`1beb1235f3`), no-gradient Skill Library + TRM/SOAR learning (`99eb04407e`, `bd4c60c017`), Archivist lessons bridge (`4aee63f56f`, `f030999647`), and the Hillclimber MAP orchestrator + blind test generation (`TestGen`) (`edaa376557`, `a1e7e31371`, `4cd5b9fca4`).

#### Integrity Reckoning #1 (`2025-12-08` – `2025-12-09`)

After `fcd540c688` reported 100% on a miniature FM suite and `d289007318` ("Complete TTC integration success - 89.5% achieved!") celebrated `17/19` tests on `regex-log`, an immediate self-audit surfaced two critical flaws:

1. **Suite conflation (`149c8d3056`):** Clarified that `89.5% (17/19)` was against TB2's benchmark tests while `45.8% (11/24)` was against TestGen's stricter edge-case suite.
2. **Hardcoded solution leakage (`893538db6a`, `02a79b6cb0`):** Discovered that `decomposer.ts` lines 67–79 contained a task-specific hardcoded regex solution. `893538db6a` flagged this explicitly as **cheating**, removed hardcoded target solutions in `02a79b6cb0`, and produced `docs/logs/20251208/1219-benchmark-gaming-analysis.md` mapping the spectrum from legitimate domain/process knowledge through test feedback to expected-output leakage and hardcoded solutions.

#### Sub-Phase 1b: Rust GPUI Workbench & Python `harbor/` (`2025-12-10` – `2025-12-19`)

On `2025-12-10`, `7cfe9ba491` deleted `src/bench/` (19 files, ~233 KB) and `3bd3998c06` ("Delete all TypeScript, go 100% Rust") deleted 37,531 lines of TypeScript. Terminal-Bench was rebuilt in Rust and Python:

- **Rust crates:** `crates/atif` & `crates/atif-store`, `crates/gym` (`25b26c33b9` — TBCC dashboard, Hillclimber monitor, RegexCrusade, TestGen visualizer, baseline/reporter), `crates/harbor` (`f4328cf7a5` — `tbench` binary), and `crates/terminalbench` (`c2214d4a1c` — Docker streaming runner + MechaCoder Gym panel, followed by six Tokio reactor crash fixes on `2025-12-12`: `bc56d8c055`–`24fae3d017`, and a migration from `claude-agent-sdk` back to Harbor's `tbench` in `e7011c3f74`).
- **Python `harbor/` adapter (`2025-12-14`):** `a483253dac` and `84891a30c3` added a clean Python `harbor/` package whose `ClaudeCodeAgent` subclassed Harbor's built-in `harbor.agents.installed.claude_code.ClaudeCode`, prepended the `TESTGEN_SKILL` protocol, used `--output-format stream-json`, and inherited Harbor's native ATIF conversion.
- **Measured Era 1b validation runs (`2025-12-11` – `2025-12-14`):**
  - `docs/logs/20251211/tb2-strategic-assessment.md` (`d85a2ded97`) catalogued all 90 TB2 tasks (4 easy, 53 medium, 33 hard) and recorded initial **3/4** TestGen v2 + Haiku 4.5 validation (`regex-log` PASS ×2, `log-summary-date-ranges` PASS, `constraints-scheduling` FAIL).
  - Subsequent logs added `overfull-hbox` PASS (`db5445dc64`), `prove-plus-comm` FAIL (`36eedcae93`), `filter-js-from-html` FAIL (`5ad2c7ca63`), `kv-store-grpc` FAIL (`52f412ee42`), raw Opus/Sonnet baselines on `regex-log` (`1e9ec77e78`, `892f3c3d15`), and the Harbor+ATIF end-to-end pass on `regex-log` with `claude-haiku-4-5-20251001` (`reward=1.0`, 9:56, `fcd94d7027`).
- **Demise:** `crates/terminalbench`, `crates/harbor`, and `crates/gym` were deleted on `2025-12-13` (`f33e72a55c`, "thx zed bye4now"); `harbor/` was deleted on `2025-12-19` (`714ea04717`, "Cleanup OpenAgents to desktop foundation `#1515`"). (The Christmas commit `839a317bba` removed `crates/autopilot`'s separate internal `B-001`..`B-013` benchmark suite, not Terminal-Bench.)

---

### 4.3 Dormancy Window (`2026-01` → `2026-05`)

For five months, the repository pursued DSPy/DSRS (`crates/dsrs`), DSE long-context datasets, ARC-ML interactive benchmarks, Apple Foundation Models adapter benches, and local Gemma/GPT-OSS inference benchmarks. **Zero Terminal-Bench or Harbor framework integrations landed during this window.** (The single keyword hit on `2026-03-05`, `46adab4382`, concerns economic "safe-harbor snapshot metrics".)

---

### 4.4 Era 2 (`2026-06-08` → `2026-07-07`): Probe Retained Fixtures, OpenAgents Gym v1, Hydralisk, and Khala (`#4563`, `#6253`, `#6261`, `#6271`)

Terminal-Bench returned in June 2026 in two connected efforts:

#### Sub-Phase 2a: SHC Harbor Probe Smoke & GEPA Retained Fixtures (`2026-06-08` – `2026-06-09`)

- **Live SHC Harbor smoke (`9e5e62a4c9`, `#4563`):** Ran `terminal-bench@2.0` task `terminal-bench/db-wal-recovery` on host `oa-shc-katy-01` via Harbor using Harbor's `codex` agent plus Probe's `coding-sqlite-wal-recovery` skill (`docs/benchmarks/2026-06-08-shc-harbor-probe-live-smoke.md`). The trial exited with `NonZeroAgentExitCodeError` (`reward=0.0`, 61.5 s), exercising the full normalized public-safe closeout bundle (`crates/benchmark-cloud/examples/probe_shc_harbor_live_smoke_receipt.rs`: `result.json`, `artifact_manifest.json`, `proof_bundle.json`, `route_scorecard.json`, `probe-closeout.json`, with `public_claim: none`).
- **Retained failure-family fixtures (`f5919c7669`):** `apps/pylon/docs/probe-port/probe-retained-terminal-bench-fixtures.md` and `packages/runtime/src/benchmark/fixtures.ts` retained 7 public-ref-only TB failure-family handles (`configure-git-webserver`, `db-wal-recovery`, `filter-js-from-html`, `gcode-to-text`, `pypi-server`, `query-optimize`, `runner-stall-supervision`) for GEPA Stage 0/1 candidate selection, with tests rejecting any hidden task prompts, verifier content, or raw Harbor traces.

#### Sub-Phase 2b: OpenAgents Gym v1, Harbor `0.15.0` Audit, Workers API, & Khala Black-Box Runner (`2026-06-23` – `2026-07-07`)

- **Harbor architecture decision (`131f750b0d`, `f1f114477c`, `2026-06-25`):** A 667-line audit (`docs/gym/2026-06-25-harbor-for-gym-terminalbench-and-benchmarks.md`) evaluated Harbor `0.15.0` (`projects/repos/harbor`, Python 3.12+/uv/Docker) and established the durable architectural boundary:
  - Run Harbor **out-of-process** on **Hydralisk** (Python/NVIDIA stack); never port Harbor's container orchestration into product workers.
  - Adopt Harbor's **separate verifier sandboxes** (`[verifier] environment_mode = "separate"`, shipped upstream `2026-05-15`) to satisfy the Gym's distinct-device verifier requirement.
  - Consume Harbor's `VerifierResult` float `reward` (`/logs/verifier/reward.txt`) and native **ATIF-v1.7** trajectories (paired with Khala/Codex/Claude Code -> ATIF-v1.7 converters landed the day before in `0e08d2dbaf`, `5d1d717d54`, `88f323b1d8`).
- **Cloudflare Workers & Web `/gym` stack (`apps/openagents.com/`):** Landed `harbor-dispatch.ts` (`94db5619a4`), verifier placement evidence (`5b3701e160`), `harbor-reward.ts` (`c07bf1fe3f`), `terminal-bench-comparison.ts` (`5b7a3ab855`), `terminal-bench-khala-orchestration.ts` (`ae9b9beb39`), `terminal-bench-goal-report.ts` (`0868f6c4c4`), `harbor-full-trace-archive-*` + migration `0239_gym_harbor_full_trace_archives.sql` (`a3391aab0b`), live Harbor `result.json` progress ingest + realtime push to `/gym` (`2d89cdcfc6`, `d7c37f5ebe`, `71d0dda9ee`, `1529cc629d`, `#6261`, `#6271`), and `demand_kind=internal` trace auto-tagging (`#6298`, `20cb17eb96`).
- **Integrity Reckoning #2 (`95a5854268`, `2026-06-25`):** *"Remove all fake/fixture data from /gym; render live-or-empty only."* Caught fabricated benchmark data rendered by `/gym` and its API — a seeded `41/89` GLM run, hardcoded comparison rows (`69.7 / 67.4 / 70.0 / 69.1%`), and a synthesized Phase-0 fixture report — and replaced all of them with live-ingested data or honest empty states.
- **Measured Khala TB 2.0 Black-Box Run (`da472748c5`, `#6253`, `2026-06-26`):** Isolated `docs/inference/terminal-bench-2/run-khala-tb2.sh` pointed Harbor's `terminus-2` agent at public `https://openagents.com/api/v1` (`openagents/khala`) against `terminal-bench@2.0` (89-task official denominator; bounded 3-task slice):
  - **Result (`docs/inference/terminal-bench-2/measured-run-summary.json`):** **2/3 solved (`mean reward = 0.667`)**, **0 errors**, 86,086 prompt / 7,481 completion tokens, 3m59s (`fix-git`: 1.0, `log-summary-date-ranges`: 1.0, `regex-log`: 0.0).
  - **Diagnostic value:** Contrasted sharply with the live full-89 Khala run's 57 errored trials (while the owner-armed 10-replica `GLM-5.2-REAP-504B` Hydralisk run trended 10/19 passed), proving the bottleneck was **Phase-0 serving reliability** (tool-bearing requests falling off GLM-REAP onto a Fireworks tool-caller, `#6310`) rather than model capability.
- **Storage migration & demise:** `2026-07-04`–`2026-07-07` (`8965d12ddc`, `906c329b65`, `5876059180`) moved gym/eval persistence off Cloudflare D1 onto Postgres + GCS. All TypeScript product roots (`apps/openagents.com`, `apps/pylon`, `packages/*`) were deleted on `2026-08-28` (`d613b8ea22`, `fae80bde79`, `a098b3e29f`).

---

### 4.5 Era 3 (`2026-08-24` → `2026-09-01`): Rust `openagents gym`, `docker/harbor-runner`, `tb2-cross-section`, and CoderBench

As the workspace pivoted away from TypeScript toward a Rust CLI + Phoenix web app, Terminal-Bench 2.0 became the first real benchmark lane for the new `openagents gym` CLI:

- **Harness & Containerization:** Built `bench/adapters/openagents_coder.py` (`e23a34eb11` — Harbor installed-agent adapter running the native Rust `coder` binary headless inside trial containers), `docker/harbor-runner/Dockerfile` (`29208175d8`, `6c41e2f1ba`), `crates/openagents-cli/src/gym/` (`suite`, `run`, `results`, `corpus`, `dataset`, `env`, `gate`, `rates`), Phoenix live Gym view (`a2110cbcaf`), and `bench/optimize/` GEPA prompt-surface optimizer over live Harbor runs (`68e81bd74f`, `e5690a0371`).
- **Curated Suites:** `4669feeeb4` curated a 12-task `tb2-cross-section` suite (`build-cython-ext`, `count-dataset-tokens`, `fix-code-vulnerability`, `git-leak-recovery`, `merge-diff-arc-agi-task`, `nginx-request-logging`, `openssl-selfsigned-cert`, `password-recovery`, `regex-log`, `sanitize-git-repo`, `schemelike-metacircular-eval`, `sqlite-with-gcov`) plus a 2-task `tb2-quick` smoke lane (`4e740208b1`).
- **Anti-Gaming Guardrails Established:**
  - `8703101286`: Report cost per accepted outcome and **refuse to guess** when metering is missing (`costDisposition: "cost_unknown"` / `"unmetered_local_lane"`).
  - `df073b9b46`: Append-only, hash-chained `openagents.bench_result.v2/v3` receipt log (`bench-results/tb2-cross-section.jsonl`).
  - `4960a2a2c5`: Pin suite coverage from trial directories on disk so a partial/smoke run cannot be published as a suite score (exits 3).
  - `129984ca8d`: Dropped the `tb2-cross-section` cost ceiling once proxy-lane costs proved unmeasurable.
- **Immutable Measured Results (`bench-results/tb2-cross-section.jsonl`, 12 tasks):**

| Date | Model | Lane | Passed | Success Rate | Wall Time | Tokens / Cost Status | Gate |
| --- | --- | --- | ---: | ---: | ---: | --- | --- |
| `2026-08-26` | `gpt-5.6-luna` (`v0.4.0`) | `proxy` | 9 / 12 | 75.0% | 3,151 s | 6,006,329 prompt / 88,163 comp; `cost_unknown` | `unverifiable` |
| `2026-08-28` | `glm-5.3-flash` | `proxy` | 7 / 12 | 58.3% | 4,243 s | `cost_unknown` | `passed` |
| `2026-08-28` | `qwen3.8:27b-mtp-q8_0` | `local` (Ollama) | 7 / 12 | 58.3% | 6,470 s | `unmetered_local_lane` | `null` |

- **CoderBench Methodology Codification (`f91388362f`, `2026-08-27`):** `docs/coderbench/terminal-bench-lessons.md` formally documented what OpenAgents learned from Harbor (87 adapters, 48 installed agents, 35 environment providers) and Terminal-Bench: copy wholesale (verifier decides, content-digest pinning, fixed installed-agent protocol, parallelism economics, mandatory ATIF receipt per graded run) and adapt (owned real-issue provenance rather than authored puzzles).
- **Demise:** Wiped on `2026-09-18` by **Nuke #2** (`dabc08102f`, 3,537 files, 1,343,538 deletions).

---

### 4.6 Era 4 (`2026-09-19` → `2026-10-20`): The `bench/terminal-bench/` Research Program (Peak Scale)

One day after Nuke #2 (`805b692ae7`, `2026-09-19`), OpenAgents launched its largest, most statistically rigorous Terminal-Bench program in the repository's history. Before Nuke #3, it reached **259 commits** and **19,085 committed files** under `bench/terminal-bench/` (14,744 in `traces/`, 3,370 in `experiments/`, 408 in `microcoder-runs/`, 186 in `published/`, 145 in `samples/`, 72 in `reference/`, 48 in `tbench/`, 46 in `tests/`, 35 in `studies/`, 10 in `fire/`), plus **114+ reports** in `docs/terminal-bench/`.

#### Infrastructure & Protocol Architecture

- **Python `tbench` runner (`bench/terminal-bench/pyproject.toml`, `tbench/` — 48 modules):** Pinned `harbor==0.22.0` (Python `>=3.12`, `uv.lock`). Added prebuilt toolchain Docker layers (`1efa47ffb1`), warm Docker image retention (`70cbca610e`), `tbench try` & `looptime`, resume/cancellation recovery (`e8bb753094`), GPU/disk/memory watchdogs (`eddfaa537f`, `306441be91`), usage-limit classification and requeue (`756500c1f9`, `#9564`), and multi-service Compose/separate-verifier parity for Microcoder (`f82047daf9`).
- **SHA-256-Pinned Episode Contract (`openagents.coder.episode.v1`, `docs/coder/terminal-bench-contract.md`):** Harbor adapter `tbench.coder_v05:CoderV05` verified the exact artifact SHA-256 on the host and again inside `/opt/openagents/bin/coder-v05`, ran `episode doctor` before spending inference, and collected `manifest.json`, `episode.atif.jsonl`, `trajectory.atif.json`, `artifacts/`, `verification/`, and `evaluation/`.
- **Benchmarks Targeted:**
  - **Terminal-Bench 4.0 (`v4.0.0`, commit `452bf305c6da`, 66 tasks, 8-hour timeouts):** Retained the `2026-09-23` Harbor Hub reference snapshot (`bench/terminal-bench/reference/tb4-leaderboard.json`, 27 rows × 330 trials — #1 Codex GPT-6 Astra max `192/330 = 58.2%` at `$3,267`; #2 Claude Code Fable 5.1 max `191/330 = 57.9%` at `$6,244`; #15 Fable 5.1 low `143/330 = 43.3%` at `$2,359`; #24 Codex GPT-5.6 Luna max `57/330 = 17.3%` at `$347`; 7 tasks unsolved by all 27 rows).
  - **Terminal-Bench 2.1:** 65 held-out tasks + 15 dev sweep tasks (reference: Harbor Hub Fable 5 xhigh, `299/325 = 92%`), plus 8 dev tasks at pin `3b5caaa4863d` (4 panel + 4 extended).
- **Published Leaderboards, `ATIF-v1.8`, Nostr NIPs, & GCS Offload:**
  - `73ac7c4266` (`2026-09-28`) standardized every writer on **Harbor `ATIF-v1.8`** (additive audio content parts; reading all `1.x`) while preserving `v1.7` recorded evidence unchanged and validating via `tbench.atif.harbor_validate` (since pinned Harbor `0.22.0` listed through `v1.7`).
  - `crates/gym-leaderboard` (`82fce05be5`, `8f1b733a6d`, `6bfc94876d`) built `bench/terminal-bench/published/leaderboard.v1.json` containing **7 boards** (`tb4-fable-delegate-repro-9776`, `tb21-oos-microcoder-9683`, `tb4-fable-delegate-dev-9746`, `tb4-microcoder-shared-fact`, `tb4-oos-microcoder-9683`, `reference-harbor-hub-tb4`, `reference-harbor-hub-tb21`), signed under `NIP-EVAL` kind `3195`, displayed on the Grid Gym RESULTS board (`a524fd39fe`) and agent profile pages (`d9b38bd97c`, `2026-10-20`).
  - `NIP-XP` (`d8b593ea6b`, `ce3c76257e`) built `kb-transfer` and `reproduce` quests directly over Terminal-Bench recipes (`openagents.xp-recipe.v1`, e.g., `benchmark: "terminal-bench"`, `benchmark_version: "2.1"`).
  - `#11110` (`8b03a43cdf`, `7410ecc7f0`, `2026-10-09`) moved 1,141 large archive files (**3,024,519,898 bytes / 3.02 GB**) including `bench/terminal-bench/experiments/**/*.tar.gz` to `gs://openagents-bench-artifacts` with SHA-256 manifests (`bench-artifacts.json`), cutting checkout size from **4.81 GB to 1.85 GB**.

#### Complete Quantitative Record of Era 4 Studies

| Study / Experiment | Date & Key Commits | Benchmark & Design | Measured Result | Statistical / Methodological Verdict |
| --- | --- | --- | --- | --- |
| **Coder One Dev Arc (Loop -> Jev Probe v2)** | `2026-09-22` (`9d7f081dc0`, `e75e280300`) | 8 dev tasks (`3b5caaa4863d`) × 3 trials | `jevprobe2-opus-lean-low-5m`: **24/24 (100%)**, `$0.054/trial`, `24 s` vs Claude Code Opus 5.5 (`$0.136`, `38 s`). Luna Jev probe v1: `12/12` panel at `$0.0055/trial` (v2 dropped to `17/24`). | **Validated in-sample:** **-61% cost, -37% time** for lean 6-tool + 5m cache + Jev probe briefing on Opus. Gemini explorer loop regressed (`3/4`, `$0.250`, 10× slower). |
| **Matched Controller Test (`#9567`)** | `2026-09-23` (`110486d1e6`) | 10 TB4 tasks × 3 paired trials; executor fixed to Opus 5.5 medium | Coder One v8: **18/30 (60%)**, `$45.42` (`$1.51/trial`), `14.6 min`. Plain Claude Code: **15/30 (50%)**, `$27.04` (`$0.90/trial`), `6.6 min`. | **Negative for controller:** Pass gain not significant (McNemar `p = 0.51`); **+68% cost** (sign test `p < 0.0001`), **2.2× time**. Jev briefing alone saved 20% on session 1 (`$21.64` vs `$27.04`); persistence burned `$23.31` (52%). |
| **Effort Routing (`v9`) & Escalation (`v9-escalate`)** | `2026-09-24` (`afff2317d2`, `c7d6c8ef52`) | 6 TB4 tasks (`effort-9569`) & 8 TB4 tasks (`escalate-9571b/c`) | `v9`: `8/15` vs `v3` xhigh `10/14` (McNemar `p = 0.69`); missed `gsea-proteomics` (`0/3` vs `2/3`). `v9-escalate` to GPT-6 Astra: fired 12×, cost `$20.40`, **0/12 rescued**. | **Negative:** Across 81 graded trials, final checks did not separate passes from failures (`19/38` pass when all checks passed), leaving escalation/persistence blind. |
| **Microluna (`v1`–`v19`) & Fire Loop** | `2026-09-24`–`25` (`9d6c45af75`, `3a25a0ff1f`, `cca58d1092`) | Luna-sized TB4 family (`v18`) + `embedding-drift-monitor` (`v19-fire`) | `microluna-v18`: **0/9 confirmation, 0/9 dev**. Offline validity rejected departure miners, check-line grades, and literal archive checks. `v19-fire`: `5/5` in-sample at `$0.0153/run` (excl. judge), median `5:26` vs Fable low `2:55`. | **Negative / Inconclusive:** No Microluna policy promoted; development pivoted to Microcoder (`#9584`, `#9607` closed without scientific claims passing). |
| **Microcoder TB 2.1 Out-of-Sample Study (`#9683`)** | `2026-09-26`–`27` (`63e860904d`, `2961e7a762`, `87bffb942d`, `46be7a14ab`) | **65 held-out TB2.1 tasks**, **knowledge OFF**, Microcoder `ea9c973576`, GPT-6 Luna (Codex login), 200 steps, `$2` cap | **30/65 confirmed OOS cost wins** (≥2/3 passes below Fable 5 xhigh cost/trial; corrected up from 27 after shared-log fix). **83/127 graded passes**; every pass a cost win; median pass **`$0.0075` (2.9% of Fable 5 xhigh)** in `1:49`. Spend: `$4.24–$4.45`. | **Genuine OOS cost win with two caveats:** (1) Reliability gap: `31/65 (48%)` screen pass rate vs Fable 5 xhigh `299/325 (92%)`. (2) **Zero-reasoning label (`ac3ee05901`):** Codex strict tool schema caused Luna to emit 0 reasoning tokens. |
| **Microcoder TB 4.0 OOS Knowledge Transfer (`#9683`, R1–R4)** | `2026-09-26`–`27` (`3399cf3941`, `e134a6ca00`, `5642dbfd4c`) | **24 gradeable held-out TB4 tasks** (+ Fable-fails pool), KB ON (up to 154 entries) | **R1:** paused `0/6` (quota 429). **R2** (60 steps, OpenRouter): **0/24 held-out**, 0 Fable-fails. **R3** (200 steps, Codex no-reasoning): **0/24 held-out**, 1 unconfirmed Fable-fails pass (`session-window-debug`). **R4** (`ac3ee05901` fixed, `xhigh`): **0/24 held-out**, 0 Fable-fails. | **Clear negative OOS result on TB4:** Held-out knowledge transfer with GPT-6 Luna did not generalize to TB4 even with reasoning fixed at `xhigh`. (Retrospective pre-study pass on `react-lead-form`: `$0.110` vs Fable low `$2.46`.) |
| **Jev-Briefed Fable 5.1 Low Delegate (`#9746` & `#9776`)** | `2026-09-27` (`24440ad574`, `d5852d141f`, `c545e0fd3a`) | In-sample dev (`fin-saccr-rwa`, 7 series) + frozen repro on 14 TB4 tasks × 2 passes | `#9746`: `s7a1` beat Fable 5.1 low (`$0.9429`, `149.5 s` vs `$1.2246`, `222.5 s`; `2/13` attempts). `#9776` repro: **13/28 passed**, **4/28 beat both cost and time bars** (`0/14` pass 1, `4/14` pass 2). | **In-sample only:** All 4 beats used knowledge written from that same task; 0/4 on the 2 tasks lacking own-task knowledge; 20/28 delegates hit deadline with unknown cost. |
| **Frozen `#10356` Validation** | `2026-10-03` (`65175b1e6c`, `060ce8911e`) | **6 unseen TB2.1 medium tasks** × 4 arms × 3 trials (**72 runs**); calibrated before any run | **Lean Claude vs bare Claude:** **`0.58×` cost (95% CI `0.48–0.66`)**, **18/18 vs 18/18**, time `1.14×` (NS). **Routed Codex vs bare Codex:** `0.75×` cost (`0.59–1.08`, NS), **`2.29×` time (`1.89–2.55`)**, 17/18 vs 18/18. | **Validated:** Lean Claude cuts cost ~42% on unseen tasks at equal quality. **Loss:** Routed Codex step-by-step Microcoder loop is >2× slower than `codex exec`. |

#### Integrity Reckoning #3 (`docs/terminal-bench/data-quality.md`)

Era 4 set the repository's highest bar for experimental honesty by explicitly logging every operational and analytical failure rather than hiding or silently rerunning them:

- **Quota & OAuth failures (`2026-09-23`):** 21 TB4 trials hit Claude 429 usage limits and were initially graded as agent failures; 10 trials died on `401 OAuth access token has been revoked` when local logins refreshed (fixed by `claude setup-token`).
- **Harness bugs caught in post-mortems:** 24 `coder-one-tunable-luna-v2` trials ran a stale pre-manifest binary (`*-wrong-artifact-*`); root disk hit `ENOSPC` under parallel ML task builds; `report.py` miscounted 24 shared-millisecond confirmation pairs (fixed in `87bffb942d`, moving TB2.1 OOS wins from 27 to 30); and **the Codex route diff (`2026-09-26-route-diff.md`, `ac3ee05901`)** discovered that passing `next_action` as a strict function tool suppressed reasoning tokens completely on Codex, prompting retroactive `"no reasoning"` labels across all prior Codex-route reports (`46be7a14ab`, `e134a6ca00`).
- **Contamination & ungradeability accounting:** `ks-solver-cpp` was immediately declared burned/in-sample when an operator read reasoning lines during fault debugging; `freecad-platform-drawing` was separated as an upstream verifier image failure; `f379161124` added a lexicon check flagging prompt phrases lifted from tuned task text.

#### Demise of Era 4

On `2026-10-27`, **Nuke #3** (`0b401561a0`, 49,021 files, 24,736,635 deletions) reset the repository to Coax, removing `bench/terminal-bench/`, `docs/terminal-bench/`, `crates/{coder-one,microcoder,gym,gym-leaderboard,verse-gym,atif,atif-nostr}`, and `bench/efficiency/` from the active tree.

---

### 4.7 Era 5 (`2026-10-27` → `2026-11-21`, Current `origin/main` `424345b388`): Post-Nuke State

Every single reference to Terminal-Bench or Harbor currently tracked on `origin/main` falls into one of five categories:

#### A. Active Ecosystem Reuse — Canonical Harbor `ATIF-v1.8` in Pure Coax (Healthy)

| Path on `origin/main` | Role |
| --- | --- |
| `coax/lib/nostr/atif/log.coax`, `digest.coax`, `chunks.coax` | Pure Coax implementation of Harbor `ATIF-v1.8` append-only `.atif.jsonl` logs, sorted-key SHA-256 `steps_digest`, and `openagents.atif-manifest.v1` (`19ee04ecfc`). |
| `coax/lib/lean/atif.coax` | Emits Harbor/Psionic-readable `ATIF-v1.8` `trajectory.atif.json` + `.atif.jsonl` for Lean Gym runs (`e6d894bc0c`). |
| `evals/session-history/README.md`, `tools/ingest.coax` | Converts local Claude Code, Codex, OpenAgents Desktop, Antigravity, and OpenCode transcripts into canonical Harbor `ATIF-v1.8` (`6d7f952fa6`, `3b339140df`). |
| `docs/coder/session-history-evals-spec.md` | Standardizes multi-source session evaluation on Harbor `ATIF-v1.8` (`b6c28b83be`). |
| `docs/protocol/openagents-protocol-spec.md` | Defines `trace.v1` mirroring Harbor `ATIF-v1.8` (lines 339, 522, 608, 2008, 2067, 3311). |
| Desktop Trace Drawer & Mobile Thread Proof | `2d32e69d87`, `6c0a0f81cf` consume `ATIF-v1.8` directly in OpenAgents Desktop and iOS. |

#### B. Active External-Harbor Integration for Non-Terminal-Bench Domains (Healthy)

| Path on `origin/main` | Role |
| --- | --- |
| `apps/openagents-desktop/ple-run-kit.coax` | Detached launcher (`ple-run kit start|status|stop`) wrapping external `ple-kit`'s `eval/run.py` (Docker + Harbor + model proxy) for 3D MEP/IFC engineering tasks (e.g., `data-hall-mep-routing`). |
| `apps/openagents-desktop/GymDiscovery.swift:281-285` | Discovers sibling `verifier.json`, `reconciliation.json`, `trial.log`, and `harbor.log` files for PLE Gym runs. |
| `docs/desktop/steer-ple-from-chat.md` | Operational spec for driving `ple-kit` / Harbor runs from chat. |
| `docs/lean/roadmap.md:118` | Plans `coder/tui/lean-run.coax` mirroring `ple-run-kit.coax` for local Docker/Harbor isolation of Lean benchmarks. |

*(Note: `apps/openagents-desktop/GymBenchmarkView.swift` — landed in `272bd3a642` / `61d0f5be73` / `de37a3a426` — runs side-by-side Psionic Metal vs llama.cpp live coding benchmarks over bundled Desktop Tier 2 + Tier 4 tasks, not Terminal-Bench or Harbor.)*

#### C. Literature, Transcripts, and Historical Citations (Informational)

- `coder/book/ch028-goose.md`, `ch031-openhands-sdk.md`, `chapters.json`: Cites upstream `block/goose`'s `evals/harbor/README.md` (TB2 89-task comparisons) and OpenHands' Terminal-Bench usage as external coding-agent prior art.
- `docs/coder/microcoder.md:23`: Summarizes historical pre-Nuke `crates/microcoder` (24,315 LOC including `tbench`).
- `docs/transcripts/244.md` (June 2026 GLM 5.2 + Harbor for Terminal-Bench) and `docs/transcripts/287.md` (Sep 2026 Building a System One Coding Agent / Coder One on Terminal-Bench).
- `docs/protocol/openagents-protocol-spec.md:307,4458`: Historical count citing `tbench.*` (43) in pre-Nuke JSON schema inventory.

#### D. Stale / Broken Post-Nuke References to Deleted `bench/terminal-bench/` (Actionable Cleanup)

| File & Lines | Stale Reference | Reality on `origin/main` |
| --- | --- | --- |
| `.gitignore:66` | `/bench/terminal-bench/experiments/**/*.tar.gz` | `bench/` does not exist on `origin/main`. |
| `docs/coder/desktop-evals-spec.md:33,49,104,157` | Treats `bench/terminal-bench/` as an existing CLI harness and specifies a 25-task Tier 4 slice "drawn from `bench/terminal-bench/`". | `bench/terminal-bench/` was deleted in `0b401561a0`. `evals/openagents-desktop/tier4/slice.json` contains 4 bug-derived golden tasks + 25 small custom local tasks (`a1`–`c2`, `k01`–`k20`) — **0 Terminal-Bench tasks**. |
| `nostr/nips/openagents/NIP-EVAL.md:228,257` | Cites `bench/terminal-bench/published/` and `crates/gym-leaderboard/src/signed.rs`. | Both paths were deleted in `0b401561a0`. |
| `nostr/nips/openagents/NIP-XP.md:140,237` | Cites `"source": "bench/terminal-bench/results/…"` and `"benchmark": "terminal-bench"`. | `bench/terminal-bench/` was deleted in `0b401561a0`; no Coax runner currently emits `openagents.xp-recipe.v1` Terminal-Bench runs. |
| `nostr/nips/openagents/NIP-ATIF.md:3,17,37,41,54` | Still declares implementation in deleted `crates/atif`, `crates/atif-nostr`, and `crates/nostr`. | Replaced post-Nuke by pure Coax `coax/lib/nostr/atif/{log,digest,chunks}.coax`. |
| `docs/transcripts/README.md:321,845` | Relative Markdown links to `../terminal-bench/README.md`, `../gym.md`, and `../coder/runtime/traces.md`. | Dead links on `origin/main`; those docs exist only pre-`0b401561a0`. |

#### E. Critical Integrity Finding — Synthetic "Terminal-Bench-Style" Buckets in `psionic/tools/executor-h7-qual.coax` (`#13928`)

`psionic/docs/executor-network-master-spec.md:320` specifies the Hypothesis H7 qualification bar:

> *"Seal a frozen, held-out coding benchmark combining **Terminal-Bench** tasks and **real OpenAgents multi-file repository tasks**..."*

Inspection of `psionic/tools/executor-h7-qual.coax` (committed in `7b8d5181ef`, `2026-11-21`) and its verification receipt `docs/verification/executor-network-effect-h7.md` reveals a major divergence from that spec:

1. **No Terminal-Bench tasks, containers, Harbor harness, or live model/agent execution occur in `executor-h7-qual.coax`.** The 16 tasks in buckets `C_tbench_verify` (`h7_17_tb_schema_1`..`h7_24_tb_diff_2`) and `D_tbench_composite` (`h7_25_tb_comp_1`..`h7_32_tb_comp_8`, lines 440–455) do not run an agent in a terminal environment.
2. **Hardcoded constant-input micro-checks + synthetic baseline & cost formulas (`lines 312–369`):**
   - For Round 0 (`!use_modules`), line 317 hardcodes `passed = route_ok && (ti % 2 == 0)` (even indices pass when `route_ok` is true; odd indices always fail).
   - For Rounds 1–3 (`use_modules`), lines 319–340 switch on `ti % 4` to call four fixed constant invocations (`mc_exec_check_schema`, `mc_exec_check_dep`, `mc_exec_check_budget`, `mc_exec_check_diff`) plus `mc_exec_verify_replay`, gated by a 12-feature synthetic linear classifier (`h7_predict_prob`) over seeded feature vectors (`seeds 9016..9031`).
   - Token counts and dollar costs are synthesized by formula (`toks := roundtrips * 4200 + e_fail * 1800; cost_m := toks * 2`, lines 361–362), and median wall-clock time is `64–121 µs` (`executor-network-effect-h7.md:44-47`) — microseconds of in-process arithmetic, not terminal task execution.
3. **Why this matters under `AGENTS.md`:** The repository's standing rule (*"Done means the real thing works"*, `649c31f297`, `2026-11-20`) explicitly prohibits synthetic or miniature stand-ins from reporting under the real benchmark's name. While `executor-h7-qual.coax` is a valid deterministic unit/replay harness for Class-E compiled weight cores and DiLoCo/Ch 239–241 gating math (and Section 2 of `executor-network-effect-h7.md` separately ran a real 2-file Metal 4 `Inkling-2-Mini` edit test), calling `C_tbench_verify` / `D_tbench_composite` "Terminal-Bench-style" and reporting `8/8 (100%)` under H7 risks repeating the exact benchmark-inflation patterns that Eras 1, 2, and 4 explicitly fought to eliminate.

---

## 5. Cross-Era Synthesis: Lessons Learned

### What Proved Durable

1. **ATIF (`v1.4 -> v1.7 -> v1.8`) as the universal trajectory format:** Every single era from Dec 2025 through Nov 2026 converged on Harbor's ATIF RFC 0001. Keeping one canonical JSONL/JSON format across benchmarks, local agent histories, desktop trace drawers, and Nostr (`NIP-ATIF`) eliminated adapter churn.
2. **Harbor out-of-process, never reimplemented:** Every time OpenAgents tried to hand-roll container orchestration, pytest parsing, or Docker lifecycle management (Era 1a TS sandbox, Era 1b Rust `crates/terminalbench`), it spent days fighting Tokio reactors, apt noise, and mounting bugs before switching back to upstream Harbor (`e7011c3f74`, `131f750b0d`, `e4542628f7`, and post-Nuke `ple-run-kit.coax`).
3. **Scientific discipline built the hard way:** Pre-registered protocols committed before runs (`65175b1e6c`, `63e860904d`), content-digest pinning (`openagents.coder.episode.v1`), separate verifier sandboxes, Wilson/bootstrap CIs + McNemar/sign tests, explicit infra-fault vs task-failure separation (`data-quality.md`), and raw HTTP request dumping (`MICROCODER_DUMP_REQUESTS`) repeatedly caught subtle bugs (hardcoded solutions, fake `/gym` fixtures, `#6310` tool-call routing outage, Codex zero-reasoning suppression, shared-millisecond log double-counting) that ad-hoc benchmarks miss.
4. **Configuration beats complex control:** Across hundreds of measured trials, simple structural wins (trimming tool schemas, 5-minute prompt caching, deterministic read-only `Jev` file/probe briefings, appropriate reasoning effort) delivered durable 40–60% cost savings; heuristic controllers, self-checks, escalation loops, and TB4 knowledge transfer consistently failed to earn their spend out of sample.

### What Repeatedly Broke or Wasted Effort

1. **Rebuilding custom benchmark dashboards four times:** TB HUD (Era 1a), GPUI TBCC/RegexCrusade (Era 1b), `/gym` Cloudflare Workers UI (Era 2), Phoenix LiveView Gym (Era 3), and `crates/gym` TUI + Grid Gym RESULTS board (Era 4) were all built from scratch and deleted within weeks.
2. **Committing raw trace archives into git:** Era 4 grew to 19,085 files and 4.81 GB before `#11110` offloaded 3.02 GB to `gs://openagents-bench-artifacts`.
3. **Subscription-auth fragility under parallel benchmark load:** Using interactive OAuth subscriptions (Claude Code / ChatGPT Pro Codex) for parallel multi-hour suites caused 401 token revocations, weekly 429 walls, and unbilled list-price approximations rather than metered API bills.
4. **Conflating local smoke suites with real Terminal-Bench:** Repeating across Era 1 (mini-suite vs full TB2), Era 2 (fake `/gym` fixture rows), and Era 5 (`C_tbench_verify` synthetic checks and `desktop-evals-spec.md` claiming Tier 4 draws from `bench/terminal-bench/`).

---

## 6. Actionable Recommendations for `origin/main`

1. **Clean up stale pre-Nuke Terminal-Bench references immediately (low-risk doc/config hygiene):**
   - Remove line 66 (`/bench/terminal-bench/experiments/**/*.tar.gz`) from `.gitignore`.
   - Update `docs/coder/desktop-evals-spec.md` (lines 33, 49, 104, 157) so it accurately describes `evals/openagents-desktop/tier4/slice.json` (4 bug goldens + 25 custom local tasks) instead of citing deleted `bench/terminal-bench/`.
   - Update `nostr/nips/openagents/NIP-ATIF.md` to reference `coax/lib/nostr/atif/` instead of deleted `crates/{atif,atif-nostr,nostr}`, and mark `NIP-EVAL.md:228,257` and `NIP-XP.md:140` pre-Nuke `bench/terminal-bench/` paths as historical examples or replace them with current paths.
   - Fix or annotate the dead relative links in `docs/transcripts/README.md:321,845` to point to commit `0b401561a0^` (pre-Nuke archive).
2. **Separate synthetic H7 micro-verifiers from real Terminal-Bench (`#13928`):**
   - Rename `C_tbench_verify` / `D_tbench_composite` in `psionic/tools/executor-h7-qual.coax` and `docs/verification/executor-network-effect-h7.md` (e.g., `C_class_e_verify` / `D_class_e_composite`), and explicitly note in `executor-network-effect-h7.md` that H7's real Terminal-Bench + multi-file repo coding benchmark (`executor-network-master-spec.md:320`) remains open until executed through real Docker/Harbor task containers against live models.
3. **If re-introducing real Terminal-Bench in the Coax era, follow the battle-tested Era 4 + `ple-run-kit.coax` blueprint:**
   - **Thin out-of-process Harbor wrapper:** Drive upstream `harbor` (`uv` + Docker/Modal/Daytona with `[verifier] environment_mode = "separate"`) from a Coax launcher modeled directly on `apps/openagents-desktop/ple-run-kit.coax`, plus a minimal Python `BaseInstalledAgent` adapter implementing the SHA-256-pinned `openagents.coder.episode.v1` contract for Coax `mcworker` / `openagents` binaries.
   - **Native `ATIF-v1.8` & GCS-first storage:** Emit `ATIF-v1.8` via `coax/lib/nostr/atif/`, commit only pre-registered protocols, compact JSON summaries, and SHA-256 manifests (`bench-artifacts.json`) to git, and store all raw trajectories and container logs in `gs://openagents-bench-artifacts`.
   - **Day-one integrity gates:** Require pre-run request-body dumps verifying reasoning tokens and tool schemas, metered API keys (never shared interactive OAuth logins), separate infrastructure-fault vs task-failure columns, and deterministic verifier calibration (fail on untouched image, pass on oracle) before spending inference.
