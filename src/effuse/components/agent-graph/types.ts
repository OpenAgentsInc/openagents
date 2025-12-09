/**
 * Agent Graph Component Types
 */

import type { Point } from "./geometry.js"

// ============================================================================
// Simulation Node
// ============================================================================

/**
 * Node in the physics simulation with position, velocity, acceleration
 */
export interface SimNode {
  id: string
  label: string
  x: number // Position
  y: number
  vx: number // Velocity
  vy: number
  ax: number // Acceleration
  ay: number
  fx?: number // Fixed position (if constrained)
  fy?: number
  shape: "rect" | "circle"
  width: number // 120
  height: number // 80
  r?: number // Radius for circles
}

/**
 * Connection between two nodes
 */
export interface GraphConnection {
  from: string // Node ID
  to: string // Node ID
}

// ============================================================================
// Component State
// ============================================================================

/**
 * State for the agent graph component
 */
export interface AgentGraphState {
  nodes: SimNode[]
  connections: GraphConnection[]
  hoveredNodeId: string | null
  animationFrame: number
  simulationRunning: boolean
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
 * Events emitted by the agent graph component
 */
export type AgentGraphEvent =
  | { type: "nodeClick"; nodeId: string }
  | { type: "nodeHover"; nodeId: string | null }
  | { type: "canvasPan"; delta: Point }
  | { type: "canvasZoom"; delta: number; pointer: Point }
  | { type: "simulationTick" }

// ============================================================================
// Initial Data
// ============================================================================

/**
 * Create initial ATIF nodes for the graph
 */
export function createATIFNodes(): SimNode[] {
  return [
    {
      id: "trajectory",
      label: "Trajectory",
      x: 200,
      y: 200,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "step",
      label: "Step",
      x: 400,
      y: 200,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "agent",
      label: "Agent",
      x: 600,
      y: 200,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "toolcall",
      label: "ToolCall",
      x: 200,
      y: 400,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "observation",
      label: "Observation",
      x: 400,
      y: 400,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "metrics",
      label: "Metrics",
      x: 600,
      y: 400,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "checkpoint",
      label: "Checkpoint",
      x: 300,
      y: 300,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
    {
      id: "subagentref",
      label: "SubagentRef",
      x: 500,
      y: 300,
      vx: 0,
      vy: 0,
      ax: 0,
      ay: 0,
      shape: "rect",
      width: 120,
      height: 80,
    },
  ]
}

/**
 * Create initial connections between ATIF nodes
 */
export function createATIFConnections(): GraphConnection[] {
  return [
    { from: "trajectory", to: "step" },
    { from: "step", to: "agent" },
    { from: "agent", to: "toolcall" },
    { from: "toolcall", to: "observation" },
    { from: "observation", to: "metrics" },
    { from: "step", to: "checkpoint" },
    { from: "checkpoint", to: "subagentref" },
  ]
}
