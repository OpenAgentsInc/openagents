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
import {
  bezierNodesView,
  spinningCubeView,
  trainingRunView,
} from "@openagentsinc/three-effect/foldkit"
import {
  trainingRunVisualizationOptionsFromSnapshot,
  type TrainingRunVisualizationOptions,
} from "@openagentsinc/three-effect/core"
import type { Attribute, Document, Html } from "foldkit/html"
import { html } from "foldkit/html"

import { chooseUpdate } from "../shared/update-feed"
import type {
  AccountRow,
  ApprovalRow,
  AssignmentRow,
  IntentRow,
  NodeStateMessage,
  SessionEventRow,
  TrainingRunSummaryRow,
  TrainingRunsResponse,
} from "../shared/rpc"
import {
  ChangedAskBody,
  ChangedAskTitle,
  ChangedSessionFilter,
  ChangedSpawnAdapter,
  ChangedSpawnObjective,
  ChangedSpawnVerify,
  ClickedCancelSession,
  ClickedCoordinatorToggle,
  ClickedDeploy,
  ClickedRefreshTrainingRuns,
  ClickedQueueTrainingLaunch,
  ClickedResolveApproval,
  ClickedSpawn,
  ClickedSubmitIntent,
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
  verifyLineText,
  walletSummary,
} from "./helpers"
import {
  type Model,
  type PaneId,
  type SessionFilter,
  modelNode,
  modelNotifications,
  modelTrainingRuns,
} from "./model"

const h = html<Message>()
const cls = (value: string): Attribute<Message> => h.Class(value)

// ── Small shared building blocks (own h.* — no hand DOM) ─────────────────────

const card = (title: string, children: ReadonlyArray<Html>): Html =>
  h.section([cls("card")], [h.h2([cls("card-title")], [title]), ...children])

const emptyLine = (text: string): Html => h.p([cls("empty-state")], [text])

const paneTitle = (text: string): Html => h.h1([cls("pane-title")], [text])

const liveScenePanel = (): Html =>
  h.section(
    [cls("three-effect-panel")],
    [
      h.header(
        [cls("three-effect-header")],
        [
          h.h2([cls("three-effect-title")], ["Live Scene"]),
          h.p([cls("three-effect-caption")], ["spinning cube"]),
        ],
      ),
      spinningCubeView<Message>([cls("three-effect-cube")]),
    ],
  )

const bezierNodesPanel = (): Html =>
  h.section(
    [cls("three-effect-panel")],
    [
      h.header(
        [cls("three-effect-header")],
        [
          h.h2([cls("three-effect-title")], ["Bezier Nodes"]),
          h.p([cls("three-effect-caption")], ["drag graph"]),
        ],
      ),
      bezierNodesView<Message>([cls("three-effect-bezier")]),
    ],
  )

const threeEffectPreview = (): Html =>
  h.div([cls("three-effect-grid")], [liveScenePanel(), bezierNodesPanel()])

// ── Sidebar ──────────────────────────────────────────────────────────────────

const NAV: ReadonlyArray<{ id: PaneId; label: string }> = [
  { id: "nodes", label: "Nodes" },
  { id: "training", label: "Training" },
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
      threeEffectPreview(),
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

const trainingProjectionMeta = (
  projection: TrainingRunsResponse | null,
): string => {
  if (projection === null) return "not loaded"
  const when =
    projection.fetchedAt.length > 0
      ? new Date(projection.fetchedAt).toLocaleTimeString()
      : "unknown time"
  return projection.ok
    ? `${projection.runs.length} runs · fetched ${when}`
    : `unavailable · ${projection.error ?? "fetch failed"}`
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

const trainingSceneOptions = (
  projection: TrainingRunsResponse | null,
): TrainingRunVisualizationOptions | undefined => {
  const summary = selectedTrainingSummary(projection)
  if (summary === null) return undefined
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
    lossUnderBudget: realGradient.lossUnderBudget.satisfied,
    maxAllowedStaleSteps: summary.run.maxAllowedStale,
    maxValidationLoss: realGradient.lossUnderBudget.maxValidationLoss,
    plannedWindowCount: metrics.plannedWindowCount.value,
    reconciledWindowCount: metrics.reconciledWindowCount.value,
    rejectedWorkCount: metrics.rejectedWorkCount.value,
    runDetail: summary.run.trainingRunRef,
    runLabel: summary.run.promiseRef,
    runState: summary.run.state,
    sealedWindowCount: metrics.sealedWindowCount.value,
    settledPayoutSats: metrics.providerConfirmedSettledPayoutSats.value,
    verifiedWorkCount: metrics.verifiedWorkCount.value,
  })
}

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

const trainingLaunchPanel = (model: Model): Html => {
  const statusVisible = model.trainingLaunchStatus.tone !== "idle"
  return h.section([cls("training-panel training-action-panel")], [
    h.h2([cls("training-panel-title")], ["Launch Feedback"]),
    h.p(
      [cls("training-panel-copy")],
      [
        "Queues a local Pylon intent to inspect the run gates. Admin planning and admission still belong behind authenticated training routes.",
      ],
    ),
    h.button(
      [
        cls("training-action-button"),
        h.Type("button"),
        h.Disabled(model.trainingLaunchPending),
        h.OnClick(ClickedQueueTrainingLaunch()),
      ],
      [model.trainingLaunchPending ? "Queueing..." : "Queue launch check"],
    ),
    statusVisible
      ? h.p(
          [
            cls(
              `training-action-status training-${model.trainingLaunchStatus.tone}`,
            ),
          ],
          [model.trainingLaunchStatus.text],
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
              "Tassadar/Psion run projection · Pluralis lifecycle adapted by issue 4855",
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
          trainingSceneOptions(projection),
        ),
      ]),
      h.div([cls("training-grid")], [
        liveTrainingProjectionPanel(model),
        selectedTrainingEvidencePanel(projection),
        h.section([cls("training-panel")], [
          h.h2([cls("training-panel-title")], ["Issue 4855 Gates"]),
          h.ul([cls("training-gates")], [
            trainingGate("join lifecycle", "registered -> active", "ready"),
            trainingGate("bootstrap", "last durable seal only", "ready"),
            trainingGate("seal barrier", "queue joins during merge", "watch"),
            trainingGate("staleness", "sync reentry beyond bound", "watch"),
            trainingGate("receipts", "presence split from compute", "ready"),
            trainingGate("SPARTA", "side canary only", "blocked"),
          ]),
        ]),
        trainingLaunchPanel(model),
        h.section([cls("training-panel")], [
          h.h2([cls("training-panel-title")], ["Live API Boundary"]),
          h.p(
            [cls("training-panel-copy")],
            [
              "Read projections from public training summaries. Put authenticated plan, admit, and evidence writes in the Bun main process or Worker admin surface, never in the webview.",
            ],
          ),
          h.ul([cls("training-api-list")], [
            h.li([], [h.code([], ["/api/training/runs"])]),
            h.li([], [h.code([], ["/api/training/leaderboards"])]),
            h.li([], [h.code([], ["admin: plan window / admit evidence"])]),
          ]),
        ]),
      ]),
    ],
  )
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

// ── Spawn pane ────────────────────────────────────────────────────────────────

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

const settingsPane = (model: Model): Html => {
  const node = modelNode(model)
  const schema = node?.schema ?? "—"
  const exampleChoice = chooseUpdate("0.0.0", [])
  return h.div(
    [],
    [
      paneTitle("Settings"),
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
          `Update chooser: available actions are full, bsdiff, or none (current: ${exampleChoice.action}).`,
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

const paneView = (model: Model): Html => {
  switch (model.pane) {
    case "nodes":
      return nodesPane(model)
    case "training":
      return trainingPane(model)
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

const rootView = (model: Model): Html =>
  h.div(
    [cls("app-shell")],
    [sidebar(model), h.main([cls("content")], [h.div([cls("pane")], [paneView(model)])])],
  )

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
