/**
 * Geometry utilities harvested from Unit framework
 * Source: ~/code/unit/src/client/util/geometry/
 */

// ============================================================================
// Types
// ============================================================================

export interface Point {
  x: number
  y: number
}

export interface Position {
  x: number
  y: number
}

export type Shape = "circle" | "rect"

export interface Thing {
  shape: Shape
  x: number
  y: number
  r: number
  width: number
  height: number
}

// ============================================================================
// SVG Path Utilities
// ============================================================================

/**
 * Generate SVG path for a rectangle
 */
export function describeRect(
  x: number,
  y: number,
  width: number,
  height: number
): string {
  const d = `M ${x} ${y} H ${x + width} V ${y + height} H ${x} Z`
  return d
}

/**
 * Generate SVG path for a circle
 */
export function describeCircle(x: number, y: number, r: number): string {
  const d = `M ${x - r} ${y} A ${r} ${r} 0 1 0 ${x - r} ${y - 0.001} z`
  return d
}

// ============================================================================
// Vector Math
// ============================================================================

/**
 * Calculate unit vector from (x0, y0) to (x1, y1)
 */
export function unitVector(
  x0: number,
  y0: number,
  x1: number,
  y1: number
): Point {
  const dx = x1 - x0
  const dy = y1 - y0
  const d = norm(dx, dy)
  if (d === 0) {
    return randomUnitVector()
  }
  return { x: dx / d, y: dy / d }
}

/**
 * Calculate unit vector between two points
 */
export function pointUnitVector(
  { x: x0, y: y0 }: Point,
  { x: x1, y: y1 }: Point
): Point {
  const dx = x1 - x0
  const dy = y1 - y0
  const d = norm(dx, dy)
  if (d === 0) {
    return randomUnitVector()
  }
  return { x: dx / d, y: dy / d }
}

/**
 * Calculate magnitude of vector (x, y)
 */
export function norm(x: number, y: number): number {
  return Math.sqrt(x * x + y * y)
}

/**
 * Generate random unit vector
 */
export function randomUnitVector(): Point {
  return normalize({ x: 0.5 - Math.random(), y: 0.5 - Math.random() })
}

/**
 * Normalize a point to unit vector
 */
export function normalize(point: Point): Point {
  const { x, y } = point
  const d = norm(point.x, point.y)
  return { x: x / d, y: y / d }
}

/**
 * Calculate distance between two points
 */
export function distance(
  ax: number,
  ay: number,
  bx: number,
  by: number
): number {
  return norm(ax - bx, ay - by)
}

/**
 * Calculate distance between two Point objects
 */
export function pointDistance(a: Point, b: Point): number {
  return distance(a.x, a.y, b.x, b.y)
}

// ============================================================================
// Node Geometry
// ============================================================================

/**
 * Find point on node surface in direction of unit vector u
 */
export function pointInNode(node: Thing, u: Point, padding: number = 0): Point {
  const { shape } = node
  if (shape === "circle") {
    return pointInCircle(node.x, node.y, node.r, u, padding)
  } else {
    return pointInRectangle(node.x, node.y, node.width, node.height, u, padding)
  }
}

/**
 * Find point on circle surface
 */
export function pointInCircle(
  x: number,
  y: number,
  R: number,
  u: Point,
  padding: number = 0
): Point {
  return {
    x: x + u.x * (R - padding),
    y: y + u.y * (R - padding),
  }
}

/**
 * Find point on rectangle surface
 */
export function pointInRectangle(
  x: number,
  y: number,
  width: number,
  height: number,
  u: Point,
  padding: number = 0
): Point {
  const region = rectangleRegion(x, y, width, height, u)
  const tan = u.x / u.y
  const a = Math.atan2(u.y, u.x)
  if (region === "left" || region === "right") {
    const sx = Math.sign(u.x)
    return {
      x: x + sx * (width / 2) - padding * Math.cos(a),
      y: y + sx * (width / 2 / tan) - padding * Math.sin(a),
    }
  } else {
    const sy = Math.sign(u.y)
    return {
      x: x + sy * ((height / 2) * tan) - padding * Math.cos(a),
      y: y + sy * (height / 2) - padding * Math.sin(a),
    }
  }
}

/**
 * Determine which region of rectangle the unit vector points to
 */
export function rectangleRegion(
  x: number,
  y: number,
  w: number,
  h: number,
  u: Point
): "left" | "top" | "right" | "bottom" {
  const { x: ux, y: uy } = u
  const { x: ax, y: ay } = unitVector(x, y, x - w / 2, y - h / 2)
  const { x: bx, y: by } = unitVector(x, y, x + w / 2, y - h / 2)
  const alpha = radBetween(ax, ay, bx, by)
  const beta = radBetween(ux, uy, bx, by)
  if (beta <= alpha) {
    return "top"
  } else if (beta <= Math.PI) {
    return "left"
  } else if (beta <= Math.PI + alpha) {
    return "bottom"
  } else {
    return "right"
  }
}

const TWO_PI = 2 * Math.PI

/**
 * Calculate radians between two vectors
 */
export function radBetween(
  ax: number,
  ay: number,
  bx: number,
  by: number
): number {
  let a = Math.atan2(by, bx) - Math.atan2(ay, ax)
  if (a < 0) {
    a += TWO_PI
  }
  return a
}

// ============================================================================
// Surface Distance
// ============================================================================

/**
 * Calculate surface distance between two nodes
 * Returns { l: gap distance, d: center distance, u: unit vector }
 */
export function surfaceDistance(
  a: Thing,
  b: Thing
): { l: number; d: number; u: Point } {
  const d = distance(a.x, a.y, b.x, b.y)
  const u = unitVector(a.x, a.y, b.x, b.y)

  const a_d: number = centerToSurfaceDistance(a, u)
  const b_d: number = centerToSurfaceDistance(b, u)

  const d_sum = b_d + a_d
  const l = d - d_sum

  return {
    d,
    l,
    u,
  }
}

/**
 * Calculate distance from node center to surface in direction u
 */
export function centerToSurfaceDistance(node: Thing, u: Position): number {
  const { shape, x, y, r, width, height } = node
  if (shape === "circle") {
    return r
  } else {
    const tan = u.x / u.y
    const region = rectangleRegion(x, y, width, height, u)
    if (region === "left" || region === "right") {
      return norm(width / 2, width / 2 / tan)
    } else {
      return norm((height / 2) * tan, height / 2)
    }
  }
}
