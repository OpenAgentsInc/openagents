import { existsSync, readFileSync } from "node:fs"
import { readdir, readFile, stat } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import {
  CONTROL_HEALTH_CAPABILITIES,
  decodeSessionSummary,
  pairBridge,
  createBridgeTransport,
  type BridgeCredential,
  type SessionSummary,
} from "@openagentsinc/autopilot-control-protocol"
import {
  reconcilePylonFleet,
  summarizeAssignmentLogTexts,
  type ActiveAssignmentMarker,
  type PresenceSnapshot,
} from "../shared/pylon-fleet-reconciliation.js"
import type {
  AccountRow,
  AppleFmReadinessResponse,
  AppleFmSessionStartResponse,
  ApprovalRow,
  AssignmentRow,
  IntentRow,
  SessionArtifactDetail,
  SessionArtifactStats,
  SessionEventRow,
  WalletStatusRow,
  PylonFleetReconciliation,
} from "../shared/rpc.js"

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
  capabilities?: unknown
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

const optionalObject = (value: unknown): Record<string, unknown> | null =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null

const pylonHomeCandidates = (): readonly string[] => {
  const homes = [
    process.env.PYLON_HOME?.trim(),
    join(homedir(), ".pylon-fable"),
    join(homedir(), ".openagents", "pylon"),
    join(homedir(), ".pylon"),
  ].filter((value): value is string => typeof value === "string" && value.length > 0)
  return [...new Set(homes)]
}

const readJsonRecord = async (path: string): Promise<Record<string, unknown> | null> => {
  try {
    return optionalObject(JSON.parse(await readFile(path, "utf8")))
  } catch {
    return null
  }
}

const readPresenceSnapshots = async (
  homes: readonly string[],
): Promise<PresenceSnapshot[]> => {
  const snapshots: PresenceSnapshot[] = []
  for (const home of homes) {
    const record = await readJsonRecord(join(home, "presence-state.json"))
    if (record === null) continue
    snapshots.push({
      blockerRefs: stringArray(record.blockerRefs),
      lastHeartbeatAt: optionalString(record.lastHeartbeatAt),
      pylonRef: optionalString(record.pylonRef),
    })
  }
  return snapshots
}

const readActiveAssignmentMarkers = async (
  homes: readonly string[],
): Promise<ActiveAssignmentMarker[]> => {
  const markers: ActiveAssignmentMarker[] = []
  for (const home of homes) {
    const dir = join(home, "active-assignment-runs")
    let filenames: string[] = []
    try {
      filenames = await readdir(dir)
    } catch {
      continue
    }
    for (const filename of filenames) {
      if (!filename.endsWith(".json")) continue
      const record = await readJsonRecord(join(dir, filename))
      if (record === null) continue
      const assignmentRef = optionalString(record.assignmentRef)
      const leaseRef = optionalString(record.leaseRef)
      const refreshedAt = optionalString(record.refreshedAt)
      const service = optionalString(record.service)
      if (
        assignmentRef === null ||
        leaseRef === null ||
        refreshedAt === null ||
        service === null
      ) {
        continue
      }
      const accountRefHash = optionalString(record.accountRefHash)
      markers.push({
        ...(accountRefHash === null ? {} : { accountRefHash }),
        assignmentRef,
        leaseRef,
        refreshedAt,
        service,
      })
    }
  }
  return markers
}

const processInventory = async (): Promise<{
  codexExec: number
  khalaRequestWrappers: number
}> => {
  try {
    const proc = Bun.spawn(["ps", "-axo", "pid,ppid,etime,command"], {
      stderr: "pipe",
      stdout: "pipe",
    })
    const [stdout, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      proc.exited,
    ])
    if (exitCode !== 0) return { codexExec: 0, khalaRequestWrappers: 0 }
    const lines = stdout.split(/\r?\n/)
    return {
      codexExec: lines.filter(line => /\bcodex\s+exec\b/.test(line)).length,
      khalaRequestWrappers: lines.filter(line =>
        line.includes("apps/pylon/src/index.ts khala request"),
      ).length,
    }
  } catch {
    return { codexExec: 0, khalaRequestWrappers: 0 }
  }
}

const recentAssignmentLogTexts = async (
  homes: readonly string[],
): Promise<readonly string[]> => {
  const dirs = [
    ...homes.map(home => join(home, "pr-resolver-logs")),
    join(homedir(), ".pylon-fable", "pr-resolver-logs"),
  ]
  const entries: Array<{ path: string; mtimeMs: number }> = []
  for (const dir of [...new Set(dirs)]) {
    let filenames: string[] = []
    try {
      filenames = await readdir(dir)
    } catch {
      continue
    }
    for (const filename of filenames) {
      if (!filename.startsWith("pr-review-") || !filename.endsWith(".log")) continue
      const path = join(dir, filename)
      try {
        const info = await stat(path)
        if (info.isFile()) entries.push({ path, mtimeMs: info.mtimeMs })
      } catch {
        // Ignore files that rotate while the poll is running.
      }
    }
  }
  entries.sort((a, b) => b.mtimeMs - a.mtimeMs)
  const texts: string[] = []
  for (const entry of entries.slice(0, 160)) {
    try {
      texts.push(await readFile(entry.path, "utf8"))
    } catch {
      texts.push("")
    }
  }
  return texts
}

const tokenFailureCount = async (): Promise<number> => {
  const path = join(homedir(), ".pylon-fable", "codex-turn-report-failures.jsonl")
  try {
    const text = await readFile(path, "utf8")
    return text.split(/\r?\n/).filter(line => line.trim() !== "").length
  } catch {
    return 0
  }
}

const availableCodexSlotsFromEnv = (): number | null => {
  const concurrency = Number.parseInt(process.env.OPENAGENTS_PYLON_CODEX_CONCURRENCY ?? "", 10)
  if (!Number.isSafeInteger(concurrency) || concurrency < 0) return null
  const busy = Number.parseInt(process.env.OPENAGENTS_PYLON_CODEX_BUSY ?? "0", 10)
  const queued = Number.parseInt(process.env.OPENAGENTS_PYLON_CODEX_QUEUED ?? "0", 10)
  const load =
    (Number.isSafeInteger(busy) && busy > 0 ? busy : 0) +
    (Number.isSafeInteger(queued) && queued > 0 ? queued : 0)
  return Math.max(0, concurrency - load)
}

export async function fetchPylonFleetReconciliation(): Promise<PylonFleetReconciliation> {
  const fetchedAt = new Date().toISOString()
  const homes = pylonHomeCandidates()
  const [presences, markers, processCounts, logs, failures] = await Promise.all([
    readPresenceSnapshots(homes),
    readActiveAssignmentMarkers(homes),
    processInventory(),
    recentAssignmentLogTexts(homes).then(summarizeAssignmentLogTexts),
    tokenFailureCount(),
  ])
  return reconcilePylonFleet({
    availableCodexSlots: availableCodexSlotsFromEnv(),
    fetchedAt,
    khalaRequestWrappers: processCounts.khalaRequestWrappers,
    liveCodexExecCount: processCounts.codexExec,
    logs,
    markers,
    presences,
    tokenFailureCount: failures,
  })
}

export function controlHealthSupportsDesktop(health: NodeHealth): boolean {
  if (health.ok !== true || typeof health.schema !== "string") return false
  if (!Array.isArray(health.capabilities)) return false
  const capabilities = new Set(
    health.capabilities.filter((capability): capability is string => typeof capability === "string"),
  )
  return CONTROL_HEALTH_CAPABILITIES.every(capability => capabilities.has(capability))
}

export async function probeControlCompatibility(input: {
  baseUrl: string
  fetchFn?: typeof fetch
}): Promise<boolean> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl.replace(/\/+$/, "")}/health`)
    if (!res.ok) return false
    return controlHealthSupportsDesktop((await res.json()) as NodeHealth)
  } catch {
    return false
  }
}

export function readControlToken(pylonHome: string): string | null {
  const tokenPath = join(pylonHome, "control-token")
  if (!existsSync(tokenPath)) return null

  const token = readFileSync(tokenPath, "utf8").trim()
  return token.length > 0 ? token : null
}

// CL-45b: probe a candidate control token against the live control server with
// a cheap authenticated request. A stale token in one candidate home would
// otherwise dead-end auth at `control 401`; this lets the resolver fall through
// to the next candidate home. Returns true when the server ACCEPTS the token
// (any non-401 response — 200, or any other status means "this token
// authenticated and the server is reachable"), false when the server rejects it
// with 401. A network/transport error returns false (treat as unusable here) so
// resolution keeps moving rather than hanging on an unreachable candidate.
// Never logs or returns the token itself.
export async function probeControlToken(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<boolean> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const baseUrl = input.baseUrl.replace(/\/+$/, "")
    if (!(await probeControlCompatibility({ baseUrl, fetchFn }))) return false
    const res = await fetchFn(`${baseUrl}/command`, {
      method: "POST",
      headers: { Authorization: `Bearer ${input.token}`, "content-type": "application/json" },
      body: JSON.stringify({ type: "session.list" }),
    })
    return res.status !== 401
  } catch {
    return false
  }
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
    return events.slice(-100).map((e: any) => ({
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

export function externalSessionRefsFromEventRows(events: readonly SessionEventRow[]): string[] {
  const refs = new Set<string>()
  for (const event of events) {
    const text = `${event.detail}\n${event.full ?? ""}`
    for (const match of text.matchAll(/\bexternal session:\s*(session\.pylon\.[A-Za-z0-9._-]+)/g)) {
      const ref = match[1]?.trim()
      if (ref) refs.add(ref)
    }
  }
  return [...refs]
}

function eventRowsHaveAgentText(events: readonly SessionEventRow[]): boolean {
  return events.some((event) => {
    const text =
      typeof event.full === "string" && event.full.trim() !== ""
        ? event.full
        : event.detail
    return /^agent:\s*\S/is.test(text)
  })
}

const loggedSessionBridgeDiagnostics = new Set<string>()

function logSessionBridgeDiagnostic(input: {
  sessionRef: string
  externalSessionRef: string
  controlEventCount: number
  externalEventCount: number
  externalHasAgentText: boolean
}): void {
  const key = `${input.sessionRef}\0${input.externalSessionRef}`
  if (loggedSessionBridgeDiagnostics.has(key)) return
  loggedSessionBridgeDiagnostics.add(key)
  console.log(
    `[autopilot-desktop] session bridge control=${input.sessionRef} external=${input.externalSessionRef} controlEvents=${input.controlEventCount} externalEvents=${input.externalEventCount} externalAgentText=${input.externalHasAgentText}`,
  )
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
      // CS-A1: surface the account ref/hash/selector/blockers so the per-session
      // picker can thread `accountRef` through spawn and the management UI can
      // key on a stable ref. Priority lives in the node's local config and is
      // surfaced through listManagedAccounts, so it is null on this live
      // readiness projection.
      accountRef:
        typeof a.accountRef === "string" && a.accountRef.length > 0 ? a.accountRef : null,
      accountRefHash: String(
        a.accountRefHash ?? `${String(a.provider ?? "?")}:${String(a.homeState ?? "?")}`,
      ),
      selector: String(a.selector ?? "default_home"),
      blockerRefs: Array.isArray(a.blockerRefs)
        ? a.blockerRefs.map((b: unknown) => String(b))
        : [],
      priority: null,
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
    const artifact = result.artifact && typeof result.artifact === "object" ? result.artifact : undefined
    const ex = artifact && typeof artifact.executor === "object" ? artifact.executor : undefined
    const detail = extractArtifactDetail(artifact)
    return {
      kind: String(result.kind ?? "none"),
      outcome: ex && typeof ex.outcome === "string" ? ex.outcome : null,
      editedFileCount: ex && typeof ex.editedFileCount === "number" ? ex.editedFileCount : null,
      commandCount: ex && typeof ex.commandCount === "number" ? ex.commandCount : null,
      totalTokens: ex && typeof ex.totalTokens === "number" ? ex.totalTokens : null,
      // #5470: surface the redaction-safe, ref-only detail for the artifact &
      // receipt browser. The node already proved this payload public-projection
      // -safe before writing it; we still defensively keep ONLY refs/digests/
      // enums (string|number) and never copy through any free-text/raw field.
      ...(detail !== undefined ? { detail } : {}),
    }
  } catch {
    return null
  }
}

// #5470: pull the public-safe refs out of a retained proof/failure artifact for
// the browser. Every value is read through a string/array guard so a malformed
// or unexpectedly-shaped payload degrades to null/[] rather than leaking through
// raw content. `undefined` when there's no artifact object.
function extractArtifactDetail(artifact: any): SessionArtifactDetail | undefined {
  if (!artifact || typeof artifact !== "object") return undefined
  const str = (value: unknown): string | null => (typeof value === "string" ? value : null)
  const task = artifact.task && typeof artifact.task === "object" ? artifact.task : {}
  const ex = artifact.executor && typeof artifact.executor === "object" ? artifact.executor : {}
  const devCheck = artifact.devCheck && typeof artifact.devCheck === "object" ? artifact.devCheck : {}
  const redaction =
    artifact.redactionScan && typeof artifact.redactionScan === "object" ? artifact.redactionScan : {}
  const deviationRefs = Array.isArray(artifact.deviations)
    ? artifact.deviations.filter((d: unknown): d is string => typeof d === "string")
    : []
  return {
    schema: str(artifact.schema),
    objectiveDigestRef: str(task.objectiveDigestRef),
    verifyRef: str(task.verifyRef),
    responseDigestRef: str(ex.responseDigestRef),
    externalSessionRef: str(ex.externalSessionRef),
    executionPathRef: str(ex.executionPathRef),
    executionMode: str(ex.executionMode),
    sandboxMode: str(ex.sandboxMode),
    permissionMode: str(ex.permissionMode),
    devCheckState: str(devCheck.state),
    deviationRefs,
    redactionState: str(redaction.state),
    errorClass: str(artifact.errorClass),
    errorDigestRef: str(artifact.errorDigestRef),
    workspaceRef: str(artifact.workspaceRef),
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
      // #5467: carry the REAL lifecycle timeline + timestamps from the projection
      // so the autonomous-loop view can show the intent → plan → fanout →
      // reconcile → ship progression honestly. Refs-only — no plaintext body.
      ...(Array.isArray(i.statusHistory)
        ? {
            statusHistory: i.statusHistory
              .filter((e: any) => e && typeof e.status === "string")
              .map((e: any) => ({
                status: String(e.status),
                observedAt: String(e.observedAt ?? ""),
              })),
          }
        : {}),
      ...(typeof i.createdAt === "string" ? { createdAt: i.createdAt } : {}),
      ...(typeof i.updatedAt === "string" ? { updatedAt: i.updatedAt } : {}),
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
      ...(typeof a.sessionRef === "string" ? { sessionRef: a.sessionRef } : {}),
      ...(typeof a.workspaceRef === "string" ? { workspaceRef: a.workspaceRef } : {}),
      ...(typeof a.commandClass === "string" ? { commandClass: a.commandClass } : {}),
      ...(typeof a.accountRefHash === "string" ? { accountRefHash: a.accountRefHash } : {}),
      ...(typeof a.expiresAt === "string" ? { expiresAt: a.expiresAt } : {}),
      ...(typeof a.lane === "string" ? { lane: a.lane } : {}),
      ...(typeof a.source === "string" ? { source: a.source } : {}),
      ...(typeof a.assignmentPath === "string" ? { assignmentPath: a.assignmentPath } : {}),
      ...(typeof a.persistentApprovalSupported === "boolean"
        ? { persistentApprovalSupported: a.persistentApprovalSupported }
        : {}),
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

// AF-1 (#5898): read this node's OWN raw Spark receive address from the local
// control server (`wallet.spark_backup_status` with `showLocalTarget: true`,
// which returns `localTarget` — see apps/pylon/src/index.ts §#5304/#5305). The
// node marks this value LOCAL/PRIVATE: it is PAYMENT MATERIAL. The ONLY allowed
// use is attaching it to the authenticated agent-registration request body so
// tip readiness lands as `spark_address`; it must NEVER be logged, printed,
// surfaced to the webview, persisted in plaintext status, or committed. Returns
// null whenever the wallet is not yet receive-ready, the helper is unavailable,
// or anything fails — the caller then registers without a Spark address
// (unchanged behavior), and tip readiness lands later via the payout-target /
// forum tip-recipient paths.
export async function fetchNodeSparkAddress(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<string | null> {
  const fetchFn = input.fetchFn ?? fetch
  try {
    const res = await fetchFn(`${input.baseUrl}/command`, {
      method: "POST",
      headers: {
        Authorization: `Bearer ${input.token}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        type: "wallet.spark_backup_status",
        showLocalTarget: true,
      }),
    })
    if (!res.ok) return null
    const json = (await res.json()) as { ok?: unknown; result?: any }
    const r = json.ok === true ? json.result : undefined
    if (!r || typeof r !== "object") return null
    const localTarget =
      typeof r.localTarget === "string" ? r.localTarget.trim() : ""
    return localTarget.length > 0 ? localTarget : null
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

// AO-4 (#5445): a lightweight read of just the signals the onboarding wizard
// needs — wallet receive-readiness/balance + open assignment count — without the
// full `fetchNodeState` session/event/artifact sweep. Read-only; fail-soft
// (a failed read returns the dormant snapshot so the wizard never shows fake
// progress). Never returns or logs seeds/tokens/raw addresses.
export async function fetchOnboardingSignals(input: {
  baseUrl: string
  token: string
  fetchFn?: typeof fetch
}): Promise<{
  walletReceiveReady: boolean
  walletBalanceSats: number | null
  openAssignmentCount: number
}> {
  const fetchFn = input.fetchFn ?? fetch
  const [wallet, assignments] = await Promise.all([
    fetchWalletRow({ baseUrl: input.baseUrl, token: input.token, fetchFn }),
    fetchAssignmentRows({ baseUrl: input.baseUrl, token: input.token, fetchFn }),
  ])
  return {
    walletReceiveReady: wallet?.receiveReady === true,
    walletBalanceSats:
      typeof wallet?.balanceSats === "number" ? wallet.balanceSats : null,
    openAssignmentCount: assignments.length,
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
  pylonFleet: PylonFleetReconciliation
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
  if (!controlHealthSupportsDesktop(health)) {
    throw new Error("Pylon control server is missing required desktop capabilities")
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
    for (const externalSessionRef of externalSessionRefsFromEventRows(events[session.sessionRef] ?? [])) {
      if (events[externalSessionRef] !== undefined) continue
      const externalEvents = await fetchSessionEventRows({
        baseUrl,
        token: input.token,
        sessionRef: externalSessionRef,
        fetchFn,
      })
      if (externalEvents.length > 0) events[externalSessionRef] = externalEvents
    }
    if (session.state === "completed" || session.state === "failed") {
      const stats = await fetchArtifactStats({
        baseUrl,
        token: input.token,
        sessionRef: session.sessionRef,
        fetchFn,
      })
      if (stats) {
        artifacts[session.sessionRef] = stats
        const externalSessionRef = stats.detail?.externalSessionRef
        if (externalSessionRef && events[externalSessionRef] === undefined) {
          const externalEvents = await fetchSessionEventRows({
            baseUrl,
            token: input.token,
            sessionRef: externalSessionRef,
            fetchFn,
          })
          if (externalEvents.length > 0) events[externalSessionRef] = externalEvents
          if (!eventRowsHaveAgentText(events[session.sessionRef] ?? [])) {
            logSessionBridgeDiagnostic({
              sessionRef: session.sessionRef,
              externalSessionRef,
              controlEventCount: events[session.sessionRef]?.length ?? 0,
              externalEventCount: externalEvents.length,
              externalHasAgentText: eventRowsHaveAgentText(externalEvents),
            })
          }
        }
      }
    }
  }

  const accounts = await fetchAccountRows({ baseUrl, token: input.token, fetchFn })
  // CL-26: read-only projection of the node's last deploy.
  const deploy = await fetchDeployStatus({ baseUrl, token: input.token, fetchFn })
  // CL-47..CL-51: parity surfaces — owner asks, approvals, wallet, assignments,
  // and the coordinator paused flag. All read-only; each degrades to empty/null
  // independently so one missing command can't blank the whole projection.
  const [intents, approvals, wallet, assignments, coordinatorPaused, pylonFleet] = await Promise.all([
    fetchIntentRows({ baseUrl, token: input.token, fetchFn }),
    fetchApprovalRows({ baseUrl, token: input.token, fetchFn }),
    fetchWalletRow({ baseUrl, token: input.token, fetchFn }),
    fetchAssignmentRows({ baseUrl, token: input.token, fetchFn }),
    fetchCoordinatorPausedFlag({ baseUrl, token: input.token, fetchFn }),
    fetchPylonFleetReconciliation(),
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
    pylonFleet,
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
  // #5471: managed-worktree selector. Mutually exclusive with `worktreePath`
  // (the node rejects both). When present, the node's workspace-materializer
  // checks out `commitSha` for a fresh isolated worktree. Resolved node-side by
  // `resolveManagedWorktreeRepoRef` so the webview never has to run git.
  repoRef?: ManagedWorktreeRepoRef
  // CS-A1: run under a specific provider account (resolved against the node's
  // registry). Omitted means the node's default account selection.
  accountRef?: string
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
        // #5471: a managed worktree (repoRef) and an existing worktree path are
        // mutually exclusive; prefer repoRef when both are somehow set.
        ...(input.repoRef
          ? { repoRef: input.repoRef }
          : input.worktreePath
            ? { worktreePath: input.worktreePath }
            : {}),
        ...(input.accountRef ? { accountRef: input.accountRef } : {}),
      }),
    })
    // #5453: prefer the node's typed error body over a bare `control <status>`.
    // The control server answers a malformed command with HTTP 400 and a clean
    // `{ ok:false, error, reason }`; surface that message so the composer shows
    // an honest reason instead of an opaque `control 500`/`control 400`.
    if (!res.ok) {
      const detail = (await res
        .json()
        .catch(() => null)) as { error?: unknown; reason?: unknown } | null
      const message =
        detail && typeof detail.error === "string" && detail.error.trim() !== ""
          ? detail.error
          : `control ${res.status}`
      return { ok: false, sessionRef: "", error: message }
    }
    const json = (await res.json()) as { ok?: unknown; result?: { sessionRef?: unknown } }
    if (json.ok !== true) return { ok: false, sessionRef: "", error: "spawn failed" }
    return { ok: true, sessionRef: String(json.result?.sessionRef ?? "spawned") }
  } catch (e) {
    return { ok: false, sessionRef: "", error: e instanceof Error ? e.message : "unavailable" }
  }
}

// #5471: managed-worktree repository ref. Exactly the shape Pylon's
// `repositoryRefFrom` (apps/pylon/src/node/control-sessions.ts) accepts on
// session.spawn, so what the desktop sends is what the node will check out.
export type ManagedWorktreeRepoRef = {
  provider: "github"
  visibility: "public"
  fullName: string
  branch: string
  commitSha: string
}

export type ResolveManagedWorktreeResult =
  | { ok: true; repoRef: ManagedWorktreeRepoRef }
  | { ok: false; error: string }

const GITHUB_FULL_NAME = /^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/
const SAFE_REF = /^[A-Za-z0-9_./-]+$/

// #5471: resolve a managed-worktree request (GitHub `owner/name` + base ref) to
// a concrete repoRef the node can materialize. We resolve the 40-char commit
// SHA with `git ls-remote` against the public GitHub remote — the same kind of
// resolution the Pylon CLI's `--managed-worktree` does locally, but without
// requiring a local clone, so the desktop can target any public repo/ref.
// Bun owns this (the webview never runs git); the webview only sends the typed
// request and receives a public-safe repoRef.
export async function resolveManagedWorktreeRepoRef(input: {
  fullName: string
  baseRef: string
  branch: string
  gitRunner?: (args: string[]) => Promise<{ ok: boolean; stdout: string; error?: string }>
}): Promise<ResolveManagedWorktreeResult> {
  if (!GITHUB_FULL_NAME.test(input.fullName)) {
    return { ok: false, error: "repo must be a GitHub owner/name" }
  }
  if (
    !SAFE_REF.test(input.baseRef) ||
    input.baseRef.includes("..") ||
    input.baseRef.startsWith("-")
  ) {
    return { ok: false, error: "base ref is invalid" }
  }
  // `git ls-remote` takes a remote ref, not the `origin/`-prefixed local form;
  // strip the prefix (the branch field already carries the stripped name).
  const remoteRef = input.baseRef.replace(/^origin\//, "")
  const runner = input.gitRunner ?? defaultGitRunner
  const run = await runner([
    "ls-remote",
    `https://github.com/${input.fullName}.git`,
    remoteRef,
  ])
  if (!run.ok) {
    return { ok: false, error: run.error ?? "git ls-remote failed" }
  }
  // ls-remote prints `<sha>\t<ref>` lines; the first column of the first line is
  // the resolved commit. An empty result means the ref does not exist remotely.
  const firstLine = run.stdout.split("\n").map((l) => l.trim()).find((l) => l !== "")
  const commitSha = firstLine?.split(/\s+/)[0] ?? ""
  if (!/^[a-f0-9]{40}$/i.test(commitSha)) {
    return { ok: false, error: `base ref '${input.baseRef}' did not resolve to a commit` }
  }
  return {
    ok: true,
    repoRef: {
      provider: "github",
      visibility: "public",
      fullName: input.fullName,
      branch: input.branch,
      commitSha,
    },
  }
}

async function defaultGitRunner(
  args: string[],
): Promise<{ ok: boolean; stdout: string; error?: string }> {
  try {
    const proc = Bun.spawn(["git", ...args], { stderr: "pipe", stdout: "pipe" })
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ])
    if (exitCode !== 0) {
      return { ok: false, stdout: "", error: `git failed: ${stderr.trim()}` }
    }
    return { ok: true, stdout }
  } catch (e) {
    return { ok: false, stdout: "", error: e instanceof Error ? e.message : "git unavailable" }
  }
}
