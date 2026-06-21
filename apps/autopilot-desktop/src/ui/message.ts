// CL-53: the Foldkit message set for the Autopilot Desktop webview.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/message.ts):
// each message is a callable tagged struct built with foldkit/message `m`, then
// unioned. Inbound RPC pushes (node-state / notifications) arrive as
// GotNodeState / GotNotifications via the persistent subscription stream.

import { Schema as S } from "effect"
import { m } from "foldkit/message"

import {
  PaneId,
  ProofReplaySlug,
  SessionFilter,
  ShellCodingTarget,
  ShellTarget,
  VersePresenceZone,
} from "./model.js"
// HUD H3 (#5501): the drag/handle literal schemas for the managed pane-layer
// messages. Imported as VALUES (the `m()` constructors need the runtime schema);
// pane-manager imports only the `PaneId` value from model.ts, so there is no
// cycle back to message.ts.
import { PaneDragKind, PaneResizeHandle } from "./pane-manager.js"
// #5472: the preference literal schemas (single source — see ui/preferences.ts).
import {
  DefaultAdapter,
  DefaultLane,
  GatewayInferenceFallback,
  ThemePreference,
} from "./preferences.js"

// ── Inbound (Electrobun → runtime), pushed by the subscription stream ──────
export const GotNodeState = m("GotNodeState", { node: S.Unknown })
// #5049: public pylon-network stats push (drives the home network scene).
export const GotPylonStats = m("GotPylonStats", { stats: S.Unknown })
export const GotNotifications = m("GotNotifications", { view: S.Unknown })
// #5730 (P2.5 chat-world wiring): live chat-world scene + payment-particle
// pushes from the flag-gated chat-world subscriptions. `scene` is an opaque
// ChatWorldPylonScene; `particle` an opaque evidence-bound PaymentParticle.
export const GotChatWorldScene = m("GotChatWorldScene", { scene: S.Unknown })
export const GotChatWorldPaymentParticle = m("GotChatWorldPaymentParticle", {
  particle: S.Unknown,
})
export const GotChatWorldMultiplayer = m("GotChatWorldMultiplayer", {
  world: S.Unknown,
})
// #5730/#5822: a click on a scene node/entity. Payment endpoints encode their
// particle (`pay:<eventRef>:from|to`) and pass receipt detail here while the
// rendered scene label stays short. Verse training stages use the same label
// slot for their public-ref detail string.
export const SelectedChatWorldNode = m("SelectedChatWorldNode", {
  id: S.String,
  label: S.String,
})
export const ChangedVersePresenceZone = m("ChangedVersePresenceZone", {
  zone: S.NullOr(VersePresenceZone),
})
export const ChangedVerseWorldItemProximity = m(
  "ChangedVerseWorldItemProximity",
  {
    itemId: S.NullOr(S.String),
  },
)
export const ChangedVerseLocalPose = m("ChangedVerseLocalPose", {
  pose: S.Struct({
    regionRef: S.String,
    x: S.Number,
    y: S.Number,
    z: S.Number,
    yaw: S.Number,
    animation: S.Literals(["idle", "walk", "run"]),
    capturedAtMs: S.Number,
  }),
})
export const SettledVerseLocalPosePublish = m("SettledVerseLocalPosePublish", {
  ok: S.Boolean,
  reason: S.String,
})
// #5025: honest node-launch lifecycle status from the Bun supervisor.
export const GotNodeLaunchStatus = m("GotNodeLaunchStatus", { status: S.String })

// ── Navigation ─────────────────────────────────────────────────────────────
export const NavigatedTo = m("NavigatedTo", { pane: PaneId })

// #5463: jump to a primary nav group (clicking a group header / Cmd-1..5). The
// reducer maps the group id to its `defaultPane` via the nav registry.
export const NavigatedToGroup = m("NavigatedToGroup", { group: S.String })

// ── Command palette (#5464) ──────────────────────────────────────────────────
export const OpenedCommandPalette = m("OpenedCommandPalette")
export const ClosedCommandPalette = m("ClosedCommandPalette")
export const ChangedCommandPaletteQuery = m("ChangedCommandPaletteQuery", {
  value: S.String,
})
// Move the highlighted row by `delta` (±1) through the current filtered list.
export const MovedCommandPaletteSelection = m("MovedCommandPaletteSelection", {
  delta: S.Number,
})
// Run a specific command by registry id (click a row) or the highlighted one
// (Enter, commandId null → reducer reads commandPaletteIndex).
export const RanPaletteCommand = m("RanPaletteCommand", {
  commandId: S.NullOr(S.String),
})

// ── Keyboard layer (#5465) ────────────────────────────────────────────────────
// The keyboard subscription emits ONE raw key event; the pure reducer
// (update.ts) interprets it against the active pane + palette state. Keeping the
// interpretation in the reducer makes the whole shortcut layer unit-testable
// without a DOM. `inEditable` is true when focus is in a text input/textarea so
// nav keys never fire mid-typing (#5465 scoping rule).
export const PressedKey = m("PressedKey", {
  key: S.String,
  meta: S.Boolean,
  ctrl: S.Boolean,
  shift: S.Boolean,
  inEditable: S.Boolean,
})

export const SelectedSession = m("SelectedSession", { sessionRef: S.String })
export const ChangedSessionFilter = m("ChangedSessionFilter", {
  filter: SessionFilter,
})
export const ToggledEvent = m("ToggledEvent", { eventIndex: S.Number })

// #5470 session-detail diff/artifact browser: expand a single diff file's hunk
// body, flip the unified/split layout, and expand the artifact ref browser. All
// are pure model toggles — no control verb / RPC.
export const ToggledDiffFile = m("ToggledDiffFile", { path: S.String })
export const ToggledDiffViewMode = m("ToggledDiffViewMode")
export const ToggledArtifactBrowser = m("ToggledArtifactBrowser")

// #5730 The Verse: flip the runtime toggle for the game-world view that renders
// behind chat. Pure model toggle — no control verb / RPC.
export const ToggleVerse = m("ToggleVerse")

// Chat: expand/collapse a single chat message's "program details" disclosure
// (the scoped-step / Tassadar scaffolding). Collapsed by default so the chat
// opens to a clean conversation. Pure model toggle — no control verb / RPC.
export const ToggledChatMessageDetails = m("ToggledChatMessageDetails", {
  messageId: S.String,
})

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

// ── Inference gateway default-inference (#5485, EPIC #5474) ─────────────────
// The Bun host's public-safe gateway readiness (server-flag + apiKeyPresent +
// credit balance) landing back in the model; drives the routing decision + the
// composer low-balance hint.
export const GotInferenceGatewayReadiness = m("GotInferenceGatewayReadiness", {
  projection: S.Unknown,
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

// ── AO-3/AO-4 first-run onboarding wizard (#5444 / #5445) ──────────────────
// Live onboarding chain status (the wizard steps).
export const ClickedRefreshOnboarding = m("ClickedRefreshOnboarding")
export const GotOnboardingStatus = m("GotOnboardingStatus", {
  projection: S.Unknown,
})
// Identity-choice state (first screen of the wizard).
export const GotIdentityChoiceState = m("GotIdentityChoiceState", {
  state: S.Unknown,
})
export const ChangedNewIdentityName = m("ChangedNewIdentityName", {
  value: S.String,
})
export const ClickedUseExistingIdentity = m("ClickedUseExistingIdentity")
export const ClickedCreateNewIdentity = m("ClickedCreateNewIdentity")
export const SettledChooseIdentity = m("SettledChooseIdentity", {
  result: S.Unknown,
})
// Retry a failed onboarding step (re-loads status; the supervisor converges).
export const ClickedRetryOnboarding = m("ClickedRetryOnboarding")

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
export const ClickedRefreshPublicActivity = m("ClickedRefreshPublicActivity")
export const GotPublicActivityTimeline = m("GotPublicActivityTimeline", {
  projection: S.Unknown,
})
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
export const ChangedProofReplayGeneratedFrom = m(
  "ChangedProofReplayGeneratedFrom",
  { value: S.String },
)
export const ChangedProofReplayGeneratedTo = m(
  "ChangedProofReplayGeneratedTo",
  { value: S.String },
)
export const ChangedProofReplayGeneratedRunRef = m(
  "ChangedProofReplayGeneratedRunRef",
  { value: S.String },
)
export const ChangedProofReplayGeneratedWindowRef = m(
  "ChangedProofReplayGeneratedWindowRef",
  { value: S.String },
)
export const ChangedProofReplayGeneratedActorRef = m(
  "ChangedProofReplayGeneratedActorRef",
  { value: S.String },
)
export const ChangedProofReplayGeneratedPairRef = m(
  "ChangedProofReplayGeneratedPairRef",
  { value: S.String },
)
export const ChangedProofReplayGeneratedKind = m(
  "ChangedProofReplayGeneratedKind",
  { value: S.String },
)
export const ChangedProofReplayGeneratedSource = m(
  "ChangedProofReplayGeneratedSource",
  { value: S.String },
)
export const ChangedProofReplayGeneratedSince = m(
  "ChangedProofReplayGeneratedSince",
  { value: S.String },
)
export const ChangedProofReplayGeneratedLimit = m(
  "ChangedProofReplayGeneratedLimit",
  { value: S.String },
)
export const ClickedLoadGeneratedProofReplay = m(
  "ClickedLoadGeneratedProofReplay",
)
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

// ── CS-A2 (#5362): swarm / multi-session view quick action ──────────────────
// "Open in composer" adopts an existing swarm session into the composer thread
// so the owner can continue/reply to it from the day-to-day coding surface. The
// session-detail timeline already covers read-only inspection (SelectedSession),
// and cancel reuses ClickedCancelSession — so the swarm view adds only this one
// adoption verb on top of the existing protocol.
export const ClickedOpenSessionInComposer = m("ClickedOpenSessionInComposer", {
  sessionRef: S.String,
  workspaceRef: S.NullOr(S.String),
  adapter: S.Literals(["codex", "claude_agent", "apple_fm"]),
})

// ── #5469 (EPIC #5461): swarm batch launch ──────────────────────────────────
// A bounded concurrent spawner DRIVEN BY THE REDUCER over the EXISTING
// session.spawn verb — no `sessions batch` wire verb is invented. The batch
// form lives inside the swarm pane (audit §5.2: no new top-level button). The
// reducer keeps a queue + an in-flight count and never exceeds the visible
// concurrency cap; each launched/failed spawn settles into one of the two
// result messages, which pull the next queued objective.
export const ChangedSwarmBatchObjectives = m("ChangedSwarmBatchObjectives", {
  value: S.String,
})
export const ChangedSwarmBatchConcurrency = m("ChangedSwarmBatchConcurrency", {
  value: S.String,
})
export const ClickedSwarmBatchLaunch = m("ClickedSwarmBatchLaunch")
// One batch session's spawn was accepted by the node. Carries the sessionRef so
// the status line can be honest; pulls the next queued objective.
export const SucceededSwarmBatchSpawn = m("SucceededSwarmBatchSpawn", {
  sessionRef: S.String,
})
// One batch session's spawn was rejected. Pulls the next queued objective so a
// single failure never stalls the rest of the batch.
export const FailedSwarmBatchSpawn = m("FailedSwarmBatchSpawn", {
  error: S.String,
})

// ── #5355: coding composer (the day-to-day CLI replacement loop) ────────────
// The composer reuses ChangedSpawnAdapter/Objective/Verify/Lane for the first
// turn's form fields, and adds repo-path + reply-turn inputs plus its own
// spawn/reply/new-thread verbs so the iterative loop stays a pure reducer.
export const ChangedComposerRepoPath = m("ChangedComposerRepoPath", {
  value: S.String,
})
// #5471: repo / worktree picker. Switch the compact picker between pointing at
// an existing local worktree path and requesting a Pylon-managed worktree for a
// GitHub repo + base ref. The managed inputs build a `repoRef`; the path input
// rides `worktreePath`. Both flow through the existing session.spawn — no new
// control verb.
export const ChangedComposerWorkspaceMode = m("ChangedComposerWorkspaceMode", {
  mode: S.Literals(["worktree", "managed"]),
})
export const ChangedComposerManagedRepo = m("ChangedComposerManagedRepo", {
  value: S.String,
})
export const ChangedComposerManagedBaseRef = m("ChangedComposerManagedBaseRef", {
  value: S.String,
})
// #5471: a managed-worktree request was resolved (or failed) node-side. Carries
// the resolved repoRef so the reducer can fire the spawn it deferred.
export const ResolvedComposerManagedWorktree = m("ResolvedComposerManagedWorktree", {
  result: S.Unknown,
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

// ── #5453/#5821: Verse chat + explicit Blueprint program chat ──────────────
// The default chat button talks to Tassadar/OpenAgents through the host-side
// Verse model turn. The older Blueprint/exact-replay session path is kept as an
// explicit advanced command so it no longer owns first-paint chat semantics.
export const ChangedChatInput = m("ChangedChatInput", { value: S.String })
export const ClickedChatSubmit = m("ClickedChatSubmit")
export const ClickedBlueprintChatSubmit = m("ClickedBlueprintChatSubmit")
export const SucceededVerseTurn = m("SucceededVerseTurn", {
  ok: S.Boolean,
  text: S.String,
  sourceRefs: S.Array(S.String),
  blockerRefs: S.Array(S.String),
})
export const FailedVerseTurn = m("FailedVerseTurn", { error: S.String })
export const SucceededChatTurn = m("SucceededChatTurn", { sessionRef: S.String })
export const FailedChatTurn = m("FailedChatTurn", { error: S.String })

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

// ── Settings preferences (#5472) ─────────────────────────────────────────────
// Each one updates a Model preference field AND persists via PersistPreferences
// (commands.ts) — no new RPC verb. ChangedDefault* also seed the live spawn
// fields so the new default takes effect immediately, not only next launch.
export const ChangedThemePreference = m("ChangedThemePreference", {
  theme: ThemePreference,
})
export const ChangedDefaultAdapter = m("ChangedDefaultAdapter", {
  adapter: DefaultAdapter,
})
export const ChangedDefaultLane = m("ChangedDefaultLane", {
  lane: DefaultLane,
})
export const ToggledNotificationPanel = m("ToggledNotificationPanel", {
  show: S.Boolean,
})
// #5485: route coding inference through the OpenAgents gateway when there is no
// usable own auth ("auto") vs require own auth ("off"). Persisted like the
// other preferences; applied to the live routing decision immediately.
export const ChangedGatewayInferenceFallback = m(
  "ChangedGatewayInferenceFallback",
  { value: GatewayInferenceFallback },
)
// Result of the (best-effort, local) preference write. Carries no payload and
// is a no-op in the reducer — preferences are already in the Model; this only
// closes the PersistPreferences command so side effects stay in Commands, not
// the pure reducer. Failures are swallowed in the command (local convenience).
export const SettledPersistPreferences = m("SettledPersistPreferences")

// ── Zero-base shell (owner directive, 2026-06-19) ───────────────────────────
// The minimal default surface's messages. `ChangedShellInput` tracks the bottom
// text bar; `SubmittedShell` submits the current input (a turn); `RespondedShell`
// is the response landing back (HUD H5 #5503: from the real-model `shellTurn`
// RPC in commands.ts, with the deterministic loopback as the offline/test
// fallback). `OpenedPanes` reveals the KEPT advanced multi-pane UI (explicit
// open → lands on the Code composer); `ClosedPanes` returns to the black shell.
// These are the only verbs the shell needs.
export const ChangedShellInput = m("ChangedShellInput", { value: S.String })
export const CycledShellTarget = m("CycledShellTarget")
export const SelectedShellTarget = m("SelectedShellTarget", { target: ShellTarget })
export const SubmittedShell = m("SubmittedShell")
export const RespondedShell = m("RespondedShell", {
  prompt: S.String,
  text: S.String,
})
export const SucceededShellCodingTurn = m("SucceededShellCodingTurn", {
  target: ShellCodingTarget,
  prompt: S.String,
  sessionRef: S.String,
})
export const FailedShellCodingTurn = m("FailedShellCodingTurn", {
  target: ShellCodingTarget,
  prompt: S.String,
  error: S.String,
})
export const OpenedPanes = m("OpenedPanes")
export const ClosedPanes = m("ClosedPanes")

// ── HUD H3: the managed pane layer (#5501) ──────────────────────────────────
// Open/close/focus a managed pane (pane-as-data) + the drag/resize gesture
// verbs. Each maps to one `PaneLayerAction` in update.ts (the pure PaneManager
// reducer in pane-manager.ts is the only thing that mutates the layer). The
// pointer coordinates ride on the messages so the pure reducer stays DOM-free.
//
// `OpenedManagedPane` opens the EXISTING pane content (by `PaneId`) as a floating
// managed window over the current base surface; the shell is rejected by the
// reducer (it is the base, never a window). `StartedPaneDrag.handle` is null for
// a title-bar move, or one of the 8 resize handles. A persistent window
// pointermove/up subscription (subscriptions.ts) drives `MovedPaneDragPointer` /
// `EndedPaneDrag` while a drag is in flight.
export const OpenedManagedPane = m("OpenedManagedPane", { pane: PaneId })
export const ClosedManagedPane = m("ClosedManagedPane", { paneId: S.String })
export const FocusedManagedPane = m("FocusedManagedPane", { paneId: S.String })
export const ClosedAllManagedPanes = m("ClosedAllManagedPanes")
export const StartedPaneDrag = m("StartedPaneDrag", {
  paneId: S.String,
  drag: PaneDragKind,
  handle: S.NullOr(PaneResizeHandle),
  pointerX: S.Number,
  pointerY: S.Number,
})
export const MovedPaneDragPointer = m("MovedPaneDragPointer", {
  pointerX: S.Number,
  pointerY: S.Number,
})
export const EndedPaneDrag = m("EndedPaneDrag")

export const Message = S.Union([
  GotNodeState,
  GotPylonStats,
  GotChatWorldScene,
  GotChatWorldMultiplayer,
  GotChatWorldPaymentParticle,
  SelectedChatWorldNode,
  ChangedVersePresenceZone,
  ChangedVerseWorldItemProximity,
  ChangedVerseLocalPose,
  SettledVerseLocalPosePublish,
  GotNotifications,
  GotNodeLaunchStatus,
  NavigatedTo,
  NavigatedToGroup,
  OpenedCommandPalette,
  ClosedCommandPalette,
  ChangedCommandPaletteQuery,
  MovedCommandPaletteSelection,
  RanPaletteCommand,
  PressedKey,
  SelectedSession,
  ChangedSessionFilter,
  ToggledEvent,
  ToggledDiffFile,
  ToggledDiffViewMode,
  ToggledArtifactBrowser,
  ToggleVerse,
  ToggledChatMessageDetails,
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
  GotInferenceGatewayReadiness,
  ClickedStartBuiltInAgent,
  SucceededBuiltInAgent,
  FailedBuiltInAgent,
  ClickedRefreshInstallReadiness,
  GotInstallReadiness,
  ClickedRefreshOnboarding,
  GotOnboardingStatus,
  GotIdentityChoiceState,
  ChangedNewIdentityName,
  ClickedUseExistingIdentity,
  ClickedCreateNewIdentity,
  SettledChooseIdentity,
  ClickedRetryOnboarding,
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
  ClickedRefreshPublicActivity,
  GotPublicActivityTimeline,
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
  ChangedProofReplayGeneratedFrom,
  ChangedProofReplayGeneratedTo,
  ChangedProofReplayGeneratedRunRef,
  ChangedProofReplayGeneratedWindowRef,
  ChangedProofReplayGeneratedActorRef,
  ChangedProofReplayGeneratedPairRef,
  ChangedProofReplayGeneratedKind,
  ChangedProofReplayGeneratedSource,
  ChangedProofReplayGeneratedSince,
  ChangedProofReplayGeneratedLimit,
  ClickedLoadGeneratedProofReplay,
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
  ClickedOpenSessionInComposer,
  ChangedSwarmBatchObjectives,
  ChangedSwarmBatchConcurrency,
  ClickedSwarmBatchLaunch,
  SucceededSwarmBatchSpawn,
  FailedSwarmBatchSpawn,
  ChangedComposerRepoPath,
  ChangedComposerWorkspaceMode,
  ChangedComposerManagedRepo,
  ChangedComposerManagedBaseRef,
  ResolvedComposerManagedWorktree,
  ChangedComposerReply,
  ClickedComposerSpawn,
  ClickedComposerReply,
  ClickedComposerNewThread,
  SucceededComposerTurn,
  FailedComposerTurn,
  ChangedChatInput,
  ClickedChatSubmit,
  ClickedBlueprintChatSubmit,
  SucceededVerseTurn,
  FailedVerseTurn,
  SucceededChatTurn,
  FailedChatTurn,
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
  ChangedThemePreference,
  ChangedDefaultAdapter,
  ChangedDefaultLane,
  ToggledNotificationPanel,
  ChangedGatewayInferenceFallback,
  SettledPersistPreferences,
  ChangedShellInput,
  CycledShellTarget,
  SelectedShellTarget,
  SubmittedShell,
  RespondedShell,
  SucceededShellCodingTurn,
  FailedShellCodingTurn,
  OpenedPanes,
  ClosedPanes,
  OpenedManagedPane,
  ClosedManagedPane,
  FocusedManagedPane,
  ClosedAllManagedPanes,
  StartedPaneDrag,
  MovedPaneDragPointer,
  EndedPaneDrag,
])
export type Message = typeof Message.Type
