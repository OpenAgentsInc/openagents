import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { ButtonGroup, ButtonGroupText } from "../ui/button-group.js"
import { Tooltip, TooltipContent, TooltipProvider, TooltipTrigger } from "../ui/tooltip.js"
import { cx, type AIChildren } from "./utils.js"

export type MessageProps = {
  readonly className?: string
  readonly from: "user" | "assistant" | "system"
  readonly children?: AIChildren
}

export const Message = ({ className, from, children }: MessageProps): TemplateResult => {
  const roleClass = from === "user" ? "is-user ml-auto justify-end" : "is-assistant"
  return html`
    <div class="${cx("group flex w-full max-w-[95%] flex-col gap-2", roleClass, className)}">
      ${children ?? ""}
    </div>
  `
}

export type MessageContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageContent = ({
  className,
  children,
}: MessageContentProps): TemplateResult => {
  return html`
    <div
      class="${cx(
        "is-user:dark flex w-fit min-w-0 max-w-full flex-col gap-2 overflow-hidden",
        "group-[.is-user]:ml-auto group-[.is-user]:rounded-lg group-[.is-user]:bg-[#1a1a1a] group-[.is-user]:border group-[.is-user]:border-[#333] group-[.is-user]:px-4 group-[.is-user]:py-3 group-[.is-user]:text-foreground",
        "group-[.is-assistant]:text-foreground",
        className
      )}"
    >
      ${children ?? ""}
    </div>
  `
}

export type MessageActionsProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageActions = ({ className, children }: MessageActionsProps): TemplateResult => {
  return html`<div class="${cx("flex items-center gap-1", className)}">${children ?? ""}</div>`
}

export type MessageActionProps = {
  readonly className?: string
  readonly tooltip?: string
  readonly label?: string
  readonly variant?: "ghost" | "default" | "secondary" | "outline" | "destructive" | "link"
  readonly size?: "icon-sm" | "icon" | "icon-xs" | "default" | "sm" | "lg"
  readonly children?: AIChildren
}

export const MessageAction = ({
  className,
  tooltip,
  label,
  variant = "ghost",
  size = "icon-sm",
  children,
}: MessageActionProps): TemplateResult => {
  const button = Button({
    className,
    size: size as any,
    type: "button",
    variant: variant as any,
    children: html`${children ?? ""}<span class="sr-only">${label || tooltip || ""}</span>`,
  })

  if (!tooltip) {
    return button
  }

  return TooltipProvider({
    children: Tooltip({
      children: html`
        ${TooltipTrigger({ children: button })}
        ${TooltipContent({ children: html`<p>${tooltip}</p>` })}
      `,
    }),
  })
}

export type MessageBranchProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageBranch = ({ className, children }: MessageBranchProps): TemplateResult => {
  return html`
    <div class="${cx("grid w-full gap-2 [&>div]:pb-0", className)}">
      ${children ?? ""}
    </div>
  `
}

export type MessageBranchContentProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly active?: boolean
}

export const MessageBranchContent = ({
  className,
  children,
  active = true,
}: MessageBranchContentProps): TemplateResult => {
  return html`
    <div class="${cx("grid gap-2 overflow-hidden [&>div]:pb-0", active ? "block" : "hidden", className)}">
      ${children ?? ""}
    </div>
  `
}

export type MessageBranchSelectorProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly from: "user" | "assistant" | "system"
}

export const MessageBranchSelector = ({
  className,
  children,
}: MessageBranchSelectorProps): TemplateResult => {
  return ButtonGroup({
    className: cx(
      "[&>*:not(:first-child)]:rounded-l-md [&>*:not(:last-child)]:rounded-r-md",
      className
    ),
    orientation: "horizontal",
    children,
  })
}

export type MessageBranchPreviousProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly disabled?: boolean
}

export const MessageBranchPrevious = ({
  className,
  children,
  disabled = false,
}: MessageBranchPreviousProps): TemplateResult => {
  return Button({
    className,
    size: "icon-sm",
    type: "button",
    variant: "ghost",
    disabled,
    children: children ?? "<",
  })
}

export type MessageBranchNextProps = {
  readonly className?: string
  readonly children?: AIChildren
  readonly disabled?: boolean
}

export const MessageBranchNext = ({
  className,
  children,
  disabled = false,
}: MessageBranchNextProps): TemplateResult => {
  return Button({
    className,
    size: "icon-sm",
    type: "button",
    variant: "ghost",
    disabled,
    children: children ?? ">",
  })
}

export type MessageBranchPageProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageBranchPage = ({ className, children }: MessageBranchPageProps): TemplateResult => {
  return ButtonGroupText({
    className: cx("border-none bg-transparent text-muted-foreground shadow-none", className),
    children,
  })
}

export type MessageResponseProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageResponse = ({ className, children }: MessageResponseProps): TemplateResult => {
  return html`
    <div class="${cx("size-full [&>*:first-child]:mt-0 [&>*:last-child]:mb-0", className)}">
      ${children ?? ""}
    </div>
  `
}

export type MessageToolbarProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const MessageToolbar = ({ className, children }: MessageToolbarProps): TemplateResult => {
  return html`
    <div class="${cx("mt-4 flex w-full items-center justify-between gap-4", className)}">
      ${children ?? ""}
    </div>
  `
}
