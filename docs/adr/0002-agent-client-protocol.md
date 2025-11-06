# ADR 0002 — Agent Client Protocol (ACP) as Canonical Runtime Contract

 - Date: 2025-11-01
 - Status: Accepted

## Context

OpenAgents integrates multiple provider CLIs (e.g., Codex, Claude Code) and a mobile app that consumes live updates and local history over a LAN WebSocket bridge. Provider event shapes differ significantly (Codex JSONL rollouts, Claude transcripts), and ad‑hoc JSON drifting between components makes testing and evolution brittle.

We translate provider events into ACP types (in Rust for the bridge, and in Swift for Apple‑native surfaces) and use typed rows on the app side. This ADR formalizes ACP as our canonical contract across process boundaries and persistent storage.

## Decision

Adopt Agent Client Protocol (ACP) as the single, canonical runtime contract for agent updates and state. Concretely:

- On‑wire over WS
  - Only ACP‑derived envelopes travel over the bridge: control messages and typed Tinyvex snapshots/updates that carry ACP‑translated content.
  - No ad‑hoc provider JSON is exposed to the app; provider formats are translated at the bridge boundary into ACP `SessionUpdate`s and typed rows.

- In the bridge (Rust)
  - Provider adapters map provider events to ACP `SessionUpdate` using `acp-event-translator` (Codex new‑format JSONL, Claude transcripts).
  - Tinyvex writer mirrors ACP updates into typed rows (threads, messages, tool calls, plan/state) and emits `tinyvex.update` envelopes for clients.
  - Watchers tail provider stores, translate to ACP, then write to Tinyvex; two‑way writers persist provider‑native artifacts without changing the ACP-facing contract.

- In Apple‑native apps (iOS/macOS)
  - All ACP usage MUST go through the Swift parity module `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/*` (one‑to‑one mapping with the Rust SDK).
  - The Apple WebSocket bridge uses JSON‑RPC 2.0 and ACP method names; legacy Hello‑style envelopes are deprecated and retained only as a fallback.
  - Session lifecycle and streamed updates (`session/new`, `session/prompt`, `session/update`, `session/cancel`) are implemented natively in Swift using this module.
  - Client‑side services (fs.*, terminal.*, `session/request_permission`) are implemented in Swift behind permissions.

- In the database (Tinyvex)
  - Tables remain the minimal, typed projection required by the app (snake_case fields).
  - An append‑only `acp_events` log is maintained for traceability of ACP updates (already present in schema).
  - The Tinyvex Rust crate is DB‑only; the bridge hosts the writer that ingests ACP updates and mirrors into Tinyvex.

- Naming & casing
  - All public payloads the app observes are snake_case and align with ACP concepts. No mixed‑case fallback in the app.

## Rationale

- Interop: ACP provides a shared vocabulary for agent updates (messages, thoughts, tools, plans) across providers.
- Testability: A single contract enables layered tests (unit translator tests → writer invariants → watcher ingest → WS smoke) and stable Maestro flows.
- Evolution: Providers change; centralizing translation in one crate keeps drift contained while preserving the app contract.

## Scope

- Applies to all WS payloads and Tinyvex projections produced by the bridge.
- Provider‑native persistence (two‑way) is out‑of‑band; it must not leak provider formats to the app WS contract.
- App renders only ACP‑derived content; library demos and tests refer to ACP types.
- iOS/macOS apps MUST use the AgentClientProtocol Swift module for all ACP types and JSON‑RPC method names. Do not embed ad‑hoc copies of ACP shapes.

## Implementation Plan

1) Maintain and extend `acp-event-translator` for Codex/Claude mapping parity.
2) Keep the bridge‑hosted writer (`tvx_writer`) focused on ACP `SessionUpdate` ingestion and typed row upserts; avoid provider‑specific logic inside the Tinyvex crate.
3) Ensure watchers call translator → bridge writer exclusively; no bypasses.
4) WS controls expose only typed Tinyvex snapshots/updates and control envelopes; remove any accidental passthrough of provider JSON.
5) Tests (see issue 1351 TDD plan):
   - Unit: translator mappings for Codex/Claude; writer invariants.
   - Integration: watcher ingest into Tinyvex via temp dirs.
   - WS smoke: snake_case envelopes, `tinyvex.update` push.
   - Maestro: UI flows assert ACP‑derived renderers and history.

## Consequences

- Positive: Stronger guarantees at the app boundary; easier to add providers; simpler tests; fewer ad‑hoc shapes.
- Neutral: More rigorous mapping work when providers evolve; offset by containment in one crate.
- Negative: Some provider‑specific richness may be normalized; where needed, we can pass structured metadata through ACP `meta` fields.

## Acceptance

- All bridge→app payloads are ACP‑derived with snake_case fields.
- Provider adapters live behind `acp-event-translator`; Tinyvex writer and watchers operate only on ACP `SessionUpdate`.
- Tests exist at each layer to enforce the contract.

## References

- ADR‑0003 — Swift Cross‑Platform App (macOS + iOS) Experiment
- ADR‑0006 — iOS ↔ Desktop WebSocket Bridge and Pairing
- ACP Introduction: https://agentclientprotocol.com/overview/introduction
