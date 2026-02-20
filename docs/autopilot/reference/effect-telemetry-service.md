# Spec: Effect-centric Telemetry Service (Logging + Analytics)

This spec defines a **single telemetry surface** intended to be the *first* cross-cutting Effect service you add (e.g. in `apps/web/src/router.tsx` when you introduce an `AppLayer`).

It should:

- centralize **all logging** (console today, other sinks later)
- support **structured** events (not just strings)
- allow **on/off + level control per service/namespace**
- integrate with **PostHog** the same way `apps/web-old` does (client-only snippet + safe capture helpers)

---

## Goals

- **One place** to send logs/events from any Effect program.
- **Per-namespace controls** (enable/disable, min level).
- **Multiple sinks**:
  - Console sink (immediate usefulness during migration)
  - PostHog sink (product analytics + lightweight operational signals)
  - Future sinks (OTLP, Sentry, “send to worker”, etc.)
- **SSR-safe**: no crashes if PostHog is absent; no `window` usage on server.
- **Low friction**: calling telemetry should be cheap and never block user flows.

## Non-goals

- Building a full tracing platform on day 1.
- Shipping a new backend log ingestion pipeline immediately.
- Capturing secrets/PII by default (telemetry must be privacy-aware).

---

## Conceptual API (service contract)

### Telemetry service

The core service should expose:

- **`log(level, message, fields?)`**
  - For operational logs (“what happened?”).
- **`event(name, properties?)`**
  - For analytics-style events (PostHog-friendly).
- **`identify(distinctId, properties?)`**
  - For tying events/logs to a user/session (maps to PostHog identify on client).
- **`withNamespace(namespace)`**
  - Returns a *scoped* telemetry client that automatically tags all emissions.
- **`with(fields)`**
  - Returns a scoped client that merges in default fields (e.g. `{ requestId, threadId }`).

### Namespaces (“switch on/off per service”)

Every emission should include a `namespace` string. Convention examples:

- `app.start`
- `router`
- `auth.workos`
- `db.khala`
- `ui.chat`

The control plane keys off `namespace` prefixes (so you can disable an entire subtree).

---

## Mapping to actual Effect APIs (so this stays real)

Effect already has first-class building blocks for most of this:

- **Services**: define Telemetry as a `Context.Tag("@app/Telemetry")<...>() {}` and provide implementations with `Layer.*` (see `effect-solutions show services-and-layers`).
- **Log levels + structured logs**:
  - Use `Effect.logDebug` / `Effect.logInfo` / `Effect.logWarning` / `Effect.logError`.
  - Use `Logger` + `LogLevel` to set minimum levels and formatting (`Logger.pretty` is a good default in dev).
  - In tests, logging is suppressed by default in `@effect/vitest`; you can explicitly provide a logger (`Logger.pretty`) when you want output (see `effect-solutions show testing`).
- **Namespace / per-service “switching”**:
  - Prefer modeling namespace as a **log annotation** (e.g. `namespace: "auth.workos"`) plus spans for lifecycle boundaries.
  - Use `Effect.annotateLogs({ namespace })` (or a thin helper) and `Effect.withSpan("...")` / `Effect.fn("...")` to create spans.
  - Filtering “per namespace prefix” can be implemented as a custom Logger that drops messages based on the `namespace` annotation + `LogLevel`.

Practical implication for this spec: the Telemetry contract can be a thin façade over Effect’s Logger + annotations, while still supporting additional sinks (PostHog, etc.) in parallel.

---

## Event model (what gets emitted)

Normalize all telemetry into a single internal shape before sending to sinks:

- **`timestamp`**: ISO string or epoch ms
- **`namespace`**: `string`
- **`kind`**: `"log" | "event" | "identify"`
- **`level`**: `"debug" | "info" | "warn" | "error"` (only for `kind: "log"`)
- **`message`**: `string` (only for `kind: "log"`)
- **`name`**: `string` (only for `kind: "event"`)
- **`distinctId`**: `string` (only for `kind: "identify"`)
- **`fields`** / **`properties`**: `Record<string, unknown>` (structured payload)
- **`error`** (optional):
  - `name`, `message`, `stack` (best-effort)

Additional recommended fields (when available):

- `env`: `local | preview | prod`
- `buildSha`
- `routeId`, `path`
- `sessionId`, `userId` (careful: avoid raw email)
- `requestId` / correlation id (SSR)

---

## Filtering & controls

### Configuration surface

Telemetry should be configured by a single config object supplied at Layer construction time:

- **`enabled`**: boolean (global kill switch)
- **`defaultLevel`**: min log level when no rule matches
- **`namespace` rules**:
  - `minLevelByNamespacePrefix: Array<{ prefix: string, minLevel: Level }>`
  - `disabledNamespacePrefixes: Array<string>`
- **sink enablement**:
  - `console.enabled`
  - `posthog.enabled`
  - future sinks…

Rules apply as:

1. If `namespace` matches any `disabledNamespacePrefixes` → drop.
2. Determine min level from the **longest matching** `prefix` rule; else `defaultLevel`.
3. Apply min-level filtering for `kind: "log"` only (events/identify are controlled by sink enablement + namespace disablement).

### Runtime override (dev ergonomics)

In dev, allow overrides via:

- env vars (`VITE_TELEMETRY_*`)
- and/or a localStorage key (client-only)

This makes it easy to temporarily enable `debug` for one namespace without code changes.

---

## Sinks

Telemetry fans out to sinks. Each sink:

- should be **best-effort** (never throw)
- should avoid blocking UI (fire-and-forget)
- may drop events under backpressure

### Console sink (baseline)

- Formats structured events for readability.
- In dev, include full fields and error stacks.
- In prod, keep it concise (or disable entirely).

### PostHog sink (match `apps/web-old` approach)

`apps/web-old` loads PostHog by injecting a snippet **client-only after hydration** and provides safe helpers that:

- no-op if `window.posthog` is missing
- add page context (`path`, `search`)
- swallow failures

Telemetry should follow that same contract:

- **Client-only**: never attempt to call PostHog on SSR.
- **No hard dependency**: if PostHog isn’t loaded yet, either drop or buffer a small number of events (spec choice; default is drop to keep it simple).
- **Mapping**:
  - `Telemetry.event(name, properties)` → `posthog.capture(name, properties + pageContext + namespace)`
  - `Telemetry.identify(id, props)` → `posthog.identify(id, props + pageContext)`
  - `Telemetry.log(...)`:
    - Option A (recommended): do **not** forward logs to PostHog by default (noise + cost).
    - Option B: forward only `warn`/`error` as `capture('log_error', ...)` for lightweight production signals.

**Reference implementation in web-old**:

- PostHog loader: `apps/web-old/src/routes/__root.tsx` (`PostHogLoader` injects snippet after hydration)
- Safe helpers: `apps/web-old/src/lib/posthog.ts` (`posthogCapture`, `posthogIdentify`)

---

## “First Effect service” wiring (where it plugs in)

When you introduce Effect into `apps/web`, telemetry should be constructed **at the same place other global services are composed** (the composition root).

Recommended insertion points (see `packages/effuse/docs/effect-migration-web.md`):

- **Primary**: `apps/web/src/router.tsx` (the `AppLayer` / service composition root)
- **Secondary**:
  - `apps/web/src/routes/__root.tsx` for SSR boundary + route lifecycle logging/events
  - `apps/web/src/start.ts` for request-middleware lifecycle events on server

After that, *every other service* should depend on Telemetry rather than calling `console.*` or `posthog.*` directly.

---

## Privacy & hygiene rules

- **Never log secrets**: tokens, cookies, auth headers, private keys.
- **Prefer stable non-PII IDs**: WorkOS user id, Khala user doc id, thread id.
- **PostHog properties**:
  - Avoid raw email unless explicitly approved and necessary.
  - Add a small allowlist/denylist if needed once the event surface grows.

---

## Rollout plan

1. Console sink only + namespaces + filtering (gives immediate value).
2. Add PostHog sink using the web-old pattern (client-only, safe no-ops).
3. Gradually replace direct `console.*` + `posthogCapture` calls with Telemetry.
4. Add richer sinks (OTLP/Sentry) only once the event taxonomy stabilizes.
