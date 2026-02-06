# Roadmap: Convert `apps/web` to Effuse Everywhere (Thin TanStack Host)

**Audience:** coding agents working on `apps/web` + `@openagentsinc/effuse`.  
**Goal:** move `apps/web` to **Effuse + Effect for essentially all UI and UI logic**, while keeping **TanStack Start/Router** as a thin substrate for SSR + routing until/unless we replace it later.

This roadmap is incremental and designed to keep the app shippable at each step.

---

## 0. Current Status (Baseline)

Effuse already renders most user-facing UI in `apps/web`:

- marketing header + home + login
- `/modules`, `/tools`, `/signatures`
- `/autopilot` chat column

React is still used for:

- TanStack Start/Router integration (SSR + routing, loaders)
- providers (WorkOS/Convex)
- HUD backgrounds (`@openagentsinc/hud/react`)
- stateful chrome (sidebar, blueprint panel, control panels)
- event wiring for Effuse DOM (`onRendered` and DOM queries)

Important integration behaviors already in place:

- **SPA navigation for Effuse anchors:** `apps/web/src/components/EffuseMount.tsx` intercepts same-origin `<a href="/...">` clicks and calls `router.navigate({ href })` (opt out via `data-router-ignore`).
- **Shared Effect runtime + MemoMap:** loader/serverFns and RPC handler share memoized services.
- **Effect RPC:** `POST /api/rpc` is mounted and a typed client exists.
- **@effect-atom hydration:** `SessionAtom` is dehydrated on SSR and hydrated on the client.

If any of the above regresses, fix it before proceeding.

---

## 1. Principles (What “Effuse Everywhere” Means)

### 1.1 Thin TanStack host

TanStack Start stays responsible for:

- routing + SSR plumbing
- server functions / loaders (until we migrate them)
- mounting a single top-level UI root

Everything else should move to Effect/Effuse.

### 1.2 Typed-inspired direction (without adopting Typed wholesale)

We want Typed’s “Effect-first full stack” benefits:

- typed event handlers in templates
- reactive state that drives rendering (no React `useState/useEffect` for UI state)
- SSR + hydration of Effect-owned state
- a single typed API surface + derived clients
- testability (render templates, drive navigation/state, assert outputs)

But we will keep Effuse’s core design (no VDOM) and add only what we need.

---

## 2. Step-by-Step Roadmap

Each phase has a concrete “Definition of Done” and a recommended verification loop.

### Phase 1: Eliminate Remaining “React-only UI Primitives”

**Objective:** stop using React components for design primitives (buttons, icons, small widgets) so Effuse pages can reuse the same visual language without React.

**Work:**

1. Create a dedicated Effuse “UI kit” package at `packages/effuse-ui/` (consumed by `apps/web`).
2. Port remaining SVG + CSS-module-based primitives into Effuse helpers that use **Tailwind utility classes**:
   - do **not** introduce new global `.oa-*` component classes
   - use Tailwind arbitrary values/variants when needed for SVG attributes and precise styling
   - example: `hatcheryButton()` (`packages/effuse-ui/src/hatcheryButton.ts`)
3. Make Effuse pages use these helpers (which internally use Tailwind classes) instead of duplicating long Tailwind strings in each page.

**DoD:**

- marketing and catalog pages no longer depend on React-only UI primitives for core visuals.
- button frames, custom fonts, and icon SVGs match the old UI.

**Verify:**

- `cd apps/web && npm run build`
- `cd apps/web && npm run lint`
- smoke: open `/` and `/login` and confirm (a) full-screen layout, (b) the framed button renders, (c) navigation to `/login` is SPA.

**Phase 1 Log (Implemented 2026-02-06)**

- Added `packages/effuse-ui/` (`@openagentsinc/effuse-ui`) and implemented `hatcheryButton()` using Tailwind classes only.
- Updated `apps/web` to depend on `@openagentsinc/effuse-ui` and use it from:
  - `apps/web/src/effuse-pages/home.ts`
  - `apps/web/src/effuse-pages/header.ts`
- Removed the app-local HatcheryButton helper and component CSS:
  - deleted `apps/web/src/effuse-pages/ui/hatcheryButton.ts`
  - removed `.oa-hatchery-button*` styles from `apps/web/src/app.css`
- Ensured Tailwind includes classes used by `@openagentsinc/effuse-ui`:
  - added `@source '../node_modules/@openagentsinc/effuse-ui/src/**/*.ts';` in `apps/web/src/app.css`
- Fixed Vite/SSR resolution for local workspace packages:
  - added aliases + `ssr.noExternal` for `@openagentsinc/effuse` and `@openagentsinc/effuse-ui` in `apps/web/vite.config.ts`
- Verified: `cd apps/web && npm run lint` and `cd apps/web && npm run build`

---

### Phase 2: Move HUD Backgrounds to Effuse (Remove React Canvas Hooks)

**Objective:** remove `@openagentsinc/hud/react` usage from routes so backgrounds are Effuse-managed.

**Work:**

1. Add Effuse-backed HUD helpers that render canvases and call existing non-React functions:
   - `@openagentsinc/hud` already has `createBackgroundDots` and `createBackgroundGridLines` (no React required).
2. Implement an Effuse component/service that:
   - creates `<canvas>` elements in the template
   - on mount, wires ResizeObserver + draws
   - on unmount/swap, cancels observers
3. Replace React `<DotsBackground/>` and `<DotsGridBackground/>` with Effuse equivalents in:
   - marketing layout
   - modules/tools/signatures
   - autopilot

**DoD:**

- routes no longer import from `@openagentsinc/hud/react`.
- backgrounds remain full-bleed and stable across navigation.

**Verify:**

- same as Phase 1 + visually confirm canvas still animates/renders on resize.

---

### Phase 3: Convert Autopilot Chrome to Effuse (Sidebar, Blueprint Panel, Controls)

**Objective:** make `/autopilot` a single Effuse-rendered page (or a small set of Effuse islands) and reduce React to routing + providers only.

**Work:**

1. Convert `apps/web/src/components/layout/AutopilotSidebar.tsx` into an Effuse program that renders:
   - nav links
   - account area / sign out trigger
2. Convert blueprint editor UI into Effuse:
   - blueprint view
   - edit form
   - save/export/reset controls
3. Use the same approach as today:
   - keep data/state in the route initially
   - pass payloads into Effuse
   - wire events via `onRendered`

**DoD:**

- `/autopilot` is visually identical but React renders only the mount container(s).
- sidebar navigation links remain SPA (via EffuseMount interception).

**Verify:**

- smoke `/autopilot` (unauth redirect still works)
- sign-out clears root auth cache and forces refetch on next nav

---

### Phase 4: Move UI State to Effect (Stop Using React State for Page Logic)

**Objective:** React should stop being the state manager. Effect should own UI state.

**Work (recommended order):**

1. Standardize on one state mechanism:
   - short term: `@effect-atom` for UI state + hydration
   - longer term: Effuse `StateCell` ergonomics (see Effuse `ROADMAP.md`) if it becomes the preferred abstraction
2. Convert “page payload” building from `useMemo/useState` into Effect-owned state:
   - catalog data (modules/tools/signatures) -> atoms
   - autopilot UI state (isEditingBlueprint, drafts, etc.) -> atoms
3. Introduce a small “render driver” that reruns Effuse render when relevant state changes:
   - minimal version: subscribe to atom changes and trigger a single re-render (no VDOM; just rerun Effuse render with a derived payload)

**DoD:**

- routes no longer contain meaningful UI state in React (ideally none besides trivial refs).
- Effuse renders are driven by Effect state updates.

**Verify:**

- route transitions preserve state as expected
- hydration initializes atoms correctly (no “flash” to null session)

---

### Phase 5: Typed-Style Template Directives (Remove `onRendered` DOM Queries)

**Objective:** stop wiring behavior from React via `onRendered` callbacks.

This is the key gap vs. Typed: Typed templates can bind events and update state without leaving the template layer.

**Work:**

1. Add an Effuse directive/event binding mechanism (Typed-inspired) with a strict contract:
   - events are declared in templates
   - handlers are typed and run as Effects
   - handler errors are captured and logged with Telemetry
2. Keep it minimal at first:
   - start with `click` + `submit` + `input`
   - target element and values extraction should be deterministic and tested
3. Migrate existing `onRendered` behaviors:
   - autopilot chat form submit
   - stop button
   - scroll-to-bottom
   - blueprint save/export
4. Prefer using (or extending) Effuse’s existing EZ/hypermedia runtime if it fits the semantics; otherwise add a dedicated “template events” system.

**DoD:**

- no route needs to query DOM nodes to attach listeners for core UX flows.
- Effuse templates declare events; Effects run through the shared runtime.

**Verify:**

- add targeted tests (happy-dom) for event bindings and basic flows.

---

### Phase 6: Move Autopilot Chat to Effect (Replace React Hooks)

**Objective:** remove `useAgent` / `useAgentChat` from React and make the chat loop an Effect service.

**Work:**

1. Create a `ChatService` (Effect) that encapsulates:
   - connecting/resuming
   - message stream (including tool parts)
   - send/stop/clear history
2. Model chat as Effect state (`@effect-atom` or `StateCell`) and expose a stable API.
3. Migrate `/autopilot` to consume the service, not hooks.

**DoD:**

- the chat experience works without React hooks.
- UI is driven only by Effect state and template directives.

**Verify:**

- reconnection works
- SSR safety: server render should not attempt WebSocket calls

---

### Phase 7: Effuse SSR + Hydration (Reduce Client Work, Remove “Render After Hydration”)

**Objective:** stop rendering Effuse pages only client-side. Render them on the server and hydrate on the client.

This is the hardest phase; it is also where Typed is an excellent reference implementation.

**Work:**

1. Add a server-side renderer for Effuse `TemplateResult`:
   - `TemplateResult -> string` (must match client rendering semantics)
2. Add a minimal hydration model:
   - stable markers/ids for event directives and stateful nodes
   - hydration should attach behavior without tearing down DOM
3. Integrate with TanStack Start SSR output:
   - server route renders Effuse HTML into the response stream
   - client hydrates and continues with the same state

**DoD:**

- first-paint for key pages is server-rendered Effuse HTML (not an empty mount).
- hydration attaches behavior without a full DOM replace.

**Verify:**

- measure layout stability: no reflow or flicker on hydration
- add SSR snapshot tests for core pages

---

### Phase 8: Collapse React to a True “Host Only” Layer

**Objective:** React is only a minimal host for TanStack Router + providers.

**Work:**

1. Reduce each file route to:
   - loader/beforeLoad
   - render a single `<EffuseMount/>` (or a very small number of mounts)
2. Prefer a single “App Shell” Effuse mount that renders:
   - shell + sidebar + background + outlet content
   - route content selected based on router state passed to Effuse

**DoD:**

- route components contain almost no UI code.
- UI changes do not require React changes.

---

## 3. Verification Loop (Repeat Every Phase)

Minimum:

- `cd apps/web && npm run build`
- `cd apps/web && npm run lint`
- smoke check (unauth is fine):
  - `/`
  - `/login`
  - `/modules` (redirects unauth)
  - `/autopilot` (redirects unauth)

When changing navigation/events/hydration:

- verify `/` -> `/login` is SPA (no full refresh)
- verify back/forward works

---

## 4. Notes For Future Agents

- Prefer adding capabilities to Effuse (directives, SSR/hydration, testing) over adding more React glue.
- Keep the runtime boundaries clean:
  - adapters do parsing only
  - retries/guardrails live in the runtime/operators
- Don’t re-introduce CSS modules for primitives Effuse must render; prefer Tailwind utility classes directly in templates (including arbitrary variants).
  - keep global CSS limited to true globals (fonts, `@layer base` resets, theme tokens)
