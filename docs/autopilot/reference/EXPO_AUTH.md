# Expo App Auth (Email Magic-Link)

This document describes how the **Expo (React Native) app** authenticates with the same backend as the web app: email magic-link only, with a **token in the verify response** so the mobile client can authenticate with Convex.

## Overview

- **Login flow:** Email → send code → enter code → verify. No SSO in the Expo app.
- **Worker:** When the Expo client calls `/api/auth/verify` with `X-Client: openagents-expo`, the response includes a **`token`** (WorkOS access token / JWT) so the app can store it and pass it to Convex.
- **Convex:** The Expo app uses `ConvexProviderWithAuth` and supplies that token via `useConvexAuth`; Convex accepts the same WorkOS JWT as the web app.

## Key Files

| Area | Path |
|------|------|
| Worker verify (return token for Expo) | `apps/web/src/effuse-host/auth.ts` — `handleVerify` |
| Magic verify result (include accessToken) | `apps/web/src/auth/workosAuth.ts` — `verifyMagicAuthCode` |
| Expo login UI | `apps/mobile/app/screens/LoginScreen.tsx` |
| Expo auth context | `apps/mobile/app/context/AuthContext.tsx` |
| Expo Convex auth hook | `apps/mobile/app/convex/useConvexAuth.ts` |
| Expo auth API client | `apps/mobile/app/services/authApi.ts` |

## Worker Behavior

- **`POST /api/auth/start`** — Same as web: accepts `{ email }`, sends magic code via WorkOS.
- **`POST /api/auth/verify`** — Accepts `{ email, code }`. If the request includes **`X-Client: openagents-expo`**, the JSON response includes **`token`** (WorkOS access token) in addition to `ok` and `userId`. The cookie is still set for web compatibility; the Expo app ignores cookies and uses the token.

Without the token in the response, the Expo app would have no JWT to send to Convex, so `ensureOwnedThread` and the Feed would fail with unauthorized.

## Deploying the Worker

After changing auth (or any Worker code), deploy from `apps/web`:

```bash
cd apps/web
npm run deploy:worker
```

This runs `wrangler deploy` and publishes the Worker to Cloudflare (e.g. `openagents.com/api/auth/*`, `autopilot-web.openagents.workers.dev`). Full app deploy (Convex + build + worker) is:

```bash
npm run deploy
```

## Expo Config

- **Auth API base URL:** `config.base.ts` → `authApiUrl` (default `https://openagents.com`). Override with env if pointing at a different host.
- **Convex URL:** `config.base.ts` → `convexUrl`. For local dev, set `EXPO_PUBLIC_CONVEX_URL` to the same value as `VITE_CONVEX_URL` in `apps/web/.env.local` so the Expo app talks to the same Convex deployment.

## See Also

- [PROD_E2E_TESTING.md](./PROD_E2E_TESTING.md) — Production E2E and auth bypass (web).
- [login-only-removal-of-anon.md](./login-only-removal-of-anon.md) — Web auth context (no anon).
