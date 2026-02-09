import { html } from "./effuse.js"
import type { TemplateResult } from "./effuse.js"

import type { Point } from "./layout-engine.js"

export type TreeElementNodeProps = {
  readonly id: string
  readonly position: Point
  readonly children: TemplateResult
}

export function TreeElementNode({ id, position, children }: TreeElementNodeProps): TemplateResult {
  return html`
    <foreignObject
      x="${String(position.x)}"
      y="${String(position.y)}"
      width="1"
      height="1"
      overflow="visible"
    >
      <div
        data-node-id="${id}"
        class="oa-flow-node-wrap"
        style="position:absolute;left:0;top:0;transform:translate(-50%,-50%);"
      >
        ${children}
      </div>
    </foreignObject>
  `
}
