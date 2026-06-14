import { existsSync, readFileSync } from "node:fs"
import { join } from "node:path"
import {
  decodeSessionSummary,
  pairBridge,
  createBridgeTransport,
  type BridgeCredential,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import type { AccountRow, SessionArtifactStats, SessionEventRow } from "../shared/rpc"

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

export async function fetchNodeState(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{ ok: boolean; schema: string; sessions: SessionSummary[]; events: Record<string, SessionEventRow[]>; accounts: AccountRow[]; artifacts: Record<string, SessionArtifactStats>; deploy: DesktopDeployStatus }> {
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

  return {
    ok: health.ok,
    schema: health.schema,
    sessions,
    events,
    accounts,
    artifacts,
    deploy,
  }
}
