# Overnight Orchestration - Next Steps

**Date**: 2025-11-10
**Priority**: P0 (Critical Path)
**Goal**: Enable overnight coding with Codex delegation and PR creation

---

## Phase 1: Minimal Viable Overnight (5-7 days)

### 1. Implement Constraint Checking (1-2 days)

**Goal**: Prevent overnight runs when battery is low or on cellular.

**Tasks**:

1. **Create ConstraintCheckers.swift**
   - File: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConstraintCheckers.swift`
   - Implement `ConstraintChecker` protocol
   - Implement `PluggedInChecker` using IOKit
   - Implement `WiFiOnlyChecker` using NWPathMonitor
   - Stub `CPUChecker`, `DoNotDisturbChecker`, `UserActivityChecker` for future

2. **Integrate into SchedulerService**
   - Modify: `ios/OpenAgentsCore/Sources/OpenAgensCore/Orchestration/SchedulerService.swift`
   - Add `constraintsSatisfied()` method
   - Call before triggering orchestration cycle
   - Log constraint failures for debugging

3. **Add Tests**
   - File: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/ConstraintCheckersTests.swift`
   - Mock IOKit battery API responses
   - Mock NWPathMonitor network states
   - Test constraint logic (plugged in, WiFi only)

**Acceptance Criteria**:
- [ ] SchedulerService skips cycle when not plugged in
- [ ] SchedulerService skips cycle when on cellular (if wifiOnly: true)
- [ ] Constraint checks complete in <500ms
- [ ] Tests pass with ≥90% coverage

**Implementation Reference**: See `docs/overnight/issues/001-scheduler-service.md` lines 143-235

---

### 2. Implement PR Automation Service (2-3 days)

**Goal**: Create GitHub PRs automatically after overnight agent runs.

**Tasks**:

1. **Create PRAutomationService.swift**
   - File: `ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift`
   - Implement `gh` CLI discovery (reuse CLIAgentProvider.findBinary pattern)
   - Implement `createBranch(from:name:)` - uses `git checkout -b`
   - Implement `commitWorkingTree(message:)` - uses `git add . && git commit`
   - Implement `push(branch:remote:)` - uses `git push -u`
   - Implement `createPR(title:body:branch:)` - uses `gh pr create`

2. **Integrate into AgentCoordinator**
   - Modify: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift`
   - Add PR creation after task completion
   - Update task metadata with PR number and branch name
   - Handle errors gracefully (log and continue)

3. **Add Tests**
   - File: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/GitHubIntegration/PRAutomationServiceTests.swift`
   - Test gh discovery logic
   - Test branch creation
   - Test commit generation
   - Test PR creation (mock gh CLI)
   - Integration test with skip guard for gh availability

**Acceptance Criteria**:
- [ ] PRAutomationService discovers gh CLI from PATH
- [ ] Creates branches with naming pattern `agent/{session_id}`
- [ ] Commits working tree with decision task as message
- [ ] Creates PR with orchestration metadata in body
- [ ] Updates task metadata with PR number
- [ ] Tests pass (skip if gh not available)

**Implementation Reference**:
- `docs/overnight/README.md` lines 276-286 (PRAutomationService interface)
- `docs/overnight/issues/005-pr-automation-service.md`

---

### 3. Implement Time Window and Jitter Enforcement (0.5 days)

**Goal**: Only run orchestration within configured time windows with random jitter.

**Tasks**:

1. **Add Time Window Checking to SchedulerService**
   - Modify: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`
   - Add `isWithinWindow(_:window:)` method
   - Handle midnight crossing (e.g., 22:00 - 02:00)
   - Check before triggering cycle

2. **Add Jitter Application**
   - Modify: `SchedulerService.nextWake(for:from:)` method
   - Apply random delay 0-N seconds after scheduled time
   - Use `schedule.jitter_ms` from config

3. **Add Tests**
   - Modify: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/SchedulerServiceTests.swift` (create if needed)
   - Test window enforcement (inside/outside, midnight crossing)
   - Test jitter application (within bounds, random)

**Acceptance Criteria**:
- [ ] Scheduler only triggers within configured time window
- [ ] Handles midnight crossing correctly (e.g., 22:00 - 02:00)
- [ ] Applies random jitter (0 - config.schedule.jitter_ms)
- [ ] Tests pass with ≥90% coverage

**Implementation Reference**: Audit document section "Overnight Extension Architecture"

---

### 4. Implement Process Keep-Alive (1 day)

**Goal**: Prevent macOS from suspending app during overnight runs.

**Tasks**:

1. **Add Sleep Prevention to SchedulerService**
   - Modify: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift`
   - Add `preventSleep()` method using IOKit `IOPMAssertionCreateWithName`
   - Add `allowSleep()` method using `IOPMAssertionRelease`
   - Call `preventSleep()` in `start()`
   - Call `allowSleep()` in `stop()`

2. **Handle Sleep/Wake Cycles**
   - Listen for `NSWorkspace.willSleepNotification` and `NSWorkspace.didWakeNotification`
   - Reschedule timer on wake (compute next wake time from current time)
   - Log sleep/wake events for debugging

3. **Add Tests**
   - Test assertion creation/release
   - Test timer continuation after simulated wake

**Acceptance Criteria**:
- [ ] SchedulerService prevents system sleep during active runs
- [ ] Releases assertion on stop
- [ ] Handles sleep/wake cycles gracefully
- [ ] Tests pass

**Implementation Reference**: Audit document section "Process Keep-Alive"

---

### 5. Write End-to-End Integration Test (1 day)

**Goal**: Validate overnight orchestration flow end-to-end.

**Tasks**:

1. **Create Integration Test**
   - File: `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift`
   - Test compressed overnight run (10 min instead of 4 hours)
   - Test multiple orchestration cycles (3-5 cycles)
   - Test constraint checking integration
   - Test PR creation integration (mock gh CLI)
   - Test task queue updates
   - Test metrics tracking

2. **Add Test Utilities**
   - Mock constraint checkers (return true)
   - Mock gh CLI (return fake PR numbers)
   - Mock sleep (use fast intervals for testing)
   - Compressed schedule config (every 2 min instead of 30 min)

**Acceptance Criteria**:
- [ ] Integration test runs full overnight flow in <15 minutes
- [ ] Tests multiple cycles (analyze → decide → execute → PR)
- [ ] Validates task queue state transitions
- [ ] Validates PR creation
- [ ] Validates metrics (cycles run, tasks executed, PRs created)
- [ ] Test passes reliably

**Implementation Reference**: `docs/overnight/issues/010-e2e-testing.md`

---

### 6. Create Demo Config (0.5 days)

**Goal**: Provide working example orchestration config.

**Tasks**:

1. **Create nightly-refactor.json**
   - File: `docs/overnight/examples/nightly-refactor.json`
   - Schedule: Every 30 min, 1am-5am
   - Constraints: pluggedIn, wifiOnly
   - Goals: ["refactor error handling", "increase test coverage"]
   - Agent preferences: prefer Claude Code for refactoring, prefer Codex for tests
   - Time budget: 30 minutes per cycle

2. **Create Simple Config Loader**
   - File: `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConfigLoader.swift`
   - Load JSON from file path
   - Validate config
   - Return OrchestrationConfig struct

3. **Document Config Format**
   - Update: `docs/overnight/README.md`
   - Document subset of manifest format actually supported
   - Mark unsupported fields as "Future"

**Acceptance Criteria**:
- [ ] Example config loads without errors
- [ ] Config validation passes
- [ ] Documentation is clear and accurate

**Implementation Reference**: `docs/overnight/examples/nightly-refactor.json` (sample in README)

---

### 7. Update Documentation (0.5 days)

**Goal**: Document current implementation status.

**Tasks**:

1. **Expand architecture.md**
   - File: `docs/overnight/architecture.md`
   - Document constraint checking architecture
   - Document PR automation flow
   - Add sequence diagrams for overnight cycle
   - Mark implemented vs future components

2. **Create testing-plan.md**
   - File: `docs/overnight/testing-plan.md`
   - Document unit test coverage
   - Document integration test scenarios
   - Document manual testing procedures

3. **Update README.md**
   - File: `docs/overnight/README.md`
   - Update "Key Components" section with implementation status
   - Mark implemented components with ✅
   - Mark future components with ⏳

**Acceptance Criteria**:
- [ ] Architecture doc is comprehensive and accurate
- [ ] Testing plan covers all components
- [ ] README reflects current implementation status

---

## Phase 1 Timeline

| Task | Days | Dependencies |
|------|------|--------------|
| 1. Constraint Checking | 1-2 | None |
| 2. PR Automation Service | 2-3 | None |
| 3. Time Window & Jitter | 0.5 | Task 1 |
| 4. Process Keep-Alive | 1 | None |
| 5. Integration Test | 1 | Tasks 1-4 |
| 6. Demo Config | 0.5 | None |
| 7. Documentation | 0.5 | Tasks 1-6 |

**Total**: 6-8 days (assuming some parallel work)

**Critical Path**: Task 2 (PR Automation) is longest and has no dependencies, start immediately.

---

## Phase 1 Deliverable Checklist

### Code

- [ ] `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConstraintCheckers.swift` (new)
- [ ] `ios/OpenAgentsCore/Sources/OpenAgentsCore/GitHubIntegration/PRAutomationService.swift` (new)
- [ ] `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/SchedulerService.swift` (modified)
- [ ] `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/AgentCoordinator.swift` (modified)
- [ ] `ios/OpenAgentsCore/Sources/OpenAgentsCore/Orchestration/ConfigLoader.swift` (new)

### Tests

- [ ] `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Orchestration/ConstraintCheckersTests.swift` (new)
- [ ] `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/GitHubIntegration/PRAutomationServiceTests.swift` (new)
- [ ] `ios/OpenAgentsCore/Tests/OpenAgentsCoreTests/Integration/OvernightOrchestrationIntegrationTests.swift` (new)

### Documentation

- [ ] `docs/overnight/examples/nightly-refactor.json` (new)
- [ ] `docs/overnight/architecture.md` (expanded from stub)
- [ ] `docs/overnight/testing-plan.md` (new)
- [ ] `docs/overnight/README.md` (updated status)

### Validation

- [ ] All unit tests pass (≥90% coverage on new code)
- [ ] Integration test passes (compressed overnight run)
- [ ] Manual overnight test (8+ hour run on real hardware)
- [ ] macOS app remains responsive during runs
- [ ] Battery drain <5% per hour when plugged in
- [ ] 5-10 quality PRs created overnight

---

## Phase 2: Polish & iOS Monitoring (5-7 days) - Optional

**Defer to next sprint**. Phase 1 is sufficient for overnight demo.

**Scope**:
- iOS orchestration monitoring view
- Real-time task queue updates via bridge
- Decision rationale display
- Manual intervention controls
- PR preview before push
- Completion notifications

---

## Quick Start Guide (Post Phase 1)

### Setup

1. **Configure Orchestration**
   ```bash
   # Load demo config
   cp docs/overnight/examples/nightly-refactor.json ~/nightly-refactor.json

   # Or create custom config via macOS Settings UI
   ```

2. **Start Scheduler**
   ```bash
   # Via macOS app Settings > Orchestration
   # - Load config: ~/nightly-refactor.json
   # - Click "Start Scheduler"
   # - Verify status shows "Running, next wake: 01:00"
   ```

3. **Monitor Progress**
   ```bash
   # View logs
   tail -f ~/Library/Logs/OpenAgents/orchestration.log

   # Or use iOS app (Phase 2)
   ```

4. **Morning Review**
   ```bash
   # Check GitHub for PRs
   gh pr list --author "@me" --state open

   # Review orchestration metrics
   # macOS Settings > Orchestration > History
   ```

### Troubleshooting

**Scheduler not triggering**:
- Check constraints: Settings > Orchestration > Diagnostics
- Verify plugged in (battery icon)
- Verify WiFi (network icon)
- Check logs: `~/Library/Logs/OpenAgents/orchestration.log`

**No PRs created**:
- Check gh CLI availability: `which gh`
- Check GitHub authentication: `gh auth status`
- Check git status in workspace: `git status`
- Review task queue: Settings > Orchestration > Tasks

**App suspended during night**:
- Check sleep assertion: Diagnostics > Process Status
- Check macOS power settings: System Preferences > Energy Saver
- Verify app not force-quit

---

## Development Tips

### Testing Constraints

```bash
# Simulate battery power (macOS VM)
pmset -g batt

# Simulate WiFi disconnect
# System Preferences > Network > Wi-Fi > Turn Wi-Fi Off

# Simulate cellular (requires hardware)
# Use iPhone hotspot
```

### Testing PR Creation

```bash
# Use test repository
git clone git@github.com:your-username/test-repo.git
cd test-repo

# Run orchestration with compressed schedule (every 2 min)
# Check PRs created: gh pr list
```

### Debugging SchedulerService

```swift
// Add verbose logging
OpenAgentsLog.orchestration.debug("SchedulerService state: \(state)")
OpenAgentsLog.orchestration.debug("Next wake: \(nextWake)")
OpenAgentsLog.orchestration.debug("Constraints satisfied: \(satisfied)")

// Enable debug mode in Settings > Developer > Orchestration Debug
```

---

## Success Metrics (Phase 1)

### Quantitative

- [ ] Scheduler runs 8+ hours without crash
- [ ] 15-20 orchestration cycles triggered overnight (1am-5am, 30 min intervals)
- [ ] 5-10 PRs created with quality work
- [ ] ≥90% test coverage on new code
- [ ] <500ms constraint checking latency
- [ ] <5% battery drain per hour (when plugged in)

### Qualitative

- [ ] PRs contain meaningful refactoring or test additions
- [ ] Commit messages are clear and descriptive
- [ ] PR bodies include orchestration rationale
- [ ] No duplicate work (deduplication works)
- [ ] Logs are informative and actionable

---

## Post-Phase 1 Decisions

### Continue or Pivot?

After Phase 1 completes, evaluate:

1. **PR Quality**: Are overnight PRs useful and mergeable?
2. **Decision Logic**: Does heuristic decision-making work well enough?
3. **User Value**: Do users actually want overnight orchestration?

**If Yes**: Proceed to Phase 2 (iOS monitoring) and Phase 3 (manifests)
**If No**: Investigate decision logic improvements or pivot to different use case

### Potential Improvements

- FM-based decision logic (replace heuristics)
- File change tracking (replace commit-entire-tree)
- Advanced constraint checking (CPU, DND, user activity)
- Multi-device coordination (iOS initiates, macOS executes)
- Nostr marketplace integration

---

## References

- **Main Audit**: `docs/audits/20251110/2220/overnight-orchestration-audit.md`
- **Overnight Plan**: `docs/overnight/plan.md`
- **Overnight README**: `docs/overnight/README.md`
- **Issue Templates**: `docs/overnight/issues/001-*.md` through `012-*.md`
- **Previous Audit**: `docs/overnight/issues/999-audit-review-2025-11-08.md`

---

**Created**: 2025-11-10 22:20
**Next Review**: After Phase 1 completion (estimated 2025-11-17)
