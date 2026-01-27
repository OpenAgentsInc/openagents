import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "../ui/dropdown-menu.js"
import { cx, type AIChildren } from "./utils.js"

export type OpenInProps = {
  readonly query: string
  readonly children?: AIChildren
}

export const OpenIn = ({ children }: OpenInProps): TemplateResult => DropdownMenu({ children })

export type OpenInContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const OpenInContent = ({ className, children }: OpenInContentProps): TemplateResult =>
  DropdownMenuContent({ className: cx("w-56", className), children })

export type OpenInItemProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const OpenInItem = ({ className, children }: OpenInItemProps): TemplateResult =>
  DropdownMenuItem({ className, children })

export type OpenInLabelProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const OpenInLabel = ({ className, children }: OpenInLabelProps): TemplateResult =>
  DropdownMenuLabel({ className, children })

export type OpenInSeparatorProps = {
  readonly className?: string
}

export const OpenInSeparator = ({ className }: OpenInSeparatorProps): TemplateResult =>
  DropdownMenuSeparator({ className })

export type OpenInTriggerProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const OpenInTrigger = ({ className, children }: OpenInTriggerProps): TemplateResult =>
  DropdownMenuTrigger({
    children: Button({
      className: cx("h-8 gap-1", className),
      size: "sm",
      type: "button",
      variant: "outline",
      children: children ?? html`Open in <span class="text-xs">v</span>`,
    }),
  })

const renderOpenIn = (label: string, children?: AIChildren): TemplateResult =>
  OpenInItem({ children: html`<span class="flex items-center gap-2">${children ?? ""}${label}</span>` })

export type OpenInChatGPTProps = { readonly query: string }
export const OpenInChatGPT = ({ query }: OpenInChatGPTProps): TemplateResult => renderOpenIn("ChatGPT", "open")

export type OpenInClaudeProps = { readonly query: string }
export const OpenInClaude = ({ query }: OpenInClaudeProps): TemplateResult => renderOpenIn("Claude", "claude")

export type OpenInT3Props = { readonly query: string }
export const OpenInT3 = ({ query }: OpenInT3Props): TemplateResult => renderOpenIn("T3", "chat")

export type OpenInSciraProps = { readonly query: string }
export const OpenInScira = ({ query }: OpenInSciraProps): TemplateResult => renderOpenIn("Scira", "*")

export type OpenInv0Props = { readonly query: string }
export const OpenInv0 = ({ query }: OpenInv0Props): TemplateResult => renderOpenIn("v0", "v0")

export type OpenInCursorProps = { readonly query: string }
export const OpenInCursor = ({ query }: OpenInCursorProps): TemplateResult => renderOpenIn("Cursor", "^")
