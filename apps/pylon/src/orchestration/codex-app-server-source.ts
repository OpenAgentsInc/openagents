/**
 * CUT-11 (#8691): the typed Codex app-server event source.
 *
 * The named-account probes recorded on #8691 located the Codex child-activity
 * gap precisely: the real `codex app-server` emits typed `subAgentActivity`
 * and receiver-bearing `collabAgentToolCall` thread items (the ONLY honest
 * Codex parentage source), while `codex exec --experimental-json` — the
 * transport behind the installed `@openai/codex-sdk` — drops
 * `subAgentActivity` and emits a receiver-less `collab_tool_call`. Per the
 * Pylon streamlining audit, the fix is convergence on the typed app-server
 * source through the ONE conversation-service translation
 * (`codexRawEventToRuntimeEvents`) — not tool-name/history inference and not
 * another provider sidecar.
 *
 * This module is that source: a minimal JSONL JSON-RPC 2.0 client for
 * `codex app-server` (stdio transport; the `"jsonrpc"` header is omitted on
 * the wire, matching the server) that adapts v2 notifications into the SAME
 * `CodexRawEvent` stream shape the exec encoder produces — plus the typed
 * child items the exec encoder drops — so the downstream translation seam is
 * unchanged:
 *
 * - `thread/start` / `thread/resume` response  -> `thread.started`
 * - `turn/started`                             -> `turn.started`
 * - `item/completed`                           -> `item.completed` (v2 item forwarded intact)
 * - `thread/tokenUsage/updated`                -> remembered for the terminal usage record
 * - `turn/completed` (status `completed`)      -> `turn.completed` with exec-shaped usage
 * - `turn/completed` (failed/interrupted)      -> `turn.failed`
 *
 * Sandbox/approval posture matches the owner-local executor invariant the
 * SDK path already uses: `approvalPolicy: "never"`, sandbox
 * `danger-full-access` (`runWithRealCodexSdk`'s constants). Any unexpected
 * server->client request is answered with a JSON-RPC error (fail-closed) so
 * the connection can never hang on an approval this posture should not
 * receive.
 *
 * Failure honesty: failures BEFORE any event frame (spawn, initialize,
 * thread/start, or a typed JSON-RPC error on turn/start — cases where the
 * model provably did not start) throw `CodexAppServerPreFrameError`, which
 * the composed default runner in `runtime-intent-enforcement.ts` uses to
 * fall back to the exec SDK runner exactly once. This mirrors the probe
 * finding (one bundled binary failed pre-frame while the PATH binary
 * worked). After the stream starts, failures surface as `turn.failed` /
 * thrown stream errors — never a second execution.
 */
import { spawn } from "node:child_process"
import { createInterface } from "node:readline"

/** Raw event shape consumed by `codexRawEventToRuntimeEvents` (type-only). */
import type { CodexRawEvent, RuntimeCodexThreadRunner } from "./runtime-intent-enforcement.js"

/** Environment override for the app-server executable; PATH `codex` otherwise. */
export const CODEX_APP_SERVER_BIN_ENV = "PYLON_CODEX_APP_SERVER_BIN"

const HANDSHAKE_TIMEOUT_MS = 30_000

export class CodexAppServerPreFrameError extends Error {
  constructor(message: string) {
    super(message)
    this.name = "CodexAppServerPreFrameError"
  }
}

export const isCodexAppServerPreFrameFailure = (error: unknown): boolean =>
  error instanceof CodexAppServerPreFrameError

export type CodexAppServerProcess = {
  readonly stdout: NodeJS.ReadableStream | null
  readonly stdin: { write: (chunk: string) => unknown } | null
  kill: (signal?: NodeJS.Signals) => unknown
  on: (event: string, listener: (...args: Array<unknown>) => void) => unknown
}

export type CodexAppServerSpawn = (input: {
  readonly executable: string
  readonly args: ReadonlyArray<string>
  readonly cwd: string
  readonly env: Record<string, string | undefined>
}) => CodexAppServerProcess

const defaultSpawn: CodexAppServerSpawn = (input) =>
  spawn(input.executable, [...input.args], {
    cwd: input.cwd,
    env: input.env as NodeJS.ProcessEnv,
    stdio: ["pipe", "pipe", "ignore"],
  }) as unknown as CodexAppServerProcess

type JsonRecord = Record<string, unknown>

/** Bounded async event queue bridging notifications to the runner stream. */
const makeEventQueue = () => {
  const buffered: Array<CodexRawEvent> = []
  let waiting: { resolve: (r: IteratorResult<CodexRawEvent>) => void; reject: (e: unknown) => void } | null = null
  let ended = false
  let failure: unknown = null
  const push = (event: CodexRawEvent): void => {
    if (ended) return
    if (waiting !== null) {
      const w = waiting
      waiting = null
      w.resolve({ done: false, value: event })
      return
    }
    buffered.push(event)
  }
  const end = (): void => {
    if (ended) return
    ended = true
    if (waiting !== null) {
      const w = waiting
      waiting = null
      w.resolve({ done: true, value: undefined })
    }
  }
  const fail = (error: unknown): void => {
    if (ended) return
    ended = true
    failure = error
    if (waiting !== null) {
      const w = waiting
      waiting = null
      w.reject(error)
    }
  }
  const iterable: AsyncIterable<CodexRawEvent> = {
    [Symbol.asyncIterator]: () => ({
      next: (): Promise<IteratorResult<CodexRawEvent>> => {
        if (buffered.length > 0) return Promise.resolve({ done: false, value: buffered.shift()! })
        if (failure !== null) return Promise.reject(failure)
        if (ended) return Promise.resolve({ done: true, value: undefined })
        return new Promise((resolve, reject) => {
          waiting = { resolve, reject }
        })
      },
    }),
  }
  return { push, end, fail, iterable }
}

export type CodexAppServerRunnerOptions = Readonly<{
  spawnImpl?: CodexAppServerSpawn
  handshakeTimeoutMs?: number
  log?: (line: string) => void
}>

/**
 * Builds the app-server-backed `RuntimeCodexThreadRunner`. The returned
 * runner has the exact signature/semantics of `runWithRealCodexSdk`, so the
 * dispatch call site and the translation seam do not change.
 */
export const makeCodexAppServerRunner = (
  options: CodexAppServerRunnerOptions = {},
): RuntimeCodexThreadRunner => async (input) => {
  const spawnImpl = options.spawnImpl ?? defaultSpawn
  const timeoutMs = options.handshakeTimeoutMs ?? HANDSHAKE_TIMEOUT_MS
  const executable = input.env[CODEX_APP_SERVER_BIN_ENV] ?? "codex"

  let child: CodexAppServerProcess
  try {
    child = spawnImpl({ args: ["app-server"], cwd: input.cwd, env: input.env, executable })
  } catch (error) {
    throw new CodexAppServerPreFrameError(
      `codex app-server spawn failed: ${error instanceof Error ? error.message : "unknown"}`,
    )
  }
  if (child.stdout === null || child.stdin === null) {
    child.kill()
    throw new CodexAppServerPreFrameError("codex app-server spawned without stdio pipes")
  }

  const queue = makeEventQueue()
  const pending = new Map<number, { resolve: (value: JsonRecord) => void; reject: (error: unknown) => void }>()
  let nextRequestId = 0
  let exited = false
  let streamStarted = false
  let terminalPushed = false
  /** Newest per-turn usage from thread/tokenUsage/updated (exec-shaped). */
  let lastUsage: JsonRecord | null = null
  let lastErrorDetail: string | null = null

  const send = (message: JsonRecord): void => {
    try {
      child.stdin?.write(`${JSON.stringify(message)}\n`)
    } catch {
      // Write failures surface via the exit handler.
    }
  }

  const request = (method: string, params: JsonRecord): Promise<JsonRecord> => {
    nextRequestId += 1
    const id = nextRequestId
    return new Promise<JsonRecord>((resolve, reject) => {
      pending.set(id, { reject, resolve })
      send({ id, method, params })
    })
  }

  const requestWithTimeout = async (method: string, params: JsonRecord): Promise<JsonRecord> => {
    let timer: ReturnType<typeof setTimeout> | null = null
    try {
      return await Promise.race([
        request(method, params),
        new Promise<never>((_resolve, reject) => {
          timer = setTimeout(() => reject(new Error(`${method} timed out after ${String(timeoutMs)}ms`)), timeoutMs)
        }),
      ])
    } finally {
      if (timer !== null) clearTimeout(timer)
    }
  }

  const settle = (): void => {
    terminalPushed = true
    queue.end()
    try {
      child.kill()
    } catch {
      // Already gone.
    }
  }

  const execShapedUsage = (): JsonRecord => {
    const last = (lastUsage?.last ?? null) as JsonRecord | null
    const integer = (value: unknown): number =>
      typeof value === "number" && Number.isFinite(value) ? Math.max(0, Math.trunc(value)) : 0
    const reasoning = integer(last?.reasoningOutputTokens)
    return {
      cached_input_tokens: integer(last?.cachedInputTokens),
      input_tokens: integer(last?.inputTokens),
      // The exec encoder reports output and reasoning separately and the
      // translation sums them; v2's outputTokens already includes reasoning
      // output, so the non-reasoning remainder is reported as output.
      output_tokens: Math.max(0, integer(last?.outputTokens) - reasoning),
      reasoning_output_tokens: reasoning,
    }
  }

  const handleNotification = (method: string, params: JsonRecord): void => {
    if (method === "turn/started") {
      queue.push({ type: "turn.started" })
      return
    }
    if (method === "item/completed") {
      const item = params.item
      if (item !== null && typeof item === "object") {
        queue.push({ item, type: "item.completed" })
      }
      return
    }
    if (method === "thread/tokenUsage/updated") {
      const usage = params.tokenUsage
      if (usage !== null && typeof usage === "object") lastUsage = usage as JsonRecord
      return
    }
    if (method === "error") {
      const error = params.error as JsonRecord | undefined
      if (params.willRetry !== true && typeof error?.message === "string") {
        lastErrorDetail = error.message
      }
      return
    }
    if (method === "turn/completed") {
      const turn = params.turn as JsonRecord | undefined
      const status = turn?.status
      if (status === "completed") {
        queue.push({ type: "turn.completed", usage: execShapedUsage() })
      } else {
        const turnError = turn?.error as JsonRecord | undefined
        queue.push({
          error: typeof turnError?.message === "string" ? turnError.message : lastErrorDetail ?? "turn failed",
          type: "turn.failed",
        })
      }
      settle()
    }
  }

  const handleLine = (line: string): void => {
    const trimmed = line.trim()
    if (trimmed === "") return
    let message: JsonRecord
    try {
      message = JSON.parse(trimmed) as JsonRecord
    } catch {
      return
    }
    const id = message.id
    const method = message.method
    if (typeof method === "string" && typeof id === "number") {
      // Server -> client request. The owner-local posture (approvalPolicy
      // never, danger-full-access) should produce none; refuse fail-closed
      // so the connection can never hang on an unanswerable elicitation.
      send({ error: { code: -32601, message: `unsupported client method: ${method}` }, id })
      return
    }
    if (typeof method === "string") {
      handleNotification(method, message.params !== null && typeof message.params === "object" ? message.params as JsonRecord : {})
      return
    }
    if (typeof id === "number") {
      const waiter = pending.get(id)
      if (waiter === undefined) return
      pending.delete(id)
      const errorValue = message.error as JsonRecord | undefined
      if (errorValue !== undefined && errorValue !== null) {
        waiter.reject(new Error(typeof errorValue.message === "string" ? errorValue.message : "app-server request failed"))
        return
      }
      waiter.resolve(message.result !== null && typeof message.result === "object" ? message.result as JsonRecord : {})
    }
  }

  const reader = createInterface({ input: child.stdout })
  reader.on("line", handleLine)

  child.on("exit", () => {
    exited = true
    for (const waiter of pending.values()) {
      waiter.reject(new Error("codex app-server exited during a pending request"))
    }
    pending.clear()
    if (!terminalPushed) {
      if (streamStarted) {
        queue.push({ error: lastErrorDetail ?? "codex app-server exited before the turn completed", type: "error" })
        queue.end()
      }
      // Pre-stream exits reject via the pending-request rejections above.
    }
  })
  child.on("error", () => {
    // Surfaced through pending rejections / exit handling.
  })

  const abort = (): void => {
    if (terminalPushed || exited) return
    // Best-effort typed interrupt, then hard stop. The stream fails with an
    // abort error to mirror the SDK runner's abort behavior.
    void request("turn/interrupt", { threadId: startedThreadId ?? "" }).catch(() => {})
    const abortError = new Error("aborted")
    abortError.name = "AbortError"
    queue.fail(abortError)
    terminalPushed = true
    setTimeout(() => {
      try {
        child.kill()
      } catch {
        // Already gone.
      }
    }, 1_000)
  }
  let startedThreadId: string | null = null

  try {
    await requestWithTimeout("initialize", {
      clientInfo: { name: "openagents_pylon", title: "OpenAgents Pylon", version: "1.0.0" },
    })
    send({ method: "initialized" })
    const threadResult = input.resumeThreadId === undefined
      ? await requestWithTimeout("thread/start", {})
      : await requestWithTimeout("thread/resume", { threadId: input.resumeThreadId })
    const thread = threadResult.thread as JsonRecord | undefined
    const threadId = typeof thread?.id === "string" && thread.id.length > 0 ? thread.id : null
    if (threadId === null) {
      throw new Error("thread/start returned no thread id")
    }
    startedThreadId = threadId
    // turn/start: same owner-local posture as runWithRealCodexSdk — the SDK
    // equivalent of --dangerously-bypass-approvals-and-sandbox with network
    // enabled (owner-local executor invariant, never a public wire field).
    await requestWithTimeout("turn/start", {
      approvalPolicy: "never",
      cwd: input.cwd,
      input: [{ text: input.instructions, type: "text" }],
      // Owner-local dispatch always runs full access with network enabled
      // (the dispatch call site pins networkAccessEnabled: true); mirrors
      // CODEX_AGENT_OWNER_LOCAL_SANDBOX_MODE on the SDK path.
      sandboxPolicy: { type: "danger-full-access" },
      threadId,
      ...(input.model === undefined ? {} : { model: input.model }),
    })
  } catch (error) {
    try {
      child.kill()
    } catch {
      // Already gone.
    }
    const detail = error instanceof Error ? error.message : "unknown"
    if (detail.includes("timed out after")) {
      // A handshake timeout is NOT proven-not-started for turn/start, but
      // initialize/thread timeouts are; either way no event frame exists.
      // turn/start timeouts stay fatal (no fallback) to rule out double
      // execution — only typed errors and pre-turn failures fall back.
      if (detail.startsWith("turn/start")) {
        throw new Error(`codex app-server turn/start did not respond: ${detail}`)
      }
      throw new CodexAppServerPreFrameError(detail)
    }
    throw new CodexAppServerPreFrameError(`codex app-server handshake failed: ${detail}`)
  }

  streamStarted = true
  queue.push({ thread_id: startedThreadId, type: "thread.started" })
  if (input.signal.aborted) {
    abort()
  } else {
    input.signal.addEventListener("abort", abort, { once: true })
  }
  return { events: queue.iterable }
}
