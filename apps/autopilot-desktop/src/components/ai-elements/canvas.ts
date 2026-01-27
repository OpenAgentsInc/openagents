import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type CanvasProps = {
  readonly title?: string
  readonly subtitle?: string
  readonly status?: string
  readonly className?: string
  readonly children?: TemplateResult | readonly TemplateResult[]
}

export const Canvas = ({
  title,
  subtitle,
  status,
  className = "",
  children,
}: CanvasProps): TemplateResult => html`
  <div class="ai-canvas ${className}">
    <div class="ai-canvas__grid" aria-hidden="true"></div>
    ${(title ?? subtitle ?? status)
      ? html`
          <div class="ai-canvas__meta">
            ${title ? html`<div class="ai-canvas__title">${title}</div>` : ""}
            ${subtitle ? html`<div class="ai-canvas__subtitle">${subtitle}</div>` : ""}
            ${status ? html`<div class="ai-canvas__status">${status}</div>` : ""}
          </div>
        `
      : ""}
    <div class="ai-canvas__content">${children ?? ""}</div>
  </div>
`
