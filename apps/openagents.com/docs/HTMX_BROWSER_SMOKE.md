# HTMX Browser Smoke Suite

Status: active
Owner: `owner:openagents.com`
Issue: OA-HTMX-061 (`#2072`)

## Purpose

Run browser-level HTMX smoke checks for critical user journeys and catch regressions that handler-only tests miss.

Covered flows:

- login send-code + verify-code request lane (local/mock lane)
- chat thread create + message send
- feed shout post + zone navigation
- settings profile update
- back/forward history checks on chat and feed routes
- fragment-swap guard (`#oa-shell` marker remains stable across HTMX swaps)

## Entrypoint

```bash
apps/openagents.com/scripts/htmx_browser_smoke.sh
```

Implementation script:

- `apps/openagents.com/scripts/htmx_browser_smoke.mjs`

## Local Run (full flow including login)

1. Start control service in mock auth mode:

```bash
OA_AUTH_PROVIDER_MODE=mock cargo run --manifest-path apps/openagents.com/Cargo.toml
```

2. Run browser smoke:

```bash
BASE_URL=http://127.0.0.1:8787 \
OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW=1 \
apps/openagents.com/scripts/htmx_browser_smoke.sh
```

Default mock code is `123456` (`OA_BROWSER_SMOKE_CODE` override supported).
In local HTTP runs, login verification uses browser request-context posts because auth cookies are `Secure`.

## Staging/Production Run (token-auth lane)

```bash
BASE_URL=https://staging.openagents.com \
OA_BROWSER_SMOKE_ACCESS_TOKEN=<token> \
OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW=0 \
apps/openagents.com/scripts/htmx_browser_smoke.sh
```

Token mode validates authenticated HTMX surfaces without requiring email-code verification.

## Diagnostics Artifacts

Output directory (default):

- `apps/openagents.com/docs/reports/htmx-browser-smoke/<timestamp>/`

Artifacts include:

- `summary.json` (step-by-step status + timings)
- per-step failure screenshot(s)
- `final-failure.png` when the run aborts

Override output path with `OA_BROWSER_SMOKE_ARTIFACT_DIR`.

## Environment Variables

- `BASE_URL` or `OPENAGENTS_BASE_URL` (default: `http://127.0.0.1:8787`)
- `OA_BROWSER_SMOKE_ACCESS_TOKEN` (optional bearer token for authenticated lane)
- `OA_BROWSER_SMOKE_REQUIRE_LOGIN_FLOW` (`1|0`, default: `1` when no token; `0` when token is set)
- `OA_BROWSER_SMOKE_EMAIL` (default: `htmx-smoke@openagents.com`)
- `OA_BROWSER_SMOKE_CODE` (default: `123456`)
- `OA_BROWSER_SMOKE_TIMEOUT_MS` (default: `20000`)
- `OA_BROWSER_SMOKE_HEADLESS` (`1|0`, default: `1`)
- `OA_BROWSER_SMOKE_ARTIFACT_DIR` (optional)

## Dependencies

- Node.js
- `@playwright/test` in `apps/openagents.com` (`npm install --prefix apps/openagents.com`)
- Playwright Chromium browser binaries (`npx playwright install chromium`)
