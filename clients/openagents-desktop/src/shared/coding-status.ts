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
  readonly assignmentRef: string | null
  readonly closeout: {
    readonly blockerRefs: readonly string[]
    readonly closeoutRef: string | null
    readonly status: string | null
  }
  readonly cwd: string | null
  readonly elapsed: string | null
  readonly issueRef: string | null
  readonly lastEvent: {
    readonly ageSeconds: number | null
    readonly name: string | null
    readonly timestamp: string | null
  }
  readonly leaseRef: string | null
  readonly pullRequestRef: string | null
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

export type CodingAssignmentDetail = {
  readonly assignmentRef: string
  readonly blockerRefs: readonly string[]
  readonly closeoutRef: string | null
  readonly closeoutStatus: string | null
  readonly elapsedMs: number | null
  readonly issueRef: string | null
  readonly lastEvent: string | null
  readonly lastEventAt: string | null
  readonly leaseRef: string | null
  readonly pullRequestRef: string | null
  readonly workspacePath: string | null
}

export type ParsedCodexSessionRollout = {
  readonly assignmentRef: string | null
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

export type CodingManagerAssignmentMarker = {
  readonly accountRef: string | null
  readonly accountRefHash: string | null
  readonly assignmentRef: string | null
  readonly issueRef: string | null
  readonly markerPath: string
  readonly updatedAt: string | null
}

export type CodingManagerPrState = "closed" | "merged" | "open" | "unknown"

export type CodingManagerQueueMarker = {
  readonly ageSeconds: number | null
  readonly issueRef: string | null
  readonly markerPath: string
  readonly prState: CodingManagerPrState
  readonly type: "done" | "lock"
}

export type CodingManagerCandidate = {
  readonly issueRef: string
  readonly prState: CodingManagerPrState
  readonly priority: number
  readonly title: string
}

export type CodingManagerLatestBlocker = {
  readonly blocker: string
  readonly count: number
  readonly latestLogPath: string | null
}

export type CodingManagerLogSummary = {
  readonly acceptedRunningOrUnknown: number
  readonly completedAccepted: number
  readonly completedRejected: number
  readonly empty: number
  readonly failedBeforeAccept: number
  readonly latestBlockers: readonly CodingManagerLatestBlocker[]
  readonly pendingOutput: number
  readonly scannedLogCount: number
}

export type CodingManagerTokenFailure = {
  readonly assignmentRef: string | null
  readonly error: string | null
  readonly observedAt: string | null
}

export type CodingManagerTokenFailures = {
  readonly byteLength: number
  readonly failureCount: number
  readonly latestFailures: readonly CodingManagerTokenFailure[]
  readonly spoolPath: string
}

export type CodingManagerGithubCounts = {
  readonly closedPrsSince: number | null
  readonly mergedPrsSince: number | null
  readonly openPrs: number | null
  readonly since: string
}

export type CodingManagerMismatchWarning = {
  readonly code: string
  readonly message: string
  readonly severity: "info" | "warning"
}

export type CodingManagerResumeSnapshot = {
  readonly activeAssignments: readonly CodingManagerAssignmentMarker[]
  readonly candidateLane: readonly CodingManagerCandidate[]
  readonly github: CodingManagerGithubCounts
  readonly latestBlockers: readonly CodingManagerLatestBlocker[]
  readonly liveProcesses: readonly CodingProcess[]
  readonly logs: CodingManagerLogSummary
  readonly observedAt: string
  readonly queueLocks: readonly CodingManagerQueueMarker[]
  readonly statusBlock: string
  readonly tokenFailures: CodingManagerTokenFailures
  readonly warnings: readonly CodingManagerMismatchWarning[]
}

export type CodingStatusResult =
  | {
      readonly ok: true
      readonly events: readonly CodingSupervisorEvent[]
      readonly managerResume: CodingManagerResumeSnapshot
      readonly observedAt: string
      readonly processes: readonly CodingProcess[]
      readonly sessions: readonly CodingCodexSession[]
      readonly summary: CodingStatusSummary
    }
  | {
      readonly ok: false
      readonly error: string
      readonly events: readonly CodingSupervisorEvent[]
      readonly managerResume: CodingManagerResumeSnapshot
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

export const OPENAGENTS_DESKTOP_MANAGER_GITHUB_SINCE =
  "2026-06-29T13:45:00Z"

export const OPENAGENTS_DESKTOP_MANAGER_CANDIDATE_LANE: readonly Omit<
  CodingManagerCandidate,
  "prState"
>[] = [
  { issueRef: "7557", priority: 1, title: "Expose Codex fleet quota cooldown state" },
  { issueRef: "7579", priority: 2, title: "Expose Codex quota reset policy status" },
  { issueRef: "7560", priority: 3, title: "fix(pylon): update account reset status" },
  { issueRef: "7523", priority: 4, title: "fix(pylon): parse provider quota reset hints" },
  { issueRef: "7558", priority: 5, title: "fix(pylon): classify codex account execution refusals" },
  { issueRef: "7246", priority: 6, title: "fix(pylon): surface Codex execution refusal reasons" },
  { issueRef: "7221", priority: 7, title: "fix(pylon): surface codex execution refusal reasons" },
  { issueRef: "7510", priority: 8, title: "fix(khala-desktop): add Codex account readiness controls" },
  { issueRef: "7279", priority: 9, title: "fix(codex-supervisor): GC orphan claims and fast-retry gate refusals" },
  { issueRef: "7104", priority: 10, title: "fix(codex-supervisor): GC stale claims and fast-retry gate flakes" },
  { issueRef: "7073", priority: 11, title: "feat(operator): surface live Pylon runtime progress" },
  { issueRef: "7283", priority: 12, title: "fix(operator): register fleet state observability route" },
  { issueRef: "7230", priority: 13, title: "feat(operator): surface fleet assignment progress" },
  { issueRef: "7336", priority: 14, title: "feat(operator): surface fleet assignment progress" },
  { issueRef: "7589", priority: 15, title: "Align Pylon coordinator with runner registry" },
  { issueRef: "7571", priority: 16, title: "Harden Pylon agent runner resolution" },
  { issueRef: "7486", priority: 17, title: "Harden Pylon agent runner registry contract" },
]

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
  command.match(/(?:--account-ref\s+)(codex(?:-\d+)?)/)?.[1] ??
  command.match(/\/accounts\/codex\/(codex(?:-\d+)?)(?:\/|$)/)?.[1] ??
  command.match(/\/accounts\/(codex-\d+)(?:\/|$)/)?.[1] ??
  command.match(/(?:^|[\s=])(codex-\d+)(?:[\s/]|$)/)?.[1] ??
  command.match(/(?:^|[\s=])(codex)(?:\s|$)/)?.[1] ??
  null

const issueRefFromCommand = (command: string): string | null =>
  command.match(/issue\s+#?(\d+)/i)?.[1] ??
  command.match(/#(\d{3,})/)?.[1] ??
  null

const assignmentRefPattern = /assignment[.:-][A-Za-z0-9_.:-]{3,180}/

const assignmentRefFromCommand = (command: string): string | null =>
  command.match(/(?:--assignment-ref\s+)((?:"[^"]+")|(?:'[^']+')|\S+)/)?.[1]
    ?.replace(/^(['"])(.*)\1$/, "$2")
    ?.match(assignmentRefPattern)?.[0] ??
  command.match(assignmentRefPattern)?.[0] ??
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
  assignmentRef: string | null,
  issueRef: string | null,
): string => {
  const base = processKindLabel[kind]
  const detail = [
    accountRef,
    issueRef === null ? null : `#${issueRef}`,
    assignmentRef === null ? null : assignmentRef,
  ]
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
      label: labelForProcess(kind, accountRef, assignmentRef, issueRef),
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
      label: labelForProcess(row.kind, accountRef, assignmentRef, issueRef),
    }
  })
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

const EMPTY_PID_SET: ReadonlySet<number> = new Set<number>()

/**
 * A Codex session counts as LIVE when a real running `codex exec` worker
 * process is linked to it. Liveness is a property of the running process tree
 * only; it must never depend on whether the downstream token-usage proof has
 * been posted yet. Token proof is reported separately as a sub-status so that a
 * genuinely-running worker stays visible while its proof is still pending.
 */
export const isLiveCodexSession = (session: CodingCodexSession): boolean =>
  session.active || session.status === "active" || session.pid !== null

/**
 * The live "Squadron" set: every session backed by a running codex exec
 * process, regardless of token-proof state.
 */
export const liveCodexSessions = (
  sessions: readonly CodingCodexSession[],
): readonly CodingCodexSession[] => sessions.filter(isLiveCodexSession)

export const liveCodexSessionCount = (
  sessions: readonly CodingCodexSession[],
): number => liveCodexSessions(sessions).length

/**
 * Link a parsed Codex session to a running `codex exec` process.
 *
 * The precise signal is an exact workspace-path (cwd) match for the same
 * account. Real codex exec children are frequently spawned with the working
 * directory set on the process itself rather than via a `--cd`/`--cwd` flag, so
 * their command line carries no workspace path and a strict cwd match can never
 * succeed even though the worker is genuinely running. To keep that live work
 * visible, fall back to an account-scoped match when neither side exposes a
 * comparable workspace path. `claimedPids` keeps the linkage at most 1:1 so a
 * single running process is never counted as several live sessions.
 */
export const codexProcessForSession = (
  session: {
    readonly accountRef: string | null
    readonly cwd: string | null
  },
  processes: readonly CodingProcess[],
  options: {
    readonly allowAccountFallback?: boolean
    readonly claimedPids?: ReadonlySet<number>
  } = {},
): CodingProcess | null => {
  const claimedPids = options.claimedPids ?? EMPTY_PID_SET
  const accountMatches = (process: CodingProcess): boolean =>
    session.accountRef === null ||
    process.accountRef === null ||
    session.accountRef === process.accountRef
  const available = processes.filter(
    process => process.kind === "codex_exec" && !claimedPids.has(process.pid),
  )

  const cwdMatch = available.find(
    process =>
      accountMatches(process) &&
      session.cwd !== null &&
      process.workspacePath !== null &&
      session.cwd === process.workspacePath,
  )
  if (cwdMatch !== undefined) return cwdMatch

  if (options.allowAccountFallback !== true) return null

  const accountMatch = available.find(
    process =>
      accountMatches(process) &&
      (process.workspacePath === null || session.cwd === null),
  )
  return accountMatch ?? null
}

type CodingManagerLogInput = {
  readonly path: string
  readonly text: string
}

const blockerPattern = /\bblocker\.[a-z0-9_.-]+/gi

export const summarizeManagerAssignmentLogs = (
  logs: readonly CodingManagerLogInput[],
): CodingManagerLogSummary => {
  let acceptedRunningOrUnknown = 0
  let completedAccepted = 0
  let completedRejected = 0
  let empty = 0
  let failedBeforeAccept = 0
  let pendingOutput = 0
  const blockers = new Map<string, CodingManagerLatestBlocker>()

  for (const log of logs) {
    const text = log.text
    const trimmed = text.trim()
    const matches = text.match(blockerPattern) ?? []
    for (const blocker of matches) {
      const key = blocker.toLowerCase()
      const existing = blockers.get(key)
      blockers.set(key, {
        blocker: key,
        count: (existing?.count ?? 0) + 1,
        latestLogPath: existing?.latestLogPath ?? log.path,
      })
    }

    if (trimmed === "") {
      empty += 1
    } else if (
      /"event"\s*:\s*"assignment_run\.completed"/.test(text) &&
      /"status"\s*:\s*"accepted"/.test(text)
    ) {
      completedAccepted += 1
    } else if (
      /"event"\s*:\s*"assignment_run\.completed"/.test(text) &&
      /"status"\s*:\s*"rejected"/.test(text)
    ) {
      completedRejected += 1
    } else if (/"event"\s*:\s*"assignment_run\.accepted"/.test(text)) {
      acceptedRunningOrUnknown += 1
    } else if (/"ok"\s*:\s*false/.test(text) || /"error"\s*:/.test(text)) {
      failedBeforeAccept += 1
    } else {
      pendingOutput += 1
    }
  }

  return {
    acceptedRunningOrUnknown,
    completedAccepted,
    completedRejected,
    empty,
    failedBeforeAccept,
    latestBlockers: [...blockers.values()]
      .sort((left, right) => right.count - left.count)
      .slice(0, 8),
    pendingOutput,
    scannedLogCount: logs.length,
  }
}

const processCount = (
  processes: readonly CodingProcess[],
  kind: CodingProcessKind,
): number => processes.filter(process => process.kind === kind).length

const stateForIssue = (
  issueRef: string | null,
  prStates: Readonly<Record<string, CodingManagerPrState>>,
): CodingManagerPrState =>
  issueRef === null ? "unknown" : prStates[issueRef] ?? "unknown"

export const candidateLaneWithStates = (
  prStates: Readonly<Record<string, CodingManagerPrState>>,
): readonly CodingManagerCandidate[] =>
  OPENAGENTS_DESKTOP_MANAGER_CANDIDATE_LANE.map(candidate => ({
    ...candidate,
    prState: stateForIssue(candidate.issueRef, prStates),
  }))

const compactManagerLine = (value: string | null): string =>
  value === null || value.trim() === "" ? "-" : value.trim()

export const formatManagerResumeStatusBlock = (
  snapshot: Omit<CodingManagerResumeSnapshot, "statusBlock">,
): string => {
  const assigned = snapshot.activeAssignments.length
  const executing = processCount(snapshot.liveProcesses, "codex_exec")
  const assignmentRunners = processCount(snapshot.liveProcesses, "assignment_runner")
  const khalaRequests = processCount(snapshot.liveProcesses, "khala_request")
  const locks = snapshot.queueLocks.filter(marker => marker.type === "lock")
  const done = snapshot.queueLocks.filter(marker => marker.type === "done")
  const nextCandidate =
    snapshot.candidateLane.find(candidate => candidate.prState === "open") ??
    snapshot.candidateLane[0] ??
    null
  const blockers = snapshot.latestBlockers
    .slice(0, 3)
    .map(blocker => `${blocker.blocker} x${blocker.count}`)
    .join(", ")
  const warnings = snapshot.warnings
    .slice(0, 5)
    .map(warning => `${warning.severity}:${warning.code}`)
    .join(", ")

  return [
    "OPENAGENTS MANAGER RESUME",
    `observed_at: ${snapshot.observedAt}`,
    `assigned_markers: ${assigned}`,
    `live_codex_exec: ${executing}`,
    `assignment_runners: ${assignmentRunners}`,
    `khala_requests: ${khalaRequests}`,
    `queue_locks: ${locks.length}`,
    `queue_done_markers: ${done.length}`,
    `token_failures: ${snapshot.tokenFailures.failureCount}`,
    `open_prs: ${snapshot.github.openPrs ?? "unknown"}`,
    `merged_prs_since_${snapshot.github.since}: ${
      snapshot.github.mergedPrsSince ?? "unknown"
    }`,
    `closed_prs_since_${snapshot.github.since}: ${
      snapshot.github.closedPrsSince ?? "unknown"
    }`,
    `recent_logs: accepted=${snapshot.logs.completedAccepted} rejected=${
      snapshot.logs.completedRejected
    } running_or_unknown=${snapshot.logs.acceptedRunningOrUnknown}`,
    `candidate_lane: ${compactManagerLine(
      nextCandidate === null
        ? null
        : `#${nextCandidate.issueRef} ${nextCandidate.prState} ${nextCandidate.title}`,
    )}`,
    `latest_blockers: ${compactManagerLine(blockers)}`,
    `warnings: ${compactManagerLine(warnings)}`,
  ].join("\n")
}

export const buildManagerResumeSnapshot = (input: {
  readonly activeAssignments: readonly CodingManagerAssignmentMarker[]
  readonly github: CodingManagerGithubCounts
  readonly liveProcesses: readonly CodingProcess[]
  readonly logs: CodingManagerLogSummary
  readonly observedAt: string
  readonly prStates: Readonly<Record<string, CodingManagerPrState>>
  readonly queueLocks: readonly Omit<CodingManagerQueueMarker, "prState">[]
  readonly tokenFailures: CodingManagerTokenFailures
}): CodingManagerResumeSnapshot => {
  const queueLocks = input.queueLocks.map(marker => ({
    ...marker,
    prState: stateForIssue(marker.issueRef, input.prStates),
  }))
  const candidateLane = candidateLaneWithStates(input.prStates)
  const warnings: CodingManagerMismatchWarning[] = []
  const codexExecCount = processCount(input.liveProcesses, "codex_exec")
  const assignmentRunnerCount = processCount(input.liveProcesses, "assignment_runner")
  const khalaRequestCount = processCount(input.liveProcesses, "khala_request")
  const activeMarkerCount = input.activeAssignments.length

  if (activeMarkerCount !== codexExecCount) {
    warnings.push({
      code: "markers_codex_process_mismatch",
      message: `Active assignment markers (${activeMarkerCount}) differ from live codex exec processes (${codexExecCount}).`,
      severity: "warning",
    })
  }
  const visibleLocalRuntimeCount =
    activeMarkerCount + codexExecCount + assignmentRunnerCount + khalaRequestCount
  if (input.logs.acceptedRunningOrUnknown > visibleLocalRuntimeCount) {
    warnings.push({
      code: "accepted_logs_without_local_runtime",
      message: `Recent logs contain ${input.logs.acceptedRunningOrUnknown} accepted-without-completed entries but only ${visibleLocalRuntimeCount} visible local runtimes (${activeMarkerCount} markers, ${codexExecCount} codex exec, ${assignmentRunnerCount} runners, ${khalaRequestCount} Khala request wrappers). This is history until a matching live process or marker appears.`,
      severity: visibleLocalRuntimeCount === 0 ? "info" : "warning",
    })
  }
  if (input.tokenFailures.failureCount > 0 || input.tokenFailures.byteLength > 0) {
    warnings.push({
      code: "token_usage_failure_spool_nonempty",
      message: `${input.tokenFailures.failureCount} token usage reports are waiting in the local failure spool.`,
      severity: "warning",
    })
  }
  if (
    input.github.openPrs === null ||
    input.github.mergedPrsSince === null ||
    input.github.closedPrsSince === null
  ) {
    warnings.push({
      code: "github_state_unavailable",
      message: "GitHub PR counts were unavailable, so queue and marker state may be stale.",
      severity: "warning",
    })
  }

  const staleLocks = queueLocks.filter(
    marker => marker.type === "lock" && marker.prState !== "open" && marker.prState !== "unknown",
  )
  if (staleLocks.length > 0) {
    warnings.push({
      code: "queue_lock_github_state_mismatch",
      message: `${staleLocks.length} queue locks point at merged or closed PRs.`,
      severity: "warning",
    })
  }

  const snapshotWithoutBlock = {
    activeAssignments: input.activeAssignments,
    candidateLane,
    github: input.github,
    latestBlockers: input.logs.latestBlockers,
    liveProcesses: input.liveProcesses,
    logs: input.logs,
    observedAt: input.observedAt,
    queueLocks,
    tokenFailures: input.tokenFailures,
    warnings,
  }

  return {
    ...snapshotWithoutBlock,
    statusBlock: formatManagerResumeStatusBlock(snapshotWithoutBlock),
  }
}

export const emptyManagerResumeSnapshot = (
  observedAt: string,
): CodingManagerResumeSnapshot =>
  buildManagerResumeSnapshot({
    activeAssignments: [],
    github: {
      closedPrsSince: null,
      mergedPrsSince: null,
      openPrs: null,
      since: OPENAGENTS_DESKTOP_MANAGER_GITHUB_SINCE,
    },
    liveProcesses: [],
    logs: summarizeManagerAssignmentLogs([]),
    observedAt,
    prStates: {},
    queueLocks: [],
    tokenFailures: {
      byteLength: 0,
      failureCount: 0,
      latestFailures: [],
      spoolPath: "",
    },
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
      /^(\S+)\s+slot=(\d+)\s+acc=(codex(?:-\d+)?)\s+issue=#(\d+)\s+([A-Z-]+)/,
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
  return trimmed.length > 1_200 ? `${trimmed.slice(0, 1_197)}...` : trimmed
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
  let assignmentRef: string | null = null
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
    if (assignmentRef === null) {
      const serialized = compactJsonFromUnknown(record)
      assignmentRef = serialized?.match(assignmentRefPattern)?.[0] ?? null
    }

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
    assignmentRef,
    cwd,
    messageCount: messages.length,
    messages: messages.slice(-24),
    sessionId,
    source,
    title,
  }
}

const numberValueFromUnknown = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const stringArrayFromUnknown = (value: unknown): readonly string[] =>
  Array.isArray(value)
    ? value.filter((item): item is string => typeof item === "string")
    : []

const valueMentionsAssignment = (value: unknown, assignmentRef: string): boolean => {
  if (typeof value === "string") return value === assignmentRef
  if (Array.isArray(value)) return value.some(item => valueMentionsAssignment(item, assignmentRef))
  if (!isJsonObject(value)) return false
  return Object.values(value).some(item => valueMentionsAssignment(item, assignmentRef))
}

const assignmentRefFromRecord = (record: JsonObject): string | null =>
  stringValueFromUnknown(record.assignmentRef) ??
  (isJsonObject(record.assignmentRun)
    ? assignmentRefFromRecord(record.assignmentRun)
    : null) ??
  (isJsonObject(record.closeout)
    ? stringValueFromUnknown(record.closeout.assignmentRef)
    : null)

export const parseCodingAssignmentDetails = (
  logText: string,
): readonly CodingAssignmentDetail[] => {
  const byAssignment = new Map<string, CodingAssignmentDetail>()
  const ensure = (assignmentRef: string): CodingAssignmentDetail => {
    const current = byAssignment.get(assignmentRef)
    if (current !== undefined) return current
    const next: CodingAssignmentDetail = {
      assignmentRef,
      blockerRefs: [],
      closeoutRef: null,
      closeoutStatus: null,
      elapsedMs: null,
      issueRef: null,
      lastEvent: null,
      lastEventAt: null,
      leaseRef: null,
      pullRequestRef: null,
      workspacePath: null,
    }
    byAssignment.set(assignmentRef, next)
    return next
  }
  const merge = (assignmentRef: string, patch: Partial<CodingAssignmentDetail>) => {
    const current = ensure(assignmentRef)
    byAssignment.set(assignmentRef, {
      ...current,
      ...Object.fromEntries(
        Object.entries(patch).filter(([, value]) =>
          Array.isArray(value) ? value.length > 0 : value !== null && value !== undefined,
        ),
      ),
    })
  }

  for (const line of logText.split("\n")) {
    const trimmed = line.trim()
    if (!trimmed.startsWith("{")) continue
    let parsed: unknown
    try {
      parsed = JSON.parse(trimmed)
    } catch {
      continue
    }
    if (!isJsonObject(parsed)) continue
    const assignmentRef = assignmentRefFromRecord(parsed)
    if (assignmentRef === null || !valueMentionsAssignment(parsed, assignmentRef)) continue

    const nestedRun = isJsonObject(parsed.assignmentRun) ? parsed.assignmentRun : null
    const nestedCloseout = nestedRun !== null && isJsonObject(nestedRun.closeout)
      ? nestedRun.closeout
      : isJsonObject(parsed.closeout)
        ? parsed.closeout
        : null
    const closeoutReceipt =
      nestedRun !== null && isJsonObject(nestedRun.closeoutReceipt)
        ? nestedRun.closeoutReceipt
        : null
    const issueNumber = numberValueFromUnknown(parsed.issueNumber)
    const pullRequestNumber = numberValueFromUnknown(parsed.pullRequestNumber)
    merge(assignmentRef, {
      blockerRefs:
        stringArrayFromUnknown(parsed.blockerRefs).length > 0
          ? stringArrayFromUnknown(parsed.blockerRefs)
          : nestedCloseout === null
            ? []
            : stringArrayFromUnknown(nestedCloseout.blockerRefs),
      closeoutRef:
        stringValueFromUnknown(parsed.closeoutRef) ??
        stringValueFromUnknown(closeoutReceipt?.closeoutRef),
      closeoutStatus:
        stringValueFromUnknown(parsed.status) ??
        (nestedCloseout === null ? null : stringValueFromUnknown(nestedCloseout.status)),
      elapsedMs: numberValueFromUnknown(parsed.elapsedMs),
      issueRef:
        stringValueFromUnknown(parsed.issueRef) ??
        (issueNumber === null ? null : String(issueNumber)),
      lastEvent: stringValueFromUnknown(parsed.event),
      lastEventAt: stringValueFromUnknown(parsed.observedAt),
      leaseRef: stringValueFromUnknown(parsed.leaseRef),
      pullRequestRef:
        stringValueFromUnknown(parsed.pullRequestRef) ??
        (pullRequestNumber === null ? null : String(pullRequestNumber)),
      workspacePath:
        stringValueFromUnknown(parsed.workspacePath) ??
        stringValueFromUnknown(parsed.localWorkspacePath),
    })
  }

  return [...byAssignment.values()]
}
