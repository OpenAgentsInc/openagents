import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type AIChildren } from "./utils.js"

export type PanelProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Panel = ({ className, children }: PanelProps): TemplateResult => {
  return html`
    <div class="${cx("m-4 overflow-hidden rounded-md border bg-card p-1", className)}">
      ${children ?? ""}
    </div>
  `
}
