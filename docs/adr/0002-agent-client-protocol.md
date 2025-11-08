# ADR 0002 — Agent Client Protocol (ACP) as Canonical Runtime Contract

 - Date: 2025-11-01
 - Status: Accepted

## Context

OpenAgents integrates multiple provider CLIs (e.g., Codex, Claude Code) and a mobile app that consumes live updates and local history over a LAN WebSocket bridge. Provider event shapes differ significantly (Codex JSONL rollouts, Claude transcripts), and ad‑hoc JSON drifting between components makes testing and evolution brittle.

In v0.3, the entire bridge and app are native Swift. Provider events are translated into ACP types in Swift, and typed rows are persisted locally. This ADR formalizes ACP as our canonical contract across process boundaries and persistent storage.

## Decision

Adopt Agent Client Protocol (ACP) as the single, canonical runtime contract for agent updates and state. Concretely:

- On‑wire over WS
  - Only ACP‑derived envelopes travel over the bridge: control messages and typed Tinyvex snapshots/updates that carry ACP‑translated content.
  - No ad‑hoc provider JSON is exposed to the app; provider formats are translated at the bridge boundary into ACP `SessionUpdate`s and typed rows.

- In the bridge (Swift)
  - Provider adapters map provider events to ACP `SessionUpdate` via Swift translators:
    - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/CodexAcpTranslator.swift`
    - `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/ClaudeAcpTranslator.swift`
  - The desktop WebSocket server (`DesktopWebSocketServer`) sends ACP `session/update` notifications over JSON‑RPC 2.0.
  - A Tinyvex writer (`SessionUpdateHub` + `TinyvexDbLayer`) mirrors ACP updates into typed rows (threads, messages, tool calls, plan/state) and can serve history back to clients.

- In Apple‑native apps (iOS/macOS)
  - All ACP usage MUST go through the Swift parity module `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/*` (one‑to‑one mapping with the Rust SDK).
  - The Apple WebSocket bridge uses JSON‑RPC 2.0 and ACP method names exclusively.
  - Session lifecycle and streamed updates (`session/new`, `session/prompt`, `session/update`, `session/cancel`) are implemented natively in Swift using this module.
  - Client‑side services (fs.*, terminal.*, `session/request_permission`) are implemented in Swift behind permissions.

- In the database (Tinyvex, Swift)
  - Tables remain the minimal, typed projection required by the app (snake_case fields).
  - An append‑only `acp_events` log is maintained for traceability of ACP updates (see `TinyvexDbLayer`).
  - All DB access and writing are implemented in Swift (`ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/`).

- Naming & casing
  - General rule: prefer snake_case for public payload fields and align with ACP concepts.
  - ACP canonical exceptions (use as‑is, even if camelCase):
    - `sessionUpdate` — discriminant inside `update` envelope for `session/update` notifications (e.g., `{ "update": { "sessionUpdate": "agent_message_chunk", ... } }`).
    - Content/type keys originating from ACP content model such as `type`, `mimeType`, `lastModified`, `name`, `title`, `uri` where applicable in content blocks and resource descriptors.
  - Rationale: these keys are defined by the ACP spec/examples and widely used across implementations; we adopt them verbatim to ensure interop. All other fields under our control remain snake_case (e.g., `current_mode_id`, `available_commands`).

## Rationale

- Interop: ACP provides a shared vocabulary for agent updates (messages, thoughts, tools, plans) across providers.
- Testability: A single contract enables layered tests (unit translator tests → writer invariants → WebSocket integration) and stable UI flows.
- Evolution: Providers change; centralizing translation in one crate keeps drift contained while preserving the app contract.

## Scope

- Applies to all WS payloads and Tinyvex projections produced by the Swift bridge.
- Provider‑native artifacts may be persisted for diagnostics, but must not leak provider formats to the app WS contract.
- App renders only ACP‑derived content; demos and tests refer to ACP types.
- iOS/macOS apps MUST use the AgentClientProtocol Swift module for all ACP types and JSON‑RPC method names. Do not embed ad‑hoc copies of ACP shapes.

## Implementation Plan

1) Maintain and extend the Swift translators for Codex/Claude mapping parity.
2) Keep the Swift writer (`SessionUpdateHub` + `TinyvexDbLayer`) focused on ACP `SessionUpdate` ingestion and typed row upserts; avoid provider‑specific logic in DB.
3) WS controls expose only typed Tinyvex snapshots/updates and control envelopes; remove any accidental passthrough of provider JSON.
4) Tests:
   - Unit: translator mappings for Codex/Claude; writer invariants.
   - Integration: WebSocket server/client (`DesktopWebSocketServer` ↔ `MobileWebSocketClient`).
   - Persistence: Tinyvex history queries round‑trip ACP updates.

## Consequences

- Positive: Stronger guarantees at the app boundary; easier to add providers; simpler tests; fewer ad‑hoc shapes.
- Neutral: More rigorous mapping work when providers evolve; offset by containment in translators.
- Negative: Some provider‑specific richness may be normalized; where needed, we can pass structured metadata through ACP `_meta` fields.

## Acceptance

- All bridge→app payloads are ACP‑derived with snake_case fields (with ACP‑spec exceptions noted above).
- Provider adapters live behind Swift translators; Tinyvex writer operates only on ACP `SessionUpdate`.
- Tests exist at each layer to enforce the contract (see below).

## References

- ADR‑0003 — Swift Cross‑Platform App (macOS + iOS)
- ADR‑0004 — iOS ↔ Desktop WebSocket Bridge and Pairing
- ACP Introduction: https://agentclientprotocol.com/overview/introduction

### Pointers to code and tests (v0.3 Swift)

- Swift ACP types: `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`
- Translators: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Translators/`
- Bridge server/client: `DesktopWebSocketServer` and `MobileWebSocketClient`
- Persistence: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Tinyvex/`
- Tests:
  - `ACPProtocolComprehensiveTests.swift`
  - `SessionUpdateHubTests.swift`
  - `DesktopWebSocketServerComprehensiveTests.swift`
  - `MobileWebSocketClientComprehensiveTests.swift`
