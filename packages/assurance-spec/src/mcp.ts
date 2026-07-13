/**
 * AssuranceSpec MCP server (AT-1, docs/assurance/AGENT_TOOLING.md §3).
 *
 * Hand-rolled zero-dependency JSON-RPC 2.0 over stdio at protocol
 * `2024-11-05` — no MCP SDK, because the package's entire value is
 * determinism and a tiny supply chain. Tool handlers are the exact Effect
 * programs the CLI uses (src/handlers.ts): one implementation, two
 * transports; only the JSON-RPC framing lives here.
 *
 * Every tool is read-only and deterministic and never calls a model (Law 2).
 * Tool-level failures return structured content `{ ok: false, code, message,
 * path? }` (§3.2); protocol-level errors stay JSON-RPC (-32601 unknown
 * method, -32700 parse).
 */
import { createInterface } from "node:readline"
import { isAbsolute, relative, resolve } from "node:path"

import type { Effect } from "effect"

import {
  beginAssuranceSession,
  checkAssuranceSession,
  checkCompletionClaim,
  getAssuranceSpec,
  getCoverageLedgers,
  getEnvironments,
  getEvidenceChecklist,
  getGates,
  getObligation,
  getObligationGraph,
  getObligations,
  getRepositoryInventory,
  getSeams,
  getSubjectBinding,
  getTypedGaps,
  listAssuranceSpecs,
  runTool,
  validateAssuranceSpecFile,
  type AssuranceToolError,
  type ToolFailure,
} from "./handlers.ts"

export const MCP_PROTOCOL_VERSION = "2024-11-05" as const
export const MCP_SERVER_NAME = "assurance-spec" as const
/** Keep in sync with packages/assurance-spec/package.json. */
export const MCP_SERVER_VERSION = "0.1.0" as const

type JsonRpcId = string | number | null

type JsonRpcRequest = Readonly<{
  jsonrpc?: string
  id?: JsonRpcId
  method?: string
  params?: unknown
}>

type JsonRpcResponse = Readonly<{
  jsonrpc: "2.0"
  id: JsonRpcId
  result?: unknown
  error?: Readonly<{ code: number; message: string }>
}>

// ---------------------------------------------------------------------------
// Input schema helpers (plain JSON Schema objects for tools/list)
// ---------------------------------------------------------------------------

const stringProperty = (description: string): object => ({ type: "string", description })

const requiredStringProperty = (description: string): object => ({
  type: "string",
  description,
  minLength: 1,
})

const objectSchema = (
  properties: Record<string, object>,
  required: ReadonlyArray<string> = [],
): object => ({
  type: "object",
  properties,
  ...(required.length > 0 ? { required } : {}),
  additionalProperties: false,
})

const ROOT_PROPERTY = stringProperty(
  "Optional sub-root inside the server root. Defaults to the server root; it can never escape it.",
)

const specPathSchema = (): object =>
  objectSchema(
    {
      root: ROOT_PROPERTY,
      path: requiredStringProperty("Root-relative path to a .assurance-spec.md file."),
    },
    ["path"],
  )

// ---------------------------------------------------------------------------
// Argument coercion
// ---------------------------------------------------------------------------

const asRecord = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}

const optionalString = (value: unknown): string | undefined =>
  typeof value === "string" ? value : undefined

class McpToolInputError extends Error {}

const requiredString = (args: Record<string, unknown>, key: string): string => {
  const value = args[key]
  if (typeof value !== "string" || value.trim() === "") {
    throw new McpToolInputError(`${key} is required`)
  }
  return value
}

// ---------------------------------------------------------------------------
// Tool table (§3.1 — all read-only, all deterministic)
// ---------------------------------------------------------------------------

type ToolHandler = (
  args: Record<string, unknown>,
  root: string,
) => Effect.Effect<unknown, AssuranceToolError>

type ToolDefinition = Readonly<{
  description: string
  inputSchema: object
  handler: ToolHandler
}>

/**
 * Per-call `root` is a *sub-root*: it must stay inside the server root, so
 * the process-level confinement promise of `--root` holds for every call.
 */
const confineSubRoot = (serverRoot: string, requested: string | undefined): string => {
  if (requested === undefined) return serverRoot
  const absolute = isAbsolute(requested) ? requested : resolve(serverRoot, requested)
  const relativePath = relative(serverRoot, absolute)
  if (relativePath.startsWith("..") || isAbsolute(relativePath) || absolute.split("/").includes("..")) {
    throw new McpToolInputError(`root must stay inside the server root: ${requested}`)
  }
  return absolute
}

const pathArgs = (args: Record<string, unknown>, serverRoot: string) => ({
  root: confineSubRoot(serverRoot, optionalString(args.root)),
  path: requiredString(args, "path"),
})

export const MCP_TOOLS: Readonly<Record<string, ToolDefinition>> = {
  begin_assurance_session: {
    description:
      "Validate an AssuranceSpec and its bound ProductSpec subject, then pin a stateless dual digest (spec revision+digest, subject revision+digest). Store the full returned pin; no daemon holds it.",
    inputSchema: specPathSchema(),
    handler: (args, root) => beginAssuranceSession(pathArgs(args, root)),
  },
  check_assurance_session: {
    description:
      "Recompute both digests against a pinned session and classify unchanged / assurance_spec_changed / subject_changed / both_changed / invalid_current with a typed recommended_action. Pass the full pin from begin_assurance_session (or spec_digest and subject_digest).",
    inputSchema: objectSchema(
      {
        root: ROOT_PROPERTY,
        path: requiredStringProperty("Root-relative path to the pinned .assurance-spec.md file."),
        session_id: stringProperty("Optional session id echoed back for the record."),
        pin: { type: "object", description: "The full pin object returned by begin_assurance_session." },
        spec_digest: stringProperty("Pinned AssuranceSpec document digest (sha256:<hex> or bare hex)."),
        subject_digest: stringProperty("Pinned subject ProductSpec document digest (sha256:<hex> or bare hex)."),
      },
      ["path"],
    ),
    handler: (args, root) =>
      checkAssuranceSession({
        ...pathArgs(args, root),
        ...(optionalString(args.session_id) === undefined ? {} : { session_id: optionalString(args.session_id)! }),
        ...(args.pin === undefined ? {} : { pin: args.pin }),
        ...(optionalString(args.spec_digest) === undefined ? {} : { spec_digest: optionalString(args.spec_digest)! }),
        ...(optionalString(args.subject_digest) === undefined ? {} : { subject_digest: optionalString(args.subject_digest)! }),
      }),
  },
  list_assurance_specs: {
    description:
      "List every *.assurance-spec.md under root with path, id, revision, lifecycle_state, subject path, validity, and error/warning counts.",
    inputSchema: objectSchema({ root: ROOT_PROPERTY }),
    handler: (args, root) => listAssuranceSpecs({ root: confineSubRoot(root, optionalString(args.root)) }),
  },
  get_assurance_spec: {
    description: "Return the parsed AssuranceSpec document (errors if invalid).",
    inputSchema: specPathSchema(),
    handler: (args, root) => getAssuranceSpec(pathArgs(args, root)),
  },
  validate_assurance_spec: {
    description: "Full structural validation result: { valid, errors: [{code, message}], warnings }.",
    inputSchema: specPathSchema(),
    handler: (args, root) => validateAssuranceSpecFile(pathArgs(args, root)),
  },
  get_subject_binding: {
    description:
      "Return the subject block plus a live check: recomputed subject digest vs pinned, subject_status in bound / stale / missing.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getSubjectBinding(pathArgs(args, root)),
  },
  get_obligations: {
    description:
      "Filtered obligation summaries: id, title, criterion_refs, disposition, technique, environment_refs, design_status in ready / needs_design.",
    inputSchema: objectSchema(
      {
        root: ROOT_PROPERTY,
        path: requiredStringProperty("Root-relative path to a .assurance-spec.md file."),
        criterion_ref: stringProperty("Only obligations bound to this criterion."),
        status: stringProperty('Only obligations with this design status: "ready" or "needs_design".'),
        technique: stringProperty("Only obligations using this technique."),
      },
      ["path"],
    ),
    handler: (args, root) =>
      getObligations({
        ...pathArgs(args, root),
        ...(optionalString(args.criterion_ref) === undefined ? {} : { criterion_ref: optionalString(args.criterion_ref)! }),
        ...(optionalString(args.status) === undefined ? {} : { status: optionalString(args.status)! }),
        ...(optionalString(args.technique) === undefined ? {} : { technique: optionalString(args.technique)! }),
      }),
  },
  get_obligation: {
    description:
      "Full single-obligation detail: oracle, falsifier, evidence requirements, independence, dependencies, activation gate — with explicit unresolved_fields for what design has not filled in.",
    inputSchema: objectSchema(
      {
        root: ROOT_PROPERTY,
        path: requiredStringProperty("Root-relative path to a .assurance-spec.md file."),
        obligation_id: requiredStringProperty("The obligation ID, e.g. AO-CW-AC-04-01."),
      },
      ["path", "obligation_id"],
    ),
    handler: (args, root) =>
      getObligation({ ...pathArgs(args, root), obligation_id: requiredString(args, "obligation_id") }),
  },
  get_seams: {
    description:
      "Seam obligations only. Empty today; the tool exists so 'no seam coverage' is a queryable fact, not an absence.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getSeams(pathArgs(args, root)),
  },
  get_environments: {
    description:
      "Environment references in the spec. Missing Environment Profiles return typed gaps (environment_profile_missing), not empty successes.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getEnvironments(pathArgs(args, root)),
  },
  get_gates: {
    description: "Gate definitions and which obligations each gate arms.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getGates(pathArgs(args, root)),
  },
  get_obligation_graph: {
    description:
      "Obligation dependency-graph projection: designable_now vs blocked (with waits_on) vs gated, plus edges and a dependency-respecting design_order. Declared structure only — no receipts, no satisfied-dependency claims, no execution manifest ordering, no blended score (Law 7).",
    inputSchema: specPathSchema(),
    handler: (args, root) => getObligationGraph(pathArgs(args, root)),
  },
  get_coverage_ledgers: {
    description:
      "The three coverage ledgers, separately: criterion traceability, obligation-by-environment execution (all not_run today), reachable frontier (not_computed until a compiler exists). Never a blended score.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getCoverageLedgers(pathArgs(args, root)),
  },
  get_evidence_checklist: {
    description:
      "Per criterion: bound obligations, required evidence kinds and environments, and what is missing. Deterministic; collects nothing; attaches no verdicts to links.",
    inputSchema: objectSchema(
      {
        root: ROOT_PROPERTY,
        path: requiredStringProperty("Root-relative path to a .assurance-spec.md file."),
        criterion_ref: stringProperty("Only the checklist for this criterion."),
      },
      ["path"],
    ),
    handler: (args, root) =>
      getEvidenceChecklist({
        ...pathArgs(args, root),
        ...(optionalString(args.criterion_ref) === undefined ? {} : { criterion_ref: optionalString(args.criterion_ref)! }),
      }),
  },
  check_completion_claim: {
    description:
      "The honesty tool: every obligation across all eight status axes (admission, readiness, observation, infrastructure, stability, freshness, disposition, exception). observation is not_run until receipts exist; the claim is echoed for the record, never evaluated.",
    inputSchema: objectSchema(
      {
        root: ROOT_PROPERTY,
        path: requiredStringProperty("Root-relative path to a .assurance-spec.md file."),
        claim: stringProperty("The completion claim to audit; echoed, not evaluated."),
      },
      ["path"],
    ),
    handler: (args, root) =>
      checkCompletionClaim({
        ...pathArgs(args, root),
        ...(optionalString(args.claim) === undefined ? {} : { claim: optionalString(args.claim)! }),
      }),
  },
  get_typed_gaps: {
    description:
      "Consolidated typed-gap report: unresolved obligation fields, missing oracles/falsifiers, missing environment profiles, undesigned policies — the machine-readable version of what would have to exist before this spec could be admitted.",
    inputSchema: specPathSchema(),
    handler: (args, root) => getTypedGaps(pathArgs(args, root)),
  },
  get_repository_inventory: {
    description:
      "Committed-HEAD repository inventory: candidate test artifacts and declared scripts, explicitly labeled candidates_not_proof: true. Never maps candidates to proof.",
    inputSchema: objectSchema({ root: ROOT_PROPERTY }),
    handler: (args, root) => getRepositoryInventory({ root: confineSubRoot(root, optionalString(args.root)) }),
  },
}

// ---------------------------------------------------------------------------
// JSON-RPC framing
// ---------------------------------------------------------------------------

const resultResponse = (id: JsonRpcId, result: unknown): JsonRpcResponse => ({
  jsonrpc: "2.0",
  id,
  result,
})

const errorResponse = (id: JsonRpcId, code: number, message: string): JsonRpcResponse => ({
  jsonrpc: "2.0",
  id,
  error: { code, message },
})

const textContent = (value: unknown): object => ({
  content: [{ type: "text", text: JSON.stringify(value, null, 2) }],
})

const callTool = (request: JsonRpcRequest, serverRoot: string): JsonRpcResponse => {
  const id = request.id ?? null
  const params = asRecord(request.params)
  const name = params.name
  if (typeof name !== "string") {
    return errorResponse(id, -32602, "tools/call requires a string name")
  }
  const tool = MCP_TOOLS[name]
  if (tool === undefined) {
    return errorResponse(id, -32601, `Unknown tool: ${name}`)
  }
  let program: Effect.Effect<unknown, AssuranceToolError>
  try {
    program = tool.handler(asRecord(params.arguments), serverRoot)
  } catch (error) {
    const failure: ToolFailure = {
      ok: false,
      code: "invalid_argument",
      message: error instanceof Error ? error.message : String(error),
    }
    return resultResponse(id, textContent(failure))
  }
  const outcome = runTool(program)
  return resultResponse(id, textContent(outcome.ok ? outcome.value : outcome))
}

/**
 * Handle a single JSON-RPC request object. Returns `null` for notifications
 * (no id) per JSON-RPC 2.0. Exported so tests can drive the full protocol
 * surface without a child process.
 */
export const handleMcpRequest = (
  request: JsonRpcRequest,
  serverRoot: string,
): JsonRpcResponse | null => {
  if (request.id === undefined) return null
  const id = request.id ?? null
  if (typeof request.method !== "string" || request.method === "") {
    return errorResponse(id, -32600, "method is required")
  }
  switch (request.method) {
    case "initialize":
      return resultResponse(id, {
        protocolVersion: MCP_PROTOCOL_VERSION,
        serverInfo: { name: MCP_SERVER_NAME, version: MCP_SERVER_VERSION },
        capabilities: { tools: {} },
      })
    case "tools/list":
      return resultResponse(id, {
        tools: Object.entries(MCP_TOOLS).map(([name, tool]) => ({
          name,
          description: tool.description,
          inputSchema: tool.inputSchema,
        })),
      })
    case "tools/call":
      return callTool(request, serverRoot)
    default:
      return errorResponse(id, -32601, `Unknown method: ${request.method}`)
  }
}

/** Start the stdio MCP server confined to `root`. Blocks on stdin. */
export const runAssuranceSpecMcpServer = (root?: string): void => {
  const serverRoot = resolve(root ?? process.cwd())
  const readline = createInterface({ input: process.stdin, crlfDelay: Number.POSITIVE_INFINITY })
  readline.on("line", (line) => {
    if (line.trim() === "") return
    let request: JsonRpcRequest
    try {
      request = JSON.parse(line) as JsonRpcRequest
    } catch {
      process.stdout.write(`${JSON.stringify(errorResponse(null, -32700, "Parse error"))}\n`)
      return
    }
    const response = handleMcpRequest(request, serverRoot)
    if (response !== null) process.stdout.write(`${JSON.stringify(response)}\n`)
  })
}
