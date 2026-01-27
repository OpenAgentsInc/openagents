import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type PopoverProps = {
  readonly children?: UIChildren
}

export const Popover = ({ children }: PopoverProps): TemplateResult => {
  return html`<div data-slot="popover">${children ?? ""}</div>`
}

export type PopoverTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const PopoverTrigger = ({
  className,
  children,
}: PopoverTriggerProps): TemplateResult => {
  return html`
    <button data-slot="popover-trigger" class="${cx(className)}" type="button">
      ${children ?? ""}
    </button>
  `
}

export type PopoverContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly align?: "start" | "center" | "end"
  readonly side?: "top" | "bottom" | "left" | "right"
  readonly state?: "open" | "closed"
}

export const PopoverContent = ({
  className,
  children,
  align = "center",
  side = "bottom",
  state = "open",
}: PopoverContentProps): TemplateResult => {
  return html`
    <div data-slot="popover-portal">
      <div
        data-slot="popover-content"
        data-align="${align}"
        data-side="${side}"
        data-state="${state}"
        class="${cx(
          "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-72 origin-(--radix-popover-content-transform-origin) rounded-md border p-4 shadow-md outline-hidden",
          className
        )}"
      >
        ${children ?? ""}
      </div>
    </div>
  `
}

export type PopoverAnchorProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const PopoverAnchor = ({ className, children }: PopoverAnchorProps): TemplateResult => {
  return html`<div data-slot="popover-anchor" class="${cx(className)}">${children ?? ""}</div>`
}

export type PopoverHeaderProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const PopoverHeader = ({ className, children }: PopoverHeaderProps): TemplateResult => {
  return html`
    <div data-slot="popover-header" class="${cx("flex flex-col gap-1 text-sm", className)}">
      ${children ?? ""}
    </div>
  `
}

export type PopoverTitleProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const PopoverTitle = ({ className, children }: PopoverTitleProps): TemplateResult => {
  return html`
    <div data-slot="popover-title" class="${cx("font-medium", className)}">
      ${children ?? ""}
    </div>
  `
}

export type PopoverDescriptionProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const PopoverDescription = ({
  className,
  children,
}: PopoverDescriptionProps): TemplateResult => {
  return html`
    <p data-slot="popover-description" class="${cx("text-muted-foreground", className)}">
      ${children ?? ""}
    </p>
  `
}
