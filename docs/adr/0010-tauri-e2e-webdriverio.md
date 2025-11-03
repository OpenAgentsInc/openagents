# ADR 0010 — Desktop (Tauri) E2E Testing with WebdriverIO

 - Date: 2025-11-03
 - Status: Accepted — Planned

## Context

ADR‑0004 adopts Maestro for iOS/Android E2E. We also ship a desktop app using Tauri that embeds our Expo web UI (`devUrl` in dev, `frontendDist` in prod). We need E2E coverage for desktop‑specific flows — e.g., Tauri sidecar autostart, WebSocket connect, drawer + settings navigation, composer/send, thread list/history, and log visibility — that Maestro does not target.

Tauri v2 provides a WebDriver‑compatible automation story ("Tauri Driver"
and related tooling) that allows controlling the Tauri window and interacting with the embedded webview using the W3C WebDriver protocol. WebdriverIO is a mature, open‑source (MIT) WebDriver client with excellent DX.

## Decision

Adopt WebdriverIO for Tauri desktop E2E tests, driven by Tauri’s WebDriver server ("tauri‑driver"). Keep Maestro for mobile (ADR‑0004). Use selectors consistent with our mobile tests (`testID`/`data-testid`) to reduce duplication.

## Rationale

- Maestro does not automate desktop WebViews; WebDriver tooling is the standard for desktop UI + embedded web content.
- WebdriverIO provides a batteries‑included runner, reporters, and framework adapters (Mocha/Jasmine/Cucumber), and integrates well with CI.
- Using the same durable anchors as mobile keeps flows stable and reduces maintenance.

## Implementation Plan

1) Project layout
   - Create `tauri/e2e/` with `wdio.conf.ts` and specs under `tauri/e2e/specs/*`.
   - Prefer Mocha + Spec reporter for simplicity.

2) Dependencies (dev)
   - `webdriverio`, `@wdio/cli`, `@wdio/local-runner`, `@wdio/mocha-framework`, `@wdio/spec-reporter`.
   - Tauri WebDriver binary ("tauri‑driver"): install via platform package or prebuilt in CI; start on a known port (e.g., `9515`).

3) Runner configuration (`wdio.conf.ts`)
   - Point to `hostname: '127.0.0.1'`, `port: 9515`, `path: '/'`.
   - Capabilities: Tauri app binary path + args (dev: use `devUrl`; prod: use `frontendDist`).
   - Timeouts tuned for app boot and first route warm‑up.

4) Minimal specs (smoke)
   - Boot the app; assert the header and composer anchors are present.
   - Verify bridge autostarts: surface `[bridge.sidecar]` in console panel; assert connected indicator.
   - Open drawer → Settings; assert `settings-root` and connect/disconnect controls present.
   - Compose + send; assert thread list/history changed.

5) Selectors
   - Reuse existing `testID` props on React Native components, and add `data-testid` on web wrappers if needed so WebDriver can locate nodes in the DOM.

6) CI
   - Job that installs the Tauri WebDriver binary, starts it on `9515`, builds the desktop app (or runs dev with `devUrl`), then runs `wdio` headless.
   - Artifacts: screenshots on failure, console logs, and the bridge log ring buffer if available.

## Running

Local development (devUrl):

- Start the driver: `tauri-driver --port 9515`.
- Run app dev: `cd tauri && bun run tauri dev` (or point config to `devUrl`).
- In a separate shell: `cd tauri && bunx wdio run e2e/wdio.conf.ts`.

Local package (frontendDist):

- Build web bundle: `cd expo && npx expo export -p web --output-dir web-dist`.
- Build Tauri app or point `frontendDist` in config.
- Start the driver and run the WDIO suite.

## Acceptance

- Desktop E2E runs locally and in CI, exercising boot, autostart/connect, navigation, send, and history assertions.
- Tests are stable across `devUrl` port changes.
- No changes to mobile E2E (Maestro) — both lanes coexist.

## Notes / References

- ADR‑0004 — Maestro E2E Testing for iOS & Android (mobile only).
- ADR‑0009 — Desktop‑Managed Bridge (Tauri).
- WebdriverIO Getting Started: https://webdriver.io/docs/gettingstarted

