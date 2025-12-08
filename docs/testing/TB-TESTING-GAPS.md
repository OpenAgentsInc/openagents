# Terminal Bench Testing Gap Analysis

**Date**: 2025-12-07
**Status**: Analysis Complete

## Executive Summary

This document identifies gaps between the Terminal Bench user stories (TBCC-001 to TBCC-033) and the current implementation/testing status.

## Current Status

### ✅ Fully Implemented & Tested

**TBCC E2E Tests** (`src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`):
- ✅ TBCC-001: Dashboard visible
- ✅ TBCC-002: KPIs displayed (Success Rate, Total Runs)
- ✅ TBCC-003: Recent runs table
- ✅ TBCC-004: Start Benchmark button
- ✅ TBCC-010: Browse tasks
- ✅ TBCC-011: Filter buttons exist
- ✅ TBCC-012: Search input exists
- ✅ TBCC-013: Task items clickable
- ✅ TBCC-014: Task details available
- ✅ TBCC-020: Local run history
- ✅ TBCC-021: HF Trajectories tab
- ✅ TBCC-022: Run details view
- ✅ TBCC-023: Terminal output section
- ✅ TBCC-030: Execution settings
- ✅ TBCC-031: Logging settings
- ✅ TBCC-032: Settings persistence
- ✅ TBCC-033: Reset defaults button

**TB Controls Tests** (`src/effuse/widgets/tb-controls.test.ts`):
- ✅ US-14.1: Run TB_10 subset
- ✅ US-14.2: Run random task
- ✅ US-14.3: Run specific task
- ✅ US-14.4: Error handling

## 🔴 Critical Gaps (P0)

### 1. TBCC-005: Navigate to Run from Dashboard
**Status**: ❌ Not Tested
**User Story**: As a user, I can navigate to a run from the dashboard
**Acceptance Criteria**: Clicking a recent run switches to Run Browser and selects it

**Gap**:
- Current test only checks if button exists (`button[data-run-id]`)
- No test verifies navigation/tab switching
- No test verifies run selection in Run Browser after navigation

**Required Test**:
```typescript
it("TBCC-005: Navigate to run from dashboard", async () => {
  // 1. Mount Dashboard and Run Browser
  // 2. Click on a run in Dashboard
  // 3. Verify shell emits event to switch to "runs" tab
  // 4. Verify Run Browser receives selectedRunId
  // 5. Verify Run Browser displays the selected run details
})
```

### 2. TBCC-013: View Task Details
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can view task details
**Acceptance Criteria**: Selecting a task shows description, timeout, and tags

**Gap**:
- Current test only checks that task items exist
- No test actually selects a task and verifies detail view
- No test verifies description, timeout, tags are displayed

**Required Test**:
```typescript
it("TBCC-013: View task details", async () => {
  // 1. Mount Task Browser
  // 2. Click on a task
  // 3. Wait for selectedTaskId state update
  // 4. Verify detail panel shows:
  //    - Task description
  //    - Timeout value
  //    - Tags array
  //    - Run button
})
```

### 3. TBCC-014: Run Specific Task
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can run a specific task
**Acceptance Criteria**: "Run Task" button in details view initiates execution

**Gap**:
- Current test only checks button exists
- No test verifies socket.startTBRun is called with correct taskId
- No test verifies run starts and UI updates

**Required Test**:
```typescript
it("TBCC-014: Run specific task", async () => {
  // 1. Mount Task Browser with spy on socket.startTBRun
  // 2. Select a task
  // 3. Click "Run Task" button
  // 4. Verify socket.startTBRun called with correct taskId
  // 5. Verify loading state or success message
})
```

### 4. TBCC-022: View Run Details
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can view run details
**Acceptance Criteria**: Selecting a run shows step-by-step execution details

**Gap**:
- Current test only checks basic content exists
- No test verifies step-by-step execution details
- No test verifies task results are displayed

**Required Test**:
```typescript
it("TBCC-022: View run details with steps", async () => {
  // 1. Mount Run Browser with detailed mock data
  // 2. Select a run
  // 3. Verify run details show:
  //    - Task list with outcomes
  //    - Execution steps
  //    - Timestamps
  //    - Token usage
})
```

### 5. TBCC-032: Settings Persistence
**Status**: ⚠️ Partially Tested
**User Story**: As a user, settings are persisted
**Acceptance Criteria**: Settings saved to local storage and restored on load

**Gap**:
- Current test only checks default values
- No test verifies localStorage save/load
- No test verifies settings persist across widget remounts

**Required Test**:
```typescript
it("TBCC-032: Settings persistence", async () => {
  // 1. Mount Settings widget
  // 2. Change a setting value
  // 3. Verify localStorage.setItem called
  // 4. Unmount and remount widget
  // 5. Verify setting value restored from localStorage
})
```

## ⚠️ High Priority Gaps (P1)

### 6. TBCC-011: Filter Tasks by Difficulty
**Status**: ❌ Not Tested
**User Story**: As a user, I can filter tasks by difficulty
**Acceptance Criteria**: Filter buttons (Easy, Medium, Hard) update the list

**Gap**: Interactive filtering not tested (simplified in current tests)

### 7. TBCC-012: Search Tasks
**Status**: ❌ Not Tested
**User Story**: As a user, I can search tasks by name
**Acceptance Criteria**: Search input filters the task list in real-time

**Gap**: Search functionality not tested (simplified in current tests)

### 8. TBCC-024: Filter Runs by Status
**Status**: ❌ Not Implemented
**User Story**: As a user, I can filter runs by status
**Acceptance Criteria**: Filter by passed/failed/running

**Gap**:
- No filter UI in Run Browser widget
- No state for status filter
- No filtering logic

### 9. TBCC-002: KPI Calculations
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can see key performance indicators
**Acceptance Criteria**: Pass rate, total runs, and average duration displayed

**Gap**:
- Test only checks text exists
- No test verifies calculations are correct
- No test with multiple runs of different outcomes

### 10. TBCC-004: Start Benchmark
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can quickly start a benchmark run
**Acceptance Criteria**: "Run Full Benchmark" button initiates a run

**Gap**:
- Test only checks button exists
- No test verifies socket.startTBRun is called
- No test verifies run starts and currentRun state updates

## 📋 Medium Priority Gaps (P2)

### 11. TBCC-023: Terminal Output
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can see terminal output for a run
**Acceptance Criteria**: Terminal output tab/section in details view

**Gap**:
- Test only checks "Execution Steps" text exists
- No test with actual terminal output data
- No test verifies output formatting

### 12. TBCC-033: Reset Settings
**Status**: ⚠️ Partially Tested
**User Story**: As a user, I can reset settings to default
**Acceptance Criteria**: "Reset Defaults" button restores original values

**Gap**:
- Test only checks button exists
- No test verifies reset functionality
- No test verifies all settings return to defaults

## 🔧 Backend/Integration Gaps

### 13. TB Run Execution
**Status**: ⚠️ Partially Tested
**Location**: `src/desktop/handlers.ts:startTBRun`

**Gaps**:
- No integration test for actual TB run execution
- No test for run status updates via WebSocket
- No test for run completion events

### 14. TB Suite Loading
**Status**: ⚠️ Partially Tested
**Location**: `src/desktop/handlers.ts:loadTBSuite`

**Gaps**:
- No test for suite file parsing
- No test for invalid suite files
- No test for missing task files

### 15. Run History Loading
**Status**: ⚠️ Partially Tested
**Location**: `src/desktop/handlers.ts:loadRecentTBRuns`

**Gaps**:
- No test for run history persistence
- No test for run history limits
- No test for corrupted run data

## 📊 Test Coverage Summary

| Category | Total Stories | Fully Tested | Partially Tested | Not Tested | Coverage |
|----------|---------------|--------------|------------------|------------|----------|
| Dashboard (TBCC-001 to 005) | 5 | 3 | 2 | 0 | 60% |
| Task Browser (TBCC-010 to 014) | 5 | 2 | 2 | 1 | 40% |
| Run Browser (TBCC-020 to 024) | 5 | 2 | 2 | 1 | 40% |
| Settings (TBCC-030 to 033) | 4 | 2 | 2 | 0 | 50% |
| **Total** | **19** | **9** | **8** | **2** | **47%** |

## 🎯 Recommended Action Plan

### Phase 1: Critical P0 Gaps (Immediate)
1. ✅ Fix TBCC-005: Navigation from Dashboard to Run Browser
2. ✅ Fix TBCC-013: Task detail view interaction
3. ✅ Fix TBCC-014: Run task with socket verification
4. ✅ Fix TBCC-022: Run details with step data
5. ✅ Fix TBCC-032: Settings persistence with localStorage

**Estimated Effort**: 4-6 hours
**Impact**: Brings P0 coverage from 60% to 100%

### Phase 2: High Priority P1 Gaps (Next Sprint)
1. Implement TBCC-011: Interactive difficulty filtering
2. Implement TBCC-012: Real-time search
3. Implement TBCC-024: Run status filtering
4. Enhance TBCC-002: KPI calculation verification
5. Enhance TBCC-004: Benchmark start verification

**Estimated Effort**: 6-8 hours
**Impact**: Brings P1 coverage from 40% to 100%

### Phase 3: Integration Tests (Following Sprint)
1. TB run execution end-to-end
2. WebSocket event flow for runs
3. Suite loading and validation
4. Run history persistence

**Estimated Effort**: 8-10 hours
**Impact**: Full backend integration coverage

### Phase 4: Polish & P2 Features
1. Terminal output formatting
2. Settings reset verification
3. Edge cases and error scenarios

**Estimated Effort**: 4-6 hours
**Impact**: Complete test coverage

## 📝 Notes

- Current E2E tests were simplified to avoid timeout issues with browser interactions
- Interactive tests (filtering, search, selection) need proper event simulation
- localStorage mocking may be needed for settings persistence tests
- Integration tests will require actual TB suite files and run data

## 🔗 Related Files

- User Stories: `docs/testing/USER-STORIES.md` (lines 461-503)
- E2E Tests: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`
- Widget Implementations:
  - `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts`
  - `src/effuse/widgets/tb-command-center/tbcc-task-browser.ts`
  - `src/effuse/widgets/tb-command-center/tbcc-run-browser.ts`
  - `src/effuse/widgets/tb-command-center/tbcc-settings.ts`
- Backend Handlers: `src/desktop/handlers.ts`
- Protocol: `src/desktop/protocol.ts`
