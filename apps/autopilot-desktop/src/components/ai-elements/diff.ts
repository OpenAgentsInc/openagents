import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type DiffProps = {
  readonly title: string
  readonly diff: string
  readonly status: string | null
}

export const Diff = ({
  title,
  diff,
  status,
}: DiffProps): TemplateResult => html`
  <div class="border border-border bg-background px-3 py-2 text-xs text-foreground">
    <div class="flex items-center justify-between text-muted-foreground">
      <span class="uppercase">Diff</span>
      ${status ? html`<span class="uppercase">${status}</span>` : ""}
    </div>
    <div class="mt-1 font-semibold text-foreground">${title}</div>
    <div class="mt-2 overflow-x-auto rounded border border-white/10 bg-black/50 p-2 font-mono text-[10px]">
      <pre>${diff}</pre>
    </div>
  </div>
`
