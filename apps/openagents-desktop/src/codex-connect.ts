/**
 * Main-process-only Codex account connect service (#8574, #8640 unblock).
 *
 * Uses the proven Pylon isolated custody module in-process and launches only
 * the package-owned Codex native executable. The installed artifact therefore
 * needs neither a Bun executable nor `apps/pylon/src/index.ts` from a checkout.
 *
 * SAFETY (repo law): this service NEVER sets or touches the default
 * `~/.codex` home. Pylon Core owns credential custody and runs
 * `codex login --device-auth` with an isolated per-account
 * `CODEX_HOME=<pylon home>/accounts/codex/<ref>` (see
 * packages/pylon-core/src/custody/account-connect.ts). We add no CODEX_HOME
 * of our own and read no credential files. Only public-safe projections
 * (refs, readiness states, verification URL + user code, typed status) leave
 * this module; the connect success line's email is deliberately dropped.
 */
import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { join } from "node:path"

import {
  loadPylonAccountRegistry,
  runPylonAccountsConnect,
  type PylonAccountsConnectArgs,
} from "@openagentsinc/pylon-core/custody"
import { resolvePylonHome } from "@openagentsinc/pylon-core/shared/bootstrap"

import {
  codexAccountRefPattern,
  codexUserCodePattern,
  type CodexAccountsResult,
  type CodexConnectStatus,
  unavailableCodexAccountsResult,
} from "./codex-connect-contract.ts"
import { resolveBundledCodexExecutable } from "./provider-runtime-host.ts"

// ---------------------------------------------------------------------------
// Legacy smoke-child stdout parser (pure; retained for deterministic fixture
// mode). Installed custody parses the native Codex prompt directly below.
//
// `pylon auth codex` (non-JSON mode) prints exactly
//   `${verificationUrl}\n${userCode}\n`
// when a device prompt fires (apps/pylon/src/index.ts onDevicePrompt), then on
// success `✓ Linked Codex account: user@example.com (codex-4)` or
// `✓ Re-authenticated Codex account ... (ref)`. Failures print a `⚠ ...`
// stderr line and exit non-zero.
// ---------------------------------------------------------------------------

export type DeviceAuthEvent =
  | { kind: "awaiting_browser"; url: string; code: string }
  | { kind: "connected"; ref: string }
  | { kind: "failed"; reason: string }

const verificationUrlPattern = /^https:\/\/\S{1,190}$/
const connectedLinePattern = /^✓ .*\(([A-Za-z0-9][A-Za-z0-9._-]{0,79})\)\s*$/u

/** Strip ANSI color escapes (defense; pylon already strips its own). */
const stripAnsi = (value: string): string =>
  value.replace(/\u001b/g, "").replace(/\[[0-9;]*m/g, "")

/**
 * Bounded public-safe failure detail (EP250 owner receipt: the CLI's
 * `Pylon auth failed: <detail>` was collapsed to a bare `pylon_auth_failed`,
 * hiding the actual reason from the UI). Emails, home paths, and token-like
 * material are redacted before anything crosses the bridge; the contract
 * decode additionally caps failure reasons at 120 chars.
 */
export const publicSafeFailureDetail = (value: string): string =>
  value
    .replace(/[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}/g, "<email>")
    .replace(/(?:\/Users|\/home|\/private|~)\/\S*/g, "<path>")
    .replace(/\b(?:oa_agent_\S+|sk-\S+|Bearer\s+\S+|eyJ[\w-]{8,}\S*)/g, "<redacted>")
    .replace(/\s+/g, " ")
    .trim()
    .slice(0, 100)

/**
 * Incremental line parser over a child stdio stream. Feed chunks as they
 * arrive; complete lines produce typed events. A URL line followed by a
 * user-code line yields `awaiting_browser` (the pylon CLI may print two
 * prompts — OpenAgents link then Codex device — each pair re-emits).
 */
export const createDeviceAuthStdoutParser = () => {
  let buffer = ""
  let pendingUrl: string | null = null

  const parseLine = (rawLine: string): DeviceAuthEvent | null => {
    const line = stripAnsi(rawLine).trim()
    if (line === "") return null
    if (verificationUrlPattern.test(line)) {
      pendingUrl = line
      return null
    }
    if (pendingUrl !== null && codexUserCodePattern.test(line)) {
      const url = pendingUrl
      pendingUrl = null
      return { kind: "awaiting_browser", url, code: line }
    }
    const connected = line.match(connectedLinePattern)
    if (connected?.[1] !== undefined && codexAccountRefPattern.test(connected[1])) {
      // Public-safe: keep the ref only; the email on this line never leaves.
      return { kind: "connected", ref: connected[1] }
    }
    if (line.startsWith("⚠")) {
      return { kind: "failed", reason: "credentials_invalid_relogin_incomplete" }
    }
    if (line.startsWith("Pylon auth failed")) {
      // Surface the actual (public-safe, bounded) detail — the bare token hid
      // the real reason from the owner while the flow had half-succeeded.
      const detail = publicSafeFailureDetail(
        line.slice("Pylon auth failed".length).replace(/^[:\s]+/, ""),
      )
      return {
        kind: "failed",
        reason: detail === "" ? "pylon_auth_failed" : `pylon_auth_failed: ${detail}`,
      }
    }
    return null
  }

  return {
    feed: (chunk: string): DeviceAuthEvent[] => {
      buffer += chunk
      const events: DeviceAuthEvent[] = []
      let newline = buffer.indexOf("\n")
      while (newline !== -1) {
        const line = buffer.slice(0, newline)
        buffer = buffer.slice(newline + 1)
        const event = parseLine(line)
        if (event !== null) events.push(event)
        newline = buffer.indexOf("\n")
      }
      return events
    },
    /** Flush a trailing unterminated line (child exit). */
    end: (): DeviceAuthEvent[] => {
      const rest = buffer
      buffer = ""
      const event = rest.trim() === "" ? null : parseLine(rest)
      return event === null ? [] : [event]
    },
  }
}

/** Public-safe accounts-list projection from pylon's accounts_list JSON. */
export const parseAccountsListJson = (stdout: string): CodexAccountsResult => {
  try {
    const parsed = JSON.parse(stdout) as {
      schema?: unknown
      accounts?: Array<{
        provider?: unknown
        accountRef?: unknown
        readiness?: { state?: unknown }
      }>
    }
    if (!Array.isArray(parsed.accounts)) return unavailableCodexAccountsResult()
    const accounts = parsed.accounts
      .filter((account) => account.provider === "codex")
      .map((account) => ({
        ref: typeof account.accountRef === "string" ? account.accountRef : "",
        readiness:
          typeof account.readiness?.state === "string" ? account.readiness.state : "unknown",
      }))
      .filter((account) => codexAccountRefPattern.test(account.ref))
    return { state: "ok", accounts }
  } catch {
    return unavailableCodexAccountsResult()
  }
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

type ChildLike = {
  stdout: NodeJS.ReadableStream | null
  stderr: NodeJS.ReadableStream | null
  on: (event: "close" | "error", listener: (...args: unknown[]) => void) => unknown
  kill: (signal?: NodeJS.Signals) => boolean
  killed: boolean
  exitCode: number | null
}

export type CodexConnectServiceDependencies = Readonly<{
  /** Legacy source-CLI seam retained only for deterministic smoke fixtures. */
  spawnPylon?: (args: ReadonlyArray<string>) => ChildLike | null
  installedCustody?: InstalledCodexCustody
  openExternal?: (url: string) => Promise<void>
  listTimeoutMs?: number
  connectTimeoutMs?: number
}>

export type InstalledCodexCustody = Readonly<{
  listAccounts: () => Promise<CodexAccountsResult>
  connect: (
    ref: string | null,
    forceDeviceLogin: boolean,
    onPrompt: (prompt: Readonly<{ url: string; code: string }>) => void,
  ) => Promise<string>
  cancel: () => void
  dispose: () => void
}>

export type InstalledCodexCustodyDependencies = Readonly<{
  env?: Record<string, string | undefined>
  resolveCodex?: () => string | null
  spawnCodex?: (input: Readonly<{
    executable: string
    args: ReadonlyArray<string>
    env: Record<string, string | undefined>
  }>) => ChildLike | null
}>

const codexConnectArgs = (
  accountRef: string,
  forceDeviceLogin: boolean,
): PylonAccountsConnectArgs => ({
  provider: "codex",
  accountRef,
  accountLabel: accountRef,
  agentToken: null,
  baseUrl: null,
  createNewOpenAgentsAccount: true,
  home: null,
  forceDeviceLogin,
  json: true,
  openAgentsAttemptId: null,
  openAgentsLink: false,
  providerAccountRef: null,
  setupToken: null,
  skipDeviceLogin: false,
})

const nextCodexAccountRef = (refs: ReadonlyArray<string>): string => {
  const existing = new Set(refs)
  if (!existing.has("codex")) return "codex"
  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `codex-${index}`
    if (!existing.has(candidate)) return candidate
  }
  throw new Error("codex_account_ref_capacity_exhausted")
}

const defaultSpawnInstalledCodex = (input: Readonly<{
  executable: string
  args: ReadonlyArray<string>
  env: Record<string, string | undefined>
}>): ChildLike | null => {
  try {
    return spawn(input.executable, [...input.args], {
      stdio: ["ignore", "pipe", "pipe"],
      env: input.env as NodeJS.ProcessEnv,
    }) as unknown as ChildLike
  } catch {
    return null
  }
}

const codexDevicePrompt = (raw: string): Readonly<{ url: string; code: string }> | null => {
  const cleaned = raw.replace(/\u001b\[[0-9;]*m/g, "")
  const url = cleaned.match(/https:\/\/auth\.openai\.com\/codex\/device\b/)?.[0]
  const code = cleaned.match(/\b[A-Z0-9]{4}-[A-Z0-9]{4,6}\b/)?.[0]
  return url === undefined || code === undefined ? null : { url, code }
}

/**
 * Installed-artifact custody path. It calls the bundled Pylon custody module
 * and package-owned Codex executable directly; no Bun binary, source checkout,
 * or `apps/pylon/src/index.ts` exists in this execution path.
 */
export const makeInstalledCodexCustody = (
  dependencies: InstalledCodexCustodyDependencies = {},
): InstalledCodexCustody => {
  const env = dependencies.env ?? (process.env as Record<string, string | undefined>)
  const paths = resolvePylonHome(env as NodeJS.ProcessEnv)
  const summary = {
    paths,
    bootstrap: {
      registerOpenAgents: false,
      setupMdkWallet: false,
      pylonRef: null,
      displayName: null,
      resourceMode: "desktop_local",
      capabilityRefs: [],
    },
  }
  const resolveCodex = dependencies.resolveCodex ?? resolveBundledCodexExecutable
  const spawnCodex = dependencies.spawnCodex ?? defaultSpawnInstalledCodex
  let disposed = false
  let active: ChildLike | null = null

  const registry = () => loadPylonAccountRegistry(summary)
  const listAccounts = async (): Promise<CodexAccountsResult> => {
    if (disposed) return unavailableCodexAccountsResult()
    const accounts = (await registry())
      .filter(account => account.provider === "codex" && codexAccountRefPattern.test(account.ref))
      .map(account => ({
        ref: account.ref,
        readiness: existsSync(join(account.home, "auth.json")) ? "ready" : "credentials_missing",
      }))
    return { state: "ok", accounts }
  }

  const runLogin = (
    input: Readonly<{ env: Record<string, string | undefined>; home: string }>,
    onPrompt: (prompt: Readonly<{ url: string; code: string }>) => void,
  ): Promise<{ exitCode: number }> => new Promise(resolve => {
    const executable = resolveCodex()
    if (disposed || executable === null) {
      resolve({ exitCode: 1 })
      return
    }
    const child = spawnCodex({
      executable,
      args: ["login", "--device-auth"],
      env: { ...env, ...input.env, CODEX_HOME: input.home },
    })
    if (child === null) {
      resolve({ exitCode: 1 })
      return
    }
    active = child
    let settled = false
    let buffer = ""
    let promptEmitted = false
    const finish = (exitCode: number): void => {
      if (settled) return
      settled = true
      if (active === child) active = null
      resolve({ exitCode })
    }
    const consume = (text: string): void => {
      buffer = `${buffer}${text}`.slice(-8_192)
      if (promptEmitted) return
      const prompt = codexDevicePrompt(buffer)
      if (prompt === null) return
      promptEmitted = true
      onPrompt(prompt)
    }
    collectStream(child.stdout, consume)
    collectStream(child.stderr, consume)
    child.on("error", () => finish(1))
    child.on("close", (...args: unknown[]) =>
      finish(typeof args[0] === "number" ? args[0] : 1))
  })

  const connect = async (
    requestedRef: string | null,
    forceDeviceLogin: boolean,
    onPrompt: (prompt: Readonly<{ url: string; code: string }>) => void,
  ): Promise<string> => {
    if (disposed) throw new Error("pylon_runtime_unavailable")
    const accounts = await registry()
    const ref = requestedRef ?? nextCodexAccountRef(
      accounts.filter(account => account.provider === "codex").map(account => account.ref),
    )
    const projection = await runPylonAccountsConnect(
      summary,
      codexConnectArgs(ref, forceDeviceLogin),
      {
        env,
        runCodexDeviceLogin: input => runLogin(input, onPrompt),
      },
    )
    if (projection.deviceLogin.status === "blocked_invalid_auth") {
      throw new Error("credentials_invalid_relogin_incomplete")
    }
    return projection.accountRef
  }

  const cancel = (): void => {
    if (active !== null && !active.killed) active.kill("SIGTERM")
  }
  return {
    listAccounts,
    connect,
    cancel,
    dispose: () => {
      if (disposed) return
      disposed = true
      cancel()
    },
  }
}

const collectStream = (stream: NodeJS.ReadableStream | null, onChunk: (text: string) => void): void => {
  if (stream === null) return
  stream.on("data", (chunk: Buffer | string) => {
    onChunk(typeof chunk === "string" ? chunk : chunk.toString("utf8"))
  })
}

export type CodexConnectService = Readonly<{
  listAccounts: () => Promise<CodexAccountsResult>
  start: () => CodexConnectStatus
  /**
   * Re-auth an EXISTING registered account into its existing isolated home
   * (EP250 owner mandate: the UI owns reconnect). Receipted CLI behavior:
   * `pylon auth codex --account <ref> --force-device-login` targets the same
   * ref/home (apps/pylon/src/auth.ts accountRef ?? nextCodexAccountRef;
   * pylon-core account-connect runs device login into the SAME home when
   * forced). The ref must be one main itself listed from the registry.
   */
  startReconnect: (ref: string) => CodexConnectStatus
  status: () => CodexConnectStatus
  openVerification: () => Promise<boolean>
  dispose: () => void
}>

export const makeCodexConnectService = (
  _here: string,
  dependencies: CodexConnectServiceDependencies = {},
): CodexConnectService => {
  const spawnPylon = dependencies.spawnPylon ?? null
  const installedCustody = spawnPylon === null
    ? dependencies.installedCustody ?? makeInstalledCodexCustody()
    : null
  const listTimeoutMs = dependencies.listTimeoutMs ?? 120_000
  const connectTimeoutMs = dependencies.connectTimeoutMs ?? 15 * 60_000
  let current: CodexConnectStatus = { state: "idle" }
  let disposed = false
  let child: ChildLike | null = null
  let connectTimer: ReturnType<typeof setTimeout> | null = null
  // Refs main itself has listed from the pylon registry. Reconnect only
  // accepts one of these: the renderer can pick among refs main has seen,
  // never inject an arbitrary target.
  const knownRefs = new Set<string>()
  const listOperations = new Set<Readonly<{
    child: ChildLike
    finish: (result: CodexAccountsResult) => void
  }>>()

  const settle = (next: CodexConnectStatus): void => {
    // Terminal states win; late child output must not resurrect a flow.
    if (current.state === "connected" || current.state === "failed") return
    current = next
  }

  const clearChild = (): void => {
    if (connectTimer !== null) {
      clearTimeout(connectTimer)
      connectTimer = null
    }
    child = null
  }

  const rememberAccounts = (result: CodexAccountsResult): CodexAccountsResult => {
    if (result.state === "ok") {
      for (const account of result.accounts) knownRefs.add(account.ref)
    }
    return result
  }

  const listAccounts = (): Promise<CodexAccountsResult> => {
    if (disposed) return Promise.resolve(unavailableCodexAccountsResult())
    if (installedCustody !== null) {
      return installedCustody.listAccounts()
        .then(rememberAccounts)
        .catch(() => unavailableCodexAccountsResult())
    }
    return new Promise((resolve) => {
      if (disposed) {
        resolve(unavailableCodexAccountsResult())
        return
      }
      const listChild = spawnPylon?.(["codex", "accounts", "list", "--json"]) ?? null
      if (listChild === null) {
        resolve(unavailableCodexAccountsResult())
        return
      }
      let stdout = ""
      let done = false
      let timer: ReturnType<typeof setTimeout> | null = null
      let operation: Readonly<{ child: ChildLike; finish: (result: CodexAccountsResult) => void }> | null = null
      const finish = (result: CodexAccountsResult): void => {
        if (done) return
        done = true
        if (timer !== null) clearTimeout(timer)
        if (operation !== null) listOperations.delete(operation)
        resolve(rememberAccounts(result))
      }
      operation = { child: listChild, finish }
      listOperations.add(operation)
      timer = setTimeout(() => {
        listChild.kill("SIGTERM")
        finish(unavailableCodexAccountsResult())
      }, listTimeoutMs)
      collectStream(listChild.stdout, (text) => {
        stdout += text
      })
      collectStream(listChild.stderr, () => {})
      listChild.on("error", () => {
        finish(unavailableCodexAccountsResult())
      })
      listChild.on("close", () => {
        finish(parseAccountsListJson(stdout))
      })
    })
  }

  const launchDeviceAuth = (cliArgs: ReadonlyArray<string>): CodexConnectStatus => {
    if (disposed) return { state: "failed", reason: "pylon_runtime_unavailable" }
    if (current.state === "starting" || current.state === "awaiting_browser") {
      return current // single-flight: one device-auth attempt at a time
    }
    current = { state: "starting" }
    const connectChild = spawnPylon?.(cliArgs) ?? null
    if (connectChild === null) {
      current = { state: "failed", reason: "pylon_runtime_unavailable" }
      return current
    }
    child = connectChild
    const stdoutParser = createDeviceAuthStdoutParser()
    const stderrParser = createDeviceAuthStdoutParser()
    const applyEvents = (events: ReadonlyArray<DeviceAuthEvent>): void => {
      for (const event of events) {
        if (event.kind === "awaiting_browser") {
          settle({ state: "awaiting_browser", url: event.url, code: event.code })
        } else if (event.kind === "connected") {
          // A successful isolated Pylon account registration is authoritative
          // even when generic wrapper stderr or process-exit ordering races it.
          current = { state: "connected", ref: event.ref }
        } else if (current.state !== "connected") {
          current = { state: "failed", reason: event.reason }
        }
      }
    }
    collectStream(connectChild.stdout, (text) => applyEvents(stdoutParser.feed(text)))
    collectStream(connectChild.stderr, (text) => applyEvents(stderrParser.feed(text)))
    connectChild.on("error", () => {
      settle({ state: "failed", reason: "spawn_failed" })
      clearChild()
    })
    connectChild.on("close", (...args: unknown[]) => {
      applyEvents([...stdoutParser.end(), ...stderrParser.end()])
      const exitCode = typeof args[0] === "number" ? args[0] : null
      if (current.state !== "connected" && current.state !== "failed") {
        settle({
          state: "failed",
          reason: exitCode === 0 ? "ended_without_connection" : `exit_${exitCode ?? "signal"}`,
        })
      }
      clearChild()
    })
    connectTimer = setTimeout(() => {
      settle({ state: "failed", reason: "device_auth_timeout" })
      connectChild.kill("SIGTERM")
    }, connectTimeoutMs)
    return current
  }

  const launchInstalledDeviceAuth = (
    ref: string | null,
    forceDeviceLogin: boolean,
  ): CodexConnectStatus => {
    if (disposed || installedCustody === null) {
      return { state: "failed", reason: "pylon_runtime_unavailable" }
    }
    if (current.state === "starting" || current.state === "awaiting_browser") return current
    current = { state: "starting" }
    const timer = setTimeout(() => {
      settle({ state: "failed", reason: "device_auth_timeout" })
      installedCustody.cancel()
    }, connectTimeoutMs)
    void installedCustody.connect(ref, forceDeviceLogin, prompt => {
      settle({ state: "awaiting_browser", url: prompt.url, code: prompt.code })
    }).then(connectedRef => {
      if (!disposed && current.state !== "failed") {
        knownRefs.add(connectedRef)
        current = { state: "connected", ref: connectedRef }
      }
    }).catch(error => {
      const detail = publicSafeFailureDetail(error instanceof Error ? error.message : String(error))
      settle({ state: "failed", reason: detail === "" ? "connect_failed" : detail })
    }).finally(() => clearTimeout(timer))
    return current
  }

  const start = (): CodexConnectStatus => installedCustody === null
    ? launchDeviceAuth(["auth", "codex"])
    : launchInstalledDeviceAuth(null, false)

  const startReconnect = (ref: string): CodexConnectStatus => {
    if (disposed) return { state: "failed", reason: "pylon_runtime_unavailable" }
    if (current.state === "starting" || current.state === "awaiting_browser") {
      return current // single-flight holds across connect AND reconnect
    }
    // Grammar first, then membership in main's own listing — the renderer may
    // only pick among refs this service has itself read from the registry.
    if (!codexAccountRefPattern.test(ref)) {
      current = { state: "failed", reason: "invalid_account_ref" }
      return current
    }
    if (!knownRefs.has(ref)) {
      current = { state: "failed", reason: "unknown_account_ref" }
      return current
    }
    // Receipted per-ref re-auth (apps/pylon/src/auth.ts: --account targets
    // the existing ref; --force-device-login re-runs device auth into the
    // SAME isolated home even when a stale auth.json is present).
    return installedCustody === null
      ? launchDeviceAuth(["auth", "codex", "--account", ref, "--force-device-login"])
      : launchInstalledDeviceAuth(ref, true)
  }

  return {
    listAccounts,
    start,
    startReconnect,
    status: () => disposed ? { state: "failed", reason: "pylon_runtime_unavailable" } : current,
    openVerification: async () => {
      // The renderer sends no URL: main opens only the URL it parsed itself.
      if (disposed || current.state !== "awaiting_browser") return false
      if (dependencies.openExternal === undefined) return false
      await dependencies.openExternal(current.url)
      return true
    },
    dispose: () => {
      if (disposed) return
      disposed = true
      current = { state: "failed", reason: "pylon_runtime_unavailable" }
      if (child !== null && !child.killed) {
        child.kill("SIGTERM")
      }
      clearChild()
      for (const operation of [...listOperations]) {
        if (!operation.child.killed) operation.child.kill("SIGTERM")
        operation.finish(unavailableCodexAccountsResult())
      }
      installedCustody?.dispose()
    },
  }
}

// ---------------------------------------------------------------------------
// Smoke fixture (honest: used ONLY when the smoke harness asks for it; logs
// loudly from main). It never spawns anything — scripted device-auth output
// so the pixel receipts can show the awaiting_browser state headlessly.
// ---------------------------------------------------------------------------

export const fixtureAccountsListStdout = JSON.stringify({
  schema: "openagents.pylon.accounts_list.v0.3",
  observedAt: "2026-07-10T00:00:00.000Z",
  accounts: [
    {
      provider: "codex",
      accountRef: "codex-2",
      readiness: { state: "credentials_revoked" },
    },
    {
      provider: "codex",
      accountRef: "codex-b7d4438c",
      readiness: { state: "credentials_revoked" },
    },
    {
      provider: "claude_agent",
      accountRef: "claude-1",
      readiness: { state: "ready" },
    },
  ],
})

export const fixtureDeviceAuthStdout =
  "https://auth.openai.com/codex/device\n1234-ABCDE\n"

export const makeFixtureSpawnPylon = (): NonNullable<
  CodexConnectServiceDependencies["spawnPylon"]
> => {
  return (args) => {
    const isList = args[0] === "codex"
    const listeners = new Map<string, Array<(...values: unknown[]) => void>>()
    const stdoutHandlers: Array<(chunk: string) => void> = []
    const child: ChildLike = {
      stdout: {
        on: (event: string, listener: (chunk: string) => void) => {
          if (event === "data") stdoutHandlers.push(listener)
        },
      } as unknown as NodeJS.ReadableStream,
      stderr: null,
      on: (event, listener) => {
        const existing = listeners.get(event) ?? []
        listeners.set(event, [...existing, listener])
        return child
      },
      kill: () => true,
      killed: false,
      exitCode: null,
    }
    queueMicrotask(() => {
      for (const handler of stdoutHandlers) {
        handler(isList ? fixtureAccountsListStdout : fixtureDeviceAuthStdout)
      }
      if (isList) {
        for (const listener of listeners.get("close") ?? []) listener(0)
      }
      // The connect fixture stays open on awaiting_browser: a headless smoke
      // cannot complete a real browser device-auth, and must not pretend to.
    })
    return child
  }
}
