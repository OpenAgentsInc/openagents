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
import {
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
} from "@openagentsinc/proof-replay"

import type { NotificationCenterView } from "@openagentsinc/autopilot-control-protocol"
// #5472: functional Settings preferences — the literal schemas live in the
// preferences module (the persistence + seam home) so the Model just re-uses them.
import {
  DefaultAdapter,
  DefaultLane,
  GatewayInferenceFallback,
  ThemePreference,
} from "./preferences"
import {
  decideInference,
  type InferenceRoutingDecision,
} from "../shared/inference-routing"
// HUD H3 (#5501): the managed pane-layer state. Stored on the Model as
// `S.Unknown` and re-narrowed by `modelPaneLayer` (same opaque-sub-state idiom as
// `node`/`notifications`). This keeps the import TYPE-ONLY here, so model.ts and
// pane-manager.ts do NOT form a runtime cycle (pane-manager imports the `PaneId`
// VALUE from this file; this file imports only the `PaneLayer` TYPE from there).
import type { PaneLayer } from "./pane-manager"
import type { PylonStatsSnapshot } from "../shared/pylon-network-scene"
import type {
  ChatWorldPylonScene,
  PaymentParticle,
} from "../shared/chat-world-scene"
import type { ChatWorldMultiplayerProjection } from "../shared/chat-world-multiplayer"
import type { DesktopProofReplayProjection } from "../shared/proof-replays"
import {
  DEFAULT_DESKTOP_PROOF_REPLAY_SLUG as DefaultDesktopProofReplaySlug,
} from "../shared/proof-replays"
import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
  IdentityChoiceStateResponse,
  InferenceGatewayReadinessResponse,
  InstallReadinessResponse,
  ManagedAccountsResponse,
  OnboardingStatusResponse,
  NodeStateMessage,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
  PublicActivityTimelineResponse,
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
//
// ZERO-BASE SHELL (owner directive, 2026-06-19): "shell" is the new DEFAULT
// surface — a black screen with nothing on it except a single text bar at the
// bottom. Everything else (the full multi-pane UI: network/chat/code/supervise/
// explore/settings + nav + palette) is KEPT and still mounts, but it is HIDDEN
// behind an explicit open (Cmd-K palette → any "Go to …" / "Open panes", or the
// small "open panes" affordance). The default is one thing at a time: black +
// the bottom input + the clean conversation above it once there is a response.
export const PaneId = S.Literals([
  "shell",
  "network",
  // AO-4 (#5445): the first-run onboarding wizard / live status surface. Shows
  // the identity choice (AO-3) and the live chain (registered → online → wallet
  // → payout → presence → Tassadar → claimed → earned) with retry on failure.
  "onboarding",
  "builtin-agent",
  "nodes",
  "training",
  "training-fullscreen",
  "sessions",
  "decisions",
  "spawn",
  // #5355: the interactive coding composer — the foreground "code in the app"
  // loop (objective → live transcript → inline approvals → reply/continue →
  // cancel) built on the existing control protocol (session.spawn/events/cancel
  // + approvals). The day-to-day Claude-Code/Codex replacement surface.
  "composer",
  // #5453: Blueprint chat pane — routes turns through the Blueprint program
  // runtime using the existing session.spawn + node-state-poll control path.
  "chat",
  // CS-A2 (#5362): the swarm / multi-session view — a lane/grid over the N
  // concurrent coding sessions the runtime can run (concurrent spawner #4869,
  // control `session.list`, external-session `parentRef` nesting). A pure read
  // projection over node-state + a per-cell open-in-composer / cancel quick
  // action + a top-level pending-approvals roll-up. No new wire verb.
  "swarm",
  // #5467 (EPIC #5461): the Autonomous loop view — a first-class, read-only
  // projection of the autonomous coordinator loop (intent → plan → fanout →
  // reconcile → ship gate) over `intent.list` + `coordinator.status`. Lives in
  // the Supervise nav group (nav.ts); reuses the existing pause/resume. No new
  // spend/execution authority. Pane module: autonomous-loop-pane.ts.
  "autonomous-loop",
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

export const VersePresenceZone = S.Literals(["tassadar_area"])
export type VersePresenceZone = typeof VersePresenceZone.Type

// Transient status for the Spawn form (validation/submit feedback). Kept in the
// Model so the view stays a pure function of state (no hidden DOM).
export const SpawnStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "idle"]),
})
export type SpawnStatus = typeof SpawnStatus.Type

// #5355: transient status for the coding composer (spawn/reply/cancel feedback).
export const ComposerStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type ComposerStatus = typeof ComposerStatus.Type

export const ChatStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type ChatStatus = typeof ChatStatus.Type

export const ChatStepStatus = S.Literals([
  "pending",
  "running",
  "completed",
  "verified",
  "blocked",
])
export type ChatStepStatus = typeof ChatStepStatus.Type

export const ChatStepKind = S.Literals([
  "signature",
  "tool_scope",
  "tassadar_module_step",
  "replay_module",
])
export type ChatStepKind = typeof ChatStepKind.Type

export const ChatStepVerdict = S.Literals(["verified", "rejected", "pending"])
export type ChatStepVerdict = typeof ChatStepVerdict.Type

export const ChatStep = S.Struct({
  id: S.String,
  kind: ChatStepKind,
  label: S.String,
  status: ChatStepStatus,
  signatureRef: S.NullOr(S.String),
  toolRef: S.NullOr(S.String),
  moduleRef: S.NullOr(S.String),
  digestRef: S.NullOr(S.String),
  verdict: S.NullOr(ChatStepVerdict),
  evidenceRef: S.NullOr(S.String),
  receiptRef: S.NullOr(S.String),
  tassadarModuleStepRef: S.NullOr(S.String),
  proofReplayRef: S.NullOr(S.String),
  contentRedacted: S.Boolean,
  linkedSessionRef: S.NullOr(S.String),
})
export type ChatStep = typeof ChatStep.Type

export const ChatMessage = S.Struct({
  id: S.String,
  role: S.Literals(["user", "assistant", "system"]),
  body: S.String,
  timestamp: S.String,
  linkedSessionRef: S.NullOr(S.String),
  steps: S.Array(ChatStep),
})
export type ChatMessage = typeof ChatMessage.Type

// ── Zero-base shell (owner directive, 2026-06-19) ───────────────────────────
// The minimal default surface's state. ONE text bar, a clean conversation above
// it, and nothing else. `shellTarget` chooses where that bar sends a turn:
// hosted/current model, Claude Code, or Codex. The coding targets retain their
// own session refs/turn history so Shift-Tab never cross-wires the two agents.
export const ShellRole = S.Literals(["you", "autopilot"])
export type ShellRole = typeof ShellRole.Type

export const ShellTarget = S.Literals(["current", "claude_code", "codex"])
export type ShellTarget = typeof ShellTarget.Type

export const ShellCodingTarget = S.Literals(["claude_code", "codex"])
export type ShellCodingTarget = typeof ShellCodingTarget.Type

export const ShellTurn = S.Struct({
  id: S.String,
  role: ShellRole,
  target: ShellTarget,
  sessionRef: S.NullOr(S.String),
  text: S.String,
})
export type ShellTurn = typeof ShellTurn.Type

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

export const AgentMode = S.Literals(["hosted", "local-apple-fm"])
export type AgentMode = typeof AgentMode.Type

export const AppleFmStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type AppleFmStatus = typeof AppleFmStatus.Type

export const InstallReadinessStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type InstallReadinessStatus = typeof InstallReadinessStatus.Type

export const PromiseSurfacingStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type PromiseSurfacingStatus = typeof PromiseSurfacingStatus.Type

export const PublicActivityStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type PublicActivityStatus = typeof PublicActivityStatus.Type

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

export const ProofReplaySlug = S.Literals([
  FIRST_REAL_SETTLEMENT_REPLAY_SLUG,
  LAUNCH_RECOGNITION_REPLAY_SLUG,
])
export type ProofReplaySlug = typeof ProofReplaySlug.Type

export const ProofReplayMode = S.Literals(["catalog", "generated"])
export type ProofReplayMode = typeof ProofReplayMode.Type

export const ProofReplayCommandRequest = S.Union([
  S.Struct({
    mode: S.Literal("catalog"),
    slug: ProofReplaySlug,
  }),
  S.Struct({
    actorRef: S.String,
    from: S.String,
    kind: S.String,
    limit: S.String,
    mode: S.Literal("generated"),
    pairRef: S.String,
    runRef: S.String,
    since: S.String,
    source: S.String,
    to: S.String,
    windowRef: S.String,
  }),
])
export type ProofReplayCommandRequest =
  typeof ProofReplayCommandRequest.Type

export const ProofReplayStatus = S.Struct({
  text: S.String,
  tone: S.Literals(["error", "info", "success", "idle"]),
})
export type ProofReplayStatus = typeof ProofReplayStatus.Type

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

  // #5730 (P2.5 chat-world wiring): live chat-world-behind-chat state, fed by
  // the flag-gated chat-world subscriptions (chat-world-subscriptions.ts) and
  // read by chatPane to drive the scene. Both stay at their empty defaults (and
  // the subscriptions stay noop) when the chat-world flags are OFF, so the chat
  // pane is byte-identical to current main unless the flags are built in.
  //   - chatWorldScene: latest projected ChatWorldPylonScene (opaque; read via
  //     modelChatWorldScene). Null until the first pylon poll lands → static seed.
  //   - chatWorldParticles: the bounded set of active payment-particle
  //     descriptors (opaque PaymentParticle[]; read via modelChatWorldParticles),
  //     each evidence-bound to a real sourceRef.
  //   - chatWorldMultiplayer: latest public SpacetimeDB world projection
  //     (stations, avatars, proximity chat) or null while disconnected.
  chatWorldScene: S.NullOr(S.Unknown),
  chatWorldParticles: S.Array(S.Unknown),
  chatWorldMultiplayer: S.NullOr(S.Unknown),
  // #5730: the receipt/source ref of the last-clicked payment beam endpoint, for
  // the inspector chip. Null when nothing is selected. Click → SelectedChatWorldNode.
  chatWorldInspectedRef: S.NullOr(S.String),

  // #5428: public activity timeline projection for Network/Training. The Bun
  // host fetches and schema-validates the Worker envelope; the webview renders
  // it as public-safe read-only activity, independent of local-node reachability.
  publicActivityTimeline: S.NullOr(S.Unknown),
  publicActivityTimelineStatus: PublicActivityStatus,
  publicActivityTimelinePending: S.Boolean,

  // #5063: public-safe readiness for the no-user-key built-in agent. Bun keeps
  // OpenAgents compute credentials; the webview sees only bounds/status refs.
  builtInAgentReadiness: S.NullOr(S.Unknown),
  builtInAgentStatus: BuiltInAgentStatus,
  builtInAgentPending: S.Boolean,
  appleFmReadiness: S.NullOr(S.Unknown),
  appleFmStatus: AppleFmStatus,
  appleFmPending: S.Boolean,
  // #5485 (EPIC #5474): public-safe OpenAgents inference-gateway readiness
  // (server-flag state + apiKeyPresent + credit balance). Drives the
  // own-auth-vs-gateway routing decision + the composer low-balance hint. Bun
  // owns the API key; the webview never sees it. Null until first fetched.
  inferenceGatewayReadiness: S.NullOr(S.Unknown),
  agentMode: AgentMode,

  // #5064: first-run/install health projection. Bun composes local node
  // status, hosted-agent readiness, platform/runtime, and auto-update state.
  installReadiness: S.NullOr(S.Unknown),
  installReadinessStatus: InstallReadinessStatus,
  installReadinessPending: S.Boolean,

  // AO-3/AO-4 (#5444/#5445): first-run onboarding wizard. `onboardingStatus`
  // is the live chain projection; `identityChoiceState` is the first-screen
  // detect-existing-vs-create-new state; `newIdentityName` is the create-new
  // name input; `onboardingPending`/`identityChoicePending` gate their refreshes.
  onboardingStatus: S.NullOr(S.Unknown),
  identityChoiceState: S.NullOr(S.Unknown),
  newIdentityName: S.String,
  onboardingPending: S.Boolean,
  identityChoicePending: S.Boolean,
  onboardingStatusLine: InstallReadinessStatus,

  // #5065: Product Promises Forum surfacing flow. The webview carries only
  // public-safe report fields; Bun owns the registered-agent token and posting.
  promiseSurfacingReadiness: S.NullOr(S.Unknown),
  promiseSurfacingResult: S.NullOr(S.Unknown),
  promiseSurfacingStatus: PromiseSurfacingStatus,
  promiseSurfacingReadinessPending: S.Boolean,
  promiseSurfacingSubmitPending: S.Boolean,
  promiseSurfacingPromiseId: S.String,
  promiseSurfacingSurface: S.String,
  promiseSurfacingClaimText: S.String,
  promiseSurfacingExpectedBehavior: S.String,
  promiseSurfacingObservedBehavior: S.String,
  promiseSurfacingEvidenceOrSteps: S.String,
  promiseSurfacingEnvironment: S.String,
  promiseSurfacingImpact: S.String,
  promiseSurfacingSuggestedState: S.Literals([
    "green",
    "yellow",
    "red",
    "degraded",
    "planned",
    "unknown",
  ]),

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

  // #5470 session-detail diff browser: which diff file paths are expanded
  // (per-file hunk body open). Empty = collapsed-by-default for files with
  // hunks. Keyed by the public-safe file ref the node emits.
  expandedDiffFiles: S.Array(S.String),

  // #5470 session-detail: side-by-side vs unified diff hunk layout.
  diffViewMode: S.Literals(["unified", "split"]),

  // #5470 session-detail: whether the artifact & receipt browser detail is
  // expanded (the ref rows under the artifact summary line).
  artifactBrowserOpen: S.Boolean,

  // #5730 The Verse: runtime toggle for the game-world view that renders behind
  // the chat surface. Defaults TRUE (the Verse shows by default). The build flag
  // CHAT_WORLD_SCENE remains a hard kill-switch; this is the user-facing control.
  verseEnabled: S.Boolean,
  // #5730/#5887: spatially scoped Verse chrome. Null means the avatar has walked
  // away from the Tassadar lot, so run-specific HUD chrome stays hidden.
  versePresenceZone: S.NullOr(VersePresenceZone),

  // Approvals optimistically resolved this session (hidden until the next poll
  // confirms). Keyed by approvalRef.
  resolvedApprovals: S.Array(S.String),

  // Spawn form fields + transient status.
  // CS-A1: `apple_fm` joins the runtime toggle as a spawn-adapter option
  // alongside codex/claude (it routes through the local Apple FM control verb).
  spawnAdapter: S.Literals(["codex", "claude_agent", "apple_fm"]),
  spawnObjective: S.String,
  spawnVerify: S.String,
  // #4998: requested execution lane for the spawn form. Default `auto`
  // (own-Pylon-first then cloud-gcp); cloud-gcp = Google GCE, cloud-shc = SHC.
  spawnLane: S.Literals(["auto", "local", "cloud-gcp", "cloud-shc"]),
  spawnStatus: SpawnStatus,
  spawnPending: S.Boolean,

  // #5355: coding composer state. The composer reuses the spawn form fields
  // (adapter/objective/verify/lane) for the FIRST turn, then drives the
  // iterative loop with its own state:
  //   - composerSessionRef: the active session the composer is tailing (null
  //     before the first spawn / after a fresh-thread reset).
  //   - composerRepoPath: the repo / worktree path the coding turns run in
  //     (passed through session.spawn's worktreePath — no new contract).
  //   - composerReply: the follow-up turn text the owner is composing.
  //   - composerTurns: the human-readable objectives sent so far, so the in-pane
  //     transcript shows the conversation across continuation turns even though
  //     each turn is its own bounded control session.
  composerSessionRef: S.NullOr(S.String),
  composerRepoPath: S.String,
  // #5471: repo / worktree picker mode. "worktree" points the session at an
  // existing local path (composerRepoPath, the original behavior); "managed"
  // requests a Pylon-managed worktree for a GitHub repo + base ref, resolved to
  // a repoRef node-side. Both ride the existing session.spawn — no new verb.
  composerWorkspaceMode: S.Literals(["worktree", "managed"]),
  composerManagedRepo: S.String,
  composerManagedBaseRef: S.String,
  composerReply: S.String,
  composerTurns: S.Array(S.String),
  composerStatus: ComposerStatus,
  composerPending: S.Boolean,
  // CS-A1: which provider account the composer's coding turns run under. Null
  // = the node's default account selection. Threaded through session.spawn's
  // `accountRef` (codex/claude) — no new control contract. Ignored for the
  // apple_fm adapter, which has no per-account selection.
  composerAccountRef: S.NullOr(S.String),
  // #5471: the fully-built objective a managed-worktree turn is waiting to send
  // once its repoRef resolves node-side. Null when no managed resolution is in
  // flight. Lets the resolve→spawn handoff stay a pure two-step reducer.
  composerPendingObjective: S.NullOr(S.String),

  // #5469 (EPIC #5461): swarm batch-launch state. The batch form lives inside
  // the swarm pane (no new top-level button). The reducer drives a bounded
  // concurrent spawner over the EXISTING session.spawn verb — no new wire verb.
  //   - swarmBatchObjectives: the textarea (one objective per line)
  //   - swarmBatchConcurrency: the visible concurrency cap (string-bound input)
  //   - swarmBatchQueue: objectives not yet dispatched (drained as spawns settle)
  //   - swarmBatchActive: how many batch spawns are currently in flight
  //   - swarmBatchLaunched/swarmBatchFailed/swarmBatchTotal: honest counters
  // The active/queue/launched/failed/total mirror the SwarmBatchState the pure
  // swarm-batch.ts module threads; they live on the Model so the reducer stays a
  // pure function of state.
  swarmBatchObjectives: S.String,
  swarmBatchConcurrency: S.String,
  swarmBatchQueue: S.Array(S.String),
  swarmBatchActive: S.Number,
  swarmBatchLaunched: S.Number,
  swarmBatchFailed: S.Number,
  swarmBatchTotal: S.Number,

  // #5453: Blueprint chat state. The messages are persisted in the Foldkit
  // model so pane navigation does not discard the visible conversation. Turns
  // spawn bounded sessions and are reconciled by the node-state poll.
  chatMessages: S.Array(ChatMessage),
  // Which chat messages have their per-message "program details" disclosure
  // expanded (the scoped-step / Tassadar scaffolding). Empty = collapsed by
  // default; the conversation text is what shows by default. Keyed by message
  // id, mirroring the `expandedEvents` / `expandedDiffFiles` toggle pattern.
  expandedChatMessages: S.Array(S.String),
  chatInput: S.String,
  chatStatus: ChatStatus,
  chatPending: S.Boolean,
  chatSessionRef: S.NullOr(S.String),

  // CS-A1: account-management surface state (add/select/priority over the
  // node's local dev.accounts config). `managedAccounts` holds the last
  // ManagedAccountsResponse projection (opaque, read via the typed accessor);
  // the rest are the add-account form fields + transient status.
  managedAccounts: S.NullOr(S.Unknown),
  managedAccountsPending: S.Boolean,
  managedAccountsStatus: ComposerStatus,
  addAccountRef: S.String,
  addAccountProvider: S.Literals(["codex", "claude_agent"]),
  addAccountHome: S.String,
  addAccountPriority: S.String,

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
  selectedProofReplayMode: ProofReplayMode,
  selectedProofReplaySlug: ProofReplaySlug,
  generatedProofReplayFrom: S.String,
  generatedProofReplayTo: S.String,
  generatedProofReplayRunRef: S.String,
  generatedProofReplayWindowRef: S.String,
  generatedProofReplayActorRef: S.String,
  generatedProofReplayPairRef: S.String,
  generatedProofReplayKind: S.String,
  generatedProofReplaySource: S.String,
  generatedProofReplaySince: S.String,
  generatedProofReplayLimit: S.String,
  proofReplay: S.NullOr(S.Unknown),
  proofReplayStatus: ProofReplayStatus,
  proofReplayPending: S.Boolean,

  // Deploy feedback (null until the card has been interacted with or a deploy
  // projection has landed).
  deployFeedback: S.NullOr(DeployFeedback),

  // #5464: Cmd-K command-palette state. `commandPaletteOpen` toggles the
  // overlay; `commandPaletteQuery` is the live fuzzy query; `commandPaletteIndex`
  // is the keyboard-highlighted row into the CURRENT filtered list (clamped in
  // the reducer). Kept in the Model so the palette is a pure function of state —
  // no hidden DOM. See nav.ts for the registry these read against.
  commandPaletteOpen: S.Boolean,
  commandPaletteQuery: S.String,
  commandPaletteIndex: S.Number,

  // #5472: functional Settings preferences (local, refs-only; persisted to
  // localStorage by the preferences module). `themePreference` drives the live
  // `data-theme` attribute on the app shell; `defaultAdapter`/`defaultLane` seed
  // the spawn form at init (consumed by spawn/composer/chat through the existing
  // spawnAdapter/spawnLane fields — no new wiring); `showNotificationPanel`
  // gates the in-app Settings notification panel. See ui/preferences.ts.
  themePreference: ThemePreference,
  defaultAdapter: DefaultAdapter,
  defaultLane: DefaultLane,
  showNotificationPanel: S.Boolean,
  // #5485: whether to route coding inference through the OpenAgents gateway
  // when there is no usable own auth. Persisted + applied to the live routing
  // decision (see shared/inference-routing.ts).
  gatewayInferenceFallback: GatewayInferenceFallback,

  // ── Zero-base shell (owner directive, 2026-06-19) ─────────────────────────
  // The minimal default surface. `shellInput` is the bottom text bar; `shellTurns`
  // is the clean conversation rendered above it (what you typed → the answer);
  // `shellPending` gates the input while a response is in flight. Shift-Tab
  // cycles `shellTarget`; Claude/Codex targets retain independent lightweight
  // continuation state over the existing session.spawn bridge.
  shellTarget: ShellTarget,
  shellInput: S.String,
  shellTurns: S.Array(ShellTurn),
  shellPending: S.Boolean,
  shellClaudeSessionRef: S.NullOr(S.String),
  shellCodexSessionRef: S.NullOr(S.String),
  shellClaudeTurns: S.Array(S.String),
  shellCodexTurns: S.Array(S.String),

  // ── HUD H3: the managed pane layer (#5501) ────────────────────────────────
  // The set of open managed panes (pane-as-data: id, kind, rect, z) + the
  // monotonic seq + the in-flight drag, opaque to the Codec (S.Unknown) and
  // re-narrowed by `modelPaneLayer`. EMPTY by default — the shell stays black
  // until a pane is explicitly opened (hotbar / palette). This is ORTHOGONAL to
  // `pane` (the single-pane router above): the managed panes float OVER whatever
  // base surface `pane` is showing, so neither the shell nor the full UI regress.
  paneLayer: S.Unknown,
})
export type Model = typeof Model.Type

// Re-narrow the opaque pane-layer field at its single read boundary (mirrors
// modelNode / modelNotifications). Defaults to an empty layer if (impossibly)
// unset, so the view never has to null-check.
export const modelPaneLayer = (model: Model): PaneLayer =>
  (model.paneLayer as PaneLayer | null) ?? { panes: [], seq: 0, drag: null }

// ── Zero-base shell introspection (programmatic-control parity) ─────────────
// A pure, plain-text projection of exactly what the shell screen shows the user
// (the conversation above the bar). The headless/RPC control path reads THIS so
// a driver (Claude) sees the SAME rendered state the owner does — no DOM, no
// hidden fields. One line per turn: "you: …" / "autopilot: …"; non-current
// targets include the same target label rendered in the shell turn header.
const shellTargetTranscriptLabel = (target: ShellTarget): string =>
  target === "claude_code" ? "Claude Code" : target === "codex" ? "Codex" : ""

export const shellTranscriptText = (model: Model): string =>
  model.shellTurns.map((turn) => {
    const target = shellTargetTranscriptLabel(turn.target)
    const prefix = target === "" ? "" : `[${target}] `
    return `${turn.role}: ${prefix}${turn.text}`
  }).join("\n")

// ── Typed accessors over the opaque projection fields ──────────────────────
//
// The Model carries `node`/`notifications` as `S.Unknown` so makeProgram's
// Schema.Codec stays happy with externally-owned payloads; these helpers re-narrow
// them at the single read boundary so the rest of the app is typed.

export const modelNode = (model: Model): NodeStateMessage | null =>
  model.node as NodeStateMessage | null

export const modelManagedAccounts = (
  model: Model,
): ManagedAccountsResponse | null =>
  model.managedAccounts as ManagedAccountsResponse | null

export const modelPylonStats = (model: Model): PylonStatsSnapshot | null =>
  model.pylonStats as PylonStatsSnapshot | null

// #5730: typed read boundary for the opaque chat-world state.
export const modelChatWorldScene = (
  model: Model,
): ChatWorldPylonScene | null => model.chatWorldScene as ChatWorldPylonScene | null

export const modelChatWorldParticles = (
  model: Model,
): ReadonlyArray<PaymentParticle> =>
  model.chatWorldParticles as ReadonlyArray<PaymentParticle>

export const modelChatWorldMultiplayer = (
  model: Model,
): ChatWorldMultiplayerProjection | null =>
  model.chatWorldMultiplayer as ChatWorldMultiplayerProjection | null

export const modelPublicActivityTimeline = (
  model: Model,
): PublicActivityTimelineResponse | null =>
  model.publicActivityTimeline as PublicActivityTimelineResponse | null

export const modelBuiltInAgentReadiness = (
  model: Model,
): BuiltInAgentReadinessResponse | null =>
  model.builtInAgentReadiness as BuiltInAgentReadinessResponse | null

export const modelAppleFmReadiness = (
  model: Model,
): AppleFmReadinessResponse | null =>
  model.appleFmReadiness as AppleFmReadinessResponse | null

export const modelInferenceGatewayReadiness = (
  model: Model,
): InferenceGatewayReadinessResponse | null =>
  model.inferenceGatewayReadiness as InferenceGatewayReadinessResponse | null

// #5485: derive the inference routing decision for the CURRENTLY-selected coding
// adapter from the live model (provider accounts + gateway readiness + the saved
// fallback preference). Single source so the composer hint, the spawn path, and
// the tests all agree on the own-auth-vs-gateway choice. Apple FM and a not-yet-
// fetched gateway readiness both degrade to a safe "own_auth/local" decision.
export const modelInferenceDecision = (
  model: Model,
): InferenceRoutingDecision => {
  const node = modelNode(model)
  const accounts = (node?.accounts ?? []).map((row) => ({
    provider: row.provider,
    ready: row.ready,
  }))
  const readiness = modelInferenceGatewayReadiness(model)
  return decideInference({
    adapter: model.spawnAdapter,
    accounts,
    preference: model.gatewayInferenceFallback,
    gateway: {
      enabled: readiness?.enabled ?? false,
      apiKeyPresent: readiness?.apiKeyPresent ?? false,
      creditBalance: readiness?.creditBalance ?? null,
    },
  })
}

export const modelInstallReadiness = (
  model: Model,
): InstallReadinessResponse | null =>
  model.installReadiness as InstallReadinessResponse | null

export const modelOnboardingStatus = (
  model: Model,
): OnboardingStatusResponse | null =>
  model.onboardingStatus as OnboardingStatusResponse | null

export const modelIdentityChoiceState = (
  model: Model,
): IdentityChoiceStateResponse | null =>
  model.identityChoiceState as IdentityChoiceStateResponse | null

export const modelPromiseSurfacingReadiness = (
  model: Model,
): PromiseSurfacingReadinessResponse | null =>
  model.promiseSurfacingReadiness as PromiseSurfacingReadinessResponse | null

export const modelPromiseSurfacingResult = (
  model: Model,
): PromiseSurfacingResponse | null =>
  model.promiseSurfacingResult as PromiseSurfacingResponse | null

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

export const modelProofReplay = (
  model: Model,
): DesktopProofReplayProjection | null =>
  model.proofReplay as DesktopProofReplayProjection | null

export const BLUEPRINT_CHAT_SIGNATURE_REF =
  "signature.openagents.autopilot_continue.v1"
export const BLUEPRINT_CHAT_CONTEXT_TOOL_REF = "tool.context_pack.read"
export const BLUEPRINT_CHAT_TASSADAR_TOOL_REF = "tool.tassadar.module.execute"
export const BLUEPRINT_CHAT_TASSADAR_MODULE_REF =
  "listing.public.tassadar_compiled_weight_module.cc1403674fc0d388"
export const BLUEPRINT_CHAT_TASSADAR_STEP_REF =
  "step.blueprint.tassadar.linked_dense.exact_replay.v1"
export const BLUEPRINT_CHAT_TASSADAR_DIGEST_REF =
  "sha256:0caa43ace27a5b86da14cfe037e65c30f250f0c0a0ac1c01f1fe3a3a45a230b2"
export const BLUEPRINT_CHAT_TASSADAR_EVIDENCE_REF =
  "evidence.openagents.blueprint_tassadar_step.cc1403674fc0d388"
export const BLUEPRINT_CHAT_TASSADAR_RECEIPT_REF =
  "receipt.openagents.blueprint_tassadar_step.cc1403674fc0d388"
export const BLUEPRINT_CHAT_TASSADAR_PROOF_REPLAY_REF =
  DefaultDesktopProofReplaySlug
export const BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF =
  "program_signature.blueprint.show_replay.v1"
export const BLUEPRINT_CHAT_REPLAY_TOOL_REF =
  "tool.proof_replay.bundle.show"
export const BLUEPRINT_CHAT_REPLAY_MODULE_REF =
  "module.openagents.public_proof_replay_runtime"
export const BLUEPRINT_CHAT_REPLAY_EVIDENCE_REF =
  "evidence.openagents.blueprint_replay_module.first-real-settlement"
export const BLUEPRINT_CHAT_REPLAY_RECEIPT_REF =
  "receipt.public_proof_replay_bundle"

export const blueprintChatScopedSteps = (
  input: Readonly<{
    linkedSessionRef?: string | null
    signatureStatus?: ChatStepStatus
    contextStatus?: ChatStepStatus
    tassadarStatus?: ChatStepStatus
    tassadarVerdict?: ChatStepVerdict
    tassadarEvidenceRef?: string | null
    tassadarReceiptRef?: string | null
    replayStatus?: ChatStepStatus
  }> = {},
): Array<ChatStep> => {
  const linkedSessionRef = input.linkedSessionRef ?? null
  const tassadarVerdict =
    input.tassadarVerdict ??
    (linkedSessionRef === null ? "pending" : "verified")
  const tassadarReceiptRef =
    input.tassadarReceiptRef ??
    (tassadarVerdict === "verified" ? BLUEPRINT_CHAT_TASSADAR_RECEIPT_REF : null)
  return [
    {
      id: "blueprint-chat-signature",
      kind: "signature",
      label: "Selected signature",
      status:
        input.signatureStatus ??
        (linkedSessionRef === null ? "pending" : "completed"),
      signatureRef: BLUEPRINT_CHAT_SIGNATURE_REF,
      toolRef: null,
      moduleRef: null,
      digestRef: null,
      verdict: null,
      evidenceRef: null,
      receiptRef: null,
      tassadarModuleStepRef: null,
      proofReplayRef: null,
      contentRedacted: false,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-context-pack",
      kind: "tool_scope",
      label: "Scoped tool",
      status:
        input.contextStatus ??
        (linkedSessionRef === null ? "running" : "completed"),
      signatureRef: null,
      toolRef: BLUEPRINT_CHAT_CONTEXT_TOOL_REF,
      moduleRef: null,
      digestRef: null,
      verdict: null,
      evidenceRef: null,
      receiptRef: null,
      tassadarModuleStepRef: null,
      proofReplayRef: null,
      contentRedacted: false,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-tassadar-step",
      kind: "tassadar_module_step",
      label: "Tassadar module step",
      status:
        input.tassadarStatus ??
        (linkedSessionRef === null ? "running" : "verified"),
      signatureRef: null,
      toolRef: BLUEPRINT_CHAT_TASSADAR_TOOL_REF,
      moduleRef: BLUEPRINT_CHAT_TASSADAR_MODULE_REF,
      digestRef: BLUEPRINT_CHAT_TASSADAR_DIGEST_REF,
      verdict: tassadarVerdict,
      evidenceRef:
        input.tassadarEvidenceRef ?? BLUEPRINT_CHAT_TASSADAR_EVIDENCE_REF,
      receiptRef: tassadarReceiptRef,
      tassadarModuleStepRef: BLUEPRINT_CHAT_TASSADAR_STEP_REF,
      proofReplayRef: BLUEPRINT_CHAT_TASSADAR_PROOF_REPLAY_REF,
      contentRedacted: true,
      linkedSessionRef,
    },
    {
      id: "blueprint-chat-replay-module",
      kind: "replay_module",
      label: "Proof replay bundle",
      status:
        input.replayStatus ??
        (linkedSessionRef === null ? "running" : "verified"),
      signatureRef: BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF,
      toolRef: BLUEPRINT_CHAT_REPLAY_TOOL_REF,
      moduleRef: BLUEPRINT_CHAT_REPLAY_MODULE_REF,
      digestRef: null,
      verdict: null,
      evidenceRef: BLUEPRINT_CHAT_REPLAY_EVIDENCE_REF,
      receiptRef: BLUEPRINT_CHAT_REPLAY_RECEIPT_REF,
      tassadarModuleStepRef: null,
      proofReplayRef: BLUEPRINT_CHAT_TASSADAR_PROOF_REPLAY_REF,
      contentRedacted: true,
      linkedSessionRef,
    },
  ]
}

// #5466 (EPIC #5461): the intro message carries NO program steps and NO verdict.
// Steps + verdicts now only appear on real turns, derived from live session
// events (blueprint-chat-runtime.ts). Previously this seed claimed a "verified"
// Tassadar step before any session existed — the central dishonesty #5466 fixes.
const seededChatMessage: ChatMessage = {
  id: "chat.seed.blueprint-intro",
  role: "system",
  body: "Blueprint program chat is ready. Each turn is routed to a signature and runs as a bounded session; steps and exact-replay verdicts appear here as the live session reports them.",
  timestamp: "2026-06-18T00:00:00.000Z",
  linkedSessionRef: null,
  steps: [],
}

export const initialModel: Model = Model.make({
  node: null,
  notifications: null,
  pylonStats: null,
  chatWorldScene: null,
  chatWorldParticles: [],
  chatWorldMultiplayer: null,
  chatWorldInspectedRef: null,
  publicActivityTimeline: null,
  publicActivityTimelineStatus: { text: "not loaded", tone: "idle" },
  publicActivityTimelinePending: false,
  builtInAgentReadiness: null,
  builtInAgentStatus: { text: "not checked", tone: "idle" },
  builtInAgentPending: false,
  appleFmReadiness: null,
  appleFmStatus: { text: "not checked", tone: "idle" },
  appleFmPending: false,
  inferenceGatewayReadiness: null,
  agentMode: "hosted",
  installReadiness: null,
  installReadinessStatus: { text: "not checked", tone: "idle" },
  installReadinessPending: false,
  onboardingStatus: null,
  identityChoiceState: null,
  newIdentityName: "",
  onboardingPending: false,
  identityChoicePending: false,
  onboardingStatusLine: { text: "not loaded", tone: "idle" },
  promiseSurfacingReadiness: null,
  promiseSurfacingResult: null,
  promiseSurfacingStatus: { text: "not checked", tone: "idle" },
  promiseSurfacingReadinessPending: false,
  promiseSurfacingSubmitPending: false,
  promiseSurfacingPromiseId: "",
  promiseSurfacingSurface: "Autopilot",
  promiseSurfacingClaimText: "",
  promiseSurfacingExpectedBehavior: "",
  promiseSurfacingObservedBehavior: "",
  promiseSurfacingEvidenceOrSteps: "",
  promiseSurfacingEnvironment: "Autopilot",
  promiseSurfacingImpact: "",
  promiseSurfacingSuggestedState: "yellow",
  nodeLaunchStatus: null,
  pane: "network",
  selectedSessionRef: null,
  selectedTrainingSceneNodeId: null,
  sessionFilter: "all",
  expandedEvents: [],
  expandedDiffFiles: [],
  diffViewMode: "unified",
  artifactBrowserOpen: false,
  // #5730 The Verse: on by default so the game-world view is visible now.
  verseEnabled: true,
  versePresenceZone: null,
  resolvedApprovals: [],
  spawnAdapter: "codex",
  spawnObjective: "",
  spawnVerify: "",
  spawnLane: "auto",
  spawnStatus: { text: "", tone: "idle" },
  spawnPending: false,
  composerSessionRef: null,
  composerRepoPath: "",
  // #5471: default to the existing-worktree path mode (today's behavior).
  composerWorkspaceMode: "worktree",
  composerManagedRepo: "",
  composerManagedBaseRef: "",
  composerReply: "",
  composerTurns: [],
  composerStatus: { text: "", tone: "idle" },
  composerPending: false,
  composerAccountRef: null,
  // #5469: swarm batch launch — empty objective set, default concurrency cap.
  swarmBatchObjectives: "",
  swarmBatchConcurrency: "3",
  swarmBatchQueue: [],
  swarmBatchActive: 0,
  swarmBatchLaunched: 0,
  swarmBatchFailed: 0,
  swarmBatchTotal: 0,
  composerPendingObjective: null,
  chatMessages: [seededChatMessage],
  expandedChatMessages: [],
  chatInput: "",
  chatStatus: { text: "", tone: "idle" },
  chatPending: false,
  chatSessionRef: null,
  managedAccounts: null,
  managedAccountsPending: false,
  managedAccountsStatus: { text: "", tone: "idle" },
  addAccountRef: "",
  addAccountProvider: "codex",
  addAccountHome: "",
  addAccountPriority: "",
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
  selectedProofReplayMode: "catalog",
  selectedProofReplaySlug: DefaultDesktopProofReplaySlug,
  generatedProofReplayFrom: "2026-06-18T00:00:00.000Z",
  generatedProofReplayTo: "2026-06-19T00:00:00.000Z",
  generatedProofReplayRunRef: "",
  generatedProofReplayWindowRef: "",
  generatedProofReplayActorRef: "",
  generatedProofReplayPairRef: "",
  generatedProofReplayKind: "",
  generatedProofReplaySource: "",
  generatedProofReplaySince: "",
  generatedProofReplayLimit: "20",
  proofReplay: null,
  proofReplayStatus: { text: "not loaded", tone: "idle" },
  proofReplayPending: false,
  deployFeedback: null,
  commandPaletteOpen: false,
  commandPaletteQuery: "",
  commandPaletteIndex: 0,
  // #5472: preference defaults. The real saved values are loaded + applied at
  // app entry (initial-state.ts); this neutral base keeps the view/update tests
  // deterministic (matches `defaultPreferences` in ui/preferences.ts).
  themePreference: "dark",
  defaultAdapter: "codex",
  defaultLane: "auto",
  showNotificationPanel: true,
  gatewayInferenceFallback: "auto",
  // Zero-base shell: empty input, empty conversation, idle. The real app entry
  // (initial-state.ts) sets `pane: "shell"`; this neutral base keeps `pane`
  // "network" so the existing view/update tests stay deterministic.
  shellTarget: "current",
  shellInput: "",
  shellTurns: [],
  shellPending: false,
  shellClaudeSessionRef: null,
  shellCodexSessionRef: null,
  shellClaudeTurns: [],
  shellCodexTurns: [],
  // HUD H3 (#5501): no managed panes open by default (the shell stays black).
  paneLayer: { panes: [], seq: 0, drag: null },
})
