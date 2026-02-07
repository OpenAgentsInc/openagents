# Master Plan: Complete Effect + Effuse Stack (No React, No TanStack)

**Status:** Draft (2026-02-07)  
**Audience:** implementers of `@openagentsinc/effuse` + maintainers of `apps/web`  
**Goal:** define the end-state architecture and migration plan where **Effect + Effuse fully replace React + TanStack** for the OpenAgents web app, while staying consistent with Effuse’s design constraints (no VDOM, explicit swaps, Effect-native services).

This document consolidates and extends the existing Effuse framework docs and the `apps/web` integration docs into a **single end-to-end plan**.

## 0. End State (What “Complete” Means)

When complete, the web product is a *pure Effect/Effuse application*:

- **No React**: no `react`, no JSX/TSX runtime in production.
- **No TanStack**: no `@tanstack/start`, `@tanstack/router`, `@tanstack/react-query` (or any TanStack UI/runtime dependency).
- **Effuse is the UI runtime**:
  - HTML templates (`html```, `TemplateResult`)
  - explicit DOM swaps (`DomService.swap`)
  - component render loops (`StateCell` + `mountComponent`) when needed
  - hypermedia actions / event binding via EZ (`data-ez-*`)
  - SSR rendering via `renderToString(TemplateResult)`
  - hydration/boot with no “tear down and replace DOM” for first paint
- **Effect is the application runtime**:
  - routing, loaders, navigation, state, side effects, caching
  - typed request boundaries (RPC/HTTP) and telemetry
  - a single shared Layered composition root (server + client)

## 1. Non-Goals (For This Master Plan)

- Replacing *every backend* immediately (Convex / WorkOS / Autopilot worker) is not required for removing React/TanStack. They must be wrapped behind Effect services so they can remain during migration.
- Replacing the build tool is not required. Vite (or an equivalent bundler) can remain as an implementation detail.

## 2. Baseline (What Exists Today)

Effuse core (see `README.md`, `ARCHITECTURE.md`, `SPEC.md`, `DOM.md`, `EZ.md`):

- Templates: `html`, escaping, `TemplateResult`
- DOM: swap modes + focus preservation (`DomService.swap`)
- State: `StateCell` and component mount lifecycle
- EZ runtime: `data-ez-*` action attributes with delegated listeners and switch-latest semantics
- Testing: vitest + happy-dom contract tests (currently EZ-focused)

`apps/web` integration (see `INDEX.md`, `effuse-conversion-apps-web.md`, `ROUTER-AND-APPS-WEB-INTEGRATION.md`, `APPS-WEB-FULL-EFFUSE-ROADMAP.md`):

- Effect runtime in router context, shared server runtime via `MemoMap`
- Effect RPC mounted at `POST /api/rpc` (ADR-0027)
- Minimal SSR atom hydration via `@effect-atom` (ADR-0027)
- Most UI already renders via Effuse, with React/TanStack still providing the hosting substrate

This master plan starts from that baseline and describes how to **remove the remaining substrate**.

## 3. Target Architecture (React/TanStack-Free)

### 3.1 Single Composition Root: `AppRuntime`

Define one canonical Layer bundle, with server/client specializations.

**Core invariants:**

- All “global” dependencies (config, telemetry, HTTP, auth, RPC client, etc.) are provided via Effect `Layer`s.
- The UI runtime (`EffuseLive`) is provided via `Layer`s (browser DOM on client; SSR-only rendering services on server).

**Recommended service surface (illustrative, not exhaustive):**

- `AppConfigService` (env/config)
- `TelemetryService`
- `HttpClient` (`@effect/platform`), plus `Fetch` implementation per environment
- `AuthService` (WorkOS session, sign-in/out; server cookie parsing)
- `AgentRpcClientService` (or successor)
- `RouterService` (see below)
- `DomService` (client only), `Document/Window/History` split over time (per `ROADMAP.md`)

### 3.2 Route Contract (Shared by Server + Client)

Replace TanStack “file routes + loaders” with an Effect-native route table.

Each route is a typed contract:

- `id`: stable identifier for telemetry/debugging
- `match`: path pattern + param parsing
- `loader`: `Effect<LoaderData>` (server + client compatible), cancellable
- `view`: `Effect<TemplateResult>` (pure rendering from loader data + global state)
- `head`: optional computed metadata (title, meta tags)
- `guards`: optional auth/redirect decisions as Effects

**Key constraint:** route definitions must be importable and runnable in both SSR and the browser.

### 3.3 Server: `EffectWebHost` (SSR + RPC + Static)

Replace TanStack Start’s server runtime with an Effect server that:

- Serves static assets (the built client bundle)
- Handles `POST /api/rpc` (Effect RPC) and any other API endpoints
- Performs SSR for `GET /*` via the same route contract
- Emits consistent telemetry across SSR, RPC, and auth endpoints

SSR pipeline:

1. Parse request (URL, headers, cookies) into a request-scoped service (`RequestContext`)
2. Match route
3. Run `guards` (redirects / auth checks)
4. Run `loader` to produce `LoaderData`
5. Render `TemplateResult` via `view`
6. `renderToString` to HTML
7. Produce a response HTML document:
   - server HTML for the UI root
   - dehydrated state payload (atoms or equivalent)
   - boot script + asset references

### 3.4 Client: `EffuseApp` (Boot + Navigation + Rendering)

Replace React root mounting with a plain Effect bootstrap:

- Build the client `AppRuntime` Layer
- Hydrate initial state
- Mount the EZ runtime once on the root
- Start the router loop (History + link interception)
- Render the current route into the root container

Navigation pipeline:

- Intercept internal anchor clicks (same-origin) at the Effuse root
- Convert to `router.navigate(href)` Effect
- Cancel in-flight `loader` for the previous navigation (switch-latest)
- Swap only the outlet area where possible (avoid nuking persistent shell)

### 3.5 State + Data (Effect-Owned)

Remove React state and TanStack Query by standardizing on Effect primitives:

- UI state: `@effect-atom` (SSR serializable) and/or Effuse `StateCell` (local, component-scoped)
- Server/shared caches: `MemoMap`, `Cache`, `Request`/`RequestResolver` patterns
- Network: `@effect/platform` `HttpClient` (or RPC-first)

**Rule:** if state drives UI, it must be observable by Effuse (Atom subscription or Stream) and must not require React rendering to reflect changes.

### 3.6 UI Interaction Model (Effuse-First)

Two primitives remain the core UX building blocks:

1. **Components** for durable state machines + subscriptions.
2. **EZ actions** for declarative, localized DOM updates.

The end-state should use EZ for:

- forms (submit -> run Effect -> swap result)
- buttons/toggles
- “load more” and incremental list updates
- progressive enhancement where possible

…and reserve components for:

- long-lived screens with multiple state sources
- streaming views (chat, logs)
- subscriptions (websocket, timers, push streams)

### 3.7 SSR + Hydration (Effuse-Native)

To remove React, SSR/hydration must be done with Effuse primitives:

- SSR: `TemplateResult -> renderToString -> HTML`
- Hydration: attach behavior (EZ runtime, router interception, subscriptions) to existing DOM without replacing it
- Any initial client render should be a no-op if the DOM matches the SSR output (or should patch minimally)

This implies:

- stable container structure (shell + outlet)
- stable markers for event binding where needed (EZ already uses attributes)
- deterministic rendering between server and client (same escape/serialize rules)

## 4. Replacement Matrix (React/TanStack -> Effect/Effuse)

| React/TanStack capability | Replacement |
|---|---|
| JSX UI | Effuse `html`` + helpers (`effuse-ui` style) |
| Component state (`useState`, `useEffect`) | `StateCell`, `@effect-atom`, Effects + Streams |
| Router (`@tanstack/router`) | Effect-native `RouterService` (History API + route table) |
| Loaders/serverFns (Start) | Route `loader` Effects (SSR + client) |
| SPA navigation (`<Link/>`) | `<a href>` + router interception (built into RouterService/EZ) |
| React Query caching | Effect `Cache` / `Request` / `MemoMap` patterns |
| Error boundaries | Effect error channels + EZ error targets + top-level “render error template” |
| SSR rendering | `renderToString(TemplateResult)` + Effect server |
| Hydration | Effuse boot that attaches behavior without DOM teardown |

## 5. Migration Plan (Phased, Shippable)

This is the concrete, incremental path from today to the end state.

### Phase 0: Lock Baseline + Hard Contracts

Deliverables:

- Treat `packages/effuse/docs/SPEC.md` + `DOM.md` as contracts and add missing tests (swap semantics, focus restoration, param collection).
- Ensure SSR rendering (`renderToString`) stays DOM-free via node-env tests.
- Ensure EZ runtime is mount-once and does not leak listeners.

DoD:

- Effuse has contract tests for: swap modes, focus restore, EZ parsing + params + cancellation, and render-to-string snapshots.

### Phase 1: Introduce an Effect-Native Router (Inside the Existing Host)

Deliverables:

- Implement a minimal `RouterService`:
  - route matching
  - `navigate(href)`
  - popstate handling
  - link interception at root
  - cancellable loader pipeline (switch-latest)
- Implement a top-level Effuse “app shell” template:
  - persistent chrome
  - `<main data-outlet>` target where route content swaps

DoD:

- Navigation between major pages works with **no TanStack router calls** (even if TanStack/React still exist as the hosting scaffold).

### Phase 2: Collapse `apps/web` to a Single Catch-All (Remove TanStack Routing Semantics)

Deliverables:

- Convert the web app’s route tree to a single “catch-all” host page that always serves the Effuse shell (still within the old system if necessary).
- Ensure all internal navigation is driven by the new `RouterService`.

DoD:

- The app no longer depends on TanStack’s notion of route components for UI; TanStack is only an asset+SSR transport.

### Phase 3: Stand Up `EffectWebHost` in Parallel

Deliverables:

- Create a new Effect server (Node/Bun/Worker target) that can:
  - serve the built assets
  - SSR the Effuse app via route table
  - host `POST /api/rpc`
  - host auth endpoints (`/api/auth/*`) compatible with existing flows
- In dev, proxy static asset serving to Vite as needed.

DoD:

- You can run the app end-to-end via the Effect server locally (SSR + client navigation + RPC).

### Phase 4: Production Cutover (Remove TanStack Start)

Deliverables:

- Deploy `EffectWebHost` as the production server entrypoint.
- Remove TanStack Start server runtime from the deployment pipeline.

DoD:

- Production traffic is served by the Effect server; TanStack Start is no longer in the deploy artifact.

### Phase 5: Remove React (No TSX, No React Providers)

Deliverables:

- Replace `EffuseMount.tsx` and any remaining TSX-based composition with a plain boot module (e.g. `apps/web/src/main.ts` or equivalent).
- Replace Convex/WorkOS React providers/hooks with Effect services (or move calls behind RPC/HTTP boundaries).

DoD:

- No runtime React dependency remains; the UI boots and runs from Effect.

### Phase 6: Remove Remaining TanStack/React-Query Dependencies

Deliverables:

- Remove TanStack Router/Query usage from code and dependencies.
- Replace any remaining caching/query patterns with Effect `Cache`/`Request` patterns.

DoD:

- No TanStack packages remain in the dependency graph.

### Phase 7: Hardening + Ergonomics (Make It Pleasant)

Deliverables:

- Effuse state ergonomics: `computed`, `eq` dedupe, `batch` (per `ROADMAP.md` / `inspiration-typed.md`)
- EZ runtime v1 features (loading indicators, error targets, concurrency policies)
- HMR for Effuse templates/components (see `ORIGIN.md` for historical precedent)
- Router prefetch + transition patterns (optional)
- Performance: introduce “template parts” for fine-grained updates (Phase 4 in `ROADMAP.md`) if full swaps become limiting

DoD:

- The app feels “native”: stable focus/caret, minimal flicker, predictable navigation, test coverage for core flows, and no regressions in SSR/hydration.

## 6. Open Decisions / Questions

These must be decided explicitly to finish the React/TanStack removal:

- **Hosting target:** Cloudflare Worker vs Node/Bun for `EffectWebHost`.
- **Auth integration:** how WorkOS AuthKit middleware maps into the Effect server request pipeline (cookie/session parsing, redirect flows, CSRF).
- **Convex usage:** keep it behind RPC/HTTP boundaries or integrate a non-React Convex client directly as an Effect service.
- **Hydration strictness:** do we require DOM-perfect hydration for all routes, or allow “first client render swaps outlet” for some screens?
- **Route code-splitting:** whether to support lazy route modules (dynamic import) and how to represent that in the route table.

## 7. Related Docs (Sources Consolidated Here)

- Effuse framework:
  - `README.md`
  - `ARCHITECTURE.md`
  - `SPEC.md`
  - `DOM.md`
  - `EZ.md`
  - `ROADMAP.md`
  - `TESTING.md`
  - `inspiration-typed.md`
  - `inspiration-HTMX.md`
- `apps/web` integration:
  - `INDEX.md`
  - `effuse-conversion-apps-web.md`
  - `ROUTER-AND-APPS-WEB-INTEGRATION.md`
  - `APPS-WEB-FULL-EFFUSE-ROADMAP.md`
  - `effect-migration-web.md`
  - `effect-rpc-web.md`
  - `tanstack-start-effect-comparison.md`
  - `DELEGATION-full-effect-integration.md`
- ADR copies:
  - `adr/adr-0027-effect-rpc-and-atom-hydration-web.md`
  - `adr/adr-0022-effuse-uitree-ipc.md` (desktop UITree/patch IPC contract; orthogonal to React/TanStack removal)

