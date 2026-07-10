import {
  Accordion,
  Badge,
  Button,
  Card,
  ComponentValueBinding,
  GraphFigure,
  IntentRef,
  Stack,
  StaticPayload,
  Text,
  TextField,
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

/** Local-only canvas navigation. The handler resolves this opaque node id
 * against the current exact-run projection before selecting anything. */
export const SarahFleetNodeSelected = defineIntent(
  "SarahFleetNodeSelected",
  Schema.String.check(
    Schema.isMaxLength(280),
    Schema.isPattern(
      /^(run|work|attempt|assignment|verification|closeout|worker):[A-Za-z0-9][A-Za-z0-9._:-]*$/,
    ),
  ),
)

export const SarahFleetDrilldownClosed = defineIntent(
  "SarahFleetDrilldownClosed",
  Schema.Null,
)

export const SARAH_FLEET_MAX_STEER_BODY_LENGTH = 4_096

/** Private local draft input. Intent history redacts this payload in main.ts. */
export const SarahFleetSteerDraftChanged = defineIntent(
  "SarahFleetSteerDraftChanged",
  Schema.String.check(
    Schema.isMaxLength(SARAH_FLEET_MAX_STEER_BODY_LENGTH),
  ),
)

/** Body-free submit action; the host reads and clears the in-memory draft. */
export const SarahFleetSteerSubmitted = defineIntent(
  "SarahFleetSteerSubmitted",
  Schema.Struct({
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    attemptRef: SarahFleetPublicRef,
  }),
)

export const sarahFleetSupervisionIntents = [
  SarahFleetRunControlRequested,
  SarahFleetWorkUnitOpened,
  SarahFleetApprovalDecisionRequested,
  SarahFleetEvidenceOpened,
  SarahFleetAuditToggled,
  SarahFleetNodeSelected,
  SarahFleetDrilldownClosed,
  SarahFleetSteerDraftChanged,
  SarahFleetSteerSubmitted,
] as const

export const SarahFleetSelectedNode = Schema.Union([
  Schema.Struct({
    kind: Schema.Literal("run"),
    runRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("work_unit"),
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("attempt"),
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    attemptRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("assignment"),
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    attemptRef: SarahFleetPublicRef,
    assignmentRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("verification"),
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    attemptRef: SarahFleetPublicRef,
    verificationRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("closeout"),
    runRef: SarahFleetPublicRef,
    workUnitRef: SarahFleetPublicRef,
    attemptRef: SarahFleetPublicRef,
    closeoutRef: SarahFleetPublicRef,
  }),
  Schema.Struct({
    kind: Schema.Literal("worker"),
    runRef: SarahFleetPublicRef,
    workerRef: SarahFleetPublicRef,
  }),
])
export type SarahFleetSelectedNode = typeof SarahFleetSelectedNode.Type

export type SarahFleetHostSubmission = Readonly<{
  submissionRef: string
  intentId: string | null
  kind: "fleet_run_control" | "approval_decision" | "steer_message"
  targetRef: string
  status: "requested" | "failed"
  summary: string
}>

export type SarahFleetDrilldownViewOptions = Readonly<{
  interactionMode?: SarahOwnerFleetInteractionMode
  steerDraft?: string
}>

export type SarahFleetSupervisionViewOptions = Readonly<{
  expandedAuditWorkUnitRefs?: ReadonlyArray<string>
  interactionMode?: SarahOwnerFleetInteractionMode
  hostSubmissions?: ReadonlyArray<SarahFleetHostSubmission>
}>

type FleetWorkUnit = SarahFleetOwnerProjection["workUnits"][number]
type FleetWorker = SarahFleetOwnerProjection["workers"][number]
type FleetApproval = SarahFleetOwnerProjection["approvals"][number]
type FleetAttempt = FleetWorkUnit["attempts"][number]
type FleetCommandOutcome = NonNullable<
  SarahFleetOwnerProjection["commandOutcomes"]
>[number]

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

const runNodeId = (projection: SarahFleetOwnerProjection): string =>
  `run:${projection.run.runRef}`
const workNodeId = (workUnit: FleetWorkUnit): string =>
  `work:${workUnit.workUnitRef}`
const attemptNodeId = (attempt: FleetAttempt): string =>
  `attempt:${attempt.attemptRef}`
const assignmentNodeId = (attempt: FleetAttempt): string =>
  `assignment:${attempt.attemptRef}`
const verificationNodeId = (attempt: FleetAttempt): string =>
  `verification:${attempt.attemptRef}`
const closeoutNodeId = (attempt: FleetAttempt): string =>
  `closeout:${attempt.attemptRef}`
const workerNodeId = (workerRef: string): string => `worker:${workerRef}`

export const sarahFleetNodeIdForSelection = (
  selected: SarahFleetSelectedNode,
): string => {
  switch (selected.kind) {
    case "run":
      return `run:${selected.runRef}`
    case "work_unit":
      return `work:${selected.workUnitRef}`
    case "attempt":
      return `attempt:${selected.attemptRef}`
    case "assignment":
      return `assignment:${selected.attemptRef}`
    case "verification":
      return `verification:${selected.attemptRef}`
    case "closeout":
      return `closeout:${selected.attemptRef}`
    case "worker":
      return `worker:${selected.workerRef}`
  }
}

/** Resolve, rather than parse, a graph id. Refs may themselves contain `:`,
 * so the current projection is the only selection authority. */
export const resolveSarahFleetSelectedNode = (
  projection: SarahFleetOwnerProjection,
  nodeId: string,
): SarahFleetSelectedNode | null => {
  if (nodeId === runNodeId(projection)) {
    return { kind: "run", runRef: projection.run.runRef }
  }
  for (const workUnit of projection.workUnits) {
    if (nodeId === workNodeId(workUnit)) {
      return {
        kind: "work_unit",
        runRef: projection.run.runRef,
        workUnitRef: workUnit.workUnitRef,
      }
    }
    for (const attempt of workUnit.attempts) {
      const shared = {
        runRef: projection.run.runRef,
        workUnitRef: workUnit.workUnitRef,
        attemptRef: attempt.attemptRef,
      }
      if (nodeId === attemptNodeId(attempt)) {
        return { kind: "attempt", ...shared }
      }
      if (
        attempt.assignmentRef !== null &&
        nodeId === assignmentNodeId(attempt)
      ) {
        return {
          kind: "assignment",
          ...shared,
          assignmentRef: attempt.assignmentRef,
        }
      }
      if (
        attempt.verification.verificationRef !== null &&
        nodeId === verificationNodeId(attempt)
      ) {
        return {
          kind: "verification",
          ...shared,
          verificationRef: attempt.verification.verificationRef,
        }
      }
      if (
        attempt.closeout.closeoutRef !== null &&
        nodeId === closeoutNodeId(attempt)
      ) {
        return {
          kind: "closeout",
          ...shared,
          closeoutRef: attempt.closeout.closeoutRef,
        }
      }
    }
  }
  const worker = projection.workers.find(
    (candidate) => nodeId === workerNodeId(candidate.workerRef),
  )
  return worker === undefined
    ? null
    : {
        kind: "worker",
        runRef: projection.run.runRef,
        workerRef: worker.workerRef,
      }
}

export const isSarahFleetSelectedNodePresent = (
  projection: SarahFleetOwnerProjection,
  selected: SarahFleetSelectedNode,
): boolean => {
  const resolved = resolveSarahFleetSelectedNode(
    projection,
    sarahFleetNodeIdForSelection(selected),
  )
  if (resolved === null || resolved.kind !== selected.kind) return false
  switch (selected.kind) {
    case "run":
      return resolved.runRef === selected.runRef
    case "work_unit":
      return (
        resolved.kind === "work_unit" &&
        resolved.runRef === selected.runRef &&
        resolved.workUnitRef === selected.workUnitRef
      )
    case "attempt":
      return (
        resolved.kind === "attempt" &&
        resolved.runRef === selected.runRef &&
        resolved.workUnitRef === selected.workUnitRef &&
        resolved.attemptRef === selected.attemptRef
      )
    case "assignment":
      return (
        resolved.kind === "assignment" &&
        resolved.runRef === selected.runRef &&
        resolved.workUnitRef === selected.workUnitRef &&
        resolved.attemptRef === selected.attemptRef &&
        resolved.assignmentRef === selected.assignmentRef
      )
    case "verification":
      return (
        resolved.kind === "verification" &&
        resolved.runRef === selected.runRef &&
        resolved.workUnitRef === selected.workUnitRef &&
        resolved.attemptRef === selected.attemptRef &&
        resolved.verificationRef === selected.verificationRef
      )
    case "closeout":
      return (
        resolved.kind === "closeout" &&
        resolved.runRef === selected.runRef &&
        resolved.workUnitRef === selected.workUnitRef &&
        resolved.attemptRef === selected.attemptRef &&
        resolved.closeoutRef === selected.closeoutRef
      )
    case "worker":
      return (
        resolved.kind === "worker" &&
        resolved.runRef === selected.runRef &&
        resolved.workerRef === selected.workerRef
      )
  }
}

const graphStatusForAttempt = (attempt: FleetAttempt): GraphStatus => {
  if (attempt.state === "succeeded") return "success"
  if (attempt.state === "failed") return "failed"
  if (attempt.state === "stale" || attempt.state === "evidence_pending") {
    return "pending"
  }
  return graphStatusForProgress(attempt.progress)
}

const graphStatusForVerification = (attempt: FleetAttempt): GraphStatus =>
  attempt.verification.status === "ready"
    ? "success"
    : attempt.verification.status === "failed"
      ? "failed"
      : "idle"

const graphStatusForCloseout = (attempt: FleetAttempt): GraphStatus =>
  attempt.closeout.status === "accepted"
    ? "success"
    : attempt.closeout.status === "rejected"
      ? "failed"
      : attempt.closeout.status === "submitted"
        ? "pending"
        : "idle"

const supervisionGraph = (projection: SarahFleetOwnerProjection): View => {
  const workerByRef = new Map(
    projection.workers.map((worker) => [worker.workerRef, worker]),
  )
  const rowCount = projection.workUnits.reduce(
    (count, workUnit) => count + Math.max(1, workUnit.attempts.length),
    0,
  )
  const rowHeight = 104
  const rowY = (index: number): number =>
    (index - Math.max(0, rowCount - 1) / 2) * rowHeight
  const nodes: Array<{
    id: string
    label: string
    kind: "worker" | "validator" | "arbiter" | "task" | "generic"
    status: GraphStatus
    x: number
    y: number
  }> = []
  const edges: Array<{
    id: string
    from: string
    to: string
    kind: "flow" | "dependency" | "pairing"
    status: GraphStatus
  }> = []
  const renderedWorkers = new Set<string>()
  const renderedWorkRefs = new Set(
    projection.workUnits.map((workUnit) => workUnit.workUnitRef),
  )
  let nextRow = 0

  nodes.push({
    id: runNodeId(projection),
    label: `Plan · ${projection.run.name}`,
    kind: "arbiter",
    status: graphStatusForRun(projection.run.status),
    x: -318,
    y: 0,
  })

  for (const workUnit of projection.workUnits) {
    const attemptRows = Math.max(1, workUnit.attempts.length)
    const firstY = rowY(nextRow)
    const lastY = rowY(nextRow + attemptRows - 1)
    const workY = (firstY + lastY) / 2
    nodes.push({
      id: workNodeId(workUnit),
      label: workUnit.name,
      kind: "task",
      status: graphStatusForWorkUnit(workUnit),
      x: -212,
      y: workY,
    })
    edges.push({
      id: `edge:${runNodeId(projection)}:${workNodeId(workUnit)}`,
      from: runNodeId(projection),
      to: workNodeId(workUnit),
      kind: "flow",
      status: graphStatusForWorkUnit(workUnit),
    })

    workUnit.attempts.forEach((attempt, attemptIndex) => {
      const y = rowY(nextRow + attemptIndex)
      const attemptId = attemptNodeId(attempt)
      nodes.push({
        id: attemptId,
        label: `${sentenceCase(attempt.workerKind)} attempt · ${humanize(attempt.state)}`,
        kind: "task",
        status: graphStatusForAttempt(attempt),
        x: -96,
        y,
      })
      edges.push({
        id: `edge:${workNodeId(workUnit)}:${attemptId}`,
        from: workNodeId(workUnit),
        to: attemptId,
        kind: "flow",
        status: graphStatusForAttempt(attempt),
      })

      let evidenceParent = attemptId
      if (attempt.assignmentRef !== null) {
        const assignmentId = assignmentNodeId(attempt)
        nodes.push({
          id: assignmentId,
          label: `Assignment · ${humanize(attempt.assignmentStatus ?? "status not reported")}`,
          kind: "task",
          status: graphStatusForAttempt(attempt),
          x: 24,
          y,
        })
        edges.push({
          id: `edge:${attemptId}:${assignmentId}`,
          from: attemptId,
          to: assignmentId,
          kind: "flow",
          status: graphStatusForAttempt(attempt),
        })
        evidenceParent = assignmentId
      }

      if (attempt.verification.verificationRef !== null) {
        const verificationId = verificationNodeId(attempt)
        nodes.push({
          id: verificationId,
          label: attempt.verification.summary,
          kind: "validator",
          status: graphStatusForVerification(attempt),
          x: 154,
          y,
        })
        edges.push({
          id: `edge:${evidenceParent}:${verificationId}`,
          from: evidenceParent,
          to: verificationId,
          kind: "flow",
          status: graphStatusForVerification(attempt),
        })
        evidenceParent = verificationId
      }

      if (attempt.closeout.closeoutRef !== null) {
        const closeoutId = closeoutNodeId(attempt)
        nodes.push({
          id: closeoutId,
          label: attempt.closeout.summary,
          kind: "validator",
          status: graphStatusForCloseout(attempt),
          x: 286,
          y,
        })
        edges.push({
          id: `edge:${evidenceParent}:${closeoutId}`,
          from: evidenceParent,
          to: closeoutId,
          kind: "flow",
          status: graphStatusForCloseout(attempt),
        })
      }

      if (attempt.workerRef !== null) {
        const worker = workerByRef.get(attempt.workerRef)
        if (worker !== undefined) {
          const workerId = workerNodeId(worker.workerRef)
          if (!renderedWorkers.has(worker.workerRef)) {
            renderedWorkers.add(worker.workerRef)
            nodes.push({
              id: workerId,
              label: worker.name,
              kind: "worker",
              status: graphStatusForProgress(worker.progress),
              x: 24,
              y: y + 38,
            })
          }
          edges.push({
            id: `edge:${attemptId}:${workerId}`,
            from: attemptId,
            to: workerId,
            kind: "pairing",
            status: graphStatusForProgress(worker.progress),
          })
        }
      }
    })
    nextRow += attemptRows
  }

  for (const workUnit of projection.workUnits) {
    for (const dependencyRef of workUnit.dependsOnRefs) {
      if (!renderedWorkRefs.has(dependencyRef)) continue
      edges.push({
        id: `edge:dependency:${dependencyRef}:${workUnit.workUnitRef}`,
        from: `work:${dependencyRef}`,
        to: workNodeId(workUnit),
        kind: "dependency",
        status: graphStatusForWorkUnit(workUnit),
      })
    }
  }

  return GraphFigure({
    key: `fleet-supervision-${projection.run.runRef}-graph`,
    nodes,
    edges,
    layout: "precomputed",
    width: 760,
    height: Math.max(520, rowCount * rowHeight + 128),
    onNodeSelect: IntentRef(
      SarahFleetNodeSelected.name,
      ComponentValueBinding(),
    ),
    a11y: {
      role: "group",
      label: `${projection.run.name} plan map. ${projection.workUnits.length} work units, ${projection.workUnits.reduce((count, workUnit) => count + workUnit.attempts.length, 0)} attempts, and only reported assignment and evidence links. Use the Open buttons below for keyboard navigation.`,
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

const openNodeButton = (
  key: string,
  label: string,
  a11yLabel: string,
  nodeId: string,
  variant: "primary" | "secondary" | "ghost" = "secondary",
): View =>
  Button({
    key,
    label,
    variant,
    onPress: IntentRef(SarahFleetNodeSelected.name, StaticPayload(nodeId)),
    a11y: { label: a11yLabel },
  })

const attemptOpenButtons = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
  attempt: FleetAttempt,
): ReadonlyArray<View> => {
  const workerExists =
    attempt.workerRef !== null &&
    projection.workers.some(
      (worker) => worker.workerRef === attempt.workerRef,
    )
  return [
    openNodeButton(
      `fleet-supervision-${attempt.attemptRef}-open`,
      "Open attempt",
      `Open ${attempt.workerKind} attempt for ${workUnit.name}`,
      attemptNodeId(attempt),
    ),
    ...(attempt.assignmentRef === null
      ? []
      : [
          openNodeButton(
            `fleet-supervision-${attempt.attemptRef}-assignment-open`,
            "Open assignment",
            `Open reported assignment for ${workUnit.name}`,
            assignmentNodeId(attempt),
            "ghost",
          ),
        ]),
    ...(attempt.verification.verificationRef === null
      ? []
      : [
          openNodeButton(
            `fleet-supervision-${attempt.attemptRef}-verification-open`,
            "Open verification",
            `Open reported verification for ${workUnit.name}`,
            verificationNodeId(attempt),
            "ghost",
          ),
        ]),
    ...(attempt.closeout.closeoutRef === null
      ? []
      : [
          openNodeButton(
            `fleet-supervision-${attempt.attemptRef}-closeout-open`,
            "Open closeout",
            `Open reported closeout for ${workUnit.name}`,
            closeoutNodeId(attempt),
            "ghost",
          ),
        ]),
    ...(!workerExists || attempt.workerRef === null
      ? []
      : [
          openNodeButton(
            `fleet-supervision-${attempt.attemptRef}-worker-open`,
            "Open worker",
            `Open reported worker for ${workUnit.name}`,
            workerNodeId(attempt.workerRef),
            "ghost",
          ),
        ]),
  ]
}

const attemptSteerControls = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
  attempt: FleetAttempt,
  options: SarahFleetDrilldownViewOptions,
): ReadonlyArray<View> => {
  if (
    workUnit.latestAttemptRef !== attempt.attemptRef ||
    workUnit.state !== "running" ||
    attempt.state !== "running"
  ) {
    return []
  }
  if (
    (options.interactionMode ?? SARAH_OWNER_FLEET_READ_ONLY) !==
    SARAH_OWNER_FLEET_INTERACTIVE
  ) {
    return [
      text(
        `fleet-drilldown-attempt-${attempt.attemptRef}-steer-read-only`,
        "Steering is unavailable in this read-only surface.",
        "caption",
        "textMuted",
      ),
    ]
  }
  const steerDraft = options.steerDraft ?? ""
  const validDraft =
    steerDraft.trim().length > 0 &&
    steerDraft.length <= SARAH_FLEET_MAX_STEER_BODY_LENGTH
  return [
    Stack(
      {
        key: `fleet-drilldown-attempt-${attempt.attemptRef}-steer`,
        direction: "column",
        gap: "2",
        a11y: {
          role: "group",
          label: `Steer the exact active ${attempt.workerKind} attempt`,
        },
        style: { width: "full" },
      },
      [
        TextField({
          key: `fleet-drilldown-attempt-${attempt.attemptRef}-steer-input`,
          value: steerDraft,
          label: "Steer this attempt",
          placeholder: "Give this active attempt a bounded instruction…",
          multiline: true,
          onChange: IntentRef(
            SarahFleetSteerDraftChanged.name,
            ComponentValueBinding(),
          ),
          a11y: {
            label: `Private steer instruction for ${workUnit.name}`,
          },
          style: { width: "full" },
        }),
        Button({
          key: `fleet-drilldown-attempt-${attempt.attemptRef}-steer-submit`,
          label: "Send steer",
          variant: "primary",
          disabled: !validDraft,
          onPress: IntentRef(
            SarahFleetSteerSubmitted.name,
            StaticPayload({
              runRef: projection.run.runRef,
              workUnitRef: workUnit.workUnitRef,
              attemptRef: attempt.attemptRef,
            }),
          ),
          a11y: {
            label: `Send private steer to the exact active ${attempt.workerKind} attempt`,
          },
        }),
        text(
          `fleet-drilldown-attempt-${attempt.attemptRef}-steer-privacy`,
          `Private body stays in this browser until submission and is limited to ${SARAH_FLEET_MAX_STEER_BODY_LENGTH} characters.`,
          "caption",
          "textMuted",
        ),
      ],
    ),
  ]
}

const attemptRows = (
  projection: SarahFleetOwnerProjection,
  workUnit: FleetWorkUnit,
): ReadonlyArray<View> => {
  if (workUnit.attempts.length === 0) {
    return [
      text(
        `fleet-supervision-${workUnit.workUnitRef}-attempts-empty`,
        "No attempt has been claimed. The plan stops here until a real work claim is reported.",
        "caption",
        "textMuted",
      ),
    ]
  }
  return workUnit.attempts.map((attempt) =>
    Stack(
      {
        key: `fleet-supervision-${attempt.attemptRef}-row`,
        direction: "column",
        gap: "1",
        padding: "2",
        a11y: {
          role: "listitem",
          label: `${attempt.summary}. ${progressLabel(attempt.progress)}. ${attempt.verification.summary}. ${attempt.closeout.summary}.`,
        },
        style: { width: "full", backgroundColor: "surfaceRaised" },
      },
      [
        Stack(
          {
            key: `fleet-supervision-${attempt.attemptRef}-heading`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            justify: "between",
            style: { width: "full" },
          },
          [
            text(
              `fleet-supervision-${attempt.attemptRef}-summary`,
              attempt.summary,
              "caption",
            ),
            Badge({
              key: `fleet-supervision-${attempt.attemptRef}-state`,
              label: sentenceCase(attempt.state),
              tone: progressTone(attempt.progress),
            }),
          ],
        ),
        Stack(
          {
            key: `fleet-supervision-${attempt.attemptRef}-actions`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            style: { width: "full" },
          },
          attemptOpenButtons(projection, workUnit, attempt),
        ),
      ],
    ),
  )
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
      openNodeButton(
        `fleet-supervision-${workUnit.workUnitRef}-details-open`,
        "Open details",
        `Open details for work unit ${workUnit.name}`,
        workNodeId(workUnit),
      ),
      Stack(
        {
          key: `fleet-supervision-${workUnit.workUnitRef}-attempts`,
          direction: "column",
          gap: "1",
          a11y: { role: "list", label: `Attempts for ${workUnit.name}` },
          style: { width: "full" },
        },
        [
          text(
            `fleet-supervision-${workUnit.workUnitRef}-attempts-title`,
            "Attempts",
            "caption",
          ),
          ...attemptRows(projection, workUnit),
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

type CommandPresentation = Readonly<{
  status: "delivered" | "completed" | "failed" | "stale"
  label: string
  message: string
  tone: Tone
}>

const commandOutcomePresentation = (
  outcome: FleetCommandOutcome,
): CommandPresentation => {
  if (
    outcome.deliveryOutcome === "skipped_stale" ||
    outcome.completionOutcome === "skipped_stale"
  ) {
    return {
      status: "stale",
      label: "Stale",
      message:
        "The command was skipped as stale. No effective fleet state was claimed.",
      tone: "warn",
    }
  }
  if (
    outcome.deliveryOutcome === "failed" ||
    outcome.deliveryOutcome === "rejected" ||
    outcome.completionOutcome === "failed" ||
    outcome.completionOutcome === "rejected"
  ) {
    return {
      status: "failed",
      label: "Failed",
      message:
        outcome.completionOutcome === null
          ? "The executor refused or failed the command during delivery."
          : "The delivered command failed before it changed effective fleet state.",
      tone: "danger",
    }
  }
  if (outcome.completionOutcome === "applied") {
    return {
      status: "completed",
      label: "Completed",
      message:
        outcome.effectiveOutcome === null
          ? "The command completed without a reported effective state."
          : `The command completed: ${humanize(outcome.effectiveOutcome)}.`,
      tone: "success",
    }
  }
  return {
    status: "delivered",
    label: "Delivered",
    message: "The executor received the command and its completion is pending.",
    tone: "info",
  }
}

const commandActivity = (
  projection: SarahFleetOwnerProjection,
  hostSubmissions: ReadonlyArray<SarahFleetHostSubmission>,
): View => {
  const durableOutcomes = [...(projection.commandOutcomes ?? [])]
    .slice(-12)
    .reverse()
  const localRows = hostSubmissions.slice(-8).reverse().map((submission) => {
    const failed = submission.status === "failed"
    return Stack(
      {
        key: `fleet-command-host-${submission.submissionRef}`,
        direction: "column",
        gap: "1",
        padding: "2",
        a11y: {
          role: "listitem",
          label: failed
            ? `${submission.summary}. Submission failed before a durable receipt was returned.`
            : `${submission.summary}. Awaiting a durable delivery receipt.`,
        },
        style: { width: "full", backgroundColor: "surface" },
      },
      [
        Badge({
          key: `fleet-command-host-${submission.submissionRef}-status`,
          label: failed ? "Submission failed" : "Requested",
          tone: failed ? "danger" : "info",
        }),
        text(
          `fleet-command-host-${submission.submissionRef}-summary`,
          failed
            ? `${submission.summary}. The host could not submit it; no delivery is claimed.`
            : `${submission.summary}. Waiting for a durable delivery receipt.`,
          "caption",
          "textMuted",
        ),
      ],
    )
  })
  const durableRows = durableOutcomes.map((outcome) => {
    const presentation = commandOutcomePresentation(outcome)
    return Stack(
      {
        key: `fleet-command-outcome-${outcome.intentId}`,
        direction: "column",
        gap: "1",
        padding: "2",
        a11y: {
          role: "listitem",
          label: `${presentation.label} ${humanize(outcome.kind)} command. ${presentation.message}`,
        },
        style: { width: "full", backgroundColor: "surface" },
      },
      [
        Stack(
          {
            key: `fleet-command-outcome-${outcome.intentId}-heading`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            justify: "between",
            style: { width: "full" },
          },
          [
            text(
              `fleet-command-outcome-${outcome.intentId}-kind`,
              sentenceCase(outcome.kind),
              "caption",
            ),
            Badge({
              key: `fleet-command-outcome-${outcome.intentId}-status`,
              label: presentation.label,
              tone: presentation.tone,
            }),
          ],
        ),
        text(
          `fleet-command-outcome-${outcome.intentId}-message`,
          presentation.message,
          "caption",
          "textMuted",
        ),
      ],
    )
  })
  const rows = [...localRows, ...durableRows]
  return Stack(
    {
      key: `fleet-supervision-${projection.run.runRef}-command-activity`,
      direction: "column",
      gap: "2",
      a11y: { role: "list", label: "Fleet command activity" },
      style: { width: "full" },
    },
    [
      text(
        `fleet-supervision-${projection.run.runRef}-command-activity-title`,
        "Command activity",
        "label",
      ),
      ...(rows.length === 0
        ? [
            text(
              `fleet-supervision-${projection.run.runRef}-command-activity-empty`,
              "No command receipts reported.",
              "caption",
              "textMuted",
            ),
          ]
        : rows),
    ],
  )
}

const detailRows = (
  key: string,
  rows: ReadonlyArray<readonly [string, string]>,
): View =>
  Stack(
    {
      key,
      direction: "column",
      gap: "1",
      a11y: { role: "list", label: "Selected fleet item details" },
      style: { width: "full" },
    },
    rows.map(([label, value], index) =>
      Stack(
        {
          key: `${key}-${index}`,
          direction: { base: "column", sm: "row" },
          gap: "1",
          align: "start",
          a11y: { role: "listitem", label: `${label}: ${value}` },
          style: { width: "full" },
        },
        [
          text(`${key}-${index}-label`, label, "caption"),
          text(`${key}-${index}-value`, value, "caption", "textMuted"),
        ],
      ),
    ),
  )

const referenceRows = (
  label: string,
  refs: ReadonlyArray<string>,
): ReadonlyArray<readonly [string, string]> =>
  refs.length === 0
    ? [[label, "not reported"]]
    : refs.map((ref, index) => [
        refs.length === 1 ? label : `${label} ${index + 1}`,
        ref,
      ])

const detailCard = (
  key: string,
  title: string,
  badge: Readonly<{ label: string; tone: Tone }>,
  summary: string,
  children: ReadonlyArray<View>,
): View =>
  Card(
    {
      key,
      padding: "4",
      radius: "lg",
      a11y: { role: "region", label: `${title}. ${summary}` },
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
          key: `${key}-heading`,
          direction: { base: "column", sm: "row" },
          gap: "2",
          align: "start",
          justify: "between",
          style: { width: "full" },
        },
        [
          text(`${key}-title`, title, "title"),
          Badge({
            key: `${key}-status`,
            label: badge.label,
            tone: badge.tone,
          }),
        ],
      ),
      text(`${key}-summary`, summary, "body", "textMuted"),
      ...children,
    ],
  )

const attemptForSelection = (
  projection: SarahFleetOwnerProjection,
  selected: Extract<
    SarahFleetSelectedNode,
    { kind: "attempt" | "assignment" | "verification" | "closeout" }
  >,
): Readonly<{ workUnit: FleetWorkUnit; attempt: FleetAttempt }> | null => {
  const workUnit = projection.workUnits.find(
    (candidate) => candidate.workUnitRef === selected.workUnitRef,
  )
  const attempt = workUnit?.attempts.find(
    (candidate) => candidate.attemptRef === selected.attemptRef,
  )
  return workUnit === undefined || attempt === undefined
    ? null
    : { workUnit, attempt }
}

const missingSelectionCard = (): View =>
  detailCard(
    "fleet-drilldown-unavailable",
    "Fleet item unavailable",
    { label: "Unavailable", tone: "warn" },
    "This item is no longer present in the exact-run projection.",
    [],
  )

/** Full Fleet-tab detail view. It reads only the selected node's current
 * projection and never manufactures a missing assignment or evidence hop. */
export const sarahFleetNodeDrilldownView = (
  projection: SarahFleetOwnerProjection,
  selected: SarahFleetSelectedNode,
  options: SarahFleetDrilldownViewOptions = {},
): View => {
  if (!isSarahFleetSelectedNodePresent(projection, selected)) {
    return missingSelectionCard()
  }
  if (selected.kind === "run") {
    return detailCard(
      `fleet-drilldown-run-${selected.runRef}`,
      projection.run.name,
      {
        label: sentenceCase(projection.run.status),
        tone: runTone(projection.run.status),
      },
      "The exact run plan and its reported counters.",
      [
        detailRows(`fleet-drilldown-run-${selected.runRef}-details`, [
          ["Run", projection.run.runRef],
          ["Desired slots", String(projection.run.desiredSlots)],
          ["Worker policy", humanize(projection.run.workerKind)],
          ["Work units", String(projection.run.counters.workUnitsTotal)],
          ["Active", String(projection.run.counters.activeAssignments)],
          ["Completed", String(projection.run.counters.completedAssignments)],
          ["Failed", String(projection.run.counters.failedAssignments)],
          ["Blocked", String(projection.run.counters.blockedAssignments)],
          ["Updated", projection.run.updatedAt],
        ]),
        Stack(
          {
            key: `fleet-drilldown-run-${selected.runRef}-work-openers`,
            direction: "column",
            gap: "1",
            a11y: { role: "list", label: "Open work units in this plan" },
            style: { width: "full" },
          },
          projection.workUnits.map((workUnit) =>
            openNodeButton(
              `fleet-drilldown-run-${selected.runRef}-work-${workUnit.workUnitRef}`,
              `Open ${workUnit.name}`,
              `Open work unit ${workUnit.name}`,
              workNodeId(workUnit),
              "ghost",
            ),
          ),
        ),
      ],
    )
  }

  if (selected.kind === "work_unit") {
    const workUnit = projection.workUnits.find(
      (candidate) => candidate.workUnitRef === selected.workUnitRef,
    )
    if (workUnit === undefined) return missingSelectionCard()
    const knownDependencies = workUnit.dependsOnRefs.flatMap((dependencyRef) => {
      const dependency = projection.workUnits.find(
        (candidate) => candidate.workUnitRef === dependencyRef,
      )
      return dependency === undefined ? [] : [dependency]
    })
    return detailCard(
      `fleet-drilldown-work-${workUnit.workUnitRef}`,
      workUnit.name,
      {
        label: sentenceCase(workUnit.state),
        tone: progressTone(workUnit.progress),
      },
      workUnit.attempts.length === 0
        ? "This work unit is planned. No attempt, assignment, verification, or closeout has been reported."
        : `${workUnit.attempts.length} reported attempt${workUnit.attempts.length === 1 ? "" : "s"}.`,
      [
        detailRows(`fleet-drilldown-work-${workUnit.workUnitRef}-details`, [
          ["Work unit", workUnit.workUnitRef],
          ["Issue", workUnit.issueRef ?? "not reported"],
          ["State", humanize(workUnit.state)],
          ["Dependencies", String(workUnit.dependsOnRefs.length)],
          ["Latest attempt", workUnit.latestAttemptRef ?? "not reported"],
          ["Accepted attempt", workUnit.acceptedAttemptRef ?? "not reported"],
          ["Updated", workUnit.updatedAt],
        ]),
        ...(knownDependencies.length === 0
          ? []
          : [
              Stack(
                {
                  key: `fleet-drilldown-work-${workUnit.workUnitRef}-dependencies`,
                  direction: "column",
                  gap: "1",
                  a11y: {
                    role: "list",
                    label: `Dependencies for ${workUnit.name}`,
                  },
                  style: { width: "full" },
                },
                knownDependencies.map((dependency) =>
                  openNodeButton(
                    `fleet-drilldown-work-${workUnit.workUnitRef}-dependency-${dependency.workUnitRef}`,
                    `Open dependency ${dependency.name}`,
                    `Open dependency ${dependency.name}`,
                    workNodeId(dependency),
                    "ghost",
                  ),
                ),
              ),
            ]),
        Stack(
          {
            key: `fleet-drilldown-work-${workUnit.workUnitRef}-attempts`,
            direction: "column",
            gap: "2",
            a11y: { role: "list", label: `Attempts for ${workUnit.name}` },
            style: { width: "full" },
          },
          attemptRows(projection, workUnit),
        ),
      ],
    )
  }

  if (selected.kind === "worker") {
    const worker = projection.workers.find(
      (candidate) => candidate.workerRef === selected.workerRef,
    )
    if (worker === undefined) return missingSelectionCard()
    const linkedAttempts = projection.workUnits.flatMap((workUnit) =>
      workUnit.attempts
        .filter((attempt) => attempt.workerRef === worker.workerRef)
        .map((attempt) => ({ workUnit, attempt })),
    )
    return detailCard(
      `fleet-drilldown-worker-${worker.workerRef}`,
      worker.name,
      {
        label: sentenceCase(worker.phase),
        tone: progressTone(worker.progress),
      },
      progressLabel(worker.progress),
      [
        detailRows(`fleet-drilldown-worker-${worker.workerRef}-details`, [
          ["Worker", worker.workerRef],
          ["Harness", worker.harnessKind ?? "not reported"],
          ["Account", worker.accountRefHash ?? "not reported"],
          ["Updated", worker.updatedAt],
        ]),
        Stack(
          {
            key: `fleet-drilldown-worker-${worker.workerRef}-attempts`,
            direction: "column",
            gap: "1",
            a11y: { role: "list", label: `Attempts linked to ${worker.name}` },
            style: { width: "full" },
          },
          linkedAttempts.length === 0
            ? [
                text(
                  `fleet-drilldown-worker-${worker.workerRef}-attempts-empty`,
                  "No attempt link is reported for this worker.",
                  "caption",
                  "textMuted",
                ),
              ]
            : linkedAttempts.map(({ workUnit, attempt }) =>
                openNodeButton(
                  `fleet-drilldown-worker-${worker.workerRef}-attempt-${attempt.attemptRef}`,
                  `Open ${workUnit.name} attempt`,
                  `Open attempt for ${workUnit.name}`,
                  attemptNodeId(attempt),
                  "ghost",
                ),
              ),
        ),
      ],
    )
  }

  const found = attemptForSelection(projection, selected)
  if (found === null) return missingSelectionCard()
  const { workUnit, attempt } = found

  if (selected.kind === "attempt") {
    const usageRows: ReadonlyArray<readonly [string, string]> =
      attempt.usageEvidence.truth === "exact"
        ? [
            ["Usage", "exact"],
            ["Total tokens", String(attempt.usageEvidence.totalTokens)],
            ["Token rows", String(attempt.usageEvidence.tokenRows)],
          ]
        : [["Usage", humanize(attempt.usageEvidence.truth)]]
    return detailCard(
      `fleet-drilldown-attempt-${attempt.attemptRef}`,
      `${sentenceCase(attempt.workerKind)} attempt`,
      {
        label: sentenceCase(attempt.state),
        tone: progressTone(attempt.progress),
      },
      `${workUnit.name}. ${progressLabel(attempt.progress)}.`,
      [
        detailRows(`fleet-drilldown-attempt-${attempt.attemptRef}-details`, [
          ["Attempt / work claim", attempt.attemptRef],
          ["Work unit", attempt.workUnitRef],
          ["Assignment", attempt.assignmentRef ?? "not reported"],
          ["Assignment state", attempt.assignmentStatus ?? "not reported"],
          ["Harness", attempt.workerKind],
          ["Worker", attempt.workerRef ?? "not reported"],
          ["Capacity", attempt.capacity.capacityClass],
          ["Pylon", attempt.capacity.pylonRef],
          ["Account", attempt.capacity.accountRefHash ?? "not reported"],
          ["Marginal cost", humanize(attempt.marginalCostClass)],
          ...usageRows,
          ["Started", attempt.startedAt],
          ["Last server observation", attempt.lastObservedAt],
          ["Remote clock (audit only)", attempt.remoteObservedAtAudit],
          ["Terminal", attempt.terminalAt ?? "not reported"],
          ...referenceRows("Artifact", attempt.artifactRefs),
          ...referenceRows("Proof", attempt.proofRefs),
          ...referenceRows("Authority receipt", attempt.authorityReceiptRefs),
          ...referenceRows("Blocker", attempt.blockerRefs),
        ]),
        Stack(
          {
            key: `fleet-drilldown-attempt-${attempt.attemptRef}-next`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            a11y: { role: "group", label: "Reported attempt links" },
            style: { width: "full" },
          },
          attemptOpenButtons(projection, workUnit, attempt).slice(1),
        ),
        ...attemptSteerControls(projection, workUnit, attempt, options),
      ],
    )
  }

  if (selected.kind === "assignment") {
    return detailCard(
      `fleet-drilldown-assignment-${attempt.attemptRef}`,
      "Assignment",
      {
        label: sentenceCase(attempt.assignmentStatus ?? "status not reported"),
        tone: progressTone(attempt.progress),
      },
      `${workUnit.name}. This is an optional edge from the exact attempt, not its identity.`,
      [
        detailRows(`fleet-drilldown-assignment-${attempt.attemptRef}-details`, [
          ["Assignment", selected.assignmentRef],
          ["Attempt / work claim", attempt.attemptRef],
          ["Work unit", workUnit.workUnitRef],
          ["Worker", attempt.workerRef ?? "not reported"],
          ["Status", attempt.assignmentStatus ?? "not reported"],
        ]),
        Stack(
          {
            key: `fleet-drilldown-assignment-${attempt.attemptRef}-next`,
            direction: { base: "column", sm: "row" },
            gap: "1",
            align: "start",
            style: { width: "full" },
          },
          attemptOpenButtons(projection, workUnit, attempt).slice(2),
        ),
      ],
    )
  }

  if (selected.kind === "verification") {
    return detailCard(
      `fleet-drilldown-verification-${attempt.attemptRef}`,
      "Verification",
      {
        label: sentenceCase(attempt.verification.status),
        tone:
          attempt.verification.status === "ready"
            ? "success"
            : attempt.verification.status === "failed"
              ? "danger"
              : "neutral",
      },
      attempt.verification.summary,
      [
        detailRows(`fleet-drilldown-verification-${attempt.attemptRef}-details`, [
          ["Verification", selected.verificationRef],
          ["Attempt / work claim", attempt.attemptRef],
          ...referenceRows("Evidence", attempt.verification.evidenceRefs),
        ]),
        ...(attempt.closeout.closeoutRef === null
          ? []
          : [
              openNodeButton(
                `fleet-drilldown-verification-${attempt.attemptRef}-closeout`,
                "Open closeout",
                `Open closeout for ${workUnit.name}`,
                closeoutNodeId(attempt),
                "ghost",
              ),
            ]),
      ],
    )
  }

  return detailCard(
    `fleet-drilldown-closeout-${attempt.attemptRef}`,
    "Closeout",
    {
      label: sentenceCase(attempt.closeout.status),
      tone:
        attempt.closeout.status === "accepted"
          ? "success"
          : attempt.closeout.status === "rejected"
            ? "danger"
            : attempt.closeout.status === "submitted"
              ? "info"
              : "neutral",
    },
    attempt.closeout.summary,
    [
      detailRows(`fleet-drilldown-closeout-${attempt.attemptRef}-details`, [
        ["Closeout", selected.closeoutRef],
        ["Attempt / work claim", attempt.attemptRef],
        ["Class", attempt.closeout.closeoutClass ?? "not reported"],
        ...referenceRows("Artifact", attempt.artifactRefs),
        ...referenceRows("Proof", attempt.proofRefs),
        ...referenceRows("Authority receipt", attempt.authorityReceiptRefs),
      ]),
    ],
  )
}

/**
 * Pure FC-3 Effect Native supervision view over an owner-scoped projection.
 *
 * This component only emits typed intent data. It does not fetch, authorize,
 * infer controls, open private logs, or compose steer bodies. The host must
 * bind the exported intent definitions to the authenticated fleet-intent
 * boundary. Exact public-safe refs appear only in audit rows and selected-node
 * detail; raw prompts, logs, paths, and excess fields are never read.
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
  const hostSubmissions = options.hostSubmissions ?? []
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
      openNodeButton(
        `fleet-supervision-${projection.run.runRef}-details-open`,
        "Open run details",
        `Open run details for ${projection.run.name}`,
        runNodeId(projection),
      ),
      supervisionGraph(projection),
      runControls(projection, interactionMode),
      commandActivity(projection, hostSubmissions),
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
