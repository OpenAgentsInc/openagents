// Dev-transport control client: the mobile app reaches a Pylon control server
// over the network (loopback for a simulator, or the host's tailnet/LAN IP for a
// real device on the same tailnet/Wi-Fi) using the node's bearer token.
// Pure-ish: just fetch + shape; no UI. The secure bridge pairing layer (CL-14)
// is wired below and supersedes the dev token for read access.

import {
  buildSubscribeRequest,
  pairBridge,
  parseEventBatch,
  createBridgeTransport,
  decodeBootstrapPayload,
  projectArtifactContentView,
  resolveBaseUrls,
  type ArtifactContentView,
  type BridgeCredential,
  type Capability,
  type ProjectionLevel,
  type SessionEvent,
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

export type BridgeSession = {
  transport: ReturnType<typeof createBridgeTransport>
  credential: BridgeCredential
  // The node base URL this credential is bound to. Tracked so the live
  // `session.subscribe` stream (#5493) can POST its cursor envelopes over the
  // same /bridge endpoint the transport uses, without re-deriving it.
  baseUrl: string
}

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
  return pairBridgeWithBootstrap(conn.baseUrl, boot, opts)
}

// Shared pairing tail: exchange an already-minted single-use bootstrap for a
// scoped credential and return a transport bound to it. Used by both the
// dev-token mint path (connectBridge) and the dev-token-free external-bootstrap
// path (connectBridgeWithBootstrap).
async function pairBridgeWithBootstrap(
  baseUrl: string,
  boot: { bootstrapId: string; secret: string },
  opts: { capabilities?: Capability[]; projectionLevel?: ProjectionLevel; clientId?: string },
): Promise<BridgeSession | null> {
  const pair = await pairBridge({
    baseUrl,
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
  return { transport: createBridgeTransport({ baseUrl, credential }), credential, baseUrl }
}

// Dev-token-FREE pairing: pair onto the bridge using an externally supplied
// single-use bootstrap (decoded from a QR/pasted pairing code or URI the node
// operator displays) instead of minting one over the dev token. Accepts the
// three bootstrap encodings the node renders: an `autopilot://pair?...` URI, the
// rendered text block, or a raw `bootstrapId:secret` code. Returns the paired
// BridgeSession plus the resolved baseUrl (so the caller can drive reads without
// ever holding a long-lived dev token), or null if the code is undecodable / no
// reachable address / the exchange is rejected.
export async function connectBridgeWithBootstrap(
  qrOrCode: string,
  opts: { capabilities?: Capability[]; projectionLevel?: ProjectionLevel; clientId?: string } = {},
): Promise<{ session: BridgeSession; baseUrl: string } | null> {
  let payload: ReturnType<typeof decodeBootstrapPayload>
  try {
    payload = decodeBootstrapPayload(qrOrCode.trim())
  } catch {
    return null
  }
  const baseUrl = resolveBaseUrls(payload.addresses)[0]
  if (!baseUrl) return null
  const session = await pairBridgeWithBootstrap(
    baseUrl,
    { bootstrapId: payload.bootstrapId, secret: payload.secret },
    opts,
  )
  return session ? { session, baseUrl } : null
}

// List sessions over the bridge transport (CL-14 secure read path).
export async function fetchSessionsViaBridge(bridge: BridgeSession): Promise<SessionSummary[]> {
  return bridge.transport.list()
}

// List sessions over the capability-scoped bridge, projected into the same
// ControlSessionRow shape the screens consume (CL-14 secure read path). This is
// the bridge-native session LIST: it reads over the scoped credential with no
// long-lived dev token on the wire. SessionSummary does not carry the
// artifact/result/error refs (those are detail-level), so they project to null
// here — the session-detail screen fetches the artifact separately.
export async function fetchSessionRowsViaBridge(bridge: BridgeSession): Promise<ControlSessionRow[]> {
  const summaries = await fetchSessionsViaBridge(bridge)
  return summaries.map((s) => ({
    sessionRef: s.sessionRef,
    adapter: String(s.adapter),
    state: String(s.state),
    lastProgressRef: s.lastProgressRef ?? "none",
    artifactRef: null,
    resultRef: null,
    errorClass: null,
    latestActivity: s.latestActivity ?? "",
    parentRef: s.parentRef ?? null,
    agentKind: s.agentKind ?? null,
  }))
}

// Stream a session's events over the bridge (#5000 session.history). Same row
// shape as the dev-token fetchSessionEvents; the node returns its session-events
// projection (recentEvents) over the capability-scoped bridge, so the read-only
// app can watch a session (local OR cloud — #5005 made the stream lane-uniform)
// without a long-lived dev token on the wire.
export async function fetchSessionEventsViaBridge(
  bridge: BridgeSession,
  sessionRef: string,
): Promise<ControlSessionEventRow[]> {
  const result = (await bridge.transport.history(sessionRef)) as { recentEvents?: unknown }
  const events = result?.recentEvents
  if (!Array.isArray(events)) return []
  return events.map((e: any) => ({
    eventIndex: Number(e.eventIndex ?? 0),
    observedAt: String(e.observedAt ?? ""),
    phase: String(e.phase ?? "?"),
    state: String(e.state ?? "?"),
    detail: typeof e.messageText === "string" && e.messageText.length > 0 ? e.messageText : "",
    full: typeof e.messageFull === "string" ? e.messageFull : "",
  }))
}

// G3 (#5495) artifact/diff viewer over the bridge: read the retained
// proof/failure artifact a completed session produced (read_artifact
// capability), projected into the render-ready ArtifactContentView (changed-file
// list, dev-check command transcript, deviations, verbatim text). No long-lived
// dev token on the wire. Returns null if the node has no artifact for the
// session (kind "none") or the read fails.
export async function fetchSessionArtifactContentViaBridge(
  bridge: BridgeSession,
  sessionRef: string,
): Promise<ArtifactContentView | null> {
  const envelope = await bridge.transport.readArtifact(sessionRef)
  if (envelope.kind === "none") return null
  return projectArtifactContentView({ kind: envelope.kind, artifact: envelope.artifact })
}

// #5493 live streaming: fetch the next batch of a session's ordered events over
// the capability-scoped bridge using the `session.subscribe` cursor verb. This
// is the bridge-native streaming read the mobile `session-subscription` cursor
// machine consumes — pass the cursor's `lastSequence` and the node replays only
// events after it (empty batch when nothing new), so the timeline advances with
// no full re-fetch and no long-lived dev token on the wire. Pure-ish: builds the
// envelope from the shared protocol, POSTs over the same /bridge endpoint as the
// transport, and decodes the typed event batch. Throws on a non-ok response so
// the caller can classify the drop and fall back to polling.
let bridgeSubscribeCounter = 0

export async function fetchSessionEventBatchViaBridge(
  bridge: BridgeSession,
  sessionRef: string,
  cursor: number,
): Promise<SessionEvent[]> {
  const clientRequestId = `mobile.subscribe.${++bridgeSubscribeCounter}`
  const envelope = buildSubscribeRequest({
    sessionRef,
    pairingRef: bridge.credential.pairingRef,
    capabilityRef: bridge.credential.capabilityRef ?? "observe_public",
    clientRequestId,
    idempotencyKey: clientRequestId,
    // The cursor is the resume point; omit it for a fresh subscribe so the node
    // replays from the start of its retained window.
    ...(cursor > 0 ? { cursor } : {}),
  })
  const res = await fetch(`${bridge.baseUrl.replace(/\/+$/, "")}/bridge`, {
    method: "POST",
    headers: {
      authorization: `Bridge ${bridge.credential.pairingRef}:${bridge.credential.jti}`,
      "content-type": "application/json",
    },
    body: JSON.stringify(envelope),
  })
  const json = (await res.json()) as { ok?: unknown; result?: unknown; error?: unknown }
  if (!res.ok || json.ok !== true) {
    throw new Error(typeof json.error === "string" ? json.error : `bridge subscribe failed (${res.status})`)
  }
  return parseEventBatch(json.result)
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

// G3 (#5495) dev-token fallback for the artifact/diff viewer: read the full
// retained artifact body via the `session.artifact` /command verb and project it
// into the render-ready ArtifactContentView. Used when no bridge credential is
// established (the bridge read_artifact path is preferred). Returns null on a
// "none"/absent artifact or transport failure.
export async function fetchSessionArtifactContent(
  conn: ConnectInfo,
  sessionRef: string,
): Promise<ArtifactContentView | null> {
  try {
    const json = (await command(conn, { type: "session.artifact", sessionRef })) as {
      ok?: boolean
      result?: { kind?: unknown; artifact?: unknown }
    }
    const result = json.ok === true ? json.result : undefined
    const kind = result?.kind === "proof" || result?.kind === "failure" ? result.kind : "none"
    if (kind === "none") return null
    return projectArtifactContentView({ kind, artifact: result?.artifact ?? null })
  } catch {
    return null
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

// CL-17 (rescoped): pause/resume the node's autonomous coordinator work loop.
// (Per-session pause isn't possible — agents run to completion; use cancel.)
export async function fetchCoordinatorPaused(conn: ConnectInfo): Promise<boolean | null> {
  try {
    const json = (await command(conn, { type: "coordinator.status" })) as { ok?: boolean; result?: { paused?: unknown } }
    const r = json.ok === true ? json.result : undefined
    return typeof r?.paused === "boolean" ? r.paused : null
  } catch {
    return null
  }
}

export async function setCoordinatorPaused(conn: ConnectInfo, paused: boolean): Promise<boolean> {
  const json = (await command(conn, { type: paused ? "coordinator.pause" : "coordinator.resume" })) as {
    ok?: boolean
    result?: { paused?: unknown }
  }
  const r = json.ok === true ? json.result : undefined
  return typeof r?.paused === "boolean" ? r.paused : paused
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
export type SessionLane = "auto" | "local" | "cloud-gcp" | "cloud-shc"

export async function spawnSession(
  conn: ConnectInfo,
  draft: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[]; lane?: SessionLane },
): Promise<string> {
  const json = (await command(conn, {
    type: "session.spawn",
    adapter: draft.adapter,
    objective: draft.objective,
    verify: draft.verify ?? [],
    // #4998 lane selector: `auto` = own-Pylon-first-and-free then Google GCE.
    lane: draft.lane ?? "auto",
  })) as { ok?: boolean; result?: { sessionRef?: unknown } }
  if (json.ok !== true) throw new Error("spawn failed")
  return String(json.result?.sessionRef ?? "spawned")
}

// #5002 bridge write actions. Resolve a decision / cancel a session over the
// capability-scoped bridge credential (answer_decision / cancel) instead of the
// dev token. Returns the classified outcome so the UI can render a typed receipt.
export async function resolveDecisionViaBridge(
  bridge: BridgeSession,
  input: { approvalRef: string; decision: "approve" | "deny" | "answer"; answer?: string },
): Promise<{ applied: boolean; duplicate: boolean; decision: string }> {
  const result = (await bridge.transport.resolveDecision({
    requestId: input.approvalRef,
    verb: input.decision,
    ...(input.answer === undefined ? {} : { answer: input.answer }),
  })) as { applied?: unknown; duplicate?: unknown; decision?: unknown }
  return {
    applied: result?.applied === true,
    duplicate: result?.duplicate === true,
    decision: typeof result?.decision === "string" ? result.decision : input.decision,
  }
}

export async function cancelSessionViaBridge(bridge: BridgeSession, sessionRef: string): Promise<string> {
  const result = (await bridge.transport.cancel(sessionRef)) as { state?: unknown }
  return String(result?.state ?? "cancelled")
}

// #5494 (epic #5492 G1): the remaining four steer-actions over the
// capability-scoped bridge — spawn, submit-intent, pause/resume, deploy. The
// bridge transport sends each over POST /bridge with the scoped credential
// (capability classes spawn_session / send_instruction / pause_resume /
// deploy_cloud), so the mobile client no longer needs the raw node dev token on
// the wire for these. Each mirrors the dev-token return shape its caller
// already consumes; the ConnectionContext prefers these when a bridge
// credential is paired and falls back to the dev-token path otherwise.

export async function spawnSessionViaBridge(
  bridge: BridgeSession,
  draft: { adapter: "codex" | "claude_agent"; objective: string; verify?: string[]; lane?: SessionLane },
): Promise<string> {
  const result = (await bridge.transport.spawn({
    adapter: draft.adapter,
    objective: draft.objective,
    verify: draft.verify ?? [],
    lane: draft.lane ?? "auto",
  })) as { sessionRef?: unknown }
  return String(result?.sessionRef ?? "spawned")
}

export async function submitIntentViaBridge(
  bridge: BridgeSession,
  draft: { title: string; body: string },
): Promise<string> {
  const result = (await bridge.transport.submitIntent({
    title: draft.title,
    body: draft.body,
    submittedByClientRef: "mobile",
  })) as { status?: unknown }
  return String(result?.status ?? "received")
}

export async function setCoordinatorPausedViaBridge(bridge: BridgeSession, paused: boolean): Promise<boolean> {
  const result = (paused
    ? await bridge.transport.pauseCoordinator()
    : await bridge.transport.resumeCoordinator()) as { paused?: unknown }
  return typeof result?.paused === "boolean" ? result.paused : paused
}

export async function deployToCloudViaBridge(
  bridge: BridgeSession,
  input: { target: DeployTarget; ref: string; env?: DeployEnv },
): Promise<DeployResult> {
  const result = (await bridge.transport.deployCloud({
    target: input.target,
    ref: input.ref,
    ...(input.env ? { env: input.env } : {}),
  })) as { accepted?: unknown; reason?: unknown; errors?: unknown }
  return {
    accepted: result?.accepted === true,
    reason: typeof result?.reason === "string" ? result.reason : "unknown",
    errors: Array.isArray(result?.errors) ? result.errors.map((e: unknown) => String(e)) : [],
  }
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
