import { html, rawHtml } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

const gapStyle = (gap?: number) =>
  gap !== undefined ? rawHtml(` style="gap: ${gap}px"`) : ""

export type StackProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
  readonly gap?: number
  readonly align?: "start" | "center" | "end" | "stretch"
}

export const Stack = ({ children, gap, align = "stretch" }: StackProps): TemplateResult => html`
  <div class="flex flex-col items-${align}"${gapStyle(gap)}>
    ${children}
  </div>
`

export type RowProps = {
  readonly children: TemplateResult | readonly TemplateResult[]
  readonly gap?: number
  readonly align?: "start" | "center" | "end" | "stretch"
  readonly justify?: "start" | "center" | "end" | "between"
}

export const Row = ({
  children,
  gap,
  align = "center",
  justify = "start",
}: RowProps): TemplateResult => html`
  <div class="flex items-${align} justify-${justify}"${gapStyle(gap)}>
    ${children}
  </div>
`

export type PanelProps = {
  readonly title?: string
  readonly subtitle?: string
  readonly children: TemplateResult | readonly TemplateResult[]
}

export const Panel = ({ title, subtitle, children }: PanelProps): TemplateResult => html`
  <section class="flex flex-col gap-3 rounded-md border border-border bg-surface px-4 py-3 shadow-sm">
    ${title
      ? html`
          <header class="flex flex-col">
            <span class="text-[11px] uppercase text-muted-foreground">${title}</span>
            ${subtitle ? html`<span class="text-xs text-foreground">${subtitle}</span>` : ""}
          </header>
        `
      : ""}
    <div class="flex flex-col gap-2 text-[12px] text-foreground">${children}</div>
  </section>
`
