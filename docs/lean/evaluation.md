# Lean in the Desktop Gym: Technical Evaluation & Trade-Off Analysis

**Date:** 2026-11-21
**Companion documents:** [README.md](README.md) · [spec.md](spec.md) · [wire-and-artifacts.md](wire-and-artifacts.md) · [verification-and-security.md](verification-and-security.md) · [roadmap.md](roadmap.md)

This document evaluates the concrete engineering choices for implementing Lean 4 in the OpenAgents Desktop Gym (`apps/openagents-desktop/`), grounded in the current `main` codebase (`bbb266c888`) and repository rules (`AGENTS.md`, [docs/reuse.md](../reuse.md), [docs/architecture/contexts.md](../architecture/contexts.md)).

---

## 1. Current Desktop Gym Inventory (What Exists Today)

Before choosing how Lean plugs in, here is the exact inventory of the Gym on `main`:

| Component | Path | Platform | What It Does Today |
| --- | --- | --- | --- |
| Sidebar entry & rect | `apps/openagents-desktop/page/layout/sidebar.coax`, `layout.coax`, `state.coax`, `host-api.coax` | Mac / Linux / Win | Draws `coder.gym` under Foreman when `experimental == true`; sets `gym_x/y/w/h` to the content rect beside the sidebar and below the title bar; exposes `cd_gym_set`, `cd_gym_open`, `cd_gym_rect` in `host.h`. |
| Gym host container & mode switcher | `apps/openagents-desktop/Window+Gym.swift`, `GymWorkbenchView.swift` | **macOS only** (AppKit) | Mounts `GymWorkbenchView` over `cd_gym_rect`. Top 36 pt `modeBar` switches between **Tab 0: `Benchmarks`** and **Tab 1: `PLE 3D`**. Polls `~/.openagents/ple/gym-request.json` every 0.5 s and handles `/v1/gym`. |
| Live Benchmarks tab | `apps/openagents-desktop/GymBenchmarkView.swift` | **macOS only** (AppKit + embedded Coax GPU views) | Races `psionic-inkling-serve` (Metal 4) vs `llama-server` (Metal) on 10 bundled coding tasks (`Resources/gym-evals/{tier2,tier4}/`), streaming both live into two `GymChatSurfaceView` instances (`cd_open`/`cd_view`/`cd_line`). Persists `~/.openagents/gym/last-benchmark.json`. |
| PLE 3D Workbench tab | `GymWorkbenchView.swift`, `GymDiscovery.swift`, `GymWorkbenchView+Plan.swift`, `SceneView.swift` | **macOS only** (AppKit + Metal) | Discovers `.ple-scene.json` / `.ifc` runs in `~/.openagents/ple/runs`, `ple-kit/runs`, and bundled sample; provides 3D viewport, Live tailing (1.5 s), baseline diff overlay, Clash Inspector / Diagnostics & Gate Badges drawer, and 38 pt Timeline Scrubber + Pin. |
| Programmatic Gym plan | `coax/lib/agent/desktop/gym.coax`, `desktop-core.coax` (`oa_dc_gym_plan`) | Cross-platform Coax | Strictly validates `ple-gym-request.v1` JSON (unknown keys refused) and emits a normalized plan JSON for Swift to execute. |
| Chat CLI & inline cards | `apps/openagents-desktop/ple-run*.coax`, `coax/lib/agent/desktop/worker.coax` (`dc_tools_note`), `page/shows.coax` | Cross-platform Coax | Bundles `Resources/ple-run`; teaches Microcoder about `ple-run` when Experimental is on; renders up to 4 `OA-SHOW` cards per message (`OA-SHOW-OPEN` opens scenes/files; `OA-SHOW-LIVE: on` mounts a live 3D card). |
| Foreman Control API | `ForemanOperator.swift`, `Window+ForemanAPI.swift`, `foreman-ctl` | macOS (Swift) | Exposes `GET /v1/gym` and `POST /v1/gym` alongside chat/wallet routes, audited in `foreman-audit.jsonl`. |
| Helper-daemon + MCP precedent | `coax/lib/agent/device/`, `coder/tui/oasim.coax`, `coder/tui/oasim-mcp.coax` | Coax (POSIX/macOS) | Proves the exact architecture for a user-scoped UNIX-socket helper daemon (`~/.openagents/sim/simhost.sock`), CLI verbs, `#authority` policy (`sim-policy.coax`), and stdio MCP server (`oasim mcp` / `openagents mcp --domains device`). |
| Trace export & distillation | `coax/lib/agent/desktop/proof-export.coax`, `coax/lib/nostr/atif/`, `psionic/` W3.1–W3.2 | Cross-platform Coax | Converts runs into canonical ATIF-v1.8 logs with verified SHA-256 step digests; feeds Psionic GPU verified-trace distillation (#13918) and Metal 4 LoRA hot-swap (#13920). |

**Key observation:** The Gym already has *two distinct modes* — **Head-to-Head Live Benchmarks** and **Interactive Domain Workbench** — and OpenAgents already has a battle-tested Coax pattern for stateful local tool daemons (`oasim`), inline chat cards (`shows.coax`), programmatic Gym steering (`gym.coax`), and ATIF-v1.8 verified trajectories (`proof-export.coax`).

---

## 2. Evaluation 1: Lean Execution Backend

How should OpenAgents interact with Lean 4? Unlike Python/C/JS test suites where cold process startup is 20–100 ms, a Lean 4 file that imports `Mathlib` loads gigabytes of precompiled `.olean` modules. Cold startup latency dominates everything if the execution interface is chosen poorly.

### Options Compared

| Criterion | Option B1: One-Shot `lake env lean` CLI | Option B2: Warm `lake serve` LSP Daemon | Option B3: Tactic REPL (`Pantograph` / `LeanREPL`) | Option B4: Containerized Remote Verifier |
| --- | --- | --- | --- | --- |
| **Cold latency (Init-only)** | 0.4–1.5 s | 0.5–1.5 s once, then 20–100 ms | 0.5–1.5 s once, then 5–30 ms | 2–5 s container boot + compile |
| **Edit/tactic latency (Mathlib)** | **8–30 s per check (unusable interactively)** | **50–300 ms incremental** (imports stay hot in RAM) | **10–80 ms per tactic step** (immutable state handles) | Batch only (seconds–minutes) |
| **Works on arbitrary Lake repos** | Yes (zero project edits) | **Yes (ships in every Lean 4 toolchain)** | **No** — must build Pantograph/REPL matching exact `lean-toolchain` | Yes (with matching image) |
| **Interactive goal state (`⊢ ...`)** | Hacky (`trace_state` injection) | **Native** (`$/lean/plainGoal`, `$/lean/plainTermGoal`, `hover`, `definition`) | **Native** (structured goal list + metavariable coupling) | Post-hoc only |
| **Tree search / MCTS branching** | Impractical | Moderate (scratch buffer overlays; re-elaborates changed suffix) | **Optimal** (`goal.tactic(state_id, goal_id, t)` branches cheaply without file I/O) | Parallel worker pool |
| **Elaboration progress signal** | Exit code only | **Native** (`$/lean/fileProgress` eliminates stale diagnostic races) | Synchronous RPC response | Exit code + receipt |
| **Soundness as final grader** | **Insufficient alone** (allows `sorryAx`, `native_decide`, custom axioms, `#eval`) | **Insufficient alone** (same + LSP edits are in-memory) | **Insufficient alone** (same; REPL state can diverge from clean file build) | **Strong** when paired with `lean4checker` / `SafeVerify` |
| **Coax implementation effort** | Trivial (`proc_run`) | Low–Moderate (~500 lines pure Coax JSON-RPC 2.0 + `Content-Length` framing over pipes) | Moderate (JSON-lines client + toolchain build/version matching) | Moderate (reuses `ple-run kit` detached runner + `coax/remote/`) |
| **Runtime Python dependency?** | None | **None (100% Coax)** | None for CLI/FFI (avoid `PyPantograph` at runtime) | Optional (container host) |

### Analysis & Recommendation

1. **No single backend covers all three needs** (interactive IDE/agent checking, authoritative benchmark grading, and high-throughput tree search):
   - **Interactive editing & general agent workflows** need `lake serve` LSP (B2): it works out of the box on *any* Lean 4 project (`lean-toolchain` + `lakefile.lean`/`lakefile.toml`) with zero extra Lean dependencies, keeps Mathlib hot in RAM, gives exact `$/lean/plainGoal` + `$/lean/plainTermGoal` + `$/lean/fileProgress` + `textDocument/publishDiagnostics`, and is trivial to drive from pure Coax over stdio pipes.
   - **Authoritative verification & benchmark grading** MUST NEVER trust LSP diagnostics or `lake build` alone: it must run a clean isolated build + independent kernel replay (`lean4checker` / `SafeVerify`-equivalent checker) verifying exact target theorem signature match and strict axiom allowlist (`propext`, `Quot.sound`, `Classical.choice`).
   - **Tactic tree search / RL rollouts** (MCTS, best-first search, parallel tactic sampling) eventually benefit hugely from Pantograph or LeanREPL (B3), because stepping an immutable `state_id` avoids file serialization and exposes metavariable coupling explicitly.
   - **Large untrusted benchmark suites** (MiniF2F, PutnamBench, FrontierMath) require process/container isolation (B4) because Lean 4 elaboration executes arbitrary `#eval` / metaprogramming code.

2. **Recommended architecture: One Coax helper daemon (`oa-leanhost`) with pluggable execution engines:**
   - **Phase 0–1 (MVP):** Pure Coax `lake serve` LSP pool inside `oa-leanhost` (`lean-run serve`, `~/.openagents/lean/leanhost.sock`) **plus** the independent 3-stage kernel verifier (`lean-run verify`).
   - **Phase 3–4 (Scale):** Add optional `pantograph` engine behind the same `oa-leanhost` RPC (`tactic_step` / `branch`) and optional Docker / remote-host benchmark runner (`lean-run bench start`, mirroring `ple-run kit start`).

3. **Why a shared per-user daemon (`oa-leanhost`) is mandatory (not per-chat or per-UI-panel processes):**
   - A single Mathlib-loaded `lake serve` consumes **1.5–4.0 GiB RSS** and takes **10–25 s** to warm up.
   - If the Desktop Gym UI, Microcoder, a Claude Code sub-agent (`lean-run mcp`), and a right-hand File Pane each spawned their own `lake serve`, a 32 GB Mac would immediately lose 8–16 GiB RAM and pay cold-start penalties repeatedly.
   - Sharing one bounded LRU pool inside `oa-leanhost` (default **max 2 concurrent warm Lake workspaces**, configurable via `OA_LEAN_MAX_WORKSPACES`, with RSS watchdog and 10-minute idle shutdown — identical lifecycle to `oasim serve` in [ios-simulator-spec.md](../desktop/ios-simulator-spec.md)) gives instant sub-300 ms responses to both human and agents at a bounded memory footprint.

---

## 3. Evaluation 2: Desktop UI Hosting Architecture

Where and how should the Lean Gym UI render? This is the sharpest architectural fork in the current desktop codebase because `GymWorkbenchView.swift` and `GymBenchmarkView.swift` are currently macOS AppKit/Swift views, while the rest of OpenAgents Desktop (`page.coax`, `components/app/`, `page/filepane.coax`, `page/simpane.coax`) is compiled Coax running across **macOS Metal, Linux Vulkan, and Windows Vulkan**.

### Options Compared

| Criterion | Option U1: Pure Coax Lean Workbench (`page/gym-lean.coax`) + Lifted Coax Mode Bar | Option U2: macOS Swift `GymLeanView.swift` inside `GymWorkbenchView.swift` |
| --- | --- | --- |
| **Cross-platform support** | **macOS + Linux + Windows** identically (critical: serious Lean/Mathlib + GPU RL runs heavily on Linux hosts) | macOS only (`apps/openagents-desktop/windows/README.md` already notes Gym is absent on Windows; Linux also lacks it) |
| **Alignment with `AGENTS.md` & `docs/reuse.md`** | **Full compliance:** Coax-first product logic; reuses `components/app/`, `components/zeron/editor.coax` (`UiEditor`), `components/app/diff.coax`, `components/core/scroll.coax`, `components/paint/` | Violates Coax-first and `docs/reuse.md`: creates a second text editor (`NSTextView`), second diff viewer, and second log/list UI in Swift |
| **Tier 0 deterministic testing & `coax drive`** | **100% testable** in `coax test apps/openagents-desktop/page.coax --library` (`< 5 s`); every control has a semantic row in `ElTable` | Requires macOS GUI launch (Tier 3) to test UI logic; no Coax semantic tree |
| **Overlay occlusion (menus, ⌘K palette, dialogs, toasts)** | **Zero occlusion bugs** — drawn in the same GPU display list below `over` layer | Requires `setHoles` / native-view hiding whenever Coax popovers open over the Gym |
| **Large file editing (`> 16 KB`)** | `UiEditor` currently caps at `EB_TEXT = 16384` bytes (`512` lines in `filepane.coax`). Single-theorem Gym tasks are 1–6 KB (fits comfortably); larger files open in virtualized read-only mode up to 256 KB unless `EB_TEXT` is raised or chunked | `NSTextView` handles multi-MB files out of the box with native Find |
| **Unicode math input (`∀`, `∃`, `→`, `⊢`, `≤`, `ℕ`, `ℝ`)** | **Needs a small shared Coax abbrev helper** (`\forall` -> `∀`, `\to` -> `→`, etc., reusable by composer/filepane!) + Geist/fallback glyph shaping (already supported by `ct-atlas.m` / CoreText & FreeType glyph cache!) | Still needs a custom Lean abbrev handler; `NSTextView` also uses CoreText fallback |
| **Time to first working macOS prototype** | Medium (~1.5x U2 initially, then 0x for Linux/Windows and 0x for Tier 0 tests) | Fast on macOS alone; 3x total work once ported to Linux/Windows |
| **Live head-to-head Benchmarks view (`GymBenchmarkView`)** | Can stay mounted on macOS for Tab 0 (`Benchmarks`) while Tab 2 (`Lean`) renders in Coax; future Coax port replaces Tab 0 cleanly | Tab 0 (`Benchmarks`) and Tab 2 (`Lean`) both live inside `GymWorkbenchView.swift` |

### Analysis & Recommendation

- **Choose Option U1 (Coax-drawn `page/gym-lean.coax`) for the Lean Workbench tab.**
  - Unlike PLE (which required a custom 3D Metal mesh/cylinder renderer, justifying `PleScenePanelView` in Swift), a Lean proof workbench is **100% 2D UI**: run list, code buffer with gutter markers, tactic goal infoview, proof state node graph, unified diff, gate pills, and timeline scrubber.
  - Lifting the 36 pt `GYM` mode switcher bar (`Benchmarks` | `PLE 3D` | `Lean`) into `apps/openagents-desktop/page/gym.coax` gives a clean, incremental migration boundary:
    - When `gym_mode == CP_GYM_LEAN`: `coder_page_gym_rect` returns `false` (automatically hiding `GymWorkbenchView` on macOS!), and the Coax page draws the complete Lean Workbench natively on macOS, Linux, and Windows.
    - When `gym_mode == CP_GYM_BENCH` or `CP_GYM_PLE`: `coder_page_gym_rect` reports the rect *below* the 36 pt Coax mode bar; on macOS, `GymWorkbenchView` mounts there and switches directly to `benchmarkView` or `PLE 3D` (hiding its redundant internal `modeBar`). On Linux/Windows, the Coax page renders an honest platform status card for `PLE 3D` / macOS Metal benchmarks while keeping `Lean` fully usable.
- **AND integrate Lean Benchmark Suites into existing `GymBenchmarkView.swift` (Tab 0) in Phase 3.**
  - `GymBenchmarkView.swift` already embeds two live OpenAgents Coax GPU chat views (`GymChatSurfaceView`) racing Psionic Metal vs `llama.cpp` Metal.
  - Adding a `Lean` suite category to `GymBenchmarkView` reuses all of that live racing infrastructure (scorecards, delta banner, dual GPU chat transcripts, saved run replay) with minimal Swift delta, simply delegating grading to `lean-run verify` instead of `python3 -m unittest`!
- **Bonus low-cost win in `page/filepane.coax`:**
  - When any `.lean` file is opened from a chat transcript into the right-hand file pane (`page/filepane.coax`), attach an optional collapsible **Lean Goal Strip** at the bottom of the pane (queried from `oa-leanhost` on caret line change) + an **"Open in Lean Gym"** button. Zero duplication: both the file pane and the Lean Gym use the same `coax/lib/lean/` socket client.

### Handling the `EB_TEXT` (16 KB) & Unicode Constraints Cleanly

Two real technical details of U1 must be designed upfront rather than hand-waved:

1. **`EB_TEXT = 16384` bytes (`components/core/edit.coax:22`):**
   - Typical theorem-proving benchmark tasks (MiniF2F, ProofNet, PutnamBench, single-file proofs) are **0.5 KB to 6 KB** (20–200 lines) — well inside 16 KB.
   - Files between 16 KB and 256 KB (e.g., inspecting a Mathlib module) open in **virtualized read-only mode** with click-any-line goal inspection (reusing `cp_fp_read_view` from `page/filepane.coax`), displaying an honest banner ("Read-only above 16 KB; edit via chat or Open in Default App").
   - If larger editable proof files become common, extend `UiEditor` once in `components/core/edit.coax` (obeying `docs/reuse.md`) rather than writing a second editor.
2. **Lean Unicode Symbols & Backslash Abbreviations:**
   - Lean 4 heavily uses Unicode operators (`∀`, `∃`, `→`, `↔`, `⊢`, `≤`, `≥`, `≠`, `∧`, `∨`, `¬`, `⟨`, `⟩`, `ℕ`, `ℤ`, `ℚ`, `ℝ`, `∈`, `⊆`, `∘`, `λ`, `α`, `β`).
   - OpenAgents' GPU glyph cache (`apps/shared/host/apple/ct-atlas.m` on macOS, FreeType/HarfBuzz on Linux/Windows) already rasterizes fallback glyphs on demand for characters outside Geist's Latin set — **verify specifically in Tier 3** that `⊢`, `∀`, `∃`, `→`, `⟨`, `⟩`, `ℕ`, `ℝ` render without tofu boxes (a known desktop regression class; see recent commits `602f916b03`, `6dcd58cd27`, `a9581a215a`).
   - Add a small shared Coax abbreviation table (`coax/lib/lean/unicode.coax`, ~120 core Lean4 symbols like `\to` -> `→`, `\forall` -> `∀`, `\r` -> `→`, `\<` -> `⟨`, `\>` -> `⟩`, `\N` -> `ℕ`, `\R` -> `ℝ`, `\|-` -> `⊢`) triggered on `Tab` / `Space` inside Lean edit buffers, and verify it in Tier 0 unit tests.

---

## 4. Evaluation 3: Toolchain, Mathlib Cache & Resource Budgets

Lean 4's operational footprint is very different from Python or C projects. Ignoring it causes broken UX and OOMs.

| Dimension | Reality | Design Consequence in OpenAgents |
| --- | --- | --- |
| **Toolchain version pinning** | Every Lean project pins its exact compiler in `lean-toolchain` (e.g., `leanprover/lean4:v4.19.0` or `v4.20.0`). `elan` dispatches `lean`/`lake` by walking up to `lean-toolchain`. | `oa-leanhost` MUST key warm LSP sessions by `(canonical_lake_root, sha256(lean-toolchain))` and invoke `elan`'s shim (`~/.elan/bin/lake`, added to worker `PATH` in `dc_worker_path` alongside `~/.local/bin` and Homebrew!). |
| **Mathlib `.olean` build cost** | Building Mathlib from source takes 30–90 min; downloading prebuilt oleans via `lake exe cache get` takes 1–3 min and ~2–4 GB disk. | **Never** trigger an accidental source build of Mathlib. `lean-run doctor` detects missing Mathlib `.olean` cache before starting `lake serve`, warns clearly, and requires explicit user opt-in to run `lake exe cache get`. |
| **Bundled self-contained fixtures** | Users trying the Lean Gym for the first time shouldn't need a 4 GB Mathlib download just to prove a theorem. | Ship a **pure Lean 4 `Init`-only sample pack** (`resources/fixtures/lean-samples/`: arithmetic, list induction, propositional logic, group-like algebraic identities) that compiles and kernel-verifies in `< 2 s` using only Lean's built-in `Init`. |
| **RAM footprint per `lake serve`** | ~150 MB for `Init`-only projects; **1.5–4.0 GiB** for Mathlib projects. | `oa-leanhost` enforces a strict bounded workspace pool (`OA_LEAN_MAX_WORKSPACES`, default `2`), LRU eviction, RSS monitoring (warning/recycling above `OA_LEAN_MAX_RSS_MIB`, default `6144 MiB`), and automatic shutdown after 10 min idle with no active clients. |
| **Multi-process contention** | Running `lake build` while `lake serve` writes `.lake/build` can race or stall elaboration. | `oa-leanhost` serializes workspace mutations per Lake root via a per-workspace lock and runs authoritative `lean-run verify` builds into an isolated scratch/verifier build directory (or clean copy) so interactive LSP caches are never corrupted. |

---

## 5. Evaluation 4: Verification Soundness & Adversarial Threats

In AI theorem proving, **naive compilation checks are routinely gamed** by both stochastic LLMs and adversarial search loops. A Lean Gym that marks a `sorry` or `native_decide` proof as "PASS" violates `AGENTS.md` (*"Done means the real thing works"*, *"No UI that lies"*).

| Cheat / Failure Vector | Why `lake build` Misses It | How OpenAgents Blocks It (`verification-and-security.md`) |
| --- | --- | --- |
| **`sorry` / `admit`** | Compiles with exit code `0` (only emits a warning: `declaration uses 'sorry'`). | Kernel axiom audit rejects `sorryAx` unconditionally; diagnostic gate `no_sorry` fails. |
| **Custom `axiom`** | Compiles with exit code `0` (`axiom cheat : False`). | Strict axiom allowlist: ONLY `{propext, Quot.sound, Classical.choice}` permitted (configurable stricter mode `{}` for constructive proofs, never looser in benchmark grading). |
| **`native_decide` / `@[implemented_by]` / `@[extern]`** | Compiles with exit code `0`; relies on compiler/native code rather than the logical kernel (known unsoundness exploits via `@[implemented_by]`). | Rejects `Lean.ofReduceBool`, `Lean.trustCompiler`, and any `unsafe`/`implemented_by`/`extern` transitive dependency in target declarations. |
| **Theorem statement tampering** | Agent edits the problem statement to `theorem foo : True := trivial` (or weakens a hypothesis, changes universe levels, or renames the theorem). | **Frozen specification check:** Verifier loads the pristine immutable `spec.lean` (SHA-256 pinned in task manifest) and checks kernel-level definitional/structural equality of the target constant's name, universe parameters, and type against the submitted `.olean` (the `SafeVerify` / `Comparator` pattern). |
| **Metaprogramming kernel bypass (`#eval`, custom elaborators modifying `Environment`)** | Arbitrary Lean metaprograms execute during elaboration and can inject unchecked declarations or exfiltrate files. | (1) Independent kernel replay (`lean4checker` / `SafeVerify`) replays every `.olean` constant through a fresh trusted kernel with zero user macros; (2) Benchmark verification runs compile & replay in a networkless sandboxed directory/container so `#eval` cannot read `~/.openagents/` secrets or phone home. |
| **Stale LSP diagnostics** | Reading diagnostics before `$/lean/fileProgress` clears returns empty errors on an unchecked file. | `coax/lib/lean/lsp.coax` tracks document `version` and waits until `$/lean/fileProgress` reports `processing: []` for that exact `version` before emitting a settled check result. |

---

## 6. Evaluation 5: Synergy With the OpenAgents Stack

Implementing Lean cleanly in the Gym multiplies the value of systems OpenAgents *already built*:

1. **Psionic Metal 4 & Inkling 2 Mini (`GymBenchmarkView.swift`, #13920):**
   - Today's Gym Benchmarks race Inkling 2 Mini on Psionic Metal vs `llama.cpp` Metal across 10 Python/C/JS/Bash tasks.
   - Formal theorem proving is a **high-thinking, multi-turn, tool-dense workload** (short edits, rapid LSP feedback, deep chain-of-thought) — an ideal stress test for Psionic's prefix caching, TTFT, and hybrid executor (`hybrid-exec-v1`).
2. **Closed-Loop Verified-Trace Distillation (ATIF-v1.8 -> W3.2 -> W3.1 LoRA -> Gym):**
   - Every solved Lean task generates a verified proof trajectory (`lean-trace.v1`) paired with a cryptographic kernel-replay receipt (`lean-verify.v1`).
   - `lean-run export-atif` (sharing `coax/lib/nostr/atif/{log,digest}.coax` with `proof-export`) exports canonical ATIF-v1.8 trajectories containing only **kernel-verified** reasoning & tactic steps.
   - Psionic W3.2 (`#13918`) can distill directly from those traces into a Lean-specialized LoRA adapter, hot-swapped onto Inkling 2 Mini via W3.1 (`#13920`), and immediately raced side-by-side against the base model inside the Gym `Benchmarks` tab!
3. **Coax Capability Authoring (`capability-eval`, #13896):**
   - Lean tactic skills (e.g., `mathlib-algebra-tactics`, `lean-induction-patterns`) can be evaluated two-arm (`baseline` vs `candidate`) using `lean-run verify` as the deterministic positive/held-out grader inside `capability-eval`.
4. **Universal Agent Access (`lean-run` CLI + `lean-run mcp`):**
   - Microcoder uses `lean-run` shell verbs (just like `ple-run` and `oasim`).
   - Claude Code sub-agents (`claudeworker.coax`) and Antigravity (`agyworker.coax`) connect to `lean-run mcp` (or `openagents mcp --domains lean`), reusing the exact `OmTool` / JSON-RPC 2.0 pattern of `coder/tui/oasim-mcp.coax`.
   - Foreman (`foreman-ctl gym lean ...` / `POST /v1/gym/lean`) can orchestrate long-running proof search chats and inspect results in the Gym automatically.

---

## 7. Summary Recommendation Matrix

| Phase | Core Deliverable | Primary Value Unlocked |
| --- | --- | --- |
| **Phase 0** | `coax/lib/lean/` (`policy.coax` `#authority`, `verify.coax`, `wire.coax`) + `lean-run` CLI (`doctor`, `verify`, `sample`) + `Init`-only fixtures | Real, zero-cheat Lean 4 kernel verification from CLI & chat; honest toolchain diagnostics |
| **Phase 1** | `oa-leanhost` (`lean-run serve`) warm `lake serve` LSP pool + `check`, `goal`, `try`, `hover`, `def`, `search` + `lean-run mcp` | Sub-300 ms interactive tactic/goal feedback shared across Desktop + Microcoder + Claude Code |
| **Phase 2** | Cross-platform Coax **Lean Gym Workbench** (`page/gym-lean.coax` + lifted `GYM` mode bar) + `lean-run gym` + `/v1/gym/lean` + `OA-SHOW` cards + File Pane goal strip | Full visual Proof Workbench (Source + Infoview + Proof DAG + Timeline + Axiom Gates + Compare Diff) on Mac, Linux & Windows |
| **Phase 3** | **Lean Benchmark Suites** in `GymBenchmarkView` (head-to-head dual GPU chats) + `lean-run export-atif` -> Psionic W3.2 distillation loop | Live side-by-side formal proving races (Psionic vs `llama.cpp` / LoRA / Cloud) + verified RL/distillation data pipeline |
| **Phase 4** | Optional `Pantograph` / `LeanREPL` branching tactic engine + containerized `lean-run bench start` (MiniF2F / PutnamBench / remote hosts) | High-throughput MCTS/tree-search proof gyms and sandboxed large-scale formal math benchmarks |
