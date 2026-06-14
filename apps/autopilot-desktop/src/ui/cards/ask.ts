// CL-47: "Ask Autopilot" (submit a work intent) + "Your asks" (ship-status
// round-trip). STUB — implemented in CL-47. Owns this file only.
//
// Implementation notes for CL-47:
// - Title + optional body inputs, validated via `validateIntentDraft` from
//   `@openagentsinc/autopilot-control-protocol`; submit via
//   `ctx.request.submitIntent({ title, body })`; show sending/sent/error.
// - "Your asks": list `ctx.node?.intents` with a ship-status line
//   (received → planning → fanning_out → shipping → shipped/failed), toned.

import type { PaneContext } from "../context"
import { card, emptyLine } from "../dom"

export function renderAskCard(container: HTMLElement, ctx: PaneContext): void {
  const { section, body } = card("Ask Autopilot")
  void ctx
  body.append(emptyLine("Ask Autopilot — coming in CL-47."))
  container.append(section)
}
