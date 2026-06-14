import { BrowserView, BrowserWindow } from "electrobun/bun"
import { discoverPylonHome } from "./node-home"
import { createNodeStatePoller } from "./node-state-poll"
import { createSessionNotifier } from "./notifier"
import { raiseOsNotification } from "./os-notification"
import {
  cancelSession,
  deployToCloud,
  fetchNodeState,
  readControlToken,
  resolveApproval,
  setCoordinatorPaused,
  spawnSession,
  submitIntent,
} from "./pylon-control"
import { fetchTrainingRuns, planTrainingRunWindow } from "./training-runs"
import type { DesktopRPCSchema } from "../shared/rpc"

const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pollIntervalMs = Number(Bun.env.AUTOPILOT_DESKTOP_NODE_POLL_MS ?? "2000")
const trainingBaseUrl =
  Bun.env.OPENAGENTS_TRAINING_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  "https://openagents.com"
const trainingAdminToken =
  Bun.env.OPENAGENTS_TRAINING_ADMIN_API_TOKEN ??
  Bun.env.OPENAGENTS_ADMIN_API_TOKEN ??
  null
const trainingAdminEnabled =
  Bun.env.OPENAGENTS_DESKTOP_TRAINING_ADMIN_ENABLE === "1"

// CL-45: resolve the node home once per call so a node that starts after the app
// (or rotates its home) is picked up without a restart. Falls back to the env
// default. `readControlToken` returns null when the chosen home has no token,
// which the poll surfaces as an honest offline state.
function resolveHome(): string | null {
  return discoverPylonHome({ env: Bun.env.PYLON_HOME, cwd: process.cwd() })
}

function tokenForCommand(): string | null {
  const home = resolveHome()
  return home === null ? null : readControlToken(home)
}

const rpc = BrowserView.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {
      // CL-26: the webview's "Deploy to Cloud" button routes here. We read the
      // node's control token and forward the gated deploy.cloud command — the
      // node enforces the OA_DEPLOY_ENABLE=1 fail-safe, so nothing deploys by
      // default.
      async deployCloud(params) {
        const token = tokenForCommand()
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
      // CL-47: submit an "ask" (work intent) to the node.
      async submitIntent(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, status: "error", error: "control token unavailable" }
        return submitIntent({ baseUrl: controlBaseUrl, token, title: params.title, body: params.body })
      },
      async listTrainingRuns() {
        return fetchTrainingRuns({ baseUrl: trainingBaseUrl })
      },
      async planTrainingRunWindow() {
        return planTrainingRunWindow({
          adminToken: trainingAdminToken,
          baseUrl: trainingBaseUrl,
          enabled: trainingAdminEnabled,
        })
      },
      // CL-48: resolve a pending approval (approve/deny).
      async resolveApproval(params) {
        const token = tokenForCommand()
        if (token === null) return { applied: false, duplicate: false, decision: params.decision }
        return resolveApproval({ baseUrl: controlBaseUrl, token, approvalRef: params.approvalRef, decision: params.decision })
      },
      // CL-51: pause/resume the node's autonomous coordinator loop.
      async setCoordinatorPaused(params) {
        const token = tokenForCommand()
        if (token === null) return { paused: params.paused }
        return setCoordinatorPaused({ baseUrl: controlBaseUrl, token, paused: params.paused })
      },
      // CL-52: cancel a running/queued session.
      async cancelSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, state: "error" }
        return cancelSession({ baseUrl: controlBaseUrl, token, sessionRef: params.sessionRef })
      },
      // CL-57: directly spawn a bounded session on the node.
      async spawnSession(params) {
        const token = tokenForCommand()
        if (token === null) return { ok: false, sessionRef: "", error: "control token unavailable" }
        return spawnSession({
          baseUrl: controlBaseUrl,
          token,
          adapter: params.adapter,
          objective: params.objective,
          ...(params.verify ? { verify: params.verify } : {}),
        })
      },
    },
    messages: {},
  },
})

const window = new BrowserWindow({
  title: "Autopilot Desktop",
  url: "views://autopilot-desktop/index.html",
  // Open large by default (Electrobun defaults to 800×600, too small for the
  // sidebar + dashboard). A roomy 1400×900 window with a small offset.
  frame: { x: 80, y: 60, width: 1400, height: 900 },
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
    const token = tokenForCommand()
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
