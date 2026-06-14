// CL-58: "Settings" pane — node/connection, notifications, theme, updates,
// about. STUB — implemented in CL-58. Owns this file only.
//
// Implementation notes for CL-58:
// - Connection: node status (ctx.node?.ok / schema), loopback home info.
// - Notifications: desktop OS notifications (CL-30) state.
// - Theme: read-only dark-token section.
// - Updates: the auto-update feed chooser (full/bsdiff/none) — see
//   src/shared/update-feed.ts.
// - About: app version/build + protocol schema tag (ctx.node?.schema).

import type { PaneContext } from "../context"
import { emptyLine } from "../dom"

export function renderSettingsPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Settings"
  container.append(h)
  const schema = ctx.node?.schema ?? "not connected"
  container.append(emptyLine(`protocol: ${schema} — full Settings coming in CL-58.`))
}
