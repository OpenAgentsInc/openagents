import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button, type ButtonVariant } from "./button.js"
import { Separator } from "./separator.js"
import { cx, type UIChildren } from "./utils.js"

export type ButtonGroupOrientation = "horizontal" | "vertical"

export type ButtonGroupProps = {
  readonly className?: string
  readonly orientation?: ButtonGroupOrientation
  readonly children?: UIChildren
}

export type ButtonGroupTextProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export type ButtonGroupSeparatorProps = {
  readonly className?: string
  readonly orientation?: "horizontal" | "vertical"
}

export type ButtonGroupButtonSize = "xs" | "sm" | "icon-xs" | "icon-sm"

export type ButtonGroupButtonProps = {
  readonly className?: string
  readonly type?: "button" | "submit" | "reset"
  readonly variant?: ButtonVariant
  readonly size?: ButtonGroupButtonSize
  readonly children?: UIChildren
}

const groupBase =
  "flex w-fit items-stretch [&>*]:focus-visible:z-10 [&>*]:focus-visible:relative [&>[data-slot=select-trigger]:not([class*='w-'])]:w-fit [&>input]:flex-1 has-[select[aria-hidden=true]:last-child]:[&>[data-slot=select-trigger]:last-of-type]:rounded-r-md has-[>[data-slot=button-group]]:gap-2"

const orientationClasses: Record<ButtonGroupOrientation, string> = {
  horizontal:
    "[&>*:not(:first-child)]:rounded-l-none [&>*:not(:first-child)]:border-l-0 [&>*:not(:last-child)]:rounded-r-none",
  vertical:
    "flex-col [&>*:not(:first-child)]:rounded-t-none [&>*:not(:first-child)]:border-t-0 [&>*:not(:last-child)]:rounded-b-none",
}

const addonClasses =
  "bg-muted flex items-center gap-2 rounded-md border px-4 text-sm font-medium shadow-xs [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4"

const buttonSizeClasses: Record<ButtonGroupButtonSize, string> = {
  xs: "h-6 gap-1 px-2 rounded-[calc(var(--radius)-5px)] [&>svg:not([class*='size-'])]:size-3.5 has-[>svg]:px-2",
  sm: "h-8 px-2.5 gap-1.5 rounded-md has-[>svg]:px-2.5",
  "icon-xs": "size-6 rounded-[calc(var(--radius)-5px)] p-0 has-[>svg]:p-0",
  "icon-sm": "size-8 p-0 has-[>svg]:p-0",
}

export const ButtonGroup = ({
  className,
  orientation = "horizontal",
  children,
}: ButtonGroupProps): TemplateResult => {
  return html`
    <div
      role="group"
      data-slot="button-group"
      data-orientation="${orientation}"
      class="${cx(groupBase, orientationClasses[orientation], className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export const ButtonGroupText = ({
  className,
  children,
}: ButtonGroupTextProps): TemplateResult => {
  return html`
    <div class="${cx(addonClasses, className)}">${children ?? ""}</div>
  `
}

export const ButtonGroupSeparator = ({
  className,
  orientation = "vertical",
}: ButtonGroupSeparatorProps): TemplateResult => {
  return Separator({
    className: cx(
      "bg-input relative !m-0 self-stretch data-[orientation=vertical]:h-auto",
      className
    ),
    orientation,
  })
}

export const ButtonGroupButton = ({
  className,
  type = "button",
  variant = "ghost",
  size = "xs",
  children,
}: ButtonGroupButtonProps): TemplateResult => {
  return Button({
    type,
    variant,
    size: "default",
    className: cx(buttonSizeClasses[size], className),
    children,
  })
}
