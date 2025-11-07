# Discovery — Bonjour (LAN)

Service

- Type: `_openagents._tcp.` (per ADR‑0004)
- Name: `Tinyvex on <hostname>`
- TXT records: `{ version: "<semver>", tinyvex: "true" }`

Client Flow (iOS)

- Browse for `_openagents._tcp.`
- Resolve service and connect to reported host/port over WebSocket.
- Perform `tinyvex/connect` handshake immediately.

Server Flow (macOS)

- Publish the service on startup when the WebSocket server is listening.
- Withdraw when shutting down or if the port is no longer available.

Security Notes

- For MVP, discovery is unauthenticated. Pairing/TLS can be added later; the handshake supports `auth.setToken` already.

