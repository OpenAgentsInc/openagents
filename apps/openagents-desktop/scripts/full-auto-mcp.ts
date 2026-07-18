/**
 * FA-H13 (#8886): a minimal stdio MCP server over the Full Auto local control
 * API -- a deliberately thin pass-through client of the one OpenAPI surface
 * Desktop main serves. Each tool discovers the loopback server from the
 * mode-0600 connection file, calls the HTTP route, and returns the JSON.
 *
 * The MCP handshake (initialize / tools/list / tools/call over
 * newline-delimited JSON-RPC on stdio) is implemented by hand: the workspace
 * carries no direct `@modelcontextprotocol/sdk` dependency (it appears only
 * as a transitive peer of the Claude agent SDK, which pnpm's strict linker
 * does not expose to this package), and the protocol subset needed for six
 * pass-through tools is deliberately small. The protocol revision matches the
 * repo's public MCP surface (PUBLIC_MCP_PROTOCOL_VERSION in
 * apps/openagents.com/workers/api/src/public-agent-mcp-discovery.ts).
 *
 * Usage: node --import tsx scripts/full-auto-mcp.ts [--user-data <path>]
 * (or set OPENAGENTS_DESKTOP_USER_DATA).
 */
import { createInterface } from "node:readline"

import {
  ControlUnavailableError,
  controlOperations,
  readControlConnection,
  resolveUserDataDir,
} from "./full-auto-control-client.ts"

const MCP_PROTOCOL_VERSION = "2025-06-18"
const SERVER_INFO = { name: "openagents-desktop-full-auto", version: "1.0.0" } as const

const threadRefProperty = {
  threadRef: { type: "string", minLength: 1, maxLength: 120, description: "Desktop thread ref." },
} as const
const runRefProperty = {
  runRef: { type: "string", minLength: 1, maxLength: 180, description: "FullAutoRun ref (FA-RUN-01)." },
} as const

const TOOLS = [
  {
    name: "provider_lanes_list",
    description: "List every configured provider lane with honest authentication, admission, and capability status.",
    inputSchema: { type: "object", additionalProperties: false, properties: {} },
  },
  {
    name: "full_auto_list",
    description: "List every Full Auto registry record with its coarse live state (public-safe projection).",
    inputSchema: { type: "object", additionalProperties: false, properties: {} },
  },
  {
    name: "full_auto_status",
    description: "One thread's Full Auto record plus coarse live state.",
    inputSchema: { type: "object", additionalProperties: false, required: ["threadRef"], properties: threadRefProperty },
  },
  {
    name: "full_auto_start",
    description:
      "Bootstrap Full Auto with no existing thread: mint a brand-new local thread, enable Full " +
      "Auto on it, and schedule the first continuation in one call. You MUST name the workspace " +
      "you expect (workspaceRef); on 409 workspace_mismatch NO thread is created. The new " +
      "threadRef is returned inside the record.",
    inputSchema: {
      type: "object",
      additionalProperties: false,
      required: ["workspaceRef"],
      properties: {
        workspaceRef: { type: "string", minLength: 1, maxLength: 1024, description: "Expected absolute workspace path." },
        title: { type: "string", minLength: 1, maxLength: 80, description: "Optional owner-visible thread title." },
        lane: { type: "string", minLength: 1, maxLength: 80, description: "Optional ProviderLane ref; defaults to codex-local." },
      },
    },
  },
  {
    name: "full_auto_enable",
    description:
      "Enable Full Auto for a thread. You MUST name the workspace you expect (workspaceRef); the " +
      "server refuses with 409 workspace_mismatch when it does not match the currently resolved " +
      "workspace, and can never grant a new workspace.",
    inputSchema: {
      type: "object",
      additionalProperties: false,
      required: ["threadRef", "workspaceRef"],
      properties: {
        ...threadRefProperty,
        workspaceRef: { type: "string", minLength: 1, maxLength: 1024, description: "Expected absolute workspace path." },
        lane: { type: "string", minLength: 1, maxLength: 80, description: "Optional ProviderLane ref; defaults to codex-local." },
      },
    },
  },
  {
    name: "full_auto_disable",
    description: "Durably disable Full Auto for a thread.",
    inputSchema: { type: "object", additionalProperties: false, required: ["threadRef"], properties: threadRefProperty },
  },
  {
    name: "full_auto_continue_now",
    description:
      "Schedule an immediate Full Auto reconciliation attempt through the same serialized path as " +
      "every other trigger; returns { scheduled: true } immediately.",
    inputSchema: { type: "object", additionalProperties: false, required: ["threadRef"], properties: threadRefProperty },
  },
  {
    name: "full_auto_turns",
    description: "Bounded recent Full Auto turn history (identity/phase/disposition/timestamps; never transcript text).",
    inputSchema: { type: "object", additionalProperties: false, required: ["threadRef"], properties: threadRefProperty },
  },
  // FA-RUN-01 (#8969): the durable FullAutoRun lifecycle surface.
  {
    name: "full_auto_runs_list",
    description: "List every durable FullAutoRun, settled against current thread-level truth.",
    inputSchema: { type: "object", additionalProperties: false, properties: {} },
  },
  {
    name: "full_auto_run_status",
    description: "One run's settled current lifecycle state, objective, and transition history.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
  {
    name: "full_auto_run_start",
    description:
      "Start a new FullAutoRun: title, objective, and done condition are REQUIRED. Refused with " +
      "409 active_run_conflict when a Full Auto run is already active for this Desktop profile -- " +
      "v1 allows at most one active run per profile.",
    inputSchema: {
      type: "object",
      additionalProperties: false,
      required: ["workspaceRef", "title", "objective", "doneCondition"],
      properties: {
        workspaceRef: { type: "string", minLength: 1, maxLength: 1024, description: "Expected absolute workspace path." },
        title: { type: "string", minLength: 1, maxLength: 120, description: "Owner-visible run title." },
        objective: { type: "string", minLength: 1, maxLength: 4000, description: "What this run should accomplish." },
        doneCondition: { type: "string", minLength: 1, maxLength: 2000, description: "How to know the run is done." },
        lane: { type: "string", minLength: 1, maxLength: 80, description: "Optional ProviderLane ref; defaults to codex-local." },
        turnCap: { type: "integer", minimum: 1, maximum: 1000, description: "Optional turn cap; defaults to 20." },
      },
    },
  },
  {
    name: "full_auto_run_pause",
    description:
      "Pause a run. Prevents any new dispatch immediately. With an active turn, transitions to " +
      "Pausing until that turn resolves, then Paused; with no turn in flight, transitions directly to Paused.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
  {
    name: "full_auto_run_resume",
    description: "Resume a run. Legal ONLY from Paused; re-validates workspace/lane admission before dispatching again.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
  {
    name: "full_auto_run_stop",
    description: "Stop a run. Terminal; legal from any non-terminal state; a stopped run is never resumed.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
  // FA-RUN-04 (#8972) / FA-RPT-01 (#8988): the run report/receipt surface.
  {
    name: "full_auto_run_report",
    description:
      "One run's bounded PRIVATE FullAutoRunReport, freshly synced on read: lifecycle transitions, " +
      "liveness gaps, provider handoffs, per-turn dispositions with lane/account identity, typed " +
      "thread failure history with disabledBy attribution, rotation history when present, typed " +
      "stop attribution, claimed commit-SHA evidence refs, and local-only default-on metrics " +
      "counters. Never raw transcript text.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
  {
    name: "full_auto_run_receipt",
    description:
      "One run's derived PUBLIC-SAFE FullAutoRunReceipt: identities, digests, dispositions, and " +
      "counts only -- structurally incapable of carrying objective/reason/path/transcript text.",
    inputSchema: { type: "object", additionalProperties: false, required: ["runRef"], properties: { runRef: runRefProperty.runRef } },
  },
] as const

type JsonRpcRequest = Readonly<{
  jsonrpc?: string
  id?: number | string | null
  method?: string
  params?: Record<string, unknown>
}>

const send = (message: Record<string, unknown>): void => {
  process.stdout.write(`${JSON.stringify(message)}\n`)
}
const sendResult = (id: number | string | null, result: unknown): void =>
  send({ jsonrpc: "2.0", id, result })
const sendRpcError = (id: number | string | null, code: number, message: string): void =>
  send({ jsonrpc: "2.0", id, error: { code, message } })

const takeOption = (name: string): string | undefined => {
  const index = process.argv.indexOf(name)
  return index === -1 ? undefined : process.argv[index + 1]
}
const userDataDir = resolveUserDataDir(takeOption("--user-data"))

const callTool = async (name: string, args: Record<string, unknown>): Promise<{
  content: Array<{ type: "text"; text: string }>
  isError?: boolean
}> => {
  const connection = readControlConnection(userDataDir)
  const operations = controlOperations(connection)
  const threadRef = typeof args.threadRef === "string" ? args.threadRef : ""
  const workspaceRef = typeof args.workspaceRef === "string" ? args.workspaceRef : ""
  const runRef = typeof args.runRef === "string" ? args.runRef : ""
  const result = name === "provider_lanes_list"
    ? await operations.lanes()
    : name === "full_auto_list"
    ? await operations.list()
    : name === "full_auto_status"
    ? await operations.status(threadRef)
    : name === "full_auto_start"
    ? await operations.start(
        workspaceRef,
        typeof args.title === "string" ? args.title : undefined,
        typeof args.lane === "string" ? args.lane : undefined,
      )
    : name === "full_auto_enable"
    ? await operations.enable(threadRef, workspaceRef, typeof args.lane === "string" ? args.lane : undefined)
    : name === "full_auto_disable"
    ? await operations.disable(threadRef)
    : name === "full_auto_continue_now"
    ? await operations.continueNow(threadRef)
    : name === "full_auto_turns"
    ? await operations.turns(threadRef)
    : name === "full_auto_runs_list"
    ? await operations.runsList()
    : name === "full_auto_run_status"
    ? await operations.runStatus(runRef)
    : name === "full_auto_run_start"
    ? await operations.runsStart({
        workspaceRef,
        title: typeof args.title === "string" ? args.title : "",
        objective: typeof args.objective === "string" ? args.objective : "",
        doneCondition: typeof args.doneCondition === "string" ? args.doneCondition : "",
        ...(typeof args.lane === "string" ? { lane: args.lane } : {}),
        ...(typeof args.turnCap === "number" ? { turnCap: args.turnCap } : {}),
      })
    : name === "full_auto_run_pause"
    ? await operations.runPause(runRef)
    : name === "full_auto_run_resume"
    ? await operations.runResume(runRef)
    : name === "full_auto_run_stop"
    ? await operations.runStop(runRef)
    : name === "full_auto_run_report"
    ? await operations.runReport(runRef)
    : name === "full_auto_run_receipt"
    ? await operations.runReceipt(runRef)
    : null
  if (result === null) {
    return { content: [{ type: "text", text: `unknown tool: ${name}` }], isError: true }
  }
  return {
    content: [{ type: "text", text: JSON.stringify(result.body, null, 2) }],
    ...(result.status >= 200 && result.status < 300 ? {} : { isError: true }),
  }
}

const handle = async (request: JsonRpcRequest): Promise<void> => {
  const id = request.id ?? null
  switch (request.method) {
    case "initialize":
      sendResult(id, {
        protocolVersion: MCP_PROTOCOL_VERSION,
        capabilities: { tools: {} },
        serverInfo: SERVER_INFO,
      })
      return
    case "notifications/initialized":
    case "notifications/cancelled":
      return // notifications get no response
    case "ping":
      sendResult(id, {})
      return
    case "tools/list":
      sendResult(id, { tools: TOOLS })
      return
    case "tools/call": {
      const name = typeof request.params?.name === "string" ? request.params.name : ""
      const args = (request.params?.arguments ?? {}) as Record<string, unknown>
      try {
        sendResult(id, await callTool(name, args))
      } catch (error) {
        const message = error instanceof ControlUnavailableError
          ? error.message
          : `Full Auto control call failed: ${error instanceof Error ? error.message : String(error)}`
        sendResult(id, { content: [{ type: "text", text: message }], isError: true })
      }
      return
    }
    default:
      if (id !== null) sendRpcError(id, -32601, `method not found: ${request.method ?? "(none)"}`)
  }
}

const lines = createInterface({ input: process.stdin, terminal: false })
lines.on("line", line => {
  const trimmed = line.trim()
  if (trimmed.length === 0) return
  let parsed: JsonRpcRequest
  try {
    parsed = JSON.parse(trimmed) as JsonRpcRequest
  } catch {
    sendRpcError(null, -32700, "parse error")
    return
  }
  void handle(parsed)
})
lines.on("close", () => process.exit(0))
