import { html } from "../effuse.js"
import type { TemplateResult } from "../effuse.js"

import type { FlowNode, FlowNodeMetadata } from "../types.js"
import { StatusDot } from "../ui.js"

const cx = (...parts: Array<string | null | undefined | false>): string =>
  parts.filter(Boolean).join(" ")

export type RootNodeProps = {
  readonly node: FlowNode & { readonly metadata: FlowNodeMetadata & { readonly type: "root" } }
  readonly selected?: boolean
}

export function RootNode({ node, selected = false }: RootNodeProps): TemplateResult {
  return html`
    <div class="${cx("oa-flow-node oa-flow-node--root", selected && "oa-flow-node--selected")}" data-selected="${selected ? "1" : "0"}">
      <div class="oa-flow-node__title" title="${node.label}">${node.label}</div>
      ${StatusDot({ status: node.metadata?.status })}
    </div>
  `
}
