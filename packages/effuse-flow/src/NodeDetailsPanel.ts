import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import type { FlowNode } from "./types.js"
import { Pill, StatusPill } from "./ui.js"

export type NodeDetailsPanelProps = {
  readonly node: FlowNode | null
  /** Optional action renderer (buttons/links). */
  readonly renderActions?: ((node: FlowNode) => TemplateResult) | undefined
}

const detailRow = (label: string, value: string): TemplateResult =>
  html`
    <div class="oa-flow-details__row">
      <span class="oa-flow-details__label">${label}</span>
      <span class="oa-flow-details__value" title="${value}">${value}</span>
    </div>
  `

export function NodeDetailsPanel({ node, renderActions }: NodeDetailsPanelProps): TemplateResult {
  if (!node) return html``

  const childCount = node.children?.length ?? 0
  const childLabel = childCount === 1 ? "1 child" : `${childCount} children`

  const status = node.metadata?.status
  const badge = node.metadata?.badge
  const kind = node.metadata?.kind
  const updatedAt = node.metadata?.updatedAt

  return html`
    <aside class="oa-flow-details" data-oa-flow-details="1" data-node-id="${node.id}">
      <header class="oa-flow-details__header">
        <div style="min-width:0;flex:1;">
          <div class="oa-flow-details__title" title="${node.label}">${node.label}</div>
          ${node.metadata?.subtitle
            ? html`<div style="margin-top:4px;font-size:12px;color:var(--oa-flow-muted);line-height:1.25;">
                ${node.metadata.subtitle}
              </div>`
            : html``}
        </div>
        <button
          type="button"
          class="oa-flow-details__close"
          data-oa-flow-action="details.close"
          aria-label="Close"
        >
          ×
        </button>
      </header>

      <div class="oa-flow-details__body">
        <div style="display:flex;flex-wrap:wrap;gap:8px;align-items:center;">
          ${StatusPill({ status })}
          ${badge?.label ? Pill({ tone: badge.tone, children: badge.label }) : html``}
          ${kind ? Pill({ tone: "neutral", children: kind }) : html``}
          ${updatedAt ? Pill({ tone: "neutral", children: updatedAt }) : html``}
        </div>

        ${node.metadata?.detail
          ? html`<div style="margin-top:10px;font-size:12px;line-height:1.35;color:var(--oa-flow-muted);">
              ${node.metadata.detail}
            </div>`
          : html``}

        <div style="margin-top:12px;border:1px solid rgba(255,255,255,0.10);border-radius:10px;padding:10px 12px;background:rgba(255,255,255,0.02);">
          ${detailRow("ID", node.id)}
          ${detailRow("Children", childLabel)}
        </div>

        ${renderActions
          ? html`<div style="margin-top:12px;">
              <div style="font-size:12px;font-weight:800;color:var(--oa-flow-text);">Quick actions</div>
              <div style="display:flex;flex-wrap:wrap;gap:8px;margin-top:8px;">
                ${renderActions(node)}
              </div>
            </div>`
          : html``}
      </div>
    </aside>
  `
}
