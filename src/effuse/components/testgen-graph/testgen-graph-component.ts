/**
 * TestGen Graph Component
 * SVG-based visualization of TestGen/HillClimber workflow
 */

import { Effect } from "effect"
import type { Component, ComponentContext } from "../../component/types.js"
import { html, rawHtml } from "../../template/html.js"
import type {
  TestGenGraphState,
  TestGenGraphEvent,
} from "./types.js"
import {
  createTestGenNodes,
  createTestGenConnections,
} from "./types.js"
import { renderGraph } from "./render.js"
import type { Point } from "../agent-graph/geometry.js"

// ============================================================================
// Initial State
// ============================================================================

function initialState(): TestGenGraphState {
  return {
    nodes: createTestGenNodes(),
    connections: createTestGenConnections(),
    hoveredNodeId: null,
    draggedNodeId: null,
    animationFrame: 0,
    canvas: {
      pan: { x: 200, y: 100 },
      zoom: 1.2,
      viewport: { width: 0, height: 0 },
    },
  }
}

// ============================================================================
// Render
// ============================================================================

function render(ctx: ComponentContext<TestGenGraphState, TestGenGraphEvent>) {
  return Effect.gen(function* () {
    const state = yield* ctx.state.get

    return html`
      <svg
        id="testgen-graph-svg"
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

function setupEvents(ctx: ComponentContext<TestGenGraphState, TestGenGraphEvent>) {
  return Effect.gen(function* () {
    const container = ctx.container
    const getSvg = () => container.querySelector<SVGSVGElement>("#testgen-graph-svg")

    // Animation loop for dash offset
    let _animationId: number
    const animate = () => {
      Effect.runFork(
        Effect.gen(function* () {
          const state = yield* ctx.state.get
          const newAnimationFrame = (state.animationFrame + 1) % 1000
          yield* ctx.state.update((s) => ({
            ...s,
            animationFrame: newAnimationFrame,
          }))
        })
      )
      _animationId = requestAnimationFrame(animate)
    }
    _animationId = requestAnimationFrame(animate)

    // Interaction state
    let isDraggingNode = false
    let isPanningCanvas = false
    let lastMouseX = 0
    let lastMouseY = 0
    let clickStartTime = 0
    let clickStartPos = { x: 0, y: 0 }

    // Helper: Get world coordinates from screen coordinates
    const screenToWorld = (screenX: number, screenY: number): Point => {
      return Effect.runSync(
        Effect.gen(function* () {
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
      )
    }

    // Helper: Find node at world coordinates
    const findNodeAt = (worldX: number, worldY: number) => {
      return Effect.runSync(
        Effect.gen(function* () {
          const state = yield* ctx.state.get
          return state.nodes.find((node) => {
            const left = node.x - node.width / 2
            const right = node.x + node.width / 2
            const top = node.y - node.height / 2
            const bottom = node.y + node.height / 2
            return (
              worldX >= left &&
              worldX <= right &&
              worldY >= top &&
              worldY <= bottom
            )
          })
        })
      )
    }

    // Mouse down: Start drag or pan
    const handleMouseDown = (e: MouseEvent) => {
      if (e.button !== 0) return // Only left mouse button

      clickStartTime = Date.now()
      clickStartPos = { x: e.clientX, y: e.clientY }
      lastMouseX = e.clientX
      lastMouseY = e.clientY

      Effect.runFork(
        Effect.gen(function* () {
          const worldPos = screenToWorld(e.clientX, e.clientY)
          const node = findNodeAt(worldPos.x, worldPos.y)

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
    }

    // Mouse move: Update drag or pan
    const handleMouseMove = (e: MouseEvent) => {
      const dx = e.clientX - lastMouseX
      const dy = e.clientY - lastMouseY

      if (isDraggingNode) {
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = screenToWorld(e.clientX, e.clientY)
            yield* ctx.emit({ type: "nodeDragMove", worldPoint: worldPos })
          })
        )
      } else if (isPanningCanvas) {
        Effect.runFork(ctx.emit({ type: "canvasPan", delta: { x: dx, y: dy } }))
      } else {
        // Just hovering - update hover state
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = screenToWorld(e.clientX, e.clientY)
            const node = findNodeAt(worldPos.x, worldPos.y)
            yield* ctx.emit({ type: "nodeHover", nodeId: node?.id || null })
          })
        )
      }

      lastMouseX = e.clientX
      lastMouseY = e.clientY
    }

    // Mouse up: End drag or pan, detect click
    const handleMouseUp = (e: MouseEvent) => {
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
            const worldPos = screenToWorld(e.clientX, e.clientY)
            const node = findNodeAt(worldPos.x, worldPos.y)
            if (node) {
              yield* ctx.emit({ type: "nodeClick", nodeId: node.id })
            }
          })
        )
      }
    }

    // Wheel zoom
    const handleWheel = (e: WheelEvent) => {
      e.preventDefault()
      const svg = getSvg()
      if (!svg) return

      const delta = e.deltaY > 0 ? 0.9 : 1.1
      const rect = svg.getBoundingClientRect()
      const pointer = { x: e.clientX - rect.left, y: e.clientY - rect.top }

      Effect.runFork(
        ctx.emit({
          type: "canvasZoom",
          delta,
          pointer,
        })
      )
    }

    // Attach event listeners
    container.addEventListener("mousedown", handleMouseDown)
    document.addEventListener("mousemove", handleMouseMove)
    document.addEventListener("mouseup", handleMouseUp)
    container.addEventListener("wheel", handleWheel, { passive: false })

    // Cleanup
    return Effect.sync(() => {
      cancelAnimationFrame(_animationId)
      container.removeEventListener("mousedown", handleMouseDown)
      document.removeEventListener("mousemove", handleMouseMove)
      document.removeEventListener("mouseup", handleMouseUp)
      container.removeEventListener("wheel", handleWheel)
    })
  })
}

// ============================================================================
// Handle Events
// ============================================================================

function handleEvent(
  event: TestGenGraphEvent,
  ctx: ComponentContext<TestGenGraphState, TestGenGraphEvent>
) {
  return Effect.gen(function* () {
    switch (event.type) {
      case "nodeHover":
        yield* ctx.state.update((s) => ({
          ...s,
          hoveredNodeId: event.nodeId,
        }))
        break

      case "nodeClick":
        console.log("[TestGen Graph] Clicked node:", event.nodeId)
        // TODO: Show node details or emit to parent
        break

      case "nodeDragStart":
        yield* ctx.state.update((s) => ({
          ...s,
          draggedNodeId: event.nodeId,
        }))
        break

      case "nodeDragMove":
        yield* ctx.state.update((s) => {
          if (!s.draggedNodeId) return s
          const node = s.nodes.find((n) => n.id === s.draggedNodeId)
          if (!node) return s

          return {
            ...s,
            nodes: s.nodes.map((n) =>
              n.id === s.draggedNodeId
                ? { ...n, x: event.worldPoint.x, y: event.worldPoint.y }
                : n
            ),
          }
        })
        break

      case "nodeDragEnd":
        yield* ctx.state.update((s) => ({
          ...s,
          draggedNodeId: null,
        }))
        break

      case "canvasPan":
        yield* ctx.state.update((s) => ({
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
        yield* ctx.state.update((s) => {
          const newZoom = Math.max(0.25, Math.min(4, s.canvas.zoom * event.delta))
          return {
            ...s,
            canvas: {
              ...s.canvas,
              zoom: newZoom,
            },
          }
        })
        break

      case "animationTick":
        // Handled in animation loop
        break
    }
  })
}

// ============================================================================
// Component Definition
// ============================================================================

export const TestGenGraphComponent: Component<TestGenGraphState, TestGenGraphEvent> = {
  id: "testgen-graph",
  initialState,
  render,
  setupEvents,
  handleEvent,
}
