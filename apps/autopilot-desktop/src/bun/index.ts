import { join } from "node:path"
import { BrowserView, BrowserWindow } from "electrobun/bun"
import { createNodeStatePoller } from "./node-state-poll"
import { createSessionNotifier } from "./notifier"
import { raiseOsNotification } from "./os-notification"
import { deployToCloud, fetchNodeState, readControlToken } from "./pylon-control"
import type { DesktopRPCSchema } from "../shared/rpc"

const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pylonHome = Bun.env.PYLON_HOME ?? join(process.cwd(), ".pylon-local")
const pollIntervalMs = Number(Bun.env.AUTOPILOT_DESKTOP_NODE_POLL_MS ?? "2000")

const rpc = BrowserView.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {
      // CL-26: the webview's "Deploy to Cloud" button routes here. We read the
      // node's control token and forward the gated deploy.cloud command — the
      // node enforces the OA_DEPLOY_ENABLE=1 fail-safe, so nothing deploys by
      // default.
      async deployCloud(params) {
        const token = readControlToken(pylonHome)
        if (token === null) {
          return { accepted: false, reason: "control token unavailable", errors: [] }
        }
        return deployToCloud({
          baseUrl: controlBaseUrl,
          token,
          target: params.target,
          ref: params.ref,
          ...(params.env ? { env: params.env } : {}),
        })
      },
    },
    messages: {},
  },
})

const window = new BrowserWindow({
  title: "Autopilot Desktop",
  url: "views://autopilot-desktop/index.html",
  rpc,
})

// CL-30: each poll, fold the session list into the notifier so a session that
// newly enters a notify-worthy state (needs_decision / failed / completed)
// raises a native OS notification and updates the in-app notification center.
const notifier = createSessionNotifier({ raise: raiseOsNotification })

const poller = createNodeStatePoller({
  intervalMs: Number.isFinite(pollIntervalMs) && pollIntervalMs > 0 ? pollIntervalMs : 2000,
  onState(message) {
    rpc.send.nodeState(message)
    rpc.send.notifications(notifier.ingest(message.sessions))
  },
  async fetchNodeState() {
    const token = readControlToken(pylonHome)
    if (token === null) throw new Error("Pylon control token is not available")
    return fetchNodeState({
      baseUrl: controlBaseUrl,
      token,
    })
  },
})

window.webview.on("dom-ready", () => {
  poller.start()
})

window.on("close", () => {
  poller.stop()
})
