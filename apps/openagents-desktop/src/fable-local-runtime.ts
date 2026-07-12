/**
 * Fable local runtime (#8712) — one real streaming Claude turn on this
 * machine, with zero login flow, for the desktop composer's "Fable" lane in
 * local (not-signed-in) mode.
 *
 * Reuses the Pylon account mechanics rather than reinventing them:
 * `discoverPylonSiblingAccountHomes` finds the isolated `~/.claude-pylon-*`
 * sibling homes (the same discovery `pylon accounts list` and the runtime
 * intent supervisor use), `pylonClaudeAccountHomeHasAuth` gates readiness on
 * the pooled `claude-oauth-token` file, and `pylonAccountEnvironment` builds
 * the per-account child env (`CLAUDE_CONFIG_DIR` + `CLAUDE_CODE_OAUTH_TOKEN`).
 * The DEFAULT `~/.claude` home is explicitly excluded and no login flow is
 * ever run: no ready sibling home means a typed unavailable result — never a
 * fall-through to the cloud gateway (the no-silent-substitution law).
 *
 * Permission posture (EP250 owner override, statement verbatim 2026-07-11:
 * "disallowing bash is retarded, give them full tools full permissions
 * etc"): this is the OWNER-LOCAL danger profile — the full SDK toolset
 * (Bash, Write, Edit, WebSearch, WebFetch, NotebookEdit, Agent children, …)
 * with full permissions, consistent with the repo's owner-local executor
 * invariant (the Khala->Pylon runbook's danger-full-access / approval-never
 * posture; never a public wire field). The per-THREAD scratch workspace
 * under the app's userData remains the DEFAULT cwd only — it is no longer a
 * boundary, and the old PreToolUse containment guard is gone. MECHANISM
 * (receipted from sdk.d.ts): `permissionMode: "bypassPermissions"` would
 * "Bypass all permission checks" — including the `canUseTool` handler the
 * AskUserQuestion flow parks on — so the lane instead keeps `permissionMode:
 * "default"` with a `canUseTool` that ALLOWS every tool except
 * AskUserQuestion, which routes through the real question flow: it surfaces
 * as a typed question_pending event and resolves with the user's answers via
 * `answerQuestion` (allow + updatedInput — the SDK-documented answer
 * mechanism), with honest timeout/denied outcomes. Interactive-only tools
 * that could only fail headless (plan mode, Skill, onboarding picker) stay
 * disallowed — separately decided UX noise, not a permission bound.
 * `settingSources: []`; every emitted payload is bounded and path-redacted.
 *
 * Multi-turn: the runtime keeps an in-memory threadRef -> SDK session map and
 * resumes the session (`options.resume`) when this process already ran a turn
 * for the thread; otherwise it prepends bounded thread history to the prompt.
 * The thread workspace is derived from threadRef, so a follow-up turn sees
 * the files an earlier turn wrote.
 *
 * This module never imports `electron` (unit-testable under `bun test`); the
 * IPC wiring lives in main.ts.
 */
import { createHash } from "node:crypto"
import { mkdirSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"

import {
  discoverPylonSiblingAccountHomes,
  hashPylonAccountRef,
  pylonAccountEnvironment,
  pylonClaudeAccountHomeHasAuth,
  type ResolvedPylonAccountSelection,
} from "@openagentsinc/pylon-core/custody/account-registry"

import {
  FABLE_LOCAL_DELTA_LIMIT,
  FABLE_LOCAL_FINAL_TEXT_LIMIT,
  FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT,
  FABLE_LOCAL_PLAN_ENTRY_LIMIT,
  FABLE_LOCAL_SUMMARY_LIMIT,
  normalizeFableLocalMcpServers,
  type FableChildUsage,
  type FableLocalAnswerQuestionRequest,
  type FableLocalAvailability,
  type FableLocalEvent,
  type FableLocalFailureReason,
  type FableLocalImageAttachment,
  type FableLocalMcpServerConfig,
  type FableLocalPlanEntry,
  type FableLocalQuestion,
  type FableLocalQueueFollowupRequest,
  type FableLocalSteerChildRequest,
} from "./fable-local-contract.ts"
import {
  CODEX_CHILD_MODEL,
  CODEX_CHILD_REASONING_EFFORT,
  type CodexChildResult,
  type CodexChildRunInput,
} from "./codex-child-contract.ts"

const CLAUDE_AGENT_SDK_PACKAGE = "@anthropic-ai/claude-agent-sdk"
/**
 * The lane's requested model ("IT HAS TO BE FABLE"): the SDK `Options.model`
 * key accepts a full model ID and its own docs name `'claude-fable-5'` as an
 * example. Without this the turn silently runs on the account home's default
 * model (seen live: claude-sonnet-4-6).
 */
export const FABLE_LOCAL_MODEL = "claude-fable-5"
/** Prefix-match tolerance for versioned Fable IDs (e.g. claude-fable-5-…). */
export const FABLE_LOCAL_MODEL_FAMILY_PREFIX = "claude-fable"
/**
 * Tools this headless lane strips from the model's context entirely (never
 * offered, not offered-then-denied — the model must not see tools that can
 * only fail). These are UX-noise removals decided separately from the
 * owner's full-tools/full-permissions override (which the owner has NOT
 * reversed for these):
 * - `Skill`: skills are removed from the chat lane (`skills: []` pairs with
 *   this so bundled skill listings never auto-trigger).
 * - `EnterPlanMode`/`ExitPlanMode`: plan mode is an interactive CLI flow with
 *   no counterpart in this lane (seen dead on camera, EP250).
 * - `ShowOnboardingRolePicker`: interactive-only host dialog (sdk-tools.d.ts).
 * NotebookEdit is no longer here (full-tools override); AskUserQuestion is
 * deliberately NOT here — it is wired to a real question UI through the
 * canUseTool answer path below.
 */
export const FABLE_LOCAL_DISALLOWED_TOOLS = [
  "Skill",
  "EnterPlanMode",
  "ExitPlanMode",
  "ShowOnboardingRolePicker",
] as const
export const FABLE_LOCAL_MAX_TURNS = 16
export const FABLE_LOCAL_TIMEOUT_MS = 180_000
/**
 * How long a pending AskUserQuestion waits for the user before resolving as
 * a graceful typed deny (outcome "timeout"). The turn wall clock is PAUSED
 * while a question is pending — waiting on the user is not model latency.
 */
export const FABLE_LOCAL_QUESTION_TIMEOUT_MS = 600_000
/** The one interactive tool this lane answers through a real UI path. */
export const FABLE_LOCAL_QUESTION_TOOL = "AskUserQuestion"

/**
 * Deterministic per-thread workspace directory name under
 * `<scratchRoot>/threads/`. Persistent for the life of userData, so a
 * follow-up turn in the same thread sees files an earlier turn wrote.
 * Sanitized (no dots, no separators) plus a digest suffix so distinct
 * threadRefs can never collide or traverse.
 */
export const fableThreadWorkspaceSlug = (threadRef: string): string => {
  const digest = createHash("sha256").update(threadRef).digest("hex").slice(0, 12)
  const safe = threadRef.replace(/[^A-Za-z0-9_-]/g, "-").replace(/^-+/, "").slice(0, 40)
  return safe.length === 0 ? `thread-${digest}` : `${safe}-${digest}`
}

/**
 * Codex delegation (#8712 Lane C): the fully-qualified SDK MCP tool name.
 * Kept in `allowedTools` (auto-allow) whenever the delegate MCP server is
 * offered; every other tool is allowed through the allow-all canUseTool.
 */
export const FABLE_DELEGATE_TOOL_NAME = "mcp__codex__delegate"
/** Up to 3 simultaneous delegate calls per turn (the SDK parallelizes
 * concurrency-safe tool calls); at most 6 children per turn total. Over-cap
 * calls return a typed refusal — no child is ever spawned past a cap. */
export const FABLE_DELEGATE_MAX_CONCURRENT = 3
export const FABLE_DELEGATE_MAX_CHILDREN_PER_TURN = 6
/**
 * SDK MCP tool calls can exceed the CLI's 60s stream-close default and the
 * Codex children run up to 240s, so the child env pins
 * CLAUDE_CODE_STREAM_CLOSE_TIMEOUT above the child budget.
 */
export const FABLE_STREAM_CLOSE_TIMEOUT_MS = 270_000
/** Turn wall clock when delegation is enabled: children may take 240s each. */
export const FABLE_LOCAL_DELEGATION_TIMEOUT_MS = 600_000
/**
 * The delegate tool contract shown to the model. LIMITATION (receipted): the
 * codex exec --json stream does not echo model/effort, so the pin is
 * spawn-config truth — the child is REQUESTED as gpt-5.6-sol at medium
 * reasoning and results are labeled "(requested)".
 */
export const FABLE_DELEGATE_TOOL_DESCRIPTION =
  "Delegate a bounded task to a Codex sub-agent (gpt-5.6-sol, medium reasoning). " +
  "Returns the sub-agent's final answer. Up to 3 delegations may run at once; " +
  "at most 6 per turn. The sub-agent starts in an EMPTY scratch directory (not " +
  "your project) with full filesystem access — always include absolute paths " +
  "to any repo, code, or files it should read or examine in the task/context, " +
  "or it will explore an empty directory. " +
  "Model/effort are pinned at spawn config (the Codex exec stream does not echo them back)."
/** Bounded history window prepended when no resumable session exists. */
export const FABLE_LOCAL_HISTORY_MESSAGES = 12
export const FABLE_LOCAL_HISTORY_MESSAGE_LIMIT = 2_000

/**
 * The SDK image content block shape (capability I1). Matches the Anthropic
 * `ImageBlockParam` with a `Base64ImageSource` (sdk.d.ts): a `{ type: "image",
 * source: { type: "base64", media_type, data } }` block. Threaded into the
 * user message content array so the model receives the screenshot.
 */
export type FableLocalSdkImageBlock = Readonly<{
  type: "image"
  source: Readonly<{ type: "base64"; media_type: FableLocalImageAttachment["mediaType"]; data: string }>
}>

/**
 * A single streaming-input user message (capability I1). When a turn carries
 * images the runtime switches `query({ prompt })` from a plain string to an
 * `AsyncIterable<SDKUserMessage>` yielding exactly this shape — text block plus
 * one image block per attachment — because a bare string prompt cannot carry an
 * image (sdk.d.ts: image support requires the content-block array form).
 */
export type FableLocalSdkUserMessage = Readonly<{
  type: "user"
  message: Readonly<{
    role: "user"
    content: ReadonlyArray<Readonly<{ type: "text"; text: string }> | FableLocalSdkImageBlock>
  }>
  parent_tool_use_id: null
}>

export type FableLocalQuery = (input: {
  prompt: string | AsyncIterable<FableLocalSdkUserMessage>
  options: Record<string, unknown>
}) => AsyncIterable<unknown>

/**
 * Build the streaming-input user message for an image-carrying turn
 * (capability I1): one text block (the same prompt string the no-image path
 * would send) followed by one base64 image block per attachment. Yielding a
 * single message and returning ends the input stream, so the SDK runs exactly
 * one assistant turn — identical lifecycle to the string-prompt path.
 */
export const fableSdkUserMessageWithImages = (
  text: string,
  images: ReadonlyArray<FableLocalImageAttachment>,
): FableLocalSdkUserMessage => ({
  type: "user",
  message: {
    role: "user",
    content: [
      { type: "text", text },
      ...images.map((image): FableLocalSdkImageBlock => ({
        type: "image",
        source: { type: "base64", media_type: image.mediaType, data: image.data },
      })),
    ],
  },
  parent_tool_use_id: null,
})

async function* fableImagePromptStream(
  text: string,
  images: ReadonlyArray<FableLocalImageAttachment>,
): AsyncGenerator<FableLocalSdkUserMessage> {
  yield fableSdkUserMessageWithImages(text, images)
}

export type FableLocalHistoryMessage = Readonly<{
  role: "user" | "assistant" | "system"
  text: string
}>

export type FableLocalTurnInput = Readonly<{
  turnRef: string
  threadRef: string
  history: ReadonlyArray<FableLocalHistoryMessage>
  message: string
  accountRef?: string
  /** Exact host-validated SDK skill name selected through explicit slash syntax. */
  skillName?: string
  /**
   * Optional image attachments (capability I1). When present the turn's SDK
   * input becomes a streaming-input user message carrying the text plus one
   * `type:"image"` base64 block per attachment. Absent/empty = the prior
   * string-prompt behavior, byte-for-byte unchanged.
   */
  images?: ReadonlyArray<FableLocalImageAttachment>
  emit: (event: FableLocalEvent) => void
  /**
   * Opt-in plan mode (J2, EP250). Default off — current behavior unchanged.
   * When true the SDK runs with `permissionMode: "plan"` (receipted sdk.d.ts:
   * "'plan' - Planning mode, no actual tool execution") and the ExitPlanMode
   * tool is allowed so the model can present a plan for review. Todos still
   * flow via `plan_updated` in BOTH modes (TodoWrite is never disallowed).
   */
  planMode?: boolean
}>

export type FableLocalTurnResult =
  | Readonly<{
      ok: true
      text: string
      totalTokens: number | null
      /** Names the isolated account home that ran the turn (a ref, never a
       * path) — message-metadata inspector + Lane C ledger attribution. */
      accountRef: string
      /** Additive (#8712 Lane C): exact usage split for the session ledger. */
      usage?: FableChildUsage
    }>
  | Readonly<{ ok: false; reason: FableLocalFailureReason; detail: string }>

export type FableLocalAccountHome = Readonly<{ ref: string; home: string }>

/**
 * The Codex child executor behind the delegate tool (see
 * ./codex-child-runtime.ts). Injectable so tests and the smoke fixture drive
 * the REAL delegate handler with a scripted child.
 */
export type FableDelegateRuntime = Readonly<{
  /**
   * G4 substrate: an optional `signal` is threaded so an in-flight child can
   * be interrupted (`steerChild`). The real Codex child runtime ignores the
   * signal today (interrupt therefore ABANDONS the child from the turn's view;
   * a true SIGTERM is a one-line follow-up in codex-child-runtime.ts, which
   * this lane deliberately does not own). Fixture delegates honor it to prove
   * the path end-to-end.
   */
  runChild: (input: CodexChildRunInput & { signal?: AbortSignal }) => Promise<CodexChildResult>
}>

/**
 * The SDK-MCP construction surface (createSdkMcpServer + tool + the zod raw
 * shape for the delegate input). Lazy-loaded from the real SDK by default;
 * injectable so unit tests and the smoke fixture never import the SDK.
 */
export type FableSdkMcpFactory = Readonly<{
  createSdkMcpServer: (options: {
    name: string
    version?: string
    tools: Array<unknown>
  }) => unknown
  tool: (
    name: string,
    description: string,
    inputSchema: Record<string, unknown>,
    handler: (args: Record<string, unknown>, extra: unknown) => Promise<unknown>,
  ) => unknown
  delegateInputShape: Record<string, unknown>
}>

export type FableLocalRuntimeOptions = Readonly<{
  /** Resolved lazily: Electron's userData path is not final at module load. */
  scratchRoot: () => string
  env?: Record<string, string | undefined>
  /** Injectable SDK loader (tests, smoke fixture). Default lazy-imports the SDK. */
  queryImpl?: () => Promise<FableLocalQuery>
  /** Injectable account discovery (tests, smoke fixture). */
  discoverImpl?: () => Promise<ReadonlyArray<FableLocalAccountHome>>
  /** When present, the lane exposes the mcp__codex__delegate tool. */
  delegate?: FableDelegateRuntime
  /** Injectable MCP construction (tests, smoke fixture). */
  mcpImpl?: () => Promise<FableSdkMcpFactory>
  /**
   * User-configured MCP servers (I2, EP250). A host getter — the SETTINGS UI
   * that edits this is a SEPARATE wave-2 lane; the config CONTRACT is frozen
   * in fable-local-contract.ts (`FableLocalMcpServerConfigSchema`). Defaults
   * to none, so current behavior is unchanged. Enabled servers are merged into
   * `Options.mcpServers` next to the internal `codex` delegate server; their
   * tools surface as `mcp__<name>__<tool>`. Read fresh per turn so config
   * edits take effect on the next turn without a runtime restart.
   */
  userMcpServers?: () => ReadonlyArray<FableLocalMcpServerConfig>
  /** Main-owned, validated local Claude plugin directories offered on the next turn. */
  userPlugins?: () => ReadonlyArray<string>
  timeoutMs?: number
  /** Pending-question window override (tests). Default 10 minutes. */
  questionTimeoutMs?: number
}>

const bounded = (value: string, limit: number): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value

/**
 * Public-safe path redaction for everything crossing the event boundary:
 * the scratch workspace becomes `<workspace>` and any path under the user's
 * home directory loses its absolute prefix.
 */
export const redactFableLocalText = (
  value: string,
  input: Readonly<{ workspace: string; home?: string }>,
): string => {
  const home = input.home ?? homedir()
  let out = value
  if (input.workspace.length > 0) out = out.split(input.workspace).join("<workspace>")
  if (home.length > 0) out = out.split(home).join("~")
  return out
}

/**
 * Ready Claude account homes, discovered exactly like the Pylon supervisor:
 * sibling `~/.claude-pylon-*`-style homes with a pooled OAuth token. The
 * default `~/.claude` home is never a candidate. Deterministic order (ref).
 */
export const discoverReadyFableClaudeHomes = async (
  env: Record<string, string | undefined> = process.env,
): Promise<ReadonlyArray<FableLocalAccountHome>> => {
  const root = (env.PYLON_ACCOUNT_HOME_ROOT ?? "").trim() || homedir()
  const defaultHome = join(root, ".claude")
  const siblings = await discoverPylonSiblingAccountHomes(env)
  const candidates = siblings
    .filter(entry => entry.provider === "claude_agent" && entry.home !== defaultHome)
    .sort((left, right) => left.ref.localeCompare(right.ref))
  const ready: FableLocalAccountHome[] = []
  for (const entry of candidates) {
    if (await pylonClaudeAccountHomeHasAuth(entry.home)) {
      ready.push({ ref: entry.ref, home: entry.home })
    }
  }
  return ready
}

const historyPrompt = (
  history: ReadonlyArray<FableLocalHistoryMessage>,
  message: string,
): string => {
  const window = history
    .filter(note => note.role !== "system")
    .slice(-FABLE_LOCAL_HISTORY_MESSAGES)
    .map(note =>
      `${note.role === "user" ? "User" : "Assistant"}: ${bounded(note.text, FABLE_LOCAL_HISTORY_MESSAGE_LIMIT)}`)
  if (window.length === 0) return message
  return [
    "Conversation so far (for context; reply only to the final user message):",
    ...window,
    `User: ${message}`,
  ].join("\n\n")
}

type SdkRecord = Record<string, unknown> & {
  type?: unknown
  subtype?: unknown
  session_id?: unknown
}

const contentBlocks = (message: unknown): ReadonlyArray<Record<string, unknown>> => {
  const content = (message as { content?: unknown } | undefined)?.content
  return Array.isArray(content)
    ? content.filter((block): block is Record<string, unknown> =>
        block !== null && typeof block === "object")
    : []
}

const usageTotalTokens = (value: unknown): number | null => {
  if (value === null || typeof value !== "object") return null
  const usage = value as Record<string, unknown>
  const finite = (candidate: unknown): number =>
    typeof candidate === "number" && Number.isFinite(candidate) && candidate > 0
      ? Math.trunc(candidate)
      : 0
  const total = finite(usage.input_tokens) + finite(usage.output_tokens) +
    finite(usage.cache_read_input_tokens) + finite(usage.cache_creation_input_tokens)
  return total > 0 ? total : null
}

/** Exact SDK result usage split for the ledger (same fields as the total). */
const usageSplitFromResult = (value: unknown): FableChildUsage | null => {
  if (value === null || typeof value !== "object") return null
  const usage = value as Record<string, unknown>
  const finite = (candidate: unknown): number =>
    typeof candidate === "number" && Number.isFinite(candidate) && candidate > 0
      ? Math.trunc(candidate)
      : 0
  const inputTokens = finite(usage.input_tokens)
  const cachedInputTokens = finite(usage.cache_read_input_tokens) +
    finite(usage.cache_creation_input_tokens)
  const outputTokens = finite(usage.output_tokens)
  const totalTokens = inputTokens + cachedInputTokens + outputTokens
  return totalTokens > 0
    ? { inputTokens, cachedInputTokens, outputTokens, reasoningTokens: 0, totalTokens }
    : null
}

/**
 * Default MCP construction: the real SDK's createSdkMcpServer/tool plus a
 * zod raw shape for the delegate input. zod is deliberately NOT an app
 * dependency (renderer boundary law bans it); the shape is built from the
 * SDK's OWN installed zod (createRequire from the SDK entry), so the SDK's
 * schema handling always sees its own zod instances.
 */
const defaultMcpFactory = async (): Promise<FableSdkMcpFactory> => {
  const sdk = (await import(CLAUDE_AGENT_SDK_PACKAGE)) as {
    createSdkMcpServer?: unknown
    tool?: unknown
  }
  if (typeof sdk.createSdkMcpServer !== "function" || typeof sdk.tool !== "function") {
    throw new Error("Claude Agent SDK did not expose createSdkMcpServer()/tool().")
  }
  const { createRequire } = await import("node:module")
  const requireFromHere = createRequire(import.meta.url)
  const sdkRequire = createRequire(requireFromHere.resolve(CLAUDE_AGENT_SDK_PACKAGE))
  const zodModule = sdkRequire("zod") as { z?: Record<string, unknown> } & Record<string, unknown>
  const z = (zodModule.z ?? zodModule) as {
    string: () => { describe: (text: string) => unknown; optional: () => { describe: (text: string) => unknown } }
  }
  return {
    createSdkMcpServer: sdk.createSdkMcpServer as FableSdkMcpFactory["createSdkMcpServer"],
    tool: sdk.tool as FableSdkMcpFactory["tool"],
    delegateInputShape: {
      task: z.string().describe("The bounded task for the Codex sub-agent."),
      context: z.string().optional().describe("Optional extra context for the task."),
    },
  }
}

const childUsageToLedger = (
  usage: Extract<CodexChildResult, { ok: true }>["usage"],
): FableChildUsage | null =>
  usage === null
    ? null
    : {
        inputTokens: usage.inputTokens,
        cachedInputTokens: usage.cachedInputTokens,
        outputTokens: usage.outputTokens,
        reasoningTokens: usage.reasoningOutputTokens,
        totalTokens: usage.totalTokens,
      }

const childUsageFooter = (result: Extract<CodexChildResult, { ok: true }>): string => {
  const usage = result.usage === null
    ? "usage unavailable"
    : `${result.usage.totalTokens.toLocaleString("en-US")} tokens (in ${result.usage.inputTokens} / cached ${result.usage.cachedInputTokens} / out ${result.usage.outputTokens} / reasoning ${result.usage.reasoningOutputTokens})`
  return `[codex child · account ${result.accountRef} · ${result.requestedModel} (requested, ${result.requestedEffort} reasoning) · ${usage} · ${(result.durationMs / 1000).toFixed(1)}s]`
}

/** Result of a `steerChild` control (G4). */
export type FableLocalSteerChildOutcome = Readonly<{
  ok: boolean
  outcome: "interrupted" | "delivered" | "unsupported" | "not_found"
}>

/** Result of a `queueFollowup` control (A3). */
export type FableLocalQueueFollowupOutcome =
  | Readonly<{ ok: true; queued: true; queueRef: string; position: number }>
  | Readonly<{ ok: false; queued: false; reason: "no_active_turn" }>

export type FableLocalRuntime = Readonly<{
  availability: () => Promise<FableLocalAvailability>
  runTurn: (input: FableLocalTurnInput) => Promise<FableLocalTurnResult>
  interrupt: (turnRef: string) => boolean
  /**
   * Delivers the user's answers to a pending AskUserQuestion (EP250 question
   * flow). Returns true when the pending question accepted the answers and
   * the tool call resolved; false is a typed rejection — unknown or already
   * settled questionRef, turnRef mismatch, or no answer matching any asked
   * question — and a still-pending question stays pending.
   */
  answerQuestion: (request: FableLocalAnswerQuestionRequest) => boolean
  /**
   * Steer or interrupt a running delegate child (G4). `interrupt` signals the
   * child's abort (and emits `child_steered` outcome `interrupted`);
   * `message` is honestly `unsupported` (Codex exec children are
   * non-interactive; SDK Agent subagents expose no per-child message API).
   * An unknown child for the turn returns `not_found` with no event.
   */
  steerChild: (request: FableLocalSteerChildRequest) => FableLocalSteerChildOutcome
  /**
   * Enqueue a follow-up message while a turn for the thread is streaming (A3).
   * Delivery is QUEUE-UNTIL-IDLE: the queued message is promoted (emitting
   * `followup_promoted`) when the current turn ends, not injected mid-stream.
   * Returns `no_active_turn` when the thread has no running turn — the caller
   * should just start a normal turn instead.
   */
  queueFollowup: (request: FableLocalQueueFollowupRequest) => FableLocalQueueFollowupOutcome
  dispose: () => void
}>

/**
 * Parsed AskUserQuestion input: the bounded/redacted questions the renderer
 * sees, plus the event-text -> original-text map so answers key back to the
 * SDK's exact question strings (the answers record is keyed by question
 * text; truncation must never break that keying).
 */
type ParsedFableQuestions = Readonly<{
  eventQuestions: ReadonlyArray<FableLocalQuestion>
  originalByEventText: ReadonlyMap<string, string>
}>

const parseAskUserQuestions = (
  rawInput: Record<string, unknown>,
  redact: (value: string) => string,
): ParsedFableQuestions | null => {
  const rawQuestions = rawInput.questions
  if (!Array.isArray(rawQuestions) || rawQuestions.length === 0) return null
  const eventQuestions: Array<FableLocalQuestion> = []
  const originalByEventText = new Map<string, string>()
  for (const candidate of rawQuestions.slice(0, 4)) {
    if (candidate === null || typeof candidate !== "object") return null
    const record = candidate as Record<string, unknown>
    if (typeof record.question !== "string" || record.question.trim() === "") return null
    const rawOptions = Array.isArray(record.options) ? record.options.slice(0, 4) : []
    const parsedOptions: Array<{ label: string; description?: string }> = []
    for (const option of rawOptions) {
      if (option === null || typeof option !== "object") continue
      const optionRecord = option as Record<string, unknown>
      if (typeof optionRecord.label !== "string" || optionRecord.label.trim() === "") continue
      parsedOptions.push({
        label: bounded(redact(optionRecord.label), 200),
        ...(typeof optionRecord.description === "string" && optionRecord.description.length > 0
          ? { description: bounded(redact(optionRecord.description), FABLE_LOCAL_SUMMARY_LIMIT) }
          : {}),
      })
    }
    if (parsedOptions.length === 0) return null
    const eventText = bounded(redact(record.question), FABLE_LOCAL_SUMMARY_LIMIT)
    eventQuestions.push({
      question: eventText,
      header: bounded(typeof record.header === "string" ? redact(record.header) : "", 120),
      options: parsedOptions,
      multiSelect: record.multiSelect === true,
    })
    originalByEventText.set(eventText, record.question)
  }
  return { eventQuestions, originalByEventText }
}

type PendingFableQuestion = Readonly<{
  turnRef: string
  accept: (answers: FableLocalAnswerQuestionRequest["answers"]) => boolean
  denyForTurnEnd: () => void
}>

export const makeFableLocalRuntime = (options: FableLocalRuntimeOptions): FableLocalRuntime => {
  const env = options.env ?? (process.env as Record<string, string | undefined>)
  const discover = options.discoverImpl ?? (() => discoverReadyFableClaudeHomes(env))
  // With delegation enabled the turn budget covers up to 240s Codex children.
  const timeoutMs = options.timeoutMs ??
    (options.delegate === undefined ? FABLE_LOCAL_TIMEOUT_MS : FABLE_LOCAL_DELEGATION_TIMEOUT_MS)
  const loadMcp = options.mcpImpl ?? defaultMcpFactory
  const loadQuery = options.queryImpl ?? (async () => {
    const sdk = (await import(CLAUDE_AGENT_SDK_PACKAGE)) as { query?: unknown }
    if (typeof sdk.query !== "function") throw new Error("Claude Agent SDK did not expose query().")
    return sdk.query as FableLocalQuery
  })
  const questionTimeoutMs = options.questionTimeoutMs ?? FABLE_LOCAL_QUESTION_TIMEOUT_MS
  const activeTurns = new Map<string, { interrupted: boolean; abort: () => void }>()
  /**
   * In-memory continuity: threadRef -> last completed SDK session, pinned to
   * the account that created it (a session is only resumable from the same
   * isolated account home).
   */
  const sessionByThread = new Map<string, { sessionId: string; accountRef: string }>()
  /**
   * Pending AskUserQuestion registry, keyed by questionRef (which embeds the
   * turnRef, so refs never collide across turns). Parallel-safe: several
   * questions may be pending at once without deadlock — each entry settles
   * independently on answer, timeout, or turn end.
   */
  const pendingQuestions = new Map<string, PendingFableQuestion>()
  /**
   * Running delegate children (G4), keyed by childRef. Each entry carries the
   * owning turnRef, an AbortController to interrupt it, and the turn's emit
   * sink so `steerChild` can surface a typed `child_steered` event.
   */
  const runningChildren = new Map<
    string,
    { turnRef: string; controller: AbortController; emit: (event: FableLocalEvent) => void }
  >()
  /**
   * The turn currently streaming for each thread (A3), so `queueFollowup` can
   * tell "a turn is running" from "idle" and emit onto the live stream.
   */
  const activeTurnByThread = new Map<
    string,
    { turnRef: string; emit: (event: FableLocalEvent) => void }
  >()
  /** Per-thread FIFO of queued follow-ups awaiting the idle boundary (A3). */
  const followupQueue = new Map<string, Array<{ queueRef: string; message: string }>>()
  let followupSequence = 0

  const availability = async (): Promise<FableLocalAvailability> => {
    const ready = await discover()
    const first = ready[0]
    if (first === undefined) return { state: "unavailable", reason: "no_claude_account" }
    return { state: "available", accountRef: first.ref }
  }

  const runTurn = async (input: FableLocalTurnInput): Promise<FableLocalTurnResult> => {
    const failure = (
      reason: FableLocalFailureReason,
      detail: string,
    ): Extract<FableLocalTurnResult, { ok: false }> =>
      ({ ok: false, reason, detail: bounded(detail, FABLE_LOCAL_SUMMARY_LIMIT) })
    const emitFailure = (result: Extract<FableLocalTurnResult, { ok: false }>): FableLocalTurnResult => {
      input.emit({ kind: "turn_failed", reason: result.reason, detail: result.detail })
      return result
    }

    const discovered = await discover()
    const ready = input.accountRef === undefined
      ? discovered
      : discovered.filter(account => account.ref === input.accountRef)
    if (ready.length === 0) {
      return emitFailure(failure("no_claude_account", "no linked Claude account home found"))
    }
    let query: FableLocalQuery
    try {
      query = await loadQuery()
    } catch (error) {
      return emitFailure(failure("sdk_unavailable", error instanceof Error ? error.name : "sdk import failed"))
    }

    // Per-THREAD scratch workspace (EP250): derived from threadRef and stable
    // across turns, so a follow-up turn sees the files an earlier turn wrote
    // (e.g. greetings.md). Always under userData's scratch root — never a
    // repo, never shared across threads.
    const workspace = join(options.scratchRoot(), "threads", fableThreadWorkspaceSlug(input.threadRef))
    try {
      mkdirSync(workspace, { recursive: true })
    } catch (error) {
      return emitFailure(failure("session_failed", error instanceof Error ? error.name : "workspace unavailable"))
    }
    const redact = (value: string): string => redactFableLocalText(value, { workspace })
    /** Per-turn AskUserQuestion sequence for stable questionRefs. */
    const questionState = { sequence: 0 }
    const planMode = input.planMode === true
    /**
     * Plan/todo entries (J2/J4) from the SDK TodoWrite tool input. TodoWrite
     * is NEVER disallowed, so this fires in both normal and plan mode. The
     * high-frequency (1,617-obs) todo signal from the daily-coding audit.
     */
    const planEntriesFromTodoWrite = (
      rawInput: unknown,
    ): ReadonlyArray<FableLocalPlanEntry> | null => {
      if (rawInput === null || typeof rawInput !== "object") return null
      const todos = (rawInput as { todos?: unknown }).todos
      if (!Array.isArray(todos)) return null
      const entries: Array<FableLocalPlanEntry> = []
      for (const todo of todos.slice(0, FABLE_LOCAL_PLAN_ENTRY_LIMIT)) {
        if (todo === null || typeof todo !== "object") continue
        const record = todo as Record<string, unknown>
        const content = typeof record.content === "string" && record.content.trim() !== ""
          ? record.content
          : typeof record.activeForm === "string" ? record.activeForm : ""
        if (content.trim() === "") continue
        const status = record.status === "in_progress" || record.status === "completed"
          ? record.status
          : "pending"
        entries.push({ step: bounded(redact(content), FABLE_LOCAL_SUMMARY_LIMIT), status })
      }
      return entries.length > 0 ? entries : null
    }
    // User MCP servers (I2): normalized once per turn; invalid configs surface
    // as typed mcp_server_unavailable and the turn still runs. `codex` is the
    // reserved internal delegate server name.
    const userMcp = normalizeFableLocalMcpServers(options.userMcpServers?.() ?? [])
    const userServerNames = new Set(userMcp.valid.map(server => server.name))
    /** SDK-reported MCP failures already surfaced this turn (no dupes). */
    const announcedMcpUnavailable = new Set<string>()

    // -----------------------------------------------------------------------
    // Codex delegation (#8712 Lane C): one SDK MCP server exposing
    // mcp__codex__delegate. The handler runs the injected child runtime with
    // per-turn caps (3 concurrent / 6 total). All caps refuse TYPED — no
    // child is spawned past a cap and the model sees the refusal text.
    // Per-child lifecycle flows into the SAME FableLocalEvent envelope so
    // the UI has live child visibility without new transcript components.
    // -----------------------------------------------------------------------
    const delegation = { active: 0, total: 0, sequence: 0 }
    const toolText = (text: string, isError = false): Record<string, unknown> => ({
      content: [{ type: "text", text }],
      ...(isError ? { isError: true } : {}),
    })
    const delegateHandler = async (args: Record<string, unknown>): Promise<unknown> => {
      const delegate = options.delegate
      if (delegate === undefined) {
        return toolText("Delegation is not available in this session.", true)
      }
      const task = typeof args.task === "string" ? args.task.trim() : ""
      const context = typeof args.context === "string" ? args.context : undefined
      if (task === "") return toolText("Delegation refused: task must be a non-empty string.", true)
      if (delegation.total >= FABLE_DELEGATE_MAX_CHILDREN_PER_TURN) {
        return toolText(
          `Delegation refused: this turn already dispatched ${FABLE_DELEGATE_MAX_CHILDREN_PER_TURN} Codex children (per-turn cap). No child was spawned.`,
          true,
        )
      }
      if (delegation.active >= FABLE_DELEGATE_MAX_CONCURRENT) {
        return toolText(
          `Delegation refused: ${FABLE_DELEGATE_MAX_CONCURRENT} Codex children are already running (concurrency cap). Wait for one to finish, then delegate again. No child was spawned.`,
          true,
        )
      }
      delegation.total += 1
      delegation.active += 1
      delegation.sequence += 1
      const childRef = bounded(`child.codex.${input.turnRef}.${delegation.sequence}`, 120)
      // G4: register the running child so steerChild/interrupt can reach it,
      // and thread its abort signal into the child runtime.
      const childAbort = new AbortController()
      runningChildren.set(childRef, {
        turnRef: input.turnRef,
        controller: childAbort,
        emit: input.emit,
      })
      input.emit({
        kind: "child_started",
        childRef,
        summary: bounded(redact(task), FABLE_LOCAL_SUMMARY_LIMIT),
      })
      // Resolves to a sentinel the moment the child is interrupted, so the
      // handler unblocks even if the (real) child runtime does not yet honor
      // the abort signal. The child is then ABANDONED from the turn's view.
      const CHILD_INTERRUPTED = Symbol("child-interrupted")
      const interruptSignal = new Promise<typeof CHILD_INTERRUPTED>(resolve => {
        if (childAbort.signal.aborted) {
          resolve(CHILD_INTERRUPTED)
          return
        }
        childAbort.signal.addEventListener("abort", () => resolve(CHILD_INTERRUPTED), { once: true })
      })
      try {
        const raced = await Promise.race([
          delegate.runChild({
          childRef,
          task,
          ...(context === undefined ? {} : { context }),
          signal: childAbort.signal,
          onEvent: event => {
            // Once interrupted, stop surfacing residual child activity so the
            // transcript ends cleanly at the child_steered event.
            if (childAbort.signal.aborted) return
            if (event.kind === "attempt_started") {
              input.emit({
                kind: "child_activity",
                childRef,
                activity: "item",
                accountRef: event.accountRef,
                summary: bounded(
                  `spawning codex exec on account ${event.accountRef} (${CODEX_CHILD_MODEL}, ${CODEX_CHILD_REASONING_EFFORT} reasoning requested)`,
                  FABLE_LOCAL_SUMMARY_LIMIT,
                ),
              })
              return
            }
            if (event.kind === "item") {
              input.emit({
                kind: "child_activity",
                childRef,
                activity: "item",
                summary: bounded(`${event.itemType}: ${event.summary}`, FABLE_LOCAL_SUMMARY_LIMIT),
              })
              return
            }
            if (event.kind === "pre_content_failure_rotated") {
              // A NON-auth pre-content failure is rotated past VISIBLY —
              // typed event, never a silent rotation (EP250 broadening).
              input.emit({
                kind: "child_activity",
                childRef,
                activity: "pre_content_failure_rotated",
                accountRef: event.accountRef,
                summary: bounded(
                  `account ${event.accountRef} failed before producing content (${event.detail}) — rotating to the next candidate Codex account`,
                  FABLE_LOCAL_SUMMARY_LIMIT,
                ),
              })
              return
            }
            // account_reconnect_required: a bad-credential account is
            // skipped VISIBLY — typed event, never a silent rotation.
            input.emit({
              kind: "child_activity",
              childRef,
              activity: "account_reconnect_required",
              accountRef: event.accountRef,
              summary: bounded(
                `account ${event.accountRef} needs reconnect (${event.detail}) — rotating to the next candidate Codex account`,
                FABLE_LOCAL_SUMMARY_LIMIT,
              ),
            })
          },
          }),
          interruptSignal,
        ])
        if (raced === CHILD_INTERRUPTED) {
          // G4: the user interrupted this child. steerChild already emitted
          // child_steered(interrupted); tell the model the child is gone.
          return toolText(
            "The Codex child was interrupted before it finished. No child output is available.",
            true,
          )
        }
        const result = raced
        if (result.ok) {
          input.emit({
            kind: "child_completed",
            childRef,
            accountRef: result.accountRef,
            summary: bounded(redact(result.text), FABLE_LOCAL_SUMMARY_LIMIT),
            usage: childUsageToLedger(result.usage),
            durationMs: result.durationMs,
          })
          return toolText(`${result.text}\n\n${childUsageFooter(result)}`)
        }
        input.emit({
          kind: "child_failed",
          childRef,
          accountRef: result.accountRef,
          reason: result.reason,
          detail: bounded(redact(result.detail), FABLE_LOCAL_SUMMARY_LIMIT),
        })
        const failureText = result.reason === "account_reconnect_required"
          ? `Delegation unavailable: ${result.detail} No Codex child produced output.`
          : result.reason === "no_codex_account"
            ? "Delegation unavailable: no Codex account is registered on this machine."
            : result.reason === "child_timeout"
              ? `The Codex child timed out (${result.detail}).`
              : `The Codex child failed: ${result.detail}`
        return toolText(failureText, true)
      } finally {
        runningChildren.delete(childRef)
        delegation.active -= 1
      }
    }
    // MCP servers (I2 + Lane C): the internal `codex` delegate SDK-MCP server
    // (when a delegate is present) PLUS the enabled user-configured servers,
    // merged into one record. A record with zero servers stays null so the
    // SDK option is omitted entirely (no-delegate/no-user-servers behavior is
    // byte-for-byte unchanged).
    const serverRecord: Record<string, unknown> = {}
    if (options.delegate !== undefined) {
      try {
        const mcp = await loadMcp()
        serverRecord.codex = mcp.createSdkMcpServer({
            name: "codex",
            version: "1.0.0",
            tools: [
              mcp.tool(
                "delegate",
                FABLE_DELEGATE_TOOL_DESCRIPTION,
                mcp.delegateInputShape,
                (args, _extra) => delegateHandler(args),
              ),
            ],
          })
      } catch {
        // MCP construction failed: the lane still chats, with no delegate
        // tool offered (never offered-then-denied, never a silent stub).
        delete serverRecord.codex
      }
    }
    // Merge enabled user servers (I2). Their tools reach the model as
    // mcp__<name>__<tool> and flow through the allow-all canUseTool.
    for (const server of userMcp.valid) {
      serverRecord[server.name] = server.sdkConfig
    }
    const delegateOffered = Object.prototype.hasOwnProperty.call(serverRecord, "codex")
    const mcpServers: Record<string, unknown> | null =
      Object.keys(serverRecord).length > 0 ? serverRecord : null
    // I2: a configured server that failed bounded validation is surfaced now
    // (before the SDK runs) as a typed event — it never blocks the turn.
    for (const rejected of userMcp.invalid) {
      input.emit({
        kind: "mcp_server_unavailable",
        name: bounded(rejected.name, 120),
        reason: bounded(rejected.reason, FABLE_LOCAL_SUMMARY_LIMIT),
      })
    }

    const control = {
      interrupted: false,
      inner: null as AbortController | null,
      abort(): void {
        this.interrupted = true
        this.inner?.abort()
      },
    }
    activeTurns.set(input.turnRef, control)
    // A3: mark this thread as having a live turn so queueFollowup can enqueue.
    activeTurnByThread.set(input.threadRef, { turnRef: input.turnRef, emit: input.emit })

    let started = false
    const emitStarted = (): void => {
      if (started) return
      started = true
      input.emit({ kind: "turn_started" })
    }
    /**
     * Last init-reported model already surfaced for THIS turn. Turn-scoped
     * (not attempt-scoped): real sessions can emit more than one init, and a
     * rotated-away attempt's init already announced the model (both seen live
     * 2026-07-11) — an unchanged model is not re-announced.
     */
    let announcedModel: string | null = null

    /** One SDK session against one isolated account home. Emits stream events
     * but never turn_failed — the rotation loop below owns finalization. */
    const runAttempt = async (
      account: FableLocalAccountHome,
    ): Promise<Readonly<{ result: FableLocalTurnResult; sawContent: boolean }>> => {
      const selection: ResolvedPylonAccountSelection = {
        provider: "claude_agent",
        selector: "registry_ref",
        accountRef: account.ref,
        accountRefHash: hashPylonAccountRef("claude_agent", account.ref),
        home: account.home,
      }
      const abort = new AbortController()
      control.inner = abort
      if (control.interrupted) abort.abort()
      let timedOut = false
      // Pausable wall clock: waiting on the USER (a pending question) is not
      // model latency, so the turn budget is suspended while questions are
      // pending and resumes with the remaining allowance afterwards.
      const clock = {
        remainingMs: timeoutMs,
        startedAt: Date.now(),
        pauses: 0,
        timer: null as ReturnType<typeof setTimeout> | null,
      }
      const onTimeout = (): void => {
        timedOut = true
        abort.abort()
      }
      clock.timer = setTimeout(onTimeout, clock.remainingMs)
      const pauseTurnClock = (): void => {
        clock.pauses += 1
        if (clock.timer !== null) {
          clearTimeout(clock.timer)
          clock.timer = null
          clock.remainingMs = Math.max(clock.remainingMs - (Date.now() - clock.startedAt), 1_000)
        }
      }
      const resumeTurnClock = (): void => {
        clock.pauses = Math.max(0, clock.pauses - 1)
        if (clock.pauses === 0 && clock.timer === null && !timedOut && !abort.signal.aborted) {
          clock.startedAt = Date.now()
          clock.timer = setTimeout(onTimeout, clock.remainingMs)
        }
      }

      /**
       * AskUserQuestion answer path (EP250). Mechanism receipt: the SDK
       * routes AskUserQuestion through `canUseTool`; the host resolves it
       * with `{ behavior: "allow", updatedInput: { ...input, answers } }`
       * where `answers` maps question text -> selected label(s), multi-select
       * comma-separated (AskUserQuestionOutput.answers in sdk-tools.d.ts;
       * same flow as Anthropic's ask-user-question-previews demo).
       */
      const awaitUserAnswer = (
        rawInput: Record<string, unknown>,
        signal: AbortSignal | undefined,
      ): Promise<Record<string, unknown>> =>
        new Promise(resolveAnswer => {
          const parsed = parseAskUserQuestions(rawInput, redact)
          if (parsed === null) {
            resolveAnswer({
              behavior: "deny",
              message: "AskUserQuestion input was malformed; ask again with 1-4 questions, each with 2-4 labeled options.",
            })
            return
          }
          questionState.sequence += 1
          const questionRef = bounded(`q.${input.turnRef}.${questionState.sequence}`, 120)
          pauseTurnClock()
          let settled = false
          let questionTimer: ReturnType<typeof setTimeout> | null = null
          const settle = (
            outcome: "answered" | "timeout" | "denied",
            result: Record<string, unknown>,
          ): void => {
            if (settled) return
            settled = true
            pendingQuestions.delete(questionRef)
            if (questionTimer !== null) clearTimeout(questionTimer)
            if (signal !== undefined) signal.removeEventListener("abort", onQuestionAbort)
            resumeTurnClock()
            input.emit({ kind: "question_resolved", questionRef, outcome })
            resolveAnswer(result)
          }
          const onQuestionAbort = (): void =>
            settle("denied", { behavior: "deny", message: "The turn ended before the user answered." })
          questionTimer = setTimeout(
            () => settle("timeout", {
              behavior: "deny",
              message: "The user did not answer within the question window. Proceed without this input or finish the turn.",
            }),
            questionTimeoutMs,
          )
          pendingQuestions.set(questionRef, {
            turnRef: input.turnRef,
            accept: answers => {
              const record: Record<string, string> = {}
              for (const entry of answers) {
                const original = parsed.originalByEventText.get(entry.question)
                if (original === undefined) continue
                const labels = entry.labels.map(label => label.trim()).filter(label => label.length > 0)
                if (labels.length === 0) continue
                record[original] = labels.join(", ")
              }
              if (Object.keys(record).length === 0) return false
              settle("answered", {
                behavior: "allow",
                updatedInput: { ...rawInput, answers: record },
              })
              return true
            },
            denyForTurnEnd: onQuestionAbort,
          })
          if (signal?.aborted === true || control.interrupted) {
            onQuestionAbort()
            return
          }
          if (signal !== undefined) signal.addEventListener("abort", onQuestionAbort, { once: true })
          input.emit({ kind: "question_pending", questionRef, questions: parsed.eventQuestions })
        })

      /**
       * Every non-auto-allowed tool call lands here. AskUserQuestion parks on
       * the real question flow; EVERYTHING ELSE IS ALLOWED — the owner-local
       * full-tools/full-permissions override (statement verbatim 2026-07-11:
       * "disallowing bash is retarded, give them full tools full permissions
       * etc"). Mechanism receipt (sdk.d.ts): `bypassPermissions` would
       * "Bypass all permission checks" and so skip this handler entirely,
       * killing the question flow; `default` mode routes the permission
       * "ask" path through canUseTool ("The 'ask' path surfaces via a
       * can_use_tool control_request"), so allow-all here IS full
       * permissions while AskUserQuestion still parks. Applies to Agent
       * (subagent) tool calls too — can_use_tool mirrors agent_id for
       * subagent-originated calls.
       */
      const canUseTool = async (
        toolName: string,
        toolInput: Record<string, unknown>,
        extra: { signal?: AbortSignal } | undefined,
      ): Promise<Record<string, unknown>> => {
        if (toolName === FABLE_LOCAL_QUESTION_TOOL) {
          return awaitUserAnswer(toolInput, extra?.signal)
        }
        return { behavior: "allow", updatedInput: toolInput }
      }

      const continuity = sessionByThread.get(input.threadRef)
      const resumeSessionId =
        continuity !== undefined && continuity.accountRef === account.ref
          ? continuity.sessionId
          : undefined
      const promptText = resumeSessionId === undefined
        ? historyPrompt(input.history, input.message)
        : input.message
      // Capability I1: a turn with images cannot use a bare string prompt (the
      // SDK only carries an image in the content-block array of a streaming
      // user message). Switch to an AsyncIterable that yields exactly one
      // user message: the text block plus one base64 image block per
      // attachment. No images = the unchanged string prompt.
      const images = input.images ?? []
      const prompt: string | AsyncIterable<FableLocalSdkUserMessage> =
        images.length === 0 ? promptText : fableImagePromptStream(promptText, images)

      let sawContent = false
      let sessionId: string | null = null
      let deltaText = ""
      let assistantText = ""
      let resultText: string | null = null
      let resultIsError = false
      let resultSubtype: string | null = null
      let totalTokens: number | null = null
      let usageSplit: FableChildUsage | null = null
      const pendingToolCalls = new Map<string, string>()

      const finish = (
        result: FableLocalTurnResult,
      ): Readonly<{ result: FableLocalTurnResult; sawContent: boolean }> => {
        if (clock.timer !== null) clearTimeout(clock.timer)
        return { result, sawContent }
      }

      try {
        const session = query({
          prompt,
          options: {
            cwd: workspace,
            env: {
              ...pylonAccountEnvironment(env, selection),
              // Children run up to 240s; SDK MCP calls must not hit the CLI's
              // 60s stream-close default while a child is still working.
              ...(mcpServers === null
                ? {}
                : { CLAUDE_CODE_STREAM_CLOSE_TIMEOUT: String(FABLE_STREAM_CLOSE_TIMEOUT_MS) }),
            },
            abortController: abort,
            includePartialMessages: true,
            maxTurns: FABLE_LOCAL_MAX_TURNS,
            model: FABLE_LOCAL_MODEL,
            // Owner full-access lane, but NOT bypassPermissions: bypass
            // would "Bypass all permission checks" (sdk.d.ts) — including
            // the canUseTool handler AskUserQuestion parks on. Default mode
            // + allow-all canUseTool = full permissions with a live
            // question flow. Plan mode (J2, opt-in) switches to "plan"
            // ("Planning mode, no actual tool execution" — sdk.d.ts).
            permissionMode: planMode ? "plan" : "default",
            // No tool restriction (full SDK toolset). The delegate MCP tool
            // (when a delegate server is offered) stays auto-allowed;
            // everything else — including user MCP tools — flows through the
            // allow-all canUseTool below.
            ...(delegateOffered ? { allowedTools: [FABLE_DELEGATE_TOOL_NAME] } : {}),
            // Plan mode allows ExitPlanMode so the model can present a plan;
            // otherwise the interactive-only plan tools stay disallowed.
            disallowedTools: FABLE_LOCAL_DISALLOWED_TOOLS.filter(tool =>
              (planMode && tool === "ExitPlanMode") || (input.skillName !== undefined && tool === "Skill")
                ? false
                : true),
            // AskUserQuestion answers + allow-everything-else (owner
            // full-access override; no PreToolUse containment guard).
            canUseTool,
            skills: input.skillName === undefined ? [] : [input.skillName],
            settingSources: [],
            ...(options.userPlugins === undefined
              ? {}
              : { plugins: options.userPlugins().map(pluginPath => ({ type: "local" as const, path: pluginPath })) }),
            ...(mcpServers === null ? {} : { mcpServers }),
            ...(resumeSessionId === undefined ? {} : { resume: resumeSessionId }),
          },
        })
        for await (const message of session) {
          const record = message as SdkRecord
          const type = typeof record.type === "string" ? record.type : undefined
          if (type === "system" && record.subtype === "init") {
            if (typeof record.session_id === "string" && record.session_id.length > 0) {
              sessionId = record.session_id
            }
            emitStarted()
            // MODEL-LEVEL NO-SUBSTITUTION ("IT HAS TO BE FABLE"): the init
            // message reports the effective model. Emit it for renderer
            // visibility, then fail typed — before any content can stream —
            // if it is outside the Fable family. This is a provider-side
            // substitution, not an account failure, so it never rotates.
            const effectiveModel = typeof (record as { model?: unknown }).model === "string"
              ? (record as { model: string }).model
              : null
            if (effectiveModel !== null && effectiveModel.length > 0) {
              if (effectiveModel !== announcedModel) {
                announcedModel = effectiveModel
                input.emit({ kind: "model_effective", model: bounded(effectiveModel, 120) })
              }
              if (!effectiveModel.startsWith(FABLE_LOCAL_MODEL_FAMILY_PREFIX)) {
                abort.abort()
                return finish(failure(
                  "model_substituted",
                  `requested ${FABLE_LOCAL_MODEL}, effective ${effectiveModel}`,
                ))
              }
            }
            // I2: the SDK init reports each MCP server's connection status.
            // Surface a typed mcp_server_unavailable for any USER server that
            // did not connect (never the internal `codex` delegate). The turn
            // keeps running — a failed server never aborts it.
            const initServers = (record as { mcp_servers?: unknown }).mcp_servers
            if (Array.isArray(initServers)) {
              for (const entry of initServers) {
                if (entry === null || typeof entry !== "object") continue
                const name = typeof (entry as { name?: unknown }).name === "string"
                  ? (entry as { name: string }).name
                  : ""
                const status = typeof (entry as { status?: unknown }).status === "string"
                  ? (entry as { status: string }).status
                  : ""
                if (!userServerNames.has(name)) continue
                if (status === "connected" || status === "pending") continue
                if (announcedMcpUnavailable.has(name)) continue
                announcedMcpUnavailable.add(name)
                input.emit({
                  kind: "mcp_server_unavailable",
                  name: bounded(name, 120),
                  reason: bounded(status === "" ? "server failed to connect" : status, FABLE_LOCAL_SUMMARY_LIMIT),
                })
              }
            }
            continue
          }
          if (type === "stream_event") {
            const event = record.event as Record<string, unknown> | undefined
            const delta = event?.delta as Record<string, unknown> | undefined
            if (event?.type === "content_block_delta" && delta?.type === "text_delta" &&
              typeof delta.text === "string" && delta.text.length > 0) {
              emitStarted()
              sawContent = true
              const text = bounded(redact(delta.text), FABLE_LOCAL_DELTA_LIMIT)
              deltaText = bounded(deltaText + text, FABLE_LOCAL_FINAL_TEXT_LIMIT)
              input.emit({ kind: "text_delta", text })
            }
            continue
          }
          if (type === "assistant") {
            emitStarted()
            for (const block of contentBlocks(record.message)) {
              if (block.type === "text" && typeof block.text === "string") {
                assistantText = bounded(assistantText + block.text, FABLE_LOCAL_FINAL_TEXT_LIMIT)
                continue
              }
              if (block.type === "tool_use") {
                sawContent = true
                const toolCallId = typeof block.id === "string" ? block.id : `${pendingToolCalls.size}`
                const toolName = bounded(typeof block.name === "string" ? block.name : "tool", 120)
                pendingToolCalls.set(toolCallId, toolName)
                const summary = bounded(
                  redact(JSON.stringify(block.input ?? {}) ?? ""),
                  FABLE_LOCAL_SUMMARY_LIMIT,
                )
                input.emit({ kind: "tool_use", toolName, summary })
                // J2/J4: the TodoWrite tool carries the current plan/todo list.
                // Emit the structured plan_updated ADDITIONALLY (the tool_use
                // trace above is unchanged) so a renderer can draw progress.
                if (toolName === "TodoWrite") {
                  const entries = planEntriesFromTodoWrite(block.input)
                  if (entries !== null) input.emit({ kind: "plan_updated", entries })
                }
              }
            }
            continue
          }
          if (type === "user") {
            for (const block of contentBlocks(record.message)) {
              if (block.type !== "tool_result") continue
              const toolCallId = typeof block.tool_use_id === "string" ? block.tool_use_id : ""
              const toolName = pendingToolCalls.get(toolCallId) ?? "tool"
              const ok = block.is_error !== true
              const content = typeof block.content === "string"
                ? block.content
                : contentBlocks(block).map(part =>
                    typeof part.text === "string" ? part.text : "").join(" ")
              sawContent = true
              input.emit({
                kind: "tool_result",
                toolName,
                ok,
                summary: bounded(redact(content), FABLE_LOCAL_SUMMARY_LIMIT),
              })
            }
            continue
          }
          if (type === "result") {
            resultSubtype = typeof record.subtype === "string" ? record.subtype : null
            resultIsError = record.is_error === true
            if (typeof record.result === "string" && record.result.length > 0) {
              resultText = bounded(redact(record.result), FABLE_LOCAL_FINAL_TEXT_LIMIT)
            }
            totalTokens = usageTotalTokens(record.usage)
            usageSplit = usageSplitFromResult(record.usage)
          }
        }
      } catch (error) {
        if (timedOut) return finish(failure("timeout", "wall clock budget reached"))
        if (abort.signal.aborted) return finish(failure("interrupted", "turn interrupted"))
        return finish(failure(
          "session_failed",
          redact(error instanceof Error ? error.message : "unknown session error"),
        ))
      }
      if (timedOut) return finish(failure("timeout", "wall clock budget reached"))
      if (abort.signal.aborted) return finish(failure("interrupted", "turn interrupted"))
      if (resultSubtype !== null && resultSubtype.includes("max_turns")) {
        return finish(failure("budget_exceeded", "turn budget reached"))
      }
      if (resultIsError || (resultSubtype !== null && resultSubtype.startsWith("error"))) {
        return finish(failure("session_failed", resultSubtype ?? "provider error"))
      }
      // The final text authority order: the SDK result text, then complete
      // assistant blocks, then accumulated stream deltas — so a build that
      // skips partial events still yields the full reply.
      const text = resultText ?? (assistantText.length > 0 ? assistantText : deltaText)
      if (text.trim() === "") return finish(failure("session_failed", "the session produced no assistant text"))
      if (sessionId !== null) {
        sessionByThread.set(input.threadRef, { sessionId, accountRef: account.ref })
      }
      input.emit({
        kind: "turn_completed",
        totalTokens,
        accountRef: account.ref,
        ...(usageSplit === null ? {} : { usage: usageSplit }),
      })
      return finish({
        ok: true,
        text,
        totalTokens,
        accountRef: account.ref,
        ...(usageSplit === null ? {} : { usage: usageSplit }),
      })
    }

    try {
      let last = failure("session_failed", "no account attempt ran")
      for (let index = 0; index < ready.length; index += 1) {
        const attempt = await runAttempt(ready[index]!)
        const result = attempt.result
        if (result.ok) return result
        last = result
        // Account rotation (mirrors the Pylon supervisor): a session that
        // failed BEFORE producing any content is an account/session-level
        // refusal (e.g. an org with Claude Code access disabled — seen live
        // on this machine's first sibling home), so the next ready isolated
        // Claude home gets the turn. Same lane, same provider — never a
        // silent substitution. Once content streamed, no rerun: a partial
        // reply must fail honestly rather than double-answer.
        const rotatable = result.reason === "session_failed" &&
          !attempt.sawContent &&
          !control.interrupted
        if (!rotatable || index === ready.length - 1) return emitFailure(result)
      }
      return emitFailure(last)
    } finally {
      // Turn end denies any question still pending for THIS turn (typed
      // outcome "denied") so no canUseTool promise dangles past the turn.
      for (const pending of [...pendingQuestions.values()]) {
        if (pending.turnRef === input.turnRef) pending.denyForTurnEnd()
      }
      activeTurns.delete(input.turnRef)
      // A3: this thread is idle now — clear its active-turn marker (only if it
      // is still this turn) and promote the next queued follow-up, if any, so
      // the host can start it as the next turn (queue-until-idle semantics).
      const current = activeTurnByThread.get(input.threadRef)
      if (current !== undefined && current.turnRef === input.turnRef) {
        activeTurnByThread.delete(input.threadRef)
      }
      const queue = followupQueue.get(input.threadRef)
      if (queue !== undefined && queue.length > 0) {
        const next = queue.shift()!
        if (queue.length === 0) followupQueue.delete(input.threadRef)
        input.emit({
          kind: "followup_promoted",
          queueRef: next.queueRef,
          message: bounded(next.message, FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT),
        })
      }
    }
  }

  return {
    availability,
    runTurn,
    interrupt: turnRef => {
      const active = activeTurns.get(turnRef)
      if (active === undefined) return false
      // Deny this turn's pending questions FIRST so a session parked on the
      // canUseTool promise unwinds instead of dangling past the abort.
      for (const pending of [...pendingQuestions.values()]) {
        if (pending.turnRef === turnRef) pending.denyForTurnEnd()
      }
      // G4: a whole-turn interrupt also signals every delegate child of the
      // turn (the SDK query abort does not reach the Codex child processes).
      for (const child of runningChildren.values()) {
        if (child.turnRef === turnRef) child.controller.abort()
      }
      active.abort()
      return true
    },
    answerQuestion: request => {
      const pending = pendingQuestions.get(request.questionRef)
      if (pending === undefined || pending.turnRef !== request.turnRef) return false
      return pending.accept(request.answers)
    },
    steerChild: request => {
      const child = runningChildren.get(request.childRef)
      if (child === undefined || child.turnRef !== request.turnRef) {
        return { ok: false, outcome: "not_found" }
      }
      if (request.action === "interrupt") {
        child.controller.abort()
        child.emit({
          kind: "child_steered",
          childRef: request.childRef,
          action: "interrupt",
          outcome: "interrupted",
          detail: "child interrupt requested",
        })
        return { ok: true, outcome: "interrupted" }
      }
      // action "message": honest capability truth. `codex exec --json` is
      // non-interactive (no mid-flight stdin), and the SDK Agent tool exposes
      // no per-subagent message API — only whole-query interrupt/stopTask. So
      // steering a running child by message is unsupported today.
      child.emit({
        kind: "child_steered",
        childRef: request.childRef,
        action: "message",
        outcome: "unsupported",
        detail: "Running Codex/Agent children cannot receive a mid-flight message; interrupt is the only supported control.",
      })
      return { ok: true, outcome: "unsupported" }
    },
    queueFollowup: request => {
      const active = activeTurnByThread.get(request.threadRef)
      // No live turn for this thread → not a "queue during a turn"; the caller
      // should just start a normal turn with this message.
      if (active === undefined) return { ok: false, queued: false, reason: "no_active_turn" }
      followupSequence += 1
      const queueRef = bounded(
        `followup.${fableThreadWorkspaceSlug(request.threadRef)}.${followupSequence}`,
        120,
      )
      const message = bounded(request.message, FABLE_LOCAL_FOLLOWUP_MESSAGE_LIMIT)
      const queue = followupQueue.get(request.threadRef) ?? []
      queue.push({ queueRef, message })
      followupQueue.set(request.threadRef, queue)
      active.emit({ kind: "followup_queued", queueRef, position: queue.length })
      return { ok: true, queued: true, queueRef, position: queue.length }
    },
    dispose: () => {
      for (const pending of [...pendingQuestions.values()]) pending.denyForTurnEnd()
      pendingQuestions.clear()
      for (const child of runningChildren.values()) child.controller.abort()
      runningChildren.clear()
      for (const active of activeTurns.values()) active.abort()
      activeTurns.clear()
      activeTurnByThread.clear()
      followupQueue.clear()
      sessionByThread.clear()
    },
  }
}

const sleep = (ms: number): Promise<void> => new Promise(resolve => setTimeout(resolve, ms))

/**
 * Fixture reply carries markdown (`**streaming**`) split MID-MARKER across
 * deltas so the smoke journey proves both markdown rendering and graceful
 * unterminated-marker streaming (#8712 owner directive 4).
 */
export const FABLE_LOCAL_FIXTURE_TEXT = "Fable local **streaming** proof."
export const FABLE_LOCAL_FIXTURE_ACCOUNT: FableLocalAccountHome = {
  ref: "claude-pylon-fixture",
  home: "/nonexistent/fable-local-fixture-home",
}

/**
 * Fixture MCP factory (tests + smoke): produces PLAIN, introspectable server
 * objects instead of real SDK McpServer instances, so the fixture query below
 * can find the delegate tool handler and CALL it — driving the REAL delegate
 * path (caps, child runtime, child events, ledger feed) with zero SDK import.
 */
export type FixtureFableMcpTool = Readonly<{
  name: string
  description: string
  inputSchema: Record<string, unknown>
  handler: (args: Record<string, unknown>, extra: unknown) => Promise<unknown>
}>

export const makeFixtureFableMcpFactory = (): FableSdkMcpFactory => ({
  createSdkMcpServer: options => ({
    type: "sdk-fixture",
    name: options.name,
    version: options.version ?? "0.0.0",
    tools: options.tools,
  }),
  tool: (name, description, inputSchema, handler) =>
    ({ name, description, inputSchema, handler }) satisfies FixtureFableMcpTool,
  delegateInputShape: { task: "string", context: "string?" },
})

/** Finds the fixture-shaped delegate tool inside session options, if any. */
const fixtureDelegateTool = (options: Record<string, unknown>): FixtureFableMcpTool | null => {
  const servers = options.mcpServers as Record<string, unknown> | undefined
  const codex = servers?.codex as { type?: unknown; tools?: unknown } | undefined
  if (codex?.type !== "sdk-fixture" || !Array.isArray(codex.tools)) return null
  const tool = codex.tools.find(candidate =>
    typeof candidate === "object" && candidate !== null &&
    (candidate as { name?: unknown }).name === "delegate" &&
    typeof (candidate as { handler?: unknown }).handler === "function")
  return (tool as FixtureFableMcpTool | undefined) ?? null
}

export const FABLE_FIXTURE_DELEGATE_TASK = "Summarize the fixture delegation task"

/**
 * Scripted smoke fixture (OPENAGENTS_DESKTOP_SMOKE=1): a canned SDK message
 * sequence — init, spaced text deltas, one read-only tool round trip, and a
 * success result — driven through the REAL event mapping above. When the
 * session options carry the FIXTURE MCP delegate tool, the fixture also
 * invokes the REAL delegate handler once and replays its result as a
 * tool_use/tool_result pair (the smoke's deterministic delegation proof).
 * Never used in normal runs; main.ts logs when it is active.
 */
/** Marker the image-aware fixture appends so the smoke can prove the image
 * content block reached the SDK query input (capability I1). */
export const FABLE_LOCAL_FIXTURE_IMAGE_MARKER = (count: number): string =>
  ` [fixture-received-images:${count}]`

export const makeFixtureFableLocalQuery = (): FableLocalQuery =>
  async function* fixture(input): AsyncGenerator<unknown> {
    // Capability I1: when the turn carries images the prompt is a streaming
    // AsyncIterable of user messages (not a string). Draining it — exactly
    // what the real SDK does with streaming input — lets the fixture count the
    // image content blocks and echo the count in its result, so the smoke can
    // assert the image actually reached the query payload.
    let imageCount = 0
    const prompt: unknown = input.prompt
    if (
      typeof prompt !== "string" && prompt !== null && prompt !== undefined &&
      typeof (prompt as AsyncIterable<unknown>)[Symbol.asyncIterator] === "function"
    ) {
      for await (const message of prompt as AsyncIterable<FableLocalSdkUserMessage>) {
        const content = message?.message?.content
        if (Array.isArray(content)) {
          imageCount += content.filter((block) => block?.type === "image").length
        }
      }
    }
    yield {
      type: "system",
      subtype: "init",
      session_id: "fable-local-fixture-session",
      model: FABLE_LOCAL_MODEL,
    }
    yield {
      type: "stream_event",
      event: { type: "content_block_delta", delta: { type: "text_delta", text: "Fable local " } },
    }
    await sleep(150)
    yield {
      type: "stream_event",
      event: { type: "content_block_delta", delta: { type: "text_delta", text: "**streaming" } },
    }
    await sleep(150)
    yield {
      type: "assistant",
      message: {
        content: [{ type: "tool_use", id: "fixture-tool-1", name: "Read", input: { file_path: "notes.md" } }],
      },
    }
    yield {
      type: "user",
      message: {
        content: [{ type: "tool_result", tool_use_id: "fixture-tool-1", content: "bounded fixture read" }],
      },
    }
    // Plan/todo progress rung (EP250 wave-2 J2/J4): a TodoWrite tool call emits
    // `plan_updated` ADDITIONALLY to its tool_use, so the renderer draws the
    // compact task-progress card (status glyphs from the exact enum) that the
    // wave-2 smoke asserts.
    yield {
      type: "assistant",
      message: {
        content: [{
          type: "tool_use",
          id: "fixture-todo-1",
          name: "TodoWrite",
          input: {
            todos: [
              { content: "Read the fixture notes", status: "completed", activeForm: "Reading the fixture notes" },
              { content: "Summarize for the user", status: "in_progress", activeForm: "Summarizing for the user" },
            ],
          },
        }],
      },
    }
    yield {
      type: "user",
      message: {
        content: [{ type: "tool_result", tool_use_id: "fixture-todo-1", content: "todos updated" }],
      },
    }
    // Deterministic delegation rung: call the REAL mcp__codex__delegate
    // handler (scripted Codex child behind it) and replay its CallToolResult
    // through the same assistant/user mapping a live SDK session uses.
    const delegate = fixtureDelegateTool(input.options)
    if (delegate !== null) {
      yield {
        type: "assistant",
        message: {
          content: [{
            type: "tool_use",
            id: "fixture-delegate-1",
            name: FABLE_DELEGATE_TOOL_NAME,
            input: { task: FABLE_FIXTURE_DELEGATE_TASK },
          }],
        },
      }
      const raw = await delegate.handler({ task: FABLE_FIXTURE_DELEGATE_TASK }, {})
      const record = raw as { content?: Array<{ type?: unknown; text?: unknown }>; isError?: unknown }
      const text = Array.isArray(record.content)
        ? record.content.map(part => typeof part.text === "string" ? part.text : "").join(" ")
        : ""
      yield {
        type: "user",
        message: {
          content: [{
            type: "tool_result",
            tool_use_id: "fixture-delegate-1",
            is_error: record.isError === true,
            content: text,
          }],
        },
      }
    }
    await sleep(150)
    yield {
      type: "stream_event",
      event: { type: "content_block_delta", delta: { type: "text_delta", text: "** proof." } },
    }
    yield {
      type: "result",
      subtype: "success",
      is_error: false,
      result: imageCount > 0
        ? FABLE_LOCAL_FIXTURE_TEXT + FABLE_LOCAL_FIXTURE_IMAGE_MARKER(imageCount)
        : FABLE_LOCAL_FIXTURE_TEXT,
      usage: { input_tokens: 42, output_tokens: 7 },
    }
  }
