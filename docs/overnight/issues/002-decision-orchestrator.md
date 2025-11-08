# Issue #002: Decision Logic (Explore + Session Insights)

**Component**: Orchestration Layer — Decision (Demo Scope)
**Priority**: P0 (Critical Path)
**Estimated Effort**: 2–3 days (reuses existing components)
**Assignee**: TBD

---

## Overview

Implement demo‑scoped “decision logic” that chooses the next task and which agent to run based on recent session history, using only classes that already exist in the codebase. The output is a concrete agent selection (`ACPSessionModeId`) plus a terse, actionable prompt. This feeds the runner/AgentCoordinator to execute work.

- Data source: `SessionAnalyzeTool` (from `SessionTools`) → `SessionAnalyzeResult`
- Optional FM assist: `ExploreOrchestrator.fmAnalysis()` when Foundation Models are available (macOS 26+)
- Agent mapping (existing modes): `.claude_code` for refactoring/docs; `.codex` for tests/boilerplate
- No `repo.status`/coverage metrics in the demo (post‑demo)

---

## Goals (Demo)

- Produce high‑signal tasks suitable for overnight execution without new data sources
- Stable, bounded prompts (no long narratives)
- Deterministic fallback when Foundation Models are not available

---

## Inputs and Outputs

**Inputs**
- `SessionAnalyzeResult` (see `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchestrationTypes.swift`)
  - `fileFrequency: [String: Int]?`
  - `toolFrequency: [String: Int]?`
  - `goalPatterns: [String]?`
  - `avgConversationLength: Double?`
  - `userIntent: String?`
- `timeBudgetSeconds: TimeInterval` (e.g., 1800)

**Output** (runner‑level struct consumed by AgentCoordinator)
- `agentMode: ACPSessionModeId` (e.g., `.claude_code` or `.codex`)
- `prompt: String` (concise, task‑oriented)
- `estimatedDuration: TimeInterval`
- `rationale: String` (1–2 sentences)
- `confidence: Double` (0.0–1.0)

---

## Decision Heuristic (Baseline)

1) If `fileFrequency` has a clear top file (≥2 references) or `userIntent` hints at “refactor/cleanup”, choose a refactor task → `.claude_code`.
2) Else choose a test‑generation task → `.codex`.
3) Clamp `estimatedDuration` to the provided budget (e.g., min(2400, budget)).
4) Confidence defaults: refactor = 0.7, tests = 0.8. Adjust ±0.05 based on signal.

---

## Optional FM Assist (When Available)

If Foundation Models are available (macOS 26+), call `ExploreOrchestrator.fmAnalysis()` after a lightweight exploration loop to enrich the rationale and nudge the heuristic (+/‑ confidence). Keep the final decision bounded by the same rules above.

Reference: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ExploreOrchestrator.swift` (method `fmAnalysis()`)

---

## Pseudocode (uses existing classes)

```swift
import Foundation

struct DecisionOutput: Sendable {
    let agentMode: ACPSessionModeId
    let prompt: String
    let estimatedDuration: TimeInterval
    let rationale: String
    let confidence: Double
}

enum DecisionEngineError: Error { case noSignal }

actor DecisionEngine {
    func analyzeSessions() async throws -> SessionAnalyzeResult {
        let tool = SessionAnalyzeTool()
        return try await tool.analyze(sessionIds: [], provider: nil, metrics: nil)
    }

    func decideNextTask(
        from insights: SessionAnalyzeResult,
        timeBudgetSeconds: TimeInterval
    ) async throws -> DecisionOutput {
        // Baseline: choose refactor vs tests using existing signals
        let topFile = insights.fileFrequency?.sorted { $0.value > $1.value }.first?.key
        let intent = insights.userIntent?.lowercased()
        let preferRefactor = (insights.fileFrequency?.values.max() ?? 0) >= 2
            || (intent?.contains("refactor") == true || intent?.contains("cleanup") == true)

        if preferRefactor, let file = topFile {
            return DecisionOutput(
                agentMode: .claude_code,
                prompt: "Refactor \(file) to improve error handling and clarity. Keep behavior identical; reduce duplication and add guard statements where needed.",
                estimatedDuration: min(2400, timeBudgetSeconds),
                rationale: "\(file) appears most in recent sessions and users requested quality improvements.",
                confidence: 0.72
            )
        } else {
            let fileHint = topFile.map { " Focus on \($0) if feasible." } ?? ""
            return DecisionOutput(
                agentMode: .codex,
                prompt: "Generate focused unit tests for critical components. Aim to increase coverage in the most referenced files first.\(fileHint)",
                estimatedDuration: min(2400, timeBudgetSeconds),
                rationale: "Tests add safety; recent sessions indicate repetitive edits warranting coverage.",
                confidence: 0.80
            )
        }
    }
}
```

Notes:
- `SessionAnalyzeTool` and `SessionAnalyzeResult` are implemented today (Phase 2.5 tools).
- `ACPSessionModeId` maps to currently registered providers via `AgentRegistry`.
- Prompts are intentionally concise to avoid excessive agent drift.

---

## Integration Points

- Runner calls:
  - `let insights = try await DecisionEngine().analyzeSessions()`
  - `let decision = try await DecisionEngine().decideNextTask(from: insights, timeBudgetSeconds: 1800)`
  - Pass `decision.agentMode` and `decision.prompt` to `AgentProvider.start(sessionId:prompt:context:updateHub:)` via `AgentCoordinator`.
- Stream all outputs and rationale through `SessionUpdateHub` (existing).

---

## Acceptance Criteria

- Uses only existing shipped classes (no FM wrapper classes, no new DBs)
- Heuristic produces stable, bounded prompts for both paths (refactor/tests)
- Agent mapping uses `ACPSessionModeId` (.claude_code/.codex)
- Optional FM assist via `ExploreOrchestrator.fmAnalysis()` enriches rationale when available
- Unit tests: deterministic outcomes given crafted `SessionAnalyzeResult` fixtures

---

## Testing

- `testDecideRefactor_whenTopFileFrequent()` → returns `.claude_code` with refactor prompt
- `testDecideTests_whenNoStrongRefactorSignal()` → returns `.codex` with tests prompt
- `testRespectTimeBudget()` → clamps `estimatedDuration`
- `testConfidenceBounds()` → 0.0 ≤ confidence ≤ 1.0
- FM‑available path: skip if FM not available (macOS 26+)

---

## Post‑Demo Enhancements

- Incorporate repo signals (status/coverage/complexity) once implemented
- Learn weights from historical success (revisit confidence scaling)
- Expand decision templates to include documentation/update‑changelog tasks when user intent signals it

