# Khala Mobile "Loading threads" forever — root cause audit and fix

Date: 2026-07-06

## Symptom

Every signed-in launch of the Khala Code mobile app (builds 10–13) landed on
a permanent "Loading threads" spinner. Two earlier client-side fixes shipped
against this symptom did not resolve it:

1. `must_refetch` surfacing (`16031fabfa`-adjacent work): maps the sync
   session's own give-up phase to a visible error. Never fired — the session
   never reached that phase.
2. A hard client watchdog (`16031fabfa`): force-errors after 15s of
   unresolved loading. Fired, but the error flashed for ~1 second and the UI
   went straight back to "Loading threads" (see "Watchdog flip-back" below).

The login-screen skip reported alongside this was correct behavior — the
stored credential is genuinely valid (it passes a real server-side bootstrap
check on every launch per
`khala_mobile.auth.stored_credential_revalidated_on_launch.v1`).

## Root cause (server-side, affects every mobile user, all builds)

`GET /api/sync/connect` — the WebSocket live-tail route — never accepted the
bearer token the client sends.

- The client transport (`packages/khala-sync-client/src/transport.ts`
  `connectLive`) sends the bearer as a `?token=` **query parameter**, by
  documented design: WebSocket clients (browser **and** React Native) cannot
  set an `Authorization` header on the upgrade request.
- The server route's `authenticate` dependency was wired in `index.ts` as a
  closure over the raw request calling `authenticateRequestActor`, which
  reads **only** the `Authorization` header or the browser session cookie.
  Nothing anywhere read the `token` query parameter.
- Result: every authenticated (non-`scope.public.*`) WebSocket connect from
  the mobile app was refused `401 unauthenticated`, always.

Why this was invisible on web/desktop: browsers attach the session **cookie**
automatically on same-origin WebSocket upgrades, so `verifySession` inside
`authenticateRequestActor` succeeded there. The mobile app has no cookie
session — only the bearer — so it was the only surface hitting the missing
query-token path.

Why the app showed an infinite spinner rather than an error: the sync
session's `driveScope` loop (`packages/khala-sync-client/src/session.ts`)
treats a live-connect failure as retryable — bootstrap (HTTP, header auth —
works) → catch-up (HTTP, header auth — works) → `connectLive` (WS, query
token — 401) → backoff → repeat, forever. The scope phase cycles
`catching_up` and never reaches `live`; with zero already-synced items,
`useKhalaSyncScopeEntities` maps that to `loading` indefinitely.

## Proof (reproduced and verified outside the app)

Driving the REAL `createHttpKhalaSyncTransport` from Bun against production:

- Anonymous `scope.public.tokens-served`: bootstrap OK, log page OK,
  **WebSocket connected OK** (public scopes skip the 401).
- Authenticated `scope.user.<agent user id>` with a valid agent bearer:
  bootstrap OK (16 entities), log page OK, **WebSocket FAILED**.
- Raw upgrade via curl with the token in the query string returned exactly:
  `401 {"code":"unauthenticated","messageSafe":"Khala Sync connect requires
  an authenticated session or agent token."}`.

## Fix (server)

`apps/openagents.com/workers/api/src/khala-sync-connect-routes.ts`:

- New exported `withBearerFromQueryToken(request)`: promotes `?token=` into
  an `Authorization: Bearer` header **only when no Authorization header is
  already present** (an existing header always wins; empty/missing token is
  a no-op). Applied before the standard actor auth runs, so agent bearers
  and OpenAuth user bearers both authenticate through the exact same
  `authenticateRequestActor` path as every HTTP route — the promotion is
  not a parallel auth system and grants nothing by itself (a garbage token
  still resolves no actor and still 401s).
- `KhalaSyncConnectDependencies.authenticate` now receives the normalized
  request; the `index.ts` wiring authenticates against it instead of a
  closure over the raw request.
- The internal hub-forward URL still carries only `scope` + `cursor` — the
  raw token is never re-encoded onto it.

Coverage: 5 new tests in `khala-sync-connect-routes.test.ts` (promotion,
header precedence, no-op cases, end-to-end authenticate-sees-promoted-header
through the route, and garbage-token-still-401s), full suite 35/35.

## Fix (client, follow-up in the same incident)

Watchdog flip-back: `useKhalaSyncScopeEntities`'s 15s watchdog set an error
state, but every subsequent `refresh()` (triggered by the session's own
`catching_up` state churn on each retry cycle) recomputed status from phase
and overwrote the error back to `loading` — producing the observed
1-second error flash. Fixed by making the watchdog error sticky: once the
watchdog fires, later refreshes may only replace it with a real resolution
(`ready`, or a phase-derived error), never silently back to `loading`.

## Deployment / verification checklist

1. Server fix deployed to production via `deploy:safe`.
2. Bun repro re-run against production: authenticated user-scope WebSocket
   must now connect (same script that failed before the deploy).
3. On-device: relaunch the app (no update needed for the core fix — it is
   entirely server-side); the thread list must resolve. The sync session
   already retries in a loop, so even an app that is currently stuck
   recovers on its next backoff cycle without a restart.

## Lessons recorded

- The two prior fixes treated the SYMPTOM (spinner never resolving) at the
  client. Neither could have worked: the failure was a server auth gap on a
  path with silent, infinitely-retried failures. The watchdog was still
  right to add (it converts silent hangs into visible errors) but its
  flip-back bug masked it.
- Silent retry loops need error-budget visibility: `driveScope` retries
  connect failures forever with no cap and no surfaced state distinguishing
  "retrying the WS" from "loading". The watchdog now covers the UX side;
  transport-level telemetry for repeated 401/403 on connect is worth a
  follow-up (a 401 on connect is arguably non-retryable and should park the
  scope like a 403 does).
- WebSocket auth is structurally different from HTTP auth (no headers
  available). Any new WS route must be tested with query-token auth from a
  cookie-less client — a browser-only test will always pass for the wrong
  reason.
