# Master Plan: Complete Effuse Stack (No React, No TanStack)

**Status:** Draft (2026-02-07)  
**Audience:** implementers of `@openagentsinc/effuse` + maintainers of `apps/web`  
**Goal:** define the end-state architecture and migration plan where **Effuse (built on Effect) fully replaces React + TanStack** for the OpenAgents web app, while staying consistent with Effuse’s design constraints (no VDOM, explicit swaps, Effect-native services).

This document consolidates and extends the existing Effuse framework docs and the `apps/web` integration docs into a **single end-to-end plan**.

## 0. End State (What “Complete” Means)

When complete, the web product is a *pure Effuse application (built on Effect)*:

- **No React**: no `react`, no JSX/TSX runtime in production.
- **No TanStack**: no `@tanstack/start`, `@tanstack/router`, `@tanstack/react-query` (or any TanStack UI/runtime dependency).
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
- **Multi-backend host, Cloudflare-first**: Effuse is designed to run on multiple server backends, but the initial target (and plan in this doc) is **Cloudflare Workers** plus relevant Cloudflare infra (Durable Objects, DO SQLite, R2, KV, Queues, etc.).
- **Single Worker host** (Cloudflare): in production, **one Worker** serves SSR + static assets + API endpoints (`/api/*`). This is a “single host” architecture, not a split across Pages Functions / multiple workers.

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

## 0.2 Implementer Quick Start (Non-Normative)

This section is a “how do I not accidentally violate the spec” cheat sheet. The normative rules are in Appendix A.

- If you are adding a route: implement `Route.guard` (optional) + `Route.loader` returning `RouteOutcome`, and keep the UI pure in `Route.view` (return `TemplateResult`, do not perform side effects in views).
- If you need state:
  - Shared/durable UI state: `@effect-atom/atom` (SSR-hydratable only if `Atom.serializable(...)` with stable key + `Schema`).
  - Component-local/high-frequency state: `StateCell` (ephemeral, not SSR/hydration, not a cross-route cache).
- If you need interactivity: prefer EZ (`data-ez-*`) for localized actions; use component loops only for streaming/subscriptions.
- If you are adding a tool: schema-first params/outputs, bounded outputs + `BlobRef` for large payloads, and always emit receipts (toolName/toolCallId correlation).
- If you are touching Convex: do not use React bindings; use the Effect-first `ConvexService` and standard wrappers in `apps/web/convex/effect/*`.
- If you are touching AI: do not introduce new provider SDKs in app code; treat AI as `@effect/ai` (`LanguageModel` + `Response` parts) and keep WebSocket streaming as the canonical transport.

## 1. Non-Goals (For This Master Plan)

- Replacing *every backend* immediately (Convex / WorkOS / Autopilot worker) is not required for removing React/TanStack. They must be wrapped behind Effect services so they can remain during migration.
- Replacing the build tool is not required. Vite (or an equivalent bundler) can remain as an implementation detail.

## 2. Baseline (What Exists Today)

Effuse core (see `README.md`, `ARCHITECTURE.md`, `SPEC.md`, `DOM.md`, `EZ.md`):

- Templates: `html`, escaping, `TemplateResult`
- DOM: swap modes + focus preservation (`DomService.swap`)
- State: `StateCell` and component mount lifecycle
- EZ runtime: `data-ez-*` action attributes with delegated listeners and switch-latest semantics
- Testing: vitest + happy-dom contract + conformance tests (EZ + swaps + router + SSR determinism)

`apps/web` integration (see `INDEX.md`, `effuse-conversion-apps-web.md`, `ROUTER-AND-APPS-WEB-INTEGRATION.md`, `APPS-WEB-FULL-EFFUSE-ROADMAP.md`):

- Effect runtime in router context, shared server runtime via `MemoMap`
- (legacy) Effect RPC was mounted at `POST /api/rpc` (ADR-0027). As of 2026-02-08, the Effuse Worker host uses narrower HTTP endpoints (auth/contracts/autopilot) instead of a general RPC mount.
- Minimal SSR atom hydration via `@effect-atom/atom` (and currently `@effect-atom/atom-react` while React is still the host) (ADR-0027)
- `apps/web` is now “Effuse everywhere” at the page level (templates + swaps + EZ actions + Effect-owned state), while React/TanStack still provide the hosting substrate (file routes, SSR glue, providers).
- Shared Effuse UI primitives live in `@openagentsinc/effuse-ui` (Tailwind-first helpers).
- Autopilot chat is Effect-first (`ChatService`) and the UI is driven by atoms (not React hooks).
- Key routes are SSR-rendered as Effuse HTML and hydrated without DOM teardown (via mount-level `ssrHtml`), even before TanStack is removed.
- **MVP direction (resolved, implemented 2026-02-08):** Autopilot chat is **Convex-first**:
  - the `apps/web` Worker runs inference (Cloudflare `AI` binding) and writes chunked `messageParts` to Convex
  - the browser subscribes to Convex for realtime updates (Convex WS)
  - the legacy Agents SDK + `/agents/*` Durable Object transport is removed from `apps/web`
  - per-user Cloudflare execution planes (DO/DO SQLite) are deferred to post-MVP optimizations.

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
- `ConvexService` (Effect-first wrapper over Convex HTTP + WS clients; no React bindings)
- `AgentRpcClientService` (or successor)
- `LanguageModel` (`@effect/ai/LanguageModel`) (Effect-native LLM interface; provider-backed)
- `Tokenizer` (`@effect/ai/Tokenizer`) (prompt token counting + truncation for budget enforcement)
- `EmbeddingModel` (`@effect/ai/EmbeddingModel`) (embeddings with batching/caching)
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

export type RouteContext =
  | { readonly _tag: "Server"; readonly url: URL; readonly match: RouteMatch; readonly request: Request }
  | { readonly _tag: "Client"; readonly url: URL; readonly match: RouteMatch }

export type CachePolicy =
  | { readonly mode: "no-store" }
  | { readonly mode: "cache-first"; readonly ttlMs?: number }
  | { readonly mode: "stale-while-revalidate"; readonly ttlMs: number; readonly swrMs: number }

export type CookieMutation =
  | { readonly _tag: "Set"; readonly name: string; readonly value: string; readonly attributes?: string }
  | { readonly _tag: "Delete"; readonly name: string; readonly attributes?: string }

// Merge rules:
// - `dehydrate` is stored under a routeId namespace (no deep merge).
// - `receipts` is append-only (array) or a stable-id map (no last-write-wins).
export type DehydrateFragment = unknown
export type ReceiptsFragment = unknown

export type RouteOkHints = {
  readonly cache?: CachePolicy
  readonly headers?: ReadonlyArray<readonly [string, string]>
  readonly cookies?: ReadonlyArray<CookieMutation> // server only
  readonly dehydrate?: DehydrateFragment
  readonly receipts?: ReceiptsFragment
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

  // Guards are optional sugar but MUST be standardized to avoid ad hoc redirects in loaders.
  // If a guard returns an outcome, the route run short-circuits (redirect/not-found/fail).
  readonly guard?: (ctx: RouteContext) => Effect.Effect<RouteOutcome<never> | void>

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

### 3.3 Server: `EffuseWebHost` (SSR + APIs + Static)

Replace TanStack Start’s server runtime with an Effuse host (implemented with Effect) that:

- Serves static assets (the built client bundle)
- Handles HTTP API endpoints (e.g. `/api/auth/*`, `/api/contracts/*`, `/api/autopilot/*`)
- Performs SSR for `GET /*` via the same route contract
- Emits consistent telemetry across SSR and API endpoints

### 3.3.1 Backend Targets (Multi-Backend, Cloudflare First)

Effuse should support multiple server backends, but we want to **ship on Cloudflare first** and treat portability as an explicit adapter boundary.

Hard requirements for the host surface:

- Use standard Web APIs (`Request`, `Response`, `Headers`, `URL`) as the primary contract so Cloudflare is the “native” environment.
- Keep backend-specific details behind `EffuseWebHost` adapters so `RouterService` + route tables don’t hardcode a hosting platform.

Initial planned backend: **Cloudflare Workers**, using a **single Worker** fetch handler for SSR + static + API, plus relevant Cloudflare infra:

- **R2** (or Convex file storage) for large blob storage (`BlobStore`) and any large tool/LLM artifacts.
- **KV** for small global caches/flags/pointers where appropriate (not canonical state).
- **Queues** for background or deferred work (e.g. compile jobs, log flushing) when needed.
- **D1** only if/when we need a global relational store (optional).

Post-MVP (optional): add **Durable Objects + DO SQLite** as an execution/workspace plane (cheaper true streaming, strong per-user consistency) while keeping Convex as the product DB and subscription surface.

Additional backend used by the product (not Cloudflare infra):

- **Convex** is the product database and realtime subscription backbone. The browser MUST be able to connect to Convex via WebSockets for live updates (see §3.5.5).

Secondary backend (later, optional): Node/Bun adapter for local dev or alternative deployment, but the plan in this doc should assume Cloudflare’s constraints and primitives.

SSR note: Workers can stream `Response` bodies, but **v1 may buffer HTML for simplicity**. Streaming SSR is an optimization, not a requirement for the migration.
Even when buffering, SSR MUST still respect request abort signals and budgets, and MUST enforce a max HTML byte cap to avoid Worker memory blow-ups.

SSR pipeline:

1. Build `RouteContext.Server` and install request-scoped services (requestId, cookies, auth/session scope).
2. Match route (or return `NotFound`).
3. Run `route.guard` (if present); if it returns a `RouteOutcome`, apply it and stop.
4. Run `route.loader` to produce `RouteOutcome<LoaderData>`.
5. If `Ok`, run `route.head` (optional) and `route.view` to produce `TemplateResult`.
6. `renderToString` to HTML.
7. Apply `RouteOkHints` (headers/cookies/cache/dehydrate/receipts) and produce a response HTML document:
   - server HTML for shell + outlet
   - hydration payload (dehydrate fragments namespaced by routeId)
   - boot script + asset references (and optional dev-only SSR key/hash metadata)

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

- **Loader keying**: each loader run has a stable key derived from `(routeId, pathname, params, search, sessionScopeKey)`.
- **Session scope**: `sessionScopeKey` is a stable string derived from `AuthService` (for example: `anon` | `user:<id>` | `session:<id>`). It MUST NOT include high-churn fields (e.g. expiring tokens, request ids).
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

Remove React state and TanStack Query by standardizing on Effect primitives and a single state story:

- **Canonical app/UI store:** `@effect-atom/atom` (Effect-native; SSR-hydratable; no React bindings in the end state)
- **Local ephemeral component state:** Effuse `StateCell` for tight render loops (or an Atom-backed equivalent later)
- **Server/shared caches:** `MemoMap`, `Cache`, `Request`/`RequestResolver` (service-owned; not per-screen ad hoc)
- **Network:** Effect RPC + `@effect/platform` `HttpClient`

**Rule:** if state drives DOM, it must be observable by Effuse without React (Atom subscription, `Stream`, or `StateCell.changes`).

### 3.5.1 Blueprint/Bootstrap State (Schema-Backed, Exportable)

Autopilot-specific “bootstrap” (Identity/User/Character/Tools/Heartbeat/Memory) should be treated as **durable, typed records** (Effect `Schema`), not a pile of markdown files:

- MVP canonical store: **Convex** (product DB + realtime backbone)
- post-MVP (optional): move selected state into a DO/DO SQLite execution plane and project summaries into Convex for realtime UI
- export/import: a single versioned JSON “Blueprint” format, validated at boundaries
- prompt injection: rendered “context file” view is derived; canonical representation remains structured
- memory visibility: enforce `main_only` vs `all` mechanically (not by convention)

### 3.5.2 Canonical UI Store: `@effect-atom/atom` (No React Bindings)

`@effect-atom/atom` fits Effuse’s constraints: it is Effect-native, supports effectful reads, derived values, and SSR-safe serialization. It should be the **default UI state mechanism** in Effuse apps (and is already in use in `apps/web`).

Hard choices (to prevent “three competing state systems”):

- Use **atoms** for state that is shared across routes/components, needs SSR hydration, or must outlive a single component mount (session/auth, router state, query results, view-models).
- Use **`StateCell`** for state that is purely local and high-frequency (text inputs, caret/selection, transient UI toggles inside a single mounted component) where you want a small, explicit render loop.

End-state binding requirements:

- Effuse MUST provide an Atom runtime/registry as part of `AppRuntime` (an Effect service), so routes, EZ actions, and components can read/write atoms without React.
- SSR/hydration MUST be implemented without `@effect-atom/atom-react`:
  - **Server:** create a request-scoped Atom registry, run `guard`/`loader`/`view`, then serialize only atoms declared via `Atom.serializable(...)` into the route’s `dehydrate` payload.
  - **Client:** hydrate the Atom registry from the payload **before** router boot. In `strict` hydration, this MUST NOT trigger any `DomService.swap` during boot.
- SSR I/O discipline:
  - request-scoped registries MUST NOT spawn unbounded background fibers during SSR (all work is request-scoped and finalizes at end of request)
  - effectful atoms that perform network I/O MUST use request-scoped `HttpClient`/`Fetch` configured from the incoming request URL (do not rely on relative-URL `fetch()` on the server)

Caching boundary (avoid “React Query by accident”):

- `RouterService` owns the cache/dedupe/cancel rules for **route loaders** (per §3.4.1). Atoms MAY reflect the current `RouteRun`, but they MUST NOT introduce a second ad hoc route-data cache with different semantics.
- Non-route “queries” should use Effect `Request`/`Cache`/`MemoMap` in services, and atoms should be the UI-facing projection of those services (so SSR/client parity and receipts stay consistent).

### 3.5.3 Learnings From `typed`: Dedupe + Batching Ergonomics

We reviewed `~/code/typed`. It does **not** use `@effect-atom/atom`, but it heavily uses `@typed/fx/RefSubject` (an “atom-like” primitive) with a few patterns worth copying into the Effuse state story:

- **Batched/atomic updates:** `RefSubject.runUpdates(...)` lets you apply multiple reads/writes as one transition (avoid intermediate states and double renders).
- **Equivalence-based dedupe:** `skipRepeatsWith(eq)` prevents recomputation/re-render when computed values are structurally equal.
- **Context-tag wiring:** `computedFromTag(...)` / `filteredFromTag(...)` keeps router/navigation state explicit via tags, rather than hidden globals.

Effuse state ergonomics (Phase 7) should ensure these capabilities exist for atoms (or as a tiny wrapper layer) so implementers don’t reinvent batching/dedupe per screen.

### 3.5.4 Convex (Effect-First, No React)

Convex remains an allowed backend during the React/TanStack removal, but it MUST be wrapped behind Effect services so:

- UI code never depends on `convex/react` or React Query.
- SSR and client behavior are consistent and testable.
- auth token handling is centralized (WorkOS session or API token) and observable.
- caching/dedupe/cancellation rules are standardized (no “React Query by accident”).

We reviewed `~/code/confect`, `~/code/crest`, and our prior Effevex take (in `~/code/oapreclean/src/effevex/*`) for patterns. We will **not** depend on Confect directly, but we will copy the architecture moves:

- treat Convex as an adapter boundary (Promises -> Effects, nullables -> Option where appropriate)
- use Schema encode/decode at request boundaries (like Confect’s React bindings, but without React)
- provide request-scoped Convex HTTP clients on the server (like Crest’s `ConvexService`)
- optionally write Convex functions as Effect programs (like Crest’s `convex/effect/*` bridge and Effevex’s `query/mutation/action` wrappers)

#### 3.5.4.1 Service Surface (App-Side)

Define a single `ConvexService` used everywhere (routes/loaders/EZ handlers/components/services).

Illustrative shape:

```ts
import type { FunctionArgs, FunctionReference, FunctionReturnType } from "convex/server"
import { Effect, Option, Stream } from "effect"

export type ConvexCallError =
  | { readonly _tag: "Unauthorized" }
  | { readonly _tag: "UdfFailed"; readonly message: string }
  | { readonly _tag: "Transport"; readonly error: unknown }

export interface ConvexService {
  // One-shot calls (HTTP on server; WS or HTTP on client).
  readonly query: <Q extends FunctionReference<"query">>(
    query: Q,
    args: FunctionArgs<Q>,
  ) => Effect.Effect<Awaited<FunctionReturnType<Q>>, ConvexCallError>

  readonly mutation: <M extends FunctionReference<"mutation">>(
    mutation: M,
    args: FunctionArgs<M>,
  ) => Effect.Effect<Awaited<FunctionReturnType<M>>, ConvexCallError>

  readonly action: <A extends FunctionReference<"action">>(
    action: A,
    args: FunctionArgs<A>,
  ) => Effect.Effect<Awaited<FunctionReturnType<A>>, ConvexCallError>

  // Live queries (client only). Useful for realtime UIs; integrates with atoms.
  readonly subscribeQuery: <Q extends FunctionReference<"query">>(
    query: Q,
    args: FunctionArgs<Q>,
  ) => Stream.Stream<Awaited<FunctionReturnType<Q>>, ConvexCallError>

  // Optional: expose connection status in the browser.
  readonly connectionState?: Stream.Stream<unknown, never>
}
```

Typed boundary upgrades (recommended):

- Add optional Schema encoding/decoding helpers so call sites can be domain-typed and avoid `unknown`:
  - `callQuery({ query, argsSchema, returnsSchema })(args)` (Confect-style)
- Centralize `Option` conversions (nullable -> `Option`) at the boundary so app logic is consistent.

#### 3.5.4.2 Server Implementation (Cloudflare Workers)

Server-side Convex calls MUST use `convex/browser` `ConvexHttpClient`:

- `ConvexHttpClient` is **stateful** (auth + queued mutations) so treat it as **request-scoped**.
- Auth token comes from `AuthService` (WorkOS session, API token exchange, etc.) and is applied via `client.setAuth(token)` before the first call.
- Provide a Worker-safe `fetch` into the client (if needed) and enforce timeouts/budgets in the Effect wrapper.

This gives SSR-safe loaders and RPC handlers a consistent Convex access path without React.

#### 3.5.4.3 Client Implementation (No React)

Client-side live queries MUST use `convex/browser` `ConvexClient` (WebSocket), wrapped in Effect. **Realtime is non-negotiable** and we do not proxy subscriptions through the Worker.

- Create one long-lived `ConvexClient` (singleton in the browser runtime Layer).
- Set auth via `client.setAuth(fetchToken)` where `fetchToken` delegates to `AuthService`:
  - must support refresh (`forceRefreshToken`) and return `null` when unauthenticated.
- Wrap `client.onUpdate(...)` into `Stream.async` (unsubscribe in the stream finalizer).
- Wrap `client.query/mutation/action` Promises with `Effect.tryPromise` and map errors to `ConvexCallError`.

#### 3.5.4.4 Caching + Dedupe + Cancellation

Rules:

- Route-level caching is owned by `RouterService` (§3.4.1). Convex calls inside loaders MUST respect loader cancellation.
- For non-route usage, standardize on Effect `Request` + `Cache`:
  - `ConvexQueryRequest` keyed by `(functionPath, encodedArgs, sessionScopeKey)`
  - supports in-flight dedupe and equivalence-based dedupe for derived atoms

No new “query library” is allowed; the combination of `RouterService` rules + Effect cache primitives is the query system.

#### 3.5.4.5 Optional: Convex Functions Written as Effect Programs

If/when we invest in making Convex backend code Effect-native, follow Crest’s lightweight `convex/effect/*` pattern:

- `convex/effect/ctx.ts`: wrap Convex `QueryCtx/MutationCtx/ActionCtx` into Effect-friendly contexts (db/auth/storage/scheduler).
- `convex/effect/functions.ts`: `effectQuery/effectMutation/effectAction` wrappers that run Effect handlers in Convex function definitions.
- `convex/effect/validators.ts`: table-derived document validators for consistent return typing.

Effevex adds two additional ideas that are worth adopting when we do this:

- **Typed errors in the Effect error channel** (mapped to thrown errors only at the Convex boundary).
- **DB/auth/storage wrappers** that eliminate `null` and push normalization to the boundary (e.g. `Option` for nullable returns).

If we want schema-driven args/returns validators derived from Effect `Schema`, adopt Confect’s idea later (compile Schema -> `v.*` validators), but keep it as an internal build step or helper library, not a runtime dependency.

### 3.5.5 Data Residency and Sync (Convex vs Cloudflare)

MVP choice (resolved): **Convex-first**.

For the MVP we intentionally avoid a second per-user persistence plane (DO/DO-SQLite). Convex is the canonical durable system for product *and* chat/execution history, and the Cloudflare Worker is the host + compute enforcement layer.

### 3.5.5.1 MVP Rules (Single Canonical Store)

- **Convex is canonical** for:
  - threads, messages, and **message parts** (chunked streaming deltas)
  - receipts/budgets/tool calls (bounded; large payloads are `BlobRef`s)
  - bootstrap/blueprint state and user profile/ownership/membership
  - any state the UI must observe in realtime (multiplayer, observers, presence)
- The **browser always connects to Convex via WebSockets** for realtime subscriptions (no subscription proxying through the Worker).
- The **Worker is the enforcement point**:
  - validates access to a target thread (owner or valid anon key)
  - runs inference/tools with budgets
  - writes chunked deltas and receipts back into Convex
- Large payload discipline:
  - large content is stored once in blob storage (Cloudflare R2 or Convex file storage)
  - Convex stores only `BlobRef`s + metadata (never inline megabytes)

### 3.5.5.2 MVP Chat Streaming Contract (Convex)

We model “streaming” as realtime Convex updates.

Rules:

- Streaming writes MUST be **chunked** (~250–500ms and/or N chars); never per-token.
- Parts MUST be **idempotent** and retry-safe:
  - unique key `(runId, seq)` (or equivalent)
  - duplicates are safe (upsert/no-op)
- Runs MUST finalize:
  - append a terminal `finish` (with usage) or `error` part
  - mark the assistant message `status` to `final|error|canceled`
- Backpressure/cancel MUST exist:
  - if the client cancels/disconnects or budgets are exceeded, stop writing parts and finalize the run state

Anon continuity (required):

- anon threads MUST be migratable to owned threads on auth (prefer “claim ownership in place” by verifying a secret `anonKey`, rather than copying data).

### 3.5.5.3 Post-MVP (Optional): Dual-Plane Execution + Projection

If/when we want cheaper “true streaming” and stronger per-user consistency, we MAY introduce a Cloudflare execution/workspace plane (DO/DO SQLite) and project normalized state into Convex for subscriptions.

If introduced, mirroring MUST be event-sourced and idempotent:

- execution plane maintains an append-only event log with monotonic `seq` and stable `eventId` (ULID)
- Convex projection uses idempotent upserts keyed by `eventId`
- large payloads remain blobs (`BlobRef`s), not inline Convex fields

### 3.5.6 AI Inference (Effect-Native, `@effect/ai`)

To keep AI inference coherent with the rest of the Effect/Effuse stack (budgets, receipts, schemas, telemetry), we standardize on `@effect/ai` primitives instead of ad hoc provider SDKs.

Core choices:

- **Canonical LLM interface:** `@effect/ai/LanguageModel` (`generateText`, `streamText`, `generateObject` with Effect `Schema`)
- **Canonical tool interface:** `@effect/ai/Tool` + `@effect/ai/Toolkit` (Schema-derived JSON Schema for tool parameters; Effect handlers; standardized toolChoice + concurrency)
- **Canonical error taxonomy:** `@effect/ai/AiError` (request/response/parse/input/output failures)
- **Canonical telemetry:** `@effect/ai/Telemetry` + provider-specific telemetry (for example `@effect/ai-openai` `OpenAiTelemetry`) for GenAI semantic conventions
- **Token accounting + truncation:** `@effect/ai/Tokenizer` (use OpenAI tokenizer when using OpenAI models)

Provider posture:

- Keep using the **current provider** for inference, but route all calls through `LanguageModel` so the rest of the runtime (budgets/receipts/telemetry) is provider-agnostic.
- If the current provider is Cloudflare (Workers AI, Agents, or an AI gateway reachable from the Worker), implement a `LanguageModel` adapter using `@effect/ai` conventions (Response parts, `AiError`, telemetry). Borrow patterns from `@effect/ai-openai` even if we are not using OpenAI.
- If the current provider is OpenAI, prefer `@effect/ai-openai` (Responses API, streaming + tool calling).
- If the current provider is OpenRouter, prefer `@effect/ai-openrouter`.
- Provider-specific “extras” (OpenAI web search / file search / code interpreter) are allowed, but must still produce standard `Response` tool parts so UI + receipts remain consistent.

Current implementation in this repo (as of 2026-02-08):

- Inference runs in the **single `apps/web` Cloudflare Worker host** using `@effect/ai/LanguageModel` with the Cloudflare Workers AI `env.AI` binding (model id: `@cf/openai/gpt-oss-120b`).
- Streaming is implemented as **chunked writes into Convex** (`messageParts`), and the UI streams by subscribing over Convex WebSockets.
- The Worker exposes:
  - `POST /api/autopilot/send` (create run + start background stream)
  - `POST /api/autopilot/cancel` (best-effort cancel + persisted cancelRequested flag)
  - `GET /api/contracts/*` (tool/signature/module contracts for the UI)
- The legacy `/agents/*` Durable Object + Agents SDK transport is removed from `apps/web`.

Legacy implementation (kept for reference / post-MVP):

- `apps/autopilot-worker/src/server.ts` still contains the older DO + Agents SDK transport pattern and `CF_AGENT_*` envelope logic.

Inference placement (MVP, implemented):

- Inference runs in the **single Cloudflare Worker host** (Effect `LanguageModel.streamText`, tool execution as needed, budgets/receipts enforcement as we harden).
- Streaming is **Convex-first**: chunked parts written into Convex; browser subscribes via Convex WS.

Integration requirements (Effuse-side):

- Standardize the streaming/message part shape on `@effect/ai/Response` parts (`text-*`, `tool-*`, `finish`, `error`) and adapt any legacy stream formats into this shape at the boundary.
- WebSockets are the canonical transport for streaming AI parts (no SSE dependency in the end state):
  - MVP: Convex WebSockets (subscriptions over `messageParts`)
  - post-MVP: optional direct Worker/DO WebSockets if/when we add an execution plane
- Tool call resolution must emit **tool call receipts** and renderable UI parts:
  - `Response.ToolCallPart` (toolName + toolCallId + params)
  - `Response.ToolResultPart` (toolName + toolCallId + success/failure + bounded payload/BlobRef)
- Model calls MUST be budgeted and receipted:
  - prompt inputs are BlobRef-backed when large
  - token estimates via `Tokenizer` gate max prompt size
  - `finish` part usage is recorded (input/output tokens, cached tokens, reasoning tokens when available)

Optional (later): use `@effect/ai/Chat.Persistence` with a DO SQLite-backed `BackingPersistence` store to persist chat histories in the user-space plane while still projecting key events into Convex for realtime UI.

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

Tool calling (via `@effect/ai` `Response` parts or provider adapters) produces tool-call/tool-result/error parts; if the UI hides tool parts, users experience “nothing happened”.

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

Minimal UI schema:

- Every tool part renders as `{ status, toolName, toolCallId, summary, details? }`.
- `details` MUST be behind a disclosure affordance; “view full” uses `BlobRef` when payloads are large.
- This schema is the UI projection of `@effect/ai/Response` parts (`tool-call`, `tool-result`, and `error`) so provider adapters can’t drift UI semantics.

Implementation posture:

- Provide a default `ToolPartRenderer` used across the app (one place to enforce truncation, receipt linking, and error visibility).

### 3.7 SSR + Hydration (Effuse-Native)

Effuse must support SSR and a crisp hydration contract. Otherwise, implementers will ship a pile of “works on my machine” hydrators.

Effuse provides three hydration modes (per-route), with a single default:

- `strict` (default): **attach behavior only**. The server renders shell + outlet HTML; the client boot mounts EZ + router + subscriptions without performing an initial outlet swap.
  - Strict mode MUST NOT call `DomService.swap` during boot.
  - Strict mode MUST NOT run an initial `view` render pass during boot.
  - Strict mode MUST NOT re-run the initial `loader` during boot (it relies on SSR output + hydration payload).
- `soft`: **render once on boot**. The client runs `loader` + `view` once and swaps the outlet after hydration; this is safer when SSR markup is incomplete or non-deterministic.
- `client-only`: **no SSR outlet**. The server emits an empty outlet placeholder and the client renders normally; use only when SSR is not worth it.

“Matches SSR output” must be a defined property:

- Strict hydration assumes determinism: given the same `RouteOutcome.Ok.data` and the same render pipeline (`renderToString`), the HTML output is stable. We enforce this via conformance tests, not a runtime DOM diff.
- Optionally (dev/test), the server can embed a `data-effuse-ssr-key` derived from `(routeId, loaderKey, sessionScopeKey, dataHash)` and the client can recompute that key from the hydrated data (no `view` run). On mismatch, fall back to `soft` (debugging aid, not the core model).

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
- **LLM/tooling invariants**: Response part mapping is stable, tool calls always render, and `finish.usage` is recorded + receipted.

These tests should run in CI and be treated as “framework regressions”, not app-level snapshots.

## 4. Replacement Matrix (React/TanStack -> Effect/Effuse)

| React/TanStack capability | Replacement |
|---|---|
| JSX UI | Effuse `html`` + helpers (`effuse-ui` style) |
| Component state (`useState`, `useEffect`) | `StateCell`, `@effect-atom/atom`, Effects + Streams |
| Router (`@tanstack/router`) | `RouterService` (Effuse-owned, Effect-based; History API + route table) |
| Loaders/serverFns (Start) | Route `loader` Effects (SSR + client) |
| SPA navigation (`<Link/>`) | `<a href>` + router interception (built into RouterService/EZ) |
| React Query caching | Effect `Cache` / `Request` / `MemoMap` patterns |
| Error boundaries | Effect error channels + EZ error targets + top-level “render error template” |
| SSR rendering | `renderToString(TemplateResult)` + `EffuseWebHost` |
| Hydration | Effuse boot that attaches behavior without DOM teardown |

## 5. Implementation Roadmap (Detailed, File-Level)

This is the concrete, incremental path from **today’s repo state** to the end state. It is intentionally **file-level** so implementers don’t invent parallel routers/hosts/caches.

### 5.0 Current State (Already Done in This Repo)

This master plan assumes the following is already implemented and is our baseline:

- Effuse core primitives + tests:
  - `packages/effuse/src/template/*` (`html```, `TemplateResult`, `renderToString`)
  - `packages/effuse/src/services/dom-live.ts` (swap + focus restore)
  - `packages/effuse/src/ez/*` (`data-ez` runtime)
  - `packages/effuse/tests/ez-runtime.test.ts`
  - `packages/effuse/tests/render-to-string.test.ts`
- Effuse UI kit exists:
  - `packages/effuse-ui/src/*`
- `apps/web` is Effuse-first end-to-end (no React/TanStack substrate):
  - templates in `apps/web/src/effuse-pages/*`
  - client boot in `apps/web/src/effuse-app/boot.ts` + entry `apps/web/src/effuse-app/client.ts`
  - Worker host in `apps/web/src/effuse-host/worker.ts` + SSR in `apps/web/src/effuse-host/ssr.ts`
  - `apps/web/wrangler.jsonc` points at `apps/web/src/effuse-host/worker.ts`
  - Effect-first chat loop in `apps/web/src/effect/chat.ts` consuming `@effect/ai/Response` parts
  - Atom runtime in `apps/web/src/effect/atoms/appRuntime.ts`

Everything below is “what’s left”.

### Cross-Cutting Verification (Every Phase)

- `cd packages/effuse && npm test`
- `cd apps/web && npm run lint`
- `cd apps/web && npm run build`
- smoke: `cd apps/web && wrangler dev` (or equivalent dev wiring) and exercise `/`, `/login`, `/autopilot`, `/api/auth/session`, `/api/contracts/tools`

### Phase 1: Tighten Effuse Contracts + Conformance Harness

**Goal:** make the framework-level semantics testable so future router/host work can’t regress swaps/hydration/tool rendering.

Work log:
- 2026-02-07: added DomService swap contract tests (`packages/effuse/tests/dom-swap.test.ts`).
- 2026-02-07: added component mount lifecycle tests (`packages/effuse/tests/component-mount.test.ts`).
- 2026-02-07: added conformance skeleton tests for hydration + cancellation semantics (`packages/effuse/tests/conformance-hydration.test.ts`, `packages/effuse/tests/conformance-router.test.ts`).
- 2026-02-07: hardened EZ runtime contracts (mount-once, switch-latest cancellation, bounded failure) with tests (`packages/effuse/src/ez/runtime.ts`, `packages/effuse/tests/ez-runtime.test.ts`).

Add/Change:

- Add DomService swap contract tests:
  - new: `packages/effuse/tests/dom-swap.test.ts` (focus restore, scroll restore, outer/inner/replace semantics)
- Add component mount lifecycle tests:
  - new: `packages/effuse/tests/component-mount.test.ts` (render loop, subscriptions, finalizers)
- Add a conformance test skeleton (even if it’s initially small):
  - new: `packages/effuse/tests/conformance-hydration.test.ts`
  - new: `packages/effuse/tests/conformance-router.test.ts`

DoD:

- Conformance tests can assert “strict hydration does not swap” by instrumenting `DomService.swap`.
- We have a place to put the future “shell/outlet invariant” tests.

### Phase 2: Implement the Route Contract in Code (Not Just Docs)

**Goal:** make the `Route` / `RouteOutcome` contract real and importable on server + client.

Work log:
- 2026-02-07: implemented route contract types + `runRoute` normalizer in `packages/effuse/src/app/*` and exported from `packages/effuse/src/index.ts`.
- 2026-02-07: added an initial `apps/web` Effuse route table in `apps/web/src/effuse-app/routes.ts` (views reuse existing `apps/web/src/effuse-pages/*` templates).

Add/Change (library):

- new: `packages/effuse/src/app/route.ts` (types: `Route`, `RouteContext`, `RouteOutcome`, `RouteOkHints`, `CachePolicy`)
- new: `packages/effuse/src/app/run.ts` (normalize a route execution into an internal `RouteRun` shape)
- change: `packages/effuse/src/index.ts` export the app/route surface

Add/Change (apps/web):

- new: `apps/web/src/effuse-app/routes.ts` re-exporting the current page templates as `Route` entries:
  - reuse existing templates in `apps/web/src/effuse-pages/*` for `view` (and `head` where relevant)

DoD:

- Both SSR code and client router code import the same route table and can execute `guard/loader/view/head` without branching on env beyond `RouteContext`.

### Phase 3: Effect-First Auth + Convex + AI Services (Remove React Providers as a Dependency)

**Goal:** make auth/session and Convex access available everywhere via Effect services, so we can delete React providers later without losing behavior.

Work log:
- 2026-02-07: added `RequestContextService` tag + helpers in `apps/web/src/effect/requestContext.ts` and wired it into the app Layer (`apps/web/src/effect/layer.ts`).
- 2026-02-07: implemented `AuthService` in `apps/web/src/effect/auth.ts` with a client cache and SSR-only WorkOS AuthKit parsing; added `GET /api/auth/session` in `apps/web/src/routes/api.auth.session.tsx`.
- 2026-02-07: implemented `ConvexService` in `apps/web/src/effect/convex.ts` (SSR uses `ConvexHttpClient` request-scoped via `FiberRef`; client uses `ConvexClient` (WS) + `Stream` wrapper for `onUpdate`).
- 2026-02-07: refactored `apps/web/src/effect/chat.ts` to remove `@ai-sdk/react` and stream directly over the Agents WebSocket protocol.
- 2026-02-07: added Convex Effect wrapper helpers under `apps/web/convex/effect/*` and migrated `apps/web/convex/myFunctions.ts` to the wrappers.
- 2026-02-07: migrated `apps/autopilot-worker` chat DO to `@effect/ai` (`LanguageModel` + toolkit + wire-level streaming on `@effect/ai/Response` parts), and updated `apps/web` chat client to consume those parts directly (removed `ai` + `@cloudflare/ai-chat` types; new `apps/web/src/effect/chatProtocol.ts`).

Add/Change (apps/web Effect runtime):

- new: `apps/web/src/effect/auth.ts` (`AuthService`):
  - `getSession()` (server: parse cookie; client: cached atom or RPC)
  - `sessionScopeKey()` (`anon` | `user:<id>` | `session:<id>`)
  - `getAccessToken({ forceRefreshToken })` (used by Convex and any authenticated HTTP)
- new: `apps/web/src/effect/convex.ts` (`ConvexService`):
  - server impl wraps `ConvexHttpClient` (request-scoped, `setAuth(token)` per request)
  - client impl wraps `ConvexClient` + `onUpdate` into `Stream` subscriptions
- change: `apps/web/src/effect/layer.ts` include `AuthServiceLive` + `ConvexServiceLive*`
- change: `apps/web/src/effect/config.ts` becomes the single source of `convexUrl` and any Convex auth config

Add/Change (apps/web Convex backend code, REQUIRED standard):

- new: `apps/web/convex/effect/ctx.ts` (Effect-friendly `QueryCtx/MutationCtx/ActionCtx` tags)
- new: `apps/web/convex/effect/auth.ts` (nullable identity -> `Option`)
- new: `apps/web/convex/effect/storage.ts` (storage APIs as Effects)
- new: `apps/web/convex/effect/scheduler.ts` (scheduler APIs as Effects)
- new: `apps/web/convex/effect/functions.ts` (`effectQuery/effectMutation/effectAction` wrappers)
- new: `apps/web/convex/effect/validators.ts` (schema-derived doc validators, like Crest)
- new: `apps/web/convex/effect/tryPromise.ts` (central Promise -> Effect error mapping)
- change: `apps/web/convex/myFunctions.ts` migrate to the wrappers as a sanity check (keeps behavior the same, but enforces the pattern)

Add/Change (agent plane AI inference wiring, `@effect/ai`):

- new: `apps/autopilot-worker/src/effect/ai/languageModel.ts` (Layer providing `@effect/ai/LanguageModel` via Cloudflare Workers AI; map provider errors to `AiError`)
- new: `apps/autopilot-worker/src/effect/ai/toolkit.ts` (maps the tool registry/contracts into `@effect/ai/Tool` + `Toolkit` handlers)
- new: `apps/autopilot-worker/src/effect/ai/streaming.ts` (encode/decode `@effect/ai/Response` parts onto the WebSocket stream envelope)
- change: `apps/autopilot-worker/src/server.ts` migrate `onChatMessage` from Vercel `ai` (`streamText`) to `@effect/ai` (`LanguageModel.streamText`) while preserving the WebSocket protocol and receipts

Add/Change (apps/web client, end-state cleanup):

- change: `apps/web/src/effect/chat.ts` stop bridging WebSocket -> SSE for the Vercel `ai` SDK; instead consume `@effect/ai/Response` parts directly and build `ChatSnapshot` from those parts

Refactor targets (transitional, while TanStack still hosts):

- `apps/web/src/routes/__root.tsx`: plan to remove `ConvexProviderWithAuth` and `@effect-atom/atom-react` over time; until then, ensure `beforeLoad` and `fetchWorkosAuth` are thin shims that call `AuthService`.
- `apps/web/src/router.tsx`: stop constructing `ConvexReactClient`/`ConvexQueryClient` for “app logic”; those become host-only legacy until deleted.

DoD:

- Any Effect program can call Convex without React (`ConvexService`), can get a stable auth scope (`AuthService.sessionScopeKey`), and can call the LLM through `LanguageModel` (provider-agnostic, with receipts + token usage).

### Phase 4: Implement `RouterService` (Effuse-Owned Navigation + Loader Pipeline)

**Goal:** make navigation + loader caching/dedupe/cancel rules framework-owned, not page-owned.

Work log:
- 2026-02-07: implemented Effuse-owned router surface (`History`, loader keying, in-flight dedupe, cache/SWR skeleton, switch-latest cancellation) in `packages/effuse/src/router/*` and exported from `packages/effuse/src/index.ts`.
- 2026-02-07: added RouterService contract tests for dedupe + cancellation + shared in-flight semantics (`packages/effuse/tests/router-service.test.ts`).
- 2026-02-07: added `apps/web` boot wiring for EZ runtime + RouterService (strict hydration: no initial swap) in `apps/web/src/effuse-app/boot.ts`.

Add/Change (library):

- new: `packages/effuse/src/router/*`:
  - History adapter (push/replace/popstate)
  - loader pipeline with standardized keying + cache + in-flight dedupe
  - apply rules for `RouteOutcome` (redirect/not-found/fail/ok + hints)
  - shell/outlet swap default behavior
- change: export router surface from `packages/effuse/src/index.ts`

Add/Change (apps/web integration):

- new: `apps/web/src/effuse-app/boot.ts`:
  - mounts EZ runtime + RouterService on `[data-effuse-shell]`
  - binds outlet swaps to `[data-effuse-outlet]`
  - strict hydration by default (no initial swap)

DoD:

- `RouterService.navigate()` drives outlet swaps and loader execution.
- In-flight loader dedupe + cancellation rules match §3.4.1 and are covered by tests.

### Phase 5: Stand Up `EffuseWebHost` on Cloudflare Workers (Parallel Host)

**Goal:** add a real Worker host that can serve the app without TanStack Start, while keeping the current host available until cutover.

Work log:
- 2026-02-07: added parallel Effuse Worker host entry + SSR + API mounts in `apps/web/src/effuse-host/*` and config `apps/web/wrangler.effuse.jsonc`.
- 2026-02-07: added a dedicated Effuse-only client bootstrap entry (`apps/web/src/effuse-app/client.ts`) and build config (`apps/web/vite.effuse.config.ts`) to produce stable `effuse-client.{js,css}` assets.
- 2026-02-08: implemented **Convex-first Autopilot** execution plane in `apps/web` (MVP):
  - removed legacy Durable Object + Agents SDK transport (`/agents/*`) from `apps/web`
  - removed DO bindings from Wrangler configs
  - added Convex schema + functions for threads/messages/parts/runs/blueprints/receipts
  - added Worker endpoints: `POST /api/autopilot/send`, `POST /api/autopilot/cancel`, `GET /api/contracts/*`
  - refactored client `ChatService` to subscribe to Convex and call the Worker HTTP endpoints (no browser WebSocket transport)
  - removed the `agents` dependency from `apps/web`

Add/Change (apps/web host):

- new: `apps/web/src/effuse-host/worker.ts` (Cloudflare Worker fetch handler)
- new: `apps/web/src/effuse-host/ssr.ts` (SSR entry: request -> `RouteRun` -> HTML + headers/cookies + dehydrate payload)
- new: `apps/web/src/effuse-host/assets.ts` (static asset serving strategy; stable asset names in v1, manifest integration later)
- new: `apps/web/src/effuse-host/auth.ts` mounts existing WorkOS endpoints using:
  - `apps/web/src/auth/workosAuth.ts` (already exists)
- new: `apps/web/src/effuse-host/autopilot.ts` (Convex-first run creation + chunked streaming -> Convex)
- new: `apps/web/src/effuse-host/contracts.ts` (serves tool/signature/module contracts to the UI)
- new: `apps/web/src/effuse-app/client.ts` (Effuse-only client entry)
- new: `apps/web/vite.effuse.config.ts` (builds stable `effuse-client.{js,css}`)

Add/Change (Convex-first Autopilot storage + APIs):

- change: `apps/web/convex/schema.ts` now includes canonical tables for MVP chat + blueprint state:
  - `threads`, `messages`, `messageParts`, `runs`, `blueprints`, `receipts`
- new: `apps/web/convex/autopilot/*`:
  - thread creation + anon-to-owned claim (`threads.ts`)
  - message snapshots + run lifecycle + chunked part append (`messages.ts`)
  - blueprint get/set/reset (`blueprint.ts`)
  - reset thread helper (`reset.ts`)
  - access control helper (`access.ts`)

Parallel deploy options:

- deploy the same single-worker host under a second Worker name (recommended) so we can deploy without risk:
  - new: `apps/web/wrangler.effuse.jsonc` with a different Worker `name` and `main`
- or add a path-prefix mount in the existing Worker for preview (less clean)

DoD:

- `wrangler dev` can serve SSR HTML + boot the client router without TanStack Start.
- `/api/auth/*`, `/api/autopilot/*`, and `/api/contracts/*` work and are request-scoped where required.

### Phase 6: Cut Over Production Host (Remove TanStack Start Server Runtime)

**Goal:** production traffic is served by `EffuseWebHost`; TanStack Start is removed from deploy artifacts.

Work log:
- 2026-02-07: cut over `apps/web/wrangler.jsonc` to `apps/web/src/effuse-host/worker.ts`, added Wrangler module aliasing for local workspace packages, and updated `apps/web/package.json` build/deploy scripts to build `dist/effuse-client` (Effuse client bootstrap) before `wrangler deploy` (legacy TanStack build kept as `npm run build:tanstack`; deploy script clears `.wrangler/deploy/config.json` to avoid TanStack config redirection). Also added `workers-ai-provider` + `@openagentsinc/dse` to `apps/web` dependencies so the single-worker bundle does not rely on `apps/autopilot-worker` installs.

Change:

- `apps/web/wrangler.jsonc`:
  - set `main` to `apps/web/src/effuse-host/worker.ts`
  - ensure assets binding/serving is correct
- `apps/web/vite.config.ts`:
  - remove `@tanstack/react-start/plugin/vite`
  - keep a client build for the Effuse boot bundle
- `apps/web/package.json`:
  - update `build`/`deploy` to build client assets + worker bundle, then `wrangler deploy`

DoD:

- Deployed Worker serves openagents.com without TanStack Start.

### Phase 7: Remove React + TanStack + React Query (No TSX Anywhere)

**Goal:** no runtime React, no TanStack Router/Start, no Convex React bindings.

Work log:
- 2026-02-07: removed all React/TanStack TSX substrate from `apps/web` (deleted `src/routes/*`, `src/components/*`, `src/router.tsx`, `src/start.ts`, `src/routeTree.gen.ts`, `src/useAuthFromWorkOS.tsx`), replaced WorkOS `/callback` handler with native `@workos/authkit-session`, and moved “controller” logic into Effuse boot + non-React route controllers (`src/effuse-app/controllers/*`). Added Atom registry hydration from SSR dehydrate payload, ported PostHog loading to a non-React boot helper, and updated `apps/web` scripts + ESLint config to typecheck/lint/build without React/TanStack.

Remove/Replace (apps/web):

- delete React/TanStack host files:
  - `apps/web/src/router.tsx` (removed)
  - `apps/web/src/start.ts` (removed)
  - `apps/web/src/routeTree.gen.ts` (removed)
  - `apps/web/src/routes/*` (removed)
  - `apps/web/src/components/*` (removed)
- remove dependencies from `apps/web/package.json`:
  - `react`, `react-dom`
  - `@tanstack/react-start`, `@tanstack/react-router`, `@tanstack/react-router-ssr-query`, `@tanstack/react-query`
  - `convex/react`, `@convex-dev/react-query`
  - `@ai-sdk/react`
  - `@effect-atom/atom-react`
  - `@workos/authkit-tanstack-react-start` (replaced by `@workos/authkit-session`)
- ensure a single client entrypoint:
  - `apps/web/src/effuse-app/client.ts` (boots Effuse app; built by `apps/web/vite.effuse.config.ts`)

DoD:

- The app runs with Effuse + Effect only; builds and deploys on Cloudflare Workers.
- Use `apps/web/docs/REACT-USAGE-REPORT.md` as the checklist: update it during the migration, and ensure every item is eliminated before calling Phase 7 “done”.

### Phase 8: Hardening (Performance, Ergonomics, Regression Gates)

**Goal:** “ship quality” once the substrate is gone.

Work log:
- 2026-02-07: implemented `StateCell` ergonomics in `@openagentsinc/effuse` (`computed`, `filtered`, `withEq`, `batch`) with correctness-focused contract tests (`tests/state-cell.test.ts` + render coalescing in `tests/component-mount.test.ts`). Expanded conformance suite to enforce shell/outlet invariants and strict router boot no-swap behavior (`tests/conformance-shell-outlet.test.ts`).
- 2026-02-07: implemented framework-level Tool Part rendering + BlobRef bounding helpers in `@openagentsinc/effuse` (`boundText`, `renderToolPart`) with conformance tests (`tests/conformance-tool-parts.test.ts`). Updated `apps/web` Autopilot chat to render tool parts via `renderToolPart` and added a client-side `UiBlobStore` + `effuse.blob.view` EZ action for “view full” payload swaps.
- 2026-02-07: hardening: added SSR request abort handling + max HTML byte cap (`apps/web/src/effuse-host/ssr.ts`), plus prompt budgeting via `@effect/ai/Tokenizer` and model-call receipt recording (with `finish.usage`) in the Chat DO (`apps/autopilot-worker/src/server.ts`, `apps/autopilot-worker/src/effect/ai/receipts.ts`).
- 2026-02-07: hardening: added tool-call execution receipts (toolName/toolCallId, params/output blobs, timing, correlation) recorded into DO SQLite + exposed via `GET /agents/chat/:id/ai/tool-receipts`, and added an integration test that stubs Workers AI to force a tool call and asserts (1) Blueprint state mutation occurs and (2) both model receipts (`finish.usage`) and tool receipts are present (`apps/autopilot-worker/src/server.ts`, `apps/autopilot-worker/src/effect/ai/receipts.ts`, `apps/autopilot-worker/tests/index.test.ts`).
- 2026-02-07: hardening: added RouterService caching contract tests for `cache-first` (ttl unset + ttl expiry) and `stale-while-revalidate` (render stale immediately, refresh in background, and ensure refresh does not apply after navigating away) (`packages/effuse/tests/router-service.test.ts`).
- 2026-02-07: hardening: escaped SSR dehydrate JSON before embedding in `<script type="application/json">` to prevent `</script>` breakouts; added `escapeJsonForHtmlScript` helper + tests and wired it into `apps/web` SSR (`packages/effuse/src/template/escape.ts`, `packages/effuse/tests/escape-json.test.ts`, `apps/web/src/effuse-host/ssr.ts`).
- 2026-02-07: hardening: applied `RouteOkHints.cache` to SSR responses via `Cache-Control` (default `no-store`, `private` for cacheable routes, and never cache HTML when `Set-Cookie` is emitted) using a shared helper (`packages/effuse/src/app/cache-control.ts`, `packages/effuse/tests/cache-control.test.ts`, `apps/web/src/effuse-host/ssr.ts`).
- 2026-02-07: hardening: added RouterService contract test that `prefetch()` does not mutate history or swap DOM and that it warms the cache for `cache-first` routes (so subsequent `navigate()` does not re-run the loader) (`packages/effuse/tests/router-service.test.ts`).
- 2026-02-07: hardening: ensured SSR non-OK responses are never cached (`Cache-Control: no-store` for redirect/not-found/fail/aborted/too-large responses) and added loader-key stability tests (params/search ordering + session scope) (`apps/web/src/effuse-host/ssr.ts`, `packages/effuse/tests/loader-key.test.ts`).
- 2026-02-07: hardening: added `RouterService.stop` disposal API (removes click/popstate listeners, invalidates in-flight applies, resets state) with contract tests (prevents navigation after stop) (`packages/effuse/src/router/router.ts`, `packages/effuse/tests/router-service.test.ts`).
- 2026-02-07: hardening: tightened strict hydration conformance by asserting `RouterService.start` does not execute loaders on boot (not just “no swap”) (`packages/effuse/tests/conformance-shell-outlet.test.ts`).
- 2026-02-07: hardening: expanded `DomService.swap` focus/caret contract tests to cover textarea selection restoration and outer-swap focus restoration (caret preservation for Blueprint-like editing flows) (`packages/effuse/tests/dom-swap.test.ts`).
- 2026-02-07: hardening: expanded `DomService.swap` contract tests to cover scroll restoration for `data-scroll-id` elements on `outer` swaps (prevents scroll jumps when swapping whole panels/outlets) (`packages/effuse/tests/dom-swap.test.ts`).
- 2026-02-07: hardening: added `runRoute` contract tests covering guard short-circuiting, stage attribution (`guard`/`loader`/`head`/`view`) on defects, and hydration/navigation defaults/overrides (`packages/effuse/tests/run-route.test.ts`).
- 2026-02-07: hardening: ensured SSR route meta tags include `data-effuse-meta="1"` so RouterService head management can reliably clear/replace them on client navigations (prevents duplicate stale meta tags after the first SPA navigation) (`apps/web/src/effuse-host/ssr.ts`).
- 2026-02-07: hardening: tightened RouterService head semantics to always clear router-managed meta tags on navigation (even when the next route has no `head`), and added contract tests to prevent meta tag duplication/staleness across navigations (`packages/effuse/src/router/router.ts`, `packages/effuse/tests/router-head.test.ts`).
- 2026-02-07: hardening: added RouterService “prefetch intent” behavior (`data-router-prefetch` triggers prefetch on hover/focus) with contract tests ensuring no DOM swap occurs during prefetch (`packages/effuse/src/router/router.ts`, `packages/effuse/tests/router-service.test.ts`).
- 2026-02-07: hardening: CI gate wiring is optional; workflows were intentionally removed from this repo. See §9.6 for the recommended gates to wire into CI.
- 2026-02-07: hardening: added RouterService link interception contract tests (same-origin click interception, modifier-key bypass, cross-origin bypass) to prevent regressions in SPA navigation semantics (`packages/effuse/tests/router-link-interception.test.ts`).
- 2026-02-07: hardening: implemented soft/client-only hydration semantics for `RouterService.start` (per-route initial navigation apply), and added conformance tests for hydration modes + strict “matched route” boot (no loader/view on boot) (`packages/effuse/src/router/router.ts`, `packages/effuse/tests/conformance-hydration-modes.test.ts`, `packages/effuse/tests/conformance-shell-outlet.test.ts`).
- 2026-02-07: hardening: added SSR determinism conformance test ensuring `runRoute` + `renderToString` is stable in a node environment (no DOM) for fixed inputs (`packages/effuse/tests/conformance-ssr-determinism.test.ts`).
- 2026-02-07: hardening: stabilized `apps/autopilot-worker` tests by disabling Wrangler `remoteBindings` in the Workers Vitest pool (we stub Workers AI in tests; avoids remote proxy session flake + potential usage charges) (`apps/autopilot-worker/vitest.config.ts`, `apps/autopilot-worker/tests/index.test.ts`).
- 2026-02-07: hardening: strengthened AI receipt BlobRef discipline by asserting model/tool receipts always include prompt/input/output blob refs (and that huge user inputs are truncated before prompt blob serialization) (`apps/autopilot-worker/tests/index.test.ts`).
- 2026-02-07: hardening: added a wire-protocol regression gate: the chat WebSocket stream must emit valid `@effect/ai/Response` parts (`tool-call`/`tool-result`/`finish`) and MUST NOT forward reasoning parts (`reasoning-*`) (`apps/autopilot-worker/tests/index.test.ts`).
- 2026-02-07: hardening: added RouterService outcome contract tests for redirect handling (replace semantics + loop cutoff), not-found rendering, and loader fail rendering (view not executed) (`packages/effuse/tests/router-outcomes.test.ts`).
- 2026-02-07: hardening: improved default Router error UI so non-`Error` failures (notably `RouterError`) render a useful message instead of `[object Object]` (`packages/effuse/src/router/router.ts`).

Add/Change:

- implement Effuse state ergonomics from `packages/effuse/docs/ROADMAP.md`:
  - `computed`, `eq` dedupe, `batch`
  - tests for coalescing + dedupe
- expand conformance suite (CI gates):
  - strict hydration no-swap invariant
  - shell/outlet swap invariant
  - cancellation + dedupe correctness
  - tool part rendering + BlobRef discipline
  - LLM/tooling: `@effect/ai/Response` part mapping is stable, tool calls always render, and `finish.usage` is recorded/receipted

DoD:

- CI fails on hydration/router regressions; UX is stable (focus/caret preserved, minimal flicker).

### Phase 9: Build Effuse Test Runner (Browser + Visual Execution UI)

**Goal:** Playwright-class browser/integration tests that are **Effect-first** and **watchable** (headed browser + live flow graph viewer), plus a no-browser visual mode that still renders Effect execution.

Work log:

- 2026-02-07: added `packages/effuse-test` v1 runner (Bun + `@effect/cli`), Chromium control via CDP, live viewer (WS + flow graph), and failure artifacts (screenshot + HTML snapshot + `events.jsonl`).
- 2026-02-07: wired `apps/web` `npm run test:e2e` to call the Effuse Test Runner.
- 2026-02-07: added a dev/test-only swap counter in `DomService.swap` (`globalThis.__effuseSwapCount`) to assert strict hydration “no swap” in real-browser E2E.
- 2026-02-07: added a no-browser runner mode (when selected tests do not have the `browser` tag) and added no-browser HTTP smoke tests (`apps-web.http.*`) plus a browser history back/forward gate (`apps-web.navigation.back-forward`).

Add/Change:

- new: `packages/effuse-test/*`
  - CLI: `packages/effuse-test/src/cli.ts`
  - runner core: `packages/effuse-test/src/runner/*`
  - browser controller: `packages/effuse-test/src/browser/*`
  - viewer: `packages/effuse-test/src/viewer/*`
- change: `apps/web/package.json` add `test:e2e` script that runs the runner.
- change: `packages/effuse/src/services/dom-live.ts` increment `__effuseSwapCount` when set by an E2E harness.

DoD:

- `cd packages/effuse-test && bun run src/cli.ts run --project ../../apps/web --headless` passes locally.
- `--watch` starts the viewer and streams spans/events live during a run.
- on failure, artifacts include `failure.png`, `failure.html`, and `events.jsonl`.

## 6. Open Decisions / Questions

These must be decided explicitly to finish the React/TanStack removal:

- **Hosting target (resolved):** single Cloudflare Worker (not Pages Functions). Portability remains an explicit adapter boundary for an optional Node/Bun host.
- **Auth integration:** how WorkOS AuthKit middleware maps into the `EffuseWebHost` request pipeline (cookie/session parsing, redirect flows, CSRF).
- **Autopilot unauthenticated mode (resolved):** `/autopilot` (and anything it depends on, notably Convex WebSocket subscriptions and the Worker inference endpoints) MUST work for unauthed users. Auth is used to unlock user identity and ownership, but the core chat experience must not require a WorkOS session.
- **Convex usage (resolved):** browser connects directly to Convex via WebSockets (`ConvexClient`) for realtime subscriptions; Convex MUST still be accessed through the Effect-first `ConvexService` (no `convex/react`, no React Query).
- **Workspace plane (resolved for MVP):** Convex is canonical for user-space/chat state and receipts. A DO/DO SQLite execution plane is deferred to post-MVP; if introduced, it must follow the projection/mirroring rules in §3.5.5.3.
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
  - `APPS-WEB-FULL-EFFUSE-ROADMAP.md` (deprecated; historical record)
  - `apps/web/docs/REACT-USAGE-REPORT.md` (inventory + checklist for Phase 7)
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

## 8. Test Strategy

Effuse replaces *framework-owned correctness guarantees* (React reconciliation, TanStack data consistency) with **explicit contracts** enforced by:

- a conformance suite (no-swap strict hydration, shell/outlet invariants, router cache/dedupe/cancel semantics)
- Worker/DO integration tests (SSR, API surfaces, DO SQLite invariants, Convex replication)
- browser E2E tests (what users actually experience)

Effect is the test harness and DI system: tests run Effects with Layers to inject fakes and deterministic services. “Newing up” SDK clients directly in tests is treated as a smell.

### 8.1 Test Pyramid

Levels and what they cover:

- **L0: Pure unit tests (no DOM, no Worker)**
  - route/run normalization (`runRoute`)
  - loader key computation + stability
  - cache-control derivation
  - HTML/JSON escaping helpers
  - receipt hashing / BlobRef bounding helpers
- **L1: DOM contract tests (happy-dom)**
  - `DomService.swap` focus/caret/scroll invariants
  - EZ parsing/binding + switch-latest cancellation
  - component mount lifecycle (finalizers, render coalescing)
- **L2: Router conformance tests (happy-dom)**
  - strict hydration boot: **no swap / no loader / no view**
  - loader dedupe + switch-latest cancellation
  - cache-first + SWR semantics
  - outlet-only swap invariant; shell node identity stable
  - head/meta management (no duplication; replace router-managed tags)

> L2 and above are treated as **conformance gates**: any framework change that violates L2 MUST fail CI.

- **L3: Worker integration tests (Workers runtime)**
  - `fetch` handler SSR/static/API behavior in-process
  - abort/budgets/max HTML caps enforced
  - `RouteOkHints` applied consistently (headers/cookies/cache-control)
  - SSR dehydrate payload shape + escaping
- **L4: Durable Object integration tests (DO + DO SQLite)**
  - DO SQLite schema invariants
  - append-only event logs (seq monotonic, idempotent eventId)
  - Convex projection adapter semantics (idempotent upsert, BlobRef discipline)
  - `ctx.waitUntil` replication is best-effort and non-blocking
- **L5: Browser E2E tests (real browser, Effuse Test Runner)**
  - SSR -> strict hydration “attach only” (no initial swap)
  - SPA navigation + back/forward (History)
  - focus/caret preservation across swaps (Blueprint-like flows)
  - tool part rendering (tool-error always visible; “view full” uses BlobRef)
  - websocket chat streaming (Response parts)
  - Convex realtime rendering (when feasible in local test env)
- **L6: Production smoke tests (optional / nightly)**
  - deployed Worker endpoints sanity checks (non-blocking, best-effort)
  - not a merge gate, but a “we didn’t break prod” early warning

### 8.2 Effect Test Harness Primitives (Required)

All suites should share a small set of test harness primitives, built as Effect services and composed with Layers.

Required harness services/patterns:

- `TestClock` (deterministic time + timeouts)
- `TestRandom` (stable ids; avoid `Math.random()` in tests)
- `TestTelemetrySink` (capture spans/events by `requestId` for assertions)
- `TestBlobStore` (supports `BlobRef` put/get; asserts bounding/truncation rules)
- `TestLanguageModel` (scripted `@effect/ai/Response` parts; deterministic tool-call flows)
- `TestConvexService`
  - stubbed request/response for query/mutation/action
  - stream-based live query events (push updates into `Stream`)
- `TestAuthService` (explicit `sessionScopeKey` variations: `anon` vs `user:<id>`)

Hard rule:

- Tests SHOULD NOT directly instantiate provider SDK clients (WorkOS/Convex/AI) except inside adapter modules under test.
- Tests MUST prefer `Layer.provide(...)` of fakes rather than ad hoc mocking.

Suggested harness file layout (v1):

```txt
packages/effuse/tests/harness/
  dom.ts
  telemetry.ts

apps/web/tests/harness/
  workerEnv.ts
  testRuntime.ts
  telemetry.ts
  blobStore.ts
  auth.ts
  convex.ts
  ai.ts

apps/autopilot-worker/tests/harness/
  testRuntime.ts
  telemetry.ts
  blobStore.ts
  ai.ts
```

### 8.3 Determinism and Replayability Tests (Required)

The following properties are core to “replayable receipts” and must be tested explicitly:

- **Loader key determinism**
  - same URL with reordered query params yields same key
  - params/search ordering normalized consistently across server/client
  - `sessionScopeKey` is stable and low-churn (never includes access tokens)
- **SSR determinism**
  - `runRoute` + `renderToString` output is stable for fixed inputs
  - SSR dehydrate payload is stable and HTML-safe (no `</script>` breakouts)
- **Receipt discipline**
  - tool-call/tool-result correlation by `toolCallId`
  - model receipts include `finish.usage`
  - large inputs/outputs never inline in receipts; replaced with `BlobRef`
- **Budget/abort semantics**
  - SSR respects `AbortSignal` and returns a deterministic abort response
  - max HTML byte cap enforced (prevents Worker memory blowups)
  - chat streaming cancellation yields visible UI state (finish/error part) + a receipt “reason”

### 8.4 How To Run (Local + CI)

Local (fast):

- `cd packages/effuse && npm test`
- `cd apps/autopilot-worker && npm run typecheck && npm test`
- `cd apps/web && npm run lint && npm run build`

Local (dev + manual smoke):

- `cd apps/web && npm run dev`
  - opens Worker at `http://localhost:3000`

CI gates (required):

- run all L0-L2 suites for `packages/effuse`
- run `apps/web` lint/build
- run `apps/autopilot-worker` typecheck/tests

As we add L3-L5, CI should expand to include:

- Worker integration suites (L3/L4) in a Workers runtime pool
- Effuse Test Runner browser E2E (L5) against a locally started Worker

## 9. Test Suites (Concrete)

This section enumerates the concrete suites we maintain, what they assert, and how they run.

### 9.1 `packages/effuse`: Contract + Conformance (Unit/DOM/Router)

These tests are the framework’s “non-negotiable” gates. They should be fast and run on every change.

- DOM swap contract tests
  - `packages/effuse/tests/dom-swap.test.ts`
  - asserts: focus restoration (input/textarea), selection/caret restoration, scroll restoration for `data-scroll-id`, inner/outer/replace invariants
- EZ runtime contract tests
  - `packages/effuse/tests/ez-runtime.test.ts`
  - asserts: delegated mount-once, param extraction, switch-latest cancellation (2nd action cancels 1st), bounded error behavior
- RouterService contract tests
  - `packages/effuse/tests/router-service.test.ts`
  - `packages/effuse/tests/router-link-interception.test.ts`
  - `packages/effuse/tests/router-outcomes.test.ts`
  - `packages/effuse/tests/router-head.test.ts`
  - asserts: loader keying, in-flight dedupe, switch-latest apply semantics, prefetch warms cache (no DOM/history mutation), redirects (loop cutoff + replace), not-found/fail behavior, head/meta tag management without duplication
- Hydration conformance suite
  - `packages/effuse/tests/conformance-hydration.test.ts`
  - `packages/effuse/tests/conformance-hydration-modes.test.ts`
  - `packages/effuse/tests/conformance-shell-outlet.test.ts`
  - asserts:
    - strict boot: **no `DomService.swap`**, **no loader**, **no view**
    - soft/client-only boot: allowed initial apply semantics
    - outlet-only swap invariant by default
    - shell node identity stable across navigations
- Security + correctness helpers
  - `packages/effuse/tests/escape-json.test.ts`
  - `packages/effuse/tests/cache-control.test.ts`
  - `packages/effuse/tests/loader-key.test.ts`
  - `packages/effuse/tests/run-route.test.ts`
  - `packages/effuse/tests/conformance-ssr-determinism.test.ts`
  - asserts: HTML-safe JSON escaping, cache-control rules (no-store when `Set-Cookie`, never cache non-OK), guard short-circuit stage attribution, SSR determinism for fixed inputs

### 9.2 `apps/web`: Worker Host Integration (L3)

Goal: verify the **single Worker** host in-process in a Workers runtime (no browser).

Implemented suites (2026-02-08):

- `apps/web/tests/worker/ssr.test.ts`
  - SSR respects abort signal
  - max HTML byte cap enforced
  - `RouteOkHints` applied (headers/cookies/cache-control)
  - SSR dehydrate payload is namespaced by routeId and HTML-safe
- `apps/web/tests/worker/auth.test.ts`
  - `GET /api/auth/session` ok without external network
  - WorkOS refresh `Set-Cookie` header is persisted (when stubbing refresh path)
  - magic code endpoints are stubbed (no real WorkOS network)
- `apps/web/tests/worker/assets.test.ts`
  - `ASSETS` binding serves `/effuse-client.css` + `/effuse-client.js`
  - asset requests never fall through to SSR
- `apps/web/tests/worker/chat-streaming-convex.test.ts` (2026-02-08)
  - Worker starts a run and writes `messageParts` into Convex in **chunked** batches (no per-token writes)
  - terminal behavior: `finish.usage` is written; canceled runs finalize predictably

Harness:

- runs under a Workers runtime pool (`@cloudflare/vitest-pool-workers`) via `apps/web/vitest.config.ts` + `apps/web/wrangler.jsonc`
- configured with `singleWorker: true` to avoid flaky isolated-runtime startup on localhost module fallback ports
- uses `cloudflare:test` `env` bindings (`ASSETS`, `AI`) from Wrangler config (no DO namespaces required for MVP)
- external services are stubbed/blocked by default in tests (WorkOS paths mocked; AI provider stubbed; Convex calls stubbed or routed to a local test deployment)
- run: `cd apps/web && npm test`

### 9.3 `apps/web`: Durable Objects + DO SQLite Integration (Post-MVP, L4)

Goal (post-MVP): verify DO SQLite invariants and Convex projection semantics *without* a browser.

Status (2026-02-08):

- DO/DO-SQLite suites are intentionally **deferred** for the Convex-first MVP, and the legacy DO code/tests were removed from `apps/web`.
- When we reintroduce a DO execution/workspace plane post-MVP, we should add this suite back with:
  - DO SQLite event log invariants (seq monotonic, idempotent apply by `eventId`)
  - replication/projection invariants into Convex (idempotent, bounded payloads via BlobRefs)

### 9.4 Chat Streaming + AI Receipts Integration (MVP: Convex, Post-MVP: DO) (L4/L5 boundary)

Goal: verify the chat execution plane produces **user-visible tool parts** and **replayable receipts**.

MVP target:

- Chat streaming is via Convex `messageParts` (chunked), and receipts are stored in Convex (bounded + BlobRefs).
- The primary regression gates are the Worker+Convex integration tests described in §9.2 and the browser E2E scenarios in §9.5.

Legacy / post-MVP coverage (DO + Agents SDK):

- `apps/autopilot-worker/tests/index.test.ts`
  - gates: Response part vocabulary (`tool-call`/`tool-result`/`finish`, no `reasoning-*`)
  - gates: BlobRef discipline on receipts; `finish.usage` present; tool receipts present

Implemented suites (2026-02-07):

- `apps/autopilot-worker/tests/chat-protocol.test.ts`
  - provider `finish_reason` is preserved on the wire `finish` part and in model receipts (e.g. `"length"`)
  - cancel (`CF_AGENT_CHAT_REQUEST_CANCEL`) emits a terminal wire `finish` part (`reason: "pause"`) and records a model receipt with the same finish reason

Planned suite split (when we consolidate single-worker host further):

- (optional) `apps/web/tests/do/chat.test.ts`
  - websocket stream emits valid `@effect/ai/Response` parts
  - large prompt/output bounded and stored as BlobRefs

Harness requirements:

- `TestLanguageModel` provides scripted Response parts including tool calls and failures
- AI provider bindings are stubbed by default (no remote usage charges in CI)

### 9.5 Browser E2E (Effuse Test Runner) (L5)

Goal: verify user-visible behavior in a real browser (Chromium), against a locally started Worker, using **Effect-native** tests (no Promise soup) and a **live execution viewer**.

Implemented suites (2026-02-07):

- `packages/effuse-test/src/suites/apps-web.ts`
  - `apps-web.http.ssr-home`
    - asserts: `GET /` returns SSR HTML with `data-effuse-shell` + `data-effuse-outlet` (no browser)
  - `apps-web.http.assets`
    - asserts: `GET /effuse-client.css` + `GET /effuse-client.js` return 200 with non-empty bodies (no browser)
  - `apps-web.navigation.back-forward`
    - asserts: SPA history back/forward navigates between `/` and `/login` and preserves shell node identity
  - `apps-web.hydration.strict-no-swap`
    - asserts: initial load has SSR `[data-effuse-shell]` + `[data-effuse-outlet]`
    - asserts: strict boot performs **no initial outlet swap** (`window.__effuseSwapCount === 0`)
    - asserts: click `/login` swaps outlet and preserves shell node identity

Harness:

- Runner core + CLI: `packages/effuse-test/src/cli.ts` (Bun + `@effect/cli`)
- Browser control: `packages/effuse-test/src/browser/*` (Chromium via CDP)
- Live viewer UI: `packages/effuse-test/src/viewer/*` (WebSocket event stream + flow graph)
- Local server: starts `apps/web` via `wrangler dev --local` after building `effuse-client` assets

Run:

- `cd apps/web && npm run test:e2e`
- or `cd packages/effuse-test && bun run src/cli.ts run --project ../../apps/web`

Minimum assertions (expand from here):

1. **SSR + strict hydration**
   - initial load shows SSR HTML
   - strict boot performs **no initial outlet swap**
2. **Navigation**
   - click internal links -> outlet swaps, shell stable
   - back/forward works (implemented: `apps-web.navigation.back-forward`)
3. **Focus/caret preservation**
   - type into textarea, trigger outlet swap, caret preserved
4. **EZ action semantics**
   - verify switch-latest cancellation (2nd action cancels 1st)
5. **Tool part visibility**
   - force a tool-error: ensure `{status, toolName, toolCallId, summary}` renders
   - “view full” uses BlobRef fetch/swap (no huge inline DOM)
6. **Convex chat streaming (WebSocket subscriptions)**
   - assistant text streams incrementally from `messageParts`
   - tool parts correlate (toolCallId stable)

Instrumentation for “no swap on strict hydration”:

- `packages/effuse/src/services/dom-live.ts`: dev/test-only counter in `DomService.swap`:
  - if `globalThis.__effuseSwapCount` exists and is a number, increment it per swap
- in E2E, assert `__effuseSwapCount === 0` after initial load, then increases on navigation

Golden-path E2E scenarios (recommended):

- Unauthed `/autopilot`: start chat, stream assistant, tool-error renders, no login required
- Authed flow: login sets cookie, `sessionScopeKey` changes, loader cache keys differ from anon
- Blueprint edit flow: caret stable; “view full” BlobRef works
- Navigation stress: rapid navigations (20+) do not apply stale runs

### 9.6 CI Wiring (Gates)

Recommended gates (wire into your CI of choice):

- `packages/effuse` tests (L0-L2)
- `apps/web` lint/build
- `apps/autopilot-worker` typecheck/tests

Planned expansion:

- add `apps/web` Worker/DO integration suites (L3/L4) as CI gates
- add Effuse Test Runner browser E2E (L5) as a CI gate
  - headless, deterministic fakes, screenshots/videos on failure
  - no external network calls by default (stub WorkOS/Convex/AI)

## 10. Stress + Property Tests (Recommended)

These tests reduce flake and catch “only happens at scale” regressions. Prefer deterministic randomness (Effect `TestRandom`) over introducing new deps unless justified.

- **Property tests: loader key normalization**
  - randomized query param ordering yields same loader key
  - randomized param maps normalize deterministically
- **Fuzz tests: DomService.swap**
  - random DOM trees containing inputs/selection/scroll anchors maintain focus/caret invariants
- **Stress tests: RouterService cancellation**
  - rapid navigation sequences never commit stale results
  - cache/SWR refresh never applies after navigating away
- **Load tests: chat streaming**
  - many streamed parts do not cause unbounded DOM growth
  - bounded tool I/O (BlobRef discipline) holds under large payloads

## 11. Effuse Test Runner (Browser + Visual Execution UI)

This section specifies the Effuse-native browser/integration test runner: Playwright-class capability, but **Effect-first** with a **live execution viewer**.

### 11.1 Goals and Non-Goals

Goals:

- Run browser tests as **Effect programs**.
- Support **headed** mode where a human can watch the run live (demos/debugging).
- Produce artifacts: screenshots (required), HTML snapshots (required), trace/video (optional).
- Allow “no browser” tests to still be watchable via a live **flow graph** of Effect execution:
  - services called
  - spans/events
  - budgets/receipts (when present)

Non-goals (v1):

- Multi-browser parity (Chromium only initially).
- Perfect Playwright API compatibility.
- Distributed execution / device farms.

### 11.2 Architecture Overview

Effuse Test Runner is three cooperating pieces:

1. Runner core (Bun/Node process)
   - orchestrates tests
   - launches browser
   - collects artifacts
   - streams live events to the viewer
2. Browser controller (CDP-based)
   - talks to Chromium via the Chrome DevTools Protocol (CDP)
   - implements navigation, click, type, evaluate, screenshot
3. Live viewer UI (Flow Graph)
   - runs as a local UI for watch mode
   - renders: test steps, Effect spans, service calls, router swaps, tool receipts (when present)

### 11.3 Packages and File Layout

Canonical package layout:

```txt
packages/effuse-test/
  src/
    cli.ts
    spec.ts
    runner/
      runner.ts
      TestContext.ts
      TestServer.ts
      Test.ts
    browser/
      BrowserService.ts
      cdp.ts
      page.ts
    effect/
      ProbeService.ts
      span.ts
    viewer/
      assets.ts
      server.ts
    suites/
      apps-web.ts
  tests/ (unit tests for the runner itself)
```

Notes:

- The viewer may later be split into a dedicated app (e.g. `apps/effuse-test-viewer/`) if we want a richer UI, but the event protocol MUST remain stable.

### 11.4 Test Authoring Model (Effect-First)

Core types:

- `TestCase = { id, tags, timeoutMs?, steps: Effect<void> }`
- `TestContext` (per-test) includes:
  - `runId`, `testId`
  - `baseUrl` (wrangler dev)
  - `artifactsDir`

Authoring rules:

- Tests MUST be written as Effects.
- Tests SHOULD be written in explicit steps, so the viewer can render them as spans:
  - `step("navigates to /login", Effect...)`

### 11.5 Watched Mode (Headed Browser + Live UI)

Watch mode (`--watch`) MUST:

- run Chromium in headed mode
- start the viewer UI
- stream step + span + service-call events live

Recommended behavior:

- on failure, pause (configurable) before shutdown so a human can inspect state
- provide easy repro command output (copyable)

### 11.6 “No Browser” Visual Mode (Flow Graph)

If a suite has no browser steps (Worker integration, conformance, DO tests), the runner MUST still:

- emit spans/events for the Effect execution
- produce a recorded event log artifact (JSONL)
- render the flow graph in the viewer (when `--watch`)

### 11.7 Service Call Instrumentation (ProbeService)

`ProbeService` powers the viewer and artifacts.

MUST:

- be best-effort and bounded (dropping buffer; never block the test)
- record an event stream (JSONL) suitable for replay/view
- broadcast events to the live viewer (WS)

Event capture sources (v1):

- step spans (start/finish)
- service spans (browser operations, server lifecycle, artifacts)

Planned expansion:

- RouterService events (navigate/prefetch/apply)
- DomService swap events (mode/target/focus restoration)
- receipts/budgets (hashes only in viewer; payloads via BlobRefs)

### 11.8 Runner Protocol (Event Stream)

The runner emits a structured event stream over WebSocket (for live UI) and JSONL (for artifacts).

Minimum event types:

- `run.started`, `run.finished`
- `test.started`, `test.finished`
- `span.started`, `span.finished`
- `artifact.created`
- `server.started`, `server.stopped`

### 11.9 Artifacts and Determinism

Artifacts:

- MUST: screenshots on failure
- MUST: HTML snapshot on failure
- SHOULD: record the full event stream as JSONL
- MAY: video/trace (v2+)

Determinism guidelines:

- Prefer explicit waits (`waitForFunction`) over sleeps.
- External services MUST be stubbed by default (WorkOS/Convex/AI), unless the test is explicitly tagged as integration.

### 11.10 CLI Spec

Command: `effuse-test run`

Options (v1 subset):

- `--project <path>` (currently `apps/web`)
- `--watch` (headed + viewer)
- `--headed` / `--headless`
- `--server-port <n>`
- `--viewer-port <n>`
- `--grep <pattern>`
- `--tag <tag1,tag2>`

### 11.11 Integration Points With Existing Suites

- Browser E2E scenarios in §9.5 MUST be implemented with Effuse Test Runner (not raw Playwright).
- Worker/DO integration suites in §9.2/§9.3 MAY optionally run under the runner in no-browser visual mode for watchability.

Work log:

- 2026-02-07: added `packages/effuse-test` v1:
  - Effect-first CLI (`effuse-test run`)
  - Chromium control via CDP
  - live viewer UI (WS + flow graph)
  - artifacts: failure screenshot + HTML snapshot + `events.jsonl`
- 2026-02-07: wired `apps/web` `npm run test:e2e` to run the Effuse Test Runner.

## Appendix A: Normative Requirements (MUST/SHOULD/MAY)

This appendix is the “don’t bikeshed it” spec layer. If something conflicts, this appendix is the contract.

**Routes**
- MUST: `Route.loader` returns `RouteOutcome` (not ad hoc “data or throw”).
- MUST: `Route.guard` (if present) runs before `loader`; if it returns a `RouteOutcome`, the run short-circuits.
- MUST: `RouteOutcome.Ok.hints.dehydrate` is namespaced by `routeId` (no deep merge semantics).
- MUST: `RouteOutcome.Ok.hints.receipts` is mergeable as append-only arrays or stable-id maps (no last-write-wins).
- SHOULD: `Route.view` is pure with respect to `LoaderData` (side effects belong in loaders/subscriptions).

**Shell/Outlet**
- MUST: the shell is stable across navigation; by default only `[data-effuse-outlet]` swaps.
- MAY: routes explicitly opt into `navigation.swap="document"` (treat as exceptional).

**Router Cache/Dedupe/Cancellation**
- MUST: loader keys include `(routeId, pathname, params, search, sessionScopeKey)` and are stable across server/client.
- MUST: `sessionScopeKey` is a stable, low-churn string derived from `AuthService` (for example `anon` | `user:<id>` | `session:<id>`).
- MUST: in-flight loader runs are deduped by loader key.
- MUST: navigation applies results switch-latest; stale results MUST NOT commit to the outlet.
- SHOULD: `prefetch(href)` shares the same loader keying + cache and must not mutate history.

**State (Atoms/Cells)**
- MUST: `@effect-atom/atom` is the canonical application/UI store and is accessible from `AppRuntime` (no “React-only” state).
- MUST: the end state MUST NOT depend on `@effect-atom/atom-react` or any other React binding.
- MUST: only atoms declared with `Atom.serializable(...)` are included in SSR hydration payloads, and each MUST have a stable key + `Schema`.
- MUST: atom hydration is applied before router boot; in `strict` hydration this MUST NOT trigger `DomService.swap` during boot.
- SHOULD: state transitions that can cause DOM updates are batched/atomic, and derived state is equivalence-deduped to avoid needless re-renders.
- SHOULD: `StateCell` is component-local and ephemeral; it MUST NOT be used as a cross-route cache or as an SSR-hydration mechanism.
- SHOULD: loaders MAY read atoms, but SHOULD NOT write atoms as their primary output mechanism; route data MUST flow through `RouteOutcome` to preserve cache semantics and SSR/client parity.

**Convex**
- MUST: the end state MUST NOT depend on `convex/react`, `@convex-dev/react-query`, or React Query.
- MUST: all Convex interactions happen through `ConvexService` and are represented as Effects (Promises do not escape the boundary).
- MUST: server-side Convex calls use `ConvexHttpClient` and are request-scoped (auth is applied per request).
- MUST: the browser connects directly to Convex via WebSockets for realtime subscriptions (no subscription proxying through the Worker).
- MUST: client-side live queries use `ConvexClient` subscriptions wrapped as `Stream` with correct finalizers (unsubscribe on scope close).
- MUST: new `apps/web/convex/*` functions are implemented using the Effect wrappers in `apps/web/convex/effect/*` (centralized normalization, typed errors, and Promise->Effect discipline).
- SHOULD: Convex call args/returns are Schema-encoded/decoded (or otherwise validated) at the boundary for determinism and SSR/client parity.

**Data Residency**
- MUST (MVP): Convex is canonical for user-space/chat state (threads/messages/messageParts), receipts/budgets/tool calls, bootstrap/blueprints, and user identity/ownership/membership.
- SHOULD: large payloads are stored once as blobs (R2 or Convex file storage) and referenced by `BlobRef` in Convex records (never inline megabytes).
- MAY (post-MVP): introduce a DO/DO SQLite execution plane; if introduced, execution plane -> Convex mirroring MUST be event-sourced and idempotent (stable `eventId`, monotonic `seq`), with append-only semantics.

**Hydration**
- MUST: default hydration mode is `strict`.
- MUST: `strict` boot does not call `DomService.swap`.
- MUST: `strict` boot does not run an initial `view` render pass.
- MUST: `strict` boot does not re-run the initial `loader` (it relies on SSR + hydration payload).
- MAY: `soft` mode performs one initial `loader`+`view` and swaps the outlet once.

**Tool Parts**
- MUST: tool calls/results/errors are always user-visible (no “silent stall”).
- MUST: tool parts render at least `{ status, toolName, toolCallId, summary, details? }`.
- MUST: large tool I/O is truncated in DOM and referenced via `BlobRef` for “view full”.

**AI Inference**
- MUST: app inference uses `@effect/ai/LanguageModel` (and `Tokenizer` / `EmbeddingModel` where relevant) from the Effect environment; provider SDKs are confined to adapter modules.
- MUST: streaming output is standardized on `@effect/ai/Response` parts (or a boundary adapter that is 1:1 mappable to Response parts); legacy stream vocabularies MUST be mapped at the boundary.
- MUST: each model call emits a receipt including `(provider, model, params_hash, prompt_blobrefs, output_blobrefs, finish.usage)` and is correlated to tool receipts.
- MUST: tool resolution uses `Toolkit` handlers and emits `tool-call` / `tool-result` / `error` parts so the Tool Parts contract can render them.
- MUST: prompt budgeting uses `Tokenizer` and enforces token caps before provider calls.
- SHOULD: GenAI telemetry spans are emitted with `@effect/ai/Telemetry` (and provider telemetry when available) and are linked to receipts.
- MUST (MVP): chat transcripts/parts are persisted in Convex.
- MAY (post-MVP): add `Chat.Persistence` backed by DO SQLite and project key events into Convex for realtime UI.

**Cloudflare Host**
- MUST: v1 targets **a single Cloudflare Worker** (not Pages Functions) and relevant Cloudflare infra (R2, KV, Queues as needed).
- MAY (post-MVP): add Durable Objects + DO SQLite as an execution/workspace plane.
- MAY: SSR is buffered in v1; streaming SSR is an optimization.
- MUST: buffered SSR enforces a max HTML byte cap and respects request abort + budgets (no unbounded buffering in Workers).

**Conformance**
- MUST: the conformance suite runs in CI and gates framework changes (SSR determinism, hydration, cancellation, shell/outlet invariants, focus/caret, tool visibility, BlobRef discipline).
- MUST: conformance includes LLM/tooling invariants (Response part mapping stable, tool calls always render, `finish.usage` is recorded and receipted).

**Testing & CI**
- MUST: browser E2E runs via the Effuse Test Runner (Effect-native), not raw Playwright.
- MUST: the runner supports `--watch` (headed + live viewer) and `--headless`.
- MUST: the runner emits a structured event stream consumable by the flow graph viewer, and records it as an artifact (JSONL).
- MUST: ProbeService is best-effort and bounded (dropping buffer); it MUST NOT block test execution.
- MUST: artifacts include failure screenshots + HTML snapshots at minimum.
- MUST: browser E2E exists and asserts strict hydration performs no initial swap (attach-only boot).
- MUST: Worker integration tests assert SSR abort handling and max HTML byte cap enforcement.
- MUST: AI/tool receipt tests assert `finish.usage` is recorded and large prompt/output payloads are BlobRefs (never inline).
- MUST (MVP): Convex streaming tests assert idempotency (`runId`, monotonic `seq`) and that chunking rules are enforced (no per-token writes).
- MAY (post-MVP): DO replication tests assert idempotency (`eventId`) and no large payloads are mirrored into Convex without BlobRefs.
- MUST: test suites do not rely on React/TanStack, and do not call real external services by default (WorkOS/Convex/AI); provider SDKs must be stubbed behind Effect Layers.
