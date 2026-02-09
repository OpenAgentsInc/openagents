import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

export type DevTreeGeneratorProps = {
  /** Whether to render. Prefer `enabled: import.meta.env.DEV` in Vite apps. */
  readonly enabled?: boolean
}

/**
 * Dev-only overlay: preset buttons (Small, Medium, Large) and Reset to generate or clear a flow tree.
 *
 * This component is intentionally "headless": it emits `data-oa-flow-action` attributes
 * that a host controller can listen for via event delegation.
 */
export function DevTreeGenerator({ enabled = false }: DevTreeGeneratorProps): TemplateResult {
  if (!enabled) return html``

  const button = (label: string, action: string, preset?: string) =>
    html`<button
      type="button"
      class="oa-flow-dev-btn"
      data-oa-flow-action="${action}"
      ${preset ? html`data-oa-flow-preset="${preset}"` : html``}
    >
      ${label}
    </button>`

  return html`
    <div
      class="oa-flow-dev"
      style="pointer-events:auto;position:absolute;left:14px;bottom:14px;border-radius:12px;border:1px solid var(--oa-flow-stroke);background:rgba(18,26,40,0.92);backdrop-filter:blur(10px);padding:10px 12px;box-shadow:0 16px 50px rgba(0,0,0,0.35);"
    >
      <div style="font-size:11px;font-weight:800;color:var(--oa-flow-muted);">Dev: tree</div>
      <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:8px;">
        ${button("Small", "dev.tree.generate", "small")}
        ${button("Medium", "dev.tree.generate", "medium")}
        ${button("Large", "dev.tree.generate", "large")}
        ${button("Reset", "dev.tree.reset")}
      </div>
    </div>
  `
}
