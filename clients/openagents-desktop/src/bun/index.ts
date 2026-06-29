import { BrowserView, BrowserWindow } from "electrobun/bun"

import {
  fetchPylonStatus,
  OPENAGENTS_DESKTOP_DEFAULT_BASE_URL,
} from "../shared/pylon-status.js"
import {
  OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type OpenAgentsDesktopRPCSchema,
} from "../shared/rpc.js"

const baseUrl =
  Bun.env.PYLON_OPENAGENTS_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  OPENAGENTS_DESKTOP_DEFAULT_BASE_URL

const rpc = BrowserView.defineRPC<OpenAgentsDesktopRPCSchema>({
  maxRequestTime: OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {
      async pylonStatus() {
        return fetchPylonStatus({
          baseUrl,
          token: Bun.env.OPENAGENTS_AGENT_TOKEN ?? null,
        })
      },
    },
    messages: {},
  },
})

new BrowserWindow({
  title: "OpenAgents",
  url: "views://openagents-desktop/index.html",
  frame: { x: 128, y: 96, width: 1024, height: 720 },
  rpc,
})
