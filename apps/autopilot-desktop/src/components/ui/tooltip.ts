import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type TooltipProviderProps = {
  readonly delayDuration?: number
  readonly children?: UIChildren
}

export const TooltipProvider = ({ delayDuration, children }: TooltipProviderProps): TemplateResult => {
  return html`
    <div data-slot="tooltip-provider" data-delay-duration="${delayDuration ?? ""}">
      ${children ?? ""}
    </div>
  `
}

export type TooltipProps = {
  readonly children?: UIChildren
}

export const Tooltip = ({ children }: TooltipProps): TemplateResult => {
  return TooltipProvider({ children: html`<div data-slot="tooltip" data-state="closed">${children ?? ""}</div>` })
}

export type TooltipTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const TooltipTrigger = ({ className, children }: TooltipTriggerProps): TemplateResult => {
  return html`
    <span data-slot="tooltip-trigger" class="${cx(className)}">${children ?? ""}</span>
  `
}

export type TooltipContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly side?: "top" | "bottom" | "left" | "right"
  readonly state?: "open" | "closed"
}

export const TooltipContent = ({
  className,
  children,
  side = "top",
  state = "closed",
}: TooltipContentProps): TemplateResult => {
  return html`
    <div data-slot="tooltip-portal">
      <div
        data-slot="tooltip-content"
        data-side="${side}"
        data-state="${state}"
        class="${cx(
          "bg-foreground text-background animate-in fade-in-0 zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-fit origin-(--radix-tooltip-content-transform-origin) rounded-md px-3 py-1.5 text-xs text-balance",
          className
        )}"
      >
        ${children ?? ""}
        <span class="bg-foreground fill-foreground z-50 size-2.5 translate-y-[calc(-50%_-_2px)] rotate-45 rounded-[2px]"></span>
      </div>
    </div>
  `
}
