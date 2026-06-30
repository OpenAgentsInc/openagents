import { spawn } from "node:child_process"
import { existsSync } from "node:fs"
import { readFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"

import type {
  KhalaCodexRateLimitProviderStatus,
  KhalaCodexRateLimitResetCredits,
  KhalaCodexRateLimitResetOutcome,
  KhalaCodexRateLimitWindow,
} from "../shared/codex-rate-limits.js"

const CODEX_RPC_TIMEOUT_MS = 10_000
const CODEX_RATE_LIMIT_RESET_CREDITS_URL =
  "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits"
const CODEX_RATE_LIMIT_RESET_CREDITS_CONSUME_URL =
  "https://chatgpt.com/backend-api/wham/rate-limit-reset-credits/consume"

type RpcResponse = {
  readonly id?: number
  readonly result?: unknown
  readonly error?: { readonly message?: string }
}

type RpcRateWindow = {
  readonly usedPercent?: number
  readonly windowDurationMins?: number
  readonly resetsAt?: number
}

type RpcRateLimitsResponse = {
  readonly rateLimits?: {
    readonly primary?: RpcRateWindow
    readonly secondary?: RpcRateWindow
  }
  readonly rateLimitResetCredits?: RawRpcRateLimitResetCredits | null
}

type RawRpcRateLimitResetCredits = {
  readonly availableCount?: number
  readonly totalEarnedCount?: number
  readonly nextExpiresAt?: number | string | null
  readonly credits?: readonly {
    readonly status?: string
    readonly expiresAt?: number | string | null
    readonly grantedAt?: number | string | null
  }[]
}

type RawBackendRateLimitResetCredits = {
  readonly available_count?: number
  readonly total_earned_count?: number
  readonly credits?: readonly {
    readonly status?: string
    readonly expires_at?: string | null
    readonly granted_at?: string | null
  }[]
}

type RawBackendConsumeRateLimitResetCredit = {
  readonly code?: string
}

type CodexAuthFile = {
  readonly tokens?: {
    readonly access_token?: string
    readonly account_id?: string
  }
}

type ProcessStream = {
  readonly on: (event: "data", listener: (chunk: Buffer) => void) => unknown
  readonly off: (event: "data", listener: (chunk: Buffer) => void) => unknown
}

type CodexRateLimitChild = {
  readonly stdout: ProcessStream
  readonly stderr: ProcessStream
  readonly stdin: { readonly write: (line: string) => unknown }
  readonly kill: () => unknown
  readonly on: (
    event: "close" | "error",
    listener: ((error: Error) => void) | (() => void),
  ) => unknown
  readonly off: (
    event: "close" | "error",
    listener: ((error: Error) => void) | (() => void),
  ) => unknown
}

type SpawnFn = (
  command: string,
  args: readonly string[],
  options: Parameters<typeof spawn>[2],
) => CodexRateLimitChild

type ReadTextFileFn = (path: string, encoding: "utf8") => Promise<string>

export type FetchKhalaCodexRateLimitStatusOptions = {
  readonly authExists?: (codexHomePath: string) => boolean
  readonly codexCommand?: string
  readonly codexHomePath?: string | null
  readonly env?: NodeJS.ProcessEnv
  readonly fetchFn?: typeof fetch
  readonly now?: () => Date
  readonly readFileFn?: ReadTextFileFn
  readonly spawnFn?: SpawnFn
  readonly timeoutMs?: number
}

export type ConsumeKhalaCodexRateLimitResetCreditOptions = {
  readonly codexHomePath?: string | null
  readonly env?: NodeJS.ProcessEnv
  readonly fetchFn?: typeof fetch
  readonly idempotencyKey: string
  readonly now?: () => Date
  readonly readFileFn?: ReadTextFileFn
}

const isoNow = (now: () => Date = () => new Date()): string => now().toISOString()

export function resolveCodexHomePath(
  codexHomePath?: string | null,
  env: NodeJS.ProcessEnv = process.env,
): string {
  const configured = codexHomePath ?? env.CODEX_HOME
  return configured && configured.trim().length > 0
    ? configured
    : join(homedir(), ".codex")
}

function defaultCodexAuthExists(codexHomePath: string): boolean {
  return existsSync(join(codexHomePath, "auth.json"))
}

function normalizePercent(value: number): number {
  return Math.min(100, Math.max(0, value))
}

function timestampFromUnknown(value: number | string | null | undefined): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value < 10_000_000_000 ? value * 1000 : value
  }
  if (typeof value !== "string" || value.trim().length === 0) return null
  const numeric = Number(value)
  if (Number.isFinite(numeric)) return numeric < 10_000_000_000 ? numeric * 1000 : numeric
  const parsed = Date.parse(value)
  return Number.isFinite(parsed) ? parsed : null
}

function isoFromTimestamp(value: number | string | null | undefined): string | null {
  const timestamp = timestampFromUnknown(value)
  if (timestamp === null) return null
  const date = new Date(timestamp)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

function resetDescriptionFrom(timestamp: number | null, now: Date): string | null {
  if (timestamp === null) return null
  const date = new Date(timestamp)
  if (Number.isNaN(date.getTime())) return null
  const isToday = date.toDateString() === now.toDateString()
  return isToday
    ? date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })
    : date.toLocaleDateString(undefined, {
        weekday: "short",
        hour: "numeric",
        minute: "2-digit",
      })
}

function mapRpcWindow(
  raw: RpcRateWindow | undefined,
  expectedWindowMinutes: number,
  now: Date,
): KhalaCodexRateLimitWindow | null {
  if (raw === undefined || typeof raw.usedPercent !== "number") return null
  const resetsAt = timestampFromUnknown(raw.resetsAt)
  const usedPercent = normalizePercent(raw.usedPercent)
  return {
    usedPercent,
    remainingPercent: Math.max(0, 100 - usedPercent),
    windowMinutes: expectedWindowMinutes,
    resetsAtIso: isoFromTimestamp(resetsAt),
    resetDescription: resetDescriptionFrom(resetsAt, now),
  }
}

function normalizeCreditStatus(status: string | undefined): string {
  return status?.toLowerCase() ?? "unknown"
}

function nextAvailableCreditExpiry(
  credits: KhalaCodexRateLimitResetCredits["credits"] | undefined,
): string | null {
  const expiries =
    credits
      ?.filter(credit => credit.status === "available" && credit.expiresAtIso !== null)
      .map(credit => Date.parse(credit.expiresAtIso ?? ""))
      .filter(Number.isFinite)
      .sort((left, right) => left - right) ?? []
  return expiries.length === 0 ? null : new Date(expiries[0]).toISOString()
}

function mapRpcResetCredits(
  raw: RawRpcRateLimitResetCredits | null | undefined,
): KhalaCodexRateLimitResetCredits | null | undefined {
  if (raw === null || raw === undefined) return raw
  if (typeof raw.availableCount !== "number" || !Number.isFinite(raw.availableCount)) {
    return null
  }
  const credits = raw.credits?.map(credit => ({
    status: normalizeCreditStatus(credit.status),
    expiresAtIso: isoFromTimestamp(credit.expiresAt),
    grantedAtIso: isoFromTimestamp(credit.grantedAt),
  }))
  return {
    availableCount: Math.max(0, Math.floor(raw.availableCount)),
    ...(typeof raw.totalEarnedCount === "number" && Number.isFinite(raw.totalEarnedCount)
      ? { totalEarnedCount: Math.max(0, Math.floor(raw.totalEarnedCount)) }
      : {}),
    nextExpiresAtIso: isoFromTimestamp(raw.nextExpiresAt) ?? nextAvailableCreditExpiry(credits),
    ...(credits ? { credits } : {}),
  }
}

function mapBackendResetCredits(
  raw: RawBackendRateLimitResetCredits | null | undefined,
): KhalaCodexRateLimitResetCredits | null | undefined {
  if (raw === null || raw === undefined) return raw
  const credits = raw.credits?.map(credit => ({
    status: normalizeCreditStatus(credit.status),
    expiresAtIso: isoFromTimestamp(credit.expires_at),
    grantedAtIso: isoFromTimestamp(credit.granted_at),
  }))
  const availableCount =
    typeof raw.available_count === "number" && Number.isFinite(raw.available_count)
      ? raw.available_count
      : credits?.filter(credit => credit.status === "available").length
  if (availableCount === undefined) return null
  return {
    availableCount: Math.max(0, Math.floor(availableCount)),
    ...(typeof raw.total_earned_count === "number" && Number.isFinite(raw.total_earned_count)
      ? { totalEarnedCount: Math.max(0, Math.floor(raw.total_earned_count)) }
      : {}),
    nextExpiresAtIso: nextAvailableCreditExpiry(credits),
    ...(credits ? { credits } : {}),
  }
}

function buildRpcMessage(id: number, method: string, params: unknown = {}): string {
  return `${JSON.stringify({ jsonrpc: "2.0", id, method, params })}\n`
}

function buildRpcNotification(method: string, params: unknown = {}): string {
  return `${JSON.stringify({ jsonrpc: "2.0", method, params })}\n`
}

function providerStatus(input: {
  readonly error: string | null
  readonly now?: (() => Date) | undefined
  readonly rateLimitResetCredits?: KhalaCodexRateLimitResetCredits | null | undefined
  readonly session?: KhalaCodexRateLimitWindow | null
  readonly status: KhalaCodexRateLimitProviderStatus["status"]
  readonly weekly?: KhalaCodexRateLimitWindow | null
}): KhalaCodexRateLimitProviderStatus {
  return {
    provider: "codex",
    session: input.session ?? null,
    weekly: input.weekly ?? null,
    ...(input.rateLimitResetCredits !== undefined
      ? { rateLimitResetCredits: input.rateLimitResetCredits }
      : {}),
    updatedAtIso: isoNow(input.now),
    error: input.error,
    status: input.status,
  }
}

async function fetchViaRpc(
  options: FetchKhalaCodexRateLimitStatusOptions,
): Promise<KhalaCodexRateLimitProviderStatus> {
  const now = options.now ?? (() => new Date())
  const codexHomePath = resolveCodexHomePath(options.codexHomePath, options.env)
  const spawnFn = options.spawnFn ?? spawn
  const child = spawnFn(options.codexCommand ?? "codex", [
    "-s",
    "read-only",
    "-a",
    "untrusted",
    "app-server",
  ], {
    stdio: ["pipe", "pipe", "pipe"],
    env: {
      ...(options.env ?? process.env),
      CODEX_HOME: codexHomePath,
    },
  })

  return await new Promise<KhalaCodexRateLimitProviderStatus>(resolve => {
    let buffer = ""
    let stderr = ""
    let resolved = false
    let rpcId = 0
    let rateLimitsId: number | null = null

    const cleanup = () => {
      child.stdout.off("data", onStdoutData)
      child.stderr.off("data", onStderrData)
      child.off("error", onError)
      child.off("close", onClose)
      clearTimeout(timeout)
    }

    const settle = (
      result: KhalaCodexRateLimitProviderStatus,
      action: { kill?: boolean } = {},
    ) => {
      if (resolved) return
      resolved = true
      cleanup()
      if (action.kill) child.kill()
      resolve(result)
    }

    const timeout = setTimeout(() => {
      settle(providerStatus({
        error: "Codex rate-limit RPC timeout",
        now,
        status: "error",
      }), { kill: true })
    }, options.timeoutMs ?? CODEX_RPC_TIMEOUT_MS)

    const sendRpc = (method: string, params?: unknown): number => {
      const id = ++rpcId
      child.stdin.write(buildRpcMessage(id, method, params))
      return id
    }

    let initializeId: number | null = null

    function onStdoutData(chunk: Buffer): void {
      buffer += chunk.toString()
      let newlineIndex = buffer.indexOf("\n")
      while (newlineIndex !== -1) {
        const line = buffer.slice(0, newlineIndex).trim()
        buffer = buffer.slice(newlineIndex + 1)
        newlineIndex = buffer.indexOf("\n")
        if (line.length === 0) continue

        let message: RpcResponse
        try {
          message = JSON.parse(line) as RpcResponse
        } catch {
          continue
        }
        if (message.id === undefined) continue
        if (initializeId !== null && message.id === initializeId) {
          child.stdin.write(buildRpcNotification("initialized"))
          rateLimitsId = sendRpc("account/rateLimits/read")
          continue
        }
        if (rateLimitsId === null || message.id !== rateLimitsId) continue
        if (message.error) {
          const error = message.error.message ?? "Codex rate-limit RPC failed"
          settle(providerStatus({ error, now, status: "error" }), { kill: true })
          return
        }

        const wrapper = message.result as RpcRateLimitsResponse | undefined
        const result = wrapper?.rateLimits
        const observedNow = now()
        const rateLimitResetCredits = mapRpcResetCredits(wrapper?.rateLimitResetCredits)
        const resetCreditsProjection =
          rateLimitResetCredits === undefined ? {} : { rateLimitResetCredits }
        settle(providerStatus({
          error: null,
          now: () => observedNow,
          ...resetCreditsProjection,
          session: mapRpcWindow(result?.primary, 300, observedNow),
          status: "ok",
          weekly: mapRpcWindow(result?.secondary, 10080, observedNow),
        }), { kill: true })
      }
    }

    function onStderrData(chunk: Buffer): void {
      stderr += chunk.toString()
      if (stderr.length > 20_000) stderr = stderr.slice(-20_000)
    }

    function onError(error: Error): void {
      const code = (error as NodeJS.ErrnoException).code
      settle(providerStatus({
        error: code === "ENOENT" ? "Codex CLI not found" : error.message,
        now,
        status: code === "ENOENT" ? "unavailable" : "error",
      }))
    }

    function onClose(): void {
      settle(providerStatus({
        error: stderr.trim() || "Codex rate-limit RPC exited before returning usage",
        now,
        status: "error",
      }))
    }

    child.stdout.on("data", onStdoutData)
    child.stderr.on("data", onStderrData)
    child.on("error", onError)
    child.on("close", onClose)
    initializeId = sendRpc("initialize", {
      clientInfo: { name: "khala-code-desktop", version: "0.1.0" },
    })
  })
}

async function codexBackendAuthHeaders(
  options: {
    readonly codexHomePath?: string | null
    readonly env?: NodeJS.ProcessEnv
    readonly readFileFn?: ReadTextFileFn
  },
): Promise<Record<string, string> | null> {
  const read: ReadTextFileFn = options.readFileFn ?? ((path, encoding) => readFile(path, encoding))
  const authPath = join(resolveCodexHomePath(options.codexHomePath, options.env), "auth.json")
  const auth = JSON.parse(await read(authPath, "utf8")) as CodexAuthFile
  const accessToken = auth.tokens?.access_token
  if (!accessToken) return null
  const headers: Record<string, string> = {
    Authorization: `Bearer ${accessToken}`,
    "OpenAI-Beta": "codex-1",
    "User-Agent": "codex-cli",
    originator: "Codex Desktop",
  }
  if (auth.tokens?.account_id) headers["ChatGPT-Account-Id"] = auth.tokens.account_id
  return headers
}

async function fetchBackendResetCredits(
  options: FetchKhalaCodexRateLimitStatusOptions,
): Promise<KhalaCodexRateLimitResetCredits | null> {
  const headers = await codexBackendAuthHeaders(options)
  if (headers === null) return null
  const fetchFn = options.fetchFn ?? fetch
  const response = await fetchFn(CODEX_RATE_LIMIT_RESET_CREDITS_URL, { headers })
  if (!response.ok) return null
  return mapBackendResetCredits(await response.json() as RawBackendRateLimitResetCredits) ?? null
}

async function withBackendResetCredits(
  status: KhalaCodexRateLimitProviderStatus,
  options: FetchKhalaCodexRateLimitStatusOptions,
): Promise<KhalaCodexRateLimitProviderStatus> {
  if (
    status.status !== "ok" ||
    (status.rateLimitResetCredits?.nextExpiresAtIso !== undefined &&
      status.rateLimitResetCredits.nextExpiresAtIso !== null)
  ) {
    return status
  }
  try {
    const rateLimitResetCredits = await fetchBackendResetCredits(options)
    return rateLimitResetCredits === null
      ? status
      : { ...status, rateLimitResetCredits }
  } catch {
    return status
  }
}

export async function fetchKhalaCodexRateLimitStatus(
  options: FetchKhalaCodexRateLimitStatusOptions = {},
): Promise<KhalaCodexRateLimitProviderStatus> {
  const codexHomePath = resolveCodexHomePath(options.codexHomePath, options.env)
  const authExists = options.authExists ?? defaultCodexAuthExists
  if (!authExists(codexHomePath)) {
    return providerStatus({
      error: "Codex not signed in",
      now: options.now,
      status: "unavailable",
    })
  }
  try {
    return await withBackendResetCredits(await fetchViaRpc(options), options)
  } catch (error) {
    return providerStatus({
      error: error instanceof Error ? error.message : String(error),
      now: options.now,
      status: "error",
    })
  }
}

function mapConsumeOutcome(code: string | undefined): KhalaCodexRateLimitResetOutcome {
  if (code === "reset") return "reset"
  if (code === "nothing_to_reset") return "nothingToReset"
  if (code === "no_credit") return "noCredit"
  if (code === "already_redeemed") return "alreadyRedeemed"
  throw new Error(`Unknown Codex reset outcome: ${code ?? "missing"}`)
}

export async function consumeKhalaCodexRateLimitResetCredit(
  options: ConsumeKhalaCodexRateLimitResetCreditOptions,
): Promise<KhalaCodexRateLimitResetOutcome> {
  if (options.idempotencyKey.trim().length === 0) {
    throw new Error("Codex reset idempotency key is required")
  }
  const headers = await codexBackendAuthHeaders(options)
  if (headers === null) throw new Error("Codex not signed in")
  const fetchFn = options.fetchFn ?? fetch
  const response = await fetchFn(CODEX_RATE_LIMIT_RESET_CREDITS_CONSUME_URL, {
    method: "POST",
    headers: {
      ...headers,
      "Content-Type": "application/json",
    },
    body: JSON.stringify({ redeem_request_id: options.idempotencyKey }),
  })
  if (!response.ok) throw new Error(`Codex reset failed: HTTP ${response.status}`)
  const payload = await response.json() as RawBackendConsumeRateLimitResetCredit
  return mapConsumeOutcome(payload.code)
}
