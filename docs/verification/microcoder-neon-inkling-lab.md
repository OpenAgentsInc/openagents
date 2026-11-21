# Microcoder Neon / Inkling / Two-Device Optimization Lab

> Living empirical engineering log comparing pure **Gemini 4 Neon** (Vertex AI), pure local **Inkling 2 Mini** (Apple M5 Max Metal 4, `$0.00`), second-device **Qwen3.5-9B** (`coderos-4080` NVIDIA RTX 4080 CUDA over Tailscale, `$0.00`), and **Neon + Inkling** multi-model combos on the stratified 27-task OpenAgents session-history coding benchmark (`evals/session-history/tasks/`, Buckets A–E).

## Methodology & Holdout Discipline

- **Dev slice (9 tasks, ~1–2 per bucket):** `sh_001`, `sh_004` (Bucket A: edit precision), `sh_008`, `sh_010` (Bucket B: multi-file), `sh_013`, `sh_017` (Bucket C: verification), `sh_019`, `sh_021` (Bucket D: error recovery), `sh_023` (Bucket E: long context).
- **Held-out slice (18 tasks):** Untouched during all dev iterations; evaluated only after dev iterations lock to verify zero overfitting.
- **Devices exercised:**
  - **Device 1 (macOS):** Apple M5 Max (`100.127.107.31`), local Metal 4 `llama-server` serving `Inkling-2-Mini-IQ4_NL.gguf` (`32K` ctx, `127.0.0.1:18794`) + Vertex AI `us` multi-region (`gemini-4-neon`).
  - **Device 2 (NixOS / CoderOS):** `coderos-4080` (`100.74.238.61` over Tailscale), NVIDIA RTX 4080 16 GB VRAM serving `Qwen3.5-9B-Q4_K_M.gguf` (`32K` ctx, CUDA 12.8, `100.74.238.61:18795`).

---

## Live API & Hardware Probes (Physics of the Problem)

1. **Vertex AI `gemini-4-neon` (`us` & `global`):**
   - Does **not** support explicit Vertex `cachedContents` (HTTP 400: `"Model gemini-4-neon does not support explicit context caching"`).
   - Does **not** support `thinkingLevel: "minimal"` (HTTP 400; `"low"` is minimum).
   - Implicit prompt cache hit rate on Vertex `us` averaged **2%–4%** across multi-step runs, meaning **75.5% of baseline Neon spend (`$1.38 / $1.83`) was uncached input tokens** at `$3.00/M` input and `$15.00/M` output/thinking.
   - Under tight `$0.12–$0.25` per-task budgets, baseline Microcoder's `10K–22K`-token prompts gave Neon only **3–5 steps** before hitting budget stops on all 6 failing tasks (`6/6` failures were `ended: stopped` due to budget!).
2. **Local `inkling-2-mini` (Apple M5 Max Metal 4, `$0.00`):**
   - Baseline failed 7/9 tasks due to four Microcoder handicaps:
     1. Missing `inkling-2-mini` / `hybrid-exec-v1` entries in `GW_WINDOWS` (`card.coax`) → unbounded 27K-token context crawl.
     2. `GW_NO_STRICT_SCHEMA` on `psionic` caused empty Step-1 replies and lost structured fields in fallback parsing.
     3. Step-0 seeding disabled on `local: true` runs (`mcworker.coax`).
     4. `UINSTR_DEFAULT` (`user-instructions.coax`) appended `"push the commit"` at the tail of the system prompt, causing 9B models to trip `git push` safety guards after solving tasks.
     5. Placing `"view PATH:A-B"` inside `commands[]` executed `/usr/bin/view` (interactive vim), hanging 120 seconds until timeout!

---

## Dev-9 Empirical Progression (Observed Runs)

### Neon-Only (`gemini-4-neon`, Vertex AI)

| Run | Pass Rate | Total Cost | Wall | Bucket A | Bucket B | Bucket C | Bucket D | Bucket E | Notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| **`base-neon-dev9`** (baseline) | **3/9 (33%)** | `$1.8279` | 381s | 2/2 (`$0.2670`) | 0/2 | 0/2 | 1/2 | 0/1 | 6/6 failures budget-stopped (`sh_021` cmd passed) |
| **`iter1-neon-dev9`** | 3/9 (33%) | `$1.5543` (`-15%`) | 375s | 2/2 (`$0.1620`) | 0/2 | 0/2 | 1/2 | 0/1 | 34–44% cost cut on solved tasks; self-compaction bug found |
| **`iter1b-neon-dev9`** | 2/9 (22%) | `$1.5707` (`-14%`) | 385s | 2/2 (`$0.1615`) | 0/2 | 0/2 | 0/2 | 0/1 | `min_steps=2` fixed self-compaction; `sh_010`/`sh_021` cmd passed |
| **`iter2-neon-dev9`** | **4/9 (44%)** | `$1.5835` (`-13%`) | 478s | 2/2 (`$0.1374`) | 0/2 | 0/2 | **2/2** | 0/1 | `wm_task_tokens_within` unlocked `sh_021`; `sh_008` & `sh_010` cmd passed (6/9 code solved!) |
| **`iter2b-neon-dev9`** | **4/9 (44%)** | **`$1.4481` (`-20.8%`)** | **379s** | **2/2 (`$0.1224`, `-54%`)** | 0/2 | 0/2 | **2/2 (`$0.2209`)** | 0/1 | `sh_001` `$0.0636` (`-52%`), `sh_004` `$0.0587` (`-56%`), `sh_019` `$0.1252` (`-26%`), `sh_021` `$0.0956` (`-33%`); 0 search_bloat |

### Local Single-Device Baselines (`$0.00` Cost)

| Run | Device & Model | Pass Rate | Cost | Wall | Highlights |
| --- | --- | --- | --- | --- | --- |
| **`base-inkling-dev9`** | M5 Max Metal 4 · `inkling-2-mini` | **2/9 (22%)** | `$0.0000` | 2098s | `sh_001` (103s), `sh_004` (98s); `sh_010` cmd passed (timeout); `sh_021` cmd passed (`git push` guard) |
| **`iter1-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | **2/9 (22%)** | `$0.0000` | 1539s | `sh_001` **37s**, `sh_004` **35s** (2.8× faster than M5 Max); `sh_008` hit pre-fix 32K ctx truncation |
| **`iter2b-inkling-dev9`** | M5 Max Metal 4 · `inkling-2-mini` | *(in progress)* | `$0.0000` | — | `sh_004` dropped to **66.8s** (`-32%`); zero `git push` trips |
| **`iter2b-4080-qwen35-dev9`** | `coderos-4080` RTX 4080 CUDA · `qwen35-9b` | *(in progress)* | `$0.0000` | — | **`sh_010` (3-file Bucket B multi-file task) PASSED in 158s, 5 att, `$0.0000`!** |

---

## Implemented Architecture Improvements (Checkpoint #1)

1. **Universal Microcoder Core (`coax/lib/microcoder/`, `coax/lib/agent/workmem/`, `coax/lib/gateway/`):**
   - **Token-subsequence check matching (`workmem/task.coax`):** `wm_task_tokens_within` recognizes verification commands with inserted flags (e.g. `cc -fsyntax-only -I... stub.c`) or path-prefixed executables (`coax/bin/coax test --library ...`), eliminating false finish-refusal loops (`sh_021`).
   - **Symbol-anchored Step-0 seeding (`run.coax`):** Up to 4 task-mentioned files (`MC_SEED_BUDGET = 8192`, skipping `AGENTS.md`/`CLAUDE.md`) centered on task symbols for files >80 lines; enabled for all live local runs (preserving `GW_REPLAY` C16 determinism).
   - **Cache-adaptive rolling compaction (`state.coax`, `run.coax`):** `mc_compact_keep_steps(st, keep, min_steps)` compacts older steps on `<25%` cache-hit paid cloud endpoints while preserving the newest 2 closed steps verbatim (zero self-compaction or multi-file re-view amnesia) and leaving local Metal/CUDA KV caches intact.
   - **Passing-check output trimmer (`run.coax`):** Spills full passing test output (`exit == 0 && MC_CMD_CHECK`) to disk while keeping a 1,200 B summary tail in prompt state; failing tests retain full diagnostics.
   - **Lean system prompt & Coax cue (`loop.coax`, `instructions.coax`, `run.coax`):** Cut static prompt overhead ~1,200 tokens/call, added a 1-line Coax syntax cue inside Coax repos, and appended `MC_SAFETY_TAIL` after user/workspace instructions so local models never run unasked `git push`.
   - **Local strict GBNF JSON schema & 32K context window (`card.coax`, `psionic.coax`, `microcoder_keys.coax`, `text_action.coax`):** Added `inkling-2-mini`/`hybrid-exec-v1` 32K windows, strict `json_schema` primary with fallback, full field preservation in `mc_text_action_as`, and `length`-truncation auto-compaction.
   - **`/usr/bin/view` vim-hang interceptor (`run.coax`):** `mc_cmd_view_arg` intercepts `"view PATH[:A-B]"` placed in `commands[]` and satisfies it directly via `mc_run_view` in <1 ms instead of hanging 120 s on interactive vim.
   - **2-step read-only edit nudge (`aware.coax`):** Nudges agents to apply their first edit while steps 1–2 remain verbatim in context.
2. **Multi-Model Neon + Inkling Combos & Two-Device Pooling (Iteration 3):**
   - **Combo A — `inkling-neon` / `hybrid-neon` (Dual-Plan Step-Routed Hybrid in `run.coax`, `mcworker.coax`, `apps/openagents-desktop/page/`):** Starts on free local Inkling/CUDA (`$0.00`) for up to `MC_HYBRID_LOCAL_STEPS` (default 3) or `MC_HYBRID_LOCAL_MS` (default 75 s); solves easy tasks for `$0.00`; escalates seamlessly to Vertex `gemini-4-neon` on step/time cap, bad reply, or edit/check failure with gathered context intact; routes the verified closing summary turn back to free local Inkling (`$0.00`) once checks pass with zero open items.
   - **Combo B — `neon+inkling` (Free Local `level: "inkling"` Sub-Agents in `action.coax`, `loop.coax`, `mcworker.coax`, `page/state.coax`, `page/transcript.coax`):** Exposes `level: "inkling"` to cloud runs whenever a local/LAN psionic server is reachable, offloading file search/reading/checks to `$0.00` local GPUs.
   - **Two-Device `PSIONIC_PEERS` Pooling (`psionic.coax`, `microcoder_keys.coax`):** Supports comma-separated `URL#MODEL` endpoints (e.g. `http://100.74.238.61:18795#qwen35-9b,http://127.0.0.1:18794#inkling-2-mini`), distributing local runs and sub-agents across Apple M5 Max Metal 4 and `coderos-4080` RTX 4080 CUDA.

## Verification Status (Checkpoint #1)

- `coax/bin/coax test coax/lib/agent/workmem/task.coax --library`: **4/4 passed**
- `coax/bin/coax test coax/lib/gateway/psionic.coax --library`: **passed**
- `coax/bin/coax test coax/lib/microcoder/loop.coax --library`: **247/247 passed**
- `coax/bin/coax test coax/lib/microcoder/run.coax --library`: **448/448 passed**
- `coax/bin/coax test coder/tui/microcoder_keys.coax --library`: **466/466 passed**
- `coax/bin/coax test apps/openagents-desktop/page.coax --library`: **836/836 passed**
- `evals/openagents-desktop/run.sh --tier 0 --tier 1` (fixtures, replay 846/846, host scenarios 20/20, followups 12/12, compaction C1–C16 486/486 + `aware-worker.coax`): **5/5 passed, 0 failed**
- `coax/web/tool.sh tools-check`, `wire-check`, `layer-check`: **0 failed**
- `openagents contribute selftest`: **PASS (`default_off=ok adversarial_redaction=clean verifier_gate=ok heldout_block=ok`)**

---

*Next sections (appended after Iteration 3 live combo benchmarks, two-device Pylon `cj-executor`/`cj-validate` runs, and full 27-task dev+held-out verification):*
- *Iteration 3 Dev-9 Combo Scorecards (`inkling-neon`, `rtx4080-neon`, `neon+inkling` delegation, `hybrid-exec-v1`)*
- *Two-Device Pylon Executor Network & Trajectory Contribution Verification*
- *Final 27-Task Full Benchmark Scorecard (Dev-9 vs. Held-Out-18)*
