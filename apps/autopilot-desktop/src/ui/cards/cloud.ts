// CL-51: "Cloud" card (honest "unavailable") + coordinator Pause/Resume toggle.
// STUB — implemented in CL-51. Owns this file only.
//
// Implementation notes for CL-51:
// - renderCloudCard: use `renderCloudCard(null)` from
//   `@openagentsinc/autopilot-control-protocol` (the shared view-model) to show
//   the honest "metering unavailable" body; point failover at the Accounts card.
// - renderCoordinatorToggle: reflect `ctx.node?.coordinatorPaused`; flip via
//   `ctx.request.setCoordinatorPaused({ paused })`. "⏸ Pause" / "▶ Resume".
//   Mounted in the shell header — keep the exported name stable.

import type { PaneContext } from "../context"
import { card, emptyLine } from "../dom"

export function renderCloudCard(container: HTMLElement, ctx: PaneContext): void {
  const { section, body } = card("Cloud")
  void ctx
  body.append(emptyLine("Cloud status — coming in CL-51."))
  container.append(section)
}

// Mounted into the shell header slot (#coord-toggle). STUB renders nothing until
// CL-51; keep this exported signature stable so the shell import doesn't break.
export function renderCoordinatorToggle(container: HTMLElement, ctx: PaneContext): void {
  void container
  void ctx
}
