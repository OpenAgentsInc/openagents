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
 * Safety bounds: the session cwd is a scratch directory under the app's
 * userData (never a repo), `permissionMode: "default"` with an explicit
 * read-only `allowedTools` set (Read/Glob/Grep — no Bash, no Write/Edit, no
 * WebSearch) and `settingSources: []`. In headless stream-json mode a
 * non-allowed tool is auto-denied by the CLI and surfaces here as a visible
 * `tool_result` trace event — a denied tool is evidence, not a hang. Every
 * emitted payload is bounded and path-redacted.
 *
 * Multi-turn: the runtime keeps an in-memory threadRef -> SDK session map and
 * resumes the session (`options.resume`) when this process already ran a turn
 * for the thread; otherwise it prepends bounded thread history to the prompt.
 *
 * This module never imports `electron` (unit-testable under `bun test`); the
 * IPC wiring lives in main.ts.
 */
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
  FABLE_LOCAL_SUMMARY_LIMIT,
  type FableLocalAvailability,
  type FableLocalEvent,
  type FableLocalFailureReason,
} from "./fable-local-contract.ts"

const CLAUDE_AGENT_SDK_PACKAGE = "@anthropic-ai/claude-agent-sdk"
/** Read-only chat-turn tool set: no Bash, no Write/Edit, no WebSearch. */
export const FABLE_LOCAL_ALLOWED_TOOLS = ["Read", "Glob", "Grep"] as const
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
 * Skills are removed from this chat lane entirely: `disallowedTools: ["Skill"]`
 * strips the Skill tool from the model's context (never offered, not
 * offered-then-denied) and `skills: []` enables no skills, so bundled skill
 * listings never auto-trigger against the read-only whitelist.
 */
export const FABLE_LOCAL_DISALLOWED_TOOLS = ["Skill"] as const
export const FABLE_LOCAL_MAX_TURNS = 16
export const FABLE_LOCAL_TIMEOUT_MS = 180_000
/** Bounded history window prepended when no resumable session exists. */
export const FABLE_LOCAL_HISTORY_MESSAGES = 12
export const FABLE_LOCAL_HISTORY_MESSAGE_LIMIT = 2_000

export type FableLocalQuery = (input: {
  prompt: string
  options: Record<string, unknown>
}) => AsyncIterable<unknown>

export type FableLocalHistoryMessage = Readonly<{
  role: "user" | "assistant" | "system"
  text: string
}>

export type FableLocalTurnInput = Readonly<{
  turnRef: string
  threadRef: string
  history: ReadonlyArray<FableLocalHistoryMessage>
  message: string
  emit: (event: FableLocalEvent) => void
}>

export type FableLocalTurnResult =
  /** `accountRef` names the isolated account home that ran the turn
   * (additive, #8712 message-metadata inspector) — a ref, never a path. */
  | Readonly<{ ok: true; text: string; totalTokens: number | null; accountRef: string }>
  | Readonly<{ ok: false; reason: FableLocalFailureReason; detail: string }>

export type FableLocalAccountHome = Readonly<{ ref: string; home: string }>

export type FableLocalRuntimeOptions = Readonly<{
  /** Resolved lazily: Electron's userData path is not final at module load. */
  scratchRoot: () => string
  env?: Record<string, string | undefined>
  /** Injectable SDK loader (tests, smoke fixture). Default lazy-imports the SDK. */
  queryImpl?: () => Promise<FableLocalQuery>
  /** Injectable account discovery (tests, smoke fixture). */
  discoverImpl?: () => Promise<ReadonlyArray<FableLocalAccountHome>>
  timeoutMs?: number
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

export type FableLocalRuntime = Readonly<{
  availability: () => Promise<FableLocalAvailability>
  runTurn: (input: FableLocalTurnInput) => Promise<FableLocalTurnResult>
  interrupt: (turnRef: string) => boolean
  dispose: () => void
}>

export const makeFableLocalRuntime = (options: FableLocalRuntimeOptions): FableLocalRuntime => {
  const env = options.env ?? (process.env as Record<string, string | undefined>)
  const discover = options.discoverImpl ?? (() => discoverReadyFableClaudeHomes(env))
  const timeoutMs = options.timeoutMs ?? FABLE_LOCAL_TIMEOUT_MS
  const loadQuery = options.queryImpl ?? (async () => {
    const sdk = (await import(CLAUDE_AGENT_SDK_PACKAGE)) as { query?: unknown }
    if (typeof sdk.query !== "function") throw new Error("Claude Agent SDK did not expose query().")
    return sdk.query as FableLocalQuery
  })
  const activeTurns = new Map<string, { interrupted: boolean; abort: () => void }>()
  /**
   * In-memory continuity: threadRef -> last completed SDK session, pinned to
   * the account that created it (a session is only resumable from the same
   * isolated account home).
   */
  const sessionByThread = new Map<string, { sessionId: string; accountRef: string }>()

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

    const ready = await discover()
    if (ready.length === 0) {
      return emitFailure(failure("no_claude_account", "no linked Claude account home found"))
    }
    let query: FableLocalQuery
    try {
      query = await loadQuery()
    } catch (error) {
      return emitFailure(failure("sdk_unavailable", error instanceof Error ? error.name : "sdk import failed"))
    }

    const workspace = join(options.scratchRoot(), "turns")
    try {
      mkdirSync(workspace, { recursive: true })
    } catch (error) {
      return emitFailure(failure("session_failed", error instanceof Error ? error.name : "workspace unavailable"))
    }
    const redact = (value: string): string => redactFableLocalText(value, { workspace })

    const control = {
      interrupted: false,
      inner: null as AbortController | null,
      abort(): void {
        this.interrupted = true
        this.inner?.abort()
      },
    }
    activeTurns.set(input.turnRef, control)

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
      const timer = setTimeout(() => {
        timedOut = true
        abort.abort()
      }, timeoutMs)

      const continuity = sessionByThread.get(input.threadRef)
      const resumeSessionId =
        continuity !== undefined && continuity.accountRef === account.ref
          ? continuity.sessionId
          : undefined
      const prompt = resumeSessionId === undefined
        ? historyPrompt(input.history, input.message)
        : input.message

      let sawContent = false
      let sessionId: string | null = null
      let deltaText = ""
      let assistantText = ""
      let resultText: string | null = null
      let resultIsError = false
      let resultSubtype: string | null = null
      let totalTokens: number | null = null
      const pendingToolCalls = new Map<string, string>()

      const finish = (
        result: FableLocalTurnResult,
      ): Readonly<{ result: FableLocalTurnResult; sawContent: boolean }> => {
        clearTimeout(timer)
        return { result, sawContent }
      }

      try {
        const session = query({
          prompt,
          options: {
            cwd: workspace,
            env: pylonAccountEnvironment(env, selection),
            abortController: abort,
            includePartialMessages: true,
            maxTurns: FABLE_LOCAL_MAX_TURNS,
            model: FABLE_LOCAL_MODEL,
            permissionMode: "default",
            allowedTools: [...FABLE_LOCAL_ALLOWED_TOOLS],
            disallowedTools: [...FABLE_LOCAL_DISALLOWED_TOOLS],
            skills: [],
            settingSources: [],
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
      input.emit({ kind: "turn_completed", totalTokens })
      return finish({ ok: true, text, totalTokens, accountRef: account.ref })
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
      activeTurns.delete(input.turnRef)
    }
  }

  return {
    availability,
    runTurn,
    interrupt: turnRef => {
      const active = activeTurns.get(turnRef)
      if (active === undefined) return false
      active.abort()
      return true
    },
    dispose: () => {
      for (const active of activeTurns.values()) active.abort()
      activeTurns.clear()
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
 * Scripted smoke fixture (OPENAGENTS_DESKTOP_SMOKE=1): a canned SDK message
 * sequence — init, spaced text deltas, one read-only tool round trip, and a
 * success result — driven through the REAL event mapping above. Never used
 * in normal runs; main.ts logs when it is active.
 */
export const makeFixtureFableLocalQuery = (): FableLocalQuery =>
  async function* fixture(): AsyncGenerator<unknown> {
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
    await sleep(150)
    yield {
      type: "stream_event",
      event: { type: "content_block_delta", delta: { type: "text_delta", text: "** proof." } },
    }
    yield {
      type: "result",
      subtype: "success",
      is_error: false,
      result: FABLE_LOCAL_FIXTURE_TEXT,
      usage: { input_tokens: 42, output_tokens: 7 },
    }
  }
