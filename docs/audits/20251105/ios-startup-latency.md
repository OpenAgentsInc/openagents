## Summary
- Launching the native iOS app often shows a dark screen for ~10–15 seconds before it connects to the WebSocket server. This delay is primarily caused by the client’s WebSocket handshake timeout (10s) combined with discovery/connection fallbacks and backoff.
- The UI does render a minimalist loading state on a dark background; on iPhone this can read as a “black screen” until the first successful connection or initial data loads.

## Repro (Typical)
- Launch the iOS app when the desktop bridge is not immediately reachable at the configured default host/port or when the wrong service is listening.
- Observe ~10–15s before status changes and content appears.

## What Happens At Launch
- App entry sets up fonts and immediately starts the bridge client:
  - `ios/OpenAgents/OpenAgentsApp.swift:24` uses `.task { bridge.start() }` to kick off connection.
  - The main view shows a loading placeholder until a timeline is populated: `ios/OpenAgents/AcpThreadView.swift:68` and `ios/OpenAgents/AcpThreadView.swift:655`.
- On iOS, the bridge manager attempts a direct connect to a default host right away; optional Bonjour discovery can also run:
  - Default connect decision and call: `ios/OpenAgents/Bridge/BridgeManager.swift:71`–`99`.
  - Multicast discovery (off by default) starts in parallel if enabled: `ios/OpenAgents/Bridge/BridgeManager.swift:101`–`119`; the service resolution itself allows up to 5s per service: `ios/OpenAgents/Bridge/BonjourBrowser.swift:25`.
- Default host/port constants:
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeConfig.swift:5` (port `9099`), `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeConfig.swift:9` (host `192.168.1.11`).

## Where The Time Goes
1) WebSocket handshake timeout (10 seconds)
- The mobile client starts a `URLSessionWebSocketTask` and immediately sends ACP JSON‑RPC `initialize`, then waits for a response with a fixed handshake timeout:
  - Handshake timeout value: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:35`.
  - Timer started: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:160`–`166`.
  - On timeout: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:167`–`173`.
- If the default host is stale/unreachable, or a non‑ACP server answers without a JSON‑RPC initialize response, the client waits the full 10s before failing this attempt.

2) Reconnect backoff (≥1 second)
- After a failed connect/handshake, the client schedules a reconnect with exponential backoff (1s, 2s, 4s … capped):
  - Reconnect scheduling: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:175`–`189`.
  - Backoff formula: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:196`–`199`.
- In practice this adds at least 1s between the first failed handshake and the next attempt.

3) Bonjour discovery resolution (5 seconds, optional)
- If multicast discovery is enabled, `NetService.resolve(withTimeout: 5.0)` adds up to 5 seconds to resolve service addresses:
  - `ios/OpenAgents/Bridge/BonjourBrowser.swift:25`.
- Discovery runs in parallel with the initial direct connect. When the default host is wrong and discovery is enabled, the additive effect of a 10s handshake timeout plus up‑to‑5s resolution is consistent with ~15s before a successful connection.

4) Perception: dark UI with minimal chrome
- While connecting, `AcpThreadView` shows a `ProgressView` and a short status string on a dark background:
  - Loading state: `ios/OpenAgents/AcpThreadView.swift:600`–`614` and status text mapping `ios/OpenAgents/AcpThreadView.swift:633`–`644`.
- On phone, the small spinner + dark theme can read as a “black screen” until either handshake succeeds or the next attempt begins.

## Evidence That Successful Connects Are Fast
- When the macOS bridge is reachable, the server immediately responds to JSON‑RPC `initialize` and the client transitions into the main receive loop without delay:
  - Server handshake handling, ACP path: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift:214`–`239`.
  - Client handshake success path: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:246`–`254` and `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:255`–`262`.

## Contributing Factors
- Stale default target: The initial iOS connect always dials the default host/port (simulator uses `127.0.0.1`, device uses `BridgeConfig.defaultHost`). If that host isn’t running the bridge, the 10s handshake gate is hit before the first retry.
- Strict ACP handshake: The client only treats a connection as “up” after a valid JSON‑RPC `initialize` response. Non‑ACP servers (e.g., older or different WS endpoints) cause the full 10s wait per attempt.
- Discovery defaults to off: Bonjour discovery must be enabled via `Features.multicastEnabled` (`OPENAGENTS_ENABLE_MULTICAST=1` or UserDefaults), so many launches rely solely on the default host path.

## Recommendations
- Cut initial handshake timeout on iOS
  - Reduce `handshakeTimeout` from 10s → 3s for the first attempt. Keep exponential backoff for subsequent attempts but fail fast initially to avoid long black‑screen impressions.
  - File to adjust: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift:35`.

- Happy‑eyeballs connection strategy
  - Attempt multiple candidates concurrently and race the winner: last‑successful host, simulator `127.0.0.1`, `BridgeConfig.defaultHost`, and Bonjour discoveries (if enabled). Cancel the slower tasks once one succeeds.
  - Persist last successful host/port in `UserDefaults` and prefer it on next launch.

- Tighten reconnect cadence
  - Keep `initialRetryDelay = 1.0` but bound total time to first two attempts within ~5–6s when no endpoint responds.
  - Optionally add a hard cap on total “connect window” at startup and surface a visible manual connect CTA immediately after.

- Improve connecting UX on iPhone
  - Promote a prominent “Connecting to bridge…” banner or `BridgeStatusChip` on iOS (currently only in the macOS sidebar: `ios/OpenAgents/HistorySidebar.swift:12` and `ios/OpenAgents/Bridge/BridgeStatusChip.swift:67`).
  - Offer an upfront “Connect manually” action from the initial state, using `ManualConnectSheet` (`ios/OpenAgents/Bridge/ManualConnectSheet.swift:7`).

- Optional: adjust URLSession behavior
  - Use a custom `URLSessionConfiguration` (shorter `timeoutIntervalForRequest`, `waitsForConnectivity = false`) for the WebSocket client to fail quickly on unreachable networks.

## Acceptance Checks
- With the default host unreachable and multicast off, app should fail the first attempt within ~3s and either connect on a subsequent fast attempt or surface the manual connect UI promptly.
- With multicast on and a discoverable bridge present, discovery should succeed within ~1–2s and the successful connect should cancel the slower, stale default attempt.
- Users should see an obvious connecting indicator, not an indistinguishable dark screen.

## Notes
- This audit ignores the Expo app per request and focuses solely on the native iOS target.
