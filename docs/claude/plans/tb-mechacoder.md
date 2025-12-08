# Plan: Connect TB UI to FM MechaCoder with Live Streaming

## Goal
Trigger a real TB run with Foundation Model MechaCoder from the Effuse UI and see live updates streamed back.

## Current State (What Works)
- **TBCC Widgets**: Dashboard, TaskBrowser, RunBrowser, Settings - all tested (~89% coverage)
- **SocketService**: Full implementation for TB operations
- **Desktop Server**: WebSocket message routing working
- **CLI**: `tbench-local.ts` supports `--model fm|claude-code` flag
- **HUD Protocol**: All message types defined (`tb_run_start`, `tb_task_output`, etc.)
- **TBOutputWidget**: Exists and subscribes to streaming messages - **BUT NOT MOUNTED**

## Gaps to Fix

| Gap | File(s) | Description |
|-----|---------|-------------|
| 1. Model option not passed | `socket.ts`, `handlers.ts` | CLI supports `--model` but it's not in StartTBRunOptions |
| 2. TBOutputWidget not mounted | `effuse-main.ts` | Widget exists but isn't rendered in UI |
| 3. No model selector UI | `tbcc-settings.ts` | No way for user to choose FM vs Claude Code |
| 4. Suite hardcoded | `tbcc-dashboard.ts` | Uses `"default.json"` and `"TB_10"` |

---

## Implementation Steps

### Step 1: Add Model Option to StartTBRunOptions
**Files to modify:**
- `src/effuse/services/socket.ts`
- `src/desktop/handlers.ts`

**Changes:**
1. Add `model?: "claude-code" | "fm" | string` to `StartTBRunOptions` interface
2. In `startTBRun()` handler, add:
   ```typescript
   if (options.model) {
     args.push("--model", options.model);
   }
   ```

### Step 2: Mount TBOutputWidget
**Files to modify:**
- `src/mainview/effuse-main.ts`
- `src/effuse/index.ts` (ensure export)

**Changes:**
1. Import `TBOutputWidget` in effuse-main.ts
2. Add a container in HTML or mount as overlay (widget renders as fixed position popup)
3. Mount: `yield* mountWidgetById(TBOutputWidget, "tb-output-widget")`

The TBOutputWidget auto-opens on `tb_run_start` message, so it should appear when a run begins.

### Step 3: Add Model Selection to Settings
**File to modify:**
- `src/effuse/widgets/tb-command-center/tbcc-settings.ts`

**Changes:**
1. Add `model: "claude-code" | "fm"` to settings state (default: `"fm"`)
2. Add dropdown/toggle in render for model selection
3. Persist to localStorage with other settings
4. Show "(default)" badge on FM option since it's the primary model

### Step 4: Wire Dashboard to Use Settings
**Files to modify:**
- `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts`
- `src/effuse/widgets/tb-command-center/types.ts` (shared settings type)

**Changes:**
1. Dashboard reads model preference from shared state or settings
2. Update `runFullBenchmark` handler to include `model` in startTBRun call:
   ```typescript
   socket.startTBRun({
     suitePath: "tasks/terminal-bench-2.json",
     model: settings.model  // "fm" or "claude-code"
   })
   ```

### Step 5: Hardcode TB-2.0 Suite
**Files to modify:**
- `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts`

**Changes:**
1. Update Dashboard to use `"tasks/terminal-bench-2.json"` as the default suite
2. Remove placeholder `"default.json"` and `subset: "TB_10"`
3. (Suite selection in Settings deferred - keep it simple for now)

---

## File Summary

| File | Action |
|------|--------|
| `src/effuse/services/socket.ts` | Add `model` to `StartTBRunOptions` |
| `src/desktop/handlers.ts` | Pass `--model` flag to CLI |
| `src/mainview/effuse-main.ts` | Mount TBOutputWidget |
| `src/effuse/widgets/tb-command-center/tbcc-settings.ts` | Add model selector UI |
| `src/effuse/widgets/tb-command-center/tbcc-dashboard.ts` | Use settings model in startTBRun |

---

## Verification Steps

1. **Start desktop app**: `bun run desktop`
2. **Open TBCC Settings**: Select "Foundation Model" as model
3. **Click "Run Full Benchmark"** on Dashboard
4. **Verify**:
   - TBOutputWidget popup appears
   - `tb_run_start` message received (check console)
   - `tb_task_output` messages stream in
   - Live output visible in TBOutputWidget
   - Run completes and results appear

---

## Architecture Flow (After Implementation)

```
User clicks "Run Full Benchmark"
        ↓
Dashboard reads model from settings ("fm")
        ↓
socket.startTBRun({ suitePath: "...", model: "fm" })
        ↓
Handler spawns: bun tbench-local.ts --suite ... --model fm
        ↓
CLI creates FM model runner, connects to HUD WebSocket
        ↓
tbEmit.runStart() → HudMessage → Desktop Server → All UI clients
        ↓
TBOutputWidget receives tb_run_start → becomes visible
        ↓
tbEmit.taskOutput() → streams to TBOutputWidget
        ↓
TBOutputWidget displays live agent/tool/verification output
        ↓
tbEmit.runComplete() → Dashboard refreshes, shows results
```

---

## Risk Mitigation

1. **FM not available**: Add fallback in Settings - show "FM unavailable" if not macOS 26+
2. **Output not appearing**: Add console logging in TBOutputWidget subscriptions for debugging
3. **Suite not found**: Use Settings to validate suite path exists

---

## Estimated Effort

| Step | Estimate |
|------|----------|
| 1. Model option | 10 min |
| 2. Mount TBOutputWidget | 15 min |
| 3. Settings model UI | 20 min |
| 4. Dashboard wiring | 10 min |
| 5. Testing | 15 min |
| **Total** | **~1 hour** |
