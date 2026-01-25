import { html } from "../../effuse/template/html.js"
import type { TemplateResult } from "../../effuse/template/types.js"

export type CanvasProps = {
  readonly title?: string
  readonly subtitle?: string
  readonly status?: string
  readonly children?: TemplateResult | readonly TemplateResult[]
}

export const Canvas = ({
  title = "Autopilot Canvas",
  subtitle,
  status = "Idle",
  children,
}: CanvasProps): TemplateResult => html`
  <section class="flex h-full w-full flex-col overflow-hidden rounded-md border border-border bg-surface shadow-sm">
    <header class="flex items-center justify-between border-b border-border px-4 py-2 text-[11px] uppercase text-muted-foreground">
      <div class="flex flex-col">
        <span>${title}</span>
        ${subtitle ? html`<span class="text-[10px] normal-case">${subtitle}</span>` : ""}
      </div>
      <span class="rounded border border-border px-2 py-0.5 text-[10px] uppercase">${status}</span>
    </header>
    <div class="relative flex-1 bg-surface-muted">
      <div
        class="pointer-events-none absolute inset-0 bg-[radial-gradient(circle_at_center,rgba(255,255,255,0.08)_1px,transparent_1px)] bg-[size:20px_20px]"
        aria-hidden="true"
      ></div>
      <div class="relative z-10 flex h-full w-full flex-col gap-4 p-6">
        ${children ?? ""}
      </div>
    </div>
  </section>
`
