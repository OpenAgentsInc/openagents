import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Badge } from "../ui/badge.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"
import { CodeBlock } from "./code-block.js"

export type ToolState =
  | "input-streaming"
  | "input-available"
  | "approval-requested"
  | "approval-responded"
  | "output-available"
  | "output-error"
  | "output-denied"

export type ToolProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Tool = ({ className, children }: ToolProps): TemplateResult =>
  Collapsible({ className: cx("group not-prose mb-4 w-full rounded-md border", className), children })

export type ToolPart = {
  readonly type: string
  readonly state: ToolState
  readonly input?: unknown
  readonly output?: unknown
  readonly errorText?: string
}

const statusLabels: Record<ToolState, string> = {
  "input-streaming": "Pending",
  "input-available": "Running",
  "approval-requested": "Awaiting Approval",
  "approval-responded": "Responded",
  "output-available": "Completed",
  "output-error": "Error",
  "output-denied": "Denied",
}

const statusIcons: Record<ToolState, string> = {
  "input-streaming": "o",
  "input-available": "...",
  "approval-requested": "...",
  "approval-responded": "ok",
  "output-available": "ok",
  "output-error": "x",
  "output-denied": "no",
}

export const getStatusBadge = (status: ToolState): TemplateResult =>
  Badge({ className: "gap-1.5 rounded-full text-xs", variant: "secondary", children: html`${statusIcons[status]} ${statusLabels[status]}` })

export type ToolHeaderProps = {
  readonly className?: string
  readonly title?: string
  readonly type: ToolPart["type"]
  readonly state: ToolPart["state"]
  readonly toolName?: string
}

export const ToolHeader = ({ className, title, type, state, toolName }: ToolHeaderProps): TemplateResult => {
  const derivedName = type === "dynamic-tool" ? toolName : type.split("-").slice(1).join("-")
  return CollapsibleTrigger({
    className: cx("flex w-full items-center justify-between gap-4 p-3", className),
    children: html`
      <div class="flex items-center gap-2">
        <span class="size-4 text-muted-foreground">tool</span>
        <span class="font-medium text-sm">${title ?? derivedName}</span>
        ${getStatusBadge(state)}
      </div>
      <span class="size-4 text-muted-foreground transition-transform group-data-[state=open]:rotate-180">v</span>
    `,
  })
}

export type ToolContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ToolContent = ({ className, children }: ToolContentProps): TemplateResult =>
  CollapsibleContent({
    className: cx(
      "data-[state=closed]:fade-out-0 data-[state=closed]:slide-out-to-top-2 data-[state=open]:slide-in-from-top-2 text-popover-foreground outline-none data-[state=closed]:animate-out data-[state=open]:animate-in",
      className
    ),
    children,
  })

export type ToolInputProps = {
  readonly className?: string
  readonly input?: unknown
}

export const ToolInput = ({ className, input }: ToolInputProps): TemplateResult => html`
  <div class="${cx("space-y-2 overflow-hidden p-4", className)}">
    <h4 class="font-medium text-muted-foreground text-xs uppercase tracking-wide">Parameters</h4>
    <div class="rounded-md bg-muted/50">${CodeBlock({ code: JSON.stringify(input ?? {}, null, 2), language: "json" })}</div>
  </div>
`

export type ToolOutputProps = {
  readonly className?: string
  readonly output?: unknown
  readonly errorText?: string
}

export const ToolOutput = ({ className, output, errorText }: ToolOutputProps): TemplateResult => {
  if (!(output || errorText)) {
    return html``
  }
  const body = typeof output === "string" ? output : JSON.stringify(output ?? {}, null, 2)
  return html`
    <div class="${cx("space-y-2 p-4", className)}">
      <h4 class="font-medium text-muted-foreground text-xs uppercase tracking-wide">${errorText ? "Error" : "Result"}</h4>
      <div class="${cx(
        "overflow-x-auto rounded-md text-xs [&_table]:w-full",
        errorText ? "bg-destructive/10 text-destructive" : "bg-muted/50 text-foreground"
      )}">
        ${errorText ? html`<div>${errorText}</div>` : ""}
        ${CodeBlock({ code: body, language: "json" })}
      </div>
    </div>
  `
}
