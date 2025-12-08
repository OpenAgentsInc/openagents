# Phase 2 Implementation Plan - Updated

**Date**: 2025-12-07
**Status**: In Progress

## Revised Approach

After analyzing the browser interaction timeout issue, I've determined that fixing the happy-dom event delegation is complex and may introduce regressions. Instead, I'll use the **direct event emission pattern** that proved successful in Phase 1.

## Remaining P0 Tests

### 1. TBCC-013: View Task Details (Interactive) ✅ CAN IMPLEMENT

**Approach**: Use direct event emission instead of browser clicks
```typescript
// Instead of: yield* browser.click("div[data-task-id='task-1']")
// Use: yield* taskHandle.emit({ type: "selectTask", taskId: "task-1" })
```

**Implementation**:
- Emit `selectTask` event directly
- Wait for `selectedTaskId` state update
- Verify detail panel content

**Estimated Time**: 10 minutes

### 2. TBCC-014: Run Specific Task ✅ CAN IMPLEMENT

**Approach**: Combine direct emission with spy pattern
```typescript
// Emit selectTask, then emit runTask
yield* taskHandle.emit({ type: "selectTask", taskId: "task-1" })
yield* taskHandle.emit({ type: "runTask", taskId: "task-1" })
```

**Implementation**:
- Use spy pattern from TBCC-004
- Verify socket.startTBRun called with correct taskId
- Verify task suite path

**Estimated Time**: 10 minutes

### 3. TBCC-032: Settings Persistence ✅ CAN IMPLEMENT

**Approach**: Mock localStorage in test
```typescript
const storage: Record<string, string> = {}
const mockLocalStorage = {
  getItem: (key: string) => storage[key] || null,
  setItem: (key: string, value: string) => { storage[key] = value },
  // ...
}
```

**Implementation**:
- Create localStorage mock
- Emit settings update events
- Verify storage calls
- Remount and verify restoration

**Estimated Time**: 15 minutes

### 4. TBCC-005: Navigate from Dashboard ⏸️ DEFER

**Blocker**: Requires shell widget integration
- Shell widget needs to handle `viewRun` events
- Shell needs to switch tabs
- Run Browser needs to receive run selection

**Recommendation**: Defer to separate task focused on shell widget event wiring

## Implementation Order

1. **TBCC-013** - Task detail view (easiest, reuses patterns)
2. **TBCC-014** - Run task (builds on TBCC-013)
3. **TBCC-032** - Settings persistence (new pattern, most complex)
4. **TBCC-005** - Deferred (requires shell work)

## Expected Outcomes

After implementing tests 1-3:
- **Coverage**: 63% → **79%** (+16%)
- **Dashboard**: 100% (unchanged)
- **Task Browser**: 40% → **100%** (+60%)
- **Settings**: 50% → **75%** (+25%)
- **Fully Tested**: 12/19 → **15/19** (+3)

## Time Estimate

- TBCC-013: 10 min
- TBCC-014: 10 min
- TBCC-032: 15 min
- Documentation: 5 min
- **Total**: 40 minutes

## Success Criteria

- [ ] All 3 new tests passing
- [ ] No test timeouts
- [ ] Coverage reaches 79%
- [ ] Task Browser reaches 100% coverage
- [ ] Documentation updated
