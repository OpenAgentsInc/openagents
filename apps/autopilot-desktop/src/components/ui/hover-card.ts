import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type HoverCardProps = {
  readonly children?: UIChildren
}

export const HoverCard = ({ children }: HoverCardProps): TemplateResult => {
  return html`<div data-slot="hover-card" data-state="closed">${children ?? ""}</div>`
}

export type HoverCardTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const HoverCardTrigger = ({
  className,
  children,
}: HoverCardTriggerProps): TemplateResult => {
  return html`
    <span data-slot="hover-card-trigger" class="${cx(className)}">${children ?? ""}</span>
  `
}

export type HoverCardContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly align?: "start" | "center" | "end"
  readonly side?: "top" | "bottom" | "left" | "right"
  readonly state?: "open" | "closed"
}

export const HoverCardContent = ({
  className,
  children,
  align = "center",
  side = "bottom",
  state = "closed",
}: HoverCardContentProps): TemplateResult => {
  return html`
    <div data-slot="hover-card-portal">
      <div
        data-slot="hover-card-content"
        data-align="${align}"
        data-side="${side}"
        data-state="${state}"
        class="${cx(
          "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-64 origin-(--radix-hover-card-content-transform-origin) rounded-md border p-4 shadow-md outline-hidden",
          className
        )}"
      >
        ${children ?? ""}
      </div>
    </div>
  `
}
