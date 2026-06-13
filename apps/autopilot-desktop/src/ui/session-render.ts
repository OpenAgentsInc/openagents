import type { NodeStateMessage } from "../shared/rpc"
import { nodeStatusLine, sessionRows } from "./session-view"

function escapeHtml(value: string): string {
  return value
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#39;")
}

export function sessionsHtml(message: NodeStateMessage): string {
  const rows = sessionRows(message.sessions, message.events ?? {})
  const body =
    message.sessions.length === 0
      ? '<p class="empty-state">No sessions yet.</p>'
      : `<div class="session-list">${rows}</div>`

  return [
    '<header class="session-header">',
    "<h2>Sessions</h2>",
    `<p class="node-status">${escapeHtml(nodeStatusLine(message))}</p>`,
    "</header>",
    body,
  ].join("")
}

export function renderSessions(container: HTMLElement, message: NodeStateMessage): void {
  container.innerHTML = sessionsHtml(message)
}
