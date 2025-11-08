# ACP / ADR-0002 Compliance Audit

References:
- ADR: docs/adr/0002-agent-client-protocol.md (Accepted 2025‑11‑01)
- Swift ACP: ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/
- Bridge: DesktopWebSocketServer, MobileWebSocketClient, JsonRpcRouter

Status: High compliance with ACP shapes and lifecycle; a handful of naming mismatches vs ADR‑0002’s snake_case guidance need explicit decisions.

What’s Compliant:
- JSON‑RPC 2.0 transport with ACP method names
  - Implements `initialize`, `session/new`, `session/prompt`, `session/cancel` (notify), and streams `session/update` (notify).
  - `session/set_mode` supported as an extension aligned with ADR‑0004.

- SessionUpdate variants and payloads
  - Supported: `user_message_chunk`, `agent_message_chunk`, `agent_thought_chunk`, `plan`, `available_commands_update`, `current_mode_update`, `tool_call`, `tool_call_update` with snake_case payload fields (e.g., `current_mode_id`, `available_commands`).
  - Classification tests (e.g., ACPMessageTypeComplianceTests) ensure messages vs thoughts are correctly distinguished.

- Client‑handled services present and namespaced under ACPRPC
  - `fs/read_text_file`, `fs/write_text_file` and `terminal/run` are implemented; additional terminal methods are defined for future use.

Items to Clarify / Align:
- Discriminant key casing: `sessionUpdate`
  - Wire uses `{"update":{"sessionUpdate":"user_message_chunk", ...}}` (camelCase). ADR‑0002 says “All public payloads the app observes are snake_case.”
  - Action: Decide whether `sessionUpdate` is an ACP‑mandated exception. If yes, codify as an ADR note. If not, consider switching to `session_update` with backward‑compatible decoding in Swift.

- Content field casing: `mimeType`, `lastModified`
  - Several content blocks use camelCase keys. If these mirror ACP examples, document as exceptions in ADR. Else, migrate to snake_case keys with compatibility decoders.

- `session/load` optional method
  - Declared in ACPRPC, but DesktopWebSocketServer has no router entry for it. Implement or remove from the surface until supported to avoid divergent contracts.

- Extension methods and capabilities
  - `orchestrate.explore.*` lives outside core ACP. Ensure capability negotiation via `ACPExt*` wire shapes and document in ios‑bridge docs to avoid client surprises.

Recommended Tests (additions):
- Golden fixture round‑trips
  - Encode/decode known‑good ACP JSON examples (including `session/update` variants) to lock wire shapes. Store fixtures under `ios/OpenAgentsCore/Tests/Fixtures/acp/`.
- Snake vs camel compatibility
  - If adopting snake_case for `session_update`/content, add decoders tolerant of legacy camelCase for one release and test both.

