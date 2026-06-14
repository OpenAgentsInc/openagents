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
        // CL-26: refresh the deploy status line from the latest projection,
        // unless a tap is currently showing a transient line.
        if (message.deploy && !deployBusy) renderDeployStatus(message.deploy.state, `${message.deploy.state} · ${message.deploy.message}`)
      },
      notifications(view) {
        latestNotifications = view
        const panel = document.querySelector<HTMLElement>("#notifications")
        if (panel !== null) renderNotifications(panel, view)
      },
    },
  },
})

// CL-26: while a deploy tap is in flight (or just resolved), keep the transient
// line visible instead of letting the next nodeState poll overwrite it.
let deployBusy = false

// CL-26: paint the deploy status line with a state-tinted class.
function renderDeployStatus(
  state: "queued" | "building" | "deployed" | "failed" | "unknown",
  text: string,
): void {
  const line = document.querySelector<HTMLElement>("#deploy .deploy-status")
  if (line === null) return
  line.className = `deploy-status deploy-${state}`
  line.textContent = text
}

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

  // CL-26 "Deploy to Cloud": a button that triggers a deploy of the node's own
  // Cloud Run service (cloudrun · main · production) through our pipeline, plus
  // a status line. The node fail-safe-gates execution behind OA_DEPLOY_ENABLE=1.
  const deploy = document.createElement("section")
  deploy.id = "deploy"
  const dh = document.createElement("h2")
  dh.textContent = "Deploy to Cloud"
  deploy.append(dh)
  const dHelp = document.createElement("p")
  dHelp.className = "deploy-help"
  dHelp.textContent =
    "Deploy this node's Cloud Run service (cloudrun · main · production) through our pipeline. Disabled unless the node has OA_DEPLOY_ENABLE=1."
  deploy.append(dHelp)
  const dBtn = document.createElement("button")
  dBtn.textContent = "Deploy to Cloud"
  const dStatus = document.createElement("p")
  dStatus.className = "deploy-status deploy-unknown"
  dStatus.textContent = latestNodeState?.deploy
    ? `${latestNodeState.deploy.state} · ${latestNodeState.deploy.message}`
    : "no deploy yet"
  dBtn.addEventListener("click", () => {
    deployBusy = true
    dBtn.disabled = true
    renderDeployStatus("queued", "deploying…")
    void rpc.request
      .deployCloud({ target: "cloudrun", ref: "main", env: "production" })
      .then((r) => {
        if (r.accepted) {
          renderDeployStatus("queued", "queued · cloudrun · main")
        } else if (r.reason === "deploy_disabled") {
          renderDeployStatus("unknown", "disabled (set OA_DEPLOY_ENABLE=1 on the node)")
        } else {
          renderDeployStatus("failed", `not accepted: ${r.errors[0] ?? r.reason}`)
        }
      })
      .catch((e: unknown) => {
        renderDeployStatus("failed", `error: ${e instanceof Error ? e.message : String(e)}`)
      })
      .finally(() => {
        dBtn.disabled = false
        deployBusy = false
      })
  })
  deploy.append(dBtn, dStatus)
  root.append(deploy)

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
