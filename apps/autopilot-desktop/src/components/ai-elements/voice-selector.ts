import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Command, CommandDialog, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator, CommandShortcut } from "../ui/command.js"
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "../ui/dialog.js"
import { cx, type AIChildren } from "./utils.js"

export const useVoiceSelector = () => ({ voices: [], loading: false })

export type VoiceSelectorProps = { readonly children?: AIChildren }
export const VoiceSelector = ({ children }: VoiceSelectorProps): TemplateResult => Dialog({ children })

export type VoiceSelectorTriggerProps = { readonly children?: AIChildren }
export const VoiceSelectorTrigger = ({ children }: VoiceSelectorTriggerProps): TemplateResult => DialogTrigger({ children })

export type VoiceSelectorContentProps = { readonly className?: string; readonly title?: AIChildren; readonly children?: AIChildren }
export const VoiceSelectorContent = ({ className, title = "Voice Selector", children }: VoiceSelectorContentProps): TemplateResult =>
  DialogContent({
    className: cx("outline! border-none! p-0 outline-border! outline-solid!", className),
    children: html`${DialogTitle({ className: "sr-only", children: title })}${Command({ children })}`,
  })

export type VoiceSelectorDialogProps = { readonly children?: AIChildren }
export const VoiceSelectorDialog = ({ children }: VoiceSelectorDialogProps): TemplateResult => CommandDialog({ children })

export type VoiceSelectorInputProps = { readonly className?: string }
export const VoiceSelectorInput = ({ className }: VoiceSelectorInputProps): TemplateResult =>
  CommandInput({ className: cx("h-auto py-3.5", className) })

export type VoiceSelectorListProps = { readonly children?: AIChildren }
export const VoiceSelectorList = ({ children }: VoiceSelectorListProps): TemplateResult => CommandList({ children })

export type VoiceSelectorEmptyProps = { readonly children?: AIChildren }
export const VoiceSelectorEmpty = ({ children }: VoiceSelectorEmptyProps): TemplateResult => CommandEmpty({ children })

export type VoiceSelectorGroupProps = { readonly children?: AIChildren }
export const VoiceSelectorGroup = ({ children }: VoiceSelectorGroupProps): TemplateResult => CommandGroup({ children })

export type VoiceSelectorItemProps = { readonly children?: AIChildren }
export const VoiceSelectorItem = ({ children }: VoiceSelectorItemProps): TemplateResult => CommandItem({ children })

export type VoiceSelectorShortcutProps = { readonly children?: AIChildren }
export const VoiceSelectorShortcut = ({ children }: VoiceSelectorShortcutProps): TemplateResult => CommandShortcut({ children })

export type VoiceSelectorSeparatorProps = {}
export const VoiceSelectorSeparator = (_props: VoiceSelectorSeparatorProps): TemplateResult => CommandSeparator({})

export type VoiceSelectorGenderProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorGender = ({ className, children }: VoiceSelectorGenderProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Gender"}</span>`

export type VoiceSelectorAccentProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorAccent = ({ className, children }: VoiceSelectorAccentProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Accent"}</span>`

export type VoiceSelectorAgeProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorAge = ({ className, children }: VoiceSelectorAgeProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Age"}</span>`

export type VoiceSelectorNameProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorName = ({ className, children }: VoiceSelectorNameProps): TemplateResult =>
  html`<span class="${cx("font-medium", className)}">${children ?? "Voice"}</span>`

export type VoiceSelectorDescriptionProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorDescription = ({ className, children }: VoiceSelectorDescriptionProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? ""}</span>`

export type VoiceSelectorAttributesProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorAttributes = ({ className, children }: VoiceSelectorAttributesProps): TemplateResult =>
  html`<div class="${cx("flex flex-wrap gap-2", className)}">${children ?? ""}</div>`

export type VoiceSelectorBulletProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorBullet = ({ className, children }: VoiceSelectorBulletProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "*"}</span>`

export type VoiceSelectorPreviewProps = { readonly className?: string; readonly children?: AIChildren }
export const VoiceSelectorPreview = ({ className, children }: VoiceSelectorPreviewProps): TemplateResult =>
  html`<div class="${cx("rounded-md border bg-muted/40 p-2 text-xs", className)}">${children ?? "Preview"}</div>`
