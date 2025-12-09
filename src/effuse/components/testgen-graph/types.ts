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
// Initial Data (Hardcoded for now)
// ============================================================================

/**
 * Create initial TestGen nodes with hardcoded data
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
      status: "completed",
      data: {
        taskName: "regex-log",
        description: "Extract dates from log lines with IPv4",
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
      status: "completed",
      data: {
        testCount: 31,
        phase: "complete",
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
      status: "completed",
      data: {
        subtaskCount: 4,
      },
    },
    // Category nodes (5 categories)
    {
      id: "category-boundary",
      label: "boundary",
      x: 50,
      y: 250,
      width: 100,
      height: 50,
      status: "completed",
      data: {
        categoryName: "boundary",
        categoryTestCount: 4,
      },
    },
    {
      id: "category-existence",
      label: "existence",
      x: 50,
      y: 320,
      width: 100,
      height: 50,
      status: "completed",
      data: {
        categoryName: "existence",
        categoryTestCount: 5,
      },
    },
    {
      id: "category-anti_cheat",
      label: "anti_cheat",
      x: 50,
      y: 390,
      width: 100,
      height: 50,
      status: "completed",
      data: {
        categoryName: "anti_cheat",
        categoryTestCount: 4,
      },
    },
    {
      id: "category-correctness",
      label: "correctness",
      x: 50,
      y: 460,
      width: 100,
      height: 50,
      status: "completed",
      data: {
        categoryName: "correctness",
        categoryTestCount: 3,
      },
    },
    {
      id: "category-integration",
      label: "integration",
      x: 50,
      y: 530,
      width: 100,
      height: 50,
      status: "completed",
      data: {
        categoryName: "integration",
        categoryTestCount: 5,
      },
    },
    // Subtask nodes (4 subtasks)
    {
      id: "subtask-write-regex",
      label: "write-regex",
      x: 300,
      y: 250,
      width: 120,
      height: 50,
      status: "completed",
      data: {
        subtaskName: "write-regex",
        isActive: false,
      },
    },
    {
      id: "subtask-boundaries",
      label: "boundaries",
      x: 300,
      y: 320,
      width: 120,
      height: 50,
      status: "completed",
      data: {
        subtaskName: "boundaries",
        isActive: false,
      },
    },
    {
      id: "subtask-iterate",
      label: "iterate",
      x: 300,
      y: 390,
      width: 120,
      height: 50,
      status: "running",
      data: {
        subtaskName: "iterate",
        isActive: true,
      },
    },
    {
      id: "subtask-final",
      label: "final-validation",
      x: 300,
      y: 460,
      width: 120,
      height: 50,
      status: "waiting",
      data: {
        subtaskName: "final-validation",
        isActive: false,
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
      status: "running",
      data: {
        action: "tool_call",
        toolName: "write_file",
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
      status: "completed",
      data: {
        content: "regex.txt",
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
      status: "running",
      data: {
        running: true,
        passed: 17,
        total: 31,
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
      status: "partial",
      data: {
        percentage: 54.8,
        bestPercentage: 89.5,
        turn: 2,
        maxTurns: 10,
      },
    },
  ]
}

/**
 * Create initial connections between TestGen nodes
 */
export function createTestGenConnections(): TestGenConnection[] {
  return [
    // Task to TestGen and Decomposer
    { from: "task", to: "testgen" },
    { from: "task", to: "decomposer" },
    // TestGen to categories
    { from: "testgen", to: "category-boundary" },
    { from: "testgen", to: "category-existence" },
    { from: "testgen", to: "category-anti_cheat" },
    { from: "testgen", to: "category-correctness" },
    { from: "testgen", to: "category-integration" },
    // TestGen and Decomposer to FM
    { from: "testgen", to: "fm" },
    { from: "decomposer", to: "fm" },
    // Decomposer to subtasks
    { from: "decomposer", to: "subtask-write-regex" },
    { from: "decomposer", to: "subtask-boundaries" },
    { from: "decomposer", to: "subtask-iterate" },
    { from: "decomposer", to: "subtask-final" },
    // Subtasks to FM
    { from: "subtask-write-regex", to: "fm" },
    { from: "subtask-boundaries", to: "fm" },
    { from: "subtask-iterate", to: "fm" },
    { from: "subtask-final", to: "fm" },
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
