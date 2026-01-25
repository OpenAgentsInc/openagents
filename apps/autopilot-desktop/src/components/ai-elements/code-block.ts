import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type CodeBlockProps = {
  readonly code: string
  readonly language?: string
}

export const CodeBlock = ({
  code,
  language = "text",
}: CodeBlockProps): TemplateResult => html`
  <pre class="overflow-x-auto rounded-md border border-border bg-surface-strong p-3 text-[11px] text-foreground">
    <code data-language="${language}">${code}</code>
  </pre>
`
