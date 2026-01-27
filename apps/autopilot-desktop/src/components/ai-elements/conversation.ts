import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import { Button } from "../ui/button.js"
import { cx, type AIChildren } from "./utils.js"

export type ConversationProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const Conversation = ({ className, children }: ConversationProps): TemplateResult => {
  return html`
    <div class="${cx("relative flex-1 overflow-y-hidden", className)}" role="log">
      ${children ?? ""}
    </div>
  `
}

export type ConversationContentProps = {
  readonly className?: string
  readonly children?: AIChildren
}

export const ConversationContent = ({
  className,
  children,
}: ConversationContentProps): TemplateResult => {
  return html`
    <div class="${cx("flex flex-col gap-8 p-4", className)}">${children ?? ""}</div>
  `
}

export type ConversationEmptyStateProps = {
  readonly className?: string
  readonly title?: string
  readonly description?: string
  readonly icon?: AIChildren
  readonly children?: AIChildren
}

export const ConversationEmptyState = ({
  className,
  title = "No messages yet",
  description = "Start a conversation to see messages here",
  icon,
  children,
}: ConversationEmptyStateProps): TemplateResult => {
  const fallback = html`
    ${icon ? html`<div class="text-muted-foreground">${icon}</div>` : ""}
    <div class="space-y-1">
      <h3 class="font-medium text-sm">${title}</h3>
      ${description ? html`<p class="text-muted-foreground text-sm">${description}</p>` : ""}
    </div>
  `
  return html`
    <div
      class="${cx(
        "flex size-full flex-col items-center justify-center gap-3 p-8 text-center",
        className
      )}"
    >
      ${children ?? fallback}
    </div>
  `
}

export type ConversationScrollButtonProps = {
  readonly className?: string
}

export const ConversationScrollButton = ({
  className,
}: ConversationScrollButtonProps): TemplateResult => {
  return Button({
    className: cx(
      "absolute bottom-4 left-[50%] translate-x-[-50%] rounded-full dark:bg-background dark:hover:bg-muted",
      className
    ),
    size: "icon",
    type: "button",
    variant: "outline",
    children: html`<span class="size-4">down</span>`,
  })
}
