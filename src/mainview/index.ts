import { calculateLayout } from "../flow/layout.js"
import { sampleMechaCoderTree, sampleNodeSizes } from "../flow/sample-data.js"
import {
  initialCanvasState,
  reduceCanvasState,
  DEFAULT_CONFIG,
  type CanvasEvent,
} from "../flow/canvas.js"
import {
  renderFlowSVG,
  svgElementToString,
  DEFAULT_RENDER_CONFIG,
} from "../flow-host-svg/render.js"

// Calculate layout once from sample data
const layout = calculateLayout({
  root: sampleMechaCoderTree,
  nodeSizes: sampleNodeSizes,
  config: { padding: 20, spacing: 40 },
})

// Get DOM elements
const container = document.getElementById("flow-container")!
const svg = document.getElementById("flow-svg")!
const resetBtn = document.getElementById("reset-btn")!
const zoomLevel = document.getElementById("zoom-level")!

// Initialize canvas state with viewport size
let canvasState = initialCanvasState(window.innerWidth, window.innerHeight)

// Center the view on the layout initially
const centerX = window.innerWidth / 2
const centerY = window.innerHeight / 2
canvasState = {
  ...canvasState,
  panX: centerX - 200,
  panY: centerY - 100,
}

// Render SVG content
function render(): void {
  const flowGroup = renderFlowSVG(layout, canvasState, DEFAULT_RENDER_CONFIG)
  svg.innerHTML = svgElementToString(flowGroup)
  
  // Update zoom display
  zoomLevel.textContent = `${Math.round(canvasState.scale * 100)}%`
}

// Apply canvas event and re-render
function dispatch(event: CanvasEvent): void {
  canvasState = reduceCanvasState(canvasState, event, DEFAULT_CONFIG)
  render()
}

// Mouse/pointer event handlers
container.addEventListener("mousedown", (e) => {
  container.classList.add("dragging")
  dispatch({
    type: "PAN_START",
    pointer: { x: e.clientX, y: e.clientY },
    timestamp: e.timeStamp,
  })
})

container.addEventListener("mousemove", (e) => {
  if (canvasState.isDragging) {
    dispatch({
      type: "PAN_MOVE",
      pointer: { x: e.clientX, y: e.clientY },
      timestamp: e.timeStamp,
    })
  }
})

container.addEventListener("mouseup", (e) => {
  container.classList.remove("dragging")
  dispatch({
    type: "PAN_END",
    timestamp: e.timeStamp,
  })
})

container.addEventListener("mouseleave", (e) => {
  if (canvasState.isDragging) {
    container.classList.remove("dragging")
    dispatch({
      type: "PAN_END",
      timestamp: e.timeStamp,
    })
  }
})

// Wheel zoom
container.addEventListener("wheel", (e) => {
  e.preventDefault()
  dispatch({
    type: "ZOOM",
    pointer: { x: e.clientX, y: e.clientY },
    delta: e.deltaY,
  })
}, { passive: false })

// Reset button
resetBtn.addEventListener("click", () => {
  dispatch({ type: "RESET" })
  // Re-center after reset
  canvasState = {
    ...canvasState,
    panX: centerX - 200,
    panY: centerY - 100,
  }
  render()
})

// Handle window resize
window.addEventListener("resize", () => {
  dispatch({
    type: "RESIZE",
    width: window.innerWidth,
    height: window.innerHeight,
  })
})

// Inertial animation loop
let animationId: number | null = null

function tick(): void {
  if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
    dispatch({ type: "TICK" })
    animationId = requestAnimationFrame(tick)
  } else {
    animationId = null
  }
}

// Start inertia after pan ends
const originalDispatch = dispatch
function dispatchWithInertia(event: CanvasEvent): void {
  originalDispatch(event)
  
  if (event.type === "PAN_END" && !animationId) {
    if (canvasState.velocityX !== 0 || canvasState.velocityY !== 0) {
      animationId = requestAnimationFrame(tick)
    }
  }
}

// Replace dispatch to use inertia version
container.removeEventListener("mouseup", () => {})
container.addEventListener("mouseup", (e) => {
  container.classList.remove("dragging")
  dispatchWithInertia({
    type: "PAN_END",
    timestamp: e.timeStamp,
  })
})

// Initial render
render()
console.log("Flow HUD loaded")