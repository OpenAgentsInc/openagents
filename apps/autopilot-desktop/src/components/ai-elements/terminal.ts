import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { cx, type AIChildren } from "./utils.js"

export type TerminalProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Terminal = ({ className, children }: TerminalProps): TemplateResult => html`
  <div class="${cx("overflow-hidden rounded-lg border bg-background", className)}">${children ?? ""}</div>
`

export type TerminalHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalHeader = ({ className, children }: TerminalHeaderProps): TemplateResult => html`
  <div class="${cx("flex items-center justify-between border-b px-3 py-2", className)}">${children ?? ""}</div>
`

export type TerminalTitleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalTitle = ({ className, children }: TerminalTitleProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-2 text-sm font-medium", className)}">${children ?? "Terminal"}</div>
`

export type TerminalStatusProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalStatus = ({ className, children }: TerminalStatusProps): TemplateResult => html`
  <div class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Idle"}</div>
`

export type TerminalActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalActions = ({ className, children }: TerminalActionsProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-1", className)}">${children ?? ""}</div>
`

export type TerminalCopyButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalCopyButton = ({ className, children }: TerminalCopyButtonProps): TemplateResult =>
  Button({ className: cx("size-7", className), size: "icon", type: "button", variant: "ghost", children: children ?? "copy" })

export type TerminalClearButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalClearButton = ({ className, children }: TerminalClearButtonProps): TemplateResult =>
  Button({ className: cx("size-7", className), size: "icon", type: "button", variant: "ghost", children: children ?? "clear" })

export type TerminalContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const TerminalContent = ({ className, children }: TerminalContentProps): TemplateResult => html`
  <div class="${cx("max-h-64 overflow-auto p-3 font-mono text-xs text-foreground", className)}">${children ?? ""}</div>
`
