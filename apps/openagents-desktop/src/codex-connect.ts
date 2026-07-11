/**
 * Main-process-only Codex account connect service (#8574, #8640 unblock).
 *
 * Wraps the proven Pylon isolated device-auth flow by spawning the pylon CLI
 * from the repo root:
 *
 *   bun apps/pylon/src/index.ts codex accounts list --json   (readiness list)
 *   bun apps/pylon/src/index.ts auth codex                   (device-auth connect)
 *
 * SAFETY (repo law): this service NEVER sets or touches the default
 * `~/.codex` home. The pylon flow itself owns credential custody and runs
 * `codex login --device-auth` with an isolated per-account
 * `CODEX_HOME=<pylon home>/accounts/codex/<ref>` (see
 * packages/pylon-core/src/custody/account-connect.ts). We add no CODEX_HOME
 * of our own and read no credential files. Only public-safe projections
 * (refs, readiness states, verification URL + user code, typed status) leave
 * this module; the connect success line's email is deliberately dropped.
 */
import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import path from "node:path"

import {
  codexAccountRefPattern,
  codexUserCodePattern,
  type CodexAccountsResult,
  type CodexConnectStatus,
  unavailableCodexAccountsResult,
} from "./codex-connect-contract.ts"

// ---------------------------------------------------------------------------
// Device-auth stdout parser (pure; unit-tested against the pylon CLI format).
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
      return { kind: "failed", reason: "pylon_auth_failed" }
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
  /** Spawn a pylon CLI invocation; overridable for tests/fixture mode. */
  spawnPylon?: (args: ReadonlyArray<string>) => ChildLike | null
  openExternal?: (url: string) => Promise<void>
  listTimeoutMs?: number
  connectTimeoutMs?: number
}>

const repoRootFromHere = (here: string): string | null => {
  // dist/main.js -> apps/openagents-desktop -> repo root
  const root = path.resolve(here, "..", "..", "..")
  return existsSync(path.join(root, "apps", "pylon", "src", "index.ts")) ? root : null
}

const defaultSpawnPylon = (here: string) => (args: ReadonlyArray<string>): ChildLike | null => {
  const root = repoRootFromHere(here)
  if (root === null) return null
  // No CODEX_HOME injection here — pylon assigns the isolated per-account
  // home itself. Inherited env only.
  return spawn("bun", ["apps/pylon/src/index.ts", ...args], {
    cwd: root,
    stdio: ["ignore", "pipe", "pipe"],
    env: process.env,
  }) as unknown as ChildLike
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
  status: () => CodexConnectStatus
  openVerification: () => Promise<boolean>
  dispose: () => void
}>

export const makeCodexConnectService = (
  here: string,
  dependencies: CodexConnectServiceDependencies = {},
): CodexConnectService => {
  const spawnPylon = dependencies.spawnPylon ?? defaultSpawnPylon(here)
  const listTimeoutMs = dependencies.listTimeoutMs ?? 120_000
  const connectTimeoutMs = dependencies.connectTimeoutMs ?? 15 * 60_000
  let current: CodexConnectStatus = { state: "idle" }
  let disposed = false
  let child: ChildLike | null = null
  let connectTimer: ReturnType<typeof setTimeout> | null = null
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

  const listAccounts = (): Promise<CodexAccountsResult> =>
    new Promise((resolve) => {
      if (disposed) {
        resolve(unavailableCodexAccountsResult())
        return
      }
      const listChild = spawnPylon(["codex", "accounts", "list", "--json"])
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
        resolve(result)
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

  const start = (): CodexConnectStatus => {
    if (disposed) return { state: "failed", reason: "pylon_runtime_unavailable" }
    if (current.state === "starting" || current.state === "awaiting_browser") {
      return current // single-flight: one device-auth attempt at a time
    }
    current = { state: "starting" }
    const connectChild = spawnPylon(["auth", "codex"])
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

  return {
    listAccounts,
    start,
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
