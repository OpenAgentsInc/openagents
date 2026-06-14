// CL-44: a clickable session list with sub-agent nesting, reused by the Nodes
// dashboard (preview) and the Sessions pane (CL-55). Wraps the existing
// `sessionRows` HTML and adds data-session-ref + delegated click → onSelect, so
// callers get navigation without re-implementing row markup.

import type { NodeStateMessage } from "../shared/rpc"
import { escapeHtml } from "./dom"
import { sessionRows } from "./session-view"

export type SessionListOptions = {
  // Optional predicate to filter which sessions show (Sessions pane filters).
  readonly filter?: (state: string) => boolean
  // Called with the sessionRef when a row is clicked.
  readonly onSelect?: (sessionRef: string) => void
}

export function renderSessionList(
  container: HTMLElement,
  message: NodeStateMessage,
  options: SessionListOptions = {},
): void {
  const sessions = options.filter ? message.sessions.filter((s) => options.filter!(s.state)) : message.sessions
  if (sessions.length === 0) {
    container.innerHTML = '<p class="empty-state">No sessions.</p>'
    return
  }

  // Wrap each top-level + child row in a clickable shell carrying its ref. We
  // re-use sessionRows per session so nesting/verify/artifact/timeline markup
  // stays identical to the rest of the app.
  const filtered = { ...message, sessions }
  const childrenOf = (ref: string) => sessions.filter((s) => s.parentRef === ref)
  const isTop = (s: (typeof sessions)[number]) => !s.parentRef || !sessions.some((p) => p.sessionRef === s.parentRef)

  const html = sessions
    .filter(isTop)
    .map((s) => {
      const group = [s, ...childrenOf(s.sessionRef)]
      const rowsHtml = sessionRows(group, filtered.events ?? {}, filtered.artifacts ?? {})
      return `<div class="session-click" data-session-ref="${escapeHtml(s.sessionRef)}" role="button" tabindex="0">${rowsHtml}</div>`
    })
    .join("")
  container.innerHTML = html

  if (options.onSelect) {
    const select = options.onSelect
    container.querySelectorAll<HTMLElement>(".session-click").forEach((node) => {
      const ref = node.dataset.sessionRef
      if (!ref) return
      node.addEventListener("click", () => select(ref))
      node.addEventListener("keydown", (e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault()
          select(ref)
        }
      })
    })
  }
}
