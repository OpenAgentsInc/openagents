# Overnight orchestration tests still run in real time

**Date**: 2025-11-11 01:26 local
**Topic**: Overnight orchestration test strategy
**Scope**: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/*`, `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift`
**Config IDs**: n/a

## Executive Summary
- Scheduler integration tests literally sleep 2–7 minutes per case, so a full suite exceeds 20 minutes and is never part of normal CI/dev loops.
- The orchestration log confirms expectations of ~22–25 minute test runs, meaning current coverage is aspirational rather than enforced.
- SchedulerService hard-codes real clocks and Task.sleep, so there is no way to swap in a virtual clock or deterministic timer for tests.
- Without a controllable clock, the team cannot add regressions tests for catch-up policies, jitter, or constraint retries without further slowing the suite.

## Context Reviewed
- Prior audits: `docs/audits/20251111/0105/scheduler-lifecycle-visibility.md` (most recent scheduler audit, currently empty) and `docs/audits/20251111/0930/README.md` to avoid overlap.
- Issues considered: `gh issue list` is unavailable due to restricted network, so I used `docs/logs/20251110/2248-overnight-orchestration-implementation.md` for current-state signals.
- Code/docs touched: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift`, `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`, `docs/logs/20251110/2248-overnight-orchestration-implementation.md`.

## Findings

1. **Integration tests depend on multi-minute real time delays.**
   - *Problem*: Every scheduler integration test waits real minutes (2–7) using `Task.sleep`, so running the file takes well over 20 minutes. Developers and CI skip these tests, leaving overnight orchestration effectively untested.
   - *Evidence*: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift:96` (7-minute sleep), `:145` (3 minutes), `:203` (2 minutes), `:241` (4 minutes), `:355` and `:394` (150-second waits). The implementation log expects ~22–25 minutes for integration tests: `docs/logs/20251110/2248-overnight-orchestration-implementation.md:303` and `:580`.
   - *Impact*: Nobody will block on a 20+ minute XCTest run, so regressions in scheduler timing, constraint handling, or coordinator wiring will hit production before being detected. This undermines the overnight autonomy story.
   - *Recommendation*: Replace real sleeps with a deterministic test clock. Options include (a) extracting a `Clock` dependency (similar to Swift's `Clock` protocol) so tests can advance virtual time instantly, or (b) splitting SchedulerService into a pure schedule calculator plus a shell that runs real timers, letting tests cover the pure logic synchronously. Rework the integration tests to run under seconds.

2. **SchedulerService has no seam for injecting clocks or timers.**
   - *Problem*: SchedulerService captures `Date()` and calls `Task.sleep` directly in production code, including a fixed 5-minute retry when constraints fail. There is no hook for tests (or simulator UI) to control timing, forcing the slow tests above and making new coverage (e.g., retry policy) prohibitively expensive.
   - *Evidence*: Real clock usage happens at `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift:64-82` (calculating next wake and sleeping) and `:107-115` (fixed 5-minute constraint retry sleep). There is no dependency injection for a clock or timer anywhere in the actor.
   - *Impact*: Runtime-only validation is risky: if cron parsing, jitter, or pause/resume logic regresses, you need multi-minute manual tests to catch it. It also prevents unit tests from exercising edge cases like crossing midnight, repeated constraint failures, or jitter randomness.
   - *Recommendation*: Introduce abstractions (`Clock`, `Sleeper`, or `SchedulerTimer`) that SchedulerService accepts via initializer, defaulting to real time in production. Tests can provide a mock that advances immediately. Once in place, rework retry/backoff logic to rely on that abstraction so policy coverage is feasible.

## Proposed Changes / Work Items
- **"Make SchedulerService testable with a virtual clock"**
  - *Acceptance criteria*: SchedulerService takes a clock/timer dependency with production defaults; integration tests replace it to advance time without waiting; constraint retry waits are configurable/injectable; total runtime of `OvernightOrchestrationIntegrationTests` < 15 seconds on CI hardware.
  - *Implementation notes*: Define a small protocol (e.g., `protocol SchedulerClock { func now() -> Date; func sleep(_:) async throws }`). Provide `RealSchedulerClock` and `TestSchedulerClock`. Thread through via `SchedulerService` init or configure().
- **"Rewrite overnight orchestration integration tests to use the virtual clock"**
  - *Acceptance criteria*: Each test asserts the same behaviors (jitter spread, window enforcement, coordinator wiring) but runs deterministically without `Task.sleep` > 100ms; suite can run inside pull-request CI by default; document how to add new cases.
  - *Implementation notes*: Build helper harness that steps the virtual clock and drains pending timers. Use expectation counters instead of real-time waits.

## Next Run Handoff
- Verify that a virtual clock abstraction lands and that integration tests are updated to use it.
- Once clock injection exists, add coverage for `onMissed = run_once_at_next_opportunity` and constraint retries without sleeping.
- Sanity-check CI configs to ensure the updated test target runs on every PR.
