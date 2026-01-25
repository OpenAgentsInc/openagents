import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"
import type { CodexToolCall } from "../../types/codex.js"

export type ToolCallProps = Pick<
  CodexToolCall,
  "title" | "detail" | "output" | "status" | "durationMs" | "changes" | "toolType"
> & {
  readonly isStreaming?: boolean
}

export const ToolCall = ({
  title,
  detail,
  output,
  status,
  isStreaming = false,
  durationMs,
  changes,
  toolType,
}: ToolCallProps): TemplateResult => html`
  <div class="border border-border bg-background px-3 py-2 text-xs text-foreground">
    <div class="flex items-center justify-between text-muted-foreground">
      <span class="uppercase">Tool</span>
      <div class="flex items-center gap-2">
        ${durationMs ? html`<span>${durationMs}ms</span>` : ""}
        <span class="uppercase">${status ?? (isStreaming ? "running" : "completed")}</span>
      </div>
    </div>
    <div class="mt-1 font-mono text-foreground">
      ${title}${toolType ? html` <span class="text-muted-foreground">(${toolType})</span>` : ""}
    </div>
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
    ${
      changes && changes.length
        ? html`<div class="mt-2 space-y-2 text-muted-foreground">
            <div class="text-[10px] uppercase">Changes</div>
            <div class="space-y-1">
              ${changes.map(
                (change) => html`<div class="flex items-center justify-between gap-3">
                  <span class="font-mono text-[11px] text-foreground">${change.path}</span>
                  ${change.kind ? html`<span class="text-[10px] uppercase">${change.kind}</span>` : ""}
                </div>`
              )}
            </div>
          </div>`
        : ""
    }
  </div>
`
