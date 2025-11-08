# Recommendations (Prioritized)

## P0 — Structural refactors and safety

- Split DesktopWebSocketServer.swift into modules:
  - Transport (NWListener lifecycle)
  - JsonRpcRouter (method dispatch)
  - HistoryApi (Tinyvex access)
  - SessionUpdateHub (broadcasts)
  - BonjourService (discovery)
- Break ExploreOrchestrator.swift into state-machine reducers and sub-orchestrators.
- Extract BridgeManager responsibilities into ConnectionManager, PromptDispatcher, TimelineStore.

## P0 — Developer experience and guardrails

- Adopt SwiftLint + SwiftFormat; enforce:
  - No `print(...)` in source (use Logger)
  - Discourage force unwraps/casts; require `guard`/`if let`
  - File length/function length thresholds (warnings)
- Introduce Logging utility wrapping `os.Logger` with categories and levels; gate verbose logs behind `#if DEBUG`.
- Ensure CI runs build + tests + lint on PRs (issue #1426 is closed — verify in CI settings).

## P1 — Safety cleanup

- Remove/replace `)!`/`x!` patterns where feasible; use throwing APIs or optional chaining.
- Replace `as!` with safe casts + error surfaces (tests can use `XCTUnwrap`).
- Review `fatalError` sites; prefer thrown errors and surface in UI/logs.

## P1 — Stale/deprecated

- Remove deprecated raw JSONL hydrate path in DesktopWebSocketServer.swift:313.
- Confirm `packages/tricoder/` remains read-only and excluded from builds/tests.

## P2 — Test strategy

- Add unit tests around newly extracted modules (router, history API, connection manager).
- Maintain high coverage on ACP parsing and bridge protocol.

## P2 — Documentation

- Update docs/ios-bridge to reflect JSON-RPC `initialize` semantics (issue #1425 closed — verify merged changes reflect ADR-0004).
- Document logging/diagnostics strategy in docs.

## Tracking

- Convert these into issues with clear ownership and acceptance criteria.
- Sequence refactors starting with DesktopWebSocketServer and ExploreOrchestrator.

