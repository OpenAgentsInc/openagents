# OpenAgents

OpenAgents is a local-first stack for autonomous agents that can **sign**, **run tools**, **verify work**, **account for cost**, and (optionally) **buy compute** in open markets. The wedge product in this repo is **Autopilot**: an autonomous coding agent that plans → executes → verifies → emits replayable artifacts.

If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./MANIFESTO.md)**.

## Quick Start (Autopilot)

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# build + run the Autopilot app
cargo build -p autopilot
cargo run -p autopilot
```

Run a one-off task via CLI mode:

```bash
cargo run -p autopilot -- run "Summarize @README.md and run !git status"
```

Tip: this repo defines a Cargo alias in `.cargo/config.toml`, so `cargo autopilot ...`
is shorthand for `cargo run -p autopilot -- ...`.

## What you get

Autopilot is designed to be:

* **Local-first**: runs against your repo on your machine by default
* **Verifiable**: uses deterministic checks (tests/builds) as the ground-truth loop
* **Inspectable**: emits structured session logs and artifacts for replay/debug/audit
* **Optimizable**: uses DSPy-style signatures/modules/optimizers to improve policies over time

## Documentation map

Start here depending on what you're doing:

* **Terms / vocabulary**: [GLOSSARY.md](./GLOSSARY.md)
* **Run + ship priorities**: [ROADMAP.md](./ROADMAP.md)
* **Architecture + strategy**: [SYNTHESIS.md](./SYNTHESIS.md)
* **Current implementation reality**: [SYNTHESIS_EXECUTION.md](./SYNTHESIS_EXECUTION.md)
* **Formal write-up**: [PAPER.md](./PAPER.md)
* **Repo layout + crate map**: [PROJECT_OVERVIEW.md](./PROJECT_OVERVIEW.md)

Autopilot-specific docs:

* [crates/autopilot/docs/MVP.md](crates/autopilot/docs/MVP.md)
* [crates/autopilot/docs/ROADMAP.md](crates/autopilot/docs/ROADMAP.md)
* [crates/autopilot-core/docs/EXECUTION_FLOW.md](crates/autopilot-core/docs/EXECUTION_FLOW.md)

DSPy / dsrs docs:

* [crates/dsrs/docs/README.md](crates/dsrs/docs/README.md)
* [crates/dsrs/docs/DSPY_ROADMAP.md](crates/dsrs/docs/DSPY_ROADMAP.md)

## Contributing / navigating the codebase

If you're trying to find "where does X live?", use **[PROJECT_OVERVIEW.md](./PROJECT_OVERVIEW.md)**.
It's the repo map (crates, responsibilities, data flows).
