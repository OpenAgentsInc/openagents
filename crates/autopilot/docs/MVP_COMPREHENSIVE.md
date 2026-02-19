# Autopilot MVP (Comprehensive)

This document is the full, current-state description of the Autopilot MVP. It
is intentionally detailed so it can serve as the source of truth for how the UI
and CLI behave today, what data they depend on, where state lives, and which
features are implemented versus explicitly out of scope. It is designed to be
read by contributors who need to understand how Autopilot operates end to end.

## Scope and Definition of the MVP

The Autopilot MVP is a local-first autonomous coding agent that exposes a WGPU
GUI and a CLI, both driven by the Codex app-server for interactive sessions and
by Adjutant's DSPy loop for fully autonomous runs. The MVP is considered feature
complete when a user can open a repository, issue a prompt, see streamed output
and tool actions, review diffs, and resume local session history without needing
any cloud-side state beyond Codex itself.

The MVP does not attempt to cover every backend or protocol yet. Codex is the
only interactive backend that is fully wired, and the app-server is treated as a
sidecar process rather than an embedded library. The focus is stability and
observability rather than exotic routing, so the documentation favors precise
behavior over future intent.

## High-Level Architecture

Autopilot is split into three cooperating layers. The desktop application in
`crates/autopilot/` handles the WGPUI interface, local session storage, command
palette, and panel rendering. The CLI entry point also lives in this crate, but
it delegates to Adjutant so the command-line interface shares the same execution
loop as the autonomous mode. The core planning, execution, and verification
signatures are implemented in `crates/autopilot-core/` and wired by Adjutant
through DSPy.

The interactive path uses the Codex app-server as a supervised subprocess. The
UI and CLI initiate a thread, start turns, and consume the streaming JSONL event
feed. The autonomous path uses Adjutant's loop, which performs OANIX discovery,
builds a DSPy plan, executes tasks, and verifies results. Both flows emit the
same UI events so the user sees consistent cards, diffs, and tool timelines.

## Execution Modes

Autopilot has two primary modes of operation. The first is interactive mode,
which uses Codex app-server runs. This is the default for the GUI and for CLI
commands unless `--autopilot-loop` is passed. The second is autonomous mode,
which runs Adjutant's DSPy loop with OANIX boot and a bounded iteration cap.

Interactive mode is oriented around streaming feedback and approvals. The UI
spawns the app-server, performs the initialize handshake, starts a thread, and
begins a turn. The app-server emits tool events, file changes, and diffs that
are rendered live in the timeline. Approvals are surfaced in the UI or CLI and
must be accepted or declined depending on the access mode.

Autonomous mode is oriented around structured decision-making. OANIX discovers
hardware, compute, network, identity, and workspace metadata. The DSPy planning
pipeline generates a plan, a todo list, and the execution decisions required to
apply changes. Verification runs after execution and can insert retry steps when
needed. The user still sees streamed output, but the decisions are driven by
DSPy signatures rather than by interactive chat turns.

## User Journey (GUI)

The GUI is a Winit/WGPU desktop application. Launching `autopilot` from a
repository opens the UI in that workspace context. The user enters a prompt in
the composer and submits it. Autopilot spawns the Codex app-server, starts a
thread for the workspace, and streams response events into the timeline.

The interface exposes a command palette and keyboard shortcuts to toggle
sidebars, switch panels, and open status views. The right side surfaces
telemetry and live data for infrastructure components such as OANIX, Pylon, and
NIP-90 jobs, while the left side handles workspace navigation and session
context. The design is optimized for reading long outputs and inspecting diffs
without losing the continuity of the chat timeline.

## User Journey (CLI)

The CLI path is backed by Adjutant. Running `autopilot run "<prompt>"` triggers
an app-server run with the same prompt expansion and approval behavior as the
GUI. The CLI streams output to stdout and renders structured events using the
same DSPy stage markers as the GUI.

Autopilot also supports issue-driven runs through the CLI, where you can list,
claim, and complete issues. This workflow integrates with OANIX workspace
metadata, so issues are resolved against `.openagents/issues.json` and
associated directives when present.

## CLI Commands and Flags

The CLI supports `run`, `status`, `issue`, and `dspy` top-level commands. The
`run` command is the primary entry point for task execution, and it can operate
against a specific issue or an ad-hoc prompt.

`autopilot run` accepts an optional task prompt, or it can be pointed at a
specific issue number with `--issue`. The `--loop-mode` flag enables a continuous
loop where the agent claims and completes issues until stopped. The
`--autopilot-loop` flag forces the DSPy loop even for ad-hoc prompts.

Boot behavior is controlled with `--full-boot` to enable full OANIX discovery,
while the default is a faster boot that skips network and compute discovery. The
`--max-iterations` flag bounds the DSPy loop, and `--no-verify` disables the
verification phase.

Backend and access mode selection are provided through `--backend` and
`--access`. Supported backends are `auto`, `codex`, `local-llm`, and
`local-tools`, although Codex is the only interactive backend currently wired
for the app-server path. Access modes are `full`, `workspace`, and `read-only`.
These map directly to approval policy and sandbox configuration. The default
access mode is full, which auto-accepts approvals and runs with full access.
Workspace mode prompts for approvals and restricts write access to the workspace
roots. Read-only mode auto-declines writes and execs and sets a read-only
sandbox policy.

Model and reasoning control are exposed through `--model` and `--effort`. The
model override is passed directly to the app-server thread. Reasoning effort
accepts the values `none`, `minimal`, `low`, `medium`, `high`, and `xhigh` and is
only applied to Codex app-server runs.

The `issue` subcommands cover `list`, `claim`, `complete`, and `show`, which
manipulate the OANIX issue tracker for a repository. The `dspy` subcommands cover
status, optimization, export, session inspection, performance summaries, and
auto-optimization toggles.

## Prompt Expansion and Context Injection

Autopilot supports prompt expansion using `@file` and `!command` tokens. Files
are loaded from the local filesystem with size limits and truncation, and shell
commands are executed through the app-server sandbox so they obey the active
policy. The resulting outputs are injected directly into the prompt before the
turn starts.

Autopilot also injects static context by scanning for `AGENTS.md` and `TODO.md`
files. It searches in the current workspace, `.openagents/`, and the global
`~/.openagents/` directory. The contents are wrapped in labeled sections so the
model can distinguish instructions from task text.

When the DSPy loop runs, additional OANIX context is added to the task, including
active directives, issues, and recent git history. This keeps the autonomous
plan grounded in the repository state even when the user prompt is brief.

## Permissions, Approvals, and Safety

Autopilot treats approvals as the autonomy control surface. In app-server mode,
Codex emits approval requests for command execution and file changes, and the UI
or CLI responds based on the configured access mode. Full access maps to auto
accept. Workspace access maps to prompt-based approvals. Read-only access maps
to auto decline for any write or exec request.

In addition to approvals, sandboxing is enforced by the app-server policy
configuration. Workspace mode uses a workspace-write sandbox policy, and
read-only mode uses a read-only policy. Full access uses the dangerous
full-access mode, which is why the default is suited only for trusted local
execution.

## Sessions, Storage, and Logs

Autopilot is local-first. The config directory defaults to
`~/.openagents/autopilot/` but will fall back to `~/.openagents/coder/` if the
legacy path exists. The core files are `config.toml`, `keybindings.json`,
`permissions.json`, `hooks.json`, and `workspaces.json`, plus a `sessions/`
folder that stores session state.

Session metadata is indexed in `~/.openagents/autopilot/sessions/index.json`.
Each session has a folder named after the Codex thread ID, and that folder
contains `messages.jsonl` for timeline events and any associated logs.

App-server runs also capture raw wire logs and trace logs when configured.
These are written as `wire-<run_id>.jsonl` and `trace-<run_id>.jsonl` inside the
session directory, which makes it possible to replay or audit the raw protocol
traffic for a given run.

Autonomous loop data is stored under `~/.openagents/adjutant/`, which includes
sessions, training data, and performance metrics for DSPy optimization. This
separation keeps UI state independent from the decision loop logs.

Autopilot issues are loaded from `.openagents/autopilot.db` when present, and
workspace issues and directives come from `.openagents/issues.json` and
`.openagents/directives/` via OANIX discovery.

## Catalogs and Configuration Sources

Autopilot discovers agents, skills, and hooks from both project and global
locations. Project-local definitions live under `.openagents/agents/`,
`.openagents/skills/`, and `.openagents/hooks/`. Global defaults live under
`~/.openagents/agents/`, `~/.openagents/skills/`, and `~/.openagents/hooks/`.

MCP configuration is discovered from `.mcp.json` in the workspace root. The UI
can display MCP server status and enumerate MCP-provided tools, but the MVP does
not yet apply skill or agent catalogs directly to Codex runs.

## UI Panels and Telemetry

The UI includes panels for infrastructure visibility and local data. OANIX
panels show environment discovery and workspace metadata. Directives and issues
panels show `.openagents` state. The Autopilot issues panel reads from
`.openagents/autopilot.db` and surfaces issue status. Pylon earnings and jobs
panels show local wallet and compute activity, while DVM, NIP-90, and NIP-28
panels surface relay-based job and chat activity. The Nexus panel shows relay
stats, the LM router panel surfaces routing state, and the Gateway panel
summarizes provider health. The Spark wallet panel displays Lightning wallet
state, and the RLM panel provides trace visibility for recursive runs.

These panels are backed by refreshable data sources. The MVP does not yet
correlate data across panels automatically, but the structure is in place for
future cross-panel telemetry.

## Known Limitations and Gaps

Codex thread resume is not wired in the UI. Sessions restore cached local
history, but they do not rehydrate state from Codex itself. Session deletion is
not implemented. The Codex tool list in the UI enumerates MCP tools only and
does not include built-in tools. Catalog discovery does not yet influence active
Codex or Adjutant runs, even though the UI can manage the catalog entries.

Interactive runs use Codex exclusively. The backend registry exists for future
multi-backend support, but only Codex is a full interactive path today. The
app-server flow is primary, and the `/backend` service exists only as a
placeholder for future non-Codex implementations.

## Build and Runtime Requirements

Autopilot expects a working Rust toolchain for local builds and a Codex
subscription for interactive runs. The Codex CLI must be installed and
authenticated for app-server execution. The CLI and UI both rely on the same
`codex app-server` binary, so the app-server must be available on the PATH.

Optional backends include local LLM servers (llama.cpp) or local tools mode, but
these are not the default interactive path. Environment variables such as
`AUTOPILOT_BACKEND`, `CEREBRAS_API_KEY`, and `PYLON_MNEMONIC` influence backend
selection and DSPy provider routing as documented in the Adjutant configuration
guide.

## Source Map

The MVP implementation primarily spans `crates/autopilot/` (GUI and state),
`crates/adjutant/` (CLI, app-server executor, autopilot loop),
`crates/autopilot-core/` (DSPy planning, execution, verification),
`crates/oanix/` (environment discovery), and `crates/dsrs/` (DSPy runtime).
This document should be kept in sync with those crates as the MVP evolves.
