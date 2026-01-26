# OpenAgents: Autopilot

**Autopilot** is the primary product in this repo: a local-first desktop agent that plans → executes → verifies → produces replayable artifacts. It runs on your machine, works in your repo, and keeps the full trace of what happened.

OpenAgents also contains the broader runtime/compiler/market stack used by Autopilot.
If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./MANIFESTO.md)**.

## Quick Start (Desktop)

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents/apps/autopilot-desktop
bun install
bun run tauri dev
```

For release builds, see **[apps/autopilot-desktop/docs/autopilot/DISTRIBUTION.md](apps/autopilot-desktop/docs/autopilot/DISTRIBUTION.md)**.

## What you get

Autopilot is designed to be:

* **Local-first**: runs against your repo on your machine by default
* **Verifiable**: uses deterministic checks (tests/builds) as the ground-truth loop
* **Inspectable**: emits structured session logs and artifacts for replay/debug/audit
* **Optimizable**: uses DSPy-style signatures/modules/optimizers to improve policies over time
* **Signature-driven UI**: Effuse UITree + UI patch streaming for live, structured visibility

## CLI / other surfaces

The CLI and other agent surfaces live in the same repo but are not the primary path here.
If you need them, use the docs index below to navigate the relevant crates and guides.

## Documentation

Start with:

* **Desktop docs**: [apps/autopilot-desktop/docs/README.md](apps/autopilot-desktop/docs/README.md)
* **Docs index (everything else)**: [docs/README.md](docs/README.md)
* **Repo map / ownership**: [PROJECT_OVERVIEW.md](./PROJECT_OVERVIEW.md)
* **Agent contract / contribution norms**: [AGENTS.md](./AGENTS.md)
