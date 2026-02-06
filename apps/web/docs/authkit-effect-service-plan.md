# AuthKit as an Effect Service — Refactor Plan

**Audience:** Implementers refactoring auth in `apps/web`.  
**Goal:** Remove `@workos/authkit-tanstack-react-start` (and any AuthKit React provider/hooks) and implement WorkOS AuthKit as an **Effect service** end-to-end, aligned with `packages/effuse/docs` and `effect-migration-web.md`.

---

## Status (Implemented)

As of **2026-02-06**, we started the “Effect-first, Effuse-first” auth refactor by removing the hosted AuthKit redirect from `/login` and replacing it with **email code (Magic Auth)** handled entirely on our site:

- `POST /api/auth/start` — sends a one-time code to the provided email.
- `POST /api/auth/verify` — verifies the code and sets the encrypted `wos-session` cookie (via `@workos/authkit-session`).
- `/login` UI is Effuse-rendered and submits to these endpoints (no hosted OAuth page).

Files:

- `apps/web/src/auth/workosAuth.ts`
- `apps/web/src/auth/sessionCookieStorage.ts`
- `apps/web/src/routes/api.auth.start.tsx`
- `apps/web/src/routes/api.auth.verify.tsx`
- `apps/web/src/effuse-pages/login.ts` + `apps/web/src/routes/_marketing.login.tsx`

This unblocks login immediately while we continue the broader “remove React AuthKitProvider/hooks” work.

---

## 1. References

**Read first:**

- **[packages/effuse/docs/effect-migration-web.md](../../packages/effuse/docs/effect-migration-web.md)** — Composition roots (router, __root, start.ts), migration order, “services not singletons.”
- **[packages/effuse/docs/effuse-conversion-apps-web.md](../../packages/effuse/docs/effuse-conversion-apps-web.md)** — What stays React (auth, Convex, EffuseMount), data flow (loaders → payload → Effuse).
- **[packages/effuse/docs/DELEGATION-full-effect-integration.md](../../packages/effuse/docs/DELEGATION-full-effect-integration.md)** — RPC, MemoMap, @effect-atom, hydration; auth as a service boundary.
- **Local authkit-react** (`/Users/christopherdavid/code/authkit-react/`) — Reference for client API: `createClient` from `@workos-inc/authkit-js`, state shape, `Context` + `useAuth` (to be replaced by Effect).

**Existing app auth usage:**

- `apps/web/src/routes/__root.tsx` — `AuthKitProvider`, `fetchWorkosAuth` (server fn calling `getAuth()`), `SessionAtom` hydration, `useAuthFromWorkOS` for Convex.
- `apps/web/src/start.ts` — `authkitMiddleware()` wrapped in safe error handling.
- `apps/web/src/useAuthFromWorkOS.tsx` — `useAuth` + `useAccessToken` → Convex `fetchAccessToken` / `isAuthenticated`.
- `apps/web/src/routes/callback.tsx` — `handleCallbackRoute()` (GET handler).
- `apps/web/src/routes/_marketing.login.tsx` — Effuse login UI; uses `/api/auth/start` + `/api/auth/verify` (Magic Auth code flow).
- Other routes — `getAuth()` in loaders for redirect-if-unauthenticated.
- `apps/web/src/components/layout/AutopilotSidebar.tsx` — `useAuth()` for `user`, `loading`, `signOut`.
- `apps/web/src/routes/api.auth.start.tsx` + `apps/web/src/routes/api.auth.verify.tsx` — Magic Auth code flow endpoints.

---

## 2. Current surface to replace

### 2.1 From `@workos/authkit-tanstack-react-start`

| Surface | Where used | Purpose |
|--------|-------------|---------|
| `getAuth()` | __root (fetchWorkosAuth), login, modules, signatures, tools, autopilot, assistant, chat, marketing index | Server: resolve session from request (cookies/session). |
| `getSignInUrl({ data })` | _marketing.login | Server: build sign-in URL with return path. |
| `authkitMiddleware()` | start.ts | Server: request middleware that establishes session. |
| `handleCallbackRoute()` | callback.tsx | Server: GET handler for OAuth callback. |
| `AuthKitProvider` | __root.tsx | Client: wraps app, creates authkit-js client, holds React state. |
| `useAuth()` | AutopilotSidebar, useAuthFromWorkOS | Client: user, loading, signIn, signUp, signOut, getAccessToken, etc. |
| `useAccessToken()` | useAuthFromWorkOS | Client: accessToken, getAccessToken for Convex. |

The TanStack Start package likely depends on `@workos/authkit-session` for server and exposes the same client pattern as **authkit-react** (provider + context + hooks over `@workos-inc/authkit-js` `createClient`).

### 2.2 From local authkit-react (reference)

- **context.ts** — React context holding `Client` + `State`.
- **state.ts** — `State`: `isLoading`, `user`, `role`, `roles`, `organizationId`, `permissions`, `featureFlags`, `impersonator`, `authenticationMethod`.
- **types.ts** — `Client`: `signIn`, `signUp`, `getUser`, `getAccessToken`, `signOut`, `switchToOrganization`, `getSignInUrl`, `getSignUpUrl`; `CreateClientOptions` from `createClient`.
- **provider.tsx** — Creates client via `createClient(clientId, options)`, `onRefresh` → `setState`, provides `{ ...client, ...state }` via context.
- **hook.ts** — `useAuth()` = `useContext`; throws if outside provider.

We do **not** depend on the local authkit-react package in apps/web; we use the published `@workos/authkit-tanstack-react-start`. The local repo is a reference for the **client API shape** (createClient, state, methods) so we can reimplement that surface as an Effect service.

---

## 3. Target architecture: Auth as Effect service

### 3.1 Principles

- **Single abstraction:** Auth is a single Effect service (or a small set of related services) available from the app layer. No React Context for auth state.
- **Server vs client:** Server and client can use different Layer implementations that both satisfy the same service interface where possible (e.g. “get current user / token”).
- **Session state:** Prefer existing `SessionAtom` (dehydrated from server) for SSR/hydration; client AuthService can sync with it or be the source of truth for client-only state (e.g. loading, getAccessToken).
- **Convex:** Convex still needs a React hook that returns `{ isAuthenticated, fetchAccessToken, isLoading }`. That hook will call into the Effect runtime / AuthService instead of React context.

### 3.2 Service shape (conceptual)

Define an **AuthService** (or split into **AuthClientService** and **AuthServerService** if envs differ too much):

**Shared interface (both server and client where applicable):**

- `getUser(): Effect<User \| null>`
- `getAccessToken(): Effect<string | null>` (or fail with typed error)
- `getSession(): Effect<Session>` — e.g. `{ userId: string | null }` to align with `SessionAtom`.

**Client-only (browser):**

- `signIn(options?): Effect<void>` — redirects.
- `signUp(options?): Effect<void>` — redirects.
- `signOut(): Effect<void>`
- `getSignInUrl(options?): Effect<string>`
- `getSignUpUrl(options?): Effect<string>`
- `switchToOrganization(orgId): Effect<void>`
- Optional: reactive state (e.g. `Ref<AuthState>`) for loading/user/claims so UI can subscribe without blocking.

**Server-only (request-scoped or per-handler):**

- `getAuth(): Effect<AuthResult | null>` — same as current `getAuth()` but run inside Effect (adapter around authkit-session or equivalent).
- Middleware and callback stay as thin adapters that call into Effect (or existing server APIs) so request pipeline still works with TanStack Start.

### 3.3 Layer placement

- **Client:** Add `AuthService` (or `AuthClientService`) to the app layer in `apps/web/src/effect/layer.ts`. Implementation will:
  - Call `createClient(clientId, options)` from `@workos-inc/authkit-js` (config from `AppConfigService` or a new `AuthConfigService`).
  - Hold client instance in a Layer resource (or Ref) and expose methods as Effects.
  - Optionally maintain a `Ref<AuthState>` updated from `onRefresh` and expose it for React via a small “subscribe to Ref” hook or by syncing to `SessionAtom` on the client.
- **Server:** Either:
  - **Option A:** Keep using `@workos/authkit-session` (or the same primitives the TanStack Start package uses) inside a server-only **AuthServerService** that provides `getAuth()` / `getSignInUrl()` as Effect, and keep `authkitMiddleware` / `handleCallbackRoute` as thin wrappers that call into the same session/cookie logic; or
  - **Option B:** Reimplement middleware and callback in Effect so that the request pipeline is entirely Effect-driven (larger change; only if we want zero dependency on the TanStack Start auth package).

Recommendation: **Option A** for the first iteration — introduce `AuthServerService` that wraps the existing server auth APIs in Effect, and replace only the **client** React provider/hooks with the Effect AuthService. Then, if desired, replace server helpers with Effect-native implementations.

### 3.4 Convex integration

- **Current:** `ConvexProviderWithAuth` takes `useAuth={useAuthFromWorkOS}`; `useAuthFromWorkOS` uses `useAuth()` and `useAccessToken()` to implement `fetchAccessToken` and `isAuthenticated`.
- **Target:** A new hook, e.g. `useConvexAuth()`, that:
  - Uses the Effect runtime from router context (`context.effectRuntime`).
  - Runs `AuthService.getUser()` / `AuthService.getAccessToken()` in a way that Convex can call (e.g. store latest token in a ref and return `{ isAuthenticated, fetchAccessToken, isLoading }`).
  - No dependency on React Context for auth; all auth comes from Effect.
- Implementation detail: Convex expects a hook that returns an object; we can run Effect synchronously or via `useEffect` + state to populate that object from AuthService.

### 3.5 React components that need auth

- **AutopilotSidebar:** Today uses `useAuth()` for `user`, `loading`, `signOut`. After refactor, it should get the same data from Effect (e.g. a thin hook `useAuthState()` that runs `AuthService.getUser()` / subscribes to auth state from the service and returns `{ user, loading, signOut }`).
- **Root:** No longer wraps with `AuthKitProvider`; instead, the app layer (and thus AuthService) is created in `router.tsx` and available to all routes/components via `context.effectRuntime`. Root only needs `RegistryProvider`, `HydrationBoundary`, and `ConvexProviderWithAuth` with the new Convex auth hook.

---

## 4. Implementation plan (phased)

### Phase 1: Define AuthService interface and client implementation

1. **Add auth config**
   - Extend `AppConfigService` (or add `AuthConfigService`) with `authKitClientId`, `authKitApiHostname`, `redirectUri`, etc., from env (e.g. `VITE_WORKOS_CLIENT_ID` or existing WorkOS env vars).
2. **Define AuthService tag and interface**
   - In `apps/web/src/effect/auth/` (or `effect/authService.ts`):
     - `AuthService` as `Context.Tag` with methods: `getUser`, `getAccessToken`, `getSession`, `signIn`, `signUp`, `signOut`, `getSignInUrl`, `getSignUpUrl`, `switchToOrganization`.
     - Use `@workos-inc/authkit-js` types (`User`, etc.) where appropriate.
3. **Implement client layer**
   - `AuthClientLive`: Layer that:
     - Acquires `AppConfigService` (or `AuthConfigService`).
     - Calls `createClient(clientId, options)` from `@workos-inc/authkit-js`.
     - Wraps the returned client in a service implementation that delegates to the client and returns Effects (e.g. `Effect.tryPromise` for async methods).
     - Handles `onRefresh`: update a `Ref<AuthState>` or sync to `SessionAtom` so UI can react.
   - Provide this layer only on client (e.g. in `makeAppLayer` when `typeof window !== 'undefined'`, or in a separate client-only layer composed in router for browser).
4. **Do not remove AuthKitProvider yet** — keep it so app still works. Optionally run both in parallel and compare (e.g. log both sources of truth) to verify.

### Phase 2: Server auth as Effect

1. **AuthServerService**
   - Define (or extend AuthService with) server-only operations: `getAuth(): Effect<AuthResult | null>`, `getSignInUrl(options): Effect<string>`.
   - Implement by wrapping existing `getAuth` / `getSignInUrl` from `@workos/authkit-tanstack-react-start` in `Effect.tryPromise` inside a Layer that runs only on server (or a shared server runtime).
2. **Use in loaders and server fn**
   - Replace direct `getAuth()` / `getSignInUrl()` calls in loaders and in `fetchWorkosAuth` with `yield* AuthServerService` (or `AuthService`) and the new Effect-based API.
   - Keep `authkitMiddleware()` and `handleCallbackRoute()` as-is for now (they remain the request/callback entry points).

### Phase 3: Replace React auth usage

1. **Convex**
   - Implement `useConvexAuth()` that uses `context.effectRuntime` and `AuthService` to provide `isAuthenticated`, `fetchAccessToken`, `isLoading`.
   - Switch `ConvexProviderWithAuth` to `useAuth={useConvexAuth}`.
   - Remove `useAuthFromWorkOS` once verified.
2. **AutopilotSidebar**
   - Add `useAuthState()` (or similar) that reads from Effect AuthService (and optionally from Ref/atom for reactive updates) and returns `{ user, loading, signOut }`.
   - Replace `useAuth()` with `useAuthState()`.
3. **Root**
   - Remove `AuthKitProvider` from `__root.tsx`.
   - Ensure app layer (including AuthService) is built and available in router context before any route that needs auth.

### Phase 4: Remove AuthKit React package and optional server cleanup

1. Remove dependency on `@workos/authkit-tanstack-react-start` from `package.json`.
2. **Server:** If we kept Option A, we still need the **server** part of the auth flow (middleware + callback). Options:
   - Depend only on `@workos/authkit-session` (or whatever the TanStack Start package uses under the hood) and implement `authkitMiddleware` and `handleCallbackRoute` ourselves in `apps/web` using that, so we can remove the TanStack Start auth package entirely; or
   - Split the npm package usage so we only import server-side symbols from a package that has no React dependency (if such a package exists).
3. Document the final auth flow in this doc and in `effect-migration-web.md`.

---

## 5. File map (target)

| Area | File(s) | Purpose |
|------|---------|---------|
| Auth service | `src/effect/auth/authService.ts` (or `auth/auth.ts`) | Tag, interface, types. |
| Client impl | `src/effect/auth/authClientLive.ts` | Layer that wraps authkit-js `createClient`. |
| Server impl | `src/effect/auth/authServerLive.ts` | Layer that wraps getAuth / getSignInUrl (and optionally middleware/callback). |
| Config | `src/effect/config.ts` or `src/effect/auth/config.ts` | Auth-related config (clientId, apiHostname, redirectUri). |
| Layer | `src/effect/layer.ts` | Compose AuthClientLive (client) and AuthServerLive (server) as appropriate. |
| Convex hook | `src/useConvexAuth.ts` or `src/useAuthFromEffect.ts` | Hook for ConvexProviderWithAuth using AuthService. |
| UI hook | `src/useAuthState.ts` (or keep in a single hook file) | Hook for components (e.g. AutopilotSidebar) that need user/loading/signOut. |
| Root | `src/routes/__root.tsx` | Remove AuthKitProvider; use SessionAtom + HydrationBoundary + new Convex hook. |
| Start | `src/start.ts` | Keep middleware; eventually replace with Effect-driven middleware if Option B. |
| Callback | `src/routes/callback.tsx` | Keep GET handler; eventually use AuthServerService if we reimplement callback. |

---

## 6. Verification

- **Build:** `cd apps/web && npm run build` succeeds.
- **Tests:** Any auth-related tests pass (or add minimal tests for AuthService).
- **Smoke:** Log in, log out, open autopilot, navigate to modules/signatures/tools; Convex and WorkOS session work; no `useAuth must be used within an AuthKitProvider` or equivalent.
- **Docs:** Update [effect-migration-web.md](../../packages/effuse/docs/effect-migration-web.md) to list AuthService and auth flow; add ADR if we introduce a new contract (e.g. “Auth as Effect service in apps/web”).

---

## 7. Summary

| Current | Target |
|--------|--------|
| AuthKitProvider + useAuth / useAccessToken | AuthService in app layer; React hooks that call Effect |
| getAuth() / getSignInUrl() in loaders | AuthServerService (or AuthService on server) in Effect |
| useAuthFromWorkOS for Convex | useConvexAuth (or similar) backed by AuthService |
| SessionAtom hydrated from server fn that calls getAuth() | Unchanged; optional sync from AuthService to SessionAtom on client |

The refactor aligns auth with the rest of the Effect migration: auth becomes a composed service, testable and mockable, with no React-specific auth context. The existing SessionAtom and hydration path remain the source of SSR session; the client gains a single Effect-backed auth surface for Convex and UI.
