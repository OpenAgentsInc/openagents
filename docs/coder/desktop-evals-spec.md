# OpenAgents Desktop Evaluation Specification

**Status:** Implemented (updated for single-model Inkling 2 Mini v1.0 release on native `inky` worker)  
**Target product:** `apps/openagents-desktop` (macOS/Linux/Windows native host + Coax GPU page + `inky` native Inkling 2.0 worker + shipped `psionic-inkling-serve` Metal runtime)  
**Related docs:** [`apps/openagents-desktop/README.md`](../../apps/openagents-desktop/README.md), [`evals/openagents-desktop/README.md`](../../evals/openagents-desktop/README.md), [`inky-spec.md`](inky-spec.md), [`../verification/inky-vs-microcoder-qualification.md`](../verification/inky-vs-microcoder-qualification.md), [`claude-desktop-parity-process.md`](claude-desktop-parity-process.md), [`evaluation-qualification.md`](evaluation-qualification.md), [`microcoder.md`](microcoder.md) *(deprecated)*, [`../desktop/macos-drive-and-screenshots.md`](../desktop/macos-drive-and-screenshots.md)

---

## 1. Goals and Scope

This specification defines a tiered, prioritized evaluation program for **OpenAgents Desktop** (`apps/openagents-desktop`).

Its goals are to:

1. **Catch desktop-specific regressions before shipping** — especially around streaming UI, local Inkling 2 Mini inference on Psionic Metal (`--ctx 32768`), feature-flag gating, background chats, chat persistence, `/goal` workflows, and native host integration.
2. **Measure end-to-end coding effectiveness through the real desktop stack** — not only in a bare CLI harness.
3. **Provide fast, deterministic local feedback** for day-to-day changes alongside slower, higher-fidelity trajectory and native UI evaluations for releases and CI.
4. **Standardize artifacts and grading** so every eval run emits inspectable, comparable results tied to git SHA, engine/model, wall time, TTFT, decode tok/s, memory footprint, and worker protocol version.

### 1.1 Current Architecture Under Test

OpenAgents Desktop is split across four layers, each with distinct failure modes:

| Layer | Key Files | Responsibilities |
|---|---|---|
| **Pure UI & State Machine (Coax)** | `apps/openagents-desktop/page.coax`, `apps/openagents-desktop/page/*`, `coax/lib/agent/desktop/flags.coax`, `components/app/*`, `components/core/edit.coax` | Shared `#authority;` 28-flag feature registry (all off in single-model Inkling release), conversation state (`CP_TURNS = 24`), streaming Markdown + code blocks, thinking rows (`MWW_THOUGHT`), tool/command rows, first-run hardware/download onboarding, `/goal` lifecycle, sidebar recents + drag-reorderable pins, chat save/load (`> 1 MiB` support), hit-testing, GPU vertex emission, and developer-flagged sub-agent/budget/cloud surfaces. Zero side effects. |
| **C ABI Bridge** | `apps/openagents-desktop/host.h`, `apps/openagents-desktop/host.c`, `apps/openagents-desktop/app.coax` | Exposes `coder_page_*` state transitions, feature-flag & local-model state updates (`cd_flags_set`, `cd_local_state`), action dispatch (`CP_ASK_SEND`, `CP_ASK_STOP`, `CP_ASK_NEW`, `CP_ASK_OPEN`, `CP_ASK_GOAL`, `CP_ASK_REORDER_PIN`, etc.), and `cd_headless` page handoff. |
| **Native Host (macOS Swift / AppKit / Metal; Linux & Windows C)** | `apps/openagents-desktop/AppDelegate.swift`, `Window*.swift`, `Store.swift`, `Worker.swift`, `Worktree.swift`, `host-main.c` | Metal/Vulkan rendering, native input method integration, window resize/zoom (`CP_ZOOMS`), folder picker, disk persistence for chats/pins, background `cd_headless` pages for concurrent running chats, SHA-256 verified Inkling model management & `psionic-inkling-serve` lifecycle, and worker subprocess/daemon management. |
| **Worker Wire & Agent Loop** | `coax/lib/agent/engine/mcworker-wire.coax`, `coder/tui/inky.coax`, `coax/lib/inky/*`, `psionic/platform/inkling-serve.c` (legacy `--full` cloud comparison: `coder/tui/mcworker.coax`, deprecated) | Line-delimited JSON protocol (`mww_task`, `mww_answer`, `mww_results`, `MWW_RECALL`, `MWW_REPLY`, `MWW_THOUGHT`, `MWW_COMMAND`, `MWW_VIEW`, `MWW_ASK`, `MWW_DELEGATE`, `MWW_ATTEMPT`, `MWW_JOURNAL`, `MWW_ENDED`, `MWW_STATS`), native Inkling 2.0 tool calling with append-only KV-cache-preserving history and `<\|content_tool_error\|>` feedback on Psionic Metal (32K context), `#authority` sandbox/policy/guard enforcement, and SWA-aware 2-stage compaction. |

### 1.2 What Makes OpenAgents Desktop Distinct from a Bare CLI Agent

Existing CLI and benchmark harnesses (`docs/coder/evaluation-qualification.md`, `evals/terminal-bench/`, `evals/session-history/`, `coax/lib/microcoder/tools_smoke.sh`) evaluate model/tool capability in isolation, but miss desktop-critical behaviors:

- **Interactive streaming & layout integrity:** Incremental Markdown/code-fence streaming, word-wrapped thinking rows, copy buttons (`CP_COPIED_*`), sidebar/transcript-pane width negotiation (`CP_SIDE_FROM`, `CP_PANE_MAIN`), and frame-time responsiveness under high-volume tool output (up to `CP_AGENT_CALLS = 6000` calls).
- **Multi-chat & background execution:** Switching chats or opening a new chat while a turn is running (`cd_headless` / `coder_page_take`), preserving active-run indicators in the sidebar, and reattaching cleanly without dropping worker lines or corrupting state.
- **Persistence & recall fidelity:** Saving/loading large chats (`> 1 MiB`), preserving pinned order, budget overrides, `/goal` state, sub-agent transcripts, and sending accurate `MWW_RECALL` history to a fresh worker when reopening a chat.
- **Human-in-the-loop controls:** Command gating (`MWW_ASK` / `mww_answer`), mid-turn Stop (`MWW_STOP_LINE`), per-thread budget auto-stop + manual approval/increase, and quiet-loop pause (`CP_QUIET = 100`) handback to Microcoder.
- **Heterogeneous sub-agent fan-out:** Delegating parallel rounds (`MWW_DELEGATE`, up to 6 tasks/round) across Gemini (`hydrogen`, `carbon`, `neon`) and Claude Code (`haiku`, `sonnet`, `opus`, `fable`), tracking wait/retry indicators, and returning merged `mww_results`.
- **Workspace & OS integration:** Working folder switching, installed-skills prompt prefix injection (`CP_SKILLS`, hidden from user bubble), native editor focus handoff, clipboard copy fidelity (excluding thinking/sub-agent chatter from chat copy), and graceful recovery from a crashed worker process.

### 1.3 Existing Coverage & Gaps

What already exists today:
- **`apps/coder-desktop/page.coax` `#test` suite (~34 tests):** Strong unit coverage of pure page state, serialization, budget, `/goal`, sub-agent state, and layout hit-tests (`coax/bin/coax test apps/coder-desktop/page.coax --library`).
- **`coax/lib/agent/engine/mcworker-wire.coax` `#test` suite:** Wire codec serialization/deserialization tests.
- **`coax/lib/microcoder/tools_smoke.sh`:** Sandboxed tool smoke check.
- **`docs/coder/claude-desktop-parity-process.md`:** Visual/interaction parity process.
- **`docs/coder/evaluation-qualification.md`, `evals/terminal-bench/`, and `evals/session-history/`:** CLI-level coding qualification, Harbor Terminal-Bench harness, and 27-task Session History suite.

Key gaps:
- No **deterministic replay / mock-worker integration harness** testing `host.c` + `page.coax` (and `App.swift` host logic) end-to-end against scripted wire transcripts.
- No **headless desktop-protocol agent eval suite** exercising live `mcworker` + `claudeworker` under desktop `McTask` flags (`gated`, `edits`, `agents`, `claude`, `recall`, `budget`, `/goal`).
- No **automated native macOS E2E UI & visual/responsiveness evals** driving the compiled `.app` bundle.
- No **golden trajectory regression suite** or unified scoring/reporting pipeline for local and CI execution (`.github/workflows/` currently has no desktop eval jobs).

---

## 2. Proposed Eval Suites (Tiers)

Evals are organized into five tiers from fastest/most deterministic to broadest/most realistic.

### Tier 0 — Pure Component, Page State & Wire Codec Evals (Deterministic, `< 5s`)
- **Scope:** `apps/coder-desktop/page.coax`, `coax/lib/agent/engine/mcworker-wire.coax`, `components/app/*`, `coax/lib/microcoder/tools_smoke.sh`.
- **What it measures:**
  - Pure state-machine invariants: turn limits, streaming code fences, copy payloads, thought wrapping, sub-agent ring-buffer eviction (`6000` cap), quiet-loop pause, `$50` budget enforcement, `/goal` commands, pin drag-reorder, `> 1 MiB` save/load round-trips.
  - Layout & hit-test invariants across breakpoints (`< 900pt` no sidebar, `1200x800` standard, split-pane with transcript open, all 8 `CP_ZOOMS` steps).
  - Wire codec round-trip and malformed/unknown-field resilience.
- **Runtime:** Zero LLM calls, zero AppKit/Metal, runs on every edit.

### Tier 1 — Deterministic Host & Scripted-Worker Replay Evals (Deterministic, `< 30s`)
- **Scope:** `page.coax` + `host.c` + `App.swift` host/session layer paired with a **mock worker binary** that speaks `mcworker-wire` over stdin/stdout.
- **What it measures:**
  - **Protocol & lifecycle:** Startup (`MWW_READY`), streaming reply/thought/command/view interleaving, `MWW_ASK` gating and `mww_answer`, mid-stream Stop (`MWW_STOP_LINE`), and unexpected worker EOF/crash recovery.
  - **Concurrency & background chats:** Switching between chats while a mocked long-running turn streams events into a `cd_headless` page, opening a new chat concurrently, and verifying zero dropped lines and correct sidebar running-dot state.
  - **Persistence & recall:** Saving a chat to a temp data dir, restarting the host, reloading the chat, and asserting the exact `MWW_RECALL` payload sent on the next user turn.
  - **Sub-agent fan-out & edge cases:** Scripted `MWW_DELEGATE` rounds (mixed Gemini + Claude levels, >12 sub-agent cap refusal, 100-call quiet-loop pause, worker timeout/failure).
  - **Budget & `/goal` enforcement:** Scripted `MWW_ATTEMPT` cost events crossing `CP_BUDGET_DEFAULT` -> auto-stop -> manual override/increase -> continuation.
  - **Stress / responsiveness:** High-frequency event burst (e.g., 1,000 tool rows + 60 KiB Markdown reply) measuring page update time and vertex budget (`CP_MAX_VERTICES = 200000`) without overflow or frame stalls.
- **Runtime:** Zero LLM calls; deterministic seed and scripted timing.

### Tier 2 — Headless Live-Agent Desktop Harness Evals (Live Models, `5–15 min`)
- **Scope:** A headless runner driving `host.c` + `page.coax` against the real `inky` worker (`--worker inky`, default; deprecated `--worker mcworker` remains available for legacy comparison) in isolated temporary git repos. Defaults to the single-model **Inkling 2 Mini** release path on auto-managed `psionic-inkling-serve --ctx 32768` (`$0.00`, release flags `gated: true`, `edits: true`, `local: true`, `unlimited: true`, `model: "inkling-2-mini"`, `agents: false`). Sub-agent (`c1`–`c3`) and USD-budget (`d2`, `d3`) tasks are recorded as `not_applicable` (`N/A`) on single-model local Inkling and excluded from the pass-rate denominator. Cloud models (`--model <id>`) and `claudeworker` (`--claude`) remain opt-in for developer comparison.
- **What it measures:**
  - **Single-turn & multi-turn coding tasks:** Repo exploration, targeted multi-file edits, running verification commands, and respecting working-folder boundaries.
  - **Session & recall continuity:** Multi-turn tasks with a simulated chat reload (`coder_page_save` -> new worker -> `coder_page_load` -> `MWW_RECALL`) to verify the agent retains context and finishes accurately.
  - **`/goal` completion & safety reliability:** Multi-turn `/goal` runs verifying prompt injection and continuation until completion, unambiguous edit application, command exit-code handling, no runaway loops, and adherence to sandbox/policy roots.
  - **Local runtime efficiency:** Server boot time, peak RSS, per-task wall time (p50/p95), TTFT, and decode tok/s.
  - **Opt-in multi-model checks:** Sub-agent delegation (`c1`–`c3`) and USD budget caps (`d2`, `d3`) when running with an explicit cloud `--model`.
- **Runtime:** Local Inkling 2 Mini on Psionic Metal (`$0.00`); run on-demand locally, nightly, and before releases.

### Tier 3 — Native macOS Desktop E2E, UI & Visual Evals (macOS GUI, `2–5 min`)
- **Scope:** The built `OpenAgents.app` (`apps/openagents-desktop/build.sh`) launched with an isolated `--data-dir` / environment pointing at either the Tier 1 mock worker (deterministic UI E2E) or a small live-model smoke set.
- **What it measures:**
  - **Real AppKit/Metal integration:** Launch-to-ready latency, Metal rendering without blank frames or crashes, native editor focus/typing/paste/Cmd+Enter, Cmd+`/`-` zoom, sidebar resize drag, pin drag-and-drop, folder picker hook, and copy-to-pasteboard verification.
  - **Visual & layout parity:** Deterministic screenshot capture (building on `docs/desktop/macos-drive-and-screenshots.md` and `docs/coder/claude-desktop-parity-process.md`) at canonical sizes (`1200x800`, `820x700` narrow/no-sidebar, `1440x900` with sub-agent transcript pane open) checked for clipping, overlap, missing glyphs, and visual diff against approved baselines.
  - **UI responsiveness under load:** Frame-time p50/p95/p99 during rapid streaming and scroll-while-streaming.
- **Runtime:** Requires macOS GUI session (local dev machine or dedicated macOS runner).

### Tier 4 — Golden Trajectories & Real-World Coding Benchmarks (Release / Weekly, `15–60 min`)
- **Scope:** Curated real-world tasks executed through the Tier 2 desktop harness (defaulting to local Inkling 2 Mini on shipped `psionic-inkling-serve --ctx 32768`; sub-agent task `g2` marked `not_applicable` in single-model mode).
- **What it measures:**
  - **Golden regression trajectories:** Anonymized/reproducible tasks captured from real desktop bugs (e.g., broken streaming code fence, lost background chat output, failed multi-file edit, reload recall drop). Every P0/P1 desktop bug adds a Tier 1 deterministic replay case and, if model-behavioral, a Tier 4 golden task.
  - **Curated coding benchmark slice:** A fixed 25-task coding slice in `evals/openagents-desktop/tier4/tasks/` and `tier4/slice.json` (Python, shell, C, Swift, JavaScript, repo maintenance) scored by objective test suites, plus wall-clock latency, TTFT, decode tok/s, tokens, tool-call count, and turn count (alongside `evals/session-history/` for the 27-task release scorecard).
  - **Runtime / prompt / worker comparisons:** Controlled side-by-side runs (e.g., Psionic Metal vs. `llama-server` in Gym Benchmarks, or opt-in cloud arms) tracked over time.

---

## 3. Phased Implementation Order

The order below maximizes **signal-to-effort** by securing fast deterministic coverage first, reusing the same wire-protocol and artifact primitives across higher tiers, and deferring flaky/expensive GUI and large benchmark runs until the lower layers are rock-solid.

### Phase 1 — Tier 0 Hardening & Unified Eval Runner Skeleton (Week 1)
**Why first:** `page.coax` and `mcworker-wire.coax` already have fast `#test` suites, and `tools_smoke.sh` already exists, but they are invoked ad hoc and produce no structured summary. Unifying them gives immediate regression protection in `< 5s`.

- **Deliverables:**
  1. Single entrypoint script `evals/desktop/run.sh` (supporting `--tier 0|1|2|3|4`, `--filter`, `--out <dir>`) writing a standardized `summary.json`.
  2. Run `coax test apps/coder-desktop/page.coax --library`, `coax test coax/lib/agent/engine/mcworker-wire.coax --library`, and `coax/lib/microcoder/tools_smoke.sh` under Tier 0.
  3. Expand `page.coax` `#test` coverage for any untested layout/state edge cases (e.g., narrow-window `< 900pt` transitions, `CP_TURNS = 24` full-chat boundary, `CP_AGENTS = 12` cap, zoom + transcript-pane resize, malformed worker JSON resilience).
- **Success criteria:** `evals/desktop/run.sh --tier 0` passes in `< 10s` locally with 100% deterministic output and zero external dependencies.

### Phase 2 — Tier 1 Deterministic Host & Mock-Worker Replay Suite (Week 2)
**Why second:** Most desktop-specific bugs live at the seam between `page.coax`, `host.c`, `App.swift`, and `mcworker-wire` (streaming races, background `cd_headless` handoff, save/load + `MWW_RECALL`, budget auto-stop, worker crash). A mock worker tests all of these deterministically in seconds without spending tokens.

- **Deliverables:**
  1. Mock worker binary (`evals/desktop/mock_worker.py` or Coax tool) that replays JSONL `mcworker-wire` scenarios and records all owner->worker lines (`mww_task`, `mww_answer`, `mww_results`, `MWW_STOP_LINE`, `MWW_RECALL`) for assertion.
  2. Headless C/Swift test driver exercising `host.c` + `page.coax` against the mock worker (and testable session-manager hooks in `App.swift` if needed).
  3. Initial suite of **15–20 scripted scenarios** covering: basic streaming, code-fence chunking, `MWW_ASK` allow/deny, Stop mid-turn, worker crash/EOF, concurrent background chat (`cd_headless`), save/restart/load/`MWW_RECALL`, `$50` budget stop + override, `/goal` lifecycle, sub-agent parallel delegation + quiet-loop pause, and 1,000-row stress test.
- **Success criteria:** All Tier 1 scenarios run in `< 30s`, 0% flake rate across 20 consecutive runs, and vertex/memory/latency bounds are asserted.

### Phase 3 — Tier 2 Headless Live-Agent Desktop Harness (Weeks 3–4)
**Why third:** Once the desktop protocol seam is verified deterministically, we need to measure how real `mcworker` + `claudeworker` perform under actual desktop flags (`gated`, `edits`, `agents`, `claude`, `MWW_RECALL`, `/goal`, skills prefix).

- **Deliverables:**
  1. Workspace fixture runner that clones/copies isolated temp git repos, launches the headless desktop host with real `mcworker`, and captures full artifacts (`journal.jsonl`, `wire.jsonl`, `chat.saved`, `attempts.jsonl`, `git.diff`).
  2. Initial **15-task desktop agent suite** across 5 categories (3 tasks each): (a) single-turn code fix + test, (b) multi-turn with simulated chat reload (`MWW_RECALL`), (c) sub-agent delegation task, (d) `/goal` multi-step task + budget-cap test, (e) safety/sandbox & refusal behavior.
  3. Automated deterministic graders (unit test execution, diff checks, wire-event assertions, cost/turn/latency thresholds).
- **Success criteria:** Full Tier 2 suite runs end-to-end with one command, emits per-task pass/fail + cost/latency breakdown, and establishes an initial baseline report committed or archived.
- **Status (2026-11-19, #13703):** Built in `evals/openagents-desktop/tier2/` (`run.sh --tier 2`, opt-in; see `evals/openagents-desktop/README.md`). Baseline in `evals/openagents-desktop/tier2/baseline/`: 15/15 passed on Gemini 4 Neon with Hydrogen sub-agents, about $0.50 and 3 minutes a full run. The workspace diff artifact is named `workspace.diff`. `claudeworker` runs only with `--claude`.

### Phase 4 — Tier 3 Native macOS Desktop E2E & Visual Parity Suite (Weeks 5–6)
**Why fourth:** Driving a real macOS `.app` bundle has higher setup and maintenance cost; doing it after Tier 1 lets us reuse the exact Tier 1 mock worker so GUI tests are deterministic and fast, without LLM flakiness.

- **Deliverables:**
  1. Launch flags / env overrides in `Coder.app` for isolated test runs (e.g., `CODER_DATA_DIR`, `CODER_WORKER_BIN`, `CODER_INITIAL_CWD`, optional deterministic clock/animation disable).
  2. macOS GUI driver (using the tooling in `docs/desktop/macos-drive-and-screenshots.md`) running **8–10 E2E UI scenarios** against the mock worker: launch & type & send, streaming scroll & copy-code-block to pasteboard, sidebar collapse/resize, pin drag-reorder, background chat switch, sub-agent transcript pane open/close, budget modal/approval, and `/goal` UI.
  3. Automated screenshot capture + pixel/structural diff check for 4 canonical layouts (`1200x800` idle, `1200x800` active streaming + thinking, `1440x900` with sub-agent pane, `820x700` narrow) aligned with `docs/coder/claude-desktop-parity-process.md`.
  4. Frame-time telemetry capture during the streaming stress scenario.
- **Success criteria:** Tier 3 runs unattended on macOS in `< 5 min`, catches layout/focus/clipboard regressions, and stores before/after screenshots as artifacts on failure.
- **Status (2026-11-19, #13704):** Built in `evals/openagents-desktop/tier3/` (`run.sh --tier 3`, opt-in; see `evals/openagents-desktop/README.md`). No env seams were added to `Coder.app`: the runner stages a copy of the build with its own bundle id and the mock worker in place of `mcworker`, runs it under a temp `HOME`/`CFFIXED_USER_HOME`, and drives it through a probe dylib (`DYLD_INSERT_LIBRARIES`) with real `NSEvent`s. 7 flows and 15 goldens (idle, narrow, streaming, stopped, replied, new chat, sub-agent pane at 1440x900, sidebar resize, five settings pages, quit and reopen); about 75 s after the build. Not yet: pasteboard copy, pin drag-reorder, background chat switch, budget and `/goal` UI, frame-time telemetry.

### Phase 5 — Tier 4 Golden Trajectories, Benchmark Slice & CI/Nightly Automation (Weeks 7–8)
**Why fifth:** With all harness layers and artifact schemas proven, we scale to high-signal release gating, trend tracking, and continuous regression capture.

- **Deliverables:**
  1. **Golden regression policy & corpus:** Template and workflow to convert any P0/P1 desktop bug into a Tier 1 replay test (mandatory) and Tier 4 task (when model-behavioral).
  2. **25-task real-world coding benchmark slice** (`evals/openagents-desktop/tier4/tasks/` and `tier4/slice.json`) running through the Tier 2 desktop harness.
  3. **CI & scheduled workflows:** PR checks for Tier 0 + Tier 1, nightly macOS runner for Tier 2 + Tier 3, and weekly/pre-release Tier 4 benchmark + baseline diff report.
- **Success criteria:** Automated trend table tracking pass rate, cost/task, p95 latency, and regression count across commits and model releases.
- **Status (2026-11-19, #13705):** Built. `evals/openagents-desktop/tier4/` holds the golden template, 4 bug-derived golden tasks (#13729 recall cut, #13676 sub-agent read loop, the `/goal` continuation, #13706 compaction) and 20 benchmark tasks (Python, shell, C, Swift, JavaScript, repo maintenance); `tier4/slice.json` adds Tier 2's five coding tasks for the 25-task slice. `run.sh --tier 4` runs them through the Tier 2 runner (`run_tasks.py --manifest`), opt-in, about $1.20 a run (29/29 on the first run), and diffs against `tier4/baseline/`; `compare.py` gives the baseline diff and the trend table (`run.sh --compare`, `--trend`). There are no GitHub Actions in this repo: the green-main pipeline runs the page tests and the Tier 1 replay suite on every head that reaches Coder Desktop (`check.sh coder` / `coder-since`), and `evals/openagents-desktop/nightly.sh` runs the nightly (Tier 0, 1, 2, 3) and weekly (Tier 4) jobs on `origin/main-green` from a Mac's crontab (`nightly.sh install` prints the lines; nothing runs until they are added), with a 30-day spend cap and one issue for a bad run. The host scenarios and compaction suite run on Linux too (#13734; on NixOS `host_driver/build.sh` uses the wrapped C compiler, since the system clang has no C library headers), and the Linux real-window input test (`apps/openagents-desktop/linux/input-test.sh`) covers typing, Return, presses, the clipboard and a keyboard replug through a headless compositor. The golden regression policy is in `evals/openagents-desktop/README.md`.

---

## 4. Harness Architecture

```text
evals/openagents-desktop/
├── run.sh                      # Unified CLI entrypoint (--tier, --filter, --model, --out)
├── compare.py                  # Baseline diff & trend reporting (handles N/A tasks + local runtime metrics)
├── nightly.sh                  # Scheduled nightly/weekly macOS runner (local Inkling by default)
├── mock_worker/                # Deterministic mcworker-wire replay & assertion worker
├── host_driver/                # Headless C driver wrapping host.c + page.coax
├── tier1/                      # Scripted JSONL wire transcripts + 20 host scenarios
├── tier2/                      # Temp-repo tasks + verification scripts + graders + Inkling baseline
├── tier3/                      # Native OpenAgents.app UI scripts + visual baseline goldens
└── tier4/                      # Bug-derived golden trajectories + 25-task coding slice
```

### Design Principles

1. **Reuse production code paths:** Every tier above Tier 0 must exercise the real `mcworker-wire.coax` protocol and `page.coax` state machine via `host.h`/`host.c` — never a reimplemented simulator of desktop behavior.
2. **Hermetic execution:** Every scenario runs in a fresh `mktemp` workspace and isolated `CODER_DATA_DIR` so local user chats, pins, and `~/.claude` or `~/.coder` state are never mutated.
3. **Injectable seams in `Coder.app`:** Add minimal, env-gated test seams (`CODER_WORKER_BIN`, `CODER_CLAUDE_WORKER_BIN`, `CODER_DATA_DIR`, `CODER_INITIAL_CWD`) rather than `#ifdef` test forks, so the tested binary is identical to the shipped binary.
4. **Protocol-level recording:** Wrap the worker pipes in a tee logger (`wire.jsonl`) so every Tier 2/4 live run can be converted into a Tier 1 deterministic replay case with minimal editing.

---

## 5. Scoring and Grading

Prefer **deterministic, objective graders** wherever possible; reserve LLM-as-judge strictly for open-ended UX/communication rubrics.

| Dimension | Primary Grader | Key Metrics & Pass Criteria |
|---|---|---|
| **Functional / Coding Correctness** | Deterministic repo checks (test suites, compiler checks, `git diff` assertions, exit codes) | Binary pass@1 (and pass@k for Tier 4 variance); zero unintended file mutations outside workspace roots. |
| **Desktop State & Protocol Integrity** | Deterministic assertions on `page.coax` state, `chat.saved`, and `wire.jsonl` | Exact `MWW_RECALL` match, zero dropped events, budget auto-stop fires within 1 attempt of limit, `/goal` state transitions match spec, zero `CP_MAX_VERTICES` overflows. |
| **Sub-Agent Orchestration** | Deterministic wire & transcript checks | Valid level routing, `mww_results` returned for every delegated ticket, quiet-loop pause triggers at `CP_QUIET = 100`, sub-agent output preserved across save/load. |
| **UI / Visual & Responsiveness** | AX/clipboard/focus assertions + perceptual/pixel diff + Metal frame-time log | Zero crashes/hangs, clipboard exact match, visual diff below threshold on canonical views, streaming frame-time p95 `< 16.7 ms` (60 fps) and p99 `< 33.3 ms`. |
| **Efficiency & Cost** | Aggregated from `MWW_ATTEMPT` (`coder.observability.v2`) | Total USD per task, input/output/cached tokens, turn count, tool-call count, wall-clock seconds. Regressions `> 20%` on cost or latency flagged even when functional pass holds. |
| **Reply Clarity / UX Polish (Optional, Tier 2/4)** | Structured rubric (LLM-judge with pinned model + prompt, only as secondary signal) | Conciseness, accurate summary of edits/checks, no hallucinated test results. Never blocks Tier 0/1/3. |

---

## 6. Artifact and Trajectory Logging

Every eval invocation writes a self-contained run directory (e.g., `evals/desktop/runs/<YYYYMMDD-HHMMSS>-<git-sha>/` — gitignored) with:

```text
<run_dir>/
├── summary.json                # Run metadata (git_sha, branch, os, tier, model, totals, per-task scores)
├── report.md                   # Human-readable table + regression diff vs. previous baseline
└── cases/<case_id>/
    ├── result.json             # Pass/fail, grader breakdown, cost_usd, latency_ms, turns, tool_calls
    ├── wire.jsonl              # Timestamped owner<->worker mcworker-wire lines
    ├── journal.jsonl           # Copy of Microcoder's MWW_JOURNAL file (when live worker ran)
    ├── attempts.jsonl          # Extracted MWW_ATTEMPT (coder.observability.v2) records
    ├── chat.saved              # Final coder_page_save() payload for inspection/replay
    ├── workspace.diff          # Final git diff of the temp repo (Tier 2/4)
    ├── host.log                # Stdout/stderr of host driver or Coder.app
    └── screenshots/            # PNG captures + visual diff images (Tier 3)
```

This aligns directly with the existing observability formats (`MWW_JOURNAL`, `coder.observability.v2`, and `coder_page_save`), enabling any failing live run to be replayed locally in Tier 1.

---

## 7. Running Locally and in CI

### Local Developer Workflow

```bash
# Fast pre-commit check (< 10s, deterministic)
evals/openagents-desktop/run.sh --tier 0

# Host + mock-worker protocol & concurrency check (< 30s, deterministic)
evals/openagents-desktop/run.sh --tier 1

# Run a single live-agent desktop scenario (default: local Inkling 2 Mini on Psionic Metal)
evals/openagents-desktop/run.sh --tier 2 --filter a1_fix_mean_off_by_one

# Native macOS UI & visual check against mock worker
evals/openagents-desktop/run.sh --tier 3

# Compare a run against a saved baseline
evals/openagents-desktop/run.sh --compare <baseline_summary.json> <new_summary.json>
```

### CI & Scheduled Execution

There are no GitHub Actions in this repo; the green-main pipeline (`coax/remote/after.sh`) runs Tier 0 + Tier 1 on every head touching `apps/openagents-desktop/`, and `evals/openagents-desktop/nightly.sh` handles scheduled macOS runs:

| Trigger | Suites Run | Runner | Gate Policy |
|---|---|---|---|
| **Every head touching `apps/openagents-desktop/`, `components/`, `coax/lib/agent/engine/mcworker-wire.coax`, `coax/lib/inky/`, or `coax/lib/microcoder/`** | Tier 0 + Tier 1 + compaction | Green-main build pool (Linux) | **Required green-main gate** (100% pass). |
| **Nightly (`main`)** | Tier 0 + Tier 1 + Tier 2 (local Inkling 2 Mini) + Tier 3 (macOS GUI) | Dedicated Apple Silicon Mac (`nightly.sh night`) | Alerts on pass-rate drop `> 5%`, any Tier 3 failure, or wall-time spike `> 20%`. |
| **Weekly / Pre-Release** | Tier 4 (Golden trajectories + 25-task coding benchmark slice on local Inkling) + Session History (27 tasks) | Dedicated Apple Silicon Mac (`nightly.sh week`) | Generates release scorecard (`report.md`) comparing against saved baselines before cutting a desktop build. |

### Flakiness & Cost Controls
- **Tier 0, Tier 1, and mock-backed Tier 3** must maintain **0% flake rate**; any flaky test is quarantined and fixed immediately.
- **Tier 2 and Tier 4** enforce a hard per-task budget cap (via the desktop's own `CP_BUDGET` mechanism) and per-task wall-clock timeout, and record exact USD spend in `summary.json` so nightly CI spend stays bounded and predictable.
