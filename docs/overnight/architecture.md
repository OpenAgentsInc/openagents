# Overnight Orchestration — Architecture (Stub)

This document will capture the overnight orchestration architecture for the demo:

- SchedulerService (macOS-only, cron + window + jitter + minimal constraints)
- OvernightRunner (analyze → decide → execute → PR)
- DecisionOrchestrator (ExploreOrchestrator + SessionTools; FM when available)
- AgentCoordinator (delegates to AgentRegistry providers; streams via SessionUpdateHub)
- PRAutomationService (optional draft PRs; `gh` discovery via PATH)

Post-demo, expand with full upgrade manifest runtime and richer constraints.

