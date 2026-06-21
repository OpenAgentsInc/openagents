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

import {
  DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type DesktopRPCSchema,
} from "../shared/rpc.js"
import { type DesktopRequests, pushInbound, setRequest } from "./bridge.js"
import { initialRuntimeState } from "./initial-state.js"
import {
  ChangedComposerReply,
  ChangedSpawnObjective,
  ChangedVerseMode,
  ChangedShellInput,
  GotNodeLaunchStatus,
  GotNodeState,
  GotNotifications,
  GotPylonStats,
  OpenedManagedPane,
  SelectedComposerAccount,
  SucceededComposerTurn,
  SubmittedShell,
} from "./message.js"
import type { PaneId } from "./model.js"
import { Model } from "./model.js"
import { subscriptions } from "./subscriptions.js"
import { update } from "./update.js"
import { view } from "./view.js"

declare global {
  interface Window {
    __OA_ENABLE_DESKTOP_SMOKE_HOOK?: boolean
    __OA_DESKTOP_SMOKE__?: Readonly<{
      enterCodeMode: () => void
      exitCodeMode: () => void
      openPane: (pane: PaneId) => boolean
      setComposerSession: (sessionRef: string) => void
      selectComposerAccount: (accountRef: string | null) => void
      setComposerObjective: (value: string) => void
      setComposerReply: (value: string) => void
      pushNodeState: (node: unknown) => void
      setNodeLaunchStatus: (status: string) => void
    }>
  }
}

// Dev error boundary: when a render/update crashes, Foldkit's `crash.view`
// replaces the (otherwise blank) screen with the error + stack so failures are
// visible instead of a white window. This is a local operator tool, so we always
// show the stack; gate on a prod flag here later if the app ever ships hardened.
const ch = html<never>()
// Must return a Document ({ title, body }) — the runtime renders `.body`. A bare
// Html here left the crash overlay's body undefined (blank crash screen).
const crashView = (error: Error): Document => ({
  title: "Autopilot — error",
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
        ["⚠ Autopilot — render error"],
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
  maxRequestTime: DESKTOP_RPC_MAX_REQUEST_TIME_MS,
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
      // ZERO-BASE SHELL programmatic control: a Bun→webview push drives the
      // shell's text bar through the SAME inbound messages the UI dispatches, so
      // an operator / headless control path can set the input and submit it and
      // a driver sees exactly the state the owner sees (model.shellTurns).
      shellControl(message) {
        if (message.action === "set-input") {
          pushInbound(ChangedShellInput({ value: message.value ?? "" }))
        } else {
          pushInbound(SubmittedShell())
        }
      },
    },
  },
})

new Electroview({ rpc })

// rpc.request mirrors the DesktopRequests surface (webview → Bun verbs).
setRequest(rpc.request as unknown as DesktopRequests)

const smokePaneIds = new Set<PaneId>([
  "agent-stream",
  "accounts",
  "composer",
  "decisions",
  "diagnostics",
  "diff-artifacts",
  "session-detail",
  "sessions",
  "swarm",
  "terminal-log",
])

const smokeHookEnabled = (): boolean =>
  window.__OA_ENABLE_DESKTOP_SMOKE_HOOK === true ||
  new URLSearchParams(window.location.search).has("__oa_desktop_smoke")

if (smokeHookEnabled()) {
  window.__OA_DESKTOP_SMOKE__ = Object.freeze({
    enterCodeMode: () => pushInbound(ChangedVerseMode({ mode: "code" })),
    exitCodeMode: () => pushInbound(ChangedVerseMode({ mode: "explore" })),
    openPane: (pane) => {
      if (!smokePaneIds.has(pane)) return false
      pushInbound(OpenedManagedPane({ pane }))
      return true
    },
    setComposerSession: (sessionRef) =>
      pushInbound(SucceededComposerTurn({ sessionRef })),
    selectComposerAccount: (accountRef) =>
      pushInbound(SelectedComposerAccount({ accountRef })),
    setComposerObjective: (value) =>
      pushInbound(ChangedSpawnObjective({ value })),
    setComposerReply: (value) =>
      pushInbound(ChangedComposerReply({ value })),
    pushNodeState: (node) => pushInbound(GotNodeState({ node })),
    setNodeLaunchStatus: (status) =>
      pushInbound(GotNodeLaunchStatus({ status })),
  })
}

// External links MUST open in the system browser — never navigate this webview.
// A raw `<a href="https://…">` click would otherwise load the external page
// INSIDE the app, stranding the user off the local UI (e.g. on github.com) with
// no way back (the app's keyboard + "← Shell" handlers no longer run once the
// webview has navigated away). Intercept every external anchor click and route
// it to the Bun `openExternal` verb instead. Capture phase so it wins before any
// default navigation; only http(s) is intercepted (in-app `views://` etc. pass
// through untouched).
document.addEventListener(
  "click",
  (event) => {
    const anchor = (event.target as HTMLElement | null)?.closest?.("a[href]") as
      | HTMLAnchorElement
      | null
    if (!anchor) return
    const href = anchor.getAttribute("href") ?? ""
    if (/^https?:\/\//i.test(href)) {
      event.preventDefault()
      void (rpc.request as unknown as DesktopRequests).openExternal({ url: href })
    }
  },
  true,
)

function start(): void {
  Runtime.run(
    Runtime.makeProgram({
      Model,
      init: initialRuntimeState,
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
