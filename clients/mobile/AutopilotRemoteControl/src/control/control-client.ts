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
  artifactRef: string | null
  resultRef: string | null
  errorClass: string | null
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
    artifactRef: typeof s.artifactRef === "string" ? s.artifactRef : null,
    resultRef: typeof s.resultRef === "string" ? s.resultRef : null,
    errorClass: typeof s.errorClass === "string" ? s.errorClass : null,
  }))
}

export type SessionArtifact = {
  kind: "proof" | "failure" | "none"
  outcome: string | null
  editedFileCount: number | null
  commandCount: number | null
  totalTokens: number | null
}

// Fetch the retained artifact a completed session produced (CL-19). Returns key
// executor stats from the projection-safe proof/failure JSON.
export async function fetchSessionArtifact(conn: ConnectInfo, sessionRef: string): Promise<SessionArtifact> {
  const json = (await command(conn, { type: "session.artifact", sessionRef })) as {
    ok?: boolean
    result?: { kind?: unknown; artifact?: any }
  }
  const result = json.ok === true ? json.result : undefined
  const a = result?.artifact
  const ex = a && typeof a === "object" ? a.executor : undefined
  return {
    kind: (result?.kind as SessionArtifact["kind"]) ?? "none",
    outcome: ex && typeof ex.outcome === "string" ? ex.outcome : null,
    editedFileCount: ex && typeof ex.editedFileCount === "number" ? ex.editedFileCount : null,
    commandCount: ex && typeof ex.commandCount === "number" ? ex.commandCount : null,
    totalTokens: ex && typeof ex.totalTokens === "number" ? ex.totalTokens : null,
  }
}

export type AccountRow = { provider: string; homeState: string; ready: boolean }

// Read-only accounts + readiness panel (CL-18). Public-projection-safe on the
// node side (refs only); we render provider + home presence + ready/blocked.
export async function fetchAccounts(conn: ConnectInfo): Promise<AccountRow[]> {
  const json = (await command(conn, { type: "accounts.list" })) as {
    ok?: boolean
    result?: { accounts?: unknown }
  }
  const accounts = json.ok === true ? json.result?.accounts : undefined
  if (!Array.isArray(accounts)) return []
  return accounts.map((a: any) => ({
    provider: String(a.provider ?? "?"),
    homeState: String(a.homeState ?? "?"),
    ready: Array.isArray(a.blockerRefs) ? a.blockerRefs.length === 0 : false,
  }))
}

// Submit a composed "ask" to the node (CL-34). The node enqueues it as a work
// intent for the coordinator to plan + fan out. Returns the intent status.
export async function submitIntent(
  conn: ConnectInfo,
  draft: { title: string; body: string },
): Promise<string> {
  const json = (await command(conn, {
    type: "intent.submit",
    title: draft.title,
    body: draft.body,
    submittedByClientRef: "mobile",
  })) as { ok?: boolean; result?: { status?: unknown } }
  if (json.ok !== true) throw new Error("submit failed")
  return String(json.result?.status ?? "received")
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
