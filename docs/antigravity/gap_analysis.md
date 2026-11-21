# Architectural Gap Analysis: Antigravity vs. MicroCoder

This report compares Google's **Antigravity** desktop agent architecture (`v2.34.0` on Electron `44.9.1`, as documented in [`README.md`](./README.md), [`modules.md`](./modules.md), and [`inference-and-agent-loop.md`](./inference-and-agent-loop.md)) against **MicroCoder**, OpenAgents' bounded-memory, zero-heap-allocation coding engine (`coax/lib/microcoder/`, `coder/tui/mcworker.coax`, `coder/tui/memory.coax`, [`docs/coder/microcoder.md`](../coder/microcoder.md), and [`docs/protocol/microcoder-journal-ajr1.md`](../protocol/microcoder-journal-ajr1.md)).

The analysis covers four operational pillars—**Execution Flow**, **Agent Loop Mechanism**, **Tool Calling Interfaces**, and **Persistence**—highlighting structural trade-offs, capabilities Antigravity has that MicroCoder lacks, invariants where MicroCoder is stronger, and targeted patterns MicroCoder can adopt without compromising its determinism.

---

## Executive Summary

Antigravity and MicroCoder optimize for fundamentally different points in the local agent design space:

- **Antigravity (`com.google.antigravity` `v2.34.0`)** is a **multi-plane desktop agent host and actuation platform**. An Electron `44.9.1` Hub shell (`dist/main.js`, `20.7 MB` uncompressed JS across ~125 modules) supervises a `196.9 MB` Go `language_server` sidecar plus native macOS sidecars (`agy-desktop-helper`, `agy-desktop-script`, `cable_ble_scanner`). The Go sidecar owns all LLM communication and runs the **Cascade** trajectory loop (dual `planner_model` / `executor_model` slots, 100+ `CortexStep` types, `TOOL_SEARCH` dynamic tool selection, and explicit token/invocation budgets), while calling *back* into the Electron main process over **Connect-RPC + Protobuf** (`actuator_pb.js`, `computer_viewer_pb.js`, `browser_viewer_pb.js`) to drive Chrome via CDP, macOS desktop UI, scripted sandboxes (`scriptJail.js`), and SSH remote hosts (`dist/remote/`).
- **MicroCoder (`coax/lib/microcoder/`)** is a **zero-heap, single-schema state machine** compiled natively in Coax. A host UI (`coder` TUI, OpenAgents Desktop, or CLI harness) drives a single-threaded `mcworker` process over `stdin`/`stdout` JSONL (`proto/coder/microcoder-worker-lines-v1.json`). Each turn runs a deterministic local router (`gw_plan`) and zero-LLM Step-0 workspace seed (`mc_seed`), then iterates a bounded loop where one grammar-constrained JSON response (`NextAction`) batches exact-match edits, iOS Simulator operations, `/bin/sh` commands, ranged file views, and parallel read-only sub-agent delegations in a single round-trip—backed by deterministic 3-tier byte compaction, machine-verified completion gates (`mc_finish_gate`), and a SHA-256 hash-chained `AJR1` binary journal.

### Summary Matrix

| Dimension | Antigravity (`v2.34.0`) | MicroCoder (`coax/lib/microcoder`) | Primary Gap Direction |
| :--- | :--- | :--- | :--- |
| **Process Topology** | Remote Hub renderer + Electron main (`20.7 MB` JS) + `196.9 MB` Go `language_server` + 3 Mach-O helpers | Owner UI <-> single-threaded `mcworker` native Coax binary (`McStateBuffers` static memory, zero heap) | MicroCoder leads on footprint & auditability |
| **Host <-> Engine IPC** | Loopback HTTPS + pinned cert + `x-codeium-csrf-token` (UI -> LS) **and** reverse Connect-RPC Actuator (LS -> Electron main) | Unidirectional `stdin`/`stdout` JSONL pipe (`microcoder-worker-lines-v1.json`) | Antigravity leads on bidirectional host actuation |
| **Upstream Inference** | Go LS -> `daily-cloudcode-pa.googleapis.com` (`v1internal:streamGenerateContent`, OAuth) or `generativelanguage.googleapis.com` (`x-goog-api-key`); Gemini + Claude/GPT enums | `provider.coax` direct HTTPS -> Gemini (`generativelanguage` / Cloud Code OAuth) or OpenRouter | Comparable; both split Cloud Code OAuth vs. API-key paths |
| **Pre-Loop Orientation** | Enters `StartCascade` -> LLM step; relies on `TOOL_SEARCH` and prior `brain/<cascadeId>/` context | Local `gw_plan` router (`local_work` read-only gate) + deterministic Step-0 `mc_seed` (repo tree, named files, Coax diagnostics) | MicroCoder saves 1 full LLM round-trip per task |
| **UI Progress Delivery** | Reactive server-push RPCs (`StreamCascadeReactiveUpdates`, `StreamAgentStateUpdates`, `StreamTerminalOutput`) | `thinking` heartbeats during HTTP generation; atomic `step` JSONL emitted once full `NextAction` parses & executes | Antigravity leads on token-by-token live streaming |
| **Loop Unit & Roles** | `Cascade` trajectory of 100+ typed `CortexStep`s; separate `planner_model` (`READ_ONLY`) and `executor_model` slots | Unified `NextAction` batch step (`edits -> device -> commands -> view -> delegate`) in `mc_loop` | Antigravity separates read-only planning; MicroCoder cuts round-trips |
| **Termination & Gates** | Named `EXECUTOR_TERMINATION_REASON_*` (`NO_TOOL_CALL`, `MAX_INVOCATIONS`, `MAX_TOKEN_BUDGET_EXCEEDED`, `HOOK_REJECTED`) + `NEEDS_PERMISSION` / `WAITING` human gates | `max_steps` cap + **hard `mc_finish_gate`** (refuses `finished: true` until task backtick checks exit `0` post-edit & Coax checks pass; `git commit` `last_check`) | Antigravity has token budgets & permission states; MicroCoder enforces post-edit test verification |
| **Context Compaction** | LLM checkpointer at `max_token_limit: 256000` (`CHECKPOINT_STRATEGY_SAME_MODEL`), emitting visible `CORTEX_STEP_TYPE_CHECKPOINT` | Zero-LLM 3-tier byte compaction (`32 KB`/step cap, `140 KB` `actions`, `earlier`, `left_out`) + KV-prefix-aligned `prompt.coax` + line-invalidated `McAware` | MicroCoder eliminates summary drift & extra LLM calls |
| **Tool Calling Model** | Streamed provider `functionCall` -> `ToolCallStep` (`tool_name` + `arguments_json`), `TOOL_SEARCH` + `TOOL_CALL_CHOICE`, MCP (`CallMcpAppTool` / WebMCP) | Single `responseJsonSchema` `NextAction` envelope (`proto/coder/microcoder-prompt-v1.json` + `text_action.coax` fallback); conditional `device`/`delegate` fields | Antigravity leads on large/dynamic MCP tool catalogues |
| **Desktop, Web & Remote** | Chrome CDP (`browserEngine/` + `browserStream/`), macOS desktop (`agy-desktop-helper`, `scriptJail.js`), WebRTC (`desktopHost/`), 34-module SSH stack (`remote/`) | Conditional 18-op `device` array (`device.coax` -> `oasim` iOS Simulator CLI + auto-settle screenshot); local `/bin/sh` with `McFence` | Antigravity leads on browser, macOS GUI & SSH; MicroCoder has native iOS Simulator |
| **Persistence & Replay** | `<appDataDir>/brain/<cascadeId>/.system_generated/logs/transcript.jsonl` & `transcript_full.json`, `browser_sandbox/`, `GetCascadeTrajectory` reattach, `revert_to_step_called`, `[ForkConversation]` | SHA-256 hash-chained `AJR1` binary journal (`0600`, poison-on-fail) + `<journal>.out/` raw command spills + `turns.jsonl`/`turns.bin` RRF semantic recall + persistent `MwQueue` | MicroCoder leads on cryptographic integrity; Antigravity leads on trajectory fork/revert |

---

## 1. Execution Flow

### 1.1 Antigravity Execution Flow

As documented in [`README.md`](./README.md) (§§2–9) and [`inference-and-agent-loop.md`](./inference-and-agent-loop.md) (§§1, 5, 7, 8), Antigravity is **not** a traditional bundled IDE editor—it is an Electron `44.9.1` Hub shell whose renderer loads a remote UI (`entryUrl`) while coordinating local execution across three local process tiers:

1. **Hub Renderer (`BrowserWindow` + `WebContentsView` compositor)**: Communicates with Electron main through a narrow `contextBridge` (`dist/preload.js`) and connects directly to the loopback `language_server` over HTTPS using a pinned certificate (`pinnedLsCert.js`) and per-boot `x-codeium-csrf-token`. It starts runs via `LanguageServerService/StartCascade` and subscribes—without polling—to push streams (`StreamCascadeReactiveUpdates`, `StreamCascadePanelReactiveUpdates`, `StreamAgentStateUpdates`, `StreamTerminalShellCommand`, `StreamTerminalOutput`).
2. **Electron Main Supervision & Actuator Plane (`dist/main.js`, `languageServer.js`, `actuator/`)**: Spawns `language_server` with `--standalone` and `ANTIGRAVITY_BOOT_TOKEN`, honors self-restart exit code `42`, manages 34 SSH remote-bootstrap modules (`dist/remote/`, freezing SSH windows across updates), enforces guest `<webview>` / `WebContentsView` security (`wcvZOrderController.js`, `browserViewerUrlPolicy.js`), and serves **Connect-RPC + Protobuf** Actuator endpoints (`actuator_pb.js`, `computer_viewer_pb.js`, `browser_viewer_pb.js`, `host_bridge_pb.js`).
3. **Go `language_server` Sidecar (`196.9 MB` binary)**: Owns 100% of prompt assembly, model routing, and the Cascade loop. Upstream requests go either to Google's internal Cloud Code Assist gateway (`daily-cloudcode-pa.googleapis.com` / `cloudcode-pa.googleapis.com`, `v1internal:streamGenerateContent` with OAuth bearer auth) or `generativelanguage.googleapis.com` (`x-goog-api-key`). Local file/search/shell tools run inside Go; GUI and browser tools execute via a **reverse Connect-RPC round-trip** back to Electron main and the native helpers (`agy-desktop-helper`, `agy-desktop-script`).

```text
[Remote Hub UI Renderer] --HTTPS + Pinned Cert + CSRF--> [Go language_server (Cascade Loop)]
          ^                                                       |                    |
          | StreamCascadeReactiveUpdates                          | Connect-RPC        | HTTPS SSE
          | StreamTerminalOutput                                  v (Actuator)         v
          +--- IPC (preload.js) --- [Electron Main + Chrome CDP + agy-desktop-*]   [CloudCode PA /
                                                                                    Gemini API]
```

### 1.2 MicroCoder Execution Flow

MicroCoder (`coder/tui/mcworker.coax`, `coax/lib/microcoder/loop.coax`, `coax/lib/microcoder/run.coax`) eliminates browser runtimes, reverse RPC servers, and dynamic memory allocation in favor of a **strictly hierarchical two-process pipe**:

1. **Owner Process** (`coder` TUI, OpenAgents Desktop, or CLI runner): Renders UI, manages git worktrees, and drives a dedicated `mcworker` child process over `stdin`/`stdout` JSONL (`proto/coder/microcoder-worker-lines-v1.json`).
2. **Worker Process (`mcworker`)**: Operates entirely inside statically sized Coax structs (`McStateBuffers`, `McLoopBuffers`). Its poll loop interleaves active HTTP/step execution with `stdin` control frames (`task`, `steer`, `interrupt`, `queue`, `answer`, `stop`, `embed`, `recall`, `forget`) and emits structured `stdout` events (`ready`, `journal`, `turn`, `route`, `thinking`, `step`, `ask`, `steered`, `queue`, `done`, `error`).

```text
[Owner: TUI / Desktop / Eval] ==stdin/stdout JSONL==> [mcworker (Static Coax Buffers)]
                                                               |
                                   +---------------------------+---------------------------+
                                   |                           |                           |
                                   v                           v                           v
                        [Local Router (gw_plan)     [Direct HTTPS: Gemini /     [Direct OS: FS edits,
                         + Step-0 mc_seed]           OpenRouter (provider.coax)] /bin/sh, oasim, sub-runs]
```

Each turn passes through four deterministic stages:
- **Dispatch (`mw_dispatch`)**: Pops an interrupted turn's combined steers (`MW_SRC_RESUBMIT`), the next item from the workspace `MwQueue` (`MW_SRC_QUEUE`), or a direct `task` message.
- **Local Routing & Step-0 Seeding (`gw_plan` + `mc_seed`)**: Computes `route`, `difficulty`, `effort`, and `local_work` locally. When `route.local_work < 0.5`, `mc_read_only` disables shell commands. Before calling the LLM, `mc_seed` deterministically populates **Step 0** with the repository file listing, slices of files named in the prompt, and Coax compiler diagnostics (`mc_aware_seed`).
- **Atomic Batch Step (`mc_run_step`)**: Sends a prefix-cached JSON-schema request (`chat_loop.coax`), parses the complete `NextAction` response, drains pending mid-turn steers (`mw_drained`), executes `edits -> device -> commands -> view -> delegate` directly against the local OS, writes `AJR1` journal frames, and emits one `step` event.
- **Turn Completion (`mw_turn_end`)**: Enforces `mc_finish_gate`, handles residual steers or queue pausing on error, and emits `done`.

### 1.3 Execution Flow Gaps

- **Gap EF-1 — Reverse Host Actuation & Remote SSH Bootstrap (Antigravity -> MicroCoder gap)**: Antigravity's Go loop can call *upward* into Electron main over Connect-RPC (`actuator_pb.js`) to drive streamed Chrome tabs, macOS desktop windows, WebRTC remote takeovers (`desktopHost/`), and remote SSH hosts where `dist/remote/` automatically provisions and tunnels to a remote `language_server`. MicroCoder executes strictly downward from `mcworker` into local files, `/bin/sh`, and `oasim` within its local `cwd`.
- **Gap EF-2 — Token-Granular Reactive Streams vs. Atomic Step Commits (Antigravity -> MicroCoder gap)**: Antigravity streams partial thoughts, prose tokens, and live terminal output chunks via `StreamCascadeReactiveUpdates` and `StreamTerminalOutput`, and allows a reloaded UI to reattach mid-flight via `GetCascadeTrajectory`. MicroCoder emits `thinking` status pulses during generation and commits each `step` atomically after JSON validation and tool execution—preventing partial/malformed tool UI state at the cost of intra-step streaming granularity.
- **Gap EF-3 — Deterministic Step-0 Seeding & Local-Work Routing (MicroCoder -> Antigravity gap)**: MicroCoder's `gw_plan` + `mc_seed` eliminates the cold-start orientation turn by injecting workspace structure, named file ranges, and Coax compiler state before the first LLM request, and enforces zero-command execution when `local_work < 0.5`. Antigravity enters `StartCascade` directly into LLM inference.
- **Gap EF-4 — Mid-Turn `steer` Injection & `interrupt` Auto-Resubmit (MicroCoder -> Antigravity gap)**: `mcworker` (`coder/tui/mcworker.coax:230-331`, `coax/lib/microcoder/steer.coax`) distinguishes non-destructive between-step steering (`steer`, drained into `McState` before the next prompt) from preemptive steering (`interrupt`, canceling the in-flight step and automatically resubmitting joined steers via `MW_SRC_RESUBMIT`) and persistent `MwQueue` backlog management, while protecting sub-agent turns (`active_turn_not_steerable`). Antigravity exposes `USER_CANCELED` and reactive trajectory updates, without a comparable split between non-preemptive intra-loop steer injection and automatic interrupt-resubmission.
- **Gap EF-5 — Source Auditability vs. Opaque Sidecar Binary (MicroCoder -> Antigravity gap)**: As noted in `inference-and-agent-loop.md` §9, Antigravity's entire loop, prompt construction, and safety routing live inside an unstripped `196.9 MB` closed Go binary, whereas MicroCoder's entire engine (`~12 KLOC` of Coax) is source-auditable and runs with zero dynamic heap allocation.

---

## 2. Agent Loop Mechanism

### 2.1 Antigravity Agent Loop (`Cascade`)

Antigravity models an agent run as a **Cascade trajectory** (`inference-and-agent-loop.md` §§3–6):

- **Steps as the First-Class Unit (`CortexStep`)**: Rather than a flat `[user, assistant, tool]` chat array, a trajectory is an ordered sequence of 100+ typed `CortexStep` records (`CORTEX_STEP_TYPE_PLANNER_RESPONSE`, `THOUGHT`, `CODE_ACTION`, `VIEW_FILE`, `RUN_COMMAND`, `GREP_SEARCH`, `CODE_SEARCH`, `BROWSER_SUBAGENT`, `SUBAGENTS`, `MCP_TOOL`, `TOOL_SEARCH`, `CHECKPOINT`, `MEMORY`, `KNOWLEDGE_ARTIFACTS`, `COMPILE_APPLET`, etc.).
- **Planner + Executor Model Split**: Cascade configures distinct `planner_model` and `executor_model` slots (`RequestedModel` / `ModelOrAlias`), supporting `PLANNER_MODE` (`READ_ONLY`) so a frontier thinking tier can plan without mutating workspace state before handing off to an execution tier.
- **Permission States & Named Termination Contract**: Steps pause in `NEEDS_PERMISSION` / `WAITING` when human approval is required. Loop exit is governed by an explicit enum (`EXECUTOR_TERMINATION_REASON_NO_TOOL_CALL`, `MAX_INVOCATIONS`, `MAX_TOKEN_BUDGET_EXCEEDED`, `USER_CANCELED`, `ERROR`, `HOOK_REJECTED`, plus `EXECUTOR_LOOP_BROKEN`).
- **Token-Threshold Checkpointer (`256,000` tokens)**: Long runs use `CASCADE_USE_EXPERIMENT_CHECKPOINTER` (`{"strategy": "CHECKPOINT_STRATEGY_SAME_MODEL", "max_token_limit": "256000"}`) to summarize history at a mid-hop token threshold and append a user-visible `CORTEX_STEP_TYPE_CHECKPOINT` step.
- **Nested Subagents**: `CORTEX_STEP_TYPE_SUBAGENTS` (`SubagentsCall`, with isolated subagent snapshot directories), `BROWSER_SUBAGENT`, and `KNOWLEDGE_ARTIFACTS` spawn specialized child loops inside the Go server.

### 2.2 MicroCoder Agent Loop (`mc_loop`)

MicroCoder (`coax/lib/microcoder/loop.coax`, `state.coax`, `aware.coax`, `delegate.coax`) implements a **single-schema, batch-per-step state machine**:

- **Batched `NextAction` Turn**: Every LLM call produces one JSON object (`coax/lib/microcoder/action.coax`) with `rationale`, `notes` (up to 8), `edits` (up to 8), optional `device` (up to 8), `commands` (up to 8), `view` (up to 8), optional `delegate` (up to 4), optional `todos` (up to 12), `finished`, `reply`, and `ask`. A single step routinely applies an edit, runs a build/test command, and views neighboring lines in one inference hop.
- **Zero-LLM 3-Tier Byte Compaction & Prefix Alignment**:
  1. **Per-step cap (`~32 KB`, `MC_STEP_BUDGET`)**: Oversized views or outputs in a single step are marked `held` with exact range instructions rather than overflowing context.
  2. **Recent window (`~140 KB` `actions`)**: Retains full numbered views, edit diffs, and command outputs for recent steps.
  3. **Compacted history (`earlier` -> `left_out`)**: Older steps deterministically shed bulk file text while keeping `rationale`, edited paths/line ranges, command exit codes + head/tail snippets, and viewed byte sizes; oldest steps eventually roll into `left_out`. `prompt.coax` serializes static rules and older tiers first to preserve provider KV-prefix caching.
- **File-Anchored Working Memory (`McAware`, `aware.coax`)**: Maintains a never-compacted `memory` summary containing `findings` (up to 8 `PATH:A-B` notes that automatically invalidate when those lines change on disk), `changed` (all edits so far), and `open` (active completion blockers).
- **Machine-Verified Finish Gates (`mc_finish_gate`)**: When the model sets `finished: true`, `mc_loop` checks whether every backtick-quoted check command named in the task has exited `0` *after* the last file edit and whether Coax awareness reports clean compilation on modified `.coax` files. If not, `finished: true` is **refused**, `memory.open` explains what check is missing or failing, and the loop continues. `git commit` results similarly inject `last_check`.
- **Parallel Read-Only Delegation (`delegate.coax`)**: `mc_delegate_batch` runs up to 4 isolated read-only child `McSubRun` loops per step, returning concise answers without polluting the parent's `140 KB` window.

### 2.3 Agent Loop Gaps

- **Gap AL-1 — Dual-Model `READ_ONLY` Planner + Executor Split (Antigravity -> MicroCoder gap)**: Antigravity natively separates `planner_model` (including enforced `READ_ONLY` planning mode) from `executor_model`, and models `NEEDS_PERMISSION` / `WAITING` inside step state. MicroCoder uses one model configuration per `mc_loop` run (selected up front via `gw_plan`) and embeds planning inside `rationale`, `todos`, and `notes`.
- **Gap AL-2 — Token-Budget Accounting & Hook Rejection Reasons (Antigravity -> MicroCoder gap)**: Antigravity enforces cumulative `MAX_TOKEN_BUDGET_EXCEEDED` and `HOOK_REJECTED` termination states alongside `MAX_INVOCATIONS`. MicroCoder bounds step count (`max_steps`), per-command wall time (`command_seconds`), and prompt state byte size (`140 KB`), but does not enforce a cumulative billing-token ceiling inside `mc_loop`.
- **Gap AL-3 — Deterministic Byte Compaction + Staleness Invalidation vs. `256k` LLM Checkpointer (MicroCoder -> Antigravity gap)**: Antigravity's `CHECKPOINT_STRATEGY_SAME_MODEL` requires an extra LLM generation at `256k` tokens and relies on natural-language summarization. MicroCoder compacts deterministically in microseconds with zero LLM cost, bounded static RAM, warm KV prefixes, and automatic line-level invalidation of stale `McAware` notes.
- **Gap AL-4 — Hard Post-Edit Verification Gate (`mc_finish_gate`) (MicroCoder -> Antigravity gap)**: Antigravity terminates normally whenever the executor emits `NO_TOOL_CALL`. MicroCoder structurally refuses premature `finished: true` until task-specified check commands pass *after* the final edit.

---

## 3. Tool Calling Interfaces

### 3.1 Antigravity Tool Calling Interface

Antigravity uses **native provider function calling** (`functionCall` parts parsed into `ToolCallStep { tool_name, arguments_json }`, documented in `inference-and-agent-loop.md` §5 and `README.md` §8):

- **Dynamic Tool Catalogue (`TOOL_SEARCH` + `TOOL_CALL_CHOICE`)**: Because Antigravity integrates dozens of built-in tools plus external MCP servers, `language_server` uses `CORTEX_STEP_TYPE_TOOL_SEARCH` (`TOOL_SEARCH` / `TOOL_CALL_CHOICE`) to search and bind relevant tool declarations on demand rather than stuffing every schema into every prompt.
- **Rich Specialized Built-In Vocabulary**:
  - *Code & AST*: `read_file` / `view_file`, `view_code_item`, `view_file_outline`, `edit_file`, `write_to_file`, `replace_file_content`, `multi_replace_file_content`, `code_action`.
  - *Search & Web*: `grep_search`, `find_by_name`, `code_search`, `list_directory`, `search_web`, `fetch_web_page`, `read_url_content`, `view_content_chunk`.
  - *Interactive Terminal*: `run_command`, `command_status`, `send_command_input`, `read_terminal`, `bg_command_input`.
  - *Host Actuation (Connect-RPC -> Electron)*: `computer_use` (`computerActuatorHandler.js` -> `macComputerDriver.js`, `agy-desktop-helper`, `agy-desktop-script` inside `dist/scriptJail.js`) and `browser_*` (`browserEngine.js` over Chrome CDP pipe + `browserStream/` capture & `browserViewerUrlPolicy.js`).
  - *App Lifecycle & Media*: `generate_image`, `compile_applet`, `deploy_web_app`, `check_deploy_status`.
  - *MCP & WebMCP*: `LanguageServerService/CallMcpAppTool` (`CORTEX_STEP_TYPE_MCP_TOOL`) plus guest-page `guestWebMcpController.js`.

### 3.2 MicroCoder Tool Calling Interface

MicroCoder uses **grammar-constrained structured output** (`responseJsonSchema` / `json_schema` defined in `proto/coder/microcoder-prompt-v1.json`, with `coax/lib/microcoder/text_action.coax` fallback parsing) rather than provider `functionCall` parts:

- **`edits` (`tools.coax`)**: Up to 8 `{path, old, new}` exact replacements (`old: ""` writes the full file). Non-unique or missing `old` fails atomically and returns numbered nearest-match context lines.
- **`commands` (`tools.coax`)**: Up to 8 sequential `/bin/sh` commands run in `cwd` under `command_seconds` timeout. Long outputs are truncated to head + tail inline while the **entire raw output** is written to `<journal>.out/s<step>-c<cmd>.txt`, named directly in the cut notice so `view` can inspect any line range.
- **`view` (`tools.coax`)**: Up to 8 `PATH` or `PATH:A-B` entries (300-line default cap, unchanged-view deduplication header) plus multimodal image loading (`.png`, `.jpg`, `.jpeg`, `.webp`, `.gif`).
- **Conditional Profile Extensions (`action.coax`, `device.coax`, `delegate.coax`)**: Optional schema properties (`device`, `delegate`, `ask` suppression) are compiled into the JSON Schema only when enabled for the session, keeping baseline schema overhead minimal (`~1.5 KB`). `device` exposes 18 typed iOS Simulator operations (`tap`, `double_tap`, `long_press`, `swipe`, `type`, `key`, `button`, `describe`, `screenshot`, `launch`, `terminate`, `install`, `open_url`, `boot`, `list`, `erase`, `build`, `record`) lowered to `oasim` CLI invocations with an automatic `300 ms` settle screenshot.
- **Workspace Fence (`fence.coax`)**: `McFence` enforces lexical and path boundary checks across edits, views, and shell commands.

### 3.3 Tool Calling Gaps

- **Gap TC-1 — `TOOL_SEARCH` & First-Class MCP / WebMCP Integration (Antigravity -> MicroCoder gap)**: Antigravity can attach arbitrary MCP servers (`CallMcpAppTool`, `guestWebMcpController.js`) and scale to massive tool libraries via `TOOL_SEARCH` without linear prompt-schema growth. MicroCoder's `NextAction` schema is intentionally fixed in Coax; external capabilities must be invoked as CLI tools through `/bin/sh` `commands`.
- **Gap TC-2 — Stateful Background PTYs & Stdin Driving (Antigravity -> MicroCoder gap)**: Antigravity's `run_command` + `command_status` + `send_command_input` + `read_terminal` let the loop spawn long-lived dev servers or interactive REPLs and feed `stdin` across turns. MicroCoder's `commands` are synchronous one-shot `/bin/sh` invocations bounded by `command_seconds`.
- **Gap TC-3 — Browser CDP, macOS Desktop Control & Script Jail vs. iOS Simulator `device` (Bidirectional asymmetry)**: Antigravity ships full Chrome CDP automation (`browserEngine/` + URL policy), macOS desktop `computer_use` (`agy-desktop-helper`), and jailed script execution (`agy-desktop-script` + `scriptJail.js`), whereas MicroCoder ships native schema-typed iOS Simulator actuation (`device.coax` -> `oasim`) and relies on CLI tools for web/desktop tasks.
- **Gap TC-4 — Semantic `code_search` & AST Outline Tools vs. `git grep` + Ranged `view` (Antigravity -> MicroCoder gap)**: Antigravity exposes `code_search`, `view_file_outline`, and `view_code_item` directly to the model; MicroCoder relies on fast `git grep -n` via `/bin/sh` plus `PATH:A-B` views (supplemented by Coax symbol awareness for `.coax` files).
- **Gap TC-5 — Dependent Multi-Stage Single-Turn Batching & Lossless Spill Views (MicroCoder -> Antigravity gap)**: Provider `functionCall` turns can emit parallel calls, but normally require a new LLM turn to run a test *after* seeing whether an edit succeeded or to view a file *after* generating it. MicroCoder's ordered `edits -> device -> commands -> view -> delegate` batch routinely executes an edit, runs its test suite, and reads a reference slice in one LLM round-trip, with lossless `<journal>.out/` range inspection on huge build logs.

---

## 4. Persistence

### 4.1 Antigravity Persistence

Antigravity persists trajectories and host state across local files and Electron managers (`README.md` §§2, 6, `modules.md`, `inference-and-agent-loop.md` §6):

- **Per-Cascade Trajectory Directory (`<appDataDir>/brain/<cascadeId>/`)**:
  - `.system_generated/logs/transcript.jsonl` (incremental step log) and `.system_generated/logs/transcript_full.json` (complete serialized trajectory, stored unencrypted in application support).
  - `browser_sandbox/<cascadeId>/...` (isolated per-cascade browser artifacts).
  - **Cross-Cascade Read-Only Rule**: Prompts and runtime rules enforce that any parent or sibling `brain/<other_id>` directory is strictly **read-only reference only**.
- **Trajectory Reattachment, Revert & Forking**: `LanguageServerService/GetCascadeTrajectory` reconstructs live or historical runs on UI reload; the runtime supports step-level rollback (`revert_to_step_called`), conversation branching (`[ForkConversation]`), visible `CORTEX_STEP_TYPE_CHECKPOINT` compaction boundaries, and persistent `CORTEX_STEP_TYPE_MEMORY` / `KNOWLEDGE_ARTIFACTS` records (with subagent snapshots stored alongside).
- **Host & Credential State**: `dist/storage.js` (`StorageManager` reactive KV store), `windowState.js`, `settingsService.js`, `sshRecentHosts.js` / `sshWindowRestore.js`, and enterprise Google Cloud Secret Manager key references (`api_key_secret`, `api_key_secret_version`).

### 4.2 MicroCoder Persistence

MicroCoder separates **tamper-evident session auditing**, **raw output spills**, and **cross-session semantic recall**:

1. **Tamper-Evident `AJR1` Binary Journal (`docs/protocol/microcoder-journal-ajr1.md`, `coax/lib/microcoder/journal.coax`)**:
   - Stored at `~/.openagents/coder/sessions/worker-<unix_us>.ajr` (`0600` permissions).
   - Append-only `89-byte header + payload` frames across 11 typed frame kinds (`session_start`, `turn_start`, `step_prompt`, `step_response`, `edit_applied`, `command_run`, `file_viewed`, `steer_drained`, `turn_end`, `error`, `session_end`).
   - **SHA-256 Hash Chain**: Frame `0` chains from `SHA256("AJR1")`; every subsequent frame records `prev_hash = SHA256(header[0..56] || payload)` of the prior frame. Any write failure **poisons** the handle for the rest of the session rather than silently dropping audit records. Journals export cleanly to ATIF-v1.8 for deterministic replay and evals.
2. **Command Output Spill Store (`<journal>.out/`)**: Full stdout/stderr streams exceeding inline step caps are persisted verbatim as `s<step>-c<cmd>.txt` so neither the model nor post-hoc auditors lose truncated compiler/test output.
3. **Hybrid Semantic Turn Memory (`coder/tui/memory.coax`)**:
   - Stores `(user, reply, timestamp, cwd)` records in `turns.jsonl` paired with fixed-stride `f32` embedding vectors in `turns.bin` (plus `forget` tombstones).
   - Queries via `recall` combine lexical token overlap and vector cosine similarity using **Reciprocal Rank Fusion (RRF)**—pure Coax math with zero external database dependencies.
4. **Persistent Task Queue (`MwQueue`, `coder/tui/mcworker.coax`)**: Queued workspace tasks survive across turns and pause deterministically on error or `stop` so queued follow-ups never execute blindly over a broken workspace.

### 4.3 Persistence Gaps

- **Gap P-1 — Cryptographic Hash-Chained Auditability vs. Unencrypted JSON Logs (MicroCoder -> Antigravity gap)**: Antigravity writes plain unchained `transcript.jsonl` / `transcript_full.json` files (and multiplied subagent snapshots) in application support (`inference-and-agent-loop.md` §9). MicroCoder's `0600` `AJR1` journal provides cryptographic tamper evidence, strict write-failure poisoning, and exact byte-level prompt/response replay.
- **Gap P-2 — Step-Level Revert (`revert_to_step_called`) & Trajectory Forking (`[ForkConversation]`) (Antigravity -> MicroCoder gap)**: Because Antigravity treats `CortexStep` trajectories as mutable/branchable DAGs via `GetCascadeTrajectory`, users can rewind a conversation to step *N* or fork an alternate branch in place. MicroCoder's `AJR1` journal is strictly append-only; rewinding requires git worktree reset plus starting a new session.
- **Gap P-3 — Distilled `KNOWLEDGE_ARTIFACTS` vs. Raw Turn-Pair RRF Recall (Antigravity -> MicroCoder gap)**: Antigravity's `CORTEX_STEP_TYPE_MEMORY` and `KNOWLEDGE_ARTIFACTS` persist distilled architectural lessons across runs, whereas `coder/tui/memory.coax` indexes raw `(user, reply)` turn pairs via RRF and drops `McAware` file-anchored `findings` when a `mcworker` process exits.
- **Gap P-4 — Per-Cascade Browser Sandbox Isolation (Antigravity -> MicroCoder gap)**: Antigravity provisions `<appDataDir>/brain/<cascadeId>/browser_sandbox/` with cross-cascade read-only rules; MicroCoder relies on git worktree isolation (`~/.openagents/worktrees/...`) and `McFence`.

---

## 5. Prioritized Recommendations for MicroCoder

To close the highest-impact capability gaps identified above **without** compromising MicroCoder's core architectural invariants (zero heap allocation, small fixed JSON schema, warm KV-prefix cache, and `AJR1` hash-chained determinism), four targeted enhancements are recommended:

1. **OS Kernel Sandboxing & Script Jail Parity (`TC-1` / `EF-1` Containment)**: Keep `McFence` (`coax/lib/microcoder/fence.coax`) for fast lexical checks, and wrap `/bin/sh` child spawns in `tools.coax` with generated macOS `sandbox-exec` Seatbelt profiles (and Linux Landlock/bwrap) restricting filesystem writes to `cwd`, `$TMPDIR`, and `.out/` spills.
2. **CLI-Lowered MCP & Headless Browser Shims (`TC-1`, `TC-3`)**: Rather than expanding `NextAction` into 40+ dynamic `functionCall` schemas (which would break Coax static buffers and KV-prefix stability), follow the `device.coax` -> `oasim` pattern: expose Chrome CDP inspection and configured MCP servers (`mcp call` / `mcp search`, analogous to Antigravity's `TOOL_SEARCH`) as deterministic local CLI tools whose outputs flow naturally through MicroCoder's existing `commands` journal and `<journal>.out/` ranged viewer.
3. **Optional Read-Only Plan Checkpoint for High-Effort Tasks (`AL-1`)**: When `gw_plan` classifies a task as `high` effort / multi-file architecture, support an optional two-phase mode in `mcworker.coax` that runs initial discovery under `mc_read_only = 1` (mirroring Antigravity's `PLANNER_MODE: READ_ONLY`) before unlocking workspace `edits`.
4. **Persist Distilled `McAware` Findings into `memory.coax` (`P-3`, `P-2`)**: At `session_end`, export surviving file-anchored `McAware` findings alongside `(user, reply)` embeddings in `coder/tui/memory.coax`, and add an `AJR1` checkpoint-slice loader so `coder` can fork a new session seeded from step *N* of a prior `.ajr` journal (`[ForkConversation]` parity without breaking append-only `AJR1` immutability).
