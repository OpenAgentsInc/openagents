import {
  Accordion,
  Badge,
  Button,
  Card,
  GraphFigure,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  defineIntent,
  type GraphStatus,
  type TextView,
  type Tone,
  type View,
} from "@effect-native/core"
import { Schema } from "@effect-native/core/effect"
import type {
  SarahFleetOwnerProjection,
  SarahFleetProgress,
} from "../contracts/fleet-owner-projection.ts"
import {
  SARAH_OWNER_FLEET_INTERACTIVE,
  SARAH_OWNER_FLEET_READ_ONLY,
  type SarahOwnerFleetInteractionMode,
} from "./owner-fleet-interaction.ts"

// The domain packages and vendored Effect Native core currently resolve
// different Effect v4 beta builds. Intent schemas stay on the EN runtime and
// mirror the domain ref/action constraints exactly instead of unsafely casting
// schema ASTs across those two runtime copies.
const SarahFleetPublicRef = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const SarahFleetIssueRef = Schema.String.check(
  Schema.isMaxLength(128),
  Schema.isPattern(
    /^([A-Za-z0-9][A-Za-z0-9._-]*\/[A-Za-z0-9][A-Za-z0-9._-]*)?#\d+$/,
  ),
)
const SarahFleetApprovalWorkUnitRef = Schema.Union([
  SarahFleetPublicRef,
  SarahFleetIssueRef,
])
const SarahFleetRunControlAction = Schema.Literals([
  "pause",
  "resume",
  "drain",
  "stop",
])
const SarahFleetApprovalDecision = Schema.Literals(["allow", "deny"])

export const SarahFleetRunControlRequested = defineIntent(
  "SarahFleetRunControlRequested",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    action: SarahFleetRunControlAction,
  }),
)

export const SarahFleetWorkUnitOpened = defineIntent(
  "SarahFleetWorkUnitOpened",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    assignmentRef: SarahFleetPublicRef,
    workerRef: Schema.NullOr(SarahFleetPublicRef),
  }),
)

export const SarahFleetApprovalDecisionRequested = defineIntent(
  "SarahFleetApprovalDecisionRequested",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    approvalRef: SarahFleetPublicRef,
    workUnitRef: Schema.NullOr(SarahFleetApprovalWorkUnitRef),
    workerRef: Schema.NullOr(SarahFleetPublicRef),
    decision: SarahFleetApprovalDecision,
  }),
)

export const SarahFleetEvidenceOpened = defineIntent(
  "SarahFleetEvidenceOpened",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    assignmentRef: SarahFleetPublicRef,
    workerRef: Schema.NullOr(SarahFleetPublicRef),
    evidenceKind: Schema.Literals(["verification", "closeout"]),
    evidenceRef: SarahFleetPublicRef,
  }),
)

export const SarahFleetAuditToggled = defineIntent(
  "SarahFleetAuditToggled",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    assignmentRef: Schema.NullOr(SarahFleetPublicRef),
    workerRef: Schema.NullOr(SarahFleetPublicRef),
  }),
)

export const sarahFleetSupervisionIntents = [
  SarahFleetRunControlRequested,
  SarahFleetWorkUnitOpened,
  SarahFleetApprovalDecisionRequested,
  SarahFleetEvidenceOpened,
  SarahFleetAuditToggled,
] as const

export type SarahFleetSupervisionViewOptions = Readonly<{
  expandedAuditWorkUnitRefs?: ReadonlyArray<string>
  interactionMode?: SarahOwnerFleetInteractionMode
}>

type FleetWorkUnit = SarahFleetOwnerProjection["workUnits"][number]
type FleetWorker = SarahFleetOwnerProjection["workers"][number]
type FleetApproval = SarahFleetOwnerProjection["approvals"][number]

const text = (
  key: string,
  content: string,
  variant: TextView["variant"] = "body",
  color: TextView["color"] = "textPrimary",
): TextView => Text({ key, content, variant, color })

const humanize = (value: string): string => value.replaceAll("_", " ")

const sentenceCase = (value: string): string => {
  const words = humanize(value)
  return words.length === 0
    ? words
    : `${words[0]!.toUpperCase()}${words.slice(1)}`
}

const runTone = (status: SarahFleetOwnerProjection["run"]["status"]): Tone => {
  if (status === "completed") return "success"
  if (status === "paused" || status === "draining") return "warn"
  if (status === "stopped") return "neutral"
  return status === "running" ? "info" : "neutral"
}

const progressTone = (progress: SarahFleetProgress): Tone => {
  if (progress.status === "completed") return "success"
  if (progress.status === "blocked") {
    return progress.phase === "failed" || progress.phase === "circuit_broken"
      ? "danger"
      : "warn"
  }
  if (progress.status === "stalled") return "warn"
  if (progress.status === "fresh") return "info"
  return "neutral"
}

const progressLabel = (progress: SarahFleetProgress): string => {
  if (progress.status === "stalled") {
    const ageSeconds = Math.max(30, Math.floor(progress.ageMs / 1_000))
    return `Reconnecting · no fresh progress for ${ageSeconds}s`
  }
  if (progress.status === "fresh") {
    return `Fresh progress · ${Math.floor(progress.ageMs / 1_000)}s ago`
  }
  if (progress.status === "not_assigned") return "Not assigned"
  return progress.summary
}

const graphStatusForProgress = (progress: SarahFleetProgress): GraphStatus => {
  if (progress.status === "completed") return "success"
  if (progress.status === "blocked") {
    return progress.phase === "failed" || progress.phase === "circuit_broken"
      ? "failed"
      : "pending"
  }
  if (progress.status === "fresh") return "active"
  if (progress.status === "stalled") return "pending"
  if (progress.status === "not_assigned") return "idle"
  return progress.phase === "paused" ? "pending" : "idle"
}

const graphStatusForWorkUnit = (workUnit: FleetWorkUnit): GraphStatus => {
  if (
    workUnit.verification.status === "failed" ||
    workUnit.closeout.status === "rejected"
  ) {
    return "failed"
  }
  if (workUnit.closeout.status === "accepted") return "success"
  return graphStatusForProgress(workUnit.progress)
}

const graphStatusForRun = (
  status: SarahFleetOwnerProjection["run"]["status"],
): GraphStatus => {
  if (status === "completed") return "success"
  if (status === "stopped") return "idle"
  if (status === "running" || status === "draining") return "active"
  return status === "paused" ? "pending" : "idle"
}

const supervisionGraph = (projection: SarahFleetOwnerProjection): View => {
  const workNodeId = (workUnit: FleetWorkUnit): string =>
    `work:${workUnit.workUnitRef}`
  const workerNodeId = (workerRef: string): string => `worker:${workerRef}`
  const runNodeId = `run:${projection.run.runRef}`
  const workerByRef = new Map(
    projection.workers.map((worker) => [worker.workerRef, worker]),
  )
  const nodes = [
    {
      id: runNodeId,
      label: projection.run.name,
      kind: "arbiter" as const,
      status: graphStatusForRun(projection.run.status),
    },
    ...projection.workUnits.map((workUnit) => ({
      id: workNodeId(workUnit),
      label: workUnit.name,
      kind: "task" as const,
      status: graphStatusForWorkUnit(workUnit),
    })),
    ...projection.workers.map((worker) => ({
      id: workerNodeId(worker.workerRef),
      label: worker.name,
      kind: "worker" as const,
      status: graphStatusForProgress(worker.progress),
    })),
  ]
  const edges = projection.workUnits.flatMap((workUnit) => {
    const workId = workNodeId(workUnit)
    const runEdge = {
      id: `edge:${runNodeId}:${workId}`,
      from: runNodeId,
      to: workId,
      kind: "flow" as const,
      status: graphStatusForWorkUnit(workUnit),
    }
    if (workUnit.workerRef === null) return [runEdge]
    const worker = workerByRef.get(workUnit.workerRef)
    if (worker === undefined) return [runEdge]
    return [
      runEdge,
      {
        id: `edge:${workId}:${workerNodeId(worker.workerRef)}`,
        from: workId,
        to: workerNodeId(worker.workerRef),
        kind: "pairing" as const,
        status: graphStatusForProgress(worker.progress),
      },
    ]
  })

  return GraphFigure({
    key: `fleet-supervision-${projection.run.runRef}-graph`,
    nodes,
    edges,
    layout: "tree",
    height: 320,
    a11y: {
      role: "group",
      label: `${projection.run.name} map. ${projection.workUnits.length} work units and ${projection.workers.length} workers.`,
    },
    style: {
      width: "full",
      backgroundColor: "surface",
      borderRadius: "md",
    },
  })
}

const runControls = (
  projection: SarahFleetOwnerProjection,
  interactionMode: SarahOwnerFleetInteractionMode,
): View => {
  const interactive = interactionMode === SARAH_OWNER_FLEET_INTERACTIVE
  const controls = projection.run.availableControls.map((action) =>
    Button({
      key: `fleet-supervision-${projection.run.runRef}-control-${action}`,
      label: sentenceCase(action),
      variant:
        action === "pause" || action === "resume" ? "primary" : "secondary",
      onPress: IntentRef(
        SarahFleetRunControlRequested.name,
        StaticPayload({ runRef: projection.run.runRef, action }),
      ),
      a11y: {
        label: `${sentenceCase(action)} fleet run ${projection.run.name}`,
      },
    }),
  )

  return Stack(
    {
      key: `fleet-supervision-${projection.run.runRef}-controls`,
      direction: "column",
      gap: "2",
      a11y: {
        role: "group",
        label: "Run controls from the current fleet projection",
      },
      style: { width: "full" },
    },
    [
      text(
        `fleet-supervision-${projection.run.runRef}-controls-title`,
        "Run controls",
        "label",
      ),
      ...(!interactive
        ? [
            text(
              `fleet-supervision-${projection.run.runRef}-controls-unavailable`,
              "Controls unavailable in this surface. Run state remains visible and no authority is inferred.",
              "caption",
              "textMuted",
            ),
          ]
        : controls.length === 0
        ? [
            text(
              `fleet-supervision-${projection.run.runRef}-controls-empty`,
              "No run controls available for this state.",
              "caption",
              "textMuted",
            ),
          ]
        : [
            Stack(
              {
                key: `fleet-supervision-${projection.run.runRef}-control-buttons`,
                direction: { base: "column", sm: "row" },
                gap: "2",
                align: "start",
                style: { width: "full" },
              },
              controls,
            ),
          ]),
      text(
        `fleet-supervision-${projection.run.runRef}-control-authority`,
        interactive
          ? "Availability reflects durable run state. Authorization is checked when submitted."
          : "This surface is not connected to fleet control actions. The server must still recheck authority wherever actions are submitted.",
        "caption",
        "textMuted",
      ),
    ],
  )
}

const auditRows = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
): ReadonlyArray<View> => {
  const rows: Array<readonly [string, string]> = [
    ["Run", projection.run.runRef],
    ["Work unit", workUnit.workUnitRef],
    ["Worker", workUnit.workerRef ?? "not reported"],
    ["Verification", workUnit.verification.verificationRef ?? "not reported"],
    ["Closeout", workUnit.closeout.closeoutRef ?? "not reported"],
  ]
  if (workUnit.latestAttemptRef !== null) {
    rows.splice(2, 0, ["Latest attempt", workUnit.latestAttemptRef])
  }
  if (workUnit.acceptedAttemptRef !== null) {
    rows.splice(3, 0, ["Accepted attempt", workUnit.acceptedAttemptRef])
  }
  if (workUnit.assignmentRef !== null) {
    rows.splice(4, 0, ["Assignment", workUnit.assignmentRef])
  }
  workUnit.approvalRefs.forEach((approvalRef, index) => {
    rows.push([`Approval ${index + 1}`, approvalRef])
  })
  return rows.map(([label, value], index) =>
    Stack(
      {
        key: `fleet-supervision-${workUnit.workUnitRef}-audit-${index}`,
        direction: { base: "column", sm: "row" },
        gap: "1",
        align: "start",
        a11y: { role: "listitem", label: `${label}: ${value}` },
        style: { width: "full" },
      },
      [
        text(
          `fleet-supervision-${workUnit.workUnitRef}-audit-${index}-label`,
          label,
          "caption",
        ),
        text(
          `fleet-supervision-${workUnit.workUnitRef}-audit-${index}-value`,
          value,
          "caption",
          "textMuted",
        ),
      ],
    ),
  )
}

const auditDisclosure = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
  expanded: boolean,
): View =>
  Accordion({
    key: `fleet-supervision-${workUnit.workUnitRef}-audit`,
    mode: "single",
    expandedIds: expanded ? ["references"] : [],
    onToggle: IntentRef(
      SarahFleetAuditToggled.name,
      StaticPayload({
        runRef: projection.run.runRef,
        workUnitRef: workUnit.workUnitRef,
        assignmentRef: workUnit.assignmentRef,
        workerRef: workUnit.workerRef,
      }),
    ),
    items: [
      {
        id: "references",
        header: "Audit references",
        content: [
          Stack(
            {
              key: `fleet-supervision-${workUnit.workUnitRef}-audit-list`,
              direction: "column",
              gap: "1",
              padding: "2",
              a11y: {
                role: "list",
                label: `Audit references for ${workUnit.name}`,
              },
              style: { width: "full" },
            },
            auditRows(projection, workUnit),
          ),
        ],
      },
    ],
    a11y: {
      role: "group",
      label: `Audit references for ${workUnit.name}`,
      expanded,
    },
    style: { width: "full" },
  })

const evidenceButton = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
  evidenceKind: "verification" | "closeout",
  evidenceRef: string | null,
): ReadonlyArray<View> => {
  if (evidenceRef === null || workUnit.assignmentRef === null) return []
  const label = evidenceKind === "verification" ? "View verification" : "View closeout"
  return [
    Button({
      key: `fleet-supervision-${workUnit.workUnitRef}-${evidenceKind}`,
      label,
      variant: "ghost",
      onPress: IntentRef(
        SarahFleetEvidenceOpened.name,
        StaticPayload({
          runRef: projection.run.runRef,
          workUnitRef: workUnit.workUnitRef,
          assignmentRef: workUnit.assignmentRef,
          workerRef: workUnit.workerRef,
          evidenceKind,
          evidenceRef,
        }),
      ),
      a11y: { label: `${label} for ${workUnit.name}` },
    }),
  ]
}

const workUnitRow = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
  worker: FleetWorker | undefined,
  expanded: boolean,
  interactionMode: SarahOwnerFleetInteractionMode,
): View => {
  const workerLabel =
    worker === undefined
      ? workUnit.workerRef === null
        ? "Worker not assigned"
        : "Worker details not reported"
      : worker.name
  const verificationLabel =
    workUnit.verification.status === "not_reported"
      ? "Verification not reported"
      : workUnit.verification.summary

  return Stack(
    {
      key: `fleet-supervision-${workUnit.workUnitRef}`,
      direction: "column",
      gap: "2",
      padding: "3",
      a11y: {
        role: "listitem",
        label: `${workUnit.name}. ${workerLabel}. ${progressLabel(workUnit.progress)}. ${verificationLabel}. ${workUnit.closeout.summary}.`,
      },
      style: {
        width: "full",
        backgroundColor: "surface",
        borderRadius: "md",
      },
    },
    [
      Stack(
        {
          key: `fleet-supervision-${workUnit.workUnitRef}-heading`,
          direction: { base: "column", sm: "row" },
          gap: "2",
          align: "start",
          justify: "between",
          style: { width: "full" },
        },
        [
          Stack(
            {
              key: `fleet-supervision-${workUnit.workUnitRef}-identity`,
              direction: "column",
              gap: "0.5",
            },
            [
              text(
                `fleet-supervision-${workUnit.workUnitRef}-name`,
                workUnit.name,
                "label",
              ),
              text(
                `fleet-supervision-${workUnit.workUnitRef}-worker-name`,
                workerLabel,
                "caption",
                "textMuted",
              ),
            ],
          ),
          Badge({
            key: `fleet-supervision-${workUnit.workUnitRef}-progress`,
            label: progressLabel(workUnit.progress),
            tone: progressTone(workUnit.progress),
            a11y: { label: `Progress: ${progressLabel(workUnit.progress)}` },
          }),
        ],
      ),
      text(
        `fleet-supervision-${workUnit.workUnitRef}-summary`,
        workUnit.summary,
        "body",
        "textMuted",
      ),
      Stack(
        {
          key: `fleet-supervision-${workUnit.workUnitRef}-evidence-status`,
          direction: { base: "column", sm: "row" },
          gap: "1",
          align: "start",
          style: { width: "full" },
        },
        [
          Badge({
            key: `fleet-supervision-${workUnit.workUnitRef}-verification-status`,
            label: verificationLabel,
            tone:
              workUnit.verification.status === "ready"
                ? "info"
                : workUnit.verification.status === "failed"
                  ? "danger"
                  : "neutral",
          }),
          Badge({
            key: `fleet-supervision-${workUnit.workUnitRef}-closeout-status`,
            label: workUnit.closeout.summary,
            tone:
              workUnit.closeout.status === "accepted"
                ? "success"
                : workUnit.closeout.status === "rejected"
                  ? "danger"
                  : workUnit.closeout.status === "submitted"
                    ? "info"
                    : "neutral",
          }),
        ],
      ),
      ...(interactionMode === SARAH_OWNER_FLEET_INTERACTIVE &&
      workUnit.assignmentRef !== null
        ? [
            Stack(
              {
                key: `fleet-supervision-${workUnit.workUnitRef}-actions`,
                direction: { base: "column", sm: "row" },
                gap: "2",
                align: "start",
                style: { width: "full" },
              },
              [
                Button({
                  key: `fleet-supervision-${workUnit.workUnitRef}-open`,
                  label: "Open work unit",
                  variant: "secondary",
                  onPress: IntentRef(
                    SarahFleetWorkUnitOpened.name,
                    StaticPayload({
                      runRef: projection.run.runRef,
                      workUnitRef: workUnit.workUnitRef,
                      assignmentRef: workUnit.assignmentRef,
                      workerRef: workUnit.workerRef,
                    }),
                  ),
                  a11y: { label: `Open work unit ${workUnit.name}` },
                }),
                ...evidenceButton(
                  projection,
                  workUnit,
                  "verification",
                  workUnit.verification.verificationRef,
                ),
                ...evidenceButton(
                  projection,
                  workUnit,
                  "closeout",
                  workUnit.closeout.closeoutRef,
                ),
              ],
            ),
          ]
        : []),
      auditDisclosure(projection, workUnit, expanded),
    ],
  )
}

const approvalRow = (
  projection: SarahFleetOwnerProjection,
  approval: FleetApproval,
  interactionMode: SarahOwnerFleetInteractionMode,
): View => {
  const target =
    approval.workUnitRef === null
      ? "Work unit not reported"
      : `Work unit ${approval.workUnitRef}`
  const tool =
    approval.toolClass === null
      ? "Tool class not reported"
      : `Tool class: ${humanize(approval.toolClass)}`
  const decisions = approval.availableDecisions.map((decision) =>
    Button({
      key: `fleet-supervision-${approval.approvalRef}-${decision}`,
      label: sentenceCase(decision),
      variant: decision === "allow" ? "primary" : "secondary",
      onPress: IntentRef(
        SarahFleetApprovalDecisionRequested.name,
        StaticPayload({
          runRef: projection.run.runRef,
          approvalRef: approval.approvalRef,
          workUnitRef: approval.workUnitRef,
          workerRef: approval.workerRef,
          decision,
        }),
      ),
      a11y: {
        label: `${sentenceCase(decision)} approval for ${target.toLowerCase()}`,
      },
    }),
  )

  return Stack(
    {
      key: `fleet-supervision-${approval.approvalRef}`,
      direction: "column",
      gap: "2",
      padding: "3",
      a11y: {
        role: "listitem",
        label: `${approval.summary}. ${target}. ${tool}.`,
      },
      style: {
        width: "full",
        backgroundColor: "surface",
        borderRadius: "md",
      },
    },
    [
      Stack(
        {
          key: `fleet-supervision-${approval.approvalRef}-heading`,
          direction: { base: "column", sm: "row" },
          gap: "2",
          align: "start",
          justify: "between",
          style: { width: "full" },
        },
        [
          text(
            `fleet-supervision-${approval.approvalRef}-title`,
            "Approval needed",
            "label",
          ),
          Badge({
            key: `fleet-supervision-${approval.approvalRef}-status`,
            label: "Pending",
            tone: "warn",
          }),
        ],
      ),
      text(
        `fleet-supervision-${approval.approvalRef}-target`,
        `${target}. ${tool}.`,
        "body",
        "textMuted",
      ),
      ...(interactionMode === SARAH_OWNER_FLEET_READ_ONLY
        ? [
            text(
              `fleet-supervision-${approval.approvalRef}-decisions-unavailable`,
              "Controls unavailable in this surface. This approval remains pending and no decision was submitted.",
              "caption",
              "textMuted",
            ),
          ]
        : decisions.length === 0
        ? [
            text(
              `fleet-supervision-${approval.approvalRef}-decisions-empty`,
              "Decision options not reported.",
              "caption",
              "textMuted",
            ),
          ]
        : [
            Stack(
              {
                key: `fleet-supervision-${approval.approvalRef}-decisions`,
                direction: { base: "column", sm: "row" },
                gap: "2",
                align: "start",
              },
              decisions,
            ),
          ]),
      text(
        `fleet-supervision-${approval.approvalRef}-authority`,
        "The server rechecks owner scope and approval authority for every decision.",
        "caption",
        "textMuted",
      ),
    ],
  )
}

/**
 * Pure FC-3 Effect Native supervision view over an owner-scoped projection.
 *
 * This component only emits typed intent data. It does not fetch, authorize,
 * infer controls, open private logs, or compose steer bodies. The host must
 * bind the exported intent definitions to the authenticated fleet-intent
 * boundary. Exact refs stay in callback payloads and collapsed audit detail.
 * A steer affordance is intentionally absent: the projection needs an
 * additive `workUnits[].availableActions: Array<"steer">` field (typed by a
 * shared action enum) before the UI can present steering without inferring
 * authority from worker state.
 */
export function sarahFleetRunSupervisionView(
  projection: SarahFleetOwnerProjection,
  options: SarahFleetSupervisionViewOptions = {},
): View {
  const interactionMode =
    options.interactionMode ?? SARAH_OWNER_FLEET_READ_ONLY
  const workerByRef = new Map(
    projection.workers.map((worker) => [worker.workerRef, worker]),
  )
  const expanded = new Set(options.expandedAuditWorkUnitRefs ?? [])
  const pendingApprovals = projection.approvals.filter(
    (approval) => approval.status === "pending",
  )

  return Card(
    {
      key: `fleet-supervision-${projection.run.runRef}`,
      padding: "4",
      radius: "lg",
      a11y: {
        role: "region",
        label: `${projection.run.name}. ${sentenceCase(projection.run.status)}. ${projection.workUnits.length} work units. ${pendingApprovals.length} pending approvals.`,
      },
      style: {
        width: "full",
        backgroundColor: "surfaceRaised",
        borderColor: "border",
        borderWidth: 1,
      },
    },
    [
      Stack(
        {
          key: `fleet-supervision-${projection.run.runRef}-header`,
          direction: { base: "column", sm: "row" },
          gap: "2",
          align: "start",
          justify: "between",
          style: { width: "full" },
        },
        [
          Stack(
            {
              key: `fleet-supervision-${projection.run.runRef}-identity`,
              direction: "column",
              gap: "0.5",
            },
            [
              text(
                `fleet-supervision-${projection.run.runRef}-title`,
                projection.run.name,
                "title",
              ),
              text(
                `fleet-supervision-${projection.run.runRef}-ref`,
                `Run identity: ${projection.run.runRef}`,
                "caption",
                "textMuted",
              ),
            ],
          ),
          Badge({
            key: `fleet-supervision-${projection.run.runRef}-status`,
            label: sentenceCase(projection.run.status),
            tone: runTone(projection.run.status),
            a11y: {
              label: `Fleet run status: ${sentenceCase(projection.run.status)}`,
            },
          }),
        ],
      ),
      text(
        `fleet-supervision-${projection.run.runRef}-summary`,
        `${projection.run.counters.activeAssignments} active · ${projection.run.counters.completedAssignments} complete · ${projection.run.counters.blockedAssignments} blocked`,
        "body",
        "textMuted",
      ),
      supervisionGraph(projection),
      runControls(projection, interactionMode),
      Stack(
        {
          key: `fleet-supervision-${projection.run.runRef}-work-units`,
          direction: "column",
          gap: "2",
          a11y: { role: "list", label: "Fleet work units" },
          style: { width: "full" },
        },
        [
          text(
            `fleet-supervision-${projection.run.runRef}-work-units-title`,
            "Work units",
            "label",
          ),
          ...(projection.workUnits.length === 0
            ? [
                text(
                  `fleet-supervision-${projection.run.runRef}-work-units-empty`,
                  "No work units reported.",
                  "caption",
                  "textMuted",
                ),
              ]
            : projection.workUnits.map((workUnit) =>
                workUnitRow(
                  projection,
                  workUnit,
                  workUnit.workerRef === null
                    ? undefined
                    : workerByRef.get(workUnit.workerRef),
                  expanded.has(workUnit.workUnitRef),
                  interactionMode,
                ),
              )),
        ],
      ),
      Stack(
        {
          key: `fleet-supervision-${projection.run.runRef}-approvals`,
          direction: "column",
          gap: "2",
          a11y: { role: "list", label: "Pending fleet approvals" },
          style: { width: "full" },
        },
        [
          text(
            `fleet-supervision-${projection.run.runRef}-approvals-title`,
            "Pending approvals",
            "label",
          ),
          ...(pendingApprovals.length === 0
            ? [
                text(
                  `fleet-supervision-${projection.run.runRef}-approvals-empty`,
                  "No pending approvals.",
                  "caption",
                  "textMuted",
                ),
              ]
            : pendingApprovals.map((approval) =>
                approvalRow(projection, approval, interactionMode),
              )),
        ],
      ),
    ],
  )
}
