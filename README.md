# OpenAgents Autopilot

Autopilot is the autonomous coding agent in the OpenAgents workspace. It runs a
full GUI and CLI experience on top of the Codex app-server for interactive work,
and it can switch into Adjutant's DSPy-driven autopilot loop for end-to-end
execution. The focus here is simple: give Autopilot a task, let it plan, execute,
verify, and report without hand-holding, while keeping the entire run local-first
and fully inspectable.

If you are looking for the broader OpenAgents vision, product lineup, and
infrastructure stack, see [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md).

## Quick Start

Build and run the Autopilot app from source in this workspace:

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo build -p autopilot
cargo autopilot
```

Run Autopilot in CLI mode for a one-off task:

```bash
cargo run -p autopilot -- run "Summarize @README.md and run !git status"
```

## What Autopilot Delivers

Autopilot keeps one conversation per repo, expands prompts with local files and
commands, and streams tool calls as first-class UI items. When you enable
autopilot mode, it bootstraps OANIX discovery, runs DSPy planning and execution
pipelines, and tracks outcomes for self-improvement, so the same task surface
becomes a training signal for future runs.

## Documentation

- Autopilot MVP: [crates/autopilot/docs/MVP.md](crates/autopilot/docs/MVP.md)
- Autopilot Roadmap: [crates/autopilot/docs/ROADMAP.md](crates/autopilot/docs/ROADMAP.md)
- Autopilot Execution Flow: [crates/autopilot-core/docs/EXECUTION_FLOW.md](crates/autopilot-core/docs/EXECUTION_FLOW.md)
- DSPy Strategy: [crates/dsrs/docs/README.md](crates/dsrs/docs/README.md)
- DSPy Roadmap: [crates/dsrs/docs/DSPY_ROADMAP.md](crates/dsrs/docs/DSPY_ROADMAP.md)
- Project Overview: [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md)
