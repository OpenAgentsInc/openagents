# Claude Plan: FM Micro-Task Supervisor Implementation

**Date**: 2025-12-08
**Time**: 22:10 CST
**Author**: Claude Opus 4.5
**Status**: Implementation Plan

---

## Reactions to 2208-chat.md Analysis

The analysis in the chat is **spot-on**. Key insights I agree with:

### 1. The Core Problem is Architectural, Not Parametric
The current FM runner tries to be a mini Claude Code clone with conversation history. But FM's ~200-250 char limit makes that fundamentally impossible. No amount of truncation will fix this - the architecture must change.

### 2. The Multi-Turn Loop is the Enemy
```typescript
// This pattern CANNOT work with FM:
while (turns < maxTurns) {
  messages.push(assistant);
  messages.push(toolResult);
  truncatedMessages = truncate(messages); // Always too big
  response = await fm.chat(truncatedMessages);
}
```

### 3. Supervisor-Worker Split is the Right Model
- **Supervisor FM**: Decomposes task into micro-steps (JSON list)
- **Worker FM**: Executes ONE action per call (pure function)
- **Orchestrator**: Manages state, tool execution, flow

### 4. State Lives Outside FM
FM should never see:
- Full file contents
- Raw tool outputs
- Conversation history
- Multiple tool results

FM only sees:
- Current action (40 chars)
- Tiny context summary (30 chars)
- Previous step summary (30 chars)

---

## Implementation Plan

### Phase 1: Define Types (New File: src/fm/micro-task-types.ts)

```typescript
export type MicroStepKind = 
  | "READ_FILE_RANGE"
  | "WRITE_FILE"
  | "EDIT_FILE"
  | "COMPILE"
  | "RUN_COMMAND"
  | "FIX_ERROR";

export interface MicroStep {
  id: number;
  kind: MicroStepKind;
  action: string;          // Human-readable, max 40 chars
  params: Record<string, unknown>;
  status: "pending" | "in_progress" | "done" | "failed";
  resultSummary?: string;  // Max 50 chars
  errorSummary?: string;   // Max 50 chars
}

export interface MicroPlan {
  taskId: string;
  steps: MicroStep[];
}

export interface TaskState {
  plan: MicroPlan;
  currentStep: number;
  files: Map<string, string>;  // File cache
  workspace: string;
  history: string[];           // Condensed summaries only
}
```

### Phase 2: Create Static Planners (src/fm/planners.ts)

Start with heuristic planners for TB tasks, not FM-based planning:

```typescript
export type Planner = (task: TerminalBenchTask) => MicroPlan;

// Example for path-tracing:
export const pathTracingPlanner: Planner = (task) => ({
  taskId: task.id,
  steps: [
    { id: 1, kind: "READ_FILE_RANGE", action: "Read PPM header", params: { path: "/app/image.ppm", start: 1, end: 20 } },
    { id: 2, kind: "WRITE_FILE", action: "Write read_ppm function", params: { path: "image.c" } },
    { id: 3, kind: "EDIT_FILE", action: "Add main function", params: { path: "image.c" } },
    { id: 4, kind: "COMPILE", action: "Compile image.c", params: { command: "gcc -static -o image image.c -lm" } },
    { id: 5, kind: "RUN_COMMAND", action: "Test output", params: { command: "./image" } },
  ],
});
```

### Phase 3: Single-Turn FM Worker (src/fm/worker.ts)

```typescript
export interface WorkerInput {
  action: string;        // Max 40 chars
  context: string;       // Max 30 chars
  previous: string;      // Max 30 chars
}

export interface WorkerOutput {
  toolName: string;
  toolArgs: Record<string, unknown>;
}

// Pure function: tiny prompt -> single tool call
export async function callFMWorker(
  client: FMClient,
  input: WorkerInput
): Promise<WorkerOutput> {
  const WORKER_PROMPT = `Tools: read_file(p,s,e), write_file(p,c), edit_file(p,o,n), run_command(c)
Action: ${input.action}
Context: ${input.context}
Previous: ${input.previous}
Output: <tool_call>{"name":"..","arguments":{..}}</tool_call>`;

  // Single call, no history, no retry for context
  const messages = [{ role: "user", content: WORKER_PROMPT }];
  const response = await Effect.runPromise(client.chat({ messages }));
  return parseToolCall(response);
}
```

### Phase 4: Orchestrator (src/fm/orchestrator.ts)

```typescript
export async function runTaskWithMicroSteps(
  task: TerminalBenchTask,
  plan: MicroPlan,
  options: RunTaskOptions
): Promise<TaskRunResult> {
  const state: TaskState = {
    plan,
    currentStep: 0,
    files: new Map(),
    workspace: options.workspace,
    history: [],
  };

  for (const step of plan.steps) {
    state.currentStep = step.id;
    step.status = "in_progress";

    // Build worker input from state (condensed)
    const workerInput = buildWorkerInput(step, state);
    
    // Call FM worker (single turn)
    const toolCall = await callFMWorker(client, workerInput);
    
    // Execute tool
    const result = await executeTool(toolCall, state.workspace);
    
    // Update state with condensed result
    updateState(state, step, result);
    
    if (step.status === "failed") {
      // Create fix micro-task and retry
      const fixStep = createFixStep(step, result);
      plan.steps.splice(state.currentStep + 1, 0, fixStep);
    }
  }

  return summarizeResult(state);
}
```

### Phase 5: Integrate with Model Adapter

Modify `createFMRunner` to use new micro-task architecture:

```typescript
const createFMRunner = (fmConfig: FMModelConfig): ModelRunner => {
  // ... existing setup ...

  return {
    config: fmConfig,
    modelName: "fm:micro-task",

    async runTask(task, options): Promise<TaskRunResult> {
      // Get planner for task type (static for now)
      const planner = getPlannerForTask(task);
      const plan = planner(task);

      // Run with micro-task orchestrator
      return runTaskWithMicroSteps(task, plan, options);
    },

    // ... existing health check ...
  };
};
```

---

## Key Changes Summary

| Component | Before | After |
|-----------|--------|-------|
| FM calls | Multi-turn conversation | Single-turn pure function |
| Messages array | Grows unbounded | Fixed 1-message request |
| Tool results | Full output in context | 50-char summary in state |
| Planning | Implicit in conversation | Explicit micro-step list |
| State | Inside FM context | Orchestrator-managed |
| Error handling | Reflection in prompt | New micro-step inserted |

---

## Files to Create/Modify

### New Files:
1. `src/fm/micro-task-types.ts` - Type definitions
2. `src/fm/planners.ts` - Static planners for TB tasks
3. `src/fm/worker.ts` - Single-turn FM worker
4. `src/fm/orchestrator.ts` - Micro-task orchestrator

### Modified Files:
1. `src/bench/model-adapter.ts` - Use new micro-task system
2. `src/fm/micro-task.ts` - Update to use new types (or deprecate)

---

## Implementation Order

1. **Types first** - Define MicroStep, MicroPlan, TaskState
2. **Worker next** - Pure single-turn FM caller
3. **Orchestrator** - State management and flow control
4. **Planners** - Static planners for path-tracing and other TB tasks
5. **Integration** - Wire into model-adapter.ts
6. **Test** - Run TB with FM to verify it works

---

## Risk Mitigation

1. **Keep old FM runner** - Don't delete, just add new mode
2. **Start with one task** - path-tracing only initially
3. **Static planners first** - Don't try FM-based planning until worker is stable
4. **Comprehensive logging** - Keep the detailed logging for debugging

---

## Expected Outcome

With this architecture:
- Each FM call stays under 200 chars
- No context overflow errors
- Tasks can be completed with multiple micro-steps
- Error recovery is explicit (new micro-step)
- State is preserved across FM calls

---

## Implementation Log

### 22:10 - Started implementation
- Created this plan document
- Read all relevant files
- Understood the problem and solution

### Next: Create type definitions and begin implementation...

