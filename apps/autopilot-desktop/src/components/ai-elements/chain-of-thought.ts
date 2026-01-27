import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type ChainOfThoughtProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly open?: boolean
}

export const ChainOfThought = ({ className, children }: ChainOfThoughtProps): TemplateResult => html`
  <div class="${cx("not-prose max-w-prose space-y-4", className)}">${children ?? ""}</div>
`

export type ChainOfThoughtHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ChainOfThoughtHeader = ({ className, children }: ChainOfThoughtHeaderProps): TemplateResult =>
  Collapsible({
    children: CollapsibleTrigger({
      className: cx(
        "flex w-full items-center gap-2 text-muted-foreground text-sm transition-colors hover:text-foreground",
        className
      ),
      children: children ?? html`
        <span class="size-4">think</span>
        <span class="flex-1 text-left">Chain of Thought</span>
        <span class="size-4">v</span>
      `,
    }),
  })

export type ChainOfThoughtStepProps = {
  readonly className?: string
  readonly label: AIChildren
  readonly description?: AIChildren
  readonly status?: "complete" | "active" | "pending"
  readonly children?: AIChildren
}

export const ChainOfThoughtStep = ({
  className,
  label,
  description,
  status = "complete",
  children,
}: ChainOfThoughtStepProps): TemplateResult => {
  const statusStyles: Record<"complete" | "active" | "pending", string> = {
    complete: "text-muted-foreground",
    active: "text-foreground",
    pending: "text-muted-foreground/50",
  }
  return html`
    <div class="${cx("flex gap-2 text-sm fade-in-0 slide-in-from-top-2 animate-in", statusStyles[status], className)}">
      <div class="relative mt-0.5">
        <span class="size-4">*</span>
        <div class="absolute top-7 bottom-0 left-1/2 -mx-px w-px bg-border"></div>
      </div>
      <div class="flex-1 space-y-2 overflow-hidden">
        <div>${label}</div>
        ${description ? html`<div class="text-muted-foreground text-xs">${description}</div>` : ""}
        ${children ?? ""}
      </div>
    </div>
  `
}

export type ChainOfThoughtSearchResultsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ChainOfThoughtSearchResults = ({ className, children }: ChainOfThoughtSearchResultsProps): TemplateResult => html`
  <div class="${cx("flex flex-wrap items-center gap-2", className)}">${children ?? ""}</div>
`

export type ChainOfThoughtSearchResultProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ChainOfThoughtSearchResult = ({ className, children }: ChainOfThoughtSearchResultProps): TemplateResult =>
  Badge({
    className: cx("gap-1 px-2 py-0.5 font-normal text-xs", className),
    variant: "secondary",
    children,
  })

export type ChainOfThoughtContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ChainOfThoughtContent = ({ className, children }: ChainOfThoughtContentProps): TemplateResult =>
  Collapsible({
    children: CollapsibleContent({
      className: cx(
        "mt-2 space-y-3",
        "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-popover-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
        className
      ),
      children,
    }),
  })
