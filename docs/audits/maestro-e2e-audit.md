# Maestro E2E — Full Audit (Local Baseline)

- Date: 2025-11-02
- Owners: Mobile + Bridge

## Scope
- Platforms: iOS Simulator (primary), Android AVD (mirrors pending).
- Surfaces: Header (menu/connection), Drawer (History/Library), Settings, Composer, ACP/Tinyvex renderers.
- Bridge: Rust Tricoder (oa-bridge) over WebSocket (`/ws`), Tinyvex persistence and WS contract.

## What We Cover (Green Flows)
- Settings navigation and visibility via drawer anchors (`header-menu-button` → `drawer-settings` → `settings-root`).
- Settings toggles (e.g., Full Rescan), status pill presence (`settings-status`).
- Manual bridge connect (host/token), header connection indicator visible.
- Connect and stream: warm-up send, main send; assert drawer history container (`drawer-threads`) after stream.
- Disconnect flow via Settings; status pill shows Disconnected.
- Library (dev only): Component Library opens; sample ACP views render (Agent Message, Tool Call, Plan, etc.).
- Drawer row timestamp: duplicate timestamp removed when meta exists (provider badge + timestamp); provider badges render (Codex/Claude).

## What We Intentionally Don’t Hard-Assert
- Exact agent message text during streaming (timing variance). We verify history persisted instead, aligned with ADR‑0003.
- Deep-link pairing flows (optional, flaky with cold Metro). We prefer drawer paths and warmed routes.
- Android lane: planned to mirror iOS selectors; pending stabilization.

## TestIDs (Contract)
- Header: `header-menu-button`, `header-connection-indicator`, `header-new-chat`.
- Settings: `settings-root`, `settings-status`, `settings-host-input`, `settings-token-input`, `settings-apply`, `settings-connect`, `settings-disconnect`, `settings-full-rescan`, `settings-dev-quick` (dev-only auto-connect).
- Composer: `composer-input`, `composer-send`.
- Drawer: container `drawer-threads`, items `drawer-thread-<id>`; provider badges `provider-badge-codex`, `provider-badge-claude`.

## Deterministic Local Setup (Repeatable)
- Scripts: `scripts/maestro-prepare.sh` boots iOS Simulator, starts bridge and Metro, and writes `scripts/maestro.env` with `BRIDGE_HOST`, `BRIDGE_TOKEN`, and `EXP_URL`.
- Running suites:
  - Stable flows: `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-stable.sh`
  - All flows: `MAESTRO_ENV_FILE=scripts/maestro.env scripts/maestro-run-all.sh`
- Flows accept `-e key=value` from env file to avoid hardcoding IPs/tokens.

## Maestro Coverage Map
- `.maestro/flows/ui_drawer_settings.yaml` — drawer to Settings (PASS)
- `.maestro/flows/settings_toggles.yaml` — Settings toggles & pill (PASS)
- `.maestro/flows/ui_drawer_history_empty.yaml` — History container anchor (PASS)
- `.maestro/flows/bridge_connect_manual.yaml` — manual connect (PASS)
- `.maestro/flows/bridge_connect_and_stream.yaml` — connect + stream + history (PASS)
- `.maestro/flows/bridge_header_indicator.yaml` — connection indicator, with Settings fallback (flaky earlier, addressed; PASS locally)
- `.maestro/flows/bridge_disconnect.yaml` — disconnect (PASS)
- Library flows (`ui_library_*`) — dev-only demos (PASS when `EXPO_PUBLIC_ENV=development`)

## Problems Encountered & Fixes
- iOS Simulator networking to local bridge: cannot use `127.0.0.1`; must use LAN IP. Solved by Dev Quick Connect and env-driven flows.
- Settings showing as Disconnected despite host/token set: addressed by route warm-ups and an explicit Settings anchor `settings-root`.
- Host concatenation (IP repeated): sanitized host on save and on connect; regex extracts first `host:port` and strips protocol/suffix.
- Drawer timestamp duplicating: base timestamp hidden when meta exists; ThreadListItem now renders either (meta) or (base timestamp), not both.

## Pros
- Cross-platform runner with declarative flows and stable id-based selectors.
- Deterministic setup scripts reduce human steps and flakiness.
- Alignment with ADR‑0002/0003: Tinyvex-backed assertions; snake_case contract enforced.

## Cons / Risks
- Streaming content assertions remain timing-sensitive; mitigated by falling back to history assertions.
- Android parity pending; selectors should carry over but timings may differ.
- Dev-only affordances (Library, Dev Quick Connect) must be guarded to avoid CI or prod confusion.

## Gaps / Next Additions
- Android suite mirroring the iOS flows.
- CI lane on macOS runners with artifact upload.
- Stress test flows: message burst to validate Tinyvex throttling and UI rendering under load.
- Provider parity: more Claude writer/read tests (images, tool_use deltas) and watcher robustness under file rotation.

## References
- docs/maestro/*.md — How-to, troubleshooting, artifacts
- docs/adr/0004-maestro-e2e-testing.md — Decision and targets
- scripts/maestro-prepare.sh, scripts/maestro-run-*.sh — Automation

