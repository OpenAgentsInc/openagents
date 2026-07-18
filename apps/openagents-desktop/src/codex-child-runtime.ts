/**
 * Codex child runtime (#8712 Lane C): runs ONE bounded `codex exec --json`
 * sub-agent per call, pinned to gpt-5.5 / medium reasoning, against the
 * user's ordinary authenticated `~/.codex` session first, with the Pylon
 * account registry's isolated Codex homes as fallback capacity.
 *
 * Receipted spawn recipe (codex-cli 0.144.1, 2026-07-11):
 *
 *   codex exec --json -m gpt-5.5 -c model_reasoning_effort=medium \
 *     -s danger-full-access --skip-git-repo-check \
 *     -C <bounded scratch workspace> --ephemeral "<prompt>"
 *
 * with CODEX_HOME injected via `pylonAccountEnvironment` (provider codex).
 * The sandbox is `danger-full-access` per the owner-local danger profile
 * (owner statement 2026-07-11: "disallowing bash is retarded, give them full
 * tools full permissions etc" — the same owner-local executor invariant the
 * Khala->Pylon runbook uses; never a public wire field). There is NO
 * `codex exec` timeout flag: the bound is host-side (timer + SIGTERM, the
 * same pattern as provider-accounts.ts runProjection).
 *
 * Account rotation (BROADENED 2026-07-11 after a live miss): registry
 * readiness "ready" is auth.json-PRESENCE-only, so a spawned child can still
 * fail with bad credentials. Classification:
 * - AUTH-CLASS: any failure text matching `CODEX_RECONNECT_MARKERS`
 *   (including the SHORT live variant "Your access token could not be
 *   refreshed. Please log out and sign in again.") is typed
 *   `account_reconnect_required`, marks the account auth-failed in the
 *   in-process health memory, and rotates — visibly (a stream event per
 *   skipped account), never silently.
 * - ANY OTHER PRE-CONTENT failure (no completed agent_message, zero usage)
 *   also rotates, typed `pre_content_failure_rotated`: children are
 *   ephemeral and rotation is bounded by the registry size, so pre-content
 *   rotation on any failure is safe and loses nothing.
 * - POST-content failures and timeouts fail the child (no rotation).
 * When every candidate is exhausted, the call returns a typed failure:
 * `account_reconnect_required` when every rotation was auth-class, else
 * `child_failed` summarizing the mix.
 *
 * Account health memory: a module-level (main-process lifetime) map orders
 * candidates per call — last-known-good first (most recent success first),
 * then untried accounts, then auth-failed accounts LAST (still tried when
 * everything else is exhausted, since a reconnect may have fixed them; a
 * success clears the mark). Concurrent siblings and subsequent calls stop
 * burning attempts on a known-broken ref while a known-good one exists.
 *
 * Concurrency: every call gets its own scratch dir under
 * `<scratchRoot>/codex-children/<childRef>` and its own parser state, so
 * multiple children run simultaneously without shared mutable state. This
 * module never imports `electron` (unit-testable under `pnpm exec vp test`).
 */
import { spawn } from "node:child_process"
import { mkdirSync } from "node:fs"
import { access } from "node:fs/promises"
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
import { codexRuntimeAuthority } from "./provider-runtime-host.ts"

import {
  CODEX_CHILD_MODEL,
  CODEX_CHILD_REASONING_EFFORT,
  CODEX_CHILD_SANDBOX,
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

export type CodexChildAccount = Readonly<{
  ref: string
  home: string
  source?: "current_session" | "pylon"
}>

const DESKTOP_DRIVER_ENV_PREFIXES = [
  "OPENAGENTS_DESKTOP_MVP_PROOF",
  "OPENAGENTS_DESKTOP_SMOKE",
  "OPENAGENTS_DESKTOP_STARTUP_",
] as const

const DESKTOP_DRIVER_ENV_KEYS = new Set([
  "OPENAGENTS_DESKTOP_CLAUDE_PROJECTS",
  "OPENAGENTS_DESKTOP_CODEX_SESSIONS",
  "OPENAGENTS_DESKTOP_HEADED",
  "OPENAGENTS_DESKTOP_ISOLATED_APP_PROOF",
  "OPENAGENTS_DESKTOP_ISOLATED_WORKSPACE_ROOT",
  "OPENAGENTS_DESKTOP_LIVE_PROOF",
  "OPENAGENTS_DESKTOP_LOCAL_TURN_RESTART_PROBE",
  "OPENAGENTS_DESKTOP_USER_DATA",
])

/**
 * Desktop test/proof controls belong to the host process. Letting them reach a
 * provider subprocess can alter model-visible workflow selection and makes the
 * proof exercise a different environment from an ordinary user turn.
 */
export const codexProviderEnvironment = (
  source: Record<string, string | undefined>,
  options: Readonly<{ clearCodexHome?: boolean }> = {},
): Record<string, string | undefined> => {
  const sanitized = { ...source }
  if (options.clearCodexHome === true) delete sanitized.CODEX_HOME
  for (const key of Object.keys(sanitized)) {
    if (
      DESKTOP_DRIVER_ENV_KEYS.has(key) ||
      DESKTOP_DRIVER_ENV_PREFIXES.some(prefix => key.startsWith(prefix))
    ) delete sanitized[key]
  }
  return sanitized
}

// ---------------------------------------------------------------------------
// In-process account health memory (EP250 rotation fix). Module-level =
// main-process lifetime: the delegate runtime is constructed once in main.ts,
// and every runChild (including concurrent siblings' subsequent calls) shares
// this ordering so a known-broken ref stops being tried FIRST while a
// known-good one exists. Never persisted; a restart forgets everything.
// ---------------------------------------------------------------------------

export type CodexAccountHealthState = "last_good" | "auth_failed"

export type CodexAccountHealth = Readonly<{
  /** A successful child completion: promotes the ref and clears auth marks. */
  recordSuccess: (ref: string) => void
  /** An auth-class (reconnect-required) failure: demotes the ref to last. */
  recordAuthFailure: (ref: string) => void
  stateOf: (ref: string) => CodexAccountHealthState | null
  /**
   * Candidate ordering per call: last-known-good first (most recent success
   * first), then untried accounts (discovery order), then auth-failed
   * accounts LAST (still tried when everything else is exhausted — a
   * reconnect may have fixed them).
   */
  order: (accounts: ReadonlyArray<CodexChildAccount>) => ReadonlyArray<CodexChildAccount>
}>

export const makeCodexAccountHealth = (): CodexAccountHealth => {
  const marks = new Map<string, { state: CodexAccountHealthState; at: number }>()
  let tick = 0
  return {
    recordSuccess: ref => {
      tick += 1
      marks.set(ref, { state: "last_good", at: tick })
    },
    recordAuthFailure: ref => {
      tick += 1
      marks.set(ref, { state: "auth_failed", at: tick })
    },
    stateOf: ref => marks.get(ref)?.state ?? null,
    order: accounts => {
      const good = accounts
        .filter(account => marks.get(account.ref)?.state === "last_good")
        .sort((left, right) => marks.get(right.ref)!.at - marks.get(left.ref)!.at)
      const untried = accounts.filter(account => !marks.has(account.ref))
      const authFailed = accounts.filter(
        account => marks.get(account.ref)?.state === "auth_failed",
      )
      return [...good, ...untried, ...authFailed]
    },
  }
}

/** The shared main-process-lifetime health memory (default for runtimes). */
export const sharedCodexAccountHealth: CodexAccountHealth = makeCodexAccountHealth()

const bounded = (value: string, limit: number): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value

/** Public-safe redaction: the child workspace and the home prefix never leak.
 * Shared with the codex-local chat lane (./codex-local-runtime.ts). */
export const redactChildText = (value: string, workspace: string, home = homedir()): string => {
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
  const ownerHome = (env.HOME ?? "").trim() || homedir()
  const currentHome = join(ownerHome, ".codex")
  const accounts: CodexChildAccount[] = []
  try {
    await access(join(currentHome, "auth.json"))
    accounts.push({ ref: "codex-current", home: currentHome, source: "current_session" })
  } catch {
    // No ordinary Codex session; isolated Pylon capacity remains available.
  }
  const summary = { paths: { config: resolvePylonHome(env as NodeJS.ProcessEnv).config } }
  const registry = await loadPylonAccountRegistry(summary)
  const refs = registry
    .filter(entry => entry.provider === "codex" && entry.paused !== true)
    .map(entry => entry.ref)
    .sort((left, right) => left.localeCompare(right))
  for (const ref of refs) {
    try {
      const selection = await resolvePylonAccountSelection(summary, {
        provider: "codex",
        accountRef: ref,
      })
      if (selection !== null) accounts.push({ ref, home: selection.home, source: "pylon" })
    } catch {
      // account home missing — skip, never a hard failure for the whole pool
    }
  }
  return accounts
}

export type ChildLike = {
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

/** Shared validated installed Codex spawn (children, local chat, and preflight). */
export const defaultSpawnCodex: CodexChildSpawn = input => {
  try {
    const executable = codexRuntimeAuthority.executable()
    if (executable === null) return null
    return spawn(executable, [...input.args], {
      cwd: input.cwd,
      stdio: ["ignore", "pipe", "pipe"],
      env: input.env as NodeJS.ProcessEnv,
    }) as unknown as ChildLike
  } catch {
    return null
  }
}

/**
 * Shared newline-delimited JSON consumer (the exact parser the child runtime
 * streams `codex exec --json` stdout through). Extracted so the codex-local
 * chat lane and the preflight prober parse IDENTICALLY — one parser, three
 * consumers. Non-JSON lines are ignored (the exit code decides).
 */
export const makeCodexJsonLineConsumer = (
  onEvent: (event: Record<string, unknown>) => void,
): { push: (chunk: string) => void; flush: () => void } => {
  let buffer = ""
  const drain = (): void => {
    let newline = buffer.indexOf("\n")
    while (newline >= 0) {
      const line = buffer.slice(0, newline).trim()
      buffer = buffer.slice(newline + 1)
      if (line.length > 0) {
        try {
          onEvent(JSON.parse(line) as Record<string, unknown>)
        } catch {
          // non-JSON output lines are ignored; the exit code decides
        }
      }
      newline = buffer.indexOf("\n")
    }
  }
  return {
    push: chunk => {
      buffer += chunk
      drain()
    },
    flush: () => {
      if (buffer.trim().length > 0) {
        buffer += "\n"
        drain()
      }
    },
  }
}

export type CodexChildRuntimeOptions = Readonly<{
  /** Resolved lazily (Electron's userData path is not final at module load). */
  scratchRoot: () => string
  env?: Record<string, string | undefined>
  spawnImpl?: CodexChildSpawn
  discoverImpl?: () => Promise<ReadonlyArray<CodexChildAccount>>
  timeoutMs?: number
  /**
   * Injectable account health memory (tests). Defaults to the shared
   * module-level (main-process lifetime) map so concurrent siblings and
   * subsequent calls share ordering.
   */
  health?: CodexAccountHealth
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
  /**
   * True when the attempt failed BEFORE producing content: no completed
   * agent_message and zero usage. Pre-content failures are rotation-eligible
   * (ephemeral children lose nothing); post-content failures are terminal.
   */
  preContent: boolean
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
  const health = options.health ?? sharedCodexAccountHealth

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
          CODEX_CHILD_SANDBOX,
          "--skip-git-repo-check",
          "-C",
          input.workspace,
          "--ephemeral",
          input.prompt,
        ],
        env: input.account.source === "current_session"
          ? codexProviderEnvironment(env, { clearCodexHome: true })
          : pylonAccountEnvironment(codexProviderEnvironment(env), selection),
        cwd: input.workspace,
      })
      if (child === null) {
        resolve({
          outcome: "failed",
          text: "",
          usage: null,
          threadId: null,
          detail: "codex executable unavailable",
          preContent: true,
        })
        return
      }

      let done = false
      let timedOut = false
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

      // The SHARED parser (makeCodexJsonLineConsumer): identical parsing for
      // children, the codex-local chat lane, and the preflight prober.
      const jsonLines = makeCodexJsonLineConsumer(handleEvent)

      child.stdout?.on("data", (chunk: Buffer | string) => {
        jsonLines.push(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
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
          preContent: true,
        })
      })
      child.on("close", (...args: unknown[]) => {
        jsonLines.flush()
        const exitCode = typeof args[0] === "number" ? args[0] : null
        // Pre-content = the child never completed an agent_message AND
        // consumed zero usage. Only pre-content failures are rotation-
        // eligible; anything after content is terminal for the child.
        const currentUsage: CodexChildUsage | null = usage
        const preContent = agentText.trim() === "" &&
          (currentUsage === null || currentUsage.totalTokens === 0)
        if (timedOut) {
          finish({
            outcome: "timeout",
            text: "",
            usage,
            threadId,
            detail: `wall clock budget reached (${Math.round(timeoutMs / 1000)}s)`,
            preContent,
          })
          return
        }
        const failureText = `${errorMessage ?? ""}\n${stderrText}`
        if ((exitCode !== 0 || errorMessage !== null) &&
          isCodexReconnectRequiredText(failureText)) {
          finish({
            outcome: "reconnect_required",
            text: "",
            usage,
            threadId,
            detail: "credentials rejected (auth-class failure) — reconnect this Codex account",
            preContent,
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
            preContent,
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
            preContent,
          })
          return
        }
        finish({
          outcome: "success",
          text: agentText,
          usage,
          threadId,
          detail: "",
          preContent: false,
        })
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
        "no authenticated local Codex session or registered Pylon Codex account is available",
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

    // Candidate ordering from the in-process health memory: last-known-good
    // first, untried next, auth-failed LAST (still tried when everything
    // else is exhausted — a reconnect may have fixed them).
    const ordered = health.order(accounts)
    let reconnectCount = 0
    let preContentCount = 0
    let lastPreContentDetail = ""
    for (const account of ordered) {
      emit({ kind: "attempt_started", accountRef: account.ref })
      const attempt = await runAttempt({ account, prompt, workspace, emit })
      if (attempt.outcome === "success") {
        // A success clears any auth-failed mark and promotes the ref for
        // the NEXT call's ordering.
        health.recordSuccess(account.ref)
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
        // AUTH-CLASS: TYPED, VISIBLE rotation — a bad-credential account is
        // never silently skipped: every skip emits this event (and demotes
        // the account in the health memory) before the next candidate Codex
        // home gets the child.
        health.recordAuthFailure(account.ref)
        reconnectCount += 1
        emit({
          kind: "account_reconnect_required",
          accountRef: account.ref,
          detail: attempt.detail,
        })
        continue
      }
      if (attempt.preContent) {
        // NON-auth pre-content failure: rotation-eligible with its own typed
        // reason. Children are ephemeral and rotation is bounded by the
        // registry size, so this loses nothing. The account is NOT health-
        // demoted (the failure may be transient and is not credential-class).
        preContentCount += 1
        lastPreContentDetail = attempt.detail
        emit({
          kind: "pre_content_failure_rotated",
          accountRef: account.ref,
          detail: attempt.detail,
        })
        continue
      }
      // POST-content failure: terminal — a partially-produced child must
      // fail honestly rather than double-run.
      return failure("child_failed", attempt.detail, account.ref)
    }
    if (preContentCount === 0) {
      return failure(
        "account_reconnect_required",
        `all ${reconnectCount} available Codex session(s) need reconnect (credentials rejected)`,
        null,
      )
    }
    return failure(
      "child_failed",
      `all ${ordered.length} available Codex session(s) failed before producing content` +
        ` (${reconnectCount} need reconnect, ${preContentCount} other pre-content failure(s));` +
        ` last: ${lastPreContentDetail}`,
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

/**
 * The LIVE SHORT auth-failure variant (owner run, 2026-07-11): a turn.failed
 * whose message carries NONE of the original markers ("revoked",
 * "refresh_token_invalidated", "token_invalidated") — the variant the
 * pre-broadening classifier missed, so no rotation happened for that child.
 * Kept VERBATIM from the live evidence.
 */
export const FIXTURE_CODEX_SHORT_AUTH_MESSAGE =
  "Your access token could not be refreshed. Please log out and sign in again."

export const fixtureCodexShortAuthStdout = JSON.stringify({
  type: "turn.failed",
  error: { message: FIXTURE_CODEX_SHORT_AUTH_MESSAGE },
})

// ---------------------------------------------------------------------------
// EP250 failure-signature corpus fixtures — VERBATIM from live captures on
// this machine (2026-07-11, codex-cli 0.144.1) unless marked otherwise. These
// are the checked-in regression corpus rows codex-connection-signatures.test.ts
// drives through the REAL parser/classifier/rotation path.
// ---------------------------------------------------------------------------

/** Live dead-token home: the 401 token_invalidated stderr shape (bounded). */
export const fixtureCodex401TokenInvalidatedStderr =
  "2026-07-11T22:24:59.492116Z ERROR codex_models_manager::manager: failed to refresh available models: " +
  "unexpected status 401 Unauthorized: Your authentication token has been invalidated. Please try signing in again., " +
  "url: https://chatgpt.com/backend-api/codex/models?client_version=0.144.1, auth error: 401, auth error code: token_invalidated\n"

/** Live dead-token home: the refresh_token_invalidated stderr shape. */
export const fixtureCodexRefreshTokenInvalidatedStderr =
  '2026-07-11T22:25:01.451920Z ERROR codex_login::auth::manager: Failed to refresh token: 401 Unauthorized: {\n' +
  '  "error": {\n' +
  '    "message": "Your session has ended. Please log in again.",\n' +
  '    "type": "invalid_request_error",\n' +
  '    "param": null,\n' +
  '    "code": "refresh_token_invalidated"\n' +
  "  }\n" +
  "}\n"

/**
 * Live MISSING-auth.json home (empty CODEX_HOME): codex still starts a
 * thread, then fails the turn with the bearer-missing 401 (exit 1).
 */
export const FIXTURE_CODEX_MISSING_AUTH_MESSAGE =
  "unexpected status 401 Unauthorized: Missing bearer or basic authentication in header, " +
  "url: https://api.openai.com/v1/responses, cf-ray: a19b39369db416c1-IAH, request id: req_339cb3060e904432b50586883d8fb6c6"

export const fixtureCodexMissingAuthStdout = [
  JSON.stringify({ type: "thread.started", thread_id: "019f5348-b91a-7543-a191-1a73f143dd24" }),
  JSON.stringify({ type: "turn.started" }),
  JSON.stringify({ type: "error", message: `Reconnecting... 5/5 (${FIXTURE_CODEX_MISSING_AUTH_MESSAGE})` }),
  JSON.stringify({ type: "error", message: FIXTURE_CODEX_MISSING_AUTH_MESSAGE }),
  JSON.stringify({ type: "turn.failed", error: { message: FIXTURE_CODEX_MISSING_AUTH_MESSAGE } }),
].join("\n")

/**
 * Live MALFORMED auth.json: exit 1 with EMPTY stdout (no JSONL events at
 * all) and only this stderr line — a NON-auth pre-content failure shape (no
 * marker matches), so it must rotate as pre_content_failure_rotated.
 */
export const fixtureCodexMalformedAuthStderr = "key must be a string at line 1 column 2\n"

/** Synthetic (shape mirrors codex-rs retry copy): quota/429 rate-limit. */
export const FIXTURE_CODEX_RATE_LIMIT_MESSAGE =
  "unexpected status 429 Too Many Requests: You've hit your usage limit. Try again later."

export const fixtureCodexRateLimitStdout = [
  JSON.stringify({ type: "thread.started", thread_id: "thread-rate-limit" }),
  JSON.stringify({ type: "turn.started" }),
  JSON.stringify({ type: "turn.failed", error: { message: FIXTURE_CODEX_RATE_LIMIT_MESSAGE } }),
].join("\n")

/**
 * LIVE VERBATIM usage-limit variant (captured on codex-5, 2026-07-11 during
 * the EP250 live proof): the account's credential is VALID but quota is
 * exhausted. No "429" substring at all — the "usage limit" marker is what
 * classifies it rate-limit, never auth-class (a reconnect cannot fix quota).
 */
export const FIXTURE_CODEX_USAGE_LIMIT_MESSAGE =
  "You've hit your usage limit. Visit https://chatgpt.com/codex/settings/usage to purchase more credits or try again at 8:31 PM."

export const fixtureCodexUsageLimitStdout = [
  JSON.stringify({ type: "thread.started", thread_id: "019f536f-4069-76a2-afd8-4b0e3c2cd9ee" }),
  JSON.stringify({ type: "turn.started" }),
  JSON.stringify({ type: "error", message: FIXTURE_CODEX_USAGE_LIMIT_MESSAGE }),
  JSON.stringify({ type: "turn.failed", error: { message: FIXTURE_CODEX_USAGE_LIMIT_MESSAGE } }),
].join("\n")

/** Synthetic (reqwest connect-error shape): network refused pre-content. */
export const FIXTURE_CODEX_NETWORK_REFUSED_MESSAGE =
  "error sending request for url (https://chatgpt.com/backend-api/codex/responses): connection refused"

export const fixtureCodexNetworkRefusedStdout = [
  JSON.stringify({ type: "thread.started", thread_id: "thread-net-refused" }),
  JSON.stringify({ type: "error", message: FIXTURE_CODEX_NETWORK_REFUSED_MESSAGE }),
  JSON.stringify({ type: "turn.failed", error: { message: FIXTURE_CODEX_NETWORK_REFUSED_MESSAGE } }),
].join("\n")

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
