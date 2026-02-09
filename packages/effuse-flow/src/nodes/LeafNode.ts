import { html } from "../effuse.js"
import type { TemplateResult } from "../effuse.js"

import type { FlowNode, FlowNodeMetadata } from "../types.js"
import { Pill, StatusDot } from "../ui.js"

const cx = (...parts: Array<string | null | undefined | false>): string =>
  parts.filter(Boolean).join(" ")

export type LeafNodeProps = {
  readonly node: FlowNode & { readonly metadata: FlowNodeMetadata & { readonly type: "leaf" } }
  readonly selected?: boolean
}

export function LeafNode({ node, selected = false }: LeafNodeProps): TemplateResult {
  const subtitle = node.metadata?.subtitle
  const status = node.metadata?.status
  const badge = node.metadata?.badge

  return html`
    <div class="${cx("oa-flow-node oa-flow-node--leaf", selected && "oa-flow-node--selected")}" data-selected="${selected ? "1" : "0"}">
      <div class="oa-flow-node__row">
        <div class="oa-flow-node__main">
          <div class="oa-flow-node__title" title="${node.label}">${node.label}</div>
          <div class="oa-flow-node__subtitle" title="${subtitle ?? ""}">${subtitle ?? ""}</div>
        </div>
        <div class="oa-flow-node__aside">
          ${StatusDot({ status })}
          ${badge?.label
            ? Pill({
                tone: badge.tone,
                className: "oa-flow-pill--tiny",
                children: badge.label,
              })
            : html``}
        </div>
      </div>
    </div>
  `
}
