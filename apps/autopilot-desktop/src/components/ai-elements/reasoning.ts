import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type ReasoningProps = {
  readonly summary?: string
  readonly content: string
  readonly isStreaming?: boolean
  readonly open?: boolean
}

export const Reasoning = ({
  summary = "Reasoning",
  content,
  isStreaming = false,
  open,
}: ReasoningProps): TemplateResult => html`
  <details
    class="border border-border bg-background px-3 py-2 text-xs text-foreground"
    ${open || isStreaming ? "open" : ""}
  >
    <summary class="flex cursor-pointer list-none items-center justify-between text-[10px] uppercase text-muted-foreground">
      <span class="flex items-center gap-2">
        <span
          class="inline-flex h-2 w-2 rounded-full ${
            isStreaming
              ? "bg-accent"
              : "bg-muted-foreground"
          }"
        ></span>
        ${summary}
      </span>
      <span
        class="text-[10px] font-semibold ${
          isStreaming
            ? "text-accent"
            : "text-muted-foreground"
        }"
      >
        ${isStreaming ? "Thinking" : "Complete"}
      </span>
    </summary>
    <div class="mt-2 whitespace-pre-wrap break-words text-xs leading-relaxed text-foreground">
      ${content || "No reasoning captured yet."}
    </div>
  </details>
`
