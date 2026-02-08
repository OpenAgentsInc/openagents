# OpenAgents

**Autopilot (Web MVP)** is the primary product in this repo: a web-first agent workspace built on **Effect** (application runtime) + **Effuse** (UI runtime), with **Convex** realtime state and **WorkOS** AuthKit, deployed on a **single Cloudflare Worker**.

If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./docs/MANIFESTO.md)**.

## Quick Start (Web)

The web app lives in **[apps/web/](apps/web/)**.

```bash
cd apps/web
npm install
npm run dev
```

Deploy (Convex + Worker):

```bash
cd apps/web
npm run deploy
```

## Desktop (deprioritized)

The desktop client exists but is not the current focus while we ship the web MVP.

```bash
cargo run -p autopilot-desktop
```

## Documentation

Start with:

* **Web app**: [apps/web/](apps/web/)
* **Effuse plan/spec**: [packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md](packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md)
* **Docs index (everything else)**: [docs/README.md](docs/README.md)
* **Repo map / ownership**: [PROJECT_OVERVIEW.md](./docs/PROJECT_OVERVIEW.md)
* **Agent contract / contribution norms**: [AGENTS.md](./AGENTS.md)
