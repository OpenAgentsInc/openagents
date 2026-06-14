// CL-52: "Session Detail" pane — focused view of one session: verify line,
// artifact line, Cancel, expandable event timeline. STUB — implemented in
// CL-52. Owns this file only.
//
// Implementation notes for CL-52:
// - Read the selected session via `ctx.selectedSessionRef` +
//   `ctx.node?.sessions.find(...)`; events from `ctx.node?.events?.[ref]`.
// - "‹ back" → `ctx.navigate("sessions")`.
// - Cancel when state ∈ {running, queued, started} via
//   `ctx.request.cancelSession({ sessionRef })`.
// - Expandable events: click a row to toggle `event.full` (fixed-height when
//   collapsed). Reuse helpers from "../session-view" where possible.

import type { PaneContext } from "../context"
import { emptyLine, escapeHtml } from "../dom"

export function renderSessionDetailPane(container: HTMLElement, ctx: PaneContext): void {
  const back = document.createElement("button")
  back.className = "link-button"
  back.textContent = "‹ sessions"
  back.addEventListener("click", () => ctx.navigate("sessions"))
  container.append(back)

  const ref = ctx.selectedSessionRef
  const session = ref ? ctx.node?.sessions.find((s) => s.sessionRef === ref) ?? null : null
  if (!session) {
    container.append(emptyLine("Session not found."))
    return
  }
  const h = document.createElement("p")
  h.className = "detail-ref"
  h.textContent = ref ?? ""
  container.append(h)
  container.append(emptyLine(`state: ${escapeHtml(session.state)} — full detail coming in CL-52.`))
}
