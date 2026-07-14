import { Runtime } from "@openagentsinc/runtime-platform"
/**
 * FEED-1 (openagents #8783): the OpenAgents harness MCP server.
 *
 * When Pylon supervises a foreign harness session (the Codex SDK thread in
 * the `codex_agent_task` lane), the harness cannot normally see any
 * OpenAgents-owned context. This module inverts that: Pylon runs a small
 * loopback MCP HTTP server that hands a READ-ONLY toolkit TO the wrapped
 * harness, the same direction T3 Code's control-plane MCP server works
 * (`projects/repos/t3code/apps/server/src/mcp/McpHttpServer.ts`).
 *
 * Toolkit (no mutating tools in this slice):
 * - `pylon.assignment.context` — assignmentRef, public-safe objective
 *   summary, and the pinned verify command for the current assignment;
 * - `pylon.fleet.status` — public-safe fleet/thread status projection;
 * - `pylon.receipt.lookup` — receipt lookup by ref, public-safe fields only.
 *
 * Convergence, not a third registry: tools are ordinary
 * `RegisteredKhalaTool`s in a `@openagentsinc/khala-tools` registry served by
 * the existing `handleKhalaMcpRequest` JSON-RPC path, filtered by the shared
 * `@openagentsinc/mcp-contract` authority classes (the new read-only
 * `status_read` Khala authority maps to `operator_read`).
 *
 * Credential: a per-session scoped token minted at harness launch. Scopes are
 * drawn from `@openagentsinc/environment-auth` (ENV-2 #8780) and evaluated
 * with its narrowing-only exchange — no third scope vocabulary.
 *
 * TODO(ENV-2 #8780): upgrade the plain per-session bearer secret to a
 * DPoP-bound token issued through the environment-auth token exchange once
 * wrapped harness MCP clients can produce RFC 9449 DPoP proofs. Codex's MCP
 * client today presents only a static bearer header, so this slice binds the
 * secret to one session + expiry instead of a key thumbprint.
 *
 * Redaction is load-bearing: every tool output passes BOTH the
 * `@openagentsinc/mcp-contract` unsafe-material rules and the khala-tools
 * public-text redaction before it can reach the harness. A field that still
 * looks secret-bearing after redaction is omitted, never served.
 */
import { createHash, randomBytes, timingSafeEqual } from "node:crypto"
import {
  detectOpenAgentsMcpUnsafeMaterial,
  redactOpenAgentsMcpUnsafeText,
  type OpenAgentsMcpReceipt,
} from "@openagentsinc/mcp-contract"
import {
  evaluateEnvironmentScopeExchange,
  type EnvironmentCapabilityScope,
} from "@openagentsinc/environment-auth"
import {
  handleKhalaMcpRequest,
  khalaToolError,
  khalaToolOk,
  makeKhalaToolRegistry,
  makeKhalaToolServices,
  type KhalaMcpClientPolicy,
  type KhalaMcpRequest,
  type KhalaMcpResponse,
  type KhalaToolResult,
  type RegisteredKhalaTool,
} from "@openagentsinc/khala-tools"
import { Effect } from "effect"

export const HARNESS_MCP_SERVER_NAME = "openagents-harness-mcp"
export const HARNESS_MCP_ENDPOINT_PATH = "/mcp"
/** Env var Pylon injects into the harness process env for this session only. */
export const HARNESS_MCP_SESSION_TOKEN_ENV = "OPENAGENTS_HARNESS_MCP_SESSION_TOKEN"
/** Env var carrying the loopback server URL for this session only. */
export const HARNESS_MCP_SERVER_URL_ENV = "OPENAGENTS_HARNESS_MCP_SERVER_URL"
export const HARNESS_MCP_DEFAULT_TTL_SECONDS = 4 * 60 * 60

export const HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL = "pylon.assignment.context"
export const HARNESS_MCP_FLEET_STATUS_TOOL = "pylon.fleet.status"
export const HARNESS_MCP_RECEIPT_LOOKUP_TOOL = "pylon.receipt.lookup"

/**
 * The full scope set a harness session credential may carry in this slice.
 * Read-only by construction: `satisfies` keeps it inside the ENV-2
 * environment-auth capability-scope vocabulary (itself a checked subset of
 * the OpenAgents MCP authority classes).
 */
export const HARNESS_MCP_SESSION_SCOPES = [
  "operator_read",
  "workspace_read",
] as const satisfies ReadonlyArray<EnvironmentCapabilityScope>

const harnessMcpAuthorityPolicy: KhalaMcpClientPolicy = {
  allowedAuthorities: ["operator_read"],
  denyHighRisk: true,
}

export type HarnessMcpAssignmentContext = Readonly<{
  assignmentRef: string
  workflow: string
  /** Public-safe objective summary only — never the raw prompt. */
  objectivePublicSummary: string
  /** The pinned verification command for this assignment (argv form). */
  verifyCommand: ReadonlyArray<string>
  runRef: string
  leaseRef: string
  workspaceRef?: string
}>

export type HarnessMcpThreadStatus = Readonly<{
  threadRef: string
  workflow: string
  phase: string
  tokensSoFar?: number
  tokenCountKind?: "exact" | "estimated"
  updatedAtIso: string
}>

export type HarnessMcpSessionContext = Readonly<{
  sessionRef: string
  assignment: HarnessMcpAssignmentContext
  fleetStatus: () => ReadonlyArray<HarnessMcpThreadStatus>
  lookupReceipt: (receiptRef: string) => OpenAgentsMcpReceipt | null
}>

export type HarnessMcpSessionCredential = Readonly<{
  /** The secret. Injected into the harness session env only; never persisted. */
  token: string
  sessionRef: string
  scopes: ReadonlyArray<EnvironmentCapabilityScope>
  issuedAtIso: string
  expiresAtIso: string
}>

export type MintHarnessMcpSessionCredentialInput = Readonly<{
  sessionRef: string
  now?: Date
  ttlSeconds?: number
  /** Narrowing-only against HARNESS_MCP_SESSION_SCOPES; empty inherits all. */
  requestedScopes?: ReadonlyArray<EnvironmentCapabilityScope>
}>

/**
 * Mints the per-session scoped credential injected at harness launch. Scope
 * evaluation reuses the ENV-2 narrowing-only exchange: requesting any scope
 * outside the read-only session set rejects the whole mint, so a launch bug
 * can never hand a harness a wider credential than this slice allows.
 */
export function mintHarnessMcpSessionCredential(
  input: MintHarnessMcpSessionCredentialInput,
): HarnessMcpSessionCredential {
  const decision = evaluateEnvironmentScopeExchange({
    subjectScopes: HARNESS_MCP_SESSION_SCOPES,
    requestedScopes: input.requestedScopes ?? [],
  })
  if (!decision.ok) {
    throw new Error(
      `harness MCP session credential mint rejected (${decision.reason}): ${decision.offendingScopes.join(", ")}`,
    )
  }
  const now = input.now ?? new Date()
  const ttlSeconds = input.ttlSeconds ?? HARNESS_MCP_DEFAULT_TTL_SECONDS
  if (!Number.isFinite(ttlSeconds) || ttlSeconds <= 0) {
    throw new Error("harness MCP session credential ttlSeconds must be positive")
  }
  return {
    token: `oahm_${randomBytes(32).toString("base64url")}`,
    sessionRef: input.sessionRef,
    scopes: decision.grantedScopes,
    issuedAtIso: now.toISOString(),
    expiresAtIso: new Date(now.getTime() + ttlSeconds * 1000).toISOString(),
  }
}

export type HarnessMcpCredentialRejection = "absent" | "mismatch" | "expired"

export function verifyHarnessMcpCredential(input: {
  readonly credential: HarnessMcpSessionCredential
  readonly presentedToken: string | undefined
  readonly now?: Date
}): { readonly ok: true } | { readonly ok: false; readonly reason: HarnessMcpCredentialRejection } {
  const presented = input.presentedToken?.trim() ?? ""
  if (presented.length === 0) return { ok: false, reason: "absent" }
  const expected = createHash("sha256").update(input.credential.token).digest()
  const actual = createHash("sha256").update(presented).digest()
  if (!timingSafeEqual(expected, actual)) return { ok: false, reason: "mismatch" }
  const now = input.now ?? new Date()
  if (now.getTime() >= Date.parse(input.credential.expiresAtIso)) {
    return { ok: false, reason: "expired" }
  }
  return { ok: true }
}

/**
 * Redacts one output field with the shared mcp-contract rules and then the
 * khala-tools public-text rules run inside `khalaToolOk`. If the field still
 * carries secret-shaped material after redaction, it is omitted entirely.
 */
function publicSafeField(value: string): string {
  const redacted = redactOpenAgentsMcpUnsafeText(value)
  return detectOpenAgentsMcpUnsafeMaterial(redacted).length > 0
    ? "[omitted:unsafe_output]"
    : redacted
}

function publicSafeReceiptProjection(receipt: OpenAgentsMcpReceipt): Record<string, unknown> {
  // Public-safe fields only: refs, kind, status, timestamps, and a redacted
  // summary. No provider payloads, prompts, tokens, or local paths.
  return {
    receiptRef: publicSafeField(receipt.receiptRef),
    kind: receipt.kind,
    status: receipt.status,
    generatedAt: receipt.generatedAt,
    authorityClass: receipt.authorityClass,
    targetRef: publicSafeField(receipt.targetRef),
    summary: publicSafeField(receipt.summary),
    artifactRefs: receipt.artifactRefs.map(publicSafeField),
    sourceRefs: receipt.sourceRefs.map(publicSafeField),
    ...(receipt.amountSats === undefined ? {} : { amountSats: receipt.amountSats }),
  }
}

function toolOkJson(payload: Record<string, unknown>, publicSummary: string): KhalaToolResult {
  const text = JSON.stringify(payload, null, 2)
  if (detectOpenAgentsMcpUnsafeMaterial(text).length > 0) {
    // Belt and suspenders: per-field projection should already have caught
    // this; if anything secret-shaped survived, omit the whole output.
    return khalaToolError("unsafe_output_omitted", "Tool output omitted by MCP safety policy.")
  }
  return khalaToolOk({ modelText: text, publicSummary })
}

function assignmentContextTool(session: HarnessMcpSessionContext): RegisteredKhalaTool {
  return {
    definition: {
      authority: "status_read",
      availability: ["coding"],
      description:
        "Current OpenAgents assignment context for this supervised session: assignment ref, public-safe objective, and the pinned verify command.",
      executionMode: "local",
      inputSchema: { additionalProperties: false, properties: {}, type: "object" },
      internalId: "pylon.harness_mcp.assignment_context",
      label: "Assignment Context",
      name: HARNESS_MCP_ASSIGNMENT_CONTEXT_TOOL,
      permissionMode: "allow",
      prompt: "Fetch the OpenAgents assignment context for the current supervised session.",
      promptGuidelines: [
        "Read-only. Returns public-safe projection fields only.",
        "Use the pinned verify command to confirm your change before finishing.",
      ],
    },
    execute: () =>
      Effect.sync(() => {
        const assignment = session.assignment
        return toolOkJson(
          {
            assignmentRef: publicSafeField(assignment.assignmentRef),
            workflow: publicSafeField(assignment.workflow),
            objectivePublicSummary: publicSafeField(assignment.objectivePublicSummary),
            verifyCommand: assignment.verifyCommand.map(publicSafeField),
            runRef: publicSafeField(assignment.runRef),
            leaseRef: publicSafeField(assignment.leaseRef),
            ...(assignment.workspaceRef === undefined
              ? {}
              : { workspaceRef: publicSafeField(assignment.workspaceRef) }),
          },
          "Assignment context projected for the supervised harness session.",
        )
      }),
  }
}

function fleetStatusTool(session: HarnessMcpSessionContext): RegisteredKhalaTool {
  return {
    definition: {
      authority: "status_read",
      availability: ["coding"],
      description:
        "Public-safe fleet/thread status projection for OpenAgents-supervised sessions visible to this Pylon.",
      executionMode: "local",
      inputSchema: { additionalProperties: false, properties: {}, type: "object" },
      internalId: "pylon.harness_mcp.fleet_status",
      label: "Fleet Status",
      name: HARNESS_MCP_FLEET_STATUS_TOOL,
      permissionMode: "allow",
      prompt: "Fetch the public-safe status projection of OpenAgents fleet threads.",
      promptGuidelines: ["Read-only status projection. Refs and phases only."],
    },
    execute: () =>
      Effect.sync(() =>
        toolOkJson(
          {
            threads: session.fleetStatus().map((thread) => ({
              threadRef: publicSafeField(thread.threadRef),
              workflow: publicSafeField(thread.workflow),
              phase: publicSafeField(thread.phase),
              updatedAtIso: thread.updatedAtIso,
              ...(thread.tokensSoFar === undefined ? {} : { tokensSoFar: thread.tokensSoFar }),
              ...(thread.tokenCountKind === undefined
                ? {}
                : { tokenCountKind: thread.tokenCountKind }),
            })),
          },
          "Fleet thread status projected for the supervised harness session.",
        )
      ),
  }
}

function receiptLookupTool(session: HarnessMcpSessionContext): RegisteredKhalaTool {
  return {
    definition: {
      authority: "status_read",
      availability: ["coding"],
      description:
        "Look up an OpenAgents receipt by receiptRef. Returns public-safe receipt fields only.",
      executionMode: "local",
      inputSchema: {
        additionalProperties: false,
        properties: { receiptRef: { type: "string" } },
        required: ["receiptRef"],
        type: "object",
      },
      internalId: "pylon.harness_mcp.receipt_lookup",
      label: "Receipt Lookup",
      name: HARNESS_MCP_RECEIPT_LOOKUP_TOOL,
      permissionMode: "allow",
      prompt: "Look up a public-safe OpenAgents receipt projection by receiptRef.",
      promptGuidelines: ["Read-only. Receipts carry refs and summaries, never raw payloads."],
    },
    execute: (args) =>
      Effect.sync(() => {
        const receiptRef = typeof args.receiptRef === "string" ? args.receiptRef.trim() : ""
        if (receiptRef.length === 0) {
          return khalaToolError("validation_failed", "receiptRef is required")
        }
        const receipt = session.lookupReceipt(receiptRef)
        if (receipt === null) {
          return khalaToolError("receipt_not_found", `no receipt visible to this session: ${receiptRef}`)
        }
        return toolOkJson(
          publicSafeReceiptProjection(receipt),
          "Receipt projected for the supervised harness session.",
        )
      }),
  }
}

export function createHarnessMcpToolRegistry(session: HarnessMcpSessionContext) {
  return makeKhalaToolRegistry([
    assignmentContextTool(session),
    fleetStatusTool(session),
    receiptLookupTool(session),
  ])
}

/**
 * Transport-neutral request handler (usable from tests or a future stdio
 * bridge). Credential verification happens in the transport layer; this
 * handler assumes an authenticated session.
 */
export async function handleHarnessMcpRequest(
  request: KhalaMcpRequest,
  session: HarnessMcpSessionContext,
): Promise<KhalaMcpResponse> {
  return handleKhalaMcpRequest(request, {
    policy: harnessMcpAuthorityPolicy,
    registry: createHarnessMcpToolRegistry(session),
    serverName: HARNESS_MCP_SERVER_NAME,
    services: makeKhalaToolServices(),
  })
}

export type HarnessMcpServer = Readonly<{
  /** Loopback origin, e.g. `http://127.0.0.1:49152`. */
  origin: string
  endpointPath: string
  /** Full MCP endpoint URL injected into the harness config. */
  url: string
  credential: HarnessMcpSessionCredential
  stop: () => void
}>

export type StartHarnessMcpServerInput = Readonly<{
  session: HarnessMcpSessionContext
  now?: () => Date
  port?: number
  ttlSeconds?: number
}>

function jsonResponse(payload: unknown, status: number, headers: Record<string, string> = {}): Response {
  return new Response(JSON.stringify(payload), {
    headers: { "cache-control": "no-store", "content-type": "application/json", ...headers },
    status,
  })
}

function unauthorizedResponse(reason: HarnessMcpCredentialRejection): Response {
  // Same shape as the T3 Code control-plane MCP server: a 401 with
  // `www-authenticate: Bearer` and a stable machine-readable error tag.
  return jsonResponse(
    {
      error: "invalid_mcp_credential",
      message: `A valid per-session OpenAgents MCP credential is required (${reason}).`,
    },
    401,
    { "www-authenticate": "Bearer" },
  )
}

/**
 * Starts the loopback harness MCP server for one supervised session and
 * mints its scoped credential. Loopback-only by construction
 * (hostname 127.0.0.1); every request must present the session bearer token.
 */
export function startHarnessMcpServer(input: StartHarnessMcpServerInput): HarnessMcpServer {
  const nowFn = input.now ?? (() => new Date())
  const credential = mintHarnessMcpSessionCredential({
    sessionRef: input.session.sessionRef,
    now: nowFn(),
    ...(input.ttlSeconds === undefined ? {} : { ttlSeconds: input.ttlSeconds }),
  })
  const server = Runtime.serve({
    fetch: async (request) => {
      const url = new URL(request.url)
      if (url.pathname !== HARNESS_MCP_ENDPOINT_PATH) {
        return jsonResponse({ error: "not_found" }, 404)
      }
      if (request.method !== "POST") {
        return jsonResponse({ error: "method_not_allowed" }, 405)
      }
      const authorization = request.headers.get("authorization") ?? ""
      const presentedToken = authorization.startsWith("Bearer ")
        ? authorization.slice("Bearer ".length)
        : undefined
      const verified = verifyHarnessMcpCredential({
        credential,
        now: nowFn(),
        presentedToken,
      })
      if (!verified.ok) return unauthorizedResponse(verified.reason)
      let parsed: KhalaMcpRequest
      try {
        parsed = (await request.json()) as KhalaMcpRequest
      } catch {
        return jsonResponse(
          { error: { code: -32700, message: "parse error" }, id: "null", jsonrpc: "2.0" },
          400,
        )
      }
      const response = await handleHarnessMcpRequest(parsed, input.session)
      return jsonResponse(response, 200)
    },
    hostname: "127.0.0.1",
    idleTimeout: 60,
    port: input.port ?? 0,
  })
  const origin = `http://127.0.0.1:${server.port}`
  return {
    credential,
    endpointPath: HARNESS_MCP_ENDPOINT_PATH,
    origin,
    stop: () => {
      server.stop(true)
    },
    url: `${origin}${HARNESS_MCP_ENDPOINT_PATH}`,
  }
}

/**
 * Codex CLI `--config` overrides (dotted-path flattening is done by the
 * Codex SDK) that register the per-session OpenAgents MCP server as a
 * streamable-HTTP server for that session only. The token itself stays out
 * of the config object: the CLI resolves it from the session-scoped env var
 * at connect time.
 */
export function codexHarnessMcpConfigOverrides(input: {
  readonly url: string
  readonly tokenEnvVar: string
}): Record<string, unknown> {
  return {
    mcp_servers: {
      openagents: {
        bearer_token_env_var: input.tokenEnvVar,
        url: input.url,
      },
    },
  }
}
