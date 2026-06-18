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
  DeployCloud,
  LoadAppleFmReadiness,
  LoadBuiltInAgentReadiness,
  LoadInstallReadiness,
  LoadManagedAccounts,
  LoadPromiseSurfacingReadiness,
  LoadProofReplayBundle,
  LoadTrainingDashboard,
  LoadTrainingEvidencePacketSummary,
  LoadTrainingOperatorReadiness,
  LoadTrainingPromiseGates,
  LoadTrainingRuns,
  PlanTrainingRunWindow,
  QueueTrainingCloseout,
  QueueTrainingLaunch,
  ReconcileTrainingWindow,
  RemoveManagedAccount,
  RequestTrainingBootstrapGrant,
  ResolveApproval,
  SetCoordinatorPaused,
  SetManagedAccountPriority,
  SpawnAppleFmComposerTurn,
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
import type { Message } from "./message"
import { Model, type PaneId } from "./model"
import type { DesktopProofReplayProjection } from "../shared/proof-replays"
import { validatePromiseSurfacingInput } from "../shared/promise-surfacing"
import type {
  AppleFmReadinessResponse,
  BuiltInAgentReadinessResponse,
  InstallReadinessResponse,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
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

const loadTrainingProjectionCommands = (
  model: Model,
): ReadonlyArray<Command.Command<Message>> => [
  LoadTrainingRuns(),
  LoadTrainingDashboard(),
  LoadTrainingPromiseGates(),
  LoadTrainingOperatorReadiness(),
  LoadTrainingEvidencePacketSummary(),
  LoadProofReplayBundle({ slug: model.selectedProofReplaySlug }),
]

const trainingBootstrapShouldRefresh = (
  projection: TrainingBootstrapGrantResponse,
): boolean =>
  projection.reason === "granted" ||
  projection.reason === "queued" ||
  projection.reason === "refused"

const isTrainingPane = (pane: PaneId): boolean =>
  pane === "training" || pane === "training-fullscreen"

const isBuiltInAgentPane = (pane: PaneId): boolean => pane === "builtin-agent"
const isSettingsPane = (pane: PaneId): boolean => pane === "settings"
// CS-A1: the composer pane hosts the per-session account picker, so opening it
// (and the Spawn pane / Settings, which also surface accounts) refreshes the
// managed-account registry from the node's local config.
const isAccountManagingPane = (pane: PaneId): boolean =>
  pane === "composer" || pane === "spawn" || pane === "settings"

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
        [LoadInstallReadiness()],
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
            : isBuiltInAgentPane(message.pane)
              ? [LoadBuiltInAgentReadiness(), LoadAppleFmReadiness(), LoadPromiseSurfacingReadiness()]
              : isSettingsPane(message.pane)
                ? [LoadInstallReadiness()]
                : noCommands),
          ...(isAccountManagingPane(message.pane) ? [LoadManagedAccounts()] : noCommands),
        ],
      ]
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
    case "ClickedRefreshTrainingRuns":
      return [
        Model.make({
          ...model,
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
          selectedProofReplaySlug: message.slug,
          proofReplay: null,
          proofReplayPending: true,
          proofReplayStatus: {
            text: "loading public replay bundle...",
            tone: "info",
          },
        }),
        [LoadProofReplayBundle({ slug: message.slug })],
      ]
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
        [LoadProofReplayBundle({ slug: model.selectedProofReplaySlug })],
      ]
    case "GotProofReplayBundle": {
      const projection = message.projection as DesktopProofReplayProjection
      const sourceLabel = projection.entry?.title ?? projection.sourceUrl
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
