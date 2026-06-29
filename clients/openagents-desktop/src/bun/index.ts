import { BrowserView, BrowserWindow } from "electrobun/bun"
import { existsSync } from "node:fs"
import { mkdir, readdir, rename, stat } from "node:fs/promises"
import type { Stats } from "node:fs"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import {
  type CodingCodexSession,
  emptyCodingStatusSummary,
  parseCodexSessionRollout,
  parseCodingProcesses,
  parseSupervisorLog,
  type CodingProcess,
  summarizeCodingProcesses,
  type CodingStatusResult,
  type CodingStatusSummary,
} from "../shared/coding-status.js"
import {
  connectedPylonCount,
  type CreatePylonResult,
  type DesktopPylon,
  fetchPylonStatus,
  OPENAGENTS_DESKTOP_DEFAULT_BASE_URL,
  type PylonStatusResult,
} from "../shared/pylon-status.js"
import {
  OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  type OpenAgentsDesktopRPCSchema,
} from "../shared/rpc.js"
import {
  emptyTokenAccountingSpool,
  isPublicSafeAssignmentRef,
  OPENAGENTS_DESKTOP_TOKEN_FAILURE_SPOOL_NAME,
  parseTokenFailureSpool,
  type AssignmentTokenUsageVerification,
  type TokenAccountingReplayResult,
  type TokenAccountingSpool,
  type TokenAccountingStatusResult,
  verificationFromProofPayload,
} from "../shared/token-accounting.js"

const baseUrl =
  Bun.env.PYLON_OPENAGENTS_BASE_URL ??
  Bun.env.OPENAGENTS_COM_BASE_URL ??
  OPENAGENTS_DESKTOP_DEFAULT_BASE_URL

const pathCandidates = (): readonly string[] => [
  Bun.env.PATH ?? "",
  join(homedir(), ".bun", "bin"),
  "/opt/homebrew/bin",
  "/usr/local/bin",
  "/usr/bin",
  "/bin",
]

const withExtraPath = (env: NodeJS.ProcessEnv): NodeJS.ProcessEnv => ({
  ...env,
  PATH: pathCandidates().filter(path => path !== "").join(":"),
})

const ancestorPylonCandidates = (anchor: string): readonly string[] => {
  const candidates: string[] = []
  let current = resolve(anchor)
  for (let index = 0; index < 12; index += 1) {
    candidates.push(resolve(current, "apps/pylon"))
    candidates.push(resolve(current, "../../apps/pylon"))
    const parent = dirname(current)
    if (parent === current) break
    current = parent
  }
  return candidates
}

const pylonAppCandidates = (): readonly string[] => [
  ...(Bun.env.OPENAGENTS_PYLON_APP_PATH
    ? [Bun.env.OPENAGENTS_PYLON_APP_PATH]
    : []),
  ...(Bun.env.OPENAGENTS_REPO_ROOT
    ? [resolve(Bun.env.OPENAGENTS_REPO_ROOT, "apps/pylon")]
    : []),
  ...(Bun.env.INIT_CWD ? ancestorPylonCandidates(Bun.env.INIT_CWD) : []),
  ...(Bun.env.PWD ? ancestorPylonCandidates(Bun.env.PWD) : []),
  ...ancestorPylonCandidates(process.cwd()),
  join(homedir(), "work", "openagents", "apps", "pylon"),
  resolve(process.cwd(), "../../apps/pylon"),
  resolve(process.cwd(), "apps/pylon"),
]

const resolvePylonAppPath = async (): Promise<string> => {
  const seen = new Set<string>()
  for (const candidate of pylonAppCandidates()) {
    if (seen.has(candidate)) continue
    seen.add(candidate)
    if (await Bun.file(resolve(candidate, "package.json")).exists()) {
      return candidate
    }
  }
  return pylonAppCandidates()[0] ?? resolve(process.cwd(), "../../apps/pylon")
}

const bunExecutableCandidates = (): readonly string[] => [
  ...(Bun.env.OPENAGENTS_BUN_PATH ? [Bun.env.OPENAGENTS_BUN_PATH] : []),
  process.execPath,
  resolve(process.cwd(), "bun"),
  join(homedir(), ".bun", "bin", "bun"),
  "/opt/homebrew/bin/bun",
  "/usr/local/bin/bun",
  "/usr/bin/bun",
]

const resolveBunExecutable = (): string => {
  for (const candidate of bunExecutableCandidates()) {
    if (candidate !== "" && existsSync(candidate)) return candidate
  }
  return "bun"
}

const pylonHomeCandidates = (): readonly string[] => {
  const home = homedir()
  return [
    ...(Bun.env.PYLON_HOME ? [Bun.env.PYLON_HOME] : []),
    join(home, ".openagents", "pylon"),
    join(home, ".pylon"),
  ]
}

const resolvePylonHome = (): string => {
  const candidates = pylonHomeCandidates()
  const withIdentity = candidates.find(candidate =>
    existsSync(resolve(candidate, "identity.json")),
  )
  if (withIdentity !== undefined) return withIdentity
  return candidates[0] ?? join(homedir(), ".openagents", "pylon")
}

const readJsonFile = async (
  path: string,
): Promise<Record<string, unknown> | null> => {
  try {
    if (!(await Bun.file(path).exists())) return null
    const value = await Bun.file(path).json()
    return typeof value === "object" && value !== null && !Array.isArray(value)
      ? value as Record<string, unknown>
      : null
  } catch {
    return null
  }
}

const readTextFile = async (path: string): Promise<string> => {
  try {
    if (!(await Bun.file(path).exists())) return ""
    return await Bun.file(path).text()
  } catch {
    return ""
  }
}

const fileStatOrNull = async (path: string): Promise<Stats | null> => {
  try {
    return await stat(path)
  } catch {
    return null
  }
}

const stringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const heartbeatFresh = (value: string | null): boolean => {
  if (value === null) return false
  const millis = Date.parse(value)
  return Number.isFinite(millis) && Date.now() - millis <= 90_000
}

const localHeartbeatLabel = (value: string | null): string | null => {
  if (value === null) return null
  const millis = Date.parse(value)
  if (!Number.isFinite(millis)) return null
  const ageSeconds = Math.max(0, Math.round((Date.now() - millis) / 1000))
  if (ageSeconds < 60) return "Just now"
  const ageMinutes = Math.round(ageSeconds / 60)
  if (ageMinutes < 60) {
    return `${ageMinutes} minute${ageMinutes === 1 ? "" : "s"} ago`
  }
  const ageHours = Math.round(ageMinutes / 60)
  if (ageHours < 48) {
    return `${ageHours} hour${ageHours === 1 ? "" : "s"} ago`
  }
  const ageDays = Math.round(ageHours / 24)
  return `${ageDays} day${ageDays === 1 ? "" : "s"} ago`
}

const readLocalPylons = async (): Promise<readonly DesktopPylon[]> => {
  const pylonHome = resolvePylonHome()
  const [identity, presence, runtime] = await Promise.all([
    readJsonFile(resolve(pylonHome, "identity.json")),
    readJsonFile(resolve(pylonHome, "presence-state.json")),
    readJsonFile(resolve(pylonHome, "runtime-state.json")),
  ])

  if (identity === null && presence === null && runtime === null) return []

  const latestHeartbeatAt = nullableString(presence?.lastHeartbeatAt)
  const lifecycle = stringValue(runtime?.lifecycle, "offline")
  const isFresh = heartbeatFresh(latestHeartbeatAt)
  const isRuntimeOnline =
    lifecycle === "online" || lifecycle === "assignment-ready"
  const pylonRef =
    stringValue(presence?.pylonRef) ||
    stringValue(identity?.pylonRef, "local-pylon")
  const status = isFresh
    ? "online"
    : latestHeartbeatAt !== null
      ? "stale"
      : isRuntimeOnline
        ? "local"
        : lifecycle

  return [
    {
      busySlots: 0,
      heartbeatFresh: isFresh,
      latestHeartbeatAt,
      latestHeartbeatLabel: localHeartbeatLabel(latestHeartbeatAt),
      ownerAgentRef: null,
      pylonRef,
      queuedSlots: 0,
      readySlots: 0,
      status,
    },
  ]
}

const localPylonStatus = async (
  notice?: string,
): Promise<PylonStatusResult> => {
  const pylons = await readLocalPylons()
  return {
    ok: true,
    count: connectedPylonCount(pylons),
    ...(notice === undefined ? {} : { notice }),
    observedAt: new Date().toISOString(),
    pylons,
  }
}

const desktopPylonStatus = async (): Promise<PylonStatusResult> => {
  const result = await fetchPylonStatus({
    baseUrl,
    token: Bun.env.OPENAGENTS_AGENT_TOKEN ?? null,
  })

  return result.ok ? result : localPylonStatus(result.error)
}

const supervisorHome = (): string =>
  Bun.env.CODEX_SUPERVISOR_HOME ??
  join(homedir(), ".codex-supervisor")

const countDirectoryEntries = async (path: string): Promise<number> => {
  try {
    return (await readdir(path)).length
  } catch {
    return 0
  }
}

const countNonEmptyLines = async (path: string): Promise<number | null> => {
  const text = await readTextFile(path)
  if (text === "") return null
  return text.split("\n").filter(line => line.trim() !== "").length
}

const codexSessionDateSegments = (): readonly string[] => {
  const segments = new Set<string>()
  const now = new Date()
  for (let offset = 0; offset < 3; offset += 1) {
    const date = new Date(now)
    date.setDate(now.getDate() - offset)
    const year = String(date.getFullYear()).padStart(4, "0")
    const month = String(date.getMonth() + 1).padStart(2, "0")
    const day = String(date.getDate()).padStart(2, "0")
    segments.add(join(year, month, day))
  }
  return [...segments]
}

const pylonCodexAccountRootCandidates = (): readonly string[] => [
  resolve(resolvePylonHome(), "accounts", "codex"),
  join(homedir(), ".pylon-fable", "accounts", "codex"),
  join(homedir(), ".openagents", "pylon", "accounts", "codex"),
  join(homedir(), ".pylon", "accounts", "codex"),
]

const codexHomeCandidates = async (): Promise<readonly string[]> => {
  const homes = new Set<string>()
  if (Bun.env.CODEX_HOME) homes.add(Bun.env.CODEX_HOME)
  homes.add(join(homedir(), ".codex"))

  for (const root of pylonCodexAccountRootCandidates()) {
    try {
      for (const entry of await readdir(root, { withFileTypes: true })) {
        if (entry.isDirectory() && /^codex-\d+$/.test(entry.name)) {
          homes.add(resolve(root, entry.name))
        }
      }
    } catch {
      // Optional account homes are best-effort.
    }
  }

  return [...homes]
}

const accountRefFromCodexHome = (home: string): string | null =>
  home.match(/\/accounts\/codex\/(codex-\d+)(?:\/|$)/)?.[1] ?? null

type CodexSessionFile = {
  readonly accountRef: string | null
  readonly modifiedAtMs: number
  readonly path: string
}

let sessionFileScanCache:
  | {
      readonly files: readonly CodexSessionFile[]
      readonly scannedAtMs: number
    }
  | null = null

const collectCodexSessionFiles = async (
  root: string,
  accountRef: string | null,
): Promise<readonly CodexSessionFile[]> => {
  const files: CodexSessionFile[] = []

  const visit = async (directory: string): Promise<void> => {
    let entries
    try {
      entries = await readdir(directory, { withFileTypes: true })
    } catch {
      return
    }

    await Promise.all(
      entries.map(async entry => {
        const path = resolve(directory, entry.name)
        if (entry.isDirectory()) {
          await visit(path)
          return
        }
        if (!entry.isFile() || !entry.name.endsWith(".jsonl")) return
        try {
          const fileStat = await stat(path)
          files.push({
            accountRef,
            modifiedAtMs: fileStat.mtimeMs,
            path,
          })
        } catch {
          // Ignore files that disappear while scanning.
        }
      }),
    )
  }

  await visit(root)
  return files
}

const recentCodexSessionFiles = async (): Promise<readonly CodexSessionFile[]> => {
  if (
    sessionFileScanCache !== null &&
    Date.now() - sessionFileScanCache.scannedAtMs <= 15_000
  ) {
    return sessionFileScanCache.files
  }

  const homes = await codexHomeCandidates()
  const segments = codexSessionDateSegments()
  const files = (
    await Promise.all(
      homes.flatMap(home =>
        segments.map(segment =>
          collectCodexSessionFiles(
            resolve(home, "sessions", segment),
            accountRefFromCodexHome(home),
          ),
        ),
      ),
    )
  ).flat()

  const recentFiles = files
    .sort((left, right) => right.modifiedAtMs - left.modifiedAtMs)
    .slice(0, 32)
  sessionFileScanCache = {
    files: recentFiles,
    scannedAtMs: Date.now(),
  }
  return recentFiles
}

const sessionIdFromRolloutPath = (path: string): string =>
  path.match(/rollout-.+-([0-9a-f]{8}-[0-9a-f-]{27})\.jsonl$/i)?.[1] ??
  path.split("/").at(-1)?.replace(/\.jsonl$/, "") ??
  path

const matchingCodexProcess = (
  session: {
    readonly accountRef: string | null
    readonly cwd: string | null
  },
  processes: readonly CodingProcess[],
): CodingProcess | null =>
  processes.find(process => {
    if (process.kind !== "codex_exec") return false
    if (
      session.accountRef !== null &&
      process.accountRef !== null &&
      session.accountRef !== process.accountRef
    ) {
      return false
    }
    return (
      session.cwd !== null &&
      process.workspacePath !== null &&
      session.cwd === process.workspacePath
    )
  }) ?? null

const fallbackSessionTitle = (sessionId: string): string =>
  sessionId.startsWith("rollout-") ? sessionId : `Codex ${sessionId.slice(0, 8)}`

const readCodexSessions = async (
  processes: readonly CodingProcess[],
): Promise<readonly CodingCodexSession[]> => {
  const files = await recentCodexSessionFiles()
  const sessions = await Promise.all(
    files.map(async file => {
      const text = await readTextFile(file.path)
      const parsed = parseCodexSessionRollout(text)
      const sessionId = parsed.sessionId ?? sessionIdFromRolloutPath(file.path)
      const accountRef = file.accountRef
      const process = matchingCodexProcess(
        {
          accountRef,
          cwd: parsed.cwd,
        },
        processes,
      )
      const isRecent = Date.now() - file.modifiedAtMs <= 10 * 60 * 1_000
      const active = process !== null
      const modifiedAt = new Date(file.modifiedAtMs).toISOString()

      return {
        accountRef,
        active,
        assignmentRef: parsed.assignmentRef ?? process?.assignmentRef ?? null,
        cwd: parsed.cwd,
        issueRef: process?.issueRef ?? null,
        messageCount: parsed.messageCount,
        messages: parsed.messages,
        modifiedAt,
        path: file.path,
        pid: process?.pid ?? null,
        sessionId,
        source: parsed.source,
        status: active ? "active" : isRecent ? "recent" : "idle",
        title: parsed.title ?? process?.label ?? fallbackSessionTitle(sessionId),
      } satisfies CodingCodexSession
    }),
  )

  return sessions
    .sort((left, right) => {
      if (left.active !== right.active) return left.active ? -1 : 1
      if (left.status !== right.status) {
        if (left.status === "active") return -1
        if (right.status === "active") return 1
        if (left.status === "recent") return -1
        if (right.status === "recent") return 1
      }
      return Date.parse(right.modifiedAt) - Date.parse(left.modifiedAt)
    })
    .slice(0, 24)
}

const spawnText = async (cmd: readonly string[]): Promise<string> => {
  const proc = Bun.spawn({
    cmd: [...cmd],
    env: withExtraPath(Bun.env),
    stderr: "ignore",
    stdout: "pipe",
  })
  const stdout = await new Response(proc.stdout).text()
  await proc.exited
  return stdout
}

const tokenFailureSpoolPath = (): string =>
  resolve(join(homedir(), ".pylon-fable"), OPENAGENTS_DESKTOP_TOKEN_FAILURE_SPOOL_NAME)

const readTokenFailureSpool = async (): Promise<TokenAccountingSpool> => {
  const path = tokenFailureSpoolPath()
  const fileStat = await fileStatOrNull(path)
  if (fileStat === null || fileStat.size <= 0) {
    return emptyTokenAccountingSpool(path)
  }

  return parseTokenFailureSpool({
    byteLength: fileStat.size,
    path,
    text: await readTextFile(path),
  })
}

const tokenAccountingStatus = async (): Promise<TokenAccountingStatusResult> => ({
  ok: true,
  observedAt: new Date().toISOString(),
  spool: await readTokenFailureSpool(),
})

const tokenCandidates = (): readonly string[] => [
  ...(Bun.env.OPENAGENTS_AGENT_TOKEN ? [Bun.env.OPENAGENTS_AGENT_TOKEN] : []),
]

const readStoredAgentToken = async (): Promise<string | null> => {
  for (const candidate of [
    resolve(join(homedir(), ".pylon-fable"), "auth", "openagents-agent-token"),
    resolve(resolvePylonHome(), "auth", "openagents-agent-token"),
  ]) {
    const token = (await readTextFile(candidate)).trim()
    if (token !== "") return token
  }
  return null
}

const resolveAgentToken = async (): Promise<string | null> => {
  const envToken = tokenCandidates().find(value => value.trim() !== "")
  return envToken?.trim() ?? await readStoredAgentToken()
}

const compactError = (value: unknown): string => {
  const text = (value instanceof Error ? value.message : String(value)).replace(
    /oa_agent_[A-Za-z0-9._-]+/g,
    "oa_agent_[redacted]",
  )
  return text.length > 180 ? `${text.slice(0, 177)}...` : text
}

const rawFailureReports = async (): Promise<readonly Record<string, unknown>[]> => {
  const path = tokenFailureSpoolPath()
  const text = await readTextFile(path)
  const reports: Record<string, unknown>[] = []
  for (const line of text.split("\n")) {
    if (line.trim() === "") continue
    try {
      const parsed = JSON.parse(line) as unknown
      if (
        typeof parsed === "object" &&
        parsed !== null &&
        !Array.isArray(parsed) &&
        typeof (parsed as Record<string, unknown>).report === "object" &&
        (parsed as Record<string, unknown>).report !== null
      ) {
        reports.push((parsed as { report: Record<string, unknown> }).report)
      }
    } catch {
      // Invalid JSONL rows remain in the spool for manual inspection.
    }
  }
  return reports
}

const nonNegativeInteger = (value: unknown): number =>
  typeof value === "number" && Number.isFinite(value)
    ? Math.max(0, Math.trunc(value))
    : 0

const replayReport = async (
  report: Record<string, unknown>,
  agentToken: string,
): Promise<void> => {
  const assignmentRef = String(report.assignmentRef ?? "")
  const pylonRef = String(report.pylonRef ?? "")
  const leaseRef = String(report.leaseRef ?? "")
  const turnIndex = nonNegativeInteger(report.turnIndex) + 1
  if (
    !isPublicSafeAssignmentRef(assignmentRef) ||
    !isPublicSafeAssignmentRef(pylonRef) ||
    !isPublicSafeAssignmentRef(leaseRef)
  ) {
    throw new Error("spooled Codex turn report contains a non-public-safe ref")
  }
  const usage =
    typeof report.usage === "object" && report.usage !== null && !Array.isArray(report.usage)
      ? report.usage as Record<string, unknown>
      : {}
  const sessionRef =
    typeof report.sessionRef === "string" && report.sessionRef.trim() !== ""
      ? report.sessionRef.trim()
      : "session.pending"
  const body = {
    assignmentRef,
    leaseRef,
    pylonRef,
    runnerKind: "codex_sdk",
    schemaVersion: "openagents.pylon.codex_turn.v1",
    turnIndex,
    ...(typeof report.runRef === "string" ? { runRef: report.runRef } : {}),
    ...(typeof report.sessionRef === "string" ? { sessionRef: report.sessionRef } : {}),
    ...(typeof report.workspaceRef === "string" ? { workspaceRef: report.workspaceRef } : {}),
    ...(typeof report.observedAt === "string" ? { observedAt: report.observedAt } : {}),
    items: Array.isArray(report.items) ? report.items : [],
    ...(Array.isArray(report.rawEvents) ? { rawEvents: report.rawEvents } : {}),
    usage: {
      inputTokens: nonNegativeInteger(usage.inputTokens),
      ...(usage.cachedInputTokens === undefined
        ? {}
        : { cachedInputTokens: nonNegativeInteger(usage.cachedInputTokens) }),
      outputTokens: nonNegativeInteger(usage.outputTokens),
      ...(usage.reasoningOutputTokens === undefined
        ? {}
        : { reasoningOutputTokens: nonNegativeInteger(usage.reasoningOutputTokens) }),
    },
  }
  const response = await fetch(new URL("/api/pylon/codex/turns", baseUrl), {
    method: "POST",
    headers: {
      authorization: `Bearer ${agentToken}`,
      "content-type": "application/json",
      "Idempotency-Key": [
        "pylon.codex.turn",
        pylonRef,
        assignmentRef,
        sessionRef,
        String(turnIndex),
      ].join("."),
    },
    body: JSON.stringify(body),
  })
  if (!response.ok) {
    throw new Error(`Pylon Codex turn replay failed (${response.status})`)
  }
}

const archiveTimestamp = (): string =>
  new Date().toISOString().replace(/[-:]/g, "").replace(/\.\d{3}Z$/, "Z")

const replayTokenFailures = async (): Promise<TokenAccountingReplayResult> => {
  const observedAt = new Date().toISOString()
  const before = await readTokenFailureSpool()
  if (!before.exists || before.lineCount === 0) {
    return {
      ok: true,
      archivedPath: null,
      observedAt,
      replayedReports: 0,
      spool: before,
    }
  }

  const token = await resolveAgentToken()
  if (token === null) {
    return {
      ok: false,
      error: "No OpenAgents agent token is available for replay.",
      observedAt,
      replayedReports: 0,
      spool: before,
    }
  }

  const reports = await rawFailureReports()
  let replayedReports = 0
  try {
    if (reports.length !== before.lineCount) {
      throw new Error("failure spool contains rows that cannot be replayed safely")
    }
    for (const report of reports) {
      await replayReport(report, token)
      replayedReports += 1
    }
    const archiveDir = resolve(join(homedir(), ".pylon-fable"), "replayed-failures")
    await mkdir(archiveDir, { recursive: true })
    const archivedPath = resolve(
      archiveDir,
      `codex-turn-report-failures-${archiveTimestamp()}.jsonl`,
    )
    await rename(before.path, archivedPath)
    return {
      ok: true,
      archivedPath,
      observedAt: new Date().toISOString(),
      replayedReports,
      spool: await readTokenFailureSpool(),
    }
  } catch (error) {
    return {
      ok: false,
      error: compactError(error),
      observedAt: new Date().toISOString(),
      replayedReports,
      spool: await readTokenFailureSpool(),
    }
  }
}

const spawnJson = async (cmd: readonly string[]): Promise<unknown> => {
  const agentToken = await resolveAgentToken()
  const proc = Bun.spawn({
    cmd: [...cmd],
    env: withExtraPath({
      ...Bun.env,
      PYLON_OPENAGENTS_BASE_URL: baseUrl,
      ...(agentToken === null ? {} : { OPENAGENTS_AGENT_TOKEN: agentToken }),
    }),
    stderr: "pipe",
    stdout: "pipe",
  })
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ])
  if (exitCode !== 0) {
    throw new Error(stderr.trim() || stdout.trim() || `command exited ${exitCode}`)
  }
  return JSON.parse(stdout)
}

const verifyAssignmentTokenUsage = async (
  assignmentRef: string,
): Promise<AssignmentTokenUsageVerification> => {
  const observedAt = new Date().toISOString()
  if (!isPublicSafeAssignmentRef(assignmentRef)) {
    return {
      ok: false,
      assignmentRef,
      blockerRef: "blocker.desktop_token_accounting.assignment_ref_missing",
      observedAt,
    }
  }

  try {
    const pylonAppPath = await resolvePylonAppPath()
    const bunExecutable = resolveBunExecutable()
    const payload = await spawnJson([
      bunExecutable,
      resolve(pylonAppPath, "src", "index.ts"),
      "khala",
      "proof",
      assignmentRef,
      "--json",
    ])
    return verificationFromProofPayload(assignmentRef, payload, new Date().toISOString())
  } catch (error) {
    return {
      ok: false,
      assignmentRef,
      blockerRef: "blocker.desktop_token_accounting.proof_unavailable",
      error: compactError(error),
      observedAt,
    }
  }
}

const codingStatus = async (): Promise<CodingStatusResult> => {
  const observedAt = new Date().toISOString()
  const home = supervisorHome()
  const [psOutput, supervisorLog, claimCount, openIssueCount] =
    await Promise.all([
      spawnText([
        "/bin/ps",
        "axww",
        "-o",
        "pid=",
        "-o",
        "ppid=",
        "-o",
        "pcpu=",
        "-o",
        "etime=",
        "-o",
        "command=",
      ]),
      readTextFile(resolve(home, "supervisor.log")),
      countDirectoryEntries(resolve(home, "claims")),
      countNonEmptyLines(resolve(home, "open-issues.set")),
    ])

  const processes = parseCodingProcesses(psOutput)
  const sessions = await readCodexSessions(processes)
  const processSummary = summarizeCodingProcesses(processes)
  const logSummary = parseSupervisorLog(supervisorLog)
  const summary: CodingStatusSummary = {
    ...emptyCodingStatusSummary(),
    ...processSummary,
    claimCount,
    desiredSlots: logSummary.desiredSlots,
    lastDispatchAt: logSummary.lastDispatchAt,
    lockoutRecent: logSummary.lockoutRecent,
    noDispatchRecent: logSummary.noDispatchRecent,
    okRecent: logSummary.okRecent,
    openIssueCount,
    readyCodex: logSummary.readyCodex,
  }

  return {
    ok: true,
    events: logSummary.events,
    observedAt,
    processes,
    sessions,
    summary,
  }
}

const createPylon = async (): Promise<CreatePylonResult> => {
  const observedAt = new Date().toISOString()
  try {
    const pylonAppPath = await resolvePylonAppPath()
    const bunExecutable = resolveBunExecutable()
    const proc = Bun.spawn({
      cmd: [bunExecutable, "run", "start"],
      cwd: pylonAppPath,
      env: withExtraPath({
        ...Bun.env,
        PYLON_OPENAGENTS_BASE_URL: baseUrl,
      }),
      stderr: "ignore",
      stdin: "ignore",
      stdout: "ignore",
    })

    const unref = (proc as { unref?: () => void }).unref
    unref?.call(proc)

    return {
      ok: true,
      observedAt,
      pid: typeof proc.pid === "number" ? proc.pid : null,
    }
  } catch (error) {
    return {
      ok: false,
      error: error instanceof Error ? error.message : String(error),
      observedAt,
    }
  }
}

const rpc = BrowserView.defineRPC<OpenAgentsDesktopRPCSchema>({
  maxRequestTime: OPENAGENTS_DESKTOP_RPC_MAX_REQUEST_TIME_MS,
  handlers: {
    requests: {
      codingStatus,
      createPylon,
      async pylonStatus() {
        return desktopPylonStatus()
      },
      replayTokenFailures,
      tokenAccountingStatus,
      verifyAssignmentTokenUsage,
    },
    messages: {},
  },
})

new BrowserWindow({
  title: "OpenAgents",
  url: "views://openagents-desktop/index.html",
  frame: { x: 128, y: 96, width: 1024, height: 720 },
  rpc,
})
