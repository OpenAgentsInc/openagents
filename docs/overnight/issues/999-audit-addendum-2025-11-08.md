# Audit Addendum — Orchiestration Plans (2025-11-08)

This addendum reviews the updates claimed in commit `16bc4ff7` and applies targeted corrections to align the plans with the current Swift codebase and demo scope.

## Summary of Review

- Plans moved from `private/overnight/issues/` to `docs/overnight/issues/` and example manifests added under `docs/overnight/examples/`.
- High-level intent matches the audit, but a few contradictions remained (e.g., code snippets still referenced hardcoded `gh` path; FM wrapper types not present in the codebase; runner spec still described a full pipeline/registry).

## Changes Applied

- README: Recommend opening the Xcode workspace, not the project (consistent with AGENTS.md).
  - File: README.md
- PRAutomationService plan: Fix location typo and replace hardcoded `gh` path with a PATH-based lookup helper (mirrors CLIAgentProvider strategy).
  - File: docs/overnight/issues/005-pr-automation-service.md
- OrchiestrationRunner plan: Trim to the minimal demo runner (analyze → decide → execute → PR), removing the heavy UpgradeExecutor/OperationsRegistry sections.
  - File: docs/overnight/issues/006-upgrade-executor.md (retitled in place to “Implement OrchiestrationRunner”).
- DecisionOrchestrator plan: Note reliance on ExploreOrchestrator + SessionAnalyzeTool and `ACPSessionModeId` mapping for agent selection; FM fallback guidance remains. (Deep rewrite deferred to keep plans readable; conflicts called out below.)

## Remaining Deltas (for follow‑up)

- DecisionOrchestrator doc still shows illustrative interfaces that reference placeholder types. Keep as guidance, but implementation should use:
  - ExploreOrchestrator (native FM path when available) or PlanningReducer fallback
  - SessionAnalyzeTool for insights
  - ACPSessionModeId for agent mapping
- Example manifests include non‑implemented ops (`github.list_issues`, `repo.coverage`, `test.run`). Treat as future/expanded examples; the demo runner should rely on the minimal path only (`session.analyze`, decide, `agent.execute`, PR draft).

If you want, I can further edit `002-decision-orchestrator.md` to replace the illustrative interface with a compact, codebase‑accurate snippet and add a minimal manifest using only implemented steps.
