import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx } from "./utils.js"

export type ProgressProps = {
  readonly className?: string
  readonly value?: number
}

export const Progress = ({ className, value = 0 }: ProgressProps): TemplateResult => {
  const clamped = Math.max(0, Math.min(100, value))
  return html`
    <div
      data-slot="progress"
      class="${cx("bg-primary/20 relative h-2 w-full overflow-hidden rounded-full", className)}"
    >
      <div
        data-slot="progress-indicator"
        class="bg-primary h-full w-full flex-1 transition-all"
        style="transform: translateX(-${100 - clamped}%);"
      ></div>
    </div>
  `
}
