# OpenAgents

OpenAgents is building a **cross-platform Effuse app**: web at **openagents.com**, mobile in **`apps/expo/`**, and a desktop app in progress. The stack is centered on **Effect** (application runtime) + **Effuse** (UI runtime), with **Convex** realtime state and **WorkOS** AuthKit.

If you're looking for the philosophy / "why open", start with **[MANIFESTO.md](./docs/MANIFESTO.md)**.

## What’s possible with Autopilot (evolving agent)

Autopilot is an agent you can chat with in plain language. Right now you can sign in on the web, start a conversation, and use it for the capabilities we’ve shipped today (e.g. analyzing code you paste, suggesting refactors, generating configs). What it can do is not fixed: **the system is built to evolve**.

New behaviors are added as **signatures**—versioned, measurable steps with clear inputs and outputs—and can be compiled, tested, and rolled out like software. Over time, the Autopilot “network” will grow through a **marketplace** of signatures and tools: as we and others add capabilities (new tools, new skills), those become available to your Autopilot. So if you ask for something it can’t do yet, the answer is “not yet”—we’re building toward a world where user demand and new signatures continuously expand what’s possible.

**To try it now:** go to [openagents.com](https://openagents.com), sign in, and start chatting. Your Autopilot will remember your preferences and the thread; you can use it for coding help, Blueprint updates, and whatever we’ve enabled so far—with more coming as the ecosystem grows.

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

## Mobile

The mobile app lives in **`apps/expo/`**.

## Documentation

Start with:

* **Web app**: [apps/web/](apps/web/)
* **Mobile app**: [apps/expo/](apps/expo/)
* **Effuse plan/spec**: [packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md](packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md)
* **Docs index (everything else)**: [docs/README.md](docs/README.md)
* **Repo map / ownership**: [PROJECT_OVERVIEW.md](./docs/PROJECT_OVERVIEW.md)
* **Agent contract / contribution norms**: [AGENTS.md](./AGENTS.md)
