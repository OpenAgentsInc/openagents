import { createHash } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import { Effect, Schema as S } from "effect"

import { runChatTurn } from "./client.js"
import { type ChatMode, type KhalaChatMessage } from "./types.js"

export type KhalaCodexStatus =
  | {
      readonly ready: true
      readonly codexHome: string
      readonly credentialSource: "khala_codex_home" | "codex_home_env" | "default_codex_home" | "pylon_account"
      readonly sdk: "available"
    }
  | {
      readonly ready: false
      readonly blocker: "codex_sdk_missing" | "codex_auth_missing"
      readonly codexHome: string
      readonly sdk: "available" | "missing"
    }

export type KhalaCodexRunOptions = {
  readonly cwd: string
  readonly env?: Record<string, string | undefined>
  readonly onEvent?: ((event: KhalaCodexDisplayEvent) => void) | undefined
  readonly prompt: string
  readonly signal?: AbortSignal | undefined
  readonly timeoutMs?: number | undefined
}

export type KhalaCodexDisplayEvent =
  | { readonly kind: "message"; readonly text: string }
  | { readonly kind: "reasoning"; readonly text: string }
  | { readonly kind: "command"; readonly text: string }
  | { readonly kind: "file_change"; readonly text: string }
  | { readonly kind: "meta"; readonly text: string }

export type KhalaCodexRunResult = {
  readonly commandCount: number
  readonly editedFileCount: number
  readonly sessionRef: string | null
  readonly text: string
  readonly turnCount: number
}

export type KhalaRouteSelection =
  | { readonly route: "chat"; readonly reason: string }
  | { readonly route: "local_codex"; readonly reason: string }
  | {
      readonly route: "spawn_khala"
      readonly count?: number | undefined
      readonly intent: "execute" | "explain_capability"
      readonly objective?: string | undefined
      readonly reason: string
      readonly requiresWorkspace: boolean
    }

type KhalaCodexCredentialSource =
  | "khala_codex_home"
  | "codex_home_env"
  | "default_codex_home"
  | "pylon_account"

const CODEX_AGENT_SDK_PACKAGE = "@openai/codex-sdk"
const DEFAULT_CODEX_TIMEOUT_MS = 10 * 60 * 1000

const KhalaRouteSelectionJson = S.Union([
  S.Struct({
    route: S.Literal("chat"),
    reason: S.optional(S.String),
  }),
  S.Struct({
    route: S.Literal("local_codex"),
    reason: S.optional(S.String),
  }),
  S.Struct({
    route: S.Literal("spawn_khala"),
    count: S.optional(S.Number),
    intent: S.optional(S.Literals(["execute", "explain_capability"])),
    objective: S.optional(S.String),
    reason: S.optional(S.String),
    requiresWorkspace: S.optional(S.Boolean),
  }),
])
type KhalaRouteSelectionJson = typeof KhalaRouteSelectionJson.Type

type CodexThreadEvent = {
  type?: string
  thread_id?: string
  error?: { message?: string }
  item?: {
    type?: string
    status?: string
    text?: string
    command?: string
    aggregated_output?: string
    exit_code?: number
    changes?: Array<{ path?: string; kind?: string }>
  }
}

export function khalaHome(env: Record<string, string | undefined> = Bun.env): string {
  const explicit = env.KHALA_HOME?.trim()
  if (explicit) return resolveHome(explicit)
  return join(homedir(), ".khala")
}

export function defaultKhalaCodexHome(env: Record<string, string | undefined> = Bun.env): string {
  return join(khalaHome(env), "codex", "default")
}

export async function connectKhalaCodex(input: {
  readonly env?: Record<string, string | undefined>
  readonly force?: boolean | undefined
  readonly home?: string | undefined
} = {}): Promise<{ readonly codexHome: string; readonly status: "connected" | "already_connected" }> {
  const env = input.env ?? Bun.env
  const codexHome = resolveHome(input.home?.trim() || defaultKhalaCodexHome(env))
  await forceCodexFileCredentialStore(codexHome)
  if (!input.force && await codexHomeHasLogin(codexHome)) {
    return { codexHome, status: "already_connected" }
  }
  const child = Bun.spawn(["codex", "login", "--device-auth"], {
    env: {
      ...process.env,
      ...env,
      CODEX_HOME: codexHome,
    },
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })
  const exitCode = await child.exited
  if (exitCode !== 0) {
    throw new Error(`codex login --device-auth exited with status ${exitCode}`)
  }
  if (!await codexHomeHasLogin(codexHome)) {
    throw new Error("codex login completed but auth.json was not written")
  }
  return { codexHome, status: "connected" }
}

export async function resolveKhalaCodexStatus(
  env: Record<string, string | undefined> = Bun.env,
): Promise<KhalaCodexStatus> {
  const sdk = await codexSdkAvailable() ? "available" : "missing"
  const homes = await candidateCodexHomes(env)
  const readyHome = await firstLoggedInHome(homes)
  const fallbackHome = homes[0]?.home ?? defaultKhalaCodexHome(env)
  if (sdk === "missing") {
    return {
      ready: false,
      blocker: "codex_sdk_missing",
      codexHome: readyHome?.home ?? fallbackHome,
      sdk,
    }
  }
  if (readyHome === null) {
    return {
      ready: false,
      blocker: "codex_auth_missing",
      codexHome: fallbackHome,
      sdk,
    }
  }
  return {
    ready: true,
    codexHome: readyHome.home,
    credentialSource: readyHome.source,
    sdk,
  }
}

export async function runKhalaCodexTask(options: KhalaCodexRunOptions): Promise<KhalaCodexRunResult> {
  const env = options.env ?? Bun.env
  const status = await resolveKhalaCodexStatus(env)
  if (!status.ready) {
    throw new Error(status.blocker === "codex_sdk_missing"
      ? "@openai/codex-sdk is not installed for this Khala build."
      : "Codex is not connected. Run: khala auth codex")
  }
  const sdk = (await import(CODEX_AGENT_SDK_PACKAGE)) as {
    Codex: new (options?: { env?: Record<string, string | undefined> }) => {
      startThread: (options: Record<string, unknown>) => {
        runStreamed: (
          prompt: string,
          turnOptions?: Record<string, unknown>,
        ) => Promise<{ events: AsyncIterable<unknown> }>
      }
    }
  }
  const abort = new AbortController()
  const forwardAbort = () => abort.abort()
  if (options.signal?.aborted) {
    abort.abort()
  } else {
    options.signal?.addEventListener("abort", forwardAbort, { once: true })
  }
  const timer = setTimeout(() => abort.abort(), options.timeoutMs ?? DEFAULT_CODEX_TIMEOUT_MS)
  let text = ""
  let threadId: string | null = null
  let commandCount = 0
  let editedFileCount = 0
  let turnCount = 0
  try {
    const codex = new sdk.Codex({
      env: {
        ...process.env,
        ...env,
        CODEX_HOME: status.codexHome,
      },
    })
    const thread = codex.startThread({
      workingDirectory: resolve(options.cwd),
      sandboxMode: env.KHALA_CODEX_SANDBOX_MODE ?? "danger-full-access",
      approvalPolicy: "never",
      skipGitRepoCheck: true,
      networkAccessEnabled: true,
      ...(env.KHALA_CODEX_MODEL === undefined ? {} : { model: env.KHALA_CODEX_MODEL }),
    })
    options.onEvent?.({ kind: "meta", text: `delegating to local Codex (${status.credentialSource})` })
    const { events } = await thread.runStreamed(localCodexInstructions(options.prompt), {
      signal: abort.signal,
    })
    for await (const raw of events) {
      const event = raw as CodexThreadEvent
      if (event.type === "thread.started" && typeof event.thread_id === "string") {
        threadId = event.thread_id
      }
      if (event.type === "turn.completed") {
        turnCount += 1
      }
      if (event.type === "turn.failed" || event.type === "error") {
        const message = event.error?.message ?? "Codex turn failed."
        throw new Error(message)
      }
      if (event.type !== "item.completed") continue
      const item = event.item
      if (item?.type === "agent_message" && typeof item.text === "string") {
        text += item.text
        options.onEvent?.({ kind: "message", text: item.text })
      } else if (item?.type === "reasoning" && typeof item.text === "string") {
        options.onEvent?.({ kind: "reasoning", text: item.text })
      } else if (item?.type === "command_execution") {
        commandCount += 1
        options.onEvent?.({
          kind: "command",
          text: `command ${item.exit_code === undefined ? "ran" : `exited ${item.exit_code}`}`,
        })
      } else if (item?.type === "file_change") {
        const changes = Array.isArray(item.changes) ? item.changes : []
        editedFileCount += changes.length
        options.onEvent?.({ kind: "file_change", text: `${changes.length} file change(s)` })
      }
    }
  } finally {
    clearTimeout(timer)
    options.signal?.removeEventListener("abort", forwardAbort)
  }
  return {
    commandCount,
    editedFileCount,
    sessionRef: threadId === null ? null : stableRef("session.khala.codex", threadId),
    text,
    turnCount,
  }
}

export async function selectKhalaRoute(input: {
  readonly baseUrl: string
  readonly env?: Record<string, string | undefined>
  readonly history: ReadonlyArray<KhalaChatMessage>
  readonly mode: ChatMode
  readonly prompt: string
  readonly token?: string | undefined
}): Promise<KhalaRouteSelection> {
  if ((input.env ?? Bun.env).KHALA_CODEX_AUTO === "off") {
    return { route: "chat", reason: "local Codex auto-routing disabled" }
  }
  const selectorPrompt = [
    "Blueprint route selector. Return only minified JSON matching exactly one of these schema shapes:",
    "{\"route\":\"chat\",\"reason\":\"short\"}",
    "{\"route\":\"local_codex\",\"reason\":\"short\"}",
    "{\"route\":\"spawn_khala\",\"reason\":\"short\",\"intent\":\"execute\"|\"explain_capability\",\"count\":5,\"objective\":\"task for child workers\",\"requiresWorkspace\":true}",
    "Choose local_codex only when the user's newest request requires local workspace, filesystem, shell, git, code editing, tests, or reading project files.",
    "Choose spawn_khala when the user is in the Khala CLI and asks to start, spin up, launch, create, or coordinate supervised Khala workers/subagents/instances, or asks whether this CLI can spawn them.",
    "For spawn_khala, set intent=execute only when the user wants workers started now. Set intent=explain_capability for capability questions. Include count only when explicit, objective only when there is a task to give workers, and requiresWorkspace when the task needs repository/filesystem work.",
    "Choose chat for general conversation, explanation, brainstorming, math, writing not requiring local files, or questions about Khala itself that are not about CLI spawning.",
    "Newest request:",
    JSON.stringify(input.prompt),
  ].join("\n")
  try {
    void input.history
    const result = await Effect.runPromise(runChatTurn({
      baseUrl: input.baseUrl,
      mode: input.mode,
      token: input.token,
      messages: [{ role: "user", content: selectorPrompt }],
    }))
    return parseRouteSelection(result.text)
  } catch {
    return { route: "chat", reason: "selector unavailable" }
  }
}

export function parseRouteSelection(text: string): KhalaRouteSelection {
  const match = /\{[\s\S]*\}/.exec(text)
  if (match === null) return { route: "chat", reason: "selector returned prose" }
  try {
    const parsed = S.decodeUnknownSync(KhalaRouteSelectionJson)(JSON.parse(match[0]))
    if (parsed.route === "local_codex") {
      return { route: "local_codex", reason: routeReason(parsed.reason, "workspace capability selected") }
    }
    if (parsed.route === "spawn_khala") {
      return normalizeSpawnRouteSelection(parsed)
    }
    return { route: "chat", reason: routeReason(parsed.reason, "chat selected") }
  } catch {
    return { route: "chat", reason: "selector JSON schema parse failed" }
  }
}

function normalizeSpawnRouteSelection(
  parsed: Extract<KhalaRouteSelectionJson, { readonly route: "spawn_khala" }>,
): KhalaRouteSelection {
  const count = parsed.count === undefined ? undefined : normalizedSpawnCount(parsed.count)
  const objective = normalizedOptionalText(parsed.objective)
  const intent = parsed.intent ?? (objective === undefined ? "explain_capability" : "execute")
  return {
    route: "spawn_khala",
    ...(count === undefined ? {} : { count }),
    intent,
    ...(objective === undefined ? {} : { objective }),
    reason: routeReason(parsed.reason, intent === "execute" ? "spawn requested" : "spawn capability question"),
    requiresWorkspace: parsed.requiresWorkspace ?? false,
  }
}

function normalizedSpawnCount(value: number): number {
  if (!Number.isSafeInteger(value) || value <= 0) {
    throw new Error("spawn count must be a positive integer")
  }
  return value
}

function normalizedOptionalText(value: string | undefined): string | undefined {
  const trimmed = value?.trim()
  return trimmed === undefined || trimmed.length === 0 ? undefined : trimmed
}

function routeReason(value: string | undefined, fallback: string): string {
  const trimmed = normalizedOptionalText(value)
  return trimmed ?? fallback
}

function localCodexInstructions(prompt: string): string {
  return [
    "You are Codex running as Khala's local workspace delegate.",
    "Answer the user's request by inspecting and editing this local workspace when needed.",
    "Do not claim you lack filesystem access; you are the filesystem/tool-capable delegate.",
    "Be concise in the final answer and include relevant file paths when useful.",
    "",
    prompt,
  ].join("\n")
}

async function codexSdkAvailable(): Promise<boolean> {
  try {
    await import(CODEX_AGENT_SDK_PACKAGE)
    return true
  } catch {
    return false
  }
}

function resolveHome(value: string): string {
  const trimmed = value.trim()
  if (trimmed === "~") return homedir()
  if (trimmed.startsWith("~/")) return resolve(homedir(), trimmed.slice(2))
  return resolve(trimmed)
}

async function codexHomeHasLogin(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, "auth.json"))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

async function firstLoggedInHome(
  homes: ReadonlyArray<{ readonly home: string; readonly source: KhalaCodexCredentialSource }>,
) {
  for (const home of homes) {
    if (await codexHomeHasLogin(home.home)) return home
  }
  return null
}

async function candidateCodexHomes(env: Record<string, string | undefined>) {
  const homes: Array<{
    home: string
    source: "khala_codex_home" | "codex_home_env" | "default_codex_home" | "pylon_account"
  }> = [
    { home: defaultKhalaCodexHome(env), source: "khala_codex_home" },
  ]
  if (env.CODEX_HOME?.trim()) {
    homes.push({ home: resolveHome(env.CODEX_HOME), source: "codex_home_env" })
  }
  homes.push({ home: join(homedir(), ".codex"), source: "default_codex_home" })
  for (const home of await pylonCodexAccountHomes()) {
    homes.push({ home, source: "pylon_account" })
  }
  const seen = new Set<string>()
  return homes.filter(candidate => {
    const key = candidate.home
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

async function pylonCodexAccountHomes(): Promise<string[]> {
  const configs = [
    join(homedir(), ".openagents", "pylon", "config.json"),
    join(homedir(), ".pylon", "config.json"),
  ]
  const homes: string[] = []
  for (const configPath of configs) {
    try {
      const raw = JSON.parse(await readFile(configPath, "utf8")) as { dev?: { accounts?: unknown } }
      const accounts = raw.dev?.accounts
      if (!Array.isArray(accounts)) continue
      for (const account of accounts) {
        if (account === null || typeof account !== "object") continue
        const record = account as { provider?: unknown; home?: unknown }
        if (record.provider === "codex" && typeof record.home === "string" && record.home.trim()) {
          homes.push(resolveHome(record.home))
        }
      }
    } catch {
      // Missing Pylon config is normal.
    }
  }
  return homes
}

function codexConfigWithFileCredentialStore(raw: string): string {
  const fileStoreLine = 'cli_auth_credentials_store = "file"'
  if (/^cli_auth_credentials_store\s*=\s*["'][^"']+["']\s*$/m.test(raw)) {
    return raw.replace(/^cli_auth_credentials_store\s*=\s*["'][^"']+["']\s*$/m, fileStoreLine)
  }
  const trimmed = raw.trimEnd()
  return `${trimmed.length === 0 ? "" : `${trimmed}\n`}${fileStoreLine}\n`
}

async function forceCodexFileCredentialStore(home: string): Promise<void> {
  await mkdir(home, { recursive: true })
  const configPath = join(home, "config.toml")
  let raw = ""
  if (existsSync(configPath)) raw = await readFile(configPath, "utf8")
  const next = codexConfigWithFileCredentialStore(raw)
  if (next !== raw) {
    await mkdir(dirname(configPath), { recursive: true })
    await writeFile(configPath, next)
  }
}

function stableRef(prefix: string, value: string): string {
  return `${prefix}.${createHash("sha256").update(value).digest("hex").slice(0, 24)}`
}
