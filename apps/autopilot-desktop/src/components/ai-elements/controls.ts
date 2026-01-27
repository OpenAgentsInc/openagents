import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type AIChildren } from "./utils.js"

export type ControlsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Controls = ({ className, children }: ControlsProps): TemplateResult => html`
  <div
    class="${cx(
      "gap-px overflow-hidden rounded-md border bg-card p-1 shadow-none!",
      "[&>button]:rounded-md [&>button]:border-none! [&>button]:bg-transparent! [&>button]:hover:bg-secondary!",
      className
    )}"
  >
    ${children ?? ""}
  </div>
`
