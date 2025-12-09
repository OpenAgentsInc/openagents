/**
 * TestGen Graph Component Types
 * Based on design from docs/logs/20251209/1226-testgen-component-design.md
 */

import type { Point } from "../agent-graph/geometry.js"

// ============================================================================
// TestGen Node Types
// ============================================================================

/**
 * Node status/phase indicator
 */
export type NodeStatus = "waiting" | "running" | "completed" | "failed" | "partial"

/**
 * Node in the TestGen graph
 */
export interface TestGenNode {
  id: string
  label: string
  x: number
  y: number
  width: number
  height: number
  status: NodeStatus
  // Additional data for specific node types
  data?: {
    // For Task node
    taskName?: string
    description?: string
    // For TestGen node
    testCount?: number
    phase?: "start" | "category" | "complete"
    // For Category nodes
    categoryName?: string
    categoryTestCount?: number
    // For Decomposer node
    subtaskCount?: number
    // For Subtask nodes
    subtaskName?: string
    isActive?: boolean
    // For FM node
    action?: "thinking" | "tool_call" | "complete"
    toolName?: string
    // For Solution node
    content?: string
    // For Verifier node
    running?: boolean
    passed?: number
    total?: number
    // For Progress node
    percentage?: number
    bestPercentage?: number
    turn?: number
    maxTurns?: number
  }
}

/**
 * Connection between two nodes
 */
export interface TestGenConnection {
  from: string // Node ID
  to: string // Node ID
  style?: "normal" | "feedback" // Feedback connections are dashed/curved
}

// ============================================================================
// Session State (for multi-session tracking)
// ============================================================================

/**
 * Per-session run state
 */
export interface SessionRunState {
  sessionId: string
  status: "waiting" | "testgen" | "running" | "completed" | "failed"

  // TestGen phase
  testGenProgress: { category: string; count: number }[]
  totalTests: number

  // MAP phase
  currentTurn: number
  maxTurns: number
  currentSubtask: string
  fmAction: string
  testsPassed: number
  testsTotal: number
  progress: number
  bestProgress: number

  // Timestamps
  startedAt: number
  lastUpdateAt: number
}

/**
 * Create a new empty session state
 */
export function createNewSession(sessionId: string): SessionRunState {
  return {
    sessionId,
    status: "waiting",
    testGenProgress: [],
    totalTests: 0,
    currentTurn: 0,
    maxTurns: 10,
    currentSubtask: "",
    fmAction: "",
    testsPassed: 0,
    testsTotal: 0,
    progress: 0,
    bestProgress: 0,
    startedAt: Date.now(),
    lastUpdateAt: Date.now(),
  }
}

// ============================================================================
// Component State
// ============================================================================

/**
 * State for the TestGen graph component
 */
export interface TestGenGraphState {
  // Multi-session tracking
  sessions: Map<string, SessionRunState>
  activeSessionId: string | null

  // UI state
  isStarting: boolean // True while waiting for server response

  // Graph layout (shared)
  nodes: TestGenNode[]
  connections: TestGenConnection[]

  // Interaction
  hoveredNodeId: string | null
  draggedNodeId: string | null
  animationFrame: number
  canvas: {
    pan: Point
    zoom: number
    viewport: { width: number; height: number }
  }
}

// ============================================================================
// Component Events
// ============================================================================

/**
 * Events emitted by the TestGen graph component
 */
export type TestGenGraphEvent =
  | { type: "nodeClick"; nodeId: string }
  | { type: "nodeHover"; nodeId: string | null }
  | { type: "nodeDragStart"; nodeId: string; startPoint: Point }
  | { type: "nodeDragMove"; worldPoint: Point }
  | { type: "nodeDragEnd" }
  | { type: "canvasPan"; delta: Point }
  | { type: "canvasZoom"; delta: number; pointer: Point }
  | { type: "animationTick" }
  | { type: "selectSession"; sessionId: string }
  | { type: "startRun"; mode: "quick" | "standard" | "full" }

// ============================================================================
// Initial Graph Structure
// ============================================================================

/**
 * Create initial TestGen nodes with waiting status (populated by live data)
 */
export function createTestGenNodes(): TestGenNode[] {
  return [
    // Task node
    {
      id: "task",
      label: "Task",
      x: 200,
      y: 50,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        taskName: "",
        description: "",
      },
    },
    // TestGen node
    {
      id: "testgen",
      label: "TestGen",
      x: 100,
      y: 150,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        testCount: 0,
        phase: "start",
      },
    },
    // Decomposer node
    {
      id: "decomposer",
      label: "Decomposer",
      x: 300,
      y: 150,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        subtaskCount: 0,
      },
    },
    // FM node
    {
      id: "fm",
      label: "FM",
      x: 200,
      y: 300,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        action: "thinking",
        toolName: "",
      },
    },
    // Solution node
    {
      id: "solution",
      label: "Solution",
      x: 200,
      y: 400,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        content: "",
      },
    },
    // Verifier node
    {
      id: "verifier",
      label: "Verifier",
      x: 200,
      y: 500,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        running: false,
        passed: 0,
        total: 0,
      },
    },
    // Progress node
    {
      id: "progress",
      label: "Progress",
      x: 350,
      y: 500,
      width: 140,
      height: 60,
      status: "waiting",
      data: {
        percentage: 0,
        bestPercentage: 0,
        turn: 0,
        maxTurns: 0,
      },
    },
  ]
}

/**
 * Create initial connections between TestGen nodes
 * Category and subtask nodes are added dynamically when data arrives
 */
export function createTestGenConnections(): TestGenConnection[] {
  return [
    // Task to TestGen and Decomposer
    { from: "task", to: "testgen" },
    { from: "task", to: "decomposer" },
    // TestGen and Decomposer to FM
    { from: "testgen", to: "fm" },
    { from: "decomposer", to: "fm" },
    // FM to Solution
    { from: "fm", to: "solution" },
    // Solution to Verifier
    { from: "solution", to: "verifier" },
    // Verifier to Progress
    { from: "verifier", to: "progress" },
    // Feedback loop: Verifier to FM
    { from: "verifier", to: "fm", style: "feedback" },
  ]
}
