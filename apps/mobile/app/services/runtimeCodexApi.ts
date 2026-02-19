import Config from "@/config"

const base = () => Config.authApiUrl.replace(/\/$/, "")

export type RuntimeCodexWorkerStatus = "starting" | "running" | "stopping" | "stopped" | "failed"

export type RuntimeCodexProjectionStatus = {
  document_id: string
  last_runtime_seq: number
  lag_events: number
  status: "in_sync" | "lagging"
  projection_version: string
  last_projected_at: string | null
}

export type RuntimeCodexWorkerSummary = {
  worker_id: string
  status: RuntimeCodexWorkerStatus
  latest_seq: number
  workspace_ref: string | null
  codex_home_ref: string | null
  adapter: string
  metadata: Record<string, unknown>
  started_at: string | null
  stopped_at: string | null
  last_heartbeat_at: string | null
  heartbeat_age_ms: number | null
  heartbeat_stale_after_ms: number
  heartbeat_state: "fresh" | "stale" | "missing" | "stopped" | "failed"
  updated_at: string | null
  convex_projection: RuntimeCodexProjectionStatus | null
}

export type RuntimeCodexWorkerSnapshot = {
  worker_id: string
  status: RuntimeCodexWorkerStatus
  latest_seq: number
  workspace_ref: string | null
  codex_home_ref: string | null
  adapter: string
  metadata: Record<string, unknown>
  started_at: string | null
  stopped_at: string | null
  last_heartbeat_at: string | null
  heartbeat_age_ms: number | null
  heartbeat_stale_after_ms: number
  heartbeat_state: "fresh" | "stale" | "missing" | "stopped" | "failed"
  updated_at: string | null
}

export type RuntimeCodexWorkerRequestResult = {
  worker_id: string
  request_id: string
  ok: boolean
  response: unknown
}

export type RuntimeCodexWorkerStopResult = {
  worker_id: string
  status: RuntimeCodexWorkerStatus
  idempotent_replay: boolean
}

export type RuntimeCodexStreamEvent = {
  id: number | null
  event: string
  payload: unknown
}

export type MobileConvexToken = {
  token: string
  token_type: string
  expires_in: number
  expires_at?: string
}

export class RuntimeCodexApiError extends Error {
  code: "auth" | "forbidden" | "conflict" | "invalid" | "network" | "unknown"
  status?: number

  constructor(
    message: string,
    code: "auth" | "forbidden" | "conflict" | "invalid" | "network" | "unknown",
    status?: number,
  ) {
    super(message)
    this.name = "RuntimeCodexApiError"
    this.code = code
    this.status = status
  }
}

type RequestOptions = {
  method?: "GET" | "POST"
  token: string
  body?: unknown
  headers?: Record<string, string>
}

async function requestJson<T>(path: string, opts: RequestOptions): Promise<T> {
  let response: Response

  try {
    response = await fetch(`${base()}${path}`, {
      method: opts.method ?? "GET",
      headers: {
        "Authorization": `Bearer ${opts.token}`,
        "content-type": "application/json",
        ...(opts.headers ?? {}),
      },
      body: opts.body === undefined ? undefined : JSON.stringify(opts.body),
    })
  } catch (error) {
    throw new RuntimeCodexApiError(
      `network_error: ${String((error as Error)?.message ?? error)}`,
      "network",
    )
  }

  const parsed = (await response.json().catch(() => null)) as {
    data?: unknown
    error?: { code?: string; message?: string }
  } | null

  if (!response.ok) {
    throw toRuntimeError(response.status, parsed)
  }

  return parsed as T
}

async function requestText(path: string, opts: RequestOptions): Promise<string> {
  let response: Response

  try {
    response = await fetch(`${base()}${path}`, {
      method: opts.method ?? "GET",
      headers: {
        Authorization: `Bearer ${opts.token}`,
        Accept: "text/event-stream",
        ...(opts.headers ?? {}),
      },
    })
  } catch (error) {
    throw new RuntimeCodexApiError(
      `network_error: ${String((error as Error)?.message ?? error)}`,
      "network",
    )
  }

  const text = await response.text()

  if (!response.ok) {
    let parsed: { error?: { code?: string; message?: string } } | null = null
    try {
      parsed = JSON.parse(text)
    } catch {
      parsed = null
    }
    throw toRuntimeError(response.status, parsed)
  }

  return text
}

function toRuntimeError(
  status: number,
  payload: { error?: { code?: string; message?: string } } | null,
): RuntimeCodexApiError {
  const message = payload?.error?.message ?? `request_failed_${status}`

  if (status === 401) {
    return new RuntimeCodexApiError(message, "auth", status)
  }

  if (status === 403) {
    return new RuntimeCodexApiError(message, "forbidden", status)
  }

  if (status === 409) {
    return new RuntimeCodexApiError(message, "conflict", status)
  }

  if (status === 400 || status === 422) {
    return new RuntimeCodexApiError(message, "invalid", status)
  }

  return new RuntimeCodexApiError(message, "unknown", status)
}

export async function listRuntimeCodexWorkers(
  token: string,
  status: "all" | RuntimeCodexWorkerStatus = "all",
): Promise<RuntimeCodexWorkerSummary[]> {
  const query = new URLSearchParams()
  if (status !== "all") query.set("status", status)
  query.set("limit", "100")
  const suffix = query.toString()

  const response = await requestJson<{ data?: RuntimeCodexWorkerSummary[] }>(
    `/api/runtime/codex/workers${suffix.length > 0 ? `?${suffix}` : ""}`,
    { token },
  )

  return Array.isArray(response.data) ? response.data : []
}

export async function getRuntimeCodexWorkerSnapshot(
  token: string,
  workerId: string,
): Promise<RuntimeCodexWorkerSnapshot> {
  const encoded = encodeURIComponent(workerId)
  const response = await requestJson<{ data?: RuntimeCodexWorkerSnapshot }>(
    `/api/runtime/codex/workers/${encoded}`,
    { token },
  )

  if (!response?.data) {
    throw new RuntimeCodexApiError("worker_snapshot_missing", "unknown")
  }

  return response.data
}

export async function requestRuntimeCodexWorker(
  token: string,
  workerId: string,
  method: string,
  params: Record<string, unknown>,
): Promise<RuntimeCodexWorkerRequestResult> {
  const encoded = encodeURIComponent(workerId)
  const response = await requestJson<{ data?: RuntimeCodexWorkerRequestResult }>(
    `/api/runtime/codex/workers/${encoded}/requests`,
    {
      method: "POST",
      token,
      body: {
        request: {
          request_id: `mobile_req_${Date.now()}`,
          method,
          params,
        },
      },
    },
  )

  if (!response?.data) {
    throw new RuntimeCodexApiError("worker_request_missing", "unknown")
  }

  return response.data
}

export async function stopRuntimeCodexWorker(
  token: string,
  workerId: string,
  reason = "mobile_admin_stop",
): Promise<RuntimeCodexWorkerStopResult> {
  const encoded = encodeURIComponent(workerId)
  const response = await requestJson<{ data?: RuntimeCodexWorkerStopResult }>(
    `/api/runtime/codex/workers/${encoded}/stop`,
    {
      method: "POST",
      token,
      body: { reason },
    },
  )

  if (!response?.data) {
    throw new RuntimeCodexApiError("worker_stop_missing", "unknown")
  }

  return response.data
}

export async function streamRuntimeCodexWorker(
  token: string,
  workerId: string,
  cursor: number,
  tailMs = 8000,
): Promise<{ events: RuntimeCodexStreamEvent[]; nextCursor: number }> {
  const encoded = encodeURIComponent(workerId)
  const query = new URLSearchParams()
  query.set("cursor", String(Math.max(0, cursor)))
  query.set("tail_ms", String(Math.max(1000, tailMs)))

  const body = await requestText(
    `/api/runtime/codex/workers/${encoded}/stream?${query.toString()}`,
    { token },
  )

  const events = parseSseEvents(body)
  let nextCursor = cursor

  for (const event of events) {
    if (typeof event.id === "number" && event.id > nextCursor) {
      nextCursor = event.id
    }
  }

  return { events, nextCursor }
}

export function parseSseEvents(raw: string): RuntimeCodexStreamEvent[] {
  const chunks = raw
    .split(/\n\n+/)
    .map((chunk) => chunk.trim())
    .filter((chunk) => chunk.length > 0)

  const events: RuntimeCodexStreamEvent[] = []

  for (const chunk of chunks) {
    const lines = chunk.split(/\n/)
    let id: number | null = null
    let event = "message"
    const dataLines: string[] = []

    for (const line of lines) {
      if (line.startsWith("id:")) {
        const rawId = line.slice(3).trim()
        const parsedId = Number.parseInt(rawId, 10)
        id = Number.isFinite(parsedId) ? parsedId : null
      } else if (line.startsWith("event:")) {
        event = line.slice(6).trim() || "message"
      } else if (line.startsWith("data:")) {
        dataLines.push(line.slice(5).trim())
      }
    }

    if (dataLines.length === 0) continue
    const dataRaw = dataLines.join("\n")

    let payload: unknown = dataRaw
    try {
      payload = JSON.parse(dataRaw)
    } catch {
      payload = dataRaw
    }

    events.push({ id, event, payload })
  }

  return events
}

export async function mintConvexToken(token: string): Promise<MobileConvexToken> {
  const response = await requestJson<{ data?: MobileConvexToken }>("/api/convex/token", {
    method: "POST",
    token,
    body: {
      scope: ["mobile", "codex.read", "codex.admin"],
      role: "member",
    },
  })

  if (!response?.data || typeof response.data.token !== "string") {
    throw new RuntimeCodexApiError("convex_token_missing", "unknown")
  }

  return response.data
}
