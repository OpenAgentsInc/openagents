# OpenAgents: Autopilot

**Autopilot** is the primary product in this repo: a local-first desktop agent that plans → executes → verifies → produces replayable artifacts. It runs on your machine, works in your repo, and keeps the full trace of what happened.

OpenAgents also ships a **web app** ([openagents.com](https://openagents.com)) and the broader runtime/compiler/market stack used by Autopilot.
If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./docs/MANIFESTO.md)**.

## Quick Start

### Web app

The main web UI (openagents.com) lives in **[apps/web/](apps/web/)** — TanStack Start + Convex + WorkOS, deployed on Cloudflare Workers. To run locally:

```bash
cd apps/web
npm install
npm run dev
```

### Desktop app (Autopilot)

```bash
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents
cargo run -p autopilot-desktop
```

For release builds, see **[apps/autopilot-desktop/docs/migration/EFFUSE_TO_WGPUI_PLAN.md](apps/autopilot-desktop/docs/migration/EFFUSE_TO_WGPUI_PLAN.md)**.

## What you get

Autopilot is designed to be:

* **Local-first**: runs against your repo on your machine by default
* **Verifiable**: uses deterministic checks (tests/builds) as the ground-truth loop
* **Inspectable**: emits structured session logs and artifacts for replay/debug/audit
* **Optimizable**: uses DSPy-style signatures/modules/optimizers to improve policies over time
* **Immediate-mode UI**: WGPUI render tree with Zed-inspired layout patterns

## Autonomy-as-a-Service (predictable autonomy)

Autopilot is built to sell **predictable autonomy** — not “AI,” not “tokens,” but a
**contracted outcome over time**:

* **Scope**: “Do X”
* **Horizon**: “over the next 24–48 hours”
* **Constraints**: budget, privacy, repo boundaries, allowed tools
* **Verification**: objective checks (tests pass, PR merged, receipts emitted)
* **Reliability**: known failure modes + escalation (“pause + ask human”)

Composability makes this sellable: signatures/modules have stable I/O contracts,
emit receipts and utility labels, and can be optimized and A/B tested. That is
how Autopilot turns *vibes* into *predictability*.

## CLI / other surfaces

The CLI and other agent surfaces live in the same repo but are not the primary path here.
If you need them, use the docs index below to navigate the relevant crates and guides.

The slim per-user runtime template lives in `apps/openclaw-runtime`.

## Documentation

Start with:

* **Web app**: [apps/web/](apps/web/)
* **Desktop docs**: [apps/autopilot-desktop/docs/migration/EFFUSE_TO_WGPUI_PLAN.md](apps/autopilot-desktop/docs/migration/EFFUSE_TO_WGPUI_PLAN.md)
* **Docs index (everything else)**: [docs/README.md](docs/README.md)
* **Repo map / ownership**: [PROJECT_OVERVIEW.md](./docs/PROJECT_OVERVIEW.md)
* **Agent contract / contribution norms**: [AGENTS.md](./AGENTS.md)
* **OpenClaw runtime (managed)**: [docs/openclaw/openclaw-slim-runtime-options.md](docs/openclaw/openclaw-slim-runtime-options.md)
