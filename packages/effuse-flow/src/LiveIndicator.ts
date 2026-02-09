import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

/**
 * Small "Live" status badge for overlay parity. No backend required.
 */
export function LiveIndicator(): TemplateResult {
  return html`
    <div
      class="oa-flow-live"
      style="pointer-events:none;position:absolute;left:14px;top:54px;display:flex;align-items:center;gap:8px;border-radius:10px;border:1px solid var(--oa-flow-stroke);background:rgba(18,26,40,0.92);backdrop-filter:blur(10px);padding:8px 10px;box-shadow:0 10px 30px rgba(0,0,0,0.22);"
    >
      <span style="width:6px;height:6px;border-radius:999px;background:rgb(52,211,153);" aria-hidden="true"></span>
      <span style="font-size:11px;font-weight:800;color:var(--oa-flow-text);">Live</span>
    </div>
  `
}
