import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "./button.js"
import { cx, type UIChildren } from "./utils.js"

export type DialogProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly state?: "open" | "closed"
}

export const Dialog = ({ className, children, state = "open" }: DialogProps): TemplateResult => {
  return html`
    <div data-slot="dialog" data-state="${state}" class="${cx(className)}">
      ${children ?? ""}
    </div>
  `
}

export type DialogTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DialogTrigger = ({ className, children }: DialogTriggerProps): TemplateResult => {
  return html`
    <button data-slot="dialog-trigger" class="${cx(className)}" type="button">
      ${children ?? ""}
    </button>
  `
}

export const DialogPortal = ({ children }: { children?: UIChildren }): TemplateResult => {
  return html`<div data-slot="dialog-portal">${children ?? ""}</div>`
}

export const DialogClose = ({ className, children }: DialogTriggerProps): TemplateResult => {
  return html`
    <button data-slot="dialog-close" class="${cx(className)}" type="button">
      ${children ?? ""}
    </button>
  `
}

export type DialogOverlayProps = {
  readonly className?: string
  readonly state?: "open" | "closed"
}

export const DialogOverlay = ({
  className,
  state = "open",
}: DialogOverlayProps): TemplateResult => {
  return html`
    <div
      data-slot="dialog-overlay"
      data-state="${state}"
      class="${cx(
        "data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 fixed inset-0 z-50 bg-black/50",
        className
      )}"
    ></div>
  `
}

export type DialogContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly showCloseButton?: boolean
  readonly state?: "open" | "closed"
}

export const DialogContent = ({
  className,
  children,
  showCloseButton = true,
  state = "open",
}: DialogContentProps): TemplateResult => {
  return DialogPortal({
    children: html`
      ${DialogOverlay({ state })}
      <div
        data-slot="dialog-content"
        data-state="${state}"
        class="${cx(
          "bg-background data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 fixed top-[50%] left-[50%] z-50 grid w-full max-w-[calc(100%-2rem)] translate-x-[-50%] translate-y-[-50%] gap-4 rounded-lg border p-6 shadow-lg duration-200 outline-none sm:max-w-lg",
          className
        )}"
      >
        ${children ?? ""}
        ${
          showCloseButton
            ? html`
                <button
                  data-slot="dialog-close"
                  class="ring-offset-background focus:ring-ring data-[state=open]:bg-accent data-[state=open]:text-muted-foreground absolute top-4 right-4 rounded-xs opacity-70 transition-opacity hover:opacity-100 focus:ring-2 focus:ring-offset-2 focus:outline-hidden disabled:pointer-events-none [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4"
                  type="button"
                >
                  <span aria-hidden="true">×</span>
                  <span class="sr-only">Close</span>
                </button>
              `
            : ""
        }
      </div>
    `,
  })
}

export type DialogHeaderProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DialogHeader = ({ className, children }: DialogHeaderProps): TemplateResult => {
  return html`
    <div data-slot="dialog-header" class="${cx("flex flex-col gap-2 text-center sm:text-left", className)}">
      ${children ?? ""}
    </div>
  `
}

export type DialogFooterProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly showCloseButton?: boolean
}

export const DialogFooter = ({
  className,
  children,
  showCloseButton = false,
}: DialogFooterProps): TemplateResult => {
  return html`
    <div data-slot="dialog-footer" class="${cx("flex flex-col-reverse gap-2 sm:flex-row sm:justify-end", className)}">
      ${children ?? ""}
      ${showCloseButton ? Button({ variant: "outline", children: "Close" }) : ""}
    </div>
  `
}

export type DialogTitleProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DialogTitle = ({ className, children }: DialogTitleProps): TemplateResult => {
  return html`
    <div data-slot="dialog-title" class="${cx("text-lg leading-none font-semibold", className)}">
      ${children ?? ""}
    </div>
  `
}

export type DialogDescriptionProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DialogDescription = ({
  className,
  children,
}: DialogDescriptionProps): TemplateResult => {
  return html`
    <div data-slot="dialog-description" class="${cx("text-muted-foreground text-sm", className)}">
      ${children ?? ""}
    </div>
  `
}
