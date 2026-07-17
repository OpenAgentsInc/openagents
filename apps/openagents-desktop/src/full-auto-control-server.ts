import { createHash, randomBytes, timingSafeEqual } from "node:crypto"
import { chmodSync, mkdirSync, rmSync, writeFileSync } from "node:fs"
import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http"
import path from "node:path"

import {
  evaluateEnvironmentScopeExchange,
  type EnvironmentCapabilityScope,
} from "@openagentsinc/environment-auth"

import {
  FULL_AUTO_CONTROL_CALLER,
  FULL_AUTO_CONTROL_ENV_FLAG,
  FULL_AUTO_CONTROL_SCHEMA,
  FULL_AUTO_CONTROL_TURNS_LIMIT,
  decodeFullAutoControlEnableRequest,
  decodeFullAutoControlRunHandoffRequest,
  decodeFullAutoControlRunRef,
  decodeFullAutoControlRunStartRequest,
  decodeFullAutoControlStartRequest,
  decodeFullAutoControlThreadRef,
  type FullAutoControlError,
  type FullAutoControlLive,
  type FullAutoControlRecord,
  type FullAutoControlRun,
  type FullAutoControlTurn,
} from "./full-auto-control-contract.ts"
import { fullAutoControlOpenApiDocument } from "./full-auto-control-openapi.ts"
import type { FullAutoRecord, FullAutoRegistry } from "./full-auto-registry.ts"
import type { LocalTurnRecord } from "./local-turn-journal.ts"
import { FULL_AUTO_DEFAULT_LANE } from "./full-auto-lane.ts"
import {
  retryFullAutoRunNow,
  settleFullAutoRunLiveness,
  type FullAutoLivenessProjection,
} from "./full-auto-liveness.ts"
import {
  type FullAutoRun,
  type FullAutoRunRegistry,
  type FullAutoRunThreadSnapshot,
} from "./full-auto-run-registry.ts"
import type { DesktopThread } from "./chat-contract.ts"
import type { ProviderLaneRegistry, ProviderLaneRegistryEntry } from "./provider-lane-registry.ts"
import {
  buildProviderHandoffEnvelope,
  providerHandoffDispositionForEnvelope,
  type ProviderHandoffRegistry,
} from "./full-auto-provider-handoff.ts"

/**
 * FA-H13 (#8886): the Phase 1 local Full Auto control server. A plain
 * node:http loopback server (127.0.0.1 only, ephemeral or env-pinned port)
 * that Desktop main starts ONLY when OPENAGENTS_DESKTOP_FULL_AUTO_CONTROL=1,
 * exposing the six control operations plus the OpenAPI document that
 * describes them. The MCP server and CLI under scripts/ are deliberately thin
 * pass-through clients of this one surface.
 *
 * Auth follows the Harness MCP pilot pattern exactly
 * (apps/pylon/src/harness-mcp-server.ts): one per-process scoped bearer
 * credential minted at startup, scopes drawn from and validated by
 * `@openagentsinc/environment-auth`'s narrowing-only exchange (no new scope
 * vocabulary, no new auth framework), verified with a constant-time
 * comparison on every request. The connection info (url + token) is written
 * mode-0600 to full-auto/control.json under Electron userData so a local
 * agent can discover it; the file is removed on stop.
 *
 * Authority boundaries (issue #8886 hard requirements):
 * - enable NAMES the workspace it expects and the server refuses (409,
 *   registry untouched) when it does not match the current resolution --
 *   never a redirect, never a new grant;
 * - continue-now invokes the exact same serialized reconciliation trigger
 *   main already uses (a new TRIGGER, not a new dispatch mechanism);
 * - every mutation appends a durable, distinctly-attributed system note so
 *   the owner can always tell a programmatic action from their own click.
 */
export const FULL_AUTO_CONTROL_SCOPES = [
  "operator_read",
  "coding_session_control",
] as const satisfies ReadonlyArray<EnvironmentCapabilityScope>

/** The opt-in gate main checks before constructing the server at all. */
export const isFullAutoControlEnabled = (
  env: Readonly<Record<string, string | undefined>>,
): boolean => env[FULL_AUTO_CONTROL_ENV_FLAG] === "1"

export type FullAutoControlCredential = Readonly<{
  /** The secret. Written only to the mode-0600 control file; never logged. */
  token: string
  scopes: ReadonlyArray<EnvironmentCapabilityScope>
  issuedAtIso: string
}>

/**
 * Mints the per-process scoped credential. Scope evaluation reuses the ENV-2
 * narrowing-only exchange exactly like the Harness MCP pilot: requesting any
 * scope outside FULL_AUTO_CONTROL_SCOPES rejects the whole mint, so a wiring
 * bug can never hand a caller a wider credential than this surface allows.
 */
export const mintFullAutoControlCredential = (input?: Readonly<{
  now?: Date
  requestedScopes?: ReadonlyArray<EnvironmentCapabilityScope>
}>): FullAutoControlCredential => {
  const decision = evaluateEnvironmentScopeExchange({
    subjectScopes: FULL_AUTO_CONTROL_SCOPES,
    requestedScopes: input?.requestedScopes ?? [],
  })
  if (!decision.ok) {
    throw new Error(
      `full auto control credential mint rejected (${decision.reason}): ${decision.offendingScopes.join(", ")}`,
    )
  }
  return {
    token: `oafa_${randomBytes(32).toString("base64url")}`,
    scopes: decision.grantedScopes,
    issuedAtIso: (input?.now ?? new Date()).toISOString(),
  }
}

/** Constant-time bearer verification (same sha256 + timingSafeEqual shape as
 * verifyHarnessMcpCredential). */
export const verifyFullAutoControlToken = (
  expectedToken: string,
  presentedToken: string | undefined,
): boolean => {
  const presented = presentedToken?.trim() ?? ""
  if (presented.length === 0) return false
  const expected = createHash("sha256").update(expectedToken).digest()
  const actual = createHash("sha256").update(presented).digest()
  return timingSafeEqual(expected, actual)
}

/**
 * The narrow capability set main hands the server -- an options object, not
 * an import of main's internals, so the module is testable with fakes. Every
 * capability maps 1:1 onto something the IPC handlers already do.
 */
export type FullAutoControlCapabilities = Readonly<{
  registry: Pick<FullAutoRegistry, "list" | "record" | "set">
  /** FA-H2: the SAME workspace resolution codex-local turns execute against. */
  resolveWorkspaceRef: () => string
  /** FA-H3: the SAME serialized reconciliation trigger every other Full Auto
   * trigger point uses (main passes runFullAutoReconciliation itself). */
  triggerReconciliation: () => Promise<void>
  /** FA-H4: the coarse live state for one thread, or null when idle/unknown. */
  liveState: (threadRef: string) => FullAutoControlLive | null
  /** Local-turn journal records for one thread (the server bounds/projects). */
  listTurns: (threadRef: string) => ReadonlyArray<LocalTurnRecord>
  /** Durable owner-visible receipt on the thread (appendFullAutoSystemNote). */
  appendSystemNote: (threadRef: string, text: string) => void
  /** start bootstrap: mint a brand-new local thread in main's own thread
   * store (main mints the ref -- callers never name one) and return its ref. */
  createThread: (title: string | null, laneRef: string) => string
  /** L6: capability-gated ProviderLane selection. */
  isLaneEligible?: (laneRef: string) => boolean
  /** L8: public-safe lane registry. Includes unavailable/unadmitted lanes. */
  listLanes?: () => Promise<ReadonlyArray<ProviderLaneRegistryEntry>>
  /** FA-RUN-01 (#8969): the durable FullAutoRun objective/lifecycle store.
   * Every run-level route below operates exclusively through this registry's
   * own typed transition function -- the server never writes `state`
   * directly. */
  runRegistry: FullAutoRunRegistry
  /** FA-AC-44 Pause: best-effort request to interrupt the thread's actively
   * running turn (the exact same three-way codexLocal/fableLocal/ACP-driver
   * interrupt chain the existing CodexLocalFullAutoInterruptChannel IPC
   * handler already uses). Returns false when nothing was running or no
   * lane accepted the interrupt; Pause still transitions to Pausing and
   * waits for the turn to resolve either way. */
  interruptLiveTurn?: (threadRef: string) => boolean
  /** FA-HO-01 (#8975): the same admission/auth/capability re-check the
   * existing interactive manual-switch IPC handler already uses. Absent
   * means the handoff route refuses cleanly rather than mutating anything. */
  providerLaneRegistry?: Pick<ProviderLaneRegistry, "switchThread">
  /** The Desktop thread bound to a run, for the envelope's bounded-history
   * projection. Absent threadRef or a null return both project an empty,
   * explicitly-omitted context rather than fabricating history. */
  getThread?: (threadRef: string) => DesktopThread | null
  /** FA-HO-01 (#8975): the durable receipt store every handoff appends to,
   * independent of restart. Absent means the handoff route refuses cleanly
   * (never a silent, unreceipted switch). */
  providerHandoffRegistry?: ProviderHandoffRegistry
}>

export type StartFullAutoControlServerInput = Readonly<{
  capabilities: FullAutoControlCapabilities
  /** Absolute path of the connection-info file (…/full-auto/control.json). */
  controlFilePath: string
  /** Explicit loopback port; omitted means ephemeral. */
  port?: number
  now?: () => Date
}>

export type FullAutoControlServer = Readonly<{
  /** Loopback origin, e.g. `http://127.0.0.1:49321`. */
  url: string
  credential: FullAutoControlCredential
  /** Opaque identity minted once for this server lifetime and echoed by
   * authenticated list/status responses for exact process ownership checks. */
  instanceId: string
  controlFilePath: string
  stop: () => Promise<void>
}>

const MAX_BODY_BYTES = 64 * 1024

const projectRecord = (
  record: FullAutoRecord,
  live: FullAutoControlLive | null,
): FullAutoControlRecord => ({
  threadRef: record.threadRef,
  enabled: record.enabled,
  continuationCount: record.continuationCount,
  updatedAt: record.updatedAt,
  workspaceRef: record.workspaceRef ?? null,
  lane: record.profile?.lane ?? FULL_AUTO_DEFAULT_LANE,
  // Public-safe projection: never raw profile material beyond the accountRef.
  accountRef: record.profile?.accountRef ?? null,
  blockedReason: record.blockedReason ?? null,
  disabledBy: record.disabledBy ?? null,
  disabledAt: record.disabledAt ?? null,
  live: live ?? { state: "idle", turnRef: null },
})

const projectRun = (run: FullAutoRun, projection: FullAutoLivenessProjection): FullAutoControlRun => ({
  runRef: run.runRef,
  threadRef: run.threadRef ?? null,
  title: run.title,
  objective: run.objective,
  objectiveSource: run.objectiveSource,
  doneCondition: run.doneCondition,
  workspaceRef: run.workspaceRef ?? null,
  lane: run.profile?.lane ?? null,
  turnCap: run.turnCap,
  successfulAttempts: run.successfulAttempts,
  failedAttempts: run.failedAttempts,
  state: run.state,
  stateRevision: run.stateRevision,
  terminalReason: run.terminalReason ?? null,
  predecessorRunRef: run.predecessorRunRef ?? null,
  migratedFrom: run.migratedFrom ?? null,
  createdAt: run.createdAt,
  startedAt: run.startedAt ?? null,
  lastProgressAt: run.lastProgressAt ?? null,
  pausedAt: run.pausedAt ?? null,
  stoppedAt: run.stoppedAt ?? null,
  completedAt: run.completedAt ?? null,
  transitions: run.transitions,
  stallCause: projection.cause,
  nextRetryAt: projection.nextRetryAt,
  recoveryAction: projection.recoveryAction,
})

const threadSnapshot = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
): FullAutoRunThreadSnapshot => ({
  threadRecord: run.threadRef === undefined ? null : capabilities.registry.record(run.threadRef),
  turnRunning: run.threadRef !== undefined && capabilities.liveState(run.threadRef)?.state === "turn_running",
})

/** FA-RUN-03 (#8971): settles a run against the current thread-level truth
 * (Pausing->Paused, cap/failure/orphan sync -- `settleFullAutoRunFromThreadState`'s
 * exact rules) AND the liveness/stall classifier before projecting it, so
 * every GET/mutation response and the sidebar/control-API AC ("Sidebar/run
 * view and control API return the same typed state and retry deadline")
 * always agree with the persisted state. */
const settleRun = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): Readonly<{ run: FullAutoRun; projection: FullAutoLivenessProjection }> =>
  settleFullAutoRunLiveness(capabilities.runRegistry, run, threadSnapshot(capabilities, run), now)

/** Settle + project in one call -- every response on the run-level surface
 * routes through this so GET and every mutation agree on the same typed
 * state, cause, and retry deadline (see `settleRun`'s doc comment). */
const projectSettled = (
  capabilities: FullAutoControlCapabilities,
  run: FullAutoRun,
  now: () => Date,
): FullAutoControlRun => {
  const { run: settled, projection } = settleRun(capabilities, run, now)
  return projectRun(settled, projection)
}

const projectTurns = (records: ReadonlyArray<LocalTurnRecord>): ReadonlyArray<FullAutoControlTurn> =>
  [...records]
    .filter(record => record.turnRef.startsWith("turn.full-auto."))
    .sort((left, right) => right.updatedAt.localeCompare(left.updatedAt))
    .slice(0, FULL_AUTO_CONTROL_TURNS_LIMIT)
    // Identity, phase, disposition, timestamps only -- never transcript text.
    .map(record => ({
      turnRef: record.turnRef,
      phase: record.phase,
      disposition: record.disposition,
      createdAt: record.createdAt,
      updatedAt: record.updatedAt,
    }))

const sendJson = (response: ServerResponse, status: number, payload: unknown): void => {
  const body = JSON.stringify(payload)
  response.writeHead(status, {
    "cache-control": "no-store",
    "content-length": Buffer.byteLength(body),
    "content-type": "application/json",
    ...(status === 401 ? { "www-authenticate": "Bearer" } : {}),
  })
  response.end(body)
}

const sendError = (
  response: ServerResponse,
  status: number,
  error: FullAutoControlError,
): void => sendJson(response, status, error)

const readJsonBody = (request: IncomingMessage): Promise<unknown | null> =>
  new Promise(resolve => {
    let size = 0
    const chunks: Array<Buffer> = []
    request.on("data", (chunk: Buffer) => {
      size += chunk.length
      if (size > MAX_BODY_BYTES) {
        request.destroy()
        resolve(null)
        return
      }
      chunks.push(chunk)
    })
    request.on("end", () => {
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString("utf8")))
      } catch {
        resolve(null)
      }
    })
    request.on("error", () => resolve(null))
  })

const writeControlFile = (
  controlFilePath: string,
  value: Readonly<{ url: string; credential: FullAutoControlCredential; instanceId: string }>,
): void => {
  const parent = path.dirname(controlFilePath)
  mkdirSync(parent, { recursive: true, mode: 0o700 })
  if (process.platform !== "win32") chmodSync(parent, 0o700)
  writeFileSync(
    controlFilePath,
    `${JSON.stringify({
      schema: FULL_AUTO_CONTROL_SCHEMA,
      url: value.url,
      token: value.credential.token,
      scopes: value.credential.scopes,
      issuedAtIso: value.credential.issuedAtIso,
      // #8928: cleanup may target only this exact PID after an authenticated
      // response echoes the same opaque serverInstanceId.
      pid: process.pid,
      serverInstanceId: value.instanceId,
    })}\n`,
    { encoding: "utf8", mode: 0o600 },
  )
  if (process.platform !== "win32") chmodSync(controlFilePath, 0o600)
}

/** Public-safe audit line -- refs and outcomes only, never the token. */
const auditLog = (operation: string, threadRef: string, outcome: string): void => {
  console.log(
    `[openagents-desktop full-auto-control] ${operation} threadRef=${threadRef} ${outcome}`,
  )
}

export const startFullAutoControlServer = (
  input: StartFullAutoControlServerInput,
): Promise<FullAutoControlServer> => {
  const now = input.now ?? (() => new Date())
  const credential = mintFullAutoControlCredential({ now: now() })
  const instanceId = `oafa_instance_${randomBytes(24).toString("base64url")}`
  const capabilities = input.capabilities

  const handle = async (
    request: IncomingMessage,
    response: ServerResponse,
  ): Promise<void> => {
    // Bearer gate on EVERY request, the OpenAPI document included.
    const authorization = request.headers.authorization ?? ""
    const presentedToken = authorization.startsWith("Bearer ")
      ? authorization.slice("Bearer ".length)
      : undefined
    if (!verifyFullAutoControlToken(credential.token, presentedToken)) {
      sendError(response, 401, {
        error: "unauthorized",
        message: "A valid Full Auto control bearer credential is required.",
      })
      return
    }
    const url = new URL(request.url ?? "/", "http://127.0.0.1")
    const method = (request.method ?? "GET").toLowerCase()

    if (url.pathname === "/v1/openapi.json") {
      if (method !== "get") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
        return
      }
      sendJson(response, 200, fullAutoControlOpenApiDocument)
      return
    }
    if (url.pathname === "/v1/lanes") {
      if (method !== "get") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
        return
      }
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        lanes: await (capabilities.listLanes?.() ?? Promise.resolve([])),
      })
      return
    }
    if (url.pathname === "/v1/full-auto") {
      if (method !== "get") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
        return
      }
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        serverInstanceId: instanceId,
        records: capabilities.registry.list().map(record =>
          projectRecord(record, capabilities.liveState(record.threadRef))),
      })
      return
    }

    if (url.pathname === "/v1/full-auto/start") {
      if (method !== "post") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use POST." })
        return
      }
      const body = decodeFullAutoControlStartRequest(await readJsonBody(request))
      if (body === null) {
        sendError(response, 400, {
          error: "invalid_request",
          message:
            "start requires a JSON body naming the expected workspace: { workspaceRef, title? }.",
        })
        return
      }
      // Same fail-closed rule as enable: name the workspace or nothing
      // happens. On mismatch NO thread is created and NO record is written.
      const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
      if (body.workspaceRef !== resolvedWorkspaceRef) {
        auditLog("start", "-", "refused workspace_mismatch")
        sendError(response, 409, {
          error: "workspace_mismatch",
          message:
            "The named workspace does not match the currently resolved workspace; no thread was " +
            "created and Full Auto was NOT started. Start is a refusal on mismatch, never a " +
            "redirect or a new grant.",
          expectedWorkspaceRef: body.workspaceRef,
          resolvedWorkspaceRef,
        })
        return
      }
      const lane = body.lane ?? FULL_AUTO_DEFAULT_LANE
      if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
        sendError(response, 409, {
          error: "lane_not_eligible",
          message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
        })
        return
      }
      // Bootstrap: main mints the thread, the same registry.set path as the
      // composer toggle binds workspace + enables, and the shared serialized
      // reconcile pass dispatches the first continuation.
      const startedThreadRef = capabilities.createThread(body.title ?? null, lane)
      const record = capabilities.registry.set(startedThreadRef, true, {
        workspaceRef: resolvedWorkspaceRef,
        profile: { lane },
      })
      capabilities.appendSystemNote(
        startedThreadRef,
        `Full Auto started programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
      )
      void capabilities.triggerReconciliation().catch(() => {})
      auditLog("start", startedThreadRef, "ok")
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        ok: true,
        record: projectRecord(record, capabilities.liveState(startedThreadRef)),
      })
      return
    }

    // FA-RUN-01 (#8969): the run-level lifecycle surface. Matched BEFORE the
    // generic thread-ref regex below so "runs" is never mistaken for a
    // threadRef, exactly like /v1/full-auto/start is special-cased above.
    if (url.pathname === "/v1/full-auto/runs") {
      if (method !== "get") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
        return
      }
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        serverInstanceId: instanceId,
        runs: capabilities.runRegistry.list().map(run => projectSettled(capabilities, run, now)),
      })
      return
    }

    if (url.pathname === "/v1/full-auto/runs/start") {
      if (method !== "post") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use POST." })
        return
      }
      const body = decodeFullAutoControlRunStartRequest(await readJsonBody(request))
      if (body === null) {
        sendError(response, 400, {
          error: "invalid_request",
          message:
            "runs/start requires a JSON body: { workspaceRef, title, objective, doneCondition, lane?, turnCap? }.",
        })
        return
      }
      const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
      if (body.workspaceRef !== resolvedWorkspaceRef) {
        auditLog("runs/start", "-", "refused workspace_mismatch")
        sendError(response, 409, {
          error: "workspace_mismatch",
          message:
            "The named workspace does not match the currently resolved workspace; no run was started.",
          expectedWorkspaceRef: body.workspaceRef,
          resolvedWorkspaceRef,
        })
        return
      }
      const lane = body.lane ?? FULL_AUTO_DEFAULT_LANE
      if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
        sendError(response, 409, {
          error: "lane_not_eligible",
          message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
        })
        return
      }
      // FA-AC-39: check BEFORE minting anything -- a refusal must leave no
      // side effect behind, never a half-started thread.
      const existingActive = capabilities.runRegistry.activeRun()
      if (existingActive !== null) {
        auditLog("runs/start", "-", `refused active_run_conflict runRef=${existingActive.runRef}`)
        sendError(response, 409, {
          error: "active_run_conflict",
          message: "A Full Auto run is already active for this Desktop profile.",
          activeRunRef: existingActive.runRef,
        })
        return
      }
      const startedThreadRef = capabilities.createThread(body.title, lane)
      capabilities.registry.set(startedThreadRef, true, { workspaceRef: resolvedWorkspaceRef, profile: { lane } })
      const result = capabilities.runRegistry.startNew({
        title: body.title,
        objective: body.objective,
        doneCondition: body.doneCondition,
        objectiveSource: "control_caller",
        workspaceRef: resolvedWorkspaceRef,
        profile: { lane },
        ...(body.turnCap === undefined ? {} : { turnCap: body.turnCap }),
        threadRef: startedThreadRef,
        actor: "control_api",
        reason: `started via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER})`,
      })
      if (!result.ok) {
        // Genuinely unexpected given the pre-check above (Node is
        // single-threaded and nothing awaits between it and here), but never
        // silently drop the minted thread's Full Auto grant if this ever
        // fires -- report the conflict honestly.
        auditLog("runs/start", startedThreadRef, `unexpected refusal: ${result.reason}`)
        sendError(response, 409, {
          error: result.reason === "active_run_conflict" ? "active_run_conflict" : "invalid_request",
          message: "A Full Auto run could not be started.",
          ...(result.reason === "active_run_conflict" ? { activeRunRef: result.activeRunRef } : {}),
        })
        return
      }
      capabilities.appendSystemNote(
        startedThreadRef,
        `Full Auto run started programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
      )
      void capabilities.triggerReconciliation().catch(() => {})
      auditLog("runs/start", startedThreadRef, `ok runRef=${result.run.runRef}`)
      sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, ok: true, run: projectSettled(capabilities, result.run, now) })
      return
    }

    const runMatch = /^\/v1\/full-auto\/runs\/([^/]+)(?:\/(pause|resume|stop|handoff|retry-now))?$/.exec(url.pathname)
    if (runMatch !== null) {
      const runRef = decodeFullAutoControlRunRef(decodeURIComponent(runMatch[1]!))
      if (runRef === null) {
        sendError(response, 400, { error: "invalid_request", message: "runRef must be a 1-180 character string." })
        return
      }
      const runAction = runMatch[2] ?? null

      if (runAction === null) {
        if (method !== "get") {
          sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
          return
        }
        const run = capabilities.runRegistry.get(runRef)
        if (run === null) {
          sendError(response, 404, { error: "not_found", message: "No Full Auto run exists for that runRef." })
          return
        }
        sendJson(response, 200, {
          schema: FULL_AUTO_CONTROL_SCHEMA,
          serverInstanceId: instanceId,
          run: projectSettled(capabilities, run, now),
        })
        return
      }

      if (method !== "post") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use POST." })
        return
      }
      const run = capabilities.runRegistry.get(runRef)
      if (run === null) {
        sendError(response, 404, { error: "not_found", message: "No Full Auto run exists for that runRef." })
        return
      }

      if (runAction === "pause") {
        const turnRunning = run.threadRef !== undefined
          && capabilities.liveState(run.threadRef)?.state === "turn_running"
        const to = turnRunning ? "pausing" : "paused"
        const result = capabilities.runRegistry.transition(runRef, {
          to,
          actor: "control_api",
          reason: `Pause requested via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
        })
        if (!result.ok) {
          if (result.reason === "not_found") {
            sendError(response, 404, { error: "not_found", message: "No Full Auto run exists for that runRef." })
            return
          }
          sendError(response, 409, {
            error: "illegal_transition",
            message: `Pause is not legal from state ${result.from}.`,
            fromState: result.from,
            toState: result.to,
          })
          return
        }
        // Pause immediately prevents any new dispatch, whether or not a
        // turn is currently in flight -- disable the thread-level gate right
        // now rather than waiting for the turn to resolve.
        if (run.threadRef !== undefined) {
          capabilities.registry.set(run.threadRef, false, { disabledBy: "control_api" })
          if (turnRunning) capabilities.interruptLiveTurn?.(run.threadRef)
          capabilities.appendSystemNote(
            run.threadRef,
            `Full Auto run paused programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
          )
        }
        auditLog("runs/pause", run.threadRef ?? runRef, `ok state=${result.run.state}`)
        sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, ok: true, run: projectSettled(capabilities, result.run, now) })
        return
      }

      if (runAction === "resume") {
        if (run.state !== "paused") {
          sendError(response, 409, {
            error: "illegal_transition",
            message: `Resume is legal only from paused (current state: ${run.state}).`,
            fromState: run.state,
            toState: "running",
          })
          return
        }
        // FA-AC-44: revalidate workspace admission before dispatching again
        // -- the same fail-closed rule Start/Enable already enforce. A
        // mismatch is a REFUSAL, never a redirect or a silent state change:
        // the run stays exactly Paused (matching the enable/start pattern of
        // leaving the registry untouched on mismatch) so the owner can Stop
        // it or Resume again once the expected workspace is available.
        const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
        if (run.workspaceRef !== undefined && run.workspaceRef !== resolvedWorkspaceRef) {
          sendError(response, 409, {
            error: "workspace_mismatch",
            message: "The run's granted workspace no longer matches the currently resolved workspace; Resume refused and the run remains Paused.",
            expectedWorkspaceRef: run.workspaceRef,
            resolvedWorkspaceRef,
          })
          return
        }
        const lane = run.profile?.lane ?? FULL_AUTO_DEFAULT_LANE
        if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
          sendError(response, 409, {
            error: "lane_not_eligible",
            message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
          })
          return
        }
        const result = capabilities.runRegistry.transition(runRef, {
          to: "running",
          actor: "control_api",
          reason: `Resume requested via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
        })
        if (!result.ok) {
          sendError(response, 409, {
            error: "illegal_transition",
            message: "Resume is no longer legal for this run.",
            fromState: run.state,
            toState: "running",
          })
          return
        }
        if (run.threadRef !== undefined) {
          // FA-AC-15/FA-AC-44: re-enable through the exact same exactly-once
          // dispatch path every other Full Auto trigger already uses.
          capabilities.registry.set(run.threadRef, true, { workspaceRef: resolvedWorkspaceRef, profile: run.profile })
          capabilities.appendSystemNote(
            run.threadRef,
            `Full Auto run resumed programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
          )
        }
        void capabilities.triggerReconciliation().catch(() => {})
        auditLog("runs/resume", run.threadRef ?? runRef, "ok")
        sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, ok: true, run: projectSettled(capabilities, result.run, now) })
        return
      }

      if (runAction === "retry-now") {
        // FA-RUN-03 (#8971), AC-48: legal only from Stalled, and only when
        // the CURRENT (freshly classified) cause is plausibly recoverable --
        // never a retry that is guaranteed to repeat the same nonrecoverable
        // failure. First settle the run (so a stale-but-actually-recovered
        // record, or a fresh orphan/cap/failure-limit sync, is reflected
        // before deciding) exactly like every other read on this surface.
        const { run: settled } = settleRun(capabilities, run, now)
        const result = retryFullAutoRunNow(
          capabilities.runRegistry,
          settled,
          threadSnapshot(capabilities, settled),
          { actor: "control_api" },
          now,
        )
        if (!result.ok) {
          if (result.reason === "not_stalled") {
            sendError(response, 409, {
              error: "illegal_transition",
              message: `Retry now is legal only from Stalled (current state: ${result.state}).`,
              fromState: result.state,
              toState: "retrying",
            })
            return
          }
          sendError(response, 409, {
            error: "not_recoverable",
            message: `This run's current stall cause (${result.cause ?? "unknown_error"}) cannot be resolved by retrying; Stop is the safe action.`,
            stallCause: result.cause ?? undefined,
          })
          return
        }
        if (settled.threadRef !== undefined) {
          capabilities.appendSystemNote(
            settled.threadRef,
            `Full Auto run retry requested programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
          )
        }
        void capabilities.triggerReconciliation().catch(() => {})
        auditLog("runs/retry-now", settled.threadRef ?? runRef, "ok")
        sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, ok: true, run: projectSettled(capabilities, result.run, now) })
        return
      }

      if (runAction === "handoff") {
        // FA-AC-58: a manual provider switch is legal only while paused --
        // the exact same state gate Resume enforces, so a switch can never
        // race an active dispatch.
        if (run.state !== "paused") {
          sendError(response, 409, {
            error: "illegal_transition",
            message: `A provider handoff is legal only while paused (current state: ${run.state}).`,
            fromState: run.state,
            toState: run.state,
          })
          return
        }
        const body = decodeFullAutoControlRunHandoffRequest(await readJsonBody(request))
        if (body === null) {
          sendError(response, 400, {
            error: "invalid_request",
            message: "handoff requires a JSON body: { targetLaneRef, reason? }.",
          })
          return
        }
        if (capabilities.providerLaneRegistry === undefined || capabilities.providerHandoffRegistry === undefined) {
          sendError(response, 409, {
            error: "handoff_refused",
            message: "Provider handoff is not available on this server instance.",
          })
          return
        }
        const sourceLaneRef = run.profile?.lane ?? FULL_AUTO_DEFAULT_LANE
        const targetLaneRef = body.targetLaneRef
        const reason = body.reason ?? `Provider handoff requested via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`
        const thread = run.threadRef === undefined ? null : (capabilities.getThread?.(run.threadRef) ?? null)
        // FA-AC-59: re-check target admission/auth/capability eligibility
        // through the exact same gate the existing interactive manual-switch
        // path uses -- a refusal leaves the run's lane/profile untouched
        // (rollback, never a partial state change).
        const switchResult = capabilities.providerLaneRegistry.switchThread({
          threadRef: run.threadRef ?? runRef,
          laneRef: targetLaneRef,
          lanes: await (capabilities.listLanes?.() ?? Promise.resolve([])),
          thread,
          requiredCapabilities: ["fullAuto"],
        })
        if (!switchResult.ok) {
          // FA-AC-59/FA-AC-58: a typed refusal is a durable record, not only
          // an HTTP response -- the run's lane/profile stays exactly as it
          // was (rollback), but the refusal itself is receipted so the
          // owner-visible history never silently omits a rejected switch
          // attempt.
          const refusedAt = (input.now ?? (() => new Date()))().toISOString()
          capabilities.providerHandoffRegistry.record({
            runRef: run.runRef,
            ...(run.threadRef === undefined ? {} : { threadRef: run.threadRef }),
            from: sourceLaneRef,
            to: targetLaneRef,
            actor: "control_api",
            at: refusedAt,
            reason,
            disposition: "refused",
            truncated: false,
            refusalReason: switchResult.reason,
          })
          auditLog("runs/handoff", run.threadRef ?? runRef, `refused ${switchResult.reason}`)
          sendError(response, 409, {
            error: "handoff_refused",
            message: switchResult.message,
            handoffRefusalReason: switchResult.reason,
          })
          return
        }
        const at = (input.now ?? (() => new Date()))().toISOString()
        const envelope = buildProviderHandoffEnvelope({
          run,
          sourceLaneRef,
          targetLaneRef,
          thread,
          reason,
          actor: "control_api",
          at,
        })
        const disposition = providerHandoffDispositionForEnvelope(envelope)
        const transitionRecord = capabilities.providerHandoffRegistry.record({
          runRef: run.runRef,
          ...(run.threadRef === undefined ? {} : { threadRef: run.threadRef }),
          from: sourceLaneRef,
          to: targetLaneRef,
          actor: "control_api",
          at,
          reason,
          disposition,
          truncated: envelope.contextTruncated,
          envelopeSchema: envelope.schema,
        })
        const rebound = capabilities.runRegistry.rebindProfile(runRef, { ...run.profile, lane: targetLaneRef })
        if (rebound === null) {
          sendError(response, 404, { error: "not_found", message: "No Full Auto run exists for that runRef." })
          return
        }
        capabilities.appendSystemNote(
          run.threadRef ?? runRef,
          `Provider handoff: ${sourceLaneRef} → ${targetLaneRef} (${disposition}). Reason: ${reason} ` +
          `(caller: ${FULL_AUTO_CONTROL_CALLER}).`,
        )
        auditLog("runs/handoff", run.threadRef ?? runRef, `ok ${sourceLaneRef}->${targetLaneRef} disposition=${disposition}`)
        sendJson(response, 200, {
          schema: FULL_AUTO_CONTROL_SCHEMA,
          ok: true,
          run: projectSettled(capabilities, rebound, now),
          transition: transitionRecord,
        })
        return
      }

      // stop: terminal, legal from any non-terminal state, never resumed.
      const turnRunning = run.threadRef !== undefined
        && capabilities.liveState(run.threadRef)?.state === "turn_running"
      const result = capabilities.runRegistry.transition(runRef, {
        to: "stopped",
        actor: "control_api",
        reason: `Stop requested via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
      })
      if (!result.ok) {
        if (result.reason === "not_found") {
          sendError(response, 404, { error: "not_found", message: "No Full Auto run exists for that runRef." })
          return
        }
        sendError(response, 409, {
          error: "illegal_transition",
          message: `Stop is not legal from state ${result.from} (the run is already terminal).`,
          fromState: result.from,
          toState: result.to,
        })
        return
      }
      if (run.threadRef !== undefined) {
        capabilities.registry.set(run.threadRef, false, { disabledBy: "control_api" })
        if (turnRunning) capabilities.interruptLiveTurn?.(run.threadRef)
        capabilities.appendSystemNote(
          run.threadRef,
          `Full Auto run stopped programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
        )
      }
      auditLog("runs/stop", run.threadRef ?? runRef, "ok")
      sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, ok: true, run: projectSettled(capabilities, result.run, now) })
      return
    }

    const match = /^\/v1\/full-auto\/([^/]+)(?:\/(enable|disable|continue-now|turns))?$/.exec(
      url.pathname,
    )
    if (match === null) {
      sendError(response, 404, { error: "not_found", message: "Unknown route." })
      return
    }
    const threadRef = decodeFullAutoControlThreadRef(decodeURIComponent(match[1]!))
    if (threadRef === null) {
      sendError(response, 400, {
        error: "invalid_request",
        message: "threadRef must be a 1-120 character string.",
      })
      return
    }
    const action = match[2] ?? null

    if (action === null || action === "turns") {
      if (method !== "get") {
        sendError(response, 405, { error: "method_not_allowed", message: "Use GET." })
        return
      }
      if (action === "turns") {
        sendJson(response, 200, {
          schema: FULL_AUTO_CONTROL_SCHEMA,
          threadRef,
          turns: projectTurns(capabilities.listTurns(threadRef)),
        })
        return
      }
      const record = capabilities.registry.record(threadRef)
      if (record === null) {
        sendError(response, 404, {
          error: "not_found",
          message: "No Full Auto record exists for that threadRef.",
        })
        return
      }
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        serverInstanceId: instanceId,
        record: projectRecord(record, capabilities.liveState(threadRef)),
      })
      return
    }

    if (method !== "post") {
      sendError(response, 405, { error: "method_not_allowed", message: "Use POST." })
      return
    }

    if (action === "enable") {
      const body = decodeFullAutoControlEnableRequest(await readJsonBody(request))
      if (body === null) {
        sendError(response, 400, {
          error: "invalid_request",
          message: "enable requires a JSON body naming the expected workspace: { workspaceRef }.",
        })
        return
      }
      // FA-H2 / #8886: the caller names the workspace it expects; main
      // resolves the current one itself. Any difference is a typed REFUSAL
      // with the registry untouched -- never a redirect, and never a grant
      // of a new workspace (granting stays a human/UI action).
      const resolvedWorkspaceRef = capabilities.resolveWorkspaceRef()
      if (body.workspaceRef !== resolvedWorkspaceRef) {
        auditLog("enable", threadRef, "refused workspace_mismatch")
        sendError(response, 409, {
          error: "workspace_mismatch",
          message:
            "The named workspace does not match the currently resolved workspace; Full Auto was " +
            "NOT enabled. Enable is a refusal on mismatch, never a redirect or a new grant.",
          expectedWorkspaceRef: body.workspaceRef,
          resolvedWorkspaceRef,
        })
        return
      }
      const lane = body.lane ?? FULL_AUTO_DEFAULT_LANE
      if (!(capabilities.isLaneEligible?.(lane) ?? lane === FULL_AUTO_DEFAULT_LANE)) {
        sendError(response, 409, {
          error: "lane_not_eligible",
          message: `Provider lane ${lane} is not admitted for Full Auto background turns.`,
        })
        return
      }
      // Same path as the CodexLocalFullAutoSetChannel handler: bind the
      // resolved workspace onto the durable record and enable.
      const record = capabilities.registry.set(threadRef, true, {
        workspaceRef: resolvedWorkspaceRef,
        profile: { lane },
      })
      capabilities.appendSystemNote(
        threadRef,
        `Full Auto enabled programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
      )
      auditLog("enable", threadRef, "ok")
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        ok: true,
        record: projectRecord(record, capabilities.liveState(threadRef)),
      })
      return
    }

    if (action === "disable") {
      const record = capabilities.registry.set(threadRef, false, { disabledBy: "control_api" })
      capabilities.appendSystemNote(
        threadRef,
        `Full Auto disabled programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
      )
      auditLog("disable", threadRef, "ok")
      sendJson(response, 200, {
        schema: FULL_AUTO_CONTROL_SCHEMA,
        ok: true,
        record: projectRecord(record, capabilities.liveState(threadRef)),
      })
      return
    }

    // continue-now: a new TRIGGER into the shared serialized reconcile path,
    // never a parallel dispatch mechanism. Fire-and-forget by design -- the
    // pass runs async behind the same promise-chain mutex as every other
    // trigger, and dispatch stays subject to lease/workspace/backoff/cap.
    const record = capabilities.registry.record(threadRef)
    if (record === null) {
      sendError(response, 404, {
        error: "not_found",
        message: "No Full Auto record exists for that threadRef.",
      })
      return
    }
    capabilities.appendSystemNote(
      threadRef,
      `Full Auto continuation requested programmatically via the local control API (caller: ${FULL_AUTO_CONTROL_CALLER}).`,
    )
    void capabilities.triggerReconciliation().catch(() => {})
    auditLog("continue-now", threadRef, "scheduled")
    sendJson(response, 200, { schema: FULL_AUTO_CONTROL_SCHEMA, scheduled: true })
  }

  const server: Server = createServer((request, response) => {
    void handle(request, response).catch(() => {
      if (!response.headersSent) {
        sendError(response, 500, {
          error: "invalid_request",
          message: "The control server failed to process the request.",
        })
      } else {
        response.end()
      }
    })
  })

  return new Promise<FullAutoControlServer>((resolve, reject) => {
    server.once("error", reject)
    // Loopback-only by construction: the listener binds 127.0.0.1 and no
    // other interface, exactly like the Harness MCP pilot.
    server.listen(input.port ?? 0, "127.0.0.1", () => {
      const address = server.address()
      if (address === null || typeof address === "string") {
        server.close()
        reject(new Error("full auto control server failed to bind a loopback port"))
        return
      }
      const url = `http://127.0.0.1:${address.port}`
      try {
        writeControlFile(input.controlFilePath, { url, credential, instanceId })
      } catch (error) {
        server.close()
        reject(error instanceof Error ? error : new Error("control file write failed"))
        return
      }
      resolve({
        url,
        credential,
        instanceId,
        controlFilePath: input.controlFilePath,
        stop: () =>
          new Promise<void>(resolveStop => {
            rmSync(input.controlFilePath, { force: true })
            server.close(() => resolveStop())
          }),
      })
    })
  })
}
