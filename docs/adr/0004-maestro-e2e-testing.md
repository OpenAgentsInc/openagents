# ADR 0004 — Maestro E2E Testing for iOS & Android

 - Date: 2025-11-01
 - Status: Accepted

## Context

We need reliable, end‑to‑end (E2E) testing for the OpenAgents mobile app across iOS and Android, including critical user flows (connect to bridge, navigate Settings/Drawer, compose/send, stream, and verify history).

The app runs as an Expo Dev Client with a local Rust bridge (Tricoder) serving WebSocket updates. Historically, content streaming has timing variance and Expo dev‑client route rendering depends on Metro being warmed. We also must align with ADR‑0002 (Rust→TS types, snake_case WS contract) and ADR‑0003 (Tinyvex local sync via WS, no REST).

## Decision

Adopt Maestro as the primary E2E test runner for both iOS and Android. Target near‑100% coverage of relevant user flows by:

1) Defining id‑based selectors (`testID`) in UI components and screens (composer, header, Settings, ACP renderers).
2) Writing portable Maestro YAML flows which:
   - Warm the dev‑client (base link + header‑visible route) before asserting.
   - Prefer drawer navigation to reach Settings; fall back to deep links.
   - Assert screen presence via durable anchors (e.g., `settings-root`).
   - Verify streaming indirectly via Tinyvex history (per ADR‑0003) to avoid content timing flakiness.
3) Supplementing Maestro with Rust unit/integration tests for the Tricoder bridge and targeted CLI smoke checks under CI (spawn bridge, verify WS handshake and snake_case compliance per ADR‑0002).

## Rationale

- Maestro is fast, declarative, and works on both platforms with minimal harnessing.
- Id‑based selectors are stable across platforms and skinning.
- Tinyvex history is the source of truth for persisted/streamed rows; asserting history aligns with ADR‑0003 and avoids brittle streaming content timing.
- Keeping Tricoder logic covered by Rust tests (and minimal WS smoke checks) separates concerns and improves signal.

## Scope & Coverage Targets

- iOS (Simulator):
  - App boot and composer visible (`/thread/new`).
  - Drawer → Settings navigation and screen presence (`settings-root`), header menu visibility.
  - Manual connect (host/token) and header connection indicator.
  - Send prompt(s) and verify history (`drawer-threads`).
  - Disconnect via Settings.
- Android (AVD): mirror the above flows using the same selectors and warm‑up patterns.
- Exclusions:
  - Deep‑link pairing stress tests (optional later).
  - Rich content assertions for streaming (we prefer history confirmation for robustness).
  - Library demos in the stable lane (run only under dev guard when needed).

## Implementation Plan

1) TestIDs & anchors
   - Ensure durable `testID`s for: `header-menu-button`, `header-connection-indicator`, `settings-root`, `settings-host-input`, `settings-token-input`, `settings-apply`, `settings-connect`, `settings-disconnect`, `composer-input`, `composer-send`, `drawer-threads`.
   - Avoid duplicate UI (e.g., base timestamp hidden when `meta` is supplied).

2) Flows (examples)
   - UI Thread → Composer Visible.
   - Drawer → Settings (warm `/thread/new` → drawer → `/settings` fallback).
   - Settings Toggles (assert `settings-root`, Full Rescan).
   - Header Connection Indicator (manual connect → assert header dot).
   - Connect & Stream (manual connect → warm‑up → main send → assert history container).
   - Disconnect (manual connect → disconnect → assert pill text).

3) CI integration
   - Stable suite (iOS) with Metro auto‑started; artifacts uploaded on failure.
   - Optional Android lane enabled after iOS is stable.

4) Bridge (Rust) supplementation
   - Continue Rust unit/integration tests (`cargo test`) for Tricoder.
   - Add a CI smoke test to spawn the bridge, verify WS handshake, and check snake_case fields per ADR‑0002.

## Consequences


## Running

Recommended env-driven workflow (no hardcoded host/token):

- Start Metro with auto-connect for dev builds so the app connects on mount:
  - `cd expo && EXPO_PUBLIC_ENV=development EXPO_PUBLIC_AUTO_CONNECT=1 EXPO_PUBLIC_BRIDGE_HOST=<lan-ip:port> EXPO_PUBLIC_BRIDGE_TOKEN=<token> bun run start`
  - Use your LAN IP and current bridge token (from tricoder/bridge logs).

- Create an env file for Maestro (do not commit secrets):
  - `cp scripts/maestro.env.example scripts/maestro.env`
  - Set:
    - `BRIDGE_HOST=<lan-ip:port>`
    - `BRIDGE_TOKEN=<token>`
    - `EXP_URL=exp://<lan-ip>:<metro-port>` (e.g., `exp://192.168.1.11:8083`)

- Run flows with the env file (scripts parse it into `-e KEY=VALUE`):
  - Stable subset: `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-stable.sh`
  - Full suite: `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-all.sh`

Notes:
- Flows prefer Settings → “Dev Quick Connect” (reads `EXPO_PUBLIC_BRIDGE_HOST/TOKEN`) and fall back to `BRIDGE_HOST`/`BRIDGE_TOKEN`.
- All exp:// deep links use `${EXP_URL}` so you can point to any Metro port.
- If Settings content doesn’t render after a deep link, opening it once via the drawer typically warms the route on simulators.

## Deterministic Local Setup (Agents)

- We provide `scripts/maestro-prepare.sh` for programmatic, repeatable setup on macOS:
  - Boots the iOS Simulator (iPhone 16) if needed.
  - Starts `oa-bridge` on `0.0.0.0:$BRIDGE_PORT` with `OPENAGENTS_BRIDGE_TOKEN`.
  - Starts Metro on `$METRO_PORT` with `EXPO_PUBLIC_AUTO_CONNECT=1` and writes `scripts/maestro.env`.
  - Warms key app routes in the Simulator via `xcrun simctl openurl`.
  - Defaults: `BRIDGE_PORT=8788`, `METRO_PORT=8083`, `BRIDGE_TOKEN=test-maestro-token`.

- One command to prepare + run:
  - `scripts/maestro-prepare.sh`
  - `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-stable.sh`
  - Or: `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-all.sh`

- Environment knobs (export before running if you need overrides):
  - `DEVICE` (default `iPhone 16`)
  - `BRIDGE_PORT`, `BRIDGE_TOKEN`, `METRO_PORT` (defaults above)
  - `BRIDGE_HOST` (default `127.0.0.1:$BRIDGE_PORT`)

- E2E coverage becomes portable and maintainable, with reliable assertions based on durable anchors and Tinyvex history.
- Tests are resilient to typical dev‑client timing issues by warming routes and using fallbacks.
- Some rich content checks are intentionally replaced by history assertions to remain stable across providers/environments.

## Risks & Mitigations

- Dev‑client routing flakiness → Warm base + `/thread/new` and prefer drawer; assert `settings-root`.
- Streaming timing variance → Validate via history (`drawer-threads`) rather than specific render timing.
- CI simulator variability → Increase waits for route loads; collect artifacts for triage.

## Alternatives Considered

- Detox/Appium — more setup, slower iteration; Maestro’s declarative flows better match our needs and DX.
- In‑app integration tests — useful but do not exercise OS‑level navigation and dev‑client boot paths.

## Compliance with ADR‑0002 and ADR‑0003

- ADR‑0002: Maestro flows verify behavior that depends on the snake_case WS contract; the app and bridge already align (types exported via ts‑rs for the app; server emits snake_case and accepts legacy input fields).
- ADR‑0003: No REST calls are introduced for tests; Tinyvex WS events power history assertions, matching our local sync engine decision.

## Acceptance Criteria

- Stable suite passes on iOS with Metro warmed.
- Streaming flow passes by asserting Tinyvex history after send.
- Disconnect and header indicator flows pass reliably with current fallbacks.
- Android suite added using identical selectors and warm‑ups.
- CI job runs the stable suite, collects artifacts on failure, and provides clear guidance for debugging.

## References

- ADR 0002 — Rust → TypeScript Types as Single Source of Truth.
- ADR 0003 — Tinyvex as the Local Sync Engine.
- docs/maestro/README.md, docs/maestro/troubleshooting.md, docs/maestro/artifacts.md.
