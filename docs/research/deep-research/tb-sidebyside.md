> Next I want to add to that a sidebyside view with one example trajectory next to our FM agent (or eventually other agent) going through the same process itself (loading it thru our existing terminalbench flow, updates streaming to the UI as it goes)

Nice, this is exactly the next thing I’d want too: “gold standard” trajectory on the left, our FM agent live on the right.

I’ll treat what you already specced as **given**, and layer a **comparison mode** on top:

* Left: **reference trajectory (OpenThoughts SFT)**
* Right: **live TerminalBench FM run** executing the *same task*, streaming as it goes

---

## 1. UX & Layout for Side-by-Side Mode

### Main area layout

Instead of just:

* [x] `HFTrajectoryDetailWidget`

We make the main column switchable:

* Mode A: **Single View** (what you just specced)
* Mode B: **Comparison View** (reference vs live run)

In comparison mode:

```text
┌───────────────────────────────────────────────────────────────┐
│ HF Trajectory Comparison                             [Single] │ <- toggle
├───────────────────────────────────────────────────────────────┤
│ Reference (OpenThoughts)          | Live Run (TerminalBench) │
│                                   |                          │
│ ┌───────────────────────────────┐ | ┌───────────────────────┐ │
│ │ Session: ...                  │ | │ Run: tb-run-123      │ │
│ │ Agent: terminus-2 (GLM-4.6)   │ | │ Agent: fm-claude     │ │
│ │ Task: task_1001 • 19 steps    │ | │ Status: RUNNING      │ │
│ └───────────────────────────────┘ | └───────────────────────┘ │
│                                   |                          │
│ ┌────────────── Step list ──────┐ | ┌────── Live step list ─┐│
│ │ #1 [USER] ...                ▼│ | │ [streaming…]          ││
│ │ #2 [AGENT] ...               ▲│ | │ ░░░ tokens / tools…   ││
│ │ #3 [USER] ...                ▼│ | │                       ││
│ └───────────────────────────────┘ | └───────────────────────┘ │
│                                   |                          │
│ [Start New Run from this Trajectory] [Stop Run] [Sync Scroll]│
└───────────────────────────────────────────────────────────────┘
```

Controls:

* **Toggle “Single / Comparison”** in the detail header
* **“Start New Run from this Trajectory”** button (only enabled when we have a reference)
* **“Stop Run”** button when live run is in progress
* **“Sync scroll”** toggle to keep both columns aligned by step index
* Click a step header on either side → highlight corresponding step on the other side (if indices align)

---

## 2. Data Flow: From SFT Trajectory to Live Run

### Core idea

When user has a trajectory selected in the sidebar:

1. They click **“Compare / Start Live Run”**.
2. We:

   * Extract the **task description + initial terminal state** from the OpenThoughts ATIF.
   * Call existing TerminalBench orchestration to **start a new FM run** with that task.
3. We stream events from that run via your existing TerminalBench flow, adapt into ATIF-ish steps, and render.

### Concrete data flow

```text
HFTrajectoryListWidget
   └─(select sessionId)→ HFTrajectoryDetailWidget (single-view)
       └─ user clicks "Compare / Start run"
          ↓
   TrajectoryComparisonWidget receives:
       { type: "startRunFromReference", trajectory: Trajectory }

          ↓
   TerminalBenchRunService.startRunFromTrajectory(trajectory)
          ↓
   returns { runId, meta }

          ↓
   TerminalBenchRunStream.subscribe(runId)
          ↓
   stream of RunEvent → mapped into LiveStep[] (ATIF-like)

          ↓
   TrajectoryComparisonWidget updates state.runSteps
          ↓
   Live steps appear on right-hand side, streaming
```

### Starting a run from the reference

You’ll want a small adapter that:

* Takes `Trajectory` (your HF/OpenThoughts ATIF)
* Finds the **first user “Task Description” step** (that long prompt with “You are an AI assistant tasked with solving…”)
* Extracts:

  * `taskDescription`: that entire “Task Description / Goal / Requirements … Current terminal state” blob
  * `initialTerminalScreen`: last “Current terminal state” segment if you model that separately
* Calls your existing TerminalBench API:

```ts
interface StartRunFromTrajectoryInput {
  referenceSessionId: string
  taskDescription: string
  initialTerminal: string | null
}

interface StartRunFromTrajectoryResult {
  runId: string
  agentName: string
  modelName: string
  startedAt: string
}

TerminalBenchRunService.startRunFromTrajectory(
  trajectory: Trajectory,
): Effect<StartRunFromTrajectoryResult>
```

On the backend you likely already have something like “run a TerminalBench task from a plain prompt+task spec”; this just wires that up.

---

## 3. Widgets & State: What to Add

### 3.1 New widget: `HFTrajectoryComparisonWidget`

**File:** `src/effuse/widgets/hf-trajectory-comparison.ts`

This widget:

* Is mounted in the same main region as the detail widget.
* Either:

  * wraps `HFTrajectoryDetailWidget` plus a live view, or
  * re-implements the step rendering using your existing ATIF components.

**State:**

```ts
interface TrajectoryComparisonState {
  // Reference
  reference: Trajectory | null
  referenceSessionId: string | null

  // Live run
  runId: string | null
  runAgentName: string | null
  runModelName: string | null
  runStatus: "idle" | "starting" | "running" | "completed" | "failed"
  runError: string | null

  // Steps
  referenceSteps: AtifStep[]
  runSteps: AtifStep[]

  // UI
  mode: "single" | "comparison"
  syncScroll: boolean
  selectedStepIndex: number | null
  loading: boolean
}
```

> `AtifStep` can just be whatever you use in `ATIFDetailsWidget`, reused for both.

**Events:**

```ts
type HFTrajectoryComparisonEvent =
  | { type: "loadReference"; trajectory: Trajectory }        // from list/detail
  | { type: "toggleMode"; mode: "single" | "comparison" }
  | { type: "startRunFromReference" }
  | { type: "stopRun" }
  | { type: "runEvent"; event: TerminalBenchRunEvent }
  | { type: "toggleSyncScroll" }
  | { type: "selectStep"; index: number }
```

### 3.2 Live run event model

You already have terminalbench flow; define a simplified event type for the UI:

```ts
type TerminalBenchRunEvent =
  | {
      type: "step_started"
      runId: string
      stepIndex: number
      timestamp: string
      source: "agent" | "user" | "system"
      message?: string
    }
  | {
      type: "step_completed"
      runId: string
      stepIndex: number
      timestamp: string
      result: {
        raw: string
        parsedJson?: unknown
      }
    }
  | {
      type: "run_status"
      runId: string
      status: "starting" | "running" | "completed" | "failed"
      error?: string
    }
```

Adapter to ATIF step for reuse:

```ts
function runEventToAtifStep(
  event: TerminalBenchRunEvent,
  previous: AtifStep[] // to update existing
): AtifStep[] {
  // pseudo: if step_index already exists, patch; else append
}
```

---

## 4. How It Hooks Into Your Existing Widgets

You currently plan:

* `HFTrajectoryListWidget` in sidebar
* `HFTrajectoryDetailWidget` in main

I’d do this small surgery:

1. **Main area owns the mode & comparison widget**.

   In `effuse-main.ts`:

   ```ts
   // Pseudocode

   const listWidget = HFTrajectoryListWidget.mount("#hf-trajectory-list-widget", ...)
   const comparisonWidget = HFTrajectoryComparisonWidget.mount("#hf-trajectory-detail-widget", ...)

   listWidget.on("select", ({ sessionId }) => {
     // fetch trajectory
     OpenThoughtsService.getTrajectory(sessionId).pipe(
       Effect.tap((trajectory) =>
         comparisonWidget.handleEvent({ type: "loadReference", trajectory })
       )
     )
   })
   ```

2. **Detail behavior moves into comparison widget.**

   * When `mode === "single"` → show only reference (like your current `HFTrajectoryDetailWidget` spec).
   * When `mode === "comparison"` → show side-by-side.

   You can implement reference rendering by **extracting shared render helpers** from your detail widget spec (e.g. `renderStepHeader`, `renderStepBody`) and reusing.

3. **Start run from reference.**

   In the comparison widget’s view:

   ```ts
   h("button", {
     onClick: () => dispatch({ type: "startRunFromReference" }),
     disabled: !state.reference || state.runStatus === "starting" || state.runStatus === "running"
   }, "Start New Run from this Trajectory")
   ```

   Event handler:

   ```ts
   case "startRunFromReference":
     if (!state.reference) return state

     return state.pipeWithEffect(
       TerminalBenchRunService.startRunFromTrajectory(state.reference),
       (result) => ({
         ...state,
         runId: result.runId,
         runAgentName: result.agentName,
         runModelName: result.modelName,
         runStatus: "running",
         runError: null,
         runSteps: [],
       })
     )
   ```

4. **Subscribe to streaming updates.**

   You probably have some `SocketService` or `EventBusService` in Effuse. Use that to attach a subscription when run starts:

   ```ts
   TerminalBenchRunStream.subscribe(result.runId, (event) =>
     comparisonWidget.handleEvent({ type: "runEvent", event })
   )
   ```

   Then in `handleEvent`:

   ```ts
   case "runEvent":
     switch (event.event.type) {
       case "run_status":
         return { ...state, runStatus: event.event.status, runError: event.event.error ?? null }
       case "step_started":
       case "step_completed":
         return {
           ...state,
           runSteps: runEventToAtifStep(event.event, state.runSteps),
         }
     }
   ```

---

## 5. Rendering the Side-by-Side Steps

### Reference column

Basically exactly your formatted `HFTrajectoryDetailWidget` step accordion:

* `#n [USER|AGENT|SYSTEM]`
* Timestamp, tool count
* Message, tool calls, observations, metrics

These are static; no streaming.

### Live column

Same visual language, but with **status tags**:

* Step header: `#n [AGENT] [RUNNING]` / `[DONE]` / `[ERROR]`
* While step is still in progress, we can show:

  * grey “streaming” skeleton
  * partial text (if you get intermediate tokens)
  * list of tool calls as they arrive

Example header:

```text
#3 [AGENT] 10:32:12   STATUS: RUNNING   🔧 2 calls
```

Expanded:

* Live message text
* Commands being sent to TerminalBench
* Terminal output snippets, etc.

---

## 6. Minimal Interfaces for the Live Run Service

To ground it, here’s a small TS contract you can implement server-side and call from the widget:

```ts
// src/terminalbench/service.ts

export interface TerminalBenchRunService {
  startRunFromTrajectory(
    trajectory: Trajectory
  ): Effect<StartRunFromTrajectoryResult>

  stopRun(runId: string): Effect<void>

  // optional, if you want explicit subscribe/unsubscribe
  subscribeToRun(
    runId: string,
    onEvent: (ev: TerminalBenchRunEvent) => void
  ): Effect<SubscriptionHandle>
}

export interface StartRunFromTrajectoryResult {
  runId: string
  agentName: string
  modelName: string
  startedAt: string
}
```

The Effuse widget just depends on that interface; you wire it to your actual backend implementation.

---

## 7. How to Use the 15K Trajectories *Now* (Concrete “Next Steps”)

Given you already have them all locally + you’re building the browser:

1. **Finish Phase 1 browser** exactly as specced.
2. **Extract a small set of “good exemplars”**:

   * Tag 10–50 trajectories as “reference tasks” (e.g., by naming convention or metadata).
   * Optional: store this in a local yaml/json index.
3. **Implement `HFTrajectoryComparisonWidget`** with:

   * Single vs comparison toggle
   * Start run from reference
   * Live run streaming + step rendering
4. **Wire TerminalBench orchestration**:

   * Implement `TerminalBenchRunService.startRunFromTrajectory`.
   * On the backend, map that to your existing TerminalBench “run” primitive.
   * Ensure the run event stream matches `TerminalBenchRunEvent`.
5. **Smoke-test**:

   * Pick one OpenThoughts task.
   * Start a live run using the same task spec.
   * Watch the two trajectories diverge/converge step-by-step.

Once that’s live, you’ve basically built a **trajectory lab**:

* Left: “how SFT models solved this on HF”
* Right: “how our FM agent is solving it right now”

From there you can start doing:

* qualitative comparison
* automatic diffing later
* reward shaping from the reference traces, etc.

---

If you want, I can next:

* sketch the **exact JSX/DOM builder code** for the comparison widget, or
* define a **precise algorithm for aligning steps** (e.g., by `(source, message type, tool count)` instead of raw index) so clicking one step highlights the “closest” step on the other side.
