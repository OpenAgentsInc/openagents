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
  decodeFullAutoControlStartRequest,
  decodeFullAutoControlThreadRef,
  type FullAutoControlError,
  type FullAutoControlLive,
  type FullAutoControlRecord,
  type FullAutoControlTurn,
} from "./full-auto-control-contract.ts"
import { fullAutoControlOpenApiDocument } from "./full-auto-control-openapi.ts"
import type { FullAutoRecord, FullAutoRegistry } from "./full-auto-registry.ts"
import type { LocalTurnRecord } from "./local-turn-journal.ts"
import { FULL_AUTO_DEFAULT_LANE } from "./full-auto-lane.ts"
import type { ProviderLaneRegistryEntry } from "./provider-lane-registry.ts"

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
  live: live ?? { state: "idle", turnRef: null },
})

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
  value: Readonly<{ url: string; credential: FullAutoControlCredential }>,
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
  const credential = mintFullAutoControlCredential({ now: (input.now ?? (() => new Date()))() })
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
      const record = capabilities.registry.set(threadRef, false)
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
        writeControlFile(input.controlFilePath, { url, credential })
      } catch (error) {
        server.close()
        reject(error instanceof Error ? error : new Error("control file write failed"))
        return
      }
      resolve({
        url,
        credential,
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
