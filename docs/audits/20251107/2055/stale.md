# Stale / Deprecated Code

## Explicit deprecation comment

- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift:313 — "Deprecated raw JSONL hydrate; prefer thread/load_latest_typed"

Action: Remove the raw JSONL hydrate path or guard behind development‑only flag with clear deprecation timeline.

## Legacy artifacts check

- TypeScript files: 0
- JavaScript files: 0
- Rust files: 0
- package.json files: 0

Result: v0.2 artifacts successfully removed per migration; good.

## Deprecated package retained by policy

- packages/tricoder/ — Present and marked deprecated in repository docs; no active build/deps. Keep until formal EOL plan executes.

## Tinyvex presence

- Tinyvex appears in docs and code (e.g., TinyvexServer.swift) as part of v0.3+ architecture; not stale.

