// CL-44: webview entrypoint. Thin bootstrap — define the typed RPC bridge to the
// Bun main process, mount the sidebar shell, and feed it the live node-state /
// notification messages. All presentation lives in shell.ts + panes/ + cards/.

import { Electroview } from "electrobun/view"
import type { DesktopRequests } from "./context"
import { mountShell, type Shell } from "./shell"
import type { DesktopRPCSchema } from "../shared/rpc"

let shell: Shell | null = null

const rpc = Electroview.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {},
    messages: {
      nodeState(message) {
        shell?.onNodeState(message)
      },
      notifications(view) {
        shell?.onNotifications(view)
      },
    },
  },
})

new Electroview({ rpc })

function start(): void {
  // rpc.request matches the DesktopRequests surface (webview → Bun verbs).
  shell = mountShell(rpc.request as unknown as DesktopRequests)
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start)
} else {
  start()
}
