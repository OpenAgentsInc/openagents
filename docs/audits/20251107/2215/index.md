# OpenAgents Repository Audit — 2025-11-07 22:15

Scope: Full repo scan with emphasis on ACP compliance per ADR‑0002 and the Swift ACP module at `ios/OpenAgentsCore/Sources/OpenAgentsCore/AgentClientProtocol/`. This builds on the 20:55 audit and incorporates the most recent 25 issues/PRs.

Summary:
- Major refactors completed since 20:55: BridgeManager split (ConnectionManager, PromptDispatcher, TimelineStore), ExploreOrchestrator reducers, DesktopWebSocketServer modularization, and SwiftLint/SwiftFormat adoption. CI is in place.
- WebSocket bridge fully handles ACP core lifecycle (`initialize`, `session/new`, `session/prompt`, `session/cancel`) and streams `session/update`. Optional `session/load` is declared but not yet wired — tracked below.
- ACP Swift types are broadly complete for Phase‑1 and used end‑to‑end. A few field naming inconsistencies vs ADR‑0002’s “snake_case” guidance remain (see Compliance), likely intentional for ACP parity.

Top Findings (Actionable):
- ACP wire naming consistency: confirm exceptions for `sessionUpdate` and content `mimeType` keys; either codify them in ADR‑0002 or switch to snake_case with compatibility decoding.
- `session/load`: either implement in server/router or remove from constants until supported.
- Round‑trip compliance tests: add golden JSON fixtures from ACP examples to prevent drift across updates.
- Document extension methods (`orchestrate.explore.*`) under ACPExt and gate via capabilities.

Files in this audit:
- issues-prs.md — Recent 25 issues/PRs snapshot and status
- metrics.md — Counts and longest files
- long-files.md — Longest Swift files (focus list)
- duplication.md — Duplicate content check
- smells.md — Code smells and risks
- compliance.md — ADR‑0002 / ACP compliance notes and gaps
- recommendations.md — Prioritized actions
- stale.md — Deprecated/stale items to keep isolated
- todos.md — Concrete next steps

