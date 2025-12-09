  TestGen/HillClimber Visualization Design

  Node Types (matching the ATIF visual style)

  | Node       | Purpose               | Live Data                             |
  |------------|-----------------------|---------------------------------------|
  | Task       | The benchmark task    | regex-log, description                |
  | TestGen    | Test generation phase | "Generating..." → "31 tests"          |
  | Category   | Test category         | anti_cheat, boundary, etc. (5 nodes)  |
  | Decomposer | Task decomposition    | "4 subtasks"                          |
  | Subtask    | Current work item     | Highlighted when active               |
  | FM         | Foundation Model      | "Thinking..." / "Writing regex.txt"   |
  | Solution   | Current code          | Shows regex content                   |
  | Verifier   | pytest runner         | "Running..." / "0/31 passed"          |
  | Progress   | Score                 | "46.7%" with color (red→yellow→green) |

  Graph Layout

      ┌──────────┐
      │   Task   │────────────────────────────────┐
      │regex-log │                                │
      └────┬─────┘                                │
           │                                      │
      ┌────┴─────┐                          ┌─────┴─────┐
      │ TestGen  │                          │Decomposer │
      │ 31 tests │                          │ 4 subtasks│
      └────┬─────┘                          └─────┬─────┘
           │                                      │
      ╔════╧════╗                           ╔═════╧═════╗
      ║Categories║                          ║ Subtasks  ║
      ╠═════════╣                           ╠═══════════╣
      ║boundary ║──┐                        ║write-regex║◀──active
      ║existence║  │                        ║boundaries ║
      ║anti_chea║  │                        ║iterate    ║
      ╚═════════╝  │                        ╚═════╤═════╝
                   │                              │
                   │    ┌───────────┐             │
                   └───▶│    FM     │◀────────────┘
                        │ Claude 4  │
                        └─────┬─────┘
                              │
                        ┌─────┴─────┐
                        │ Solution  │
                        │ regex.txt │
                        └─────┬─────┘
                              │
                        ┌─────┴─────┐          ┌──────────┐
                        │ Verifier  │─────────▶│ Progress │
                        │  pytest   │          │  46.7%   │
                        └─────┬─────┘          └──────────┘
                              │
                              └──────── feedback loop to FM

  Live State Visualization

  Phase indicators (node border color):
  - Gray = waiting/inactive
  - Blue pulse = currently running
  - Green = completed successfully
  - Red = failed/error
  - Yellow = partial progress

  Connection animation:
  - Dotted lines animate (stroke-dashoffset) in direction of data flow
  - Thicken when data is passing through
  - Feedback loop from Verifier→FM shows iteration

  Dynamic Features

  1. TestGen phase:
    - TestGen node pulses blue
    - Category nodes appear one by one as generated
    - Test count increments in TestGen node
  2. FM iteration:
    - Current subtask highlighted
    - FM shows tool calls in real-time: "write_file → regex.txt"
    - Solution node updates with regex content
    - Verifier pulses, then shows results
  3. Progress tracking:
    - Progress node changes color: 0%=red → 50%=yellow → 100%=green
    - Shows fraction: "7/15 tests"
    - Best score persists even if current iteration regresses
  4. Iteration loop:
    - Turn counter: "Turn 2/10"
    - Arrow from Verifier→FM animates on each iteration
    - Failed tests pulse red briefly before next attempt

  Socket Events to Subscribe

  interface TestGenHudMessage {
    type: "testgen" | "fm" | "verify" | "progress" | "turn"

    // TestGen
    phase?: "start" | "category" | "complete"
    category?: string
    testCount?: number

    // FM
    action?: "thinking" | "tool_call" | "complete"
    toolName?: string
    content?: string

    // Verify
    running?: boolean
    passed?: number
    total?: number

    // Progress
    percentage?: number
    bestPercentage?: number

    // Turn
    turn?: number
    maxTurns?: number
    subtask?: string
  }

  Implementation Sketch

  Extend the existing AgentGraphComponent or create TestGenGraphComponent:

  const TESTGEN_NODES = [
    { id: "task", label: "Task", x: 200, y: 50 },
    { id: "testgen", label: "TestGen", x: 100, y: 150 },
    { id: "decomposer", label: "Decomposer", x: 300, y: 150 },
    { id: "fm", label: "FM", x: 200, y: 300 },
    { id: "solution", label: "Solution", x: 200, y: 400 },
    { id: "verifier", label: "Verifier", x: 200, y: 500 },
    { id: "progress", label: "Progress", x: 350, y: 500 },
  ]

  const TESTGEN_CONNECTIONS = [
    { from: "task", to: "testgen" },
    { from: "task", to: "decomposer" },
    { from: "testgen", to: "fm" },
    { from: "decomposer", to: "fm" },
    { from: "fm", to: "solution" },
    { from: "solution", to: "verifier" },
    { from: "verifier", to: "progress" },
    { from: "verifier", to: "fm", style: "feedback" }, // Dashed, curved
  ]

  What Makes This Valuable

  1. Debugging - See exactly where the pipeline stalls (TestGen? FM? Verifier?)
  2. Progress monitoring - Watch live as FM iterates toward solution
  3. Understanding - Visualize the hillclimber loop that's otherwise just logs
  4. Demo - Great for showing how the system works

  Want me to start building this as a new Effuse component?
