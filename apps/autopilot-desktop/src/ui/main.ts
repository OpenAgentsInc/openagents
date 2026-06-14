// CL-53: webview entrypoint (the Electrobun view entry — filename kept as
// main.ts so electrobun.config.ts still resolves it).
//
// Thin bootstrap:
//   1. define the Electroview RPC bridge whose `nodeState` / `notifications`
//      message handlers PUSH GotNodeState / GotNotifications into the Foldkit
//      runtime via the persistent subscription stream (bridge.pushInbound).
//   2. construct the Electroview and stash `rpc.request` in the bridge so
//      Commands can reach the typed verbs.
//   3. run the Foldkit program (Model / init / update / view / subscriptions).
//
// All presentation lives in view.ts; all reducer logic in update.ts; all RPC
// effects in commands.ts. No hand DOM.

import { Electroview } from "electrobun/view"
import { Runtime } from "foldkit"

import type { DesktopRPCSchema } from "../shared/rpc"
import { type DesktopRequests, pushInbound, setRequest } from "./bridge"
import { GotNodeState, GotNotifications } from "./message"
import { initialModel, Model } from "./model"
import { subscriptions } from "./subscriptions"
import { update } from "./update"
import { view } from "./view"

const rpc = Electroview.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {},
    messages: {
      nodeState(message) {
        pushInbound(GotNodeState({ node: message }))
      },
      notifications(view) {
        pushInbound(GotNotifications({ view }))
      },
    },
  },
})

new Electroview({ rpc })

// rpc.request mirrors the DesktopRequests surface (webview → Bun verbs).
setRequest(rpc.request as unknown as DesktopRequests)

function start(): void {
  Runtime.run(
    Runtime.makeProgram({
      Model,
      init: () => [initialModel, []],
      update,
      view,
      subscriptions,
      container: document.getElementById("root"),
    }),
  )
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start)
} else {
  start()
}
