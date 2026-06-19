// CL-53: the Foldkit update function — the single pure reducer over the Model.
//
// Mirrors the web app idiom (apps/openagents.com/apps/web/src/update.ts): a
// Match.tags-style exhaustive switch returning [Model, Command[]]. RPC effects are
// expressed as Commands (commands.ts); validation (validateIntentDraft /
// validateSpawnRequest) runs here before a command is dispatched, so the form
// state and feedback stay pure functions of the Model.

import { validateIntentDraft, validateSpawnRequest } from "@openagentsinc/autopilot-control-protocol"
import { Command } from "foldkit"

import {
  ActivateTrainingWindow,
  AddManagedAccount,
  AdmitTrainingRealGradientEvidence,
  BuildTrainingEvidencePacket,
  CancelSession,
  ClaimTrainingWindowLease,
  ChooseIdentity,
  DeployCloud,
  LoadAppleFmReadiness,
  LoadBuiltInAgentReadiness,
  LoadIdentityChoiceState,
  LoadInstallReadiness,
  LoadManagedAccounts,
  LoadOnboardingStatus,
  LoadPromiseSurfacingReadiness,
  LoadPublicActivityTimeline,
  LoadProofReplayBundle,
  LoadTrainingDashboard,
  LoadTrainingEvidencePacketSummary,
  LoadTrainingOperatorReadiness,
  LoadTrainingPromiseGates,
  LoadTrainingRuns,
  PlanTrainingRunWindow,
  QueueTrainingCloseout,
  QueueTrainingLaunch,
  PersistPreferences,
  ReconcileTrainingWindow,
  RemoveManagedAccount,
  RequestTrainingBootstrapGrant,
  ResolveApproval,
  SetCoordinatorPaused,
  SetManagedAccountPriority,
  SpawnAppleFmComposerTurn,
  SpawnChatTurn,
  SpawnComposerTurn,
  StartAppleFmSession,
  StartBuiltInAgent,
  SurfacePromiseGap,
  SpawnSession,
  SubmitIntent,
} from "./commands"
import {
  buildComposerContinuationObjective,
  parseVerifyLines,
} from "./helpers"
import {
  ClickedChatSubmit,
  ClickedComposerReply,
  ClickedComposerSpawn,
  ClickedCoordinatorToggle,
  ClickedSubmitIntent,
  ClosedCommandPalette,
  MovedCommandPaletteSelection,
  NavigatedTo,
  NavigatedToGroup,
  OpenedCommandPalette,
  RanPaletteCommand,
  type Message,
} from "./message"
import { interpretKey } from "./keyboard"
import {
  filterPaletteCommands,
  groupById,
  paletteCommands,
  type PaletteCommand,
} from "./nav"
import {
  BLUEPRINT_CHAT_SIGNATURE_REF,
  BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF,
  BLUEPRINT_CHAT_REPLAY_TOOL_REF,
  BLUEPRINT_CHAT_TASSADAR_DIGEST_REF,
  Model,
  blueprintChatScopedSteps,
  type PaneId,
  type ProofReplayCommandRequest,
} from "./model"
import type { DesktopProofReplayProjection } from "../shared/proof-replays"
import { validatePromiseSurfacingInput } from "../shared/promise-surfacing"
import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
  ChooseIdentityResponse,
  IdentityChoiceStateResponse,
  InstallReadinessResponse,
  OnboardingStatusResponse,
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

type Result = readonly [Model, ReadonlyArray<Command.Command<Message>>]

const noCommands: ReadonlyArray<Command.Command<Message>> = []

// #5472: build the PersistPreferences command from the (already-updated) model,
// so the four preference handlers can't drift on which fields are saved.
const persistPreferencesFor = (model: Model): Command.Command<Message> =>
  PersistPreferences({
    theme: model.themePreference,
    defaultAdapter: model.defaultAdapter,
    defaultLane: model.defaultLane,
    showNotificationPanel: model.showNotificationPanel,
  })

const proofReplayCommandRequestForModel = (
  model: Model,
): ProofReplayCommandRequest =>
  model.selectedProofReplayMode === "generated"
    ? {
        actorRef: model.generatedProofReplayActorRef,
        from: model.generatedProofReplayFrom,
        kind: model.generatedProofReplayKind,
        limit: model.generatedProofReplayLimit,
        mode: "generated",
        pairRef: model.generatedProofReplayPairRef,
        runRef: model.generatedProofReplayRunRef,
        since: model.generatedProofReplaySince,
        source: model.generatedProofReplaySource,
        to: model.generatedProofReplayTo,
        windowRef: model.generatedProofReplayWindowRef,
      }
    : { mode: "catalog", slug: model.selectedProofReplaySlug }

const loadTrainingProjectionCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> => [
  LoadTrainingRuns(),
  LoadTrainingDashboard(),
  LoadTrainingPromiseGates(),
  LoadTrainingOperatorReadiness(),
  LoadTrainingEvidencePacketSummary(),
  LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(model) }),
  LoadPublicActivityTimeline(),
]

const trainingBootstrapShouldRefresh = (
  projection: TrainingBootstrapGrantResponse,
): boolean =>
  projection.reason === "granted" ||
  projection.reason === "queued" ||
  projection.reason === "refused"

const isTrainingPane = (pane: PaneId): boolean =>
  pane === "training" || pane === "training-fullscreen"

const isNetworkPane = (pane: PaneId): boolean => pane === "network"
const isBuiltInAgentPane = (pane: PaneId): boolean => pane === "builtin-agent"
const isSettingsPane = (pane: PaneId): boolean => pane === "settings"
// AO-4 (#5445): the onboarding wizard pane refreshes the identity-choice state +
// the live onboarding chain status whenever it is opened.
const isOnboardingPane = (pane: PaneId): boolean => pane === "onboarding"
// CS-A1: the composer pane hosts the per-session account picker, so opening it
// (and the Spawn pane / Settings, which also surface accounts) refreshes the
// managed-account registry from the node's local config.
const isAccountManagingPane = (pane: PaneId): boolean =>
  pane === "composer" || pane === "spawn" || pane === "settings"

const onboardingCompletionPane = (
  currentPane: PaneId,
  projection: OnboardingStatusResponse,
): PaneId =>
  projection.complete && currentPane === "onboarding" ? "chat" : currentPane

const eventCountLabel = (count: number): string =>
  `${count} ${count === 1 ? "event" : "events"}`

const sourceWarningCountLabel = (count: number): string =>
  `${count} source ${count === 1 ? "warning" : "warnings"}`

const plannedRunFirstObservedAt = (
  model: Model,
  projection: TrainingRunsResponse,
): string | null => {
  if (model.trainingPlanFirstObservedAt !== null) {
    return model.trainingPlanFirstObservedAt
  }
  if (!projection.ok) return null

  const plan = model.trainingPlan as TrainingPlanResponse | null
  const runRef = plan?.trainingRunRef ?? null
  if (runRef === null || runRef.trim() === "") return null

  const observed =
    projection.runs.some(run => run.trainingRunRef === runRef) ||
    projection.summaries.some(summary => summary.run.trainingRunRef === runRef)

  return observed ? projection.fetchedAt : null
}

const chatMessageId = (prefix: string): string =>
  `${prefix}.${Date.now().toString(36)}`

const chatTimestamp = (): string => new Date().toISOString()

const buildBlueprintChatObjective = (prompt: string): string =>
  [
    "Run one Blueprint chat-program turn for Autopilot.",
    `Selected signature: ${BLUEPRINT_CHAT_SIGNATURE_REF}`,
    [
      "Use the scoped tool menu only.",
      "Direct writes, deploys, email, and spend are denied unless returned as evidence for operator approval.",
    ].join(" "),
    [
      "For any Tassadar module step, report only public refs, the exact-replay verdict,",
      `and digest ${BLUEPRINT_CHAT_TASSADAR_DIGEST_REF}. Do not include raw traces.`,
    ].join(" "),
    [
      `For proof replay modules, use signature ${BLUEPRINT_CHAT_REPLAY_SIGNATURE_REF}`,
      `and tool ${BLUEPRINT_CHAT_REPLAY_TOOL_REF}; return public bundle refs only.`,
    ].join(" "),
    "",
    "User turn:",
    prompt,
  ].join("\n")

// #5464: the filtered palette list for the current query (also drives which
// command Enter runs and how the selection index clamps).
const paletteMatchesForModel = (model: Model): ReadonlyArray<PaletteCommand> =>
  filterPaletteCommands(model.commandPaletteQuery).map((match) => match.command)

const clampPaletteIndex = (index: number, length: number): number => {
  if (length <= 0) return 0
  if (index < 0) return 0
  if (index >= length) return length - 1
  return index
}

// Map a registry PaletteCommand to the EXISTING Message it dispatches (audit
// §5.2/§5.3 — no new control verb). `navigate` → NavigatedTo; `action` → the
// message named by `messageTag` (currently NavigatedTo / ClickedSubmitIntent /
// ClickedCoordinatorToggle). Unknown tags resolve to null (defensive no-op).
const messageForPaletteCommand = (command: PaletteCommand): Message | null => {
  if (command.kind === "navigate") return NavigatedTo({ pane: command.pane })
  switch (command.messageTag) {
    case "NavigatedTo": {
      const pane = (command.args?.pane ?? null) as PaneId | null
      return pane ? NavigatedTo({ pane }) : null
    }
    case "ClickedSubmitIntent":
      return ClickedSubmitIntent()
    case "ClickedCoordinatorToggle":
      return ClickedCoordinatorToggle({ paused: Boolean(command.args?.paused) })
    default:
      return null
  }
}

// #5465: the submit message for the current Chat/Composer turn. Composer's first
// turn spawns (ClickedComposerSpawn); subsequent turns continue the thread
// (ClickedComposerReply). Chat always uses ClickedChatSubmit.
const submitTurnMessage = (model: Model, pane: PaneId): Message | null => {
  if (pane === "chat") return ClickedChatSubmit()
  if (pane === "composer") {
    return model.composerSessionRef === null
      ? ClickedComposerSpawn()
      : ClickedComposerReply()
  }
  return null
}

export const update = (model: Model, message: Message): Result => {
  switch (message._tag) {
    // ── Inbound projections ────────────────────────────────────────────────
    case "GotNodeState":
      return [Model.make({ ...model, node: message.node }), noCommands]
    case "GotPylonStats":
      return [Model.make({ ...model, pylonStats: message.stats }), noCommands]
    case "GotNotifications":
      return [Model.make({ ...model, notifications: message.view }), noCommands]
    case "GotNodeLaunchStatus":
      return [
        Model.make({ ...model, nodeLaunchStatus: message.status }),
        // AO-4 (#5445): a node lifecycle transition also refreshes the live
        // onboarding chain when the wizard is open, so steps move in real time.
        isOnboardingPane(model.pane)
          ? [LoadInstallReadiness(), LoadOnboardingStatus()]
          : [LoadInstallReadiness()],
      ]

    // ── Navigation ─────────────────────────────────────────────────────────
    case "NavigatedTo":
      return [
        Model.make({
          ...model,
          pane: message.pane,
          expandedEvents: [],
          ...(isBuiltInAgentPane(message.pane)
            ? {
                builtInAgentStatus: {
                  text: "checking OpenAgents compute...",
                  tone: "info" as const,
                },
                appleFmPending: true,
                appleFmStatus: {
                  text: "checking local Apple FM...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isSettingsPane(message.pane)
            ? {
                installReadinessPending: true,
                installReadinessStatus: {
                  text: "checking first-run health...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isTrainingPane(message.pane)
            ? {
                publicActivityTimelinePending: true,
                publicActivityTimelineStatus: {
                  text: "loading public activity...",
                  tone: "info" as const,
                },
                trainingRunsPending: true,
                trainingRunsStatus: {
                  text: "loading Worker projection...",
                  tone: "info" as const,
                },
                trainingDashboardPending: true,
                trainingDashboardStatus: {
                  text: "loading public dashboards...",
                  tone: "info" as const,
                },
                trainingPromiseGatesPending: true,
                trainingPromiseGatesStatus: {
                  text: "loading promise gates...",
                  tone: "info" as const,
                },
                trainingOperatorReadinessPending: true,
                trainingOperatorReadinessStatus: {
                  text: "checking operator readiness...",
                  tone: "info" as const,
                },
                trainingEvidencePacketSummaryPending: true,
                trainingEvidencePacketSummaryStatus: {
                  text: "checking evidence packet...",
                  tone: "info" as const,
                },
                proofReplayPending: true,
                proofReplayStatus: {
                  text: "loading public replay bundle...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isNetworkPane(message.pane)
            ? {
                publicActivityTimelinePending: true,
                publicActivityTimelineStatus: {
                  text: "loading public activity...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isOnboardingPane(message.pane)
            ? {
                onboardingPending: true,
                identityChoicePending: true,
                onboardingStatusLine: {
                  text: "loading onboarding status...",
                  tone: "info" as const,
                },
              }
            : {}),
          ...(isAccountManagingPane(message.pane)
            ? {
                managedAccountsPending: true,
                managedAccountsStatus: {
                  text: "loading accounts...",
                  tone: "info" as const,
                },
              }
            : {}),
        }),
        [
          ...(isTrainingPane(message.pane)
            ? loadTrainingProjectionCommands(model)
            : isNetworkPane(message.pane)
              ? [LoadPublicActivityTimeline()]
              : isBuiltInAgentPane(message.pane)
              ? [LoadBuiltInAgentReadiness(), LoadAppleFmReadiness(), LoadPromiseSurfacingReadiness()]
              : isSettingsPane(message.pane)
                ? [LoadInstallReadiness()]
                : isOnboardingPane(message.pane)
                  ? [LoadIdentityChoiceState(), LoadOnboardingStatus()]
                  : noCommands),
          ...(isAccountManagingPane(message.pane) ? [LoadManagedAccounts()] : noCommands),
        ],
      ]

    // #5463: a group jump resolves to the group's default pane, then reuses the
    // full NavigatedTo handler above (so per-pane projection loads still fire).
    case "NavigatedToGroup": {
      const group = groupById(message.group)
      if (!group) return [model, noCommands]
      return update(model, NavigatedTo({ pane: group.defaultPane }))
    }

    // ── Command palette (#5464) ─────────────────────────────────────────────
    case "OpenedCommandPalette":
      return [
        Model.make({
          ...model,
          commandPaletteOpen: true,
          commandPaletteQuery: "",
          commandPaletteIndex: 0,
        }),
        noCommands,
      ]
    case "ClosedCommandPalette":
      return [Model.make({ ...model, commandPaletteOpen: false }), noCommands]
    case "ChangedCommandPaletteQuery":
      // A new query re-filters the list, so reset the highlight to the top.
      return [
        Model.make({
          ...model,
          commandPaletteQuery: message.value,
          commandPaletteIndex: 0,
        }),
        noCommands,
      ]
    case "MovedCommandPaletteSelection": {
      const length = paletteMatchesForModel(model).length
      return [
        Model.make({
          ...model,
          commandPaletteIndex: clampPaletteIndex(
            model.commandPaletteIndex + message.delta,
            length,
          ),
        }),
        noCommands,
      ]
    }
    case "RanPaletteCommand": {
      // Resolve the command: by explicit id (a click) or the highlighted row
      // (Enter). Close the palette, then re-dispatch the command's existing
      // Message so all real handlers/effects run unchanged.
      const command =
        message.commandId === null
          ? (paletteMatchesForModel(model)[model.commandPaletteIndex] ?? null)
          : (paletteCommands.find((c) => c.id === message.commandId) ?? null)
      const closed = Model.make({ ...model, commandPaletteOpen: false })
      if (!command) return [closed, noCommands]
      const next = messageForPaletteCommand(command)
      return next ? update(closed, next) : [closed, noCommands]
    }

    // ── Keyboard layer (#5465) ──────────────────────────────────────────────
    // PressedKey is interpreted purely (keyboard.ts) then re-dispatched as an
    // existing Message, so every shortcut reuses a real handler (no no-ops).
    case "PressedKey": {
      const intent = interpretKey(model, {
        key: message.key,
        meta: message.meta,
        ctrl: message.ctrl,
        shift: message.shift,
        inEditable: message.inEditable,
      })
      switch (intent.kind) {
        case "none":
          return [model, noCommands]
        case "open-palette":
          return update(model, OpenedCommandPalette())
        case "close-palette":
          return update(model, ClosedCommandPalette())
        case "palette-move":
          return update(model, MovedCommandPaletteSelection({ delta: intent.delta }))
        case "palette-run":
          return update(model, RanPaletteCommand({ commandId: null }))
        case "navigate-group":
          return update(model, NavigatedToGroup({ group: intent.group }))
        case "navigate-pane":
          return update(model, NavigatedTo({ pane: intent.pane }))
        case "submit-turn": {
          const submit = submitTurnMessage(model, intent.pane)
          return submit ? update(model, submit) : [model, noCommands]
        }
      }
    }

    case "SelectedSession":
      return [
        Model.make({
          ...model,
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "ChangedSessionFilter":
      return [Model.make({ ...model, sessionFilter: message.filter }), noCommands]
    case "ToggledEvent": {
      const set = new Set(model.expandedEvents)
      if (set.has(message.eventIndex)) set.delete(message.eventIndex)
      else set.add(message.eventIndex)
      return [Model.make({ ...model, expandedEvents: [...set] }), noCommands]
    }

    // ── Coordinator toggle ───────────────────────────────────────────────────
    case "ClickedCoordinatorToggle":
      return [model, [SetCoordinatorPaused({ paused: message.paused })]]
    case "SettledCoordinatorToggle":
    case "FailedCoordinatorToggle":
      // The next node-state poll carries the authoritative paused flag.
      return [model, noCommands]

    // ── Approvals ──────────────────────────────────────────────────────────────
    case "ClickedResolveApproval":
      // Optimistically hide the row; the command confirms with the node.
      return [
        Model.make({
          ...model,
          resolvedApprovals: [...model.resolvedApprovals, message.approvalRef],
        }),
        [
          ResolveApproval({
            approvalRef: message.approvalRef,
            decision: message.decision,
          }),
        ],
      ]
    case "SettledResolveApproval": {
      // If the node did not accept it (neither applied nor duplicate), un-hide
      // the row so the next poll shows it again.
      if (message.ok) return [model, noCommands]
      return [
        Model.make({
          ...model,
          resolvedApprovals: model.resolvedApprovals.filter(
            (ref) => ref !== message.approvalRef,
          ),
        }),
        noCommands,
      ]
    }

    // ── Deploy ───────────────────────────────────────────────────────────────
    case "ClickedDeploy":
      return [
        Model.make({
          ...model,
          deployFeedback: { state: "queued", text: "deploying…" },
        }),
        [DeployCloud()],
      ]
    case "SucceededDeploy":
      return [
        Model.make({
          ...model,
          deployFeedback: { state: message.state, text: message.text },
        }),
        noCommands,
      ]

    // ── Ask Autopilot ──────────────────────────────────────────────────────────
    case "ChangedAskTitle":
      return [Model.make({ ...model, askTitle: message.value }), noCommands]
    case "ChangedAskBody":
      return [Model.make({ ...model, askBody: message.value }), noCommands]
    case "ClickedSubmitIntent": {
      const validation = validateIntentDraft({
        title: model.askTitle,
        body: model.askBody,
      })
      if (!validation.ok) {
        return [
          Model.make({
            ...model,
            askStatus: {
              text: `error: ${validation.errors[0] ?? "invalid input"}`,
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          askPending: true,
          askStatus: { text: "sending…", tone: "info" },
        }),
        [SubmitIntent({ title: validation.title, body: validation.body })],
      ]
    }
    case "SettledSubmitIntent":
      return [
        Model.make({
          ...model,
          askPending: false,
          askStatus: { text: message.text, tone: message.ok ? "success" : "error" },
          ...(message.ok ? { askTitle: "", askBody: "" } : {}),
        }),
        noCommands,
      ]

    // ── Built-in no-user-key agent (#5063) ───────────────────────────────────
    case "ClickedRefreshBuiltInAgent":
      return [
        Model.make({
          ...model,
          builtInAgentStatus: {
            text: "checking OpenAgents compute...",
            tone: "info",
          },
        }),
        [LoadBuiltInAgentReadiness()],
      ]
    case "GotBuiltInAgentReadiness": {
      const projection = message.projection as BuiltInAgentReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        Model.make({
          ...model,
          builtInAgentReadiness: projection,
          builtInAgentStatus: projection.ok
            ? {
                text: `ready · ${projection.meteringLabel}`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  `${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "SelectedAgentMode":
      return [
        Model.make({
          ...model,
          agentMode: message.mode,
        }),
        noCommands,
      ]
    case "ClickedRefreshAppleFm":
      return [
        Model.make({
          ...model,
          appleFmPending: true,
          appleFmStatus: {
            text: "checking local Apple FM...",
            tone: "info",
          },
        }),
        [LoadAppleFmReadiness()],
      ]
    case "GotAppleFmReadiness": {
      const projection = message.projection as AppleFmReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        Model.make({
          ...model,
          appleFmReadiness: projection,
          appleFmPending: false,
          appleFmStatus: projection.ok
            ? {
                text: `ready · ${projection.model}`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  projection.message ??
                  `${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedStartAppleFm":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          appleFmPending: true,
          appleFmStatus: {
            text: "starting local Apple FM session...",
            tone: "info",
          },
        }),
        [StartAppleFmSession()],
      ]
    case "SucceededAppleFmSession":
      return [
        Model.make({
          ...model,
          appleFmPending: false,
          appleFmStatus: {
            text: `local session online · ${message.sessionRef}`,
            tone: "success",
          },
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedAppleFmSession":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          appleFmPending: false,
          appleFmStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]
    case "ClickedStartBuiltInAgent":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          builtInAgentPending: true,
          builtInAgentStatus: {
            text: "starting hosted agent...",
            tone: "info",
          },
        }),
        [StartBuiltInAgent()],
      ]
    case "SucceededBuiltInAgent":
      return [
        Model.make({
          ...model,
          builtInAgentPending: false,
          builtInAgentStatus: {
            text: `online · ${message.sessionRef}`,
            tone: "success",
          },
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedBuiltInAgent":
      return [
        Model.make({
          ...model,
          pane: "builtin-agent",
          builtInAgentPending: false,
          builtInAgentStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── First-run install/runtime readiness (#5064) ─────────────────────────
    case "ClickedRefreshInstallReadiness":
      return [
        Model.make({
          ...model,
          installReadinessPending: true,
          installReadinessStatus: {
            text: "checking first-run health...",
            tone: "info",
          },
        }),
        [LoadInstallReadiness()],
      ]
    case "GotInstallReadiness": {
      const projection = message.projection as InstallReadinessResponse
      const blockerCount = projection.blockerRefs.length
      return [
        Model.make({
          ...model,
          installReadiness: projection,
          installReadinessPending: false,
          installReadinessStatus: projection.ok
            ? {
                text: `${projection.highestRoiAction} · ready`,
                tone: "success",
              }
            : {
                text: `${projection.highestRoiAction} · ${blockerCount} blocker${blockerCount === 1 ? "" : "s"}`,
                tone: "info",
              },
        }),
        noCommands,
      ]
    }

    // ── AO-3/AO-4 first-run onboarding wizard (#5444 / #5445) ───────────────
    case "ClickedRefreshOnboarding":
      return [
        Model.make({
          ...model,
          onboardingPending: true,
          onboardingStatusLine: {
            text: "refreshing onboarding status...",
            tone: "info",
          },
        }),
        [LoadIdentityChoiceState(), LoadOnboardingStatus()],
      ]
    // AO-4: retry a failed step — re-loads the chain. The Bun supervisor keeps
    // converging (restart-on-crash + offline-tolerant registration), so a retry
    // is "re-read the real state now" rather than a dead/blank screen.
    case "ClickedRetryOnboarding":
      return [
        Model.make({
          ...model,
          onboardingPending: true,
          onboardingStatusLine: { text: "retrying...", tone: "info" },
        }),
        [LoadOnboardingStatus()],
      ]
    case "GotOnboardingStatus": {
      const projection = message.projection as OnboardingStatusResponse
      return [
        Model.make({
          ...model,
          pane: onboardingCompletionPane(model.pane, projection),
          onboardingStatus: projection,
          onboardingPending: false,
          onboardingStatusLine: projection.complete
            ? { text: "earning — onboarding complete", tone: "success" }
            : projection.ok === false
              ? { text: "status refresh needs a retry", tone: "error" }
            : projection.hasRetryableFailure
              ? { text: "a step needs a retry", tone: "error" }
              : {
                  text: `current step: ${projection.currentStepId ?? "—"}`,
                  tone: "info",
                },
        }),
        noCommands,
      ]
    }
    case "GotIdentityChoiceState":
      return [
        Model.make({
          ...model,
          identityChoiceState: message.state as IdentityChoiceStateResponse,
          identityChoicePending: false,
        }),
        noCommands,
      ]
    case "ChangedNewIdentityName":
      return [
        Model.make({ ...model, newIdentityName: message.value }),
        noCommands,
      ]
    case "ClickedUseExistingIdentity":
      return [
        Model.make({ ...model, identityChoicePending: true }),
        [ChooseIdentity({ kind: "use_existing", displayName: "" })],
      ]
    case "ClickedCreateNewIdentity":
      return [
        Model.make({ ...model, identityChoicePending: true }),
        [
          ChooseIdentity({
            kind: "create_new",
            displayName: model.newIdentityName.trim(),
          }),
        ],
      ]
    case "SettledChooseIdentity": {
      const result = message.result as ChooseIdentityResponse
      return [
        Model.make({
          ...model,
          identityChoiceState: result.state,
          identityChoicePending: false,
          onboardingStatusLine: result.ok
            ? { text: "identity chosen", tone: "success" }
            : {
                text: result.error ?? "could not record choice",
                tone: "error",
              },
        }),
        // Re-load the chain so the wizard advances past the identity step.
        result.ok ? [LoadOnboardingStatus()] : noCommands,
      ]
    }

    // ── Product Promises Forum surfacing (#5065) ────────────────────────────
    case "ChangedPromiseSurfacingPromiseId":
      return [
        Model.make({ ...model, promiseSurfacingPromiseId: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingSurface":
      return [
        Model.make({ ...model, promiseSurfacingSurface: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingClaimText":
      return [
        Model.make({ ...model, promiseSurfacingClaimText: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingExpectedBehavior":
      return [
        Model.make({
          ...model,
          promiseSurfacingExpectedBehavior: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingObservedBehavior":
      return [
        Model.make({
          ...model,
          promiseSurfacingObservedBehavior: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingEvidenceOrSteps":
      return [
        Model.make({
          ...model,
          promiseSurfacingEvidenceOrSteps: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingEnvironment":
      return [
        Model.make({
          ...model,
          promiseSurfacingEnvironment: message.value,
        }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingImpact":
      return [
        Model.make({ ...model, promiseSurfacingImpact: message.value }),
        noCommands,
      ]
    case "ChangedPromiseSurfacingSuggestedState":
      return [
        Model.make({
          ...model,
          promiseSurfacingSuggestedState: message.value,
        }),
        noCommands,
      ]
    case "ClickedRefreshPromiseSurfacing":
      return [
        Model.make({
          ...model,
          promiseSurfacingReadinessPending: true,
          promiseSurfacingStatus: {
            text: "checking Product Promises Forum...",
            tone: "info",
          },
        }),
        [LoadPromiseSurfacingReadiness()],
      ]
    case "GotPromiseSurfacingReadiness": {
      const projection = message.projection as PromiseSurfacingReadinessResponse
      return [
        Model.make({
          ...model,
          promiseSurfacingReadiness: projection,
          promiseSurfacingReadinessPending: false,
          promiseSurfacingStatus: projection.ok
            ? { text: "Forum posting ready", tone: "success" }
            : {
                text: projection.blockerRefs[0] ?? "Forum posting blocked",
                tone: "info",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedSurfacePromiseGap": {
      const validation = validatePromiseSurfacingInput({
        promiseId: model.promiseSurfacingPromiseId,
        surface: model.promiseSurfacingSurface,
        claimText: model.promiseSurfacingClaimText,
        expectedBehavior: model.promiseSurfacingExpectedBehavior,
        observedBehavior: model.promiseSurfacingObservedBehavior,
        evidenceOrSteps: model.promiseSurfacingEvidenceOrSteps,
        environment: model.promiseSurfacingEnvironment,
        impact: model.promiseSurfacingImpact,
        suggestedState: model.promiseSurfacingSuggestedState,
      })
      if (!validation.ok) {
        return [
          Model.make({
            ...model,
            promiseSurfacingStatus: {
              text: validation.errors[0] ?? "invalid promise report",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          promiseSurfacingSubmitPending: true,
          promiseSurfacingResult: null,
          promiseSurfacingStatus: {
            text: "checking ledger and posting report...",
            tone: "info",
          },
        }),
        [SurfacePromiseGap(validation.input)],
      ]
    }
    case "GotPromiseSurfacingResult": {
      const projection = message.projection as PromiseSurfacingResponse
      const label =
        projection.mode === "posted"
          ? `posted · ${projection.topicUrl ?? projection.topicId ?? "forum topic"}`
          : projection.mode === "drafted"
            ? `drafted · ${projection.blockerRefs[0] ?? "not posted"}`
            : projection.error ?? projection.blockerRefs[0] ?? "blocked"
      return [
        Model.make({
          ...model,
          promiseSurfacingResult: projection,
          promiseSurfacingSubmitPending: false,
          promiseSurfacingStatus: {
            text: label,
            tone: projection.ok
              ? "success"
              : projection.mode === "drafted"
                ? "info"
                : "error",
          },
        }),
        noCommands,
      ]
    }

    // ── Training launch/readiness feedback ───────────────────────────────────
    case "ClickedRefreshPublicActivity":
      return [
        Model.make({
          ...model,
          publicActivityTimelinePending: true,
          publicActivityTimelineStatus: {
            text: "refreshing public activity...",
            tone: "info",
          },
        }),
        [LoadPublicActivityTimeline()],
      ]
    case "GotPublicActivityTimeline": {
      const projection = message.projection as PublicActivityTimelineResponse
      const eventCount = projection.envelope?.events.length ?? 0
      const staleCount =
        projection.envelope?.sourceLag.filter(lag => lag.status !== "current")
          .length ?? 0
      return [
        Model.make({
          ...model,
          publicActivityTimeline: projection,
          publicActivityTimelinePending: false,
          publicActivityTimelineStatus: projection.ok
            ? {
                text: `${eventCountLabel(eventCount)} · ${sourceWarningCountLabel(staleCount)} · ${projection.sourceUrl}`,
                tone: staleCount === 0 ? "success" : "info",
              }
            : {
                text: projection.error ?? "public activity unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedRefreshTrainingRuns":
      return [
        Model.make({
          ...model,
          publicActivityTimelinePending: true,
          publicActivityTimelineStatus: {
            text: "refreshing public activity...",
            tone: "info",
          },
          trainingRunsPending: true,
          trainingRunsStatus: {
            text: "refreshing Worker projection...",
            tone: "info",
          },
          trainingDashboardPending: true,
          trainingDashboardStatus: {
            text: "refreshing public dashboards...",
            tone: "info",
          },
          trainingPromiseGatesPending: true,
          trainingPromiseGatesStatus: {
            text: "refreshing promise gates...",
            tone: "info",
          },
          trainingOperatorReadinessPending: true,
          trainingOperatorReadinessStatus: {
            text: "refreshing operator readiness...",
            tone: "info",
          },
          trainingEvidencePacketSummaryPending: true,
          trainingEvidencePacketSummaryStatus: {
            text: "refreshing evidence packet...",
            tone: "info",
          },
          proofReplayPending: true,
          proofReplayStatus: {
            text: "refreshing proof replay bundle...",
            tone: "info",
          },
        }),
        loadTrainingProjectionCommands(model),
      ]
    case "SelectedTrainingSceneNode":
      return [
        Model.make({
          ...model,
          selectedTrainingSceneNodeId: message.nodeId,
        }),
        noCommands,
      ]
    case "GotTrainingRuns": {
      const projection = message.projection as TrainingRunsResponse
      const runCount = projection.runs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingRuns: projection,
          trainingRunsPending: false,
          trainingPlanFirstObservedAt: plannedRunFirstObservedAt(
            model,
            projection,
          ),
          trainingRunsStatus: projection.ok
            ? {
                text: `${runCount} runs from ${projection.sourceUrl}`,
                tone: "success",
              }
            : {
                text: projection.error ?? "training projection unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingPromiseGates": {
      const projection = message.projection as TrainingPromiseGatesResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingPromiseGates: projection,
          trainingPromiseGatesPending: false,
          trainingPromiseGatesStatus: projection.ok
            ? {
                text: `${projection.promises.length} training promises · ${blockerCount} blockers · ${projection.registryVersion}`,
                tone: blockerCount === 0 ? "success" : "info",
              }
            : {
                text: projection.error ?? "training promise gates unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingOperatorReadiness": {
      const projection = message.projection as TrainingOperatorReadinessResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingOperatorReadiness: projection,
          trainingOperatorReadinessPending: false,
          trainingOperatorReadinessStatus: projection.ok
            ? {
                text: `operator ready · ${projection.trainingBaseUrl}`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  `${blockerCount} operator blockers · ${projection.trainingBaseUrl}`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingEvidencePacketSummary": {
      const projection =
        message.projection as TrainingEvidencePacketSummaryResponse
      const blockerCount = projection.blockerRefs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingEvidencePacketSummary: projection,
          trainingEvidencePacketSummaryPending: false,
          trainingEvidencePacketSummaryStatus: projection.ok
            ? {
                text: `packet ready · ${projection.receiptRefCount} receipts · ${projection.distinctPylonCount} pylons`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  `packet blocked · ${blockerCount} blockers`,
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "GotTrainingDashboard": {
      const projection = message.projection as TrainingDashboardSummaryResponse
      const lanes = projection.leaderboards?.lanes ?? []
      const rankedLaneCount = lanes.filter(lane => lane.rowCount > 0).length
      return [
        Model.make({
          ...model,
          trainingDashboard: projection,
          trainingDashboardPending: false,
          trainingDashboardStatus: projection.ok
            ? {
                text: `${rankedLaneCount}/${lanes.length} ranked lanes from ${projection.sourceUrl}`,
                tone: "success",
              }
            : {
                text: projection.error ?? "training dashboards unavailable",
                tone: "error",
              },
        }),
        noCommands,
      ]
    }
    case "SelectedProofReplay":
      return [
        Model.make({
          ...model,
          selectedProofReplayMode: "catalog",
          selectedProofReplaySlug: message.slug,
          proofReplay: null,
          proofReplayPending: true,
          proofReplayStatus: {
            text: "loading public replay bundle...",
            tone: "info",
          },
        }),
        [LoadProofReplayBundle({ request: { mode: "catalog", slug: message.slug } })],
      ]
    case "ChangedProofReplayGeneratedFrom":
      return [
        Model.make({ ...model, generatedProofReplayFrom: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedTo":
      return [
        Model.make({ ...model, generatedProofReplayTo: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedRunRef":
      return [
        Model.make({ ...model, generatedProofReplayRunRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedWindowRef":
      return [
        Model.make({ ...model, generatedProofReplayWindowRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedActorRef":
      return [
        Model.make({ ...model, generatedProofReplayActorRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedPairRef":
      return [
        Model.make({ ...model, generatedProofReplayPairRef: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedKind":
      return [
        Model.make({ ...model, generatedProofReplayKind: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedSource":
      return [
        Model.make({ ...model, generatedProofReplaySource: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedSince":
      return [
        Model.make({ ...model, generatedProofReplaySince: message.value }),
        noCommands,
      ]
    case "ChangedProofReplayGeneratedLimit":
      return [
        Model.make({ ...model, generatedProofReplayLimit: message.value }),
        noCommands,
      ]
    case "ClickedLoadGeneratedProofReplay": {
      const next = Model.make({
        ...model,
        selectedProofReplayMode: "generated",
        proofReplay: null,
        proofReplayPending: true,
        proofReplayStatus: {
          text: "loading generated public replay bundle...",
          tone: "info",
        },
      })
      return [
        next,
        [LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(next) })],
      ]
    }
    case "ClickedRefreshProofReplay":
      return [
        Model.make({
          ...model,
          proofReplayPending: true,
          proofReplayStatus: {
            text: "refreshing proof replay bundle...",
            tone: "info",
          },
        }),
        [LoadProofReplayBundle({ request: proofReplayCommandRequestForModel(model) })],
      ]
    case "GotProofReplayBundle": {
      const projection = message.projection as DesktopProofReplayProjection
      const sourceLabel =
        projection.entry?.title ??
        projection.bundle?.title ??
        projection.filterLabel ??
        projection.sourceUrl
      const amount = projection.summary?.confirmedZapSats ?? 0
      return [
        Model.make({
          ...model,
          proofReplay: projection,
          proofReplayPending: false,
          proofReplayStatus: projection.ok
            ? {
                text: `${sourceLabel} · ${projection.summary?.eventCount ?? 0} events · ${amount.toLocaleString()} sats`,
                tone: "success",
              }
            : {
                text:
                  projection.error ??
                  projection.blockerRefs[0] ??
                  "proof replay unavailable",
                tone: projection.error ? "error" : "info",
              },
        }),
        noCommands,
      ]
    }
    case "ClickedPlanTrainingWindow":
      return [
        Model.make({
          ...model,
          trainingPlanPending: true,
          trainingPlanFirstObservedAt: null,
          trainingPlanStatus: {
            text: "planning R1 run window...",
            tone: "info",
          },
        }),
        [PlanTrainingRunWindow()],
      ]
    case "SettledPlanTrainingWindow": {
      const projection = message.projection as TrainingPlanResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingPlan: projection,
          trainingPlanPending: false,
          trainingPlanFirstObservedAt: null,
          trainingPlanStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedActivateTrainingWindow":
      if (message.windowRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingActivationStatus: {
              text: "no planned window selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingActivationPending: true,
          trainingActivationStatus: {
            text: `activating ${message.windowRef}...`,
            tone: "info",
          },
        }),
        [ActivateTrainingWindow({ windowRef: message.windowRef })],
      ]
    case "SettledActivateTrainingWindow": {
      const projection = message.projection as TrainingWindowActionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingActivation: projection,
          trainingActivationPending: false,
          trainingActivationStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedReconcileTrainingWindow":
      if (message.windowRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingReconcileStatus: {
              text: "no sealed window selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingReconcilePending: true,
          trainingReconcileStatus: {
            text: `reconciling ${message.windowRef}...`,
            tone: "info",
          },
        }),
        [ReconcileTrainingWindow({ windowRef: message.windowRef })],
      ]
    case "SettledReconcileTrainingWindow": {
      const projection = message.projection as TrainingWindowActionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing"
      return [
        Model.make({
          ...model,
          trainingReconcile: projection,
          trainingReconcilePending: false,
          trainingReconcileStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedClaimTrainingLease":
      return [
        Model.make({
          ...model,
          trainingLeasePending: true,
          trainingLeaseStatus: {
            text: "claiming training lease...",
            tone: "info",
          },
        }),
        [ClaimTrainingWindowLease()],
      ]
    case "SettledClaimTrainingLease": {
      const projection = message.projection as TrainingWindowLeaseResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "pylon_ref_missing"
      return [
        Model.make({
          ...model,
          trainingLease: projection,
          trainingLeasePending: false,
          trainingLeaseStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedRequestTrainingBootstrap":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingBootstrapStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingBootstrapPending: true,
          trainingBootstrapStatus: {
            text: `requesting bootstrap for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          RequestTrainingBootstrapGrant({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledRequestTrainingBootstrap": {
      const projection = message.projection as TrainingBootstrapGrantResponse
      const inactiveReason =
        projection.reason === "pylon_ref_missing" ||
        projection.reason === "invalid_pylon_ref" ||
        projection.reason === "refused" ||
        projection.reason === "queued"
      return [
        Model.make({
          ...model,
          trainingBootstrap: projection,
          trainingBootstrapPending: false,
          trainingBootstrapStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        trainingBootstrapShouldRefresh(projection)
          ? loadTrainingProjectionCommands(model)
          : noCommands,
      ]
    }
    case "ClickedBuildTrainingEvidencePacket":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingEvidencePacketBuildStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingEvidencePacketBuildPending: true,
          trainingEvidencePacketBuildStatus: {
            text: `building evidence packet for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          BuildTrainingEvidencePacket({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledBuildTrainingEvidencePacket": {
      const projection =
        message.projection as TrainingEvidencePacketBuildResponse
      const infoReason =
        projection.reason === "disabled" ||
        projection.reason === "worker_receipts_path_missing" ||
        projection.reason === "packet_path_missing" ||
        projection.reason === "packet_blocked"
      const shouldRefresh =
        projection.reason === "written" || projection.reason === "packet_blocked"
      return [
        Model.make({
          ...model,
          trainingEvidencePacketBuild: projection,
          trainingEvidencePacketBuildPending: false,
          trainingEvidencePacketBuildStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : infoReason ? "info" : "error",
          },
        }),
        shouldRefresh ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedAdmitTrainingEvidence":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingEvidenceAdmissionStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingEvidenceAdmissionPending: true,
          trainingEvidenceAdmissionStatus: {
            text: `admitting evidence for ${message.trainingRunRef}...`,
            tone: "info",
          },
        }),
        [
          AdmitTrainingRealGradientEvidence({
            trainingRunRef: message.trainingRunRef,
          }),
        ],
      ]
    case "SettledAdmitTrainingEvidence": {
      const projection = message.projection as TrainingEvidenceAdmissionResponse
      const inactiveReason =
        projection.reason === "disabled" ||
        projection.reason === "admin_token_missing" ||
        projection.reason === "packet_path_missing"
      return [
        Model.make({
          ...model,
          trainingEvidenceAdmission: projection,
          trainingEvidenceAdmissionPending: false,
          trainingEvidenceAdmissionStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    }
    case "ClickedQueueTrainingLaunch":
      return [
        Model.make({
          ...model,
          trainingLaunchPending: true,
          trainingLaunchStatus: {
            text: "queueing launch check...",
            tone: "info",
          },
        }),
        [QueueTrainingLaunch()],
      ]
    case "SettledQueueTrainingLaunch":
      return [
        Model.make({
          ...model,
          trainingLaunchPending: false,
          trainingLaunchStatus: {
            text: message.text,
            tone: message.ok ? "success" : "error",
          },
        }),
        message.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]
    case "ClickedQueueTrainingCloseout":
      if (message.trainingRunRef.trim() === "") {
        return [
          Model.make({
            ...model,
            trainingCloseoutStatus: {
              text: "no training run selected",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          trainingCloseoutPending: true,
          trainingCloseoutStatus: {
            text: "queueing closeout packet...",
            tone: "info",
          },
        }),
        [
          QueueTrainingCloseout({
            trainingRunRef: message.trainingRunRef,
            windowRef: message.windowRef,
            leaseRef: message.leaseRef,
            bootstrapGrantRef: message.bootstrapGrantRef,
          }),
        ],
      ]
    case "SettledQueueTrainingCloseout":
      return [
        Model.make({
          ...model,
          trainingCloseoutPending: false,
          trainingCloseoutStatus: {
            text: message.text,
            tone: message.ok ? "success" : "error",
          },
        }),
        message.ok ? loadTrainingProjectionCommands(model) : noCommands,
      ]

    // ── Spawn ──────────────────────────────────────────────────────────────────
    case "ChangedSpawnAdapter":
      return [Model.make({ ...model, spawnAdapter: message.adapter }), noCommands]
    case "ChangedSpawnObjective":
      return [Model.make({ ...model, spawnObjective: message.value }), noCommands]
    case "ChangedSpawnVerify":
      return [Model.make({ ...model, spawnVerify: message.value }), noCommands]
    case "ChangedSpawnLane":
      return [Model.make({ ...model, spawnLane: message.lane }), noCommands]

    // ── Settings preferences (#5472) ─────────────────────────────────────────
    // Each handler (a) writes the preference field, (b) for theme/defaults makes
    // the choice take effect immediately, and (c) persists via PersistPreferences
    // (local, refs-only — no RPC). A `persist` helper keeps the four writes from
    // drifting on which fields get saved.
    case "ChangedThemePreference": {
      const next = Model.make({ ...model, themePreference: message.theme })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ChangedDefaultAdapter": {
      // Persist the default AND seed the live spawn adapter so the new default
      // takes effect now (spawn/composer/chat read `spawnAdapter` directly).
      const next = Model.make({
        ...model,
        defaultAdapter: message.adapter,
        spawnAdapter: message.adapter,
      })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ChangedDefaultLane": {
      const next = Model.make({
        ...model,
        defaultLane: message.lane,
        spawnLane: message.lane,
      })
      return [next, [persistPreferencesFor(next)]]
    }
    case "ToggledNotificationPanel": {
      const next = Model.make({ ...model, showNotificationPanel: message.show })
      return [next, [persistPreferencesFor(next)]]
    }
    case "SettledPersistPreferences":
      // No-op: the preference values already live in the Model; this only closes
      // the persistence command (side effect done in the command, not here).
      return [model, noCommands]
    case "ClickedSpawn": {
      const validation = validateSpawnRequest({
        adapter: model.spawnAdapter,
        objective: model.spawnObjective,
      })
      if (!validation.ok || validation.adapter === null) {
        return [
          Model.make({
            ...model,
            spawnStatus: {
              text: validation.errors[0] ?? "invalid request",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      return [
        Model.make({
          ...model,
          spawnPending: true,
          spawnStatus: { text: "sending…", tone: "info" },
        }),
        [
          SpawnSession({
            adapter: validation.adapter,
            objective: validation.objective,
            verify,
            lane: model.spawnLane,
            // CS-A1: the legacy Spawn pane has no per-account picker; the
            // composer pane is the account-aware spawn surface.
            accountRef: null,
          }),
        ],
      ]
    }
    case "SucceededSpawn":
      return [
        Model.make({
          ...model,
          spawnPending: false,
          spawnStatus: { text: "", tone: "idle" },
          spawnObjective: "",
          spawnVerify: "",
          pane: "session-detail",
          selectedSessionRef: message.sessionRef,
          expandedEvents: [],
        }),
        noCommands,
      ]
    case "FailedSpawn":
      return [
        Model.make({
          ...model,
          spawnPending: false,
          spawnStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── Session detail: cancel ──────────────────────────────────────────────────
    case "ClickedCancelSession":
      return [model, [CancelSession({ sessionRef: message.sessionRef })]]
    case "SettledCancelSession":
      // The next node-state poll carries the authoritative session state.
      return [model, noCommands]

    // ── CS-A2 (#5362): swarm quick action — open an existing session in the
    //    composer. Adopts the session as the composer's active thread (its
    //    repo/worktree + runtime) so the owner can reply/continue from the
    //    day-to-day coding surface. The live node poll then drives the
    //    transcript/approvals the composer renders — no new wire verb.
    case "ClickedOpenSessionInComposer":
      return [
        Model.make({
          ...model,
          pane: "composer",
          expandedEvents: [],
          composerSessionRef: message.sessionRef,
          composerRepoPath: message.workspaceRef ?? "",
          composerReply: "",
          composerPending: false,
          composerStatus: {
            text: "adopted session — streaming transcript",
            tone: "info" as const,
          },
          spawnAdapter: message.adapter,
          // Load the managed-account registry so the picker is populated, the
          // same way NavigatedTo("composer") does.
          managedAccountsPending: true,
          managedAccountsStatus: {
            text: "loading accounts...",
            tone: "info" as const,
          },
        }),
        [LoadManagedAccounts()],
      ]

    // ── #5355: coding composer ──────────────────────────────────────────────────
    case "ChangedComposerRepoPath":
      return [Model.make({ ...model, composerRepoPath: message.value }), noCommands]
    case "ChangedComposerReply":
      return [Model.make({ ...model, composerReply: message.value }), noCommands]
    case "SelectedComposerAccount":
      return [
        Model.make({ ...model, composerAccountRef: message.accountRef }),
        noCommands,
      ]
    case "ClickedComposerSpawn": {
      const worktreePath =
        model.composerRepoPath.trim() === "" ? null : model.composerRepoPath.trim()
      // CS-A1: Apple FM is a spawn-adapter option but uses its own control verb
      // (apple_fm.session.start), so it validates the objective directly and
      // routes through the Apple FM command rather than session.spawn.
      if (model.spawnAdapter === "apple_fm") {
        const objective = model.spawnObjective.trim()
        if (objective === "") {
          return [
            Model.make({
              ...model,
              composerStatus: { text: "objective must be non-empty", tone: "error" },
            }),
            noCommands,
          ]
        }
        return [
          Model.make({
            ...model,
            composerPending: true,
            composerStatus: { text: "starting local Apple FM session…", tone: "info" },
            composerTurns: [objective],
          }),
          [SpawnAppleFmComposerTurn({ objective, worktreePath })],
        ]
      }
      // First coding turn — reuse the shared spawn validation/fields.
      const validation = validateSpawnRequest({
        adapter: model.spawnAdapter,
        objective: model.spawnObjective,
      })
      if (!validation.ok || validation.adapter === null) {
        return [
          Model.make({
            ...model,
            composerStatus: {
              text: validation.errors[0] ?? "invalid request",
              tone: "error",
            },
          }),
          noCommands,
        ]
      }
      const verify = parseVerifyLines(model.spawnVerify)
      const objective = validation.objective
      return [
        Model.make({
          ...model,
          composerPending: true,
          composerStatus: { text: "starting coding session…", tone: "info" },
          composerTurns: [objective],
        }),
        [
          SpawnComposerTurn({
            adapter: validation.adapter,
            objective,
            verify,
            lane: model.spawnLane,
            worktreePath,
            accountRef: model.composerAccountRef,
          }),
        ],
      ]
    }
    case "ClickedComposerReply": {
      // Follow-up turn — a continuation session.spawn carrying the prior turns.
      const followUp = model.composerReply.trim()
      if (followUp === "") {
        return [
          Model.make({
            ...model,
            composerStatus: { text: "type a follow-up first", tone: "error" },
          }),
          noCommands,
        ]
      }
      const objective = buildComposerContinuationObjective(
        model.composerTurns,
        followUp,
      )
      const verify = parseVerifyLines(model.spawnVerify)
      const worktreePath =
        model.composerRepoPath.trim() === "" ? null : model.composerRepoPath.trim()
      const continuing = Model.make({
        ...model,
        composerPending: true,
        composerReply: "",
        composerStatus: { text: "continuing the thread…", tone: "info" },
        composerTurns: [...model.composerTurns, followUp],
      })
      // CS-A1: continuation turns honor the selected runtime, including Apple FM.
      if (model.spawnAdapter === "apple_fm") {
        return [continuing, [SpawnAppleFmComposerTurn({ objective, worktreePath })]]
      }
      return [
        continuing,
        [
          SpawnComposerTurn({
            adapter: model.spawnAdapter,
            objective,
            verify,
            lane: model.spawnLane,
            worktreePath,
            accountRef: model.composerAccountRef,
          }),
        ],
      ]
    }
    case "ClickedComposerNewThread":
      return [
        Model.make({
          ...model,
          composerSessionRef: null,
          composerReply: "",
          composerTurns: [],
          composerStatus: { text: "", tone: "idle" },
          composerPending: false,
          spawnObjective: "",
        }),
        // CS-A1: a fresh thread reloads the account list so a newly added
        // account is pickable without re-navigating.
        [LoadManagedAccounts()],
      ]
    case "SucceededComposerTurn":
      return [
        Model.make({
          ...model,
          composerPending: false,
          composerSessionRef: message.sessionRef,
          composerStatus: { text: "running — streaming transcript", tone: "success" },
          // First turn clears the objective box so the form is reply-ready.
          spawnObjective: "",
        }),
        noCommands,
      ]
    case "FailedComposerTurn":
      return [
        Model.make({
          ...model,
          composerPending: false,
          composerStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── #5453: Blueprint program chat ─────────────────────────────────────────
    case "ChangedChatInput":
      return [Model.make({ ...model, chatInput: message.value }), noCommands]
    case "ClickedChatSubmit": {
      const prompt = model.chatInput.trim()
      if (prompt === "") {
        return [
          Model.make({
            ...model,
            chatStatus: { text: "message must be non-empty", tone: "error" },
          }),
          noCommands,
        ]
      }
      const adapter =
        model.spawnAdapter === "claude_agent" ? "claude_agent" : "codex"
      const objective = buildBlueprintChatObjective(prompt)
      return [
        Model.make({
          ...model,
          chatInput: "",
          chatPending: true,
          chatStatus: { text: "spawning Blueprint turn...", tone: "info" },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.user"),
              role: "user",
              body: prompt,
              timestamp: chatTimestamp(),
              linkedSessionRef: null,
              steps: blueprintChatScopedSteps({
                tassadarStatus: "running",
                tassadarVerdict: "pending",
              }),
            },
          ],
        }),
        [
          SpawnChatTurn({
            adapter,
            objective,
            verify: [],
            lane: model.spawnLane,
            accountRef: model.composerAccountRef,
          }),
        ],
      ]
    }
    case "SucceededChatTurn":
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatSessionRef: message.sessionRef,
          chatStatus: {
            text: "running - node-state poll will stream the turn",
            tone: "success",
          },
          chatMessages: [
            ...model.chatMessages,
            {
              id: chatMessageId("chat.assistant"),
              role: "assistant",
              body: "Blueprint turn spawned.",
              timestamp: chatTimestamp(),
              linkedSessionRef: message.sessionRef,
              steps: blueprintChatScopedSteps({
                linkedSessionRef: message.sessionRef,
                tassadarStatus: "verified",
                tassadarVerdict: "verified",
              }),
            },
          ],
        }),
        noCommands,
      ]
    case "FailedChatTurn":
      return [
        Model.make({
          ...model,
          chatPending: false,
          chatStatus: { text: message.error, tone: "error" },
        }),
        noCommands,
      ]

    // ── CS-A1: account management (node-local dev.accounts) ─────────────────────
    case "ClickedRefreshManagedAccounts":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "loading accounts...", tone: "info" },
        }),
        [LoadManagedAccounts()],
      ]
    case "GotManagedAccounts": {
      const projection = message.projection as {
        ok: boolean
        accounts: ReadonlyArray<unknown>
        error?: string
      }
      return [
        Model.make({
          ...model,
          managedAccounts: message.projection,
          managedAccountsPending: false,
          managedAccountsStatus: projection.ok
            ? { text: "", tone: "idle" }
            : { text: projection.error ?? "could not load accounts", tone: "error" },
        }),
        noCommands,
      ]
    }
    case "ChangedAddAccountRef":
      return [Model.make({ ...model, addAccountRef: message.value }), noCommands]
    case "ChangedAddAccountProvider":
      return [Model.make({ ...model, addAccountProvider: message.provider }), noCommands]
    case "ChangedAddAccountHome":
      return [Model.make({ ...model, addAccountHome: message.value }), noCommands]
    case "ChangedAddAccountPriority":
      return [Model.make({ ...model, addAccountPriority: message.value }), noCommands]
    case "ClickedAddManagedAccount": {
      const ref = model.addAccountRef.trim()
      const home = model.addAccountHome.trim()
      if (ref === "" || home === "") {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: { text: "ref and home are required", tone: "error" },
          }),
          noCommands,
        ]
      }
      const priorityRaw = model.addAccountPriority.trim()
      const priorityParsed = priorityRaw === "" ? null : Number(priorityRaw)
      if (priorityParsed !== null && !Number.isFinite(priorityParsed)) {
        return [
          Model.make({
            ...model,
            managedAccountsStatus: { text: "priority must be a number", tone: "error" },
          }),
          noCommands,
        ]
      }
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "adding account...", tone: "info" },
        }),
        [
          AddManagedAccount({
            ref,
            provider: model.addAccountProvider,
            home,
            priority: priorityParsed,
          }),
        ],
      ]
    }
    case "ClickedRemoveManagedAccount":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "removing account...", tone: "info" },
        }),
        [RemoveManagedAccount({ ref: message.ref, provider: message.provider })],
      ]
    case "ClickedBumpManagedAccountPriority":
      return [
        Model.make({
          ...model,
          managedAccountsPending: true,
          managedAccountsStatus: { text: "updating priority...", tone: "info" },
        }),
        [
          SetManagedAccountPriority({
            ref: message.ref,
            provider: message.provider,
            priority: message.priority,
          }),
        ],
      ]
    case "SettledManagedAccountMutation": {
      const projection = message.projection as {
        ok: boolean
        accounts: ReadonlyArray<unknown>
        error?: string
      }
      // A successful mutation returns the refreshed list; clear the add-form on
      // success so the surface is ready for the next entry.
      return [
        Model.make({
          ...model,
          managedAccounts: message.projection,
          managedAccountsPending: false,
          managedAccountsStatus: projection.ok
            ? { text: "saved", tone: "success" }
            : { text: projection.error ?? "account update failed", tone: "error" },
          ...(projection.ok
            ? { addAccountRef: "", addAccountHome: "", addAccountPriority: "" }
            : {}),
        }),
        noCommands,
      ]
    }
  }
}
