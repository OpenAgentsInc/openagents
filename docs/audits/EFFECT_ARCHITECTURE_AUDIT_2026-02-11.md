# Effect Architecture Audit (2026-02-11)

## Scope

This audit covered the remaining TypeScript codebase with focus on Effect architecture:

- `packages/effuse`
- `packages/effuse-panes`
- `packages/effuse-test`
- `apps/web/src/effect`
- `apps/web/src/effuse-host` (Effect-heavy boundaries)

Guidance baseline used:

- `effect-solutions show basics services-and-layers data-modeling error-handling testing`

## Executive Summary

The codebase already has a strong Effect foundation (Tag/Layers, typed errors in many areas, and real `@effect-atom/atom` usage in app state).  
Main issues are concentrated at boundaries: untyped casts, global mutable runtime state, and a split between Effect-native primitives and custom state systems.

Highest-impact improvement:

1. Remove `any`/`unknown` escape hatches at service boundaries and atom data loaders.
2. Standardize service interfaces so methods do not expose `RequestContextService` in `R`.
3. Unify state model around Atom/SubscriptionRef adapters where custom stores currently dominate.

## Snapshot Metrics

- `as any` / `as unknown as` matches in audited Effect surfaces: **128**
- Top files by cast count:
  - `apps/web/src/effect/atoms/dseViz.ts` (36)
  - `apps/web/src/effuse-host/dseAdmin.ts` (22)
  - `apps/web/src/effect/chatWire.ts` (19)
  - `apps/web/src/effect/khala.ts` (14)
- `Runtime.runFork` usage in core libraries: **3**
  - `packages/effuse/src/router/router.ts:613`
  - `packages/effuse/src/router/router.ts:732`
  - `packages/effuse/src/ez/runtime.ts:228`
- `Effect.runSync` in library/runtime code (non-test): **5**
  - `packages/effuse/src/ez/runtime.ts:28`
  - `apps/web/src/effect/runtime.ts:19`
  - `packages/effuse/src/ui/catalog.ts:67`
  - `packages/effuse/src/ui/catalog.ts:90`
  - `packages/effuse/src/ui/renderer.ts:52`

## What Is Already Strong

- Service model is broadly consistent with `Context.Tag` + `Layer` across web/effuse/test packages.
- Many domain errors are `Schema.TaggedError` (good for structured failures).
- `apps/web` already uses `@effect-atom/atom` for session/chat/contracts/dse visual data:
  - `apps/web/src/effect/atoms/session.ts`
  - `apps/web/src/effect/atoms/chat.ts`
  - `apps/web/src/effect/atoms/contracts.ts`
  - `apps/web/src/effect/atoms/dseViz.ts`
- `packages/effuse-test` mostly follows Effect layering/scoped resource patterns.

## Findings (Prioritized)

### P0: Untyped Boundaries (`any`/`unknown` casts) are still pervasive

Evidence:

- `apps/web/src/effect/atoms/dseViz.ts`
- `apps/web/src/effect/chatWire.ts`
- `apps/web/src/effect/khala.ts`
- `apps/web/src/effect/autopilotStore.ts`
- `apps/web/src/effuse-host/dseAdmin.ts`

Why it matters:

- Violates Effect data-modeling guidance (schema-backed boundaries).
- Makes runtime behavior opaque and weakens confidence in telemetry + replay accuracy.

Recommended upgrade:

1. Add schema modules for major response shapes (ops runs, signature details, thread snapshot rows, blueprint rows).
2. Replace casts with `Schema.decodeUnknown` / `Schema.decodeUnknownSync` once at ingress.
3. Keep raw payload in error context (`Schema.Defect`) when decode fails.

---

### P0: Service interfaces leak `RequestContextService` into method `R`

Evidence:

- `apps/web/src/effect/auth.ts:39`
- `apps/web/src/effect/chat.ts:39`
- `apps/web/src/effect/khala.ts:18`
- `apps/web/src/effect/contracts.ts:40`
- `apps/web/src/effect/homeApi.ts:68`

Why it matters:

- Conflicts with service-layer best practice (methods should generally be `R = never`; dependencies are handled in layer wiring).
- Forces callers to carry request context requirements through unrelated business code.

Recommended upgrade:

1. Move request-context dependency to service construction/wiring.
2. Expose context-agnostic methods from services.
3. For per-request SSR execution, provide a request-scoped layer override once at boundary (router/handler), not in every method signature.

---

### P1: Split state model (Atom + custom state/store) is creating architecture drift

Evidence:

- Custom reactive primitive: `packages/effuse/src/state/cell.ts`
- Custom imperative pane store: `packages/effuse-panes/src/paneStore.ts`
- Direct DOM system ownership: `packages/effuse-panes/src/paneSystemDom.ts`

Why it matters:

- App-level state is already Atom-first, but core libraries still rely on bespoke stores.
- Duplicate abstractions increase maintenance surface and type drift risk.

Recommended upgrade:

1. Introduce adapters:
   - `StateCell <-> SubscriptionRef` adapter in `effuse`.
   - `PaneStore <-> Atom` adapter in `effuse-panes`.
2. Keep low-level imperative engine if needed, but expose Atom-facing APIs for app integration.
3. Use `Atom.serializable` for pane layout persistence instead of custom persistence logic in app code.

---

### P1: Unsupervised background fibers via `Runtime.runFork`

Evidence:

- `packages/effuse/src/router/router.ts:613`
- `packages/effuse/src/router/router.ts:732`
- `packages/effuse/src/ez/runtime.ts:228`

Why it matters:

- Harder to reason about lifecycle and cancellation propagation.
- Can produce orphan fibers in long-lived browser sessions.

Recommended upgrade:

1. Move background work into scoped supervision (`Effect.forkScoped`, `FiberSet`, or managed queue workers).
2. Track and expose router/ez runtime fiber health in debug state.
3. Keep cancellation semantics explicit (switch-latest + shared inflight) but supervised.

---

### P1: Global mutable runtime state bypasses service boundaries

Evidence:

- Runtime singleton state: `apps/web/src/effect/runtime.ts`
- Auth cache globals: `apps/web/src/effect/auth.ts`
- Khala debug globals: `apps/web/src/effect/khala.ts`

Why it matters:

- Makes behavior less deterministic under SSR/multi-request execution.
- Complicates test isolation and replay.

Recommended upgrade:

1. Move mutable caches into Effect services using `Ref`/`FiberRef`.
2. Gate debug globals behind a typed debug service.
3. Keep singleton runtime if desired, but move mutable internals behind service APIs.

---

### P2: Side-effectful `runSync` logging in library code

Evidence:

- `packages/effuse/src/ui/catalog.ts:67`
- `packages/effuse/src/ui/catalog.ts:90`
- `packages/effuse/src/ui/renderer.ts:52`
- `packages/effuse/src/ez/runtime.ts:28`

Why it matters:

- Hidden runtime execution in what should be pure validation/render helpers.
- Makes deterministic testing and instrumentation less predictable.

Recommended upgrade:

1. Return warnings as data (or `Effect`) from validation/render paths.
2. Route logs through host-provided telemetry/logger service.
3. Keep pure helper functions pure.

---

### P2: HTTP/JSON decode consistency is uneven

Evidence:

- Strong schema decode: `apps/web/src/effect/homeApi.ts`
- Weaker cast-based decode: `apps/web/src/effect/contracts.ts`, `apps/web/src/effect/autopilotStore.ts`

Why it matters:

- Inconsistent reliability and observability at API boundaries.

Recommended upgrade:

1. Add schemas for contracts/autopilot store responses.
2. Standardize shared `requestJson + decodeWithSchema` helper for web effect services.

---

### P3: Test tooling alignment can improve in pane package

Evidence:

- `packages/effuse-panes/package.json` uses `bun test` only.

Why it matters:

- Loses `@effect/vitest` integration patterns used elsewhere.

Recommended upgrade:

1. Add `vitest` + `@effect/vitest` in `effuse-panes` if/when effectful services are introduced there.
2. Keep pure unit tests where appropriate, but use Effect test runtime for effectful adapters.

## Concrete Upgrade Plan

1. **Boundary Typing Sweep**
   - Target files first: `dseViz.ts`, `chatWire.ts`, `khala.ts`, `autopilotStore.ts`.
   - Goal: remove >80% casts in those files with schema decoders.

2. **Service Contract Normalization**
   - Refactor web services so method `R` is `never` (or strictly local, not request-context leaked).
   - Keep request context at route/handler composition boundaries.

3. **State Unification**
   - Add `effuse` state adapter to `SubscriptionRef` and/or Atom runtime.
   - Add `effuse-panes` Atom adapter and adopt it in `apps/web` pane orchestration.

4. **Fiber Supervision Cleanup**
   - Replace direct `Runtime.runFork` call sites with scoped supervisors.
   - Expose debug counters via service state for test assertions.

5. **Logging + Telemetry Hygiene**
   - Remove in-library `runSync` logging side effects.
   - Route warnings/errors through typed telemetry service.

## Notes on “Atom vs Boutique Custom”

Current state:

- `apps/web` is significantly Atom-adopted.
- `effuse` and `effuse-panes` still expose custom primitives (`StateCell`, `PaneStore`) that are valid but diverge from app state conventions.

Recommendation:

- Keep low-level implementations where they deliver performance/control.
- Standardize public integration around Atom/SubscriptionRef adapters so application code is not forced into custom state semantics.

