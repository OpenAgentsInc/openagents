import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { ScrollArea, ScrollBar } from "../ui/scroll-area.js"
import { cx, type AIChildren } from "./utils.js"

export type SuggestionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Suggestions = ({ className, children }: SuggestionsProps): TemplateResult =>
  ScrollArea({
    className: "w-full overflow-x-auto whitespace-nowrap",
    children: html`
      <div class="${cx("flex w-max flex-nowrap items-center gap-2", className)}">${children ?? ""}</div>
      ${ScrollBar({ className: "hidden", orientation: "horizontal" })}
    `,
  })

export type SuggestionProps = {
  readonly suggestion: string
  readonly className?: string
  readonly children?: AIChildren
}

export const Suggestion = ({ suggestion, className, children }: SuggestionProps): TemplateResult =>
  Button({
    className: cx("cursor-pointer rounded-full px-4", className),
    size: "sm",
    type: "button",
    variant: "outline",
    children: children ?? suggestion,
  })
