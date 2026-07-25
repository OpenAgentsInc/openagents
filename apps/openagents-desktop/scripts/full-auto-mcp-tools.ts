/**
 * The REGISTERED MCP tool surface for the Full Auto local control API, plus
 * the single dispatcher that maps a registered tool name onto a control-API
 * operation. Split out of `full-auto-mcp.ts` (which owns the stdio JSON-RPC
 * loop) so the safety property below can be asserted over the surface itself
 * rather than over the text of a file.
 *
 * ## Gate 8 (omega#26), restated by owner direction on 2026-07-25
 *
 *   "No model-initiated path can start Full Auto authority. Only an explicit
 *    human action can, wherever that action lives."
 *
 * Everything in this file is callable by a language model: an MCP client hands
 * `tools/list` to the model and calls whatever the model names. The MCP server
 * reads the loopback bearer out of the mode-0600 connection file itself, so a
 * model needs no credential of its own. That makes this surface the exact
 * place the restated gate binds.
 *
 * `full_auto_start`, `full_auto_run_start`, and `full_auto_enable` were
 * removed on 2026-07-25 under the owner's explicit choice of removal over a
 * confirmation prompt. A human still starts Full Auto from the Desktop
 * launcher (the `FullAutoRun*` renderer IPC channels), from the Omega chat
 * surface, and from `scripts/full-auto-cli.ts`. None of those is reachable by
 * a model through this surface.
 *
 * The surface that remains is read-only projections plus the
 * pause/resume/stop/disable control intents over a run a human already
 * started -- the same allowlist the engine's `apply_control_intent` enforces
 * and the same one root `INVARIANTS.md` grants Sarah.
 */
import type { ControlResult, controlOperations } from "./full-auto-control-client.ts"

export type FullAutoControlOperations = ReturnType<typeof controlOperations>
export type FullAutoControlOperationName = keyof FullAutoControlOperations

/**
 * Control-API operations that GRANT Full Auto authority: each one brings a
 * Full Auto grant into existence that did not exist before the call, and each
 * one schedules the first unattended continuation as part of the same call.
 * No registered MCP tool may reach any of these. The test in
 * `full-auto-mcp-tools.test.ts` proves that by dispatching every registered
 * tool against a recording operations object, so a rename cannot evade it.
 */
export const FULL_AUTO_AUTHORITY_GRANTING_OPERATIONS = [
  "start",
  "enable",
  "runsStart",
] as const satisfies ReadonlyArray<FullAutoControlOperationName>

/**
 * Control-API operations a model-callable tool MAY reach. Read-only
 * projections, plus the control intents that only ever narrow an existing
 * human-granted run (pause / resume / stop / disable) or re-trigger the
 * already-serialized reconciliation pass for a record a human already
 * enabled. None of these can create a Full Auto grant.
 */
export const FULL_AUTO_MODEL_CALLABLE_OPERATIONS = [
  "lanes",
  "list",
  "status",
  "turns",
  "runsList",
  "runStatus",
  "runReport",
  "runReceipt",
  "disable",
  "resume",
  "continueNow",
  "runPause",
  "runResume",
  "runStop",
] as const satisfies ReadonlyArray<FullAutoControlOperationName>

/**
 * Operations the MCP surface deliberately does not expose at all, and which
 * are neither authority-granting nor model-callable. Present so the three
 * lists together account for the WHOLE control-client surface: a newly added
 * control operation belongs to exactly one of them, and the test fails until
 * someone classifies it.
 */
export const FULL_AUTO_UNEXPOSED_OPERATIONS = [
  "openapi",
] as const satisfies ReadonlyArray<FullAutoControlOperationName>

const threadRefProperty = {
  threadRef: { type: "string", minLength: 1, maxLength: 120, description: "Desktop thread ref." },
} as const
const runRefProperty = {
  runRef: { type: "string", minLength: 1, maxLength: 180, description: "FullAutoRun ref (FA-RUN-01)." },
} as const

export type FullAutoMcpTool = Readonly<{
  name: string
  description: string
  inputSchema: {
    readonly type: "object"
    readonly additionalProperties: false
    readonly required?: ReadonlyArray<string>
    readonly properties: Readonly<Record<string, Readonly<Record<string, unknown>>>>
  }
}>

export const FULL_AUTO_MCP_TOOLS: ReadonlyArray<FullAutoMcpTool> = [
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
    name: "full_auto_resume",
    description:
      "Resume a Full Auto record the FA-GD-01 confidence gate durably paused (pausedReason set). " +
      "Clears the pause and schedules the shared reconciliation pass; refused with 409 not_paused " +
      "when the record is not currently paused -- resume never re-enables a disabled record, and " +
      "never creates one.",
    inputSchema: { type: "object", additionalProperties: false, required: ["threadRef"], properties: threadRefProperty },
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
      "every other trigger; returns { scheduled: true } immediately. Dispatches nothing unless a " +
      "human already enabled Full Auto on the thread.",
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
]

/**
 * Tool names removed on 2026-07-25 because they let a language model start
 * Full Auto authority. Kept as data so the surface test can assert their
 * absence by name in addition to asserting the (rename-proof) reachability
 * property, and so a future reader sees the decision rather than a gap.
 */
export const FULL_AUTO_REMOVED_MODEL_CALLABLE_START_TOOLS = [
  "full_auto_start",
  "full_auto_run_start",
  "full_auto_enable",
] as const

/**
 * The one place a registered tool name becomes a control-API call. Returns
 * `null` for any name that is not registered, which the MCP server surfaces as
 * an `isError` "unknown tool" result -- including for every removed start
 * tool.
 */
export const dispatchFullAutoMcpTool = async (
  operations: FullAutoControlOperations,
  name: string,
  args: Record<string, unknown>,
): Promise<ControlResult | null> => {
  const threadRef = typeof args.threadRef === "string" ? args.threadRef : ""
  const runRef = typeof args.runRef === "string" ? args.runRef : ""
  // No remaining tool carries a routing policy or a guardrail override: those
  // ride only on start/enable, and those are gone. A model can therefore not
  // choose the lane, the account, the turn cap, or the wall-clock budget an
  // unattended run executes under.
  return name === "provider_lanes_list"
    ? await operations.lanes()
    : name === "full_auto_list"
    ? await operations.list()
    : name === "full_auto_status"
    ? await operations.status(threadRef)
    : name === "full_auto_resume"
    ? await operations.resume(threadRef)
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
}
