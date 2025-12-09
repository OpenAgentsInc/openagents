/**
 * TestGen Graph Component
 * SVG-based visualization of TestGen/HillClimber workflow
 */

import { Effect, pipe, Stream } from "effect"
import { SocketServiceTag } from "../../services/socket.js"
import { html, rawHtml } from "../../template/html.js"
import { renderGraph } from "./render.js"
import { isHillClimberMessage, mapMessageToState } from "./state-mapper.js"
import { createTestGenConnections, createTestGenNodes } from "./types.js"
import { renderLogPanel } from "./log-panel.js"

import type { Component, ComponentContext } from "../../component/types.js"
import type {
  TestGenGraphState,
  TestGenGraphEvent,
} from "./types.js"
import type { Point } from "../agent-graph/geometry.js"
import type { HudMessage } from "../../../hud/protocol.js"

// ============================================================================
// Initial State
// ============================================================================

function initialState(): TestGenGraphState {
  return {
    // Multi-session tracking
    sessions: new Map(),
    activeSessionId: null,

    // UI state
    isStarting: false,

    // Graph layout
    nodes: createTestGenNodes(),
    connections: createTestGenConnections(),

    // Interaction
    hoveredNodeId: null,
    draggedNodeId: null,
    animationFrame: 0,
    canvas: {
      pan: { x: 200, y: 100 },
      zoom: 1.2,
      viewport: { width: 0, height: 0 },
    },

    // Log/output panel
    logItems: [],
    logPanelCollapsed: false,
  }
}

// ============================================================================
// Render
// ============================================================================

function render(ctx: ComponentContext<TestGenGraphState, TestGenGraphEvent>) {
  return Effect.gen(function* () {
    const state = yield* ctx.state.get

    // Get active session for status display
    const activeSession = state.activeSessionId
      ? state.sessions.get(state.activeSessionId)
      : null

    const sessionStatus = activeSession
      ? `${activeSession.status} | Turn ${activeSession.currentTurn}/${activeSession.maxTurns} | Progress: ${(activeSession.progress * 100).toFixed(1)}%`
      : "No active session"

    // Get status color based on session status
    const getStatusColor = (status: string) => {
      switch (status) {
        case "completed": return "#0f0"
        case "failed": return "#f00"
        case "running": return "#ff0"
        case "testgen": return "#0ff"
        default: return "#888"
      }
    }

    // Render session list
    const sessionList = Array.from(state.sessions.values())
      .sort((a, b) => b.lastUpdateAt - a.lastUpdateAt)
      .map(session => {
        const isActive = session.sessionId === state.activeSessionId
        const statusColor = getStatusColor(session.status)
        const progressPct = (session.progress * 100).toFixed(0)
        const bestPct = (session.bestProgress * 100).toFixed(0)
        return `
          <div
            data-action="select-session"
            data-session-id="${session.sessionId}"
            style="
              padding: 8px;
              margin-bottom: 4px;
              background: ${isActive ? "#222" : "#111"};
              border: 1px solid ${isActive ? statusColor : "#333"};
              border-radius: 4px;
              cursor: pointer;
            "
          >
            <div style="font-size: 10px; color: #666; margin-bottom: 2px;">
              ${session.sessionId.slice(0, 16)}...
            </div>
            <div style="display: flex; justify-content: space-between; align-items: center;">
              <span style="color: ${statusColor}; font-size: 11px;">${session.status}</span>
              <span style="color: #888; font-size: 10px;">${progressPct}% (best: ${bestPct}%)</span>
            </div>
            <div style="font-size: 10px; color: #555; margin-top: 2px;">
              Turn ${session.currentTurn}/${session.maxTurns}
            </div>
          </div>
        `
      }).join("")

    return html`
      <div style="position: absolute; inset: 0; overflow: hidden; isolation: isolate;">
        <!-- Session List Sidebar -->
        <div style="
          position: absolute;
          left: 10px;
          top: 10px;
          width: 200px;
          max-height: calc(100% - 20px);
          overflow-y: auto;
          z-index: 100;
          background: rgba(0,0,0,0.8);
          border: 1px solid #333;
          border-radius: 4px;
          padding: 8px;
        ">
          <div style="color: #888; font-size: 11px; margin-bottom: 8px; text-transform: uppercase;">
            Sessions (${state.sessions.size})
          </div>
          ${rawHtml(sessionList || '<div style="color: #555; font-size: 11px;">No sessions yet</div>')}
        </div>

        <!-- Control Panel -->
        <div style="position: absolute; top: 10px; right: 10px; z-index: 100; display: flex; gap: 8px; align-items: center;">
          <span style="color: #888; font-size: 12px; margin-right: 8px;">${sessionStatus}</span>
          <button
            data-action="start-quick"
            style="padding: 6px 12px; background: #333; color: #0f0; border: 1px solid #0f0; border-radius: 4px; cursor: pointer; font-size: 12px;"
          >
            QUICK TEST v2
          </button>
          <button
            data-action="start-standard"
            style="padding: 6px 12px; background: #333; color: #ff0; border: 1px solid #ff0; border-radius: 4px; cursor: pointer; font-size: 12px;"
          >
            Standard (10 turns)
          </button>
          <button
            data-action="start-full"
            style="padding: 6px 12px; background: #333; color: #f80; border: 1px solid #f80; border-radius: 4px; cursor: pointer; font-size: 12px;"
          >
            Full (25 turns)
          </button>
        </div>

        <!-- Graph SVG -->
        <svg
          id="testgen-graph-svg"
          width="100%"
          height="100%"
          style="position: absolute; inset: 0; background: #000000; user-select: none; z-index: 0;"
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

        <!-- Log/Output Panel -->
        ${renderLogPanel(state.logItems, state.logPanelCollapsed)}
      </div>
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
    const handleMouseDown = (e: Event) => {
      const mouseEvent = e as MouseEvent
      if (mouseEvent.button !== 0) return // Only left mouse button

      clickStartTime = Date.now()
      clickStartPos = { x: mouseEvent.clientX, y: mouseEvent.clientY }
      lastMouseX = mouseEvent.clientX
      lastMouseY = mouseEvent.clientY

      Effect.runFork(
        Effect.gen(function* () {
          const worldPos = screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
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
    const handleMouseMove = (e: Event) => {
      const mouseEvent = e as MouseEvent
      const dx = mouseEvent.clientX - lastMouseX
      const dy = mouseEvent.clientY - lastMouseY

      if (isDraggingNode) {
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
            yield* ctx.emit({ type: "nodeDragMove", worldPoint: worldPos })
          })
        )
      } else if (isPanningCanvas) {
        Effect.runFork(ctx.emit({ type: "canvasPan", delta: { x: dx, y: dy } }))
      } else {
        // Just hovering - update hover state
        Effect.runFork(
          Effect.gen(function* () {
            const worldPos = screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
            const node = findNodeAt(worldPos.x, worldPos.y)
            yield* ctx.emit({ type: "nodeHover", nodeId: node?.id || null })
          })
        )
      }

      lastMouseX = mouseEvent.clientX
      lastMouseY = mouseEvent.clientY
    }

    // Mouse up: End drag or pan, detect click
    const handleMouseUp = (e: Event) => {
      const mouseEvent = e as MouseEvent
      const wasClick =
        Date.now() - clickStartTime < 200 &&
        Math.abs(mouseEvent.clientX - clickStartPos.x) < 5 &&
        Math.abs(mouseEvent.clientY - clickStartPos.y) < 5

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
            const worldPos = screenToWorld(mouseEvent.clientX, mouseEvent.clientY)
            const node = findNodeAt(worldPos.x, worldPos.y)
            if (node) {
              yield* ctx.emit({ type: "nodeClick", nodeId: node.id })
            }
          })
        )
      }
    }

    // Wheel zoom
    const handleWheel = (e: Event) => {
      const wheelEvent = e as WheelEvent
      wheelEvent.preventDefault()
      const svg = getSvg()
      if (!svg) return

      const delta = wheelEvent.deltaY > 0 ? 0.9 : 1.1
      const rect = svg.getBoundingClientRect()
      const pointer = { x: wheelEvent.clientX - rect.left, y: wheelEvent.clientY - rect.top }

      Effect.runFork(
        ctx.emit({
          type: "canvasZoom",
          delta,
          pointer,
        })
      )
    }

    // Attach event listeners
    container.addEventListener("mousedown", handleMouseDown as EventListener)
    document.addEventListener("mousemove", handleMouseMove as EventListener)
    document.addEventListener("mouseup", handleMouseUp as EventListener)
    container.addEventListener("wheel", handleWheel as EventListener, { passive: false })

    // Click handler using event delegation (works after re-renders)
    const handleClick = (e: Event) => {
      console.log("[TestGen Graph] Click detected on:", (e.target as HTMLElement).tagName, (e.target as HTMLElement).dataset)
      const target = e.target as HTMLElement

      // Check for start button clicks
      const startButton = target.closest("[data-action^='start-']") as HTMLElement | null
      if (startButton) {
        const action = startButton.dataset.action
        if (action?.startsWith("start-")) {
          const mode = action.replace("start-", "") as "quick" | "standard" | "full"
          console.log("[TestGen Graph] Start button clicked:", mode)
          Effect.runFork(ctx.emit({ type: "startRun", mode }))
          return
        }
      }

      // Check for session card clicks
      const sessionCard = target.closest("[data-action='select-session']") as HTMLElement | null
      if (sessionCard) {
        const sessionId = sessionCard.dataset.sessionId
        if (sessionId) {
          console.log("[TestGen Graph] Session selected:", sessionId)
          Effect.runFork(ctx.emit({ type: "selectSession", sessionId }))
        }
        return
      }

      // Check for log panel toggle
      const toggleLog = target.closest("[data-action='toggle-log']") as HTMLElement | null
      if (toggleLog) {
        console.log("[TestGen Graph] Toggling log panel")
        Effect.runFork(ctx.emit({ type: "toggleLogPanel" }))
        return
      }
    }

    // Attach single click handler using event delegation
    console.log("[TestGen Graph] Attaching click handler to container:", container.id || container.tagName)
    container.addEventListener("click", handleClick)

    // Cleanup
    return Effect.sync(() => {
      cancelAnimationFrame(_animationId)
      container.removeEventListener("mousedown", handleMouseDown)
      document.removeEventListener("mousemove", handleMouseMove)
      document.removeEventListener("mouseup", handleMouseUp)
      container.removeEventListener("wheel", handleWheel)
      container.removeEventListener("click", handleClick)
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

      case "selectSession":
        yield* ctx.state.update((s) => ({
          ...s,
          activeSessionId: event.sessionId,
        }))
        break

      case "startRun": {
        console.log("[TestGen Graph] Starting HillClimber run:", event.mode)
        yield* pipe(
          Effect.gen(function* () {
            const socket = yield* SocketServiceTag
            const result = yield* socket.startHillClimber("regex-log", event.mode)
            console.log("[TestGen Graph] HillClimber started:", result.sessionId)
            // Set the new session as active
            yield* ctx.state.update((s) => ({
              ...s,
              activeSessionId: result.sessionId,
            }))
          }),
          Effect.catchAll((err) => {
            console.error("[TestGen Graph] Failed to start HillClimber:", err)
            return Effect.void
          })
        )
        break
      }

      case "toggleLogPanel":
        yield* ctx.state.update((s) => ({
          ...s,
          logPanelCollapsed: !s.logPanelCollapsed,
        }))
        break
    }
  })
}

// ============================================================================
// Socket Subscriptions
// ============================================================================

function subscriptions(ctx: ComponentContext<TestGenGraphState, TestGenGraphEvent>) {
  return [
    pipe(
      Stream.unwrap(
        Effect.map(SocketServiceTag, (socket) => socket.getMessages())
      ),
      Stream.filter((msg): msg is HudMessage => isHillClimberMessage(msg)),
      Stream.map((msg) =>
        Effect.gen(function* () {
          yield* ctx.state.update((state) => mapMessageToState(state, msg))
        })
      )
    ),
  ]
}

// ============================================================================
// Component Definition
// ============================================================================

export const TestGenGraphComponent: Component<
  TestGenGraphState,
  TestGenGraphEvent,
  SocketServiceTag
> = {
  id: "testgen-graph",
  initialState,
  render,
  setupEvents,
  handleEvent,
  subscriptions,
}
