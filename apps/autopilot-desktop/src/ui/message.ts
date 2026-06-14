// CL-53: the Foldkit message set for the Autopilot Desktop webview.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/message.ts):
// each message is a callable tagged struct built with foldkit/message `m`, then
// unioned. Inbound RPC pushes (node-state / notifications) arrive as
// GotNodeState / GotNotifications via the persistent subscription stream.

import { Schema as S } from "effect"
import { m } from "foldkit/message"

import { PaneId, SessionFilter } from "./model"

// ── Inbound (Electrobun → runtime), pushed by the subscription stream ──────
export const GotNodeState = m("GotNodeState", { node: S.Unknown })
export const GotNotifications = m("GotNotifications", { view: S.Unknown })

// ── Navigation ─────────────────────────────────────────────────────────────
export const NavigatedTo = m("NavigatedTo", { pane: PaneId })
export const SelectedSession = m("SelectedSession", { sessionRef: S.String })
export const ChangedSessionFilter = m("ChangedSessionFilter", {
  filter: SessionFilter,
})
export const ToggledEvent = m("ToggledEvent", { eventIndex: S.Number })

// ── Coordinator pause/resume ────────────────────────────────────────────────
export const ClickedCoordinatorToggle = m("ClickedCoordinatorToggle", {
  paused: S.Boolean,
})
export const SettledCoordinatorToggle = m("SettledCoordinatorToggle")
export const FailedCoordinatorToggle = m("FailedCoordinatorToggle")

// ── Approvals (approve / deny) ───────────────────────────────────────────────
export const ClickedResolveApproval = m("ClickedResolveApproval", {
  approvalRef: S.String,
  decision: S.Literals(["approve", "deny"]),
})
export const SettledResolveApproval = m("SettledResolveApproval", {
  approvalRef: S.String,
  ok: S.Boolean,
})

// ── Deploy ───────────────────────────────────────────────────────────────────
export const ClickedDeploy = m("ClickedDeploy")
export const SucceededDeploy = m("SucceededDeploy", {
  state: S.Literals(["queued", "building", "deployed", "failed", "unknown"]),
  text: S.String,
})

// ── Ask Autopilot (submit intent) ────────────────────────────────────────────
export const ChangedAskTitle = m("ChangedAskTitle", { value: S.String })
export const ChangedAskBody = m("ChangedAskBody", { value: S.String })
export const ClickedSubmitIntent = m("ClickedSubmitIntent")
export const SettledSubmitIntent = m("SettledSubmitIntent", {
  ok: S.Boolean,
  text: S.String,
})

// ── Training launch/readiness feedback ──────────────────────────────────────
export const ClickedQueueTrainingLaunch = m("ClickedQueueTrainingLaunch")
export const SettledQueueTrainingLaunch = m("SettledQueueTrainingLaunch", {
  ok: S.Boolean,
  text: S.String,
})

// ── Spawn ──────────────────────────────────────────────────────────────────
export const ChangedSpawnAdapter = m("ChangedSpawnAdapter", {
  adapter: S.Literals(["codex", "claude_agent"]),
})
export const ChangedSpawnObjective = m("ChangedSpawnObjective", {
  value: S.String,
})
export const ChangedSpawnVerify = m("ChangedSpawnVerify", { value: S.String })
export const ClickedSpawn = m("ClickedSpawn")
export const SucceededSpawn = m("SucceededSpawn", { sessionRef: S.String })
export const FailedSpawn = m("FailedSpawn", { error: S.String })

// ── Session detail: cancel ────────────────────────────────────────────────────
export const ClickedCancelSession = m("ClickedCancelSession", {
  sessionRef: S.String,
})
export const SettledCancelSession = m("SettledCancelSession")

export const Message = S.Union([
  GotNodeState,
  GotNotifications,
  NavigatedTo,
  SelectedSession,
  ChangedSessionFilter,
  ToggledEvent,
  ClickedCoordinatorToggle,
  SettledCoordinatorToggle,
  FailedCoordinatorToggle,
  ClickedResolveApproval,
  SettledResolveApproval,
  ClickedDeploy,
  SucceededDeploy,
  ChangedAskTitle,
  ChangedAskBody,
  ClickedSubmitIntent,
  SettledSubmitIntent,
  ClickedQueueTrainingLaunch,
  SettledQueueTrainingLaunch,
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ClickedSpawn,
  SucceededSpawn,
  FailedSpawn,
  ClickedCancelSession,
  SettledCancelSession,
])
export type Message = typeof Message.Type
