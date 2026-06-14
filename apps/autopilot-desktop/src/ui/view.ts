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
  type TrainingRunOperatorSignalDefinition,
  type TrainingRunOperatorSignalState,
  type TrainingRunPromiseSignalDefinition,
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
  TrainingLeaderboardLaneSummary,
  TrainingOperatorReadinessResponse,
  TrainingPromiseState,
  TrainingPromiseSummary,
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
  ClickedActivateTrainingWindow,
  ClickedClaimTrainingLease,
  ClickedCoordinatorToggle,
  ClickedDeploy,
  ClickedPlanTrainingWindow,
  ClickedQueueTrainingCloseout,
  ClickedReconcileTrainingWindow,
  ClickedRefreshTrainingRuns,
  ClickedQueueTrainingLaunch,
  ClickedResolveApproval,
  ClickedRequestTrainingBootstrap,
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
  modelTrainingActivation,
  modelTrainingBootstrap,
  modelTrainingDashboard,
  modelTrainingLease,
  modelNode,
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
          h.p([cls("three-effect-caption")], ["pmndrs/drei port"]),
        ],
      ),
      bezierNodesView<Message>([cls("three-effect-bezier")]),
    ],
  )

const threeEffectPreview = (): Html =>
  h.div([cls("three-effect-grid")], [liveScenePanel(), bezierNodesPanel()])

const threeEffectSourceCard = (): Html =>
  card("three-effect sources", [
    h.p([cls("card-subtitle")], [
      "Owned Effect/Foldkit package plus the local reference files used for the Bezier port.",
    ]),
    h.ul([cls("three-effect-source-list")], [
      h.li([], [
        h.code([], ["OpenAgentsInc/three-effect:examples/bezier-nodes/"]),
      ]),
      h.li([], [
        h.code([], ["OpenAgentsInc/three-effect:packages/core/src/bezierNodes.ts"]),
      ]),
      h.li([], [
        h.code([], [
          "projects/repos/examples/demos/bezier-curves-and-nodes/src/Nodes.jsx",
        ]),
      ]),
      h.li([], [
        h.code([], ["projects/repos/drei/src/core/QuadraticBezierLine.tsx"]),
      ]),
      h.li([], [
        h.code([], ["projects/repos/drei/src/web/DragControls.tsx"]),
      ]),
    ]),
  ])

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
      threeEffectSourceCard(),
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

const trainingRoadmapItems: ReadonlyArray<{
  phase: string
  issue: string
  title: string
  status: string
  tone: TrainingGateTone
}> = [
  {
    phase: "P0.1",
    issue: "#4848",
    title: "typed contributor ladder",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P0.2",
    issue: "#4849",
    title: "seal staleness + maxAllowedStale",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P0.3",
    issue: "psionic#1124",
    title: "Pluralis mechanism ledger",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P1.1",
    issue: "psionic#1125",
    title: "shadow-window ramp",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P1.2",
    issue: "#4850",
    title: "bootstrap from durable seal",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P1.3",
    issue: "#4851",
    title: "join blocking during seal",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P1.4",
    issue: "#4852",
    title: "hardware admission gates",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P2.1",
    issue: "psionic#1126",
    title: "collective failure semantics",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P2.2",
    issue: "#4853",
    title: "staleness-priced acceptance",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P2.3",
    issue: "#4854",
    title: "presence/compute receipt split",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P2.4",
    issue: "psionic#1127",
    title: "SPARTA side canary",
    status: "harness closed, run pending",
    tone: "watch",
  },
  {
    phase: "P3.2",
    issue: "psionic#1128",
    title: "PowerSGD x Freivalds question",
    status: "closed",
    tone: "ready",
  },
  {
    phase: "P3.1",
    issue: "unfiled",
    title: "pipeline-stage sharding",
    status: "wait for R2 economics",
    tone: "watch",
  },
]

const trainingRoadmapRefs: readonly string[] = [
  "GitHub OpenAgentsInc/openagents#4855",
  "docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md",
  "docs/promises/product-promises.json",
  "training.model_ladder.v1",
  "training.marathon_operations.v1",
  "training.public_distributed_training_run.v1",
]

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

const trainingSourceMapSections: ReadonlyArray<{
  title: string
  detail: string
  refs: readonly string[]
}> = [
  {
    title: "Worker authority",
    detail: "run/window state, transitions, public projection",
    refs: [
      "apps/openagents.com/workers/api/src/training-run-window-authority.ts",
      "apps/openagents.com/workers/api/src/training-run-window-routes.ts",
      "/api/training/runs",
      "/api/training/windows/{windowRef}/activate",
      "/api/training/windows/{windowRef}/reconcile",
    ],
  },
  {
    title: "Evidence and gates",
    detail: "real-gradient claim boundary, bootstrap, receipts, dashboards",
    refs: [
      "apps/openagents.com/workers/api/src/training-real-gradient-evidence.ts",
      "apps/openagents.com/workers/api/src/training-window-bootstrap.ts",
      "apps/openagents.com/workers/api/src/training-presence-compute-receipts.ts",
      "apps/openagents.com/workers/api/src/training-leaderboards.ts",
      "/api/training/runs/{runRef}/bootstrap-grant",
    ],
  },
  {
    title: "Desktop bridge",
    detail: "Bun-held authority, Foldkit messages, public-safe model",
    refs: [
      "apps/autopilot-desktop/src/bun/training-runs.ts",
      "apps/autopilot-desktop/src/bun/index.ts",
      "apps/autopilot-desktop/src/shared/rpc.ts",
      "apps/autopilot-desktop/src/ui/commands.ts",
      "apps/autopilot-desktop/src/ui/view.ts",
    ],
  },
  {
    title: "Three scene",
    detail: "Effect-scoped WebGL scene and Foldkit custom element",
    refs: [
      "OpenAgentsInc/three-effect:packages/core/src/trainingRun.ts",
      "OpenAgentsInc/three-effect:packages/foldkit/src/index.ts",
      "OpenAgentsInc/three-effect:examples/training-run/",
      "@openagentsinc/three-effect/foldkit",
      "oa-training-run",
    ],
  },
  {
    title: "Training docs",
    detail: "Pluralis adaptation, Psion plan, Tassadar research source",
    refs: [
      "docs/training/2026-06-12-pluralis-to-pylon-adaptation-roadmap.md",
      "docs/training/2026-06-10-psion-full-pipeline-buildout-plan.md",
      "docs/training/2026-06-14-autopilot-desktop-training-ui-audit.md",
      "docs/tassadar/RESEARCH_PLAN.md",
    ],
  },
]

const trainingSourceMapPanel = (): Html =>
  h.section([cls("training-panel training-source-map-panel")], [
    h.h2([cls("training-panel-title")], ["Source Map"]),
    h.p([cls("training-panel-copy")], [
      "Implementation homes for the live Training pane, grouped by authority boundary.",
    ]),
    h.ul(
      [cls("training-source-map-list")],
      trainingSourceMapSections.map(section =>
        h.li([cls("training-source-map-section")], [
          h.div([cls("training-source-map-heading")], [
            h.strong([], [section.title]),
            h.span([], [section.detail]),
          ]),
          h.ul(
            [cls("training-api-list training-source-map-refs")],
            section.refs.map(ref => h.li([], [h.code([], [ref])])),
          ),
        ]),
      ),
    ),
  ])

type TrainingControlSurfaceRow = Readonly<{
  action: string
  authority: string
  dispatch: string
  route: string
  rpc: string
  source: string
  statusField: string
  status: (model: Model) => TrainingStatusLike
  pending: (model: Model) => boolean
  current: (model: Model) => string
}>

const trainingControlSurfaceRows: readonly TrainingControlSurfaceRow[] = [
  {
    action: "Refresh projections",
    authority: "public Worker reads through Bun",
    dispatch: "ClickedRefreshTrainingRuns",
    route:
      "GET /api/training/runs + dashboards + promises + desktop readiness",
    rpc:
      "listTrainingRuns + listTrainingDashboard + listTrainingPromiseGates + listTrainingOperatorReadiness",
    source: "apps/autopilot-desktop/src/bun/index.ts + training-runs.ts",
    statusField:
      "trainingRunsStatus / trainingDashboardStatus / trainingPromiseGatesStatus / trainingOperatorReadinessStatus",
    status: model => model.trainingRunsStatus,
    pending: model =>
      model.trainingRunsPending ||
      model.trainingDashboardPending ||
      model.trainingPromiseGatesPending ||
      model.trainingOperatorReadinessPending,
    current: model =>
      [
        trainingStatusText(model.trainingRunsStatus, "runs idle"),
        trainingStatusText(model.trainingDashboardStatus, "dashboards idle"),
        trainingStatusText(model.trainingPromiseGatesStatus, "promises idle"),
        trainingStatusText(model.trainingOperatorReadinessStatus, "operator idle"),
      ].join(" / "),
  },
  {
    action: "Plan R1 window",
    authority: "Bun admin env gate",
    dispatch: "ClickedPlanTrainingWindow",
    route: "POST /api/training/runs -> POST /api/training/windows/plan",
    rpc: "planTrainingRunWindow",
    source: "apps/autopilot-desktop/src/bun/training-runs.ts",
    statusField: "trainingPlanStatus",
    status: model => model.trainingPlanStatus,
    pending: model => model.trainingPlanPending,
    current: model => {
      const plan = modelTrainingPlan(model)
      return plan?.windowRef ?? plan?.trainingRunRef ?? "no planned ref"
    },
  },
  {
    action: "Activate window",
    authority: "Bun admin env gate",
    dispatch: "ClickedActivateTrainingWindow",
    route: "POST /api/training/windows/{windowRef}/activate",
    rpc: "activateTrainingWindow",
    source: "apps/autopilot-desktop/src/bun/training-runs.ts",
    statusField: "trainingActivationStatus",
    status: model => model.trainingActivationStatus,
    pending: model => model.trainingActivationPending,
    current: model =>
      modelTrainingActivation(model)?.windowRef ??
      activationWindowRef(model) ??
      "no planned window",
  },
  {
    action: "Claim lease",
    authority: "Bun lease env gate + public Pylon ref",
    dispatch: "ClickedClaimTrainingLease",
    route: "POST /api/training/leases/claim",
    rpc: "claimTrainingWindowLease",
    source: "apps/autopilot-desktop/src/bun/index.ts",
    statusField: "trainingLeaseStatus",
    status: model => model.trainingLeaseStatus,
    pending: model => model.trainingLeasePending,
    current: model =>
      modelTrainingLease(model)?.lease?.leaseRef ??
      (hasClaimableTrainingWindow(model) ? "claimable window" : "no active window"),
  },
  {
    action: "Request bootstrap",
    authority: "public joiner bootstrap gate",
    dispatch: "ClickedRequestTrainingBootstrap",
    route: "POST /api/training/runs/{runRef}/bootstrap-grant",
    rpc: "requestTrainingBootstrapGrant",
    source: "apps/autopilot-desktop/src/bun/training-runs.ts",
    statusField: "trainingBootstrapStatus",
    status: model => model.trainingBootstrapStatus,
    pending: model => model.trainingBootstrapPending,
    current: model => {
      const bootstrap = modelTrainingBootstrap(model)
      return bootstrap?.outcome?.kind === "granted"
        ? bootstrap.outcome.grant.grantRef
        : bootstrap?.outcome?.kind ??
            selectedTrainingSummary(modelTrainingRuns(model))?.run.trainingRunRef ??
            "no run selected"
    },
  },
  {
    action: "Queue closeout packet",
    authority: "local Pylon intent, no evidence admission",
    dispatch: "ClickedQueueTrainingCloseout",
    route: "intent.submit",
    rpc: "submitIntent",
    source: "apps/autopilot-desktop/src/ui/commands.ts",
    statusField: "trainingCloseoutStatus",
    status: model => model.trainingCloseoutStatus,
    pending: model => model.trainingCloseoutPending,
    current: model =>
      selectedTrainingSummary(modelTrainingRuns(model))?.run.trainingRunRef ??
      modelTrainingPlan(model)?.trainingRunRef ??
      "no run selected",
  },
  {
    action: "Reconcile sealed window",
    authority: "Bun admin env gate",
    dispatch: "ClickedReconcileTrainingWindow",
    route: "POST /api/training/windows/{windowRef}/reconcile",
    rpc: "reconcileTrainingWindow",
    source: "apps/autopilot-desktop/src/bun/training-runs.ts",
    statusField: "trainingReconcileStatus",
    status: model => model.trainingReconcileStatus,
    pending: model => model.trainingReconcilePending,
    current: model =>
      modelTrainingReconcile(model)?.windowRef ??
      reconcileWindowRef(model) ??
      "no sealed window",
  },
  {
    action: "Queue launch check",
    authority: "local Pylon readiness intent",
    dispatch: "ClickedQueueTrainingLaunch",
    route: "intent.submit",
    rpc: "submitIntent",
    source: "apps/autopilot-desktop/src/ui/commands.ts",
    statusField: "trainingLaunchStatus",
    status: model => model.trainingLaunchStatus,
    pending: model => model.trainingLaunchPending,
    current: () => "issue 4855 gate inspection",
  },
]

const trainingControlSurfacePanel = (model: Model): Html =>
  h.section([cls("training-panel training-control-surface-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Control Surface"]),
      h.span([cls("training-panel-kicker")], ["button -> RPC -> authority"]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Each operator control mapped to its webview message, Bun RPC, authority route, current feedback field, and source file.",
    ]),
    h.ul(
      [cls("training-control-surface-list")],
      trainingControlSurfaceRows.map(row => {
        const status = row.status(model)
        const tone = trainingStatusTone(status, row.pending(model))
        return h.li([cls(`training-control-surface-row training-${tone}`)], [
          h.div([cls("training-control-surface-head")], [
            h.strong([], [row.action]),
            h.span([], [trainingStatusText(status, "idle")]),
          ]),
          h.p([cls("training-control-surface-current")], [row.current(model)]),
          h.ul([cls("training-api-list training-control-surface-refs")], [
            h.li([], [h.code([], [row.dispatch])]),
            h.li([], [h.code([], [row.rpc])]),
            h.li([], [h.code([], [row.route])]),
            h.li([], [h.code([], [row.statusField])]),
            h.li([], [row.authority]),
            h.li([], [h.code([], [row.source])]),
          ]),
        ])
      }),
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

const trainingRoadmapPanel = (): Html =>
  h.section([cls("training-panel training-roadmap-panel")], [
    h.div([cls("training-panel-heading")], [
      h.h2([cls("training-panel-title")], ["Issue 4855 Ledger"]),
      h.span([cls("training-panel-kicker")], ["closed; receipts move to registry"]),
    ]),
    h.p([cls("training-panel-copy")], [
      "Pluralis-to-Pylon child work by phase, with live R1/R2 hardware and settlement evidence now owned by the training promise registry.",
    ]),
    h.ul(
      [cls("training-gates training-roadmap-gates")],
      [
        trainingGate(
          "acceptance owner",
          "training.* promises own live receipts",
          "watch",
        ),
        ...trainingRoadmapItems.map(item =>
          trainingGate(
            `${item.phase} ${item.issue}`,
            `${item.status} · ${item.title}`,
            item.tone,
          ),
        ),
      ],
    ),
    h.ul(
      [cls("training-api-list training-roadmap-refs")],
      trainingRoadmapRefs.map(ref => h.li([], [h.code([], [ref])])),
    ),
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

const trainingOperatorFeedPanel = (model: Model): Html => {
  const runs = modelTrainingRuns(model)
  const dashboard = modelTrainingDashboard(model)
  const gates = modelTrainingPromiseGates(model)
  const readiness = modelTrainingOperatorReadiness(model)
  const plan = modelTrainingPlan(model)
  const activation = modelTrainingActivation(model)
  const reconcile = modelTrainingReconcile(model)
  const lease = modelTrainingLease(model)
  const bootstrap = modelTrainingBootstrap(model)

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
        "Plan, activate, claim, bootstrap, reconcile, and queue closeout prep through Bun and local Pylon.",
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
        trainingRoadmapPanel(),
        trainingLaunchPanel(model),
        trainingOperatorReadinessPanel(model),
        trainingOperatorFeedPanel(model),
        trainingControlSurfacePanel(model),
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
            h.li([], [
              h.code([], ["/api/training/runs/{runRef}/bootstrap-grant"]),
            ]),
            h.li([], [h.code([], ["admin: plan window / admit evidence"])]),
          ]),
        ]),
        trainingSourceMapPanel(),
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
