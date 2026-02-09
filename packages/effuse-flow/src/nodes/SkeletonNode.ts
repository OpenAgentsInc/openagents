import { html } from "../effuse.js"
import type { TemplateResult } from "../effuse.js"

import type { FlowNode } from "../types.js"

const cx = (...parts: Array<string | null | undefined | false>): string =>
  parts.filter(Boolean).join(" ")

export type SkeletonNodeProps = {
  readonly node: FlowNode & { readonly metadata: { readonly type: "skeleton" } }
  readonly selected?: boolean
}

export function SkeletonNode({ node: _node, selected = false }: SkeletonNodeProps): TemplateResult {
  return html`
    <div class="${cx("oa-flow-node oa-flow-node--leaf oa-flow-node--skeleton", selected && "oa-flow-node--selected")}" data-selected="${selected ? "1" : "0"}">
      <div class="oa-flow-skeleton-line oa-flow-skeleton-line--a"></div>
      <div class="oa-flow-skeleton-line oa-flow-skeleton-line--b"></div>
    </div>
  `
}
