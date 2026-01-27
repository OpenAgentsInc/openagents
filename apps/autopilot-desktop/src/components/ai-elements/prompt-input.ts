import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import {
  InputGroup,
  InputGroupAddon,
  InputGroupButton,
  InputGroupTextarea,
} from "../ui/input-group.js"
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "../ui/dropdown-menu.js"
import { HoverCard, HoverCardContent, HoverCardTrigger } from "../ui/hover-card.js"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../ui/select.js"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "../ui/tabs.js"
import { Command, CommandEmpty, CommandGroup, CommandInput, CommandItem, CommandList, CommandSeparator } from "../ui/command.js"
import { Button } from "../ui/button.js"
import { cx, type AIChildren } from "./utils.js"

export interface PromptInputMessage {
  text: string
  files: unknown[]
}

export const usePromptInputController = () => null
export const useProviderAttachments = () => ({ files: [], addFiles: () => undefined })
export const usePromptInputAttachments = () => ({ files: [], addFiles: () => undefined })
export const LocalReferencedSourcesContext = {}
export const usePromptInputReferencedSources = () => ({ sources: [] })

export type PromptInputActionAddAttachmentsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInputActionAddAttachments = ({ className, children }: PromptInputActionAddAttachmentsProps): TemplateResult =>
  Button({ className, size: "icon-sm", type: "button", variant: "ghost", children: children ?? "+" })

export type PromptInputProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInput = ({ className, children }: PromptInputProps): TemplateResult => html`
  <form class="${cx("w-full", className)}">
    ${InputGroup({ className: "overflow-hidden", children })}
  </form>
`

export type PromptInputBodyProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInputBody = ({ className, children }: PromptInputBodyProps): TemplateResult =>
  html`<div class="${cx("contents", className)}">${children ?? ""}</div>`

export type PromptInputTextareaProps = {
  readonly className?: string
  readonly placeholder?: string
}

export const PromptInputTextarea = ({ className, placeholder = "What would you like to know?" }: PromptInputTextareaProps): TemplateResult =>
  InputGroupTextarea({ className, placeholder })

export type PromptInputHeaderProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInputHeader = ({ className, children }: PromptInputHeaderProps): TemplateResult =>
  InputGroupAddon({ align: "block-end", className: cx("order-first flex-wrap gap-1", className), children })

export type PromptInputFooterProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInputFooter = ({ className, children }: PromptInputFooterProps): TemplateResult =>
  InputGroupAddon({ align: "block-end", className: cx("justify-between gap-1", className), children })

export type PromptInputToolsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const PromptInputTools = ({ className, children }: PromptInputToolsProps): TemplateResult =>
  html`<div class="${cx("flex items-center gap-1", className)}">${children ?? ""}</div>`

export type PromptInputButtonProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly variant?: "ghost" | "outline" | "default" | "secondary" | "destructive" | "link"
  readonly size?: "xs" | "sm" | "icon-xs" | "icon-sm"
}

export const PromptInputButton = ({ className, children, variant = "ghost", size = "icon-sm" }: PromptInputButtonProps): TemplateResult =>
  InputGroupButton({ className, size, type: "button", variant, children })

export type PromptInputActionMenuProps = { readonly children?: AIChildren }
export const PromptInputActionMenu = ({ children }: PromptInputActionMenuProps): TemplateResult => DropdownMenu({ children })

export type PromptInputActionMenuTriggerProps = PromptInputButtonProps
export const PromptInputActionMenuTrigger = ({ className, children }: PromptInputActionMenuTriggerProps): TemplateResult =>
  DropdownMenuTrigger({ children: PromptInputButton({ className, children: children ?? "+" }) })

export type PromptInputActionMenuContentProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputActionMenuContent = ({ className, children }: PromptInputActionMenuContentProps): TemplateResult =>
  DropdownMenuContent({ className, children })

export type PromptInputActionMenuItemProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputActionMenuItem = ({ className, children }: PromptInputActionMenuItemProps): TemplateResult =>
  DropdownMenuItem({ className, children })

export type PromptInputSubmitProps = {
  readonly className?: string
  readonly status?: string
  readonly children?: AIChildren
}

export const PromptInputSubmit = ({ className, status, children }: PromptInputSubmitProps): TemplateResult => {
  const label = status === "streaming" ? "Stop" : "Submit"
  return InputGroupButton({ className, size: "icon-sm", type: "submit", variant: "default", children: children ?? label })
}

export type PromptInputSelectProps = { readonly children?: AIChildren }
export const PromptInputSelect = ({ children }: PromptInputSelectProps): TemplateResult => Select({ children })

export type PromptInputSelectTriggerProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputSelectTrigger = ({ className, children }: PromptInputSelectTriggerProps): TemplateResult =>
  SelectTrigger({ className: cx("h-7 border-none bg-transparent px-2 text-xs shadow-none", className), size: "sm", children })

export type PromptInputSelectContentProps = { readonly children?: AIChildren }
export const PromptInputSelectContent = ({ children }: PromptInputSelectContentProps): TemplateResult => SelectContent({ align: "end", children })

export type PromptInputSelectItemProps = { readonly children?: AIChildren; readonly value?: string }
export const PromptInputSelectItem = ({ children, value }: PromptInputSelectItemProps): TemplateResult => SelectItem({ value, children })

export type PromptInputSelectValueProps = { readonly children?: AIChildren }
export const PromptInputSelectValue = ({ children }: PromptInputSelectValueProps): TemplateResult => SelectValue({ children })

export type PromptInputHoverCardProps = { readonly children?: AIChildren }
export const PromptInputHoverCard = ({ children }: PromptInputHoverCardProps): TemplateResult => HoverCard({ children })

export type PromptInputHoverCardTriggerProps = { readonly children?: AIChildren }
export const PromptInputHoverCardTrigger = ({ children }: PromptInputHoverCardTriggerProps): TemplateResult => HoverCardTrigger({ children })

export type PromptInputHoverCardContentProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputHoverCardContent = ({ className, children }: PromptInputHoverCardContentProps): TemplateResult =>
  HoverCardContent({ className, children })

export type PromptInputTabsListProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputTabsList = ({ className, children }: PromptInputTabsListProps): TemplateResult =>
  TabsList({ className: cx("h-auto rounded-none border-0 bg-transparent p-0", className), children })

export type PromptInputTabProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputTab = ({ className, children }: PromptInputTabProps): TemplateResult =>
  Tabs({ className: cx("w-full", className), children })

export type PromptInputTabLabelProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputTabLabel = ({ className, children }: PromptInputTabLabelProps): TemplateResult =>
  html`<span class="${cx("text-xs text-muted-foreground", className)}">${children ?? ""}</span>`

export type PromptInputTabBodyProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputTabBody = ({ className, children }: PromptInputTabBodyProps): TemplateResult =>
  TabsContent({ className: cx("mt-2", className), children })

export type PromptInputTabItemProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputTabItem = ({ className, children }: PromptInputTabItemProps): TemplateResult =>
  TabsTrigger({ className: cx("rounded-none border-0 border-transparent border-b-2 px-4 py-2 font-medium text-muted-foreground text-sm transition-colors data-[state=active]:border-primary data-[state=active]:bg-transparent data-[state=active]:text-foreground data-[state=active]:shadow-none", className), children })

export type PromptInputCommandProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputCommand = ({ className, children }: PromptInputCommandProps): TemplateResult =>
  Command({ className, children })

export type PromptInputCommandInputProps = { readonly className?: string }
export const PromptInputCommandInput = ({ className }: PromptInputCommandInputProps): TemplateResult =>
  CommandInput({ className })

export type PromptInputCommandListProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputCommandList = ({ className, children }: PromptInputCommandListProps): TemplateResult =>
  CommandList({ className, children })

export type PromptInputCommandEmptyProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputCommandEmpty = ({ className, children }: PromptInputCommandEmptyProps): TemplateResult =>
  CommandEmpty({ className, children })

export type PromptInputCommandGroupProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputCommandGroup = ({ className, children }: PromptInputCommandGroupProps): TemplateResult =>
  CommandGroup({ className, children })

export type PromptInputCommandItemProps = { readonly className?: string; readonly children?: AIChildren }
export const PromptInputCommandItem = ({ className, children }: PromptInputCommandItemProps): TemplateResult =>
  CommandItem({ className, children })

export type PromptInputCommandSeparatorProps = {}
export const PromptInputCommandSeparator = (_props: PromptInputCommandSeparatorProps): TemplateResult => CommandSeparator({})
