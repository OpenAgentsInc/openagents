/**
 * Codex account preflight prober (EP250 anti-speedbump core — owner mandate
 * verbatim: "add thorough fucking tests or whatever to prevent this category
 * of codex connection error PLEASE I HATE ALL THEF UCKING SPEEDBUMPS HERE").
 *
 * WHY A REAL TURN: registry "ready" is auth.json-PRESENCE-only, and
 * `codex login status` is presence-only too (receipted 2026-07-11 on this
 * machine: it prints "Logged in using ChatGPT" for the revoked-token `codex`
 * home). The ONLY cheap real validity signal codex-cli 0.144.1 offers is a
 * minimal `codex exec --json` turn. Receipted probe recipe + cost:
 *
 *   codex exec --json -m gpt-5.6-sol -c model_reasoning_effort=low \
 *     -s read-only --skip-git-repo-check -C <scratch> --ephemeral \
 *     "Reply with the single word: ok"
 *
 *   live codex-5: exit 0 in ~3.5s, usage {input 13292 (8960 cached), out 5};
 *   dead `codex`: exit 1 in ~4s with the revoked-token error events.
 *
 * The probe sandbox is READ-ONLY (probes never need write access) and each
 * probe is bounded host-side (~30s SIGTERM; `codex exec` has no timeout
 * flag). Accounts without an auth.json are classified `credentials_missing`
 * WITHOUT spawning (the live missing-auth probe burns ~50s in 401 retries).
 *
 * Probe results are SESSION-SCOPED truth carrying `observedAt`; they feed:
 * - sharedCodexAccountHealth (verified => recordSuccess => ordered first;
 *   auth-failed => recordAuthFailure => ordered last),
 * - the usage ledger's typed reconnectRequired flag (fleet readiness
 *   projection: probe evidence supersedes presence-based "ready"),
 * - the composer chip (available ⇔ ≥1 verified account),
 * - the live-proof step 0 journal.
 *
 * Runs: app boot (async, non-blocking), fleet Refresh, after every reconnect
 * completion, and lazily (ensureProbed) before the FIRST dispatch this
 * session if unprobed. Probes run concurrently across accounts.
 *
 * This module never imports `electron` (unit-testable under `bun test`).
 */
import { existsSync, mkdirSync } from "node:fs"
import { join } from "node:path"

import {
  hashPylonAccountRef,
  pylonAccountEnvironment,
  type ResolvedPylonAccountSelection,
} from "@openagentsinc/pylon-core/custody/account-registry"

import {
  classifyCodexFailureText,
  isCodexReconnectRequiredText,
  CODEX_CHILD_SUMMARY_LIMIT,
} from "./codex-child-contract.ts"
import {
  defaultSpawnCodex,
  discoverRegisteredCodexAccounts,
  makeCodexJsonLineConsumer,
  redactChildText,
  sharedCodexAccountHealth,
  type CodexAccountHealth,
  type CodexChildAccount,
  type CodexChildSpawn,
} from "./codex-child-runtime.ts"
import { CODEX_LOCAL_MODEL } from "./codex-local-contract.ts"
import { runCodexAppServerTurn } from "./codex-app-server-turn.ts"
import type { CodexAppServerSpawn } from "./codex-app-server-client.ts"

/** Receipted probe spawn config (see module docstring). */
export const CODEX_PREFLIGHT_MODEL = CODEX_LOCAL_MODEL
export const CODEX_PREFLIGHT_REASONING_EFFORT = "low" as const
export const CODEX_PREFLIGHT_SANDBOX = "read-only" as const
export const CODEX_PREFLIGHT_PROMPT = "Reply with the single word: ok"
/** Host-side bound per probe (codex exec has no timeout flag). */
export const CODEX_PREFLIGHT_TIMEOUT_MS = 30_000

export type CodexProbeState =
  /** The probe turn completed with real content — the credential WORKS. */
  | "verified"
  /** Auth-class failure (CODEX_RECONNECT_MARKERS) — reconnect in Settings. */
  | "reconnect_required"
  /** No auth.json in the account home — no spawn was attempted. */
  | "credentials_missing"
  /** Usage/credit budget exhausted while the credential remains valid. */
  | "quota_exhausted"
  /** Transient provider throttling distinct from exhausted usage/credits. */
  | "rate_limited"
  /** Anything else (network, malformed home, timeout, spawn failure). */
  | "probe_failed"

export type CodexProbeResult = Readonly<{
  ref: string
  state: CodexProbeState
  /** Bounded public-safe detail (redacted; never tokens or paths). */
  detail: string
  /** Session-scoped observation instant (ISO). */
  observedAt: string
  durationMs: number
}>

export type CodexPreflightOptions = Readonly<{
  /** Resolved lazily (Electron's userData path is not final at module load). */
  scratchRoot: () => string
  env?: Record<string, string | undefined>
  spawnImpl?: CodexChildSpawn
  discoverImpl?: () => Promise<ReadonlyArray<CodexChildAccount>>
  /** auth.json presence check; injectable for tests. */
  hasAuthImpl?: (home: string) => boolean
  health?: CodexAccountHealth
  timeoutMs?: number
  /** Streamed per-account results (ledger/fleet/log feeds). */
  onResult?: (result: CodexProbeResult) => void
  now?: () => Date
  /** Production path: probe through the same pinned app-server used for work. */
  appServer?: Readonly<{
    binary: () => string | null
    installProductSpecSkill: (account: CodexChildAccount) => Readonly<{
      skillRoot: string
      skillPath: string
    }>
    spawnImpl?: CodexAppServerSpawn
  }>
}>

export type CodexPreflight = Readonly<{
  /** Full re-probe of every registered account (boot/refresh/reconnect). */
  probeAll: (trigger: string) => Promise<ReadonlyArray<CodexProbeResult>>
  /** Lazy first-dispatch gate: probes once per session, then reuses. */
  ensureProbed: () => Promise<ReadonlyArray<CodexProbeResult>>
  /** Latest session-scoped results (empty before the first round). */
  results: () => ReadonlyArray<CodexProbeResult>
  verifiedRefs: () => ReadonlyArray<string>
}>

const bounded = (value: string, limit = CODEX_CHILD_SUMMARY_LIMIT): string =>
  value.length > limit ? `${value.slice(0, limit - 1)}…` : value

export const makeCodexPreflight = (options: CodexPreflightOptions): CodexPreflight => {
  const env = options.env ?? (process.env as Record<string, string | undefined>)
  const spawnCodex = options.spawnImpl ?? defaultSpawnCodex
  const discover = options.discoverImpl ?? (() => discoverRegisteredCodexAccounts(env))
  const hasAuth = options.hasAuthImpl ?? ((home: string) => existsSync(join(home, "auth.json")))
  const health = options.health ?? sharedCodexAccountHealth
  const timeoutMs = options.timeoutMs ?? CODEX_PREFLIGHT_TIMEOUT_MS
  const now = options.now ?? (() => new Date())

  let latest: ReadonlyArray<CodexProbeResult> = []
  let inFlight: Promise<ReadonlyArray<CodexProbeResult>> | null = null
  let probedOnce = false

  const probeOne = async (account: CodexChildAccount): Promise<CodexProbeResult> => {
      const startedAt = Date.now()
      const finishWith = (state: CodexProbeState, detail: string): CodexProbeResult =>
        ({
          ref: account.ref,
          state,
          detail: bounded(redactChildText(detail, "")),
          observedAt: now().toISOString(),
          durationMs: Date.now() - startedAt,
        })

      // Fast path: no auth.json means no credential at all — spawning would
      // burn ~50s of 401 retries for a foregone conclusion (receipted).
      if (!hasAuth(account.home)) {
        return finishWith("credentials_missing", "no auth.json in the account home")
      }

      const workspace = join(options.scratchRoot(), "codex-preflight", account.ref)
      try {
        mkdirSync(workspace, { recursive: true })
      } catch {
        return finishWith("probe_failed", "probe scratch workspace unavailable")
      }
      if (options.appServer !== undefined) {
        const binary = options.appServer.binary()
        if (binary === null) return finishWith("probe_failed", "package-owned Codex app-server unavailable")
        let skill: Readonly<{ skillRoot: string; skillPath: string }>
        try {
          skill = options.appServer.installProductSpecSkill(account)
        } catch (error) {
          return finishWith("probe_failed", error instanceof Error ? error.message : "productspec-work unavailable")
        }
        const selection: ResolvedPylonAccountSelection = {
          provider: "codex",
          selector: "registry_ref",
          accountRef: account.ref,
          accountRefHash: hashPylonAccountRef("codex", account.ref),
          home: account.home,
        }
        const control = {
          interrupted: false,
          interrupt: null as (() => void) | null,
          steer: null as ((message: string) => Promise<boolean>) | null,
        }
        const appServerEnv = account.source === "current_session"
          ? (() => {
              const current = { ...env }
              delete current.CODEX_HOME
              return current
            })()
          : pylonAccountEnvironment(env, selection)
        const outcome = await runCodexAppServerTurn({
          binary,
          env: appServerEnv,
          workspace,
          threadRef: `preflight.${account.ref}`,
          turnRef: `preflight.${account.ref}.${startedAt}`,
          accountRef: account.ref,
          prompt: CODEX_PREFLIGHT_PROMPT,
          imagePaths: [],
          resumeThreadId: null,
          model: CODEX_PREFLIGHT_MODEL,
          reasoningEffort: CODEX_PREFLIGHT_REASONING_EFFORT,
          productSpecSkill: skill,
          ephemeral: true,
          sandbox: "read-only",
          includeProductSpecSkill: false,
          approvalPolicy: "never",
          control,
          emit: () => {},
          ...(options.appServer.spawnImpl === undefined ? {} : { spawnImpl: options.appServer.spawnImpl }),
          requestTimeoutMs: timeoutMs,
          turnTimeoutMs: timeoutMs,
        })
        if (outcome.outcome === "success") return finishWith("verified", "app-server probe turn completed")
        if (outcome.outcome === "reconnect_required") return finishWith("reconnect_required", outcome.detail)
        if (outcome.quotaExhausted) return finishWith("quota_exhausted", outcome.detail)
        if (outcome.rateLimited) return finishWith("rate_limited", outcome.detail)
        return finishWith("probe_failed", outcome.detail)
      }
      const selection: ResolvedPylonAccountSelection = {
        provider: "codex",
        selector: "registry_ref",
        accountRef: account.ref,
        accountRefHash: hashPylonAccountRef("codex", account.ref),
        home: account.home,
      }
      const child = spawnCodex({
        args: [
          "exec",
          "--json",
          "-m",
          CODEX_PREFLIGHT_MODEL,
          "-c",
          `model_reasoning_effort=${CODEX_PREFLIGHT_REASONING_EFFORT}`,
          "-s",
          CODEX_PREFLIGHT_SANDBOX,
          "--skip-git-repo-check",
          "-C",
          workspace,
          "--ephemeral",
          CODEX_PREFLIGHT_PROMPT,
        ],
        env: pylonAccountEnvironment(env, selection),
        cwd: workspace,
      })
      if (child === null) {
        return finishWith("probe_failed", "codex executable unavailable")
      }

      let done = false
      let timedOut = false
      let agentText = ""
      let errorMessage: string | null = null
      let stderrText = ""
      const timer = setTimeout(() => {
        timedOut = true
        child.kill("SIGTERM")
      }, timeoutMs)
      return await new Promise<CodexProbeResult>(resolve => {
      const settle = (state: CodexProbeState, detail: string): void => {
        if (done) return
        done = true
        clearTimeout(timer)
        resolve(finishWith(state, detail))
      }

      const jsonLines = makeCodexJsonLineConsumer(event => {
        const type = typeof event.type === "string" ? event.type : ""
        if (type === "item.completed") {
          const item = event.item as Record<string, unknown> | undefined
          if (item?.type === "agent_message" && typeof item.text === "string") {
            agentText = item.text
          }
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
      })
      child.stdout?.on("data", (chunk: Buffer | string) => {
        jsonLines.push(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
      })
      child.stderr?.on("data", (chunk: Buffer | string) => {
        stderrText += typeof chunk === "string" ? chunk : chunk.toString("utf8")
      })
      child.on("error", () => settle("probe_failed", "codex probe process failed to start"))
      child.on("close", (...args: unknown[]) => {
        jsonLines.flush()
        const exitCode = typeof args[0] === "number" ? args[0] : null
        if (timedOut) {
          settle("probe_failed", `probe timed out (${Math.round(timeoutMs / 1000)}s bound)`)
          return
        }
        if (exitCode === 0 && agentText.trim() !== "") {
          settle("verified", "probe turn completed")
          return
        }
        const failureText = `${errorMessage ?? ""}\n${stderrText}`
        const failureClass = classifyCodexFailureText(failureText)
        if (failureClass === "auth" || isCodexReconnectRequiredText(failureText)) {
          settle("reconnect_required", errorMessage ?? "credentials rejected (auth-class failure)")
          return
        }
        if (failureClass === "quota_exhausted") {
          settle("quota_exhausted", errorMessage ?? "Codex usage quota exhausted")
          return
        }
        if (failureClass === "rate_limit") {
          settle("rate_limited", errorMessage ?? "provider rate limit reached")
          return
        }
        settle("probe_failed", errorMessage ?? `codex exec exited ${exitCode ?? "abnormally"}`)
      })
      })
    }

  const runRound = async (): Promise<ReadonlyArray<CodexProbeResult>> => {
    const accounts = await discover()
    const results = await Promise.all(accounts.map(async account => {
      const result = await probeOne(account)
      // Health feed: verified ranks first on every subsequent ordering;
      // auth-failed ranks last (still tried when nothing else is left).
      if (result.state === "verified") health.recordSuccess(result.ref)
      if (result.state === "reconnect_required") health.recordAuthFailure(result.ref)
      options.onResult?.(result)
      return result
    }))
    latest = results
    probedOnce = true
    return results
  }

  const probeAll = (trigger: string): Promise<ReadonlyArray<CodexProbeResult>> => {
    void trigger
    if (inFlight !== null) return inFlight
    inFlight = runRound().finally(() => {
      inFlight = null
    })
    return inFlight
  }

  return {
    probeAll,
    ensureProbed: () => (probedOnce && inFlight === null
      ? Promise.resolve(latest)
      : probeAll("lazy_first_dispatch")),
    results: () => latest,
    verifiedRefs: () =>
      latest.filter(result => result.state === "verified").map(result => result.ref),
  }
}
