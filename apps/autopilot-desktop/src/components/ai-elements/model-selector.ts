import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator, CommandShortcut } from "../ui/command.js"
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "../ui/dialog.js"
import { cx, type AIChildren } from "./utils.js"

export type ModelSelectorProps = { readonly children?: AIChildren }
export const ModelSelector = ({ children }: ModelSelectorProps): TemplateResult => Dialog({ children })

export type ModelSelectorTriggerProps = { readonly children?: AIChildren }
export const ModelSelectorTrigger = ({ children }: ModelSelectorTriggerProps): TemplateResult => DialogTrigger({ children })

export type ModelSelectorContentProps = {
  readonly className?: string
  readonly title?: AIChildren
  readonly children?: AIChildren
}

export const ModelSelectorContent = ({ className, children, title = "Model Selector" }: ModelSelectorContentProps): TemplateResult =>
  DialogContent({
    className: cx("outline! border-none! p-0 outline-border! outline-solid!", className),
    children: Command({
      className: "**:data-[slot=command-input-wrapper]:h-auto",
      children: html`<div>${DialogTitle({ className: "sr-only", children: title })}</div>${children ?? ""}`,
    }),
  })

export type ModelSelectorDialogProps = { readonly children?: AIChildren }
export const ModelSelectorDialog = ({ children }: ModelSelectorDialogProps): TemplateResult => CommandDialog({ children })

export type ModelSelectorInputProps = { readonly className?: string }
export const ModelSelectorInput = ({ className }: ModelSelectorInputProps): TemplateResult =>
  CommandInput({ className: cx("h-auto py-3.5", className) })

export type ModelSelectorListProps = { readonly children?: AIChildren }
export const ModelSelectorList = ({ children }: ModelSelectorListProps): TemplateResult => CommandList({ children })

export type ModelSelectorEmptyProps = { readonly children?: AIChildren }
export const ModelSelectorEmpty = ({ children }: ModelSelectorEmptyProps): TemplateResult => CommandEmpty({ children })

export type ModelSelectorGroupProps = { readonly children?: AIChildren }
export const ModelSelectorGroup = ({ children }: ModelSelectorGroupProps): TemplateResult => CommandGroup({ children })

export type ModelSelectorItemProps = { readonly children?: AIChildren }
export const ModelSelectorItem = ({ children }: ModelSelectorItemProps): TemplateResult => CommandItem({ children })

export type ModelSelectorShortcutProps = { readonly children?: AIChildren }
export const ModelSelectorShortcut = ({ children }: ModelSelectorShortcutProps): TemplateResult => CommandShortcut({ children })

export type ModelSelectorSeparatorProps = {}
export const ModelSelectorSeparator = (_props: ModelSelectorSeparatorProps): TemplateResult => CommandSeparator({})

export type ModelSelectorLogoProps = {
  readonly provider: string
  readonly className?: string
}

export const ModelSelectorLogo = ({ provider, className }: ModelSelectorLogoProps): TemplateResult =>
  html`<span class="${cx("inline-flex size-4 items-center justify-center text-[10px]", className)}">${provider.slice(0, 2).toUpperCase()}</span>`
