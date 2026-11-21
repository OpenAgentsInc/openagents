# Lean Gym Implementation Roadmap & Open Decisions

**Date:** 2026-11-21
**Companion documents:** [README.md](README.md) · [evaluation.md](evaluation.md) · [spec.md](spec.md) · [wire-and-artifacts.md](wire-and-artifacts.md) · [verification-and-security.md](verification-and-security.md)

This roadmap sequences Lean 4 integration into five shippable, independently verifiable phases. Each phase produces real, user-usable functionality with concrete `Done` criteria (no stand-ins, no fake green badges).

---

## Prerequisite (Machine / Environment Setup)

As verified on 2026-11-21, `elan`, `lean`, and `lake` are not installed on this development Mac (`which lean lake elan` -> not found).

- **Phase 0 Tier 0/1 checks** (pure Coax policy, mutation tests, wire goldens, fake-LSP tests) and **RH Workbench Tier A** (indexing & visualizing the 175,187-file Riemann Hypothesis source tree; see [riemann-hypothesis-workbench-spec.md](riemann-hypothesis-workbench-spec.md)) can be built and verified immediately without `elan`.
- **Closing Phase 0+ live verification (Tier 2–4)** requires installing `elan` and the pinned Lean 4 toolchains (`leanprover/lean4:v4.36.0` for `lake check --paranoid`, `lake comparator`, `con-leche`, `con-ron`, `nanoda`, and `leanprover/lean4:v4.33.1` for live LSP / cache interaction with `leanprover-community/riemann-hypothesis`) on the Mac and Linux build hosts (`curl https://elan.lean-lang.org/elan-init.sh -sSf | sh`). No Mathlib download is needed until Phase 3b (`MiniF2F`) or RH Tier B.

---

## Phase 0 — Sound Verification Core & `lean-run` CLI Foundation

**Goal:** Ship the `#authority` verification policy, registered wire schemas, zero-dependency Lean kernel-replay checker, and the `lean-run` Coax CLI (`doctor`, `verify`, `sample`) so chats and scripts can run real, cheat-proof Lean 4 verification right away.

### Files to Add / Edit
- `proto/coder/lean-{host-lines,run,trace,verify,gym,bench-task}-v1.json` + rows in `proto/registry.json` (`owner: "coder"`).
- `coax/lib/lean/README.md`
- `coax/lib/lean/policy.coax` (`#authority;`, 0 surviving mutants)
- `coax/lib/lean/wire.coax` + `wire-checks.coax`
- `coax/lib/lean/verify.coax`
- `resources/lean-checker/OpenAgentsVerify.lean` (zero-Mathlib `Init`/`Lean` kernel replay + statement-match + axiom-audit script)
- `evals/openagents-desktop/lean/samples/add_zero_right/` (`Spec.lean`, `Solution.lean`, `lean-toolchain`) + anti-cheat test fixtures (`cheat_sorry`, `cheat_custom_axiom`, `cheat_native_decide`, `cheat_statement_weaken`, `cheat_wrong_name`)
- `coder/tui/lean-run.coax` (`doctor`, `verify`, `sample` verbs + `OA-SHOW` card output)
- `coax/lib/agent/desktop/worker.coax` (add `~/.elan/bin` to `dc_worker_path`; add `lean_run` to `dc_tools_note`)
- `apps/openagents-desktop/build.sh` (compile and bundle `Resources/lean-run`, `Resources/lean-checker/`, `Resources/fixtures/lean-samples/`)

### Done Criteria
- `coax/bin/coax mutate coax/lib/lean/policy.coax --root-only` -> **0 survivors**.
- `coax/web/tool.sh wire-check`, `layer-check`, `tools-check` -> **0 failures**.
- `lean-run doctor` reports honest `no_toolchain` when `elan` is absent and exact version/paths when `elan` is present.
- On a machine with Lean 4 installed: `lean-run sample` verifies cleanly (`verified: true`) in `< 3 s` and **all 6 adversarial anti-cheat fixtures fail** with their exact refusal codes (`sorry_used`, `forbidden_axiom`, `statement_mismatch`, `digest_mismatch`).

---

## Phase 1 — Warm LSP Daemon (`oa-leanhost`) & Agent MCP Server (`lean-run mcp`)

**Goal:** Eliminate 10–30 s cold startups by running one bounded warm `lake serve` LSP pool per user, exposing sub-300 ms `check`, `goal`, `try`, `hover`, `def`, `search`, and MCP tools to Microcoder, Claude Code, Codex, and Antigravity.

### Files to Add / Edit
- `coax/lib/lean/lsp.coax` (pure Coax JSON-RPC 2.0 + `Content-Length` framing, `$/lean/fileProgress` settling, `$/lean/plainGoal`, `$/lean/plainTermGoal`, `hover`, `definition`, `$/cancelRequest`)
- `coax/lib/lean/goal.coax` (goal parser, hypothesis/target split, Proof DAG builder)
- `coax/lib/lean/host.coax` (`oa-leanhost`: UNIX socket `~/.openagents/lean/leanhost.sock`, lockfile, warm LRU pool, RSS watchdog, push subscriptions, idle shutdown)
- `coax/lib/lean/tests/fake-lsp.coax` (Tier 1 scripted mock-LSP harness)
- `coder/tui/lean-run.coax` (add `serve`, `check`, `goal`, `try`, `hover`, `def`, `search`, `status`, `evict`)
- `coder/tui/lean-run-mcp.coax` (stdio JSON-RPC 2.0 MCP server following `coder/tui/oasim-mcp.coax`)
- `apps/openagents-cli/mcp.coax` (add `--domains lean` alongside `realm`, `device`, `executor`)
- `coder/tui/claudeworker.coax` (expose `lean-run mcp` in `--mcp-config` when Experimental features are on and `lean` is installed)

### Done Criteria
- Tier 0 checks pass on `lsp.coax` and `goal.coax` (including recorded real Lean 4 LSP transcripts).
- Tier 1 fake-LSP suite verifies lockfile single-start, concurrent clients, LRU eviction, crash recovery, and timeout cancellation.
- Tier 2 live check against real `lake serve`: second `lean-run goal` / `lean-run try` call completes in `< 300 ms` on a warm session; `lean-run mcp` answers `tools/list` and `tools/call` (`lean_check`, `lean_goal`, `lean_try_tactic`, `lean_verify`).

---

## Phase 2 — Cross-Platform Coax Lean Gym Workbench & Programmatic Steering

**Goal:** Bring the full interactive Lean Proof Workbench into the Desktop Gym (Tab 2: `Lean`) on **macOS, Linux, and Windows**, with File Pane goal strip, `lean-run gym`, and Foreman Control API `/v1/gym/lean`.

### Files to Add / Edit
- `coax/lib/lean/gym.coax` (`dc_lean_gym_plan`, exported as `oa_dc_lean_gym_plan` in `apps/openagents-desktop/desktop-core.coax`)
- `coax/lib/lean/unicode.coax` (Lean backslash Unicode abbreviation table)
- `apps/openagents-desktop/page/state.coax`, `host-api.coax`, `host.h`, `host.c` (`gym_mode`, `CpLeanGym`, `cd_gym_mode`, `cd_gym_set_mode`, `cd_lean_line`/`cd_lean_out` socket bridge mirroring `simpane.coax`)
- `apps/openagents-desktop/page/gym.coax` (lifted 36 pt Coax Gym mode bar: `Benchmarks` | `PLE 3D` | `Lean` + toolchain pill)
- `apps/openagents-desktop/page/gym-lean.coax` (Left Rail: Runs + Live + Compare + Obligations/Diagnostics Drawer; Center: Source Editor/Viewer + Gutter Markers + 38 pt Timeline Scrubber + Diff view; Right: Tactic Infoview + `Try Tactic` + Proof State DAG + Kernel Receipt Card; Honest missing-toolchain setup card)
- `apps/openagents-desktop/page/filepane.coax` (collapsible bottom Lean Goal Strip + "Open in Lean Gym" button for `.lean` files)
- `apps/openagents-desktop/page/layout/layout.coax` (reserve mode bar height above `cd_gym_rect`; hide native view when `gym_mode == CP_GYM_LEAN`)
- `apps/openagents-desktop/GymWorkbenchView.swift`, `Window+Gym.swift` (sync with Coax mode bar, remove duplicate internal `modeBar`, add `~/.openagents/lean/gym-request.json` polling alongside PLE's)
- `apps/openagents-desktop/ForemanOperator.swift`, `Window+ForemanAPI.swift`, `foreman-ctl` (add `GET/POST /v1/gym/lean`, additive `mode` field on `GET /v1/gym`, `foreman-ctl gym lean`)
- `apps/openagents-desktop/page/checks/gym-lean-checks.coax` (Tier 0 checks for every Lean Workbench interaction)
- `apps/openagents-desktop/linux/main.c`, `apps/openagents-desktop/windows/main.c` (wire `cd_lean_*` socket bridge so Lean Gym runs on Linux & Windows)

### Done Criteria
- `coax/bin/coax test apps/openagents-desktop/page.coax --library` passes with new Lean Workbench + mode bar + File Pane tests.
- `coax/web/tool.sh components-check` passes (zero hand-rolled editors/diffs outside `components/`).
- Tier 3 live `OpenAgents.app` launch verified via screenshots & `foreman-ctl`:
  - `Benchmarks`, `PLE 3D`, and `Lean` tabs all switch cleanly with zero PLE regression (`ple-run gym` still passes).
  - Unicode math symbols (`∀`, `∃`, `→`, `⊢`, `⟨`, `⟩`, `ℕ`, `ℝ`) render crisply with zero tofu boxes.
  - Clicking an Obligation jumps caret + refreshes Infoview; scrubbing timeline updates Proof DAG; clicking Verify runs real `lean-run verify` and updates receipt card; 1-byte edit invalidates `Verified` -> `Stale`.
- `apps/openagents-desktop/windows/build.sh --check` and Linux build both succeed.

---

## Phase 3 — Live Lean Benchmarks in `GymBenchmarkView` & Verified-Trace Distillation Loop

**Goal:** Enable head-to-head live formal theorem-proving races inside Tab 0 (`Benchmarks`) with dual embedded Coax GPU chat views, plus canonical ATIF-v1.8 trace export straight into Psionic W3.2 distillation and W3.1 LoRA hot-swap comparison.

### Files to Add / Edit
- `evals/openagents-desktop/lean/suites/core_smoke/` (5 self-contained `Init`-only Lean 4 theorem tasks + pristine `Spec.lean` fixtures)
- `evals/openagents-desktop/lean/suites/algebra_logic/` (10 harder `Init`-only theorem tasks)
- `evals/openagents-desktop/lean/suites/minif2f_slice/` (10 pinned MiniF2F-Lean4 tasks, opt-in when Mathlib cache is verified)
- `apps/openagents-desktop/GymBenchmarkView.swift` (add Lean suites to `suitePopup`, add `LeanCheck`/`LeanGoal`/`LeanTry` tools alongside `Read`/`Write`/`Bash`, add `lean_verify` grader invoking `lean-run verify --sandbox`, persist `~/.openagents/gym/last-lean-benchmark.json`, add "Inspect in Lean Workbench" jump per row, support comparing `Psionic Base` vs `Psionic + Lean LoRA` in addition to `Psionic Metal` vs `llama.cpp Metal`)
- `apps/openagents-desktop/build.sh` (bundle `Resources/gym-evals/lean/`)
- `coax/lib/lean/atif.coax` + `lean-run export-atif` (ATIF-v1.8 export verified by `atif_verify_log`, feeding Psionic W3.2 `#13918` and `openagents contribute submit` `#13926`)

### Done Criteria
- Live Tier 4 benchmark run of `Lean Core Smoke (5 Theorems)` completes end-to-end inside `OpenAgents.app` with both Coax GPU chat views streaming real tool calls and authoritative `lean-run verify` pass/fail scores.
- Clicking a completed Lean benchmark row opens its proof trace and `verify.json` in Tab 2 (`Lean`).
- `lean-run export-atif` on a solved run produces a `.atif.jsonl` file that passes `proof-export verify`.

---

## Phase 4 — Branching Tactic Search (`Pantograph` / `LeanREPL`) & Containerized Remote Benchmarks

**Goal:** Support high-throughput MCTS / best-first proof search trees and large-scale sandboxed Mathlib benchmarks (full MiniF2F / PutnamBench / custom research conjectures) without overloading a single laptop.

### Files to Add / Edit
- `coax/lib/lean/pantograph.coax` (JSON-lines client for `pantograph-repl` / `LeanREPL`, `state_id` branching, metavariable coupling, direct projection into `lean-trace.v1` DAG nodes)
- `coax/lib/lean/host.coax` (engine selector `"lsp"` vs `"pantograph"`, parallel search worker pool)
- `coder/tui/lean-run.coax` (`bench start|status|stop` detached runner mirroring `ple-run-kit.coax`, supporting local Docker/Harbor isolation or `coax/remote/` build-host offloading)
- `apps/openagents-desktop/page/gym-lean.coax` (multi-branch Proof DAG zoom/pan & branch-pruning filter for trees > 100 nodes)

### Done Criteria
- Pantograph engine executes a multi-branch proof search (at least 10 explored tactic nodes with backtracking) and streams the live DAG into the Lean Gym Workbench.
- Detached `lean-run bench start` runs a container-isolated multi-theorem benchmark, survives desktop app quit/reopen, and surfaces live progress & kernel receipts in the Gym.

---

## Flagship Corpus Track — Riemann Hypothesis Workbench (RH-0 to RH-4)

Built directly on top of Phases 0–2 (and with **RH-0 / RH-1 / RH-2 Tier A** usable immediately even before `elan` is installed). Full specification and per-phase `Done` criteria are in [riemann-hypothesis-workbench-spec.md](riemann-hypothesis-workbench-spec.md):

- **RH-0:** Pure Coax streaming indexer (`rh-index.coax`) over all 175,187 `.lean` files (`< 60 s` index, `< 15 ms` search over 818,755 theorems / 242,067 defs, shortest-path-to-`Main.lean`, SHA-256 provenance verification).
- **RH-1:** Guided 20-step `remark.pdf` tour (`rh-tour.coax`), Provenance & Multi-Kernel Trust Ledger, and Coax GPU mathematical visualizers (`rh-math.coax`: Critical Strip & Dirichlet $L$-functions, `Robin1984` $\sigma(n)$, $\sqrt{X}$ iterated theta-lift chain).
- **RH-2:** 60 fps multi-scale semantic-zoom Proof Atlas (`page/gym-rh-atlas.coax`) over the 175,187-module graph.
- **RH-3:** Module Inspector + Grounded AI Explainer (seeded with [riemann-hypothesis-2026.md](riemann-hypothesis-2026.md)) + local Markdown/LaTeX notes + Tier B live Lean `v4.33.1` LSP Infoview.
- **RH-4:** Lemma Simplification Lab (`#authority` `rh-simplify.coax` + `con-leche --compare` / `lake comparator` on Lean `v4.36.0`) + Tier C local/remote kernel audit.

---

## Open Owner Decisions

None of these block Phase 0–1 or RH-0–RH-2 design, but settling them before Phase 2/3 avoids rework:

1. **Default pinned Lean 4 toolchains:** Recommend pinning **`leanprover/lean4:v4.36.0`** for new OpenAgents Gym suites & multi-kernel judging (`lake check --paranoid`, `lake comparator`, `con-leche`, `con-ron`, `nanoda`) alongside **`leanprover/lean4:v4.33.1`** for live LSP / Tier B builds on `leanprover-community/riemann-hypothesis`.
2. **Installing `elan` on Mac & remote build hosts:** Should we add an automated `elan` + pinned-toolchain check/setup note to `NEEDS_OWNER.md` (and Nix profile in `os/nix/`) so build hosts and dev Macs can run Tier 2+ live Lean suites continuously?
3. **Experimental feature gate:** Keep the entire Gym (including `Lean`) behind `Settings > General > Experimental features` initially (matching `PLE 3D` and `Benchmarks`), or promote the `Lean` Gym out of Experimental once Phases 0–2 pass Tier 3?
4. **Cloud / Frontier side in `GymBenchmarkView`:** In Phase 3, besides `Psionic Metal vs llama.cpp Metal`, do we want a dropdown selector on each race side (`Psionic Metal`, `Psionic + LoRA`, `llama.cpp Metal`, `Gemini 4 Neon`, `Claude Opus`) so the Gym can race local Inkling LoRAs directly against frontier models on Lean theorems?
