// CL-44: the desktop app shell — a persistent left sidebar (the desktop
// equivalent of mobile's slide-over drawer) plus a content area that swaps
// panes. The shell owns the live store (latest node projection + notifications +
// navigation) and builds the PaneContext each render; panes/cards are pure
// render functions, so the nav never reaches into their internals.

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"
import { renderCoordinatorToggle } from "./cards/cloud"
import type { DesktopRequests, PaneContext, PaneId } from "./context"
import { renderDecisionsPane } from "./panes/decisions"
import { renderNodesPane } from "./panes/nodes"
import { renderSessionDetailPane } from "./panes/session-detail"
import { renderSessionsPane } from "./panes/sessions"
import { renderSettingsPane } from "./panes/settings"
import { renderSpawnPane } from "./panes/spawn"
import type { NodeStateMessage } from "../shared/rpc"

const NAV: { readonly id: PaneId; readonly label: string }[] = [
  { id: "nodes", label: "Nodes" },
  { id: "sessions", label: "Sessions" },
  { id: "decisions", label: "Decisions" },
  { id: "spawn", label: "Spawn" },
  { id: "settings", label: "Settings" },
]

function paneRenderer(pane: PaneId) {
  switch (pane) {
    case "nodes":
      return renderNodesPane
    case "sessions":
      return renderSessionsPane
    case "decisions":
      return renderDecisionsPane
    case "spawn":
      return renderSpawnPane
    case "settings":
      return renderSettingsPane
    case "session-detail":
      return renderSessionDetailPane
  }
}

export type Shell = {
  onNodeState(message: NodeStateMessage): void
  onNotifications(view: NotificationCenterView): void
}

export function mountShell(request: DesktopRequests): Shell {
  let node: NodeStateMessage | null = null
  let notifications: NotificationCenterView | null = null
  let pane: PaneId = "nodes"
  let selectedSessionRef: string | null = null

  // Frame.
  const root = document.createElement("div")
  root.className = "app-shell"
  const sidebar = document.createElement("nav")
  sidebar.className = "sidebar"
  const content = document.createElement("main")
  content.className = "content"
  root.append(sidebar, content)
  document.body.innerHTML = ""
  document.body.append(root)

  function ctx(): PaneContext {
    return {
      node,
      notifications,
      selectedSessionRef,
      request,
      navigate(next, sessionRef) {
        pane = next
        if (sessionRef !== undefined) selectedSessionRef = sessionRef
        renderSidebar()
        renderContent()
      },
      refresh() {
        renderContent()
      },
    }
  }

  function nodeLabel(): string {
    if (!node) return "connecting…"
    const count = node.sessions.length
    return node.ok ? `online · ${count} ${count === 1 ? "session" : "sessions"}` : "offline"
  }

  function renderSidebar(): void {
    sidebar.innerHTML = ""
    const header = document.createElement("div")
    header.className = "sidebar-header"
    const title = document.createElement("div")
    title.className = "sidebar-title"
    title.textContent = "🛩️ Autopilot"
    const status = document.createElement("div")
    status.className = `sidebar-status ${node?.ok ? "status-online" : "status-offline"}`
    status.textContent = nodeLabel()
    header.append(title, status)
    sidebar.append(header)

    // Coordinator Pause/Resume lives in the header slot (CL-51 fills it).
    const coord = document.createElement("div")
    coord.id = "coord-toggle"
    coord.className = "coord-slot"
    renderCoordinatorToggle(coord, ctx())
    sidebar.append(coord)

    for (const item of NAV) {
      const btn = document.createElement("button")
      btn.className = `nav-item${pane === item.id ? " active" : ""}`
      btn.textContent = item.label
      // Pending-decision badge on the Decisions item.
      if (item.id === "decisions" && (node?.approvals?.length ?? 0) > 0) {
        const badge = document.createElement("span")
        badge.className = "nav-badge"
        badge.textContent = String(node!.approvals!.length)
        btn.append(badge)
      }
      btn.addEventListener("click", () => {
        pane = item.id
        renderSidebar()
        renderContent()
      })
      sidebar.append(btn)
    }
  }

  function renderContent(): void {
    content.innerHTML = ""
    const pane_ = document.createElement("div")
    pane_.className = "pane"
    paneRenderer(pane)(pane_, ctx())
    content.append(pane_)
  }

  renderSidebar()
  renderContent()

  return {
    onNodeState(message) {
      node = message
      renderSidebar()
      renderContent()
    },
    onNotifications(view) {
      notifications = view
      // Notifications render inside the Nodes pane; refresh if it's showing.
      if (pane === "nodes") renderContent()
    },
  }
}
