import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type ConnectionProps = {
  readonly fromX: number
  readonly fromY: number
  readonly toX: number
  readonly toY: number
}

const HALF = 0.5

export const Connection = ({ fromX, fromY, toX, toY }: ConnectionProps): TemplateResult => {
  const path = `M${fromX},${fromY} C ${fromX + (toX - fromX) * HALF},${fromY} ${
    fromX + (toX - fromX) * HALF
  },${toY} ${toX},${toY}`

  return html`
    <g>
      <path
        class="animated"
        d="${path}"
        fill="none"
        stroke="var(--color-ring)"
        stroke-width="1"
      ></path>
      <circle
        cx="${toX}"
        cy="${toY}"
        fill="#fff"
        r="3"
        stroke="var(--color-ring)"
        stroke-width="1"
      ></circle>
    </g>
  `
}
