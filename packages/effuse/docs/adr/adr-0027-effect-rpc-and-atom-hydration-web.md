# ADR-0027: Effect RPC + Atom Hydration in apps/web

**Canonical location:** `docs/adr/ADR-0027-effect-rpc-and-atom-hydration-web.md` (this is a copy for Effuse doc consolidation).

## Status

Accepted

## Date

2026-02-06

## Context

`apps/web` uses TanStack Start (SSR) and already runs Effect in loaders and client-side orchestration. We want:

- A typed, Effect-native request surface to call from React/Effect (without immediately replacing Khala/WorkOS/Autopilot worker backends).
- SSR-safe, shared state that can be dehydrated on the server and hydrated on the client.
- One shared server composition root so API handlers and loader/serverFn execution use the same memoized Effect layer resources.

## Decision

We will:

1. Mount an Effect RPC endpoint at `POST /api/rpc` using `@effect/rpc` in TanStack Start server handlers.
2. Create the `apps/web` Effect runtime with a shared `MemoMap`, and pass that same `MemoMap` into the RPC web handler so RPC handlers and loader/serverFn Effects share memoized layer resources.
3. Use `@effect-atom` for SSR dehydration/hydration of a minimal serializable `SessionAtom`, wired through a root-level `RegistryProvider` + `HydrationBoundary`.

## Scope

What this ADR covers:

- RPC mount point and client/server wiring in `apps/web`.
- Shared server `MemoMap` contract for server-side entry points (loaders/serverFns and API handlers).
- `SessionAtom` dehydration/hydration wiring across SSR in `apps/web`.

What this ADR does NOT cover (non-goals):

- Replacing Khala, WorkOS, or the Autopilot worker backend with an Effect HTTP API surface.
- Migrating all React state to atoms.
- Implementing Nitro/SRVX patches unless required by concrete lifecycle bugs.

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| RPC mount point | Stable: `POST /api/rpc` |
| RPC group location | Stable: `apps/web/src/effect/api/agentRpc.ts` (until superseded by a follow-on ADR) |
| Shared MemoMap | Stable: API handlers that use Effect web handlers pass the same server `MemoMap` as loader/serverFn runtime creation |
| Session atom key | Stable: `@openagents/web/session` |

Backward compatibility expectations:

- `AgentApiService` remains supported during migration. RPC procedures may wrap it until direct RPC-native implementations exist.

## Consequences

**Positive:**

- End-to-end typed procedures for web app calls without immediately restructuring backends.
- Shared server memoization for Effect layer resources across TanStack Start server entry points.
- A concrete SSR hydration path for Effect-backed client state.

**Negative:**

- Additional dependencies (`@effect/rpc`, `@effect/platform`, `@effect-atom/*`) and some bundle/complexity overhead.
- Two request surfaces exist during migration (legacy `AgentApiService` + RPC facade).

**Neutral:**

- UI remains primarily Effuse-rendered; React continues to own routing + shell.

## Alternatives Considered

1. **Keep HTTP fetch only (`AgentApiService`)**: simplest, but no shared typed procedure surface across server/client and no standardized RPC error/serialization pipeline.
2. **Use TanStack Start server functions exclusively**: works, but doesn't provide an Effect-native RPC group/protocol and does not unify with Effect middleware patterns.
3. **Go straight to a full Effect HTTP API (`HttpApiGroup`)**: higher upfront migration cost and does not align with the "additive" requirement to avoid breaking existing backends.

## References

- RPC mount: `apps/web/src/routes/api.rpc.tsx`
- RPC definitions: `apps/web/src/effect/api/agentRpc.ts`
- RPC handlers: `apps/web/src/effect/api/agentRpcHandlers.ts`
- RPC client: `apps/web/src/effect/api/agentRpcClient.ts`
- Shared server runtime: `apps/web/src/effect/runtime.ts`, `apps/web/src/effect/serverRuntime.ts`
- Atom hydration: `apps/web/src/effect/atoms/session.ts`, `apps/web/src/routes/__root.tsx`
- Migration notes: [effect-migration-web.md](../effect-migration-web.md)
- RPC notes: [effect-rpc-web.md](../effect-rpc-web.md)
