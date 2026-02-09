import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

const ANIMATION_DURATION = 4
const MAX_RADIUS_MULTIPLIER = 1.1
const MIN_OPACITY = 0.5
const MAX_OPACITY = 0.8
/** Stagger the two animations; fixed values to avoid SSR/client hydration mismatch. */
const BEGIN_R = "0s"
const BEGIN_OPACITY = "1s"
const GRID_OFFSET = -5000
const GRID_DIMENSION = 10000

export type GridPatternProps = {
  readonly gridSize: number
  readonly dotRadius: number
  readonly dotClassName?: string
  readonly patternId?: string
}

export function GridPattern({
  gridSize,
  dotRadius,
  dotClassName = "oa-flow-grid-dot",
  patternId = "oa-flow-dot-grid",
}: GridPatternProps): TemplateResult {
  return html`
    <defs>
      <pattern
        id="${patternId}"
        x="0"
        y="0"
        width="${String(gridSize)}"
        height="${String(gridSize)}"
        patternUnits="userSpaceOnUse"
      >
        <circle
          cx="${String(gridSize / 2)}"
          cy="${String(gridSize / 2)}"
          r="${String(dotRadius)}"
          class="${dotClassName}"
        >
          <animate
            attributeName="r"
            values="${`${dotRadius};${dotRadius * MAX_RADIUS_MULTIPLIER};${dotRadius}`}"
            dur="${`${ANIMATION_DURATION}s`}"
            begin="${BEGIN_R}"
            repeatCount="indefinite"
          />
          <animate
            attributeName="opacity"
            values="${`${MIN_OPACITY};${MAX_OPACITY};${MIN_OPACITY}`}"
            dur="${`${ANIMATION_DURATION}s`}"
            begin="${BEGIN_OPACITY}"
            repeatCount="indefinite"
          />
        </circle>
      </pattern>
    </defs>
    <rect
      x="${String(GRID_OFFSET)}"
      y="${String(GRID_OFFSET)}"
      width="${String(GRID_DIMENSION)}"
      height="${String(GRID_DIMENSION)}"
      fill="${`url(#${patternId})`}"
    />
  `
}
