# Maestro Troubleshooting

## Symptoms
- Flows fail on `settings-status` not visible after opening `/--/settings`.
- Drawer navigation doesn’t reveal Settings content.
- Library routes render inconsistently.

## Likely Causes
- Metro (Expo dev server) is not running or not reachable by the Simulator.
- The dev-client isn’t warmed up before deep linking to a sub-route.
- iOS Simulator networking is not aligned with your bridge host/token.

## Fixes
- Start Metro before running flows:
  - `cd expo && EXPO_PUBLIC_BRIDGE_HOST=<lan-ip:port> EXPO_PUBLIC_BRIDGE_TOKEN=<token> bun run start`
- Warm the dev-client by opening both the base link and a header-visible route before navigating:
  - `exp://localhost:8081` then `exp://localhost:8081/--/thread/new`
- Warm the dev-client with the base link first (`exp://localhost:8081`) and then navigate to specific routes.
- For Settings, prefer drawer navigation (tap `header-menu-button` → `drawer-settings`) before using the `/--/settings` route.
- For streaming, send a short warm-up prompt first; then assert on `agent-message`. Increase `extendedWaitUntil` timeouts if needed.

## Environment Checks
- Bridge: tricoder prints the `WS URL` and `Token`. Use those values in Settings host/token fields.
- iOS Simulator cannot reach `127.0.0.1` for the host. Use your LAN IP (e.g., `192.168.1.11:8788`).

## ADR Alignment Notes
- ADR 0002: The app relies on snake_case fields for Tinyvex/bridge types. Maestro flows only interact with UI—no custom WS payloads were added.
- ADR 0003: Tests depend on WS + Tinyvex only. No REST endpoints were introduced.
