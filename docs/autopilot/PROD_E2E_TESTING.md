# Production E2E Testing (Effuse Test Runner) + Auth Bypass

This document describes the **production-targeting, Effect-native E2E testing** path for `apps/web`, including a **deterministic auth bypass** that does **not** depend on email magic-code flows.

It exists so we can:

- Run browser E2E tests against a **real production URL** (e.g. `https://openagents.com`).
- Debug “works locally / fails in prod” issues without inventing ad hoc scripts.
- Guarantee we never ship “silent stall” UX regressions (user clicks Send, nothing happens).

## What’s Now Possible

- Run a **prod smoke suite** that:
  - Logs in deterministically (no inbox access needed).
  - Navigates to `/autopilot`.
  - Sends a chat message and asserts **either**:
    - an assistant response appears, **or**
    - a visible error banner appears (no silent failure).
- Correlate user reports and E2E failures to **Worker logs** quickly via `x-oa-request-id` and `oa_req=<id>`.
- Add new prod E2E tests with confidence, because auth + prelaunch gating are handled in a consistent, auditable way.

## Key Files (Source Of Truth)

- Effuse test runner:
  - `packages/effuse-test/src/cli.ts`
  - `packages/effuse-test/src/runner/runner.ts`
  - `packages/effuse-test/src/suites/apps-web.ts`
- Worker auth endpoints:
  - `apps/web/src/effuse-host/auth.ts`
  - `apps/web/src/auth/e2eAuth.ts`
- Effect auth integration:
  - `apps/web/src/effect/auth.ts`
- Convex auth:
  - `apps/web/convex/auth.config.ts`
- UX fix to prevent silent stall:
  - `apps/web/src/effuse-app/controllers/autopilotController.ts`

## Design Overview

We want deterministic prod E2E without hardcoding a user/password and without requiring an email inbox.

Constraints:

- Production auth still uses WorkOS for real users.
- E2E bypass must be:
  - gated by a secret (Cloudflare Worker env secret),
  - verifiable by Convex (no forged tokens),
  - isolated from the WorkOS cookie/session model (WorkOS `withAuth()` validation/refresh must not be involved).

Solution:

- Add a **Worker-minted RS256 JWT** for tests only.
- Store it in a dedicated cookie: `oa-e2e=<jwt>` (HttpOnly).
- Teach the Worker session endpoint (`/api/auth/session`) and the server-side `AuthService` to **accept either**:
  - WorkOS session (primary), or
  - E2E JWT cookie (fallback).
- Configure Convex to trust the E2E issuer via a **JWKS endpoint** served by the Worker.

## How It Works End-to-End

### 1) Test calls E2E login route

The browser test (same-origin) calls:

- `POST /api/auth/e2e/login`
- Header: `Authorization: Bearer <OA_E2E_BYPASS_SECRET>`
- Body: `{ seed, email?, firstName?, lastName? }`

The Worker:

- derives a stable user id from `seed` (or generates one),
- mints a JWT signed with `OA_E2E_JWT_PRIVATE_JWK`,
- sets cookies:
  - `oa-e2e=<jwt>` (auth token for Convex + server session)
  - `prelaunch_bypass=1` (so prod prelaunch mode doesn’t block `/autopilot`)

### 2) App session sees the user as authed

`GET /api/auth/session` returns:

- WorkOS session if present
- otherwise, a session derived from `oa-e2e` claims

Client-side `AuthService` uses `/api/auth/session` to cache a `{ userId, token }`.

Server-side `AuthService` uses `oa-e2e` cookie directly (SSR/Worker calls).

### 3) Convex accepts the E2E token

Convex is configured with a custom JWT provider:

- issuer: `https://openagents.com/e2e`
- jwks: `https://openagents.com/api/auth/e2e/jwks`
- alg: `RS256`

So Convex can verify the E2E JWT and `ctx.auth.getUserIdentity()` resolves to the test user.

### 4) Chat send cannot silently stall anymore

Previously, the UI could be in a race:

- user is authed
- but `OwnedThreadIdAtom` hasn’t resolved yet
- `chatId === ""`
- user clicks Send

That caused a “no-op” feel (and E2E timed out waiting for the user bubble).

Fix (in `apps/web/src/effuse-app/controllers/autopilotController.ts`):

- On Send, if `chatId` is empty:
  - if unauthed: open the inline auth panel
  - if authed: call `chat.getOwnedThreadId()` first, then send
  - on failure: set a visible error banner (no silent stall)

## API Surface

### `POST /api/auth/e2e/login` (prod-only bypass)

Purpose:

- mint an E2E JWT and set it as `oa-e2e` cookie

Auth:

- MUST include `Authorization: Bearer <OA_E2E_BYPASS_SECRET>`

Response:

- `200 { ok: true, userId, email }` + `Set-Cookie: oa-e2e=...`
- `401 Unauthorized` if secret missing/incorrect
- `404 Not found` if `OA_E2E_JWT_PRIVATE_JWK` is not set on the Worker

### `GET /api/auth/e2e/jwks`

Purpose:

- publish the public key for Convex JWT verification

Response:

- `200 { keys: [...] }`
- `404 Not found` if `OA_E2E_JWT_PRIVATE_JWK` is not set on the Worker

## Required Worker Secrets (Cloudflare)

These are **secrets** on the Cloudflare Worker (script `autopilot-web`).

- `OA_E2E_BYPASS_SECRET`
  - symmetric secret used only to authorize `POST /api/auth/e2e/login`
- `OA_E2E_JWT_PRIVATE_JWK`
  - RSA private key in JWK JSON form used to sign RS256 JWTs

### Set / rotate secrets

From `apps/web`:

```bash
cd apps/web

# Used to authenticate requests to /api/auth/e2e/login
npx wrangler secret put OA_E2E_BYPASS_SECRET --name autopilot-web

# Private RSA JWK JSON (used to sign tokens and derive JWKS public key)
npx wrangler secret put OA_E2E_JWT_PRIVATE_JWK --name autopilot-web
```

Generate a new RSA private JWK (extractable) using the `jose` dependency:

```bash
cd apps/web
node --input-type=module - <<'NODE'
import { generateKeyPair, exportJWK } from "jose"
const { privateKey } = await generateKeyPair("RS256", { extractable: true })
const jwk = await exportJWK(privateKey)
console.log(JSON.stringify(jwk))
NODE
```

After rotating secrets:

- Deploy the Worker: `cd apps/web && npm run deploy:worker`
- Convex auth config will continue to work because it pulls the public key from the Worker JWKS endpoint.

## Running Prod E2E

Safety:

- The runner refuses to run `prod` tests unless you explicitly include `--tag prod`.

Command:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod
```

Run just Autopilot prod tests:

```bash
cd apps/web
EFFUSE_TEST_E2E_BYPASS_SECRET="..." \
  npm run test:e2e -- --base-url https://openagents.com --tag prod --grep "apps-web\\.prod\\.autopilot"
```

Useful flags:

- `--watch` runs headed Chromium and starts the live viewer UI.
- `--headed` forces headed mode (non-headless).
- `--grep <regex>` filters by test id.
- `--tag <comma,separated>` filters by tags.

## Debugging a Failing Prod E2E Run

Artifacts:

- Each run writes to `output/effuse-test/<runId>/`.
- `events.jsonl` is the primary truth for “what step hung” and “what failed”.
- On browser failures, we attempt to capture:
  - `output/effuse-test/<runId>/<testId>/failure.png`
  - `output/effuse-test/<runId>/<testId>/failure.html`

Quick triage:

```bash
ls -lt output/effuse-test | head
rg -n "test\\.finished|run\\.finished|Timed out|failed" output/effuse-test/<runId>/events.jsonl | tail -n 80
```

When a test fails on prod, also pull Worker logs for the corresponding request id (see next section).

## Correlating Worker Telemetry in Prod

Every Worker response includes `x-oa-request-id`.

Every telemetry log line includes `oa_req=<id>`.

Workflow:

1) Get `x-oa-request-id` from the failing request.

2) Tail prod logs for that id:

```bash
cd apps/web
npx wrangler tail autopilot-web --format pretty --search "oa_req=<PASTE_ID>"
```

Convex errors typically contain their own request id:

```bash
cd apps/web
npx convex logs --prod --jsonl | rg "<CONVEX_REQUEST_ID>"
```

## Notes / Gotchas

- The E2E bypass cookie is **not** a WorkOS session. It exists to make prod smoke tests deterministic without relying on inboxes.
- Convex must accept the E2E token (it does via `apps/web/convex/auth.config.ts`), otherwise `ctx.auth.getUserIdentity()` will be empty and everything becomes “forbidden”.
- Prelaunch mode: prod deploy runs with `VITE_PRELAUNCH=1`; `POST /api/auth/e2e/login` sets `prelaunch_bypass=1` so tests can reach `/autopilot`.

