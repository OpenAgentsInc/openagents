# Issue #006: Implement OrchiestrationRunner

**Component**: Orchiestration Runner
**Priority**: P1 (High)
**Estimated Effort**: 1-2 days (minimal scope for demo)
**Dependencies**: #002 (DecisionOrchestrator), #004 (AgentCoordinator)
**Assignee**: TBD

---

## Overview

**RENAMED from UpgradeExecutor per audit**: Create minimal `OrchiestrationRunner` that runs SessionAnalyzeTool → decide() → AgentCoordinator → PRAutomationService. Defer full declarative pipeline runtime to post-demo.

**Scope for Demo**: Simple runner with minimal JSON config (schedule + toggles only). Not full upgrade manifest schema.

**Location**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/OrchiestrationRunner.swift`

---

## Requirements (Demo Scope)

1. Read minimal JSON config (path, schedule, toggles) — not full upgrade schema.
2. Each cycle:
   - Run `SessionAnalyzeTool` (providers: both) to collect insights.
   - Call `DecisionOrchestrator.decideNextTask(...)` (FM or heuristic).
   - Delegate to `AgentCoordinator` to run the task (Claude Code or Codex).
   - Optionally call `PRAutomationService` to create a draft PR.
3. Stream progress via `SessionUpdateHub` for iOS monitoring.
4. Respect time budget and cancel on timeout.

---

## Implementation (Pseudocode)

```swift
actor OrchiestrationRunner {
  let taskQueue: TaskQueue
  let decision: DecisionOrchestrator
  let coordinator: AgentCoordinator
  let pr: PRAutomationService
  let updateHub: SessionUpdateHub

  func runCycle(workspace: String, budget: TimeInterval) async {
    // Analyze
    let insights = try? await SessionAnalyzeTool().analyze(sessionIds: [], provider: nil, metrics: nil)
    // Decide
    let dec = try await decision.decideNextTask(
      insights: toInsights(insights),
      availableAgents: [.claude_code, .codex],
      timeBudget: budget
    )
    // Execute
    let result = try await coordinator.delegate(dec)
    // PR (optional)
    _ = try? await pr.createDraftPR(from: dec, result: result)
  }
}
```

---

## Testing

1. `testRunCycle()` - Runs analyze→decide→execute path end-to-end (with mocks)
2. `testDecisionFallback()` - Heuristic when FM unavailable
3. `testTimeBudgetEnforcement()` - Cancel on timeout
4. `testDraftPRCreation()` - Creates draft PR (skipped if gh not authenticated)

---

## Acceptance Criteria

- [ ] Loads and validates manifest JSON
- [ ] Executes all built-in operations
- [ ] Variable substitution works
- [ ] Permission checks enforced
- [ ] Time budget enforced
- [ ] Tests pass (≥90% coverage)

---

## References

- docs/compute/issues/upgrades.md
- private/20251108-upgrades-convo/01.md
