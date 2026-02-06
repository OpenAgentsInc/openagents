# Effuse docs

These docs were part of the former autopilot-desktop app (Effuse-based Tauri UI) and are now maintained with the standalone `@openagentsinc/effuse` package.

## Usage in apps/web

The OpenAgents web app (`apps/web`) uses Effuse for almost all user-facing UI:

- **Marketing layout** – Header (logo, Log in, Start for free) is Effuse; home and login body are Effuse (`effuse-pages/home.ts`, `login.ts`, `header.ts`).
- **Catalog routes** – `/modules`, `/signatures`, `/tools` are rendered by Effuse (`effuse-pages/modules.ts`, `signatures.ts`, `tools.ts`). Data is loaded in React (Effect + AgentApiService) and passed into the Effuse program; Effuse renders the list and details.
- **Autopilot** – The chat column (header, message list, tool cards, input form) is Effuse (`effuse-pages/autopilot.ts`). The sidebar, Blueprint panel, and control buttons remain React; form submit and button actions are delegated from React via `EffuseMount`’s `onRendered` callback.

React is still used for: route loaders, auth, Convex, HUD backgrounds (DotsBackground), `EffuseMount` (run Effuse in a div and optionally `onRendered` for event delegation), and the Autopilot sidebar/blueprint/controls.

| Doc | Description |
|-----|-------------|
| [README.md](./README.md) | Framework overview, quick start, components & EZ |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Architecture and layers |
| [SPEC.md](./SPEC.md) | Spec and contracts |
| [DOM.md](./DOM.md) | DOM service |
| [EZ.md](./EZ.md) | Hypermedia (EZ) actions |
| [ORIGIN.md](./ORIGIN.md) | Origin and design |
| [ROADMAP.md](./ROADMAP.md) | Roadmap |
| [TESTING.md](./TESTING.md) | Testing |
| [inspiration-HTMX.md](./inspiration-HTMX.md) | HTMX-inspired patterns |
| [inspiration-typed.md](./inspiration-typed.md) | Typed template inspiration |
