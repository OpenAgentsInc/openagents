# Post–Signup “Unauthorized” on ensureOwnedThread: Full Incident Report

**Symptom:** After completing magic-auth signup (email → code → verify), the client throws:

```text
Uncaught (FiberFailure) Error: unauthorized
```

The failure occurs when the app calls the Convex mutation `ensureOwnedThread`. The user appears signed in (session state shows a user) but Convex receives no auth token, so the mutation rejects with `unauthorized`.

**Status:** Fix implemented and deployed on **2026-02-10**. This document records every intervention, why it was insufficient, the underlying problems, and the final fix.

---

## 1. Where the error is thrown

| Location | What happens |
|----------|----------------|
| **apps/web/convex/autopilot/threads.ts** | `ensureOwnedThreadImpl` (lines 104–108) calls `getSubject(ctx)`. If `subject` is null, it returns `Effect.fail(new Error("unauthorized"))`. |
| **apps/web/convex/autopilot/access.ts** | `getSubject(ctx)` (lines 13–21) returns the Convex auth identity: `ctx.auth.getUserIdentity()` mapped to a `subject` string. If Convex has no JWT (or invalid JWT), identity is None and subject is null. |
| **apps/web/src/effect/chat.ts** | `getOwnedThreadId()` (line 223) calls `convex.mutation(api.autopilot.threads.ensureOwnedThread, {})`. When the Convex client has no token, the mutation runs without auth and Convex returns unauthorized. |
| **apps/web/src/effuse-app/controllers/autopilotController.ts** | `ensureOwnedThreadId()` (and related logic around 453, 514, 561, 635) triggers `getOwnedThreadId()` when the user is logged in but the owned thread is not yet loaded. |

So the chain is: **client has session UI state → calls ensureOwnedThread → Convex setAuth supplies token → Convex validates JWT → getSubject returns subject**. The break is: **Convex setAuth is supplying no token** (or a token Convex rejects).

---

## 2. How Convex gets the token

| File | Role |
|------|------|
| **apps/web/convex/auth.config.ts** | Convex JWT config: WorkOS issuers (`https://api.workos.com/`, `https://api.workos.com/user_management/${clientId}`) and E2E issuer. Only requests that send a valid JWT for one of these issuers get an identity. |
| **apps/web/src/effect/convex.ts** | Convex React client is created with `c.setAuth(async () => { ... })` (lines 169–187). The callback calls `auth.getAccessToken({ forceRefreshToken: true })`, then returns that token (or null). So **whatever token the AuthService returns here is what Convex sends**. |
| **apps/web/src/effect/auth.ts** | On the client, `getAccessToken` (lines 263–276) calls `fetchClientAuthState({ forceRefreshToken: options.forceRefreshToken })` and returns `state.token`. So the token comes from the same place as session: either **client cache** or **GET /api/auth/session**. |
| **apps/web/src/effuse-host/auth.ts** | Worker `handleSession` (lines 171–259) handles GET /api/auth/session. It uses `authkit.withAuth(request)` to read the WorkOS session from the request cookie, then returns a JSON payload that includes `token: auth.user ? (auth.accessToken ?? null) : null` (lines 232–234). So **if withAuth returns no user (or no accessToken), the session response has no token**. |

So the full path is:

1. **Browser** sends Cookie (WorkOS session cookie) to GET /api/auth/session.
2. **Worker** runs `authkit.withAuth(request)`. If that returns a user with `accessToken`, the response body includes that token.
3. **Client** AuthService calls GET /api/auth/session (when it needs token), parses JSON, and uses `token` from the response.
4. **Convex** setAuth callback calls AuthService.getAccessToken; that either uses **client cache** or fetches session; the returned token is sent to Convex.
5. **Convex** validates the JWT and sets identity; `getSubject(ctx)` then returns the subject or null.

Any break in (1)–(4) results in Convex seeing no token and `ensureOwnedThread` failing with `unauthorized`.

---

## 3. What we’ve tried and why it wasn’t enough

### 3.1 Session payload and E2E path

- **Change:** In **apps/web/src/effuse-host/auth.ts**, `handleSession` was adjusted so that for “human web” we only return the WorkOS token (no E2E JWT preference). E2E path is used only when there’s no WorkOS user.
- **Why it failed:** The issue wasn’t which token we prefer; it was that **the session response often had no user/token at all** because `withAuth` was returning no user for the request (see below).

### 3.2 Convex setAuth logging and forceRefreshToken

- **Change:** In **apps/web/src/effect/convex.ts**, the setAuth callback was given `forceRefreshToken: true` and logging: `hasToken`, `tokenLength`, and errors from `getAccessToken`.
- **Observation:** Logs showed `hasToken: false`, `tokenLength: 0` — so the callback was receiving no token. That confirmed the failure is **before** Convex: the client never gets a token to send.

### 3.3 Session fetch with credentials

- **Change:** In **apps/web/src/effect/auth.ts**, `fetchClientAuthState` uses `fetch('/api/auth/session', { credentials: 'include', ... })` so the browser sends cookies.
- **Why it failed:** Credentials were correct; the problem was **either the cookie wasn’t sent yet (timing)** or **the cookie was sent but the Worker couldn’t read it** (see “cookie present but no user” below).

### 3.4 Post-verify delay (150 ms) before getOwnedThreadId

- **Change:** In **apps/web/src/effuse-app/controllers/homeController.ts**, after verify we waited 150 ms then called `getOwnedThreadId()` so the browser could apply the verify response’s Set-Cookie before the Convex auth callback ran.
- **Why it failed:** 150 ms was not reliable. The session fetch triggered by setAuth could still run before the new cookie was applied, or the cookie sent could still be a stale one. So we still got no token.

### 3.5 Worker logging when withAuth fails or returns no user

- **Change:** In **apps/web/src/effuse-host/auth.ts**, `handleSession` now:
  - Catches withAuth errors and logs full message and stack.
  - Logs when `hasCookie && !hasUser` (and whether withAuth threw).
- **Observation:** Logs showed **“cookie present but no user from withAuth” with `withAuthThrew: false`**. So the Worker was receiving **a** Cookie header, but `withAuth` was **not** throwing and **not** returning a user. So either:
  - The cookie value was empty/invalid and withAuth returned null without throwing, or
  - The cookie was from a previous (e.g. post–sign-out) state and didn’t decode to a valid session.

### 3.6 Clearing bad cookie when withAuth throws

- **Change:** In **apps/web/src/effuse-host/auth.ts**, when `withAuth` threw and we had a cookie, we called `clearSessionCookie()` and returned that Set-Cookie in the session response so the browser would drop the bad cookie.
- **Why it wasn’t enough:** Most of the time withAuth **didn’t** throw; it just returned no user. So we weren’t clearing the cookie in the common case.

### 3.7 Clearing cookie whenever cookie present but no user

- **Change:** In **apps/web/src/effuse-host/auth.ts**, we extended the clear logic to: whenever `hasCookie && !hasUser`, return Set-Cookie to clear the session cookie (lines 248–256).
- **Why it wasn’t enough:** That fixes the case where a **stale** cookie is sent. It doesn’t fix the case where **right after verify** the session request is sent **before** the new cookie is applied, or where the new cookie is applied but something else is wrong (e.g. Convex setAuth never uses the primed cache — see below).

### 3.8 Full page reload after verify

- **Change:** In **apps/web/src/effuse-app/controllers/homeController.ts**, after verify success we called `window.location.reload()` so the next document load would have the new cookie and all subsequent session fetches would send it.
- **Result:** This **did** fix the unauthorized error for many cases, because the reload guaranteed the cookie was set before any JS ran. Downsides: visible reload and a brief flash of content before the chat opened.

### 3.9 localStorage flag to open chat after reload

- **Change:** We set `localStorage.setItem("oa-open-chat-after-reload", "1")` before reload; on home mount we read it, remove it, and open the chat pane after a short delay ( **apps/web/src/effuse-app/controllers/homeController.ts** ).
- **Result:** Reload flow then showed the chat correctly, but we still depended on the full reload to avoid the auth race.

### 3.10 No-reload path: verify returns token + user, prime client cache

- **Change:**
  - **apps/web/src/effuse-host/auth.ts:** Verify handler now returns `token` (WorkOS access token) and `user` in the JSON body for web (not only Expo) (lines 353–363).
  - **apps/web/src/effect/auth.ts:** Added `setClientAuthFromVerify(session, token)` to prime the client-only cache so `getSession` / `getAccessToken` can return the token without a network call (lines 69–72).
  - **apps/web/src/effuse-app/controllers/homeController.ts:** After verify success, if the response has `token` and `user`, we call `clearAuthClientCache()`, then `setClientAuthFromVerify(session, token)`, set `SessionAtom`, and run `getOwnedThreadId()` in-pane (no reload, no localStorage) (lines 499–555).
- **Why it still failed originally:** There were two intertwined issues:
  1. Our Convex `setAuth` callback was always doing `auth.getAccessToken({ forceRefreshToken: true })`, so even though we primed the in-memory cache from verify, Convex would immediately hit `GET /api/auth/session` (bringing the cookie timing race back).
  2. More importantly: when Convex boots unauthenticated and its initial token fetch returns `null`, Convex can transition into a sticky `noAuth` state. In that state it will **not** start fetching tokens just because the app later logged in; you must call `client.setAuth(...)` again to restart the auth manager.

  This is why a full page reload “fixed” it: reload reconstructs the Convex client and runs `setAuth` again after the cookie exists.

### 3.11 Definitive fix: refresh Convex auth after login (no reload)

- **Change:**
  - **apps/web/src/effect/convex.ts:** `setAuth(fetchToken)` now respects Convex’s `{ forceRefreshToken }` argument instead of always forcing refresh. Added `ConvexService.refreshAuth()` which re-calls `client.setAuth(fetchToken)` to restart Convex auth after login.
  - **apps/web/src/effuse-app/boot.ts:** When `SessionAtom.userId` becomes authenticated (or changes), call `convex.refreshAuth()` (best-effort).
  - **apps/web/src/effuse-app/controllers/homeController.ts:** After verify success, prime the client auth cache with the verify response token, set `SessionAtom`, call `refreshConvexAuth()` (wired from boot), then call `getOwnedThreadId()`; removed reload + localStorage flow.

- **Why this works:** The verify response includes a WorkOS access token immediately (no dependency on cookie application timing). By priming the in-memory cache and re-triggering Convex `setAuth()` on login, Convex authenticates before `ensureOwnedThread` runs.

---

## 4. Root causes (summary)

1. **Cookie timing:** The browser may not have applied the verify response’s Set-Cookie before the very next request (e.g. the session fetch triggered by Convex setAuth). So the session endpoint sometimes sees no cookie or an old cookie.
2. **Stale cookie after sign-out:** After sign-out, a cookie can still be present (e.g. empty or “cleared” value). The Worker then sees “cookie present but no user” (withAuth doesn’t throw, just returns null). We now clear that cookie in the session response, but that doesn’t help the first request right after verify if that request is the one that runs before the new cookie is there.
3. **Convex sticky `noAuth` + no re-setAuth on login:** If Convex boots unauthenticated (token fetch returns `null`), it can settle into `noAuth` and won’t start fetching tokens later just because the app logged in. Without calling `client.setAuth(...)` again after verify, Convex keeps sending unauthenticated requests and `ensureOwnedThread` fails.
4. **setAuth forced refresh / ignored Convex fetch args:** Our `setAuth` callback ignored Convex’s `{ forceRefreshToken }` parameter and always forced a session fetch, which amplified the cookie timing race and prevented reliably using the verify-primed token.

---

## 5. File reference (full paths)

| Purpose | Full path |
|--------|-----------|
| Convex: where “unauthorized” is thrown | `apps/web/convex/autopilot/threads.ts` (`ensureOwnedThreadImpl`) |
| Convex: how identity is obtained | `apps/web/convex/autopilot/access.ts` (`getSubject`) |
| Convex: JWT validation config | `apps/web/convex/auth.config.ts` |
| Client: Convex setAuth callback | `apps/web/src/effect/convex.ts` (setAuth, getAccessToken) |
| Client: getAccessToken / fetchClientAuthState / cache | `apps/web/src/effect/auth.ts` |
| Client: prime cache after verify | `apps/web/src/effect/auth.ts` (`setClientAuthFromVerify`) |
| Client: getOwnedThreadId mutation | `apps/web/src/effect/chat.ts` |
| Client: post-verify flow (no-reload vs reload) | `apps/web/src/effuse-app/controllers/homeController.ts` |
| Client: ensureOwnedThreadId usage | `apps/web/src/effuse-app/controllers/autopilotController.ts` |
| Worker: session endpoint | `apps/web/src/effuse-host/auth.ts` (`handleSession`) |
| Worker: verify endpoint (returns token + user) | `apps/web/src/effuse-host/auth.ts` (`handleVerify`) |
| Worker: clear session cookie helper | `apps/web/src/auth/workosAuth.ts` (`clearSessionCookie`) |
| WorkOS session storage (cookie read/write) | `apps/web/src/auth/sessionCookieStorage.ts` |

---

## 6. Recommended next steps

1. **Re-trigger Convex auth on login:** Add a client-only `ConvexService.refreshAuth()` that re-calls `client.setAuth(fetchToken)` and call it whenever `SessionAtom.userId` becomes authenticated. (Implemented.)
2. **Respect Convex’s `forceRefreshToken` argument:** `fetchToken` should use the `forceRefreshToken` value Convex passes in, not always force refresh. (Implemented.)
3. **Remove reload/localStorage flow:** Keep the entire verify → authed → Convex path in-page by using the verify response token to prime the in-memory cache and refreshing Convex auth before the first authed mutation. (Implemented.)
4. **Keep the verify-primed cache as a safety net:** Returning the primed cache for a short window can still be useful if any code path forces a refresh at the wrong time. (Still present in `apps/web/src/effect/auth.ts`.)

---

## 7. Why this has been so difficult

- **Multiple systems:** Worker (cookie/session), browser (when cookie is applied), client AuthService (cache vs fetch), Convex client (when setAuth runs), Convex server (JWT validation). A break in any one looks the same in the UI: “unauthorized.”
- **Timing:** The critical moment is the first few hundred milliseconds after verify. Whether the cookie is sent on the very next request is browser- and timing-dependent.
- **Two code paths:** Reload path (works but bad UX) vs no-reload path (better UX but never used the cache for setAuth). The no-reload path was implemented but the Convex auth path was still “force refresh → always fetch,” so the cache was bypassed.
- **Observations spread across Worker logs, client console, and Convex:** No single log line shows “no cookie” and “setAuth used no token” together; you have to correlate Worker request id, client setAuth logs, and Convex request id.

This document should be updated when any of the above fixes are implemented or when new findings (e.g. from production logs) clarify the exact failing request.

---

## 8. Fix applied (Convex re-auth on login; no reload)

We now explicitly refresh Convex auth when the user becomes authenticated (magic-auth verify):

- `apps/web/src/effect/convex.ts`: `setAuth(fetchToken)` respects Convex `{ forceRefreshToken }`, and `ConvexService.refreshAuth()` re-calls `client.setAuth(fetchToken)` so Convex exits sticky `noAuth`.
- `apps/web/src/effuse-app/boot.ts`: calls `convex.refreshAuth()` when `SessionAtom.userId` becomes authenticated.
- `apps/web/src/effuse-app/controllers/homeController.ts`: after verify, primes auth cache from the verify response token, calls `refreshConvexAuth()`, then calls `getOwnedThreadId()` without any full-page reload or localStorage coordination.
