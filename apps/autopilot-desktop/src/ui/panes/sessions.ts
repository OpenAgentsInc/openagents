// CL-55: "Sessions" pane — full session list/management (all states, filter,
// search). STUB — implemented in CL-55. Owns this file only.
//
// Implementation notes for CL-55:
// - Use `renderSessionList(container, ctx.node, { filter, onSelect })` from
//   "../session-list" for the clickable, nested list.
// - Add state filter segments (running/queued/completed/failed/cancelled) + a
//   count breakdown; optional text filter by ref/activity.
// - onSelect → `ctx.navigate("session-detail", ref)`.

import type { PaneContext } from "../context"
import { renderSessionList } from "../session-list"

export function renderSessionsPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Sessions"
  container.append(h)
  const list = document.createElement("div")
  container.append(list)
  if (ctx.node) renderSessionList(list, ctx.node, { onSelect: (ref) => ctx.navigate("session-detail", ref) })
  else list.innerHTML = '<p class="empty-state">Connecting…</p>'
}
