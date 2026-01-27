import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type EdgeProps = {
  readonly path: string
  readonly dashed?: boolean
  readonly animated?: boolean
  readonly stroke?: string
  readonly className?: string
}

export const Edge = ({
  path,
  dashed = false,
  animated = false,
  stroke = "var(--ring)",
  className = "",
}: EdgeProps): TemplateResult => html`
  <svg class="pointer-events-none absolute inset-0 h-full w-full ${className}">
    <path
      d="${path}"
      fill="none"
      stroke="${stroke}"
      stroke-width="1"
      ${dashed ? "stroke-dasharray=\"5 5\"" : ""}
    ></path>
    ${animated
      ? html`
          <circle fill="var(--primary)" r="4">
            <animateMotion dur="2s" path="${path}" repeatCount="indefinite" />
          </circle>
        `
      : ""}
  </svg>
`
