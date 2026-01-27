import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { cx, type UIChildren } from "./utils.js"

export type DropdownMenuProps = { readonly children?: UIChildren }

export const DropdownMenu = ({ children }: DropdownMenuProps): TemplateResult => {
  return html`<div data-slot="dropdown-menu">${children ?? ""}</div>`
}

export const DropdownMenuPortal = ({ children }: DropdownMenuProps): TemplateResult => {
  return html`<div data-slot="dropdown-menu-portal">${children ?? ""}</div>`
}

export type DropdownMenuTriggerProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DropdownMenuTrigger = ({
  className,
  children,
}: DropdownMenuTriggerProps): TemplateResult => {
  return html`
    <button data-slot="dropdown-menu-trigger" class="${cx(className)}" type="button">
      ${children ?? ""}
    </button>
  `
}

export type DropdownMenuContentProps = {
  readonly className?: string
  readonly sideOffset?: number
  readonly side?: "top" | "bottom" | "left" | "right"
  readonly state?: "open" | "closed"
  readonly children?: UIChildren
}

export const DropdownMenuContent = ({
  className,
  sideOffset = 4,
  side = "bottom",
  state = "open",
  children,
}: DropdownMenuContentProps): TemplateResult => {
  return DropdownMenuPortal({
    children: html`
      <div
        data-slot="dropdown-menu-content"
        data-side="${side}"
        data-state="${state}"
        data-side-offset="${sideOffset}"
        class="${cx(
          "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 max-h-(--radix-dropdown-menu-content-available-height) min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-x-hidden overflow-y-auto rounded-md border p-1 shadow-md",
          className
        )}"
      >
        ${children ?? ""}
      </div>
    `,
  })
}

export const DropdownMenuGroup = ({ children }: DropdownMenuProps): TemplateResult => {
  return html`<div data-slot="dropdown-menu-group">${children ?? ""}</div>`
}

export type DropdownMenuItemProps = {
  readonly className?: string
  readonly inset?: boolean
  readonly variant?: "default" | "destructive"
  readonly disabled?: boolean
  readonly children?: UIChildren
}

export const DropdownMenuItem = ({
  className,
  inset = false,
  variant = "default",
  disabled = false,
  children,
}: DropdownMenuItemProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-item"
      data-inset="${inset ? "true" : "false"}"
      data-variant="${variant}"
      data-disabled="${disabled ? "true" : "false"}"
      class="${cx(
        "focus:bg-accent focus:text-accent-foreground data-[variant=destructive]:text-destructive data-[variant=destructive]:focus:bg-destructive/10 dark:data-[variant=destructive]:focus:bg-destructive/20 data-[variant=destructive]:focus:text-destructive data-[variant=destructive]:*:[svg]:!text-destructive [&_svg:not([class*='text-'])]:text-muted-foreground relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 data-[inset]:pl-8 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type DropdownMenuCheckboxItemProps = {
  readonly className?: string
  readonly checked?: boolean
  readonly children?: UIChildren
}

export const DropdownMenuCheckboxItem = ({
  className,
  checked = false,
  children,
}: DropdownMenuCheckboxItemProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-checkbox-item"
      class="${cx(
        "focus:bg-accent focus:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-sm py-1.5 pr-2 pl-8 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      <span class="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        ${checked ? "✓" : ""}
      </span>
      ${children ?? ""}
    </div>
  `
}

export const DropdownMenuRadioGroup = ({ children }: DropdownMenuProps): TemplateResult => {
  return html`<div data-slot="dropdown-menu-radio-group">${children ?? ""}</div>`
}

export type DropdownMenuRadioItemProps = {
  readonly className?: string
  readonly checked?: boolean
  readonly children?: UIChildren
}

export const DropdownMenuRadioItem = ({
  className,
  checked = false,
  children,
}: DropdownMenuRadioItemProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-radio-item"
      class="${cx(
        "focus:bg-accent focus:text-accent-foreground relative flex cursor-default items-center gap-2 rounded-sm py-1.5 pr-2 pl-8 text-sm outline-hidden select-none data-[disabled]:pointer-events-none data-[disabled]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      <span class="pointer-events-none absolute left-2 flex size-3.5 items-center justify-center">
        ${checked ? "●" : ""}
      </span>
      ${children ?? ""}
    </div>
  `
}

export type DropdownMenuLabelProps = {
  readonly className?: string
  readonly inset?: boolean
  readonly children?: UIChildren
}

export const DropdownMenuLabel = ({
  className,
  inset = false,
  children,
}: DropdownMenuLabelProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-label"
      data-inset="${inset ? "true" : "false"}"
      class="${cx("px-2 py-1.5 text-sm font-medium data-[inset]:pl-8", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export type DropdownMenuSeparatorProps = {
  readonly className?: string
}

export const DropdownMenuSeparator = ({ className }: DropdownMenuSeparatorProps): TemplateResult => {
  return html`
    <div data-slot="dropdown-menu-separator" class="${cx("bg-border -mx-1 my-1 h-px", className)}"></div>
  `
}

export type DropdownMenuShortcutProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DropdownMenuShortcut = ({
  className,
  children,
}: DropdownMenuShortcutProps): TemplateResult => {
  return html`
    <span data-slot="dropdown-menu-shortcut" class="${cx("text-muted-foreground ml-auto text-xs tracking-widest", className)}">
      ${children ?? ""}
    </span>
  `
}

export const DropdownMenuSub = ({ children }: DropdownMenuProps): TemplateResult => {
  return html`<div data-slot="dropdown-menu-sub">${children ?? ""}</div>`
}

export type DropdownMenuSubTriggerProps = {
  readonly className?: string
  readonly inset?: boolean
  readonly children?: UIChildren
}

export const DropdownMenuSubTrigger = ({
  className,
  inset = false,
  children,
}: DropdownMenuSubTriggerProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-sub-trigger"
      data-inset="${inset ? "true" : "false"}"
      class="${cx(
        "focus:bg-accent focus:text-accent-foreground data-[state=open]:bg-accent data-[state=open]:text-accent-foreground [&_svg:not([class*='text-'])]:text-muted-foreground flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[inset]:pl-8 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      ${children ?? ""}
      <span class="ml-auto size-4">›</span>
    </div>
  `
}

export type DropdownMenuSubContentProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const DropdownMenuSubContent = ({
  className,
  children,
}: DropdownMenuSubContentProps): TemplateResult => {
  return html`
    <div
      data-slot="dropdown-menu-sub-content"
      class="${cx(
        "bg-popover text-popover-foreground data-[state=open]:animate-in data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=open]:fade-in-0 data-[state=closed]:zoom-out-95 data-[state=open]:zoom-in-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 min-w-[8rem] origin-(--radix-dropdown-menu-content-transform-origin) overflow-hidden rounded-md border p-1 shadow-lg",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}
