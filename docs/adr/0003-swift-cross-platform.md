# ADR 0003 — Swift Cross‑Platform App (macOS + iOS) Experiment

 - Date: 2025-11-03
 - Status: In Progress — Desktop‑first implementation

## Context

We have introduced a new `ios/` folder to explore a native Swift app that targets both macOS (desktop) and iOS (mobile) with shared SwiftUI code. The goal is to validate whether we can deliver our full core coding flow directly in Swift — starting with the desktop app — and then pair the same codebase to the mobile app.

Key drivers:
- A unified native experience for Apple platforms with SwiftUI (windowing, shortcuts, first‑class local integrations), while keeping the Expo app for cross‑platform mobile and the Rust bridge for today’s flows.
- Faster local iteration and packaging: experiment with running provider CLIs (Codex, Claude Code) and handling ACP updates natively in Swift.
- Evaluate whether this Swift app replaces or supplements the Tauri desktop app; decision deferred until results of this experiment.

This work must preserve our canonical contracts: ACP on the wire (ADR‑0002) and Tinyvex typed rows/snapshots for history and live UI. It must not introduce ad‑hoc JSON to the app boundary.

## Decision

Proceed with an experiment to build a SwiftUI app for macOS and iOS that implements the core coding flow with two pluggable engines behind a single interface:

- Engine A — Swift Runner (default for the experiment):
  - Spawn provider CLIs directly (e.g., `codex exec --json ...`) and stream JSON lines over pipes.
  - Translate provider output into ACP updates in‑process (minimal subset at first), then mirror into a local store for the UI.

- Engine B — Bridge Client (compatibility path):
  - Connect to the Apple‑native macOS bridge over WebSocket using JSON‑RPC ACP methods (`initialize`, `session/new`, `session/prompt`, `session/update`, `session/cancel`).
  - Optionally connect to the existing Rust bridge where available; parity maintained via ACP on the wire.
  - Read‑only history works out‑of‑the‑box; live sessions mirror current behavior.

Scope and guardrails for the experiment:
- Desktop first: ship a macOS app with the shared core in a Swift package; then enable the iOS target after desktop is usable.
- Maintain ACP/Tinyvex semantics at the UI boundary. No new HTTP control plane; use WS control envelopes when talking to the bridge, or local process/stdio when running directly.
- Do not change existing Expo or Rust contracts; this ADR adds a Swift client, not a new server.

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

- Duplicate desktop surface during the experiment (Tauri and Swift). We will carry both until we conclude whether to replace or supplement Tauri.
- Type/code duplication risk for ACP/Tinyvex in Swift. We will start with a minimal, strictly typed Swift model and evaluate codegen if the experiment continues.
- Release/CI adds Apple app packaging steps. No change to bridge releases (see `docs/bridge-release.md`).
- When using the Swift Runner, translating provider output to ACP must be maintained as providers evolve (currently done in Rust for the bridge).
- Reading Tinyvex directly from SQLite introduces cross‑process concerns; we will begin read‑only and follow SQLite WAL best practices.

## Implementation Plan

1) Project bootstrap (desktop first)
- Create a Swift package `OpenAgentsCore` under `ios/` containing shared models, engines, and a small data layer.
- Create a macOS app target (`OpenAgents Desktop`) and an iOS app target (`OpenAgents Mobile`) that consume `OpenAgentsCore`.
- Use SwiftUI throughout; adopt `Codable` for all payloads; enforce snake_case via `CodingKeys`.

2) Types and contracts
- Define minimal Swift `Codable` types for ACP envelopes we render (messages, tool calls, plan/state) and Tinyvex rows we need for history.
- Keep fields snake_case to match ADR‑0002/0007. Prefer value types with precise optionals; no `Any`.
- If the experiment proceeds, evaluate codegen from JSON Schema (ACP) and/or a Rust → Swift generator for Tinyvex transport types to remove duplication.

3) History (read‑only initially)
- Primary: read Tinyvex SQLite at `~/.openagents/tinyvex/data.sqlite3` using a lightweight SQLite library (GRDB/SQLite.swift). Map rows to Swift types that mirror our Tinyvex transport.
- Fallback: if the DB is unavailable, connect to a reachable bridge over WS and fetch a snapshot of threads/messages/tool calls.

4) Live sessions
- Engine A (Swift Runner):
  - Process manager spawns `codex exec --json` with our standard flags (approvals/sandbox/model) and writes the prompt to stdin; close to signal EOF; respawn per message as needed.
  - Parse JSONL output; where output is not already ACP, apply a minimal adapter to produce ACP `SessionUpdate`s sufficient to render messages/tool calls.
  - Mirror updates into the in‑app store (and optionally into Tinyvex via direct SQLite writes in a later phase).
- Engine B (Bridge Client):
  - Connect to `ws://<host>:<port>/ws?token=…`; subscribe to Tinyvex updates and use WS controls for `run.submit` etc., exactly as mobile does today.

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
  - A legacy `Hello/HelloAck` token handshake remains as a fallback only.
  - This path enables Engine B (Bridge Client) for the Swift app without adding HTTP endpoints.

6) Security and configuration
- Respect the existing token model when connecting to a bridge (`~/.openagents/bridge.json`).
- No new HTTP endpoints; when local, use process/stdio; when remote, use the existing WS contract.

7) Testing
- Unit tests for JSON decode/encode of ACP/Tinyvex types and the process runner (mocked JSONL samples).
- Manual smoke for desktop flows; if adopted, add UI/E2E later.

## Acceptance

- Desktop Swift app loads historical threads and messages from Tinyvex (read‑only) and renders them with ACP‑aligned types.
- The app can run a new session on desktop and stream updates end‑to‑end:
  - Engine A: run a prompt via `codex exec --json`, parse, render in the thread timeline.
  - Engine B: connect to an existing bridge, send `run.submit`, and render Tinyvex updates.
- No changes required in the Expo app or Rust bridge; both continue to function unchanged.
- All Swift models are strongly typed (`Codable`), snake_case, and contain no dynamic `Any` usage.

Interim acceptance (in progress)
- Reads Codex sessions for history; renders ACP messages + tool calls/results; Markdown enabled; composer present.
- Bridge write path and live session streaming are WIP.

## Open Questions

- Provider translation: Does the Codex CLI emit ACP directly in our target flows? If not, how much of the Rust ACP translator must be ported to Swift for the experiment? We may temporarily implement a minimal subset or explore linking the Rust translator via FFI if needed.
- Tinyvex writes from Swift: For the experiment we will read Tinyvex; do we want the Swift Runner to also write rows for immediate cross‑app visibility? If yes, we need write‑path parity and idempotency guarantees (see ADR‑0003 invariants).
- Long‑term desktop surface: If the Swift app proves successful, do we deprecate Tauri (ADR‑0009) or run both? Criteria: performance, reliability, maintenance burden, and user feedback.
- Distribution: App Store vs. notarized direct download for macOS; implications for bundled CLIs (Codex/Claude) and sandboxing.

## References

- ADR‑0002 — Agent Client Protocol as Canonical Runtime Contract
- ADR‑0006 — iOS ↔ Desktop WebSocket Bridge and Pairing
