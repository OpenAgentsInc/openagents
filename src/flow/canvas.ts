import type { Point } from "./model.js"

export interface CanvasConfig {
  readonly minScale: number
  readonly maxScale: number
  readonly zoomSensitivity: number
  readonly panFriction: number
  readonly inertiaDecay: number
}

export const DEFAULT_CONFIG: CanvasConfig = {
  minScale: 0.1,
  maxScale: 4.0,
  zoomSensitivity: 0.002,
  panFriction: 0.95,
  inertiaDecay: 0.92,
}

export interface CanvasState {
  readonly scale: number
  readonly panX: number
  readonly panY: number
  readonly viewportWidth: number
  readonly viewportHeight: number
  readonly velocityX: number
  readonly velocityY: number
  readonly isDragging: boolean
  readonly lastPointer: Point | null
  readonly lastTimestamp: number
}

export const initialCanvasState = (
  viewportWidth: number,
  viewportHeight: number
): CanvasState => ({
  scale: 1,
  panX: 0,
  panY: 0,
  viewportWidth,
  viewportHeight,
  velocityX: 0,
  velocityY: 0,
  isDragging: false,
  lastPointer: null,
  lastTimestamp: 0,
})

export type CanvasEvent =
  | { type: "PAN_START"; pointer: Point; timestamp: number }
  | { type: "PAN_MOVE"; pointer: Point; timestamp: number }
  | { type: "PAN_END"; timestamp: number }
  | { type: "ZOOM"; pointer: Point; delta: number }
  | { type: "RESET" }
  | { type: "RESIZE"; width: number; height: number }
  | { type: "TICK" }

export function reduceCanvasState(
  state: CanvasState,
  event: CanvasEvent,
  config: CanvasConfig = DEFAULT_CONFIG
): CanvasState {
  switch (event.type) {
    case "PAN_START":
      return {
        ...state,
        isDragging: true,
        lastPointer: event.pointer,
        lastTimestamp: event.timestamp,
        velocityX: 0,
        velocityY: 0,
      }

    case "PAN_MOVE": {
      if (!state.isDragging || !state.lastPointer) {
        return state
      }
      const dx = event.pointer.x - state.lastPointer.x
      const dy = event.pointer.y - state.lastPointer.y
      const dt = Math.max(1, event.timestamp - state.lastTimestamp)
      
      return {
        ...state,
        panX: state.panX + dx,
        panY: state.panY + dy,
        lastPointer: event.pointer,
        lastTimestamp: event.timestamp,
        velocityX: dx / dt * 16, // normalize to ~60fps
        velocityY: dy / dt * 16,
      }
    }

    case "PAN_END":
      return {
        ...state,
        isDragging: false,
        lastPointer: null,
        lastTimestamp: event.timestamp,
      }

    case "ZOOM": {
      // Zoom around pointer position
      const oldScale = state.scale
      const newScale = Math.max(
        config.minScale,
        Math.min(config.maxScale, oldScale * (1 - event.delta * config.zoomSensitivity))
      )
      
      if (newScale === oldScale) {
        return state
      }

      // Calculate the point in canvas space that should stay fixed
      const canvasX = (event.pointer.x - state.panX) / oldScale
      const canvasY = (event.pointer.y - state.panY) / oldScale

      // Calculate new pan to keep that point fixed
      const newPanX = event.pointer.x - canvasX * newScale
      const newPanY = event.pointer.y - canvasY * newScale

      return {
        ...state,
        scale: newScale,
        panX: newPanX,
        panY: newPanY,
      }
    }

    case "RESET":
      return initialCanvasState(state.viewportWidth, state.viewportHeight)

    case "RESIZE":
      return {
        ...state,
        viewportWidth: event.width,
        viewportHeight: event.height,
      }

    case "TICK": {
      // Apply inertial panning
      if (state.isDragging) {
        return state
      }
      
      const absVelX = Math.abs(state.velocityX)
      const absVelY = Math.abs(state.velocityY)
      
      // Stop if velocity is negligible
      if (absVelX < 0.01 && absVelY < 0.01) {
        if (state.velocityX === 0 && state.velocityY === 0) {
          return state
        }
        return {
          ...state,
          velocityX: 0,
          velocityY: 0,
        }
      }

      return {
        ...state,
        panX: state.panX + state.velocityX,
        panY: state.panY + state.velocityY,
        velocityX: state.velocityX * config.inertiaDecay,
        velocityY: state.velocityY * config.inertiaDecay,
      }
    }
  }
}

// Helper to convert screen coordinates to canvas coordinates
export function screenToCanvas(state: CanvasState, screen: Point): Point {
  return {
    x: (screen.x - state.panX) / state.scale,
    y: (screen.y - state.panY) / state.scale,
  }
}

// Helper to convert canvas coordinates to screen coordinates
export function canvasToScreen(state: CanvasState, canvas: Point): Point {
  return {
    x: canvas.x * state.scale + state.panX,
    y: canvas.y * state.scale + state.panY,
  }
}
