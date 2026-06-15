// CL-53: the Foldkit (Effect TEA) Model for the Autopilot Desktop webview.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/page/demo/model.ts):
// a tagged-struct Model built with foldkit/schema `ts`, holding opaque externally
// supplied projections (the live node-state + notification-center views) as
// `S.Unknown`. Those projections are produced and validated on the Bun side and
// pushed in over the typed RPC; the webview treats them as read-only data, so
// `S.Unknown` (decoded back to the TS row types via typed accessors) matches the
// web app's treatment of externally-owned payloads.

import { Schema as S } from "effect"
import { ts } from "foldkit/schema"

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene"
import type {
  BuiltInAgentReadinessResponse,
  NodeStateMessage,
  TrainingBootstrapGrantResponse,
  TrainingDashboardSummaryResponse,
  TrainingEvidenceAdmissionResponse,
  TrainingEvidencePacketBuildResponse,
  TrainingEvidencePacketSummaryResponse,
  TrainingOperatorReadinessResponse,
  TrainingPlanResponse,
  TrainingPromiseGatesResponse,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowLeaseResponse,
} from "../shared/rpc"

// Which content pane is showing. The desktop equivalent of mobile's tab set
// plus the focused session-detail leaf.
export const PaneId = S.Literals([
  "network",
  "builtin-agent",
  "nodes",
  "training",
  "training-fullscreen",
  "sessions",
  "decisions",
  "spawn",
  "settings",
  "session-detail",
])
export type PaneId = typeof PaneId.Type

// The Sessions-pane state filter ("all" is the catch-all bucket).
export const SessionFilter = S.Literals([
  "all",
  "running",
  "queued",
  "completed",
  "failed",
  "cancelled",
])
export type SessionFilter = typeof SessionFilter.Type

// Transient status for the Spawn form (validation/submit feedback). Kept in the
// Model so the view stays a pure function of state (no hidden DOM).
export const SpawnStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "idle"]),
})
export type SpawnStatus = typeof SpawnStatus.Type

// Transient status for the Ask-Autopilot card.
export const AskStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type AskStatus = typeof AskStatus.Type

export const BuiltInAgentStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type BuiltInAgentStatus = typeof BuiltInAgentStatus.Type

// Transient status for queueing a training launch/readiness check through the
// existing local Pylon intent bridge. The desktop webview never receives admin
// training authority or secrets.
export const TrainingLaunchStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type TrainingLaunchStatus = typeof TrainingLaunchStatus.Type

export const TrainingRunsStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type TrainingRunsStatus = typeof TrainingRunsStatus.Type

export const TrainingPlanStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type TrainingPlanStatus = typeof TrainingPlanStatus.Type

export const TrainingWindowActionStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type TrainingWindowActionStatus = typeof TrainingWindowActionStatus.Type

// Transient status for the Deploy card.
export const DeployFeedback = S.Struct({
  state: S.Literals(["queued", "building", "deployed", "failed", "unknown"]),
  text: S.String,
})
export type DeployFeedback = typeof DeployFeedback.Type

export const Model = ts("AutopilotDesktop", {
  // Latest projections (None until the first poll lands). Stored opaque; read
  // through the typed accessors below.
  node: S.NullOr(S.Unknown),
  notifications: S.NullOr(S.Unknown),

  // #5049: latest public pylon-network stats (GET /api/public/pylon-stats),
  // pushed from the Bun poller. Opaque PylonStatsSnapshot; projected to the
  // home scene via projectPylonNetworkScene. Null until the first poll lands.
  pylonStats: S.NullOr(S.Unknown),

  // #5063: public-safe readiness for the no-user-key built-in agent. Bun keeps
  // OpenAgents compute credentials; the webview sees only bounds/status refs.
  builtInAgentReadiness: S.NullOr(S.Unknown),
  builtInAgentStatus: BuiltInAgentStatus,
  builtInAgentPending: S.Boolean,

  // #5025: honest node-launch lifecycle status from the Bun supervisor
  // (launching/online/adopted/failed/unavailable). Null until the first status
  // message arrives.
  nodeLaunchStatus: S.NullOr(S.String),

  // Navigation.
  pane: PaneId,
  selectedSessionRef: S.NullOr(S.String),
  selectedTrainingSceneNodeId: S.NullOr(S.String),

  // Sessions-pane filter.
  sessionFilter: SessionFilter,

  // Session-detail: which event indices are expanded (click-to-expand).
  expandedEvents: S.Array(S.Number),

  // Approvals optimistically resolved this session (hidden until the next poll
  // confirms). Keyed by approvalRef.
  resolvedApprovals: S.Array(S.String),

  // Spawn form fields + transient status.
  spawnAdapter: S.Literals(["codex", "claude_agent"]),
  spawnObjective: S.String,
  spawnVerify: S.String,
  // #4998: requested execution lane for the spawn form. Default `auto`
  // (own-Pylon-first then cloud-gcp); cloud-gcp = Google GCE, cloud-shc = SHC.
  spawnLane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
  spawnStatus: SpawnStatus,
  spawnPending: S.Boolean,

  // Ask-Autopilot form fields + transient status.
  askTitle: S.String,
  askBody: S.String,
  askStatus: AskStatus,
  askPending: S.Boolean,

  // Training pane launch/readiness feedback.
  trainingRuns: S.NullOr(S.Unknown),
  trainingRunsStatus: TrainingRunsStatus,
  trainingRunsPending: S.Boolean,
  trainingDashboard: S.NullOr(S.Unknown),
  trainingDashboardStatus: TrainingRunsStatus,
  trainingDashboardPending: S.Boolean,
  trainingPromiseGates: S.NullOr(S.Unknown),
  trainingPromiseGatesStatus: TrainingRunsStatus,
  trainingPromiseGatesPending: S.Boolean,
  trainingOperatorReadiness: S.NullOr(S.Unknown),
  trainingOperatorReadinessStatus: TrainingRunsStatus,
  trainingOperatorReadinessPending: S.Boolean,
  trainingEvidencePacketSummary: S.NullOr(S.Unknown),
  trainingEvidencePacketSummaryStatus: TrainingRunsStatus,
  trainingEvidencePacketSummaryPending: S.Boolean,
  trainingPlan: S.NullOr(S.Unknown),
  trainingPlanStatus: TrainingPlanStatus,
  trainingPlanPending: S.Boolean,
  trainingPlanFirstObservedAt: S.NullOr(S.String),
  trainingActivation: S.NullOr(S.Unknown),
  trainingActivationStatus: TrainingWindowActionStatus,
  trainingActivationPending: S.Boolean,
  trainingReconcile: S.NullOr(S.Unknown),
  trainingReconcileStatus: TrainingWindowActionStatus,
  trainingReconcilePending: S.Boolean,
  trainingLease: S.NullOr(S.Unknown),
  trainingLeaseStatus: TrainingWindowActionStatus,
  trainingLeasePending: S.Boolean,
  trainingBootstrap: S.NullOr(S.Unknown),
  trainingBootstrapStatus: TrainingWindowActionStatus,
  trainingBootstrapPending: S.Boolean,
  trainingEvidencePacketBuild: S.NullOr(S.Unknown),
  trainingEvidencePacketBuildStatus: TrainingWindowActionStatus,
  trainingEvidencePacketBuildPending: S.Boolean,
  trainingEvidenceAdmission: S.NullOr(S.Unknown),
  trainingEvidenceAdmissionStatus: TrainingWindowActionStatus,
  trainingEvidenceAdmissionPending: S.Boolean,
  trainingLaunchStatus: TrainingLaunchStatus,
  trainingLaunchPending: S.Boolean,
  trainingCloseoutStatus: TrainingLaunchStatus,
  trainingCloseoutPending: S.Boolean,

  // Deploy feedback (null until the card has been interacted with or a deploy
  // projection has landed).
  deployFeedback: S.NullOr(DeployFeedback),
})
export type Model = typeof Model.Type

// ── Typed accessors over the opaque projection fields ──────────────────────
//
// The Model carries `node`/`notifications` as `S.Unknown` so makeProgram's
// Schema.Codec stays happy with externally-owned payloads; these helpers re-narrow
// them at the single read boundary so the rest of the app is typed.

export const modelNode = (model: Model): NodeStateMessage | null =>
  model.node as NodeStateMessage | null

export const modelPylonStats = (model: Model): PylonStatsSnapshot | null =>
  model.pylonStats as PylonStatsSnapshot | null

export const modelBuiltInAgentReadiness = (
  model: Model,
): BuiltInAgentReadinessResponse | null =>
  model.builtInAgentReadiness as BuiltInAgentReadinessResponse | null

export const modelNotifications = (model: Model): NotificationCenterView | null =>
  model.notifications as NotificationCenterView | null

export const modelTrainingRuns = (model: Model): TrainingRunsResponse | null =>
  model.trainingRuns as TrainingRunsResponse | null

export const modelTrainingDashboard = (
  model: Model,
): TrainingDashboardSummaryResponse | null =>
  model.trainingDashboard as TrainingDashboardSummaryResponse | null

export const modelTrainingPromiseGates = (
  model: Model,
): TrainingPromiseGatesResponse | null =>
  model.trainingPromiseGates as TrainingPromiseGatesResponse | null

export const modelTrainingOperatorReadiness = (
  model: Model,
): TrainingOperatorReadinessResponse | null =>
  model.trainingOperatorReadiness as TrainingOperatorReadinessResponse | null

export const modelTrainingEvidencePacketSummary = (
  model: Model,
): TrainingEvidencePacketSummaryResponse | null =>
  model.trainingEvidencePacketSummary as TrainingEvidencePacketSummaryResponse | null

export const modelTrainingPlan = (model: Model): TrainingPlanResponse | null =>
  model.trainingPlan as TrainingPlanResponse | null

export const modelTrainingActivation = (
  model: Model,
): TrainingWindowActionResponse | null =>
  model.trainingActivation as TrainingWindowActionResponse | null

export const modelTrainingReconcile = (
  model: Model,
): TrainingWindowActionResponse | null =>
  model.trainingReconcile as TrainingWindowActionResponse | null

export const modelTrainingLease = (
  model: Model,
): TrainingWindowLeaseResponse | null =>
  model.trainingLease as TrainingWindowLeaseResponse | null

export const modelTrainingBootstrap = (
  model: Model,
): TrainingBootstrapGrantResponse | null =>
  model.trainingBootstrap as TrainingBootstrapGrantResponse | null

export const modelTrainingEvidencePacketBuild = (
  model: Model,
): TrainingEvidencePacketBuildResponse | null =>
  model.trainingEvidencePacketBuild as TrainingEvidencePacketBuildResponse | null

export const modelTrainingEvidenceAdmission = (
  model: Model,
): TrainingEvidenceAdmissionResponse | null =>
  model.trainingEvidenceAdmission as TrainingEvidenceAdmissionResponse | null

export const initialModel: Model = Model.make({
  node: null,
  notifications: null,
  pylonStats: null,
  builtInAgentReadiness: null,
  builtInAgentStatus: { text: "not checked", tone: "idle" },
  builtInAgentPending: false,
  nodeLaunchStatus: null,
  pane: "network",
  selectedSessionRef: null,
  selectedTrainingSceneNodeId: null,
  sessionFilter: "all",
  expandedEvents: [],
  resolvedApprovals: [],
  spawnAdapter: "codex",
  spawnObjective: "",
  spawnVerify: "",
  spawnLane: "auto",
  spawnStatus: { text: "", tone: "idle" },
  spawnPending: false,
  askTitle: "",
  askBody: "",
  askStatus: { text: "", tone: "idle" },
  askPending: false,
  trainingRuns: null,
  trainingRunsStatus: { text: "not loaded", tone: "idle" },
  trainingRunsPending: false,
  trainingDashboard: null,
  trainingDashboardStatus: { text: "not loaded", tone: "idle" },
  trainingDashboardPending: false,
  trainingPromiseGates: null,
  trainingPromiseGatesStatus: { text: "not loaded", tone: "idle" },
  trainingPromiseGatesPending: false,
  trainingOperatorReadiness: null,
  trainingOperatorReadinessStatus: { text: "not loaded", tone: "idle" },
  trainingOperatorReadinessPending: false,
  trainingEvidencePacketSummary: null,
  trainingEvidencePacketSummaryStatus: { text: "not loaded", tone: "idle" },
  trainingEvidencePacketSummaryPending: false,
  trainingPlan: null,
  trainingPlanStatus: { text: "", tone: "idle" },
  trainingPlanPending: false,
  trainingPlanFirstObservedAt: null,
  trainingActivation: null,
  trainingActivationStatus: { text: "", tone: "idle" },
  trainingActivationPending: false,
  trainingReconcile: null,
  trainingReconcileStatus: { text: "", tone: "idle" },
  trainingReconcilePending: false,
  trainingLease: null,
  trainingLeaseStatus: { text: "", tone: "idle" },
  trainingLeasePending: false,
  trainingBootstrap: null,
  trainingBootstrapStatus: { text: "", tone: "idle" },
  trainingBootstrapPending: false,
  trainingEvidencePacketBuild: null,
  trainingEvidencePacketBuildStatus: { text: "", tone: "idle" },
  trainingEvidencePacketBuildPending: false,
  trainingEvidenceAdmission: null,
  trainingEvidenceAdmissionStatus: { text: "", tone: "idle" },
  trainingEvidenceAdmissionPending: false,
  trainingLaunchStatus: { text: "", tone: "idle" },
  trainingLaunchPending: false,
  trainingCloseoutStatus: { text: "", tone: "idle" },
  trainingCloseoutPending: false,
  deployFeedback: null,
})
