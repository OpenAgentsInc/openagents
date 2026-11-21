# Lean 4 in the OpenAgents Desktop Gym

**Date:** 2026-11-21
**Status:** Evaluation and Architecture Specification (ready for phased implementation)
**Target surfaces:** `apps/openagents-desktop/` (Gym Workbench, Gym Benchmarks, Coax page, Foreman API), `coax/lib/lean/` (new shared Coax library), `coder/tui/lean-run.coax` (new CLI / helper daemon / MCP server), `proto/` (registered wire contracts), `evals/openagents-desktop/` (Lean benchmark tasks)

This folder evaluates how to implement **Lean 4 formal theorem proving and proof verification** inside the OpenAgents Desktop **Gym**, and specifies the architecture, wire contracts, verification soundness rules, and phased rollout plan.

## Contents

| Document | What it covers |
| --- | --- |
| [README.md](README.md) | Executive summary, current Gym baseline on `main`, four core architectural decisions, honest readiness/blockers, and quick links |
| [riemann-hypothesis-2026.md](riemann-hypothesis-2026.md) | **Canonical verified record (Nov 20–21, 2026):** Levent Alpöge & Claude's 17M-line Lean proof of the Riemann Hypothesis & Dirichlet GRH, Lean FRO / Mathlib Initiative joint certification, exact theorem statements, multi-kernel verification ledger (`con-leche`, `con-ron`, `nanoda`, `lean4lean`, `lake comparator`), `Robin1984` cross-check, SHA-256 provenance, and primary links |
| [riemann-hypothesis-workbench-spec.md](riemann-hypothesis-workbench-spec.md) | **Flagship RH Gym Workbench Spec:** 3-tier laptop-friendly storage model, pure Coax 175,187-module corpus indexer, 20-step guided tour (`remark.pdf` -> Lean), 60 fps semantic-zoom Proof Atlas, interactive math visualizers (critical strip, Dirichlet $L$-functions, Robin/Lagarias $\sigma(n)$, $\sqrt{X}$ theta-lift chain), Grounded AI Explainer, Lemma Simplification Lab, and Provenance & Multi-Kernel Trust Ledger |
| [evaluation.md](evaluation.md) | Deep technical trade-off analysis: Lean backends (CLI vs LSP vs Pantograph/REPL vs container sandboxes), UI hosting (Coax page vs Swift AppKit), Mathlib/toolchain budgets, soundness threats, and synergy with Psionic W3.1/W3.2 + ATIF-v1.8 |
| [spec.md](spec.md) | Full product and system architecture: `oa-leanhost` warm LSP daemon, `coax/lib/lean/` modules, 3-tab Gym (`Benchmarks` \| `PLE 3D` \| `Lean`), Lean Workbench UI, Lean Benchmark Track in `GymBenchmarkView`, `lean-run` CLI + MCP + chat steering + Foreman API |
| [wire-and-artifacts.md](wire-and-artifacts.md) | Registered schemas (`leanhost-lines-v1`, `lean-run.v1`, `lean-trace.v1`, `lean-verify.v1`, `lean-gym-request.v1`, `lean-gym-state.v1`, `lean-bench-task.v1`), ATIF-v1.8 mapping, `proto/registry.json` plan, and `~/.openagents/lean/` layout |
| [verification-and-security.md](verification-and-security.md) | `#authority` verification policy (`lean-policy.coax`), 3-stage kernel replay (`lake check --paranoid`, `lake comparator`, `con-leche`, `con-ron`, `nanoda`, `lean4lean`), axiom allowlist, untrusted-elaboration threat model & sandboxing, and 5-tier acceptance test plan |
| [roadmap.md](roadmap.md) | Phased build sequence (Phases 0–4 + RH-0–RH-4), file-by-file change map, concrete `Done` acceptance criteria per phase, and open owner decisions |

---

## Why Lean Belongs in the Desktop Gym

The Desktop Gym (`apps/openagents-desktop/`) is already evolving from a single-purpose 3D viewer into OpenAgents' **multi-domain agent evaluation, inspection, and live-racing workbench**:

- **Live Benchmarks (`GymBenchmarkView.swift`, tab 0):** Races local inference stacks head-to-head (Psionic Metal vs `llama.cpp` Metal on Inkling 2 Mini) across real coding tasks (`evals/openagents-desktop/{tier2,tier4}/`), streaming both agents live into **two embedded OpenAgents Coax GPU chat views side-by-side** (`GymChatSurfaceView` via `cd_open`/`cd_view`/`cd_line`).
- **PLE 3D Workbench (`GymWorkbenchView.swift`, tab 1):** Inspects physical engineering runs (`ple-scene.v1` / IFC4) with a left-rail run list, Live tailing, Compare baseline diff overlay, Clash Inspector / Diagnostics & Gate Badges drawer, bottom Timeline Scrubber with pinned snapshots, chat CLI (`ple-run`), inline `OA-SHOW` cards (`page/shows.coax`), and programmatic steering (`coax/lib/agent/desktop/gym.coax`, `GET/POST /v1/gym`).
- **Verified-trace training pipeline (on `main`):** `proof-export` (`coax/lib/agent/desktop/proof-export.coax`, #13895) writes canonical ATIF-v1.8 logs; Psionic W3.2 (#13918) distills GPU verified traces; W3.1 (#13920) hot-swaps LoRA adapters on Metal 4; and `openagents contribute submit` (#13926) packages redacted dataset shards.

**Lean 4 is the natural third pillar of the Gym** because:

1. **Zero-ambiguity ground truth.** Unlike heuristic unit tests or LLM-as-a-judge rubrics, a Lean 4 proof checked by independent kernel replay (`lean4checker` / `SafeVerify`) with a strict axiom allowlist provides mathematical certainty — zero reward hacking for both benchmarks and RL/distillation traces.
2. **Direct structural isomorphism with PLE.** Every Gym primitive already built for PLE maps 1:1 onto Lean:
   - Run discovery (`~/.openagents/ple/runs` -> `~/.openagents/lean/runs` + local Lake projects)
   - Interactive viewport (3D Metal scene -> Proof Source + Cursor Tactic Infoview + Proof State DAG)
   - Failure inspector (Clashes with click-to-focus camera -> Proof Obligations / Errors / `sorry` / Axiom Violations with click-to-jump line & goal)
   - Verification gates (`schema`, `clash`, `connectivity` -> `parse`, `elaborate`, `no_sorry`, `axioms_ok`, `target_match`, `kernel_replay`)
   - Trajectory timeline (`step_000..N` snapshots + Live + Pin -> tactic/edit steps + remaining goal count + branch nodes + Live + Pin)
   - Chat + Foreman steering (`ple-run gym`, `/v1/gym` -> `lean-run gym`, `/v1/gym/lean`, `lean-run mcp`)
3. **High-leverage benchmark & distillation loop.** Adding Lean theorem-proving suites to `GymBenchmarkView` lets OpenAgents race Psionic Metal (base vs distilled LoRA vs `llama.cpp` vs cloud frontier models) live on formal proofs, then export every kernel-verified proof trajectory as ATIF-v1.8 straight into Psionic W3.2 distillation.

---

## Summary of Architectural Decisions

Full analysis is in [evaluation.md](evaluation.md); the complete design is in [spec.md](spec.md).

| Decision Area | Evaluated Options | Recommended Design |
| --- | --- | --- |
| **1. Gym Placement** | (A) Separate sidebar item vs (B) Replace PLE vs (C) Expand Gym into 3-tab workbench + Benchmark track | **(C) Two complementary surfaces sharing one engine:** (1) Tab 2 `Lean` in the Gym mode bar (`Benchmarks` \| `PLE 3D` \| `Lean`) for interactive proof inspection/editing/scrubbing; (2) selectable **Lean Suites** inside Tab 0 `Benchmarks` (`GymBenchmarkView`) for side-by-side live model racing. Existing `ple-gym-request.v1` / `/v1/gym` remain backwards-compatible. |
| **2. Lean Execution Backend** | (1) One-shot `lake env lean` CLI only vs (2) Warm `lake serve` LSP daemon vs (3) Pantograph / LeanREPL vs (4) Containerized remote runner | **Layered hybrid behind one Coax helper daemon (`oa-leanhost`, `lean-run serve`, `~/.openagents/lean/leanhost.sock`):** Phase 1 uses warm `lake serve` LSP sessions in pure Coax (`coax/lib/lean/lsp.coax`) for 50–300 ms interactive checking & `$/lean/plainGoal`; every completion verdict runs an independent batch **3-stage kernel verifier** (`lake env lean` + `lean4checker`/`SafeVerify`); Phase 4 plugs in Pantograph/LeanREPL behind the same daemon for MCTS/branching tactic search and container-isolated remote benchmark runs. |
| **3. UI Rendering Layer** | (U1) Coax-drawn `page/gym-lean.coax` over shared `components/` vs (U2) macOS Swift `GymLeanView.swift` inside `GymWorkbenchView` | **(U1) Coax-drawn Lean Workbench (`page/gym-lean.coax`)** reusing `components/app/`, `components/zeron/editor.coax` (`UiEditor`), `components/app/diff.coax`, and `components/paint/`. Works on **macOS, Linux, and Windows**, obeys `AGENTS.md` ("Coax first") and [docs/reuse.md](../reuse.md), avoids native-view occlusion holes, and runs deterministic Tier 0 tests in `< 5 s`. Lift the 36 pt Gym mode bar into the Coax page so `Benchmarks` and `PLE 3D` keep their existing macOS views seamlessly underneath while `Lean` renders directly in Coax. Meanwhile, wire Lean benchmark suites into existing `GymBenchmarkView.swift` on macOS (Phase 3). |
| **4. Verification Soundness** | Trusting `lake build` exit code vs Independent multi-kernel replay + axiom & statement audit | **Mandatory `#authority` policy (`coax/lib/lean/policy.coax`, zero surviving mutants) + independent kernel replay.** `lake build` alone is vulnerable to `sorryAx`, `native_decide` (`Lean.ofReduceBool`), custom axioms, `@[implemented_by]`, and theorem-statement tampering. No run or benchmark task may be marked `solved` unless a fresh compile + inert-export kernel replay (`lake check --paranoid`, `lake comparator`, `con-leche --compare`, `con-ron`, `nanoda`) verifies exact theorem signature equality and restricts axioms strictly to `{propext, Quot.sound, Classical.choice}`. |
| **5. Agent Integration** | Shell-only vs MCP-only vs Unified CLI + MCP + chat cards | **Mirror `oasim` (`coder/tui/oasim.coax`) and `ple-run` (`apps/openagents-desktop/ple-run.coax`):** Ship one Coax binary `lean-run` in `OpenAgents.app/Contents/Resources/lean-run` exposing CLI verbs (`serve`, `doctor`, `check`, `goal`, `try`, `hover`, `def`, `search`, `verify`, `sample`, `bench`, `gym`, `export-atif`), stdio MCP server (`lean-run mcp`, also exposed via `openagents mcp --domains lean`), `dc_tools_note` discovery for Microcoder, and `OA-SHOW` proof cards in chat. |

---

## Current Baseline & Honest Blockers

Per `AGENTS.md` (*"Done means the real thing works"*, *"No UI that lies"*), we record what is present and what is missing on the development Mac as of 2026-11-21:

| Dependency | Status on This Mac (2026-11-21) | Impact & Mitigation |
| --- | --- | --- |
| Coax compiler & desktop app (`apps/openagents-desktop/`) | Present on `main` (`bbb266c888`); `GymWorkbenchView` + `GymBenchmarkView` + `ple-run` + `oasim` + `proof-export` all ship | Immediate integration points exist; zero speculative plumbing required |
| `elan` / `lean` / `lake` toolchain | **Not installed** (`which lean lake elan` returns not found; `~/.elan/bin` absent) | Phase 0 ships `lean-run doctor` which detects missing toolchain honestly (`no_toolchain`) and never fakes a pass. Installing `elan` + a pinned Lean 4.x toolchain is a prerequisite before closing any live-verification issue (Tier 2+). |
| `lean4checker` / `SafeVerify` | **Not installed** | Phase 0 bundles a small, self-contained kernel-replay & axiom-audit Lean verification harness (zero Mathlib dependency) alongside support for external `lean4checker` / `SafeVerify` binaries. |
| Mathlib `.olean` cache (~2–4 GB disk) | **Not present** | Bundled sample pack and Phase 0–2 acceptance suites use **pure Lean 4 `Init` only** (zero Mathlib download, compiles in `< 2 s`). Mathlib suites (`MiniF2F`, `PutnamBench`) are opt-in and gated on explicit cache presence (`lake exe cache get`). |
| `UiEditor` buffer size (`EB_TEXT`) | `components/core/edit.coax` caps editable buffers at `EB_TEXT = 16384` bytes (`512` lines in `page/filepane.coax`) | Covers >95% of single-theorem Gym tasks (typical proof files are 1–6 KB). Larger files (up to 256 KB) open in virtualized read-only inspection mode (matching `filepane.coax`) until `UiEditor` buffer capacity is extended if needed. |

---

## Non-Goals (v1)

- **Not a full VS Code replacement.** We do not replicate arbitrary Lean ProofWidgets HTML/React/TypeScript custom widgets (which would require embedding a WebView/JS runtime, contradicting Coax Native). v1 renders structured tactic goals, term goals, diagnostics, hover types, and our native Coax Proof DAG.
- **Not bundling Lean 4 or Mathlib inside `OpenAgents.app`.** `OpenAgents.app` bundles only the small Coax `lean-run` binary (~1–2 MB) and tiny `Init`-only sample `.lean` fixtures. Toolchains live in the user's `~/.elan/` (or Nix / container), and runs/caches live under `~/.openagents/lean/`.
- **No Python runtime requirement inside the desktop app.** Consistent with `AGENTS.md` (*"Python is tooling only; it is never shipped and never needed at run time"*), LSP JSON-RPC framing, verification policy, Gym planning, and MCP serving are written in Coax.
- **No silent auto-downloads of multi-gigabyte Mathlib caches.** `lean-run doctor` and the Gym UI clearly report toolchain/cache state and run `elan` / `lake exe cache get` only when the user explicitly requests it.

---

## Related Repository References

- [Steering the PLE Gym from a chat](../desktop/steer-ple-from-chat.md) (`ple-run`, `OA-SHOW`, `ple-gym-request.v1`)
- [iOS Simulator in OpenAgents Desktop](../desktop/ios-simulator-spec.md) (`oasim`, UNIX-socket helper daemon, MCP server, Coax-first host split)
- [Foreman control API](../coder/foreman-control-api.md) (`GET/POST /v1/gym`, `foreman-ctl`)
- [Coder Desktop evaluation specification](../coder/desktop-evals-spec.md) (5-tier eval discipline)
- [OpenAgents desktop shell reference](../desktop/coax-desktop.md) (Coax Native direction)
- [Bounded contexts and context map](../architecture/contexts.md) (`coder` context boundaries)
- [Reuse what exists](../reuse.md) (shared `components/`, `coax/lib/` catalog)
- [OpenAgents Protocol registry](../../proto/README.md) (`proto/registry.json` & `wire-check`)
