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

function accountsHtml(message: NodeStateMessage): string {
  const accounts = message.accounts ?? []
  if (accounts.length === 0) return ""
  const rows = accounts
    .map(
      (account) =>
        `<li class="account-row account-${account.ready ? "ready" : "blocked"}">${escapeHtml(account.provider)} · ${escapeHtml(account.homeState)} · ${account.ready ? "ready" : "blocked"}</li>`,
    )
    .join("")
  return `<section class="accounts"><h3>Accounts</h3><ul class="account-list">${rows}</ul></section>`
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
    accountsHtml(message),
    body,
  ].join("")
}

export function renderSessions(container: HTMLElement, message: NodeStateMessage): void {
  container.innerHTML = sessionsHtml(message)
}
