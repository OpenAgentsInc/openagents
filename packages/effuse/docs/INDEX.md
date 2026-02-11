# Effuse docs

These docs were part of the former autopilot-desktop app (Effuse-based Tauri UI) and are now maintained with the standalone `@openagentsinc/effuse` package. **All Effuse and apps/web integration documentation is consolidated here** (moved from `docs/autopilot/` and related locations).

## Usage in apps/web

The OpenAgents web app (`apps/web`) uses Effuse for almost all user-facing UI:

- **Marketing layout** – Header (logo, Log in, Start for free) is Effuse; home and login body are Effuse (`effuse-pages/home.ts`, `login.ts`, `header.ts`).
- **Catalog routes** – `/modules`, `/signatures`, `/tools` are rendered by Effuse (`effuse-pages/modules.ts`, `signatures.ts`, `tools.ts`). Data is loaded in React (Effect + AgentApiService) and passed into the Effuse program; Effuse renders the list and details.
- **Autopilot** – The chat column (header, message list, tool cards, input form) is Effuse (`effuse-pages/autopilot.ts`). The sidebar, Blueprint panel, and control buttons remain React; form submit and button actions are delegated from React via `EffuseMount`’s `onRendered` callback.

React is still used for: route loaders, auth, Convex, HUD backgrounds (DotsBackground), `EffuseMount` (run Effuse in a div and optionally `onRendered` for event delegation), and the Autopilot sidebar/blueprint/controls.

**Important integration detail:** Effuse templates use normal anchors (`<a href="/...">`). `EffuseMount` intercepts internal (same-origin) anchor clicks and calls TanStack Router navigation so links stay SPA (no full page refresh). Opt out with `data-router-ignore`.

**Full conversion doc:** [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md) (architecture, data flow, event delegation, file map, how to add or change Effuse pages).

---

## Router and apps/web integration (start here)

| Doc | Description |
|-----|-------------|
| [MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md](./MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md) | **Single master plan:** end-state Effect/Effuse architecture including full React + TanStack replacement |
| [ROUTER-AND-APPS-WEB-INTEGRATION.md](./ROUTER-AND-APPS-WEB-INTEGRATION.md) | **Comprehensive:** Router, Effect, RPC, auth cache, navigation, avoiding full-page behavior |
| [APPS-WEB-FULL-EFFUSE-ROADMAP.md](./APPS-WEB-FULL-EFFUSE-ROADMAP.md) | Step-by-step roadmap to migrate `apps/web` to Effuse everywhere (Typed-inspired, thin TanStack host) |
| [effect-rpc-web.md](./effect-rpc-web.md) | Effect RPC mount, procedures, client usage |
| [effect-migration-web.md](./effect-migration-web.md) | Effect scaffold, entry points, migration order, routing/navigation |
| [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md) | Effuse conversion: EffuseMount, data flow, file map, adding pages |
| [tanstack-start-effect-comparison.md](./tanstack-start-effect-comparison.md) | Our approach vs. Practical Effect tutorial |
| [DELEGATION-full-effect-integration.md](./DELEGATION-full-effect-integration.md) | Delegation brief for implementing RPC, MemoMap, atoms, hydration |

**ADR copies (canonical in repo `docs/adr/`):** [adr/adr-0022-effuse-uitree-ipc.md](./adr/adr-0022-effuse-uitree-ipc.md), [adr/adr-0027-effect-rpc-and-atom-hydration-web.md](./adr/adr-0027-effect-rpc-and-atom-hydration-web.md)

**Archive (legacy plans):** [archive/dsrs-effuse-ui-plan.md](./archive/dsrs-effuse-ui-plan.md), [archive/effuse-ui-implementation-plan.md](./archive/effuse-ui-implementation-plan.md)

---

## Effuse framework (core)

| Doc | Description |
|-----|-------------|
| [README.md](./README.md) | Framework overview, quick start, components & EZ |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Architecture and layers |
| [SPEC.md](./SPEC.md) | Spec and contracts |
| [DOM.md](./DOM.md) | DOM service |
| [EZ.md](./EZ.md) | Hypermedia (EZ) actions |
| [MIGRATION-SERVICES-TAGGED-ERRORS.md](./MIGRATION-SERVICES-TAGGED-ERRORS.md) | Migration notes for class-based service tags and schema-tagged errors |
| [ORIGIN.md](./ORIGIN.md) | Origin and design |
| [ROADMAP.md](./ROADMAP.md) | Roadmap |
| [TESTING.md](./TESTING.md) | Testing |
| [inspiration-HTMX.md](./inspiration-HTMX.md) | HTMX-inspired patterns |
| [inspiration-typed.md](./inspiration-typed.md) | Typed template inspiration |
