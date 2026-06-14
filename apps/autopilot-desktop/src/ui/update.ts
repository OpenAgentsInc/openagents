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
  AdmitTrainingRealGradientEvidence,
  CancelSession,
  ClaimTrainingWindowLease,
  DeployCloud,
  LoadTrainingDashboard,
  LoadTrainingOperatorReadiness,
  LoadTrainingPromiseGates,
  LoadTrainingRuns,
  PlanTrainingRunWindow,
  QueueTrainingCloseout,
  QueueTrainingLaunch,
  ReconcileTrainingWindow,
  RequestTrainingBootstrapGrant,
  ResolveApproval,
  SetCoordinatorPaused,
  SpawnSession,
  SubmitIntent,
} from "./commands"
import { parseVerifyLines } from "./helpers"
import type { Message } from "./message"
import { Model } from "./model"
import type {
  TrainingBootstrapGrantResponse,
  TrainingDashboardSummaryResponse,
  TrainingEvidenceAdmissionResponse,
  TrainingOperatorReadinessResponse,
  TrainingPlanResponse,
  TrainingPromiseGatesResponse,
  TrainingRunsResponse,
  TrainingWindowActionResponse,
  TrainingWindowLeaseResponse,
} from "../shared/rpc"

type Result = readonly [Model, ReadonlyArray<Command.Command<Message>>]

const noCommands: ReadonlyArray<Command.Command<Message>> = []

const loadTrainingProjectionCommands = (): ReadonlyArray<Command.Command<Message>> => [
  LoadTrainingRuns(),
  LoadTrainingDashboard(),
  LoadTrainingPromiseGates(),
  LoadTrainingOperatorReadiness(),
]

const trainingBootstrapShouldRefresh = (
  projection: TrainingBootstrapGrantResponse,
): boolean =>
  projection.reason === "granted" ||
  projection.reason === "queued" ||
  projection.reason === "refused"

export const update = (model: Model, message: Message): Result => {
  switch (message._tag) {
    // ── Inbound projections ────────────────────────────────────────────────
    case "GotNodeState":
      return [Model.make({ ...model, node: message.node }), noCommands]
    case "GotNotifications":
      return [Model.make({ ...model, notifications: message.view }), noCommands]

    // ── Navigation ─────────────────────────────────────────────────────────
    case "NavigatedTo":
      return [
        Model.make({
          ...model,
          pane: message.pane,
          expandedEvents: [],
          ...(message.pane === "training"
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
              }
            : {}),
        }),
        message.pane === "training"
          ? loadTrainingProjectionCommands()
          : noCommands,
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
        }),
        loadTrainingProjectionCommands(),
      ]
    case "GotTrainingRuns": {
      const projection = message.projection as TrainingRunsResponse
      const runCount = projection.runs?.length ?? 0
      return [
        Model.make({
          ...model,
          trainingRuns: projection,
          trainingRunsPending: false,
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
    case "ClickedPlanTrainingWindow":
      return [
        Model.make({
          ...model,
          trainingPlanPending: true,
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
          trainingPlanStatus: {
            text: projection.message,
            tone: projection.ok ? "success" : inactiveReason ? "info" : "error",
          },
        }),
        projection.ok ? loadTrainingProjectionCommands() : noCommands,
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
        projection.ok ? loadTrainingProjectionCommands() : noCommands,
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
        projection.ok ? loadTrainingProjectionCommands() : noCommands,
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
        projection.ok ? loadTrainingProjectionCommands() : noCommands,
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
          ? loadTrainingProjectionCommands()
          : noCommands,
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
        projection.ok ? loadTrainingProjectionCommands() : noCommands,
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
        message.ok ? loadTrainingProjectionCommands() : noCommands,
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
        message.ok ? loadTrainingProjectionCommands() : noCommands,
      ]

    // ── Spawn ──────────────────────────────────────────────────────────────────
    case "ChangedSpawnAdapter":
      return [Model.make({ ...model, spawnAdapter: message.adapter }), noCommands]
    case "ChangedSpawnObjective":
      return [Model.make({ ...model, spawnObjective: message.value }), noCommands]
    case "ChangedSpawnVerify":
      return [Model.make({ ...model, spawnVerify: message.value }), noCommands]
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
  }
}
