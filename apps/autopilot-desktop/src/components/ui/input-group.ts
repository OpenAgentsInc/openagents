import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button, type ButtonVariant } from "./button.js"
import { Input } from "./input.js"
import { Textarea } from "./textarea.js"
import { cx, type UIChildren } from "./utils.js"

export type InputGroupProps = {
  readonly className?: string
  readonly dataRole?: string
  readonly dataCopyValue?: string
  readonly children?: UIChildren
}

export const InputGroup = ({
  className,
  dataRole,
  dataCopyValue,
  children,
}: InputGroupProps): TemplateResult => {
  return html`
    <div
      data-slot="input-group"
      data-role="${dataRole ?? ""}"
      data-copy-value="${dataCopyValue ?? ""}"
      role="group"
      class="${cx(
        "group/input-group border-input dark:bg-input/30 relative flex w-full items-center rounded-md border shadow-xs transition-[color,box-shadow] outline-none",
        "h-9 min-w-0 has-[>textarea]:h-auto",
        "has-[>[data-align=inline-start]]:[&>input]:pl-2",
        "has-[>[data-align=inline-end]]:[&>input]:pr-2",
        "has-[>[data-align=block-start]]:h-auto has-[>[data-align=block-start]]:flex-col has-[>[data-align=block-start]]:[&>input]:pb-3",
        "has-[>[data-align=block-end]]:h-auto has-[>[data-align=block-end]]:flex-col has-[>[data-align=block-end]]:[&>input]:pt-3",
        "has-[[data-slot=input-group-control]:focus-visible]:border-ring has-[[data-slot=input-group-control]:focus-visible]:ring-ring/50 has-[[data-slot=input-group-control]:focus-visible]:ring-[3px]",
        "has-[[data-slot][aria-invalid=true]]:ring-destructive/20 has-[[data-slot][aria-invalid=true]]:border-destructive dark:has-[[data-slot][aria-invalid=true]]:ring-destructive/40",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type InputGroupAddonAlign = "inline-start" | "inline-end" | "block-start" | "block-end"

const addonBase =
  "text-muted-foreground flex h-auto cursor-text items-center justify-center gap-2 py-1.5 text-sm font-medium select-none [&>svg:not([class*='size-'])]:size-4 [&>kbd]:rounded-[calc(var(--radius)-5px)] group-data-[disabled=true]/input-group:opacity-50"

const addonAlignClasses: Record<InputGroupAddonAlign, string> = {
  "inline-start":
    "order-first pl-3 has-[>button]:ml-[-0.45rem] has-[>kbd]:ml-[-0.35rem]",
  "inline-end":
    "order-last pr-3 has-[>button]:mr-[-0.45rem] has-[>kbd]:mr-[-0.35rem]",
  "block-start":
    "order-first w-full justify-start px-3 pt-3 [.border-b]:pb-3 group-has-[>input]/input-group:pt-2.5",
  "block-end":
    "order-last w-full justify-start px-3 pb-3 [.border-t]:pt-3 group-has-[>input]/input-group:pb-2.5",
}

export type InputGroupAddonProps = {
  readonly className?: string
  readonly align?: InputGroupAddonAlign
  readonly children?: UIChildren
}

export const InputGroupAddon = ({
  className,
  align = "inline-start",
  children,
}: InputGroupAddonProps): TemplateResult => {
  return html`
    <div
      role="group"
      data-slot="input-group-addon"
      data-align="${align}"
      class="${cx(addonBase, addonAlignClasses[align], className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export type InputGroupButtonSize = "xs" | "sm" | "icon-xs" | "icon-sm"

const buttonSizeClasses: Record<InputGroupButtonSize, string> = {
  xs: "h-6 gap-1 px-2 rounded-[calc(var(--radius)-5px)] [&>svg:not([class*='size-'])]:size-3.5 has-[>svg]:px-2",
  sm: "h-8 px-2.5 gap-1.5 rounded-md has-[>svg]:px-2.5",
  "icon-xs": "size-6 rounded-[calc(var(--radius)-5px)] p-0 has-[>svg]:p-0",
  "icon-sm": "size-8 p-0 has-[>svg]:p-0",
}

export type InputGroupButtonProps = {
  readonly className?: string
  readonly type?: "button" | "submit" | "reset"
  readonly variant?: ButtonVariant
  readonly size?: InputGroupButtonSize
  readonly dataRole?: string
  readonly dataUi?: string
  readonly dataUiStop?: boolean
  readonly dataCopyTarget?: string
  readonly dataCopyValue?: string
  readonly ariaLabel?: string
  readonly title?: string
  readonly children?: UIChildren
}

export const InputGroupButton = ({
  className,
  type = "button",
  variant = "ghost",
  size = "xs",
  dataRole,
  dataUi,
  dataUiStop = false,
  dataCopyTarget,
  dataCopyValue,
  ariaLabel,
  title,
  children,
}: InputGroupButtonProps): TemplateResult => {
  return Button({
    type,
    variant,
    size: "default",
    dataRole,
    dataUi,
    dataUiStop,
    dataCopyTarget,
    dataCopyValue,
    ariaLabel,
    title,
    className: cx("text-sm shadow-none flex gap-2 items-center", buttonSizeClasses[size], className),
    children,
  })
}

export type InputGroupTextProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const InputGroupText = ({ className, children }: InputGroupTextProps): TemplateResult => {
  return html`
    <span
      class="${cx(
        "text-muted-foreground flex items-center gap-2 text-sm [&_svg]:pointer-events-none [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      ${children ?? ""}
    </span>
  `
}

export type InputGroupInputProps = {
  readonly className?: string
  readonly type?: string
  readonly name?: string
  readonly value?: string
  readonly placeholder?: string
  readonly disabled?: boolean
}

export const InputGroupInput = ({
  className,
  type,
  name,
  value,
  placeholder,
  disabled,
}: InputGroupInputProps): TemplateResult => {
  return Input({
    className: cx(
      "flex-1 rounded-none border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent",
      className
    ),
    type,
    name,
    value,
    placeholder,
    disabled,
  })
}

export type InputGroupTextareaProps = {
  readonly className?: string
  readonly name?: string
  readonly value?: string
  readonly placeholder?: string
  readonly disabled?: boolean
}

export const InputGroupTextarea = ({
  className,
  name,
  value,
  placeholder,
  disabled,
}: InputGroupTextareaProps): TemplateResult => {
  return Textarea({
    className: cx(
      "flex-1 resize-none rounded-none border-0 bg-transparent py-3 shadow-none focus-visible:ring-0 dark:bg-transparent",
      className
    ),
    name,
    value,
    placeholder,
    disabled,
  })
}
