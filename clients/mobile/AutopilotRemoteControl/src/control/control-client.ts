// Dev-transport control client: the mobile app reaches a Pylon control server
// over the network (loopback for a simulator, or the host's tailnet/LAN IP for a
// real device on the same tailnet/Wi-Fi) using the node's bearer token.
// Pure-ish: just fetch + shape; no UI. (The secure bridge pairing layer supersedes
// this later; this is the M1/dev path to a first live view.)

export type ConnectInfo = { baseUrl: string; token: string }

// A "connect code" is base64 of `${baseUrl}|${token}` — generate on the Mac and
// paste into the app so you don't hand-type a long token on the phone.
export function decodeConnectCode(code: string): ConnectInfo | null {
  try {
    const decoded =
      typeof atob === "function" ? atob(code.trim()) : Buffer.from(code.trim(), "base64").toString("utf8")
    const sep = decoded.lastIndexOf("|")
    if (sep <= 0) return null
    const baseUrl = decoded.slice(0, sep).trim().replace(/\/+$/, "")
    const token = decoded.slice(sep + 1).trim()
    if (!/^https?:\/\//.test(baseUrl) || token.length === 0) return null
    return { baseUrl, token }
  } catch {
    return null
  }
}

export type ControlSessionRow = {
  sessionRef: string
  adapter: string
  state: string
  lastProgressRef: string
}

async function command(conn: ConnectInfo, body: unknown): Promise<any> {
  const res = await fetch(`${conn.baseUrl}/command`, {
    method: "POST",
    headers: { authorization: `Bearer ${conn.token}`, "content-type": "application/json" },
    body: JSON.stringify(body),
  })
  if (!res.ok) throw new Error(`control ${res.status}`)
  return res.json()
}

export async function fetchHealth(conn: ConnectInfo): Promise<boolean> {
  try {
    const res = await fetch(`${conn.baseUrl}/health`, {
      headers: { authorization: `Bearer ${conn.token}` },
    })
    const json = (await res.json()) as { ok?: boolean }
    return json.ok === true
  } catch {
    return false
  }
}

export async function fetchSessions(conn: ConnectInfo): Promise<ControlSessionRow[]> {
  const json = (await command(conn, { type: "session.list" })) as { ok?: boolean; result?: unknown }
  if (json.ok !== true || !Array.isArray(json.result)) throw new Error("bad session.list response")
  return json.result.map((s: any) => ({
    sessionRef: String(s.sessionRef ?? "?"),
    adapter: String(s.adapter ?? "?"),
    state: String(s.state ?? "?"),
    lastProgressRef: String(s.lastProgressRef ?? s.resultRef ?? "none"),
  }))
}

// Cancel a running/queued session (CL-15). Best-effort: returns the new state.
export async function cancelSession(conn: ConnectInfo, sessionRef: string): Promise<string> {
  const json = (await command(conn, { type: "session.cancel", sessionRef })) as {
    ok?: boolean
    result?: { state?: unknown }
  }
  if (json.ok !== true) throw new Error("cancel failed")
  return String(json.result?.state ?? "cancelled")
}

export type ControlSessionEventRow = {
  eventIndex: number
  observedAt: string
  phase: string
  state: string
  // What the agent is actually doing (agent text / tool call / file change).
  detail: string
}

// Live session-detail timeline (dev transport). Uses the inline recentEvents
// tail from the node's in-memory event log — RN fetch can't consume the SSE
// stream cleanly, so we poll the non-streaming POST /command path.
export async function fetchSessionEvents(
  conn: ConnectInfo,
  sessionRef: string,
): Promise<ControlSessionEventRow[]> {
  const json = (await command(conn, { type: "session.events", sessionRef })) as {
    ok?: boolean
    result?: { recentEvents?: unknown }
  }
  const events = json.ok === true ? json.result?.recentEvents : undefined
  if (!Array.isArray(events)) throw new Error("bad session.events response")
  return events.map((e: any) => ({
    eventIndex: Number(e.eventIndex ?? 0),
    observedAt: String(e.observedAt ?? ""),
    phase: String(e.phase ?? "?"),
    state: String(e.state ?? "?"),
    detail: typeof e.messageText === "string" && e.messageText.length > 0 ? e.messageText : "",
  }))
}
