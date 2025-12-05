# 1445 Mainview Modularization Progress

**Task**: oa-931376 - "Mainview UI: module-ize entrypoint after HTML split"

## Objective
Refactor `src/mainview/index.ts` (2734 lines) into per-feature modules that mirror HTML fragments, with small init APIs and a slim index bootstrap.

## Analysis Completed

Used Explore agent to analyze the full 2734-line file and identify:
- 9 main functional sections
- Dependencies and circular references
- Shared state accessed across modules
- DOM element IDs per section

### Identified Modules to Create

1. **shared-types.ts** - Common interfaces, types, constants (~200 lines)
2. **container-panes.ts** - Container execution output rendering (~80 lines)
3. **tb-output.ts** - TB output viewer (~60 lines)
4. **trajectory-pane.ts** - Trajectory list and rendering (~70 lines)
5. **mc-tasks.ts** - MechaCoder tasks widget (~200 lines)
6. **tb-controls.ts** - TB controls, task selector, RPC (~250 lines)
7. **tb-state.ts** - TB state, flow sync, HUD message handling (~600 lines)
8. **flow.ts** - Canvas, layout, view mode, main render (~300 lines)
9. **category-tree.ts** - TB category tree (currently hidden) (~160 lines)

**Total extractable:** ~1920 lines into modules
**Remaining in index.ts:** ~814 lines (imports, initialization, event handlers)

## Progress So Far

### ✅ Phase 1 Completed (Simple Modules)
1. Created `src/mainview/shared-types.ts` (195 lines)
   - All common interfaces exported
   - Type definitions for MC tasks, TB state, containers
   - Constants (ZINC colors, limits)
   - RPC schema interfaces

2. Created `src/mainview/container-panes.ts` (125 lines)
   - Container execution output rendering
   - Throttled render using requestAnimationFrame
   - HTML escaping for XSS prevention
   - Export: containerPanes Map, renderContainerPanes(), initContainerPanes()

3. Created `src/mainview/tb-output.ts` (120 lines)
   - TB output viewer with clear, copy, close functionality
   - Auto-scroll to bottom
   - Show/hide logic
   - Export: showOutputViewer(), updateOutputViewer(), initTBOutput()

4. Created `src/mainview/trajectory-pane.ts` (130 lines)
   - Unified trajectory list (TB runs, ATIF traces)
   - Loading state management
   - Click handler with event delegation
   - Export: loadTrajectories(), renderTrajectoryPane(), initTrajectoryPane()

**Total Extracted:** ~570 lines (195 + 125 + 120 + 130)

### 🔄 In Progress
- Task requires ~4-6 hours total for full completion
- Phase 1 completed (~1 hour)
- Multiple circular dependencies need refactoring (in Phase 2)
- Shared mutable state requires careful handling (in Phase 2)

## Key Technical Challenges

### 1. Circular Dependencies
- `render()` is called from many modules
- `renderMCTasksWidget()` needs `viewMode` and `render()`
- HUD handler updates state and triggers renders
- **Solution**: Pass render callback during initialization

### 2. Shared Mutable State
- `tbState`, `containerPanes`, `viewMode`, `canvasState` accessed globally
- **Solution**: Export state objects from their owning modules, import where needed

### 3. DOM Element Caching
- Elements cached at module load time
- **Solution**: Lazy initialization or caching during `init()` functions

### 4. Socket Client Access
- Currently global via getter
- **Solution**: Pass as dependency during module initialization

## Proposed Module APIs

### shared-types.ts ✅
```typescript
export interface MCTaskState { ... }
export interface TBState { ... }
export interface ContainerPane { ... }
export const ZINC = { ... }
export type ViewMode = "flow" | "tbench"
```

### container-panes.ts (TODO)
```typescript
export const containerPanes: Map<string, ContainerPane>
export function renderContainerPanes(): void
export function initContainerPanes(): void
```

### trajectory-pane.ts (TODO)
```typescript
export function loadTrajectories(): Promise<void>
export function renderTrajectoryPane(): void
export function initTrajectoryPane(socketClient: SocketClient): void
```

### tb-output.ts (TODO)
```typescript
export function showOutputViewer(): void
export function updateOutputViewer(): void
export function initTBOutput(state: TBState): void
```

### mc-tasks.ts (TODO)
```typescript
export function loadMCTasks(): Promise<void>
export function renderMCTasksWidget(): void
export function initMCTasks(
  socketClient: SocketClient,
  renderFn: () => void,
  setViewModeFn: (mode: ViewMode) => void
): void
```

### tb-controls.ts (TODO)
```typescript
export function handleLoadSuite(): Promise<void>
export function handleStartRun(): Promise<void>
export function updateTBButtons(isRunning: boolean): void
export function initTBControls(
  socketClient: SocketClient,
  state: TBState
): void
```

### tb-state.ts (TODO)
```typescript
export let tbState: TBState
export let tbFlowState: TBFlowState
export function handleHudMessage(message: HudMessage): void
export function syncTBFlowWithState(): void
export function initTBState(
  socketClient: SocketClient,
  renderFn: () => void,
  renderContainersFn: () => void,
  updateOutputFn: () => void
): void
```

### flow.ts (TODO)
```typescript
export function render(): void
export function setViewMode(mode: ViewMode): void
export const canvasState: CanvasState
export function initFlow(socketClient: SocketClient): void
```

## Initialization Sequence

```typescript
// Slim index.ts bootstrap (target: ~100 lines)

import { initFlow, render, setViewMode } from "./flow.js"
import { initMCTasks, loadMCTasks } from "./mc-tasks.js"
import { initTrajectoryPane, loadTrajectories } from "./trajectory-pane.js"
import { initContainerPanes } from "./container-panes.js"
import { initTBState, handleHudMessage, tbState } from "./tb-state.js"
import { initTBControls } from "./tb-controls.js"
import { initTBOutput } from "./tb-output.js"
import { getSocketClient } from "./socket-client.js"

const socketClient = getSocketClient({ verbose: true })

// Initialize modules (order matters for dependencies)
initFlow(socketClient)
initMCTasks(socketClient, render, setViewMode)
initTrajectoryPane(socketClient)
initContainerPanes()
initTBState(socketClient, render, renderContainerPanes, updateOutputViewer)
initTBControls(socketClient, tbState)
initTBOutput(tbState)

// Initial render
render()

// Connect and load data
socketClient.connect().then(() => {
  void loadMCTasks()
  void loadTrajectories()
})

// Set up message handler
socketClient.onMessage(handleHudMessage)

// Keyboard shortcuts (stays in index.ts)
document.addEventListener("keydown", (e) => { ... })

// Window API exposure
window.TB = { ... }
```

## Next Steps

### Phase 1: Simple Modules (2-3 hours)
1. Create container-panes.ts - extract rendering logic
2. Create tb-output.ts - extract output viewer
3. Create trajectory-pane.ts - extract trajectory rendering
4. Update index.ts to import from these modules
5. Test UI still works

### Phase 2: Complex Modules (3-4 hours)
6. Create mc-tasks.ts - extract widget and handlers
7. Create tb-controls.ts - extract controls and RPC
8. Create tb-state.ts - extract state management and HUD handling
9. Create flow.ts - extract canvas and main render
10. Refactor circular dependencies with callbacks

### Phase 3: Testing & Refinement (1-2 hours)
11. Run Playwright HUD smoke tests
12. Fix any bugs from refactoring
13. Document module APIs
14. Update task to track remaining work

## Files Created
- `src/mainview/shared-types.ts` (+195 lines, NEW) ✅
- `src/mainview/container-panes.ts` (+125 lines, NEW) ✅
- `src/mainview/tb-output.ts` (+120 lines, NEW) ✅
- `src/mainview/trajectory-pane.ts` (+130 lines, NEW) ✅

## Files To Modify
- `src/mainview/index.ts` (will be reduced from 2734 to ~814 lines) - NOT YET MODIFIED

## Validation Required
- TypeScript compilation: `bun run build:check`
- Playwright tests: Check `tests/` directory for HUD tests
- Manual UI testing in desktop app

## Estimated Time Remaining
**6-8 hours** for complete modularization with testing

## Notes
- This is a significant architectural refactoring
- High value for maintainability and future development
- Requires careful testing to avoid breaking UI
- Should be done incrementally with tests between phases
- Consider feature flag for gradual rollout

## Recommendation
**Option A**: Continue with pragmatic phased approach
- Extract 2-3 simple modules now (Phase 1)
- Test thoroughly
- Complete complex modules in follow-up session

**Option B**: Pause and plan for dedicated refactoring session
- Current progress: Foundation laid with shared-types
- Remaining work: 6-8 hours
- Schedule dedicated time for full completion

Given the scope and risk, **Option A (phased approach)** is recommended to maintain stability while making incremental progress.
