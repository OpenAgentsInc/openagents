import { existsSync } from "node:fs"
import { mkdir, readFile, rename, stat, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import { spawnProcess } from "./proc.js"
import { terminalStyle } from "./terminal.js"

// `khala fleet` is the dead-simple onboarding surface for connecting your own
// Codex accounts to Khala so a per-user Artanis can burn down a backlog across
// your fleet ("Artanis as a Service"). It deliberately requires ZERO power-user
// incantation: no PYLON_HOME, no `bun`, no repo paths, no `--account codex-N`
// bookkeeping. The CLI resolves the Pylon home itself, auto-assigns the next
// account ref, drives the SAME `codex login --device-auth` flow Pylon uses
// (paste-free: the device URL + short code are shown and the browser opens),
// and registers the account into the Pylon config so a local Pylon, the codex
// supervisor, and the server dispatch gate all see it.
//
// Safety invariants (hard requirements):
//   - Each account gets an ISOLATED home under <pylon home>/accounts/codex/<ref>.
//   - NEVER touch the default `~/.codex` home (that would wipe a live session).
//   - Credentials stay local; we never read or print tokens.

const CODEX_LOGIN_MISSING_EXIT = 127

export type KhalaFleetAccountReadiness = "ready" | "credentials_missing"

export type KhalaFleetAccount = {
  readonly accountRef: string
  readonly home: string
  readonly email: string | null
  readonly readiness: KhalaFleetAccountReadiness
  readonly lastLinkedAt: string | null
}

export type KhalaFleetStatus = {
  readonly pylonHome: string
  readonly configPath: string
  readonly accounts: ReadonlyArray<KhalaFleetAccount>
  readonly readyCount: number
}

export type KhalaFleetConnectResult = {
  readonly accountRef: string
  readonly home: string
  readonly email: string | null
  readonly pylonHome: string
  readonly configPath: string
  readonly status: "connected" | "already_connected"
}

export type KhalaFleetLiveSection = {
  readonly title: string
  readonly status: string
  readonly lines: ReadonlyArray<string>
}

export type KhalaFleetLiveStatus = {
  readonly observedAt: string
  readonly sections: ReadonlyArray<KhalaFleetLiveSection>
  readonly raw: unknown
}

export type KhalaFleetLiveFetch = (input: URL, init: RequestInit) => Promise<Response>

export type KhalaCodexDeviceLoginRunner = (input: {
  readonly env: Record<string, string | undefined>
  readonly home: string
}) => Promise<{ readonly exitCode: number }>

type ConfigRecord = Record<string, unknown>

// Raised when the `codex` CLI is not installed. The CLI catches this and prints
// a friendly install hint instead of a raw spawn error.
export class CodexCliMissingError extends Error {
  readonly _tag = "CodexCliMissingError"
  constructor() {
    super(
      "The `codex` CLI is required to connect a Codex account but was not found on your PATH.\n" +
        "Install it, then re-run `khala fleet connect`:\n" +
        "  npm install -g @openai/codex\n" +
        "  (or see https://github.com/openai/codex)",
    )
    this.name = "CodexCliMissingError"
  }
}

function resolveHomePath(value: string, base: string): string {
  const trimmed = value.trim()
  if (trimmed === "~") return base
  if (trimmed.startsWith("~/")) return resolve(base, trimmed.slice(2))
  return resolve(trimmed)
}

// Resolve the Pylon home exactly the way Pylon's own bootstrap does so the
// accounts we register land where the local Pylon / supervisor / dispatch read:
//   1. an explicit PYLON_HOME always wins;
//   2. otherwise prefer `~/.openagents/pylon` when it already exists (the
//      historical identity home a live node uses);
//   3. then `~/.pylon` when IT already exists;
//   4. else default to `~/.openagents/pylon` for a fresh machine.
export function resolvePylonHome(
  env: Record<string, string | undefined> = process.env,
  base: string = homedir(),
): string {
  const explicit = env.PYLON_HOME?.trim()
  if (explicit) return resolveHomePath(explicit, base)
  const openagentsPylon = join(base, ".openagents", "pylon")
  const dotPylon = join(base, ".pylon")
  if (existsSync(join(openagentsPylon, "config.json")) || existsSync(join(openagentsPylon, "seed.json"))) {
    return openagentsPylon
  }
  if (existsSync(join(dotPylon, "config.json")) || existsSync(join(dotPylon, "seed.json"))) {
    return dotPylon
  }
  return openagentsPylon
}

export function pylonConfigPath(pylonHome: string): string {
  return join(pylonHome, "config.json")
}

export function codexAccountHome(pylonHome: string, accountRef: string): string {
  return join(pylonHome, "accounts", "codex", accountRef)
}

function asRecord(value: unknown): ConfigRecord | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as ConfigRecord)
    : null
}

// Read the codex accounts already registered in a Pylon config object.
export function parseCodexAccounts(config: ConfigRecord): Array<{ readonly ref: string; readonly home: string | null }> {
  const dev = asRecord(config.dev)
  const accounts = dev !== null && Array.isArray(dev.accounts) ? dev.accounts : []
  const out: Array<{ ref: string; home: string | null }> = []
  for (const account of accounts) {
    const record = asRecord(account)
    if (record === null) continue
    if (record.provider !== "codex") continue
    if (typeof record.ref !== "string" || record.ref.trim() === "") continue
    out.push({
      ref: record.ref,
      home: typeof record.home === "string" && record.home.trim() !== "" ? record.home : null,
    })
  }
  return out
}

// Auto-assign the next codex account ref: "codex", then "codex-2", "codex-3"...
// (mirrors Pylon's nextCodexAccountRef so refs never collide across the tools).
export function nextCodexAccountRef(existingRefs: ReadonlyArray<string>): string {
  const existing = new Set(existingRefs)
  if (!existing.has("codex")) return "codex"
  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `codex-${index}`
    if (!existing.has(candidate)) return candidate
  }
  return `codex-${Date.now()}`
}

// Register (or update) a codex account in the Pylon config object. Pure: returns
// a new config plus whether anything changed. Matches Pylon's account entry
// shape exactly: { ref, provider: "codex", home }.
export function upsertCodexAccount(
  config: ConfigRecord,
  input: { readonly ref: string; readonly home: string },
): { readonly config: ConfigRecord; readonly changed: boolean } {
  const dev = asRecord(config.dev) ?? {}
  const accounts = Array.isArray(dev.accounts) ? [...dev.accounts] : []
  const nextEntry = { ref: input.ref, provider: "codex", home: input.home }
  const index = accounts.findIndex(account => {
    const record = asRecord(account)
    return record !== null && record.ref === input.ref && record.provider === "codex"
  })
  if (index === -1) {
    accounts.push(nextEntry)
    return { config: { ...config, dev: { ...dev, accounts } }, changed: true }
  }
  const existing = asRecord(accounts[index]) ?? {}
  if (existing.home === input.home) {
    return { config, changed: false }
  }
  accounts[index] = { ...existing, ...nextEntry }
  return { config: { ...config, dev: { ...dev, accounts } }, changed: true }
}

// Decode the ChatGPT account email from a codex auth.json id_token. PII; never
// projected publicly — used only for the local "you connected X" confirmation.
export function decodeCodexIdTokenEmail(idToken: string): string | null {
  try {
    const payload = idToken.split(".")[1]
    if (payload === undefined) return null
    const claims = JSON.parse(Buffer.from(payload, "base64url").toString("utf8")) as Record<string, unknown>
    const profile = claims["https://api.openai.com/profile"]
    const auth = claims["https://api.openai.com/auth"]
    const candidates: unknown[] = [
      claims.email,
      typeof profile === "object" && profile !== null ? (profile as Record<string, unknown>).email : undefined,
      typeof auth === "object" && auth !== null ? (auth as Record<string, unknown>).email : undefined,
    ]
    const email = candidates.find(value => typeof value === "string" && value.includes("@"))
    return typeof email === "string" ? email : null
  } catch {
    return null
  }
}

// Ensure the codex account home uses the file credential store so auth.json is
// written to the isolated home (not the OS keychain), matching Pylon.
export function codexConfigWithFileCredentialStore(raw: string): string {
  const fileStoreLine = 'cli_auth_credentials_store = "file"'
  const pattern = /^cli_auth_credentials_store\s*=\s*["'][^"']+["']\s*$/m
  if (pattern.test(raw)) return raw.replace(pattern, fileStoreLine)
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

async function readPylonConfig(configPath: string): Promise<ConfigRecord> {
  try {
    const parsed = JSON.parse(await readFile(configPath, "utf8"))
    return asRecord(parsed) ?? {}
  } catch {
    return {}
  }
}

async function writePylonConfig(configPath: string, config: ConfigRecord): Promise<void> {
  await mkdir(dirname(configPath), { recursive: true })
  const tempPath = `${configPath}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tempPath, `${JSON.stringify(config, null, 2)}\n`)
  await rename(tempPath, configPath)
}

async function codexHomeHasLogin(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, "auth.json"))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

async function readCodexAccountDetails(
  home: string,
): Promise<{ email: string | null; lastLinkedAt: string | null; present: boolean }> {
  const authPath = join(home, "auth.json")
  try {
    const raw = JSON.parse(await readFile(authPath, "utf8")) as Record<string, unknown>
    const tokens = typeof raw.tokens === "object" && raw.tokens !== null ? (raw.tokens as Record<string, unknown>) : {}
    const idToken = tokens.id_token ?? raw.id_token
    const email = typeof idToken === "string" ? decodeCodexIdTokenEmail(idToken) : null
    const lastRefresh = raw.last_refresh ?? tokens.last_refresh
    const lastLinkedAt = typeof lastRefresh === "string"
      ? lastRefresh
      : (await stat(authPath)).mtime.toISOString()
    return { email, lastLinkedAt, present: true }
  } catch {
    return { email: null, lastLinkedAt: null, present: false }
  }
}

const defaultCodexDeviceLoginRunner: KhalaCodexDeviceLoginRunner = async input => {
  // Inherit stdio so the `codex` CLI shows its device URL + short code, opens
  // the browser, and polls for completion itself. CODEX_HOME pins the isolated
  // per-account home so we NEVER touch the default `~/.codex` session.
  const child = spawnProcess(["codex", "login", "--device-auth"], {
    env: { ...process.env, ...input.env, CODEX_HOME: input.home },
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })
  return { exitCode: await child.exited }
}

// List the connected Codex accounts in the user's fleet with readiness.
export async function listFleetAccounts(
  options: { readonly env?: Record<string, string | undefined> } = {},
): Promise<KhalaFleetStatus> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const configPath = pylonConfigPath(pylonHome)
  const config = await readPylonConfig(configPath)
  const registered = parseCodexAccounts(config)
  const accounts: KhalaFleetAccount[] = []
  for (const entry of registered) {
    const home = entry.home ?? codexAccountHome(pylonHome, entry.ref)
    const details = await readCodexAccountDetails(home)
    accounts.push({
      accountRef: entry.ref,
      home,
      email: details.email,
      readiness: details.present ? "ready" : "credentials_missing",
      lastLinkedAt: details.lastLinkedAt,
    })
  }
  return {
    pylonHome,
    configPath,
    accounts,
    readyCount: accounts.filter(account => account.readiness === "ready").length,
  }
}

// Connect (or re-verify) a Codex account into the fleet. The whole point: one
// short command, zero long-string pasting, sensible defaults.
export async function connectFleetAccount(
  options: {
    readonly env?: Record<string, string | undefined>
    readonly accountRef?: string | undefined
    readonly force?: boolean | undefined
    readonly runDeviceLogin?: KhalaCodexDeviceLoginRunner | undefined
  } = {},
): Promise<KhalaFleetConnectResult> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const configPath = pylonConfigPath(pylonHome)
  const config = await readPylonConfig(configPath)
  const existing = parseCodexAccounts(config)

  const requested = options.accountRef?.trim()
  const accountRef = requested && requested.length > 0
    ? requested
    : nextCodexAccountRef(existing.map(account => account.ref))

  if (accountRef === "default" || accountRef === ".codex") {
    throw new Error(`Refusing to use account ref "${accountRef}" — it could collide with the default Codex home.`)
  }

  const home = codexAccountHome(pylonHome, accountRef)
  await forceCodexFileCredentialStore(home)

  let status: KhalaFleetConnectResult["status"] = "connected"
  if (!options.force && (await codexHomeHasLogin(home))) {
    status = "already_connected"
  } else {
    const runner = options.runDeviceLogin ?? defaultCodexDeviceLoginRunner
    const result = await runner({ env, home })
    if (result.exitCode === CODEX_LOGIN_MISSING_EXIT) {
      throw new CodexCliMissingError()
    }
    if (result.exitCode !== 0) {
      throw new Error(`codex login --device-auth exited with status ${result.exitCode}`)
    }
    if (!(await codexHomeHasLogin(home))) {
      throw new Error("codex login completed but auth.json was not written in the isolated account home")
    }
  }

  const upserted = upsertCodexAccount(config, { ref: accountRef, home })
  if (upserted.changed) {
    await writePylonConfig(configPath, upserted.config)
  }

  const details = await readCodexAccountDetails(home)
  return {
    accountRef,
    home,
    email: details.email,
    pylonHome,
    configPath,
    status,
  }
}

export async function fetchFleetLiveStatus(options: {
  readonly baseUrl: string
  readonly token: string
  readonly fetch?: KhalaFleetLiveFetch | undefined
}): Promise<KhalaFleetLiveStatus> {
  const token = options.token.trim()
  if (token.length === 0) {
    throw new Error("khala fleet status --live requires an owner token. Run `khala login` or pass --token.")
  }
  const response = await (options.fetch ?? fetch)(new URL("/api/operator/fleet/status", options.baseUrl), {
    method: "GET",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
    },
  })
  if (!response.ok) {
    const text = await response.text().catch(() => "")
    const suffix = text.trim().length === 0 ? "" : `: ${text.trim().slice(0, 300)}`
    throw new Error(`operator fleet status returned HTTP ${response.status}${suffix}`)
  }
  const payload = await response.json()
  return normalizeFleetLiveStatus(payload)
}

export function normalizeFleetLiveStatus(payload: unknown): KhalaFleetLiveStatus {
  const root = asRecord(payload) ?? {}
  const observedAt = stringValue(root.observedAt) ??
    stringValue(root.updatedAt) ??
    stringValue(root.generatedAt) ??
    new Date().toISOString()
  return {
    observedAt,
    raw: payload,
    sections: [
      normalizeFleetLiveSection("Pace", root.pace ?? root.tokenPace ?? root.burnRate),
      normalizeFleetLiveSection("Fleet", root.fleet ?? root.codexFleet ?? root.pylons),
      normalizeFleetLiveSection("Watchdog", root.watchdog ?? root.stallWatchdog),
      normalizeFleetLiveSection("GLM", root.glm ?? root.glmFleet ?? root.glmReadiness),
      normalizeFleetLiveSection("Brain", root.brain ?? root.artanis ?? root.loop),
    ],
  }
}

export function renderFleetLiveDashboard(status: KhalaFleetLiveStatus, options: {
  readonly now?: Date | undefined
} = {}): string {
  const now = options.now ?? new Date()
  const header = [
    terminalStyle.assistant("Khala fleet live"),
    terminalStyle.meta(`observed ${formatObservedAt(status.observedAt, now)} | polling /api/operator/fleet/status`),
  ].join(" ")
  const blocks = status.sections.map(renderFleetLiveSection)
  return [header, "", ...blocks].join("\n")
}

function normalizeFleetLiveSection(title: string, value: unknown): KhalaFleetLiveSection {
  const record = asRecord(value)
  if (value === undefined || value === null) {
    return { title, status: "not reported", lines: ["No data in latest operator snapshot."] }
  }
  if (record === null) {
    return { title, status: shortScalar(value), lines: [] }
  }
  const status = stringValue(record.status) ??
    stringValue(record.state) ??
    stringValue(record.health) ??
    stringValue(record.readiness) ??
    summarizeObject(record)
  const detailKeys = Object.keys(record)
    .filter(key => !["status", "state", "health", "readiness"].includes(key))
    .slice(0, 8)
  const lines = detailKeys.map(key => `${humanizeKey(key)}: ${shortValue(record[key])}`)
  return { title, status, lines: lines.length === 0 ? ["No detail fields reported."] : lines }
}

function renderFleetLiveSection(section: KhalaFleetLiveSection): string {
  const title = terminalStyle.heading(`${section.title}`)
  const status = terminalStyle.strong(section.status)
  const lines = section.lines.length === 0
    ? []
    : section.lines.map(line => `  ${terminalStyle.meta(line)}`)
  return [`${title} ${status}`, ...lines].join("\n")
}

function formatObservedAt(value: string, now: Date): string {
  const parsed = Date.parse(value)
  if (!Number.isFinite(parsed)) return value
  const ageSeconds = Math.max(0, Math.round((now.getTime() - parsed) / 1000))
  if (ageSeconds < 60) return `${ageSeconds}s ago`
  if (ageSeconds < 3600) return `${Math.round(ageSeconds / 60)}m ago`
  return new Date(parsed).toISOString()
}

function humanizeKey(key: string): string {
  return key
    .replace(/([a-z0-9])([A-Z])/g, "$1 $2")
    .replace(/[_-]+/g, " ")
    .toLowerCase()
}

function summarizeObject(record: ConfigRecord): string {
  const ok = record.ok
  if (typeof ok === "boolean") return ok ? "ok" : "attention"
  return "reported"
}

function shortValue(value: unknown): string {
  if (Array.isArray(value)) {
    if (value.length === 0) return "none"
    const preview = value.slice(0, 3).map(shortValue).join(", ")
    return value.length > 3 ? `${preview}, +${value.length - 3} more` : preview
  }
  const record = asRecord(value)
  if (record !== null) return summarizeRecordFields(record)
  return shortScalar(value)
}

function summarizeRecordFields(record: ConfigRecord): string {
  const preferred = ["summary", "label", "ref", "state", "status", "health", "ready", "busy", "queued", "available"]
  const parts: Array<string> = []
  for (const key of preferred) {
    if (record[key] !== undefined) parts.push(`${humanizeKey(key)}=${shortScalar(record[key])}`)
  }
  if (parts.length > 0) return parts.slice(0, 4).join(" ")
  const entries = Object.entries(record).slice(0, 3)
  return entries.map(([key, value]) => `${humanizeKey(key)}=${shortScalar(value)}`).join(" ")
}

function shortScalar(value: unknown): string {
  if (value === null || value === undefined) return "not reported"
  if (typeof value === "string") return value.length > 80 ? `${value.slice(0, 77)}...` : value
  if (typeof value === "number" || typeof value === "boolean") return String(value)
  return JSON.stringify(value).slice(0, 120)
}

function stringValue(value: unknown): string | undefined {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : undefined
}
