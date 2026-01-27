import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { Command, CommandEmpty, CommandInput, CommandItem, CommandList } from "../ui/command.js"
import { Popover, PopoverContent, PopoverTrigger } from "../ui/popover.js"
import { cx, type AIChildren } from "./utils.js"

export type MicSelectorProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelector = ({ children }: MicSelectorProps): TemplateResult => Popover({ children })

export type MicSelectorTriggerProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorTrigger = ({ className, children }: MicSelectorTriggerProps): TemplateResult =>
  PopoverTrigger({
    children: Button({
      className: cx("w-full justify-between", className),
      variant: "outline",
      size: "sm",
      type: "button",
      children: children ?? html`Microphone <span class="text-muted-foreground">v</span>`,
    }),
  })

export type MicSelectorContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorContent = ({ className, children }: MicSelectorContentProps): TemplateResult =>
  PopoverContent({ className: cx("w-[--radix-popover-trigger-width] p-0", className), children })

export type MicSelectorInputProps = {
  readonly className?: string
}

export const MicSelectorInput = ({ className }: MicSelectorInputProps): TemplateResult =>
  CommandInput({ className: cx("h-auto py-3.5", className) })

export type MicSelectorListProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorList = ({ className, children }: MicSelectorListProps): TemplateResult =>
  CommandList({ className, children })

export type MicSelectorEmptyProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorEmpty = ({ className, children }: MicSelectorEmptyProps): TemplateResult =>
  CommandEmpty({ className, children })

export type MicSelectorItemProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorItem = ({ className, children }: MicSelectorItemProps): TemplateResult =>
  CommandItem({ className, children })

export type MicSelectorLabelProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorLabel = ({ className, children }: MicSelectorLabelProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? "Microphone"}</span>`

export type MicSelectorValueProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MicSelectorValue = ({ className, children }: MicSelectorValueProps): TemplateResult =>
  html`<span class="${cx("truncate", className)}">${children ?? "Default"}</span>`

export const useAudioDevices = () => ({
  devices: [] as MediaDeviceInfo[],
  loading: false,
  hasPermission: false,
  loadDevices: () => undefined,
})
