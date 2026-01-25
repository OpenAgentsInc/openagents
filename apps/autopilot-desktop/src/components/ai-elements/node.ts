import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type NodeTone = "default" | "accent" | "muted"

export type NodeProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
  readonly tone?: NodeTone
}

const toneClasses: Record<NodeTone, string> = {
  default: "border-border bg-surface",
  accent: "border-accent/60 bg-surface",
  muted: "border-border/60 bg-surface-muted",
}

export const Node = ({ children, tone = "default" }: NodeProps): TemplateResult => html`
  <article class="flex w-full flex-col overflow-hidden rounded-md border ${toneClasses[tone]} shadow-sm">
    ${children}
  </article>
`

export type NodeHeaderProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeHeader = ({ children }: NodeHeaderProps): TemplateResult => html`
  <header class="flex items-center justify-between border-b border-border bg-secondary px-3 py-2">
    ${children}
  </header>
`

export type NodeTitleProps = {
  readonly text: string
}

export const NodeTitle = ({ text }: NodeTitleProps): TemplateResult => html`
  <h3 class="text-xs font-semibold text-foreground">${text}</h3>
`

export type NodeDescriptionProps = {
  readonly text: string
}

export const NodeDescription = ({ text }: NodeDescriptionProps): TemplateResult => html`
  <p class="text-[11px] text-muted-foreground">${text}</p>
`

export type NodeActionProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeAction = ({ children }: NodeActionProps): TemplateResult => html`
  <div class="flex items-center gap-2 text-[11px] text-muted-foreground">${children}</div>
`

export type NodeContentProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeContent = ({ children }: NodeContentProps): TemplateResult => html`
  <div class="flex flex-col gap-2 px-3 py-3 text-[12px] text-foreground">
    ${children}
  </div>
`

export type NodeFooterProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeFooter = ({ children }: NodeFooterProps): TemplateResult => html`
  <footer class="flex items-center justify-between border-t border-border bg-secondary px-3 py-2 text-[11px] text-muted-foreground">
    ${children}
  </footer>
`
