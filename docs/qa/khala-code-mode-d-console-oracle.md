# Khala Code Mode D Console Oracle

Date: 2026-07-02
Status: implemented for QA Q4.6 / issue #8032.

Mode D visual smokes now install one shared console oracle before page
navigation and assert it after each desktop/mobile capture. The helper lives in
`packages/khala-qa-harness/src/desktop-smoke-helpers.ts` as
`installKhalaQaConsoleErrorOracle`.

## Contract

- `console.error` fails the smoke unless it matches an explicit allowlist entry.
- Playwright `pageerror` diagnostics fail the smoke unless explicitly
  allowlisted. This covers unhandled browser exceptions and rejected promises
  surfaced by the page runtime.
- Warnings are diagnostic only by default. A smoke can opt into stricter console
  types with the helper's `consoleTypes` option.
- Allowlist entries must be local to the smoke or scenario exercising the
  expected failure and must carry a reason when declared as structured entries.

## Visual Smokes

These Mode D fixture smokes install the oracle:

- `clients/khala-code-desktop/scripts/part2-ui-recording-smoke.ts`
- `clients/khala-code-desktop/scripts/cockpit-visual-smoke.ts`
- `clients/khala-code-desktop/scripts/composer-visual-smoke.ts`
- `clients/khala-code-desktop/scripts/part2-fleet-gym-visual-smoke.ts`

The same smokes also route Khala Code `/rpc/*` calls through
`clients/khala-code-desktop/scripts/visual-smoke-rpc-mocks.ts`, which delegates
to the seed-corpus fixture fetch and handles `/rpc/events` as an SSE fixture.
Smoke-specific overrides remain only for visual states that need a custom Fleet
projection.

## Regression Class Closed

The previous script-local fallback returned hard 500 responses for newly added
boot RPCs. Those 500s could create fixture noise rather than product evidence.
Boot RPCs now use the seed fixture path by default, while unexpected browser
console/page errors fail the run loudly.

Pinned verification:

```sh
bun run --cwd clients/khala-code-desktop smoke:part2-ui
```
