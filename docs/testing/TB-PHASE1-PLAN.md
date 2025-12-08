# Terminal Bench Testing - Phase 1 Implementation Plan

**Goal**: Close all P0 testing gaps for TBCC user stories
**Timeline**: Immediate (4-6 hours)
**Target Coverage**: 100% of P0 stories

## Tasks

### Task 1: TBCC-005 - Dashboard to Run Browser Navigation ✅

**File**: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`

**Implementation**:
```typescript
it("TBCC-005: Navigate to run from dashboard", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          // Mount shell to test tab navigation
          const shellHandle = yield* harness.mount(TBCCShellWidget, {
            containerId: "tbcc-shell",
          })

          // Mount dashboard
          const dashboardHandle = yield* harness.mount(TBCCDashboardWidget, {
            containerId: "tbcc-tab-dashboard",
          })
          yield* dashboardHandle.waitForState((s) => !s.loading)

          // Click on a run
          yield* browser.click("button[data-action='viewRun'][data-run-id='run-1']")

          // Verify shell switches to runs tab
          yield* shellHandle.waitForState((s) => s.activeTab === "runs")

          // Verify run browser receives the run ID
          const runBrowserHandle = yield* harness.mount(TBCCRunBrowserWidget, {
            containerId: "tbcc-tab-runs",
          })
          yield* runBrowserHandle.waitForState((s) => s.selectedRunId === "run-1")

          const html = yield* runBrowserHandle.getHTML
          expect(html).toContain("run-1")
          expect(html).toContain("Fix Bug")
        }).pipe(
          Effect.provideService(SocketServiceTag, createMockSocket()),
          Effect.provide(layer)
        )
      })
    )
  )
})
```

**Dependencies**:
- Shell widget event handling for viewRun events
- Dashboard emits viewRun event on click
- Run Browser listens for external run selection

**Status**: ⏳ Requires shell widget integration

---

### Task 2: TBCC-013 - Task Detail View Interaction ✅

**File**: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`

**Implementation**:
```typescript
it("TBCC-013: View task details - interactive", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          const taskHandle = yield* harness.mount(TBCCTaskBrowserWidget, {
            containerId: "tbcc-tab-tasks",
          })
          yield* taskHandle.waitForState((s) => !s.loading)

          // Click on a task
          yield* browser.click("div[data-action='selectTask'][data-task-id='task-1']")

          // Wait for selection
          yield* taskHandle.waitForState((s) => s.selectedTaskId === "task-1")

          // Verify detail view
          const html = yield* taskHandle.getHTML
          expect(html).toContain("Fix a critical bug") // Description
          expect(html).toContain("300s") // Timeout
          expect(html).toContain("bug") // Tag
          expect(html).toContain("urgent") // Tag
          expect(html).toContain("50") // Max turns
          expect(html).toContain("Run Task") // Action button
        }).pipe(
          Effect.provideService(SocketServiceTag, createMockSocket()),
          Effect.provide(layer)
        )
      })
    )
  )
})
```

**Status**: ⚠️ Needs browser interaction fix (currently simplified)

---

### Task 3: TBCC-014 - Run Task with Socket Verification ✅

**File**: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`

**Implementation**:
```typescript
it("TBCC-014: Run specific task with verification", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          // Create spy for socket.startTBRun
          let startTBRunCalled = false
          let capturedOptions: any = null

          const mockSocketWithSpy = (): SocketService => ({
            ...createMockSocket(),
            startTBRun: (options) => {
              startTBRunCalled = true
              capturedOptions = options
              return Effect.succeed({ runId: "new-run-123" })
            }
          })

          const taskHandle = yield* harness.mount(TBCCTaskBrowserWidget, {
            containerId: "tbcc-tab-tasks",
          })
          yield* taskHandle.waitForState((s) => !s.loading)

          // Select task
          yield* browser.click("div[data-action='selectTask'][data-task-id='task-1']")
          yield* taskHandle.waitForState((s) => s.selectedTaskId === "task-1")

          // Click run button
          yield* browser.click("button[data-action='runTask'][data-task-id='task-1']")

          // Verify socket call
          expect(startTBRunCalled).toBe(true)
          expect(capturedOptions.taskIds).toContain("task-1")
          expect(capturedOptions.suitePath).toBe("tasks/terminal-bench-2.json")
        }).pipe(
          Effect.provideService(SocketServiceTag, mockSocketWithSpy()),
          Effect.provide(layer)
        )
      })
    )
  )
})
```

**Status**: ⚠️ Needs browser interaction fix

---

### Task 4: TBCC-022 - Run Details with Step Data ✅

**File**: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`

**Implementation**:
```typescript
it("TBCC-022: View run details with execution steps", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          // Enhanced mock with step data
          const mockWithSteps = (): SocketService => ({
            ...createMockSocket(),
            loadTBRunDetails: () => Effect.succeed({
              meta: MOCK_RUNS[0],
              tasks: [
                {
                  id: "task-1",
                  name: "Fix Bug",
                  category: "Debugging",
                  difficulty: "hard",
                  outcome: "success",
                  turns: 10,
                  durationMs: 5000,
                  tokens: 1000,
                }
              ],
              steps: [
                {
                  index: 0,
                  action: "read_file",
                  timestamp: new Date().toISOString(),
                  success: true,
                },
                {
                  index: 1,
                  action: "edit_file",
                  timestamp: new Date().toISOString(),
                  success: true,
                },
              ],
            } as any),
          })

          const runHandle = yield* harness.mount(TBCCRunBrowserWidget, {
            containerId: "tbcc-tab-runs",
          })
          yield* runHandle.waitForState((s) => !s.loading)

          // Select run
          yield* browser.click("div[data-run-id='run-1']")
          yield* runHandle.waitForState((s) => s.selectedRunId === "run-1")

          const html = yield* runHandle.getHTML

          // Verify task results
          expect(html).toContain("Fix Bug")
          expect(html).toContain("success")
          expect(html).toContain("10") // turns
          expect(html).toContain("1000") // tokens

          // Verify steps section
          expect(html).toContain("Execution Steps")
          expect(html).toContain("read_file")
          expect(html).toContain("edit_file")
        }).pipe(
          Effect.provideService(SocketServiceTag, mockWithSteps()),
          Effect.provide(layer)
        )
      })
    )
  )
})
```

**Status**: ⚠️ Needs TBRunDetails type update to include steps

---

### Task 5: TBCC-032 - Settings Persistence with localStorage ✅

**File**: `src/effuse/widgets/tb-command-center/tbcc.e2e.test.ts`

**Implementation**:
```typescript
it("TBCC-032: Settings persistence across remounts", async () => {
  await Effect.runPromise(
    Effect.scoped(
      Effect.gen(function* () {
        const { layer, window } = yield* makeHappyDomLayer()

        yield* Effect.gen(function* () {
          const harness = yield* TestHarnessTag
          const browser = yield* TestBrowserTag

          // Mock localStorage
          const storage: Record<string, string> = {}
          window.localStorage = {
            getItem: (key: string) => storage[key] || null,
            setItem: (key: string, value: string) => { storage[key] = value },
            removeItem: (key: string) => { delete storage[key] },
            clear: () => { Object.keys(storage).forEach(k => delete storage[k]) },
            length: 0,
            key: () => null,
          }

          // Mount settings
          const settingsHandle1 = yield* harness.mount(TBCCSettingsWidget, {
            containerId: "tbcc-tab-settings",
          })

          // Change a setting
          yield* browser.type("input[data-key='maxAttempts']", "10")
          yield* browser.click("button[data-action='save']")

          // Verify localStorage was called
          expect(storage["tbcc-execution-settings"]).toBeDefined()
          const saved = JSON.parse(storage["tbcc-execution-settings"])
          expect(saved.maxAttempts).toBe(10)

          // Unmount
          yield* settingsHandle1.unmount()

          // Remount
          const settingsHandle2 = yield* harness.mount(TBCCSettingsWidget, {
            containerId: "tbcc-tab-settings",
          })

          // Verify setting restored
          const html = yield* settingsHandle2.getHTML
          const input = yield* browser.query("input[data-key='maxAttempts']")
          expect(input.getAttribute("value")).toBe("10")
        }).pipe(
          Effect.provideService(SocketServiceTag, createMockSocket()),
          Effect.provide(layer)
        )
      })
    )
  )
})
```

**Status**: ⚠️ Needs localStorage mock in happy-dom layer

---

## Implementation Order

1. **Task 4** (TBCC-022) - Easiest, just needs mock data enhancement
2. **Task 5** (TBCC-032) - Requires localStorage mock setup
3. **Task 2** (TBCC-013) - Requires browser interaction fix
4. **Task 3** (TBCC-014) - Requires browser interaction fix + spy
5. **Task 1** (TBCC-005) - Most complex, requires shell integration

## Blockers

### Browser Interaction Timeout Issue
**Problem**: Interactive tests with `browser.click()` and `browser.type()` are timing out
**Root Cause**: Event delegation or state updates not working in happy-dom
**Solutions**:
1. Fix event handling in happy-dom test layer
2. Use direct state manipulation instead of browser interactions
3. Use different test approach (e.g., direct widget event emission)

**Recommended**: Option 3 - Emit events directly to widget instead of simulating browser clicks

### localStorage Mock
**Problem**: happy-dom may not have full localStorage implementation
**Solution**: Add localStorage mock to makeHappyDomLayer

### TBRunDetails Type
**Problem**: Current type doesn't include steps array
**Solution**: Update protocol.ts to add optional steps field

## Success Criteria

- [ ] All 5 P0 gap tests implemented
- [ ] All tests passing
- [ ] No timeouts
- [ ] Coverage report shows 100% P0 coverage
- [ ] Documentation updated

## Next Steps After Phase 1

1. Address P1 gaps (filtering, search)
2. Add integration tests for backend
3. Add error scenario tests
4. Performance testing for large datasets
