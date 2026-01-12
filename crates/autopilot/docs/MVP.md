# Autopilot v0.1

A local-first autonomous coding agent.

## User experience

### 1. Install

User installs Autopilot via either:

1. Shell script from openagents.com
2. Building from source via our GitHub repo (requires Rust toolchain)

### 2. Run

In your working directory, run:

```bash
autopilot
```

This opens the Autopilot desktop app.

Alternately Autopilot can be steered via CLI commands.

```bash
autopilot run --help
```

For ad-hoc prompts, the CLI runs the same Codex app-server flow as the GUI:

```bash
autopilot run "Summarize @README.md and run !git status"
autopilot run "/review commit <sha> [title]"
autopilot run --access read-only --model gpt-4.1 --effort high "Explain the diff"
```

Use `--autopilot-loop` to route ad-hoc prompts through the DSPy loop instead of the app-server.

### 3. Connect Codex

This version of Autopilot requires a Codex subscription.

Future versions will support other agents and API keys.

### 4. Prompt

Enter your prompt and hit enter. This begins an Autopilot session.


## Concepts

### One Conversation Per Project

Conversation threads are separated only by project, based on its working directory. Each working directory has its own long-running conversation.

### Instant Message Processing 

You can add new prompts anytime. They are known to Autopilot immediately, not queued.

### Continuous Learning

Autopilot responses use DSPy signatures and optimizations. Read more about our DSPy integration [here](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/README.md).


## Current State (Implementation Snapshot)

- **UI**: Winit/WGPU desktop app with GPU-rendered chat, Markdown streaming, tool call cards, command palette, keybindings, and left/right sidebar panels.
- **Backends**: Interactive mode uses the Codex app-server directly; `/backend` exists but only Codex is wired. The agent backend registry and model picker are implemented for Codex only.
- **Autopilot mode**: When permission mode is set to autopilot, prompts run through Adjutant's autopilot loop with OANIX boot and DSPy stages; max iterations is 10 and verification is enabled. Requires an OANIX workspace (`oanix init`) for full context.
- **Prompt expansion**: `@file` inlines local files and `!command` runs a shell command for context. Autopilot adds OANIX context (directives, issues, recent git log) before executing.
- **Sessions**: Local history is stored in `~/.openagents/autopilot/sessions`, with list/fork/export and checkpoint restore UI. Resume loads cached history only; Codex thread resume is not wired, and delete is not implemented.
- **Tools and permissions**: Tool calls/results render for Codex and Adjutant. Permission modes map to Codex sandbox/approval policies and approvals are surfaced in the GUI/CLI; the Codex tool list still only enumerates MCP tools (not built-ins).
- **CLI parity**: `autopilot run` uses the app-server for ad-hoc prompts with `@file`/`!command` expansion, review support, and interactive approvals in `workspace` mode. Default access mode is full (no approvals); `read-only` auto-declines write/exec requests, and `--autopilot-loop` restores the DSPy loop for ad-hoc tasks.
- **Catalogs and config**: Agents, skills, hooks, and MCP configs are discovered from `.openagents` and `~/.openagents` (plus `.mcp.json`) with UI management, but they are not yet applied to Codex/Adjutant runs.
- **Panels and telemetry**: OANIX, directives/issues, autopilot issues (`.openagents/autopilot.db`), Pylon earnings/jobs, RLM runs/trace, Spark wallet, DVM providers, NIP-90 jobs, NIP-28 chat, Nexus stats, LM router, and Gateway health all have panels with refreshable data sources.
