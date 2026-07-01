import { spawn } from "node:child_process"
import { readFile } from "node:fs/promises"
import { join } from "node:path"

import type { KhalaCodeDesktopCodexHarnessStatus } from "../shared/rpc.js"
import { resolveCodexHomePath } from "./codex-rate-limits.js"

const CODEX_VERSION_TIMEOUT_MS = 5_000

type ProcessStream = {
  readonly on: (event: "data", listener: (chunk: Buffer) => void) => unknown
  readonly off: (event: "data", listener: (chunk: Buffer) => void) => unknown
}

type CodexHarnessChild = {
  readonly stdout: ProcessStream
  readonly stderr: ProcessStream
  readonly kill: () => unknown
  readonly on: (
    event: "close" | "error",
    listener: ((code: number | null, signal: NodeJS.Signals | null) => void) | ((error: Error) => void),
  ) => unknown
  readonly off: (
    event: "close" | "error",
    listener: ((code: number | null, signal: NodeJS.Signals | null) => void) | ((error: Error) => void),
  ) => unknown
}

type SpawnFn = (
  command: string,
  args: readonly string[],
  options: Parameters<typeof spawn>[2],
) => CodexHarnessChild

type ReadTextFileFn = (path: string, encoding: "utf8") => Promise<string>

type CodexAuthShape = {
  readonly tokens?: {
    readonly access_token?: string
    readonly refresh_token?: string
    readonly account_id?: string
  }
}

type VersionProbe =
  | {
      readonly available: true
      readonly version: string | null
      readonly error: null
    }
  | {
      readonly available: false
      readonly version: null
      readonly error: string
      readonly missing: boolean
    }

export type InspectCodexHarnessStatusOptions = {
  readonly codexCommand?: string
  readonly codexHomePath?: string | null
  readonly env?: NodeJS.ProcessEnv
  readonly now?: () => Date
  readonly readFileFn?: ReadTextFileFn
  readonly spawnFn?: SpawnFn
  readonly timeoutMs?: number
}

const isoNow = (now: () => Date = () => new Date()): string => now().toISOString()

const configuredCodexCommand = (
  env: NodeJS.ProcessEnv,
  explicit?: string,
): {
  readonly command: string
  readonly source: KhalaCodeDesktopCodexHarnessStatus["binary"]["source"]
} => {
  const explicitCommand = explicit?.trim()
  if (explicitCommand !== undefined && explicitCommand.length > 0) {
    return { command: explicitCommand, source: "input" }
  }
  const binary = env.KHALA_CODE_CODEX_BINARY?.trim()
  if (binary !== undefined && binary.length > 0) {
    return { command: binary, source: "env:KHALA_CODE_CODEX_BINARY" }
  }
  const command = env.KHALA_CODE_CODEX_COMMAND?.trim()
  if (command !== undefined && command.length > 0) {
    return { command, source: "env:KHALA_CODE_CODEX_COMMAND" }
  }
  return { command: "codex", source: "PATH" }
}

const codexHomeSource = (
  env: NodeJS.ProcessEnv,
  explicit?: string | null,
): KhalaCodeDesktopCodexHarnessStatus["home"]["source"] => {
  if (explicit !== undefined && explicit !== null && explicit.trim().length > 0) return "input"
  return env.CODEX_HOME?.trim() ? "env:CODEX_HOME" : "default:~/.codex"
}

const firstNonEmptyLine = (text: string): string | null =>
  text.split(/\r?\n/).map(line => line.trim()).find(line => line.length > 0) ?? null

async function probeCodexVersion(
  command: string,
  options: InspectCodexHarnessStatusOptions,
): Promise<VersionProbe> {
  const spawnFn = options.spawnFn ?? spawn
  const child = spawnFn(command, ["--version"], {
    env: options.env ?? process.env,
    stdio: ["ignore", "pipe", "pipe"],
  })

  return await new Promise<VersionProbe>(resolve => {
    let stdout = ""
    let stderr = ""
    let settled = false

    const cleanup = () => {
      child.stdout.off("data", onStdoutData)
      child.stderr.off("data", onStderrData)
      child.off("error", onError)
      child.off("close", onClose)
      clearTimeout(timeout)
    }

    const settle = (result: VersionProbe, kill = false) => {
      if (settled) return
      settled = true
      cleanup()
      if (kill) child.kill()
      resolve(result)
    }

    const timeout = setTimeout(() => {
      settle({
        available: false,
        version: null,
        error: "Codex version probe timed out",
        missing: false,
      }, true)
    }, options.timeoutMs ?? CODEX_VERSION_TIMEOUT_MS)

    function onStdoutData(chunk: Buffer): void {
      stdout += chunk.toString()
      if (stdout.length > 20_000) stdout = stdout.slice(-20_000)
    }

    function onStderrData(chunk: Buffer): void {
      stderr += chunk.toString()
      if (stderr.length > 20_000) stderr = stderr.slice(-20_000)
    }

    function onError(error: Error): void {
      const code = (error as NodeJS.ErrnoException).code
      settle({
        available: false,
        version: null,
        error: code === "ENOENT" ? "Codex CLI not found" : error.message,
        missing: code === "ENOENT",
      })
    }

    function onClose(code: number | null): void {
      if (code === 0) {
        settle({
          available: true,
          version: firstNonEmptyLine(stdout) ?? firstNonEmptyLine(stderr),
          error: null,
        })
        return
      }
      settle({
        available: false,
        version: null,
        error: firstNonEmptyLine(stderr) ?? `Codex version probe exited with code ${code ?? "unknown"}`,
        missing: false,
      })
    }

    child.stdout.on("data", onStdoutData)
    child.stderr.on("data", onStderrData)
    child.on("error", onError)
    child.on("close", onClose)
  })
}

const readAuthState = async (
  authPath: string,
  readText: ReadTextFileFn,
): Promise<KhalaCodeDesktopCodexHarnessStatus["auth"]> => {
  try {
    const raw = await readText(authPath, "utf8")
    const parsed = JSON.parse(raw) as CodexAuthShape
    const accessTokenPresent = typeof parsed.tokens?.access_token === "string" &&
      parsed.tokens.access_token.trim().length > 0
    const refreshTokenPresent = typeof parsed.tokens?.refresh_token === "string" &&
      parsed.tokens.refresh_token.trim().length > 0
    const accountIdPresent = typeof parsed.tokens?.account_id === "string" &&
      parsed.tokens.account_id.trim().length > 0
    if (!accessTokenPresent && !refreshTokenPresent) {
      return {
        state: "invalid",
        blockerRefs: ["blocker.codex.auth_json_missing_tokens"],
        accessTokenPresent,
        accountIdPresent,
        refreshTokenPresent,
        error: "Codex auth.json is present but does not contain usable tokens.",
      }
    }
    return {
      state: "ready",
      blockerRefs: [],
      accessTokenPresent,
      accountIdPresent,
      refreshTokenPresent,
    }
  } catch (error) {
    const code = (error as NodeJS.ErrnoException).code
    if (code === "ENOENT") {
      return {
        state: "credentials_missing",
        blockerRefs: ["blocker.codex.credentials_missing"],
        accessTokenPresent: false,
        accountIdPresent: false,
        refreshTokenPresent: false,
        error: "Codex auth.json is missing.",
      }
    }
    if (error instanceof SyntaxError) {
      return {
        state: "invalid",
        blockerRefs: ["blocker.codex.auth_json_invalid"],
        accessTokenPresent: false,
        accountIdPresent: false,
        refreshTokenPresent: false,
        error: "Codex auth.json is not valid JSON.",
      }
    }
    return {
      state: "error",
      blockerRefs: ["blocker.codex.auth_json_read_error"],
      accessTokenPresent: false,
      accountIdPresent: false,
      refreshTokenPresent: false,
      error: error instanceof Error ? error.message : String(error),
    }
  }
}

export async function inspectCodexHarnessStatus(
  options: InspectCodexHarnessStatusOptions = {},
): Promise<KhalaCodeDesktopCodexHarnessStatus> {
  const env = options.env ?? process.env
  const now = options.now ?? (() => new Date())
  const observedAt = isoNow(now)
  const { command, source } = configuredCodexCommand(env, options.codexCommand)
  const codexHomePath = resolveCodexHomePath(options.codexHomePath, env)
  const authPath = join(codexHomePath, "auth.json")
  const version = await probeCodexVersion(command, options)
  const readText = options.readFileFn ?? ((path, encoding) => readFile(path, encoding))
  const auth = await readAuthState(authPath, readText)

  const status =
    !version.available
      ? "unavailable" as const
      : auth.state === "ready"
        ? "ready" as const
        : auth.state === "error"
          ? "error" as const
          : "unavailable" as const
  const available = status === "ready"
  const reason = !version.available
    ? `${version.error}. Install Codex with npm install -g @openai/codex, or configure KHALA_CODE_CODEX_BINARY/KHALA_CODE_CODEX_COMMAND.`
    : auth.state === "ready"
      ? "Codex CLI is installed and the primary user Codex home has auth state."
      : `${auth.error ?? "Codex is not signed in."} Run codex login intentionally for the primary user Codex home before using Khala Code chat.`

  return {
    ok: true,
    app: "Khala Code Desktop",
    capability: "codex_harness",
    available,
    observedAt,
    reason,
    status,
    binary: {
      command,
      source,
      available: version.available,
      version: version.version,
      error: version.error,
    },
    home: {
      path: codexHomePath,
      source: codexHomeSource(env, options.codexHomePath),
      role: "main_user_codex_home",
      authPath,
      fleetIsolation: "fleet_accounts_use_pylon_isolated_homes",
    },
    auth,
    signIn: {
      required: auth.state !== "ready",
      command: "codex login",
      warning: "Run codex login yourself for the primary user Codex session; Khala Code uses separate device-auth only for isolated Pylon worker homes.",
    },
  }
}
