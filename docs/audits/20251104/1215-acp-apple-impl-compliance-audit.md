# Audit — ACP Compliance for macOS/iOS (Apple‑Native Surfaces)

- Date: 2025‑11‑04
- Scope: Apple‑native Swift app targets under `ios/` (macOS + iOS), the Apple‑native WebSocket pairing bridge, and their interaction with the Rust `oa-bridge` and ACP.
- Goal: Assess how closely current implementations adhere to the Agent Client Protocol (ACP), identify deviations, and outline a concrete path to full compliance.

## Summary

The Apple‑native surfaces implement ACP concepts in local data models and UI renderers, and partially translate provider outputs into ACP‑like events for read‑only history. However, on‑wire protocol usage is not ACP: the macOS/iOS bridge currently speaks a custom WebSocket envelope with a minimal handshake (`Hello`/`HelloAck`) and an application‑specific message (`threads.list.request/response`). JSON‑RPC method names and lifecycles defined by ACP (e.g., `initialize`, `session/new`, `session/prompt`, `session/update`) are not yet present on Apple‑native transport. Tool permission requests, file system/terminal methods, and capability negotiation are also not implemented on Apple platforms.

Conclusion: The Swift app is ACP‑aligned in data shape for rendering and translation, but not ACP‑compliant on the wire. Full compliance requires adopting ACP’s JSON‑RPC transport (or faithfully bridging to the Rust `oa-bridge` that already implements ACP/Tinyvex), implementing session lifecycle and streaming updates, and supporting client‑side request handlers defined in ACP.

## Sources Reviewed

- ACP Rust SDK (reference types and method names)
  - `/Users/christopherdavid/code/agent-client-protocol/rust/acp.rs`
  - `/Users/christopherdavid/code/agent-client-protocol/rust/agent.rs`
  - `/Users/christopherdavid/code/agent-client-protocol/rust/client.rs`
- OpenAgents ADRs
  - `docs/adr/0007-agent-client-protocol.md`
  - `docs/adr/0009-desktop-bridge-management.md`
  - `docs/adr/0011-swift-cross-platform-app-experiment.md`
  - `docs/adr/0014-ios-desktop-websocket-bridge-and-pairing.md`
- Apple‑native implementation highlights
  - Swift ACP models: `ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/*`
  - Apple WS server: `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
  - Apple WS client: `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift`
  - Bridge envelopes: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeMessages.swift`
  - iOS bridge manager + Bonjour: `ios/OpenAgents/Bridge/*.swift`
  - Provider→ACP translator (Swift): `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift`

## Current State (Apple‑Native)

- Transport
  - macOS runs `DesktopWebSocketServer` on `ws://0.0.0.0:9099` with Bonjour (`_openagents._tcp`) and a simple token handshake `Hello`/`HelloAck`.
  - iOS discovers via Bonjour or connects manually; speaks custom envelopes with `type` + `data` and today supports `threads.list.request/response`.
  - No ACP JSON‑RPC methods on the Apple‑native socket; no `initialize`, `session/new|load|set_mode|prompt|cancel`, no `session/update` streaming.

- Data models and UI
  - Swift defines minimal ACP‑like models (`ACPMessage`, `ACPContentPart.text`, `ACPToolCall`, `ACPToolResult`, `ACPPlanState`, `ACPThread`) for rendering timelines.
  - A Swift translator maps provider JSONL (Codex) into these ACP‑like events for local history views.
  - Live session sending is stubbed (composer appends locally); no ACP prompt turn lifecycle.

- Bridge integration
  - ADR‑0007 makes ACP the canonical on‑wire contract; ADR‑0011/0014 acknowledge the Apple bridge as an experiment/pairing path.
  - Rust `oa-bridge` remains the single source of truth for `/ws` and typed Tinyvex updates, but the Apple‑native surfaces currently do not consume ACP or Tinyvex over that socket.

## ACP Compliance Matrix (key areas)

- Protocol & Transport (JSON‑RPC, bidirectional)
  - Status: Not implemented on Apple‑native socket. Custom envelopes are used instead of ACP JSON‑RPC.

- Initialization & Capabilities
  - Methods: `initialize`, optional `authenticate`
  - Status: Not implemented. No capability exchange (`client_capabilities`, `agent_capabilities`) between iOS/macOS and an agent/bridge.

- Sessions
  - Methods: `session/new`, `session/load` (optional), `session/set_mode`, `session/set_model` (unstable)
  - Status: Not implemented. No session identifiers negotiated or tracked via ACP.

- Prompt Turn
  - Methods/Notifications: `session/prompt` (request), `session/update` (streamed), `session/cancel` (notification)
  - Update variants expected on the client: `UserMessageChunk`, `AgentMessageChunk`, `AgentThoughtChunk`, `ToolCall`, `ToolCallUpdate`, `Plan`, `AvailableCommandsUpdate`, `CurrentModeUpdate`.
  - Status: Not implemented on the wire. Swift models cover a subset for rendering (messages, reasoning as text, tool calls/results, plan state), but they don’t arrive via `session/update`.

- Client‑side Services (handled by the client per ACP)
  - File system: `fs/read_text_file`, `fs/write_text_file`
  - Terminals: `terminal/create|output|release|wait_for_exit|kill`
  - Permission: `session/request_permission`
  - Status: Not implemented. No client method handlers exist in iOS/macOS for ACP requests. iOS would largely be restricted; macOS can support a subset with user approval.

- MCP
  - Configurable MCP servers (stdio/http/sse) passed to agents via session setup.
  - Status: Not implemented in Swift flows; today provider CLIs are spawned directly (experiment) or proxied by the Rust bridge.

- Extensibility
  - `ext` methods/notifications
  - Status: Not implemented on Apple‑native socket.

## Deviations and Risks

- Custom transport divergence
  - The Apple‑native socket and envelopes deviate from ACP’s JSON‑RPC shape, creating a parallel protocol. Extending it risks fragmentation contrary to ADR‑0007.

- Partial data modeling
  - Swift ACP models are a minimal subset. Missing content variants (e.g., `ResourceLink`/`Resource`, `Image`, `Audio`) and several `SessionUpdate` kinds limit parity and testability.

- No permission model
  - `session/request_permission` and remembered choices are central to safe operations; the Apple surfaces lack this path and UI.

- No client services
  - Terminal and file operations are not exposed to agents through ACP, preventing end‑to‑end “apply patch → run tests” loops when using ACP‑compliant agents.

- Testing gaps
  - Without ACP on the wire, we can’t reuse ACP SDK conformance tests or simulate standard prompt‑turn sequences across platforms.

## Path to Full Compliance

Principles:
- Do not add another server. The Rust `oa-bridge` is the single source of truth for `/ws` (per repo guidelines and ADRs).
- Keep the Apple‑native socket limited to discovery/pairing only. Actual agent sessions should use ACP JSON‑RPC via the Rust bridge, or the Swift app must speak ACP JSON‑RPC directly to agents.

Recommended pathway (incremental):

1) Make Bridge Client (Engine B) primary for live flows
- Connect iOS/macOS to the Rust bridge at `ws://<host>:8787/ws` using the existing token model.
- Consume typed Tinyvex updates already emitted by the bridge for history/live timelines.
- Continue to use the Apple socket only for discovery and passing the selected `host:port` and token; do not add non‑ACP data envelopes to it.

2) Adopt ACP JSON‑RPC over the bridge
- Implement the ACP prompt turn lifecycle in Swift by consuming `session/update` and issuing `session/prompt` via the bridge. Start with baseline content: `ContentBlock::Text`, `ResourceLink`.
- Wire cancellation with `session/cancel` and reflect `StopReason::Cancelled` in the UI.

3) Implement ACP client services (macOS first)
- File system: implement `fs/read_text_file` and `fs/write_text_file` handlers in the macOS app with explicit user prompts and a restricted root (project directory).
- Terminal: implement `terminal/create|output|release|wait_for_exit|kill` with output truncation semantics; expose a terminal panel in the UI.
- Permission: implement `session/request_permission` UI and plumb remembered decisions.
- iOS: advertise limited/no capabilities where sandboxed.

4) Extend Swift ACP models to full surface (or generate)
- Add content variants used by the bridge (images, audio, embedded resources) and all `SessionUpdate` variants (`ToolCallUpdate`, `Plan`, `AvailableCommandsUpdate`, `CurrentModeUpdate`).
- Prefer codegen from ACP JSON Schema or reuse Rust‑to‑Swift typegen to avoid drift.

5) Session management
- Expose `session/new`, optional `session/load` (when supported by bridge/agent), and `session/set_mode` in the UI. Render `CurrentModeUpdate` and allow mode switching.

6) MCP and configuration
- Plumb MCP server configuration through session creation when needed (stdio is sufficient initially). Keep this behind the bridge to avoid duplicating transports on Apple.

7) Tests and conformance
- Add unit tests for Swift ACP encode/decode and for client service handlers.
- Add integration tests that drive a prompt turn over ACP and assert streamed updates render as expected.
- Reuse existing Rust bridge tests; add a smoke test that drives the Apple app against a local bridge.

8) Migration and compatibility
- Keep the Swift provider‑JSONL translator for local/offline history only.
- For live sessions, prefer ACP over the bridge to ensure uniform behavior across platforms.

## Milestones & Acceptance

- M1 — Bridge Client parity (2–3 days)
  - iOS/macOS connect to `oa-bridge` `/ws`; history and live updates come via Tinyvex and ACP `session/update`.
  - Apple socket limited to discovery/token handoff only.

- M2 — Prompt turn over ACP (1 week)
  - Implement `session/prompt` with streamed `session/update` handling (text, reasoning, tool call/result, plan).
  - Implement `session/cancel` and reflect `StopReason`.

- M3 — Client services on macOS (1–2 weeks)
  - File system read/write and terminal lifecycle implemented behind opt‑in permissions.
  - `session/request_permission` surfaced with remembered choices.

- M4 — Full content + modes (1 week)
  - Add remaining content variants and `AvailableCommandsUpdate`/`CurrentModeUpdate` handling.
  - Implement `session/set_mode` and (optionally) `session/load` if the bridge/agent supports it.

- M5 — Codegen + tests (ongoing)
  - Introduce codegen for Swift ACP types from schema to avoid drift.
  - Add integration tests that run a complete ACP prompt turn against a local bridge.

## Notes & References

- ACP method names and update types: see `agent.rs` and `client.rs` in the ACP Rust SDK.
- ADR‑0007 makes ACP the canonical contract on the wire and in storage; avoid ad‑hoc envelopes for agent/session semantics.
- ADR‑0014: keep the Apple WS server focused on discovery/pairing; do not expand it into a parallel protocol.

---

Appendix: concrete deltas observed in code
- Apple WS uses `Hello`/`HelloAck` and `threads.list.request/response` (custom), not ACP JSON‑RPC (e.g., `initialize`, `session/prompt`, `session/update`). See:
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift`
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/MobileBridge/MobileWebSocketClient.swift`
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/BridgeMessages.swift`
- Swift ACP subset models exist for rendering, but lack full ACP surface (no `ResourceLink`, `Image`, `Audio`, etc.). See:
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/ACP/*`
- Provider→ACP translation exists for Codex history (not ACP on the wire). See:
  - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift`
