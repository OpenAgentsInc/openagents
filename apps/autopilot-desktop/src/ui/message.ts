// CL-53: the Foldkit message set for the Autopilot Desktop webview.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/message.ts):
// each message is a callable tagged struct built with foldkit/message `m`, then
// unioned. Inbound RPC pushes (node-state / notifications) arrive as
// GotNodeState / GotNotifications via the persistent subscription stream.

import { Schema as S } from "effect"
import { m } from "foldkit/message"

import { PaneId, ProofReplaySlug, SessionFilter } from "./model"

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
export const SelectedAgentMode = m("SelectedAgentMode", {
  mode: S.Literals(["hosted", "local-apple-fm"]),
})
export const ClickedRefreshAppleFm = m("ClickedRefreshAppleFm")
export const GotAppleFmReadiness = m("GotAppleFmReadiness", {
  projection: S.Unknown,
})
export const ClickedStartAppleFm = m("ClickedStartAppleFm")
export const SucceededAppleFmSession = m("SucceededAppleFmSession", {
  sessionRef: S.String,
})
export const FailedAppleFmSession = m("FailedAppleFmSession", { error: S.String })
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
export const SelectedProofReplay = m("SelectedProofReplay", {
  slug: ProofReplaySlug,
})
export const ClickedRefreshProofReplay = m("ClickedRefreshProofReplay")
export const GotProofReplayBundle = m("GotProofReplayBundle", {
  projection: S.Unknown,
})

// ── Spawn ──────────────────────────────────────────────────────────────────
// CS-A1: `apple_fm` joins the runtime toggle as a spawn-adapter option.
export const ChangedSpawnAdapter = m("ChangedSpawnAdapter", {
  adapter: S.Literals(["codex", "claude_agent", "apple_fm"]),
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

// ── #5355: coding composer (the day-to-day CLI replacement loop) ────────────
// The composer reuses ChangedSpawnAdapter/Objective/Verify/Lane for the first
// turn's form fields, and adds repo-path + reply-turn inputs plus its own
// spawn/reply/new-thread verbs so the iterative loop stays a pure reducer.
export const ChangedComposerRepoPath = m("ChangedComposerRepoPath", {
  value: S.String,
})
export const ChangedComposerReply = m("ChangedComposerReply", {
  value: S.String,
})
// First coding turn: spawn a fresh session from the composer objective.
export const ClickedComposerSpawn = m("ClickedComposerSpawn")
// Follow-up turn: continue the active composer thread with a new objective
// (a continuation session.spawn carrying the prior turn context — no new verb).
export const ClickedComposerReply = m("ClickedComposerReply")
// Start a brand-new composer thread (clear the active session + turn history).
export const ClickedComposerNewThread = m("ClickedComposerNewThread")
// Composer turn settled (shared by first-turn + reply-turn spawns).
export const SucceededComposerTurn = m("SucceededComposerTurn", {
  sessionRef: S.String,
})
export const FailedComposerTurn = m("FailedComposerTurn", { error: S.String })

// ── CS-A1: per-session account picker + multi-account management ────────────
// Which provider account the composer's coding turns run under (null = default).
export const SelectedComposerAccount = m("SelectedComposerAccount", {
  accountRef: S.NullOr(S.String),
})
// Account-management surface (add/select/priority over the node's local
// dev.accounts config the runtime reads). Bun owns the home + config path.
export const ClickedRefreshManagedAccounts = m("ClickedRefreshManagedAccounts")
export const GotManagedAccounts = m("GotManagedAccounts", { projection: S.Unknown })
export const ChangedAddAccountRef = m("ChangedAddAccountRef", { value: S.String })
export const ChangedAddAccountProvider = m("ChangedAddAccountProvider", {
  provider: S.Literals(["codex", "claude_agent"]),
})
export const ChangedAddAccountHome = m("ChangedAddAccountHome", { value: S.String })
export const ChangedAddAccountPriority = m("ChangedAddAccountPriority", {
  value: S.String,
})
export const ClickedAddManagedAccount = m("ClickedAddManagedAccount")
export const ClickedRemoveManagedAccount = m("ClickedRemoveManagedAccount", {
  ref: S.String,
  provider: S.Literals(["codex", "claude_agent"]),
})
export const ClickedBumpManagedAccountPriority = m(
  "ClickedBumpManagedAccountPriority",
  {
    ref: S.String,
    provider: S.Literals(["codex", "claude_agent"]),
    priority: S.Number,
  },
)
// Settled (shared by add/remove/set-priority). Carries the refreshed list.
export const SettledManagedAccountMutation = m("SettledManagedAccountMutation", {
  projection: S.Unknown,
})

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
  SelectedAgentMode,
  ClickedRefreshAppleFm,
  GotAppleFmReadiness,
  ClickedStartAppleFm,
  SucceededAppleFmSession,
  FailedAppleFmSession,
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
  SelectedProofReplay,
  ClickedRefreshProofReplay,
  GotProofReplayBundle,
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ChangedSpawnLane,
  ClickedSpawn,
  SucceededSpawn,
  FailedSpawn,
  ClickedCancelSession,
  SettledCancelSession,
  ChangedComposerRepoPath,
  ChangedComposerReply,
  ClickedComposerSpawn,
  ClickedComposerReply,
  ClickedComposerNewThread,
  SucceededComposerTurn,
  FailedComposerTurn,
  SelectedComposerAccount,
  ClickedRefreshManagedAccounts,
  GotManagedAccounts,
  ChangedAddAccountRef,
  ChangedAddAccountProvider,
  ChangedAddAccountHome,
  ChangedAddAccountPriority,
  ClickedAddManagedAccount,
  ClickedRemoveManagedAccount,
  ClickedBumpManagedAccountPriority,
  SettledManagedAccountMutation,
])
export type Message = typeof Message.Type
