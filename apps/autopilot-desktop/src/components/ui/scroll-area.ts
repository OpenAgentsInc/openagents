import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type ScrollAreaProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type ScrollBarProps = {
  readonly className?: string
  readonly orientation?: "horizontal" | "vertical"
}

export const ScrollArea = ({ className, children }: ScrollAreaProps): TemplateResult => {
  return html`
    <div data-slot="scroll-area" class="${cx("relative", className)}">
      <div
        data-slot="scroll-area-viewport"
        class="focus-visible:ring-ring/50 size-full rounded-[inherit] transition-[color,box-shadow] outline-none focus-visible:ring-[3px] focus-visible:outline-1"
      >
        ${children ?? ""}
      </div>
      ${ScrollBar({})}
      <div data-slot="scroll-area-corner"></div>
    </div>
  `
}

export const ScrollBar = ({
  className,
  orientation = "vertical",
}: ScrollBarProps): TemplateResult => {
  return html`
    <div
      data-slot="scroll-area-scrollbar"
      data-orientation="${orientation}"
      class="${cx(
        "flex touch-none p-px transition-colors select-none",
        orientation === "vertical" &&
          "h-full w-2.5 border-l border-l-transparent",
        orientation === "horizontal" &&
          "h-2.5 flex-col border-t border-t-transparent",
        className
      )}"
    >
      <div data-slot="scroll-area-thumb" class="bg-border relative flex-1 rounded-full"></div>
    </div>
  `
}
