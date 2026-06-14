// The view script loads in <head>, so render after the DOM is ready.
import {
  CONTROL_SCHEMA_TAG,
  type NotificationCenterView,
} from "@openagentsinc/autopilot-control-protocol"
import { Electroview } from "electrobun/view"
import type { DesktopRPCSchema, NodeStateMessage } from "../shared/rpc"
import { renderNotifications } from "./notification-view"
import { renderSessions } from "./session-render"

let latestNodeState: NodeStateMessage | null = null
let latestNotifications: NotificationCenterView | null = null

const rpc = Electroview.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {},
    messages: {
      nodeState(message) {
        latestNodeState = message
        const sessions = document.querySelector<HTMLElement>("#sessions")
        if (sessions !== null) renderSessions(sessions, message)
      },
      notifications(view) {
        latestNotifications = view
        const panel = document.querySelector<HTMLElement>("#notifications")
        if (panel !== null) renderNotifications(panel, view)
      },
    },
  },
})

function render(): void {
  const root = document.createElement("main")
  const h = document.createElement("h1")
  h.textContent = "🛩️  Autopilot Desktop"
  root.append(h)
  const a = document.createElement("p")
  a.textContent = "Electrobun + Bun + (Foldkit next) — the desktop client shell is alive."
  root.append(a)
  const b = document.createElement("p")
  b.innerHTML = `shared protocol: <code>${CONTROL_SCHEMA_TAG}</code>`
  root.append(b)
  const c = document.createElement("p")
  c.textContent = "Next (CL-5): connect to the local Pylon node over loopback and render live sessions."
  root.append(c)
  const notifications = document.createElement("section")
  notifications.id = "notifications"
  if (latestNotifications !== null) renderNotifications(notifications, latestNotifications)
  root.append(notifications)
  const sessions = document.createElement("section")
  sessions.id = "sessions"
  if (latestNodeState !== null) renderSessions(sessions, latestNodeState)
  root.append(sessions)
  document.body.append(root)
}

new Electroview({ rpc })

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", render)
} else {
  render()
}
