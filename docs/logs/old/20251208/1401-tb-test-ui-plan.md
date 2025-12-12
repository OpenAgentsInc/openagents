# TB Test Generation Viewer UI

## Goal

Create a UI widget to select a TBv2 task (by ID or randomly), run test generation, and stream results into the UI one at a time showing all input/output data including environment details.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                       TB TESTGEN WIDGET                              │
│  - Task selector (dropdown or random)                                │
│  - Environment context display                                       │
│  - Streaming test cards (one per generated test)                    │
└─────────────────────────────────────────────────────────────────────┘
        │                                        ▲
        │ request:startTestGen                   │ hud events
        ▼                                        │
┌─────────────────────────────────────────────────────────────────────┐
│                    DESKTOP SERVER                                    │
│  - Handles startTestGen request                                      │
│  - Calls test-generator.ts                                          │
│  - Emits streaming HUD messages                                      │
└─────────────────────────────────────────────────────────────────────┘
```

## Files to Create/Modify

### 1. New Files

| File | Purpose |
|------|---------|
| `src/effuse/widgets/tb-testgen.ts` | Main test generation viewer widget |
| `src/hillclimber/testgen-service.ts` | Service for running test generation with streaming |

### 2. Modify

| File | Changes |
|------|---------|
| `src/hud/protocol.ts` | Add testgen HUD message types |
| `src/desktop/protocol.ts` | Add request/response types for testgen |
| `src/desktop/handlers.ts` | Add handler for testgen request |
| `src/effuse/widgets/tbcc-shell.ts` | Add TestGen tab to shell tabs |
| `src/mainview/effuse-main.ts` | Mount the new widget |
| `src/effuse/index.ts` | Export new widget |
| `src/mainview/socket-client.ts` | Add startTestGen convenience method |

## Implementation Plan

### Step 1: Add Protocol Types

**`src/hud/protocol.ts`** - Add streaming event types:

```typescript
// Test generation started
export interface TestGenStartMessage {
  type: "testgen_start";
  sessionId: string;
  taskId: string;
  taskDescription: string;
  environment: {
    platform: string;
    prohibitedTools: string[];
    languages: string[];
    fileCount: number;
    filePreviews: number;
  };
}

// Single test generated (streamed one at a time)
export interface TestGenTestMessage {
  type: "testgen_test";
  sessionId: string;
  test: {
    id: string;
    category: string; // anti_cheat, existence, correctness, boundary, integration
    input: string;
    expectedOutput: string | null;
    reasoning: string;
    confidence: number;
  };
}

// Test generation complete
export interface TestGenCompleteMessage {
  type: "testgen_complete";
  sessionId: string;
  totalTests: number;
  durationMs: number;
  uncertainties: string[];
}

// Test generation error
export interface TestGenErrorMessage {
  type: "testgen_error";
  sessionId: string;
  error: string;
}
```

**`src/desktop/protocol.ts`** - Add request/response types:

```typescript
export interface StartTestGenRequest extends BaseRequest {
  type: "request:startTestGen";
  suitePath: string;
  taskId?: string; // If not provided, pick random
  model?: "local" | "claude";
}

export interface StartTestGenResponse extends BaseResponse {
  type: "response:startTestGen";
  data?: { sessionId: string };
}
```

### Step 2: Create TestGen Service

**`src/hillclimber/testgen-service.ts`**:

- Wraps `generateTestsFromEnvironment()`
- Emits HUD messages for each test as it's generated
- Builds mock environment from task folder (like test-gen-compare.ts)
- Sends streaming updates via callback

```typescript
export interface TestGenEmitter {
  onStart: (env: EnvironmentInfo, taskDescription: string) => void;
  onTest: (test: GeneratedTest, category: string) => void;
  onComplete: (result: EnvironmentAwareTestResult) => void;
  onError: (error: Error) => void;
}

export async function runTestGenWithStreaming(
  suitePath: string,
  taskId: string | undefined,
  model: "local" | "claude",
  emitter: TestGenEmitter
): Promise<void>;
```

### Step 3: Add Desktop Handler

**`src/desktop/handlers.ts`** - Add handler:

```typescript
export async function handleStartTestGen(
  request: StartTestGenRequest,
  sendHudMessage: (msg: HudMessage) => void
): Promise<StartTestGenResponse> {
  const sessionId = generateSessionId();

  // Run in background, streaming HUD messages
  runTestGenWithStreaming(request.suitePath, request.taskId, request.model ?? "local", {
    onStart: (env, desc) => sendHudMessage({ type: "testgen_start", sessionId, taskId: ..., ... }),
    onTest: (test, cat) => sendHudMessage({ type: "testgen_test", sessionId, test: { ...test, category: cat } }),
    onComplete: (result) => sendHudMessage({ type: "testgen_complete", sessionId, ... }),
    onError: (err) => sendHudMessage({ type: "testgen_error", sessionId, error: err.message }),
  });

  return { type: "response:startTestGen", success: true, data: { sessionId } };
}
```

### Step 4: Create Widget

**`src/effuse/widgets/tb-testgen.ts`**:

```typescript
export interface TBTestGenState {
  // Task selection
  suiteLoaded: boolean;
  tasks: Array<{ id: string; name: string; category: string }>;
  selectedTaskId: string | null;

  // Generation state
  sessionId: string | null;
  status: "idle" | "loading" | "generating" | "complete" | "error";

  // Environment context
  environment: {
    platform: string;
    prohibitedTools: string[];
    languages: string[];
    fileCount: number;
    filePreviews: number;
  } | null;
  taskDescription: string | null;

  // Generated tests (stream in one at a time)
  tests: Array<{
    id: string;
    category: string;
    input: string;
    expectedOutput: string | null;
    reasoning: string;
    confidence: number;
  }>;

  // Completion
  totalTests: number;
  durationMs: number;
  uncertainties: string[];
  error: string | null;
}

export type TBTestGenEvent =
  | { type: "loadSuite" }
  | { type: "selectTask"; taskId: string }
  | { type: "selectRandom" }
  | { type: "generate" }
  | { type: "clear" };
```

Widget displays:
1. **Task Selector** - Dropdown with task IDs, or "Random" button
2. **Environment Panel** - Shows platform, prohibited tools, languages, files
3. **Test Cards** - Each test streams in as a card showing:
   - Category badge (color-coded)
   - Input/Expected Output in monospace
   - Reasoning text
   - Confidence bar
4. **Summary** - Total tests, duration, uncertainties

### Step 5: Mount Widget as New TBCC Tab

**`src/effuse/widgets/tbcc-shell.ts`**:
- Add "TestGen" to the tab definitions alongside Dashboard, Tasks, Runs, Settings
- Add `tbcc-tab-testgen` container

**`src/mainview/effuse-main.ts`**:
- Mount `TBTestGenWidget` to `tbcc-tab-testgen` container
- Add to child widget mounting section alongside dashboard, taskBrowser, runBrowser, settings

### Step 6: Wire Socket Client

**`src/mainview/socket-client.ts`**:

Add convenience method:
```typescript
async startTestGen(suitePath: string, taskId?: string, model?: "local" | "claude"): Promise<{ sessionId: string }>;
```

## UI Layout (Mockup)

```
┌──────────────────────────────────────────────────────────────────┐
│ Test Generation                                        [×]       │
├──────────────────────────────────────────────────────────────────┤
│ Task: [▼ regex-log              ] [🎲 Random] [▶ Generate]      │
├──────────────────────────────────────────────────────────────────┤
│ ┌─ Environment ─────────────────────────────────────────────┐   │
│ │ Platform: docker                                           │   │
│ │ Prohibited: R, Rscript                                    │   │
│ │ Languages: Python 3.11, Node 20                           │   │
│ │ Files: 3 files, 2 previews                                │   │
│ └───────────────────────────────────────────────────────────┘   │
├──────────────────────────────────────────────────────────────────┤
│ Generated Tests (18 total, 27.6s)                               │
├──────────────────────────────────────────────────────────────────┤
│ ┌─ [ANTI-CHEAT] anti_cheat_1 ───────────────── 95% ─────────┐  │
│ │ Input: which R 2>/dev/null || echo 'not found'             │  │
│ │ Expected: not found                                        │  │
│ │ Reasoning: R should not be installed for R→Python conv...  │  │
│ └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ┌─ [EXISTENCE] existence_1 ─────────────────── 90% ─────────┐  │
│ │ Input: test -f /app/result.py                              │  │
│ │ Expected: 0                                                │  │
│ │ Reasoning: Output Python file should exist                 │  │
│ └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ ┌─ [CORRECTNESS] correctness_1 ────────────── 85% ─────────┐  │
│ │ ...                                                        │  │
│ └────────────────────────────────────────────────────────────┘  │
│                                                                  │
│ [Show more... 15 remaining]                                     │
├──────────────────────────────────────────────────────────────────┤
│ Uncertainties:                                                   │
│ • Confidence on regex pattern matching based on minimal preview │
└──────────────────────────────────────────────────────────────────┘
```

## Implementation Order

1. **Protocol types** (`protocol.ts` files) - Add message/request types
2. **TestGen service** (`testgen-service.ts`) - Streaming wrapper
3. **Desktop handler** (`handlers.ts`) - Wire up RPC
4. **Widget** (`tb-testgen.ts`) - UI component
5. **Integration** (`effuse-main.ts`, `index.ts`) - Mount & export
6. **Socket client** (`socket-client.ts`) - Convenience method

## Notes

- Current test generation takes ~10-30s per task, so streaming is important for UX
- Environment-aware generation produces categorized tests which map nicely to color-coded cards
- The mock environment builder from `test-gen-compare.ts` can be reused
- For now, focus on display only - no actual test execution

## Critical Files Reference

- `src/hillclimber/test-generator.ts:920-957` - `generateTestsFromEnvironment()` main API
- `src/hillclimber/test-gen-compare.ts:256-333` - `buildMockEnvironmentFromTask()` for environment building
- `src/effuse/widgets/tb-output.ts` - Good reference for streaming widget pattern
- `src/effuse/widgets/tb-controls.ts` - Good reference for task selection UI
- `src/desktop/handlers.ts` - Pattern for adding new RPC handlers
