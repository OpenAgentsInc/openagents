import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import {
  decodeSessionSummary,
  pairBridge,
  createBridgeTransport,
  type BridgeCredential,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import type {
  AccountRow,
  AppleFmReadinessResponse,
  AppleFmSessionStartResponse,
  ApprovalRow,
  AssignmentRow,
  IntentRow,
  SessionArtifactStats,
  SessionEventRow,
  WalletStatusRow,
} from "../shared/rpc"

// ── CL-14 bridge transport (desktop) ──────────────────────────────────────
// Same secure path as mobile, via the shared protocol transport: mint a
// single-use bootstrap (dev-token authed `bridge.issueBootstrap`), exchange it
// for a scoped pairing credential, then list sessions over POST /bridge.
export async function connectBridgeDesktop(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{ list: () => Promise<SessionSummary[]>; credential: BridgeCredential } | null> {
  const fetchFn = input.fetchFn ?? fetch
  let boot: { bootstrapId: string; secret: string }
  try {
    const res = await fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "bridge.issueBootstrap" }),
    })
    const json = (await res.json()) as { ok?: boolean; result?: { bootstrapId?: unknown; secret?: unknown } }
    if (json.ok !== true || typeof json.result?.bootstrapId !== "string" || typeof json.result?.secret !== "string") {
      return null
    }
    boot = { bootstrapId: json.result.bootstrapId, secret: json.result.secret }
  } catch {
    return null
  }
  const pair = await pairBridge({
    baseUrl: input.baseUrl,
    bootstrapId: boot.bootstrapId,
    secret: boot.secret,
    clientId: "desktop",
    deviceClass: "desktop",
    capabilities: ["observe_public"],
    projectionLevel: "public_safe",
    fetchImpl: fetchFn,
  })
  if (!pair.ok) return null
  const credential: BridgeCredential = {
    pairingRef: pair.claims.pairingRef,
    jti: pair.claims.jti,
    capabilityRef: "observe_public",
  }
  const transport = createBridgeTransport({ baseUrl: input.baseUrl, credential, fetchImpl: fetchFn })
  return { list: () => transport.list(), credential }
}

type NodeHealth = {
  ok?: unknown
  schema?: unknown
}

type CommandResponse = {
  ok?: unknown
  result?: unknown
}

const appleFmUnavailableProjection = (
  input: {
    fetchedAt?: string
    localPylonReady: boolean
    blockerRef: string
    error?: string
  },
): AppleFmReadinessResponse => ({
  ok: false,
  fetchedAt: input.fetchedAt ?? new Date().toISOString(),
  sourceUrl: "desktop:apple-fm-readiness",
  localPylonReady: input.localPylonReady,
  available: false,
  status: input.localPylonReady ? "unavailable" : "unreachable",
  backendKind: "apple_fm_bridge",
  profileId: "apple-fm-local",
  model: "apple-foundation-model",
  capability: "probe.backend.apple_fm_bridge",
  advertisedCapabilities: [],
  baseUrl: "http://127.0.0.1:11435",
  platform: null,
  version: null,
  unavailableReason: input.localPylonReady ? "unknown" : "bridge_unreachable",
  message: input.error ?? input.blockerRef,
  blockerRefs: [input.blockerRef],
  ...(input.error ? { error: input.error } : {}),
})

const stringArray = (value: unknown): readonly string[] =>
  Array.isArray(value) ? value.map(item => String(item)) : []

const optionalString = (value: unknown): string | null =>
  typeof value === "string" && value.trim() !== "" ? value : null

const requiredString = (value: unknown, fallback: string): string =>
  typeof value === "string" && value.trim() !== "" ? value : fallback

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
      // CL-52: full untruncated content, revealed on click-to-expand.
      full: typeof e.messageFull === "string" ? e.messageFull : "",
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

export type DesktopDeployStatus = {
  state: "queued" | "building" | "deployed" | "failed" | "unknown"
  url: string | null
  deployedAt: string | null
  message: string
}

export type DesktopDeployResult = {
  accepted: boolean
  reason: string
  errors: string[]
}

// CL-26 "Deploy to Cloud" (desktop): trigger a deploy of the node's OWN cloud
// service through OUR pipeline. The node validates and only runs anything when
// OA_DEPLOY_ENABLE=1 (fail-safe) — otherwise it returns
// {accepted:false, reason:"deploy_disabled"} and nothing deploys.
export async function deployToCloud(input: {
  baseUrl: string
  token: string
  target: "cloudrun" | "workers"
  ref: string
  env?: "production" | "preview"
  fetchFn?: typeof fetch
}): Promise<DesktopDeployResult> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({
        type: "deploy.cloud",
        target: input.target,
        ref: input.ref,
        ...(input.env ? { env: input.env } : {}),
      }),
    })
    if (!res.ok) return { accepted: false, reason: `control ${res.status}`, errors: [] }
    const json = (await res.json()) as { ok?: unknown; result?: { accepted?: unknown; reason?: unknown; errors?: unknown } }
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

// Read-only projection of the node's last deploy (CL-26). Returns "unknown"
// when the node has no deploy yet or doesn't expose the deploy actions.
export async function fetchDeployStatus(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<DesktopDeployStatus> {
  const unavailable: DesktopDeployStatus = {
    state: "unknown",
    url: null,
    deployedAt: null,
    message: "Deployment status unavailable",
  }
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "deploy.status" }),
    })
    if (!res.ok) return unavailable
    const json = (await res.json()) as { ok?: unknown; result?: any }
    const r = json.ok === true ? json.result : undefined
    if (!r || typeof r !== "object") return unavailable
    return {
      state:
        r.state === "queued" || r.state === "building" || r.state === "deployed" || r.state === "failed"
          ? r.state
          : "unknown",
      url: typeof r.url === "string" ? r.url : null,
      deployedAt: typeof r.deployedAt === "string" ? r.deployedAt : null,
      message: typeof r.message === "string" ? r.message : unavailable.message,
    }
  } catch {
    return unavailable
  }
}

async function fetchArtifactStats(input: {
  baseUrl: string
  token: string
  sessionRef: string
  fetchFn: typeof fetch
}): Promise<SessionArtifactStats | null> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "session.artifact", sessionRef: input.sessionRef }),
    })
    if (!res.ok) return null
    const json = (await res.json()) as { ok?: unknown; result?: { kind?: unknown; artifact?: any } }
    const result = json.ok === true ? json.result : undefined
    if (!result || result.kind === "none") return null
    const ex = result.artifact && typeof result.artifact === "object" ? result.artifact.executor : undefined
    return {
      kind: String(result.kind ?? "none"),
      outcome: ex && typeof ex.outcome === "string" ? ex.outcome : null,
      editedFileCount: ex && typeof ex.editedFileCount === "number" ? ex.editedFileCount : null,
      commandCount: ex && typeof ex.commandCount === "number" ? ex.commandCount : null,
      totalTokens: ex && typeof ex.totalTokens === "number" ? ex.totalTokens : null,
    }
  } catch {
    return null
  }
}

// CL-47: the owner's recent asks + their ship-status (intent.list).
async function fetchIntentRows(input: { baseUrl: string; token: string; fetchFn: typeof fetch }): Promise<IntentRow[]> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "intent.list" }),
    })
    if (!res.ok) return []
    const json = (await res.json()) as { ok?: unknown; result?: { intents?: unknown } }
    const intents = json.ok === true ? json.result?.intents : undefined
    if (!Array.isArray(intents)) return []
    return intents.map((i: any) => ({
      intentId: String(i.intentId ?? "?"),
      title: String(i.title ?? ""),
      status: String(i.status ?? "received"),
      submittedByClientRef: String(i.submittedByClientRef ?? ""),
    }))
  } catch {
    return []
  }
}

// CL-48: pending approvals/decisions awaiting the owner (approvals.list).
async function fetchApprovalRows(input: { baseUrl: string; token: string; fetchFn: typeof fetch }): Promise<ApprovalRow[]> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "approvals.list" }),
    })
    if (!res.ok) return []
    const json = (await res.json()) as { ok?: unknown; result?: { approvals?: unknown } }
    const rows = json.ok === true ? json.result?.approvals : undefined
    if (!Array.isArray(rows)) return []
    return rows.map((a: any) => ({
      approvalRef: String(a.approvalRef ?? "?"),
      kind: String(a.kind ?? "approval"),
      prompt: String(a.prompt ?? ""),
      createdAt: String(a.createdAt ?? ""),
    }))
  } catch {
    return []
  }
}

// CL-49: read-only MDK wallet status (wallet.status). Null when unavailable.
async function fetchWalletRow(input: { baseUrl: string; token: string; fetchFn: typeof fetch }): Promise<WalletStatusRow | null> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "wallet.status" }),
    })
    if (!res.ok) return null
    const json = (await res.json()) as { ok?: unknown; result?: any }
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

// CL-50: open work-lease assignments (assignments.poll). Read-only.
async function fetchAssignmentRows(input: { baseUrl: string; token: string; fetchFn: typeof fetch }): Promise<AssignmentRow[]> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "assignments.poll" }),
    })
    if (!res.ok) return []
    const json = (await res.json()) as { ok?: unknown; result?: unknown }
    const rows = json.ok === true ? json.result : undefined
    if (!Array.isArray(rows)) return []
    return rows.map((a: any) => ({
      assignmentRef: String(a.assignmentRef ?? "?"),
      leaseRef: String(a.leaseRef ?? "?"),
      goal: String(a.goal ?? ""),
      paymentMode: String(a.paymentMode ?? "unknown"),
      expiresAt: String(a.expiresAt ?? ""),
    }))
  } catch {
    return []
  }
}

// CL-51: node coordinator paused flag (coordinator.status). Null if unexposed.
async function fetchCoordinatorPausedFlag(input: { baseUrl: string; token: string; fetchFn: typeof fetch }): Promise<boolean | null> {
  try {
    const res = await input.fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "coordinator.status" }),
    })
    if (!res.ok) return null
    const json = (await res.json()) as { ok?: unknown; result?: { paused?: unknown } }
    const r = json.ok === true ? json.result : undefined
    return typeof r?.paused === "boolean" ? r.paused : null
  } catch {
    return null
  }
}

export async function fetchAppleFmReadiness(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<AppleFmReadinessResponse> {
  const fetchFn = input.fetchFn ?? fetch
  const fetchedAt = new Date().toISOString()
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "apple_fm.status" }),
    })
    if (!res.ok) {
      return appleFmUnavailableProjection({
        fetchedAt,
        localPylonReady: true,
        blockerRef: "blocker.autopilot.apple_fm.control_request_failed",
        error: `control ${res.status}`,
      })
    }

    const json = (await res.json()) as { ok?: unknown; result?: Record<string, unknown> }
    const result = json.ok === true && json.result && typeof json.result === "object"
      ? json.result
      : null
    if (result === null) {
      return appleFmUnavailableProjection({
        fetchedAt,
        localPylonReady: true,
        blockerRef: "blocker.autopilot.apple_fm.control_response_malformed",
      })
    }

    const blockerRefs = stringArray(result.blockerRefs)
    const advertisedCapabilities = stringArray(result.advertisedCapabilities)
    const capability = requiredString(result.capability, "probe.backend.apple_fm_bridge")
    const available = result.available === true
    const status = requiredString(result.status, available ? "ready" : "unavailable")
    const ok = available &&
      status === "ready" &&
      advertisedCapabilities.includes(capability) &&
      blockerRefs.length === 0

    return {
      ok,
      fetchedAt,
      sourceUrl: "desktop:apple-fm-readiness",
      localPylonReady: true,
      available,
      status,
      backendKind: requiredString(result.backendKind, "apple_fm_bridge"),
      profileId: requiredString(result.profileId, "apple-fm-local"),
      model: requiredString(result.model, "apple-foundation-model"),
      capability,
      advertisedCapabilities,
      baseUrl: requiredString(result.baseUrl, "http://127.0.0.1:11435"),
      platform: optionalString(result.platform),
      version: optionalString(result.version),
      unavailableReason: optionalString(result.unavailableReason),
      message: optionalString(result.message),
      blockerRefs,
    }
  } catch (e) {
    return appleFmUnavailableProjection({
      fetchedAt,
      localPylonReady: true,
      blockerRef: "blocker.autopilot.apple_fm.control_unreachable",
      error: e instanceof Error ? e.message : "unavailable",
    })
  }
}

export async function startAppleFmSession(input: {
  baseUrl: string
  token: string
  prompt: string
  worktreePath: string
  timeoutSeconds?: number
  fetchFn?: typeof fetch
}): Promise<Omit<AppleFmSessionStartResponse, "readiness">> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({
        type: "apple_fm.session.start",
        prompt: input.prompt,
        worktreePath: input.worktreePath,
        ...(input.timeoutSeconds === undefined ? {} : { timeoutSeconds: input.timeoutSeconds }),
      }),
    })
    if (!res.ok) {
      return {
        ok: false,
        sessionRef: "",
        blockerRefs: ["blocker.autopilot.apple_fm.control_request_failed"],
        error: `control ${res.status}`,
      }
    }
    const json = (await res.json()) as {
      ok?: unknown
      result?: {
        ok?: unknown
        sessionRef?: unknown
        blockerRefs?: unknown
        error?: unknown
      }
    }
    const result = json.ok === true && json.result && typeof json.result === "object"
      ? json.result
      : null
    if (result === null) {
      return {
        ok: false,
        sessionRef: "",
        blockerRefs: ["blocker.autopilot.apple_fm.control_response_malformed"],
        error: "malformed control response",
      }
    }
    const ok = result.ok === true
    const blockerRefs = stringArray(result.blockerRefs)
    return {
      ok,
      sessionRef: ok ? requiredString(result.sessionRef, "session.pylon.apple_fm") : "",
      blockerRefs,
      ...(typeof result.error === "string" ? { error: result.error } : {}),
    }
  } catch (e) {
    return {
      ok: false,
      sessionRef: "",
      blockerRefs: ["blocker.autopilot.apple_fm.control_unreachable"],
      error: e instanceof Error ? e.message : "unavailable",
    }
  }
}

export async function fetchNodeState(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{
  ok: boolean
  schema: string
  sessions: SessionSummary[]
  events: Record<string, SessionEventRow[]>
  accounts: AccountRow[]
  artifacts: Record<string, SessionArtifactStats>
  deploy: DesktopDeployStatus
  intents: IntentRow[]
  approvals: ApprovalRow[]
  wallet: WalletStatusRow | null
  assignments: AssignmentRow[]
  coordinatorPaused: boolean | null
}> {
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
  const artifacts: Record<string, SessionArtifactStats> = {}
  for (const session of sessions.slice(0, 25)) {
    events[session.sessionRef] = await fetchSessionEventRows({
      baseUrl,
      token: input.token,
      sessionRef: session.sessionRef,
      fetchFn,
    })
    if (session.state === "completed" || session.state === "failed") {
      const stats = await fetchArtifactStats({
        baseUrl,
        token: input.token,
        sessionRef: session.sessionRef,
        fetchFn,
      })
      if (stats) artifacts[session.sessionRef] = stats
    }
  }

  const accounts = await fetchAccountRows({ baseUrl, token: input.token, fetchFn })
  // CL-26: read-only projection of the node's last deploy.
  const deploy = await fetchDeployStatus({ baseUrl, token: input.token, fetchFn })
  // CL-47..CL-51: parity surfaces — owner asks, approvals, wallet, assignments,
  // and the coordinator paused flag. All read-only; each degrades to empty/null
  // independently so one missing command can't blank the whole projection.
  const [intents, approvals, wallet, assignments, coordinatorPaused] = await Promise.all([
    fetchIntentRows({ baseUrl, token: input.token, fetchFn }),
    fetchApprovalRows({ baseUrl, token: input.token, fetchFn }),
    fetchWalletRow({ baseUrl, token: input.token, fetchFn }),
    fetchAssignmentRows({ baseUrl, token: input.token, fetchFn }),
    fetchCoordinatorPausedFlag({ baseUrl, token: input.token, fetchFn }),
  ])

  return {
    ok: health.ok,
    schema: health.schema,
    sessions,
    events,
    accounts,
    artifacts,
    deploy,
    intents,
    approvals,
    wallet,
    assignments,
    coordinatorPaused,
  }
}

// ── CL-46 mutation verbs (desktop) ─────────────────────────────────────────
// Webview → Bun RPC handlers forward these over loopback /command. The control
// token stays in the Bun process; the webview only ever sees the result shape.

// CL-47: submit an "ask" (work intent) to the node. The coordinator plans and
// fans it out; returns the initial ship-status.
export async function submitIntent(input: {
  baseUrl: string
  token: string
  title: string
  body: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; status: string; error?: string }> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "intent.submit", title: input.title, body: input.body, submittedByClientRef: "desktop" }),
    })
    if (!res.ok) return { ok: false, status: "error", error: `control ${res.status}` }
    const json = (await res.json()) as { ok?: unknown; result?: { status?: unknown } }
    if (json.ok !== true) return { ok: false, status: "error", error: "submit failed" }
    return { ok: true, status: String(json.result?.status ?? "received") }
  } catch (e) {
    return { ok: false, status: "error", error: e instanceof Error ? e.message : "unavailable" }
  }
}

// CL-48: resolve a pending approval. Exactly-once is enforced on the node; a
// duplicate resolve returns {duplicate:true} and keeps the original decision.
export async function resolveApproval(input: {
  baseUrl: string
  token: string
  approvalRef: string
  decision: "approve" | "deny"
  fetchFn?: typeof fetch
}): Promise<{ applied: boolean; duplicate: boolean; decision: string }> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "approvals.resolve", approvalRef: input.approvalRef, decision: input.decision }),
    })
    if (!res.ok) return { applied: false, duplicate: false, decision: input.decision }
    const json = (await res.json()) as { ok?: unknown; result?: { applied?: unknown; duplicate?: unknown; decision?: unknown } }
    const r = json.ok === true ? json.result : undefined
    return {
      applied: r?.applied === true,
      duplicate: r?.duplicate === true,
      decision: typeof r?.decision === "string" ? r.decision : input.decision,
    }
  } catch {
    return { applied: false, duplicate: false, decision: input.decision }
  }
}

// CL-51: pause/resume the node's autonomous coordinator loop.
export async function setCoordinatorPaused(input: {
  baseUrl: string
  token: string
  paused: boolean
  fetchFn?: typeof fetch
}): Promise<{ paused: boolean }> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: input.paused ? "coordinator.pause" : "coordinator.resume" }),
    })
    if (!res.ok) return { paused: input.paused }
    const json = (await res.json()) as { ok?: unknown; result?: { paused?: unknown } }
    const r = json.ok === true ? json.result : undefined
    return { paused: typeof r?.paused === "boolean" ? r.paused : input.paused }
  } catch {
    return { paused: input.paused }
  }
}

// CL-52: cancel a running/queued session. Best-effort; returns the new state.
export async function cancelSession(input: {
  baseUrl: string
  token: string
  sessionRef: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; state: string }> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "session.cancel", sessionRef: input.sessionRef }),
    })
    if (!res.ok) return { ok: false, state: "error" }
    const json = (await res.json()) as { ok?: unknown; result?: { state?: unknown } }
    if (json.ok !== true) return { ok: false, state: "error" }
    return { ok: true, state: String(json.result?.state ?? "cancelled") }
  } catch {
    return { ok: false, state: "error" }
  }
}

// CL-57: directly spawn a bounded session on the node. Validation is the
// caller's responsibility (see validateSpawnRequest in the shared protocol).
export async function spawnSession(input: {
  baseUrl: string
  token: string
  adapter: "codex" | "claude_agent"
  objective: string
  verify?: string[]
  // #4998: requested execution lane (auto|local|cloud-gcp|cloud-shc). Optional;
  // when omitted the node defaults to `auto`.
  lane?: "auto" | "local" | "cloud-gcp" | "cloud-shc"
  timeoutSeconds?: number
  worktreePath?: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; sessionRef: string; error?: string }> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({
        type: "session.spawn",
        adapter: input.adapter,
        objective: input.objective,
        verify:
          input.verify && input.verify.length > 0 ? input.verify : ["true"],
        ...(input.lane ? { lane: input.lane } : {}),
        ...(input.timeoutSeconds ? { timeoutSeconds: input.timeoutSeconds } : {}),
        ...(input.worktreePath ? { worktreePath: input.worktreePath } : {}),
      }),
    })
    if (!res.ok) return { ok: false, sessionRef: "", error: `control ${res.status}` }
    const json = (await res.json()) as { ok?: unknown; result?: { sessionRef?: unknown } }
    if (json.ok !== true) return { ok: false, sessionRef: "", error: "spawn failed" }
    return { ok: true, sessionRef: String(json.result?.sessionRef ?? "spawned") }
  } catch (e) {
    return { ok: false, sessionRef: "", error: e instanceof Error ? e.message : "unavailable" }
  }
}
