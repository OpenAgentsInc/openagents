import type { Document } from "foldkit/html"
import { html } from "foldkit/html"

import { FoldkitDemoClickedPing } from "./message.js"
import type { KhalaCodeFoldkitMessage } from "./message.js"
import type { KhalaCodeFoldkitModel } from "./model.js"

const h = html<KhalaCodeFoldkitMessage>()

export const view = (model: KhalaCodeFoldkitModel): Document => ({
  title: "Khala Code",
  body: h.section(
    [
      h.Class("khala-code-foldkit-demo"),
      h.DataAttribute("foldkit-mount-id", model.mountId),
    ],
    [
      h.p([h.Class("khala-code-foldkit-demo-label")], [model.label]),
      h.button(
        [
          h.Type("button"),
          h.Class("khala-code-foldkit-demo-ping"),
          h.OnClick(FoldkitDemoClickedPing()),
        ],
        [`Ping ${model.pingCount}`],
      ),
    ],
  ),
})
