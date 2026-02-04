# Browser E2E Testing Plan (Playwright)

## Summary
Adopt Playwright for end-to-end browser testing in `apps/web/`, with a test-only auth bypass that still allows calls to Cloudflare-backed resources (OpenClaw API Worker, etc.). Start with a minimal smoke suite, then expand to authenticated and Cloudflare integration flows.

## Goals
- Add reliable, repeatable E2E coverage for critical web flows.
- Avoid manual WorkOS login in E2E runs, while still allowing OpenClaw/Cloudflare calls.
- Keep tests fast and deterministic; isolate flaky external dependencies.
- Provide a clear CI path with artifacts (trace, screenshots, video on failure).

## Non-Goals
- Replace unit/integration tests.
- Full coverage of every UI component on day one.
- Hard dependency on a live Nostr relay for test data.

## Recommended Tooling
- Playwright Test Runner (`@playwright/test` + `playwright`).
- Multi-project runs: Chromium (required), Firefox and WebKit optional.
- `webServer` integration to boot the app automatically.

## Environments
Local dev E2E:
- Run app with `npm run dev` (starts Vite + Convex dev).
- Use `.env.e2e` or environment overrides.

CI E2E:
- Prefer a dedicated Convex deployment or bypass access checks in E2E mode.
- Run only the Vite dev server (or `vite preview` after `npm run build`) with `VITE_CONVEX_URL` set.
- Use Playwright browser installs via `npx playwright install --with-deps`.

## Auth Strategy (with Cloudflare access)

### Option A: Real WorkOS Login
- Use real WorkOS test users and run the OAuth flow in Playwright.
- Pros: true end-to-end auth.
- Cons: slower, brittle, requires credentials in CI.

### Option B: Test-Only Auth Bypass (recommended)
Implement a small auth shim that bypasses WorkOS when `E2E_AUTH_BYPASS=1`.
- Server-side: a wrapper around `getAuth()` that returns a synthetic user when E2E mode is enabled.
- Client-side: a wrapper around `useAuth()` that mirrors the same synthetic user in E2E mode to avoid UI mismatch.
- Required env vars:
  - `E2E_AUTH_BYPASS=1`
  - `E2E_USER_ID=...`
  - `E2E_USER_EMAIL=...` (optional)
  - `E2E_ACCESS_TOKEN=...` (optional)
- Cloudflare access:
  - Set `PUBLIC_API_URL` (or `OPENCLAW_API_BASE`) to the Cloudflare API Worker.
  - Set `OA_INTERNAL_KEY` to the test internal key.
  - If `E2E_ACCESS_TOKEN` is not a real WorkOS token, allow E2E mode to skip the Convex `access.getStatus` check in `openclaw.instance` and other gated server routes.

## Test Suite Layout
Proposed structure:
- `apps/web/playwright.config.ts`
- `apps/web/e2e/`

Baseline smoke tests:
- Home page loads and primary nav links work.
- `/login` renders.
- `/authenticated` is reachable in E2E mode (or redirects when bypass disabled).
- Hatchery route loads.
- Docs/KB routes render.

Authenticated + Cloudflare tests:
- OpenClaw instance status renders and refreshes.
- OpenClaw chat sends a message and renders streamed output.
- Approvals/admin pages render if applicable.

Tagging:
- Use `@smoke` for basic tests.
- Use `@cloudflare` for tests that require `OA_INTERNAL_KEY` + `PUBLIC_API_URL`.
- Use `@auth` for tests that require the bypass or real auth.

## Flake-Reduction Strategy
- Avoid live Nostr relays in E2E tests.
- Stub or disable analytics in E2E mode.
- Use deterministic test data or stable fixtures where possible.
- Retry only for known flaky tests; prefer fixing root causes.

## CI Integration
Minimal CI job:
1. Install deps.
2. Install Playwright browsers.
3. Start the app via `webServer`.
4. Run `npm run test:e2e`.
5. Upload Playwright artifacts on failure.

Artifacts:
- `playwright-report/`
- `test-results/` (screenshots, videos, traces)

## Rollout Plan
Phase 0: Scaffolding
- Add Playwright deps and config.
- Add `test:e2e` scripts.
- Add `.env.e2e` template.

Phase 1: Smoke tests
- Home + routing + docs.

Phase 2: Auth bypass
- Implement `getAuthOrBypass` and `useAuthOrBypass`.
- Update routes/components to use wrappers.

Phase 3: Cloudflare integration
- Provide test `PUBLIC_API_URL` and `OA_INTERNAL_KEY`.
- Add OpenClaw tests with real API calls.

Phase 4: Expand coverage
- Add flows for Hatchery, communities, profiles.

## Acceptance Criteria
- `npm run test:e2e` passes locally with `E2E_AUTH_BYPASS=1`.
- A CI run can execute at least the `@smoke` suite reliably.
- At least one `@cloudflare` test runs against a real API Worker with the bypass user.

## Open Questions
- Should E2E mode skip Convex `access.getStatus` checks, or should we provision a WorkOS token and a real Convex user?
- Which OpenClaw endpoints must be validated in CI vs. run only locally?
- Do we want Firefox/WebKit in CI, or just Chromium initially?
