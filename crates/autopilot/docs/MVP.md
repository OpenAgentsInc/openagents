# Autopilot v0.1

Autopilot is a local-first autonomous coding agent. It ships as a desktop app
with a CLI that shares the same app-server runtime, so the UI and command line
experience stay aligned while still supporting a fully autonomous mode through
Adjutant and DSPy.

## User experience

### 1. Install

Users can install Autopilot either by running the OpenAgents install script from
openagents.com or by building from source in this repository with a Rust
toolchain. The source build path is the default for contributors and keeps the
workspace aligned with the latest Autopilot UI and Adjutant behavior.

### 2. Run

From the working directory, run `autopilot` to launch the desktop app. The UI
opens in the current repository and uses the Codex app-server as its interactive
backend, preserving local sessions and tooling.

The CLI is available for ad-hoc prompts and automation. It runs the same
app-server flow as the GUI, with prompt expansion for `@file` and `!command` and
support for review commands.

```bash
autopilot run --help
autopilot run "Summarize @README.md and run !git status"
autopilot run "/review commit <sha> [title]"
autopilot run --access read-only --model gpt-4.1 --effort high "Explain the diff"
```

Use `--autopilot-loop` to route ad-hoc prompts through the DSPy loop instead of
the app-server. That mode boots OANIX, runs structured planning and execution
signatures, and emits the same UI events for visibility.

### 3. Connect Codex

Autopilot currently requires a Codex subscription because the app-server runtime
is the primary interactive backend. This is expected to expand over time as
additional backends and API keys are added to the runtime adapter layer.

### 4. Prompt

Enter a prompt and press enter to start an Autopilot session. The UI will
stream model output, tool calls, approvals, and diffs as they arrive, and the
session will be stored locally for replay and continuation.

## Concepts

### One conversation per project

Autopilot keeps one long-running thread per working directory. This keeps context
stable for each repository while still allowing users to run multiple projects
side-by-side without mixing histories.

### Instant message processing

Prompts are available to Autopilot immediately, not queued behind a global task
list. This means you can add new prompts while a session is active and the agent
can incorporate them as soon as the current turn completes.

### Continuous learning

Autopilot uses DSPy signatures and optimizations to structure decisions and to
collect learning signals. The DSPy pipeline is designed so that plan quality,
execution choices, and verification outcomes feed back into future runs without
requiring manual prompt edits.

## Current state (implementation snapshot)

The desktop UI is a Winit/WGPU app with streaming Markdown, tool call cards,
command palette actions, keybindings, and left/right panels. The layout is
optimized for reading long outputs and inspecting diffs while maintaining a
continuous chat timeline.

Interactive runs use the Codex app-server directly, while the `/backend` service
exists as a placeholder for future non-Codex backends. The backend registry and
model picker are implemented with Codex as the only active backend today.

Autopilot mode routes prompts through Adjutant's autopilot loop with OANIX boot
and DSPy stages. It currently caps iterations at 10, enables verification by
default, and expects an OANIX workspace (`oanix init`) for full directive and
issue context.

Prompt expansion supports `@file` inclusion and `!command` execution, and
Autopilot injects OANIX context (directives, issues, recent git log) before it
runs the task. This keeps the initial prompt grounded in repository state.

Sessions are stored in `~/.openagents/autopilot/sessions`, with list, fork, export,
and checkpoint restore support in the UI. Session resume currently loads cached
history only; Codex thread resume is not yet wired, and deletion is not
implemented.

Tool calls and approvals render for both Codex and Adjutant flows. Permission
modes map to Codex sandbox and approval policies, and approvals are exposed in
both GUI and CLI. The Codex tool list still enumerates MCP tools only and does
not yet include built-in tools.

The CLI path (`autopilot run`) supports ad-hoc prompts, review flows, interactive
approvals, and `@file`/`!command` expansion. The default access mode is full with
no approvals; read-only mode auto-declines write and exec requests. The
`--autopilot-loop` flag re-enables the DSPy loop for ad-hoc runs.

Catalogs and configuration for agents, skills, hooks, and MCP are discovered
from `.openagents`, `~/.openagents`, and `.mcp.json`. The UI can manage these
catalogs, but they are not yet applied to Codex or Adjutant runs.

The telemetry panels surface OANIX discovery data, directives and issues,
Autopilot issues, Pylon earnings and jobs, RLM traces, Spark wallet data, DVM
providers, NIP-90 jobs, NIP-28 chat, Nexus stats, LM router health, and Gateway
status. Each panel has a refreshable data source, but cross-panel correlations
are still manual.
