/**
 * MC-5: pure connectivity-resolution logic, free of any native/expo import
 * so it stays unit-testable under `bun test` (expo-device transitively
 * pulls in react-native's Flow-typed entry point, which bun's plain
 * TS/JS parser cannot load). The native-touching wrapper is
 * `./khala-code-connectivity.ts`.
 */

export const KHALA_CODE_TAILNET_HEALTH_PORT = 50099

/** Edit this list to match the Tailnet hosts you want to probe from a device. */
export const KHALA_CODE_TAILNET_CANDIDATE_HOSTS: ReadonlyArray<string> = [
  "imac-pro-bertha",
  "macbook-pro-m2",
]

export type KhalaCodeConnectivityStatus = Readonly<{
  reachable: boolean
  target: string | null
  hostname: string | null
  checkedAt: string
}>

export type KhalaCodeConnectionTargetKind = "simulator_loopback" | "tailnet"

export type KhalaCodeConnectionProfile = Readonly<{
  checkedAt: string
  healthTarget: string | null
  hostname: string | null
  reachable: boolean
  syncBaseUrl: string
  targetKind: KhalaCodeConnectionTargetKind
}>

export type FetchLike = (
  url: string,
  init: { signal: AbortSignal },
) => Promise<{ ok: boolean; json: () => Promise<unknown> }>

const PER_HOST_TIMEOUT_MS = 1500
const DEFAULT_OPENAGENTS_SYNC_BASE_URL = "https://openagents.com"

const probeHealthUrl = async (
  url: string,
  timeoutMs: number,
  fetchImpl: FetchLike,
): Promise<string | null> => {
  const controller = new AbortController()
  const timer = setTimeout(() => controller.abort(), timeoutMs)
  try {
    const response = await fetchImpl(url, { signal: controller.signal })
    if (!response.ok) return null
    const body = (await response.json()) as { hostname?: unknown }
    return typeof body.hostname === "string" ? body.hostname : ""
  } catch {
    return null
  } finally {
    clearTimeout(timer)
  }
}

export const candidateTargets = (
  isDevice: boolean,
  port: number = KHALA_CODE_TAILNET_HEALTH_PORT,
  tailnetHosts: ReadonlyArray<string> = KHALA_CODE_TAILNET_CANDIDATE_HOSTS,
): ReadonlyArray<string> =>
  isDevice
    ? tailnetHosts.map(host => `http://${host}:${port}/health`)
    // Simulator shares the host Mac's network stack, so localhost reaches
    // whatever is running the desktop app on this same machine.
    : [`http://127.0.0.1:${port}/health`]

export const normalizeSyncBaseUrl = (
  value: string | null | undefined,
): string => {
  const trimmed = value?.trim() ?? ""
  return (trimmed.length === 0 ? DEFAULT_OPENAGENTS_SYNC_BASE_URL : trimmed)
    .replace(/\/+$/, "")
}

export const connectionProfileFromStatus = (
  status: KhalaCodeConnectivityStatus,
  input: {
    readonly isDevice: boolean
    readonly syncBaseUrl?: string | null
  },
): KhalaCodeConnectionProfile => ({
  checkedAt: status.checkedAt,
  healthTarget: status.target,
  hostname: status.hostname,
  reachable: status.reachable,
  syncBaseUrl: normalizeSyncBaseUrl(input.syncBaseUrl),
  targetKind: input.isDevice ? "tailnet" : "simulator_loopback"
})

export const resolveKhalaCodeConnectivity = async (
  targets: ReadonlyArray<string>,
  fetchImpl: FetchLike,
  timeoutMs: number = PER_HOST_TIMEOUT_MS,
): Promise<KhalaCodeConnectivityStatus> => {
  const checkedAt = new Date().toISOString()
  for (const url of targets) {
    const hostname = await probeHealthUrl(url, timeoutMs, fetchImpl)
    if (hostname !== null) {
      return { checkedAt, hostname: hostname || null, reachable: true, target: url }
    }
  }
  return { checkedAt, hostname: null, reachable: false, target: null }
}

export const resolveKhalaCodeConnectionProfile = async (
  input: {
    readonly isDevice: boolean
    readonly fetchImpl: FetchLike
    readonly port?: number
    readonly syncBaseUrl?: string | null
    readonly tailnetHosts?: ReadonlyArray<string>
    readonly timeoutMs?: number
  },
): Promise<KhalaCodeConnectionProfile> => {
  const status = await resolveKhalaCodeConnectivity(
    candidateTargets(input.isDevice, input.port, input.tailnetHosts),
    input.fetchImpl,
    input.timeoutMs
  )
  return connectionProfileFromStatus(status, {
    isDevice: input.isDevice,
    syncBaseUrl: input.syncBaseUrl
  })
}
