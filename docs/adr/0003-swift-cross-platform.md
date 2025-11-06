# ADR 0003 — Swift Cross‑Platform App (macOS + iOS)

 - Date: 2025-11-03
 - Status: Accepted — Desktop + Mobile direction

## Context

We have adopted a native Swift app that targets both macOS (desktop) and iOS (mobile) with shared SwiftUI code. This is our chosen direction for Apple platforms.

Current operating mode:
- The desktop app runs the ACP JSON‑RPC server (and client services) that powers the mobile app.
- The mobile app requires a paired desktop app; it does not operate standalone today.

Planned evolution:
- We want the desktop app to be fully usable on its own (even without pairing the mobile app). Mobile pairing remains optional and complementary.

Key drivers:
- A unified native experience for Apple platforms with SwiftUI (windowing, shortcuts, first‑class local integrations), while keeping the Expo app and existing Rust bridge until migration completes.
- Faster local iteration and packaging; native handling of ACP updates in Swift.
- Clear pairing story: desktop provides services/modes the phone cannot (fs/terminal, process control), while offering a path for desktop‑only usage.

This work must preserve our canonical contracts: ACP on the wire (ADR‑0002) and Tinyvex typed rows/snapshots for history and live UI. It must not introduce ad‑hoc JSON to the app boundary.

## Decision

Adopt a SwiftUI app for macOS and iOS with the desktop app acting as the ACP JSON‑RPC server for the mobile app. This architecture is accepted.

- Primary mode (today):
  - Desktop runs an Apple‑native ACP JSON‑RPC server and client services (fs, terminal, permissions). iOS connects over Bonjour/WS and consumes `session/update` streams.
  - Mobile requires a paired desktop; no phone‑only mode yet.

- Roadmap:
  - Desktop must be fully usable by itself even without pairing a phone (initiate sessions, render timelines, manage files/terminal locally).
  - Mobile pairing remains optional and complementary.

- Optional engines:
  - Swift Runner (desktop): spawn provider CLIs locally and translate to ACP updates in‑process, preserving ACP contracts.
  - Optional connection to the Rust bridge where available; parity maintained via ACP on the wire.

Scope and guardrails:
- Maintain ACP semantics at the app boundary (ADR‑0002). No HTTP control plane; use JSON‑RPC over WebSocket or local process/stdio.
- Do not change existing Expo or Rust contracts; they continue to work during migration.

## Rationale

- Native UX on macOS/iOS: tighter OS integration, better windowing/menus, energy/perf characteristics, and Apple distribution options.
- Packaging simplicity for Apple users: a single app that can run sessions locally without requiring a separate terminal process, while retaining compatibility with the existing bridge.
- Flexibility: cleanly compare a pure‑Swift runner to the current Rust bridge path without committing to a replacement yet.
- Contract discipline: by continuing to use ACP and Tinyvex semantics, we avoid fragmenting data/shapes across platforms.

## Alternatives Considered

1) Keep Tauri as the only desktop app
- Pros: single desktop codebase; already integrates with the Rust bridge.
- Cons: less native feel; requires JS/TS + Rust; does not exercise a pure‑Swift path.

2) React Native for macOS or Catalyst web embed
- Pros: higher code reuse with Expo.
- Cons: platform coverage and quality vary; additional complexity vs. SwiftUI for desktop polish.

3) Flutter desktop/mobile
- Pros: single codebase across platforms.
- Cons: diverges from our existing Expo/Rust stack; adds heavy toolchain; limited direct reuse of ACP/Tinyvex types.

4) Electron or web‑only wrapper
- Pros: rapid iteration with web tech.
- Cons: heavier footprint; not aligned with native macOS/iOS goals.

## Consequences

- Two Apple surfaces (desktop + mobile) are maintained; desktop currently supports mobile and will also stand alone.
- Type/code duplication risk for ACP in Swift; evaluate schema‑driven codegen to reduce drift.
- Release/CI includes Apple packaging; no change to Rust bridge release flow.
- Provider translation must remain current; maintain Swift runner mappings or reuse a server that already emits ACP.
- SQLite read/write considerations apply when persisting locally; prefer WAL and read‑only where applicable.

## Implementation Plan

1) Project bootstrap (desktop first)
- Create a Swift package `OpenAgentsCore` under `ios/` containing shared models, engines, and a small data layer.
- Create a macOS app target (`OpenAgents Desktop`) and an iOS app target (`OpenAgents Mobile`) that consume `OpenAgentsCore`.
- Use SwiftUI throughout; adopt `Codable` for all payloads; enforce snake_case via `CodingKeys`.

2) Types and contracts
- Define minimal Swift `Codable` types for ACP envelopes we render (messages, tool calls, plan/state) and Tinyvex rows we need for history.
- Keep fields snake_case to match ADR‑0002. Prefer value types with precise optionals; no `Any`.
- Evaluate codegen from JSON Schema (ACP) and/or a Rust → Swift generator to reduce duplication.

3) History (read‑only initially)
- Primary: read Tinyvex SQLite at `~/.openagents/tinyvex/data.sqlite3` using a lightweight SQLite library (GRDB/SQLite.swift). Map rows to Swift types that mirror our Tinyvex transport.
- Fallback: if the DB is unavailable, connect to a reachable bridge over WS and fetch a snapshot of threads/messages/tool calls.

4) Live sessions
- ACP JSON‑RPC bridge (primary): desktop handles `initialize`, `session/new`, `session/prompt`, `session/update`, `session/cancel`; iOS consumes typed updates.
- Swift Runner (optional): spawn provider CLIs, translate to ACP updates, mirror into local store; maintain ACP semantics.

5) UI slices (desktop first)
- History list (threads) → detail (thread timeline) using Tinyvex rows; a settings view for connection and engine selection; a composer to submit prompts.
- Render ACP‑derived content consistently with our Expo renderers (structure, not identical visuals).

Update — current state (2025‑11‑04)
- Implemented macOS app shell with SwiftUI and Liquid Glass styling per ADR‑0012 (transparent toolbar, top gradient scroll‑edge, off‑black theme).
- Berkeley Mono is the global app font; Markdown rendering in message bodies.
- ACP renderers wired: assistant/user messages, tool calls/results; plan state pending.
- Stable, clickable composer via `safeAreaInset(edge: .bottom)` (TextField + Send) — currently appends locally; WS wiring next.

Update — bridge pairing and transport (2025‑11‑04)
- Added an Apple‑native WS bridge and Bonjour pairing flow (see ADR‑0006):
  - macOS auto‑starts `DesktopWebSocketServer` and advertises `_openagents._tcp`.
  - iOS auto‑discovers via Bonjour and connects.
  - Primary handshake and transport use JSON‑RPC 2.0 with ACP method names (`initialize` then `session/*`).
  - Handshake is strictly JSON‑RPC `initialize`; no legacy token handshakes are supported.
  - This path enables Engine B (Bridge Client) for the Swift app without adding HTTP endpoints.

6) Security and configuration
- Respect the existing token model when connecting to a bridge (`~/.openagents/bridge.json`).
- No new HTTP endpoints; when local, use process/stdio; when remote, use the existing WS contract.

7) Testing
- Unit tests for JSON decode/encode of ACP/Tinyvex types and the process runner (mocked JSONL samples).
- Manual smoke for desktop flows; if adopted, add UI/E2E later.

## Acceptance

- Desktop app runs an ACP JSON‑RPC server; iOS connects and receives streamed `session/update` notifications.
- Desktop can initiate and render sessions locally; goal is full standalone usability without mobile pairing.
- No changes required in the Expo app or Rust bridge; both continue to function unchanged.
- All Swift models are strongly typed (`Codable`), snake_case, and contain no dynamic `Any` usage.

Interim acceptance
- Reads Codex sessions for history; renders ACP messages + tool calls/results; Markdown enabled; composer present.
- Bridge write path and live session streaming are WIP.

## Open Questions

- Provider translation: Which pieces of translation should remain server‑side vs. in‑process Swift runner?
- Tinyvex writes from Swift: When/if to enable write‑path parity with idempotency guarantees.
- Legacy surfaces: De‑scoping and migration criteria from older desktop stacks.
- Distribution: App Store vs. notarized direct download for macOS; implications for bundled CLIs and sandboxing.

## References

- ADR‑0002 — Agent Client Protocol as Canonical Runtime Contract
- ADR‑0006 — iOS ↔ Desktop WebSocket Bridge and Pairing
