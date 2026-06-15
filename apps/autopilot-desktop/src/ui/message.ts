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
// #5049: public pylon-network stats push (drives the home network scene).
export const GotPylonStats = m("GotPylonStats", { stats: S.Unknown })
export const GotNotifications = m("GotNotifications", { view: S.Unknown })
// #5025: honest node-launch lifecycle status from the Bun supervisor.
export const GotNodeLaunchStatus = m("GotNodeLaunchStatus", { status: S.String })

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

// ── Built-in no-user-key agent (#5063) ─────────────────────────────────────
export const ClickedRefreshBuiltInAgent = m("ClickedRefreshBuiltInAgent")
export const GotBuiltInAgentReadiness = m("GotBuiltInAgentReadiness", {
  projection: S.Unknown,
})
export const ClickedStartBuiltInAgent = m("ClickedStartBuiltInAgent")
export const SucceededBuiltInAgent = m("SucceededBuiltInAgent", {
  sessionRef: S.String,
})
export const FailedBuiltInAgent = m("FailedBuiltInAgent", { error: S.String })

// ── First-run install/runtime readiness (#5064) ───────────────────────────
export const ClickedRefreshInstallReadiness = m("ClickedRefreshInstallReadiness")
export const GotInstallReadiness = m("GotInstallReadiness", {
  projection: S.Unknown,
})

// ── Product Promises Forum surfacing (#5065) ──────────────────────────────
export const ChangedPromiseSurfacingPromiseId = m(
  "ChangedPromiseSurfacingPromiseId",
  { value: S.String },
)
export const ChangedPromiseSurfacingSurface = m(
  "ChangedPromiseSurfacingSurface",
  { value: S.String },
)
export const ChangedPromiseSurfacingClaimText = m(
  "ChangedPromiseSurfacingClaimText",
  { value: S.String },
)
export const ChangedPromiseSurfacingExpectedBehavior = m(
  "ChangedPromiseSurfacingExpectedBehavior",
  { value: S.String },
)
export const ChangedPromiseSurfacingObservedBehavior = m(
  "ChangedPromiseSurfacingObservedBehavior",
  { value: S.String },
)
export const ChangedPromiseSurfacingEvidenceOrSteps = m(
  "ChangedPromiseSurfacingEvidenceOrSteps",
  { value: S.String },
)
export const ChangedPromiseSurfacingEnvironment = m(
  "ChangedPromiseSurfacingEnvironment",
  { value: S.String },
)
export const ChangedPromiseSurfacingImpact = m(
  "ChangedPromiseSurfacingImpact",
  { value: S.String },
)
export const ChangedPromiseSurfacingSuggestedState = m(
  "ChangedPromiseSurfacingSuggestedState",
  {
    value: S.Literals([
      "green",
      "yellow",
      "red",
      "degraded",
      "planned",
      "unknown",
    ]),
  },
)
export const ClickedRefreshPromiseSurfacing = m(
  "ClickedRefreshPromiseSurfacing",
)
export const GotPromiseSurfacingReadiness = m(
  "GotPromiseSurfacingReadiness",
  { projection: S.Unknown },
)
export const ClickedSurfacePromiseGap = m("ClickedSurfacePromiseGap")
export const GotPromiseSurfacingResult = m("GotPromiseSurfacingResult", {
  projection: S.Unknown,
})

// ── Training launch/readiness feedback ──────────────────────────────────────
export const ClickedRefreshTrainingRuns = m("ClickedRefreshTrainingRuns")
export const SelectedTrainingSceneNode = m("SelectedTrainingSceneNode", {
  nodeId: S.String,
})
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
export const GotTrainingEvidencePacketSummary = m(
  "GotTrainingEvidencePacketSummary",
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
export const ClickedBuildTrainingEvidencePacket = m(
  "ClickedBuildTrainingEvidencePacket",
  { trainingRunRef: S.String },
)
export const SettledBuildTrainingEvidencePacket = m(
  "SettledBuildTrainingEvidencePacket",
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
// #4998: execution-lane selector for the spawn form.
export const ChangedSpawnLane = m("ChangedSpawnLane", {
  lane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
})
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
  GotPylonStats,
  GotNotifications,
  GotNodeLaunchStatus,
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
  ClickedRefreshBuiltInAgent,
  GotBuiltInAgentReadiness,
  ClickedStartBuiltInAgent,
  SucceededBuiltInAgent,
  FailedBuiltInAgent,
  ClickedRefreshInstallReadiness,
  GotInstallReadiness,
  ChangedPromiseSurfacingPromiseId,
  ChangedPromiseSurfacingSurface,
  ChangedPromiseSurfacingClaimText,
  ChangedPromiseSurfacingExpectedBehavior,
  ChangedPromiseSurfacingObservedBehavior,
  ChangedPromiseSurfacingEvidenceOrSteps,
  ChangedPromiseSurfacingEnvironment,
  ChangedPromiseSurfacingImpact,
  ChangedPromiseSurfacingSuggestedState,
  ClickedRefreshPromiseSurfacing,
  GotPromiseSurfacingReadiness,
  ClickedSurfacePromiseGap,
  GotPromiseSurfacingResult,
  ClickedRefreshTrainingRuns,
  SelectedTrainingSceneNode,
  GotTrainingRuns,
  GotTrainingDashboard,
  GotTrainingPromiseGates,
  GotTrainingOperatorReadiness,
  GotTrainingEvidencePacketSummary,
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
  ClickedBuildTrainingEvidencePacket,
  SettledBuildTrainingEvidencePacket,
  ClickedAdmitTrainingEvidence,
  SettledAdmitTrainingEvidence,
  ClickedQueueTrainingLaunch,
  SettledQueueTrainingLaunch,
  ClickedQueueTrainingCloseout,
  SettledQueueTrainingCloseout,
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ChangedSpawnLane,
  ClickedSpawn,
  SucceededSpawn,
  FailedSpawn,
  ClickedCancelSession,
  SettledCancelSession,
])
export type Message = typeof Message.Type
