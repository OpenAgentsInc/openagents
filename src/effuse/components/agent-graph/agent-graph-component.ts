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
    animationFrame: 0,
    simulationRunning: true,
    canvas: {
      pan: { x: 0, y: 0 },
      zoom: 1,
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

    console.log("[AgentGraph] Rendering with", state.nodes.length, "nodes")

    return html`
      <svg
        id="agent-graph-svg"
        width="100%"
        height="100%"
        viewBox="0 0 800 600"
        style="position: absolute; inset: 0; background: #000000;"
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
    const svg = yield* ctx.dom.queryId<SVGSVGElement>("agent-graph-svg").pipe(Effect.orDie)

    console.log("[AgentGraph] SVG element found:", svg)
    console.log("[AgentGraph] SVG dimensions:", svg.clientWidth, "x", svg.clientHeight)

    // Simulation state
    let simState = createSimulationState()
    let lastTime = performance.now()

    // Animation loop
    let _animationId: number

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

      _animationId = requestAnimationFrame(animate)
    }

    _animationId = requestAnimationFrame(animate)

    // Mouse hover detection
    svg.addEventListener("mousemove", (e: MouseEvent) => {
      const rect = svg.getBoundingClientRect()
      const svgX = e.clientX - rect.left
      const svgY = e.clientY - rect.top

      Effect.runFork(
        Effect.gen(function* () {
          const state = yield* ctx.state.get

          // Transform to world coordinates
          const worldX = (svgX - state.canvas.pan.x) / state.canvas.zoom
          const worldY = (svgY - state.canvas.pan.y) / state.canvas.zoom

          // Find node at point
          const hoveredNode = state.nodes.find((node: SimNode) => {
            const dx = worldX - node.x
            const dy = worldY - node.y
            return (
              Math.abs(dx) <= node.width / 2 &&
              Math.abs(dy) <= node.height / 2
            )
          })

          yield* ctx.emit({
            type: "nodeHover",
            nodeId: hoveredNode?.id || null,
          })
        })
      )
    })

    // Click detection
    svg.addEventListener("click", (e: MouseEvent) => {
      const rect = svg.getBoundingClientRect()
      const svgX = e.clientX - rect.left
      const svgY = e.clientY - rect.top

      Effect.runFork(
        Effect.gen(function* () {
          const state = yield* ctx.state.get

          // Transform to world coordinates
          const worldX = (svgX - state.canvas.pan.x) / state.canvas.zoom
          const worldY = (svgY - state.canvas.pan.y) / state.canvas.zoom

          // Find node at point
          const clickedNode = state.nodes.find((node: SimNode) => {
            const dx = worldX - node.x
            const dy = worldY - node.y
            return (
              Math.abs(dx) <= node.width / 2 &&
              Math.abs(dy) <= node.height / 2
            )
          })

          if (clickedNode) {
            yield* ctx.emit({ type: "nodeClick", nodeId: clickedNode.id })
          }
        })
      )
    })

    // Pan: click and drag
    let isPanning = false
    let lastMouseX = 0
    let lastMouseY = 0

    svg.addEventListener("mousedown", (e: MouseEvent) => {
      // Only pan on left-click on background (not on nodes)
      if (e.button === 0) {
        const rect = svg.getBoundingClientRect()
        const svgX = e.clientX - rect.left
        const svgY = e.clientY - rect.top

        Effect.runFork(
          Effect.gen(function* () {
            const state = yield* ctx.state.get

            const worldX = (svgX - state.canvas.pan.x) / state.canvas.zoom
            const worldY = (svgY - state.canvas.pan.y) / state.canvas.zoom

            const onNode = state.nodes.some((node: SimNode) => {
              const dx = worldX - node.x
              const dy = worldY - node.y
              return (
                Math.abs(dx) <= node.width / 2 &&
                Math.abs(dy) <= node.height / 2
              )
            })

            if (!onNode) {
              isPanning = true
              lastMouseX = e.clientX
              lastMouseY = e.clientY
              svg.style.cursor = "grabbing"
            }
          })
        )
      }
    })

    document.addEventListener("mousemove", (e) => {
      if (isPanning) {
        const dx = e.clientX - lastMouseX
        const dy = e.clientY - lastMouseY

        Effect.runFork(
          ctx.emit({ type: "canvasPan", delta: { x: dx, y: dy } })
        )

        lastMouseX = e.clientX
        lastMouseY = e.clientY
      }
    })

    document.addEventListener("mouseup", () => {
      if (isPanning) {
        isPanning = false
        svg.style.cursor = "default"
      }
    })

    // Zoom: scroll wheel
    svg.addEventListener("wheel", (e: WheelEvent) => {
      e.preventDefault()
      const delta = e.deltaY > 0 ? 0.9 : 1.1 // Zoom out/in

      const rect = svg.getBoundingClientRect()
      const pointer = {
        x: e.clientX - rect.left,
        y: e.clientY - rect.top,
      }

      Effect.runFork(ctx.emit({ type: "canvasZoom", delta, pointer }))
    })

    // TODO: Add proper cleanup for animationFrame using acquireRelease
    // For now, the animation will stop when simulation alpha drops below minimum
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
        console.log("Clicked node:", event.nodeId)
        // TODO: Emit to parent or show details panel
        break

      case "canvasPan":
        yield* ctx.state.update((s: AgentGraphState) => ({
          ...s,
          canvas: {
            ...s.canvas,
            pan: {
              x: s.canvas.pan.x + event.delta.x,
              y: s.canvas.pan.y + event.delta.y,
            },
          },
        }))
        break

      case "canvasZoom":
        yield* ctx.state.update((s: AgentGraphState) => {
          const newZoom = Math.max(
            0.25,
            Math.min(4, s.canvas.zoom * event.delta)
          )
          // TODO: Zoom toward pointer position (requires more complex math)
          return {
            ...s,
            canvas: { ...s.canvas, zoom: newZoom },
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
