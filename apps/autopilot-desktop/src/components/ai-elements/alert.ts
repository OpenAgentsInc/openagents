import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type AlertTone = "info" | "warn" | "error" | "success"

const toneClasses: Record<AlertTone, string> = {
  info: "border-border bg-surface text-foreground",
  warn: "border-chart-3/60 bg-surface text-foreground",
  error: "border-destructive/60 bg-surface text-destructive",
  success: "border-chart-2/60 bg-surface text-foreground",
}

export type AlertProps = {
  readonly title?: string
  readonly message: string
  readonly tone?: AlertTone
}

export const Alert = ({
  title,
  message,
  tone = "info",
}: AlertProps): TemplateResult => html`
  <div class="rounded-md border px-3 py-2 text-[11px] ${toneClasses[tone]}">
    ${title ? html`<div class="text-[10px] uppercase text-muted-foreground">${title}</div>` : ""}
    <div class="text-[12px]">${message}</div>
  </div>
`
