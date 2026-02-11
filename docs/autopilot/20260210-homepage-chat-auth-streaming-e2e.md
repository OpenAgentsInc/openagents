# 2026-02-10: Homepage-Only Chat, Auth Handshake, Streaming Finalization, E2E Updates

## Goals

- Remove `/login` and `/autopilot` UX surfaces; all user onboarding + chat runs on `/` (home chat pane).
- Fix “thread stuck on streaming” definitively (no permanent `status: "streaming"` runs/messages).
- Ensure magic-auth verify transitions to a Convex-authed state **without reload/querystring hacks** (avoid localStorage unless required).
- Update E2E coverage to match the homepage-only flow (including DSE debug cards in chat).

## Changes

- Routes: removed `/login` and `/autopilot` from the Effuse route table.
  - `apps/web/src/effuse-app/routes.ts`
- Client boot/controller wiring: removed legacy controller paths and removed identity pill tied to the old `/autopilot` surface.
  - `apps/web/src/effuse-app/boot.ts`
  - Deleted:
    - `apps/web/src/effuse-app/controllers/autopilotController.ts`
    - `apps/web/src/effuse-app/controllers/loginController.ts`
    - `apps/web/src/effuse-pages/login.ts`
    - `apps/web/src/effuse-pages/autopilotRoute.ts`
    - `apps/web/src/effuse-pages/identityPill.ts`
- Homepage chat pane:
  - Renders full chat transcript + DSE cards in-pane.
  - Includes stable `data-oa-home-*` selectors for E2E.
  - Primes session + Convex auth in-page after verify (no reload).
  - Uses typed session adapter contract (`sessionState.read/write`) instead of unsafe atom casting in controller paths.
  - Clears browser-cached home chat snapshot on explicit logout (prevents stale transcript rehydrate after sign-out).
  - Uses a single idempotent overlay teardown path; closing pane or controller cleanup now unsubscribes active home chat subscription and fully disposes overlay resources.
  - `apps/web/src/effuse-app/controllers/homeController.ts`
- Streaming finalization + stale-run sweeper:
  - See `docs/autopilot/THREAD_STUCK_STREAMING_FIX.md`.
  - Worker: `apps/web/src/effuse-host/autopilot.ts`
  - Convex: `apps/web/convex/autopilot/messages.ts`, `apps/web/convex/crons.ts`, `apps/web/convex/schema.ts`
- Legacy link cleanup:
  - Updated internal nav links that previously pointed to `/autopilot` to now link to `/`.
  - `apps/web/src/effuse-pages/*`
- E2E updates:
  - Homepage-only selectors + flows.
  - Removed `/login`-based hydration/navigation assumptions.
  - `packages/effuse-test/src/suites/apps-web.ts`

## Verification (Local)

Ran:

```bash
cd apps/web
npm run lint
npm test
npm run test:e2e
```

All green.

## Deploy

Deployed:

```bash
cd apps/web
npm run deploy
```

## Docs Updated

- `docs/autopilot/POST_SIGNUP_UNAUTHORIZED_REPORT.md` (updated to match homepage-only flow and current fix).
- `docs/autopilot/THREAD_STUCK_STREAMING_FIX.md` (new; root cause + fix).
