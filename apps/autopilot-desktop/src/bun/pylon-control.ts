import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import {
  decodeSessionSummary,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow, SessionEventRow } from "../shared/rpc"

type NodeHealth = {
  ok?: unknown
  schema?: unknown
}

type CommandResponse = {
  ok?: unknown
  result?: unknown
}

export function readControlToken(pylonHome: string): string | null {
  const tokenPath = join(pylonHome, "control-token")
  if (!existsSync(tokenPath)) return null

  const token = readFileSync(tokenPath, "utf8").trim()
  return token.length > 0 ? token : null
}

async function fetchSessionEventRows(input: {
  baseUrl: string
  token: string
  sessionRef: string
  fetchFn: typeof fetch
}): Promise<SessionEventRow[]> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "session.events", sessionRef: input.sessionRef }),
    })
    if (!res.ok) return []
    const json = (await res.json()) as { ok?: unknown; result?: { recentEvents?: unknown } }
    const events = json.ok === true ? json.result?.recentEvents : undefined
    if (!Array.isArray(events)) return []
    return events.slice(-12).map((e: any) => ({
      eventIndex: Number(e.eventIndex ?? 0),
      phase: String(e.phase ?? "?"),
      state: String(e.state ?? "?"),
      observedAt: String(e.observedAt ?? ""),
      detail: typeof e.messageText === "string" ? e.messageText : "",
    }))
  } catch {
    return []
  }
}

async function fetchAccountRows(input: {
  baseUrl: string
  token: string
  fetchFn: typeof fetch
}): Promise<AccountRow[]> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "accounts.list" }),
    })
    if (!res.ok) return []
    const json = (await res.json()) as { ok?: unknown; result?: { accounts?: unknown } }
    const accounts = json.ok === true ? json.result?.accounts : undefined
    if (!Array.isArray(accounts)) return []
    return accounts.map((a: any) => ({
      provider: String(a.provider ?? "?"),
      homeState: String(a.homeState ?? "?"),
      ready: Array.isArray(a.blockerRefs) ? a.blockerRefs.length === 0 : false,
    }))
  } catch {
    return []
  }
}

export async function fetchNodeState(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; schema: string; sessions: SessionSummary[]; events: Record<string, SessionEventRow[]>; accounts: AccountRow[] }> {
  const fetchFn = input.fetchFn ?? fetch
  const baseUrl = input.baseUrl.replace(/\/+$/, "")

  const healthResponse = await fetchFn(`${baseUrl}/health`)
  if (!healthResponse.ok) {
    throw new Error(`Pylon health request failed: ${healthResponse.status}`)
  }

  const health = (await healthResponse.json()) as NodeHealth
  if (typeof health.ok !== "boolean" || typeof health.schema !== "string") {
    throw new Error("Pylon health response was not a control health payload")
  }

  const commandResponse = await fetchFn(`${baseUrl}/command`, {
    method: "POST",
    headers: {
      Authorization: `Bearer ${input.token}`,
      "content-type": "application/json",
    },
    body: JSON.stringify({ type: "session.list" }),
  })
  if (!commandResponse.ok) {
    throw new Error(`Pylon session list request failed: ${commandResponse.status}`)
  }

  const command = (await commandResponse.json()) as CommandResponse
  if (command.ok !== true) {
    throw new Error("Pylon session list command failed")
  }
  if (!Array.isArray(command.result)) {
    throw new Error("Pylon session list command did not return an array")
  }

  const sessions = command.result.map((row) => decodeSessionSummary(row))

  // Live detail timeline: fetch the inline recentEvents tail per session (RN/web
  // can't consume the SSE stream cleanly; the bounded tail over POST /command is
  // the portable path). Bounded to keep per-poll cost sane.
  const events: Record<string, SessionEventRow[]> = {}
  for (const session of sessions.slice(0, 25)) {
    events[session.sessionRef] = await fetchSessionEventRows({
      baseUrl,
      token: input.token,
      sessionRef: session.sessionRef,
      fetchFn,
    })
  }

  const accounts = await fetchAccountRows({ baseUrl, token: input.token, fetchFn })

  return {
    ok: health.ok,
    schema: health.schema,
    sessions,
    events,
    accounts,
  }
}
