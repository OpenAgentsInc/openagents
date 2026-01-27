import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type SeparatorProps = {
  readonly className?: string
  readonly orientation?: "horizontal" | "vertical"
  readonly decorative?: boolean
}

export const Separator = ({
  className,
  orientation = "horizontal",
  decorative = true,
}: SeparatorProps): TemplateResult => {
  return html`
    <div
      data-slot="separator"
      data-orientation="${orientation}"
      aria-hidden="${decorative ? "true" : "false"}"
      class="${cx(
        "bg-border shrink-0 data-[orientation=horizontal]:h-px data-[orientation=horizontal]:w-full data-[orientation=vertical]:h-full data-[orientation=vertical]:w-px",
        className
      )}"
    ></div>
  `
}
