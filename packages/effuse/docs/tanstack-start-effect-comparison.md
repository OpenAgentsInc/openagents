# TanStack Start + Effect: Our Approach vs. the “Practical Effect” Tutorial

This doc compares how we integrated Effect (and Effuse) into **apps/web** with the approach shown in the video tutorial that integrates **TanStack Start** with **Effect** using `@effect/rpc`, `@effect/platform`, and `@effect-atom` (SSR + hydration). The tutorial builds a To-Do app with server functions, RPC, HTTP API, and serializable atoms.

---

## Summary table

| Aspect | Tutorial (presenter) | Our apps/web (Effuse conversion) |
|--------|----------------------|-----------------------------------|
| **Effect in loaders** | Yes: `createServerFn` → `serverRuntime.runPromiseExit` → return dehydrated result | Yes: loaders use `context.effectRuntime.runPromise` (auth, telemetry, redirects) |
| **RPC** | Yes: `@effect/rpc` — RPC server in catch-all `/api` route, RPC client on frontend, HTTP protocol, `disableFatalDefects: true` | Yes: `@effect/rpc` mounted at `POST /api/rpc` (`apps/web/src/routes/api.rpc.tsx`) + client service (`AgentRpcClientService`) |
| **HTTP API** | Yes: `HttpApiGroup`, `HttpApi.make`, `HttpLayerRouter`, served via `toWebHandler` | No: no Effect-defined HTTP API surface; backend is Khala + Cloudflare Worker |
| **State management** | `@effect-atom`: atoms marked serializable, dehydrate in loader, `HydrationBoundary` on client | Mixed: React state for most UI + `@effect-atom` for `SessionAtom` with SSR dehydration/hydration in `__root.tsx` |
| **SSR data** | Loader returns dehydrated atom state; client hydrates into same atoms | Root server fn returns plain auth data plus `atomState` for hydration; other routes still fetch more data in `useEffect` |
| **Shared runtime** | Managed Runtime + **MemoMap** so API handler and loaders share the same service instance | **ManagedRuntime + MemoMap** (created in `makeAppRuntime`) and the same `MemoMap` is passed to `RpcServer.toWebHandler` |
| **UI layer** | React (presumably) for the To-Do UI | **Effuse** for most route UI; React for shell, loaders, and a few stateful panels |
| **Patches** | Yes: Nitro (AbortSignal, AbortError) and SRVX (request termination) for dev/server | No: we didn’t patch the stack |
| **Presenter’s take** | Prefers SPA + standalone server (WebSockets, full control, no patches) | We stay on TanStack Start + SSR; no opinion stated here |

---

## Where we overlap

- **Effect in the request path**
  Both use Effect in server-side code. We run Effect in route loaders (auth, telemetry, redirects) via `context.effectRuntime.runPromise`. The tutorial runs Effect in server functions and in the API handler via a managed runtime.

- **TanStack Start as the framework**
  Both use TanStack Start (file-based routes, loaders, SSR). We didn’t replace or deeply customize the server pipeline; the tutorial did (RPC route, HTTP API route, and patches).

- **Type-safe boundaries**
  We have typed Effect services (e.g. `AgentApiService`, `TelemetryService`) and clear boundaries. The tutorial has typed RPC/HTTP APIs and schemas (e.g. `Todo`, errors with `.annotations({ status: 404 })`).

---

## Where we differ

### 1. RPC and HTTP API

**Tutorial:**
The app defines the backend in Effect: RPC group + HTTP API group. The RPC server is mounted at `/api/rpc` (catch-all route); the HTTP API is built with `HttpLayerRouter` and converted to a web handler. So “server functions” and “REST” are both Effect-native, with shared schemas and one runtime.

**Us:**
We **do** introduce a small Effect RPC layer, mounted at `POST /api/rpc`, but we **don’t** (yet) define an Effect HTTP API surface. Backend is still:

- Khala (DB, serverless functions)
- WorkOS (auth)
- Autopilot worker (`/agents/*` — WebSocket + REST)

The current RPC procedures primarily wrap existing `AgentApiService` calls to `/agents/*` to give us end-to-end typed procedures and a single call surface from React/Effect.

### 2. State and SSR hydration

**Tutorial:**
State lives in **@effect-atom**. Loader runs Effect, gets data, and returns a **dehydrated** value (special encoding so atom state can cross the SSR boundary). The client wraps the app in **HydrationBoundary** and rehydrates into the same atoms. So one Effect-backed state tree from server to client.

**Us:**
State is still mostly **React state** (and Khala/React Query where used), but we now have a minimal `@effect-atom` slice:

- `SessionAtom` is marked serializable (`apps/web/src/effect/atoms/session.ts`)
- root server fn returns dehydrated `atomState`, and the app hydrates it via `RegistryProvider` + `HydrationBoundary` (`apps/web/src/routes/__root.tsx`)

### 3. Shared server runtime (MemoMap)

**Tutorial:**
The same **TodoService** (or similar) must be used by both (a) the API handler (RPC/HTTP) and (b) the loaders/server functions. So they build a **Managed Runtime** and pass a **MemoMap** into the handler. Layers are then memoized and shared across the app.

**Us:**
We create a single **ManagedRuntime** in the router context, and it is now created with a shared `MemoMap`. That same `MemoMap` is passed into the Effect RPC web handler (`RpcServer.toWebHandler`) so that RPC handlers and loader/serverFn Effects can share memoized layer resources.

### 4. UI: React vs. Effuse

**Tutorial:**
UI is presumably standard React (the transcript doesn’t emphasize the view layer). The focus is RPC, HTTP API, atoms, and hydration.

**Us:**
We pushed **UI** into **Effuse**: most route content is rendered by Effect programs (Effuse) that receive a payload and call `dom.render(container, content)`. React keeps routing, loaders, state, and shell (backgrounds, sidebar, blueprint panel). So our “Effect integration” is heavy on the **view** (Effuse) and on **data-fetch/orchestration** (Effect in loaders/components), and light on “Effect as the server API” or “Effect as the state store.”

### 5. Patches and platform limits

**Tutorial:**
They had to **patch Nitro** (AbortSignal, AbortError in dev) and **SRVX** (request lifecycle) so the server and RPC/HTTP behavior were correct. The presenter concludes they’d rather use an SPA with a standalone server (e.g. WebSockets, no patches).

**Us:**
We didn’t patch the stack. We use TanStack Start as-is: loaders, SSR, client fetch. Our only “custom” server behavior is the existing proxy to the Autopilot worker in Vite config. So we avoid the integration pain they ran into.

---

## Why the approaches differ

- **Tutorial:** Full Effect stack on the server (RPC + HTTP API), Effect-backed client state (atoms + hydration), and a single shared runtime. Maximizes type safety and consistency from API to UI state, at the cost of integrating with TanStack Start’s server pipeline (and applying patches).

- **Us:** We’re still centered on **Effuse** for most UI, but we’ve adopted enough of the tutorial’s “full stack” approach to support:
  - Effect RPC (`/api/rpc`)
  - a shared `MemoMap` between loader/serverFn and API handler execution
  - minimal `@effect-atom` dehydration/hydration (`SessionAtom`)

---

## If We Go Further

- **HTTP API:** We could add an Effect-defined HTTP API surface (`HttpApiGroup` / `HttpApi.make`) alongside RPC, if we want REST endpoints that share the same schemas/layers.
- **More atoms:** We can move more UI state into atoms (beyond `SessionAtom`) if we want more SSR-consistent state across transitions (at the cost of refactoring existing React state).
- **Patches:** If we hit the same Nitro/SRVX issues (e.g. with streaming or long-running requests), we might need similar patches or a different server setup (e.g. the presenter’s SPA + standalone server).

---

## References

- **Our conversion:** [effuse-conversion-apps-web.md](./effuse-conversion-apps-web.md)
- **Our Effect setup:** [effect-migration-web.md](./effect-migration-web.md)
- **Router and integration overview:** [ROUTER-AND-APPS-WEB-INTEGRATION.md](./ROUTER-AND-APPS-WEB-INTEGRATION.md)
- **Delegation for full integration:** [DELEGATION-full-effect-integration.md](./DELEGATION-full-effect-integration.md) (instruction for an agent implementing RPC, atoms, hydration, MemoMap).
- **Tutorial:** Video transcription (TanStack Start + Effect, “Practical Effect”); repository link was in the video description.
