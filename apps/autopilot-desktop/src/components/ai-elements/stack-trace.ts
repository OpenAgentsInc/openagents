import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { cx, type AIChildren } from "./utils.js"

export type StackTraceProps = {
  readonly trace: string
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTrace = ({ trace, className, children }: StackTraceProps): TemplateResult => html`
  <div class="${cx("not-prose w-full overflow-hidden rounded-lg border bg-background font-mono text-sm", className)}">
    ${children ?? html`
      ${StackTraceHeader({ children: html`${StackTraceError({ children: html`<span class="font-semibold text-destructive">Error</span><span class="truncate">${trace.split("\n")[0] ?? ""}</span>` })}${StackTraceActions({})}` })}
      ${StackTraceContent({ children: html`${StackTraceFrames({ children: html`<pre class="whitespace-pre-wrap p-3 text-xs text-muted-foreground">${trace}</pre>` })}` })}
    `}
  </div>
`

export type StackTraceHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceHeader = ({ className, children }: StackTraceHeaderProps): TemplateResult =>
  Collapsible({
    children: CollapsibleTrigger({
      children: html`<div class="${cx("flex w-full cursor-pointer items-center gap-3 p-3 text-left transition-colors hover:bg-muted/50", className)}">${children ?? ""}</div>`,
    }),
  })

export type StackTraceErrorProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceError = ({ className, children }: StackTraceErrorProps): TemplateResult => html`
  <div class="${cx("flex flex-1 items-center gap-2 overflow-hidden", className)}">${children ?? ""}</div>
`

export type StackTraceErrorTypeProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceErrorType = ({ className, children }: StackTraceErrorTypeProps): TemplateResult => html`
  <span class="${cx("shrink-0 font-semibold text-destructive", className)}">${children ?? "Error"}</span>
`

export type StackTraceErrorMessageProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceErrorMessage = ({ className, children }: StackTraceErrorMessageProps): TemplateResult => html`
  <span class="${cx("truncate text-muted-foreground", className)}">${children ?? ""}</span>
`

export type StackTraceActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceActions = ({ className, children }: StackTraceActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-1", className)}">${children ?? ""}</div>
`

export type StackTraceCopyButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceCopyButton = ({ className, children }: StackTraceCopyButtonProps): TemplateResult =>
  Button({ className: cx("size-7", className), size: "icon", type: "button", variant: "ghost", children: children ?? "copy" })

export type StackTraceExpandButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceExpandButton = ({ className, children }: StackTraceExpandButtonProps): TemplateResult =>
  Button({ className: cx("size-7", className), size: "icon", type: "button", variant: "ghost", children: children ?? "v" })

export type StackTraceContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceContent = ({ className, children }: StackTraceContentProps): TemplateResult =>
  CollapsibleContent({ className: cx("border-t", className), children })

export type StackTraceFramesProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const StackTraceFrames = ({ className, children }: StackTraceFramesProps): TemplateResult => html`
  <div class="${cx("divide-y", className)}">${children ?? ""}</div>
`
