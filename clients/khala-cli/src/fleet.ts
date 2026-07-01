import { createHash, randomBytes } from "node:crypto"
import { existsSync } from "node:fs"
import { mkdir, readFile, rename, stat, writeFile } from "node:fs/promises"
import { homedir, hostname } from "node:os"
import { dirname, join, resolve } from "node:path"

import { spawnProcess } from "./proc.js"
import { readStoredAgentToken } from "./token-store.js"
import { DEFAULT_BASE_URL } from "./types.js"

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
const CLAUDE_SETUP_TOKEN_MISSING_EXIT = 127
const PYLON_CLAUDE_OAUTH_TOKEN_FILE = "claude-oauth-token"
const CLAUDE_AGENT_CAPABILITY_REF = "capability.pylon.local_claude_agent"

export type KhalaFleetHarness = "codex" | "claude"
type PylonFleetProvider = "codex" | "claude_agent"

export type KhalaFleetAccountReadiness = "ready" | "credentials_missing"

export type KhalaFleetAccount = {
  readonly accountRef: string
  readonly harness: KhalaFleetHarness
  readonly provider: PylonFleetProvider
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
  readonly harness: KhalaFleetHarness
  readonly provider: PylonFleetProvider
  readonly home: string
  readonly email: string | null
  readonly pylonHome: string
  readonly configPath: string
  readonly status: "connected" | "already_connected"
}

export type KhalaFleetLinkResult = {
  readonly schema: "openagents.khala.fleet_link.v1"
  readonly pylonHome: string
  readonly configPath: string
  readonly pylonRef: string
  readonly publicKey: string
  readonly npub: string
  readonly registrationRef: string
  readonly linked: true
  readonly capabilityRefs: ReadonlyArray<string>
}

export type KhalaOperatorFleetStatusSnapshot = {
  readonly fetchedAt: string
  readonly baseUrl: string
  readonly payload: unknown
}

export type KhalaCodexDeviceLoginRunner = (input: {
  readonly env: Record<string, string | undefined>
  readonly home: string
}) => Promise<{ readonly exitCode: number }>

export type KhalaClaudeSetupTokenRunner = (input: {
  readonly env: Record<string, string | undefined>
  readonly home: string
}) => Promise<{ readonly exitCode: number; readonly stdout?: string | undefined }>

type ConfigRecord = Record<string, unknown>
type FetchLike = (input: string | URL | Request, init?: RequestInit) => Promise<Response>

const KHALA_FLEET_LINK_SCHEMA = "openagents.khala.fleet_link.v1" as const
const PYLON_CLIENT_VERSION = "openagents.pylon@1.0.5"
const CODEX_AGENT_CAPABILITY_REF = "capability.pylon.local_codex"

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

export class ClaudeCliMissingError extends Error {
  readonly _tag = "ClaudeCliMissingError"
  constructor() {
    super(
      "The `claude` CLI is required to connect a Claude account but was not found on your PATH.\n" +
        "Install Claude Code, then re-run `khala fleet connect --harness claude`.",
    )
    this.name = "ClaudeCliMissingError"
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

export function claudeAccountHome(pylonHome: string, accountRef: string): string {
  return join(pylonHome, "accounts", "claude_agent", `.claude-${accountRef}`)
}

function asRecord(value: unknown): ConfigRecord | null {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as ConfigRecord)
    : null
}

// Read the codex accounts already registered in a Pylon config object.
export function parseCodexAccounts(config: ConfigRecord): Array<{ readonly ref: string; readonly home: string | null }> {
  return parseFleetAccounts(config)
    .filter(account => account.provider === "codex")
    .map(account => ({ ref: account.ref, home: account.home }))
}

export function parseFleetAccounts(config: ConfigRecord): Array<{
  readonly ref: string
  readonly provider: PylonFleetProvider
  readonly harness: KhalaFleetHarness
  readonly home: string | null
}> {
  const dev = asRecord(config.dev)
  const accounts = dev !== null && Array.isArray(dev.accounts) ? dev.accounts : []
  const out: Array<{ ref: string; provider: PylonFleetProvider; harness: KhalaFleetHarness; home: string | null }> = []
  for (const account of accounts) {
    const record = asRecord(account)
    if (record === null) continue
    if (record.provider !== "codex" && record.provider !== "claude_agent") continue
    if (typeof record.ref !== "string" || record.ref.trim() === "") continue
    out.push({
      ref: record.ref,
      provider: record.provider,
      harness: record.provider === "claude_agent" ? "claude" : "codex",
      home: typeof record.home === "string" && record.home.trim() !== "" ? record.home : null,
    })
  }
  return out
}

// Auto-assign the next codex account ref: "codex", then "codex-2", "codex-3"...
// (mirrors Pylon's nextCodexAccountRef so refs never collide across the tools).
export function nextCodexAccountRef(existingRefs: ReadonlyArray<string>): string {
  return nextFleetAccountRef("codex", existingRefs)
}

export function nextFleetAccountRef(harness: KhalaFleetHarness, existingRefs: ReadonlyArray<string>): string {
  const base = harness === "claude" ? "claude" : "codex"
  const existing = new Set(existingRefs)
  if (!existing.has(base)) return base
  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `${base}-${index}`
    if (!existing.has(candidate)) return candidate
  }
  return `${base}-${Date.now()}`
}

// Register (or update) a codex account in the Pylon config object. Pure: returns
// a new config plus whether anything changed. Matches Pylon's account entry
// shape exactly: { ref, provider: "codex", home }.
export function upsertCodexAccount(
  config: ConfigRecord,
  input: { readonly ref: string; readonly home: string },
): { readonly config: ConfigRecord; readonly changed: boolean } {
  return upsertFleetAccount(config, { provider: "codex", ref: input.ref, home: input.home })
}

export function upsertFleetAccount(
  config: ConfigRecord,
  input: { readonly provider: PylonFleetProvider; readonly ref: string; readonly home: string },
): { readonly config: ConfigRecord; readonly changed: boolean } {
  const dev = asRecord(config.dev) ?? {}
  const accounts = Array.isArray(dev.accounts) ? [...dev.accounts] : []
  const nextEntry = { ref: input.ref, provider: input.provider, home: input.home }
  const index = accounts.findIndex(account => {
    const record = asRecord(account)
    return record !== null && record.ref === input.ref && record.provider === input.provider
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

function stableHash(input: string, length = 24): string {
  return createHash("sha256").update(input).digest("hex").slice(0, length)
}

function sanitizeLabel(value: string): string {
  const sanitized = value.toLowerCase().replace(/[^a-z0-9._-]+/g, "-").replace(/^-+|-+$/g, "")
  return sanitized || "pylon-node"
}

const BECH32_CHARSET = "qpzry9x8gf2tvdw0s3jn54khce6mua7l"

function bech32Polymod(values: ReadonlyArray<number>): number {
  const generator = [0x3b6a57b2, 0x26508e6d, 0x1ea119fa, 0x3d4233dd, 0x2a1462b3]
  let chk = 1
  for (const value of values) {
    const top = chk >> 25
    chk = (chk & 0x1ffffff) << 5 ^ value
    for (let index = 0; index < 5; index += 1) {
      if (((top >> index) & 1) === 1) chk ^= generator[index] ?? 0
    }
  }
  return chk
}

function bech32HrpExpand(hrp: string): number[] {
  const out: number[] = []
  for (const char of hrp) out.push(char.charCodeAt(0) >> 5)
  out.push(0)
  for (const char of hrp) out.push(char.charCodeAt(0) & 31)
  return out
}

function bytesToBech32Words(bytes: Buffer): number[] {
  const words: number[] = []
  let value = 0
  let bits = 0
  for (const byte of bytes) {
    value = (value << 8) | byte
    bits += 8
    while (bits >= 5) {
      words.push((value >> (bits - 5)) & 31)
      bits -= 5
    }
  }
  if (bits > 0) words.push((value << (5 - bits)) & 31)
  return words
}

function encodeNpub(publicKey: string): string {
  const hrp = "npub"
  const words = bytesToBech32Words(Buffer.from(publicKey, "hex"))
  const values = [...bech32HrpExpand(hrp), ...words]
  const polymod = bech32Polymod([...values, 0, 0, 0, 0, 0, 0]) ^ 1
  const checksum = Array.from({ length: 6 }, (_, index) => (polymod >> (5 * (5 - index))) & 31)
  return `${hrp}1${[...words, ...checksum].map(word => BECH32_CHARSET[word] ?? "").join("")}`
}

function generatePublicKey(): string {
  return randomBytes(32).toString("hex")
}

type PylonIdentityRecord = {
  readonly nodeId: string
  readonly pylonRef: string
  readonly nodeLabel: string
  readonly publicKey: string
  readonly npub: string
  readonly createdAt: string
}

function parseIdentity(value: unknown): PylonIdentityRecord | null {
  const record = asRecord(value)
  if (record === null) return null
  if (
    typeof record.nodeId === "string" &&
    typeof record.pylonRef === "string" &&
    typeof record.nodeLabel === "string" &&
    typeof record.publicKey === "string" &&
    /^[0-9a-f]{64}$/i.test(record.publicKey) &&
    typeof record.npub === "string" &&
    typeof record.createdAt === "string"
  ) {
    return record as PylonIdentityRecord
  }
  return null
}

async function loadOrCreatePylonIdentity(pylonHome: string): Promise<PylonIdentityRecord> {
  const identityPath = join(pylonHome, "identity.json")
  try {
    const existing = parseIdentity(JSON.parse(await readFile(identityPath, "utf8")))
    if (existing !== null) return existing
  } catch {
    // Fall through and create a public identity record.
  }

  const publicKey = generatePublicKey()
  const nodeLabel = sanitizeLabel(hostname())
  const identity: PylonIdentityRecord = {
    nodeId: `pylon_${stableHash(publicKey)}`,
    pylonRef: `pylon.${stableHash(`${nodeLabel}:${publicKey}`, 20)}`,
    nodeLabel,
    publicKey,
    npub: encodeNpub(publicKey),
    createdAt: new Date().toISOString(),
  }
  await mkdir(dirname(identityPath), { recursive: true })
  await writeFile(identityPath, `${JSON.stringify(identity, null, 2)}\n`, { mode: 0o600 })
  return identity
}

async function readRuntimeCapabilityRefs(pylonHome: string): Promise<string[]> {
  try {
    const parsed = JSON.parse(await readFile(join(pylonHome, "runtime-state.json"), "utf8"))
    const refs = asRecord(parsed)?.capabilityRefs
    return Array.isArray(refs) ? refs.filter((ref): ref is string => typeof ref === "string") : []
  } catch {
    return []
  }
}

async function capabilityRefsForFleetLink(pylonHome: string): Promise<string[]> {
  const refs = await readRuntimeCapabilityRefs(pylonHome)
  const config = await readPylonConfig(pylonConfigPath(pylonHome))
  const accounts = parseFleetAccounts(config)
  const readyAccounts = await Promise.all(
    accounts.map(async account => {
      const home = account.home ?? defaultAccountHomeForHarness(pylonHome, account.harness, account.ref)
      return account.provider === "claude_agent" ? claudeHomeHasLogin(home) : codexHomeHasLogin(home)
    }),
  )
  if (readyAccounts.some((ready, index) => ready && accounts[index]?.provider === "codex")) {
    refs.push(CODEX_AGENT_CAPABILITY_REF)
  }
  if (readyAccounts.some((ready, index) => ready && accounts[index]?.provider === "claude_agent")) {
    refs.push(CLAUDE_AGENT_CAPABILITY_REF)
  }
  return [...new Set(refs)]
}

async function tokenForFleetLink(input: {
  readonly env: Record<string, string | undefined>
  readonly explicitToken?: string | undefined
}): Promise<string> {
  const explicit = input.explicitToken?.trim()
  if (explicit?.startsWith("oa_agent_")) return explicit
  const envToken = input.env.OPENAGENTS_AGENT_TOKEN?.trim()
  if (envToken?.startsWith("oa_agent_")) return envToken
  const stored = await readStoredAgentToken(input.env)
  if (stored !== undefined) return stored
  throw new Error("khala fleet link requires a signed-in Khala account. Run `khala login` first.")
}

async function postPylonFleetLink(input: {
  readonly baseUrl: string
  readonly token: string
  readonly identity: PylonIdentityRecord
  readonly capabilityRefs: ReadonlyArray<string>
  readonly fetch: typeof fetch
}): Promise<{ readonly registrationRef: string }> {
  const body = {
    schema: "openagents.pylon.register.v0.3",
    pylonRef: input.identity.pylonRef,
    lifecycle: "online",
    clientProtocolVersion: "0.3.0",
    clientVersion: PYLON_CLIENT_VERSION,
    resourceMode: "background_20",
    displayName: input.identity.nodeLabel,
    capabilityRefs: input.capabilityRefs,
    statusRefs: ["status.public.khala_fleet_linked"],
    providerNostrPubkey: input.identity.publicKey,
    providerNostrNpub: input.identity.npub,
  }
  const response = await input.fetch(new URL("/api/pylons/register", input.baseUrl).toString(), {
    body: JSON.stringify(body),
    headers: {
      authorization: `Bearer ${input.token}`,
      "content-type": "application/json",
      "Idempotency-Key": `khala-fleet-link:${input.identity.pylonRef}`,
      "x-pylon-ref": input.identity.pylonRef,
    },
    method: "POST",
  })
  const text = await response.text()
  if (!response.ok) {
    throw new Error(`OpenAgents rejected the Pylon link (${response.status}): ${text || response.statusText}`)
  }
  const parsed = text.trim() ? JSON.parse(text) : {}
  const registrationRef = typeof parsed.registrationRef === "string"
    ? parsed.registrationRef
    : `registration.${input.identity.pylonRef}`
  return { registrationRef }
}

async function codexHomeHasLogin(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, "auth.json"))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

async function claudeHomeHasLogin(home: string): Promise<boolean> {
  try {
    const info = await stat(join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE))
    return info.isFile() && info.size > 0
  } catch {
    return false
  }
}

function defaultAccountHomeForHarness(pylonHome: string, harness: KhalaFleetHarness, accountRef: string): string {
  return harness === "claude" ? claudeAccountHome(pylonHome, accountRef) : codexAccountHome(pylonHome, accountRef)
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

async function readClaudeAccountDetails(
  home: string,
): Promise<{ email: string | null; lastLinkedAt: string | null; present: boolean }> {
  const tokenPath = join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE)
  try {
    const info = await stat(tokenPath)
    return { email: null, lastLinkedAt: info.mtime.toISOString(), present: info.isFile() && info.size > 0 }
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

const defaultClaudeSetupTokenRunner: KhalaClaudeSetupTokenRunner = async input => {
  // Capture stdout because `claude setup-token` emits the account OAuth token
  // there. Stderr/stdin stay inherited for the interactive login prompts.
  const child = spawnProcess(["claude", "setup-token"], {
    env: { ...process.env, ...input.env, CLAUDE_CONFIG_DIR: input.home },
    stdin: "inherit",
    stdout: "pipe",
    stderr: "inherit",
  })
  const [exitCode, stdout] = await Promise.all([child.exited, child.stdout])
  return { exitCode, stdout }
}

function extractClaudeSetupToken(stdout: string | undefined): string | null {
  // Only a token-shaped line may become the stored credential; falling back
  // to an arbitrary trailing status line would persist junk as "ready" auth.
  const lines = (stdout ?? "").split(/\r?\n/).map(line => line.trim()).filter(Boolean)
  return lines.find(line => /^sk-ant-oat-[A-Za-z0-9._-]+$/.test(line)) ?? null
}

async function writeClaudeSetupToken(home: string, token: string): Promise<void> {
  await mkdir(home, { recursive: true })
  await writeFile(join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE), `${token.trim()}\n`, { mode: 0o600 })
}

// List the connected Codex accounts in the user's fleet with readiness.
export async function listFleetAccounts(
  options: { readonly env?: Record<string, string | undefined> } = {},
): Promise<KhalaFleetStatus> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const configPath = pylonConfigPath(pylonHome)
  const config = await readPylonConfig(configPath)
  const registered = parseFleetAccounts(config)
  const accounts: KhalaFleetAccount[] = []
  for (const entry of registered) {
    const home = entry.home ?? defaultAccountHomeForHarness(pylonHome, entry.harness, entry.ref)
    const details = entry.provider === "claude_agent"
      ? await readClaudeAccountDetails(home)
      : await readCodexAccountDetails(home)
    accounts.push({
      accountRef: entry.ref,
      harness: entry.harness,
      provider: entry.provider,
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
    readonly harness?: KhalaFleetHarness | undefined
    readonly accountRef?: string | undefined
    readonly force?: boolean | undefined
    readonly runDeviceLogin?: KhalaCodexDeviceLoginRunner | undefined
    readonly runClaudeSetupToken?: KhalaClaudeSetupTokenRunner | undefined
  } = {},
): Promise<KhalaFleetConnectResult> {
  const env = options.env ?? process.env
  const harness = options.harness ?? "codex"
  const provider: PylonFleetProvider = harness === "claude" ? "claude_agent" : "codex"
  const pylonHome = resolvePylonHome(env)
  const configPath = pylonConfigPath(pylonHome)
  const config = await readPylonConfig(configPath)
  const existing = parseFleetAccounts(config).filter(account => account.harness === harness)

  const requested = options.accountRef?.trim()
  const accountRef = requested && requested.length > 0
    ? requested
    : nextFleetAccountRef(harness, existing.map(account => account.ref))

  if (accountRef === "default" || accountRef === ".codex" || accountRef === ".claude") {
    throw new Error(`Refusing to use account ref "${accountRef}" — it could collide with a default agent home.`)
  }

  const home = defaultAccountHomeForHarness(pylonHome, harness, accountRef)
  if (harness === "codex") await forceCodexFileCredentialStore(home)

  let status: KhalaFleetConnectResult["status"] = "connected"
  const hasLogin = harness === "claude" ? claudeHomeHasLogin : codexHomeHasLogin
  if (!options.force && (await hasLogin(home))) {
    status = "already_connected"
  } else if (harness === "codex") {
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
  } else {
    const runner = options.runClaudeSetupToken ?? defaultClaudeSetupTokenRunner
    const result = await runner({ env, home })
    if (result.exitCode === CLAUDE_SETUP_TOKEN_MISSING_EXIT) {
      throw new ClaudeCliMissingError()
    }
    if (result.exitCode !== 0) {
      throw new Error(`claude setup-token exited with status ${result.exitCode}`)
    }
    const token = extractClaudeSetupToken(result.stdout)
    if (token !== null && token.trim().length > 0) {
      await writeClaudeSetupToken(home, token)
    }
    if (!(await claudeHomeHasLogin(home))) {
      throw new Error("claude setup-token completed but claude-oauth-token was not written in the isolated account home")
    }
  }

  const upserted = upsertFleetAccount(config, { provider, ref: accountRef, home })
  if (upserted.changed) {
    await writePylonConfig(configPath, upserted.config)
  }

  const details = harness === "claude" ? await readClaudeAccountDetails(home) : await readCodexAccountDetails(home)
  return {
    accountRef,
    harness,
    provider,
    home,
    email: details.email,
    pylonHome,
    configPath,
    status,
  }
}

export async function linkFleetPylon(
  options: {
    readonly env?: Record<string, string | undefined>
    readonly baseUrl?: string | undefined
    readonly token?: string | undefined
    readonly fetch?: typeof fetch | undefined
  } = {},
): Promise<KhalaFleetLinkResult> {
  const env = options.env ?? process.env
  const pylonHome = resolvePylonHome(env)
  const configPath = pylonConfigPath(pylonHome)
  const token = await tokenForFleetLink({ env, explicitToken: options.token })
  const identity = await loadOrCreatePylonIdentity(pylonHome)
  const capabilityRefs = await capabilityRefsForFleetLink(pylonHome)
  const linked = await postPylonFleetLink({
    baseUrl: options.baseUrl || env.KHALA_BASE_URL || DEFAULT_BASE_URL,
    token,
    identity,
    capabilityRefs,
    fetch: options.fetch ?? fetch,
  })
  return {
    schema: KHALA_FLEET_LINK_SCHEMA,
    pylonHome,
    configPath,
    pylonRef: identity.pylonRef,
    publicKey: identity.publicKey,
    npub: identity.npub,
    registrationRef: linked.registrationRef,
    linked: true,
    capabilityRefs,
  }
}

export async function fetchOperatorFleetStatus(options: {
  readonly baseUrl?: string | undefined
  readonly token: string
  readonly fetch?: FetchLike | undefined
}): Promise<KhalaOperatorFleetStatusSnapshot> {
  const baseUrl = (options.baseUrl ?? DEFAULT_BASE_URL).replace(/\/+$/, "")
  const token = options.token.trim()
  if (token.length === 0) {
    throw new Error("khala fleet status --live requires an owner token. Run `khala login` or pass --token.")
  }
  const response = await (options.fetch ?? fetch)(`${baseUrl}/api/operator/fleet/state`, {
    method: "GET",
    headers: {
      accept: "application/json",
      authorization: `Bearer ${token}`,
    },
  })
  if (!response.ok) {
    let detail = ""
    try {
      const body = await response.json() as Record<string, unknown>
      const reason = body.reason ?? body.error ?? body.message
      detail = typeof reason === "string" && reason.trim().length > 0 ? `: ${reason.trim()}` : ""
    } catch {
      // Ignore malformed error bodies; the status code is enough to act on.
    }
    throw new Error(`operator fleet status request failed (${response.status})${detail}`)
  }
  return {
    fetchedAt: new Date().toISOString(),
    baseUrl,
    payload: await response.json(),
  }
}

export async function runOperatorFleetStatusLive(options: {
  readonly baseUrl?: string | undefined
  readonly token: string
  readonly env?: Record<string, string | undefined> | undefined
  readonly fetch?: FetchLike | undefined
  readonly write?: ((text: string) => void) | undefined
  readonly pollIntervalMs?: number | undefined
  readonly maxPolls?: number | undefined
}): Promise<void> {
  const env = options.env ?? process.env
  const write = options.write ?? (text => process.stdout.write(text))
  const intervalMs = options.pollIntervalMs ?? 5_000
  const maxPolls = options.maxPolls ?? parseOptionalPositiveInteger(env.KHALA_FLEET_LIVE_MAX_POLLS)
  let stopped = false
  const stop = (): void => {
    stopped = true
  }
  process.once("SIGINT", stop)
  try {
    for (let poll = 0; !stopped; poll += 1) {
      try {
        const snapshot = await fetchOperatorFleetStatus({
          baseUrl: options.baseUrl,
          token: options.token,
          fetch: options.fetch,
        })
        write(`${clearScreen()}${formatOperatorFleetDashboard(snapshot)}`)
      } catch (error) {
        write(`${clearScreen()}${formatOperatorFleetError(error)}\n`)
      }
      if (maxPolls !== undefined && poll + 1 >= maxPolls) return
      await sleep(intervalMs)
    }
  } finally {
    process.off("SIGINT", stop)
  }
}

export function formatOperatorFleetDashboard(snapshot: KhalaOperatorFleetStatusSnapshot): string {
  const payload = asRecord(snapshot.payload) ?? {}
  const lines = [
    `Khala fleet live dashboard`,
    `updated: ${snapshot.fetchedAt}  source: ${snapshot.baseUrl}/api/operator/fleet/state`,
    "",
    formatDashboardBlock("Pace", firstRecord(payload, ["pace", "paceToFloor", "pace_to_floor", "burnPace"])),
    formatDashboardBlock("Fleet", firstRecord(payload, ["fleet", "codexFleet", "capacity", "pylons"])),
    formatDashboardBlock("Watchdog", firstRecord(payload, ["watchdog", "stallWatchdog", "fleetWatchdog"])),
    formatDashboardBlock("GLM", firstRecord(payload, ["glm", "glmFleet", "glmFleetStatus", "inference", "readiness"])),
    formatDashboardBlock("Brain", firstRecord(payload, ["brain", "artanis", "artanisLoop", "khalaBrain"])),
  ]
  return `${lines.join("\n")}\n`
}

function formatOperatorFleetError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error)
  return [
    "Khala fleet live dashboard",
    `updated: ${new Date().toISOString()}`,
    "",
    "[status]",
    `  ${message}`,
  ].join("\n")
}

function formatDashboardBlock(title: string, value: unknown): string {
  const lines = summarizeDashboardValue(value)
  return [`[${title}]`, ...(lines.length === 0 ? ["  no data"] : lines.map(line => `  ${line}`))].join("\n")
}

function summarizeDashboardValue(value: unknown): ReadonlyArray<string> {
  if (value === undefined || value === null) return []
  if (typeof value !== "object") return [formatScalar(value)]
  if (Array.isArray(value)) {
    if (value.length === 0) return ["0 items"]
    return [`${value.length} items`, ...value.slice(0, 3).map((entry, index) => `${index + 1}. ${formatCompact(entry)}`)]
  }
  const record = value as Record<string, unknown>
  const entries = Object.entries(record)
    .filter(([, entry]) => entry !== undefined && typeof entry !== "function")
    .slice(0, 10)
  if (entries.length === 0) return []
  return entries.map(([key, entry]) => `${key}: ${formatCompact(entry)}`)
}

function firstRecord(record: Record<string, unknown>, keys: ReadonlyArray<string>): unknown {
  for (const key of keys) {
    if (record[key] !== undefined) return record[key]
  }
  return undefined
}

function formatCompact(value: unknown): string {
  if (value === null || value === undefined) return "n/a"
  if (typeof value !== "object") return formatScalar(value)
  try {
    const encoded = JSON.stringify(value)
    return truncate(encoded ?? String(value), 160)
  } catch {
    return truncate(String(value), 160)
  }
}

function formatScalar(value: unknown): string {
  if (typeof value === "number") return Number.isInteger(value) ? String(value) : value.toFixed(2)
  if (typeof value === "boolean") return value ? "yes" : "no"
  return truncate(String(value), 160)
}

function truncate(value: string, max: number): string {
  return value.length <= max ? value : `${value.slice(0, max - 1)}…`
}

function clearScreen(): string {
  return process.stdout.isTTY ? "\x1b[2J\x1b[H" : ""
}

function sleep(ms: number): Promise<void> {
  return new Promise(resolve => setTimeout(resolve, ms))
}

function parseOptionalPositiveInteger(value: string | undefined): number | undefined {
  if (value === undefined || value.trim() === "") return undefined
  const parsed = Number.parseInt(value, 10)
  return Number.isSafeInteger(parsed) && parsed > 0 ? parsed : undefined
}
