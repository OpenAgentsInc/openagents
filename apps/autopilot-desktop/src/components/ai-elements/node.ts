import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type NodeTone = "default" | "accent" | "muted"

export type NodeProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
  readonly className?: string
  readonly tone?: NodeTone
  readonly handles?: {
    readonly target?: boolean
    readonly source?: boolean
    readonly top?: boolean
    readonly bottom?: boolean
  }
}

const toneClasses: Record<NodeTone, string> = {
  default: "border-border bg-card text-card-foreground",
  accent: "border-accent/60 bg-card text-card-foreground",
  muted: "border-border/60 bg-card text-card-foreground",
}

export const Node = ({
  children,
  className = "",
  tone = "default",
  handles,
}: NodeProps): TemplateResult => html`
  <article
    class="node-container relative size-full h-auto w-sm flex flex-col gap-0 rounded-md border p-0 shadow-sm ${toneClasses[tone]} ${className}"
  >
    ${handles?.target
      ? html`<span class="node-handle node-handle--target" aria-hidden="true"></span>`
      : ""}
    ${handles?.source
      ? html`<span class="node-handle node-handle--source" aria-hidden="true"></span>`
      : ""}
    ${handles?.top
      ? html`<span class="node-handle node-handle--top" aria-hidden="true"></span>`
      : ""}
    ${handles?.bottom
      ? html`<span class="node-handle node-handle--bottom" aria-hidden="true"></span>`
      : ""}
    ${children}
  </article>
`

export type NodeHeaderProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeHeader = ({ children }: NodeHeaderProps): TemplateResult => html`
  <header class="grid auto-rows-min grid-rows-[auto_auto] grid-cols-[1fr_auto] items-start gap-0.5 rounded-t-md border-b border-border bg-secondary p-3">
    ${children}
  </header>
`

export type NodeTitleProps = {
  readonly text: string
}

export const NodeTitle = ({ text }: NodeTitleProps): TemplateResult => html`
  <div class="leading-none font-semibold">${text}</div>
`

export type NodeDescriptionProps = {
  readonly text: string
}

export const NodeDescription = ({ text }: NodeDescriptionProps): TemplateResult => html`
  <div class="text-sm text-muted-foreground">${text}</div>
`

export type NodeActionProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeAction = ({ children }: NodeActionProps): TemplateResult => html`
  <div class="col-start-2 row-span-2 row-start-1 self-start justify-self-end">
    ${children}
  </div>
`

export type NodeContentProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeContent = ({ children }: NodeContentProps): TemplateResult => html`
  <div class="p-3">${children}</div>
`

export type NodeFooterProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const NodeFooter = ({ children }: NodeFooterProps): TemplateResult => html`
  <footer class="flex items-center rounded-b-md border-t border-border bg-secondary p-3">
    ${children}
  </footer>
`
