/**
 * Simplified physics simulation for agent graph
 * Adapted from Unit's Simulation class, using simple Euler integration
 */

import type { SimNode, GraphConnection } from "./types.js"
import { surfaceDistance } from "./geometry.js"

// ============================================================================
// Constants
// ============================================================================

const REPULSION_STRENGTH = 90 // Repulsive force between nodes
const LINK_DISTANCE = 100 // Target distance for connected nodes
const DAMPING = 0.6 // Velocity damping (friction)

// ============================================================================
// Force Function
// ============================================================================

/**
 * Apply forces to nodes (repulsion + link attraction + centering)
 * Harvested from Unit Minigraph Component.ts lines 81-155
 */
export function applyForces(
  nodes: SimNode[],
  connections: GraphConnection[],
  alpha: number
): void {
  // Reset accelerations
  for (const node of nodes) {
    node.ax = 0
    node.ay = 0
  }

  // Repulsive forces between all pairs
  for (let i = 0; i < nodes.length; i++) {
    const a = nodes[i]!

    for (let j = i + 1; j < nodes.length; j++) {
      const b = nodes[j]!

      // Convert SimNode to Thing for surfaceDistance
      const aThing = {
        shape: a.shape,
        x: a.x,
        y: a.y,
        r: a.r || 0,
        width: a.width,
        height: a.height,
      }
      const bThing = {
        shape: b.shape,
        x: b.x,
        y: b.y,
        r: b.r || 0,
        width: b.width,
        height: b.height,
      }

      const { l, u } = surfaceDistance(aThing, bThing)
      const distance = Math.max(l, 1)

      // Repulsive force (inversely proportional to distance)
      const k = (-REPULSION_STRENGTH * alpha) / distance
      b.ax -= u.x * k
      b.ay -= u.y * k
      a.ax += u.x * k
      a.ay += u.y * k
    }

    // Damping toward center (vertical)
    a.ay -= (a.y * alpha) / 6
  }

  // Attractive forces for linked nodes
  for (const conn of connections) {
    const a = nodes.find((n) => n.id === conn.from)
    const b = nodes.find((n) => n.id === conn.to)

    if (!a || !b) continue

    const aThing = {
      shape: a.shape,
      x: a.x,
      y: a.y,
      r: a.r || 0,
      width: a.width,
      height: a.height,
    }
    const bThing = {
      shape: b.shape,
      x: b.x,
      y: b.y,
      r: b.r || 0,
      width: b.width,
      height: b.height,
    }

    const { l, d } = surfaceDistance(bThing, aThing)
    const gap = Math.max(l, 1)
    const centerDist = Math.max(d, 1)

    const targetGap = LINK_DISTANCE
    const k = alpha / centerDist
    const force = (gap - targetGap) * k

    const dx = (b.x - a.x) * force
    const dy = (b.y - a.y) * force

    b.ax -= dx
    b.ay -= dy
    a.ax += dx
    a.ay += dy

    // Vertical centering of linked nodes
    const midY = (a.y + b.y) / 2
    b.ay += ((midY - b.y) * alpha) / 3
    a.ay += ((midY - a.y) * alpha) / 3
  }
}

// ============================================================================
// Integration Step
// ============================================================================

/**
 * Update node positions using simple Euler integration (RK1)
 */
export function integrate(nodes: SimNode[], dt: number, alpha: number): void {
  for (const node of nodes) {
    // Skip fixed nodes
    if (node.fx !== undefined) {
      node.x = node.fx
      node.vx = 0
    } else {
      // Apply friction
      node.vx *= DAMPING

      // Update velocity from acceleration
      node.vx += node.ax * dt

      // Update position from velocity
      node.x += node.vx * dt
    }

    if (node.fy !== undefined) {
      node.y = node.fy
      node.vy = 0
    } else {
      // Apply friction
      node.vy *= DAMPING

      // Update velocity from acceleration
      node.vy += node.ay * dt

      // Update position from velocity
      node.y += node.vy * dt
    }
  }
}

// ============================================================================
// Simulation State
// ============================================================================

export interface SimulationState {
  alpha: number // Current "temperature" of simulation
  alphaTarget: number // Target alpha
  alphaDecay: number // Rate of cooling
  alphaMin: number // Minimum alpha before stopping
}

/**
 * Create initial simulation state
 */
export function createSimulationState(): SimulationState {
  return {
    alpha: 0.25,
    alphaTarget: 0,
    alphaDecay: 0.01,
    alphaMin: 0.001,
  }
}

/**
 * Update simulation alpha (cooling)
 */
export function updateAlpha(state: SimulationState): SimulationState {
  const newAlpha =
    state.alpha + (state.alphaTarget - state.alpha) * state.alphaDecay

  return {
    ...state,
    alpha: newAlpha,
  }
}

/**
 * Check if simulation should stop
 */
export function shouldStop(state: SimulationState): boolean {
  return state.alpha < state.alphaMin
}
