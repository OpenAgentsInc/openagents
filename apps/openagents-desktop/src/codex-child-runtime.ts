/**
 * Codex child runtime (#8712 Lane C): runs ONE bounded `codex exec --json`
 * sub-agent per call, pinned to gpt-5.6-sol / medium reasoning, against the
 * pylon account registry's isolated Codex homes — NEVER the default
 * `~/.codex` home.
 *
 * Receipted spawn recipe (codex-cli 0.144.1, 2026-07-11):
 *
 *   codex exec --json -m gpt-5.6-sol -c model_reasoning_effort=medium \
 *     -s read-only --skip-git-repo-check -C <bounded scratch workspace> \
 *     --ephemeral "<prompt>"
 *
 * with CODEX_HOME injected via `pylonAccountEnvironment` (provider codex).
 * There is NO `codex exec` timeout flag: the bound is host-side (timer +
 * SIGTERM, the same pattern as provider-accounts.ts runProjection).
 *
 * Account rotation: registry readiness "ready" is auth.json-PRESENCE-only,
 * so a spawned child can still fail with a revoked refresh token. That exact
 * receipted failure shape (error message "…refresh token was revoked",
 * stderr `token_invalidated` / `refresh_token_invalidated`, exit 1) is typed
 * `account_reconnect_required` and rotates to the next registered Codex
 * home — visibly (a stream event per skipped account), never silently. When
 * every registered account fails that way, the call returns a typed
 * `account_reconnect_required` failure naming the reconnect need.
 *
 * Concurrency: every call gets its own scratch dir under
 * `<scratchRoot>/codex-children/<childRef>` and its own parser state, so
 * multiple children run simultaneously without shared mutable state. This
 * module never imports `electron` (unit-testable under `bun test`).
 */
import { spawn } from "node:child_process"
import { mkdirSync } from "node:fs"
import { homedir } from "node:os"
import { join } from "node:path"

import {
  hashPylonAccountRef,
  loadPylonAccountRegistry,
  pylonAccountEnvironment,
  resolvePylonAccountSelection,
  type ResolvedPylonAccountSelection,
} from "@openagentsinc/pylon-core/custody/account-registry"
import { resolvePylonHome } from "@openagentsinc/pylon-core/shared/bootstrap"

import {
  CODEX_CHILD_MODEL,
  CODEX_CHILD_REASONING_EFFORT,
  CODEX_CHILD_SUMMARY_LIMIT,
  CODEX_CHILD_TEXT_LIMIT,
  CODEX_CHILD_TIMEOUT_MS,
  codexChildUsageFromTurnCompleted,
  isCodexReconnectRequiredText,
  type CodexChildFailure,
  type CodexChildResult,
  type CodexChildRunInput,
  type CodexChildStreamEvent,
  type CodexChildUsage,
} from "./codex-child-contract.ts"

export type CodexChildAccount = Readonly<{ ref: string; home: string }>

const bounded = (value: string, limit: number): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value

/** Public-safe redaction: the child workspace and the home prefix never leak. */
const redactChildText = (value: string, workspace: string, home = homedir()): string => {
  let out = value
  if (workspace.length > 0) out = out.split(workspace).join("<child-workspace>")
  if (home.length > 0) out = out.split(home).join("~")
  return out
}

/**
 * Registered Codex accounts from the pylon account registry (the desktop's
 * pylon home config.json `dev.accounts[]`), resolved through the SAME
 * selection path the Pylon supervisor uses. Deterministic order (ref). A
 * missing home directory drops the account rather than failing the call.
 */
export const discoverRegisteredCodexAccounts = async (
  env: Record<string, string | undefined> = process.env,
): Promise<ReadonlyArray<CodexChildAccount>> => {
  const summary = { paths: { config: resolvePylonHome(env as NodeJS.ProcessEnv).config } }
  const registry = await loadPylonAccountRegistry(summary)
  const refs = registry
    .filter(entry => entry.provider === "codex" && entry.paused !== true)
    .map(entry => entry.ref)
    .sort((left, right) => left.localeCompare(right))
  const accounts: CodexChildAccount[] = []
  for (const ref of refs) {
    try {
      const selection = await resolvePylonAccountSelection(summary, {
        provider: "codex",
        accountRef: ref,
      })
      if (selection !== null) accounts.push({ ref, home: selection.home })
    } catch {
      // account home missing — skip, never a hard failure for the whole pool
    }
  }
  return accounts
}

type ChildLike = {
  stdout: NodeJS.ReadableStream | null
  stderr: NodeJS.ReadableStream | null
  on: (event: "close" | "error", listener: (...args: unknown[]) => void) => unknown
  kill: (signal?: NodeJS.Signals) => boolean
  killed: boolean
}

export type CodexChildSpawn = (input: Readonly<{
  args: ReadonlyArray<string>
  env: Record<string, string | undefined>
  cwd: string
}>) => ChildLike | null

const defaultSpawnCodex: CodexChildSpawn = input => {
  try {
    return spawn("codex", [...input.args], {
      cwd: input.cwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: input.env as NodeJS.ProcessEnv,
    }) as unknown as ChildLike
  } catch {
    return null
  }
}

export type CodexChildRuntimeOptions = Readonly<{
  /** Resolved lazily (Electron's userData path is not final at module load). */
  scratchRoot: () => string
  env?: Record<string, string | undefined>
  spawnImpl?: CodexChildSpawn
  discoverImpl?: () => Promise<ReadonlyArray<CodexChildAccount>>
  timeoutMs?: number
}>

export type CodexChildRuntime = Readonly<{
  runChild: (input: CodexChildRunInput) => Promise<CodexChildResult>
}>

type ParsedAttempt = Readonly<{
  outcome: "success" | "reconnect_required" | "failed" | "timeout"
  text: string
  usage: CodexChildUsage | null
  threadId: string | null
  detail: string
}>

const itemSummary = (item: Record<string, unknown>, redact: (value: string) => string): string => {
  const type = typeof item.type === "string" ? item.type : "item"
  if (type === "agent_message" && typeof item.text === "string") {
    return bounded(redact(item.text), CODEX_CHILD_SUMMARY_LIMIT)
  }
  if (type === "command_execution" && typeof item.command === "string") {
    return bounded(redact(item.command), CODEX_CHILD_SUMMARY_LIMIT)
  }
  if (type === "mcp_tool_call") {
    const tool = typeof item.tool_name === "string"
      ? item.tool_name
      : typeof item.name === "string" ? item.name : "tool"
    return bounded(redact(tool), CODEX_CHILD_SUMMARY_LIMIT)
  }
  if (type === "file_change" && Array.isArray(item.changes)) {
    return `${item.changes.length} file change(s)`
  }
  return type
}

export const makeCodexChildRuntime = (options: CodexChildRuntimeOptions): CodexChildRuntime => {
  const env = options.env ?? (process.env as Record<string, string | undefined>)
  const spawnCodex = options.spawnImpl ?? defaultSpawnCodex
  const discover = options.discoverImpl ?? (() => discoverRegisteredCodexAccounts(env))
  const timeoutMs = options.timeoutMs ?? CODEX_CHILD_TIMEOUT_MS

  const runAttempt = (input: Readonly<{
    account: CodexChildAccount
    prompt: string
    workspace: string
    emit: (event: CodexChildStreamEvent) => void
  }>): Promise<ParsedAttempt> =>
    new Promise(resolve => {
      const redact = (value: string): string => redactChildText(value, input.workspace)
      const selection: ResolvedPylonAccountSelection = {
        provider: "codex",
        selector: "registry_ref",
        accountRef: input.account.ref,
        accountRefHash: hashPylonAccountRef("codex", input.account.ref),
        home: input.account.home,
      }
      const child = spawnCodex({
        args: [
          "exec",
          "--json",
          "-m",
          CODEX_CHILD_MODEL,
          "-c",
          `model_reasoning_effort=${CODEX_CHILD_REASONING_EFFORT}`,
          "-s",
          "read-only",
          "--skip-git-repo-check",
          "-C",
          input.workspace,
          "--ephemeral",
          input.prompt,
        ],
        env: pylonAccountEnvironment(env, selection),
        cwd: input.workspace,
      })
      if (child === null) {
        resolve({
          outcome: "failed",
          text: "",
          usage: null,
          threadId: null,
          detail: "codex executable unavailable",
        })
        return
      }

      let done = false
      let timedOut = false
      let stdoutBuffer = ""
      let stderrText = ""
      let agentText = ""
      let usage: CodexChildUsage | null = null
      let threadId: string | null = null
      let errorMessage: string | null = null

      const timer = setTimeout(() => {
        timedOut = true
        child.kill("SIGTERM")
      }, timeoutMs)

      const finish = (attempt: ParsedAttempt): void => {
        if (done) return
        done = true
        clearTimeout(timer)
        resolve(attempt)
      }

      const handleEvent = (event: Record<string, unknown>): void => {
        const type = typeof event.type === "string" ? event.type : ""
        if (type === "thread.started" && typeof event.thread_id === "string") {
          threadId = event.thread_id
          return
        }
        if (type === "item.completed" || type === "item.started" || type === "item.updated") {
          const item = event.item
          if (item === null || typeof item !== "object") return
          const record = item as Record<string, unknown>
          if (type === "item.completed" && record.type === "agent_message" &&
            typeof record.text === "string") {
            agentText = bounded(redact(record.text), CODEX_CHILD_TEXT_LIMIT)
          }
          if (type === "item.completed") {
            input.emit({
              kind: "item",
              itemType: typeof record.type === "string" ? record.type : "item",
              summary: itemSummary(record, redact),
            })
          }
          return
        }
        if (type === "turn.completed") {
          usage = codexChildUsageFromTurnCompleted(event)
          return
        }
        if (type === "error" && typeof event.message === "string") {
          errorMessage = event.message
          return
        }
        if (type === "turn.failed") {
          const inner = (event.error as { message?: unknown } | undefined)?.message
          if (typeof inner === "string") errorMessage = inner
        }
      }

      const consumeStdout = (chunk: string): void => {
        stdoutBuffer += chunk
        let newline = stdoutBuffer.indexOf("\n")
        while (newline >= 0) {
          const line = stdoutBuffer.slice(0, newline).trim()
          stdoutBuffer = stdoutBuffer.slice(newline + 1)
          if (line.length > 0) {
            try {
              const parsed = JSON.parse(line) as Record<string, unknown>
              handleEvent(parsed)
            } catch {
              // non-JSON output lines are ignored; the exit code decides
            }
          }
          newline = stdoutBuffer.indexOf("\n")
        }
      }

      child.stdout?.on("data", (chunk: Buffer | string) => {
        consumeStdout(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
      })
      child.stderr?.on("data", (chunk: Buffer | string) => {
        stderrText += typeof chunk === "string" ? chunk : chunk.toString("utf8")
      })
      child.on("error", () => {
        finish({
          outcome: "failed",
          text: "",
          usage: null,
          threadId: null,
          detail: "codex child process failed to start",
        })
      })
      child.on("close", (...args: unknown[]) => {
        if (stdoutBuffer.trim().length > 0) consumeStdout("\n")
        const exitCode = typeof args[0] === "number" ? args[0] : null
        if (timedOut) {
          finish({
            outcome: "timeout",
            text: "",
            usage,
            threadId,
            detail: `wall clock budget reached (${Math.round(timeoutMs / 1000)}s)`,
          })
          return
        }
        const failureText = `${errorMessage ?? ""}\n${stderrText}`
        if (isCodexReconnectRequiredText(failureText)) {
          finish({
            outcome: "reconnect_required",
            text: "",
            usage,
            threadId,
            detail: "refresh token revoked — reconnect this Codex account",
          })
          return
        }
        if (exitCode !== 0 || errorMessage !== null) {
          finish({
            outcome: "failed",
            text: "",
            usage,
            threadId,
            detail: bounded(
              redact(errorMessage ?? `codex exec exited ${exitCode ?? "abnormally"}`),
              CODEX_CHILD_SUMMARY_LIMIT,
            ),
          })
          return
        }
        if (agentText.trim() === "") {
          finish({
            outcome: "failed",
            text: "",
            usage,
            threadId,
            detail: "the child produced no agent_message text",
          })
          return
        }
        finish({ outcome: "success", text: agentText, usage, threadId, detail: "" })
      })
    })

  const runChild = async (input: CodexChildRunInput): Promise<CodexChildResult> => {
    const startedAt = Date.now()
    const emit = input.onEvent ?? (() => {})
    const failure = (
      reason: CodexChildFailure["reason"],
      detail: string,
      accountRef: string | null,
    ): CodexChildFailure => ({
      ok: false,
      reason,
      detail: bounded(detail, CODEX_CHILD_SUMMARY_LIMIT),
      accountRef,
      durationMs: Date.now() - startedAt,
    })

    const accounts = await discover()
    if (accounts.length === 0) {
      return failure(
        "no_codex_account",
        "no Codex account is registered in the pylon account registry",
        null,
      )
    }

    const safeChildRef = input.childRef.replace(/[^A-Za-z0-9._-]/g, "-").slice(0, 80)
    const workspace = join(options.scratchRoot(), "codex-children", safeChildRef)
    try {
      mkdirSync(workspace, { recursive: true })
    } catch {
      return failure("child_failed", "child scratch workspace unavailable", null)
    }
    const prompt = input.context === undefined || input.context.trim() === ""
      ? input.task
      : `${input.task}\n\nContext:\n${input.context}`

    let reconnectCount = 0
    for (const account of accounts) {
      emit({ kind: "attempt_started", accountRef: account.ref })
      const attempt = await runAttempt({ account, prompt, workspace, emit })
      if (attempt.outcome === "success") {
        return {
          ok: true,
          text: attempt.text,
          usage: attempt.usage,
          threadId: attempt.threadId,
          accountRef: account.ref,
          requestedModel: CODEX_CHILD_MODEL,
          requestedEffort: CODEX_CHILD_REASONING_EFFORT,
          durationMs: Date.now() - startedAt,
        }
      }
      if (attempt.outcome === "timeout") {
        return failure("child_timeout", attempt.detail, account.ref)
      }
      if (attempt.outcome === "reconnect_required") {
        // TYPED, VISIBLE rotation — a revoked-credential account is never
        // silently skipped: every skip emits this event before the next
        // registered Codex home gets the child.
        reconnectCount += 1
        emit({
          kind: "account_reconnect_required",
          accountRef: account.ref,
          detail: attempt.detail,
        })
        continue
      }
      return failure("child_failed", attempt.detail, account.ref)
    }
    return failure(
      "account_reconnect_required",
      `all ${reconnectCount} registered Codex account(s) need reconnect (revoked refresh tokens); reconnect with khala fleet connect`,
      null,
    )
  }

  return { runChild }
}

// ---------------------------------------------------------------------------
// Scripted fixture (smoke + tests): the scout's EXACT receipted event shapes
// driven through the REAL JSONL parser above. Never used in normal runs;
// main.ts logs when it is active.
// ---------------------------------------------------------------------------

export const FIXTURE_CODEX_CHILD_TEXT = "Codex child fixture answer."
export const FIXTURE_CODEX_CHILD_USAGE = {
  input_tokens: 1200,
  cached_input_tokens: 900,
  output_tokens: 180,
  reasoning_output_tokens: 60,
} as const

/** The receipted revoked-token failure stream (exit 1, ~4s live). */
export const fixtureCodexRevokedStdout = [
  JSON.stringify({
    type: "error",
    message:
      "Your access token could not be refreshed because your refresh token was revoked",
  }),
].join("\n")

export const fixtureCodexRevokedStderr =
  "ERROR codex_core::auth: refresh_token_invalidated token_invalidated\n"

/** A successful child stream with exact usage totals. */
export const fixtureCodexSuccessStdout = (threadId = "thread-fixture-1"): string =>
  [
    JSON.stringify({ type: "thread.started", thread_id: threadId }),
    JSON.stringify({ type: "turn.started" }),
    JSON.stringify({
      type: "item.started",
      item: { id: "item_0", type: "reasoning", text: "thinking" },
    }),
    JSON.stringify({
      type: "item.completed",
      item: { id: "item_0", type: "reasoning", text: "thought about it" },
    }),
    JSON.stringify({
      type: "item.completed",
      item: { id: "item_1", type: "agent_message", text: FIXTURE_CODEX_CHILD_TEXT },
    }),
    JSON.stringify({ type: "turn.completed", usage: FIXTURE_CODEX_CHILD_USAGE }),
  ].join("\n")

export type FixtureCodexScript = Readonly<{
  stdout: string
  stderr?: string
  exitCode: number
  /** Never closes until killed — exercises the host-side timeout bound. */
  hang?: boolean
  delayMs?: number
}>

/**
 * Scripted spawn: one script per successive spawn (keyed by call order); the
 * last script repeats. Streams through the same `.on("data")` interface the
 * real child uses so the parser path is identical.
 */
export const makeFixtureCodexChildSpawn = (
  scripts: ReadonlyArray<FixtureCodexScript>,
  onSpawn?: (input: Readonly<{ args: ReadonlyArray<string>; env: Record<string, string | undefined>; cwd: string }>) => void,
): CodexChildSpawn => {
  let calls = 0
  return input => {
    const script = scripts[Math.min(calls, scripts.length - 1)]!
    calls += 1
    onSpawn?.(input)
    const listeners = new Map<string, Array<(...values: unknown[]) => void>>()
    const stdoutHandlers: Array<(chunk: string) => void> = []
    const stderrHandlers: Array<(chunk: string) => void> = []
    let killed = false
    const child: ChildLike = {
      stdout: {
        on: (event: string, listener: (chunk: string) => void) => {
          if (event === "data") stdoutHandlers.push(listener)
        },
      } as unknown as NodeJS.ReadableStream,
      stderr: {
        on: (event: string, listener: (chunk: string) => void) => {
          if (event === "data") stderrHandlers.push(listener)
        },
      } as unknown as NodeJS.ReadableStream,
      on: (event, listener) => {
        const existing = listeners.get(event) ?? []
        listeners.set(event, [...existing, listener])
        return child
      },
      kill: () => {
        killed = true
        // A SIGTERMed fixture child closes like the real one does.
        setTimeout(() => {
          for (const listener of listeners.get("close") ?? []) listener(null)
        }, 0)
        return true
      },
      killed: false,
    }
    setTimeout(() => {
      for (const handler of stdoutHandlers) handler(`${script.stdout}\n`)
      if (script.stderr !== undefined) {
        for (const handler of stderrHandlers) handler(script.stderr)
      }
      if (script.hang !== true && !killed) {
        for (const listener of listeners.get("close") ?? []) listener(script.exitCode)
      }
    }, script.delayMs ?? 0)
    return child
  }
}
