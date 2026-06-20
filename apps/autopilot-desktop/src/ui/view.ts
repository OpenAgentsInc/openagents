// CL-53: the Foldkit view for the Autopilot Desktop webview.
//
// Replaces the hand-DOM shell + panes/ + cards/. The Verse home is immersive;
// advanced panes use the grouped left sidebar (Code/Supervise/Explore/Settings)
// and a content-pane router once explicitly opened.
//
// Read-only DISPLAY uses the shared `@openagentsinc/autopilot-ui` components
// (typed with message `never` — embedding them inside a view of message Message
// is fine because Html is covariant). All INTERACTIVITY (nav, buttons, inputs,
// approve/deny, cancel, submit, toggle, click-to-expand) is wired with our own
// `foldkit/html` h.* + h.OnClick/h.OnInput against the Message set.

import type {
  NotificationCenterView,
  SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import { renderCloudCard as cloudCardView } from "@openagentsinc/autopilot-control-protocol"
import {
  AccountList,
  DiffReview,
  PublicActivityStrip,
  SessionList,
  type AccountSummary,
} from "@openagentsinc/autopilot-ui"
import type { ProofReplayBundle } from "@openagentsinc/proof-replay"
import { trainingRunView } from "@openagentsinc/three-effect/foldkit"
import {
  defaultTrainingRunNodes,
  trainingRunVisualizationOptionsFromSnapshot,
  type TrainingRunNodeDefinition,
  type TrainingRunOperatorSignalDefinition,
  type TrainingRunOperatorSignalState,
  type TrainingRunPromiseSignalDefinition,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"
import { Option } from "effect"
import type { Attribute, Document, Html } from "foldkit/html"
import { html } from "foldkit/html"
// #5467: the Autonomous loop view's own pane module (Supervise group).
import { autonomousLoopPane } from "./autonomous-loop-pane"
// HUD H7 (#5504): the live status/meters HUD overlay (three-effect H2 kit).
import { statusHudView } from "./hud-status-element"
import {
  OPENAGENTS_PUBLIC_ORIGIN,
  TASSADAR_REPLAY_ORIGIN_DATA_KEY,
  TASSADAR_REPLAY_SLUG_DATA_KEY,
  tassadarProofReplayView,
} from "../../../openagents.com/apps/web/src/scene/tassadarProofReplayElement"
import {
  DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
  desktopProofReplayCatalog,
  type DesktopProofReplayProjection,
} from "../shared/proof-replays"
import { isGatewayBalanceLow } from "../shared/inference-routing"
// #5735: adapt the already-written pylon-network scene onto the three-effect
// bezier graph so the chat can render as glass over a calm 3D world scene.
import { pylonNetworkVisualizationOptions } from "./pylon-network-visualization"
import type {
  PylonNetworkNode,
  PylonNetworkScene,
} from "../shared/pylon-network-scene"
// #5730 (P2.5): feed the LIVE pylon scene + Bitcoin payment particles into the
// chat background scene. Pure projections live in shared/chat-world-*.ts.
import {
  liveChatWorldNetworkScene,
  withChatWorldPaymentLayer,
} from "../shared/chat-world-visualization"
import {
  VERSE_TRAINING_NODE_PREFIX,
  withVerseTrainingLayer,
} from "../shared/verse-training-visualization"
import {
  PYLON_BASE_NODE_PREFIX,
  projectPylonBase,
  withPylonBaseLayer,
  type PylonBaseProjection,
} from "../shared/pylon-base-scene"
import {
  agentCharacterCreationFlag,
  chatWorldBuildFlags,
} from "../shared/chat-world-flags"
import {
  projectCharacterCreationOnboarding,
  type CharacterCreationBeat,
} from "../shared/character-creation-onboarding"

import type {
  AccountRow,
  AppleFmReadinessResponse,
  ApprovalRow,
  AssignmentRow,
  IntentRow,
  BuiltInAgentReadinessResponse,
  InstallReadinessResponse,
  ManagedAccountRow,
  NodeStateMessage,
  OnboardingStep,
  OnboardingStepStatus,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
  SessionArtifactStats,
  SessionEventRow,
  TrainingEvidencePacketSummaryResponse,
  TrainingLeaderboardLaneSummary,
  TrainingOperatorReadinessResponse,
  TrainingPromiseState,
  TrainingPromiseSummary,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "../shared/rpc"
import {
  ChangedPromiseSurfacingClaimText,
  ChangedPromiseSurfacingEnvironment,
  ChangedPromiseSurfacingEvidenceOrSteps,
  ChangedPromiseSurfacingExpectedBehavior,
  ChangedPromiseSurfacingImpact,
  ChangedPromiseSurfacingObservedBehavior,
  ChangedPromiseSurfacingPromiseId,
  ChangedPromiseSurfacingSuggestedState,
  ChangedPromiseSurfacingSurface,
  ChangedAskBody,
  ChangedAskTitle,
  ChangedSessionFilter,
  ChangedComposerRepoPath,
  ChangedComposerWorkspaceMode,
  ChangedComposerManagedRepo,
  ChangedComposerManagedBaseRef,
  ChangedComposerReply,
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ChangedSpawnLane,
  ChangedThemePreference,
  ChangedDefaultAdapter,
  ChangedDefaultLane,
  ToggledNotificationPanel,
  ChangedGatewayInferenceFallback,
  ChangedChatInput,
  ChangedAddAccountHome,
  ChangedAddAccountPriority,
  ChangedAddAccountProvider,
  ChangedAddAccountRef,
  ChangedProofReplayGeneratedActorRef,
  ChangedProofReplayGeneratedFrom,
  ChangedProofReplayGeneratedKind,
  ChangedProofReplayGeneratedLimit,
  ChangedProofReplayGeneratedPairRef,
  ChangedProofReplayGeneratedRunRef,
  ChangedProofReplayGeneratedSince,
  ChangedProofReplayGeneratedSource,
  ChangedProofReplayGeneratedTo,
  ChangedProofReplayGeneratedWindowRef,
  ClickedAddManagedAccount,
  ClickedBumpManagedAccountPriority,
  ClickedLoadGeneratedProofReplay,
  ClickedRefreshManagedAccounts,
  ClickedRemoveManagedAccount,
  SelectedChatWorldNode,
  SelectedComposerAccount,
  ClickedCancelSession,
  ClickedOpenSessionInComposer,
  ChangedSwarmBatchObjectives,
  ChangedSwarmBatchConcurrency,
  ClickedSwarmBatchLaunch,
  ClickedComposerNewThread,
  ClickedComposerReply,
  ClickedComposerSpawn,
  ClickedChatSubmit,
  ClickedActivateTrainingWindow,
  ClickedAdmitTrainingEvidence,
  ClickedBuildTrainingEvidencePacket,
  ClickedClaimTrainingLease,
  ClickedCoordinatorToggle,
  ClickedDeploy,
  ClickedPlanTrainingWindow,
  ClickedQueueTrainingCloseout,
  ClickedReconcileTrainingWindow,
  ClickedRefreshPublicActivity,
  ClickedRefreshProofReplay,
  ClickedRefreshInstallReadiness,
  ClickedRefreshOnboarding,
  ClickedRetryOnboarding,
  ClickedUseExistingIdentity,
  ClickedCreateNewIdentity,
  ChangedNewIdentityName,
  ClickedRefreshPromiseSurfacing,
  ClickedRefreshBuiltInAgent,
  ClickedRefreshAppleFm,
  ClickedRefreshTrainingRuns,
  ClickedQueueTrainingLaunch,
  ClickedResolveApproval,
  ClickedStartAppleFm,
  ClickedRequestTrainingBootstrap,
  ClickedStartBuiltInAgent,
  ClickedSpawn,
  ClickedSubmitIntent,
  ClickedSurfacePromiseGap,
  SelectedAgentMode,
  SelectedProofReplay,
  SelectedTrainingSceneNode,
  type Message,
  NavigatedTo,
  NavigatedToGroup,
  OpenedCommandPalette,
  ClosedCommandPalette,
  ChangedCommandPaletteQuery,
  RanPaletteCommand,
  SelectedSession,
  ToggledEvent,
  ToggledDiffFile,
  ToggledDiffViewMode,
  ToggleVerse,
  ToggledArtifactBrowser,
  ToggledChatMessageDetails,
  ChangedShellInput,
  CycledShellTarget,
  SelectedShellTarget,
  SubmittedShell,
  ClosedPanes,
  // HUD H3 (#5501): managed pane-layer verbs.
  ClosedManagedPane,
  FocusedManagedPane,
  StartedPaneDrag,
} from "./message"
// HUD H3 (#5501): the layer geometry/types + the 8 resize handles, all pure.
import {
  PANE_RESIZE_HANDLES,
  type ManagedPane,
  type PaneLayer,
  type PaneResizeHandle,
} from "./pane-manager"
import {
  DEFAULT_MANAGED_BASE_REF,
  managedWorktreeLabel,
  parseManagedWorktreeRequest,
  worktreePathLabel,
} from "./composer-workspace"
import {
  HOTBAR_SLOTS,
  NAV_GROUPS,
  SHORTCUTS,
  filterPaletteCommands,
  groupForPane,
  type HotbarSlot,
  type NavGroup,
} from "./nav"
// #5472: live theme attribute helper for the Settings preferences.
import { themeAttr } from "./preferences"
import {
  approvalLabel,
  artifactBrowserSections,
  artifactLineText,
  assignmentMeta,
  diffReviewProvenance,
  parseChangeSetFromEvents,
  composerCanReply,
  composerTurnSummary,
  connectionSummary,
  coordinatorToggleLabel,
  eventExpandable,
  eventRowText,
  isComposerTranscriptEvent,
  nodeStatusLine,
  orderSwarmSessions,
  sessionCancellable,
  shipStatusLine,
  stateBreakdown,
  swarmAccountLabel,
  swarmSessionPendingApprovals,
  swarmStatusLabel,
  swarmSummaryLine,
  swarmWorkspaceLabel,
  trainingProjectionMeta,
  verifyLineText,
  walletSummary,
} from "./helpers"
// Concise + markdown-rendered session-stream transcript (owner report,
// 2026-06-19): replaces the raw label/markdown dump with a clean readable view.
import { conciseTranscript } from "./stream-render"
// #5468 (EPIC #5461): bounded auto-approve policy + audit-trail projection.
import {
  boundedAutoApprovalPolicySummary,
  projectAutoApprovalAudit,
  summarizeAutoApprovalAudit,
} from "./auto-approval-view"
import {
  SWARM_BATCH_MAX_CONCURRENCY,
  SWARM_BATCH_MAX_OBJECTIVES,
  buildSwarmTree,
  parseSwarmBatchObjectives,
  swarmBatchRunning,
  swarmBatchStatusLine,
  swarmFailoverRouting,
  swarmRoutingReasonLabel,
} from "./swarm-batch"
import {
  type Model,
  type ChatMessage,
  type ChatStep,
  type PaneId,
  type SessionFilter,
  type ShellTarget,
  modelTrainingActivation,
  modelTrainingBootstrap,
  modelTrainingDashboard,
  modelTrainingEvidenceAdmission,
  modelTrainingEvidencePacketBuild,
  modelTrainingEvidencePacketSummary,
  modelTrainingLease,
  modelProofReplay,
  modelAppleFmReadiness,
  modelBuiltInAgentReadiness,
  modelInferenceDecision,
  modelInferenceGatewayReadiness,
  modelInstallReadiness,
  modelOnboardingStatus,
  modelIdentityChoiceState,
  modelChatWorldParticles,
  modelChatWorldScene,
  modelManagedAccounts,
  modelPaneLayer,
  modelPromiseSurfacingReadiness,
  modelPromiseSurfacingResult,
  modelNode,
  modelNotifications,
  modelPublicActivityTimeline,
  modelTrainingOperatorReadiness,
  modelTrainingPlan,
  modelTrainingPromiseGates,
  modelTrainingReconcile,
  modelTrainingRuns,
} from "./model"

const h = html<Message>()
const cls = (value: string): Attribute<Message> => h.Class(value)

const tassadarProofReplayScene = (
  className: string,
  slug: string = DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
  bundle?: ProofReplayBundle | null,
): Html =>
  tassadarProofReplayView<Message>(
    [
      cls(className),
      h.DataAttribute(TASSADAR_REPLAY_SLUG_DATA_KEY, slug),
      h.DataAttribute(TASSADAR_REPLAY_ORIGIN_DATA_KEY, OPENAGENTS_PUBLIC_ORIGIN),
    ],
    bundle,
  )

// ── Small shared building blocks (own h.* — no hand DOM) ─────────────────────

const card = (title: string, children: ReadonlyArray<Html>): Html =>
  h.section([cls("card")], [h.h2([cls("card-title")], [title]), ...children])

const emptyLine = (text: string): Html => h.p([cls("empty-state")], [text])

const paneTitle = (text: string): Html => h.h1([cls("pane-title")], [text])

const publicActivityEventCountLabel = (count: number): string =>
  `${count} ${count === 1 ? "event" : "events"}`

const publicActivityStatusLabel = (model: Model): string => {
  const projection = modelPublicActivityTimeline(model)
  if (model.publicActivityTimelinePending) return "loading"
  if (projection?.ok === true) {
    const count = projection.envelope?.events.length ?? 0
    const stale =
      projection.envelope?.sourceLag.filter(lag => lag.status !== "current")
        .length ?? 0
    return stale > 0
      ? `${publicActivityEventCountLabel(count)} · ${stale} warnings`
      : publicActivityEventCountLabel(count)
  }
  return "unavailable"
}

const publicActivityPane = (
  model: Model,
  input: Readonly<{ className: string; maxEvents: number; title: string }>,
): Html => {
  const projection = modelPublicActivityTimeline(model)
  return h.div([cls(`public-activity-desktop ${input.className}`)], [
    h.div([cls("public-activity-desktop-actions")], [
      h.button(
        [
          cls("public-activity-refresh"),
          h.Type("button"),
          h.Disabled(model.publicActivityTimelinePending),
          h.OnClick(ClickedRefreshPublicActivity()),
        ],
        [model.publicActivityTimelinePending ? "Refreshing" : "Refresh"],
      ),
    ]),
    PublicActivityStrip({
      className: "public-activity-desktop-strip",
      emptyLabel:
        projection?.error === undefined
          ? "No public activity events loaded."
          : `Public activity unavailable: ${projection.error}`,
      envelope: projection?.envelope ?? null,
      maxEvents: input.maxEvents,
      pending: model.publicActivityTimelinePending,
      sourceUrl: projection?.sourceUrl ?? null,
      statusLabel: publicActivityStatusLabel(model),
      title: input.title,
    }),
  ])
}

// ── Sidebar (grouped, #5463; audit §5.2) ─────────────────────────────────────
//
// The primary sidebar shows the ~5 intent-named GROUPS from the nav registry
// (nav.ts), never a flat per-pane wall. The active group also renders a
// secondary in-section strip of its destinations, so every pane stays reachable
// without growing the top level. Group buttons / sub-pane buttons all dispatch
// the existing NavigatedTo / NavigatedToGroup messages — Phase-2 panes appear
// here automatically by adding one registry entry (see nav.ts seam comment).

const sidebarStatusLabel = (node: NodeStateMessage | null): string => {
  if (!node) return "connecting…"
  const count = node.sessions.length
  return node.ok ? `online · ${count} ${count === 1 ? "session" : "sessions"}` : "offline"
}

const coordinatorToggle = (node: NodeStateMessage | null): Html => {
  const paused = node?.coordinatorPaused ?? null
  if (paused === null) return h.empty
  return h.div(
    [cls("coord-slot")],
    [
      h.button(
        [
          cls(`coord-toggle ${paused ? "coord-paused" : ""}`),
          h.Type("button"),
          h.OnClick(ClickedCoordinatorToggle({ paused: !paused })),
        ],
        [coordinatorToggleLabel(paused)],
      ),
    ],
  )
}

// The pending-approvals badge follows the pane it belongs to (decisions), so it
// surfaces on whichever group owns that pane (Supervise) — preserving the old
// badge without a flat Decisions button.
const groupPendingCount = (group: NavGroup, pendingCount: number): number =>
  group.destinations.some((dest) => dest.pane === "decisions") ? pendingCount : 0

const primaryNavButton = (
  model: Model,
  group: NavGroup,
  activeGroupId: string | null,
  pendingCount: number,
): Html => {
  const badge = groupPendingCount(group, pendingCount)
  const isActive = group.id === activeGroupId
  return h.button(
    [
      cls(`nav-item nav-group${isActive ? " active" : ""}`),
      h.Type("button"),
      h.OnClick(NavigatedToGroup({ group: group.id })),
    ],
    [
      h.span([cls("nav-accel")], [`⌘${group.accel}`]),
      h.span([cls("nav-group-label")], [group.label]),
      badge > 0 ? h.span([cls("nav-badge")], [String(badge)]) : h.empty,
    ],
  )
}

// The secondary in-section strip for the active group. Only rendered when the
// group has more than one destination (a single-destination group like Chat
// needs no strip).
const secondaryNavStrip = (model: Model, group: NavGroup | null): Html => {
  if (!group || group.destinations.length <= 1) return h.empty
  return h.div(
    [cls("nav-subgroup")],
    group.destinations.map((dest) =>
      h.button(
        [
          cls(`nav-subitem${model.pane === dest.pane ? " active" : ""}`),
          h.Type("button"),
          h.OnClick(NavigatedTo({ pane: dest.pane })),
        ],
        [dest.label],
      ),
    ),
  )
}

const sidebar = (model: Model): Html => {
  const node = modelNode(model)
  const pendingCount = node?.approvals?.length ?? 0
  const activeGroup = groupForPane(model.pane)
  const activeGroupId = activeGroup?.id ?? null

  return h.nav(
    [cls("sidebar")],
    [
      h.div(
        [cls("sidebar-header")],
        [
          h.div([cls("sidebar-title")], ["🛩️ Autopilot"]),
          h.div(
            [cls(`sidebar-status ${node?.ok ? "status-online" : "status-offline"}`)],
            [sidebarStatusLabel(node)],
          ),
        ],
      ),
      // The command palette release valve (#5464): one always-visible affordance
      // that opens the "everything" surface, so depth lives in search.
      h.button(
        [cls("nav-palette"), h.Type("button"), h.OnClick(OpenedCommandPalette())],
        [h.span([cls("nav-palette-label")], ["Search commands"]), h.span([cls("nav-accel")], ["⌘K"])],
      ),
      coordinatorToggle(node),
      ...NAV_GROUPS.map((group) =>
        primaryNavButton(model, group, activeGroupId, pendingCount),
      ),
      secondaryNavStrip(model, activeGroup),
    ],
  )
}

// ── Command palette overlay (#5464) ──────────────────────────────────────────
// A searchable overlay over the typed command registry (nav.ts). The query +
// highlighted index live in the Model, so this is a pure function of state; keys
// (↑/↓/Enter/Esc) are handled by the keyboard layer (#5465). Clicking a row
// dispatches the existing RanPaletteCommand (which also closes); clicking the
// backdrop closes. No new control verb (audit §5.2).
//
// Foldkit's OnClick has no stopPropagation variant, so the dialog itself carries
// NO click handler — only the backdrop closes, and only an actual backdrop click
// (outside the dialog box) reaches it. Esc (the keyboard layer) is the primary
// dismiss. Clicks on the input/rows hit their own handlers; they do bubble to the
// backdrop, but the input is the only inert inner target and a stray close there
// is recoverable (Cmd-K reopens), so we accept that over a brittle no-op guard.
const commandPalette = (model: Model): Html => {
  if (!model.commandPaletteOpen) return h.empty
  const matches = filterPaletteCommands(model.commandPaletteQuery)
  const selected =
    matches.length === 0
      ? -1
      : Math.min(Math.max(model.commandPaletteIndex, 0), matches.length - 1)
  return h.div(
    [cls("palette-backdrop"), h.OnClick(ClosedCommandPalette())],
    [
      h.div(
        [cls("palette-dialog"), h.Role("dialog")],
        [
          h.input([
            cls("palette-input"),
            h.Type("text"),
            h.Placeholder("Type a command or destination…"),
            h.Value(model.commandPaletteQuery),
            h.Autofocus(true),
            h.OnInput((value: string) => ChangedCommandPaletteQuery({ value })),
          ]),
          matches.length === 0
            ? h.div([cls("palette-empty")], ["No matching commands."])
            : h.ul(
                [cls("palette-list")],
                matches.map((match, index) =>
                  h.li(
                    [
                      cls(`palette-row${index === selected ? " active" : ""}`),
                      h.OnClick(RanPaletteCommand({ commandId: match.command.id })),
                    ],
                    [
                      h.span([cls("palette-row-label")], [match.command.label]),
                      h.span([cls("palette-row-group")], [match.command.group]),
                    ],
                  ),
                ),
              ),
        ],
      ),
    ],
  )
}

// ── HUD H1: the numbered hotbar (#5499) ──────────────────────────────────────
// A StarCraft-style bottom-center command bar (the recurring OpenAgents HUD
// pattern — Commander's `Hotbar.tsx`, the WGPUI K-slot bar; see
// docs/launch/2026-06-19-previous-hud-systems-audit.md §3/§7). It is the FOURTH
// consumer of the one nav registry: slots come straight from `HOTBAR_SLOTS`
// (derived from `NAV_GROUPS` in nav.ts), NOT a parallel list. Each slot
// dispatches an EXISTING message only for the ⌘K command-palette slot. The
// numbered group cells are intentionally blank/inert placeholders per the
// owner directive (2026-06-19): no click action, no icon, no label, no pane open.
//
// Rendered inside the zero-base shell input row on the bottom-left, and as a
// floating bottom-left compact strip on the full UI. Group slots are decorative
// blanks; ⌘K remains clickable.
const hotbarSlotView = (model: Model, slot: HotbarSlot): Html => {
  if (slot.kind === "palette") {
    const title = `${slot.label} (${slot.chord})`
    return h.button(
      [
        cls("hotbar-slot hotbar-slot-button hotbar-slot-palette"),
        h.Type("button"),
        h.Title(title),
        h.AriaLabel(title),
        h.OnClick(OpenedCommandPalette()),
      ],
      [
        h.span([cls("hotbar-slot-chord"), h.AriaHidden("true")], [slot.chord]),
        h.span([cls("hotbar-slot-tooltip")], [title]),
      ],
    )
  }
  // #5730 The Verse: an active toggle slot. Lit (on) when the runtime toggle is
  // enabled; clicking (or ⌘⇧V) flips it. The world view itself only renders when
  // the CHAT_WORLD_SCENE build flag is also on, but the toggle is always shown so
  // the user-facing control is discoverable.
  if (slot.kind === "verse") {
    const on = model.verseEnabled
    const title = `${slot.label}: ${on ? "on" : "off"} (${slot.chord})`
    return h.button(
      [
        cls(
          `hotbar-slot hotbar-slot-button hotbar-slot-verse${
            on ? " hotbar-slot-verse-on" : ""
          }`,
        ),
        h.Type("button"),
        h.Title(title),
        h.AriaLabel(title),
        h.AriaPressed(on ? "true" : "false"),
        h.OnClick(ToggleVerse()),
      ],
      [
        h.span([cls("hotbar-slot-label")], [slot.label]),
        h.span([cls("hotbar-slot-tooltip")], [title]),
      ],
    )
  }
  return h.div(
    [cls("hotbar-slot hotbar-slot-empty"), h.AriaHidden(true)],
    [],
  )
}

const hotbar = (
  model: Model,
  placement: "floating" | "inline" = "floating",
): Html =>
  h.div(
    [
      cls(`hotbar hotbar-${placement}`),
      h.Role("toolbar"),
      h.AriaLabel("Hotbar"),
    ],
    HOTBAR_SLOTS.map((slot) => hotbarSlotView(model, slot)),
  )

// ── Nodes pane ────────────────────────────────────────────────────────────────

const deployCard = (model: Model): Html => {
  const node = modelNode(model)
  const projected = node?.deploy ?? null
  const feedback = model.deployFeedback
  const state = feedback?.state ?? projected?.state ?? "unknown"
  const text =
    feedback?.text ??
    (projected ? `${projected.state} · ${projected.message}` : "no deploy yet")

  return h.section(
    [cls("card"), h.Id("deploy")],
    [
      h.h2([cls("card-title")], ["Deploy to Cloud"]),
      h.p(
        [cls("deploy-help")],
        [
          "Deploy this node's Cloud Run service (cloudrun · main · production) through our pipeline. Disabled unless the node has OA_DEPLOY_ENABLE=1.",
        ],
      ),
      h.button([h.Type("button"), h.OnClick(ClickedDeploy())], ["Deploy to Cloud"]),
      h.p([cls(`deploy-status deploy-${state}`)], [text]),
    ],
  )
}

const askCard = (model: Model): Html => {
  const node = modelNode(model)
  const intents: ReadonlyArray<IntentRow> = node?.intents ?? []
  const statusVisible = model.askStatus.tone !== "idle"

  const askForm = card("Ask Autopilot", [
    h.input([
      cls("text-input"),
      h.Type("text"),
      h.Placeholder("title — what do you want done?"),
      h.Value(model.askTitle),
      h.OnInput((value: string) => ChangedAskTitle({ value })),
    ]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(3),
        h.Placeholder("details (optional)"),
        h.Value(model.askBody),
        h.OnInput((value: string) => ChangedAskBody({ value })),
      ],
      [],
    ),
    h.button(
      [h.Type("button"), h.Disabled(model.askPending), h.OnClick(ClickedSubmitIntent())],
      [model.askPending ? "sending…" : "Send to node"],
    ),
    statusVisible
      ? h.p([cls(`deploy-status ask-${model.askStatus.tone}`)], [model.askStatus.text])
      : h.empty,
  ])

  const asksList = card(
    "Your asks",
    intents.length === 0
      ? [emptyLine("No asks yet.")]
      : [
          h.ul(
            [cls("asks-list")],
            intents.slice(0, 5).map((intent) => {
              const sl = shipStatusLine(intent.status)
              const label =
                intent.title.trim() !== "" ? intent.title : intent.intentId.slice(-8)
              return h.li(
                [cls("ask-row")],
                [
                  h.span(
                    [cls("ask-dot"), h.Style({ backgroundColor: sl.dotColor })],
                    [],
                  ),
                  h.span([cls("ask-text")], [`${label} · ${sl.text}`]),
                ],
              )
            }),
          ),
        ],
  )

  return h.div([], [askForm, asksList])
}

const approvalRowView = (approval: ApprovalRow): Html =>
  h.div(
    [cls("approval-row"), h.DataAttribute("autopilot-approval-ref", approval.approvalRef)],
    [
      h.p([cls("approval-prompt")], [approvalLabel(approval)]),
      h.div(
        [cls("approval-buttons")],
        [
          h.button(
            [
              cls("btn-approve"),
              h.Type("button"),
              h.OnClick(
                ClickedResolveApproval({
                  approvalRef: approval.approvalRef,
                  decision: "approve",
                }),
              ),
            ],
            ["Approve"],
          ),
          h.button(
            [
              cls("btn-deny"),
              h.Type("button"),
              h.OnClick(
                ClickedResolveApproval({
                  approvalRef: approval.approvalRef,
                  decision: "deny",
                }),
              ),
            ],
            ["Deny"],
          ),
        ],
      ),
    ],
  )

const pendingApprovals = (model: Model): ReadonlyArray<ApprovalRow> => {
  const node = modelNode(model)
  const resolved = new Set(model.resolvedApprovals)
  return (node?.approvals ?? []).filter((a) => !resolved.has(a.approvalRef))
}

const approvalsCard = (model: Model): Html => {
  const approvals = pendingApprovals(model)
  if (approvals.length === 0) return h.empty
  return card(`Needs you (${approvals.length})`, approvals.map(approvalRowView))
}

const balanceCard = (model: Model): Html => {
  const wallet = modelNode(model)?.wallet ?? null
  if (!wallet) return h.empty
  const { value, summary } = walletSummary(wallet)
  return card("Balance", [
    h.p([cls("balance-value")], [value]),
    h.p([cls("balance-summary")], [summary]),
  ])
}

const assignmentRowView = (row: AssignmentRow): Html => {
  const { goal, meta } = assignmentMeta(row)
  return h.div(
    [cls("assignment-row")],
    [h.div([cls("assignment-goal")], [goal]), h.div([cls("assignment-meta")], [meta])],
  )
}

const assignmentsCard = (model: Model): Html => {
  const assignments: ReadonlyArray<AssignmentRow> =
    modelNode(model)?.assignments ?? []
  if (assignments.length === 0) return h.empty
  return card(`Assignments (${assignments.length})`, [
    h.p([cls("card-subtitle")], ["open work leases · read-only"]),
    ...assignments.map(assignmentRowView),
  ])
}

const cloudCard = (model: Model): Html => {
  const view = cloudCardView(modelNode(model) ?? null)
  if (!view.visible) return h.empty
  return card(view.title, [
    h.p([], [view.body]),
    h.p([cls("cloud-failover")], ["Provider failover: see Accounts."]),
  ])
}

// AccountRow (rpc.ts) → AccountSummary (autopilot-ui). Read-only display via the
// shared AccountList component. CS-A1: use the node's stable accountRefHash and
// keep readiness state honest (ready vs blocked).
const toAccountSummary = (row: AccountRow): AccountSummary => ({
  accountRefHash: row.accountRefHash,
  provider: row.provider,
  state: row.ready ? "ready" : "quota_blocked",
})

// CS-A1: a short, public-safe label for an account in the per-session picker.
// Prefer the explicit registry ref; fall back to provider + default-home note.
const accountPickerLabel = (row: AccountRow): string => {
  const base =
    row.accountRef !== null
      ? `${row.provider} · ${row.accountRef}`
      : `${row.provider} · default home`
  return row.ready ? base : `${base} (blocked)`
}

const accountsSection = (node: NodeStateMessage): Html => {
  const accounts = node.accounts ?? []
  if (accounts.length === 0) return h.empty
  return card("Accounts", [AccountList({ accounts: accounts.map(toAccountSummary) })])
}

const notificationsSection = (view: NotificationCenterView): Html => {
  const heading = view.unread > 0 ? `Notifications · ${view.unread}` : "Notifications"
  return h.section(
    [cls("notifications"), h.Id("notifications")],
    [
      h.header(
        [cls(`notif-header ${view.hasHigh ? "notif-has-high" : ""}`)],
        [h.h2([], [heading])],
      ),
      view.items.length === 0
        ? emptyLine("No notifications yet.")
        : h.ul(
            [cls("notif-list")],
            view.items.map((item) =>
              h.li(
                [cls(`notif-row notif-${item.priority}`)],
                [
                  h.span([cls("notif-title")], [item.title]),
                  h.span([cls("notif-body")], [item.body]),
                ],
              ),
            ),
          ),
    ],
  )
}

const sessionsPreview = (node: NodeStateMessage): Html =>
  card("Sessions", [
    h.div(
      [cls("session-preview-list")],
      node.sessions.length === 0
        ? [emptyLine("No sessions yet.")]
        : node.sessions.map((session) =>
            h.div(
              [
                cls("session-click"),
                h.Tabindex(0),
                h.DataAttribute("autopilot-session-ref", session.sessionRef),
                h.OnClick(SelectedSession({ sessionRef: session.sessionRef })),
              ],
              [SessionList({ sessions: [session] })],
            ),
          ),
    ),
  ])

// #5025: honest node-launch lifecycle badge, fed by the Bun supervisor's
// onStatus over the `nodeLaunchStatus` message. Distinct from the live
// node-state poll above — this says whether the app launched/adopted/failed to
// bring up the local node. No fake "online".
const NODE_LAUNCH_LABEL: Record<string, string> = {
  launching: "Launching local node…",
  online: "Local node online",
  adopted: "Adopted running node",
  failed: "Local node failed to start",
  unavailable: "No bundled node (discover-only)",
}

const nodeLaunchBadge = (model: Model): Html => {
  const status = model.nodeLaunchStatus
  if (status === null) return h.empty
  return h.p(
    [cls(`node-launch-badge node-launch-${status}`)],
    [NODE_LAUNCH_LABEL[status] ?? status],
  )
}

const nodesPane = (model: Model): Html => {
  const node = modelNode(model)
  const notifications = modelNotifications(model)
  return h.div(
    [],
    [
      paneTitle("Autopilot"),
      h.p(
        [cls("node-status")],
        [node ? nodeStatusLine({ ok: node.ok, sessions: node.sessions }) : "connecting…"],
      ),
      nodeLaunchBadge(model),
      deployCard(model),
      askCard(model),
      approvalsCard(model),
      balanceCard(model),
      assignmentsCard(model),
      cloudCard(model),
      node ? accountsSection(node) : h.empty,
      notifications ? notificationsSection(notifications) : h.empty,
      node ? sessionsPreview(node) : emptyLine("Connecting…"),
    ],
  )
}

// ── Training pane ────────────────────────────────────────────────────────────

const trainingMetric = (label: string, value: string, tone = "ready"): Html =>
  h.div([cls(`training-metric training-${tone}`)], [
    h.span([cls("training-metric-label")], [label]),
    h.strong([cls("training-metric-value")], [value]),
  ])

const trainingGate = (
  label: string,
  value: string,
  tone: "ready" | "watch" | "blocked",
): Html =>
  h.li([cls(`training-gate training-${tone}`)], [
    h.span([cls("training-gate-dot")], []),
    h.span([cls("training-gate-label")], [label]),
    h.span([cls("training-gate-value")], [value]),
  ])

type TrainingGateTone = "ready" | "watch" | "blocked"

type TrainingStatusTone = "error" | "info" | "success" | "idle"

type TrainingStatusLike = {
  readonly text: string
  readonly tone: TrainingStatusTone
}

const trainingStatusTone = (
  status: TrainingStatusLike,
  pending = false,
): TrainingGateTone =>
  pending
    ? "watch"
    : status.tone === "success"
      ? "ready"
      : status.tone === "error"
        ? "blocked"
        : "watch"

const trainingStatusText = (
  status: TrainingStatusLike,
  fallback: string,
): string => {
  const trimmed = status.text.trim()
  return trimmed.length > 0 ? trimmed : fallback
}

const uniqueTrainingRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): string[] => {
  const seen = new Set<string>()
  const result: string[] = []
  for (const ref of refs) {
    const trimmed = ref?.trim() ?? ""
    if (trimmed === "" || seen.has(trimmed)) continue
    seen.add(trimmed)
    result.push(trimmed)
  }
  return result
}

const trainingRefList = (
  title: string,
  refs: readonly string[],
  emptyText = "not observed",
): Html =>
  h.div([cls("training-ledger-block")], [
    h.h3([cls("training-ledger-title")], [title]),
    refs.length === 0
      ? h.p([cls("training-ledger-empty")], [emptyText])
      : h.ul(
          [cls("training-ledger-list")],
          refs.slice(0, 8).map(ref =>
            h.li([cls("training-ledger-ref")], [h.code([], [ref])]),
          ),
        ),
  ])

const readinessFlag = (value: boolean): string => value ? "ready" : "missing"

const trainingOperatorReadinessRows = (
  readiness: TrainingOperatorReadinessResponse | null,
  model: Model,
): readonly Html[] => {
  if (readiness === null) {
    return [
      trainingGate(
        "operator readiness",
        trainingStatusText(model.trainingOperatorReadinessStatus, "not loaded"),
        trainingStatusTone(
          model.trainingOperatorReadinessStatus,
          model.trainingOperatorReadinessPending,
        ),
      ),
    ]
  }

  return [
    trainingGate(
      "admin plan gate",
      `${readiness.adminEnabled ? "enabled" : "disabled"} · token ${readinessFlag(readiness.adminTokenPresent)}`,
      readiness.adminReady ? "ready" : "blocked",
    ),
    trainingGate(
      "lease gate",
      `${readiness.leaseEnabled ? "enabled" : "disabled"} · pylon ${readinessFlag(readiness.pylonRefPresent)}`,
      readiness.leaseReady ? "ready" : "blocked",
    ),
    trainingGate(
      "local Pylon",
      `home ${readinessFlag(readiness.pylonHomePresent)} · token ${readinessFlag(readiness.controlTokenPresent)}`,
      readiness.localPylonReady ? "ready" : "blocked",
    ),
    trainingGate(
      "evidence packet",
      `${readiness.evidenceEnabled ? "enabled" : "disabled"} · packet ${readinessFlag(readiness.evidencePacketPathPresent)}`,
      readiness.evidenceReady ? "ready" : "blocked",
    ),
    trainingGate(
      "pylon ref source",
      readiness.pylonRef === null
        ? `${readiness.pylonRefSource} · missing`
        : `${readiness.pylonRefSource} · ${readiness.pylonRef}`,
      readiness.pylonRefPresent ? "ready" : "blocked",
    ),
    trainingGate(
      "training base",
      readiness.trainingBaseUrl,
      readiness.ok ? "ready" : "watch",
    ),
  ]
}

const trainingOperatorReadinessPanel = (model: Model): Html => {
  const readiness = modelTrainingOperatorReadiness(model)
  const blockerRefs = readiness?.blockerRefs ?? []
  return h.section([cls("training-panel training-operator-readiness-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Operator Readiness"]),
      h.span([cls("training-panel-kicker")], [
        trainingStatusText(model.trainingOperatorReadinessStatus, "not loaded"),
      ]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Public-safe Bun readiness for admin planning, lease claims, local Pylon control, and bootstrap prerequisites.",
    ]),
    h.ul(
      [cls("training-gates training-operator-readiness")],
      trainingOperatorReadinessRows(readiness, model),
    ),
    h.ul([cls("training-api-list training-readiness-blockers")], [
      blockerRefs.length === 0
        ? h.li([], ["no readiness blockers"])
        : h.li([], [
            `${blockerRefs.length} blockers`,
          ]),
      ...blockerRefs.map(ref => h.li([], [h.code([], [ref])])),
    ]),
  ])
}

const trainingEvidencePacketLossText = (
  summary: TrainingEvidencePacketSummaryResponse,
): string =>
  summary.finalValidationLoss === null || summary.maxValidationLoss === null
    ? "loss budget missing"
    : `${summary.finalValidationLoss} / ${summary.maxValidationLoss}`

const trainingEvidencePacketRows = (
  summary: TrainingEvidencePacketSummaryResponse | null,
  model: Model,
): readonly Html[] => {
  if (summary === null) {
    return [
      trainingGate(
        "packet summary",
        trainingStatusText(
          model.trainingEvidencePacketSummaryStatus,
          "not loaded",
        ),
        trainingStatusTone(
          model.trainingEvidencePacketSummaryStatus,
          model.trainingEvidencePacketSummaryPending,
        ),
      ),
    ]
  }

  const refCount =
    Number(summary.budgetRefPresent) +
    Number(summary.evalRefPresent) +
    Number(summary.mergeRefPresent)

  return [
    trainingGate(
      "packet source",
      summary.configured
        ? summary.packetSource ?? "configured"
        : "not configured",
      summary.configured ? "ready" : "blocked",
    ),
    trainingGate(
      "loss budget",
      trainingEvidencePacketLossText(summary),
      summary.finalValidationLoss !== null &&
        summary.maxValidationLoss !== null &&
        summary.finalValidationLoss <= summary.maxValidationLoss
        ? "ready"
        : "blocked",
    ),
    trainingGate(
      "budget label",
      summary.budgetLabel ?? "not supplied",
      summary.budgetLabel === null ? "watch" : "ready",
    ),
    trainingGate(
      "merge/eval/budget refs",
      `${refCount}/3 present`,
      refCount === 3 ? "ready" : "blocked",
    ),
    trainingGate(
      "distinct Pylons",
      `${summary.distinctPylonCount}/2 observed`,
      summary.distinctPylonCount >= 2 ? "ready" : "blocked",
    ),
  ]
}

const trainingEvidencePacketPanel = (model: Model): Html => {
  const summary = modelTrainingEvidencePacketSummary(model)
  const blockerRefs = summary?.blockerRefs ?? []
  return h.section([cls("training-panel training-evidence-packet-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Evidence Packet"]),
      h.span([cls("training-panel-kicker")], [
        trainingStatusText(
          model.trainingEvidencePacketSummaryStatus,
          "not loaded",
        ),
      ]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Public-safe Bun inspection of the configured local packet before admission; only counts, booleans, and blocker refs reach the webview.",
    ]),
    h.div([cls("training-metrics training-evidence-packet-metrics")], [
      trainingMetric(
        "receipts",
        String(summary?.receiptRefCount ?? 0),
        summary?.receiptRefCount ? "ready" : "watch",
      ),
      trainingMetric(
        "shards",
        String(summary?.shardContributionCount ?? 0),
        summary?.shardContributionCount ? "ready" : "watch",
      ),
      trainingMetric(
        "pylons",
        String(summary?.distinctPylonCount ?? 0),
        (summary?.distinctPylonCount ?? 0) >= 2 ? "ready" : "watch",
      ),
      trainingMetric(
        "loss points",
        String(summary?.lossPointCount ?? 0),
        (summary?.lossPointCount ?? 0) >= 2 ? "ready" : "watch",
      ),
      trainingMetric(
        "freivalds",
        String(summary?.freivaldsCommitmentRefCount ?? 0),
        summary?.freivaldsCommitmentRefCount ? "ready" : "watch",
      ),
      trainingMetric(
        "closeouts",
        String(summary?.gradientCloseoutRefCount ?? 0),
        summary?.gradientCloseoutRefCount ? "ready" : "watch",
      ),
    ]),
    h.ul(
      [cls("training-gates training-evidence-packet-gates")],
      trainingEvidencePacketRows(summary, model),
    ),
    h.ul([cls("training-api-list training-evidence-packet-blockers")], [
      blockerRefs.length === 0
        ? h.li([], ["no packet blockers"])
        : h.li([], [`${blockerRefs.length} blockers`]),
      ...blockerRefs.map(ref => h.li([], [h.code([], [ref])])),
    ]),
  ])
}

const selectedTrainingSummary = (
  projection: TrainingRunsResponse | null,
): TrainingRunSummaryRow | null => {
  const summaries = projection?.summaries ?? []
  return (
    summaries.find(summary =>
      summary.run.promiseRef.includes("first_real_model_training_run"),
    ) ??
    summaries[0] ??
    null
  )
}

const trainingSummaryByRunRef = (
  projection: TrainingRunsResponse | null,
  runRef: string | null | undefined,
): TrainingRunSummaryRow | null => {
  const target = runRef?.trim() ?? ""
  if (target === "") return null
  return (
    projection?.summaries.find(summary => summary.run.trainingRunRef === target) ??
    null
  )
}

const trainingWindowByRef = (
  projection: TrainingRunsResponse | null,
  windowRef: string | null | undefined,
) => {
  const target = windowRef?.trim() ?? ""
  if (target === "") return null
  for (const summary of projection?.summaries ?? []) {
    const match = summary.windows.find(window => window.windowRef === target)
    if (match !== undefined) return match
  }
  return null
}

const trainingWindowStateRank = (state: string): number => {
  switch (state) {
    case "planned":
      return 0
    case "active":
      return 1
    case "sealed":
      return 2
    case "reconciled":
      return 3
    default:
      return -1
  }
}

const activationWindowRef = (model: Model): string | null => {
  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  const projectedPlannedWindow =
    summary?.windows.find(window => window.state === "planned")?.windowRef ??
    null
  if (projectedPlannedWindow !== null) return projectedPlannedWindow

  const planWindowRef = modelTrainingPlan(model)?.windowRef ?? null
  const activation = modelTrainingActivation(model)
  if (
    planWindowRef !== null &&
    !(activation?.ok === true && activation.windowRef === planWindowRef)
  ) {
    return planWindowRef
  }

  return null
}

const hasClaimableTrainingWindow = (model: Model): boolean => {
  const activation = modelTrainingActivation(model)
  if (activation?.ok === true) return true
  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  return summary?.windows.some(window => window.state === "active") ?? false
}

const closeoutWindowRef = (model: Model): string | null => {
  const lease = modelTrainingLease(model)?.lease ?? null
  if (lease !== null) return lease.windowRef

  const activation = modelTrainingActivation(model)
  if (activation?.ok === true) return activation.windowRef

  const bootstrap = modelTrainingBootstrap(model)
  if (bootstrap?.outcome?.kind === "granted") {
    return bootstrap.outcome.grant.sealedWindowRef
  }

  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  return (
    summary?.windows.find(window => window.state === "active")?.windowRef ??
    summary?.windows.find(window => window.state === "sealed")?.windowRef ??
    summary?.windows.find(window => window.state === "planned")?.windowRef ??
    modelTrainingPlan(model)?.windowRef ??
    null
  )
}

const reconcileWindowRef = (model: Model): string | null => {
  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  const projectedSealedWindow =
    summary?.windows.find(window => window.state === "sealed")?.windowRef ??
    null
  if (projectedSealedWindow !== null) return projectedSealedWindow

  const reconcile = modelTrainingReconcile(model)
  if (reconcile?.ok === true) return null

  return null
}

const lifecycleCountsFromTrainingSummary = (summary: TrainingRunSummaryRow) => {
  const metrics = summary.metrics
  const device = summary.realGradient.deviceRequirement
  const assigned = Math.max(0, metrics.assignedContributorCount.value)
  const observed = Math.max(0, device.observedDistinctContributorDevices)
  const durableSeals =
    metrics.sealedWindowCount.value + metrics.reconciledWindowCount.value
  return {
    active: metrics.verifiedWorkCount.value,
    qualified: observed,
    registered: Math.max(0, assigned - observed),
    state_synced: durableSeals > 0 ? Math.max(1, observed) : 0,
    sync_reentry: metrics.rejectedWorkCount.value,
    warmup:
      metrics.activeWindowCount.value + metrics.plannedWindowCount.value > 0
        ? Math.max(1, assigned - metrics.verifiedWorkCount.value)
        : 0,
  }
}

const promiseSignalLabel = (promiseId: string): string => {
  const simplified = promiseId
    .replace(/^training\./, "")
    .replace(/\.v\d+$/, "")
    .replaceAll("_", " ")
  return simplified.length > 16 ? `${simplified.slice(0, 15)}...` : simplified
}

const trainingPromiseSignals = (
  promises: readonly TrainingPromiseSummary[],
): readonly TrainingRunPromiseSignalDefinition[] =>
  promises.slice(0, 7).map(promise => ({
    blockerCount: promise.blockerRefs.length,
    evidenceRefCount: promise.evidenceRefCount,
    id: promise.promiseId,
    label: promiseSignalLabel(promise.promiseId),
    state: promise.state,
  }))

const operatorSignalState = (
  status: TrainingStatusLike,
  pending: boolean,
): TrainingRunOperatorSignalState =>
  pending
    ? "info"
    : status.tone === "success"
      ? "success"
      : status.tone === "error"
        ? "error"
        : status.tone === "info"
          ? "info"
          : "idle"

const operatorSignalDetail = (
  status: TrainingStatusLike,
  fallback: string,
): string => {
  const detail = trainingStatusText(status, fallback).replace(/\s+/g, " ")
  return detail.length > 18 ? `${detail.slice(0, 17)}...` : detail
}

const trainingOperatorSignals = (
  model: Model,
): readonly TrainingRunOperatorSignalDefinition[] => [
  {
    detail: operatorSignalDetail(
      model.trainingOperatorReadinessStatus,
      "not loaded",
    ),
    id: "readiness",
    label: "ready",
    state: operatorSignalState(
      model.trainingOperatorReadinessStatus,
      model.trainingOperatorReadinessPending,
    ),
  },
  {
    detail: operatorSignalDetail(
      model.trainingEvidencePacketSummaryStatus,
      "not loaded",
    ),
    id: "packet",
    label: "packet",
    state: operatorSignalState(
      model.trainingEvidencePacketSummaryStatus,
      model.trainingEvidencePacketSummaryPending,
    ),
  },
  {
    detail: operatorSignalDetail(model.trainingPlanStatus, "idle"),
    id: "plan",
    label: "plan",
    state: operatorSignalState(model.trainingPlanStatus, model.trainingPlanPending),
  },
  {
    detail: operatorSignalDetail(model.trainingActivationStatus, "idle"),
    id: "activate",
    label: "activate",
    state: operatorSignalState(
      model.trainingActivationStatus,
      model.trainingActivationPending,
    ),
  },
  {
    detail: operatorSignalDetail(model.trainingLeaseStatus, "idle"),
    id: "lease",
    label: "lease",
    state: operatorSignalState(model.trainingLeaseStatus, model.trainingLeasePending),
  },
  {
    detail: operatorSignalDetail(model.trainingBootstrapStatus, "idle"),
    id: "bootstrap",
    label: "bootstrap",
    state: operatorSignalState(
      model.trainingBootstrapStatus,
      model.trainingBootstrapPending,
    ),
  },
  {
    detail: operatorSignalDetail(model.trainingCloseoutStatus, "idle"),
    id: "closeout",
    label: "closeout",
    state: operatorSignalState(
      model.trainingCloseoutStatus,
      model.trainingCloseoutPending,
    ),
  },
  {
    detail: operatorSignalDetail(
      model.trainingEvidencePacketBuildStatus,
      "idle",
    ),
    id: "packet-build",
    label: "build",
    state: operatorSignalState(
      model.trainingEvidencePacketBuildStatus,
      model.trainingEvidencePacketBuildPending,
    ),
  },
  {
    detail: operatorSignalDetail(
      model.trainingEvidenceAdmissionStatus,
      "idle",
    ),
    id: "admit",
    label: "admit",
    state: operatorSignalState(
      model.trainingEvidenceAdmissionStatus,
      model.trainingEvidenceAdmissionPending,
    ),
  },
  {
    detail: operatorSignalDetail(model.trainingReconcileStatus, "idle"),
    id: "reconcile",
    label: "reconcile",
    state: operatorSignalState(
      model.trainingReconcileStatus,
      model.trainingReconcilePending,
    ),
  },
]

const trainingSceneOptions = (model: Model): TrainingRunVisualizationOptions | undefined => {
  const projection = modelTrainingRuns(model)
  const gates = modelTrainingPromiseGates(model)
  const promises = gates?.promises ?? []
  const promiseEvidenceRefCount = promises.reduce(
    (total, promise) => total + promise.evidenceRefCount,
    0,
  )
  const promiseSignalSnapshot = {
    operatorSignals: trainingOperatorSignals(model),
    promiseBlockerRefCount: gates?.blockerRefs.length ?? 0,
    promiseDegradedCount: gates?.stateCounts.degraded ?? 0,
    promiseEvidenceRefCount,
    promiseGreenCount: gates?.stateCounts.green ?? 0,
    promisePlannedCount: gates?.stateCounts.planned ?? 0,
    promiseRedCount: gates?.stateCounts.red ?? 0,
    promiseSignals: trainingPromiseSignals(promises),
    promiseUnknownCount: gates?.stateCounts.unknown ?? 0,
    promiseWithdrawnCount: gates?.stateCounts.withdrawn ?? 0,
    promiseYellowCount: gates?.stateCounts.yellow ?? 0,
  }
  const summary = selectedTrainingSummary(projection)
  if (summary === null) {
    return promises.length === 0
      ? undefined
      : trainingRunVisualizationOptionsFromSnapshot(promiseSignalSnapshot)
  }
  const metrics = summary.metrics
  const realGradient = summary.realGradient
  return trainingRunVisualizationOptionsFromSnapshot({
    activeWindowCount: metrics.activeWindowCount.value,
    assignedContributorCount: metrics.assignedContributorCount.value,
    deviceObserved:
      realGradient.deviceRequirement.observedDistinctContributorDevices,
    deviceRequired:
      realGradient.deviceRequirement.requiredDistinctContributorDevices,
    externalStatus: realGradient.externalAsk.status,
    finalValidationLoss: realGradient.lossUnderBudget.finalValidationLoss,
    freivaldsRefCount:
      realGradient.closeoutRequirement.freivaldsCommitmentRefs.length,
    gradientCloseoutRefCount:
      realGradient.closeoutRequirement.gradientCloseoutRefs.length,
    blockerRefCount:
      realGradient.externalAsk.blockerRefs.length +
      realGradient.externalAsk.requirementRefs.length,
    closeoutSatisfied: realGradient.closeoutRequirement.satisfied,
    lifecycleCounts: lifecycleCountsFromTrainingSummary(summary),
    lossUnderBudget: realGradient.lossUnderBudget.satisfied,
    maxAllowedStaleSteps: summary.run.maxAllowedStale,
    maxValidationLoss: realGradient.lossUnderBudget.maxValidationLoss,
    pendingPayoutCount: metrics.pendingPayoutCount.value,
    plannedWindowCount: metrics.plannedWindowCount.value,
    ...promiseSignalSnapshot,
    receiptRefCount: metrics.receiptRefCount.value,
    reconciledWindowCount: metrics.reconciledWindowCount.value,
    rejectedWorkCount: metrics.rejectedWorkCount.value,
    runDetail: summary.run.trainingRunRef,
    runLabel: summary.run.promiseRef,
    runState: summary.run.state,
    sealInFlight: summary.run.sealInFlight,
    sealedWindowCount: metrics.sealedWindowCount.value,
    settledPayoutSats: metrics.providerConfirmedSettledPayoutSats.value,
    verifiedWorkCount: metrics.verifiedWorkCount.value,
  })
}

const trainingNodeTone = (
  status: TrainingRunNodeDefinition["status"],
): TrainingGateTone =>
  status === "blocked"
    ? "blocked"
    : status === "planned" || status === "queued" || status === "sync"
      ? "watch"
      : "ready"

const trainingStatNumber = (value: number | null | undefined): string =>
  String(Math.max(0, value ?? 0))

const selectedTrainingSceneNode = (
  nodes: readonly TrainingRunNodeDefinition[],
  selectedNodeId: string | null,
): TrainingRunNodeDefinition | null =>
  nodes.find(node => node.id === selectedNodeId) ??
  nodes.find(node => node.id === "run") ??
  nodes[0] ??
  null

type TrainingFullscreenFact = Readonly<{
  label: string
  value: string
  tone: TrainingGateTone
}>

const trainingFullscreenFact = (
  label: string,
  value: string,
  tone: TrainingGateTone = "ready",
): TrainingFullscreenFact => ({ label, value, tone })

const trainingFullscreenStatView = (stat: TrainingFullscreenFact): Html =>
  h.div([cls(`training-fullscreen-stat training-${stat.tone}`)], [
    h.span([cls("training-fullscreen-stat-label")], [stat.label]),
    h.strong([cls("training-fullscreen-stat-value")], [stat.value]),
  ])

const trainingFullscreenStats = (model: Model): readonly TrainingFullscreenFact[] => {
  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  const dashboard = modelTrainingDashboard(model)
  const packet = modelTrainingEvidencePacketSummary(model)
  const metrics = summary?.metrics
  const lanes = dashboard?.leaderboards.lanes ?? []
  const rankedLanes = lanes.filter(lane => lane.rowCount > 0).length
  const finalLoss = summary?.realGradient.lossUnderBudget.finalValidationLoss
  const lossLabel =
    finalLoss === null || finalLoss === undefined ? "pending" : String(finalLoss)

  return [
    trainingFullscreenFact(
      "active windows",
      trainingStatNumber(metrics?.activeWindowCount.value),
      (metrics?.activeWindowCount.value ?? 0) > 0 ? "ready" : "watch",
    ),
    trainingFullscreenFact(
      "verified work",
      trainingStatNumber(metrics?.verifiedWorkCount.value),
      (metrics?.verifiedWorkCount.value ?? 0) > 0 ? "ready" : "watch",
    ),
    trainingFullscreenFact(
      "receipts",
      trainingStatNumber(metrics?.receiptRefCount.value),
      (metrics?.receiptRefCount.value ?? 0) > 0 ? "ready" : "watch",
    ),
    trainingFullscreenFact(
      "loss",
      lossLabel,
      summary?.realGradient.lossUnderBudget.satisfied ? "ready" : "watch",
    ),
    trainingFullscreenFact(
      "packet pylons",
      trainingStatNumber(packet?.distinctPylonCount),
      (packet?.distinctPylonCount ?? 0) >= 2 ? "ready" : "watch",
    ),
    trainingFullscreenFact(
      "ranked lanes",
      `${rankedLanes}/${lanes.length}`,
      rankedLanes > 0 ? "ready" : "watch",
    ),
  ]
}

const trainingNodeFacts = (
  node: TrainingRunNodeDefinition,
  model: Model,
): readonly TrainingFullscreenFact[] => {
  const summary = selectedTrainingSummary(modelTrainingRuns(model))
  const metrics = summary?.metrics
  const realGradient = summary?.realGradient
  const gates = modelTrainingPromiseGates(model)
  const packet = modelTrainingEvidencePacketSummary(model)

  switch (node.id) {
    case "registered":
      return [
        trainingFullscreenFact(
          "assigned pylons",
          trainingStatNumber(metrics?.assignedContributorCount.value),
          (metrics?.assignedContributorCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "observed devices",
          trainingStatNumber(
            realGradient?.deviceRequirement.observedDistinctContributorDevices,
          ),
          realGradient?.deviceRequirement.satisfied ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "run ref",
          summary?.run.trainingRunRef ?? "not loaded",
          summary === null ? "watch" : "ready",
        ),
      ]
    case "qualified":
      return [
        trainingFullscreenFact(
          "device gate",
          `${realGradient?.deviceRequirement.observedDistinctContributorDevices ?? 0}/${realGradient?.deviceRequirement.requiredDistinctContributorDevices ?? 0}`,
          realGradient?.deviceRequirement.satisfied ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "packet pylons",
          trainingStatNumber(packet?.distinctPylonCount),
          (packet?.distinctPylonCount ?? 0) >= 2 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "readiness",
          trainingStatusText(model.trainingOperatorReadinessStatus, "not loaded"),
          trainingStatusTone(
            model.trainingOperatorReadinessStatus,
            model.trainingOperatorReadinessPending,
          ),
        ),
      ]
    case "state_synced":
    case "sealed_window":
      return [
        trainingFullscreenFact(
          "sealed windows",
          trainingStatNumber(metrics?.sealedWindowCount.value),
          (metrics?.sealedWindowCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "reconciled",
          trainingStatNumber(metrics?.reconciledWindowCount.value),
          (metrics?.reconciledWindowCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "seal barrier",
          summary?.run.sealInFlight ? "in flight" : "open",
          summary?.run.sealInFlight ? "watch" : "ready",
        ),
      ]
    case "warmup":
    case "active":
    case "training_window":
      return [
        trainingFullscreenFact(
          "planned",
          trainingStatNumber(metrics?.plannedWindowCount.value),
          (metrics?.plannedWindowCount.value ?? 0) > 0 ? "watch" : "ready",
        ),
        trainingFullscreenFact(
          "active",
          trainingStatNumber(metrics?.activeWindowCount.value),
          (metrics?.activeWindowCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "max stale",
          trainingStatNumber(summary?.run.maxAllowedStale),
          "watch",
        ),
      ]
    case "sync_reentry":
      return [
        trainingFullscreenFact(
          "rejected work",
          trainingStatNumber(metrics?.rejectedWorkCount.value),
          (metrics?.rejectedWorkCount.value ?? 0) > 0 ? "blocked" : "ready",
        ),
        trainingFullscreenFact(
          "blockers",
          trainingStatNumber(realGradient?.externalAsk.blockerRefs.length),
          (realGradient?.externalAsk.blockerRefs.length ?? 0) > 0
            ? "blocked"
            : "ready",
        ),
        trainingFullscreenFact(
          "external ask",
          realGradient?.externalAsk.status ?? "not loaded",
          realGradient?.externalAsk.status === "blocked_external"
            ? "blocked"
            : "watch",
        ),
      ]
    case "freivalds":
      return [
        trainingFullscreenFact(
          "freivalds refs",
          trainingStatNumber(
            realGradient?.closeoutRequirement.freivaldsCommitmentRefs.length,
          ),
          (realGradient?.closeoutRequirement.freivaldsCommitmentRefs.length ?? 0) >
            0
            ? "ready"
            : "watch",
        ),
        trainingFullscreenFact(
          "gradient refs",
          trainingStatNumber(
            realGradient?.closeoutRequirement.gradientCloseoutRefs.length,
          ),
          (realGradient?.closeoutRequirement.gradientCloseoutRefs.length ?? 0) > 0
            ? "ready"
            : "watch",
        ),
        trainingFullscreenFact(
          "loss budget",
          realGradient?.lossUnderBudget.finalValidationLoss === null ||
            realGradient?.lossUnderBudget.finalValidationLoss === undefined
            ? realGradient?.lossUnderBudget.budgetLabel ?? "pending"
            : `${realGradient.lossUnderBudget.finalValidationLoss}/${realGradient.lossUnderBudget.maxValidationLoss ?? "?"}`,
          realGradient?.lossUnderBudget.satisfied ? "ready" : "watch",
        ),
      ]
    case "receipt":
      return [
        trainingFullscreenFact(
          "receipts",
          trainingStatNumber(metrics?.receiptRefCount.value),
          (metrics?.receiptRefCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "verified work",
          trainingStatNumber(metrics?.verifiedWorkCount.value),
          (metrics?.verifiedWorkCount.value ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "closeout",
          realGradient?.closeoutRequirement.satisfied ? "satisfied" : "open",
          realGradient?.closeoutRequirement.satisfied ? "ready" : "watch",
        ),
      ]
    case "settlement":
      return [
        trainingFullscreenFact(
          "pending payouts",
          trainingStatNumber(metrics?.pendingPayoutCount.value),
          (metrics?.pendingPayoutCount.value ?? 0) > 0 ? "watch" : "ready",
        ),
        trainingFullscreenFact(
          "settled sats",
          trainingStatNumber(metrics?.providerConfirmedSettledPayoutSats.value),
          (metrics?.providerConfirmedSettledPayoutSats.value ?? 0) > 0
            ? "ready"
            : "watch",
        ),
        trainingFullscreenFact(
          "promise blockers",
          trainingStatNumber(gates?.blockerRefs.length),
          (gates?.blockerRefs.length ?? 0) > 0 ? "blocked" : "ready",
        ),
      ]
    case "r1":
    case "r2":
      return [
        trainingFullscreenFact(
          "leader lanes",
          trainingStatNumber(modelTrainingDashboard(model)?.leaderboards.lanes.length),
          modelTrainingDashboard(model) === null ? "watch" : "ready",
        ),
        trainingFullscreenFact(
          "evidence refs",
          trainingStatNumber(packet?.evidenceRefCount),
          (packet?.evidenceRefCount ?? 0) > 0 ? "ready" : "watch",
        ),
        trainingFullscreenFact(
          "packet status",
          trainingStatusText(model.trainingEvidencePacketSummaryStatus, "not loaded"),
          trainingStatusTone(
            model.trainingEvidencePacketSummaryStatus,
            model.trainingEvidencePacketSummaryPending,
          ),
        ),
      ]
    case "run":
    default:
      return [
        trainingFullscreenFact(
          "run state",
          summary?.run.state ?? "not loaded",
          summary === null ? "watch" : "ready",
        ),
        trainingFullscreenFact(
          "promise",
          summary?.run.promiseRef ?? "not loaded",
          summary === null ? "watch" : "ready",
        ),
        trainingFullscreenFact(
          "windows",
          trainingStatNumber(summary?.windows.length),
          (summary?.windows.length ?? 0) > 0 ? "ready" : "watch",
        ),
      ]
  }
}

const trainingFullscreenNodePanel = (
  node: TrainingRunNodeDefinition,
  model: Model,
): Html =>
  h.aside(
    [cls(`training-fullscreen-node-panel training-${trainingNodeTone(node.status)}`)],
    [
      h.div([cls("training-fullscreen-node-kicker")], [
        h.span([], [node.role]),
        h.span([], [node.status]),
      ]),
      h.h2([cls("training-fullscreen-node-title")], [node.label]),
      h.p([cls("training-fullscreen-node-detail")], [node.detail]),
      h.div(
        [cls("training-fullscreen-node-facts")],
        trainingNodeFacts(node, model).map(trainingFullscreenStatView),
      ),
    ],
  )

const stateCounts = (
  projection: TrainingRunsResponse | null,
): Record<string, number> => {
  const counts: Record<string, number> = {
    active: 0,
    planned: 0,
    reconciled: 0,
    sealed: 0,
  }
  for (const run of projection?.runs ?? []) {
    counts[run.state] = (counts[run.state] ?? 0) + 1
  }
  return counts
}

const trainingRunRow = (summary: TrainingRunSummaryRow): Html =>
  h.li([cls(`training-run-row training-run-${summary.run.state}`)], [
    h.div([cls("training-run-main")], [
      h.span([cls("training-run-ref")], [summary.run.trainingRunRef]),
      h.span([cls("training-run-promise")], [summary.run.promiseRef]),
    ]),
    h.div([cls("training-run-facts")], [
      h.span([], [summary.run.state]),
      h.span([], [`${summary.metrics.verifiedWorkCount.value} verified`]),
      h.span([], [`${summary.metrics.assignedContributorCount.value} pylons`]),
    ]),
  ])

const liveTrainingProjectionPanel = (model: Model): Html => {
  const projection = modelTrainingRuns(model)
  const summary = selectedTrainingSummary(projection)
  const counts = stateCounts(projection)
  const activeSummary =
    summary === null
      ? "no selected run"
      : `${summary.run.state} · ${summary.windows.length} windows · ${summary.metrics.verifiedWorkCount.value} verified`

  return h.section([cls("training-panel training-live-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Worker Projection"]),
      h.button(
        [
          cls("training-refresh-button"),
          h.Type("button"),
          h.Disabled(model.trainingRunsPending),
          h.OnClick(ClickedRefreshTrainingRuns()),
        ],
        [model.trainingRunsPending ? "Refreshing..." : "Refresh"],
      ),
    ]),
    h.p(
      [cls(`training-panel-copy training-${model.trainingRunsStatus.tone}`)],
      [model.trainingRunsStatus.text],
    ),
    h.div([cls("training-metrics")], [
      trainingMetric("runs", String(projection?.runs.length ?? 0)),
      trainingMetric("active", String(counts.active)),
      trainingMetric("planned", String(counts.planned), "watch"),
      trainingMetric("selected", activeSummary, summary === null ? "blocked" : "ready"),
    ]),
    projection === null || projection.summaries.length === 0
      ? emptyLine("Open the pane or refresh to load public training runs.")
      : h.ul(
          [cls("training-run-list")],
          projection.summaries.slice(0, 5).map(trainingRunRow),
        ),
  ])
}

const replaySatsLabel = (amount: number): string =>
  `${amount.toLocaleString()} sats`

const proofReplayEventTone = (
  kind: ProofReplayBundle["events"][number]["kind"],
): "blocked" | "ready" | "watch" => {
  if (
    kind === "payment_zap_confirmed" ||
    kind === "recipient_confirmation_recorded" ||
    kind === "proof_verified"
  ) {
    return "ready"
  }
  if (kind === "settlement_blocked_closed") return "blocked"
  return "watch"
}

const proofReplayEventRows = (
  projection: DesktopProofReplayProjection | null,
): ReadonlyArray<Html> => {
  const events = projection?.bundle?.events ?? []
  return events
    .slice(0, 6)
    .map(event =>
      trainingGate(
        event.kind,
        event.amountSats === undefined
          ? event.displayText
          : `${replaySatsLabel(event.amountSats)} · ${event.displayText}`,
        proofReplayEventTone(event.kind),
      ),
    )
}

const proofReplayGapRows = (
  projection: DesktopProofReplayProjection | null,
): ReadonlyArray<Html> =>
  (projection?.bundle?.gaps ?? [])
    .slice(0, 5)
    .map(gap => trainingGate(gap.gapRef, gap.reason, "watch"))

const proofReplaySourceRows = (
  projection: DesktopProofReplayProjection | null,
): ReadonlyArray<Html> => {
  const sources = projection?.bundle?.sourceRefs ?? []
  return sources.slice(0, 5).map(source =>
    h.li([], [
      source.url === undefined
        ? h.code([], [source.ref])
        : h.a([h.Href(source.url)], [source.ref]),
      source.kind === undefined ? "" : ` · ${source.kind}`,
    ]),
  )
}

const proofReplayCaveatRows = (
  projection: DesktopProofReplayProjection | null,
): ReadonlyArray<Html> =>
  (projection?.caveatRefs ?? []).slice(0, 5).map(ref =>
    h.li([], [h.code([], [ref])]),
  )

const proofReplaySceneSlug = (
  projection: DesktopProofReplayProjection | null,
  fallback: string,
): string =>
  projection?.request?.mode === "generated"
    ? "generated-public-activity"
    : projection?.entry?.slug ?? fallback

const proofReplayGeneratedField = (
  label: string,
  name: string,
  value: string,
  placeholder: string,
  onInput: (value: string) => Message,
): Html =>
  h.label([cls("training-replay-filter-field")], [
    h.span([cls("training-replay-filter-label")], [label]),
    h.input([
      cls("training-replay-filter-input"),
      h.Type("text"),
      h.Name(name),
      h.AriaLabel(label),
      h.Placeholder(placeholder),
      h.Value(value),
      h.OnInput(onInput),
    ]),
  ])

const proofReplayGeneratedFilters = (model: Model): Html =>
  h.div([cls("training-replay-generated-filters")], [
    proofReplayGeneratedField(
      "From",
      "proof-replay-from",
      model.generatedProofReplayFrom,
      "2026-06-18T00:00:00.000Z",
      value => ChangedProofReplayGeneratedFrom({ value }),
    ),
    proofReplayGeneratedField(
      "To",
      "proof-replay-to",
      model.generatedProofReplayTo,
      "2026-06-19T00:00:00.000Z",
      value => ChangedProofReplayGeneratedTo({ value }),
    ),
    proofReplayGeneratedField(
      "Run",
      "proof-replay-run-ref",
      model.generatedProofReplayRunRef,
      "run.tassadar.executor.20260615",
      value => ChangedProofReplayGeneratedRunRef({ value }),
    ),
    proofReplayGeneratedField(
      "Window",
      "proof-replay-window-ref",
      model.generatedProofReplayWindowRef,
      "training.window.tassadar.executor.20260615.w1",
      value => ChangedProofReplayGeneratedWindowRef({ value }),
    ),
    proofReplayGeneratedField(
      "Actor",
      "proof-replay-actor-ref",
      model.generatedProofReplayActorRef,
      "pylon.448ba824b5fc879f3a59",
      value => ChangedProofReplayGeneratedActorRef({ value }),
    ),
    proofReplayGeneratedField(
      "Pair",
      "proof-replay-pair-ref",
      model.generatedProofReplayPairRef,
      "pylon.one+pylon.two",
      value => ChangedProofReplayGeneratedPairRef({ value }),
    ),
    proofReplayGeneratedField(
      "Kind",
      "proof-replay-kind",
      model.generatedProofReplayKind,
      "real_bitcoin_moved",
      value => ChangedProofReplayGeneratedKind({ value }),
    ),
    proofReplayGeneratedField(
      "Source",
      "proof-replay-source",
      model.generatedProofReplaySource,
      "settlement_receipt",
      value => ChangedProofReplayGeneratedSource({ value }),
    ),
    proofReplayGeneratedField(
      "Since",
      "proof-replay-since",
      model.generatedProofReplaySince,
      "2026-06-18T12:00:00.000Z:settlement_receipt:receipt",
      value => ChangedProofReplayGeneratedSince({ value }),
    ),
    proofReplayGeneratedField(
      "Limit",
      "proof-replay-limit",
      model.generatedProofReplayLimit,
      "20",
      value => ChangedProofReplayGeneratedLimit({ value }),
    ),
    h.button(
      [
        cls("training-action-button secondary training-replay-generated-load"),
        h.Type("button"),
        h.Disabled(model.proofReplayPending),
        h.OnClick(ClickedLoadGeneratedProofReplay()),
      ],
      [model.proofReplayPending ? "Loading..." : "Load generated"],
    ),
  ])

const proofReplayUnavailableText = (
  projection: DesktopProofReplayProjection | null,
  fallback: string,
): string =>
  projection?.error ??
  projection?.blockerRefs[0] ??
  projection?.cacheLabel ??
  fallback

const proofReplayLoadingPanel = (
  model: Model,
  projection: DesktopProofReplayProjection | null,
  className: string,
): Html => {
  const blocked = projection !== null && projection.ok === false
  return h.div(
    [
      cls(
        `${className} ${blocked ? "proof-replay-status-error" : "proof-replay-status-loading"}`,
      ),
    ],
    [
      h.span([cls("proof-replay-status-label")], [
        blocked ? "Proof replay unavailable" : "Loading Tassadar replay",
      ]),
      h.p([cls("proof-replay-status-copy")], [
        blocked
          ? proofReplayUnavailableText(
              projection,
              "The public proof replay bundle is unavailable.",
            )
          : model.proofReplayStatus.text,
      ]),
      blocked
        ? h.button(
            [
              cls("proof-replay-status-retry"),
              h.Type("button"),
              h.Disabled(model.proofReplayPending),
              h.OnClick(ClickedRefreshProofReplay()),
            ],
            [model.proofReplayPending ? "Retrying..." : "Retry"],
          )
        : h.empty,
    ],
  )
}

const proofReplayPanel = (model: Model): Html => {
  const projection = modelProofReplay(model)
  const entries = desktopProofReplayCatalog()
  const selectedEntry =
    entries.find(entry => entry.slug === model.selectedProofReplaySlug) ?? entries[0]
  const selectedProjection =
    projection?.request?.mode === "generated" &&
    model.selectedProofReplayMode === "generated"
      ? projection
      : projection?.entry?.slug === selectedEntry?.slug
        ? projection
        : null
  const summary = selectedProjection?.summary ?? null
  const sourceRows = proofReplaySourceRows(selectedProjection)
  const eventRows = proofReplayEventRows(selectedProjection)
  const gapRows = proofReplayGapRows(selectedProjection)
  const caveatRows = proofReplayCaveatRows(selectedProjection)
  const selectedBundle = selectedProjection?.bundle ?? null
  const generatedFrom = selectedProjection?.generatedFrom ?? null
  const generatedSourceUrl = generatedFrom?.source?.url ?? null

  return h.section([cls("training-panel training-proof-replay-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Proof Replays"]),
      h.button(
        [
          cls("training-refresh-button"),
          h.Type("button"),
          h.Disabled(model.proofReplayPending),
          h.OnClick(ClickedRefreshProofReplay()),
        ],
        [model.proofReplayPending ? "Refreshing..." : "Refresh"],
      ),
    ]),
    h.p(
      [cls(`training-panel-copy training-${model.proofReplayStatus.tone}`)],
      [model.proofReplayStatus.text],
    ),
    h.div(
      [cls("training-action-row training-replay-selector")],
      entries.map(entry =>
        h.button(
          [
            cls(
              `training-action-button secondary${model.selectedProofReplayMode === "catalog" && entry.slug === model.selectedProofReplaySlug ? " active" : ""}`,
            ),
            h.Type("button"),
            h.Disabled(model.proofReplayPending),
            h.OnClick(SelectedProofReplay({ slug: entry.slug })),
          ],
          [entry.slug === "first-real-settlement" ? "First settlement" : "Recognition"],
        ),
      ),
    ),
    proofReplayGeneratedFilters(model),
    h.p([cls("training-panel-copy")], [
      model.selectedProofReplayMode === "generated"
        ? selectedProjection?.filterLabel ?? "Generated public activity replay"
        : selectedEntry?.summary ?? "Receipt-backed replay shelf.",
    ]),
    h.div([cls("training-proof-replay-viewport")], [
      selectedBundle === null
        ? proofReplayLoadingPanel(
            model,
            selectedProjection,
            "training-proof-replay-placeholder",
          )
        : tassadarProofReplayScene(
            "training-proof-replay-scene",
            proofReplaySceneSlug(
              selectedProjection,
              selectedEntry?.slug ?? DEFAULT_DESKTOP_PROOF_REPLAY_SLUG,
            ),
            selectedBundle,
          ),
    ]),
    summary === null
      ? emptyLine("Open Training or refresh to load the selected public replay bundle.")
      : h.div([cls("training-metrics")], [
          trainingMetric("actors", String(summary.actorCount), "ready"),
          trainingMetric("events", String(summary.eventCount), "ready"),
          trainingMetric(
            "confirmed",
            replaySatsLabel(summary.confirmedZapSats),
            summary.confirmedZapSats > 0 ? "ready" : "watch",
          ),
          trainingMetric(
            "gaps",
            String(summary.gapCount),
            summary.gapCount > 0 ? "watch" : "ready",
          ),
        ]),
    h.ul(
      [cls("training-gates training-proof-replay-events")],
      eventRows.length === 0
        ? [trainingGate("events", "not loaded", "watch")]
        : eventRows,
    ),
    gapRows.length === 0
      ? h.empty
      : h.ul([cls("training-gates training-proof-replay-gaps")], gapRows),
    h.div([cls("training-replay-links")], [
      model.selectedProofReplayMode === "generated" || selectedEntry === undefined
        ? h.empty
        : h.a([h.Href(selectedEntry.websitePath)], ["Open web replay"]),
      model.selectedProofReplayMode === "generated" ||
      selectedEntry?.socialPath === undefined
        ? h.empty
        : h.a([h.Href(selectedEntry.socialPath)], ["Open social cut"]),
      selectedProjection === null
        ? h.empty
        : h.a([h.Href(selectedProjection.sourceUrl)], ["Open bundle API"]),
      generatedSourceUrl === null
        ? h.empty
        : h.a([h.Href(generatedSourceUrl)], ["Open activity API"]),
    ]),
    h.p([cls("training-panel-copy")], [
      selectedProjection?.cacheLabel ??
        "No offline snapshot is cached; the desktop shelf waits for the live public bundle.",
    ]),
    generatedFrom === null
      ? h.empty
      : h.ul([cls("training-api-list training-replay-generated")], [
          h.li([], [
            "generated · ",
            selectedProjection?.filterLabel ?? "public activity range",
          ]),
          h.li([], [
            "source lag · ",
            String(generatedFrom.sourceLag?.length ?? 0),
          ]),
        ]),
    caveatRows.length === 0
      ? h.empty
      : h.ul([cls("training-api-list training-replay-caveats")], caveatRows),
    sourceRows.length === 0
      ? h.empty
      : h.ul([cls("training-api-list training-replay-sources")], sourceRows),
  ])
}

const selectedTrainingEvidencePanel = (
  projection: TrainingRunsResponse | null,
): Html => {
  const summary = selectedTrainingSummary(projection)
  if (summary === null) {
    return h.section([cls("training-panel")], [
      h.h2([cls("training-panel-title")], ["Evidence"]),
      emptyLine("No Worker summary loaded yet."),
    ])
  }

  const device = summary.realGradient.deviceRequirement
  const closeout = summary.realGradient.closeoutRequirement
  const loss = summary.realGradient.lossUnderBudget
  const externalAsk = summary.realGradient.externalAsk

  return h.section([cls("training-panel")], [
    h.h2([cls("training-panel-title")], ["Evidence"]),
    h.ul([cls("training-gates")], [
      trainingGate(
        "devices",
        `${device.observedDistinctContributorDevices}/${device.requiredDistinctContributorDevices}`,
        device.satisfied ? "ready" : "watch",
      ),
      trainingGate(
        "Freivalds refs",
        String(closeout.freivaldsCommitmentRefs.length),
        closeout.freivaldsCommitmentRefs.length > 0 ? "ready" : "watch",
      ),
      trainingGate(
        "gradient closeouts",
        String(closeout.gradientCloseoutRefs.length),
        closeout.gradientCloseoutRefs.length > 0 ? "ready" : "watch",
      ),
      trainingGate(
        "loss budget",
        loss.finalValidationLoss === null
          ? loss.budgetLabel || "not observed"
          : `${loss.finalValidationLoss}/${loss.maxValidationLoss ?? "?"}`,
        loss.satisfied ? "ready" : "blocked",
      ),
      trainingGate(
        "external ask",
        externalAsk.status,
        externalAsk.status === "ready" || externalAsk.status === "observed"
          ? "ready"
          : "blocked",
      ),
      trainingGate(
        "settled sats",
        String(summary.metrics.providerConfirmedSettledPayoutSats.value),
        summary.metrics.providerConfirmedSettledPayoutSats.value > 0
          ? "ready"
          : "watch",
      ),
    ]),
  ])
}

const selectedTrainingLedgerPanel = (
  projection: TrainingRunsResponse | null,
): Html => {
  const summary = selectedTrainingSummary(projection)
  if (summary === null) {
    return h.section([cls("training-panel training-ledger-panel")], [
      h.h2([cls("training-panel-title")], ["Evidence Ledger"]),
      emptyLine("No selected run refs loaded yet."),
    ])
  }

  const closeout = summary.realGradient.closeoutRequirement
  const loss = summary.realGradient.lossUnderBudget
  const externalAsk = summary.realGradient.externalAsk
  const latestWindows = summary.windows.slice(0, 4)
  const authorityRefs = uniqueTrainingRefs([
    summary.run.trainingRunRef,
    summary.run.promiseRef,
    ...summary.run.sourceRefs,
    ...summary.sourceRefs,
    ...summary.copyBoundaryRefs,
    ...summary.realGradient.scopeBoundaryRefs,
  ])
  const evidenceRefs = uniqueTrainingRefs([
    ...closeout.freivaldsCommitmentRefs,
    ...closeout.gradientCloseoutRefs,
    closeout.mergeRef,
    closeout.evalRef,
    loss.budgetRef,
    ...loss.sourceRefs,
    ...summary.realGradient.deviceRequirement.sourceRefs,
  ])
  const receiptRefs = uniqueTrainingRefs([
    ...summary.receiptRefs,
    ...summary.run.receiptRefs,
    ...latestWindows.flatMap(window =>
      Array.isArray(window.receiptRefs) ? window.receiptRefs : [],
    ),
  ])
  const blockerRefs = uniqueTrainingRefs([
    ...externalAsk.blockerRefs,
    ...externalAsk.requirementRefs,
  ])

  return h.section([cls("training-panel training-ledger-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Evidence Ledger"]),
      h.span([cls("training-panel-kicker")], [summary.run.state]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Selected public refs behind the run, windows, evidence, receipts, and blockers.",
    ]),
    h.div([cls("training-metrics")], [
      trainingMetric("windows", String(summary.windows.length)),
      trainingMetric("receipts", String(receiptRefs.length)),
      trainingMetric(
        "evidence refs",
        String(evidenceRefs.length),
        evidenceRefs.length > 0 ? "ready" : "watch",
      ),
      trainingMetric(
        "blockers",
        String(blockerRefs.length),
        blockerRefs.length === 0 ? "ready" : "blocked",
      ),
    ]),
    latestWindows.length === 0
      ? emptyLine("No window records projected for this run yet.")
      : h.ul(
          [cls("training-ledger-windows")],
          latestWindows.map(window => {
            const datasetCount = Array.isArray(window.datasetRefs)
              ? window.datasetRefs.length
              : 0
            const receiptCount = Array.isArray(window.receiptRefs)
              ? window.receiptRefs.length
              : 0
            const homeworkKind = window.homeworkKind ?? "unknown"
            return h.li([cls(`training-ledger-window training-${window.state}`)], [
              h.code([], [window.windowRef]),
              h.span([], [
                `${window.state} · ${homeworkKind} · ${datasetCount} datasets · ${receiptCount} receipts`,
              ]),
            ])
          }),
        ),
    trainingRefList("authority", authorityRefs),
    trainingRefList("evidence", evidenceRefs),
    trainingRefList("receipts", receiptRefs),
    trainingRefList("blockers", blockerRefs, "no blockers observed"),
  ])
}

const countTone = (count: number): TrainingGateTone =>
  count > 0 ? "ready" : "watch"

const selectedTrainingLifecyclePanel = (
  projection: TrainingRunsResponse | null,
): Html => {
  const summary = selectedTrainingSummary(projection)
  if (summary === null) {
    return h.section([cls("training-panel training-lifecycle-panel")], [
      h.h2([cls("training-panel-title")], ["Run Lifecycle"]),
      emptyLine("No selected run lifecycle loaded yet."),
    ])
  }

  const metrics = summary.metrics
  const device = summary.realGradient.deviceRequirement
  const closeout = summary.realGradient.closeoutRequirement
  const activeWindows = metrics.activeWindowCount.value
  const plannedWindows = metrics.plannedWindowCount.value
  const sealedWindows = metrics.sealedWindowCount.value
  const reconciledWindows = metrics.reconciledWindowCount.value
  const verifiedWork = metrics.verifiedWorkCount.value
  const rejectedWork = metrics.rejectedWorkCount.value
  const hasDurableSeal = sealedWindows + reconciledWindows > 0

  return h.section([cls("training-panel training-lifecycle-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Run Lifecycle"]),
      h.span([cls("training-panel-kicker")], [summary.run.trainingRunRef]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Pluralis join ramp mapped onto the Worker run/window authority.",
    ]),
    h.ul([cls("training-gates training-lifecycle-gates")], [
      trainingGate(
        "registered",
        `${metrics.assignedContributorCount.value} pylons assigned`,
        countTone(metrics.assignedContributorCount.value),
      ),
      trainingGate(
        "qualified",
        `${device.observedDistinctContributorDevices}/${device.requiredDistinctContributorDevices} devices`,
        device.satisfied ? "ready" : "watch",
      ),
      trainingGate(
        "state_synced",
        hasDurableSeal ? "last durable seal visible" : "awaiting sealed window",
        hasDurableSeal ? "ready" : "watch",
      ),
      trainingGate(
        "warmup",
        activeWindows > 0
          ? `${activeWindows} active windows`
          : `${plannedWindows} planned windows`,
        activeWindows > 0 ? "ready" : plannedWindows > 0 ? "watch" : "blocked",
      ),
      trainingGate(
        "active",
        `${verifiedWork} verified work refs`,
        verifiedWork > 0 ? "ready" : "watch",
      ),
      trainingGate(
        "sync_reentry",
        rejectedWork > 0
          ? `${rejectedWork} rejected work refs`
          : `max stale ${summary.run.maxAllowedStale}`,
        rejectedWork > 0 ? "blocked" : "watch",
      ),
    ]),
    h.ul([cls("training-gates training-window-timeline")], [
      trainingGate("planned", String(plannedWindows), countTone(plannedWindows)),
      trainingGate("active", String(activeWindows), countTone(activeWindows)),
      trainingGate("sealed", String(sealedWindows), countTone(sealedWindows)),
      trainingGate(
        "reconciled",
        String(reconciledWindows),
        countTone(reconciledWindows),
      ),
      trainingGate(
        "seal barrier",
        summary.run.sealInFlight ? "in flight" : "open",
        summary.run.sealInFlight ? "watch" : "ready",
      ),
      trainingGate(
        "closeout",
        closeout.satisfied ? "satisfied" : "missing refs",
        closeout.satisfied ? "ready" : "watch",
      ),
    ]),
  ])
}

const dashboardGateTone = (
  blockerRefs: readonly string[],
  observedCount: number,
): "ready" | "watch" | "blocked" =>
  blockerRefs.length > 0 ? "blocked" : observedCount > 0 ? "ready" : "watch"

const leaderboardGate = (lane: TrainingLeaderboardLaneSummary): Html => {
  const top = lane.topRow
  return trainingGate(
    lane.title,
    top === null
      ? `${lane.blockerRefs.length} blockers`
      : `#${top.rank} ${top.contributorRef} · ${top.scoreLabel || top.score}`,
    lane.rowCount > 0 ? "ready" : dashboardGateTone(lane.blockerRefs, 0),
  )
}

const trainingDashboardPanel = (model: Model): Html => {
  const dashboard = modelTrainingDashboard(model)
  const lanes = dashboard?.leaderboards.lanes ?? []
  const rankedLaneCount = lanes.filter(lane => lane.rowCount > 0).length
  const blockerCount = dashboard
    ? [
        ...dashboard.leaderboards.blockerRefs,
        ...dashboard.a2.blockerRefs,
        ...dashboard.a3.blockerRefs,
        ...dashboard.a4.blockerRefs,
        ...dashboard.a4.evalDeltaBonusBlockerRefs,
        ...dashboard.a5.blockerRefs,
      ].length
    : 0
  const laneRows =
    lanes.length === 0
      ? [trainingGate("leaderboards", "not loaded", "watch")]
      : lanes.slice(0, 5).map(leaderboardGate)

  return h.section([cls("training-panel training-dashboard-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["CS336 Dashboards"]),
      h.span([cls("training-panel-kicker")], [
        dashboard === null ? "public summaries" : dashboard.sourceUrl,
      ]),
    ]),
    h.p(
      [cls(`training-panel-copy training-${model.trainingDashboardStatus.tone}`)],
      [model.trainingDashboardStatus.text],
    ),
    h.div([cls("training-metrics")], [
      trainingMetric(
        "ranked lanes",
        `${rankedLaneCount}/${lanes.length}`,
        rankedLaneCount > 0 ? "ready" : "watch",
      ),
      trainingMetric(
        "A2 classes",
        String(dashboard?.a2.observedDeviceClassCount ?? 0),
        dashboard === null
          ? "watch"
          : dashboardGateTone(
              dashboard.a2.blockerRefs,
              dashboard.a2.observedDeviceClassCount,
            ),
      ),
      trainingMetric(
        "A3 cells",
        `${dashboard?.a3.verifiedCellCount ?? 0}/${dashboard?.a3.cellCount ?? 0}`,
        dashboard === null
          ? "watch"
          : dashboardGateTone(
              dashboard.a3.blockerRefs,
              dashboard.a3.verifiedCellCount,
            ),
      ),
      trainingMetric(
        "A4 stages",
        `${dashboard?.a4.observedVerifiedStages.length ?? 0}/${dashboard?.a4.requiredVerifiedStageCount ?? 0}`,
        dashboard === null
          ? "watch"
          : dashboardGateTone(
              dashboard.a4.blockerRefs,
              dashboard.a4.observedVerifiedStages.length,
            ),
      ),
      trainingMetric(
        "A5 suites",
        `${dashboard?.a5.verifiedSuiteCount ?? 0}/${dashboard?.a5.evalSuiteCount ?? 0}`,
        dashboard === null
          ? "watch"
          : dashboardGateTone(
              dashboard.a5.blockerRefs,
              dashboard.a5.verifiedSuiteCount,
            ),
      ),
      trainingMetric(
        "blockers",
        String(blockerCount),
        blockerCount === 0 ? "ready" : "blocked",
      ),
    ]),
    h.ul([cls("training-gates training-dashboard-lanes")], laneRows),
  ])
}

const promiseTone = (
  state: TrainingPromiseState,
): "ready" | "watch" | "blocked" => {
  switch (state) {
    case "green":
      return "ready"
    case "yellow":
    case "planned":
      return "watch"
    case "degraded":
    case "red":
    case "withdrawn":
    case "unknown":
      return "blocked"
  }
}

const promiseGate = (promise: TrainingPromiseSummary): Html =>
  trainingGate(
    promise.promiseId,
    `${promise.state} · ${promise.blockerRefs.length} blockers · ${promise.evidenceRefCount} refs`,
    promiseTone(promise.state),
  )

const trainingPromiseGatesPanel = (model: Model): Html => {
  const gates = modelTrainingPromiseGates(model)
  const promises = gates?.promises ?? []
  const rows =
    promises.length === 0
      ? [trainingGate("product promises", "not loaded", "watch")]
      : promises.slice(0, 7).map(promiseGate)

  return h.section([cls("training-panel training-promise-gates-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Promise Gates"]),
      h.span([cls("training-panel-kicker")], [
        gates === null ? "public registry" : gates.sourceUrl,
      ]),
    ]),
    h.p(
      [
        cls(
          `training-panel-copy training-${model.trainingPromiseGatesStatus.tone}`,
        ),
      ],
      [model.trainingPromiseGatesStatus.text],
    ),
    h.div([cls("training-metrics")], [
      trainingMetric("promises", String(promises.length)),
      trainingMetric(
        "green",
        String(gates?.stateCounts.green ?? 0),
        (gates?.stateCounts.green ?? 0) > 0 ? "ready" : "watch",
      ),
      trainingMetric(
        "yellow",
        String(gates?.stateCounts.yellow ?? 0),
        (gates?.stateCounts.yellow ?? 0) > 0 ? "watch" : "ready",
      ),
      trainingMetric(
        "red",
        String(gates?.stateCounts.red ?? 0),
        (gates?.stateCounts.red ?? 0) > 0 ? "blocked" : "ready",
      ),
      trainingMetric(
        "planned",
        String(gates?.stateCounts.planned ?? 0),
        (gates?.stateCounts.planned ?? 0) > 0 ? "watch" : "ready",
      ),
      trainingMetric(
        "blockers",
        String(gates?.blockerRefs.length ?? 0),
        (gates?.blockerRefs.length ?? 0) > 0 ? "blocked" : "ready",
      ),
    ]),
    h.ul([cls("training-gates training-promise-gates")], rows),
  ])
}

const trainingProjectionFeedTone = (
  pending: boolean,
  ok: boolean | null,
  status: TrainingStatusLike,
): TrainingGateTone =>
  pending
    ? "watch"
    : ok === true
      ? "ready"
      : ok === false
        ? "blocked"
        : trainingStatusTone(status)

type TrainingProjectionCatchUpRow = Readonly<{
  label: string
  value: string
  tone: TrainingGateTone
}>

const trainingProjectionCatchUpRows = (
  model: Model,
): readonly TrainingProjectionCatchUpRow[] => {
  const projection = modelTrainingRuns(model)
  const plan = modelTrainingPlan(model)
  const activation = modelTrainingActivation(model)
  const lease = modelTrainingLease(model)
  const admission = modelTrainingEvidenceAdmission(model)
  const reconcile = modelTrainingReconcile(model)

  const planSummary = trainingSummaryByRunRef(
    projection,
    plan?.trainingRunRef,
  )
  const planWindow = trainingWindowByRef(projection, plan?.windowRef)
  const activationWindow = trainingWindowByRef(
    projection,
    activation?.windowRef,
  )
  const admissionSummary = trainingSummaryByRunRef(
    projection,
    admission?.trainingRunRef,
  )
  const reconcileWindow = trainingWindowByRef(
    projection,
    reconcile?.windowRef,
  )

  const planRow: TrainingProjectionCatchUpRow =
    model.trainingPlanPending
      ? {
          label: "plan observed",
          value: "planning command in flight",
          tone: "watch",
        }
      : plan === null
        ? {
            label: "plan observed",
            value: "no plan yet",
            tone: "watch",
          }
        : !plan.ok
          ? {
              label: "plan observed",
              value: plan.reason,
              tone: "blocked",
            }
          : projection === null
            ? {
                label: "plan observed",
                value: `${plan.trainingRunRef ?? "run pending"} · waiting for projection`,
                tone: "watch",
              }
            : planSummary === null
              ? {
                  label: "plan observed",
                  value: `${plan.trainingRunRef ?? "run pending"} · not projected yet`,
                  tone: "watch",
                }
              : {
                  label: "plan observed",
                  value: `${model.trainingPlanFirstObservedAt ?? projection.fetchedAt} · ${planSummary.run.state} · ${planWindow?.state ?? "window pending"}`,
                  tone: "ready",
                }

  const activationRow: TrainingProjectionCatchUpRow =
    model.trainingActivationPending
      ? {
          label: "activation",
          value: "activation command in flight",
          tone: "watch",
        }
      : activation === null
        ? {
            label: "activation",
            value: "no activation yet",
            tone: "watch",
          }
        : !activation.ok
          ? {
              label: "activation",
              value: activation.reason,
              tone: "blocked",
            }
          : activationWindow === null
            ? {
                label: "activation",
                value: `${activation.windowRef ?? "window pending"} · waiting for projection`,
                tone: "watch",
              }
            : trainingWindowStateRank(activationWindow.state) >=
                trainingWindowStateRank("active")
              ? {
                  label: "activation",
                  value: `${activationWindow.windowRef} · ${activationWindow.state}`,
                  tone: "ready",
                }
              : {
                  label: "activation",
                  value: `${activationWindow.windowRef} · still ${activationWindow.state}`,
                  tone: "watch",
                }

  const leaseRow: TrainingProjectionCatchUpRow =
    model.trainingLeasePending
      ? {
          label: "lease claim",
          value: "lease command in flight",
          tone: "watch",
        }
      : lease === null
        ? {
            label: "lease claim",
            value: "no lease claim yet",
            tone: "watch",
          }
        : !lease.ok
          ? {
              label: "lease claim",
              value: lease.reason,
              tone: "blocked",
            }
          : lease.lease === null
            ? {
                label: "lease claim",
                value: lease.reason,
                tone: "watch",
              }
            : {
                label: "lease claim",
                value: `${lease.lease.state} · ${lease.lease.leaseRef} · ${lease.lease.leaseExpiresInSeconds}s`,
                tone: lease.lease.state === "active" ? "ready" : "watch",
              }

  const admissionReceiptTarget = admission?.receiptRefCount ?? 0
  const projectedReceiptCount =
    admissionSummary?.metrics.receiptRefCount.value ?? 0
  const evidenceRow: TrainingProjectionCatchUpRow =
    model.trainingEvidenceAdmissionPending
      ? {
          label: "evidence receipts",
          value: "admission command in flight",
          tone: "watch",
        }
      : admission === null
        ? {
            label: "evidence receipts",
            value: "no admission yet",
            tone: "watch",
          }
        : !admission.ok
          ? {
              label: "evidence receipts",
              value: admission.reason,
              tone: "blocked",
            }
          : admissionSummary === null
            ? {
                label: "evidence receipts",
                value: `${admission.trainingRunRef ?? "run pending"} · waiting for projection`,
                tone: "watch",
              }
            : projectedReceiptCount >= admissionReceiptTarget
              ? {
                  label: "evidence receipts",
                  value: `${projectedReceiptCount}/${admissionReceiptTarget} receipts · ${admission.evidenceRefCount} evidence refs`,
                  tone: "ready",
                }
              : {
                  label: "evidence receipts",
                  value: `${projectedReceiptCount}/${admissionReceiptTarget} receipts projected`,
                  tone: "watch",
                }

  const reconcileRow: TrainingProjectionCatchUpRow =
    model.trainingReconcilePending
      ? {
          label: "reconcile",
          value: "reconcile command in flight",
          tone: "watch",
        }
      : reconcile === null
        ? {
            label: "reconcile",
            value: "no reconcile yet",
            tone: "watch",
          }
        : !reconcile.ok
          ? {
              label: "reconcile",
              value: reconcile.reason,
              tone: "blocked",
            }
          : reconcileWindow === null
            ? {
                label: "reconcile",
                value: `${reconcile.windowRef ?? "window pending"} · waiting for projection`,
                tone: "watch",
              }
            : reconcileWindow.state === "reconciled"
              ? {
                  label: "reconcile",
                  value: `${reconcileWindow.windowRef} · reconciled`,
                  tone: "ready",
                }
              : {
                  label: "reconcile",
                  value: `${reconcileWindow.windowRef} · still ${reconcileWindow.state}`,
                  tone: "watch",
                }

  return [planRow, activationRow, leaseRow, evidenceRow, reconcileRow]
}

const trainingProjectionCatchUpPanel = (model: Model): Html =>
  h.section([cls("training-panel training-projection-catchup-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Projection Catch-Up"]),
      h.span([cls("training-panel-kicker")], [
        model.trainingPlanFirstObservedAt ?? "awaiting planned run",
      ]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Compares Bun-held command results with the latest Worker projection so the operator can see when public state has caught up.",
    ]),
    h.ul(
      [cls("training-gates training-projection-catchup")],
      trainingProjectionCatchUpRows(model).map(row =>
        trainingGate(row.label, row.value, row.tone),
      ),
    ),
  ])

const trainingOperatorFeedPanel = (model: Model): Html => {
  const runs = modelTrainingRuns(model)
  const dashboard = modelTrainingDashboard(model)
  const gates = modelTrainingPromiseGates(model)
  const readiness = modelTrainingOperatorReadiness(model)
  const packetSummary = modelTrainingEvidencePacketSummary(model)
  const plan = modelTrainingPlan(model)
  const activation = modelTrainingActivation(model)
  const reconcile = modelTrainingReconcile(model)
  const lease = modelTrainingLease(model)
  const bootstrap = modelTrainingBootstrap(model)
  const packetBuild = modelTrainingEvidencePacketBuild(model)
  const evidenceAdmission = modelTrainingEvidenceAdmission(model)

  const planRef =
    plan?.windowRef ?? plan?.trainingRunRef ?? plan?.reason ?? "idle"
  const activationRef = activation?.windowRef ?? activation?.reason ?? "idle"
  const reconcileRef = reconcile?.windowRef ?? reconcile?.reason ?? "idle"
  const leaseRef =
    lease?.lease?.leaseRef ??
    lease?.pylonRef ??
    lease?.reason ??
    "idle"
  const bootstrapRef =
    bootstrap?.outcome?.kind === "granted"
      ? bootstrap.outcome.grant.grantRef
      : bootstrap?.outcome?.kind ?? bootstrap?.reason ?? "idle"
  const evidenceRef =
    evidenceAdmission?.ok === true
      ? `${evidenceAdmission.receiptRefCount} receipts`
      : evidenceAdmission?.reason ?? "idle"
  const packetRef =
    packetSummary === null
      ? "idle"
      : packetSummary.ok
        ? `${packetSummary.receiptRefCount} receipts`
        : `${packetSummary.blockerRefs.length} blockers`
  const packetBuildRef = packetBuild?.reason ?? "idle"

  return h.section([cls("training-panel training-operator-feed-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Operator Feed"]),
      h.span([cls("training-panel-kicker")], [
        runs === null
          ? "waiting for projection"
          : runs.ok
            ? `${runs.runs.length} public runs`
            : "projection unavailable",
      ]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Immediate command feedback and projection catch-up from the public Worker reads and Bun-held operator calls.",
    ]),
    h.ul([cls("training-gates training-operator-feed")], [
      trainingGate(
        "projection",
        runs === null
          ? trainingStatusText(model.trainingRunsStatus, "not loaded")
          : trainingProjectionMeta(runs),
        trainingProjectionFeedTone(
          model.trainingRunsPending,
          runs?.ok ?? null,
          model.trainingRunsStatus,
        ),
      ),
      trainingGate(
        "dashboards",
        dashboard === null
          ? trainingStatusText(model.trainingDashboardStatus, "not loaded")
          : trainingStatusText(
              model.trainingDashboardStatus,
              `${dashboard.leaderboards.lanes.length} lanes`,
            ),
        trainingProjectionFeedTone(
          model.trainingDashboardPending,
          dashboard?.ok ?? null,
          model.trainingDashboardStatus,
        ),
      ),
      trainingGate(
        "promise gates",
        gates === null
          ? trainingStatusText(model.trainingPromiseGatesStatus, "not loaded")
          : trainingStatusText(
              model.trainingPromiseGatesStatus,
              `${gates.promises.length} promises`,
            ),
        trainingProjectionFeedTone(
          model.trainingPromiseGatesPending,
          gates?.ok ?? null,
          model.trainingPromiseGatesStatus,
        ),
      ),
      trainingGate(
        "operator readiness",
        readiness === null
          ? trainingStatusText(
              model.trainingOperatorReadinessStatus,
              "not loaded",
            )
          : trainingStatusText(
              model.trainingOperatorReadinessStatus,
              `${readiness.blockerRefs.length} blockers`,
            ),
        trainingProjectionFeedTone(
          model.trainingOperatorReadinessPending,
          readiness?.ok ?? null,
          model.trainingOperatorReadinessStatus,
        ),
      ),
      trainingGate(
        "evidence packet",
        `${trainingStatusText(model.trainingEvidencePacketSummaryStatus, "not loaded")} · ${packetRef}`,
        trainingProjectionFeedTone(
          model.trainingEvidencePacketSummaryPending,
          packetSummary?.ok ?? null,
          model.trainingEvidencePacketSummaryStatus,
        ),
      ),
      trainingGate(
        "plan R1",
        `${trainingStatusText(model.trainingPlanStatus, "idle")} · ${planRef}`,
        trainingStatusTone(model.trainingPlanStatus, model.trainingPlanPending),
      ),
      trainingGate(
        "activate",
        `${trainingStatusText(model.trainingActivationStatus, "idle")} · ${activationRef}`,
        trainingStatusTone(
          model.trainingActivationStatus,
          model.trainingActivationPending,
        ),
      ),
      trainingGate(
        "claim lease",
        `${trainingStatusText(model.trainingLeaseStatus, "idle")} · ${leaseRef}`,
        trainingStatusTone(model.trainingLeaseStatus, model.trainingLeasePending),
      ),
      trainingGate(
        "bootstrap",
        `${trainingStatusText(model.trainingBootstrapStatus, "idle")} · ${bootstrapRef}`,
        trainingStatusTone(
          model.trainingBootstrapStatus,
          model.trainingBootstrapPending,
        ),
      ),
      trainingGate(
        "closeout",
        trainingStatusText(model.trainingCloseoutStatus, "idle"),
        trainingStatusTone(
          model.trainingCloseoutStatus,
          model.trainingCloseoutPending,
        ),
      ),
      trainingGate(
        "build packet",
        `${trainingStatusText(model.trainingEvidencePacketBuildStatus, "idle")} · ${packetBuildRef}`,
        trainingStatusTone(
          model.trainingEvidencePacketBuildStatus,
          model.trainingEvidencePacketBuildPending,
        ),
      ),
      trainingGate(
        "admit evidence",
        `${trainingStatusText(model.trainingEvidenceAdmissionStatus, "idle")} · ${evidenceRef}`,
        trainingStatusTone(
          model.trainingEvidenceAdmissionStatus,
          model.trainingEvidenceAdmissionPending,
        ),
      ),
      trainingGate(
        "reconcile",
        `${trainingStatusText(model.trainingReconcileStatus, "idle")} · ${reconcileRef}`,
        trainingStatusTone(
          model.trainingReconcileStatus,
          model.trainingReconcilePending,
        ),
      ),
      trainingGate(
        "launch check",
        trainingStatusText(model.trainingLaunchStatus, "idle"),
        trainingStatusTone(
          model.trainingLaunchStatus,
          model.trainingLaunchPending,
        ),
      ),
    ]),
  ])
}

const trainingLaunchPanel = (model: Model): Html => {
  const plan = modelTrainingPlan(model)
  const lease = modelTrainingLease(model)?.lease ?? null
  const bootstrap = modelTrainingBootstrap(model)
  const selectedRunRef =
    selectedTrainingSummary(modelTrainingRuns(model))?.run.trainingRunRef ??
    plan?.trainingRunRef ??
    null
  const bootstrapRunRef = selectedRunRef
  const closeoutRunRef = selectedRunRef
  const closeoutWindow = closeoutWindowRef(model)
  const bootstrapGrantRef =
    bootstrap?.outcome?.kind === "granted"
      ? bootstrap.outcome.grant.grantRef
      : null
  const activatableWindowRef = activationWindowRef(model)
  const reconciliableWindowRef = reconcileWindowRef(model)
  const claimableWindowKnown = hasClaimableTrainingWindow(model)
  const planStatusVisible = model.trainingPlanStatus.tone !== "idle"
  const activationStatusVisible =
    model.trainingActivationStatus.tone !== "idle"
  const reconcileStatusVisible =
    model.trainingReconcileStatus.tone !== "idle"
  const leaseStatusVisible = model.trainingLeaseStatus.tone !== "idle"
  const bootstrapStatusVisible =
    model.trainingBootstrapStatus.tone !== "idle"
  const evidencePacketBuildStatusVisible =
    model.trainingEvidencePacketBuildStatus.tone !== "idle"
  const evidenceAdmissionStatusVisible =
    model.trainingEvidenceAdmissionStatus.tone !== "idle"
  const launchStatusVisible = model.trainingLaunchStatus.tone !== "idle"
  const closeoutStatusVisible =
    model.trainingCloseoutStatus.tone !== "idle"
  const activateAttrs: Attribute<Message>[] = [
    cls("training-action-button training-activate-button secondary"),
    h.Type("button"),
    h.Disabled(model.trainingActivationPending || activatableWindowRef === null),
  ]
  if (activatableWindowRef !== null) {
    activateAttrs.push(
      h.OnClick(
        ClickedActivateTrainingWindow({ windowRef: activatableWindowRef }),
      ),
    )
  }
  const leaseAttrs: Attribute<Message>[] = [
    cls("training-action-button training-lease-button secondary"),
    h.Type("button"),
    h.Disabled(model.trainingLeasePending || !claimableWindowKnown),
  ]
  if (claimableWindowKnown) {
    leaseAttrs.push(h.OnClick(ClickedClaimTrainingLease()))
  }
  const bootstrapAttrs: Attribute<Message>[] = [
    cls("training-action-button training-bootstrap-button secondary"),
    h.Type("button"),
    h.Disabled(model.trainingBootstrapPending || bootstrapRunRef === null),
  ]
  if (bootstrapRunRef !== null) {
    bootstrapAttrs.push(
      h.OnClick(
        ClickedRequestTrainingBootstrap({ trainingRunRef: bootstrapRunRef }),
      ),
    )
  }
  const closeoutAttrs: Attribute<Message>[] = [
    cls("training-action-button training-closeout-button secondary"),
    h.Type("button"),
    h.Disabled(model.trainingCloseoutPending || closeoutRunRef === null),
  ]
  if (closeoutRunRef !== null) {
    closeoutAttrs.push(
      h.OnClick(
        ClickedQueueTrainingCloseout({
          trainingRunRef: closeoutRunRef,
          windowRef: closeoutWindow,
          leaseRef: lease?.leaseRef ?? null,
          bootstrapGrantRef,
        }),
      ),
    )
  }
  const evidenceBuildAttrs: Attribute<Message>[] = [
    cls("training-action-button training-evidence-build-button secondary"),
    h.Type("button"),
    h.Disabled(
      model.trainingEvidencePacketBuildPending || closeoutRunRef === null,
    ),
  ]
  if (closeoutRunRef !== null) {
    evidenceBuildAttrs.push(
      h.OnClick(
        ClickedBuildTrainingEvidencePacket({
          trainingRunRef: closeoutRunRef,
        }),
      ),
    )
  }
  const evidenceAttrs: Attribute<Message>[] = [
    cls("training-action-button training-evidence-button secondary"),
    h.Type("button"),
    h.Disabled(
      model.trainingEvidenceAdmissionPending || closeoutRunRef === null,
    ),
  ]
  if (closeoutRunRef !== null) {
    evidenceAttrs.push(
      h.OnClick(
        ClickedAdmitTrainingEvidence({
          trainingRunRef: closeoutRunRef,
        }),
      ),
    )
  }
  const reconcileAttrs: Attribute<Message>[] = [
    cls("training-action-button training-reconcile-button secondary"),
    h.Type("button"),
    h.Disabled(model.trainingReconcilePending || reconciliableWindowRef === null),
  ]
  if (reconciliableWindowRef !== null) {
    reconcileAttrs.push(
      h.OnClick(
        ClickedReconcileTrainingWindow({ windowRef: reconciliableWindowRef }),
      ),
    )
  }
  const refRows: Html[] = []
  if (plan?.trainingRunRef !== null && plan?.trainingRunRef !== undefined) {
    refRows.push(
      h.li([], [h.code([], [plan.trainingRunRef])]),
    )
  }
  if (plan?.windowRef !== null && plan?.windowRef !== undefined) {
    refRows.push(
      h.li([], [h.code([], [plan.windowRef])]),
    )
  }
  if (lease !== null) {
    refRows.push(
      h.li([], [
        h.code([], [lease.leaseRef]),
        ` · ${lease.windowRef} · ${lease.leaseExpiresInSeconds}s`,
      ]),
    )
  }
  if (bootstrap?.outcome?.kind === "granted") {
    refRows.push(
      h.li([], [
        h.code([], [bootstrap.outcome.grant.grantRef]),
        ` · ${bootstrap.outcome.grant.sealedWindowRef}`,
      ]),
    )
  }

  return h.section([cls("training-panel training-action-panel")], [
    h.h2([cls("training-panel-title")], ["Run Operations"]),
    h.p(
      [cls("training-panel-copy")],
      [
        "Plan, activate, claim, bootstrap, build evidence packets, admit evidence, reconcile, and queue closeout prep through Bun and local Pylon.",
      ],
    ),
    h.div(
      [cls("training-action-row")],
      [
        h.button(
          [
            cls("training-action-button training-admin-plan-button"),
            h.Type("button"),
            h.Disabled(model.trainingPlanPending),
            h.OnClick(ClickedPlanTrainingWindow()),
          ],
          [model.trainingPlanPending ? "Planning..." : "Plan R1 window"],
        ),
        h.button(
          [
            cls("training-action-button training-queue-button secondary"),
            h.Type("button"),
            h.Disabled(model.trainingLaunchPending),
            h.OnClick(ClickedQueueTrainingLaunch()),
          ],
          [model.trainingLaunchPending ? "Queueing..." : "Queue launch check"],
        ),
        h.button(
          activateAttrs,
          [
            model.trainingActivationPending
              ? "Activating..."
              : activatableWindowRef === null
                ? "No planned window"
                : "Activate window",
          ],
        ),
        h.button(
          leaseAttrs,
          [
            model.trainingLeasePending
              ? "Claiming..."
              : claimableWindowKnown
                ? "Claim lease"
                : "No active window",
          ],
        ),
        h.button(
          bootstrapAttrs,
          [
            model.trainingBootstrapPending
              ? "Requesting..."
              : bootstrapRunRef === null
                ? "No run selected"
                : "Request bootstrap",
          ],
        ),
        h.button(
          closeoutAttrs,
          [
            model.trainingCloseoutPending
              ? "Queueing..."
              : closeoutRunRef === null
                ? "No run selected"
                : "Queue closeout packet",
          ],
        ),
        h.button(
          evidenceBuildAttrs,
          [
            model.trainingEvidencePacketBuildPending
              ? "Building..."
              : closeoutRunRef === null
                ? "No run selected"
                : "Build evidence packet",
          ],
        ),
        h.button(
          evidenceAttrs,
          [
            model.trainingEvidenceAdmissionPending
              ? "Admitting..."
              : closeoutRunRef === null
                ? "No run selected"
                : "Admit evidence packet",
          ],
        ),
        h.button(
          reconcileAttrs,
          [
            model.trainingReconcilePending
              ? "Reconciling..."
              : reconciliableWindowRef === null
                ? "No sealed window"
                : "Reconcile window",
          ],
        ),
      ],
    ),
    planStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingPlanStatus.tone}`,
            ),
          ],
          [model.trainingPlanStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    refRows.length > 0
      ? h.ul([cls("training-api-list training-plan-refs")], refRows)
      : h.p([cls("training-action-status")], [" "]),
    activationStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingActivationStatus.tone}`,
            ),
          ],
          [model.trainingActivationStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    leaseStatusVisible
      ? h.p(
          [
            cls(`training-action-status training-${model.trainingLeaseStatus.tone}`),
          ],
          [model.trainingLeaseStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    bootstrapStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingBootstrapStatus.tone}`,
            ),
          ],
          [model.trainingBootstrapStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    reconcileStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingReconcileStatus.tone}`,
            ),
          ],
          [model.trainingReconcileStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    evidencePacketBuildStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingEvidencePacketBuildStatus.tone}`,
            ),
          ],
          [model.trainingEvidencePacketBuildStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    evidenceAdmissionStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingEvidenceAdmissionStatus.tone}`,
            ),
          ],
          [model.trainingEvidenceAdmissionStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    launchStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingLaunchStatus.tone}`,
            ),
          ],
          [model.trainingLaunchStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
    closeoutStatusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingCloseoutStatus.tone}`,
            ),
          ],
          [model.trainingCloseoutStatus.text],
        )
      : h.p([cls("training-action-status")], [" "]),
  ])
}

const trainingPane = (model: Model): Html => {
  const projection = modelTrainingRuns(model)
  return h.div(
    [cls("training-page")],
    [
      h.header([cls("training-topline")], [
        h.div([], [
          paneTitle("Training"),
          h.p(
            [cls("node-status")],
            [
              "Tassadar/Psion run projection",
            ],
          ),
        ]),
        h.div([cls("training-ref")], [trainingProjectionMeta(projection)]),
      ]),
      h.section([cls("training-visual")], [
        h.div([cls("training-visual-copy")], [
          h.h2([cls("training-visual-title")], ["Run Window"]),
          h.p(
            [cls("training-visual-caption")],
            [
              "Lifecycle, staleness, seal, Freivalds, receipts, settlement, and ladder readiness in one live Three surface.",
            ],
          ),
        ]),
        trainingRunView<Message>(
          [cls("three-effect-training")],
          trainingSceneOptions(model),
        ),
      ]),
      h.div([cls("training-grid")], [
        publicActivityPane(model, {
          className: "training-public-activity-panel",
          maxEvents: 6,
          title: "Public Activity",
        }),
        liveTrainingProjectionPanel(model),
        proofReplayPanel(model),
        selectedTrainingLifecyclePanel(projection),
        trainingDashboardPanel(model),
        trainingPromiseGatesPanel(model),
        selectedTrainingEvidencePanel(projection),
        selectedTrainingLedgerPanel(projection),
        trainingLaunchPanel(model),
        trainingOperatorReadinessPanel(model),
        trainingEvidencePacketPanel(model),
        trainingOperatorFeedPanel(model),
        trainingProjectionCatchUpPanel(model),
      ]),
    ],
  )
}

const trainingFullscreenPane = (model: Model): Html => {
  const visualization = trainingSceneOptions(model)
  const nodes = visualization?.nodes ?? defaultTrainingRunNodes
  const selectedNode = selectedTrainingSceneNode(
    nodes,
    model.selectedTrainingSceneNodeId,
  )

  return h.div([cls("training-fullscreen-page")], [
    h.div([cls("training-fullscreen-scene")], [
      trainingRunView<Message>(
        [cls("three-effect-training-fullscreen")],
        visualization,
        node => SelectedTrainingSceneNode({ nodeId: node.id }),
      ),
    ]),
    h.div([cls("training-fullscreen-overlay")], [
      h.section([cls("training-fullscreen-title")], [
        h.span([cls("training-fullscreen-eyebrow")], [
          trainingProjectionMeta(modelTrainingRuns(model)),
        ]),
        h.h1([], ["Training Live"]),
        h.p([], [
          selectedTrainingSummary(modelTrainingRuns(model))?.run.trainingRunRef ??
            "waiting for Worker projection",
        ]),
      ]),
      h.section(
        [cls("training-fullscreen-stats")],
        trainingFullscreenStats(model).map(trainingFullscreenStatView),
      ),
    ]),
    selectedNode === null
      ? h.empty
      : trainingFullscreenNodePanel(selectedNode, model),
  ])
}

// ── Sessions pane ─────────────────────────────────────────────────────────────

const FILTERS: ReadonlyArray<SessionFilter> = [
  "all",
  "running",
  "queued",
  "completed",
  "failed",
  "cancelled",
]

const sessionsPane = (model: Model): Html => {
  const node = modelNode(model)
  const allSessions: ReadonlyArray<SessionSummary> = node?.sessions ?? []
  const filtered =
    model.sessionFilter === "all"
      ? allSessions
      : allSessions.filter((s) => s.state === model.sessionFilter)

  return h.div(
    [],
    [
      paneTitle("Sessions"),
      h.p(
        [cls("node-status")],
        [
          allSessions.length === 0
            ? node
              ? "No sessions."
              : "Connecting…"
            : stateBreakdown(allSessions),
        ],
      ),
      h.div(
        [cls("filter-bar")],
        FILTERS.map((f) =>
          h.button(
            [
              cls(`filter-btn${f === model.sessionFilter ? " active" : ""}`),
              h.Type("button"),
              h.OnClick(ChangedSessionFilter({ filter: f })),
            ],
            [f === "all" ? "All" : f],
          ),
        ),
      ),
      node === null
        ? emptyLine("Connecting…")
        : h.div(
            [cls("session-list")],
            filtered.length === 0
              ? [emptyLine("No sessions.")]
              : filtered.map((session) =>
                  h.div(
                    [
                      cls("session-click"),
                      h.Tabindex(0),
                      h.DataAttribute("autopilot-session-ref", session.sessionRef),
                      h.OnClick(SelectedSession({ sessionRef: session.sessionRef })),
                    ],
                    [SessionList({ sessions: [session] })],
                  ),
                ),
          ),
    ],
  )
}

// ── CS-A2 (#5362): Swarm / multi-session view ───────────────────────────────
//
// A lane/grid over the N concurrent coding sessions the runtime can run. Each
// cell is a pure read projection over node-state (session + its event tail +
// the global approvals queue): status, the account it runs under (CS-A1 data),
// repo/worktree, a per-cell pending-approval count, plus per-cell quick actions
// (open in composer / cancel). A top-level roll-up shows the pending approvals
// across ALL sessions and links to the Decisions queue. No new wire verb — it
// is `session.list` + the existing per-session events/approvals.

const swarmCell = (
  session: SessionSummary,
  accounts: ReadonlyArray<AccountRow>,
  events: ReadonlyArray<SessionEventRow> | undefined,
  // #5469: tree depth (0 = root) + direct-child count for the sub-agent tree.
  depth: number,
  childCount: number,
): Html => {
  const status = swarmStatusLabel(session.state)
  const accountLabel = swarmAccountLabel(session, accounts)
  const workspaceLabel = swarmWorkspaceLabel(session)
  const pendingApprovals = swarmSessionPendingApprovals(events)
  const activity = session.latestActivity ?? session.lastProgressRef ?? ""
  const isChild = depth > 0
  const laneLabel = session.lane && session.lane !== "local" ? session.lane : null
  // #5469: refs-only account-failover/routing summary from the event tail.
  const routing = swarmFailoverRouting(events)
  const routingLabel =
    routing.reason !== null ? swarmRoutingReasonLabel(routing.reason) : null

  return h.div(
    [
      cls(`swarm-cell ${status.toneClass}${isChild ? " swarm-cell-child" : ""}`),
      h.DataAttribute("autopilot-session-ref", session.sessionRef),
      // Indent each nesting level so the parentRef hierarchy reads as a tree.
      // Depth is capped at 4 levels of visible indent so deep chains never push
      // a cell off the grid.
      ...(depth > 0
        ? [h.Style({ marginLeft: `${Math.min(depth, 4) * 0.85}rem` })]
        : []),
    ],
    [
      h.div(
        [cls("swarm-cell-head")],
        [
          h.span([cls(`swarm-status ${status.toneClass}`)], [status.text]),
          h.span([cls("swarm-adapter")], [session.adapter]),
          ...(laneLabel ? [h.span([cls("swarm-lane")], [laneLabel])] : []),
          ...(isChild ? [h.span([cls("swarm-child-badge")], ["nested"])] : []),
          ...(childCount > 0
            ? [
                h.span(
                  [cls("swarm-child-count")],
                  [`${childCount} sub-agent${childCount === 1 ? "" : "s"}`],
                ),
              ]
            : []),
        ],
      ),
      h.div([cls("swarm-cell-meta")], [`account: ${accountLabel}`]),
      // #5469: account-failover / routing line (refs-only). Only shown when the
      // event tail surfaced a routing reason; otherwise the active account above
      // is the route.
      ...(routingLabel !== null
        ? [
            h.div(
              [cls(`swarm-cell-routing ${routingLabel.toneClass}`)],
              [
                `route: ${routingLabel.text}`,
                ...(routing.failovers > 0
                  ? [` · re-routed ${routing.failovers}×`]
                  : []),
              ],
            ),
          ]
        : []),
      h.div([cls("swarm-cell-meta")], [`repo: ${workspaceLabel}`]),
      ...(activity.trim() !== ""
        ? [h.div([cls("swarm-cell-activity")], [activity])]
        : []),
      ...(pendingApprovals > 0
        ? [
            h.div(
              [cls("swarm-cell-approvals")],
              [
                `${pendingApprovals} pending approval${pendingApprovals === 1 ? "" : "s"}`,
              ],
            ),
          ]
        : []),
      h.div(
        [cls("swarm-cell-actions")],
        [
          h.button(
            [
              cls("swarm-action"),
              h.Type("button"),
              h.OnClick(
                ClickedOpenSessionInComposer({
                  sessionRef: session.sessionRef,
                  workspaceRef: session.workspaceRef ?? null,
                  adapter: session.adapter,
                }),
              ),
            ],
            ["Open in composer"],
          ),
          h.button(
            [
              cls("swarm-action"),
              h.Type("button"),
              h.OnClick(SelectedSession({ sessionRef: session.sessionRef })),
            ],
            ["Details"],
          ),
          ...(sessionCancellable(session.state)
            ? [
                h.button(
                  [
                    cls("swarm-action swarm-action-cancel"),
                    h.Type("button"),
                    h.OnClick(
                      ClickedCancelSession({ sessionRef: session.sessionRef }),
                    ),
                  ],
                  ["Cancel"],
                ),
              ]
            : []),
        ],
      ),
    ],
  )
}

// #5469 (EPIC #5461): the swarm batch-launch form. Lives INSIDE the swarm pane
// (audit §5.2: no new top-level button). Launches a bounded batch of sessions
// over the EXISTING session.spawn verb with a VISIBLE concurrency cap. The
// adapter/lane/account come from the shared spawn-form state (so the batch runs
// on the same selection the rest of the desktop uses); only the objective set +
// the cap are batch-specific.
const swarmBatchForm = (model: Model): Html => {
  const running = swarmBatchRunning({
    queue: model.swarmBatchQueue,
    active: model.swarmBatchActive,
    concurrency: Number(model.swarmBatchConcurrency) || 1,
    launched: model.swarmBatchLaunched,
    failed: model.swarmBatchFailed,
    total: model.swarmBatchTotal,
  })
  const objectiveCount = parseSwarmBatchObjectives(model.swarmBatchObjectives).length
  const statusLine = swarmBatchStatusLine({
    queue: model.swarmBatchQueue,
    active: model.swarmBatchActive,
    concurrency: Number(model.swarmBatchConcurrency) || 1,
    launched: model.swarmBatchLaunched,
    failed: model.swarmBatchFailed,
    total: model.swarmBatchTotal,
  })
  const adapterLabel =
    model.spawnAdapter === "apple_fm" ? "claude_agent" : model.spawnAdapter

  return h.details(
    [cls("swarm-batch")],
    [
      h.summary([cls("swarm-batch-summary")], ["Batch launch"]),
      h.p(
        [cls("swarm-batch-hint")],
        [
          `One objective per line (max ${SWARM_BATCH_MAX_OBJECTIVES}). Runs on the ${adapterLabel} adapter / ${model.spawnLane} lane with a bounded concurrency cap.`,
        ],
      ),
      h.textarea(
        [
          cls("text-area"),
          h.Rows(4),
          h.Placeholder("Fix the failing test\nAdd a README section\nBump the dependency"),
          h.Value(model.swarmBatchObjectives),
          h.OnInput((value: string) => ChangedSwarmBatchObjectives({ value })),
        ],
        [],
      ),
      h.div(
        [cls("swarm-batch-controls")],
        [
          h.label([cls("field-label")], ["Max concurrent"]),
          h.input([
            cls("text-input mono"),
            h.Type("number"),
            h.Min("1"),
            h.Max(String(SWARM_BATCH_MAX_CONCURRENCY)),
            h.Value(model.swarmBatchConcurrency),
            h.OnInput((value: string) => ChangedSwarmBatchConcurrency({ value })),
          ]),
          h.button(
            [
              cls("primary-button"),
              h.Type("button"),
              h.Disabled(running || objectiveCount === 0),
              h.OnClick(ClickedSwarmBatchLaunch()),
            ],
            [
              running
                ? "Launching…"
                : `Launch ${objectiveCount} session${objectiveCount === 1 ? "" : "s"}`,
            ],
          ),
        ],
      ),
      statusLine !== ""
        ? h.p([cls("swarm-batch-status")], [statusLine])
        : h.empty,
    ],
  )
}

const swarmPane = (model: Model): Html => {
  const node = modelNode(model)
  const sessions: ReadonlyArray<SessionSummary> = node?.sessions ?? []
  const accounts: ReadonlyArray<AccountRow> = node?.accounts ?? []
  const events = node?.events ?? {}
  const ordered = orderSwarmSessions(sessions)
  // #5469: turn the adjacency ordering into an explicit depth-annotated tree so
  // sub-agents (parentRef children) render nested instead of as flat rows.
  const tree = buildSwarmTree(ordered)
  // The authoritative pending-approval count across all sessions is the node's
  // global queue length (the same queue the Decisions pane resolves).
  const pendingApprovalCount = pendingApprovals(model).length

  return h.div(
    [cls("swarm-pane")],
    [
      paneTitle("Swarm"),
      h.p(
        [cls("node-status")],
        [
          node === null
            ? "Connecting to your local node… Start it with `pylon dev` to drive a swarm."
            : swarmSummaryLine(sessions, pendingApprovalCount),
        ],
      ),
      // #5469: batch launch lives inside the pane (no new top-level button).
      node === null ? h.empty : swarmBatchForm(model),
      // Top-level "pending approvals across all sessions" roll-up → Decisions.
      pendingApprovalCount > 0
        ? h.button(
            [
              cls("swarm-approvals-rollup"),
              h.Type("button"),
              h.OnClick(NavigatedTo({ pane: "decisions" })),
            ],
            [
              `${pendingApprovalCount} approval${pendingApprovalCount === 1 ? "" : "s"} need you — review in Decisions →`,
            ],
          )
        : h.empty,
      node === null
        ? h.empty
        : tree.length === 0
          ? emptyLine("No sessions. Spawn one from the Composer to start a swarm.")
          : h.div(
              [cls("swarm-grid")],
              tree.map((node_) =>
                swarmCell(
                  node_.session,
                  accounts,
                  events[node_.session.sessionRef],
                  node_.depth,
                  node_.childCount,
                ),
              ),
            ),
    ],
  )
}

// ── Bounded auto-approve surface (#5468, EPIC #5461) ────────────────────────
// Surfaced INSIDE the Decisions/Supervise roll-up (not a new pane/button). It
// shows, honestly: that the bounded `--on-approval auto` policy is fail-closed,
// its allow-list + default caps/window, the categories that ALWAYS escalate,
// and — when a session ran under it — the refs-only audit trail of what the
// policy actually decided. Manual approve/deny stays the default; this never
// implies the desktop is auto-approving when no audit trail is present.

const autoApproveCategoryRow = (
  category: (typeof boundedAutoApprovalPolicySummary.categories)[number],
): Html =>
  h.li(
    [cls("auto-approve-legend-row"), h.DataAttribute("autopilot-auto-approve-category", category.id)],
    [
      h.span([cls(`auto-approve-chip auto-approve-chip-${category.id}`)], [category.label]),
      h.span([cls("auto-approve-legend-desc")], [category.description]),
    ],
  )

const autoApproveAuditRowView = (
  row: ReturnType<typeof projectAutoApprovalAudit>[number],
): Html =>
  h.div(
    [
      cls("auto-approve-audit-row"),
      h.DataAttribute("autopilot-auto-approval-ref", row.approvalRef),
      h.DataAttribute("autopilot-auto-approval-category", row.category),
    ],
    [
      h.span([cls(`auto-approve-chip auto-approve-chip-${row.category}`)], [row.categoryLabel]),
      h.span([cls("auto-approve-audit-kind")], [row.kind]),
      h.span([cls("auto-approve-audit-reason")], [row.reasonGloss]),
      h.span([cls("auto-approve-audit-ref")], [row.approvalRef]),
    ],
  )

const boundedAutoApproveCard = (model: Model): Html => {
  const policy = boundedAutoApprovalPolicySummary
  const auditRows = projectAutoApprovalAudit(modelNode(model)?.autoApprovals)
  const summary = summarizeAutoApprovalAudit(auditRows)

  // Honest status line: off by default; "active" only when a real audit trail
  // exists (the desktop does not enable auto-approve itself today).
  const statusLine = summary.active
    ? `Auto-approve active — ${summary.autoApproved} auto-approved, ${summary.escalated} escalated to you, ${summary.denied} denied.`
    : "Auto-approve is OFF by default. Sessions use manual approve/deny unless a run opts into the bounded policy via the CLI."

  return card("Bounded auto-approve", [
    h.p([cls("auto-approve-status")], [statusLine]),
    h.p(
      [cls("auto-approve-failclosed")],
      [
        "Fail-closed: an approval is auto-approved only when its kind is allow-listed, in-scope, and within caps. ",
        "Destructive, spend/secret, and network actions always escalate or deny — never silently approved.",
      ],
    ),
    h.ul([cls("auto-approve-legend")], policy.categories.map(autoApproveCategoryRow)),
    h.div(
      [cls("auto-approve-bounds")],
      [
        h.p([cls("auto-approve-bounds-line")], [
          `Allow-list: ${policy.allowKinds.join(", ")}.`,
        ]),
        h.p([cls("auto-approve-bounds-line")], [
          `Caps: up to ${policy.defaultMaxAutoApprovals} auto-approvals within a ${policy.defaultWindowMinutes}-minute window; over either cap escalates to you.`,
        ]),
        h.p([cls("auto-approve-bounds-line auto-approve-escalates")], [
          `Always escalates / denies: ${policy.alwaysEscalates.join("; ")}.`,
        ]),
        h.p([cls("auto-approve-policy-ref")], [
          `Policy: ${policy.cliFlag} (${policy.policyRef}).`,
        ]),
      ],
    ),
    auditRows.length === 0
      ? h.p([cls("empty-state auto-approve-audit-empty")], [
          "No auto-approve decisions recorded. When a session runs under the bounded policy, each decision appears here with its approval ref, category, and reason.",
        ])
      : h.div(
          [cls("auto-approve-audit")],
          [
            h.h3([cls("auto-approve-audit-title")], [`Audit trail (${summary.total})`]),
            h.div([cls("auto-approve-audit-rows")], auditRows.map(autoApproveAuditRowView)),
          ],
        ),
  ])
}

// ── Decisions pane ────────────────────────────────────────────────────────────

const decisionsPane = (model: Model): Html => {
  const approvals = pendingApprovals(model)
  return h.div(
    [],
    [
      paneTitle("Decisions"),
      approvals.length === 0
        ? emptyLine("Nothing needs you right now.")
        : h.div([cls("decisions-queue")], approvals.map(approvalRowView)),
      // #5468: the bounded auto-approve policy + audit trail, in the roll-up.
      boundedAutoApproveCard(model),
    ],
  )
}

// ── Built-in Agent pane (#5063) ─────────────────────────────────────────────

const builtInAgentStatusText = (
  readiness: BuiltInAgentReadinessResponse | null,
): string => {
  if (readiness === null) return "not checked"
  if (readiness.ok) return "ready"
  if (!readiness.enabled) return "disabled"
  if (!readiness.localPylonReady) return "local node offline"
  if (!readiness.hostedComputeConfigured) return "hosted compute unconfigured"
  return "blocked"
}

const appleFmStatusText = (
  readiness: AppleFmReadinessResponse | null,
): string => {
  if (readiness === null) return "not checked"
  if (readiness.ok) return "ready"
  if (!readiness.localPylonReady) return "local node offline"
  if (readiness.unavailableReason === "unsupported_hardware") return "unsupported"
  if (readiness.unavailableReason === "apple_intelligence_disabled") return "Apple Intelligence disabled"
  if (readiness.unavailableReason === "bridge_unreachable") return "bridge missing"
  if (readiness.status === "malformed") return "malformed health"
  return readiness.status === "unreachable" ? "unavailable" : "blocked"
}

const appleFmDetailText = (
  readiness: AppleFmReadinessResponse | null,
): string =>
  readiness === null
    ? "Local Foundation Models through Pylon."
    : readiness.ok
      ? `${readiness.model} · ${readiness.platform ?? "macOS"}`
      : readiness.message ?? readiness.unavailableReason ?? "Local Apple FM is not ready."

const promiseSurfacingReadinessText = (
  readiness: PromiseSurfacingReadinessResponse | null,
): string => {
  if (readiness === null) return "not checked"
  return readiness.agentTokenPresent ? "Forum posting ready" : "draft only"
}

const promiseSurfacingResultLine = (
  result: PromiseSurfacingResponse | null,
): string | null => {
  if (result === null) return null
  if (result.mode === "posted") {
    return `posted · ${result.topicUrl ?? result.topicId ?? "Product Promises Forum"}`
  }
  if (result.mode === "drafted") {
    return `drafted · ${result.blockerRefs[0] ?? "agent token missing"}`
  }
  return result.error ?? result.blockerRefs[0] ?? "blocked"
}

const promiseSurfacingCard = (model: Model): Html => {
  const readiness = modelPromiseSurfacingReadiness(model)
  const result = modelPromiseSurfacingResult(model)
  const resultLine = promiseSurfacingResultLine(result)
  const stateOptions = [
    "green",
    "yellow",
    "red",
    "degraded",
    "planned",
    "unknown",
  ] as const

  return card("Surface Promise Gap", [
    h.p([cls("card-body")], [
      "Forum: ",
      h.strong([], [promiseSurfacingReadinessText(readiness)]),
    ]),
    readiness?.blockerRefs.length
      ? h.ul(
          [cls("empty-state mono")],
          readiness.blockerRefs.map(blocker => h.li([], [blocker])),
        )
      : h.empty,
    h.label([cls("field-label")], ["Promise ID"]),
    h.input([
      cls("text-input mono"),
      h.Placeholder("autopilot.builtin_compute_agent.v1"),
      h.Value(model.promiseSurfacingPromiseId),
      h.OnInput((value: string) =>
        ChangedPromiseSurfacingPromiseId({ value }),
      ),
    ]),
    h.label([cls("field-label")], ["Surface"]),
    h.input([
      cls("text-input"),
      h.Value(model.promiseSurfacingSurface),
      h.OnInput((value: string) =>
        ChangedPromiseSurfacingSurface({ value }),
      ),
    ]),
    h.label([cls("field-label")], ["Claim text"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(2),
        h.Value(model.promiseSurfacingClaimText),
        h.OnInput((value: string) =>
          ChangedPromiseSurfacingClaimText({ value }),
        ),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Expected"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(2),
        h.Value(model.promiseSurfacingExpectedBehavior),
        h.OnInput((value: string) =>
          ChangedPromiseSurfacingExpectedBehavior({ value }),
        ),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Observed"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(3),
        h.Value(model.promiseSurfacingObservedBehavior),
        h.OnInput((value: string) =>
          ChangedPromiseSurfacingObservedBehavior({ value }),
        ),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Evidence or steps"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(3),
        h.Value(model.promiseSurfacingEvidenceOrSteps),
        h.OnInput((value: string) =>
          ChangedPromiseSurfacingEvidenceOrSteps({ value }),
        ),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Environment"]),
    h.input([
      cls("text-input"),
      h.Value(model.promiseSurfacingEnvironment),
      h.OnInput((value: string) =>
        ChangedPromiseSurfacingEnvironment({ value }),
      ),
    ]),
    h.label([cls("field-label")], ["Impact"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(2),
        h.Value(model.promiseSurfacingImpact),
        h.OnInput((value: string) =>
          ChangedPromiseSurfacingImpact({ value }),
        ),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Suggested state"]),
    h.div(
      [cls("adapter-toggle")],
      stateOptions.map(state =>
        h.button(
          [
            cls(
              `adapter-btn${model.promiseSurfacingSuggestedState === state ? " active" : ""}`,
            ),
            h.Type("button"),
            h.OnClick(ChangedPromiseSurfacingSuggestedState({ value: state })),
          ],
          [state],
        ),
      ),
    ),
    result?.draft
      ? h.div([cls("empty-state mono")], [
          h.p([], [`title: ${result.draft.title}`]),
          h.p([], [`ledger: ${result.draft.ledgerVerdict}`]),
          h.p([], [`registry: ${result.draft.registryVersion}`]),
        ])
      : h.empty,
    resultLine
      ? h.p([cls(`spawn-status spawn-${model.promiseSurfacingStatus.tone}`)], [
          resultLine,
        ])
      : model.promiseSurfacingStatus.tone !== "idle"
        ? h.p([cls(`spawn-status spawn-${model.promiseSurfacingStatus.tone}`)], [
            model.promiseSurfacingStatus.text,
          ])
        : h.p([cls("spawn-status")], [" "]),
    h.div([cls("adapter-toggle")], [
      h.button(
        [
          cls("primary-button"),
          h.Type("button"),
          h.Disabled(model.promiseSurfacingSubmitPending),
          h.OnClick(ClickedSurfacePromiseGap()),
        ],
        [
          model.promiseSurfacingSubmitPending
            ? "Surfacing..."
            : readiness?.agentTokenPresent
              ? "Surface to Forum"
              : "Draft report",
        ],
      ),
      h.button(
        [
          cls("adapter-btn"),
          h.Type("button"),
          h.Disabled(model.promiseSurfacingReadinessPending),
          h.OnClick(ClickedRefreshPromiseSurfacing()),
        ],
        [model.promiseSurfacingReadinessPending ? "Checking..." : "Refresh"],
      ),
    ]),
  ])
}

const builtInAgentPane = (model: Model): Html => {
  const readiness = modelBuiltInAgentReadiness(model)
  const appleFmReadiness = modelAppleFmReadiness(model)
  const blockers = readiness?.blockerRefs ?? []
  const appleFmBlockers = appleFmReadiness?.blockerRefs ?? []
  const hostedSelected = model.agentMode === "hosted"
  const localSelected = model.agentMode === "local-apple-fm"
  const canStart = (readiness === null || readiness.ok) && hostedSelected
  const canStartLocalAppleFm = localSelected && (appleFmReadiness?.ok ?? false)
  const statusVisible = model.builtInAgentStatus.tone !== "idle"
  const appleStatusVisible = model.appleFmStatus.tone !== "idle"

  return h.div(
    [],
    [
      paneTitle("Agent"),
      card("Hosted OpenAgents Compute", [
        h.p([cls("card-body")], [
          "Status: ",
          h.strong([], [builtInAgentStatusText(readiness)]),
        ]),
        h.p([cls("card-body")], [
          "Compute: ",
          h.strong([], [
            readiness?.hostedComputeConfigured
              ? `OpenAgents hosted · ${readiness.modelSet}`
              : "OpenAgents hosted",
          ]),
        ]),
        h.p([cls("card-body")], [
          "User key: ",
          h.strong([], ["not required"]),
        ]),
        h.p([cls("card-body")], [
          "Bounds: ",
          h.strong([], [
            readiness
              ? `${readiness.meteringLabel} · ${readiness.dailySessionsUsed}/${readiness.dailySessionCap} used today`
              : "3 sessions/day · 600s/session",
          ]),
        ]),
        h.p([cls("card-body")], [
          "Lane: ",
          h.strong([], [readiness ? spawnLaneLabel(readiness.lane) : "Google GCE"]),
        ]),
        h.p([cls("card-body")], [
          "Mode: ",
          h.strong([], [hostedSelected ? "selected" : "available"]),
        ]),
        blockers.length > 0
          ? h.ul(
              [cls("empty-state mono")],
              blockers.map((blocker) => h.li([], [blocker])),
            )
          : h.empty,
        statusVisible
          ? h.p([cls(`spawn-status spawn-${model.builtInAgentStatus.tone}`)], [
              model.builtInAgentStatus.text,
            ])
          : h.p([cls("spawn-status")], [" "]),
        h.div(
          [cls("adapter-toggle")],
          [
            h.button(
              [
                cls(`adapter-btn${hostedSelected ? " active" : ""}`),
                h.Type("button"),
                h.OnClick(SelectedAgentMode({ mode: "hosted" })),
              ],
              [hostedSelected ? "Hosted selected" : "Use hosted"],
            ),
            h.button(
              [
                cls("primary-button"),
                h.Type("button"),
                h.Disabled(model.builtInAgentPending || !canStart),
                h.OnClick(ClickedStartBuiltInAgent()),
              ],
              [
                model.builtInAgentPending
                  ? "Going online..."
                  : hostedSelected
                    ? "Go online"
                    : "Select hosted first",
              ],
            ),
            h.button(
              [
                cls("adapter-btn"),
                h.Type("button"),
                h.Disabled(model.builtInAgentPending),
                h.OnClick(ClickedRefreshBuiltInAgent()),
              ],
              ["Refresh"],
            ),
          ],
        ),
      ]),
      card("Local Apple FM", [
        h.p([cls("card-body")], [
          "Status: ",
          h.strong([], [appleFmStatusText(appleFmReadiness)]),
        ]),
        h.p([cls("card-body")], [
          "Compute: ",
          h.strong([], ["on-device Apple Foundation Models"]),
        ]),
        h.p([cls("card-body")], [
          "Bridge: ",
          h.strong([], [appleFmReadiness?.baseUrl ?? "Pylon loopback"]),
        ]),
        h.p([cls("card-body")], [
          "Mode: ",
          h.strong([], [localSelected ? "selected" : "optional"]),
        ]),
        h.p([cls("card-body")], [appleFmDetailText(appleFmReadiness)]),
        appleFmBlockers.length > 0
          ? h.ul(
              [cls("empty-state mono")],
              appleFmBlockers.map((blocker) => h.li([], [blocker])),
            )
          : h.empty,
        localSelected && appleFmBlockers.length > 0
          ? h.p([cls("spawn-status spawn-info")], [
              appleFmBlockers[0] ?? "local Apple FM blocked",
            ])
          : appleStatusVisible
            ? h.p([cls(`spawn-status spawn-${model.appleFmStatus.tone}`)], [
                model.appleFmStatus.text,
              ])
            : h.p([cls("spawn-status")], [" "]),
        h.div([cls("adapter-toggle")], [
          h.button(
            [
              cls(`adapter-btn${localSelected ? " active" : ""}`),
              h.Type("button"),
              h.OnClick(SelectedAgentMode({ mode: "local-apple-fm" })),
            ],
            [localSelected ? "Local selected" : "Use local Apple FM"],
          ),
          h.button(
            [
              cls("adapter-btn"),
              h.Type("button"),
              h.Disabled(model.appleFmPending),
              h.OnClick(ClickedRefreshAppleFm()),
            ],
            [model.appleFmPending ? "Checking..." : "Refresh local"],
          ),
          h.button(
            [
              cls("adapter-btn"),
              h.Type("button"),
              h.Disabled(model.appleFmPending || !canStartLocalAppleFm),
              h.OnClick(ClickedStartAppleFm()),
            ],
            [model.appleFmPending ? "Starting..." : "Start local"],
          ),
        ]),
      ]),
      promiseSurfacingCard(model),
    ],
  )
}

// ── Spawn pane ────────────────────────────────────────────────────────────────

// #4998: human-readable label for an execution lane in the spawn picker.
const spawnLaneLabel = (
  lane: "auto" | "local" | "cloud-gcp" | "cloud-shc",
): string => {
  switch (lane) {
    case "auto":
      return "Auto"
    case "local":
      return "Local"
    case "cloud-gcp":
      return "Google GCE"
    case "cloud-shc":
      return "SHC"
  }
}

// #4998: short "running on …" provenance for a session's recorded lane, shown
// where the session's lane is available (session list / detail).
export const sessionLaneProvenance = (
  lane: "auto" | "local" | "cloud-gcp" | "cloud-shc" | undefined,
): string | null => {
  switch (lane) {
    case "cloud-gcp":
      return "running on Google GCE"
    case "cloud-shc":
      return "running on SHC"
    case "local":
      return "running locally"
    case "auto":
    case undefined:
      return null
  }
}

const spawnPane = (model: Model): Html => {
  const statusVisible = model.spawnStatus.tone !== "idle"
  return h.div(
    [],
    [
      paneTitle("Spawn"),
      card("New Session", [
        h.label([cls("field-label")], ["Adapter"]),
        h.div(
          [cls("adapter-toggle")],
          (["codex", "claude_agent"] as const).map((adapter) =>
            h.button(
              [
                cls(`adapter-btn${model.spawnAdapter === adapter ? " active" : ""}`),
                h.Type("button"),
                h.OnClick(ChangedSpawnAdapter({ adapter })),
              ],
              [adapter],
            ),
          ),
        ),
        // #4998: execution-lane selector. auto = own-Pylon-first then Google
        // GCE; cloud-gcp = Google GCE (default cloud); cloud-shc = SHC fallback.
        h.label([cls("field-label")], ["Execution lane"]),
        h.div(
          [cls("adapter-toggle")],
          (["auto", "local", "cloud-gcp", "cloud-shc"] as const).map((lane) =>
            h.button(
              [
                cls(`adapter-btn${model.spawnLane === lane ? " active" : ""}`),
                h.Type("button"),
                h.OnClick(ChangedSpawnLane({ lane })),
              ],
              [spawnLaneLabel(lane)],
            ),
          ),
        ),
        h.label([cls("field-label")], ["Objective"]),
        h.textarea(
          [
            cls("text-area"),
            h.Rows(5),
            h.Placeholder("Describe the session objective…"),
            h.Value(model.spawnObjective),
            h.OnInput((value: string) => ChangedSpawnObjective({ value })),
          ],
          [],
        ),
        h.label([cls("field-label")], ["Verify commands (optional — one per line)"]),
        h.textarea(
          [
            cls("text-area mono"),
            h.Rows(3),
            h.Placeholder("bun test\nbun run typecheck"),
            h.Value(model.spawnVerify),
            h.OnInput((value: string) => ChangedSpawnVerify({ value })),
          ],
          [],
        ),
        statusVisible
          ? h.p([cls(`spawn-status spawn-${model.spawnStatus.tone}`)], [
              model.spawnStatus.text,
            ])
          : h.p([cls("spawn-status")], [" "]),
        h.button(
          [
            cls("primary-button"),
            h.Type("button"),
            h.Disabled(model.spawnPending),
            h.OnClick(ClickedSpawn()),
          ],
          [model.spawnPending ? "Spawning…" : "Spawn session"],
        ),
      ]),
    ],
  )
}

// ── Settings pane ─────────────────────────────────────────────────────────────

const installReadinessTone = (
  status: InstallReadinessResponse["items"][number]["status"],
): string => {
  switch (status) {
    case "ready":
      return "ready"
    case "waiting":
      return "watch"
    case "attention":
      return "watch"
    case "blocked":
      return "blocked"
  }
}

const installReadinessSummary = (
  readiness: InstallReadinessResponse | null,
): string =>
  readiness === null
    ? "checking first-run health..."
    : readiness.ok
      ? "ready"
      : `${readiness.highestRoiAction} · ${readiness.blockerRefs.length} blocker${readiness.blockerRefs.length === 1 ? "" : "s"}`

const installReadinessRows = (
  readiness: InstallReadinessResponse | null,
): ReadonlyArray<Html> => {
  if (readiness === null) {
    return [
      h.li([cls("readiness-row")], [
        h.span([cls("readiness-name")], ["First-run health"]),
        h.span([cls("readiness-detail")], ["not checked"]),
      ]),
    ]
  }
  return readiness.items.map(item =>
    h.li([cls(`readiness-row readiness-${installReadinessTone(item.status)}`)], [
      h.span([cls("readiness-name")], [item.label]),
      h.span([cls("readiness-detail")], [item.detail]),
      h.code([cls("readiness-status")], [item.status]),
    ]),
  )
}

// #5472: a reusable segmented toggle for a Settings preference. Each option is
// a button; the active one is highlighted. Reuses the existing `.adapter-toggle`
// / `.adapter-btn` styles so preferences match the spawn-form look. `onSelect`
// maps the chosen option to the Message the reducer handles.
const settingsToggle = <T extends string>(
  options: ReadonlyArray<{ readonly value: T; readonly label: string }>,
  selected: T,
  onSelect: (value: T) => Message,
): Html =>
  h.div(
    [cls("adapter-toggle")],
    options.map((option) =>
      h.button(
        [
          cls(`adapter-btn${selected === option.value ? " active" : ""}`),
          h.Type("button"),
          h.OnClick(onSelect(option.value)),
        ],
        [option.label],
      ),
    ),
  )

// #5485: a compact, presentational line describing how the next coding turn's
// inference will be paid for (own auth vs OpenAgents gateway credits) plus the
// gateway credit balance / low-balance hint. Reads the live routing decision +
// the gateway readiness off the model — no spend authority, no key, refs only.
const inferenceRouteHint = (model: Model): Html => {
  const decision = modelInferenceDecision(model)
  const readiness = modelInferenceGatewayReadiness(model)
  const balance = readiness?.creditBalance ?? null
  const threshold = readiness?.lowBalanceThreshold ?? 1
  const low = isGatewayBalanceLow(balance, threshold)
  const routeLabel =
    decision.route === "gateway"
      ? "OpenAgents gateway credits (pay-as-you-go)"
      : decision.route === "own_auth"
        ? "Your own auth"
        : "Unavailable"
  const tone: "error" | "info" | "success" | "idle" =
    decision.route === "blocked"
      ? "error"
      : decision.route === "gateway" && low
        ? "info"
        : decision.route === "gateway"
          ? "success"
          : "idle"
  const balanceText =
    balance === null
      ? readiness?.enabled
        ? "balance unknown"
        : "gateway off"
      : `${balance} credits${low ? " · low balance" : ""}`
  return h.p(
    [
      cls(`field-hint inference-route-hint inference-route-${tone}`),
      h.DataAttribute("autopilot-inference-route", decision.route),
    ],
    [
      "Inference: ",
      h.strong([], [routeLabel]),
      decision.usedFallback ? " (no own auth — using credits)" : "",
      " · ",
      balanceText,
    ],
  )
}

// #5485: the Settings card for the gateway-fallback preference + a live readout
// of the current routing decision and credit balance.
const inferenceGatewayCard = (model: Model): Html => {
  const readiness = modelInferenceGatewayReadiness(model)
  const enabledLabel = readiness?.enabled
    ? "gateway available"
    : "gateway not yet enabled (server-gated)"
  return card("Inference", [
    h.p([cls("card-body")], [
      "When you have no usable Claude/Codex auth, route coding-session inference through the OpenAgents gateway (pay-as-you-go credits) instead of requiring your own keys. Your own auth is always used when present.",
    ]),
    settingsToggle(
      [
        { value: "auto" as const, label: "Use credits when needed" },
        { value: "off" as const, label: "Require own auth" },
      ],
      model.gatewayInferenceFallback,
      (value) => ChangedGatewayInferenceFallback({ value }),
    ),
    inferenceRouteHint(model),
    emptyLine(
      `Status: ${enabledLabel}. The OpenAgents API key + credit ledger live server-side; this only sets your routing preference.`,
    ),
  ])
}

const settingsPane = (model: Model): Html => {
  const node = modelNode(model)
  const installReadiness = modelInstallReadiness(model)
  const schema = node?.schema ?? "—"
  return h.div(
    [],
    [
      paneTitle("Settings"),
      // #5465: the shortcut listing, read from the single source of truth
      // (nav.ts SHORTCUTS) so it can never drift from the keyboard layer.
      card("Keyboard shortcuts", [
        h.p([cls("card-body")], [
          "Press ⌘K (Ctrl-K on non-mac) any time to open the command palette — the searchable list of every destination and action.",
        ]),
        h.ul(
          [cls("shortcut-list")],
          SHORTCUTS.map((shortcut) =>
            h.li([cls("shortcut-row")], [
              h.kbd([cls("shortcut-chord")], [shortcut.chord]),
              h.span([cls("shortcut-desc")], [shortcut.description]),
              h.span([cls("shortcut-when")], [shortcut.when]),
            ]),
          ),
        ),
      ]),
      card("First-run Health", [
        h.p([cls("card-body")], [
          "Status: ",
          h.strong([], [installReadinessSummary(installReadiness)]),
        ]),
        h.p([cls("card-body")], [
          "System: ",
          h.strong([], [
            installReadiness
              ? `${installReadiness.platform}-${installReadiness.arch} · ${installReadiness.runtime}`
              : "not checked",
          ]),
        ]),
        h.ul(
          [cls("training-gates install-readiness-list")],
          installReadinessRows(installReadiness),
        ),
        installReadiness?.blockerRefs.length
          ? h.ul(
              [cls("empty-state mono install-readiness-blockers")],
              installReadiness.blockerRefs.map(blocker => h.li([], [blocker])),
            )
          : h.empty,
        h.div([cls("adapter-toggle")], [
          h.button(
            [
              cls("adapter-btn"),
              h.Type("button"),
              h.Disabled(model.installReadinessPending),
              h.OnClick(ClickedRefreshInstallReadiness()),
            ],
            [model.installReadinessPending ? "Checking..." : "Refresh"],
          ),
        ]),
      ]),
      card("Connection", [
        h.p([cls("card-body")], ["Status: ", h.strong([], [connectionSummary(node)])]),
        h.p([cls("card-body")], ["Protocol schema: ", h.code([], [schema])]),
        emptyLine(
          "The desktop app connects to the local Pylon node over loopback (auto-discovered home: .pylon-tailnet / .pylon-local).",
        ),
      ]),
      // #5472: functional defaults. The chosen adapter/lane are persisted AND
      // seed the live spawn fields, so spawn/composer/chat use them without any
      // hardcoded fallback. Editing here changes the default for the next turn.
      card("Defaults", [
        h.p([cls("card-body")], [
          "The adapter and execution lane new sessions start with. Spawn, Composer, and Chat read these — change them once here instead of per-session.",
        ]),
        h.label([cls("field-label")], ["Default adapter"]),
        settingsToggle(
          [
            { value: "codex" as const, label: SPAWN_ADAPTER_LABEL.codex },
            { value: "claude_agent" as const, label: SPAWN_ADAPTER_LABEL.claude_agent },
            { value: "apple_fm" as const, label: SPAWN_ADAPTER_LABEL.apple_fm },
          ],
          model.defaultAdapter,
          (adapter) => ChangedDefaultAdapter({ adapter }),
        ),
        h.label([cls("field-label")], ["Default execution lane"]),
        settingsToggle(
          (["auto", "local", "cloud-gcp", "cloud-shc"] as const).map((lane) => ({
            value: lane,
            label: spawnLaneLabel(lane),
          })),
          model.defaultLane,
          (lane) => ChangedDefaultLane({ lane }),
        ),
      ]),
      // #5485 (EPIC #5474): OpenAgents inference gateway as the default coding
      // inference when there is no usable own Claude/Codex auth — pay-as-you-go
      // credits instead of requiring own keys. The toggle is the user's routing
      // intent; BYO-auth always wins when present. INERT-safe: the live effect
      // only kicks in once the gateway is served server-side.
      inferenceGatewayCard(model),
      // #5472: a real Theme control (was static "read-only" copy). Persisted and
      // applied live via the `data-theme` attribute on the app shell (rootView)
      // + the [data-theme="light"] CSS overrides. Dark is the canonical default.
      card("Theme", [
        h.p([cls("card-body")], [
          "Switches the desktop between the canonical dark palette and a light palette. Applied immediately and remembered across restarts.",
        ]),
        settingsToggle(
          [
            { value: "dark" as const, label: "Dark" },
            { value: "light" as const, label: "Light" },
          ],
          model.themePreference,
          (theme) => ChangedThemePreference({ theme }),
        ),
      ]),
      // #5472: notifications. The in-app feed visibility is a real persisted
      // toggle (gates the panel below). The OS channel is honestly described as
      // always-on / not yet user-gateable from here (it fires in the Bun poll).
      card("Notifications", [
        h.p([cls("card-body")], [
          "Show the in-app notification feed in Settings: ",
          h.strong([], [model.showNotificationPanel ? "on" : "off"]),
        ]),
        settingsToggle(
          [
            { value: "on" as const, label: "Show feed" },
            { value: "off" as const, label: "Hide feed" },
          ],
          model.showNotificationPanel ? "on" : "off",
          (value) => ToggledNotificationPanel({ show: value === "on" }),
        ),
        emptyLine(
          "Desktop OS notifications fire automatically on new session state transitions (CL-30) and are not yet user-configurable here — this toggle only controls the in-app feed.",
        ),
        model.showNotificationPanel
          ? modelNotifications(model)
            ? notificationsSection(modelNotifications(model) as NotificationCenterView)
            : emptyLine("No notifications yet.")
          : h.empty,
      ]),
      card("Updates", [
        h.p(
          [cls("card-body")],
          [
            "Auto-update: BSDIFF feed (full / bsdiff / none). The desktop checks updates.openagents.com on startup. If a patch is available it applies a binary diff (bsdiff) for a smaller download; otherwise it fetches the full bundle.",
          ],
        ),
        emptyLine(
          "Update behavior is managed by the app (not yet user-configurable here): available modes are full, bsdiff, or none.",
        ),
      ]),
      card("About", [
        h.p([cls("card-body")], ["Autopilot"]),
        h.p([cls("card-body")], ["Protocol schema: ", h.code([], [node?.schema ?? "not connected"])]),
      ]),
    ],
  )
}

// ── Session-detail pane ─────────────────────────────────────────────────────────

const eventTimeline = (
  model: Model,
  events: ReadonlyArray<SessionEventRow>,
): Html => {
  if (events.length === 0) return emptyLine("No events yet.")
  const expanded = new Set(model.expandedEvents)
  return h.ul(
    [cls("session-timeline")],
    events.map((event) => {
      const isOpen = expanded.has(event.eventIndex)
      const { label, meta } = eventRowText(event, isOpen)
      const expandable = eventExpandable(event)
      const attrs: Array<Attribute<Message>> = [cls(`event-row event-${event.state}`)]
      if (expandable) {
        attrs.push(cls("event-expandable"))
        attrs.push(h.OnClick(ToggledEvent({ eventIndex: event.eventIndex })))
      }
      return h.li(attrs, [
        h.span([cls("event-detail")], [label]),
        h.span([cls("event-meta")], [meta]),
      ])
    }),
  )
}

const sessionDetailPane = (model: Model): Html => {
  const node = modelNode(model)
  const ref = model.selectedSessionRef
  const session = ref ? (node?.sessions.find((s) => s.sessionRef === ref) ?? null) : null

  const back = h.button(
    [cls("link-button"), h.Type("button"), h.OnClick(NavigatedTo({ pane: "sessions" }))],
    ["‹ sessions"],
  )

  if (!session || !ref) {
    return h.div([], [back, emptyLine("Session not found.")])
  }

  const { text: verifyText, toneClass } = verifyLineText(session)
  const stats = node?.artifacts?.[ref]
  const artText = artifactLineText(stats)
  const events = node?.events?.[ref] ?? []

  return h.div(
    [],
    [
      back,
      h.p([cls("detail-ref")], [ref]),
      // #4998: lane provenance ("running on Google GCE / SHC / local") where the
      // session recorded a non-auto lane.
      (() => {
        const provenance = sessionLaneProvenance(session.lane)
        return provenance === null
          ? h.empty
          : h.p([cls("session-lane-provenance")], [provenance])
      })(),
      h.p([cls(`verify-line ${toneClass}`)], [verifyText]),
      artText.length > 0 ? h.p([cls("artifact-line")], [artText]) : h.empty,
      sessionCancellable(session.state)
        ? h.button(
            [
              cls("cancel-button"),
              h.Type("button"),
              h.OnClick(ClickedCancelSession({ sessionRef: ref })),
            ],
            ["Cancel session"],
          )
        : h.empty,
      sessionDiffCard(events, stats?.editedFileCount, {
        fileTree: true,
        viewMode: model.diffViewMode,
        expandedFiles: model.expandedDiffFiles,
      }),
      artifactBrowserCard(stats, model.artifactBrowserOpen),
      eventTimeline(model, events),
    ],
  )
}

// ── #5355: coding composer pane ──────────────────────────────────────────────
//
// The interactive day-to-day coding loop in one foreground surface, on the
// EXISTING control protocol (session.spawn / events / cancel + approvals):
//   - objective + repo/worktree + adapter/lane → spawn the first coding turn;
//   - live streamed transcript: the polled session-event tail rendered as a
//     readable turn/diff view (the same `/events` content the node emits);
//   - inline approvals: the node's pending exactly-once decisions, approve/deny
//     in-pane (wired to the existing resolveApproval flow);
//   - reply / continue: a follow-up turn (a continuation spawn carrying prior
//     turn context — no new verb) once the active turn is terminal;
//   - cancel: session.cancel on the active turn.
//
// Reuses the shared spawn form fields, the event timeline, the approval row, and
// the verify/artifact lines so it is a thin composition over surfaces that
// already exist — not a parallel runtime.

// ── CS-A3 (#5363): structured diff card ───────────────────────────────────────
//
// Renders the SHARED `DiffReview` component (the UI port of
// apps/pylon/src/tas/diff-review.ts) over a ChangeSet derived from the session's
// event tail (parseChangeSetFromEvents). Used by both the composer's active
// session and the session-detail pane so a coding turn's file edits read as a
// per-file +/- diff, not a flat transcript row. Hidden when no file changes were
// reported (the diff would be empty / misleading).
// #5470: when called from session-detail we pass the model-driven browse
// options (file tree, side-by-side, per-file expand) + a header toggle row, so
// a coding turn's diff is browsable by file with hunks and large diffs stay
// legible. The composer keeps the simple default rendering (no `browse`).
type DiffBrowseOptions = Readonly<{
  fileTree: boolean
  viewMode: "unified" | "split"
  expandedFiles: ReadonlyArray<string>
}>

const diffBrowseControls = (viewMode: "unified" | "split"): Html =>
  h.div([cls("diff-browse-controls"), h.DataAttribute("autopilot-diff-browse-controls", "")], [
    h.button(
      [
        cls("adapter-btn"),
        h.Type("button"),
        h.DataAttribute("autopilot-diff-view-toggle", viewMode),
        h.OnClick(ToggledDiffViewMode()),
      ],
      [viewMode === "split" ? "Side-by-side ✓" : "Side-by-side"],
    ),
  ])

const sessionDiffCard = (
  events: ReadonlyArray<SessionEventRow>,
  artifactEditedFileCount: number | null | undefined,
  browse?: DiffBrowseOptions,
): Html => {
  const changeSet = parseChangeSetFromEvents(events)
  if (changeSet.files.length === 0) return h.empty
  return card("Diff", [
    ...(browse ? [diffBrowseControls(browse.viewMode)] : []),
    DiffReview({
      files: changeSet.files,
      summary: changeSet.summary,
      provenance: diffReviewProvenance(changeSet, artifactEditedFileCount),
      ...(browse
        ? {
            fileTree: browse.fileTree,
            viewMode: browse.viewMode,
            expandedFiles: browse.expandedFiles,
          }
        : {}),
    }),
    // #5470: a per-file expand strip so a reviewer can open the hunk body of the
    // files they care about (the rest stay collapsed). Only when browsing and
    // some file actually has hunk lines (otherwise there's nothing to expand).
    ...(browse && changeSet.files.some((f) => f.hunkLines && f.hunkLines.length > 0)
      ? [
          h.div(
            [cls("diff-file-toggles"), h.DataAttribute("autopilot-diff-file-toggles", "")],
            changeSet.files
              .filter((f) => f.hunkLines && f.hunkLines.length > 0)
              .map((f) =>
                h.button(
                  [
                    cls(`adapter-btn${browse.expandedFiles.includes(f.path) ? " active" : ""}`),
                    h.Type("button"),
                    h.DataAttribute("autopilot-diff-file-toggle", f.path),
                    h.OnClick(ToggledDiffFile({ path: f.path })),
                  ],
                  [`${browse.expandedFiles.includes(f.path) ? "▾" : "▸"} ${f.path}`],
                ),
              ),
          ),
        ]
      : []),
  ])
}

// ── #5470: artifact & receipt browser ─────────────────────────────────────────
//
// An expand-on-demand inspector over `session.artifact` (proof for completed,
// failure for failed). It renders the redaction-safe, ref-only detail the node
// retained — refs/digests/enums only, never a seed/token/raw path/raw secret.
// Inspection lives INSIDE session-detail (audit UX constraint): collapsed by
// default behind a toggle so the pane stays uncluttered.
const artifactBrowserCard = (
  stats: SessionArtifactStats | undefined | null,
  open: boolean,
): Html => {
  const sections = artifactBrowserSections(stats)
  if (sections.length === 0) return h.empty
  return card("Artifacts & receipts", [
    h.button(
      [
        cls("adapter-btn"),
        h.Type("button"),
        h.DataAttribute("autopilot-artifact-browser-toggle", open ? "open" : "closed"),
        h.OnClick(ToggledArtifactBrowser()),
      ],
      [open ? "Hide refs ▾" : "Inspect refs ▸"],
    ),
    open
      ? h.div(
          [cls("artifact-browser"), h.DataAttribute("autopilot-artifact-browser", "")],
          sections.map((section) =>
            h.div(
              [cls("artifact-browser-section"), h.DataAttribute("autopilot-artifact-section", section.id)],
              [
                h.p([cls("card-body")], [h.strong([], [section.title])]),
                h.ul(
                  [cls("artifact-ref-list")],
                  section.rows.map((row) =>
                    h.li([cls("artifact-ref-row"), h.DataAttribute("autopilot-artifact-ref-label", row.label)], [
                      h.span([cls("artifact-ref-label")], [`${row.label}: `]),
                      h.code([cls("artifact-ref-value"), h.Title(row.value)], [row.value]),
                    ]),
                  ),
                ),
              ],
            ),
          ),
        )
      : h.empty,
  ])
}

// The composer's live transcript: prefer the agent's transcript-worthy events
// (text / tool calls / file changes); fall back to the full timeline so a turn
// that has only lifecycle events still shows progress.
const composerTranscript = (
  model: Model,
  events: ReadonlyArray<SessionEventRow>,
): Html => {
  if (events.length === 0) return emptyLine("Waiting for the agent's first turn…")
  // Concise, markdown-rendered transcript: assistant/reasoning text as markdown,
  // tool actions as one-liners, token/lifecycle noise suppressed. Falls back to
  // the raw lifecycle timeline only when a turn has produced no readable content
  // yet (so progress is still visible).
  const concise = conciseTranscript(events)
  if (concise !== null) return concise
  const transcriptEvents = events.filter(isComposerTranscriptEvent)
  const shown = transcriptEvents.length > 0 ? transcriptEvents : events
  return eventTimeline(model, shown)
}

// CS-A1: short label for a spawn-adapter runtime option.
const SPAWN_ADAPTER_LABEL: Record<"codex" | "claude_agent" | "apple_fm", string> = {
  codex: "codex",
  claude_agent: "claude",
  apple_fm: "Apple FM",
}

// CS-A1: per-session account picker. Lists the node's codex/claude accounts
// (from the live accounts.list projection) for the selected runtime so a
// coding turn can run under a specific provider account (threaded through
// session.spawn's accountRef). Hidden for Apple FM (no per-account selection).
const composerAccountPicker = (model: Model): Html => {
  if (model.spawnAdapter === "apple_fm") return h.empty
  const node = modelNode(model)
  const accounts = (node?.accounts ?? []).filter(
    (row) => row.provider === model.spawnAdapter && row.accountRef !== null,
  )
  if (accounts.length === 0) return h.empty
  const defaultActive = model.composerAccountRef === null
  return h.div(
    [],
    [
      h.label([cls("field-label")], ["Account"]),
      h.div(
        [cls("adapter-toggle"), h.DataAttribute("autopilot-composer-account-picker", "")],
        [
          h.button(
            [
              cls(`adapter-btn${defaultActive ? " active" : ""}`),
              h.Type("button"),
              h.OnClick(SelectedComposerAccount({ accountRef: null })),
            ],
            ["Default"],
          ),
          ...accounts.map((row) =>
            h.button(
              [
                cls(
                  `adapter-btn${model.composerAccountRef === row.accountRef ? " active" : ""}`,
                ),
                h.Type("button"),
                h.DataAttribute("autopilot-composer-account-ref", row.accountRef ?? ""),
                h.OnClick(
                  SelectedComposerAccount({ accountRef: row.accountRef }),
                ),
              ],
              [accountPickerLabel(row)],
            ),
          ),
        ],
      ),
    ],
  )
}

// #5471: compact repo / worktree picker inside the composer spawn form (NOT a
// new pane — UX constraint in the issue). Two modes, both riding the existing
// session.spawn: "Worktree" points at an existing local path (worktreePath);
// "Managed" requests a Pylon-managed worktree for a GitHub repo + base ref
// (resolved to a repoRef node-side). A small hint line shows the chosen
// repo/worktree provenance / validation up front. Apple FM has no managed
// worktree (its own control verb takes only a worktree path), so it shows the
// path field alone.
const composerWorkspacePicker = (model: Model): Html => {
  const isAppleFm = model.spawnAdapter === "apple_fm"
  const mode = isAppleFm ? "worktree" : model.composerWorkspaceMode
  const modeToggle = isAppleFm
    ? h.empty
    : h.div(
        [cls("adapter-toggle"), h.DataAttribute("autopilot-composer-workspace-mode", mode)],
        (
          [
            ["worktree", "Existing worktree"],
            ["managed", "Managed worktree"],
          ] as const
        ).map(([value, label]) =>
          h.button(
            [
              cls(`adapter-btn${mode === value ? " active" : ""}`),
              h.Type("button"),
              h.OnClick(ChangedComposerWorkspaceMode({ mode: value })),
            ],
            [label],
          ),
        ),
      )

  const worktreeFields: ReadonlyArray<Html> = [
    h.label([cls("field-label")], ["Worktree path (optional)"]),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder("/Users/you/code/your-repo"),
      h.Value(model.composerRepoPath),
      h.OnInput((value: string) => ChangedComposerRepoPath({ value })),
    ]),
    h.p([cls("field-hint")], [`Runs in: ${worktreePathLabel(model.composerRepoPath)}`]),
  ]

  // For managed mode show a parse-result hint so a bad repo/ref is caught
  // before the spawn round-trips.
  const managedParsed = parseManagedWorktreeRequest({
    repo: model.composerManagedRepo,
    baseRef: model.composerManagedBaseRef,
  })
  const managedHint =
    model.composerManagedRepo.trim() === ""
      ? "Enter a GitHub owner/name to materialize a fresh isolated worktree."
      : managedParsed.ok
        ? `Will materialize: ${managedWorktreeLabel(managedParsed.request)}`
        : managedParsed.error
  const managedHintTone = managedParsed.ok || model.composerManagedRepo.trim() === ""
    ? "field-hint"
    : "field-hint field-hint-error"
  const managedFields: ReadonlyArray<Html> = [
    h.label([cls("field-label")], ["GitHub repo (owner/name)"]),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder("OpenAgentsInc/openagents"),
      h.Value(model.composerManagedRepo),
      h.OnInput((value: string) => ChangedComposerManagedRepo({ value })),
    ]),
    h.label([cls("field-label")], ["Base ref (optional)"]),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder(DEFAULT_MANAGED_BASE_REF),
      h.Value(model.composerManagedBaseRef),
      h.OnInput((value: string) => ChangedComposerManagedBaseRef({ value })),
    ]),
    h.p([cls(managedHintTone)], [managedHint]),
  ]

  return h.div(
    [cls("composer-workspace-picker")],
    [
      h.label([cls("field-label")], ["Repo / worktree"]),
      modeToggle,
      ...(mode === "managed" ? managedFields : worktreeFields),
    ],
  )
}

const composerSpawnForm = (model: Model): Html =>
  card("Start a coding session", [
    h.label([cls("field-label")], ["Runtime"]),
    h.div(
      [cls("adapter-toggle")],
      (["codex", "claude_agent", "apple_fm"] as const).map((adapter) =>
        h.button(
          [
            cls(`adapter-btn${model.spawnAdapter === adapter ? " active" : ""}`),
            h.Type("button"),
            h.OnClick(ChangedSpawnAdapter({ adapter })),
          ],
          [SPAWN_ADAPTER_LABEL[adapter]],
        ),
      ),
    ),
    composerAccountPicker(model),
    // #5485: show how this coding turn's inference is paid for (own auth vs
    // OpenAgents gateway credits) + the credit balance / low-balance hint.
    inferenceRouteHint(model),
    // Apple FM runs locally only — the execution-lane selector applies to the
    // codex/claude session.spawn path.
    model.spawnAdapter === "apple_fm"
      ? h.empty
      : h.div(
          [],
          [
            h.label([cls("field-label")], ["Execution lane"]),
            h.div(
              [cls("adapter-toggle")],
              (["auto", "local", "cloud-gcp", "cloud-shc"] as const).map((lane) =>
                h.button(
                  [
                    cls(`adapter-btn${model.spawnLane === lane ? " active" : ""}`),
                    h.Type("button"),
                    h.OnClick(ChangedSpawnLane({ lane })),
                  ],
                  [spawnLaneLabel(lane)],
                ),
              ),
            ),
          ],
        ),
    composerWorkspacePicker(model),
    h.label([cls("field-label")], ["What should the agent do?"]),
    h.textarea(
      [
        cls("text-area"),
        h.Rows(5),
        h.Placeholder("Describe the change — e.g. add a /health route and a test for it…"),
        h.Value(model.spawnObjective),
        h.OnInput((value: string) => ChangedSpawnObjective({ value })),
      ],
      [],
    ),
    h.label([cls("field-label")], ["Verify commands (optional — one per line)"]),
    h.textarea(
      [
        cls("text-area mono"),
        h.Rows(2),
        h.Placeholder("bun test\nbun run typecheck"),
        h.Value(model.spawnVerify),
        h.OnInput((value: string) => ChangedSpawnVerify({ value })),
      ],
      [],
    ),
    model.composerStatus.tone !== "idle"
      ? h.p([cls(`spawn-status spawn-${model.composerStatus.tone}`)], [
          model.composerStatus.text,
        ])
      : h.p([cls("spawn-status")], [" "]),
    h.button(
      [
        cls("primary-button"),
        h.Type("button"),
        h.Disabled(model.composerPending),
        h.OnClick(ClickedComposerSpawn()),
      ],
      [model.composerPending ? "Starting…" : "Start coding"],
    ),
  ])

const composerReplyBar = (model: Model, canReply: boolean): Html =>
  card("Reply / continue", [
    h.textarea(
      [
        cls("text-area"),
        h.Rows(3),
        h.Placeholder(
          canReply
            ? "Send a follow-up turn into this thread…"
            : "Reply unlocks when the current turn finishes…",
        ),
        h.Value(model.composerReply),
        h.OnInput((value: string) => ChangedComposerReply({ value })),
      ],
      [],
    ),
    h.div(
      [cls("composer-reply-actions")],
      [
        h.button(
          [
            cls("primary-button"),
            h.Type("button"),
            h.Disabled(model.composerPending || !canReply),
            h.OnClick(ClickedComposerReply()),
          ],
          [model.composerPending ? "Sending…" : "Send follow-up"],
        ),
        h.button(
          [
            cls("link-button"),
            h.Type("button"),
            h.OnClick(ClickedComposerNewThread()),
          ],
          ["New thread"],
        ),
      ],
    ),
  ])

const composerActiveSession = (model: Model): Html => {
  const node = modelNode(model)
  const ref = model.composerSessionRef
  const session = ref ? (node?.sessions.find((s) => s.sessionRef === ref) ?? null) : null
  const events = ref ? (node?.events?.[ref] ?? []) : []
  const stats = ref ? node?.artifacts?.[ref] : undefined
  const state = session?.state ?? null
  const canReply = composerCanReply(state)

  const header = h.div(
    [cls("composer-active-header")],
    [
      h.p([cls("detail-ref")], [ref ?? ""]),
      h.p([cls("composer-turn-summary")], [
        composerTurnSummary(state, model.composerTurns.length),
      ]),
      (() => {
        const provenance = session ? sessionLaneProvenance(session.lane) : null
        return provenance === null
          ? h.empty
          : h.p([cls("session-lane-provenance")], [provenance])
      })(),
    ],
  )

  const verifyLine = session
    ? (() => {
        const { text, toneClass } = verifyLineText(session)
        return h.p([cls(`verify-line ${toneClass}`)], [text])
      })()
    : h.empty

  const artText = artifactLineText(stats)
  const artifactLine = artText.length > 0 ? h.p([cls("artifact-line")], [artText]) : h.empty

  const cancelBtn =
    session && sessionCancellable(session.state)
      ? h.button(
          [
            cls("cancel-button"),
            h.Type("button"),
            h.OnClick(ClickedCancelSession({ sessionRef: ref ?? "" })),
          ],
          ["Cancel turn"],
        )
      : h.empty

  // Inline approvals — the node's pending exactly-once decisions, surfaced in
  // the composer so the owner approves/denies without leaving the loop.
  const approvals = pendingApprovals(model)
  const approvalsBlock =
    approvals.length === 0
      ? h.empty
      : card(`Needs you (${approvals.length})`, approvals.map(approvalRowView))

  return h.div(
    [cls("composer-active")],
    [
      header,
      verifyLine,
      artifactLine,
      cancelBtn,
      approvalsBlock,
      sessionDiffCard(events, stats?.editedFileCount),
      card("Transcript", [composerTranscript(model, events)]),
      composerReplyBar(model, canReply),
    ],
  )
}

// CS-A1: one row in the account-management table — ref, provider, home health,
// priority (with bump/lower), and a remove action. Edits the node's local
// dev.accounts config through the management RPC verbs.
const managedAccountRowView = (row: ManagedAccountRow): Html =>
  h.div(
    [
      cls("managed-account-row"),
      h.DataAttribute("autopilot-managed-account-ref", row.ref),
    ],
    [
      h.code([cls("managed-account-ref mono")], [row.ref]),
      h.span([cls("managed-account-provider")], [row.provider]),
      h.span(
        [cls(`managed-account-home ${row.homePresent ? "present" : "missing"}`)],
        [row.homePresent ? "home present" : "home missing"],
      ),
      h.span(
        [cls("managed-account-priority")],
        [row.priority === null ? "priority: —" : `priority: ${row.priority}`],
      ),
      h.div(
        [cls("managed-account-actions")],
        [
          h.button(
            [
              cls("link-button"),
              h.Type("button"),
              h.OnClick(
                ClickedBumpManagedAccountPriority({
                  ref: row.ref,
                  provider: row.provider,
                  priority: (row.priority ?? 0) - 1,
                }),
              ),
            ],
            ["▲ priority"],
          ),
          h.button(
            [
              cls("link-button"),
              h.Type("button"),
              h.OnClick(
                ClickedBumpManagedAccountPriority({
                  ref: row.ref,
                  provider: row.provider,
                  priority: (row.priority ?? 0) + 1,
                }),
              ),
            ],
            ["▼ priority"],
          ),
          h.button(
            [
              cls("cancel-button"),
              h.Type("button"),
              h.OnClick(
                ClickedRemoveManagedAccount({ ref: row.ref, provider: row.provider }),
              ),
            ],
            ["Remove"],
          ),
        ],
      ),
    ],
  )

// CS-A1: account-management surface — add / select / priority / quota over the
// node's local dev.accounts config. Turns the read-only AccountList into a
// managed registry (audit gap #2). Readiness/quota stays the live accounts.list
// projection (rendered via the shared AccountList); the managed table here owns
// add/remove/priority.
const accountManagementCard = (model: Model): Html => {
  const managed = modelManagedAccounts(model)
  const rows = managed?.accounts ?? []
  const node = modelNode(model)
  const liveAccounts = node?.accounts ?? []
  return card("Accounts", [
    h.p([cls("card-body")], [
      "Manage which provider accounts this node can run coding sessions under. Priority orders dispatch (lower runs first).",
    ]),
    model.managedAccountsStatus.tone !== "idle"
      ? h.p(
          [cls(`spawn-status spawn-${model.managedAccountsStatus.tone}`)],
          [model.managedAccountsStatus.text],
        )
      : h.empty,
    // Live readiness/quota for every discovered account (shared component).
    liveAccounts.length > 0
      ? h.div(
          [cls("managed-account-readiness")],
          [AccountList({ accounts: liveAccounts.map(toAccountSummary) })],
        )
      : h.empty,
    // Managed registry rows (editable).
    rows.length === 0
      ? emptyLine("No managed accounts yet. Add one below.")
      : h.div([cls("managed-account-list")], rows.map(managedAccountRowView)),
    // Add-account form.
    h.label([cls("field-label")], ["Add account"]),
    h.div(
      [cls("adapter-toggle")],
      (["codex", "claude_agent"] as const).map((provider) =>
        h.button(
          [
            cls(`adapter-btn${model.addAccountProvider === provider ? " active" : ""}`),
            h.Type("button"),
            h.OnClick(ChangedAddAccountProvider({ provider })),
          ],
          [provider === "claude_agent" ? "claude" : provider],
        ),
      ),
    ),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder("account ref — e.g. work, personal"),
      h.Value(model.addAccountRef),
      h.OnInput((value: string) => ChangedAddAccountRef({ value })),
    ]),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder("home path — e.g. ~/.codex-work"),
      h.Value(model.addAccountHome),
      h.OnInput((value: string) => ChangedAddAccountHome({ value })),
    ]),
    h.input([
      cls("text-input mono"),
      h.Type("text"),
      h.Placeholder("priority (optional, lower runs first)"),
      h.Value(model.addAccountPriority),
      h.OnInput((value: string) => ChangedAddAccountPriority({ value })),
    ]),
    h.div(
      [cls("composer-reply-actions")],
      [
        h.button(
          [
            cls("primary-button"),
            h.Type("button"),
            h.Disabled(model.managedAccountsPending),
            h.OnClick(ClickedAddManagedAccount()),
          ],
          [model.managedAccountsPending ? "Saving…" : "Add account"],
        ),
        h.button(
          [
            cls("link-button"),
            h.Type("button"),
            h.OnClick(ClickedRefreshManagedAccounts()),
          ],
          ["Refresh"],
        ),
      ],
    ),
  ])
}

const chatRoleLabel = (role: ChatMessage["role"]): string => {
  switch (role) {
    case "assistant":
      return "Autopilot"
    case "system":
      return "System"
    case "user":
      return "You"
  }
}

const chatStepTone = (status: ChatStep["status"]): string => {
  switch (status) {
    case "verified":
    case "completed":
      return "ready"
    case "running":
      return "waiting"
    case "blocked":
      return "blocked"
    case "pending":
      return "watch"
  }
}

const chatStepRef = (label: string, value: string | null): Html =>
  value === null || value.trim() === ""
    ? h.empty
    : h.div(
        [cls("chat-step-ref")],
        [
          h.span([cls("chat-step-ref-label")], [label]),
          h.code([cls("detail-ref mono")], [value]),
        ],
      )

const chatVerdictLabel = (verdict: ChatStep["verdict"]): string => {
  switch (verdict) {
    case "verified":
      return "Verified"
    case "rejected":
      return "Rejected"
    case "pending":
      return "Pending"
    case null:
      return ""
  }
}

// A single scoped step rendered inside the per-message "program details"
// disclosure. The Blueprint/Tassadar scaffolding (signature / scoped tool /
// module step / exact-replay / redaction / proof-replay refs) lives here, not
// inline in the conversation. The Tassadar REPLAY timeline itself is NOT
// rendered in chat — it lives on the Network home scene and the Training pane's
// Proof Replays panel; here we only show the public-safe ref.
const chatStepView = (step: ChatStep): Html =>
  h.li(
    [
      cls(`chat-step chat-step-${step.kind} chat-step-${chatStepTone(step.status)}`),
      h.DataAttribute("autopilot-chat-step-id", step.id),
    ],
    [
      h.div(
        [cls("chat-step-main")],
        [
          h.span([cls("chat-step-label")], [step.label]),
          h.span([cls("chat-step-status")], [step.status]),
        ],
      ),
      chatStepRef("signature", step.signatureRef),
      chatStepRef("tool", step.toolRef),
      chatStepRef("module", step.moduleRef),
      chatStepRef("step", step.tassadarModuleStepRef),
      chatStepRef("evidence", step.evidenceRef),
      chatStepRef("receipt", step.receiptRef),
      chatStepRef("digest", step.digestRef),
      step.verdict === null
        ? h.empty
        : h.div(
            [cls(`chat-verdict chat-verdict-${step.verdict}`)],
            [
              h.span([cls("chat-step-ref-label")], ["exact replay"]),
              h.code([cls("detail-ref mono")], [chatVerdictLabel(step.verdict)]),
            ],
          ),
      step.contentRedacted
        ? h.div(
            [cls("chat-step-ref")],
            [
              h.span([cls("chat-step-ref-label")], ["redaction"]),
              h.code([cls("detail-ref mono")], ["digests and public refs only"]),
            ],
          )
        : h.empty,
      chatStepRef("proof replay", step.proofReplayRef),
    ],
  )

// Per-message disclosure for the scoped-step / Tassadar scaffolding. Collapsed
// by default (only the small "▸ program details" toggle shows) so the chat
// pane opens to a clean conversation + composer. Expand state flows through the
// Foldkit model (`expandedChatMessages`) via `ToggledChatMessageDetails`.
const chatMessageDetails = (model: Model, message: ChatMessage): Html => {
  if (message.steps.length === 0) return h.empty
  const expanded = model.expandedChatMessages.includes(message.id)
  const stepCount = message.steps.length
  const summary = `program details · ${stepCount} ${stepCount === 1 ? "step" : "steps"}`
  return h.div(
    [cls(`chat-message-details${expanded ? " expanded" : ""}`)],
    [
      h.button(
        [
          cls("chat-message-details-toggle"),
          h.Type("button"),
          h.DataAttribute("autopilot-chat-details-toggle", message.id),
          h.OnClick(ToggledChatMessageDetails({ messageId: message.id })),
        ],
        [`${expanded ? "▾" : "▸"} ${summary}`],
      ),
      expanded
        ? h.ul(
            [cls("chat-step-list")],
            message.steps.map(step => chatStepView(step)),
          )
        : h.empty,
    ],
  )
}

const chatMessageView = (model: Model, message: ChatMessage): Html =>
  h.li(
    [
      cls(`chat-message chat-message-${message.role}`),
      h.DataAttribute("autopilot-chat-message-id", message.id),
    ],
    [
      h.div(
        [cls("chat-message-header")],
        [
          h.span([cls("chat-message-role")], [chatRoleLabel(message.role)]),
          h.span([cls("chat-message-time")], [message.timestamp]),
          message.linkedSessionRef === null
            ? h.empty
            : h.code([cls("chat-session-ref mono")], [message.linkedSessionRef]),
        ],
      ),
      h.p([cls("chat-message-body")], [message.body]),
      chatMessageDetails(model, message),
    ],
  )

// #5735/#5819: the Verse/world-scene behind chat. Default ON for launch, with a
// build kill-switch and the runtime Verse toggle for fallback/debug. When on,
// the chat thread + composer render as translucent glass over a full-bleed 3D
// pylon scene (mirrors the .training-fullscreen-scene/-overlay pattern).
// #5730 (P2.5): the static seed is now only a zero-state/pre-load fallback —
// when the flag is on AND live pylon-stats have arrived, the scene shows the
// REAL pylons (modelChatWorldScene), and with CHAT_WORLD_PAYMENTS on, real
// Bitcoin payment beams fly between them. Resolved from the shared build-flag
// reader so the view and the subscriptions agree on what is on.
const CHAT_WORLD_FLAGS = chatWorldBuildFlags()
const CHAT_WORLD_SCENE: boolean = CHAT_WORLD_FLAGS.CHAT_WORLD_SCENE
const CHAT_WORLD_PAYMENTS: boolean = CHAT_WORLD_FLAGS.CHAT_WORLD_PAYMENTS
const AGENT_CHARACTER_CREATION: boolean = agentCharacterCreationFlag()

// A calm static seed scene: one core + inert ring pylons. It gives first paint a
// spatial Verse without claiming live online/assigned pylons before the public
// Pylon snapshot arrives.
const CHAT_SCENE_RING_NODES: ReadonlyArray<PylonNetworkNode> = Array.from(
  { length: 8 },
  (_unused, index): PylonNetworkNode => {
    return {
      id: `seed-pylon-${index}`,
      label: "pylon",
      tone: "offline",
      flowing: false,
    }
  },
)

const CHAT_SCENE: PylonNetworkScene = {
  activityIntensity: 0,
  dormant: true,
  onlineNow: 0,
  sessionsOnlineNow: 0,
  sellableOnlineNow: 0,
  walletReadyNow: 0,
  assignmentReadyNow: 0,
  seen24h: 8,
  registeredTotal: 8,
  satsSettled24h: 0,
  satsSettledTotal: 0,
  trainingAssignedContributors: 0,
  trainingAcceptedContributors: 0,
  trainingProgressContributors: 0,
  nodes: CHAT_SCENE_RING_NODES,
  asOfLabel: null,
}

// #5730/#5822: build the Verse background from LIVE Pylon state plus the public
// Tassadar training projection. The static seed is inert until public data
// arrives; the training layer pins motion to public refs and keeps dense
// controls in Training Live.
const chatSceneVisualization = (model: Model): TrainingRunVisualizationOptions => {
  // Live pylons replace the seed once a non-empty snapshot has landed.
  const liveScene = liveChatWorldNetworkScene(modelChatWorldScene(model))
  const base = pylonNetworkVisualizationOptions(liveScene ?? CHAT_SCENE)
  const pylonBase = pylonBaseProjectionFor(model)
  const withTraining = withVerseTrainingLayer(base, {
    promiseGates: modelTrainingPromiseGates(model),
    trainingRuns: modelTrainingRuns(model),
  })
  const withBase = withPylonBaseLayer(withTraining, pylonBase)
  // Payment particles only when their flag is on; each is already evidence-bound.
  return CHAT_WORLD_PAYMENTS
    ? withChatWorldPaymentLayer(withBase, modelChatWorldParticles(model))
    : withBase
}

const pylonBaseProjectionFor = (model: Model): PylonBaseProjection =>
  projectPylonBase({
    chatWorldScene: modelChatWorldScene(model),
    identityChoice: modelIdentityChoiceState(model),
    onboardingStatus: modelOnboardingStatus(model),
    particles: modelChatWorldParticles(model),
    trainingOperatorReadiness: modelTrainingOperatorReadiness(model),
  })

// A small inspector chip naming the receipt the last-clicked payment beam ties
// to (evidence-bound: every beam carries a real sourceRef). Absent until a click.
const chatSceneInspector = (model: Model): Html =>
  model.chatWorldInspectedRef !== null
    ? h.div([cls("chat-scene-inspector mono")], [
        h.span([cls("chat-scene-inspector-label")], [
          model.chatWorldInspectedRef.startsWith("receipt") ? "receipt " : "refs ",
        ]),
        h.span([cls("chat-scene-inspector-ref")], [model.chatWorldInspectedRef]),
      ])
    : h.div([cls("chat-scene-inspector chat-scene-inspector-empty")], [])

const pylonBaseStatus = (model: Model): Html => {
  const base = pylonBaseProjectionFor(model)
  const detail =
    base.nextAction ??
    (base.readiness.assignmentReady
      ? "ready for Tassadar"
      : base.readiness.walletReady
        ? "wallet ready"
        : base.readiness.online
          ? "online"
          : "waiting")
  return h.div([cls(`pylon-base-status pylon-base-status-${base.status}`)], [
    h.div([cls("pylon-base-status-top")], [
      h.span([cls("pylon-base-status-label")], [base.label]),
      h.span([cls("pylon-base-status-mana mono")], [
        `${base.mana.current}/${base.mana.total}`,
      ]),
    ]),
    h.div([cls("pylon-base-status-meter")], [
      h.span([
        cls("pylon-base-status-meter-fill"),
        h.Style({ transform: `scaleX(${base.mana.ratio})` }),
      ], []),
    ]),
    h.div([cls("pylon-base-status-detail")], [
      `${detail} · ${base.settledSats} sats`,
    ]),
  ])
}

const chatSceneBackground = (model: Model): Html =>
  h.div([cls("chat-scene-background")], [
    trainingRunView<Message>(
      [cls("three-effect-chat-scene")],
      chatSceneVisualization(model),
      // Click a payment endpoint → surface its receipt ref in the inspector.
      (node) => {
        const hasDetailInspector =
          node.id.startsWith(VERSE_TRAINING_NODE_PREFIX) ||
          node.id.startsWith(PYLON_BASE_NODE_PREFIX)
        return SelectedChatWorldNode({
          id: node.id,
          label: hasDetailInspector ? node.detail : node.label,
        })
      },
    ),
    pylonBaseStatus(model),
    chatSceneInspector(model),
  ])

const characterCreationBeat = (beat: CharacterCreationBeat): Html =>
  h.li([cls(`character-creation-beat character-creation-${beat.status}`)], [
    h.span([cls("character-creation-dot")], []),
    h.div([cls("character-creation-beat-copy")], [
      h.strong([], [beat.label]),
      h.span([], [beat.message]),
    ]),
  ])

const characterCreationOverlay = (model: Model): Html => {
  const projection = projectCharacterCreationOnboarding({
    flagEnabled: AGENT_CHARACTER_CREATION,
    onboardingStatus: modelOnboardingStatus(model),
    chatWorldScene: modelChatWorldScene(model),
  })
  return projection.enabled
    ? h.div([cls("character-creation-overlay")], [
        h.div([cls("character-creation-head")], [
          h.span([cls("character-creation-kicker mono")], ["character creation"]),
          h.span([cls("character-creation-count mono")], [
            `${projection.pylonOnlineCount} pylons online`,
          ]),
        ]),
        h.div([cls("character-creation-mana")], [
          h.span([
            cls("character-creation-mana-fill"),
            h.Style({ transform: `scaleX(${projection.mana})` }),
          ], []),
        ]),
        h.ol([cls("character-creation-list")], [
          ...projection.beats.map(characterCreationBeat),
        ]),
      ])
    : h.empty
}

// The chat thread + composer, shared by both the plain and world-scene layouts.
const chatThread = (model: Model): Html =>
  h.div(
    [cls("chat-thread-shell")],
    [
      h.ul(
        [cls("chat-message-list")],
        model.chatMessages.length === 0
          ? [h.li([cls("chat-empty")], [
              emptyLine("Verse online. Talk to Tassadar or ask what your Pylon is doing."),
            ])]
          : model.chatMessages.map(message => chatMessageView(model, message)),
      ),
    ],
  )

const chatComposer = (model: Model): Html =>
  h.div(
    [cls("chat-composer")],
    [
      h.textarea(
        [
          cls("text-area chat-input"),
          h.Rows(4),
          h.Placeholder("Talk to Tassadar about the Verse, your Pylon, or the training run..."),
          h.Value(model.chatInput),
          h.OnInput((value: string) => ChangedChatInput({ value })),
        ],
        [],
      ),
      model.chatStatus.tone !== "idle"
        ? h.p([cls(`spawn-status spawn-${model.chatStatus.tone}`)], [
            model.chatStatus.text,
          ])
        : h.p([cls("spawn-status")], [" "]),
      h.div(
        [cls("chat-actions")],
        [
          h.button(
            [
              cls("primary-button"),
              h.Type("button"),
              h.Disabled(model.chatPending),
              h.OnClick(ClickedChatSubmit()),
            ],
            [model.chatPending ? "Sending..." : "Send"],
          ),
        ],
      ),
    ],
  )

// #5730 The Verse: the game-world scene renders behind chat when the build flag
// CHAT_WORLD_SCENE is on (hard kill-switch) AND the runtime toggle
// `model.verseEnabled` is true (user-facing control, default ON). Toggling the
// Verse off falls back to the byte-for-byte plain chat pane.
const verseVisible = (model: Model): boolean =>
  CHAT_WORLD_SCENE && model.verseEnabled

const chatPane = (model: Model): Html =>
  verseVisible(model)
    ? h.div([cls("chat-pane chat-pane-world")], [
        chatSceneBackground(model),
        characterCreationOverlay(model),
        h.div([cls("chat-content-overlay")], [
          paneTitle("The Verse"),
          chatThread(model),
          chatComposer(model),
        ]),
      ])
    : h.div(
        [cls("chat-pane")],
        [paneTitle("Chat"), chatThread(model), chatComposer(model)],
      )

const composerPane = (model: Model): Html => {
  const node = modelNode(model)
  const hasActive = model.composerSessionRef !== null
  return h.div(
    [cls("composer-pane")],
    [
      paneTitle("Composer"),
      node === null
        ? h.p([cls("node-status")], [
            "Connecting to your local node… Start it with `pylon dev` to code in the app.",
          ])
        : h.empty,
      hasActive ? composerActiveSession(model) : composerSpawnForm(model),
      // CS-A1: account management lives alongside the composer's spawn form so
      // the per-session picker and the managed registry are one surface.
      hasActive ? h.empty : accountManagementCard(model),
    ],
  )
}

// ── Zero-base shell (owner directive, 2026-06-19) ───────────────────────────
// The minimal default surface: a black screen with NOTHING on it except a single
// text bar at the bottom, and — once there is a response — the clean conversation
// above it (what you typed → the answer). No sidebar, no nav, no panes, no
// settings, no status chrome. The bar submits on Enter (Shift+Enter is left for
// a newline if the owner ever wants it; today it is a single-line input).
//
// The bottom-left ⌘K hotbar slot opens the command palette over the shell.
// Everything the old UI did is preserved and reachable through the palette/sidebar
// once open; it just no longer renders by default.
const shellTargetOptions: ReadonlyArray<{
  readonly target: ShellTarget
  readonly label: string
  readonly title: string
}> = [
  { target: "current", label: "Current", title: "Current shell model" },
  { target: "claude_code", label: "Claude", title: "Claude Code" },
  { target: "codex", label: "Codex", title: "Codex" },
]

const shellTargetLabel = (target: ShellTarget): string =>
  shellTargetOptions.find((option) => option.target === target)?.title ?? "Current"

const shellTargetTabs = (active: ShellTarget): Html =>
  h.div(
    [
      cls("shell-target-tabs"),
      h.Role("tablist"),
      h.AriaLabel("Shell target"),
      h.Title("Shift+Tab cycles shell target"),
    ],
    shellTargetOptions.map((option) =>
      h.button(
        [
          cls(`shell-target-tab${active === option.target ? " active" : ""}`),
          h.Type("button"),
          h.Role("tab"),
          h.Title(`${option.title} (Shift+Tab)`),
          h.OnClick(SelectedShellTarget({ target: option.target })),
        ],
        [option.label],
      ),
    ),
  )

type ShellStreamPartKind = "answer" | "reasoning" | "result" | "status" | "tokens" | "tool"

type ShellStreamPart = Readonly<{
  kind: ShellStreamPartKind
  label: string
  body: string
}>

const shellToolLabels = new Set([
  "agent",
  "apply_patch",
  "bash",
  "edit",
  "exec_command",
  "glob",
  "grep",
  "list",
  "ls",
  "notebookedit",
  "read",
  "task",
  "todowrite",
  "web_fetch",
  "webfetch",
  "write",
])

const shellStreamToolLabel = (label: string): boolean => {
  const normalized = label.trim().toLowerCase()
  return (
    shellToolLabels.has(normalized) ||
    normalized.startsWith("mcp__") ||
    normalized.includes("_") ||
    normalized.includes(".")
  )
}

const appendShellStreamPart = (
  parts: ReadonlyArray<ShellStreamPart>,
  part: ShellStreamPart,
): ReadonlyArray<ShellStreamPart> => {
  const previous = parts.at(-1)
  if (previous?.kind !== part.kind || previous.label !== part.label) {
    return [...parts, part]
  }
  return [
    ...parts.slice(0, -1),
    { ...previous, body: `${previous.body}\n${part.body}` },
  ]
}

const appendShellStreamContinuation = (
  parts: ReadonlyArray<ShellStreamPart>,
  line: string,
): ReadonlyArray<ShellStreamPart> => {
  const previous = parts.at(-1)
  if (!previous || previous.kind === "answer" || previous.kind === "tokens") {
    return appendShellStreamPart(parts, {
      kind: "answer",
      label: "answer",
      body: line.trim(),
    })
  }
  return [
    ...parts.slice(0, -1),
    { ...previous, body: `${previous.body}\n${line.replace(/^\s{2}/, "")}` },
  ]
}

const shellStreamPartFromLine = (line: string): ShellStreamPart | null => {
  const trimmed = line.trim()
  if (trimmed === "") return null
  if (/^thinking tokens:/i.test(trimmed)) {
    return { kind: "tokens", label: "usage", body: trimmed }
  }
  const thinking = /^thinking[:…]\s*(.*)$/is.exec(trimmed)
  if (thinking?.[1]) {
    return { kind: "reasoning", label: "thinking", body: thinking[1].trim() }
  }
  const result = /^result:\s*(.*)$/is.exec(trimmed)
  if (result?.[1]) {
    return { kind: "result", label: "result", body: result[1].trim() }
  }
  if (
    /^(redaction blocked|task started|task complete|turn started|turn completed|thread started)$/i.test(trimmed) ||
    /^control session mode:/i.test(trimmed)
  ) {
    return { kind: "status", label: "status", body: trimmed }
  }
  const tool = /^([A-Za-z][A-Za-z0-9_.-]{0,71}):\s*(.*)$/s.exec(trimmed)
  if (tool?.[1] && shellStreamToolLabel(tool[1])) {
    return { kind: "tool", label: tool[1], body: tool[2]?.trim() ?? "" }
  }
  return null
}

const shellStreamParts = (text: string): ReadonlyArray<ShellStreamPart> | null => {
  let sawStreamPart = false
  let parts: ReadonlyArray<ShellStreamPart> = []
  for (const line of text.split("\n")) {
    const part = shellStreamPartFromLine(line)
    if (part !== null) {
      sawStreamPart = true
      parts = appendShellStreamPart(parts, part)
      continue
    }
    if (line.trim() === "") continue
    parts = /^\s+/.test(line)
      ? appendShellStreamContinuation(parts, line)
      : appendShellStreamPart(parts, {
          kind: "answer",
          label: "answer",
          body: line.trim(),
        })
  }
  return sawStreamPart && parts.length > 0 ? parts : null
}

const shellStreamPartView = (part: ShellStreamPart): Html =>
  h.div(
    [
      cls(`shell-stream-part shell-stream-part-${part.kind}`),
      h.DataAttribute("autopilot-shell-stream-part", part.kind),
    ],
    [
      h.div([cls("shell-stream-part-label")], [part.label]),
      part.kind === "answer"
        ? h.div([cls("shell-stream-part-body shell-stream-answer")], [part.body])
        : h.div(
            [cls("shell-stream-part-body")],
            [h.pre([cls("shell-stream-pre")], [part.body])],
          ),
    ],
  )

const shellTurnBody = (turn: {
  role: string
  text: string
}): Html => {
  const parts = turn.role === "autopilot" ? shellStreamParts(turn.text) : null
  if (parts === null) return h.div([cls("shell-turn-text")], [turn.text])
  return h.div(
    [cls("shell-stream"), h.DataAttribute("autopilot-shell-stream", "")],
    parts.map((part) => shellStreamPartView(part)),
  )
}

const shellTurnView = (turn: {
  role: string
  target: ShellTarget
  text: string
}): Html =>
  h.div(
    [cls(`shell-turn shell-turn-${turn.role}`)],
    [
      h.div(
        [cls("shell-turn-role")],
        [
          turn.target === "current"
            ? turn.role
            : `${turn.role} · ${shellTargetLabel(turn.target)}`,
        ],
      ),
      shellTurnBody(turn),
    ],
  )

const shellPane = (model: Model): Html =>
  h.div(
    [cls("shell-pane")],
    [
      // The conversation (empty until the first response → truly black on launch).
      model.shellTurns.length === 0
        ? h.empty
        : h.div(
            [cls("shell-conversation")],
            model.shellTurns.map((turn) => shellTurnView(turn)),
          ),
      // The single bottom text bar.
      h.div(
        [cls("shell-bar")],
        [
          // Bottom-left hotbar; blank cells are inert, and the chat input sits
          // immediately to its right.
          hotbar(model, "inline"),
          shellTargetTabs(model.shellTarget),
          h.input([
            cls("shell-input"),
            h.Type("text"),
            h.Placeholder(""),
            h.Autofocus(true),
            // Intentionally NOT disabled while pending. A disabled <input> is
            // blurred by the browser, which drops focus from the chat box after
            // every send — the owner needs focus to stay on the box. The reducer
            // already no-ops a submit while `shellPending` (and on empty input),
            // so keeping the input enabled is safe and keeps focus where it
            // belongs (owner directive 2026-06-19).
            h.Value(model.shellInput),
            h.OnInput((value: string) => ChangedShellInput({ value })),
            // Shift+Tab cycles target without letting the browser move focus or
            // alter the input; Enter submits (without Shift).
            h.OnKeyDownPreventDefault((key, mods) =>
              key === "Tab" && mods.shiftKey
                ? Option.some(CycledShellTarget())
                : key === "Enter" && !mods.shiftKey
                ? Option.some(SubmittedShell())
                : Option.none(),
            ),
          ]),
        ],
      ),
    ],
  )

// ── Pane router + top-level view ────────────────────────────────────────────────

// ── Network home ─────────────────────────────────────────────────────────
// The desktop landing scene mirrors `/tassadar/replay/first-real-settlement`.
// Bun loads the public bundle through the typed desktop path, then the webview
// renders the same controlled replay element without depending on a browser
// cross-origin fetch during first paint.
const networkPane = (model: Model): Html => {
  const projection = modelProofReplay(model)
  const bundle = projection?.bundle ?? null
  return h.div([cls("network-page")], [
    h.div([cls("network-scene")], [
      bundle === null
        ? proofReplayLoadingPanel(
            model,
            projection,
            "network-replay-status",
          )
        : tassadarProofReplayScene(
            "desktop-tassadar-replay",
            proofReplaySceneSlug(projection, model.selectedProofReplaySlug),
            bundle,
          ),
    ]),
    publicActivityPane(model, {
      className: "network-public-activity-panel",
      maxEvents: 4,
      title: "Live Public Activity",
    }),
  ])
}

// ── AO-3/AO-4 onboarding wizard pane (#5444 / #5445) ────────────────────────

const onboardingStepTone = (status: OnboardingStepStatus): string => {
  switch (status) {
    case "done":
      return "ready"
    case "active":
      return "waiting"
    case "failed":
      return "blocked"
    case "pending":
      return "watch"
  }
}

const onboardingStepGlyph = (status: OnboardingStepStatus): string => {
  switch (status) {
    case "done":
      return "✓"
    case "active":
      return "…"
    case "failed":
      return "!"
    case "pending":
      return "·"
  }
}

const onboardingStepRow = (step: OnboardingStep): Html =>
  h.li(
    [cls(`readiness-row readiness-${onboardingStepTone(step.status)}`)],
    [
      h.span([cls("readiness-name")], [
        `${onboardingStepGlyph(step.status)} ${step.label}`,
      ]),
      h.span([cls("readiness-detail")], [step.message]),
      h.code([cls("readiness-status")], [step.status]),
    ],
  )

// AO-3: the first screen — detect existing Pylon vs create a new named identity.
// Create-new is ALWAYS offered (even when an existing Pylon is detected).
const identityChoiceCard = (model: Model): Html => {
  const state = modelIdentityChoiceState(model)
  const pending = model.identityChoicePending
  const detectedPresent = state?.detected.present ?? false
  const detectedLabel = state?.detected.shortLabel ?? null

  const useExisting = detectedPresent
    ? h.div([cls("onboarding-choice")], [
        h.p([cls("card-body")], [
          "Use your existing Pylon identity",
          detectedLabel ? ` (${detectedLabel})` : "",
          " — your wallet, payout target, and history carry over.",
        ]),
        h.button(
          [
            cls("adapter-btn"),
            h.Type("button"),
            h.Disabled(pending),
            h.OnClick(ClickedUseExistingIdentity()),
          ],
          [pending ? "Working…" : "Use existing identity"],
        ),
      ])
    : h.empty

  const createNew = h.div([cls("onboarding-choice")], [
    h.p([cls("card-body")], [
      "Create a new Autopilot identity — name it, and we set everything up from scratch.",
    ]),
    h.input([
      cls("text-input"),
      h.Type("text"),
      h.Placeholder("Name your agent (e.g. Studio Mac)"),
      h.Value(model.newIdentityName),
      h.OnInput((value: string) => ChangedNewIdentityName({ value })),
    ]),
    h.button(
      [
        cls("adapter-btn"),
        h.Type("button"),
        h.Disabled(pending || model.newIdentityName.trim().length === 0),
        h.OnClick(ClickedCreateNewIdentity()),
      ],
      [pending ? "Working…" : "Create new identity"],
    ),
  ])

  return card("Choose your identity", [
    useExisting,
    createNew,
  ])
}

const onboardingPane = (model: Model): Html => {
  const status = modelOnboardingStatus(model)
  const choice = modelIdentityChoiceState(model)
  const choiceNeeded = choice?.choiceNeeded ?? false

  const stepList =
    status === null
      ? [emptyLine("Loading onboarding status…")]
      : status.steps.map(onboardingStepRow)

  const retryRow =
    status?.hasRetryableFailure === true
      ? h.div([cls("adapter-toggle")], [
          h.button(
            [
              cls("adapter-btn"),
              h.Type("button"),
              h.Disabled(model.onboardingPending),
              h.OnClick(ClickedRetryOnboarding()),
            ],
            [model.onboardingPending ? "Retrying…" : "Retry"],
          ),
        ])
      : h.empty

  return h.div(
    [],
    [
      paneTitle("Get started"),
      // AO-3: the identity choice is the FIRST screen, ahead of the chain.
      choiceNeeded ? identityChoiceCard(model) : h.empty,
      card("Onboarding", [
        h.p([cls("card-body")], [
          "Status: ",
          h.strong([], [model.onboardingStatusLine.text]),
        ]),
        h.ul([cls("training-gates install-readiness-list")], stepList),
        retryRow,
        h.div([cls("adapter-toggle")], [
          h.button(
            [
              cls("adapter-btn"),
              h.Type("button"),
              h.Disabled(model.onboardingPending),
              h.OnClick(ClickedRefreshOnboarding()),
            ],
            [model.onboardingPending ? "Refreshing…" : "Refresh"],
          ),
        ]),
      ]),
    ],
  )
}

// HUD H3 (#5501): the body for a GIVEN pane kind (not just `model.pane`), so the
// managed pane layer can render the SAME existing pane content inside a floating
// window (audit §4.1 — reuse the content, key it by kind). `paneView` is the
// single-pane router; it delegates here for `model.pane`, and `managedPaneLayer`
// (below) delegates here per open pane.
const paneContent = (model: Model, kind: PaneId): Html => {
  switch (kind) {
    case "shell":
      return shellPane(model)
    case "network":
      return networkPane(model)
    case "onboarding":
      return onboardingPane(model)
    case "builtin-agent":
      return builtInAgentPane(model)
    case "nodes":
      return nodesPane(model)
    case "training":
      return trainingPane(model)
    case "training-fullscreen":
      return trainingFullscreenPane(model)
    case "sessions":
      return sessionsPane(model)
    case "swarm":
      return swarmPane(model)
    case "decisions":
      return decisionsPane(model)
    case "spawn":
      return spawnPane(model)
    case "composer":
      return composerPane(model)
    case "chat":
      return chatPane(model)
    case "autonomous-loop":
      // #5467: the Autonomous loop view (Supervise group). Own pane module.
      return autonomousLoopPane(model)
    case "settings":
      return settingsPane(model)
    case "session-detail":
      return sessionDetailPane(model)
  }
}

// HUD H7 (#5504): the small live status/meters HUD overlay. A non-intrusive
// corner card (the "game-HUD status layer") that renders REAL desktop state via
// the H2 three-effect kit: the node online/heartbeat as a status light, active
// sessions as a meter+count, and the wallet balance as a meter. It is fed a
// public-safe `HudStatusInput` (the honest node launch status + the node-state
// projection); the element derives the element states with the SAME pure
// projection the tests use, and shows an explicit offline/unknown state when a
// signal is absent (never a fabricated value).
//
// It is deliberately NOT shown on the black launch shell (keeping that screen
// quiet/black per the zero-base directive) nor over the immersive fullscreen
// training scene — only on the full multi-pane UI where there is state to read.
const statusHudOverlay = (model: Model): Html =>
  h.div(
    [cls("status-hud-overlay")],
    [
      statusHudView<Message>([], {
        nodeLaunchStatus: model.nodeLaunchStatus,
        node: modelNode(model),
      }),
    ],
  )

const paneView = (model: Model): Html => paneContent(model, model.pane)

// ── HUD H3: the managed pane layer (#5501) ──────────────────────────────────
// Floating, draggable + resizable windows over the current base surface. Each
// window is pane-as-data (`ManagedPane`): its `rect` positions it absolutely, its
// `z` stacks it, and its body reuses `paneContent` keyed by `kind` (the kept
// full-UI panes, rendered inside a window — audit §4.1). The title bar is the
// drag handle; the 8 edge/corner handles resize (Commander's 8-handle set, audit
// §4.6). Pointer-down captures the gesture (StartedPaneDrag) with the pointer's
// client coords; a window pointermove/up subscription (subscriptions.ts) drives
// the move + end. The layer renders ONLY when panes are open, so the black shell
// stays black by default.

// A human label for a pane kind, sourced from the nav registry where possible
// (one source of truth) with a readable fallback for leaves not in a group.
const PANE_KIND_LABELS: ReadonlyMap<PaneId, string> = (() => {
  const map = new Map<PaneId, string>()
  for (const group of NAV_GROUPS) {
    for (const dest of group.destinations) map.set(dest.pane, dest.label)
  }
  map.set("session-detail", "Session")
  map.set("shell", "Shell")
  return map
})()

const paneKindLabel = (kind: PaneId): string =>
  PANE_KIND_LABELS.get(kind) ?? kind

// One resize handle: a small grab target on an edge/corner. Pointer-down starts a
// resize gesture for THIS handle (the reducer captures the pane's start rect).
const paneResizeHandleView = (pane: ManagedPane, handle: PaneResizeHandle): Html =>
  h.div(
    [
      cls(`pane-window-resize pane-window-resize-${handle}`),
      h.OnPointerDown((_pointerType, button, _sx, _sy, _ts, clientX, clientY) =>
        button === 0
          ? Option.some(
              StartedPaneDrag({
                paneId: pane.id,
                drag: "resize",
                handle,
                pointerX: clientX,
                pointerY: clientY,
              }),
            )
          : Option.none(),
      ),
    ],
    [],
  )

const managedPaneWindow = (model: Model, pane: ManagedPane): Html =>
  h.div(
    [
      cls("pane-window"),
      // Absolute geometry straight from the pane data; z stacks focused on top.
      h.Style({
        left: `${pane.rect.x}px`,
        top: `${pane.rect.y}px`,
        width: `${pane.rect.w}px`,
        height: `${pane.rect.h}px`,
        "z-index": String(100 + pane.z),
      }),
      // Clicking anywhere in the window focuses (brings to front).
      h.OnMouseDown(FocusedManagedPane({ paneId: pane.id })),
    ],
    [
      // Title bar — the move handle. Pointer-down starts a move gesture.
      h.div(
        [
          cls("pane-window-titlebar"),
          h.OnPointerDown((_pointerType, button, _sx, _sy, _ts, clientX, clientY) =>
            button === 0
              ? Option.some(
                  StartedPaneDrag({
                    paneId: pane.id,
                    drag: "move",
                    handle: null,
                    pointerX: clientX,
                    pointerY: clientY,
                  }),
                )
              : Option.none(),
          ),
        ],
        [
          h.span([cls("pane-window-title")], [paneKindLabel(pane.kind)]),
          h.button(
            [
              cls("pane-window-close"),
              h.Type("button"),
              h.Title("Close pane"),
              h.OnClick(ClosedManagedPane({ paneId: pane.id })),
            ],
            ["×"],
          ),
        ],
      ),
      // The pane body: the SAME existing content, rendered inside the window.
      h.div([cls("pane-window-body")], [paneContent(model, pane.kind)]),
      // The 8 resize handles.
      ...PANE_RESIZE_HANDLES.map((handle) => paneResizeHandleView(pane, handle)),
    ],
  )

// The layer overlay. Renders nothing when no panes are open (so the shell stays
// truly black). Panes are rendered in id order; z-index (from the data) does the
// real stacking, so DOM order does not matter.
const managedPaneLayer = (model: Model): Html => {
  const layer: PaneLayer = modelPaneLayer(model)
  if (layer.panes.length === 0) return h.empty
  return h.div(
    [cls("pane-layer"), h.AriaLabel("Open panes")],
    layer.panes.map((pane) => managedPaneWindow(model, pane)),
  )
}

const rootView = (model: Model): Html => {
  // #5472: the live theme attribute. Both app-shell roots carry `data-theme` so
  // the (central) CSS can restyle for light mode; `dark` matches the hard-coded
  // dark palette so it's a visual no-op, and switching to `light` takes effect
  // immediately (no reload). See index.html `[data-theme="light"]` overrides.
  const themeData = h.DataAttribute("theme", themeAttr(model.themePreference))
  // ZERO-BASE SHELL (owner directive): the DEFAULT surface is a black screen
  // with nothing on it except the bottom text bar (and the conversation once
  // there is a response). No sidebar, no nav, no panes — just the shell. The
  // command palette (#5464) still overlays it so Cmd-K opens the hidden full UI.
  if (model.pane === "shell") {
    return h.div(
      [cls("app-shell app-shell-shell"), themeData],
      [shellPane(model), managedPaneLayer(model), commandPalette(model)],
    )
  }
  // The network home remains immersive: fullscreen replay scene, no sidebar
  // chrome, plus the public activity strip from #5428. The command palette
  // (#5464) still overlays it so Cmd-K works everywhere; the hotbar (#5499)
  // rides along as the bottom command bar.
  if (model.pane === "network") {
    return h.div(
      [cls("app-shell app-shell-network"), themeData],
      [networkPane(model), managedPaneLayer(model), hotbar(model), commandPalette(model)],
    )
  }
  // #5820: the Verse is the default product surface, so keep coding/session/
  // repo/worktree/cloud controls out of first paint. Advanced work remains one
  // Cmd-K away, and navigating to any non-Verse pane restores the full sidebar.
  if (model.pane === "chat" && verseVisible(model)) {
    return h.div(
      [cls("app-shell app-shell-verse"), themeData],
      [
        h.button(
          [
            cls("shell-return verse-advanced"),
            h.Type("button"),
            h.Title("Open advanced commands (Cmd-K)"),
            h.AriaLabel("Open advanced commands"),
            h.OnClick(OpenedCommandPalette()),
          ],
          ["Advanced"],
        ),
        chatPane(model),
        managedPaneLayer(model),
        hotbar(model),
        commandPalette(model),
      ],
    )
  }
  const fullscreenTraining = model.pane === "training-fullscreen"
  return h.div(
    [cls("app-shell"), themeData],
    [
      // Always-available fallback/debug shell. It stays reachable for advanced
      // work, but the Verse is now home, so the visible first-paint affordance
      // avoids making "shell" the product story.
      h.button(
        [
          cls("shell-return"),
          h.Type("button"),
          h.Title("Open fallback shell (Esc)"),
          h.AriaLabel("Open fallback shell"),
          h.OnClick(ClosedPanes()),
        ],
        ["Advanced"],
      ),
      sidebar(model),
      h.main(
        [
          cls(
            fullscreenTraining
              ? "content training-fullscreen-content"
              : "content",
          ),
        ],
        [
          h.div(
            [
              cls(
                fullscreenTraining
                  ? "pane training-fullscreen-pane"
                  : "pane",
              ),
            ],
            [paneView(model)],
          ),
        ],
      ),
      // HUD H1 (#5499): the same blank bottom-left command strip across the full
      // UI. Hidden in immersive training fullscreen so it does not occlude the scene.
      fullscreenTraining ? h.empty : hotbar(model),
      // HUD H7 (#5504): the live status/meters overlay, top-right corner. Hidden
      // in fullscreen training (same anti-occlusion rule as the hotbar).
      fullscreenTraining ? h.empty : statusHudOverlay(model),
      // HUD H3 (#5501): the managed pane layer also floats over the full UI (a
      // pane opened from the hotbar/palette appears as a window above the
      // sidebar+content), hidden in immersive training fullscreen.
      fullscreenTraining ? h.empty : managedPaneLayer(model),
      commandPalette(model),
    ],
  )
}

// Foldkit's element constructors strip `null` children (`Predicate.isNotNull`)
// but NOT `undefined` or `false`. An `undefined`/`false` child reaches
// `dedupeSharedVNodes`, which then does `child.children` and throws
// "undefined is not an object". `h.empty` (= null) is safe, but any helper or
// branch that yields `undefined`/`false` as a child would crash the whole view
// (blank screen). This pass drops those defensively before the tree hits the
// runtime — bulletproofing the entire view against that crash class.
export const sanitizeTree = (node: unknown): unknown => {
  if (node === null || typeof node !== "object") return node
  const vnode = node as { children?: unknown }
  const children = vnode.children
  if (Array.isArray(children)) {
    const next: unknown[] = []
    for (const child of children) {
      if (child === null || child === undefined || child === false) continue
      next.push(typeof child === "object" ? sanitizeTree(child) : child)
    }
    vnode.children = next
  }
  return node
}

// Foldkit's runtime renders `view(model).body` — `view` MUST return a
// `Document` ({ title, body }), not a bare `Html`. Returning an `Html` left
// `.body` undefined, so nothing ever mounted (blank screen). The body is the
// sanitized app shell.
export const view = (model: Model): Document => ({
  title: "Autopilot",
  body: sanitizeTree(rootView(model)) as Html,
})
