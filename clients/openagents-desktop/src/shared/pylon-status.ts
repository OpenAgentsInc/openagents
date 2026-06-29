export const OPENAGENTS_DESKTOP_DEFAULT_BASE_URL = "https://openagents.com"
export const OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS = 5_000

export type PylonStatusResult =
  | {
      readonly ok: true
      readonly count: number
      readonly observedAt: string
    }
  | {
      readonly ok: false
      readonly count: 0
      readonly error: string
      readonly observedAt: string
    }

export type PylonStatusFetchOptions = Readonly<{
  baseUrl?: string
  fetch?: typeof fetch
  token?: string | null
}>

const asRecord = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : {}

const asArray = (value: unknown): ReadonlyArray<unknown> =>
  Array.isArray(value) ? value : []

const stringValue = (value: unknown): string =>
  typeof value === "string" ? value.trim().toLowerCase() : ""

const isConnectedPylon = (value: unknown): boolean => {
  const pylon = asRecord(value)
  return pylon.heartbeatFresh === true || stringValue(pylon.status) === "online"
}

export const pylonCountFromFleetStatus = (fleetStatus: unknown): number => {
  const fleet = asRecord(fleetStatus)
  const fleetBlock = asRecord(fleet.fleet)
  return asArray(fleetBlock.spread).filter(isConnectedPylon).length
}

export const fetchPylonStatus = async (
  options: PylonStatusFetchOptions,
): Promise<PylonStatusResult> => {
  const observedAt = new Date().toISOString()
  const token = options.token?.trim()
  if (!token) {
    return {
      ok: false,
      count: 0,
      error: "Set OPENAGENTS_AGENT_TOKEN to load pylon status.",
      observedAt,
    }
  }

  const baseUrl = (options.baseUrl ?? OPENAGENTS_DESKTOP_DEFAULT_BASE_URL)
    .replace(/\/+$/, "")
  const fetchImpl = options.fetch ?? fetch

  try {
    const response = await fetchImpl(`${baseUrl}/api/operator/fleet/status`, {
      headers: {
        accept: "application/json",
        authorization: `Bearer ${token}`,
      },
      method: "GET",
    })
    if (!response.ok) {
      throw new Error(`/api/operator/fleet/status returned ${response.status}`)
    }

    return {
      ok: true,
      count: pylonCountFromFleetStatus(await response.json()),
      observedAt,
    }
  } catch (error) {
    return {
      ok: false,
      count: 0,
      error: error instanceof Error ? error.message : String(error),
      observedAt,
    }
  }
}
