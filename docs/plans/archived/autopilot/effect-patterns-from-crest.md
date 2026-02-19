# Notes from `~/code/crest`: Effect + Convex patterns to adopt

> Archived on 2026-02-19 after Laravel/runtime cutover. This document targets the retired `apps/web` migration track.

This doc summarizes concrete patterns from the `crest` repo (Next.js + Effect + Convex) that are directly applicable to how we should migrate `openagents/apps/web` toward Effect.

The goal is not to copy Next.js abstractions verbatim, but to adopt the **architecture moves**: a single composed runtime, middleware-style cross-cutting concerns, and Effect-friendly adapters at boundaries (Convex, PostHog, WorkOS, etc.).

---

## 1) Compose one server runtime Layer (don’t sprinkle `provide`)

In `crest`, server handlers are backed by a single Layer composition in:

- `~/code/crest/src/lib/server/effect-runtime.ts`

Key takeaways:

- **Make an “AppLive” layer once**, merging config + services + middleware layers.
- Be careful with `Layer.provide(...)` semantics: it **consumes** dependencies. `crest` explicitly re-merges the config layer to keep `AppConfigService` in the final environment.
- Use `ManagedRuntime.make(...)` to run many handlers against the same wiring.
- They also set unhandled error logging to none in the runtime:
  - `Layer.setUnhandledErrorLogLevel(Option.none())`
  - Useful to avoid double-logging when you already have error middleware.

**Adopt for `apps/web`:**

- Treat `apps/web/src/router.tsx` + `apps/web/src/start.ts` as the “composition root” where we build an `AppLayer`.
- For TanStack Start serverFns/loaders, run programs against that runtime rather than ad hoc services.

---

## 2) Middleware chain for cross-cutting concerns (telemetry/errors/abort)

`crest` implements typed middleware tags for handlers:

- Core handler builder: `~/code/crest/src/lib/server/effect-next/Next.ts`
- Middleware tag model: `~/code/crest/src/lib/server/effect-next/NextMiddleware.ts`
- Real middleware examples:
  - Telemetry middleware: `~/code/crest/src/lib/server/middleware/route-telemetry-middleware.ts`
  - Error middleware: `~/code/crest/src/lib/server/middleware/route-error-middleware.ts`

Takeaways:

- A handler is “just an Effect”, then middleware wraps it (think `Effect.pipe(...)` at the boundary).
- Middleware can:
  - **annotate logs** (`Effect.annotateLogs`)
  - translate errors into HTTP responses (`Effect.catchAll(...)`)
  - attach finalizers (`Effect.ensuring(...)`)
  - implement cancellation/abort handling (they also have `route-abort-middleware.ts`)

**Adopt for `apps/web`:**

- Even without Next.js, we can implement the same pattern at TanStack Start boundaries:
  - wrap `createServerFn` handlers
  - wrap route `beforeLoad`/`loader`
  - wrap any “server-side action” in a shared helper
- Start with 2 middlewares:
  - **Error normalization** (typed error → safe response/log)
  - **Telemetry context** (requestId + flush hook)

---

## 3) Request-scoped correlation: annotate logs with `requestId`

`crest` generates a per-request id and attaches it via log annotations:

- `~/code/crest/src/lib/server/middleware/route-telemetry-middleware.ts`
  - `Effect.annotateLogs("requestId", requestId)`

**Adopt for `apps/web`:**

- In our Telemetry spec, treat correlation ids as first-class fields.
- Implement a “request context middleware” early; it makes debugging Effect programs dramatically easier.

---

## 4) PostHog on server: service + opt-out + flush finalizer

`crest` uses **`posthog-node`** server-side as an Effect service:

- `~/code/crest/src/lib/server/posthog-server.ts`

Patterns worth copying:

- Config loads key as `Redacted`, and if key is missing, the client is created then **`optOut()`** is called (so code doesn’t have to branch everywhere).
- Set `flushAt: 1` and `flushInterval: 0` for near-immediate delivery (tradeoff: more network).
- Telemetry middleware flushes PostHog in a finalizer with tight bounds:
  - `flushPostHog.pipe(Effect.timeout("100 millis"), Effect.catchAll(() => Effect.void))`
  - run via `Effect.ensuring(...)` so it happens on success/failure.

**Adopt for `apps/web`:**

- If we introduce server-side analytics (for serverFns/loaders), do it as an Effect service with the same “opt-out” semantics + bounded flush finalizer.
- For browser PostHog (like `apps/web-old`), keep the current client-only approach; don’t block hydration.

---

## 5) Convex boundary: Effect adapters around Convex contexts

`crest` has an Effect-friendly wrapper layer for Convex runtime contexts:

- `~/code/crest/convex/effect/ctx.ts`
- Helper to wrap Promises: `~/code/crest/convex/effect/tryPromise.ts`

Key idea:

- Build “Effect Query/Mutation/Action contexts” that:
  - keep `ctx` available
  - expose `db`, `storage`, `scheduler`, `auth` as Effect-friendly services
  - wrap `ctx.runQuery/runMutation/runAction` into `Effect.tryPromise(...)` for composability

**Adopt for `openagents`:**

- If we migrate Convex functions (in `apps/web/convex/`) toward Effect, create a small `convex/effect/*` adapter module modeled after `crest`.
- For the web client side (`ConvexReactClient` + `ConvexQueryClient` + React Query), create an Effect service that owns those clients and exposes typed “call” helpers (even if internally it’s still React Query).

---

## 6) Config as a service (Redacted + normalization)

`crest` uses Effect `Config` primitives and `Redacted` for secrets:

- `~/code/crest/src/lib/server/config.ts`

Notable details:

- `Config.option(...)` is used to keep optional vars ergonomic.
- Private keys are normalized (`"\\n"` → `"\n"`) before use.
- Derived config fields are computed once (issuer resolution).

**Adopt for `apps/web`:**

- Use the same pattern for the Effect migration: build `AppConfig` as a Tag + Layer.
- In `apps/web`, be mindful that `import.meta.env` (client) and `process.env` (server) differ; we may end up with separate client/server config layers.

---

## 7) Client-side Effect ergonomics: fiber runner hook

`crest` provides a simple client hook to run/interrupt fibers:

- `~/code/crest/src/hooks/use-effect-runner.ts`

Pattern:

- Keep one active fiber, interrupt when a new effect starts, ensure cleanup with `Effect.ensuring`.

**Adopt for `apps/web`:**

- When we start introducing Effect into React components, create a small set of hooks:
  - run an Effect and manage cancellation on unmount
  - stream results (if needed)
  - keep UI components as consumers of Effect outputs rather than hand-rolling async state
