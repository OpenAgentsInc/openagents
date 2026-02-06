# TanStack Start + Effect: Our Approach vs. the “Practical Effect” Tutorial

This doc compares how we integrated Effect (and Effuse) into **apps/web** with the approach shown in the video tutorial that integrates **TanStack Start** with **Effect** using `@effect/rpc`, `@effect/platform`, and `@effect-atom` (SSR + hydration). The tutorial builds a To-Do app with server functions, RPC, HTTP API, and serializable atoms.

---

## Summary table

| Aspect | Tutorial (presenter) | Our apps/web (Effuse conversion) |
|--------|----------------------|-----------------------------------|
| **Effect in loaders** | Yes: `createServerFn` → `serverRuntime.runPromiseExit` → return dehydrated result | Yes: loaders use `context.effectRuntime.runPromise` (auth, telemetry, redirects) |
| **RPC** | Yes: `@effect/rpc` — RPC server in catch-all `/api` route, RPC client on frontend, HTTP protocol, `disableFatalDefects: true` | No: we call existing HTTP/WebSocket endpoints (WorkOS, Convex, Autopilot worker); no RPC layer |
| **HTTP API** | Yes: `HttpApiGroup`, `HttpApi.make`, `HttpLayerRouter`, served via `toWebHandler` | No: no Effect-defined HTTP API surface; backend is Convex + Cloudflare Worker |
| **State management** | `@effect-atom`: atoms marked serializable, dehydrate in loader, `HydrationBoundary` on client | React state (`useState`, `useMemo`) + optional Convex/React Query; no Effect atoms, no dehydrate/hydrate |
| **SSR data** | Loader returns dehydrated atom state; client hydrates into same atoms | Loader returns plain data (e.g. `signInUrl`, `userId`); components fetch more in `useEffect` (e.g. modules, messages) |
| **Shared runtime** | Managed Runtime + **MemoMap** so API handler and loaders share the same service instance | Single **ManagedRuntime** in router context; no MemoMap; one runtime for all client/loader Effect |
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
We don’t introduce an Effect RPC or HTTP API layer. Backend is:

- Convex (DB, serverless functions)
- WorkOS (auth)
- Autopilot worker (`/agents/*` — WebSocket + REST)

Our Effect usage is **client-side and loader-side**: calling those backends (e.g. `AgentApiService` for `/agents/*`), plus telemetry and auth in loaders. So Effect is the “client + orchestration” layer, not the server API surface.

### 2. State and SSR hydration

**Tutorial:**  
State lives in **@effect-atom**. Loader runs Effect, gets data, and returns a **dehydrated** value (special encoding so atom state can cross the SSR boundary). The client wraps the app in **HydrationBoundary** and rehydrates into the same atoms. So one Effect-backed state tree from server to client.

**Us:**  
State is **React state** (and Convex/React Query where used). Loaders return plain JSON (e.g. `userId`, `signInUrl`). No atom serialization, no HydrationBoundary. Data that isn’t in the loader is fetched on the client in `useEffect` (e.g. modules, tools, messages) and stored in `useState`. So classic “loader for initial/auth, then client fetch + React state,” with Effect only inside the Effect calls (e.g. `runtime.runPromise(AgentApiService.getModuleContracts(...))`).

### 3. Shared server runtime (MemoMap)

**Tutorial:**  
The same **TodoService** (or similar) must be used by both (a) the API handler (RPC/HTTP) and (b) the loaders/server functions. So they build a **Managed Runtime** and pass a **MemoMap** into the handler. Layers are then memoized and shared across the app.

**Us:**  
We have a single **ManagedRuntime** created in the router and passed via context. There is no separate “API route” that also runs Effect; our “API” is Convex + the Autopilot worker. So we don’t need MemoMap or a second composition root. One runtime, one composition point (router).

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

- **Us:** Effect where we already wanted it: **loaders** (auth, telemetry), **client-side calls** (AgentApiService), and **view** (Effuse). Backend stays Convex + Worker; state stays React (and Convex/React Query). We didn’t add RPC, HTTP API, or atoms, so we didn’t need MemoMap, dehydrate/hydrate, or server patches. The main “extra” is Effuse as the UI layer, which the tutorial doesn’t cover.

---

## If we adopted the tutorial’s ideas

- **RPC / HTTP API:** We could define an Effect RPC or HTTP API for future backend endpoints and mount it under a route (e.g. `/api`). We’d need a composition root (and possibly MemoMap) if the same services were used by both that API and our loaders.
- **@effect-atom + hydration:** We could move some client state into atoms and use a dehydrated loader + HydrationBoundary so that state is shared across SSR and client. That would be a larger refactor of how we pass loader data and fetch state.
- **Patches:** If we hit the same Nitro/SRVX issues (e.g. with streaming or long-running requests), we might need similar patches or a different server setup (e.g. the presenter’s SPA + standalone server).

---

## References

- **Our conversion:** `docs/autopilot/effuse-conversion-apps-web.md`
- **Our Effect setup:** `docs/autopilot/effect-migration-web.md`
- **Tutorial:** Video transcription (TanStack Start + Effect, “Practical Effect”); repository link was in the video description.
