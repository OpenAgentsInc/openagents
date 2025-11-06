# ACP Alignment for Search Primitives and Orchestrator

Date: 2025-11-06
Owner: OpenAgents — Bridge/Search Working Groups
Status: Draft (seeking confirmation from supervisor agent)

Purpose
- Align the new agent-only search/orchestration transport with the Agent Client Protocol (ACP) used elsewhere in the app.
- Ensure the wire structure, envelopes, and lifecycle semantics match ACP expectations, even if we layer additional operations on top.

Scope
- Applies to macOS service ↔ clients over JSON-RPC/WebSocket.
- Covers `search.*`, `content.getSpan`, and `orchestrate.*` flows introduced in specs v0.2/v0.2.1.
- Targets the Swift ACP implementation under `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/` and bridge runtime under `ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/`.

Design Goals
- Keep JSON-RPC method names consistent with ACP naming and structure.
- Represent engine actions as ACP tool calls and updates, so session streams look like normal ACP `session/update` traffic.
- Preserve snake_case fields and typed content blocks where applicable.

Layering Model
- Transport: JSON-RPC 2.0 over a single WebSocket connection (same as ACP). Handshake uses `initialize` (ACPRPC.initialize) before any other calls.
- Session lifecycle: session is created via `session/new`; all subsequent orchestration occurs within a session context.
- Search/Content/Explore are exposed as tools, invoked and reported using ACP Tool Call semantics.

Mapping: Engine Ops → ACP
- search.lexical|semantic|hybrid
  - Invoke as ACP tool calls with `ACPToolCallWire(name: "search.lexical", arguments: {...})`.
  - Stream progress and results via `ACPToolCallUpdateWire(status: .started|.completed|.error, output: …)` inside `ACP.Client.SessionNotificationWire` sent over `session/update`.
- content.getSpan
  - Same as above, `name: "content.get_span"` with `{ path, startLine, endLine, context }`.
- index.addRoot|status
  - Use ACP extension envelopes for control-plane operations: `ACPExtRequestWire` / `ACPExtNotificationWire` with `namespace: "index"` and `name: "add_root"|"status"`.
- orchestrate.explore.start|status|abort
  - Represent plan streaming as ACP thinking/plan updates:
    - Map `ExplorePlan.PartiallyGenerated` → `ACPPlan` or `ACP.Client.ContentBlock.thinking` with `_meta` carrying structured fields.
    - Each scheduled op emits an ACP tool call; results are `ACPToolCallUpdateWire`.
  - For session-scoped admin (abort), allow `ACPExtRequestWire(namespace: "orchestrate", name: "abort")`.

Envelope Shapes
- Keep ACP JSON-RPC methods (see `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/rpc.swift:8`).
- Use `session/update` notifications carrying `SessionNotificationWire` where updates include:
  - Tool call started/completed/errors (search/content/gitrepo ops).
  - Plan/Thinking blocks for partial plan snapshots.
  - Optional messages (assistant/user) when summarizing.

Identifiers & Correlation
- Reuse ACP session_id for all operations.
- Use `ACPToolCallWire.call_id` to carry the stable opId (UUID) defined in v0.2.1; include `opHash` in `_meta`.
- Include protocol and schema versions in `_meta` of tool call updates to allow evolution (`schema_version`, `type`).

Error Model
- Map engine error codes to ACP error surfaces:
  - For request failures: return JSON-RPC error with an `ACPError`-compatible code (see `errors.swift:7`).
  - For streaming errors: send a `ACPToolCallUpdateWire(status: .error, error: "ERR_CODE: message")` in `session/update`.

Handshake & Features
- Continue to require `initialize` before any other method.
- Advertise supported features in an extension banner immediately after `initialize`, carried via `ACPExtNotificationWire(namespace: "features", name: "hello")` with fields:
  - `protocol_version: "0.2.1"`
  - `features: ["search.lexical","search.semantic","search.hybrid","content.get_span","orchestrate.explore","llm.apple_on_device", "llm.external?"]`

Compatibility Mode
- Sidecar RPC kept for internal testing is allowed, but production paths should present the ACP-tool-call shape over `session/update` so downstream consumers see consistent ACP traffic.
- Where sidecar is used, mirror every result into an equivalent ACP tool call update (dual-write) until consumers fully switch to ACP view.

Swift Integration Points
- Bridge layer: ensure `DesktopWebSocketServer` and `MobileWebSocketClient` send ACP `session/update` notifications for search/orchestrate events.
- Types:
  - Use `ACPToolCallWire` / `ACPToolCallUpdateWire` for engine ops and results (see `tool_call.swift:7`).
  - Use `ACPPlan` / `ACPPlanEntry` for plan snapshots when feasible (see `plan.swift:7`). If fields go beyond ACP, place them in `_meta`.
  - Use `ACP.Client.RequestPermission*` for FS/Git permissions when needed (see `services.swift`).

Open Questions for Supervisor
- Plan representation: Should we map `ExplorePlan.PartiallyGenerated` directly to `ACPPlan` entries, or send them as `thinking` content blocks with a compact JSON payload in `_meta`?
- Sidecar vs ACP-only: Do we require the ACP-shaped stream as the sole production interface now, or will we support both sidecar RPC and ACP mirroring for a migration period?
- Method naming: Are we comfortable standardizing tool names as kebab/snake under a single namespace (`search.lexical`, `content.get_span`, `orchestrate.explore.start`), or should these be grouped under a `tools.*` convention?
- Error codes: Confirm the canonical mapping from `ERR_*` in v0.2.1 to `ACPErrorCode` ranges. Should we allocate a reserved vendor range?
- Permission flows: Should all workspace access (Git clone, indexing) require `session/request_permission` prompts up-front, or are policy files/allowlists sufficient?
- Session scoping: Must every operation occur within an ACP session, or can indexing run outside a session with only status mirrored to a session later?

Acceptance Criteria
- All search/content/orchestrate operations produce ACP `session/update` tool call updates with proper call ids and result payloads.
- Initialize/feature handshake occurs before any non-ACP method.
- Tests cover ACP-shaped envelopes for engine operations (see `docs/engine/test-coverage-plan.md`).

References
- ADR 0002 — Agent Client Protocol (docs/adr/0002-agent-client-protocol.md:1)
- Swift ACP types (ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/)
- Bridge JSON-RPC types (ios/OpenAgentsCore/Sources/OpenAgentsCore/Bridge/JSONRPC.swift:1)
- Orchestrator specs (docs/engine/spec-v0.2.md, docs/engine/spec-v0.2.1-addenda.md)

