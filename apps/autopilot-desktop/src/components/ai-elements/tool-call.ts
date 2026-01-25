import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type ToolCallProps = {
  readonly title: string
  readonly detail?: string | undefined
  readonly output?: string | null | undefined
  readonly status?: string | null | undefined
  readonly isStreaming?: boolean
  readonly durationMs?: number | null | undefined
}

export const ToolCall = ({
  title,
  detail,
  output,
  status,
  isStreaming = false,
  durationMs,
}: ToolCallProps): TemplateResult => html`
  <div class="border border-border bg-background px-3 py-2 text-xs text-foreground">
    <div class="flex items-center justify-between text-muted-foreground">
      <span class="uppercase">Tool</span>
      <div class="flex items-center gap-2">
        ${durationMs ? html`<span>${durationMs}ms</span>` : ""}
        <span class="uppercase">${status ?? (isStreaming ? "running" : "completed")}</span>
      </div>
    </div>
    <div class="mt-1 font-mono text-foreground">${title}</div>
    ${
      detail
        ? html`<div class="mt-2 whitespace-pre-wrap break-words text-muted-foreground">
            ${detail}
          </div>`
        : ""
    }
    ${
      output
        ? html`<div class="mt-2 whitespace-pre-wrap break-words text-foreground">
            ${output}
          </div>`
        : ""
    }
  </div>
`
