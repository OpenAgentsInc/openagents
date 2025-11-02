# ADR 0007 — Agent Client Protocol (ACP) as Canonical Runtime Contract

 - Date: 2025-11-01
 - Status: Accepted

## Context

OpenAgents integrates multiple provider CLIs (e.g., Codex, Claude Code) and a mobile app that consumes live updates and local history over a LAN WebSocket bridge. Provider event shapes differ significantly (Codex JSONL rollouts, Claude transcripts), and ad‑hoc JSON drifting between components makes testing and evolution brittle.

We already translate provider events into ACP types in Rust (inspired by Zed’s adapters) and use typed Tinyvex rows on the app side (ADR‑0002/0003). This ADR formalizes ACP as our canonical contract across process boundaries and persistent storage.

## Decision

Adopt Agent Client Protocol (ACP) as the single, canonical runtime contract for agent updates and state. Concretely:

- On‑wire over WS
  - Only ACP‑derived envelopes travel over the bridge: control messages and typed Tinyvex snapshots/updates that carry ACP‑translated content.
  - No ad‑hoc provider JSON is exposed to the app; provider formats are translated at the bridge boundary into ACP `SessionUpdate`s and typed rows.

- In the bridge (Rust)
  - Provider adapters map provider events to ACP `SessionUpdate` using `acp-event-translator` (Codex new‑format JSONL, Claude transcripts).
  - Tinyvex writer mirrors ACP updates into typed rows (threads, messages, tool calls, plan/state) and emits `tinyvex.update` envelopes for clients.
  - Watchers tail provider stores, translate to ACP, then write to Tinyvex; two‑way writers persist provider‑native artifacts without changing the ACP-facing contract.

- In the database (Tinyvex)
  - Tables remain the minimal, typed projection required by the app (snake_case fields; ADR‑0002).
  - An append‑only `acp_events` log is maintained for traceability of ACP updates (already present in schema).
  - The Tinyvex Rust crate is DB‑only; the bridge hosts the writer that ingests ACP updates and mirrors into Tinyvex.

- Naming & casing
  - All public payloads the app observes are snake_case and align with ACP concepts (ADR‑0002). No mixed case fallback in the app.

## Rationale

- Interop: ACP provides a shared vocabulary for agent updates (messages, thoughts, tools, plans) across providers.
- Testability: A single contract enables layered tests (unit translator tests → writer invariants → watcher ingest → WS smoke) and stable Maestro flows.
- Evolution: Providers change; centralizing translation in one crate keeps drift contained while preserving the app contract.

## Scope

- Applies to all WS payloads and Tinyvex projections produced by the bridge.
- Provider‑native persistence (two‑way) is out‑of‑band; it must not leak provider formats to the app WS contract.
- App renders only ACP‑derived content; library demos and tests refer to ACP types.

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

- ADR‑0002 — Rust → TypeScript types as source of truth (snake_case).
- ADR‑0003 — Tinyvex as the local sync engine.
- ACP Introduction: https://agentclientprotocol.com/overview/introduction
