# Autopilot MVP User Stories

This is the contract for what the Autopilot MVP must do end-to-end.

Hard constraints:
- Single Cloudflare Worker host (SSR + `/api/*`)
- Convex is the canonical product DB + realtime stream
- Anonymous-first flow must work (no auth required to chat)
- No containers / no sandboxes / no local executors

For the simplified spec, see `docs/autopilot/spec.md`.

## How To Run Verification

- Worker/unit tests: `cd apps/web && npm test`
- Browser/integration (Effuse Test Runner): `cd apps/web && npm run test:e2e`
- Typecheck + lint: `cd apps/web && npm run lint`

## MVP-001: Marketing Entry Works (SSR Home + CTAs)

As a visitor, I can open `/` and see the product entrypoint with clear next actions.

Acceptance:
- `GET /` returns `200` and `content-type: text/html`
- SSR shell/outlet markers exist (`data-effuse-shell`, `data-effuse-outlet`)
- Primary CTA exists: “Start for free” navigates to `/autopilot`
- Header contains a “Log in” link to `/login`

Test coverage:
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.http.ssr-home`)
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.navigation.start-for-free`)
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.hydration.strict-no-swap` verifies `/login` link + navigation)

## MVP-002: Login Route Renders (And Redirects When Authed)

As a visitor, I can open `/login` and see the login page.
As an authenticated user, opening `/login` redirects me into `/autopilot`.

Acceptance:
- Anonymous: `GET /login` returns `200` with the login form
- Authenticated: `GET /login` returns a redirect to `/autopilot`

Test coverage:
- `apps/web/tests/worker/routes.test.ts` (`/login` anon render, authed redirect)
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.hydration.strict-no-swap` asserts login UI in browser)

## MVP-003: Autopilot Route Works For Anonymous Users

As a visitor, I can open `/autopilot` without logging in and see the chat UI shell.

Acceptance:
- Anonymous: `GET /autopilot` returns `200 text/html`
- SSR contains `data-autopilot-shell`

Test coverage:
- `apps/web/tests/worker/routes.test.ts` (`/autopilot` anon SSR)

## MVP-004: Router/Hydration Contracts Hold

As a user, navigation should be reliable and not glitch inputs due to full DOM replacement.

Acceptance:
- Strict hydration does not swap on initial load (`swapCount == 0`)
- Navigating between routes swaps outlet (not shell) and preserves shell identity
- Back/forward navigation works without losing shell identity

Test coverage:
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.hydration.strict-no-swap`)
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.navigation.back-forward`)

## MVP-005: Static Assets Are Served (No SSR Fallthrough)

As a user, the app’s client bundle loads and missing assets return a real 404.

Acceptance:
- `/effuse-client.css` and `/effuse-client.js` return `200` with expected content types
- Missing assets return `404` (and do not return SSR HTML)

Test coverage:
- `apps/web/tests/worker/assets.test.ts`
- `packages/effuse-test/src/suites/apps-web.ts` (`apps-web.http.assets`)

## MVP-006: Contract Surfaces Are Fetchable (No-Store)

As a developer, I can fetch the current tool/signature/module contract JSON from the Worker.

Acceptance:
- `GET /api/contracts/tools|signatures|modules` returns `200` JSON
- Endpoints are `cache-control: no-store`
- Non-GET methods are rejected

Test coverage:
- `apps/web/tests/worker/contracts.test.ts`

## MVP-007: Route Guards Behave (Anon vs Authed)

As a visitor, I can’t open contract pages without auth.
As an authenticated user, I can open them.

Acceptance:
- Anonymous: `/tools`, `/modules`, `/signatures` redirect away
- Authed: `/tools` renders
- Legacy `/chat/:id` redirects to `/autopilot`

Test coverage:
- `apps/web/tests/worker/routes.test.ts`

## MVP-008: Convex Canonical Thread Exists For Anonymous Users

As an anonymous user, my thread exists in Convex and is seeded with a welcome message and a Blueprint row.

Acceptance:
- `ensureAnonThread` creates:
  - `threads` row with `(threadId, anonKey)`
  - `blueprints` row (default state)
  - a welcome assistant message
- Repeated calls are idempotent
- A mismatched `anonKey` is forbidden

Test coverage:
- `apps/web/tests/convex/autopilot-mvp.test.ts` (`ensureAnonThreadImpl`)

## MVP-009: Authed Thread Exists (Default Thread Per User)

As an authenticated user, I have a default thread that is owned by me.

Acceptance:
- `ensureOwnedThread` creates/uses a `users.defaultThreadId`
- The thread is owned by the authenticated `subject`
- Forbidden if the stored `defaultThreadId` points at a thread owned by someone else

Test coverage:
- `apps/web/tests/convex/autopilot-mvp.test.ts` (`ensureOwnedThreadImpl`)

## MVP-010: Anon To Owned Migration (Claim Thread On Auth)

As an authenticated user, I can claim my existing anonymous thread and keep the transcript.

Acceptance:
- `claimAnonThread` requires auth + correct `(threadId, anonKey)`
- After claim:
  - thread has `ownerId = subject`
  - thread’s `anonKey` is cleared
  - `users.defaultThreadId = threadId`
- Forbidden for other users

Test coverage:
- `apps/web/tests/convex/autopilot-mvp.test.ts` (`claimAnonThreadImpl`)

## MVP-011: Worker Streaming Endpoint Writes Chunked Deltas

As a user, sending a message streams assistant output and persists it in Convex in bounded batches.

Acceptance:
- `POST /api/autopilot/send` returns `200` and a `runId`
- Worker writes message parts in batches (not per token)
- Worker finalizes the run with a terminal status

Test coverage:
- `apps/web/tests/worker/chat-streaming-convex.test.ts`

## MVP-012: Canceling A Run Works (Best Effort)

As a user, I can cancel an in-flight run and the cancellation is persisted.

Acceptance:
- `POST /api/autopilot/cancel` returns `200`
- Convex run state reflects cancel requested
- Worker finalizes as `canceled` when cancel requested mid-stream

Test coverage:
- `apps/web/tests/worker/chat-streaming-convex.test.ts`
- `apps/web/tests/convex/autopilot-mvp.test.ts` (`requestCancelImpl`, `isCancelRequestedImpl`)

## MVP-013: Reset Clears The Thread (And Reseeds Welcome + Blueprint)

As a user, I can reset my conversation and get a clean slate without orphaned runs/parts/receipts.

Acceptance:
- Reset deletes messages, messageParts, runs, receipts for the thread
- Reset sets Blueprint back to default
- Reset re-seeds the welcome assistant message

Test coverage:
- `apps/web/tests/convex/autopilot-mvp.test.ts` (`resetThreadImpl`)

