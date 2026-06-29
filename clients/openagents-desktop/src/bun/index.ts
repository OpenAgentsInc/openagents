import { BrowserView, BrowserWindow } from "electrobun/bun"
import { existsSync } from "node:fs"
import { readdir } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import {
  emptyCodingStatusSummary,
  parseCodingProcesses,
  parseSupervisorLog,
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
