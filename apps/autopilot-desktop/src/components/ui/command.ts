import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Dialog, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./dialog.js"
import { cx, type UIChildren } from "./utils.js"

export type CommandProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const Command = ({ className, children }: CommandProps): TemplateResult => {
  return html`
    <div
      data-slot="command"
      class="${cx(
        "bg-popover text-popover-foreground flex h-full w-full flex-col overflow-hidden rounded-md",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type CommandDialogProps = {
  readonly title?: string
  readonly description?: string
  readonly className?: string
  readonly showCloseButton?: boolean
  readonly children?: UIChildren
}

export const CommandDialog = ({
  title = "Command Palette",
  description = "Search for a command to run...",
  className,
  showCloseButton = true,
  children,
}: CommandDialogProps): TemplateResult => {
  return Dialog({
    children: html`
      ${DialogHeader({
        className: "sr-only",
        children: html`
          ${DialogTitle({ children: title })}
          ${DialogDescription({ children: description })}
        `,
      })}
      ${DialogContent({
        className: cx("overflow-hidden p-0", className),
        showCloseButton,
        children: Command({
          className:
            "[&_[cmdk-group-heading]]:text-muted-foreground **:data-[slot=command-input-wrapper]:h-12 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:font-medium [&_[cmdk-group]]:px-2 [&_[cmdk-group]:not([hidden])_~[cmdk-group]]:pt-0 [&_[cmdk-input-wrapper]_svg]:h-5 [&_[cmdk-input-wrapper]_svg]:w-5 [&_[cmdk-input]]:h-12 [&_[cmdk-item]]:px-2 [&_[cmdk-item]]:py-3 [&_[cmdk-item]_svg]:h-5 [&_[cmdk-item]_svg]:w-5",
          children,
        }),
      })}
    `,
  })
}

export type CommandInputProps = {
  readonly className?: string
  readonly placeholder?: string
  readonly value?: string
}

export const CommandInput = ({
  className,
  placeholder,
  value,
}: CommandInputProps): TemplateResult => {
  return html`
    <div
      data-slot="command-input-wrapper"
      class="flex h-9 items-center gap-2 border-b px-3"
    >
      <span class="size-4 shrink-0 opacity-50">🔍</span>
      <input
        data-slot="command-input"
        class="${cx(
          "placeholder:text-muted-foreground flex h-10 w-full rounded-md bg-transparent py-3 text-sm outline-hidden disabled:cursor-not-allowed disabled:opacity-50",
          className
        )}"
        placeholder="${placeholder ?? ""}"
        value="${value ?? ""}"
      />
    </div>
  `
}

export type CommandListProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CommandList = ({ className, children }: CommandListProps): TemplateResult => {
  return html`
    <div
      data-slot="command-list"
      class="${cx("max-h-[300px] scroll-py-1 overflow-x-hidden overflow-y-auto", className)}"
    >
      ${children ?? ""}
    </div>
  `
}

export type CommandEmptyProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CommandEmpty = ({ className, children }: CommandEmptyProps): TemplateResult => {
  return html`
    <div data-slot="command-empty" class="${cx("py-6 text-center text-sm", className)}">
      ${children ?? ""}
    </div>
  `
}

export type CommandGroupProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CommandGroup = ({ className, children }: CommandGroupProps): TemplateResult => {
  return html`
    <div
      data-slot="command-group"
      class="${cx(
        "text-foreground [&_[cmdk-group-heading]]:text-muted-foreground overflow-hidden p-1 [&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-xs [&_[cmdk-group-heading]]:font-medium",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type CommandSeparatorProps = {
  readonly className?: string
}

export const CommandSeparator = ({ className }: CommandSeparatorProps): TemplateResult => {
  return html`
    <div data-slot="command-separator" class="${cx("bg-border -mx-1 h-px", className)}"></div>
  `
}

export type CommandItemProps = {
  readonly className?: string
  readonly children?: UIChildren
  readonly disabled?: boolean
  readonly selected?: boolean
}

export const CommandItem = ({
  className,
  children,
  disabled = false,
  selected = false,
}: CommandItemProps): TemplateResult => {
  return html`
    <div
      data-slot="command-item"
      data-disabled="${disabled ? "true" : "false"}"
      data-selected="${selected ? "true" : "false"}"
      class="${cx(
        "data-[selected=true]:bg-accent data-[selected=true]:text-accent-foreground [&_svg:not([class*='text-'])]:text-muted-foreground relative flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm outline-hidden select-none data-[disabled=true]:pointer-events-none data-[disabled=true]:opacity-50 [&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type CommandShortcutProps = {
  readonly className?: string
  readonly children?: UIChildren
}

export const CommandShortcut = ({
  className,
  children,
}: CommandShortcutProps): TemplateResult => {
  return html`
    <span data-slot="command-shortcut" class="${cx("text-muted-foreground ml-auto text-xs tracking-widest", className)}">
      ${children ?? ""}
    </span>
  `
}
