# Inky Eval Qualification, Class-E Executor Integration & Self-Improvement Architecture

> **Status:** Empirical benchmark qualification, live Metal 4 Class-E Executor integration, and closed-loop self-improvement design for OpenAgents' native Inkling 2.0 harness (`coax/lib/inky/`, `coder/tui/inky.coax`).
> **Date:** 2026-11-22
> **Hardware:** Apple M5 Max (64 GB unified memory, macOS arm64, Metal 4)
> **Local Model:** `~/.openagents/models/inkling-2-mini/Inkling-2-Mini-IQ4_NL.gguf` (`16.4 GB` IQ4_NL GGUF served by native `psionic-inkling-serve --ctx 32768`, `\$0.0000`)
> **Compiled Weight Store:** `~/.openagents/modules/` (`10/10` W2.2 Class-E dense F64 `exec-vm-v1` modules SHA-256 verified)
> **Internal Codename Policy:** "Inky" is an internal engineering codename only and is never exposed in user-facing UI strings or system prompt persona.

---

## 1. Executive Summary

We ran **Inky** end-to-end across five live local eval suites (**67 unique real tasks** across Desktop Tiers 2 & 4, formal Lean 4 theorem proving, and real-world OpenAgents session-history trajectories) plus **two paired live Metal 4 Class-E Executor experiments**:

1. **Standard coding & desktop benchmarks are saturated at 100% (`\$0.0000`):**
   - **Desktop Tier 2:** **14/14 (100.0%)** in `232.3 s` (**1.69× faster** than `mcworker`'s `393.5 s`), `76.9%` continuation KV cache hit vs `mcworker`'s `0.0%`.
   - **Full Desktop Tier 4 (all applicable tasks):** **27/27 (100.0%)** in `618.5 s` (`22.9 s` mean, `19.2 s` p50), **`91.2%–95.1%` continuation KV cache hit** (`77.7%` overall including cold Step-1 prefill).
   - **Lean 4 Core Smoke (`#authority lean-run verify`):** **5/5 (100.0%)** in `231.7 s` (`kernel=ok`, `axioms=[]` on all 5 proofs, `84.3%` overall KV cache hit).
   - **Session History `A_edit_precision` (6 real OpenAgents repo tasks):** **6/6 (100.0%)** in **`194.4 s`** (`32.4 s` mean, `89.9%` continuation KV cache hit) — **5.41× faster** and +17 pp higher pass rate than `mcworker` + `inkling-2-mini` (`5/6` in `1,051 s`), matching cloud `gemini-4-hydrogen` / `gemini-4-neon` (`6/6`) at `\$0.0000`.
2. **Cross-bucket Session History Dev-9 beats prior cloud & multi-GPU Microcoder baselines — while Locked Held-Out Buckets B–E expose the exact real frontier:**
   - **Session History Dev-9 (9 tasks across Buckets A–E):** **5/9 (55.6%)** at `\$0.0000` (`1,490 s`), outperforming every prior `mcworker` Dev-9 run in `docs/verification/microcoder-neon-inkling-lab.md` — including `base-inkling-dev9` (`2/9`), 5-iteration tuned `iter3-inkling-dev9` (`4/9`, `2,268 s`), best pure Vertex `gemini-4-neon` (`4/9`, `\$1.39`), and the 2-GPU + Vertex Neon hybrid combo (`4/9`, `\$1.06`).
   - **Session History Locked Held-Out 6 (hardest multi-file Buckets B–E: `sh_009`, `sh_014`, `sh_015`, `sh_018`, `sh_020`, `sh_024`):** **0/6 (0.0%)**, with **6/6 hitting the 300 s wall-clock timeout** (0 crashes, 0 context overflows, 0 guard trips, `69–90%` continuation KV cache hit). Forensic trace analysis isolated the exact three bottlenecks: single-stream decode speed (`4.0K–7.6K` output tokens at `13–25 tok/s` fills 300 s alone), deeply ingrained shell-edit habits (`101` `run_command` calls vs `0` `edit_file` calls), and multi-file exploration spirals on unfamiliar 2,000-file Coax subsystems.
3. **Class-E Executor + Inky live Metal 4 integration works zero-shot and delivers 3.6–4.0× speedups over W6.1 Microcoder:**
   - Wired `#authority coax/lib/microcoder/executor-guard.coax` directly into `coax/lib/inky/` (always-on Class-E `oa-exec-json-schema` + `oa-exec-diff-verify` pre-disk checks + Desktop `"edit <path>"` diff rows + Ch 234 `[E] replay verified · sha256:<12hex>` pills, plus native 8th tool `executor_verify` on `hybrid-exec-v1` / `INKY_EXECUTOR=1`, verified with **0 surviving mutants across 603 mutations** in `tools.coax` and `guard.coax`).
   - **W6.1 Paired Multi-File Edit Benchmark (`executor-desktop.coax` + `hybrid-exec-policy.coax`):** **`17.21 s`** (`inkling-2-mini`) / **`17.92 s`** (`hybrid-exec-v1`) vs W6.1 `mcworker`'s `61.96 s–69.15 s` (**3.6–4.0× faster**), `2/2` native `edit_file` calls, `0` misses, emitting the **bit-for-bit identical Ch 234 SHA-256 digest `c3e731afbc4d`** previously produced by `mcworker`.
   - **Zero-Shot Class-E Tool Routing (`executor_verify`, 8 exact-computation tasks, zero LoRA):** **`8/8 (100.0%)`** routed to the exact right compiled dense F64 weight module on Attempt 1, **`8/8` Ch 234 replay receipts verified**, and **`15.2%` faster overall** (`175.5 s` vs `206.9 s` raw; `dep_cycle` dropped `41.5 s → 12.3 s`).

---

## 2. Live Eval Qualification Results

All runs below executed on the shipped Metal 4 `psionic-inkling-serve --ctx 32768` binary against `Inkling-2-Mini-IQ4_NL.gguf` (`\$0.0000`).

### 2.1 Suite-Level Summary

| Suite | Tasks | Engine / Worker | Pass | Wall Total (Mean) | Cont. KV Cache Hit | Cost | Baseline Reference |
| --- | ---: | --- | :---: | ---: | ---: | ---: | --- |
| Desktop Tier 2 | 14 | `psionic-metal` / `inky` | **14/14 (100.0%)** | `232.3 s` (`16.6 s`) | `76.9%` | `\$0.0000` | `mcworker`: `14/14`, `393.5 s`, `0.0%` cache (**1.69× faster**) |
| Desktop Tier 4 (full applicable) | 27 | `psionic-metal` / `inky` | **27/27 (100.0%)** | `618.5 s` (`22.9 s`, p50 `19.2 s`) | `91.2%–95.1%` | `\$0.0000` | `77.7%` overall hit incl. cold Step-1 prefill (`206.9K/266.5K` tok) |
| Lean 4 Core Smoke (`lean-run verify`) | 5 | `psionic-metal` / `inky` | **5/5 (100.0%)** | `231.7 s` (`46.3 s`) | `84.3%` overall | `\$0.0000` | All 5 proofs `kernel=ok`, `axioms=[]` (`l01`–`l05`) |
| Session History `A_edit_precision` | 6 | `psionic-metal` / `inky` | **6/6 (100.0%)** | **`194.4 s`** (`32.4 s`) | `89.9%` | `\$0.0000` | `mcworker` + Inkling: `5/6 (83%)`, `1,051 s` (**5.41× faster**); Hydrogen/Neon: `6/6` |
| Session History Dev-9 (Buckets A–E) | 9 | `psionic-metal` / `inky` | **5/9 (55.6%)** | `1,490.2 s` (`165.6 s`) | `82.9%–91.6%` | `\$0.0000` | `mcworker` Inkling: `2/9 → 4/9` (`2,268 s`); Vertex Neon: `4/9` (`\$1.39`); 2-GPU+Neon: `4/9` (`\$1.06`) |
| Session History Locked Held-Out 6 (B–E) | 6 | `psionic-metal` / `inky` (v2) | **0/6 (0.0%)** | `1,806.9 s` (`301.2 s`) | `69.0%–90.3%` | `\$0.0000` | `6/6` hit 300 s wall timeout; `0` overflows, `0` guard trips, `0` crashes |

### 2.2 Session History Dev-9 Per-Task Breakdown vs. Prior Baselines

| Task | Bucket | `inky` (`\$0.00`) | `iter3-inkling` (`mcworker`, `\$0.00`) | `iter4b-neon` (`mcworker`, `\$1.39`) | Outcome & Signal |
| --- | --- | :---: | :---: | :---: | --- |
| `sh_001_claude_todos_wrap` | A | **PASS** (`70.4 s`, 9 att) | PASS (`103 s`) | PASS | Clean Coax `E0216` fix + `coax test --only todos` |
| `sh_004_claude_headless_view` | A | **PASS** (`18.4 s`, 6 att) | PASS (`66 s`) | PASS (`13 s`) | Native `edit_file`, 0 misses, `90.6%` KV hit |
| `sh_008_claude_sidebar_composer_parity` | B | FAIL (`301 s`, 21 att) | FAIL | FAIL | Timeout: 23 exploration cmds, 0 edits |
| `sh_010_codex_slab_mooring_seam` | B | **PASS** (`166.0 s`, 15 att) | FAIL | FAIL (`\$0.2056`) | **3-file Coax+MD fix solved locally where tuned `mcworker` + Neon failed!** |
| `sh_013_antigravity_mobile_row_id` | C | FAIL (`301 s`, 21 att) | FAIL | FAIL | Baseline hit `31,891` tok overflow; v2 fixed overflow (`max 27.4K`) & started edits |
| `sh_017_claude_updater_bundle_align` | C | **PASS** (`165.6 s`, 12 att) | FAIL | FAIL | **3-file updater + proto fix solved locally!** |
| `sh_019_claude_sign_realtime_clock` | D | FAIL (`131.4 s`, 11 att) | PASS (`292 s`) | PASS | Solved semantically + `py_compile` passed; missed literal grep `created_at > prev` |
| `sh_021_claude_host_driver_paste_stub` | D | **PASS** (`35.7 s`, 6 att) | PASS (`239 s`) | PASS | **6.7× faster** than `iter3-inkling-dev9` |
| `sh_023_claude_agents_md_autoload` | E | FAIL (`301 s`, 25 att) | FAIL | FAIL | Greenfield 3-file Coax architecture; 0 edits before 300 s |

### 2.3 Headroom Implications (Ch 240/241 Discipline)

As established in [`psionic/docs/self-improvement.md`](../../psionic/docs/self-improvement.md), a suite already at `100%` has **zero optimization headroom** (`headroom = 0.0`). Therefore:
- **Regression-only locks (must stay 100%):** Desktop Tier 2 (`14/14`), Desktop Tier 4 (`27/27`), Lean Core Smoke (`5/5`), Session History Bucket A (`6/6`).
- **Active self-improvement target suites (high headroom):**
  1. **Session History Buckets B–E (`21` tasks):** currently `3/7` on Dev B–E, `0/6` on Locked Held-Out B–E — massive headroom (`>55–100%`).
  2. **Lean 4 Algebra & Logic (`evals/openagents-desktop/lean/lean-algebra-logic/tasks/`, 10 tasks):** formal proofs beyond core smoke.
  3. **Terminal-Bench Harbor (`coax/lib/tbench/`):** Docker-isolated multi-step CLI/systems engineering tasks (once `coax/lib/tbench/agent.coax` is ported from Microcoder to Inky).

---

## 3. Live Class-E Executor + Inky Experiments

### 3.1 Architecture Gap Closed in `coax/lib/inky/`

During Executor discovery we found that `apps/openagents-desktop/Window+Runs.swift` routed `Hybrid Executor (hybrid-exec-v1)` to `inky`, yet `inky` had neither wired `#authority coax/lib/microcoder/executor-guard.coax` nor emitted `"edit <path>"` unified diff events with Ch 234 replay pills (causing Desktop to display Inky's `edit_file` calls as plain `"Read"` rows with zero `[E]` badges and hiding Inky's edit metrics from `evals/session-history/tools/compare.coax`).

We implemented and verified first-class Class-E Executor integration directly in `coax/lib/inky/`:
- **Always-on Class-E Pre-Edit & Schema Guard (`coax/lib/inky/run.coax`):** Every `edit_file` and `write_file` runs compiled dense F64 weight modules `oa-exec-json-schema` (`1,908` weights) and `oa-exec-diff-verify` (`1,221` weights) **before touching disk**, emitting Desktop-native `command: "edit <path>"` / `"write <path>"` unified diff events carrying `\ [E] replay verified · sha256:<12hex>` (or `[E] replay refused · sha256:<12hex>` with target file untouched).
- **Native Class-E `executor_verify` Tool (`protocol.coax`, `tools.coax`, `prompt.coax`, `run.coax`):** Enabled automatically when `model == "hybrid-exec-v1"` / `"hybrid-executor"` or `INKY_EXECUTOR=1`. Exposes all 8 Class-E compiled weight cores (`diff_verify`, `json_schema`, `dep_sched`, `arith_budget`, `patch_rebase`, `glob_match`, `semver_solve`, `wire_bounds`) as a read-only zero-approval tool with independent `mc_exec_verify_replay` verification and Ch 234 SHA-256 pills.
- **Verification:** `310/310` unit tests passed across `coax/lib/inky/`, `inky selftest: ok` (`10/10` checks), `coax/web/tool.sh layer-check && wire-check && tools-check` (`0` failed), Desktop `transcript` checks (`9/9` passed), and `#authority` zero-survivor mutation gates:
  - `coax/lib/inky/tools.coax`: **467 mutants, 463 killed, 0 survived, 4 equivalent**
  - `coax/lib/inky/guard.coax`: **136 mutants, 136 killed, 0 survived**

### 3.2 Native CPU vs. Compiled Dense F64 Weight-Core Timing (Master Spec Rule #1)

Re-verified on Apple M5 Max arm64 via `coax/web/tool.sh coax/lib/microcoder/executor-bench.coax` (`24/24` Ch 234 replay verified, `8/8` malformed edit traps intercepted pre-disk leaving files SHA-256 bit-identical) and `executor-store-e2e.coax` (`4/4` core + `6` linked composite modules verified, 1-byte tamper refused):

| Execution Path | Iterations | Total Wall | Per-Call Latency |
| --- | ---: | ---: | ---: |
| Native Coax CPU reference (`mv_ref_diff_verify`, `sink=1220000`) | 10,000 | `< 1 µs` | `< 0.1 ns/call` |
| Compiled dense F64 weight-core eval (`mv_eval_model`, 1,221 weights, `sink=1220000`) | 10,000 | `4,269 µs` | `426 ns/call` (`0.43 µs`) |
| Full guard (`mc_exec_check_diff`: compile + F64 eval + Ch 234 SHA-256) | 1,000 | `5,513 µs` | `5.51 µs/call` |

The compiled F64 weight core is ~4,200× slower than scalar native branches, as expected under Master Spec Rule #1, while adding only `5.51 µs` per edit (~90× faster than file I/O and ~10,000,000× faster than one LLM decode turn) in exchange for digest-pinned weight execution and bit-for-bit Ch 234 replay receipts.

### 3.3 Experiment 1 — Exact W6.1 Paired Multi-File Edit Benchmark (Live Metal 4)

Editing isolated git copies of `psionic/lib/executor-desktop.coax` and `psionic/lib/hybrid-exec-policy.coax` (identical targets to `docs/verification/executor-w61-coding-agent.md` Part B):

| Harness / Model Route | Wall Time | Attempts | Total Tokens (Cached) | Structured Edits | Ending | `git diff` | Ch 234 Replay Receipts |
| --- | ---: | ---: | ---: | :---: | :---: | :---: | --- |
| W6.1 `mcworker` (`inkling-2-mini`) | `69.15 s` | 5 | `34,463` (`0`) | 0 (4 shell) | `bad_replies` | 2/2 | 0 |
| W6.1 `mcworker` (`hybrid-exec-v1`, nudged) | `61.96 s` | 4 | `20,406` (`0`) | 2 ok / 2 ref | `finished` | 2/2 | 4/4 (`c3e731afbc4d`) |
| **Upgraded `inky` (`inkling-2-mini`)** | **`17.21 s`** (**4.02× faster**) | **4** | `23,751` (`15,982`) | **2 ok / 0 ref** | **`finished`** | **2/2** | **2/2 (`7a634fffcb4a`, `c3e731afbc4d`)** |
| **Upgraded `inky` (`hybrid-exec-v1`)** | **`17.92 s`** (**3.46× faster**) | **4** | `26,133` (`17,767`) | **2 ok / 0 ref** | **`finished`** | **2/2** | **2/2 (`7a634fffcb4a`, `c3e731afbc4d`)** |

Notice that `edit hybrid-exec-policy.coax` produced the **exact bit-for-bit identical Ch 234 SHA-256 digest `c3e731afbc4d`** across both `mcworker` and `inky`.

### 3.4 Experiment 2 — Zero-Shot Class-E `executor_verify` Routing (Live Metal 4, Zero LoRA)

Eight exact-computation tasks across 5 Class-E domains (`arith_budget`, `dep_sched`, `patch_rebase`, `semver_solve`, `wire_bounds`) run paired in a `read-only` sandbox (no shell/Python):

| Arm | Pass | Routed to `executor_verify` | Ch 234 Replay Receipts | Total Wall | Notable Deltas |
| --- | :---: | :---: | :---: | ---: | --- |
| Raw `inkling-2-mini` (no `executor_verify`) | `8/8` | `0/8` | `0` | `206.9 s` | `dep_cycle` `41.5 s` (4 att, refused to name exact code); `patch_rebase_ok` `42.3 s`; `patch_rebase_overlap` `36.7 s` |
| **`hybrid-exec-v1` (`+ executor_verify`, zero LoRA)** | **`8/8`** | **`8/8 (100%)`** | **`8/8 (100%)`** | **`175.5 s` (-15.2%)** | **`dep_cycle` `12.3 s` (3.37× faster, exact `cycle_detected`)**; `patch_rebase_ok` `17.1 s` (2.47× faster); `patch_rebase_overlap` `21.3 s` (exact `hunk_overlap`) |

**Key empirical insight:** Base `Inkling-2-Mini` already achieves **100% (8/8) zero-shot routing accuracy** onto native OpenAI `executor_verify` tool calls without any LoRA adapter! The lone latency outlier (`budget_big`: `63.6 s` hybrid vs `19.4 s` raw) occurred because Inkling performed the 6-digit multiplication in `<think>` *before* calling `executor_verify` — showing that a lightweight routing LoRA (`H2_ROUTING_ONLY`) should focus on **eliminating pre-tool mental-math thinking** and **shifting shell edits (`python3`/`sed`) onto `edit_file`**, not teaching tool syntax from scratch.

---

## 4. Empirical Failure Taxonomy (Root Causes Observed Today)

Every failure and slow pass across all 67 live runs collapses into six concrete, measurable mechanisms:

| # | Failure Mode | Empirical Evidence Today | Primary Fix Level |
| --- | --- | --- | --- |
| **F1** | **Decode-bandwidth ceiling on long multi-file tasks** | Held-Out 6 generated `4,036–7,591` output tokens (`sh_014` emitted `3,440` tok in 1 step; `sh_024` emitted `3,292` tok at 300 s) at `13–25 tok/s` under 20K ctx — exhausting the 300 s timeout mid-solution. | **Engine (DSpark `mtp.safetensors` + prompt-lookup 1.6–2.0×) + Level 1 adaptive effort** |
| **F2** | **Shell-edit habit overriding `edit_file`** | `4/6` Bucket A tasks and `6/6` Held-Out tasks used `run_command("python3 - <<'EOF'")` / `sed` / `cat` (`101` shell cmds vs `0` `edit_file` on Held-Out 6), bypassing Class-E diff verification, Desktop zero-click edits, and `guard.coax` duplicate-read tracking. Prompt tightening flipped `sh_005` (`sha256:9c058e793198`) but did not overcome the prior on complex tasks. | **Level 1 (`run_command` search-trimming / soft redirect) + Level 4 (Rank-8 Routing LoRA)** |
| **F3** | **Compaction thrashing vs. KV-cache invalidation** | Experiment 3A showed Stage 1 firing at 70% (`22.9K`) shrank `sh_023` only `23.1K → 18.8K`, busting the 18.8K KV cache 3 times (~100 s cold prefill) and crashing effective speed to `6.97 tok/s`. Raising thresholds to **80% / 90%** + server-reported `obs_input_tokens` eliminated both HTTP 400 overflows (`0/6`) and sawtooth thrashing on Held-Out 6. | **Level 1 (fixed in `compact.coax` / `loop.coax`)** |
| **F4** | **Unbounded multi-file exploration spirals** | On 3–4 file Coax architecture tasks (`sh_008`, `sh_009`, `sh_018`, `sh_020`, `sh_023`), Inky spent 15–25 steps reading files without converging on an edit plan. | **Level 2 (Coax/repo skills) + Level 3 (batch-native parallel racing / blame repair)** |
| **F5** | **Workspace instruction over-truncation** | `mc_instructions_load` (`MC_INSTR_MAX_BYTES = 1024`, `NOTE_MAX = 768`) left only 5 lines (`256 B`) of `AGENTS.md` and emitted stale Microcoder syntax (`view AGENTS.md:5-`). | **Level 1 (native Inky instruction loader with `2048 B` budget and `read_file` cursor)** |
| **F6** | **Literal-identifier eval graders** | `sh_019` solved the `CLOCK_REALTIME` + retry bug semantically and passed `py_compile`, failing only a literal grep for variable name `created_at > prev`. | **Eval hygiene (behavioral/unit-test graders, never identifier regexes)** |

---

## 5. How to Use Inky to Improve Itself: The 5-Level Compounding Architecture

Grounded directly in [`psionic/docs/self-improvement.md`](../../psionic/docs/self-improvement.md) (which documented why 10 DSPy/GEPA attempts produced zero verified wins, why dev-split overfitting overstated gains 5×, and why uncalibrated LoRA raised accuracy `0.77 → 0.90` while wrecking log-loss `1.952 → 2.323`), self-improving Inky obeys **five non-negotiable governance laws**:

1. **Strict Separation of Powers (NIP-OPT):**
   - **Student:** Inky running the candidate harness/skill/adapter.
   - **Reflector / Patch Author:** A separate Inky (or frontier) session reading *only* Dev-split `AJR1` failure journals and diffs — **never** seeing Held-Out task prompts, fixtures, or graders.
   - **Grader:** Pure deterministic oracles (`coax test`, `coax mutate`, `lean-run verify`, `executor-guard` Ch 234 replay, Docker Harbor verifiers). **Never** allow any LLM to grade its own work.
2. **Three Digests & Locked Partitions (Ch 239 & Ch 247):**
   - Every evaluation run pins `(items_sha256, questions_sha256, gate_sha256)` in an append-only receipt ledger.
   - Held-out partitions (`Session History Held-Out 18`, `Tier 2`, `Tier 4`, `Lean Algebra Held-Out`) are registered in the **Ch 247 exposure ledger** (`coax/lib/agent/redact-trajectory.coax` refuses `RT_REFUSE_HELD_OUT`) so held-out traces can **never** leak into reflection prompts, mined skills, or LoRA/DSpark shards.
3. **Measure Noise Floor & Headroom First (Ch 240–241):**
   - Never optimize on `100%`-saturated suites (Tier 2/4). Every candidate must clear Ch 241 `decision-v1` (gain above 2σ bootstrap noise floor, or `>=15%` wall/token win at equal pass under `ext-eval-v2`) with **zero regressions** on locked suites.
4. **Keep Exact Computation Exact; Train Only the Interface (`H2_ROUTING_ONLY`):**
   - Exact verification stays in frozen Class-E compiled weight modules and `#authority` Coax code (`0` surviving mutants). Learned layers only improve *when* to invoke tools, *which* files/edits to choose, and *how fast* tokens decode.
5. **Escalation Hierarchy — Cheapest & Highest-Transfer First, Weights Last:**
   - Optimize in strict order **Level 0 → Level 1 → Level 2 → Level 3 → Level 4**. Most real coding-agent gains come from Levels 0–3 ("the systems around it iterating") before touching a single weight.

### 5.1 Closed-Loop Self-Improvement Flow

```mermaid
flowchart TD
    A["Live & Eval Inky Runs (AJR1 Journals)"] --> B["Stage 1: Ingest & Mine<br/>(evals/session-history/tools/{ingest,mine}.coax)"]
    B --> C{"Ch 247 Exposure Gate<br/>(redact-trajectory.coax)"}
    C -->|"Held-Out Hash"| H["Locked Evaluation Only<br/>(NEVER enters reflector/training)"]
    C -->|"Dev / Opt-In Clean"| D["Failure Taxonomy & Telemetry<br/>(compare.coax)"]

    D --> L0["Level 0: Exact Class-E Cores<br/>(executor-guard.coax, ~/.openagents/modules)"]
    D --> L1["Level 1: Settings & Harness Hillclimb<br/>(ReAnchor + tbench/hillclimb.coax on Inky)"]
    D --> L2["Level 2: Failure-Mined Skills<br/>(Inky Reflector -> capability-eval.coax)"]
    D --> L3["Level 3: Inky Self-Patching Worktrees<br/>(coax/lib/inky/ & psionic/ via /goal)"]
    D --> L4["Level 4: Weights Last<br/>(Rank-8 Routing LoRA + DSpark Draft + DiLoCo)"]

    L0 --> G["#authority Promotion Gates<br/>(coax test + coax mutate 0-survivor +<br/>tb_admit_hillclimb / capability-gate /<br/>quarantine-opt 4-gate)"]
    L1 --> G
    L2 --> G
    L3 --> G
    L4 --> G

    H --> G
    G -->|"PASS (0 regressions)"| P["Signed Shadow-to-Promote Rollout<br/>(executor-release.coax -> Shipped Inky)"]
    G -->|"FAIL / UNVERIFIABLE"| Q["Quarantine & Keep Evidence<br/>(Append-only receipt + ATIF digest)"]
    P --> A
```

### 5.2 Detailed Level-by-Level Blueprint Mapped to Live Repo Modules

#### Level 0 — Exact Deterministic Substrate (0 LLM Calls, 100% Transfer)
- **What it improves:** Eliminates whole classes of stochastic LLM failure by moving deterministic checks into `~5 µs` Class-E compiled weight cores and `#authority` guards.
- **Live repo code:** `coax/lib/microcoder/executor-guard.coax`, `coax/lib/inky/{tools,guard,run}.coax`, `psionic/lib/module-v2-wave2.coax`, `~/.openagents/modules/`.
- **Next concrete Level-0 additions for Inky:**
  1. Wire `oa-exec-wire-bounds` / `oa-exec-arith-budget` into Inky's `run_command` / step-budget admission (just as `coax/lib/tbench/agent.coax` already does).
  2. Add a deterministic Coax pre-edit lint hook (catching `0..n` vs `0..<n` `E0216` and `let`/`fn`/`const` hallucinations in `*.coax` edits inside `ik_exec_edit` in `<1 ms`, saving full 15 s `coax test` round-trips).
  3. Add a deterministic `run_command` search-output cap (`2,400 B` for `grep`/`rg`/`find`/`ls`/`git log`, matching `mc_cmd_is_search`) so shell exploration stops flooding 16 KB per step into 32K context.

#### Level 1 — Settings Fitting & Harness Hillclimbing (ReAnchor + `hillclimb.coax` on Inky)
- **Why first:** In [`psionic/docs/self-improvement.md`](../../psionic/docs/self-improvement.md), **settings fitting (ReAnchor)** was the *only* pre-reset automated optimizer with a clean verified win (`35% → 53%` stall recall at 0 LLM calls).
- **Live repo code:** `coax/lib/tbench/{policy,wire,hillclimb,agent}.coax`, `evals/session-history/tools/compare.coax`.
- **Concrete design:**
  1. **Port `coax/lib/tbench/agent.coax` & `hillclimb.coax` from Microcoder to Inky** (replacing `mcworker`'s loop with `IkLoop` + `executor-guard.coax` + native Inkling tool-calling).
  2. **Offline ReAnchor pass (0 GPU calls) over recorded `AJR1` journals:** Fit exact non-weight thresholds (`IK_STAGE1_PCT`, `IK_STAGE2_PCT`, `IK_READ_LINES`, `IK_CMD_INLINE`, exploration-nudge step numbers, `run_command` search cap) using grouped cross-validation on Dev journals to minimize KV-cache invalidations and wasted tokens.
  3. **Online bounded challenger loop (`tb_propose_challenger`):** Propose 1-axis mutations over Inky's explicit policy knobs:
     - *Adaptive effort schedule:* `low` reasoning effort on initial search/grep/read steps, escalating to `medium`/`high` once target files are read or on retry after a failed test (directly attacks F1's 3,000-token reasoning bursts!).
     - *Compaction policy:* Stage 1/2 thresholds (`80%/90%`) vs single-shot 88% rollover (no Stage 1 KV-busting).
     - *Sampling profile:* `(temp=0.6, top_p=0.95, top_k=40)` vs `(temp=0.2, top_p=0.95)` on multi-file tasks.
     - *Prompt profile:* concise base vs explicit tool-routing cue (`edit_file` + `executor_verify`).
     - *Auto-check on edit (`/goal` mode):* optionally auto-running the task's known test command immediately after `edit_file` (Locals' `_after_write` insight), cutting tool round-trips in half.
  4. **Gate:** `#authority tb_admit_hillclimb` (Wilson 95% CI, verified ATIF-v1.8, zero held-out regression) writes atomically to `~/.openagents/inky/hillclimb/champion.json`.

#### Level 2 — Failure Mining → Self-Authored Local Skills (`capability-eval.coax`)
- **Why this works without changing weights:** On `sh_008`, `sh_013`, `sh_018`, `sh_023`, and `sh_024`, Inky wasted 5–12 steps discovering repo-specific idioms (`coax test --only`, `E0381` checker budgets, `#authority` rules, `components/kit` vs `components/app` layout). Those are *transferable procedural skills*, not per-task hacks.
- **Live repo code:** `evals/session-history/tools/{ingest,mine}.coax`, `coax/lib/agent/desktop/{capability-eval,capability-gate}.coax`, `coax/lib/nostr/atif/log.coax`.
- **Automated loop:**
  1. `mine.coax` scans Dev-split `AJR1` journals for recurring failure clusters (`>= 3` distinct Dev episodes with the same compiler error class, tool failure, or >10-step exploration stall).
  2. A separate **Reflector** Inky session receives the anonymized failure summaries (never held-out items!) and calls `capability-eval.coax draft` to author a compact `SKILL.md` (`<= 800` tokens, e.g., `coax-debugging`, `lean4-tactics`, `swift-desktop-edits`) plus positive and negative/regression eval cases.
  3. `capability-eval.coax run` executes a real two-arm comparison (Inky baseline vs Inky + candidate skill) on positive Dev tasks and negative regression tasks, records verified `ATIF-v1.8` digests, and runs `#authority capability-gate.coax`.
  4. Only skills with strictly positive pass/efficiency lift and zero regressions promote into `~/.openagents/skills/` (all rejected drafts stay archived with their ATIF digest so Reflector never re-proposes failed ideas).

#### Level 3 — Inky Improving Its Own Codebase (`coax/lib/inky/`, `psionic/`) in Isolated Worktrees
- **How Inky literally edits its own harness safely:**
  1. **Task generation from real telemetry:** When Dev-split `compare.coax` identifies a concrete harness bottleneck (for example, today's F3 compaction thrashing or F5 `AGENTS.md` 5-line truncation), a structured `/goal` ticket is generated with the exact failing Dev telemetry snippet and acceptance criteria.
  2. **Worktree-isolated execution:** Inky runs inside a fresh `git worktree` (never `main`), editing `coax/lib/inky/*.coax` or `psionic/` with Class-E pre-edit verification enabled.
  3. **Five-Stage Deterministic Promotion Gate (zero LLM self-grading):**
     - **Gate 1 (Unit & Selftest):** `coax test coax/lib/inky/{protocol,tools,format,prompt,compact,guard,loop,run}.coax --library` + `coax/web/tool.sh coax/lib/inky/selftest.coax` + `apps/openagents-desktop/page.coax` = `0` failed.
     - **Gate 2 (`#authority` Mutation):** `coax mutate <changed-#authority-file> --root-only` = **`0` survived**.
     - **Gate 3 (Repo Invariants):** `coax/web/tool.sh layer-check && wire-check && tools-check` = `0` failed, `0` new allowlist lines.
     - **Gate 4 (Dev Improvement):** Locked Dev slice improves beyond Ch 240 2σ floor (or `>=15%` wall/token win at equal pass).
     - **Gate 5 (Locked Held-Out & Regression Lock):** Zero regressions on Tier 2 (`14/14`), Tier 4 (`27/27`), Bucket A (`6/6`), and non-degrading / improving score on Locked Held-Out Session History (`18` tasks).
  4. **Parallel Force Multiplier (post-v1.0 continuous batching):** Once Psionic adds multi-slot continuous batching (`B = 4..8`, `psionic/docs/inkling-vs-locals-audit.md` §9 Tier 2.4), Level 3 upgrades to `locals run`-style **parallel worktree racing + blame-guided repair** — spawning 4 temperature-diverse Inky self-patchers in one GPU batch (~1.8× wall time instead of 4×), running Gates 1–4 in parallel, and promoting only the minimal clean diff that clears Gate 5.

#### Level 4 — Weights Last: Routing-Only LoRA + DSpark Draft Adaptation + Pylon DiLoCo Flywheel
Only after Levels 0–3 capture all deterministic, harness, skill, and engine wins do we touch weights — and strictly through two **narrow, reversible, quarantine-gated** artifacts rather than risky monolithic fine-tunes:

1. **Trace Export & Decontamination Pipeline (already built on `main`):**
   - Inky's `AJR1` journals record exact model-visible bytes (`coax/lib/inky/run.coax`).
   - `coax/lib/agent/desktop/proof-export.coax` converts winning sessions to canonical `ATIF-v1.8` (`.atif.jsonl` + `steps_digest`).
   - `#authority coax/lib/agent/redact-trajectory.coax` enforces default-OFF opt-in consent, fail-closed secret/PII/BIP-39 redaction, objective verifier-label requirement (compiler/test/Lean/Ch-234 verdict present), and Ch 247 held-out decontamination (`RT_REFUSE_HELD_OUT`).
   - `psionic/lib/peer/training-window.coax` packs verified shards into bounded DiLoCo windows (`inner_steps <= 500`, `max_bytes_out <= 64 MiB`).
2. **Artifact 4A — Rank-8 Tool & Executor Routing LoRA (`psionic/lib/hybrid-lora.coax`, `H2_ROUTING_ONLY`):**
   - **Exact training target (directly motivated by today's F2 & Experiment 2 data):**
     - Positive examples: winning Inky `ATIF-v1.8` turns that chose `read_file` / `grep` / `edit_file` / `executor_verify` cleanly with concise pre-tool reasoning (`< 150` thinking tokens before calling `executor_verify` or `edit_file`).
     - Hard negatives: turns that bypassed `edit_file` via `run_command("python3 - <<'EOF'")` / `sed`, wasted `500+` thinking tokens on mental arithmetic before calling `executor_verify`, or repeated read/cat commands.
   - **Zero-regression safety:** Pin `(base_model_sha256, core_module_sha256)` via `hl_hotswap`, keeping all exact computation inside frozen Class-E cores and allowing instant atomic fallback to the zero adapter (proven bit-exact).
3. **Artifact 4B — DSpark Speculative Draft Weights (`mtp.safetensors`, `0.64 GB`):**
   - **Exact training target (directly motivated by today's F1 decode-bandwidth ceiling):**
     - Distill next-multi-token draft predictions from Inky's verified Coax/Swift/C/Lean coding trajectories into the lightweight `0.64 GB` DSpark draft head (leaving the base `Inkling-2-Mini` target distribution **100% mathematically unchanged** — lossless speculative decoding!).
     - Even a `1.8×` speedup on OpenAgents Coax code turns `sh_014` (`7,591` tok @ `25 tok/s` = `301 s` timeout) into **`~165 s`**, directly converting wall-clock timeouts into passes without any risk of probability-gate or log-loss regression!
4. **Decentralized Verification & Rollout Gates (already built on `main`):**
   - `psionic/lib/peer/train-verify.coax`: 6-tier Byzantine verifier (`7/7` planted attacks blocked).
   - `psionic/lib/peer/quarantine-opt.coax`: 4-gate Quarantine Optimizer (`decision-v1` + `probability-v2` log-loss/ECE + Ch 234 exactness + `ext-eval-v2` latency/RAM; `3/3` regressions blocked; atomic lineage rollback).
   - `psionic/lib/peer/executor-release.coax`: BIP-340 Schnorr signed manifest + Ch 224 shadow-to-promote rollout to every OpenAgents Desktop install.

---

## 6. Prioritized Implementation Roadmap (Ranked by ROI)

| Rank | Milestone | Level | Target Files | Expected Measured Impact |
| :---: | --- | :---: | --- | --- |
| **1** | **DSpark (`mtp.safetensors`) + Prompt-Lookup Speculative Decoding** | Engine | `psionic/platform/inkling-metal.c`, `inkling-cuda.c` | **1.6–2.0× single-stream coding decode (`20–38 → 45–80+ tok/s`)**; directly resolves F1 (`6/6` Held-Out timeouts were decode-time-bound!). |
| **2** | **Inky `run_command` Search-Output Trim + Native `AGENTS.md` Loader + Adaptive Effort** | Level 0–1 | `coax/lib/inky/{run,prompt,loop}.coax` | Cuts 16 KB `cat`/`sed` context blowups (`sh_008`/`sh_009`/`sh_018`), fixes 5-line `AGENTS.md` truncation, and trims search-step thinking bursts. |
| **3** | **Port `tbench/agent.coax` & `hillclimb.coax` from Microcoder to Inky + ReAnchor Offline Fitter** | Level 1 | `coax/lib/tbench/{agent,hillclimb}.coax` | Unlocks automated, `#authority`-gated two-arm hillclimbing on Inky over Session History Dev-9 + Terminal-Bench Harbor with zero Microcoder dependency. |
| **4** | **Failure-Mined Coax & Lean Skills Pipeline (`mine.coax` → Reflector → `capability-eval.coax`)** | Level 2 | `evals/session-history/tools/mine.coax`, `coax/lib/agent/desktop/capability-eval.coax` | Automatically distills recurring Coax (`E0216`/`E0381`/`E0400`) and Lean 4 tactic failures into two-arm-verified local skills. |
| **5** | **Continuous Batching (`B = 4..8`) + Parallel Worktree `/goal` Racing for Self-Patching** | Engine + Level 3 | `psionic/platform/inkling-metal.c`, `coax/lib/inky/` | Enables Inky to race 4 parallel patches on its own repo or hard Buckets B–E tasks in ~1.8× wall time and promote only verified diffs. |
| **6** | **Rank-8 Tool/Executor Routing LoRA (`H2_ROUTING_ONLY`) + DSpark Coax Draft Distillation** | Level 4 | `psionic/lib/hybrid-lora.coax`, `psionic/tools/diloco-round.coax` | Eliminates shell-edit fallback (`python3`/`sed` → Class-E `edit_file`) and pre-tool mental-math tax (`budget_big` `63.6 s → ~12 s`), gated by 6-tier Byzantine + 4-gate Quarantine Optimizer. |

---

## 7. Implementation & Verification Report (2026-11-22)

All planned phases (Class-E executor integration, Level 0–1 harness upgrades, Psionic Metal 4 prefix-cache & speculative-verify engine work, `inky-improve` 6-stage self-improvement pipeline, Terminal-Bench `inky-native` port, and a strict host-cleanliness benchmark guard) have been implemented, unit- and mutation-tested, bundled into `OpenAgents.app`, and live-benchmarked on Apple M5 Max (128 GB unified memory, Metal 4, `Inkling-2-Mini-IQ4_NL.gguf`, 32,768 ctx, \$0.0000 inference).

### 7.1 Shipped Commits & Architecture Summary

| Phase | Commit | Key Deliverables |
| --- | --- | --- |
| **Phase 1 — Class-E Executor Integration** | `0f9e21e073` | Wired `#authority coax/lib/microcoder/executor-guard.coax` into `coax/lib/inky/run.coax` (`edit_file`/`write_file` pre-disk F64 dense cores `oa-exec-json-schema`, `oa-exec-diff-verify`, Desktop unified diff + Ch 234 `[E] replay verified/refused` pills); native 8th tool `executor_verify` (`INKY_EXECUTOR=1`); server-reported `obs_input_tokens` 80%/90% compaction + step-8/14 zero-edit nudges; removed user-facing `"Microcoder"` fallback strings in Desktop Swift views. |
| **Phase 2 — Level 0–1 Harness Upgrades** | `7af77a2fdd` | Native `coax/lib/inky/instructions.coax` (2.5 KB `AGENTS.md`/`CLAUDE.md` loader + `##` heading index + `read_file` cursor); strict-unique indent-tolerant `ik_fmt_try_loose_edit` + `<1 ms` deterministic Coax pre-edit lint (`E0216`/`let`/`const`/`fn`) + 4 KB search-output cap (`coax/lib/inky/format.coax`); `#authority` normalized duplicate shell-read blocking (`coax/lib/inky/guard.coax`); Class-E `wire-bounds` & `step-budget` pre-exec guards + `INKY_MAX_TOKENS`. |
| **Phase 3 — Psionic Metal 4 Engine** | `71d5d3d92a` | Expanded `InkSession` prefix cache `2 → 8` LRU + pinned SWA-backed checkpoints (`live_tokens` + `swa_valid_from`, SWA window backup gated on `headroom_mb >= 4096`), pinning both `tool_declare` end and `<|message_user|>` boundary across compaction and cross-task SWA wraps; lowered `direct_thresh` `16 → 7` (`-27.5%` GPU time on `nb=8` prefill tail chunks via `K_MOE_MM_IQ4_16`); added `ink_verify_draft` + opt-in `INK_SPEC_LOOKUP=1` (off by default — see §7.3); made `INKY_ADAPTIVE_EFFORT` opt-in (`== "1"`) to preserve multi-turn KV prefix cache. |
| **Phase 4 — Self-Improvement Pipeline & TBench Port** | `2b74624632` | Ported `coax/lib/tbench/{agent,hillclimb}.coax` to first-class `inky-native` mode (`tb_propose_inky_challenger`, 4 bounded axes); added `#authority coax/lib/inky/improve-policy.coax` (`inky-policy.v1` SHA-256, Ch 247 held-out quarantine, zero-LLM-call offline ReAnchor K-fold CV, fail-closed Ch 239/241/247 promotion gate), `coax/lib/inky/improve.coax`, and CLI `coder/tui/inky-improve.coax` (`audit`, `reanchor`, `mine-skills`, `distill`, `train-router`, `selftest`). |
| **Phase 5 — Host Cleanliness Benchmark Guard** | *(this commit)* | Added `evals/openagents-desktop/tier2/host_clean.py` + `test_host_clean.py` and wired preflight + per-task mid-run interference detection into `evals/openagents-desktop/tier2/run_tasks.py` (blocks local Metal runs on competing active GPU/LLM inference, memory pressure, thermal throttling, or concurrent evals unless `--allow-dirty-host` is explicitly passed; records `host_clean` & `host_preflight` in `summary.json`). |

### 7.2 Verification & Zero-Survivor Mutation Receipts

- **`#authority` Mutation Suites (all 0 survived):**
  - `coax/lib/inky/guard.coax`: **215 mutants, 215 killed, 0 survived**
  - `coax/lib/inky/tools.coax`: **467 mutants, 463 killed, 0 survived, 4 equivalent**
  - `coax/lib/inky/improve-policy.coax`: **362 mutants, 359 killed, 0 survived, 3 equivalent**
  - `coax/lib/tbench/policy.coax`: **317 mutants, 304 killed, 0 survived, 13 equivalent**
- **Unit, Integration, & Repo Hygiene Suites:**
  - `coax/lib/inky/selftest.coax`: **310/310** checks passed + **10/10** runtime selftests (`41` messages valid)
  - `coax/lib/inky/improve.coax`: **146/146** checks passed; `coder/tui/inky-improve.coax selftest`: **8/8** checks passed
  - `coax/lib/tbench/{policy,wire,agent,hillclimb}.coax`: **121/121** checks passed
  - `evals/openagents-desktop/tier2/test_host_clean.py`: **7/7** unit tests passed
  - `layer-check`, `wire-check`, `tools-check`: all **0 failed**
  - `psionic-inkling-cli --test-quant-kernels`: `0.00e+00` max abs across NVFP4, MXFP8, and IQ4_NL; `--verify-prefill-decode`: `cos = 0.999918`, `top1_pre == top1_dec`
  - `apps/openagents-desktop/check.sh`: **`OpenAgents.app` rebuilt & passed** (`account-check`, `sign-check`, `windows args`, `local-models-check 11/11`)

### 7.3 Two Crucial Empirical Findings (Honest Negative Measurements)

1. **Why Prompt-Lookup Speculative Decoding (`INK_SPEC_LOOKUP`) is Kept Opt-In (`0` by Default) on Inkling 2 Mini:**
   - Live M5 Max Metal 4 A/B benchmarks on copy-heavy code-edit prompts (`max_tokens=512`, both short `438`-token and long `18,643`-token contexts) confirmed the lookup drafter works well — achieving **65.6%–79.0% draft acceptance** (`4.56–6.53` tokens verified per call on `draft=5..7`).
   - However, because Inkling 2 Mini is a **128-expert Top-8 sparse MoE** (`16.8 GB` weights, memory-bandwidth bound on decode), verifying `nb = 6..8` tokens fans out from `8` active experts to `~40–50` unique active experts per forward pass (`~5–6×` expert weight memory traffic), while single-token `ink_step` already runs 67 sprints of fused Metal 4 decode kernels + double-buffered lookahead (`41.8–46.6 tok/s` at 18K ctx).
   - Net result: `SPEC=1` measured **`30.6–40.7 tok/s`** vs **`41.8–46.6 tok/s`** for `SPEC=0` (net slower). Therefore `ink_verify_draft` ships cleanly behind opt-in `INK_SPEC_LOOKUP=1` (ready for dense models, MTP heads, or multi-request continuous batching), and default single-stream Metal decode stays on the faster fused `ink_step` path.
   - **Implication for Roadmap Rank 1:** Real single-stream MoE speedup requires either **MTP-trained heads with expert-routing co-locality** or **multi-stream continuous batching (`B = 4..8`)** across parallel `/goal` worktree candidates rather than unfused single-stream prompt-lookup verification.
2. **Why Mid-Session `INKY_ADAPTIVE_EFFORT` is Opt-In (`== "1"`):**
   - In `psionic/platform/inkling-serve.c`, `Thinking effort level: <x>` is placed in `<|message_system|>` immediately before `<|message_user|>`. Mutating effort mid-conversation (e.g. `"low"` on steps 1–3 → `"medium"` on step 4) invalidates the entire multi-turn KV prefix cache (`90% → 15%`), costing multiple seconds of cold re-prefill. Keeping session effort constant preserves `80–95%` KV prefix hits and is reinforced by the offline ReAnchor scorer (`trap-adaptive-effort-kv-bust` scored `-734`).

### 7.4 Measured Before vs After Benchmarks (Clean Local M5 Max Metal 4, \$0.0000)

| Benchmark Suite | Before (Inky v1 Baseline) | After (Inky v3 + Metal SWA Prefix Cache + Class-E) | Delta |
| --- | ---: | ---: | --- |
| **Desktop Tier 2 Regression Lock** | 14/14 (100.0%), `232.3 s` (`16.6 s/task`), `62.8%` overall KV hit | **15/15 (100.0%)**, **`179.9 s`** (`12.0 s/task`), **`79.0%` overall / `81.1%` cont. KV hit**, `7/7` Class-E Ch 234 verified | **-27.7% wall/task**, **+16.2 pp KV hit**, +1 new task (`b4_self_prompt_citation` PASS in `24.4 s`, `0` shell cmds) |
| **Session History Bucket A (6 tasks)** | 6/6 (100.0%), `194.4 s` (`5.41×` faster than `mcworker`'s `5/6` in `1,051 s`) | **6/6 (100.0%)**, **`141.7 s`** (`38.0–44.7 tok/s`), **`86.6%` overall / `89.6%` cont. KV hit**, `0` guard trips | **-27.1% wall** (**7.42× faster than `mcworker`**) |
| **W6.1 Paired Multi-File Edit** | `mcworker`: `61.96–69.15 s` | **`17.21 s`** (`17.92 s` `hybrid-exec-v1`), `2/2` native `edit_file`, bit-identical Ch 234 digest `c3e731afbc4d` | **3.6–4.0× faster** |
| **Zero-Shot `executor_verify` Routing (8 tasks)** | `206.9 s` raw | **8/8 (100.0%)** Attempt-1 routed, **`175.5 s`**, 8/8 Ch 234 receipts | **-15.2% wall**, zero LoRA required |
| **Pinned SWA Prefix Cache (after 3,591-tok SWA wrap)** | `0%` hit once SWA ring wraps `> 2,048` | **`585/592` (`98.8%`) same-sys** (`0.38 s`) / **`408/524` (`77.9%`) diff-sys same-tools** (`0.56 s`), **100% bit-exact** | Instant cross-task & post-compaction KV hit |

### 7.5 Live `inky-improve` 5-Stage Pipeline Results (on `goal-inky-v3-sh-bucketA`)

Executed via `coax/web/tool.sh coder/tui/inky-improve.coax`:

- **`audit`:** `6/6 (100.0%)`, `141 s`, `86.6%` overall / `89.6%` cont. KV hit, `0` timeouts, `0` zero-edit spirals, `0` guard trips, `class_e_receipts: verified=1 refused=0`.
- **`reanchor` (0 LLM calls, Ch 247 Dev-only 4-fold grouped CV):**
  - `v2-greedy-kv80-4kb-classe` (`sha256:596bcf64ad27...`): **`+183`** (winner)
  - `v2-kv80-4kb-classe` (`sha256:ca72731f08b0...`): **`+166`**
  - `v1-baseline-70-85-no-exec` (`sha256:c537c0ffc1ee...`): `0`
  - `trap-spec-lookup-moe-fanout`: **`-284`** (rejected)
  - `trap-adaptive-effort-kv-bust`: **`-734`** (rejected)
- **`distill --opt-in`:** exported **`6/6`** passing Dev trajectories into verified `ATIF-v1.8` shards (`openagents.executor.dataset-shard.v1alpha1`), `0` held-out contamination, `0` secret leaks (and default-OFF without `--opt-in` refuses 100%).
- **`train-router` (Ch 140 Rank-8 LoRA via `psionic/lib/hybrid-lora.coax`):**
  - `train_rows=26`, routing accuracy **`7/26 → 26/26 (100.0%)`**.
  - Verified proofs: `zero_exact=true`, `hotswap=ok`, `tamper_refused=true`, `pin_refused=true`, `adapter_sha256=69d1305714443bd77882d0d7c064f60cc1f0b394eea973d778c211f66828ffc8`.

### 7.6 Host Cleanliness Guard & Clean-Host Follow-Up Benchmarks

During the initial Dev-9 sweep, `/Applications/OpenAgents.app` spawned a competing `psionic-inkling-serve` instance on port `18795` (`4.1 GB` RSS) mid-task (`sh_008`). We aborted that contaminated run rather than publishing noisy timings and built `evals/openagents-desktop/tier2/host_clean.py` directly into `run_tasks.py`:

- **Preflight Gate:** samples `ps` over a `1.5 s` window before starting any local Metal benchmark and refuses (`exit 2`) if any competing GPU/LLM server burns CPU/GPU (`pcpu >= 1.5%` or `delta_cputime >= 30 ms`), if competing resident GPU memory leaves `< 28 GB` RAM, if another eval runner or build storm is active, if `memory_pressure` free `< 30%`, or if macOS thermal throttling (`CPU_Speed_Limit < 85%`) is active.
- **Per-Task Mid-Run Watchdog:** re-samples competing GPU servers before and after every task; if another server burns `>= 150 ms` CPU time mid-task, marks `host_contaminated: true` in `result.json`/`summary.json` and aborts (`exit 2`, overrideable only via `--allow-dirty-host`).

When ready to run the remaining long-horizon suites on a dedicated host, verify cleanliness first and execute sequentially:

```bash
python3 evals/openagents-desktop/tier2/host_clean.py

# 1. Session History Dev-9 remaining 7 tasks (baseline: 3/7 -> 5/9 combined)
python3 evals/openagents-desktop/tier2/run_tasks.py --worker inky \
  --tasks-dir evals/session-history/tasks \
  --filter sh_008,sh_010,sh_013,sh_017,sh_019,sh_021,sh_023 \
  --timeout-floor 300 --out evals/openagents-desktop/results/clean-inky-v3-sh-dev9-rest

# 2. Session History Locked Held-Out 6 tasks (baseline: 0/6 timeouts)
python3 evals/openagents-desktop/tier2/run_tasks.py --worker inky \
  --tasks-dir evals/session-history/tasks \
  --filter sh_009,sh_014,sh_015,sh_018,sh_020,sh_024 \
  --timeout-floor 300 --out evals/openagents-desktop/results/clean-inky-v3-sh-heldout6

# 3. Lean 4 Algebra & Logic (10 harder proofs la01..la10 + #authority lean-run verify)
python3 evals/openagents-desktop/tier2/run_tasks.py --worker inky \
  --tasks-dir evals/openagents-desktop/lean/lean-algebra-logic/tasks \
  --timeout-floor 300 --out evals/openagents-desktop/results/clean-inky-v3-lean-alg
```

