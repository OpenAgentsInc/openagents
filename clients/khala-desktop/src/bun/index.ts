import { BrowserView, BrowserWindow } from "electrobun/bun"
import { Effect } from "effect"

import { AppleFmSidecarManager } from "./apple-fm-sidecar.js"
import {
  fetchOperatorDashboard,
  KHALA_OPERATOR_DEFAULT_BASE_URL,
} from "../shared/operator-dashboard.js"
import {
  KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type KhalaDesktopRPCSchema,
} from "../shared/rpc.js"

const baseUrl =
  Bun.env.PYLON_OPENAGENTS_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  KHALA_OPERATOR_DEFAULT_BASE_URL
const appleFmSidecar = new AppleFmSidecarManager()

const rpc = BrowserView.defineRPC<KhalaDesktopRPCSchema>({
  maxRequestTime: KHALA_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {
      async operatorDashboard() {
        return Effect.runPromise(
          fetchOperatorDashboard({
            baseUrl,
            token: Bun.env.OPENAGENTS_AGENT_TOKEN ?? null,
          }),
        )
      },
      async appleFmSidecarStatus() {
        return appleFmSidecar.status()
      },
      async openExternal({ url }) {
        if (/^https?:\/\//i.test(url)) {
          try {
            Bun.spawn(["open", url], { stderr: "ignore", stdout: "ignore" })
          } catch {
            // Best-effort only; never break dashboard rendering.
          }
        }
        return { ok: true }
      },
    },
    messages: {},
  },
})

new BrowserWindow({
  title: "Khala Fleet",
  url: "views://khala-desktop/index.html",
  frame: { x: 96, y: 72, width: 1280, height: 820 },
  rpc,
})

process.on("beforeExit", () => {
  appleFmSidecar.stop()
})
