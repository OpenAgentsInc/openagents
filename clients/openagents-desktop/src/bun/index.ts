import { BrowserView, BrowserWindow } from "electrobun/bun"
import { existsSync } from "node:fs"
import { homedir } from "node:os"
import { join, resolve } from "node:path"

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

const pylonAppCandidates = (): readonly string[] => [
  ...(Bun.env.OPENAGENTS_PYLON_APP_PATH
    ? [Bun.env.OPENAGENTS_PYLON_APP_PATH]
    : []),
  resolve(process.cwd(), "../../apps/pylon"),
  resolve(process.cwd(), "apps/pylon"),
]

const resolvePylonAppPath = async (): Promise<string> => {
  for (const candidate of pylonAppCandidates()) {
    if (await Bun.file(resolve(candidate, "package.json")).exists()) {
      return candidate
    }
  }
  return pylonAppCandidates()[0] ?? resolve(process.cwd(), "../../apps/pylon")
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

const createPylon = async (): Promise<CreatePylonResult> => {
  const observedAt = new Date().toISOString()
  try {
    const pylonAppPath = await resolvePylonAppPath()
    const proc = Bun.spawn({
      cmd: ["bun", "run", "start"],
      cwd: pylonAppPath,
      env: {
        ...Bun.env,
        PYLON_OPENAGENTS_BASE_URL: baseUrl,
      },
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
