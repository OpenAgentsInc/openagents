# Maestro E2E: Local Dev + Flows

## Overview
We use Maestro to drive basic, reliable E2E checks against the Expo Dev Client. Flows exercise:
- Drawer navigation and Settings visibility.
- Bridge connectivity (Dev Quick Connect).
- Streaming a simple conversation and rendering typed ACP rows.
- Component Library (renderer sanity checks).

These tests run locally during development and in CI in a simulator. They do not hit the public network; the bridge runs on your LAN.

## Prereqs
- iOS Simulator (Xcode) installed and an Expo Dev Client for this app.
- A running bridge on your desktop LAN IP (via tricoder or `cargo bridge`).
  - Example tricoder command:
    - `OPENAGENTS_REPO_DIR=/Users/you/code/openagents TRICODER_PREFER_BIN=0 bun dev`
  - Note the `WS URL` (e.g., `ws://192.168.1.11:8788/ws`) and `Token` that tricoder prints.
- Metro bundler running for the app:
  - `cd expo && bun install && bun run start`

## Environment
Dev Quick Connect uses environment variables exported into the app at runtime:
- `EXPO_PUBLIC_BRIDGE_HOST` (LAN host:port, e.g. `192.168.1.11:8788`)
- `EXPO_PUBLIC_BRIDGE_TOKEN` (bridge token printed by tricoder)

You can export these when starting Metro so the Settings Dev Quick button can pick them up.

## Flows
Flows live under `.maestro/flows/`:
- `ui_drawer_settings.yaml` — open the drawer, navigate to Settings, assert status pill renders.
- `ui_library_components.yaml` — open the drawer, navigate to Component Library, open “Agent Message” sample, assert renderer.
- `ui_library_acp_thought.yaml` — open ACP Thought sample, assert `agent-thought`.
- `ui_library_acp_tool_call.yaml` — open ACP Tool Call sample, assert `tool-call`.
- `ui_library_acp_plan.yaml` — open ACP Plan sample, assert `plan`.
- `ui_library_acp_available_commands.yaml` — open ACP Available Commands sample, assert `available-commands`.
- `ui_library_acp_current_mode.yaml` — open ACP Current Mode sample, assert `current-mode`.
- `ui_onboarding_manual.yaml` — open onboarding and assert manual link is visible.
- `bridge_local_stream.yaml` — connect via Dev Quick, assert Connected (or header dot) in Settings, start a new chat, send a prompt, wait for `user-message` and `agent-message`.
- `history_after_stream.yaml` — after streaming, open the drawer and assert the threads list renders (id `drawer-threads`).

Selectors are stabilized via explicit testIDs:
- Header: `header-menu-button`, `header-connection-indicator`, `header-new-chat`
- Settings: `settings-status`, `settings-connect`, `settings-disconnect`, `settings-dev-quick`
- Composer: `composer-input`, `composer-send`
- Drawer: `drawer-settings`, `drawer-library`, `drawer-threads`
- Library: `library-root`, `library-link-<kebab-title>`
- Streamed rows: `user-message`, `agent-message`, `tool-call`

## Running
From the repo root (with Metro and the bridge running):
- `maestro test .maestro/flows/ui_drawer_settings.yaml`
- `maestro test .maestro/flows/ui_library_components.yaml`
- `maestro test .maestro/flows/ui_library_acp_thought.yaml`
- `maestro test .maestro/flows/ui_library_acp_tool_call.yaml`
- `maestro test .maestro/flows/ui_library_acp_plan.yaml`
- `maestro test .maestro/flows/ui_library_acp_available_commands.yaml`
- `maestro test .maestro/flows/ui_library_acp_current_mode.yaml`
- `maestro test .maestro/flows/ui_onboarding_manual.yaml`
- `maestro test .maestro/flows/bridge_local_stream.yaml`
- `maestro test .maestro/flows/history_after_stream.yaml`

Tips:
- iOS Simulator cannot reach your host at `127.0.0.1`. Use your LAN IP in `EXPO_PUBLIC_BRIDGE_HOST` (e.g., `192.168.1.11:8788`).
- If deep links fail to open (`OSStatus 194`), use the Settings screen’s Dev Quick Connect button (we added it for testing).
- Increase `extendedWaitUntil` timeouts if your simulator or bridge is slow.

## Adding Tests
- Prefer id-based selectors; add `testID` props to components used by tests.
- Group related taps with a single drawer open (`header-menu-button`) to reduce flakiness.
- Keep flows short and focused (≤ 100 lines). Create separate flows per scenario.
- When asserting a list that cannot be matched by dynamic ids, add a static container id (e.g., `drawer-threads`).

## CI Notes
- The `.maestro/` folder is gitignored by default for local scratch. Committed flows live under the same directory and are explicitly added in commits.
- For CI, ensure the LAN bridge is replaced with a localhost-forwarded port via a simulator host mapping, or run a mock provider.
