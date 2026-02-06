# Router and apps/web Integration (Comprehensive)

This document explains how **routing**, **Effect**, **RPC**, and **Effuse** fit together in `apps/web`, what was implemented to avoid full-page behavior, and where everything lives. It is the single entry point for “how does the web app’s router relate to Effect?” and “how do we prevent full page renders?”

---

## 1. How routing relates to Effect

- **One router, one Effect runtime**  
  The app creates a single router in `getRouter()` (`apps/web/src/router.tsx`) and passes `effectRuntime: makeAppRuntime(appConfig)` in context. Every route and component that uses `router.options.context.effectRuntime` shares that same runtime—and thus the same app layer, RPC client, telemetry, and config.

- **RPC is independent of the route**  
  The Effect RPC endpoint is just another route: `POST /api/rpc` (`apps/web/src/routes/api.rpc.tsx`). Page navigation does not touch it. When a page runs an Effect that calls `AgentRpcClientService`, the client sends a request to `/api/rpc` regardless of the current URL.

- **Summary**  
  **Routing** decides which page component and loaders run; **Effect** lives in router context and is used inside those (loaders, components, RPC client). They are connected only by that shared context.

---

## 2. What we do *not* do: full browser reloads

Navigation uses **TanStack Router**’s client-side routing. The document should not reload; only the tree under `<Outlet />` changes.

### 2.1 Effuse-rendered links (important)

Effuse templates cannot render TanStack’s `<Link>`, so they use plain anchors:

```html
<a href="/login">Log in</a>
```

To keep these anchors **SPA** (no full page refresh), `apps/web/src/components/EffuseMount.tsx` installs a click handler on the Effuse container that:

- detects internal `<a href="/...">` clicks (same-origin, left-click, no modifier keys)
- calls `router.navigate({ href })`
- prevents the browser’s default document navigation

**Opt-outs (not intercepted):**

- external links (different origin)
- `target != "_self"`
- `download`
- `href` starting with `#`

If you need an internal link to do a full reload, add `data-router-ignore`:

```html
<a href="/somewhere" data-router-ignore>Force full reload</a>
```

---

## 3. What can still feel like “full page” (and what we did)

Two things can make navigation feel heavy:

### 3.1 Root `beforeLoad` runs on every navigation

TanStack Router runs `beforeLoad` from root down on **each** navigation. Our root `beforeLoad` used to call `fetchWorkosAuth()` (a server function) on every client-side navigation, so every link click triggered a server round-trip.

**What we did:**

- **Client-side auth cache** in `apps/web/src/routes/__root.tsx`:
  - On the **client**, the first `beforeLoad` calls `fetchWorkosAuth()` and stores the result in a module-level `clientAuthCache`.
  - Subsequent client-side navigations reuse that cache instead of calling the server again.
  - On the **server** we still fetch every time (each request is new).
- **Cache invalidation on sign-out:** `clearRootAuthCache()` is exported and called from the sidebar “Sign out” handlers (`apps/web/src/components/layout/AutopilotSidebar.tsx`) so the next navigation after logout refetches auth instead of using stale data.

### 3.2 Entire page content swaps on navigation

Routes like `/autopilot`, `/modules`, `/tools` are direct children of `__root__`. When you navigate between them, the whole page component unmounts and the new one mounts. The root (providers, layout) stays mounted, but the main content is a full swap—there is no shared “app shell” that stays mounted with only an inner `<Outlet />` changing.

**What you can do (documented, not implemented):**

- Add a **shared layout route** (e.g. `_app` or `_dashboard`) that wraps those routes and renders a persistent shell (e.g. sidebar) plus `<Outlet />` for the main content. Then only the outlet remounts when switching pages.

---

## 4. Effect RPC in apps/web (summary)

- **Endpoint:** `POST /api/rpc` (TanStack Start route in `api.rpc.tsx`).
- **Definition:** RPC group `AgentRpcs` in `apps/web/src/effect/api/agentRpc.ts`; handlers in `agentRpcHandlers.ts` (wrap legacy `AgentApiService` + telemetry).
- **Client:** `AgentRpcClientService` in `agentRpcClient.ts`, wired in `layer.ts`; available via `context.effectRuntime` in any component/loader.
- **Why:** Single typed contract, Effect-native server and client, shared server resources (MemoMap), additive migration without replacing Convex/WorkOS/autopilot backends.

Details: [effect-rpc-web.md](./effect-rpc-web.md).

---

## 5. Shared server runtime (MemoMap)

- **`getServerRuntime()`** (`apps/web/src/effect/serverRuntime.ts`) returns a shared `{ runtime, memoMap }`.
- The same **MemoMap** is passed into `RpcServer.toWebHandler` in `api.rpc.tsx`, so RPC handlers and route loaders/serverFns use the same memoized Effect layer resources.
- **`makeAppRuntime()`** (`apps/web/src/effect/runtime.ts`) builds the app runtime with that MemoMap; `getAppLayer()` exposes the layer for the RPC handler.

Details: [effect-migration-web.md](./effect-migration-web.md), [ADR-0027 (copy)](./adr/adr-0027-effect-rpc-and-atom-hydration-web.md).

---

## 6. @effect-atom and SSR hydration

- **SessionAtom** (`apps/web/src/effect/atoms/session.ts`) is a serializable atom holding minimal session data (e.g. `userId`).
- **Root server fn** `fetchWorkosAuth` runs Effect on the server, builds a registry, sets `SessionAtom`, and returns **dehydrated** `atomState`.
- The app is wrapped in **RegistryProvider** + **HydrationBoundary** in `__root.tsx` with that state so atoms are hydrated on the client before child render.

Details: [effect-migration-web.md](./effect-migration-web.md), [ADR-0027 (copy)](./adr/adr-0027-effect-rpc-and-atom-hydration-web.md).

---

## 7. Entry points and composition roots

| Surface | Role |
|--------|------|
| **`apps/web/src/start.ts`** | Start instance, request middleware (e.g. WorkOS authkit), requestId/telemetry. |
| **`apps/web/src/router.tsx`** | `getRouter()` builds router, Convex + Query clients, **effectRuntime**; injects them into router context. Primary composition root. |
| **`apps/web/src/routes/__root.tsx`** | Root route, app shell (providers), **beforeLoad** (auth + client auth cache), server fn `fetchWorkosAuth`, HydrationBoundary. |
| **`apps/web/src/routes/api.rpc.tsx`** | POST handler for `/api/rpc`; builds RPC web handler with shared MemoMap and app layer. |

---

## 8. Effuse’s place in the stack

- **Effuse** renders most user-facing UI (marketing header, home, login, modules/signatures/tools catalogs, autopilot chat column). Data is loaded in React/Effect and passed into Effuse as payloads; Effuse is pure view.
- **React** keeps: route loaders, auth, Convex, HUD backgrounds, `EffuseMount`, and a few stateful panels (sidebar, Blueprint, controls).

See [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md) and [INDEX.md](./INDEX.md).

---

## 9. References (all in this package)

| Doc | Description |
|-----|-------------|
| [effect-rpc-web.md](./effect-rpc-web.md) | RPC mount, procedures, client usage. |
| [effect-migration-web.md](./effect-migration-web.md) | Effect scaffold, entry points, migration order, routing/navigation. |
| [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md) | Effuse conversion: EffuseMount, data flow, file map, how to add pages. |
| [tanstack-start-effect-comparison.md](./tanstack-start-effect-comparison.md) | Our approach vs. Practical Effect tutorial (RPC, atoms, MemoMap). |
| [DELEGATION-full-effect-integration.md](./DELEGATION-full-effect-integration.md) | Delegation brief for implementing RPC, MemoMap, atoms, hydration. |
| [adr/adr-0027-effect-rpc-and-atom-hydration-web.md](./adr/adr-0027-effect-rpc-and-atom-hydration-web.md) | ADR: Effect RPC + atom hydration (canonical: `docs/adr/ADR-0027-...`). |
| [INDEX.md](./INDEX.md) | Effuse docs index and usage in apps/web. |

---

## 10. Toward 100% Effect/Effuse (thin TanStack layer)

Today `apps/web` is “Effect-first” in **services** and “Effuse-first” in **rendering**, but still uses React for:

- routing + SSR (TanStack Start)
- loaders and most side effects
- stateful chrome (sidebar, blueprint editor, control panels)
- HUD backgrounds (canvas hooks)

If the goal is **near-100% Effect/Effuse**, keep TanStack Start as the minimal router/SSR substrate and migrate everything above it:

- **UI:** Convert remaining React chrome to Effuse programs (sidebar, blueprint panel, controls) and drive them from Effect state (`@effect-atom` or an Effect service). Keep React only as the mounting/runtime host.
- **Events:** Move from “React wiring + DOM listeners in `onRendered`” toward **typed template event directives** (Typed-style) so Effuse templates can bind events without React code.
- **SSR/hydration:** Effuse pages are currently rendered client-side (after hydration) via `useEffect`. To remove most React and improve perceived performance, add an Effuse SSR + hydration story (or adopt a Typed-like template layer for SSR while keeping Effuse runtime semantics).
- **API surface:** Expand RPC usage (or add HttpApi/HttpApiGroup) so both loaders and client state use one typed Effect-native request surface.
- **Testing:** Add “render Effuse template to string/DOM and assert” helpers (Typed has good patterns) so UI logic is testable without a browser.

Nothing in this section is implemented automatically; it’s a direction for future work so agents don’t accidentally re-introduce React-heavy patterns when “Effect everywhere” is the goal.
