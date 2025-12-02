import type { Point } from './model.js'

export interface PathConfig {
  readonly cornerRadius: number
}

/**
 * Builds an SVG path string for a polyline with rounded corners at turns.
 * Uses exact quarter-circle arcs for orthogonal 90° turns, straight lines otherwise.
 * Detects corners via angle (>5°), segment length, and collinearity.
 */
export function buildRoundedPath(points: readonly Point[], config: PathConfig): string {
  const r = config.cornerRadius
  if (r < 0) throw new Error('cornerRadius must be >= 0')
  if (points.length < 2) return ''

  let d = `M ${points[0].x.toFixed(2)} ${points[0].y.toFixed(2)}`

  for (let i = 0; i < points.length - 1; i++) {
    const prev = points[i]
    const curr = points[i + 1]
    const hasNext = i + 2 < points.length
    if (!hasNext) {
      // Last segment always straight
      d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`
      continue
    }

    const next = points[i + 2]

    // Incoming vector unit
    const dx1 = curr.x - prev.x
    const dy1 = curr.y - prev.y
    const len1 = Math.hypot(dx1, dy1)
    
    // Outgoing unit
    const dx2 = next.x - curr.x
    const dy2 = next.y - curr.y
    const len2 = Math.hypot(dx2, dy2)

    // Skip corner rounding if radius is 0 or segments too short
    if (r === 0 || len1 === 0 || len2 === 0 || len1 < 2 * r || len2 < 2 * r) {
      d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`
      continue
    }

    const ux1 = dx1 / len1
    const uy1 = dy1 / len1
    const ux2 = dx2 / len2
    const uy2 = dy2 / len2

    // Collinear check
    const dot = ux1 * ux2 + uy1 * uy2
    if (dot > 0.95) {
      d += ` L ${curr.x.toFixed(2)} ${curr.y.toFixed(2)}`
      continue
    }

    // Corner! Compute offsets
    const cross = ux1 * uy2 - uy1 * ux2
    const endInX = curr.x - r * ux1
    const endInY = curr.y - r * uy1
    const startOutX = curr.x + r * ux2
    const startOutY = curr.y + r * uy2

    // Add straight to endIn, arc to startOut
    d += ` L ${endInX.toFixed(2)} ${endInY.toFixed(2)}`
    const sweep = cross > 0 ? 1 : 0
    d += ` A ${r.toFixed(2)} ${r.toFixed(2)} 0 0 ${sweep} ${startOutX.toFixed(2)} ${startOutY.toFixed(2)}`
  }

  return d
}
