// CL-51: "Cloud" card (honest "unavailable") + coordinator Pause/Resume toggle.
// renderCloudCard: delegates to the shared view-model from the protocol package
// (aliased to avoid a name clash), always renders the honest "unavailable" body
// for contributor nodes that report no metering.
// renderCoordinatorToggle: reflects ctx.node?.coordinatorPaused; flips the
// coordinator paused state via ctx.request.setCoordinatorPaused({ paused }).
// The toggle is mounted in the shell header slot (#coord-toggle).

import { renderCloudCard as cloudCardView } from "@openagentsinc/autopilot-control-protocol"
import type { PaneContext } from "../context"
import { card } from "../dom"

// Pure helper — exported so tests can cover label logic without a DOM.
export function coordinatorToggleLabel(paused: boolean): string {
  return paused ? "▶ Resume" : "⏸ Pause"
}

export function renderCloudCard(container: HTMLElement, ctx: PaneContext): void {
  // Pass the raw cloud metering from the latest node projection (or null if
  // the node hasn't reported yet). The protocol function always returns
  // visible:true with an honest body for contributor nodes.
  const raw: unknown = ctx.node ?? null
  const view = cloudCardView(raw)

  if (!view.visible) return

  const { section, body } = card(view.title)

  const bodyText = document.createElement("p")
  bodyText.textContent = view.body
  body.append(bodyText)

  const failoverNote = document.createElement("p")
  failoverNote.textContent = "Provider failover: see Accounts."
  failoverNote.style.marginTop = "4px"
  failoverNote.style.opacity = "0.7"
  body.append(failoverNote)

  container.append(section)
}

// Mounted into the shell header slot (#coord-toggle). Renders nothing when
// coordinatorPaused is null (node not yet reporting the field).
export function renderCoordinatorToggle(container: HTMLElement, ctx: PaneContext): void {
  const paused = ctx.node?.coordinatorPaused ?? null
  if (paused === null) return

  const btn = document.createElement("button")
  btn.textContent = coordinatorToggleLabel(paused)

  // Inline style to match the sidebar header slot without touching index.html.
  btn.style.fontFamily = "monospace"
  btn.style.padding = "3px 10px"
  btn.style.border = "1px solid"
  btn.style.borderColor = paused ? "#ffb454" : "currentColor"
  btn.style.background = "transparent"
  btn.style.cursor = "pointer"
  btn.style.color = "inherit"
  btn.style.borderRadius = "3px"

  btn.addEventListener("click", () => {
    const current = ctx.node?.coordinatorPaused ?? null
    if (current === null) return
    void ctx.request.setCoordinatorPaused({ paused: !current }).then(() => {
      ctx.refresh()
    })
  })

  container.append(btn)
}
