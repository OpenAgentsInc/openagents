import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type ConversationProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const Conversation = ({ children }: ConversationProps): TemplateResult => html`
  <section class="flex h-full flex-col gap-5">
    ${children}
  </section>
`

export type ConversationContentProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const ConversationContent = ({
  children,
}: ConversationContentProps): TemplateResult => html`
  <div class="flex flex-col gap-5">
    ${children}
  </div>
`

export type ConversationEmptyStateProps = {
  readonly title?: string
  readonly description?: string
}

export const ConversationEmptyState = ({
  title = "No messages yet",
  description = "Start a conversation to see messages here",
}: ConversationEmptyStateProps): TemplateResult => html`
  <div class="flex flex-col items-center justify-center gap-2 border border-border bg-surface-muted px-6 py-8 text-center text-xs text-muted-foreground">
    <div class="text-sm font-semibold text-foreground">${title}</div>
    <div class="text-sm text-muted-foreground">${description}</div>
  </div>
`
