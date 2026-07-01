import { spawn } from "node:child_process"
import { fileURLToPath } from "node:url"

import type {
  KhalaCodeDesktopCodexAppServerControlResult,
  KhalaCodeDesktopCodexAppServerStatus,
} from "../shared/rpc.js"
import { resolveCodexHomePath } from "./codex-rate-limits.js"
import { khalaCodeConfigFromRuntimeEnv } from "./khala-code-config.js"

export const KHALA_CODE_CODEX_APP_SERVER_ADAPTER_VERSION =
  "codex-app-server-v2-2026-07-01"

const DEFAULT_REQUEST_TIMEOUT_MS = 30_000
const DEFAULT_INITIALIZE_TIMEOUT_MS = 10_000
const MAX_DIAGNOSTIC_LINES = 80

type JsonRpcId = number | string

type JsonRpcResponse = {
  readonly id?: JsonRpcId
  readonly result?: unknown
  readonly error?: {
    readonly code?: number
    readonly message?: string
    readonly data?: unknown
  }
}

type JsonRpcNotification = {
  readonly id?: JsonRpcId
  readonly method?: string
  readonly params?: unknown
}

type ProcessStream = {
  readonly on: (event: "data", listener: (chunk: Buffer) => void) => unknown
  readonly off: (event: "data", listener: (chunk: Buffer) => void) => unknown
}

type CodexAppServerChild = {
  readonly pid?: number
  readonly stdout: ProcessStream
  readonly stderr: ProcessStream
  readonly stdin: {
    readonly write: (line: string) => unknown
  }
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
) => CodexAppServerChild

type PendingRequest = {
  readonly method: string
  readonly reject: (error: Error) => void
  readonly resolve: (value: unknown) => void
  readonly timeout: ReturnType<typeof setTimeout>
}

export type CodexAppServerNotification = Readonly<{
  id?: JsonRpcId
  method: string
  params: unknown
  receivedAt: string
}>

export type CodexAppServerNotificationHandler = (
  notification: CodexAppServerNotification,
) => void

export type CodexAppServerHost = Readonly<{
  dispose: () => void
  request: <Result = unknown>(
    method: string,
    params?: unknown,
    options?: { readonly timeoutMs?: number },
  ) => Promise<Result>
  respondToServerRequest: (
    id: JsonRpcId,
    result: unknown,
  ) => void
  restart: () => Promise<KhalaCodeDesktopCodexAppServerControlResult>
  start: () => Promise<KhalaCodeDesktopCodexAppServerControlResult>
  status: () => KhalaCodeDesktopCodexAppServerStatus
  stop: () => Promise<KhalaCodeDesktopCodexAppServerControlResult>
  subscribe: (handler: CodexAppServerNotificationHandler) => () => void
}>

export type CreateCodexAppServerHostOptions = {
  readonly codexArgs?: readonly string[]
  readonly clientInfo?: {
    readonly name: string
    readonly title: string
    readonly version: string
  }
  readonly codexCommand?: string
  readonly codexHomePath?: string | null
  readonly env?: NodeJS.ProcessEnv
  readonly initializeTimeoutMs?: number
  readonly requestTimeoutMs?: number
  readonly spawnFn?: SpawnFn
}

const defaultClientInfo = {
  name: "khala_code_desktop",
  title: "Khala Code Desktop",
  version: "0.1.0",
}

const DEFAULT_CODEX_APP_SERVER_ARGS = ["app-server", "--stdio"] as const

const DEFAULT_FIXTURE_APP_SERVER_PATH = fileURLToPath(
  new URL("./fixture-codex-app-server.ts", import.meta.url),
)

const isoNow = (): string => new Date().toISOString()

const configuredCodexCommand = (
  env: NodeJS.ProcessEnv,
  explicit?: string,
): string => {
  const explicitCommand = explicit?.trim()
  if (explicitCommand !== undefined && explicitCommand.length > 0) return explicitCommand
  const binary = env.KHALA_CODE_CODEX_BINARY?.trim()
  if (binary !== undefined && binary.length > 0) return binary
  const command = env.KHALA_CODE_CODEX_COMMAND?.trim()
  if (command !== undefined && command.length > 0) return command
  return "codex"
}

const fixtureEnabled = (env: NodeJS.ProcessEnv): boolean => {
  const value = env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE?.trim().toLowerCase()
  return value === "1" || value === "true" || value === "yes"
}

const configuredLaunch = (
  env: NodeJS.ProcessEnv,
  explicitCommand?: string,
  explicitArgs?: readonly string[],
): { readonly args: readonly string[]; readonly command: string } => {
  if (explicitArgs !== undefined) {
    return {
      args: explicitArgs,
      command: configuredCodexCommand(env, explicitCommand),
    }
  }
  if (fixtureEnabled(env)) {
    return {
      args: [
        env.KHALA_CODE_CODEX_APP_SERVER_FIXTURE_PATH?.trim() || DEFAULT_FIXTURE_APP_SERVER_PATH,
        "--stdio",
      ],
      command:
        explicitCommand?.trim() ||
        env.KHALA_CODE_BUN_BINARY?.trim() ||
        process.execPath,
    }
  }
  return {
    args: DEFAULT_CODEX_APP_SERVER_ARGS,
    command: configuredCodexCommand(env, explicitCommand),
  }
}

const appendDiagnostic = (
  lines: string[],
  line: string,
): readonly string[] => {
  const trimmed = line.trimEnd()
  if (trimmed.length === 0) return lines
  lines.push(trimmed)
  if (lines.length > MAX_DIAGNOSTIC_LINES) lines.splice(0, lines.length - MAX_DIAGNOSTIC_LINES)
  return lines
}

const diagnosticErrorMessage = (error: unknown): string => {
  const message = error instanceof Error ? error.message : String(error)
  return message.slice(0, 240)
}

export function createCodexAppServerHost(
  options: CreateCodexAppServerHostOptions = {},
): CodexAppServerHost {
  const env = options.env ?? khalaCodeConfigFromRuntimeEnv().env
  const codexLaunch = configuredLaunch(env, options.codexCommand, options.codexArgs)
  const codexCommand = codexLaunch.command
  const codexArgs = codexLaunch.args
  const codexHome = resolveCodexHomePath(options.codexHomePath, env)
  const spawnFn: SpawnFn =
    options.spawnFn ??
    ((command, args, spawnOptions) =>
      spawn(command, [...args], spawnOptions) as unknown as CodexAppServerChild)
  const requestTimeoutMs = options.requestTimeoutMs ?? DEFAULT_REQUEST_TIMEOUT_MS
  const initializeTimeoutMs = options.initializeTimeoutMs ?? DEFAULT_INITIALIZE_TIMEOUT_MS
  const clientInfo = options.clientInfo ?? defaultClientInfo

  let state: KhalaCodeDesktopCodexAppServerStatus["state"] = "stopped"
  let initialized = false
  let child: CodexAppServerChild | null = null
  let nextRequestId = 0
  let stdoutBuffer = ""
  let stderrBuffer = ""
  let expectedStop = false
  let initializeResult: unknown = null
  let lastError: string | null = null
  const diagnostics: string[] = []
  const pending = new Map<JsonRpcId, PendingRequest>()
  const subscribers = new Set<CodexAppServerNotificationHandler>()

  const status = (): KhalaCodeDesktopCodexAppServerStatus => ({
    ok: true,
    app: "Khala Code Desktop",
    adapterVersion: KHALA_CODE_CODEX_APP_SERVER_ADAPTER_VERSION,
    codexCommand,
    codexHome,
    diagnostics: [...diagnostics],
    initialized,
    lastError,
    pendingRequestCount: pending.size,
    pid: child?.pid ?? null,
    state,
    transport: "stdio",
    initializeResult,
  })

  const rejectAllPending = (error: Error): void => {
    for (const request of pending.values()) {
      clearTimeout(request.timeout)
      request.reject(error)
    }
    pending.clear()
  }

  const cleanupChild = (): void => {
    if (child === null) return
    child.stdout.off("data", onStdoutData)
    child.stderr.off("data", onStderrData)
    child.off("error", onError)
    child.off("close", onClose)
  }

  const transitionError = (message: string): void => {
    state = "errored"
    initialized = false
    lastError = message
    appendDiagnostic(diagnostics, `error: ${message}`)
  }

  const sendNotification = (method: string, params: unknown = {}): void => {
    if (child === null) throw new Error("Codex app-server is not running")
    child.stdin.write(`${JSON.stringify({ method, params })}\n`)
  }

  const request = async <Result = unknown>(
    method: string,
    params: unknown = {},
    requestOptions: { readonly timeoutMs?: number } = {},
  ): Promise<Result> => {
    if (child === null || state !== "running") {
      throw new Error("Codex app-server is not running")
    }
    const id = ++nextRequestId
    const timeout = setTimeout(() => {
      const pendingRequest = pending.get(id)
      if (pendingRequest === undefined) return
      pending.delete(id)
      const error = new Error(`Codex app-server request timed out: ${pendingRequest.method}`)
      lastError = error.message
      pendingRequest.reject(error)
    }, requestOptions.timeoutMs ?? requestTimeoutMs)

    const promise = new Promise<Result>((resolve, reject) => {
      pending.set(id, {
        method,
        resolve: value => resolve(value as Result),
        reject,
        timeout,
      })
    })
    child.stdin.write(`${JSON.stringify({ id, method, params })}\n`)
    return promise
  }

  const respondToServerRequest = (
    id: JsonRpcId,
    result: unknown,
  ): void => {
    if (child === null || state !== "running") {
      throw new Error("Codex app-server is not running")
    }
    child.stdin.write(`${JSON.stringify({ id, result })}\n`)
  }

  const handleResponse = (message: JsonRpcResponse): void => {
    if (message.id === undefined) return
    const pendingRequest = pending.get(message.id)
    if (pendingRequest === undefined) {
      appendDiagnostic(diagnostics, `unknown response id: ${message.id}`)
      return
    }
    pending.delete(message.id)
    clearTimeout(pendingRequest.timeout)
    if (message.error !== undefined) {
      const error = new Error(
        message.error.message ?? `Codex app-server request failed: ${pendingRequest.method}`,
      )
      lastError = error.message
      pendingRequest.reject(error)
      return
    }
    pendingRequest.resolve(message.result ?? {})
  }

  const handleNotification = (message: JsonRpcNotification): void => {
    if (typeof message.method !== "string" || message.method.trim().length === 0) {
      appendDiagnostic(diagnostics, "ignored malformed app-server notification")
      return
    }
    const notification: CodexAppServerNotification = {
      method: message.method,
      params: message.params ?? {},
      receivedAt: isoNow(),
      ...(message.id === undefined ? {} : { id: message.id }),
    }
    for (const subscriber of subscribers) {
      try {
        subscriber(notification)
      } catch (error) {
        const diagnostic = `notification subscriber failed for ${notification.method}: ${diagnosticErrorMessage(error)}`
        appendDiagnostic(diagnostics, diagnostic)
        if (env.KHALA_CODE_DEBUG_APP_SERVER === "1") console.debug(diagnostic)
      }
    }
  }

  function onStdoutData(chunk: Buffer): void {
    stdoutBuffer += chunk.toString()
    let newlineIndex = stdoutBuffer.indexOf("\n")
    while (newlineIndex !== -1) {
      const line = stdoutBuffer.slice(0, newlineIndex).trim()
      stdoutBuffer = stdoutBuffer.slice(newlineIndex + 1)
      newlineIndex = stdoutBuffer.indexOf("\n")
      if (line.length === 0) continue
      let message: JsonRpcResponse & JsonRpcNotification
      try {
        message = JSON.parse(line) as JsonRpcResponse & JsonRpcNotification
      } catch {
        appendDiagnostic(diagnostics, `invalid stdout JSON: ${line.slice(0, 240)}`)
        continue
      }
      if (typeof message.method === "string") {
        handleNotification(message)
      } else if (message.id !== undefined) {
        handleResponse(message)
      } else {
        appendDiagnostic(diagnostics, "ignored stdout JSON without id or method")
      }
    }
  }

  function onStderrData(chunk: Buffer): void {
    stderrBuffer += chunk.toString()
    let newlineIndex = stderrBuffer.indexOf("\n")
    while (newlineIndex !== -1) {
      const line = stderrBuffer.slice(0, newlineIndex)
      stderrBuffer = stderrBuffer.slice(newlineIndex + 1)
      newlineIndex = stderrBuffer.indexOf("\n")
      appendDiagnostic(diagnostics, line)
    }
    if (stderrBuffer.length > 20_000) {
      appendDiagnostic(diagnostics, stderrBuffer.slice(-20_000))
      stderrBuffer = ""
    }
  }

  function onError(error: Error): void {
    const code = (error as NodeJS.ErrnoException).code
    transitionError(code === "ENOENT" ? "Codex CLI not found" : error.message)
    rejectAllPending(error)
  }

  function onClose(code: number | null, signal: NodeJS.Signals | null): void {
    cleanupChild()
    const wasExpected = expectedStop
    child = null
    initialized = false
    if (wasExpected) {
      state = "stopped"
      expectedStop = false
      rejectAllPending(new Error("Codex app-server stopped"))
      return
    }
    const message = `Codex app-server exited before stop request (code ${code ?? "unknown"}, signal ${signal ?? "none"})`
    transitionError(message)
    rejectAllPending(new Error(message))
  }

  const start = async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => {
    if (state === "running" && initialized) {
      return { ok: true, status: status(), action: "start", changed: false }
    }
    if (state === "starting") {
      return { ok: false, status: status(), action: "start", changed: false, error: "Codex app-server is already starting" }
    }

    state = "starting"
    initialized = false
    lastError = null
    expectedStop = false
    stdoutBuffer = ""
    stderrBuffer = ""
    try {
      const spawned = spawnFn(codexCommand, codexArgs, {
        env: {
          ...env,
          CODEX_HOME: codexHome,
          LOG_FORMAT: env.LOG_FORMAT ?? "json",
        },
        stdio: ["pipe", "pipe", "pipe"],
      })
      child = spawned
    } catch (error) {
      transitionError(error instanceof Error ? error.message : String(error))
      return { ok: false, status: status(), action: "start", changed: false, error: lastError ?? "spawn failed" }
    }

    const runningChild = child
    runningChild.stdout.on("data", onStdoutData)
    runningChild.stderr.on("data", onStderrData)
    runningChild.on("error", onError)
    runningChild.on("close", onClose)
    state = "running"

    try {
      initializeResult = await request("initialize", {
        clientInfo,
        capabilities: {
          experimentalApi: true,
          mcpServerOpenaiFormElicitation: true,
        },
      }, { timeoutMs: initializeTimeoutMs })
      sendNotification("initialized")
      initialized = true
      return { ok: true, status: status(), action: "start", changed: true }
    } catch (error) {
      transitionError(error instanceof Error ? error.message : String(error))
      child?.kill()
      return {
        ok: false,
        status: status(),
        action: "start",
        changed: true,
        error: lastError ?? "Codex app-server initialize failed",
      }
    }
  }

  const stop = async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => {
    if (child === null) {
      state = "stopped"
      initialized = false
      return { ok: true, status: status(), action: "stop", changed: false }
    }
    expectedStop = true
    child.kill()
    cleanupChild()
    child = null
    initialized = false
    state = "stopped"
    rejectAllPending(new Error("Codex app-server stopped"))
    return { ok: true, status: status(), action: "stop", changed: true }
  }

  const restart = async (): Promise<KhalaCodeDesktopCodexAppServerControlResult> => {
    await stop()
    const result = await start()
    return { ...result, action: "restart" }
  }

  const dispose = (): void => {
    if (child !== null) {
      expectedStop = true
      child.kill()
      cleanupChild()
      child = null
    }
    initialized = false
    state = "stopped"
    rejectAllPending(new Error("Codex app-server disposed"))
  }

  const subscribe = (handler: CodexAppServerNotificationHandler): (() => void) => {
    subscribers.add(handler)
    return () => {
      subscribers.delete(handler)
    }
  }

  return {
    dispose,
    request,
    respondToServerRequest,
    restart,
    start,
    status,
    stop,
    subscribe,
  }
}
