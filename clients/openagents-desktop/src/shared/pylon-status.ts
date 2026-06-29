export const OPENAGENTS_DESKTOP_DEFAULT_BASE_URL = "https://openagents.com"
export const OPENAGENTS_DESKTOP_PYLON_POLL_INTERVAL_MS = 5_000

export type DesktopPylon = {
  readonly busySlots: number
  readonly heartbeatFresh: boolean
  readonly latestHeartbeatAt: string | null
  readonly latestHeartbeatLabel: string | null
  readonly ownerAgentRef: string | null
  readonly pylonRef: string
  readonly queuedSlots: number
  readonly readySlots: number
  readonly status: string
}

export type PylonStatusResult =
  | {
      readonly ok: true
      readonly count: number
      readonly notice?: string
      readonly pylons: readonly DesktopPylon[]
      readonly observedAt: string
    }
  | {
      readonly ok: false
      readonly count: 0
      readonly pylons: readonly DesktopPylon[]
      readonly error: string
      readonly observedAt: string
    }

export type CreatePylonResult =
  | {
      readonly ok: true
      readonly observedAt: string
      readonly pid: number | null
    }
  | {
      readonly ok: false
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

const stringValue = (value: unknown, fallback = ""): string =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : fallback

const nullableString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value.trim() : null

const numberValue = (value: unknown, fallback = 0): number =>
  typeof value === "number" && Number.isFinite(value) ? value : fallback

const sumCapacityField = (
  capacity: readonly Record<string, unknown>[],
  field: "available" | "busy" | "queued" | "ready",
): number =>
  capacity.reduce((total, item) => total + numberValue(item[field]), 0)

const agentRefFromMe = (value: unknown): string | null => {
  const body = asRecord(value)
  const agent = asRecord(body.agent)
  const user = asRecord(agent.user)
  const id = stringValue(user.id)
  return id === "" ? null : `agent:${id}`
}

const pylonFromPublicRegistration = (value: unknown): DesktopPylon => {
  const pylon = asRecord(value)
  const codingCapacity = asArray(pylon.codingCapacity).map(asRecord)
  const latestHeartbeatStatus = stringValue(pylon.latestHeartbeatStatus)
  const status = latestHeartbeatStatus || stringValue(pylon.status, "unknown")

  return {
    busySlots: sumCapacityField(codingCapacity, "busy"),
    heartbeatFresh: status.trim().toLowerCase() === "online",
    latestHeartbeatAt: nullableString(pylon.latestHeartbeatAt),
    latestHeartbeatLabel: nullableString(pylon.latestHeartbeatDisplay),
    ownerAgentRef: nullableString(pylon.ownerAgentRef),
    pylonRef: stringValue(pylon.pylonRef, "unknown-pylon"),
    queuedSlots: sumCapacityField(codingCapacity, "queued"),
    readySlots: sumCapacityField(codingCapacity, "ready"),
    status,
  }
}

export const pylonsFromPublicPylonList = (
  pylonList: unknown,
  ownerAgentRef?: string | null,
): readonly DesktopPylon[] => {
  const list = asRecord(pylonList)
  const pylons = asArray(list.pylons).map(pylonFromPublicRegistration)
  return ownerAgentRef === undefined || ownerAgentRef === null
    ? pylons
    : pylons.filter(pylon => pylon.ownerAgentRef === ownerAgentRef)
}

export const connectedPylonCount = (
  pylons: readonly DesktopPylon[],
): number =>
  pylons.filter(
    pylon =>
      pylon.heartbeatFresh === true ||
      pylon.status.trim().toLowerCase() === "online",
  ).length

export const pylonCountFromPublicPylonList = (
  pylonList: unknown,
  ownerAgentRef?: string | null,
): number => {
  return connectedPylonCount(
    pylonsFromPublicPylonList(pylonList, ownerAgentRef),
  )
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
      pylons: [],
      error: "Set OPENAGENTS_AGENT_TOKEN to load your pylons.",
      observedAt,
    }
  }

  const baseUrl = (options.baseUrl ?? OPENAGENTS_DESKTOP_DEFAULT_BASE_URL)
    .replace(/\/+$/, "")
  const fetchImpl = options.fetch ?? fetch

  try {
    const agentResponse = await fetchImpl(`${baseUrl}/api/agents/me`, {
      headers: {
        accept: "application/json",
        authorization: `Bearer ${token}`,
      },
      method: "GET",
    })
    if (!agentResponse.ok) {
      throw new Error(
        agentResponse.status === 401
          ? "OPENAGENTS_AGENT_TOKEN is not an active OpenAgents agent token."
          : "OpenAgents could not load your account right now.",
      )
    }

    const ownerAgentRef = agentRefFromMe(await agentResponse.json())
    if (ownerAgentRef === null) {
      throw new Error(
        "OpenAgents could not identify your account from the active token.",
      )
    }

    const pylonsResponse = await fetchImpl(`${baseUrl}/api/pylons`, {
      headers: { accept: "application/json" },
      method: "GET",
    })
    if (!pylonsResponse.ok) {
      throw new Error("OpenAgents could not load your pylons right now.")
    }

    const pylons = pylonsFromPublicPylonList(
      await pylonsResponse.json(),
      ownerAgentRef,
    )
    return {
      ok: true,
      count: connectedPylonCount(pylons),
      pylons,
      observedAt,
    }
  } catch (error) {
    return {
      ok: false,
      count: 0,
      pylons: [],
      error: error instanceof Error ? error.message : String(error),
      observedAt,
    }
  }
}
