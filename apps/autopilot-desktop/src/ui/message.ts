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
export const ClickedRefreshTrainingRuns = m("ClickedRefreshTrainingRuns")
export const GotTrainingRuns = m("GotTrainingRuns", { projection: S.Unknown })
export const GotTrainingDashboard = m("GotTrainingDashboard", {
  projection: S.Unknown,
})
export const GotTrainingPromiseGates = m("GotTrainingPromiseGates", {
  projection: S.Unknown,
})
export const GotTrainingOperatorReadiness = m(
  "GotTrainingOperatorReadiness",
  { projection: S.Unknown },
)
export const ClickedPlanTrainingWindow = m("ClickedPlanTrainingWindow")
export const SettledPlanTrainingWindow = m("SettledPlanTrainingWindow", {
  projection: S.Unknown,
})
export const ClickedActivateTrainingWindow = m("ClickedActivateTrainingWindow", {
  windowRef: S.String,
})
export const SettledActivateTrainingWindow = m("SettledActivateTrainingWindow", {
  projection: S.Unknown,
})
export const ClickedReconcileTrainingWindow = m("ClickedReconcileTrainingWindow", {
  windowRef: S.String,
})
export const SettledReconcileTrainingWindow = m("SettledReconcileTrainingWindow", {
  projection: S.Unknown,
})
export const ClickedClaimTrainingLease = m("ClickedClaimTrainingLease")
export const SettledClaimTrainingLease = m("SettledClaimTrainingLease", {
  projection: S.Unknown,
})
export const ClickedRequestTrainingBootstrap = m(
  "ClickedRequestTrainingBootstrap",
  { trainingRunRef: S.String },
)
export const SettledRequestTrainingBootstrap = m(
  "SettledRequestTrainingBootstrap",
  { projection: S.Unknown },
)
export const ClickedAdmitTrainingEvidence = m("ClickedAdmitTrainingEvidence", {
  trainingRunRef: S.String,
})
export const SettledAdmitTrainingEvidence = m("SettledAdmitTrainingEvidence", {
  projection: S.Unknown,
})
export const ClickedQueueTrainingLaunch = m("ClickedQueueTrainingLaunch")
export const SettledQueueTrainingLaunch = m("SettledQueueTrainingLaunch", {
  ok: S.Boolean,
  text: S.String,
})
export const ClickedQueueTrainingCloseout = m("ClickedQueueTrainingCloseout", {
  trainingRunRef: S.String,
  windowRef: S.NullOr(S.String),
  leaseRef: S.NullOr(S.String),
  bootstrapGrantRef: S.NullOr(S.String),
})
export const SettledQueueTrainingCloseout = m("SettledQueueTrainingCloseout", {
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
  ClickedRefreshTrainingRuns,
  GotTrainingRuns,
  GotTrainingDashboard,
  GotTrainingPromiseGates,
  GotTrainingOperatorReadiness,
  ClickedPlanTrainingWindow,
  SettledPlanTrainingWindow,
  ClickedActivateTrainingWindow,
  SettledActivateTrainingWindow,
  ClickedReconcileTrainingWindow,
  SettledReconcileTrainingWindow,
  ClickedClaimTrainingLease,
  SettledClaimTrainingLease,
  ClickedRequestTrainingBootstrap,
  SettledRequestTrainingBootstrap,
  ClickedAdmitTrainingEvidence,
  SettledAdmitTrainingEvidence,
  ClickedQueueTrainingLaunch,
  SettledQueueTrainingLaunch,
  ClickedQueueTrainingCloseout,
  SettledQueueTrainingCloseout,
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
