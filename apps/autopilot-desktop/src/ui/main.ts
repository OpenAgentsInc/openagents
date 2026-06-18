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
import type { Document } from "foldkit/html"
import { html } from "foldkit/html"

import type { DesktopRPCSchema } from "../shared/rpc"
import { type DesktopRequests, pushInbound, setRequest } from "./bridge"
import { LoadInstallReadiness } from "./commands"
import {
  GotNodeLaunchStatus,
  GotNodeState,
  GotNotifications,
  GotPylonStats,
} from "./message"
import { initialModel, Model } from "./model"
import { subscriptions } from "./subscriptions"
import { update } from "./update"
import { view } from "./view"

// Dev error boundary: when a render/update crashes, Foldkit's `crash.view`
// replaces the (otherwise blank) screen with the error + stack so failures are
// visible instead of a white window. This is a local operator tool, so we always
// show the stack; gate on a prod flag here later if the app ever ships hardened.
const ch = html<never>()
// Must return a Document ({ title, body }) — the runtime renders `.body`. A bare
// Html here left the crash overlay's body undefined (blank crash screen).
const crashView = (error: Error): Document => ({
  title: "Autopilot Desktop — error",
  body: ch.div(
    [
      ch.Style({
        background: "#0b0d12",
        color: "#e6e9ef",
        fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace",
        fontSize: "13px",
        inset: "0",
        lineHeight: "1.5",
        overflow: "auto",
        padding: "24px",
        position: "fixed",
        zIndex: "99999",
      }),
    ],
    [
      ch.h1(
        [ch.Style({ color: "#ff6b6b", fontSize: "16px", margin: "0 0 8px" })],
        ["⚠ Autopilot Desktop — render error"],
      ),
      ch.p(
        [ch.Style({ color: "#8b93a7", margin: "0 0 12px" })],
        ["The webview hit an unrecoverable error. Details below (dev build)."],
      ),
      ch.pre(
        [
          ch.Style({
            color: "#ffb454",
            fontWeight: "600",
            margin: "0",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }),
        ],
        [error.message],
      ),
      ch.pre(
        [
          ch.Style({
            background: "#11151d",
            border: "1px solid #1c2230",
            borderRadius: "6px",
            color: "#cdd3e0",
            margin: "12px 0 0",
            padding: "12px",
            whiteSpace: "pre-wrap",
            wordBreak: "break-word",
          }),
        ],
        [error.stack ?? "(no stack)"],
      ),
    ],
  ),
})

const rpc = Electroview.defineRPC<DesktopRPCSchema>({
  handlers: {
    requests: {},
    messages: {
      nodeState(message) {
        pushInbound(GotNodeState({ node: message }))
      },
      pylonStats(message) {
        pushInbound(GotPylonStats({ stats: message }))
      },
      notifications(view) {
        pushInbound(GotNotifications({ view }))
      },
      nodeLaunchStatus(message) {
        pushInbound(GotNodeLaunchStatus({ status: message.status }))
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
      init: () => [initialModel, [LoadInstallReadiness()]],
      update,
      view,
      subscriptions,
      container: document.getElementById("root"),
      crash: {
        view: ({ error }) => crashView(error),
        report: ({ error }) => console.error("[autopilot-desktop] crash:", error),
      },
    }),
  )
}

if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", start)
} else {
  start()
}
