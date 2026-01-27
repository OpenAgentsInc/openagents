import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type AIChildren } from "./utils.js"

export type ToolbarProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Toolbar = ({ className, children }: ToolbarProps): TemplateResult =>
  html`<div class="${cx("flex items-center gap-1 rounded-sm border bg-background p-1.5", className)}">${children ?? ""}</div>`
