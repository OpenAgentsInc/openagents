// Dev-transport control client: the mobile app reaches a Pylon control server
// over the network (loopback for a simulator, or the host's tailnet/LAN IP for a
// real device on the same tailnet/Wi-Fi) using the node's bearer token.
// Pure-ish: just fetch + shape; no UI. The secure bridge pairing layer (CL-14)
// is wired below and supersedes the dev token for read access.

import {
  pairBridge,
  createBridgeTransport,
  type BridgeCredential,
  type Capability,
  type ProjectionLevel,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"

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

// ── CL-14 bridge transport ────────────────────────────────────────────────
// The secure path: exchange a single-use bootstrap (minted by the node via the
// `bridge.issueBootstrap` command) for a scoped, expiring pairing credential,
// then read sessions over POST /bridge — capability-gated, no long-lived dev
// token on the wire. Uses the shared `@openagentsinc/autopilot-control-protocol`
// transport (same code path as desktop).

export type BridgeSession = { transport: ReturnType<typeof createBridgeTransport>; credential: BridgeCredential }

// Mint a single-use bridge bootstrap from the node (dev-token authed). The
// secret is exchanged immediately at /bridge/pair and never re-used.
export async function issueBridgeBootstrap(conn: ConnectInfo): Promise<{ bootstrapId: string; secret: string }> {
  const json = (await command(conn, { type: "bridge.issueBootstrap" })) as {
    ok?: boolean
    result?: { bootstrapId?: unknown; secret?: unknown }
  }
  const r = json.ok === true ? json.result : undefined
  if (!r || typeof r.bootstrapId !== "string" || typeof r.secret !== "string") {
    throw new Error("bridge bootstrap unavailable")
  }
  return { bootstrapId: r.bootstrapId, secret: r.secret }
}

// Pair onto the bridge and return a transport bound to the scoped credential.
// Read-only by default (observe_public). Falls back to the dev token if the
// node doesn't expose bridge pairing.
export async function connectBridge(
  conn: ConnectInfo,
  opts: { capabilities?: Capability[]; projectionLevel?: ProjectionLevel; clientId?: string } = {},
): Promise<BridgeSession | null> {
  let boot: { bootstrapId: string; secret: string }
  try {
    boot = await issueBridgeBootstrap(conn)
  } catch {
    return null
  }
  const pair = await pairBridge({
    baseUrl: conn.baseUrl,
    bootstrapId: boot.bootstrapId,
    secret: boot.secret,
    clientId: opts.clientId ?? "mobile",
    deviceClass: "ios",
    capabilities: opts.capabilities ?? ["observe_public"],
    projectionLevel: opts.projectionLevel ?? "public_safe",
  })
  if (!pair.ok) return null
  const credential: BridgeCredential = {
    pairingRef: pair.claims.pairingRef,
    jti: pair.claims.jti,
    capabilityRef: opts.capabilities?.[0] ?? "observe_public",
  }
  return { transport: createBridgeTransport({ baseUrl: conn.baseUrl, credential }), credential }
}

// List sessions over the bridge transport (CL-14 secure read path).
export async function fetchSessionsViaBridge(bridge: BridgeSession): Promise<SessionSummary[]> {
  return bridge.transport.list()
}

export type ControlSessionRow = {
  sessionRef: string
  adapter: string
  state: string
  lastProgressRef: string
  artifactRef: string | null
  resultRef: string | null
  errorClass: string | null
  latestActivity: string
  parentRef: string | null
  agentKind: string | null
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
    latestActivity: typeof s.latestActivity === "string" ? s.latestActivity : "",
    parentRef: typeof s.parentRef === "string" ? s.parentRef : null,
    agentKind: typeof s.agentKind === "string" ? s.agentKind : null,
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

// Raw accounts.list rows for the registry-detail card (#4953). Returned as-is
// (public-projection-safe on the node side) so the client can run the shared
// projectAccountRegistryDetail() view-model over them.
export async function fetchAccountsRaw(conn: ConnectInfo): Promise<unknown[]> {
  const json = (await command(conn, { type: "accounts.list" })) as {
    ok?: boolean
    result?: { accounts?: unknown }
  }
  const accounts = json.ok === true ? json.result?.accounts : undefined
  return Array.isArray(accounts) ? accounts : []
}

export type ApprovalRow = {
  approvalRef: string
  kind: string
  prompt: string
  createdAt: string
}

// CL-16 approvals: read-only pending list. The node enforces exactly-once on
// resolve, so the client never needs to dedupe.
export async function fetchApprovals(conn: ConnectInfo): Promise<ApprovalRow[]> {
  const json = (await command(conn, { type: "approvals.list" })) as {
    ok?: boolean
    result?: { approvals?: unknown }
  }
  const rows = json.ok === true ? json.result?.approvals : undefined
  if (!Array.isArray(rows)) return []
  return rows.map((a: any) => ({
    approvalRef: String(a.approvalRef ?? "?"),
    kind: String(a.kind ?? "approval"),
    prompt: String(a.prompt ?? ""),
    createdAt: String(a.createdAt ?? ""),
  }))
}

// Resolve a pending approval (approve/deny/answer). Exactly-once is enforced on
// the node; a duplicate resolve returns {duplicate:true} and keeps the original.
export async function resolveApproval(
  conn: ConnectInfo,
  input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string },
): Promise<{ applied: boolean; duplicate: boolean; decision: string }> {
  const json = (await command(conn, {
    type: "approvals.resolve",
    approvalRef: input.approvalRef,
    decision: input.decision,
    ...(input.answer ? { answer: input.answer } : {}),
  })) as { ok?: boolean; result?: { applied?: unknown; duplicate?: unknown; decision?: unknown } }
  const r = json.ok === true ? json.result : undefined
  return {
    applied: r?.applied === true,
    duplicate: r?.duplicate === true,
    decision: typeof r?.decision === "string" ? r.decision : input.decision,
  }
}

export type IntentRow = {
  intentId: string
  title: string
  status: string
  submittedByClientRef: string
}

// Ship-status round-trip (CL-40): the phone polls the status of the asks it
// submitted, watching them advance received → planning → fanning_out →
// shipping → shipped/failed on the originating client.
export async function fetchIntents(conn: ConnectInfo): Promise<IntentRow[]> {
  const json = (await command(conn, { type: "intent.list" })) as {
    ok?: boolean
    result?: { intents?: unknown }
  }
  const intents = json.ok === true ? json.result?.intents : undefined
  if (!Array.isArray(intents)) return []
  return intents.map((i: any) => ({
    intentId: String(i.intentId ?? "?"),
    title: String(i.title ?? ""),
    status: String(i.status ?? "received"),
    submittedByClientRef: String(i.submittedByClientRef ?? ""),
  }))
}

export type AssignmentRow = {
  assignmentRef: string
  leaseRef: string
  goal: string
  paymentMode: string
  expiresAt: string
}

// Read-first work/assignment view (CL-22). Polls the node's open assignment
// leases. Read-only — accepting a lease is a separate, gated action.
export async function fetchAssignments(conn: ConnectInfo): Promise<AssignmentRow[]> {
  const json = (await command(conn, { type: "assignments.poll" })) as { ok?: boolean; result?: unknown }
  const rows = json.ok === true ? json.result : undefined
  if (!Array.isArray(rows)) return []
  return rows.map((a: any) => ({
    assignmentRef: String(a.assignmentRef ?? "?"),
    leaseRef: String(a.leaseRef ?? "?"),
    goal: String(a.goal ?? ""),
    paymentMode: String(a.paymentMode ?? "unknown"),
    expiresAt: String(a.expiresAt ?? ""),
  }))
}

export type WalletStatus = {
  configured: boolean
  daemonOnline: boolean
  balanceSats: number | null
  receiveReady: boolean
  sendReady: boolean
  readiness: string
}

// Read-only live MDK wallet status (CL-23): balance + readiness, no spend
// authority. Same balance the Pylon TUI shows. Returns null if unavailable.
export async function fetchWalletStatus(conn: ConnectInfo): Promise<WalletStatus | null> {
  try {
    const json = (await command(conn, { type: "wallet.status" })) as { ok?: boolean; result?: any }
    const r = json.ok === true ? json.result : undefined
    if (!r || typeof r !== "object") return null
    return {
      configured: r.configured === true,
      daemonOnline: r.daemonOnline === true,
      balanceSats: typeof r.balanceSats === "number" ? r.balanceSats : null,
      receiveReady: r.receiveReady === true,
      sendReady: r.sendReady === true,
      readiness: typeof r.readiness === "string" ? r.readiness : "unknown",
    }
  } catch {
    return null
  }
}

export type DeployTarget = "cloudrun" | "workers"
export type DeployEnv = "production" | "preview"

export type DeployResult = {
  accepted: boolean
  reason: string
  errors: string[]
}

export type DeployStatus = {
  state: "queued" | "building" | "deployed" | "failed" | "unknown"
  url: string | null
  deployedAt: string | null
  message: string
}

// CL-26 "Deploy to Cloud": trigger a deploy of one of the node's OWN cloud
// services through OUR pipeline (Cloud Run / Workers). The node validates the
// request and only runs anything when OA_DEPLOY_ENABLE=1 (fail-safe) — otherwise
// it returns {accepted:false, reason:"deploy_disabled"} and nothing deploys.
export async function deployToCloud(
  conn: ConnectInfo,
  input: { target: DeployTarget; ref: string; env?: DeployEnv },
): Promise<DeployResult> {
  try {
    const json = (await command(conn, {
      type: "deploy.cloud",
      target: input.target,
      ref: input.ref,
      ...(input.env ? { env: input.env } : {}),
    })) as { ok?: boolean; result?: { accepted?: unknown; reason?: unknown; errors?: unknown } }
    const r = json.ok === true ? json.result : undefined
    return {
      accepted: r?.accepted === true,
      reason: typeof r?.reason === "string" ? r.reason : json.ok === true ? "unknown" : "unavailable",
      errors: Array.isArray(r?.errors) ? r.errors.map((e: unknown) => String(e)) : [],
    }
  } catch (e) {
    return { accepted: false, reason: e instanceof Error ? e.message : "unavailable", errors: [] }
  }
}

// Read-only projection of the node's last deploy (CL-26). Returns "unknown" when
// the node has no deploy yet or doesn't expose the deploy actions.
export async function fetchDeployStatus(conn: ConnectInfo): Promise<DeployStatus> {
  try {
    const json = (await command(conn, { type: "deploy.status" })) as { ok?: boolean; result?: any }
    const r = json.ok === true ? json.result : undefined
    if (!r || typeof r !== "object") {
      return { state: "unknown", url: null, deployedAt: null, message: "Deployment status unavailable" }
    }
    return {
      state:
        r.state === "queued" || r.state === "building" || r.state === "deployed" || r.state === "failed"
          ? r.state
          : "unknown",
      url: typeof r.url === "string" ? r.url : null,
      deployedAt: typeof r.deployedAt === "string" ? r.deployedAt : null,
      message: typeof r.message === "string" ? r.message : "Deployment status unavailable",
    }
  } catch {
    return { state: "unknown", url: null, deployedAt: null, message: "Deployment status unavailable" }
  }
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

// Directly spawn a session on the node (CL-15). The compose/intent path is the
// primary UX; this is the explicit single-session spawn for advanced control.
// Returns the new session ref. Validation is the caller's responsibility
// (see validateSpawnRequest in the shared protocol).
export async function spawnSession(
  conn: ConnectInfo,
  draft: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[] },
): Promise<string> {
  const json = (await command(conn, {
    type: "session.spawn",
    adapter: draft.adapter,
    objective: draft.objective,
    verify: draft.verify ?? [],
  })) as { ok?: boolean; result?: { sessionRef?: unknown } }
  if (json.ok !== true) throw new Error("spawn failed")
  return String(json.result?.sessionRef ?? "spawned")
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
  // Full untruncated content, revealed on tap-to-expand (#4951).
  full: string
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
    full: typeof e.messageFull === "string" ? e.messageFull : "",
  }))
}
