// CL-44: the "Nodes" dashboard pane — the desktop equivalent of mobile's Nodes
// screen. Connection status + breakdown, then the parity cards (deploy, ask,
// approvals, balance, assignments, cloud), notifications, accounts, and a
// clickable sessions preview. Each card lives in its own file (CL-47..CL-51);
// this pane only composes them, so filling a card never touches this file.

import type { PaneContext } from "../context"
import { renderAskCard } from "../cards/ask"
import { renderApprovalsCard } from "../cards/approvals"
import { renderAssignmentsCard } from "../cards/assignments"
import { renderBalanceCard } from "../cards/balance"
import { renderCloudCard } from "../cards/cloud"
import { renderDeployCard } from "../cards/deploy"
import { renderNotifications } from "../notification-view"
import { renderSessionList } from "../session-list"
import { nodeStatusLine } from "../session-view"

function statusLine(ctx: PaneContext): HTMLElement {
  const p = document.createElement("p")
  p.className = "node-status"
  if (!ctx.node) {
    p.textContent = "connecting…"
    return p
  }
  p.textContent = nodeStatusLine({ ok: ctx.node.ok, sessions: ctx.node.sessions })
  return p
}

export function renderNodesPane(container: HTMLElement, ctx: PaneContext): void {
  const h = document.createElement("h1")
  h.className = "pane-title"
  h.textContent = "Autopilot"
  container.append(h)
  container.append(statusLine(ctx))

  // Action + status cards. Each renders itself (and renders nothing when its
  // data is absent), so the dashboard stays clean on a fresh node.
  renderDeployCard(container, ctx)
  renderAskCard(container, ctx)
  renderApprovalsCard(container, ctx)
  renderBalanceCard(container, ctx)
  renderAssignmentsCard(container, ctx)
  renderCloudCard(container, ctx)

  // In-app notification center (CL-30).
  if (ctx.notifications) {
    const notif = document.createElement("section")
    notif.id = "notifications"
    renderNotifications(notif, ctx.notifications)
    container.append(notif)
  }

  // Clickable sessions preview → detail. The full management list is the
  // Sessions pane (CL-55); this mirrors mobile's inline dashboard list.
  const sessions = document.createElement("section")
  sessions.className = "card"
  const sh = document.createElement("h2")
  sh.className = "card-title"
  sh.textContent = "Sessions"
  sessions.append(sh)
  const list = document.createElement("div")
  sessions.append(list)
  if (ctx.node) renderSessionList(list, ctx.node, { onSelect: (ref) => ctx.navigate("session-detail", ref) })
  else list.innerHTML = '<p class="empty-state">Connecting…</p>'
  container.append(sessions)
}
