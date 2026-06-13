import { join } from "node:path"
import { BrowserView, BrowserWindow } from "electrobun/bun"
import { createNodeStatePoller } from "./node-state-poll"
import { fetchNodeState, readControlToken } from "./pylon-control"
import type { DesktopRPCSchema } from "../shared/rpc"

const controlBaseUrl = Bun.env.PYLON_CONTROL_BASE_URL ?? "http://127.0.0.1:4716"
const pylonHome = Bun.env.PYLON_HOME ?? join(process.cwd(), ".pylon-local")
const pollIntervalMs = Number(Bun.env.AUTOPILOT_DESKTOP_NODE_POLL_MS ?? "2000")

const rpc = BrowserView.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {},
    messages: {},
  },
})

const window = new BrowserWindow({
  title: "Autopilot Desktop",
  url: "views://autopilot-desktop/index.html",
  rpc,
})

const poller = createNodeStatePoller({
  intervalMs: Number.isFinite(pollIntervalMs) && pollIntervalMs > 0 ? pollIntervalMs : 2000,
  onState(message) {
    rpc.send.nodeState(message)
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
