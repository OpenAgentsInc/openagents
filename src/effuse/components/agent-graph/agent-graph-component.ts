/**
 * Agent Graph Component
 * SVG-based graph visualization with Unit-based physics simulation
 */

import { Effect } from "effect"
import type { Component, ComponentContext } from "../../component/types.js"
import type { DomService } from "../../services/dom.js"
import { html, rawHtml } from "../../template/html.js"
import type {
  AgentGraphState,
  AgentGraphEvent,
  SimNode,
} from "./types.js"
import {
  createATIFNodes,
  createATIFConnections,
} from "./types.js"
import { renderGraph } from "./render.js"
import {
  applyForces,
  integrate,
  createSimulationState,
  updateAlpha,
  shouldStop,
} from "./simulation.js"

// ============================================================================
// Initial State
// ============================================================================

function initialState(): AgentGraphState {
  return {
    nodes: createATIFNodes(),
    connections: createATIFConnections(),
    hoveredNodeId: null,
    draggedNodeId: null,
    animationFrame: 0,
    simulationRunning: true,
    canvas: {
      pan: { x: 200, y: 100 },
      zoom: 1.5,
      viewport: { width: 0, height: 0 },
    },
  }
}

// ============================================================================
// Render
// ============================================================================

function render(ctx: ComponentContext<AgentGraphState, AgentGraphEvent>) {
  return Effect.gen(function* () {
    const state = yield* ctx.state.get

    return html`
      <svg
        id="agent-graph-svg"
        width="100%"
        height="100%"
        style="position: absolute; inset: 0; background: #000000; user-select: none;"
      >
        ${rawHtml(
          renderGraph(
            state.nodes,
            state.connections,
            state.hoveredNodeId,
            state.animationFrame,
            state.canvas.pan,
            state.canvas.zoom
          )
        )}
      </svg>
    `
  })
}

// ============================================================================
// Setup Events
// ============================================================================

function setupEvents(ctx: ComponentContext<AgentGraphState, AgentGraphEvent>) {
  return Effect.gen(function* () {
    // Use container instead of SVG since SVG gets destroyed on each render
    const container = ctx.container
    const getSvg = () => container.querySelector<SVGSVGElement>("#agent-graph-svg")

    // Simulation state
    let simState = createSimulationState()
    let lastTime = performance.now()

    // Animation loop
    let animationId: number | undefined

    const animate = (currentTime: number) => {
      const dt = Math.min((currentTime - lastTime) / 1000, 0.1) // Max 100ms step
      lastTime = currentTime

      Effect.runFork(
        Effect.gen(function* () {
          const state = yield* ctx.state.get

          if (!state.simulationRunning) {
            return
          }

          // Update simulation alpha (cooling)
          simState = updateAlpha(simState)

          // Apply forces
          applyForces(state.nodes, state.connections, simState.alpha)

          // Integrate positions
          integrate(state.nodes, dt, simState.alpha)

          // Update animation frame
          const newAnimationFrame = (state.animationFrame + 1) % 1000

          // Update state
          yield* ctx.state.update((s: AgentGraphState) => ({
            ...s,
            nodes: [...state.nodes], // Trigger re-render
            animationFrame: newAnimationFrame,
            simulationRunning: !shouldStop(simState),
          }))
        })
      )

      animationId = requestAnimationFrame(animate)
    }

    animationId = requestAnimationFrame(animate)

    // Interaction state
    let isDraggingNode = false
    let isPanningCanvas = false
    let lastMouseX = 0
    let lastMouseY = 0
    let clickStartTime = 0
    let clickStartPos = { x: 0, y: 0 }

    // Helper: Get world coordinates from screen coordinates
    const screenToWorld = (screenX: number, screenY: number) => {
      return Effect.gen(function* () {
        const svg = getSvg()
        if (!svg) return { x: 0, y: 0 }
        const state = yield* ctx.state.get
        const rect = svg.getBoundingClientRect()
        const svgX = screenX - rect.left
        const svgY = screenY - rect.top
        return {
          x: (svgX - state.canvas.pan.x) / state.canvas.zoom,
          y: (svgY - state.canvas.pan.y) / state.canvas.zoom,
        }
      })
    }

    // Helper: Find node at world coordinates
    const findNodeAt = (worldX: number, worldY: number) => {
      return Effect.gen(function* () {
        const state = yield* ctx.state.get
        return state.nodes.find((node: SimNode) => {
          const dx = worldX - node.x
          const dy = worldY - node.y
          return Math.abs(dx) <= node.width / 2 && Math.abs(dy) <= node.height / 2
        })
      })
    }

    // Mouse down: Start drag or pan
    container.addEventListener("mousedown", ((e: Event) => {
      const mouseEvent = e as MouseEvent
      if (mouseEvent.button !== 0) return // Only left mouse button

      clickStartTime = Date.now()
      clickStartPos = { x: mouseEvent.clientX, y: mouseEvent.clientY }
      lastMouseX = mouseEvent.clientX
      lastMouseY = mouseEvent.clientY

      Effect.runFork(
        Effect.gen(function* () {
          const worldPos = yield* screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
          const node = yield* findNodeAt(worldPos.x, worldPos.y)

          if (node) {
            // Start dragging node
            isDraggingNode = true
            const svg = getSvg()
            if (svg) svg.style.cursor = "grabbing"
            yield* ctx.emit({
              type: "nodeDragStart",
              nodeId: node.id,
              startPoint: worldPos,
            })
          } else {
            // Start panning canvas
            isPanningCanvas = true
            const svg = getSvg()
            if (svg) svg.style.cursor = "grabbing"
          }
        })
      )
    }) as EventListener)

    // Mouse move: Update drag or pan
    document.addEventListener("mousemove", (e: MouseEvent) => {
      const dx = e.clientX - lastMouseX
      const dy = e.clientY - lastMouseY

      if (isDraggingNode) {
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = yield* screenToWorld(e.clientX, e.clientY)
            yield* ctx.emit({ type: "nodeDragMove", worldPoint: worldPos })
          })
        )
      } else if (isPanningCanvas) {
        Effect.runFork(ctx.emit({ type: "canvasPan", delta: { x: dx, y: dy } }))
      } else {
        // Just hovering - update hover state
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = yield* screenToWorld(e.clientX, e.clientY)
            const node = yield* findNodeAt(worldPos.x, worldPos.y)
            yield* ctx.emit({ type: "nodeHover", nodeId: node?.id || null })
          })
        )
      }

      lastMouseX = e.clientX
      lastMouseY = e.clientY
    })

    // Mouse up: End drag or pan, detect click
    document.addEventListener("mouseup", (e: MouseEvent) => {
      const wasClick =
        Date.now() - clickStartTime < 200 &&
        Math.abs(e.clientX - clickStartPos.x) < 5 &&
        Math.abs(e.clientY - clickStartPos.y) < 5

      if (isDraggingNode) {
        Effect.runFork(ctx.emit({ type: "nodeDragEnd" }))
        isDraggingNode = false
        const svg = getSvg()
        if (svg) svg.style.cursor = "default"
      } else if (isPanningCanvas) {
        isPanningCanvas = false
        const svg = getSvg()
        if (svg) svg.style.cursor = "default"
      }

      // Handle click
      if (wasClick) {
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = yield* screenToWorld(e.clientX, e.clientY)
            const node = yield* findNodeAt(worldPos.x, worldPos.y)
            if (node) {
              yield* ctx.emit({ type: "nodeClick", nodeId: node.id })
            }
          })
        )
      }
    })

    // Double-click: Unpin node
    container.addEventListener("dblclick", ((e: Event) => {
      const mouseEvent = e as MouseEvent
      Effect.runFork(
        Effect.gen(function* () {
          const worldPos = yield* screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
          const node = yield* findNodeAt(worldPos.x, worldPos.y)
          if (node) {
            yield* ctx.emit({ type: "nodeDoubleClick", nodeId: node.id })
          }
        })
      )
    }) as EventListener)

    // Wheel: Zoom
    container.addEventListener("wheel", ((e: Event) => {
      const wheelEvent = e as WheelEvent
      wheelEvent.preventDefault()
      const svg = getSvg()
      if (!svg) return
      const rect = svg.getBoundingClientRect()
      const pointer = { x: wheelEvent.clientX - rect.left, y: wheelEvent.clientY - rect.top }
      const delta = wheelEvent.deltaY > 0 ? 0.975 : 1.025
      Effect.runFork(ctx.emit({ type: "canvasZoom", delta, pointer }))
    }) as EventListener, { passive: false })

    // Cleanup animation frame
    return Effect.sync(() => {
      if (animationId !== undefined) {
        cancelAnimationFrame(animationId)
      }
    })
  })
}

// ============================================================================
// Handle Events
// ============================================================================

function handleEvent(event: AgentGraphEvent, ctx: ComponentContext<AgentGraphState, AgentGraphEvent>) {
  return Effect.gen(function* () {
    switch (event.type) {
      case "nodeHover":
        yield* ctx.state.update((s: AgentGraphState) => ({
          ...s,
          hoveredNodeId: event.nodeId,
        }))
        break

      case "nodeClick":
        console.log("[AgentGraph] Clicked node:", event.nodeId)
        break

      case "nodeDragStart":
        yield* ctx.state.update((s: AgentGraphState) => {
          // Pin the node by setting fx/fy
          const nodes = s.nodes.map((n) =>
            n.id === event.nodeId ? { ...n, fx: n.x, fy: n.y } : n
          )
          return { ...s, nodes, draggedNodeId: event.nodeId }
        })
        console.log("[AgentGraph] Dragging node:", event.nodeId)
        break

      case "nodeDragMove":
        yield* ctx.state.update((s: AgentGraphState) => {
          if (!s.draggedNodeId) return s
          const nodes = s.nodes.map((n) =>
            n.id === s.draggedNodeId
              ? { ...n, fx: event.worldPoint.x, fy: event.worldPoint.y, x: event.worldPoint.x, y: event.worldPoint.y, vx: 0, vy: 0 }
              : n
          )
          return { ...s, nodes }
        })
        break

      case "nodeDragEnd":
        yield* ctx.state.update((s: AgentGraphState) => ({
          ...s,
          draggedNodeId: null,
        }))
        console.log("[AgentGraph] Drag ended")
        break

      case "nodeDoubleClick":
        yield* ctx.state.update((s: AgentGraphState) => {
          // Unpin the node by clearing fx/fy
          const nodes = s.nodes.map((n) => {
            if (n.id === event.nodeId) {
              const { fx, fy, ...rest } = n
              return rest
            }
            return n
          })
          return { ...s, nodes }
        })
        console.log("[AgentGraph] Unpinned node:", event.nodeId)
        break

      case "canvasPan":
        yield* ctx.state.update((s: AgentGraphState) => {
          const newPan = {
            x: s.canvas.pan.x + event.delta.x,
            y: s.canvas.pan.y + event.delta.y,
          }
          return {
            ...s,
            canvas: {
              ...s.canvas,
              pan: newPan,
            },
          }
        })
        break

      case "canvasZoom":
        yield* ctx.state.update((s: AgentGraphState) => {
          const oldZoom = s.canvas.zoom
          const newZoom = Math.max(0.25, Math.min(4, oldZoom * event.delta))

          // Zoom toward the pointer position
          const pointer = event.pointer
          const newPan = {
            x: pointer.x - (pointer.x - s.canvas.pan.x) * (newZoom / oldZoom),
            y: pointer.y - (pointer.y - s.canvas.pan.y) * (newZoom / oldZoom),
          }

          return {
            ...s,
            canvas: { ...s.canvas, zoom: newZoom, pan: newPan },
          }
        })
        break
    }
  })
}

// ============================================================================
// Export Component
// ============================================================================

export const AgentGraphComponent: Component<
  AgentGraphState,
  AgentGraphEvent,
  DomService
> = {
  id: "agent-graph",
  initialState,
  render,
  setupEvents,
  handleEvent,
}
