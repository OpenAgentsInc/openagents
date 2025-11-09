# OpenAgents Codebase Audit — 2025-11-10

This audit reviews the Swift iOS/macOS app (`ios/OpenAgents`) and shared core (`ios/OpenAgentsCore`) with a focus on architecture quality, code smells, stale/placeholder logic, and test coverage. It provides prioritized recommendations and a concrete coverage improvement plan.

Contents:
- findings.md — Detailed findings (architecture, smells, risks)
- coverage.md — Test coverage baseline and plan (+targets)
- refactoring-plan.md — Prioritized refactor roadmap
- actions.md — Concrete, assignable tasks

## Executive Summary

Strengths
- Swift-only architecture with a clear shared core (OpenAgentsCore).
- ACP-first transport and typed JSON-RPC routing.
- macOS chat refactor on NavigationSplitView with clean OATheme application.
- Tinyvex DB is the source of truth for session history and titles.
- Programmatic orchestration harness (run_now/status) and background scheduler are in place.

Top Risks (P0/P1)
1) Server modularity (P0): DesktopWebSocketServer still aggregates multiple concerns; hard to test in isolation.
2) Orchestration size (P1): ExploreOrchestrator remains large; summary/ACP emission code can be extracted.
3) UI compliance (P1): A few view files still mix non-UI transforms; keep logic in models/utilities.
4) Coverage gaps (P0/P1): Core routing, DB edges, bridge error paths, and UI commands lack direct tests.
5) Docs drift (P1): Bridge/orchestration docs must track the evolving JSON-RPC surface.

## Priorities
- P0: Extract server concerns into services + unit tests. Add router tests and error-path tests.
- P0: Expand coverage around DB history queries, transcript export, and BridgeManager orchestration flows.
- P1: Extract orchestration summary builder and reduce ExploreOrchestrator surface.
- P1: Ensure all export/title/compose/sidebar actions have a test.

## Targets
- New/changed code: 80%+ line coverage.
- Core subsystems (routing/history/orchestration): 85%+.
- UI command wiring/menus: smoke tests where feasible.
