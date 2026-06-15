// CL-53: the Foldkit view for the Autopilot Desktop webview.
//
// Replaces the hand-DOM shell + panes/ + cards/. A persistent left sidebar
// (Nodes/Sessions/Decisions/Spawn/Settings + pending-decision badge + node
// status line + coordinator Pause/Resume toggle) and a content-pane router.
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
  SessionList,
  type AccountSummary,
} from "@openagentsinc/autopilot-ui"
import { trainingRunView } from "@openagentsinc/three-effect/foldkit"
import { projectPylonNetworkScene } from "../shared/pylon-network-scene"
import { pylonDiamondsView } from "./pylon-diamonds-element"
import { pylonNetworkVisualizationOptions } from "./pylon-network-visualization"
import {
  defaultTrainingRunNodes,
  trainingRunVisualizationOptionsFromSnapshot,
  type TrainingRunNodeDefinition,
  type TrainingRunOperatorSignalDefinition,
  type TrainingRunOperatorSignalState,
  type TrainingRunPromiseSignalDefinition,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"
import type { Attribute, Document, Html } from "foldkit/html"
import { html } from "foldkit/html"

import type {
  AccountRow,
  AppleFmReadinessResponse,
  ApprovalRow,
  AssignmentRow,
  IntentRow,
  BuiltInAgentReadinessResponse,
  InstallReadinessResponse,
  NodeStateMessage,
  PromiseSurfacingReadinessResponse,
  PromiseSurfacingResponse,
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
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ChangedSpawnLane,
  ClickedCancelSession,
  ClickedActivateTrainingWindow,
  ClickedAdmitTrainingEvidence,
  ClickedBuildTrainingEvidencePacket,
  ClickedClaimTrainingLease,
  ClickedCoordinatorToggle,
  ClickedDeploy,
  ClickedPlanTrainingWindow,
  ClickedQueueTrainingCloseout,
  ClickedReconcileTrainingWindow,
  ClickedRefreshInstallReadiness,
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
  SelectedTrainingSceneNode,
  type Message,
  NavigatedTo,
  SelectedSession,
  ToggledEvent,
} from "./message"
import {
  approvalLabel,
  artifactLineText,
  assignmentMeta,
  connectionSummary,
  coordinatorToggleLabel,
  eventExpandable,
  eventRowText,
  nodeStatusLine,
  sessionCancellable,
  shipStatusLine,
  stateBreakdown,
  trainingProjectionMeta,
  verifyLineText,
  walletSummary,
} from "./helpers"
import {
  type Model,
  type PaneId,
  type SessionFilter,
  modelTrainingActivation,
  modelTrainingBootstrap,
  modelTrainingDashboard,
  modelTrainingEvidenceAdmission,
  modelTrainingEvidencePacketBuild,
  modelTrainingEvidencePacketSummary,
  modelTrainingLease,
  modelAppleFmReadiness,
  modelBuiltInAgentReadiness,
  modelInstallReadiness,
  modelPromiseSurfacingReadiness,
  modelPromiseSurfacingResult,
  modelNode,
  modelPylonStats,
  modelNotifications,
  modelTrainingOperatorReadiness,
  modelTrainingPlan,
  modelTrainingPromiseGates,
  modelTrainingReconcile,
  modelTrainingRuns,
} from "./model"

const h = html<Message>()
const cls = (value: string): Attribute<Message> => h.Class(value)

// ── Small shared building blocks (own h.* — no hand DOM) ─────────────────────

const card = (title: string, children: ReadonlyArray<Html>): Html =>
  h.section([cls("card")], [h.h2([cls("card-title")], [title]), ...children])

const emptyLine = (text: string): Html => h.p([cls("empty-state")], [text])

const paneTitle = (text: string): Html => h.h1([cls("pane-title")], [text])

// ── Sidebar ──────────────────────────────────────────────────────────────────

const NAV: ReadonlyArray<{ id: PaneId; label: string }> = [
  { id: "network", label: "Network" },
  { id: "builtin-agent", label: "Agent" },
  { id: "nodes", label: "Nodes" },
  { id: "training", label: "Training" },
  { id: "training-fullscreen", label: "Training Live" },
  { id: "sessions", label: "Sessions" },
  { id: "decisions", label: "Decisions" },
  { id: "spawn", label: "Spawn" },
  { id: "settings", label: "Settings" },
]

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

const sidebar = (model: Model): Html => {
  const node = modelNode(model)
  const pendingCount = node?.approvals?.length ?? 0

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
      coordinatorToggle(node),
      ...NAV.map((item) =>
        h.button(
          [
            cls(`nav-item${model.pane === item.id ? " active" : ""}`),
            h.Type("button"),
            h.OnClick(NavigatedTo({ pane: item.id })),
          ],
          item.id === "decisions" && pendingCount > 0
            ? [item.label, h.span([cls("nav-badge")], [String(pendingCount)])]
            : [item.label],
        ),
      ),
    ],
  )
}

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
                    [cls("ask-dot"), h.Style(`background-color:${sl.dotColor}`)],
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
// shared AccountList component.
const toAccountSummary = (row: AccountRow): AccountSummary => ({
  accountRefHash: `${row.provider}:${row.homeState}`,
  provider: row.provider,
  state: row.ready ? "ready" : "quota_blocked",
})

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
        liveTrainingProjectionPanel(model),
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

const settingsPane = (model: Model): Html => {
  const node = modelNode(model)
  const installReadiness = modelInstallReadiness(model)
  const schema = node?.schema ?? "—"
  return h.div(
    [],
    [
      paneTitle("Settings"),
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
      card("Notifications", [
        h.p(
          [cls("card-body")],
          [
            "Desktop OS notifications fire on new session state transitions (CL-30). No configuration required — notifications are sent automatically when a session changes state.",
          ],
        ),
      ]),
      card("Theme", [
        h.p([cls("card-body")], ["Dark (shared tokens)"]),
        emptyLine("Theme is read-only. All surfaces share the canonical dark token palette."),
      ]),
      card("Updates", [
        h.p(
          [cls("card-body")],
          [
            "Auto-update: BSDIFF feed (full / bsdiff / none). The desktop checks updates.openagents.com on startup. If a patch is available it applies a binary diff (bsdiff) for a smaller download; otherwise it fetches the full bundle.",
          ],
        ),
        emptyLine(
          "Update chooser: available actions are full, bsdiff, or none.",
        ),
      ]),
      card("About", [
        h.p([cls("card-body")], ["Autopilot Desktop"]),
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
      eventTimeline(model, events),
    ],
  )
}

// ── Pane router + top-level view ────────────────────────────────────────────────

// ── Network home (#5049) ──────────────────────────────────────────────────
// The fullscreen pylon-network visualization: the bezier graph (adapted to the
// live network) with stats overlaid. The center pylon's pulse / node tones are
// driven by live activity. Visual language:
// docs/autopilot-coder/2026-06-15-autopilot-home-network-visual-language.md
const networkStatNumber = (value: number): string => {
  const safe = Math.max(0, Math.floor(value))
  return safe.toLocaleString("en-US")
}

const networkRollText = (value: string): Html =>
  h.span([cls("stat-roll slot-text"), h.Key(value)], [
    ...Array.from(value).map((char, index) =>
      h.span([cls("char-slot"), h.Key(`${index}-${char}`)], [
        h.span([cls("char-sizer")], [char]),
        h.span([cls("char-face")], [char]),
      ]),
    ),
  ])

// A single overlaid stat. Values carry the homepage slot-text structure so
// updating numbers animate with the same digit-roll visual language (§5).
const networkStat = (label: string, value: string, hero = false): Html =>
  h.div([cls(hero ? "network-stat network-stat-hero" : "network-stat")], [
    h.span([cls("network-stat-value")], [networkRollText(value)]),
    h.span([cls("network-stat-label")], [label]),
  ])

const networkPane = (model: Model): Html => {
  const scene = projectPylonNetworkScene(modelPylonStats(model))
  const options = pylonNetworkVisualizationOptions(scene)
  const installReadiness = modelInstallReadiness(model)
  const canGoOnline = installReadiness?.builtInAgentReady ?? true

  const activityLabel = scene.dormant
    ? "network dormant"
    : scene.activityIntensity > 0.05
      ? "work in flight"
      : "online · idle"

  return h.div([cls("network-page")], [
    h.div([cls("network-scene")], [
      trainingRunView<Message>([cls("three-effect-network")], options),
      pylonDiamondsView<Message>(
        [
          cls("network-pylon-diamonds"),
          h.Style(`--network-activity:${scene.activityIntensity.toFixed(3)}`),
        ],
        scene.activityIntensity,
      ),
    ]),
    h.div([cls("network-overlay")], [
      h.section([cls("network-title")], [
        h.span([cls("network-eyebrow")], [activityLabel]),
        h.h1([cls("network-headline")], [
          networkRollText(networkStatNumber(scene.onlineNow)),
          " pylons online",
        ]),
        h.p([cls("network-subhead")], [
          scene.asOfLabel ? `as of ${scene.asOfLabel}` : "live network",
        ]),
        h.p([cls("network-health")], [
          installReadinessSummary(installReadiness),
        ]),
        h.button(
          [
            cls(
              "pointer-events-auto mt-4 w-fit border border-[var(--outline)] bg-black px-4 py-2 font-mono text-xs font-semibold uppercase tracking-[0.08em] text-[var(--primary)] hover:border-[var(--primary)] disabled:cursor-wait disabled:opacity-60",
            ),
            h.Type("button"),
            h.Disabled(model.builtInAgentPending || !canGoOnline),
            h.OnClick(ClickedStartBuiltInAgent()),
          ],
          [model.builtInAgentPending ? "Going online..." : "Go online"],
        ),
      ]),
      h.section([cls("network-stats")], [
        networkStat("working now", networkStatNumber(scene.sessionsOnlineNow), true),
        networkStat("sellable online", networkStatNumber(scene.sellableOnlineNow)),
        networkStat("wallet ready", networkStatNumber(scene.walletReadyNow)),
        networkStat("assignment ready", networkStatNumber(scene.assignmentReadyNow)),
        networkStat("seen · 24h", networkStatNumber(scene.seen24h)),
        networkStat("registered", networkStatNumber(scene.registeredTotal)),
        networkStat("sats settled · 24h", networkStatNumber(scene.satsSettled24h)),
        networkStat("sats settled · total", networkStatNumber(scene.satsSettledTotal)),
        networkStat(
          "training assigned",
          networkStatNumber(scene.trainingAssignedContributors),
        ),
        networkStat(
          "training accepted",
          networkStatNumber(scene.trainingAcceptedContributors),
        ),
        networkStat(
          "training progress",
          networkStatNumber(scene.trainingProgressContributors),
        ),
      ]),
    ]),
  ])
}

const paneView = (model: Model): Html => {
  switch (model.pane) {
    case "network":
      return networkPane(model)
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
    case "decisions":
      return decisionsPane(model)
    case "spawn":
      return spawnPane(model)
    case "settings":
      return settingsPane(model)
    case "session-detail":
      return sessionDetailPane(model)
  }
}

const rootView = (model: Model): Html => {
  // #5049: the network home is immersive — one fullscreen three-effect canvas
  // with the stat overlay and NOTHING else (no sidebar, no shell chrome).
  if (model.pane === "network") {
    return h.div([cls("app-shell app-shell-network")], [networkPane(model)])
  }
  const fullscreenTraining = model.pane === "training-fullscreen"
  return h.div(
    [cls("app-shell")],
    [
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
  title: "Autopilot Desktop",
  body: sanitizeTree(rootView(model)) as Html,
})
