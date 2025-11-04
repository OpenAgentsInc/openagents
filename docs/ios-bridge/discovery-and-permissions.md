# Discovery, Permissions, and Workarounds (iOS ↔ Desktop)

This note explains how the iOS app discovers the desktop bridge, what OS permissions are required, and the current workaround when local‑network permissions (multicast) are not available.

## Discovery Options

- Bonjour/mDNS (recommended, requires entitlement)
  - The desktop advertises the service `_openagents._tcp.` in the `local.` domain.
  - The iOS app browses for `_openagents._tcp.` and resolves the desktop host/port.
  - After resolution, the app connects by WebSocket and performs a token handshake.
- Manual Connect (fallback, no entitlement required)
  - The iOS app provides a “Manual Connect” button in the Bridge status chip.
  - Enter the desktop IP (e.g., `192.168.1.10`) and port (default `9099`) to connect directly.

## Required OS Bits for Bonjour

To use Bonjour on iOS, the app needs:

- Info.plist keys
  - `NSLocalNetworkUsageDescription`: A short string explaining why the app accesses devices on the local network.
  - `NSBonjourServices`: Must include the fully qualified service name. Example: `"_openagents._tcp."`
- (Typically) the Multicast Networking capability
  - Entitlement: `com.apple.developer.networking.multicast = true`
  - Requires Team approval from Apple. Until granted, Bonjour browsing may fail to discover services.

The project injects the Info.plist keys via build settings and supports the multicast entitlement in an iOS‑only entitlements file, but the app code works without multicast by using the fallback described below.

## Feature Flag: Disable Bonjour on iOS

If the multicast entitlement isn’t available yet, the app disables Bonjour browsing by default and relies on Manual Connect.

- Flag name: `Features.multicastEnabled` (default false)
- Enable with:
  - Env var: `OPENAGENTS_ENABLE_MULTICAST=1`
  - OR UserDefaults: `enable_multicast = true`

When the flag is on, iOS will attempt Bonjour discovery and log its progress; otherwise, the Bridge status chip explicitly shows that discovery is disabled.

## Handshake + Transport (same for both modes)

- Transport: WebSocket only (no REST)
- Handshake:
  - iOS → Desktop: `{"type":"Hello","token":"<token>"}` (sent as text)
  - Desktop → iOS: `{"type":"HelloAck","token":"<token>"}` (text)
  - After HelloAck, iOS sends `threads.list.request` and expects `threads.list.response { items }`.
- After connect, the iOS sidebar populates automatically with returned threads.

## Logs + Status Chip

- Both platforms include a Bridge status chip at the top of the sidebar showing:
  - Advertising :9099 (desktop), Discovering (mobile), Connecting host:port, Handshaking, Connected host:port, or Error.
  - The most recent log line is visible on the right.
- Typical logs:
  - Desktop: `[Bridge][server] Started on ws://0.0.0.0:9099` → `Hello received; token ok` → `threads.list.request … count=N`
  - Mobile: `[Bridge][bonjour] Searching…` → `found service name=…` → `resolved host=…` → `[client] Connecting ws://…` → `Connected; sending threads.list.request` → `Received threads.list.response count=N`

## Manual Connect Instructions

If discovery is disabled or blocked:

1. Tap the chain‑plus button on the Bridge status chip.
2. Enter the desktop IP (LAN) and port `9099`.
3. Press Connect.

You should then see logs change to `Connecting` → `Connected; sending threads.list.request` and the sidebar will populate with threads from the desktop.

## Simulator Notes

- On the iOS simulator, the app automatically attempts `ws://127.0.0.1:9099` in addition to any discovery mode.
- Desktop and simulator must be running on the same machine or IP reachable from the simulator host.

## Troubleshooting

- No prompt for Local Network
  - The prompt only appears the first time the app actively browses services with `NSLocalNetworkUsageDescription` and `NSBonjourServices` present. If multicast is disabled or missing, use Manual Connect.
- No bridge found via Bonjour
  - Verify both devices are on the same LAN (not guest/VLAN) and the desktop is advertising `_openagents._tcp.`.
  - Toggle Wi‑Fi on both devices or restart the app to re‑trigger discovery.
- Direct connect fails
  - Ensure the desktop app is running and printed `Started on ws://0.0.0.0:9099`.
  - Try connecting by LAN IP (e.g., `192.168.x.x:9099`).
  - Check desktop logs for `Hello received; token ok` and `threads.list.request`.

## File Map (implementation)

- Desktop server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
- iOS client: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift`
- iOS discovery + status: `ios/OpenAgents/Bridge/BridgeManager.swift`, `ios/OpenAgents/Bridge/BonjourBrowser.swift`, `ios/OpenAgents/Bridge/BridgeStatusChip.swift`, `ios/OpenAgents/Bridge/ManualConnectSheet.swift`
- Feature flags: `ios/OpenAgents/Features.swift`
- Docs: `docs/ios-bridge/README.md`, `docs/ios-bridge/pairing.md`

