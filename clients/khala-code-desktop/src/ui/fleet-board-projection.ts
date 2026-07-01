import {
  graphLinkStatusForRefs,
  graphPublicSafetyCheckRefPattern,
  graphSafeRefPattern,
  type GraphDatum,
  type GraphLink,
  type GraphNode,
  type GraphNodeStatus,
  type GraphPin,
  type GraphPinDirection,
  type GraphSpec,
} from "@openagentsinc/arbiter-effect"

import type {
  KhalaCodeDesktopFleetAccount,
  KhalaCodeDesktopFleetAssignment,
  KhalaCodeDesktopFleetProcess,
  KhalaCodeDesktopFleetStatus,
} from "../shared/rpc"

export const KhalaFleetBoardProjectionSchemaVersion =
  "openagents.khala_code.fleet_board_projection.v0"

export type KhalaFleetTimelineEventStatus =
  | "idle"
  | "active"
  | "blocked"
  | "complete"

export type KhalaFleetTimelineEvent = Readonly<{
  id: string
  label: string
  status: KhalaFleetTimelineEventStatus
  observedAt: string
  subjectRef: string
  detail: string
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
}>

export type KhalaFleetBoardProjection = Readonly<{
  schemaVersion: typeof KhalaFleetBoardProjectionSchemaVersion
  generatedAt: string
  graph: GraphSpec
  timeline: ReadonlyArray<KhalaFleetTimelineEvent>
  summary: Readonly<{
    readyAccounts: number
    totalAccounts: number
    activeAssignments: number
    runningProcesses: number
    availableCodexAssignments: number | null
    maxCodexAssignments: number | null
  }>
  evidenceRefs: ReadonlyArray<string>
  blockerRefs: ReadonlyArray<string>
  caveatRefs: ReadonlyArray<string>
  sourceRefs: ReadonlyArray<string>
}>

export type KhalaFleetBoardProjectionInput = Readonly<{
  status: KhalaCodeDesktopFleetStatus
  generatedAt?: string
}>

export class KhalaFleetBoardProjectionUnsafe extends Error {
  override readonly name = "KhalaFleetBoardProjectionUnsafe"

  constructor(readonly reason: string) {
    super(reason)
  }
}

const unsafePublicProjectionValue =
  /\/Users\/|\/home\/|access[_-]?token|auth\.json|bearer |authorization:|cookie|credential|customer[_-]?(email|name|value)|email[_-]?(address|body)|gho_[A-Za-z0-9_]+|ghp_[A-Za-z0-9_]+|http:\/\/|https:\/\/|invoice|lnbc|lntb|lnbcrt|lno1|mnemonic|oauth|payment[_-]?(hash|id|preimage|proof)|preimage|private[_-]?(endpoint|repo|source)|provider[_-]?(grant|payload|secret|token)|raw[_-]?(auth|email|fixture|log|payload|prompt|provider|runner|source|trace|traces)|secret|(?:^|[^A-Za-z0-9])sk-[a-z0-9]|scratch[_-]?log|token|wallet/i

const counterOnlyRefPattern = /(^|[.:/-])counter([.:/-]|$)|=\d+$/

const collectStringValues = (value: unknown): ReadonlyArray<string> => {
  if (typeof value === "string") return [value]
  if (Array.isArray(value)) return value.flatMap(item => collectStringValues(item))
  if (value !== null && typeof value === "object") {
    return Object.values(value).flatMap(item => collectStringValues(item))
  }
  return []
}

const isPublicSafeString = (value: string): boolean =>
  graphPublicSafetyCheckRefPattern.test(value) ||
  !unsafePublicProjectionValue.test(value)

const assertProjectionPublicSafe = (
  projection: KhalaFleetBoardProjection,
): void => {
  const unsafe = collectStringValues(projection).find(value => !isPublicSafeString(value))
  if (unsafe !== undefined) {
    throw new KhalaFleetBoardProjectionUnsafe(
      `Fleet board projection leaked private material: ${unsafe}`,
    )
  }
}

const uniqueRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  [
    ...new Set(
      refs
        .flatMap(ref => (ref === undefined || ref === null ? [] : [ref.trim()]))
        .filter(ref => ref !== ""),
    ),
  ].sort()

const isSafeRef = (value: string): boolean =>
  graphSafeRefPattern.test(value) &&
  (graphPublicSafetyCheckRefPattern.test(value) ||
    !unsafePublicProjectionValue.test(value))

const publicRefs = (
  refs: ReadonlyArray<string | null | undefined>,
): ReadonlyArray<string> =>
  uniqueRefs(refs).filter(ref => !counterOnlyRefPattern.test(ref) && isSafeRef(ref))

const refOrFallback = (
  value: string | null | undefined,
  fallback: string,
): string => {
  const ref = value?.trim()
  if (ref !== undefined && ref !== "" && isSafeRef(ref)) return ref
  return fallback
}

const safeLabel = (value: string | null | undefined, fallback: string): string => {
  const label = value?.trim()
  if (
    label !== undefined &&
    label !== "" &&
    label.length <= 64 &&
    isPublicSafeString(label) &&
    !label.includes("@") &&
    !label.includes("/")
  ) {
    return label
  }
  return fallback
}

const accountState = (
  account: KhalaCodeDesktopFleetAccount,
): "ready" | "missing" | "degraded" => {
  const value = account.readiness.toLowerCase()
  if (value === "ready") return "ready"
  if (value.includes("missing")) return "missing"
  return "degraded"
}

const accountStateLabel = (
  account: KhalaCodeDesktopFleetAccount,
): "ready" | "missing sign-in" | "needs attention" => {
  const state = accountState(account)
  if (state === "ready") return "ready"
  if (state === "missing") return "missing sign-in"
  return "needs attention"
}

const pylonStatusLabel = (
  status: KhalaCodeDesktopFleetStatus["pylon"]["status"],
): "online" | "started" | "offline" =>
  status === "unavailable" ? "offline" : status

const pin = (
  direction: GraphPinDirection,
  id: string,
  name: string,
  type: string,
): GraphPin => ({
  direction,
  id,
  name,
  type,
})

const datum = (
  label: string,
  value: string | number | boolean | null | undefined,
  evidenceRefs: ReadonlyArray<string> = [],
  unit?: string,
): ReadonlyArray<GraphDatum> =>
  value === undefined || value === null
    ? []
    : [{
        label,
        value,
        evidenceRefs,
        ...(unit === undefined ? {} : { unit }),
      }]

const node = (input: {
  id: string
  label: string
  kind: string
  status: GraphNodeStatus
  inputs?: ReadonlyArray<GraphPin>
  outputs?: ReadonlyArray<GraphPin>
  datum?: ReadonlyArray<GraphDatum>
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
  x: number
  y: number
}): GraphNode => ({
  id: input.id,
  label: input.label,
  kind: input.kind,
  status: input.status,
  inputs: [...(input.inputs ?? [])],
  outputs: [...(input.outputs ?? [])],
  datum: [...(input.datum ?? [])],
  evidenceRefs: [...(input.evidenceRefs ?? [])],
  blockerRefs: [...(input.blockerRefs ?? [])],
  caveatRefs: [...(input.caveatRefs ?? [])],
  position: { x: input.x, y: input.y },
})

const link = (input: {
  id: string
  label: string
  fromNodeId: string
  fromPinId: string
  toNodeId: string
  toPinId: string
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
}): GraphLink => {
  const evidenceRefs = [...(input.evidenceRefs ?? [])]
  const blockerRefs = [...(input.blockerRefs ?? [])]
  return {
    id: input.id,
    label: input.label,
    status: graphLinkStatusForRefs(evidenceRefs, blockerRefs),
    from: { nodeId: input.fromNodeId, pinId: input.fromPinId },
    to: { nodeId: input.toNodeId, pinId: input.toPinId },
    evidenceRefs,
    blockerRefs,
    caveatRefs: [...(input.caveatRefs ?? [])],
  }
}

const readyAccounts = (
  accounts: ReadonlyArray<KhalaCodeDesktopFleetAccount>,
): number => accounts.filter(account => accountState(account) === "ready").length

const graphStatus = (
  status: KhalaCodeDesktopFleetStatus,
  readyAccountCount: number,
): GraphNodeStatus => {
  if (status.pylon.status === "unavailable") return "blocked"
  if (status.accounts.length > 0 && readyAccountCount === 0) return "blocked"
  if (status.activeAssignments.length > 0 || status.processes.length > 0) {
    return "active"
  }
  return "complete"
}

const capacityNodeStatus = (
  status: KhalaCodeDesktopFleetStatus,
): GraphNodeStatus => {
  if (status.pylon.status === "unavailable") return "blocked"
  if (status.availableCodexAssignments === null || status.maxCodexAssignments === null) {
    return "idle"
  }
  if (status.maxCodexAssignments === 0 || status.availableCodexAssignments === 0) {
    return "blocked"
  }
  if (status.availableCodexAssignments < status.maxCodexAssignments) return "active"
  return "complete"
}

const accountNodeStatus = (
  accounts: ReadonlyArray<KhalaCodeDesktopFleetAccount>,
): GraphNodeStatus => {
  if (accounts.length === 0) return "idle"
  if (readyAccounts(accounts) === 0) return "blocked"
  if (readyAccounts(accounts) < accounts.length) return "active"
  return "complete"
}

const activeNodeStatus = (
  status: KhalaCodeDesktopFleetStatus,
): GraphNodeStatus =>
  status.activeAssignments.length > 0 || status.processes.length > 0
    ? "active"
    : "idle"

const timelineEvent = (input: {
  id: string
  label: string
  status: KhalaFleetTimelineEventStatus
  observedAt: string | null | undefined
  subjectRef: string
  detail: string
  evidenceRefs?: ReadonlyArray<string>
  blockerRefs?: ReadonlyArray<string>
  caveatRefs?: ReadonlyArray<string>
}): KhalaFleetTimelineEvent => ({
  id: input.id,
  label: input.label,
  status: input.status,
  observedAt: input.observedAt ?? "time.khala_fleet_board.local",
  subjectRef: input.subjectRef,
  detail: input.detail,
  evidenceRefs: [...(input.evidenceRefs ?? [])],
  blockerRefs: [...(input.blockerRefs ?? [])],
  caveatRefs: [...(input.caveatRefs ?? [])],
})

const assignmentRef = (
  assignment: KhalaCodeDesktopFleetAssignment,
  index: number,
): string => refOrFallback(assignment.assignmentRef, `assignment.khala_fleet.pending.${index + 1}`)

const issueRef = (
  assignment: KhalaCodeDesktopFleetAssignment,
  index: number,
): string => refOrFallback(assignment.issueRef, `issue.khala_fleet.unset.${index + 1}`)

const accountRef = (
  account: KhalaCodeDesktopFleetAccount,
  index: number,
): string => refOrFallback(account.accountRef, `account.khala_fleet.codex.${index + 1}`)

const processRef = (
  process: KhalaCodeDesktopFleetProcess,
  index: number,
): string => refOrFallback(process.pid, `process.khala_fleet.codex_exec.${index + 1}`)

const buildTimeline = (
  status: KhalaCodeDesktopFleetStatus,
  refs: {
    pylonRef: string
    capacityRefs: ReadonlyArray<string>
    accountRefs: ReadonlyArray<string>
    assignmentRefs: ReadonlyArray<string>
    processRefs: ReadonlyArray<string>
    blockerRefs: ReadonlyArray<string>
    caveatRefs: ReadonlyArray<string>
  },
): ReadonlyArray<KhalaFleetTimelineEvent> => {
  const events: KhalaFleetTimelineEvent[] = [
    timelineEvent({
      id: "main-codex-session",
      label: "Main Codex session",
      status: "complete",
      observedAt: status.observedAt,
      subjectRef: "session.khala_code.main_codex",
      detail: "Primary chat runs on the local Codex harness; swarm workers stay outside it.",
      evidenceRefs: ["session.khala_code.main_codex"],
      caveatRefs: ["caveat.khala_fleet.main_session_not_worker"],
    }),
    timelineEvent({
      id: "pylon-status",
      label: "Pylon status",
      status: status.pylon.status === "unavailable" ? "blocked" : "complete",
      observedAt: status.observedAt,
      subjectRef: refs.pylonRef,
      detail: `Pylon is ${pylonStatusLabel(status.pylon.status)}.`,
      evidenceRefs: status.pylon.status === "unavailable" ? [] : [refs.pylonRef],
      blockerRefs: status.pylon.status === "unavailable" ? refs.blockerRefs : [],
    }),
    timelineEvent({
      id: "capacity",
      label: "Capacity advertised",
      status: capacityNodeStatus(status) === "blocked" ? "blocked" : "complete",
      observedAt: status.observedAt,
      subjectRef: refs.capacityRefs[0] ?? "capacity.khala_fleet.codex.unknown",
      detail:
        status.availableCodexAssignments === null ||
        status.maxCodexAssignments === null
          ? "Capacity count is not reported yet."
          : `${status.availableCodexAssignments}/${status.maxCodexAssignments} Codex slots free.`,
      evidenceRefs: refs.capacityRefs,
      blockerRefs: capacityNodeStatus(status) === "blocked" ? refs.blockerRefs : [],
    }),
  ]

  status.accounts.forEach((account, index) => {
    const ref = refs.accountRefs[index] ?? `account.khala_fleet.codex.${index + 1}`
    const state = accountState(account)
    events.push(
      timelineEvent({
        id: `account-${index + 1}`,
        label: safeLabel(account.accountRef, `Codex account ${index + 1}`),
        status: state === "ready" ? "complete" : state === "missing" ? "blocked" : "active",
        observedAt: status.observedAt,
        subjectRef: ref,
        detail: `Readiness is ${accountStateLabel(account)}.`,
        evidenceRefs: state === "ready" ? [ref] : [],
        blockerRefs: state === "ready" ? [] : [`blocker.khala_fleet.account.${index + 1}`],
      }),
    )
  })

  status.activeAssignments.forEach((assignment, index) => {
    const ref = refs.assignmentRefs[index] ?? `assignment.khala_fleet.pending.${index + 1}`
    const publicIssueRef = issueRef(assignment, index)
    events.push(
      timelineEvent({
        id: `assignment-${index + 1}`,
        label: "Assignment active",
        status: "active",
        observedAt: assignment.updatedAt ?? status.observedAt,
        subjectRef: ref,
        detail: `Active run for ${publicIssueRef}.`,
        evidenceRefs: [ref, publicIssueRef],
      }),
    )
  })

  status.processes.forEach((process, index) => {
    const ref = refs.processRefs[index] ?? `process.khala_fleet.codex_exec.${index + 1}`
    events.push(
      timelineEvent({
        id: `process-${index + 1}`,
        label: "Codex process",
        status: "active",
        observedAt: status.observedAt,
        subjectRef: ref,
        detail: `Process ${ref} has run for ${safeLabel(process.elapsed, "elapsed time unknown")}.`,
        evidenceRefs: [ref],
      }),
    )
  })

  return events.sort((left, right) => {
    const byTime = left.observedAt.localeCompare(right.observedAt)
    return byTime === 0 ? left.id.localeCompare(right.id) : byTime
  })
}

export const buildKhalaFleetBoardProjection = (
  input: KhalaFleetBoardProjectionInput,
): KhalaFleetBoardProjection => {
  const { status } = input
  const generatedAt = input.generatedAt ?? status.observedAt
  const readyAccountCount = readyAccounts(status.accounts)
  const mainSessionRef = "session.khala_code.main_codex"
  const pylonRef = refOrFallback(status.pylon.pylonRef, "pylon.khala_fleet.local")
  const accountRefs = status.accounts.map(accountRef)
  const assignmentRefs = status.activeAssignments.map(assignmentRef)
  const processRefs = status.processes.map(processRef)
  const capacityRefs = publicRefs([
    pylonRef,
    status.availableCodexAssignments === null
      ? undefined
      : `capacity.khala_fleet.codex.available.${status.availableCodexAssignments}`,
    status.maxCodexAssignments === null
      ? undefined
      : `capacity.khala_fleet.codex.max.${status.maxCodexAssignments}`,
  ])
  const blockerRefs = publicRefs([
    status.pylon.status === "unavailable"
      ? "blocker.khala_fleet.pylon_unavailable"
      : undefined,
    status.accounts.length > 0 && readyAccountCount === 0
      ? "blocker.khala_fleet.no_ready_codex_accounts"
      : undefined,
    status.availableCodexAssignments === 0 && status.maxCodexAssignments !== 0
      ? "blocker.khala_fleet.no_free_codex_slots"
      : undefined,
  ])
  const caveatRefs = publicRefs([
    "caveat.khala_fleet.main_session_not_worker",
    "caveat.khala_fleet.tool_scope_projection_pending",
    status.maxCodexAssignments === null
      ? "caveat.khala_fleet.capacity_not_reported"
      : undefined,
  ])
  const allEvidenceRefs = publicRefs([
    pylonRef,
    mainSessionRef,
    ...accountRefs,
    ...assignmentRefs,
    ...status.activeAssignments.map(issueRef),
    ...processRefs,
    ...capacityRefs,
  ])
  const boardStatus = graphStatus(status, readyAccountCount)
  const activeCount = status.activeAssignments.length
  const processCount = status.processes.length

  const nodes: ReadonlyArray<GraphNode> = [
    node({
      id: "khala-code",
      label: "Khala Code",
      kind: "operator_surface",
      status: status.ok ? "complete" : "blocked",
      outputs: [
        pin("output", "main-session", "main session", "codex.main.session"),
        pin("output", "fleet-status", "fleet status", "khala.fleet.status"),
      ],
      datum: datum("observed", generatedAt, [pylonRef]),
      evidenceRefs: [pylonRef],
      x: 40,
      y: 70,
    }),
    node({
      id: "main-codex-session",
      label: "Main Codex session",
      kind: "main_codex_session",
      status: status.ok ? "complete" : "blocked",
      inputs: [pin("input", "main-session", "main session", "codex.main.session")],
      outputs: [pin("output", "chat", "chat", "codex.app_server.thread")],
      datum: datum("home", "main user Codex home", [mainSessionRef]),
      evidenceRefs: [mainSessionRef],
      caveatRefs: ["caveat.khala_fleet.main_session_not_worker"],
      x: 255,
      y: 250,
    }),
    node({
      id: "local-pylon",
      label: "Local Pylon",
      kind: "capacity_host",
      status: status.pylon.status === "unavailable" ? "blocked" : "complete",
      inputs: [pin("input", "fleet-status", "fleet status", "khala.fleet.status")],
      outputs: [pin("output", "capacity", "capacity", "pylon.capacity")],
      datum: datum("status", pylonStatusLabel(status.pylon.status), [pylonRef]),
      evidenceRefs: status.pylon.status === "unavailable" ? [] : [pylonRef],
      blockerRefs: status.pylon.status === "unavailable" ? blockerRefs : [],
      x: 255,
      y: 70,
    }),
    node({
      id: "capacity-gate",
      label: "Capacity gate",
      kind: "dispatch_gate",
      status: capacityNodeStatus(status),
      inputs: [pin("input", "capacity", "capacity", "pylon.capacity")],
      outputs: [pin("output", "workers", "workers", "codex.worker.pool")],
      datum: [
        ...datum("available", status.availableCodexAssignments, capacityRefs),
        ...datum("max", status.maxCodexAssignments, capacityRefs),
      ],
      evidenceRefs: capacityRefs,
      blockerRefs: capacityNodeStatus(status) === "blocked" ? blockerRefs : [],
      caveatRefs,
      x: 470,
      y: 70,
    }),
    node({
      id: "codex-workers",
      label: "Codex workers",
      kind: "worker_pool",
      status: accountNodeStatus(status.accounts),
      inputs: [pin("input", "workers", "workers", "codex.worker.pool")],
      outputs: [pin("output", "assignments", "assignments", "codex.assignment")],
      datum: [
        ...datum("ready", readyAccountCount, accountRefs),
        ...datum("total", status.accounts.length, accountRefs),
      ],
      evidenceRefs: accountRefs,
      blockerRefs: readyAccountCount === 0 && status.accounts.length > 0 ? blockerRefs : [],
      caveatRefs,
      x: 685,
      y: 70,
    }),
    node({
      id: "active-assignments",
      label: "Active runs",
      kind: "assignment_board",
      status: activeNodeStatus(status),
      inputs: [pin("input", "assignments", "assignments", "codex.assignment")],
      outputs: [pin("output", "timeline", "timeline", "khala.fleet.timeline")],
      datum: datum("active", activeCount, assignmentRefs),
      evidenceRefs: assignmentRefs,
      x: 900,
      y: 70,
    }),
    node({
      id: "codex-processes",
      label: "Codex exec",
      kind: "local_processes",
      status: processCount > 0 ? "active" : "idle",
      inputs: [pin("input", "assignments", "assignments", "codex.assignment")],
      outputs: [pin("output", "processes", "processes", "codex.exec.process")],
      datum: datum("running", processCount, processRefs),
      evidenceRefs: processRefs,
      x: 900,
      y: 250,
    }),
    node({
      id: "run-timeline",
      label: "Run timeline",
      kind: "trace_view",
      status: activeCount > 0 || processCount > 0 ? "active" : "idle",
      inputs: [
        pin("input", "timeline", "timeline", "khala.fleet.timeline"),
        pin("input", "processes", "processes", "codex.exec.process"),
      ],
      datum: datum("events", 3 + status.accounts.length + activeCount + processCount, allEvidenceRefs),
      evidenceRefs: allEvidenceRefs,
      caveatRefs,
      x: 1115,
      y: 160,
    }),
  ]

  const links: ReadonlyArray<GraphLink> = [
    link({
      id: "khala-to-main-codex",
      label: "wraps",
      fromNodeId: "khala-code",
      fromPinId: "main-session",
      toNodeId: "main-codex-session",
      toPinId: "main-session",
      evidenceRefs: [mainSessionRef],
    }),
    link({
      id: "khala-to-pylon",
      label: "status",
      fromNodeId: "khala-code",
      fromPinId: "fleet-status",
      toNodeId: "local-pylon",
      toPinId: "fleet-status",
      evidenceRefs: [pylonRef],
      blockerRefs: status.pylon.status === "unavailable" ? blockerRefs : [],
    }),
    link({
      id: "pylon-to-capacity",
      label: "advertises",
      fromNodeId: "local-pylon",
      fromPinId: "capacity",
      toNodeId: "capacity-gate",
      toPinId: "capacity",
      evidenceRefs: capacityRefs,
      blockerRefs: status.pylon.status === "unavailable" ? blockerRefs : [],
    }),
    link({
      id: "capacity-to-workers",
      label: "admits",
      fromNodeId: "capacity-gate",
      fromPinId: "workers",
      toNodeId: "codex-workers",
      toPinId: "workers",
      evidenceRefs: accountRefs,
      blockerRefs: readyAccountCount === 0 && status.accounts.length > 0 ? blockerRefs : [],
      caveatRefs,
    }),
    link({
      id: "workers-to-assignments",
      label: "leases",
      fromNodeId: "codex-workers",
      fromPinId: "assignments",
      toNodeId: "active-assignments",
      toPinId: "assignments",
      evidenceRefs: assignmentRefs,
    }),
    link({
      id: "workers-to-processes",
      label: "executes",
      fromNodeId: "codex-workers",
      fromPinId: "assignments",
      toNodeId: "codex-processes",
      toPinId: "assignments",
      evidenceRefs: processRefs,
    }),
    link({
      id: "assignments-to-timeline",
      label: "events",
      fromNodeId: "active-assignments",
      fromPinId: "timeline",
      toNodeId: "run-timeline",
      toPinId: "timeline",
      evidenceRefs: assignmentRefs,
    }),
    link({
      id: "processes-to-timeline",
      label: "activity",
      fromNodeId: "codex-processes",
      fromPinId: "processes",
      toNodeId: "run-timeline",
      toPinId: "processes",
      evidenceRefs: processRefs,
    }),
  ]

  const graph: GraphSpec = {
    schemaVersion: "openagents.arbiter.graph_spec.v0",
    title: "Khala Code Fleet board",
    generatedAt,
    status: boardStatus,
    nodes,
    links,
    evidenceRefs: allEvidenceRefs,
    blockerRefs,
    caveatRefs,
    sourceRefs: publicRefs([
      "doc.khala_code.fleet_management_spec.2026_06_30",
      "source.khala_code_desktop.codex_fleet_status",
    ]),
  }

  const projection: KhalaFleetBoardProjection = {
    schemaVersion: KhalaFleetBoardProjectionSchemaVersion,
    generatedAt,
    graph,
    timeline: buildTimeline(status, {
      pylonRef,
      capacityRefs,
      accountRefs,
      assignmentRefs,
      processRefs,
      blockerRefs,
      caveatRefs,
    }),
    summary: {
      readyAccounts: readyAccountCount,
      totalAccounts: status.accounts.length,
      activeAssignments: activeCount,
      runningProcesses: processCount,
      availableCodexAssignments: status.availableCodexAssignments,
      maxCodexAssignments: status.maxCodexAssignments,
    },
    evidenceRefs: allEvidenceRefs,
    blockerRefs,
    caveatRefs,
    sourceRefs: graph.sourceRefs,
  }

  assertProjectionPublicSafe(projection)
  return projection
}
