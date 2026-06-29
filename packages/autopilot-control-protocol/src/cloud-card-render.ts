import { buildCloudPanel } from "./cloud-panel-view-model.js"

export type CloudCardRender = {
  visible: boolean
  title: string
  body: string
}

const TITLE = "Cloud metering"
const UNAVAILABLE_BODY = "Cloud metering is not reported by this node."

export function renderCloudCard(raw: unknown): CloudCardRender {
  const panel = buildCloudPanel(raw)

  return {
    visible: true,
    title: TITLE,
    body: panel.available ? panel.line : UNAVAILABLE_BODY,
  }
}
