import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from "../ui/collapsible.js"
import { Input } from "../ui/input.js"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip.js"
import { cx, type AIChildren } from "./utils.js"

export type WebPreviewProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const WebPreview = ({ className, children }: WebPreviewProps): TemplateResult => html`
  <div class="${cx("flex size-full flex-col rounded-lg border bg-card", className)}">${children ?? ""}</div>
`

export type WebPreviewNavigationProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const WebPreviewNavigation = ({ className, children }: WebPreviewNavigationProps): TemplateResult => html`
  <div class="${cx("flex items-center gap-1 border-b p-2", className)}">${children ?? ""}</div>
`

export type WebPreviewNavigationButtonProps = {
  readonly tooltip?: string
  readonly children?: AIChildren
}

export const WebPreviewNavigationButton = ({ tooltip, children }: WebPreviewNavigationButtonProps): TemplateResult =>
  TooltipProvider({
    children: Tooltip({
      children: html`
        ${TooltipTrigger({ children: Button({ className: "h-8 w-8 p-0 hover:text-foreground", size: "sm", type: "button", variant: "ghost", children: children ?? "<-" }) })}
        ${TooltipContent({ children: tooltip ?? "" })}
      `,
    }),
  })

export type WebPreviewUrlProps = {
  readonly className?: string
  readonly value?: string
}

export const WebPreviewUrl = ({ className, value = "" }: WebPreviewUrlProps): TemplateResult =>
  Input({ className: cx("h-8 text-xs", className), value, placeholder: "https://" })

export type WebPreviewBodyProps = {
  readonly className?: string
  readonly url?: string
}

export const WebPreviewBody = ({ className, url = "" }: WebPreviewBodyProps): TemplateResult =>
  html`<div class="${cx("flex-1 bg-background", className)}"><iframe class="h-full w-full" src="${url}"></iframe></div>`

export type WebPreviewConsoleProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const WebPreviewConsole = ({ className, children }: WebPreviewConsoleProps): TemplateResult =>
  Collapsible({
    children: html`
      ${CollapsibleTrigger({ children: html`<div class="${cx("flex w-full items-center justify-between gap-2 border-t px-3 py-2 text-xs", className)}">Console <span>v</span></div>` })}
      ${CollapsibleContent({ children: html`<div class="p-3 text-xs text-muted-foreground">${children ?? "No console output."}</div>` })}
    `,
  })
