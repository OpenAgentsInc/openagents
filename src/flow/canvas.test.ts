import { describe, it, expect } from "bun:test"
import {
  reduceCanvasState,
  initialCanvasState,
  screenToCanvas,
  canvasToScreen,
  DEFAULT_CONFIG,
  type CanvasState,
  type CanvasEvent,
} from "./canvas.js"

describe("reduceCanvasState", () => {
  const state = initialCanvasState(800, 600)

  describe("PAN_START", () => {
    it("sets isDragging and records pointer", () => {
      const event: CanvasEvent = { type: "PAN_START", pointer: { x: 100, y: 200 }, timestamp: 1000 }
      const next = reduceCanvasState(state, event)
      expect(next.isDragging).toBe(true)
      expect(next.lastPointer).toEqual({ x: 100, y: 200 })
      expect(next.lastTimestamp).toBe(1000)
      expect(next.velocityX).toBe(0)
      expect(next.velocityY).toBe(0)
    })
  })

  describe("PAN_MOVE", () => {
    it("updates pan position based on pointer delta", () => {
      const dragging: CanvasState = {
        ...state,
        isDragging: true,
        lastPointer: { x: 100, y: 100 },
        lastTimestamp: 1000,
      }
      const event: CanvasEvent = { type: "PAN_MOVE", pointer: { x: 150, y: 120 }, timestamp: 1016 }
      const next = reduceCanvasState(dragging, event)
      expect(next.panX).toBe(50)
      expect(next.panY).toBe(20)
      expect(next.lastPointer).toEqual({ x: 150, y: 120 })
    })

    it("ignores move when not dragging", () => {
      const event: CanvasEvent = { type: "PAN_MOVE", pointer: { x: 150, y: 120 }, timestamp: 1016 }
      const next = reduceCanvasState(state, event)
      expect(next).toBe(state)
    })

    it("calculates velocity from movement", () => {
      const dragging: CanvasState = {
        ...state,
        isDragging: true,
        lastPointer: { x: 100, y: 100 },
        lastTimestamp: 1000,
      }
      const event: CanvasEvent = { type: "PAN_MOVE", pointer: { x: 116, y: 108 }, timestamp: 1016 }
      const next = reduceCanvasState(dragging, event)
      // dx=16, dy=8, dt=16 -> vel = dx/dt*16 = 16, 8
      expect(next.velocityX).toBe(16)
      expect(next.velocityY).toBe(8)
    })
  })

  describe("PAN_END", () => {
    it("stops dragging but keeps velocity for inertia", () => {
      const dragging: CanvasState = {
        ...state,
        isDragging: true,
        lastPointer: { x: 100, y: 100 },
        velocityX: 10,
        velocityY: 5,
      }
      const event: CanvasEvent = { type: "PAN_END", timestamp: 2000 }
      const next = reduceCanvasState(dragging, event)
      expect(next.isDragging).toBe(false)
      expect(next.lastPointer).toBeNull()
      expect(next.velocityX).toBe(10)
      expect(next.velocityY).toBe(5)
    })
  })

  describe("ZOOM", () => {
    it("zooms in around pointer (positive delta = zoom out)", () => {
      const event: CanvasEvent = { type: "ZOOM", pointer: { x: 400, y: 300 }, delta: -100 }
      const next = reduceCanvasState(state, event)
      expect(next.scale).toBeGreaterThan(1)
    })

    it("zooms out around pointer (negative delta = zoom in)", () => {
      const event: CanvasEvent = { type: "ZOOM", pointer: { x: 400, y: 300 }, delta: 100 }
      const next = reduceCanvasState(state, event)
      expect(next.scale).toBeLessThan(1)
    })

    it("respects minScale", () => {
      const zoomed: CanvasState = { ...state, scale: 0.15 }
      const event: CanvasEvent = { type: "ZOOM", pointer: { x: 400, y: 300 }, delta: 1000 }
      const next = reduceCanvasState(zoomed, event, DEFAULT_CONFIG)
      expect(next.scale).toBeGreaterThanOrEqual(DEFAULT_CONFIG.minScale)
    })

    it("respects maxScale", () => {
      const zoomed: CanvasState = { ...state, scale: 3.5 }
      const event: CanvasEvent = { type: "ZOOM", pointer: { x: 400, y: 300 }, delta: -1000 }
      const next = reduceCanvasState(zoomed, event, DEFAULT_CONFIG)
      expect(next.scale).toBeLessThanOrEqual(DEFAULT_CONFIG.maxScale)
    })

    it("keeps pointer fixed in canvas space (zoom-around-pointer)", () => {
      // Pointer at center of viewport
      const pointer = { x: 400, y: 300 }
      const event: CanvasEvent = { type: "ZOOM", pointer, delta: -100 }
      
      // Before zoom: screen point maps to canvas point
      const canvasBefore = screenToCanvas(state, pointer)
      
      const next = reduceCanvasState(state, event)
      
      // After zoom: same screen point should map to same canvas point
      const canvasAfter = screenToCanvas(next, pointer)
      
      expect(canvasAfter.x).toBeCloseTo(canvasBefore.x, 5)
      expect(canvasAfter.y).toBeCloseTo(canvasBefore.y, 5)
    })
  })

  describe("RESET", () => {
    it("returns to initial state with same viewport", () => {
      const modified: CanvasState = {
        ...state,
        scale: 2,
        panX: 100,
        panY: -50,
        velocityX: 10,
      }
      const event: CanvasEvent = { type: "RESET" }
      const next = reduceCanvasState(modified, event)
      expect(next.scale).toBe(1)
      expect(next.panX).toBe(0)
      expect(next.panY).toBe(0)
      expect(next.velocityX).toBe(0)
      expect(next.viewportWidth).toBe(800)
      expect(next.viewportHeight).toBe(600)
    })
  })

  describe("RESIZE", () => {
    it("updates viewport dimensions", () => {
      const event: CanvasEvent = { type: "RESIZE", width: 1024, height: 768 }
      const next = reduceCanvasState(state, event)
      expect(next.viewportWidth).toBe(1024)
      expect(next.viewportHeight).toBe(768)
    })
  })

  describe("TICK (inertial panning)", () => {
    it("applies velocity to pan position", () => {
      const moving: CanvasState = {
        ...state,
        velocityX: 10,
        velocityY: 5,
      }
      const event: CanvasEvent = { type: "TICK" }
      const next = reduceCanvasState(moving, event)
      expect(next.panX).toBe(10)
      expect(next.panY).toBe(5)
    })

    it("decays velocity over time", () => {
      const moving: CanvasState = {
        ...state,
        velocityX: 10,
        velocityY: 5,
      }
      const event: CanvasEvent = { type: "TICK" }
      const next = reduceCanvasState(moving, event)
      expect(next.velocityX).toBeLessThan(10)
      expect(next.velocityY).toBeLessThan(5)
    })

    it("stops when velocity is negligible", () => {
      const almostStopped: CanvasState = {
        ...state,
        velocityX: 0.005,
        velocityY: 0.003,
      }
      const event: CanvasEvent = { type: "TICK" }
      const next = reduceCanvasState(almostStopped, event)
      expect(next.velocityX).toBe(0)
      expect(next.velocityY).toBe(0)
    })

    it("does nothing when already stopped", () => {
      const event: CanvasEvent = { type: "TICK" }
      const next = reduceCanvasState(state, event)
      expect(next).toBe(state)
    })

    it("does nothing while dragging", () => {
      const dragging: CanvasState = {
        ...state,
        isDragging: true,
        velocityX: 10,
        velocityY: 5,
      }
      const event: CanvasEvent = { type: "TICK" }
      const next = reduceCanvasState(dragging, event)
      expect(next).toBe(dragging)
    })
  })
})

describe("coordinate conversions", () => {
  it("screenToCanvas inverts canvasToScreen", () => {
    const state: CanvasState = {
      ...initialCanvasState(800, 600),
      scale: 2,
      panX: 100,
      panY: 50,
    }
    const canvas = { x: 150, y: 200 }
    const screen = canvasToScreen(state, canvas)
    const back = screenToCanvas(state, screen)
    expect(back.x).toBeCloseTo(canvas.x, 10)
    expect(back.y).toBeCloseTo(canvas.y, 10)
  })

  it("screenToCanvas accounts for scale and pan", () => {
    const state: CanvasState = {
      ...initialCanvasState(800, 600),
      scale: 2,
      panX: 100,
      panY: 50,
    }
    // screen(100, 50) with pan(100, 50) and scale 2
    // canvas = (screen - pan) / scale = (100-100, 50-50) / 2 = (0, 0)
    const canvas = screenToCanvas(state, { x: 100, y: 50 })
    expect(canvas.x).toBe(0)
    expect(canvas.y).toBe(0)
  })
})
