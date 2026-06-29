export const OPENAGENTS_DESKTOP_CODING_POLL_INTERVAL_MS = 5_000

export type CodingProcessKind =
  | "assignment_runner"
  | "codex_exec"
  | "khala_request"
  | "pylon_node"
  | "standing_pylon"
  | "supervisor"
  | "vertex_burn"

export type CodingProcess = {
  readonly accountRef: string | null
  readonly age: string
  readonly assignmentRef: string | null
  readonly cpuPercent: number
  readonly issueRef: string | null
  readonly kind: CodingProcessKind
  readonly label: string
  readonly parentPid: number
  readonly pid: number
  readonly status: "active" | "idle"
  readonly workspacePath: string | null
}

export type CodingAssignmentRun = {
  readonly accountRefHash: string | null
  readonly assignmentRef: string
  readonly blockerRefs: readonly string[]
  readonly closeoutRef: string | null
  readonly closeoutStatus: string | null
  readonly elapsedMs: number | null
  readonly lastEvent: string | null
  readonly lastEventAt: string | null
  readonly leaseRef: string | null
  readonly phase: string | null
  readonly refreshedAt: string | null
  readonly runRef: string | null
  readonly startedAt: string | null
}

export type CodingSupervisorEvent = {
  readonly accountRef: string | null
  readonly issueRef: string | null
  readonly slot: number | null
  readonly status: string
  readonly text: string
  readonly timestamp: string | null
}

export type CodingTranscriptRole =
  | "assistant"
  | "developer"
  | "event"
  | "reasoning"
  | "system"
  | "tool"
  | "user"

export type CodingTranscriptStatus = "error" | "info" | "ok" | "running"

export type CodingTranscriptMessage = {
  readonly detail: string | null
  readonly kind: string
  readonly role: CodingTranscriptRole
  readonly status: CodingTranscriptStatus
  readonly text: string
  readonly timestamp: string | null
  readonly title: string
}

export type CodingCodexSession = {
  readonly accountRef: string | null
  readonly active: boolean
  readonly assignment: CodingAssignmentRun | null
  readonly cwd: string | null
  readonly issueRef: string | null
  readonly messageCount: number
  readonly messages: readonly CodingTranscriptMessage[]
  readonly modifiedAt: string
  readonly path: string
  readonly pid: number | null
  readonly sessionId: string
  readonly source: string | null
  readonly status: "active" | "idle" | "recent"
  readonly title: string
}

export type ParsedCodexSessionRollout = {
  readonly cwd: string | null
  readonly messageCount: number
  readonly messages: readonly CodingTranscriptMessage[]
  readonly sessionId: string | null
  readonly source: string | null
  readonly title: string | null
}

export type CodingStatusSummary = {
  readonly assignmentRunnerCount: number
  readonly burningCodexCount: number
  readonly claimCount: number
  readonly codexExecCount: number
  readonly desiredSlots: number | null
  readonly khalaRequestCount: number
  readonly lastDispatchAt: string | null
  readonly lockoutRecent: number
  readonly noDispatchRecent: number
  readonly okRecent: number
  readonly openIssueCount: number | null
  readonly pylonNodeCount: number
  readonly readyCodex: number | null
  readonly standingPylonCount: number
  readonly supervisorCount: number
  readonly vertexBurnCount: number
}

export type CodingStatusResult =
  | {
      readonly ok: true
      readonly events: readonly CodingSupervisorEvent[]
      readonly observedAt: string
      readonly processes: readonly CodingProcess[]
      readonly sessions: readonly CodingCodexSession[]
      readonly summary: CodingStatusSummary
    }
  | {
      readonly ok: false
      readonly error: string
      readonly events: readonly CodingSupervisorEvent[]
      readonly observedAt: string
      readonly processes: readonly CodingProcess[]
      readonly sessions: readonly CodingCodexSession[]
      readonly summary: CodingStatusSummary
    }

const processKindLabel: Record<CodingProcessKind, string> = {
  assignment_runner: "Assignment runner",
  codex_exec: "Codex exec",
  khala_request: "Khala request",
  pylon_node: "Pylon node",
  standing_pylon: "Standing Pylon",
  supervisor: "Supervisor",
  vertex_burn: "Vertex burn",
}

const processKindFromCommand = (command: string): CodingProcessKind | null => {
  if (/codex\/vendor.*bin\/codex exec|(^|\s)codex exec(\s|$)/i.test(command)) {
    return "codex_exec"
  }
  if (/\bkhala request\b/i.test(command)) return "khala_request"
  if (/\bassignment run-no-spend\b/i.test(command)) return "assignment_runner"
  if (/codex-supervisor\.sh/i.test(command)) return "supervisor"
  if (/khala-vertex-continual-learning/i.test(command)) return "vertex_burn"
  if (/standing-pylon\.sh/i.test(command)) return "standing_pylon"
  if (/apps\/pylon\/src\/index\.ts node/i.test(command)) return "pylon_node"
  return null
}

const accountRefFromCommand = (command: string): string | null =>
  command.match(/(?:--account-ref\s+)(codex-\d+)/)?.[1] ??
  command.match(/(?:\/|=|\s)(codex-\d+)(?:\/|\s|$)/)?.[1] ??
  null

const issueRefFromCommand = (command: string): string | null =>
  command.match(/issue\s+#?(\d+)/i)?.[1] ??
  command.match(/#(\d{3,})/)?.[1] ??
  null

const assignmentRefFromCommand = (command: string): string | null =>
  command.match(/(?:^|\s)--assignment-ref\s+((?:"[^"]+")|(?:'[^']+')|\S+)/)?.[1]
    ?.replace(/^(['"])(.*)\1$/, "$2") ??
  command.match(/\bassignment\.public\.[a-z0-9_.-]+/i)?.[0] ??
  null

const unquoteShellToken = (value: string): string =>
  value.replace(/^(['"])(.*)\1$/, "$2")

const workspacePathFromCommand = (command: string): string | null => {
  const match = command.match(/(?:^|\s)--(?:cd|cwd)\s+((?:"[^"]+")|(?:'[^']+')|\S+)/)
  return match === null ? null : unquoteShellToken(match[1])
}

const labelForProcess = (
  kind: CodingProcessKind,
  accountRef: string | null,
  issueRef: string | null,
): string => {
  const base = processKindLabel[kind]
  const detail = [accountRef, issueRef === null ? null : `#${issueRef}`]
    .filter((value): value is string => value !== null)
    .join(" ")
  return detail === "" ? base : `${base} ${detail}`
}

export const parseCodingProcesses = (
  psOutput: string,
): readonly CodingProcess[] => {
  const rows: CodingProcess[] = []
  for (const line of psOutput.split("\n")) {
    const match = line.match(/^\s*(\d+)\s+\d+\s+([0-9.]+)\s+(\S+)\s+(.+)$/)
    if (match === null) continue

    const [, pidValue, cpuValue, age, command] = match
    const parentPidValue = line.match(/^\s*\d+\s+(\d+)\s+/)?.[1] ?? "0"
    const kind = processKindFromCommand(command)
    if (kind === null) continue

    const pid = Number(pidValue)
    const parentPid = Number(parentPidValue)
    const cpuPercent = Number(cpuValue)
    if (
      !Number.isFinite(pid) ||
      !Number.isFinite(parentPid) ||
      !Number.isFinite(cpuPercent)
    ) {
      continue
    }

    const accountRef = accountRefFromCommand(command)
    const assignmentRef = assignmentRefFromCommand(command)
    const issueRef = issueRefFromCommand(command)
    rows.push({
      accountRef,
      age,
      assignmentRef,
      cpuPercent,
      issueRef,
      kind,
      label: labelForProcess(kind, accountRef, issueRef),
      parentPid,
      pid,
      status: cpuPercent >= 5 ? "active" : "idle",
      workspacePath: workspacePathFromCommand(command),
    })
  }

  const byPid = new Map(rows.map(row => [row.pid, row]))
  return rows.map(row => {
    const parent = byPid.get(row.parentPid)
    const accountRef = row.accountRef ?? parent?.accountRef ?? null
    const assignmentRef = row.assignmentRef ?? parent?.assignmentRef ?? null
    const issueRef = row.issueRef ?? parent?.issueRef ?? null
    return {
      ...row,
      accountRef,
      assignmentRef,
      issueRef,
      label: labelForProcess(row.kind, accountRef, issueRef),
    }
  })
}

const numberValueFromUnknown = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const stringArrayFromUnknown = (value: unknown): readonly string[] =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : []

const assignmentRunFromUnknown = (value: unknown): CodingAssignmentRun | null => {
  if (!isJsonObject(value)) return null
  const assignmentRef = stringValueFromUnknown(value.assignmentRef)
  if (assignmentRef === null) return null
  const observedAt = stringValueFromUnknown(value.observedAt)
  const event = stringValueFromUnknown(value.event)
  const status = stringValueFromUnknown(value.status)
  const phase = stringValueFromUnknown(value.phase)
  return {
    accountRefHash: stringValueFromUnknown(value.accountRefHash),
    assignmentRef,
    blockerRefs: stringArrayFromUnknown(value.blockerRefs),
    closeoutRef: stringValueFromUnknown(value.closeoutRef),
    closeoutStatus:
      event === "assignment_run.closeout_submitted" ||
      event === "assignment_run.completed"
        ? status
        : null,
    elapsedMs: numberValueFromUnknown(value.elapsedMs),
    lastEvent: event,
    lastEventAt: observedAt,
    leaseRef: stringValueFromUnknown(value.leaseRef),
    phase: phase ?? status,
    refreshedAt: stringValueFromUnknown(value.refreshedAt),
    runRef: stringValueFromUnknown(value.runRef),
    startedAt:
      event === "assignment_run.accepted" || event === "assignment_run.runtime_started"
        ? observedAt
        : stringValueFromUnknown(value.startedAt),
  }
}

export const mergeAssignmentRuns = (
  runs: readonly CodingAssignmentRun[],
): readonly CodingAssignmentRun[] => {
  const byAssignment = new Map<string, CodingAssignmentRun>()

  for (const run of runs) {
    const current = byAssignment.get(run.assignmentRef)
    byAssignment.set(run.assignmentRef, {
      accountRefHash: run.accountRefHash ?? current?.accountRefHash ?? null,
      assignmentRef: run.assignmentRef,
      blockerRefs:
        run.blockerRefs.length > 0 ? run.blockerRefs : current?.blockerRefs ?? [],
      closeoutRef: run.closeoutRef ?? current?.closeoutRef ?? null,
      closeoutStatus: run.closeoutStatus ?? current?.closeoutStatus ?? null,
      elapsedMs: Math.max(run.elapsedMs ?? 0, current?.elapsedMs ?? 0) || null,
      lastEvent: run.lastEvent ?? current?.lastEvent ?? null,
      lastEventAt: run.lastEventAt ?? current?.lastEventAt ?? null,
      leaseRef: run.leaseRef ?? current?.leaseRef ?? null,
      phase: run.phase ?? current?.phase ?? null,
      refreshedAt: run.refreshedAt ?? current?.refreshedAt ?? null,
      runRef: run.runRef ?? current?.runRef ?? null,
      startedAt: current?.startedAt ?? run.startedAt,
    })
  }

  return [...byAssignment.values()]
}

export const parseAssignmentRunLifecycleLog = (
  logText: string,
): readonly CodingAssignmentRun[] => {
  const runs: CodingAssignmentRun[] = []

  for (const line of logText.split("\n")) {
    const trimmed = line.trim()
    if (!trimmed.startsWith("{")) continue
    let record: unknown
    try {
      record = JSON.parse(trimmed)
    } catch {
      continue
    }
    if (!isJsonObject(record)) continue
    if (
      record.schema !== "openagents.pylon.assignment_run_lifecycle_event.v0.1" &&
      record.schema !== "openagents.pylon.active_assignment_run.v0.1"
    ) {
      continue
    }
    const run = assignmentRunFromUnknown(record)
    if (run !== null) runs.push(run)
  }

  return mergeAssignmentRuns(runs)
}

export const emptyCodingStatusSummary = (): CodingStatusSummary => ({
  assignmentRunnerCount: 0,
  burningCodexCount: 0,
  claimCount: 0,
  codexExecCount: 0,
  desiredSlots: null,
  khalaRequestCount: 0,
  lastDispatchAt: null,
  lockoutRecent: 0,
  noDispatchRecent: 0,
  okRecent: 0,
  openIssueCount: null,
  pylonNodeCount: 0,
  readyCodex: null,
  standingPylonCount: 0,
  supervisorCount: 0,
  vertexBurnCount: 0,
})

export const summarizeCodingProcesses = (
  processes: readonly CodingProcess[],
): Pick<
  CodingStatusSummary,
  | "assignmentRunnerCount"
  | "burningCodexCount"
  | "codexExecCount"
  | "khalaRequestCount"
  | "pylonNodeCount"
  | "standingPylonCount"
  | "supervisorCount"
  | "vertexBurnCount"
> => ({
  assignmentRunnerCount: processes.filter(
    process => process.kind === "assignment_runner",
  ).length,
  burningCodexCount: processes.filter(
    process => process.kind === "codex_exec" && process.status === "active",
  ).length,
  codexExecCount: processes.filter(process => process.kind === "codex_exec")
    .length,
  khalaRequestCount: processes.filter(
    process => process.kind === "khala_request",
  ).length,
  pylonNodeCount: processes.filter(process => process.kind === "pylon_node")
    .length,
  standingPylonCount: processes.filter(
    process => process.kind === "standing_pylon",
  ).length,
  supervisorCount: processes.filter(process => process.kind === "supervisor")
    .length,
  vertexBurnCount: processes.filter(process => process.kind === "vertex_burn")
    .length,
})

export const parseSupervisorLog = (
  logText: string,
): {
  readonly desiredSlots: number | null
  readonly events: readonly CodingSupervisorEvent[]
  readonly lastDispatchAt: string | null
  readonly lockoutRecent: number
  readonly noDispatchRecent: number
  readonly okRecent: number
  readonly readyCodex: number | null
} => {
  let desiredSlots: number | null = null
  let lastDispatchAt: string | null = null
  let readyCodex: number | null = null
  let lockoutRecent = 0
  let noDispatchRecent = 0
  let okRecent = 0
  const events: CodingSupervisorEvent[] = []
  const lines = logText.split("\n").filter(line => line.trim() !== "")

  for (const line of lines.slice(-240)) {
    const heartbeat = line.match(
      /heartbeat ready_codex=(\d+) desired_slots=(\d+).*last_dispatch_time=(\d+)/,
    )
    if (heartbeat !== null) {
      readyCodex = Number(heartbeat[1])
      desiredSlots = Number(heartbeat[2])
      const dispatchMillis = Number(heartbeat[3])
      lastDispatchAt = Number.isFinite(dispatchMillis)
        ? new Date(dispatchMillis).toISOString()
        : null
    }

    if (/OK \(rc=0\)/.test(line)) okRecent += 1
    if (/\bNO-DISPATCH\b/.test(line)) noDispatchRecent += 1
    if (/\bLOCKOUT\b/.test(line)) lockoutRecent += 1

    const event = line.match(
      /^(\S+)\s+slot=(\d+)\s+acc=(codex-\d+)\s+issue=#(\d+)\s+([A-Z-]+)/,
    )
    if (event !== null) {
      events.push({
        accountRef: event[3],
        issueRef: event[4],
        slot: Number(event[2]),
        status: event[5],
        text: line.replace(/^\S+\s+/, ""),
        timestamp: event[1],
      })
      continue
    }

    if (/Pylon presence failed|VMQ unavailable|dispatch_gate|unauthorized/i.test(line)) {
      events.push({
        accountRef: null,
        issueRef: null,
        slot: null,
        status: /unauthorized/i.test(line)
          ? "AUTH"
          : /VMQ unavailable/i.test(line)
            ? "VMQ"
            : "ERROR",
        text: line.length > 180 ? `${line.slice(0, 177)}...` : line,
        timestamp: line.match(/^(\S+)/)?.[1] ?? null,
      })
    }
  }

  return {
    desiredSlots,
    events: events.slice(-12).reverse(),
    lastDispatchAt,
    lockoutRecent,
    noDispatchRecent,
    okRecent,
    readyCodex,
  }
}

type JsonObject = Record<string, unknown>

const isJsonObject = (value: unknown): value is JsonObject =>
  typeof value === "object" && value !== null && !Array.isArray(value)

const stringValueFromUnknown = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const truncateTranscriptText = (value: string): string => {
  const trimmed = value.replace(/\s+$/g, "")
  return trimmed.length > 2_400 ? `${trimmed.slice(0, 2_397)}...` : trimmed
}

const transcriptMessage = (
  message: Omit<CodingTranscriptMessage, "detail" | "status" | "title"> &
    Partial<Pick<CodingTranscriptMessage, "detail" | "status" | "title">>,
): CodingTranscriptMessage => ({
  detail: message.detail ?? null,
  kind: message.kind,
  role: message.role,
  status: message.status ?? "info",
  text: message.text,
  timestamp: message.timestamp,
  title: message.title ?? message.kind,
})

const textFromUnknown = (value: unknown): string | null => {
  if (typeof value === "string") return value
  if (Array.isArray(value)) {
    const text = value
      .map(item => textFromUnknown(item))
      .filter((item): item is string => item !== null)
      .join("\n")
      .trim()
    return text === "" ? null : text
  }
  if (!isJsonObject(value)) return null

  const direct =
    stringValueFromUnknown(value.text) ??
    stringValueFromUnknown(value.output_text) ??
    stringValueFromUnknown(value.input_text) ??
    stringValueFromUnknown(value.content)
  if (direct !== null) return direct

  return (
    textFromUnknown(value.content_items) ??
    textFromUnknown(value.items) ??
    textFromUnknown(value.summary)
  )
}

const compactJsonFromUnknown = (value: unknown): string | null => {
  if (typeof value === "string") return value
  if (value === null || value === undefined) return null
  try {
    return JSON.stringify(value, null, 2)
  } catch {
    return null
  }
}

const transcriptRoleFromMessageRole = (role: string | null): CodingTranscriptRole => {
  if (
    role === "assistant" ||
    role === "developer" ||
    role === "system" ||
    role === "user"
  ) {
    return role
  }
  return "event"
}

const transcriptMessageFromResponseItem = (
  payload: JsonObject,
  timestamp: string | null,
): CodingTranscriptMessage | null => {
  const type = stringValueFromUnknown(payload.type)
  if (type === null) return null

  if (type === "message") {
    const text = textFromUnknown(payload.content)
    if (text === null || text.trim() === "") return null
    const role = transcriptRoleFromMessageRole(stringValueFromUnknown(payload.role))
    return transcriptMessage({
      kind: "message",
      role,
      text: truncateTranscriptText(text),
      timestamp,
      title: role,
    })
  }

  if (type === "agent_message") {
    const text = textFromUnknown(payload.content)
    if (text === null || text.trim() === "") return null
    return transcriptMessage({
      kind: "agent message",
      role: "assistant",
      text: truncateTranscriptText(text),
      timestamp,
      title: "assistant",
    })
  }

  if (type === "reasoning") {
    const text = textFromUnknown(payload.summary) ?? "Reasoning"
    return transcriptMessage({
      kind: "reasoning",
      role: "reasoning",
      text: truncateTranscriptText(text),
      timestamp,
      title: "reasoning",
    })
  }

  if (type === "function_call" || type === "custom_tool_call") {
    const name = stringValueFromUnknown(payload.name) ?? "tool"
    const args = stringValueFromUnknown(payload.arguments) ??
      stringValueFromUnknown(payload.input) ??
      compactJsonFromUnknown(payload.arguments) ??
      compactJsonFromUnknown(payload.input)
    return transcriptMessage({
      detail: stringValueFromUnknown(payload.call_id),
      kind: "tool call",
      role: "tool",
      status: "running",
      text: args === null ? "No input captured." : truncateTranscriptText(args),
      timestamp,
      title: name,
    })
  }

  if (type === "function_call_output" || type === "custom_tool_call_output") {
    const text = textFromUnknown(payload.output) ?? compactJsonFromUnknown(payload.output)
    if (text === null || text.trim() === "") return null
    const isError = payload.is_error === true
    return transcriptMessage({
      detail: stringValueFromUnknown(payload.call_id),
      kind: "tool output",
      role: "tool",
      status: isError ? "error" : "ok",
      text: truncateTranscriptText(text),
      timestamp,
      title: isError ? "tool error" : "tool result",
    })
  }

  if (type === "local_shell_call") {
    const text =
      textFromUnknown(payload.action) ??
      compactJsonFromUnknown(payload.action) ??
      "shell command"
    return transcriptMessage({
      detail: stringValueFromUnknown(payload.call_id),
      kind: "shell",
      role: "tool",
      status: "running",
      text: truncateTranscriptText(text),
      timestamp,
      title: "shell",
    })
  }

  return null
}

const transcriptMessageFromEvent = (
  payload: JsonObject,
  timestamp: string | null,
): CodingTranscriptMessage | null => {
  const type = stringValueFromUnknown(payload.type)
  if (type === null || type === "token_count") return null

  if (type === "user_message") {
    const text = stringValueFromUnknown(payload.message) ?? textFromUnknown(payload.text_elements)
    if (text === null || text.trim() === "") return null
    return transcriptMessage({
      kind: "user event",
      role: "user",
      text: truncateTranscriptText(text),
      timestamp,
      title: "user",
    })
  }

  if (type === "agent_message") {
    const text = stringValueFromUnknown(payload.message) ??
      stringValueFromUnknown(payload.text) ??
      textFromUnknown(payload.content)
    if (text === null || text.trim() === "") return null
    return transcriptMessage({
      kind: "agent event",
      role: "assistant",
      text: truncateTranscriptText(text),
      timestamp,
      title: "assistant",
    })
  }

  const text =
    stringValueFromUnknown(payload.message) ??
    stringValueFromUnknown(payload.error) ??
    stringValueFromUnknown(payload.text)
  if (text === null) return null

  const kind = type.replaceAll("_", " ")
  const isError = /\berror\b|failed|failure/i.test(kind)
  return transcriptMessage({
    kind,
    role: "event",
    status: isError ? "error" : "info",
    text: truncateTranscriptText(text),
    timestamp,
    title: isError ? "error" : kind,
  })
}

const firstTranscriptLine = (value: string): string | null => {
  const line = value.split("\n").find(part => part.trim() !== "")?.trim()
  return line === undefined || line === "" ? null : line
}

const titleFromJsonText = (value: string): string | null => {
  const trimmed = value.trim()
  if (!trimmed.startsWith("{")) return null
  try {
    const parsed = JSON.parse(trimmed)
    return isJsonObject(parsed) ? stringValueFromUnknown(parsed.title) : null
  } catch {
    return null
  }
}

const usefulTitleText = (value: string): boolean => {
  const normalized = value.trim()
  if (normalized === "") return false
  if (/^#\s*AGENTS\.md instructions\b/i.test(normalized)) return false
  if (/^<environment_context>/i.test(normalized)) return false
  if (/^You are writing the title and body for a GitHub pull request/i.test(normalized)) {
    return false
  }
  if (/^You are working in a bounded public repository\b/i.test(normalized)) {
    return false
  }
  return true
}

const sessionMetaPayload = (payload: unknown): JsonObject | null => {
  if (!isJsonObject(payload)) return null
  return isJsonObject(payload.meta) ? payload.meta : payload
}

export const parseCodexSessionRollout = (
  rolloutText: string,
): ParsedCodexSessionRollout => {
  let cwd: string | null = null
  let sessionId: string | null = null
  let source: string | null = null
  let title: string | null = null
  const messages: CodingTranscriptMessage[] = []

  for (const line of rolloutText.split("\n")) {
    if (line.trim() === "") continue
    let record: unknown
    try {
      record = JSON.parse(line)
    } catch {
      continue
    }
    if (!isJsonObject(record)) continue

    const timestamp = stringValueFromUnknown(record.timestamp)
    const type = stringValueFromUnknown(record.type)
    const payload = record.payload

    if (type === "session_meta") {
      const meta = sessionMetaPayload(payload)
      if (meta === null) continue
      cwd = cwd ?? stringValueFromUnknown(meta.cwd)
      sessionId =
        sessionId ??
        stringValueFromUnknown(meta.id) ??
        stringValueFromUnknown(meta.session_id)
      source = source ?? stringValueFromUnknown(meta.source)
      continue
    }

    if (!isJsonObject(payload)) continue

    const message =
      type === "response_item"
        ? transcriptMessageFromResponseItem(payload, timestamp)
        : type === "event_msg"
          ? transcriptMessageFromEvent(payload, timestamp)
          : null
    if (message === null) continue

    if (title === null && (message.role === "user" || message.role === "assistant")) {
      const candidate = titleFromJsonText(message.text) ?? firstTranscriptLine(message.text)
      if (candidate !== null && usefulTitleText(candidate)) title = candidate
    }
    messages.push(message)
  }

  return {
    cwd,
    messageCount: messages.length,
    messages: messages.slice(-24),
    sessionId,
    source,
    title,
  }
}
