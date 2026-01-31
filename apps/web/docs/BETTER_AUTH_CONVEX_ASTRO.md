# Better Auth + Convex: Astro instead of Next.js

Based on the **@convex-dev/better-auth** source (`/Users/christopherdavid/code/better-auth/`). The Next.js guide is framework-specific in a few places; here’s what to do for **Astro** instead.

---

## 1. What’s the same (any framework)

- **Convex side:** `convex/auth.config.ts`, Better Auth component (convex.config, auth instance, schema, adapter), `convex/http.ts` registering auth routes — **unchanged**. Framework-agnostic.
- **Auth client plugin:** `createAuthClient({ plugins: [convexClient()] })` from `@convex-dev/better-auth/client/plugins` — **unchanged**.
- **Env in Convex:** `BETTER_AUTH_SECRET`, `SITE_URL` set via `npx convex env set` — **unchanged**.

---

## 2. What’s different: no Next.js proxy or Next.js helpers

### Next.js does two things we don’t have in Astro

1. **Proxy**  
   Next.js route `app/api/auth/[...all]/route.ts` forwards every `/api/auth/*` request to **Convex’s HTTP URL** (CONVEX_SITE_URL).  
   Implementation in source (`src/nextjs/index.ts`):

   ```ts
   const handler = (request: Request, siteUrl: string) => {
     const requestUrl = new URL(request.url);
     const nextUrl = `${siteUrl}${requestUrl.pathname}${requestUrl.search}`;
     const newRequest = new Request(nextUrl, request);
     newRequest.headers.set("accept-encoding", "application/json");
     newRequest.headers.set("host", new URL(siteUrl).host);
     return fetch(newRequest, { method: request.method, redirect: "manual" });
   };
   ```

2. **Server helpers**  
   `convexBetterAuthNextJs()` uses **Next.js `headers()`** to read cookies and call `getToken(siteUrl, headers, opts)`. That token is then used for `preloadAuthQuery`, `fetchAuthMutation`, etc. So SSR auth and “run Convex with auth” are tied to Next.js.

---

## 3. Astro option A: Direct to Convex (like the React example)

The **React** example (no Next.js) does **not** use a proxy. The auth client talks **directly** to Convex:

```ts
// examples/react/src/lib/auth-client.ts
export const authClient = createAuthClient({
  baseURL: import.meta.env.VITE_CONVEX_SITE_URL,
  plugins: [/* ... */ convexClient()],
});
```

So for **Astro**:

- Set **baseURL** to your **Convex site URL** (e.g. `https://<deployment>.convex.site`).
- Expose that in the browser, e.g. `PUBLIC_CONVEX_SITE_URL` in Astro env (or `CONVEX_SITE_URL` with `access: "public"`).
- **No** `/api/auth/*` route in Astro; no proxy.
- Auth runs entirely against Convex’s HTTP endpoint; CORS must allow your Astro origin (Convex handles this when the auth component is configured with your site).

**Auth client in Astro (e.g. `src/lib/auth-client.ts`):**

```ts
import { createAuthClient } from "better-auth/react";
import { convexClient } from "@convex-dev/better-auth/client/plugins";

export const authClient = createAuthClient({
  baseURL: import.meta.env.PUBLIC_CONVEX_SITE_URL,
  plugins: [convexClient()],
});
```

Use `authClient` in client-side scripts or React components (e.g. sign-in, sign-up). For “current user” use Convex as usual, e.g. `useQuery(api.auth.getCurrentUser)` in a React component wrapped with `ConvexBetterAuthProvider` (see below).

**ConvexProvider:** Replace plain `ConvexProvider` with `ConvexBetterAuthProvider` (same as Next.js guide), and pass `authClient`. You can omit `initialToken` if you don’t have SSR token (client will resolve auth via Convex).

---

## 4. Astro option B: Proxy (like Next.js, same-origin cookies)

If you want **same-origin** auth (browser hits your domain, cookies on your domain), add a **proxy** in Astro:

- Add a server (e.g. **Cloudflare adapter**) so Astro can handle a server route.
- Add a single route that forwards **all** `/api/auth/*` to Convex, using the same logic as Next.js:

  - Incoming: `Request` to e.g. `https://yoursite.com/api/auth/signin/email`.
  - Outgoing: `fetch(CONVEX_SITE_URL + pathname + search, { method, body, headers, redirect: "manual" })`, with `accept-encoding: application/json` and `host: new URL(CONVEX_SITE_URL).host`.
  - Return that `Response` (including cookies Convex sends, if you forward them).

- In Astro env, set **baseURL** for the auth client to **your site** (e.g. `PUBLIC_SITE_URL` or `import.meta.env.SITE_URL`), so the client calls `yoursite.com/api/auth/*` and your server proxies to Convex.

Then the **auth client** uses your site as baseURL:

```ts
export const authClient = createAuthClient({
  baseURL: import.meta.env.PUBLIC_SITE_URL, // or SITE_URL
  plugins: [convexClient()],
});
```

No other Convex or Better Auth config changes.

---

## 5. SSR / getToken in Astro (optional)

Next.js helpers (`getToken`, `preloadAuthQuery`, `fetchAuthMutation`, …) rely on **Next.js `headers()`**. In Astro you don’t have that; you have the **request** in server endpoints or middleware.

The **getToken** logic lives in the package **utils** (`src/utils/index.ts`). The package exports `./utils`, so you can do:

```ts
import { getToken } from "@convex-dev/better-auth/utils";
```

Signature:

```ts
getToken(siteUrl: string, headers: Headers, opts?: GetTokenOptions) => Promise<{ token?: string; isFresh: boolean }>
```

So in **Astro server context** (e.g. in a server route or middleware where you have the request):

- Call `getToken(CONVEX_SITE_URL, request.headers, opts)`.
- Use the returned `token` to call Convex (e.g. `fetchQuery`/`fetchMutation` with that token), or to pass `initialToken` into `ConvexBetterAuthProvider` if you render that on the server.

You don’t get `preloadAuthQuery` / `usePreloadedAuthQuery` unless you reimplement them (they use Next.js `preloadQuery` and React cache). For Astro you can either:

- Skip SSR auth and use **client-only** Convex auth (e.g. `useQuery(api.auth.getCurrentUser)`), or  
- Use `getToken` + Convex `fetchQuery`/`fetchMutation` in Astro server code and pass data as props (no preload hook).

---

## 6. Summary

| Piece | Next.js | Astro |
|-------|--------|--------|
| Convex auth (auth.config, component, http) | Same | **Same** |
| Auth client | `createAuthClient` + `convexClient()` | **Same** |
| baseURL | Often app URL (proxy) | **Option A:** CONVEX_SITE_URL (direct). **Option B:** SITE_URL (proxy) |
| /api/auth proxy | Next route forwards to CONVEX_SITE_URL | **Option A:** none. **Option B:** implement same forward in Astro server route |
| getToken / SSR | `convexBetterAuthNextJs()` + next/headers | Use `getToken(siteUrl, request.headers)` from `@convex-dev/better-auth/utils` where you have `request`; no Next.js helpers |
| ConvexProvider | ConvexBetterAuthProvider + initialToken from getToken | ConvexBetterAuthProvider; initialToken only if you implement getToken and pass it |

**Recommended for Astro:** Start with **Option A** (direct to Convex, like the React example): set `PUBLIC_CONVEX_SITE_URL`, auth client `baseURL` to that, no proxy. Add Option B (proxy + same-origin) and/or SSR getToken only if you need them.
