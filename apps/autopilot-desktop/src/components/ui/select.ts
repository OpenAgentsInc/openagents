import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type SelectProps = {
  readonly children?: UIChildren
}

export const Select = ({ children }: SelectProps): TemplateResult => {
  return html`<div data-slot="select">${children ?? ""}</div>`
}

export type SelectGroupProps = {
  readonly children?: UIChildren
}

export const SelectGroup = ({ children }: SelectGroupProps): TemplateResult => {
  return html`<div data-slot="select-group">${children ?? ""}</div>`
}

export type SelectValueProps = {
  readonly children?: UIChildren
}

export const SelectValue = ({ children }: SelectValueProps): TemplateResult => {
  return html`<span data-slot="select-value">${children ?? ""}</span>`
}

export type SelectTriggerProps = {
  readonly className?: string
  readonly size?: "sm" | "default"
  readonly children?: UIChildren
  readonly disabled?: boolean
}

export const SelectTrigger = ({
  className,
  size = "default",
  children,
  disabled = false,
}: SelectTriggerProps): TemplateResult => {
  return html`
    <button
      data-slot="select-trigger"
      data-size="${size}"
      class="${cx(
        "border-input data-[placeholder]:text-muted-foreground [&_svg:not([class*='text-'])]:text-muted-foreground focus-visible:border-ring focus-visible:ring-ring/50 aria-invalid:ring-destructive/20 dark:aria-invalid:ring-destructive/40 aria-invalid:border-destructive dark:bg-input/30 dark:hover:bg-input/50 flex w-fit items-center justify-between gap-2 rounded-md border bg-transparent px-3 py-2 text-sm whitespace-nowrap shadow-xs transition-[color,box-shadow] outline-none focus-visible:ring-[3px] disabled:cursor-not-allowed disabled:opacity-50 data-[size=default]:h-9 data-[size=sm]:h-8 *:data-[slot=select-value]:line-clamp-1 *:data-[slot=select-value]:flex *:data-[slot=select-value]:items-center *:data-[slot=select-value]:gap-2 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
      type="button"
      ${disabled ? "disabled" : ""}
    >
      ${children ?? ""}
      <span class="size-4 opacity-50">▾</span>
    </button>
  `
}

export type SelectContentProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly position?: "item-aligned" | "popper"
  readonly align?: "start" | "center" | "end"
  readonly side?: "top" | "bottom" | "left" | "right"
  readonly state?: "open" | "closed"
}

export const SelectContent = ({
  className,
  children,
  position = "item-aligned",
  align = "center",
  side = "bottom",
  state = "open",
}: SelectContentProps): TemplateResult => {
  const popperClass =
    position === "popper"
      ? "data-[side=bottom]:translate-y-1 data-[side=left]:-translate-x-1 data-[side=right]:translate-x-1 data-[side=top]:-translate-y-1"
      : ""
  return html`
    <div data-slot="select-portal">
      <div
        data-slot="select-content"
        data-position="${position}"
        data-align="${align}"
        data-side="${side}"
        data-state="${state}"
        class="${cx(
          "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 relative z-50 max-h-(--radix-select-content-available-height) min-w-[8rem] origin-(--radix-select-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-md border shadow-md",
          popperClass,
          className
        )}"
      >
        ${SelectScrollUpButton({})}
        <div
          data-slot="select-viewport"
          class="${cx(
            "p-1",
            position === "popper"
              ? "h-[var(--radix-select-trigger-height)] w-full min-w-[var(--radix-select-trigger-width)] scroll-my-1"
              : ""
          )}"
        >
          ${children ?? ""}
        </div>
        ${SelectScrollDownButton({})}
      </div>
    </div>
  `
}

export type SelectLabelProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const SelectLabel = ({ className, children }: SelectLabelProps): TemplateResult => {
  return html`
    <div data-slot="select-label" class="${cx("text-muted-foreground px-2 py-1.5 text-xs", className)}">
      ${children ?? ""}
    </div>
  `
}

export type SelectItemProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly selected?: boolean
  readonly disabled?: boolean
}

export const SelectItem = ({
  className,
  children,
  selected = false,
  disabled = false,
}: SelectItemProps): TemplateResult => {
  return html`
    <div
      data-slot="select-item"
      data-disabled="${disabled ? "true" : "false"}"
      class="${cx(
        "focus:bg-accent focus:text-accent-foreground [&_svg:not([class*='text-'])]:text-muted-foreground relative flex w-full cursor-default items-center gap-2 rounded-sm py-1.5 pr-8 pl-2 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4 *:[span]:last:flex *:[span]:last:items-center *:[span]:last:gap-2",
        className
      )}"
    >
      <span data-slot="select-item-indicator" class="absolute right-2 flex size-3.5 items-center justify-center">
        ${selected ? "✓" : ""}
      </span>
      <span data-slot="select-item-text">${children ?? ""}</span>
    </div>
  `
}

export type SelectSeparatorProps = {
  readonly className?: string
}

export const SelectSeparator = ({ className }: SelectSeparatorProps): TemplateResult => {
  return html`
    <div data-slot="select-separator" class="${cx("bg-border pointer-events-none -mx-1 my-1 h-px", className)}"></div>
  `
}

export type SelectScrollButtonProps = {
  readonly className?: string
}

export const SelectScrollUpButton = ({ className }: SelectScrollButtonProps): TemplateResult => {
  return html`
    <div
      data-slot="select-scroll-up-button"
      class="${cx("flex cursor-default items-center justify-center py-1", className)}"
    >
      <span class="size-4">▴</span>
    </div>
  `
}

export const SelectScrollDownButton = ({ className }: SelectScrollButtonProps): TemplateResult => {
  return html`
    <div
      data-slot="select-scroll-down-button"
      class="${cx("flex cursor-default items-center justify-center py-1", className)}"
    >
      <span class="size-4">▾</span>
    </div>
  `
}
