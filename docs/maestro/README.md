# Maestro Test Guide

This guide summarizes the passing E2E flows, how to run them against a local bridge, and stability tips. It complements docs/testing/maestro.md, which contains longer, step-by-step instructions.

## Passing Flows (baseline)
- `.maestro/flows/ui_drawer_settings.yaml` — navigates to Settings via the drawer and asserts the status pill renders.
- `.maestro/flows/settings_toggles.yaml` — opens Settings directly and exercises `Full Rescan` (status pill remains visible).
- `.maestro/flows/ui_drawer_history_empty.yaml` — opens the drawer and asserts the History section is present (resilient whether history is empty or not).
- `.maestro/flows/bridge_connect_manual.yaml` — opens Settings, fills host/token, taps Apply and Connect if needed, then relies on the header connection indicator.
- `.maestro/flows/bridge_connect_and_stream.yaml` — same manual connect, sends a warm‑up prompt, then a main prompt and asserts the agent reply and drawer history.

Additional flows are included but may require specific conditions (dev-client routing, streaming readiness) and can be flaky if Metro or the bridge are not warmed up:
- Library flows (`ui_library_*`) require the `Component Library` drawer item to be visible. Set `EXPO_PUBLIC_ENV=development` when starting Metro to surface the dev-only link.
- Streaming flows require a reachable LAN bridge and a codex/claude provider that can stream on demand. Prefer the combined `bridge_connect_and_stream.yaml` (manual host/token) when testing streaming, and consider a warm‑up send before asserting.
 - In `bridge_connect_and_stream.yaml`, we wait for a user echo, then prefer an `agent-message`. If not visible, we fall back to `tool-call` or `agent-thought`, then retry once with a compact follow-up.

## Troubleshooting
- If `/--/settings` deep link doesn’t show the status pill, use the drawer path first: tap `header-menu-button` → `drawer-settings` → assert `settings-status`.
- If Library deep links do not render, open the base link first (`exp://localhost:8081`) and then the specific library route. Some simulators require the base dev-client to be warmed up.
- For streaming, if `agent-message` doesn’t appear, try sending a short warm‑up message and re-sending the real prompt; also increase timeouts.

## Environment
- Start tricoder/bridge and capture LAN host:port and token printed by tricoder.
- Start Metro:
  - `cd expo && EXPO_PUBLIC_BRIDGE_HOST=<lan-ip:port> EXPO_PUBLIC_BRIDGE_TOKEN=<token> bun run start`
- iOS Simulator must be running (e.g., iPhone 16). Maestro can launch a recommended device, but having it pre-open reduces variance.

## Running
- `maestro test .maestro/flows/ui_drawer_settings.yaml`
- `maestro test .maestro/flows/settings_toggles.yaml`
- `maestro test .maestro/flows/ui_drawer_history_empty.yaml`
- `maestro test .maestro/flows/bridge_connect_manual.yaml`
- (optional) `maestro test .maestro/flows/bridge_connect_and_stream.yaml`
- `maestro test .maestro/flows/ui_thread_composer.yaml`
- `maestro test .maestro/flows/bridge_header_indicator.yaml`
- Or run everything: `scripts/maestro-run-all.sh`

## Stability Notes
- Use id-based selectors and avoid raw text where possible. We added explicit `testID`s to Settings, header, composer, and ACP renderers.
- Prefer opening the base dev-client link (`exp://localhost:8081`) and then using the drawer to navigate when direct route links prove flaky.
- For Settings specifically, warming both the base route and `/--/thread/new` helps ensure the header is visible before drawer navigation.
- The “Component Library” drawer entry is dev-only. Ensure `EXPO_PUBLIC_ENV=development` if you need to run Library flows.
- Streaming assertions can be slow depending on the bridge; we increased waits. If `user-message` is delayed, asserting only `agent-message` is more robust.
- Tinyvex WS contract is snake_case (ADR‑0002). If you built older code, restart the bridge to ensure the app and server agree on field names like `thread_id`.

## ADR Compliance
- ADR 0002 — Rust → TypeScript source of truth: the app consumes TS types exported from the bridge under `expo/types/bridge/*` (generated via `ts-rs`). Settings, Tinyvex provider, and ACP renderers use those shapes; no ad-hoc `any` for WS payloads in these paths.
- ADR 0003 — Tinyvex local sync engine: Codex and Claude watchers translate provider events into ACP updates and mirror them into Tinyvex. Maestro flows exercise UI that relies on Tinyvex snapshots/updates (drawer history, settings sync status) without introducing any REST calls.

For deeper setup and more flows, see docs/testing/maestro.md.
