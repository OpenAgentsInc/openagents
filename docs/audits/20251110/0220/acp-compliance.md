# ACP Compliance Audit

Scope
- Types: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/*`
- Transport: JSON-RPC 2.0 over WebSocket (Desktop) and client (Mobile)
- Streaming: `session/update` via `SessionUpdateHub`

What’s Good
- Protocol version: handshake replies with `0.2.2` and negotiates `ext_capabilities` (orchestrate_explore) based on OS availability with override.
- Initialize ↔ handshake:
  - Desktop: responds with `ACP.Agent.InitializeResponse` and includes `_meta.working_directory` when available.
  - Mobile: sends `ACP.Agent.InitializeRequest`, waits for result, extracts `_meta.working_directory` and begins receive loop.
- Session lifecycle:
  - `session/new`, `session/set_mode`, `session/prompt`, `session/cancel` implemented.
  - Mode updates broadcast via `session/update` with `currentModeUpdate`.
- Session updates covered:
  - `.userMessageChunk`, `.agentMessageChunk`, `.agentThoughtChunk`
  - `.plan` (plan entries), `.availableCommandsUpdate`, `.currentModeUpdate`
  - `.toolCall`/`.toolCallUpdate` with `ACPToolCallWire`/`ACPToolCallUpdateWire`
- Persistence + broadcast: All outbound updates flow through `SessionUpdateHub`, persisted to Tinyvex, then broadcast as JSON-RPC `session/update` notifications.
- Test fixtures exist for all supported update variants and initialize/session methods.

Identified Gaps or Risks
- Content modeling duplication:
  - `AgentClientProtocol/client.swift` defines `ContentBlock` (text/image/audio/resource*).
  - UI timeline renders `ACPContentPart` with `ACPToolCall`/`ACPToolResult` from `ACP/` types, mapping from Wires at receive time.
  - Risk: shape drift and confusion between wire vs UI content types. Recommendation: centralize mapping in a single utility and document the mapping contract.
- Tool use blocks parity:
  - Older tests reference `ACP.Client.ToolUse`/`ToolResult` style blocks. Current wire path prefers `tool_call`/`tool_call_update` at `SessionUpdate` level.
  - Action: ensure tests match the current shapes or add type aliases/encoders to preserve parity when beneficial.
- Client-handled services breadth:
  - FS and Terminal minimal subsets are present. Permission requests (`session/request_permission`) types exist but end-to-end request/response handling path isn’t visible in UI yet. Not a blocker, but note for parity.
- MCP server modeling is present in types; functional use is deferred (ok per ADRs). Keep docs/tests in sync as surface expands.

Fixture Conformance
- Initialize, session lifecycle, and all supported `session/update` variants round-trip via fixtures under `ios/OpenAgentsCore/Tests/Fixtures/acp`.

Recommended Actions
- Add a doc or comment block codifying “Wire vs UI Content” mapping and keep a single translator utility (e.g., in `Translators/` or `Bridge/`).
- Align or deprecate legacy tests referencing `ToolUse` in favor of the `tool_call`/`tool_call_update` flow; alternatively, add thin types to decode legacy fixtures for regression coverage.
- Add a small test for `SessionUpdateHub` metrics and error logging on persistence failure (Tinyvex unavailable).

