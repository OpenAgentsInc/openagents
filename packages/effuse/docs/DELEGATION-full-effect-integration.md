# Delegation: Full TanStack Start + Effect Integration (apps/web)

**Audience:** A coding agent implementing the integration.
**Goal:** Integrate **TanStack Start** with **Effect** in `apps/web` using the “full stack” approach from the Practical Effect video tutorial: **@effect/rpc** (or equivalent), **@effect-atom** with SSR dehydration/hydration, and a **shared Managed Runtime** (with MemoMap if the same services are used by both API routes and loaders). The current app already uses Effect in loaders and Effuse for UI; this work **adds** RPC/API surface and Effect-backed client state with hydration—**do not remove or revert** the Effuse conversion.

---

## 1. Prerequisites and references

**Read first (in order):**

1. **[effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md)**  
   Current architecture: EffuseMount, payload-based Effuse views, event delegation, which routes use Effuse, file map.

2. **[tanstack-start-effect-comparison.md](./tanstack-start-effect-comparison.md)**  
   Side-by-side of our current approach vs. the tutorial (RPC, HTTP API, @effect-atom, hydration, MemoMap, patches). Use it as the spec for “what to add.”

3. **[effect-migration-web.md](./effect-migration-web.md)**  
   Where Effect lives today (router, loaders, `makeAppRuntime()`), composition roots (`router.tsx`, `__root.tsx`, `start.ts`), and recommended migration order.

4. **AGENTS.md (repo root)**  
   Authority rules (code wins, no stubs, verification first), build/test commands, doc gates (ADRs, GLOSSARY).

5. **Tutorial source**  
   The video transcription describes: domain API (RPC group + HTTP API group), schemas with annotations, RPC client/server setup (HTTP protocol, `disableFatalDefects: true`), HTTP handler via `HttpLayerRouter.toWebHandler`, HMR dispose pattern, MemoMap for shared runtime, `createServerFn` → `serverRuntime.runPromiseExit` → dehydrate, `@effect-atom` serializable atoms, HydrationBoundary, and Nitro/SRVX patches. Obtain the actual repo link from the video description if possible and use it as the reference implementation.

**Existing code to reuse / extend:**

- `apps/web/src/effect/*` — `TelemetryService`, `AgentApiService`, `makeAppRuntime()`, etc.
- `apps/web/src/router.tsx` — creates `effectRuntime` and puts it in router context.
- Loaders in `apps/web/src/routes/*` — use `context.effectRuntime.runPromise` for auth and data.
- Effuse pages in `apps/web/src/effuse-pages/*` and `EffuseMount` — **keep as-is**; integration should feed data into the same payloads/run functions where appropriate.

---

## 2. Scope and deliverables

Implement the following. Prefer small, reviewable steps (e.g. one deliverable per PR or commit batch).

### 2.1 Effect RPC surface (optional but recommended)

- **Define** an Effect RPC API group (e.g. under `apps/web/src/effect/api/` or `apps/web/src/api/`) for at least one domain (e.g. “agent” or “chat”) with procedures that the app already needs (e.g. get blueprint, get messages, get tool contracts). Use `@effect/rpc` (or the pattern from the tutorial).
- **Implement** the RPC server in a TanStack Start route (e.g. catch-all under `/api/rpc`). Use HTTP protocol; set `disableFatalDefects: true` so the client can associate failures with request IDs.
- **Create** an RPC client for the frontend, configured for the same protocol and path. Optionally add error logging (e.g. `addRpcErrorLogging`-style) and standard pipeline (filter status OK, retries, auth if needed).
- **Document** in this package’s docs (e.g. [effect-rpc-web.md](./effect-rpc-web.md)) how the RPC is mounted and how loaders/components should call it. Do not remove existing `AgentApiService` usage until RPC equivalents are in place and verified.

### 2.2 Shared server runtime (MemoMap)

- **Introduce** a shared **Managed Runtime** built with a **MemoMap** so that:
  - The same Effect layers (e.g. services used by RPC handlers) are used by **both** (a) the API/RPC route handler and (b) route loaders / server functions.
- **Wire** this runtime so that the RPC (and any HTTP API) handler and the loader/server paths use the same memoized layers. Document where the MemoMap is created and passed (e.g. in `router.tsx` or a dedicated server runtime module).
- **Ref:** Tutorial’s `app/server-runtime.ts` and passing `memoMap` into `toWebHandler`.

### 2.3 @effect-atom and SSR hydration

- **Add** `@effect-atom` (or the exact library used in the tutorial). Define at least one **serializable** atom (or a small set) that holds data currently loaded in a loader and/or in React state (e.g. user id, or a minimal “session” shape).
- **Dehydrate** in a loader: run Effect (e.g. `serverRuntime.runPromiseExit`), get the data, and return a **dehydrated** value that TanStack Start can serialize (use the tutorial’s dehydrate helper / encoded map pattern).
- **Hydrate** on the client: wrap the app (or the appropriate subtree) in **HydrationBoundary** with the initial state from the loader so atoms are initialized from SSR data.
- **Document** the dehydrate/hydrate contract and where HydrationBoundary is used (e.g. `__root.tsx` or a layout). Ensure existing Effuse pages still receive correct props; if the payload for an Effuse run now comes from an atom, document that data flow.

### 2.4 Server functions and loaders

- **Align** loaders with the new runtime: use the shared Managed Runtime (and MemoMap) when running Effect in loaders so they share services with the RPC/API handler.
- **Optionally** convert one or two existing loader Effect calls to go through RPC (if you added RPC) so that the same procedure is used from both SSR and client. Prefer leaving existing behavior working until the new path is verified.

### 2.5 Patches (only if needed)

- **Only if** you hit request lifecycle or AbortSignal issues (e.g. client disconnect crashes, early request termination): consider the same **Nitro** and **SRVX** patches described in the tutorial. Prefer minimal, documented patches; if the stack cannot be patched cleanly, document the limitation and any workaround (e.g. SPA + standalone server for specific features).

---

## 3. Constraints and non-goals

- **Do not remove or revert the Effuse conversion.** Effuse remains the primary UI renderer for the routes described in [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md). The integration may feed data into Effuse via existing payloads (e.g. from atoms or RPC) or via loader data that components pass into `EffuseMount`.
- **Do not break** existing auth (WorkOS), Convex, or Autopilot worker usage. RPC/atoms are additive; migrate call sites only when the new path is ready and tested.
- **Preserve** AGENTS.md rules: no stubs in production paths, verification (build + tests) before claiming done, doc updates for new contracts (ADRs, GLOSSARY if terminology changes).
- **Non-goal:** Reimplementing the entire backend in Effect. Convex and the Autopilot worker stay; the integration adds an Effect RPC/API surface and Effect-backed state/hydration **alongside** them.

---

## 4. Verification

- **Build:** `cd apps/web && npm run build` must succeed.
- **Tests:** Run the app’s test script (e.g. `npm run test` in `apps/web` if present) and fix any regressions.
- **Smoke check:** Load `/`, `/login`, `/autopilot`, and at least one catalog route (`/modules`, `/signatures`, or `/tools`). Confirm SSR and client navigation still work and that any new RPC or atom-backed data appears where expected.
- **Docs:** Update [effect-migration-web.md](./effect-migration-web.md) (and any new API docs) to describe the new runtime shape, MemoMap usage, RPC mount point, and hydration flow. Add or update an ADR if you introduce a new architectural contract (e.g. “Effect RPC in apps/web”).

---

## 5. Output checklist

When the integration is done, the following should be true and documented:

- [ ] Effect RPC (and optionally HTTP API) is defined, implemented, and mounted under a route; client is configured and used from at least one call site (or documented as “ready for migration”).
- [ ] A shared Managed Runtime with MemoMap is used by both the API handler and loaders; where it is created and how it is passed is documented.
- [ ] At least one @effect-atom is serializable and used with dehydrate (loader) and HydrationBoundary (client); data flow from SSR to Effuse or React is documented.
- [ ] Existing Effuse UI and auth/Convex/worker flows still work; build and tests pass.
- [ ] Any patches (Nitro/SRVX) are minimal, documented, and justified, or limitations are documented.
- [ ] [effect-migration-web.md](./effect-migration-web.md) and, if needed, [tanstack-start-effect-comparison.md](./tanstack-start-effect-comparison.md) or a new ADR are updated to reflect the full integration.

Use this document as the single delegation brief; refer back to the comparison doc and the tutorial for concrete patterns (RPC group shape, `disableFatalDefects`, dehydrate helper, HydrationBoundary usage, MemoMap wiring).
