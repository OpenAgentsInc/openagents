# Master Plan: Complete Effuse Stack (No React, No TanStack)

**Status:** Draft (2026-02-07)  
**Audience:** implementers of `@openagentsinc/effuse` + maintainers of `apps/web`  
**Goal:** define the end-state architecture and migration plan where **Effuse (built on Effect) fully replaces React + TanStack** for the OpenAgents web app, while staying consistent with Effuse’s design constraints (no VDOM, explicit swaps, Effect-native services).

This document consolidates and extends the existing Effuse framework docs and the `apps/web` integration docs into a **single end-to-end plan**.

## 0. End State (What “Complete” Means)

When complete, the web product is a *pure Effuse application (built on Effect)*:

- **No React**: no `react`, no JSX/TSX runtime in production.
- **No TanStack**: no `@tanstack/start`, `@tanstack/router`, `@tanstack/react-query` (or any TanStack UI/runtime dependency).
- **Multi-backend host, Cloudflare-first**: Effuse is designed to run on multiple server backends, but the initial target (and plan in this doc) is **Cloudflare Workers** plus the Cloudflare primitives we already depend on (Durable Objects, DO SQLite, R2, KV, Queues, etc.).
- **Effuse is the application framework/runtime (built on Effect)**:
  - Think: “Effuse is to Effect what Next.js is to React”: Effuse owns the web app’s UI *and* the app loop (routing/loaders/navigation/SSR/hydration), and it is implemented in terms of Effect.
  - UI primitives:
    - HTML templates (`html```, `TemplateResult`)
    - explicit DOM swaps (`DomService.swap`)
    - component render loops (`StateCell` + `mountComponent`) when needed
    - hypermedia actions / event binding via EZ (`data-ez-*`)
  - App/runtime primitives (also Effuse-owned in the end state):
    - routing + loaders + navigation (`RouterService`, Effuse-owned and implemented with Effect)
    - SSR rendering via `renderToString(TemplateResult)`
    - hydration/boot with no “tear down and replace DOM” for first paint
    - typed request boundaries (RPC/HTTP), budgets, receipts/replay, telemetry
    - a single shared composed runtime (an Effect `ManagedRuntime` built from the app Layer) usable on server + client

## 0.1 Engineering Invariants (Pulled In From `docs/autopilot/*`)

The “no React / no TanStack” end-state must still preserve the core Autopilot/DSE posture:

- **Schema-first everywhere**: domain state and request boundaries are validated with Effect `Schema` (inputs, outputs, persisted records, exports/imports).
- **Artifact-first, no learning in prod**: production execution loads pinned artifacts/policies; “optimization/compile” is explicit and offline; artifacts are immutable and promotion is pointer-only.
- **Adapters only parse/format**: validation, retries, budgets, receipts, and error normalization live in runtime operators/middleware, not in API/DOM adapters.
- **Everything is replayable**: stable IDs + deterministic hashes appear in receipts (e.g. `signatureId`, `compiled_id`, prompt/policy hashes, tool call receipts).
- **Budgets are enforced**: time, steps, tool calls, LLM calls, and output-size caps are first-class and observable (why we stopped is always answerable).
- **Large inputs are blobs**: big pasted/code/log content is stored once (`BlobStore`) and referenced (`BlobRef`) in prompts/receipts (“two-bucket context”); don’t duplicate huge text in token space.
- **Tool-call failures are user-visible**: tool-call repair is used where possible; `tool-error` parts must render (or degrade to a visible fallback) so the UI never “silently stalls”.
- **Telemetry is a service**: structured, namespaced telemetry with request correlation (`requestId`) and best-effort sinks; never blocks user flows; SSR-safe.
- **No containers posture holds**: if we ever add “code mode”, it must be capability-limited (externals-only), checkpointable at I/O, and strictly resource-bounded (Monty model).

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
  - Prefer a single composed runtime (ManagedRuntime) with middleware-style wrapping for cross-cutting concerns (telemetry, errors, abort), rather than sprinkling `Layer.provide(...)` ad hoc.

**Recommended service surface (illustrative, not exhaustive):**

- `AppConfigService` (env/config)
- `TelemetryService` (namespaced, best-effort sinks, request correlation)
- `HttpClient` (`@effect/platform`), plus `Fetch` implementation per environment
- `AuthService` (WorkOS session, sign-in/out; server cookie parsing)
- `AgentRpcClientService` (or successor)
- `ExecutionBudget` (time/steps/tool+LLM call caps, output-size caps)
- `ReceiptRecorder` (stable hashes + tool receipts for replay/debug)
- `BlobStore` (BlobRef storage for large context inputs)
- `RouterService` (see below)
- `DomService` (client only), `Document/Window/History` split over time (per `ROADMAP.md`)

### 3.2 Route Contract (Shared by Server + Client)

Replace TanStack “file routes + loaders” with an Effuse-native route table (implemented with Effect).

Each route is a typed contract, but **loaders must return a first-class outcome type**, not “just data”, so the host/router doesn’t devolve into ad hoc behavior.

Proposed minimal interface (v1):

```ts
// IMPORTANT: the real implementation should use Effect Schema-tagged errors,
// not `unknown`. This is just the shape the framework must standardize on.

export type RouteId = string

export type RouteMatch = {
  readonly pathname: string
  readonly params: Readonly<Record<string, string>>
  readonly search: URLSearchParams
}

export type RouteHead = {
  readonly title?: string
  readonly meta?: ReadonlyArray<readonly [name: string, content: string]>
}

export type RouteContext = {
  readonly url: URL
  readonly match: RouteMatch
  // Server-only; on the client this is null/undefined.
  readonly request?: Request
}

export type CachePolicy =
  | { readonly mode: "no-store" }
  | { readonly mode: "cache-first"; readonly ttlMs?: number }
  | { readonly mode: "stale-while-revalidate"; readonly ttlMs: number; readonly swrMs: number }

export type CookieMutation =
  | { readonly _tag: "Set"; readonly name: string; readonly value: string; readonly attributes?: string }
  | { readonly _tag: "Delete"; readonly name: string; readonly attributes?: string }

export type RouteOkHints = {
  readonly cache?: CachePolicy
  readonly headers?: ReadonlyArray<readonly [string, string]>
  readonly cookies?: ReadonlyArray<CookieMutation> // server only
  readonly dehydrate?: unknown // merged into the SSR hydration payload
  readonly receipts?: unknown // optional receipt fragments (tool/llm budget) to surface in UI/devtools
}

export type RouteOutcome<A> =
  | { readonly _tag: "Ok"; readonly data: A; readonly hints?: RouteOkHints }
  | { readonly _tag: "Redirect"; readonly href: string; readonly status?: 301 | 302 | 303 | 307 | 308 }
  | { readonly _tag: "NotFound" }
  | { readonly _tag: "Fail"; readonly status?: number; readonly error: unknown }

export type HydrationMode = "strict" | "soft" | "client-only"
export type NavigationSwapMode = "outlet" | "document"

export type Route<LoaderData> = {
  readonly id: RouteId

  // Path matching + param parsing (must be deterministic and shared).
  readonly match: (url: URL) => RouteMatch | null

  // Loaders return RouteOutcome so redirect/not-found/cache/dehydrate is standardized.
  readonly loader: (ctx: RouteContext) => Effect.Effect<RouteOutcome<LoaderData>>

  // Views are pure w.r.t. loader data (side effects belong in loader/subscriptions).
  readonly view: (ctx: RouteContext, data: LoaderData) => Effect.Effect<TemplateResult>

  readonly head?: (ctx: RouteContext, data: LoaderData) => Effect.Effect<RouteHead>

  // Defaults: hydration="strict", navigation.swap="outlet"
  readonly hydration?: HydrationMode
  readonly navigation?: { readonly swap?: NavigationSwapMode }
}
```

Host-level normalization (so implementers don’t invent “three routers”):

- `RouterService` (client) and `EffuseWebHost` (server) should normalize route execution into a single internal “run” shape (e.g. `RouteRun`) that carries:
  - redirect/not-found/fail vs ok+rendered template
  - headers/cookies/dehydrate/cache hints
  - telemetry boundaries (loader/view/head spans)

**Key constraint:** route definitions must be importable and runnable in both SSR and the browser.

### 3.2.1 Middleware-Style Boundaries (Telemetry, Errors, Abort)

Where TanStack/React used framework boundaries (loaders, error boundaries, providers), the end-state should use explicit Effect middleware at the server and router boundaries:

- **Telemetry middleware**: annotate logs with `requestId`/`routeId`/`threadId` and emit lifecycle events; flush any bounded sinks in finalizers (best-effort).
- **Error normalization middleware**: map typed failures to user-safe responses/templates, and avoid double-logging by centralizing error emission.
- **Abort/cancellation middleware**: navigation cancels in-flight loaders; server requests observe abort signals and terminate safely.

### 3.3 Server: `EffuseWebHost` (SSR + RPC + Static)

Replace TanStack Start’s server runtime with an Effuse host (implemented with Effect) that:

- Serves static assets (the built client bundle)
- Handles `POST /api/rpc` (Effect RPC) and any other API endpoints
- Performs SSR for `GET /*` via the same route contract
- Emits consistent telemetry across SSR, RPC, and auth endpoints

### 3.3.1 Backend Targets (Multi-Backend, Cloudflare First)

Effuse should support multiple server backends, but we want to **ship on Cloudflare first** and treat portability as an explicit adapter boundary.

Hard requirements for the host surface:

- Use standard Web APIs (`Request`, `Response`, `Headers`, `URL`) as the primary contract so Cloudflare is the “native” environment.
- Keep backend-specific details behind `EffuseWebHost` adapters so `RouterService` + route tables don’t hardcode a hosting platform.

Initial planned backend: **Cloudflare Workers**, plus relevant Cloudflare infra:

- **Durable Objects** (and DO SQLite) for per-user durable state (Autopilot thread, Blueprint/bootstrap state, receipts, policy registry pointers).
- **R2** for large blob storage (`BlobStore`) and any large tool/LLM artifacts.
- **KV** for small global caches/flags/pointers where appropriate (not canonical state).
- **Queues** for background or deferred work (e.g. compile jobs, log flushing) when needed.
- **D1** only if/when we need a global relational store (optional).

Secondary backend (later, optional): Node/Bun adapter for local dev or alternative deployment, but the plan in this doc should assume Cloudflare’s constraints and primitives.

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

### 3.4.1 Router Data Cache, Dedupe, and Cancellation

`RouterService` must define caching and dedupe rules up front, otherwise we will reinvent React Query inconsistently.

Minimum rules (v1):

- **Loader keying**: each loader run has a stable key derived from `(routeId, pathname, params, search, auth/session scope)`.
- **In-flight dedupe**: if the same loader key is requested twice, the router reuses the same in-flight fiber/result (no double fetch).
- **Cancellation**:
  - navigation is switch-latest (new navigation cancels the previous navigation’s “apply”)
  - shared in-flight loader fibers are only interrupted when no longer needed (don’t cancel a shared request that a prefetch is still awaiting)
- **Caching**:
  - default is `CachePolicy: no-store`
  - caching is enabled only when a route returns `RouteOutcome.Ok(..., { cache: ... })`
  - SWR (`stale-while-revalidate`) is explicit and recorded: render with stale data, refresh in background, re-render outlet on success
- **Prefetch**:
  - `prefetch(href)` runs the same loader with the same keying and writes into the same cache
  - prefetch must respect `CachePolicy` and must not mutate navigation history/state

Implementation note: Effect already has the primitives (`Request`, `Cache`, `MemoMap`) to build this, but the *router must standardize the rules* so pages don’t each invent their own.

### 3.4.2 Shell/Outlet Swap Is a Hard Invariant

Effuse is a swap-based UI framework; the **single biggest UX lever** is a stable shell.

Hard rule:

- **The app shell never re-renders on navigation.** By default, only the route outlet swaps (the element designated by a stable marker like `[data-effuse-outlet]`).

Opt-out (rare):

- A route may explicitly request `navigation.swap="document"` when it truly needs a full document/shell replacement. This should be exceptional and treated as a regression risk.

### 3.5 State + Data (Effect-Owned)

Remove React state and TanStack Query by standardizing on Effect primitives:

- UI state: `@effect-atom` (SSR serializable) and/or Effuse `StateCell` (local, component-scoped)
- Server/shared caches: `MemoMap`, `Cache`, `Request`/`RequestResolver` patterns
- Network: `@effect/platform` `HttpClient` (or RPC-first)

**Rule:** if state drives UI, it must be observable by Effuse (Atom subscription or Stream) and must not require React rendering to reflect changes.

### 3.5.1 Blueprint/Bootstrap State (Schema-Backed, Exportable)

Autopilot-specific “bootstrap” (Identity/User/Character/Tools/Heartbeat/Memory) should be treated as **durable, typed records** (Effect `Schema`), not a pile of markdown files:

- canonical store: per-user durable storage (Durable Object SQLite), optionally mirrored to Convex for UI/querying
- export/import: a single versioned JSON “Blueprint” format, validated at boundaries
- prompt injection: rendered “context file” view is derived; canonical representation remains structured
- memory visibility: enforce `main_only` vs `all` mechanically (not by convention)

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

### 3.6.1 Tool UX Must Not Stall

Tool calling in the AI SDK produces `tool-error` parts for invalid tool names/inputs; if the UI hides tool parts, users experience “nothing happened”.

End-state requirements:

- render tool results and tool errors (or a compact, user-visible fallback)
- log/record tool-call repair events and invalid tool calls for debugging
- keep tool UI bounded (truncate big tool I/O; use BlobRefs when large)

### 3.6.2 Tool Part Rendering Contract (Non-Negotiable)

We need a single rendering contract for “tool parts” so different screens don’t silently hide tool failures.

Framework-level requirements:

- **Always render something** for tool calls, tool results, and tool errors (even if compact).
- **Stable correlation**: display at least `toolName` and `toolCallId` (and optionally a receipt id) so logs/receipts can be traced from the UI.
- **Bounded output**:
  - truncate large tool I/O in the DOM
  - store full payloads in `BlobStore` and render as a `BlobRef` “view full” affordance
- **Error visibility**:
  - `tool-error` must be rendered as a user-visible failure state
  - budget/cancellation must also be rendered (e.g. “canceled: budget exceeded”)

Implementation posture:

- Provide a default `ToolPartRenderer` used across the app (one place to enforce truncation, receipt linking, and error visibility).

### 3.7 SSR + Hydration (Effuse-Native)

Effuse must support SSR and a crisp hydration contract. Otherwise, implementers will ship a pile of “works on my machine” hydrators.

Effuse provides three hydration modes (per-route), with a single default:

- `strict` (default): **attach behavior only**. The server renders shell + outlet HTML; the client boot mounts EZ + router + subscriptions without performing an initial outlet swap.
- `soft`: **render once on boot**. The client runs `loader` + `view` once and swaps the outlet after hydration; this is safer when SSR markup is incomplete or non-deterministic.
- `client-only`: **no SSR outlet**. The server emits an empty outlet placeholder and the client renders normally; use only when SSR is not worth it.

“Matches SSR output” must be a defined property:

- Strict hydration assumes determinism: given the same `RouteOutcome.Ok.data` and the same render pipeline (`renderToString`), the HTML output is stable. We enforce this via conformance tests, not a runtime DOM diff.
- Optionally (dev/test), we can embed a server `data-effuse-ssr-hash` and compare against a client recomputation; on mismatch, fall back to a soft outlet swap (debugging aid, not the core model).

To remove React, SSR/hydration is done with Effuse primitives:

- SSR: `TemplateResult -> renderToString -> HTML`
- Hydration: attach behavior (EZ runtime, router interception, subscriptions) to existing DOM without replacing it

This requires stable DOM structure and markers:

- a persistent shell element (e.g. `[data-effuse-shell]`)
- a stable outlet element (e.g. `[data-effuse-outlet]`) that is the default swap target
- deterministic server/client rendering rules (escape/serialize/whitespace strategy)

### 3.8 Conformance Tests (Framework-Level)

If Effuse is going to replace React/TanStack, we need framework-level tests that app code can’t “accidentally bypass”.

Minimum conformance suite:

- **SSR determinism**: snapshot `renderToString` for representative routes with fixed inputs (no `document` access in node env).
- **Hydration conformance**:
  - `strict`: boot attaches EZ/router without replacing outlet HTML
  - `soft`: boot is allowed one outlet swap, and must do so deterministically
- **Navigation cancellation**: a second `navigate()` cancels the first “apply”; loaders don’t double-commit stale results.
- **Shell/outlet invariants**: navigation swaps only `[data-effuse-outlet]` by default; shell remains stable.
- **Swap focus/caret**: caret/selection stays stable across swaps for Blueprint-like editing flows (exercise `DomService` focus restore rules).
- **Tool part visibility**: `tool-error` always renders a visible fallback; tool output truncation + BlobRefs work.
- **BlobRef discipline**: large payloads are stored as blobs and referenced in prompts/receipts/UI (no megabytes in DOM or receipt JSON).

These tests should run in CI and be treated as “framework regressions”, not app-level snapshots.

## 4. Replacement Matrix (React/TanStack -> Effect/Effuse)

| React/TanStack capability | Replacement |
|---|---|
| JSX UI | Effuse `html`` + helpers (`effuse-ui` style) |
| Component state (`useState`, `useEffect`) | `StateCell`, `@effect-atom`, Effects + Streams |
| Router (`@tanstack/router`) | `RouterService` (Effuse-owned, Effect-based; History API + route table) |
| Loaders/serverFns (Start) | Route `loader` Effects (SSR + client) |
| SPA navigation (`<Link/>`) | `<a href>` + router interception (built into RouterService/EZ) |
| React Query caching | Effect `Cache` / `Request` / `MemoMap` patterns |
| Error boundaries | Effect error channels + EZ error targets + top-level “render error template” |
| SSR rendering | `renderToString(TemplateResult)` + `EffuseWebHost` |
| Hydration | Effuse boot that attaches behavior without DOM teardown |

## 5. Migration Plan (Phased, Shippable)

This is the concrete, incremental path from today to the end state.

### Cross-Cutting Prereqs (Apply To Every Phase)

- Wire a real `TelemetryService` (namespaces + request correlation + SSR-safe sinks) and remove scattered `console.*` / ad hoc analytics calls.
- Enforce budgets and cancellation at boundaries (navigation cancels loaders; server observes abort; tool/LLM calls are bounded).
- Ensure tool-error visibility (never “silent stall”).
- Prefer BlobRefs for large payloads (avoid prompt/DOM bloat; receipts stay stable).

### Phase 0: Lock Baseline + Hard Contracts

Deliverables:

- Treat `packages/effuse/docs/SPEC.md` + `DOM.md` as contracts and add missing tests (swap semantics, focus restoration, param collection).
- Ensure SSR rendering (`renderToString`) stays DOM-free via node-env tests.
- Ensure EZ runtime is mount-once and does not leak listeners.

DoD:

- Effuse has contract tests for: swap modes, focus restore, EZ parsing + params + cancellation, and render-to-string snapshots.

### Phase 1: Router Becomes Source of Truth (Inside the Existing Host)

Deliverables:

- Implement a minimal `RouterService` (this is the real milestone; the rest is plumbing):
  - route matching
  - `navigate(href)`
  - popstate handling
  - link interception at root
  - cancellable loader pipeline (switch-latest)
  - standardized `RouteOutcome` handling (redirect/not-found/fail/ok + hints)
  - standardized cache/dedupe behavior (no per-page reinvention)
- Implement a top-level Effuse “app shell” template:
  - persistent chrome
  - `<main data-outlet>` target where route content swaps

DoD:

- TanStack no longer decides what UI renders; navigation and outlet rendering are driven by `RouterService` (even if TanStack/React still exist as a temporary host).

### Phase 2: Collapse `apps/web` to a Single Catch-All (Make TanStack a Transport Only)

Deliverables:

- Convert the web app’s route tree to a single “catch-all” host page that always serves the Effuse shell (still within the old system if necessary).
- Ensure all internal navigation is driven by the new `RouterService`.

DoD:

- The app no longer depends on TanStack’s notion of route components for UI; TanStack is only an asset/SSR transport layer.

### Phase 3: Stand Up `EffuseWebHost` in Parallel

Deliverables:

- Create a new Effuse host server on **Cloudflare Workers** that can:
  - serve the built assets
  - SSR the Effuse app via route table
  - host `POST /api/rpc`
  - host auth endpoints (`/api/auth/*`) compatible with existing flows
- In dev, proxy static asset serving to Vite as needed.

DoD:

- You can run the app end-to-end via the Effuse host locally (SSR + client navigation + RPC).

### Phase 4: Production Cutover (Remove TanStack Start)

Deliverables:

- Deploy `EffuseWebHost` as the production server entrypoint.
- Remove TanStack Start server runtime from the deployment pipeline.

DoD:

- Production traffic is served by the Effuse host; TanStack Start is no longer in the deploy artifact.

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

- **Hosting target:** Cloudflare Workers first (and which flavor: Worker vs Pages Functions), plus an explicit portability boundary for an optional Node/Bun adapter.
- **Auth integration:** how WorkOS AuthKit middleware maps into the `EffuseWebHost` request pipeline (cookie/session parsing, redirect flows, CSRF).
- **Convex usage:** keep it behind RPC/HTTP boundaries or integrate a non-React Convex client directly as an Effect service.
- **Hydration mode policy:** `strict` is the default; which routes (if any) are allowed to use `soft` or `client-only`, and whether we add a dev-only SSR hash mismatch detector.
- **Route code-splitting:** whether to support lazy route modules (dynamic import) and how to represent that in the route table.
- **Telemetry sinks:** console-only vs PostHog client-only vs server-side sinks; buffering vs drop when PostHog isn’t loaded yet.
- **Blueprint editing UX:** schema-driven forms vs markdown-like editing, and how to keep edits focus-stable under Effuse swaps.
- **Receipt surfaces:** what parts of receipts (tool calls, `compiled_id`, hashes, budgets) should be user-visible vs debug-only.

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

- Autopilot / DSE sources that inform this plan:
  - `../../../docs/autopilot/spec.md`
  - `../../../docs/autopilot/effect-telemetry-service.md`
  - `../../../docs/autopilot/effect-patterns-from-crest.md`
  - `../../../docs/autopilot/tool-handling-improvements.md`
  - `../../../docs/autopilot/bootstrap-plan.md`
  - `../../../docs/autopilot/typed-synergies.md`
  - `../../../docs/autopilot/horizons-synergies.md`
  - `../../../docs/autopilot/microcode-synergies.md`
  - `../../../docs/autopilot/rlm-synergies.md`
  - `../../../docs/autopilot/monty-synergies.md`
  - `../../../docs/autopilot/dse.md`
  - `../../../docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`
