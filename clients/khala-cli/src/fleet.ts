import { existsSync } from "node:fs"
import { mkdir, readFile, rename, stat, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join, resolve } from "node:path"

import { spawnProcess } from "./proc.js"

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

export type KhalaCodexDeviceLoginRunner = (input: {
  readonly env: Record<string, string | undefined>
  readonly home: string
}) => Promise<{ readonly exitCode: number }>

export type KhalaFleetRunIssue = {
  readonly issue: number
  readonly accountRef: string
  readonly state: "planned" | "dispatched" | "failed" | "skipped"
  readonly assignmentRef?: string | undefined
  readonly durableRequestId?: string | undefined
  readonly error?: string | undefined
}

export type KhalaFleetRunPlan = {
  readonly schema: "openagents.khala.fleet_run_plan.v0.1"
  readonly pylonHome: string
  readonly configPath: string
  readonly pylonRef: string
  readonly repo: string
  readonly branch: string
  readonly commit: string
  readonly verify: string
  readonly readyAccounts: ReadonlyArray<KhalaFleetAccount>
  readonly perAccount: number
  readonly maxSlots: number
  readonly desiredSlots: number
  readonly issues: ReadonlyArray<number>
}

export type KhalaFleetRunResult = {
  readonly schema: "openagents.khala.fleet_run.v0.1"
  readonly plan: KhalaFleetRunPlan
  readonly dryRun: boolean
  readonly loop: boolean
  readonly dispatched: ReadonlyArray<KhalaFleetRunIssue>
}

export type KhalaFleetRunCommandRunner = (input: {
  readonly command: ReadonlyArray<string>
  readonly env: Record<string, string | undefined>
}) => Promise<{ readonly exitCode: number; readonly stdout: string; readonly stderr: string }>

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

export class PylonCliMissingError extends Error {
  readonly _tag = "PylonCliMissingError"
  constructor() {
    super(
      "The `pylon` CLI is required to run your Khala fleet but was not found on your PATH.\n" +
        "Install or expose Pylon, then re-run `khala fleet run`.",
    )
    this.name = "PylonCliMissingError"
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

function splitCommand(value: string | undefined, fallback: ReadonlyArray<string>): ReadonlyArray<string> {
  const trimmed = value?.trim()
  if (trimmed === undefined || trimmed.length === 0) return fallback
  return trimmed.split(/\s+/).filter(part => part.length > 0)
}

function pylonCommand(env: Record<string, string | undefined>): ReadonlyArray<string> {
  return splitCommand(env.KHALA_PYLON_COMMAND ?? env.PYLON_COMMAND, ["pylon"])
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

const defaultFleetRunCommandRunner: KhalaFleetRunCommandRunner = async input => {
  const child = spawnProcess(input.command, {
    env: { ...process.env, ...input.env },
    stdout: "pipe",
    stderr: "pipe",
  })
  const [exitCode, stdout, stderr] = await Promise.all([child.exited, child.stdout, child.stderr])
  return { exitCode, stdout, stderr }
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

function positiveInteger(value: number | undefined, fallback: number, label: string): number {
  const next = value ?? fallback
  if (!Number.isSafeInteger(next) || next <= 0) {
    throw new Error(`khala fleet run ${label} must be a positive integer`)
  }
  return next
}

function cleanIssueList(issues: ReadonlyArray<number>): ReadonlyArray<number> {
  const out: number[] = []
  const seen = new Set<number>()
  for (const issue of issues) {
    if (!Number.isSafeInteger(issue) || issue <= 0) {
      throw new Error("khala fleet run issue numbers must be positive integers")
    }
    if (!seen.has(issue)) {
      seen.add(issue)
      out.push(issue)
    }
  }
  if (out.length === 0) {
    throw new Error("khala fleet run requires at least one issue: pass --issue 123 or --issues 123,124")
  }
  return out
}

function cleanRepo(repo: string | undefined): string {
  const value = repo?.trim()
  if (value === undefined || value.length === 0) {
    throw new Error("khala fleet run requires --repo <owner/repo>")
  }
  if (!/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(value)) {
    throw new Error("khala fleet run --repo must look like owner/repo")
  }
  return value
}

function cleanBranch(branch: string | undefined): string {
  const value = branch?.trim() || "main"
  if (!/^[A-Za-z0-9._/-]{1,120}$/.test(value) || value.includes("..")) {
    throw new Error("khala fleet run --branch must be a bounded public branch name")
  }
  return value
}

function cleanCommit(commit: string | undefined): string {
  const value = commit?.trim()
  if (value === undefined || value.length === 0) {
    throw new Error("khala fleet run requires --commit <sha> for bounded public repo work")
  }
  if (!/^[a-f0-9]{40}$/i.test(value)) {
    throw new Error("khala fleet run --commit must be a 40-character git SHA")
  }
  return value
}

function cleanVerify(verify: string | undefined): string {
  const value = verify?.trim()
  if (value === undefined || value.length === 0) {
    throw new Error("khala fleet run requires --verify <command>")
  }
  return value
}

function parsePylonRef(stdout: string): string | null {
  try {
    const parsed = JSON.parse(stdout) as Record<string, unknown>
    for (const key of ["pylonRef", "ref", "providerRef"]) {
      const value = parsed[key]
      if (typeof value === "string" && value.trim().length > 0) return value.trim()
    }
    const nested = parsed.provider
    if (nested !== null && typeof nested === "object") {
      const value = (nested as Record<string, unknown>).pylonRef
      if (typeof value === "string" && value.trim().length > 0) return value.trim()
    }
  } catch {
    const match = stdout.match(/pylon[.:][a-z0-9_.:-]+/i)
    if (match !== null) return match[0]
  }
  return null
}

async function resolveFleetRunPylonRef(input: {
  readonly env: Record<string, string | undefined>
  readonly explicit: string | undefined
  readonly runner: KhalaFleetRunCommandRunner
}): Promise<string> {
  const explicit = input.explicit?.trim()
  if (explicit !== undefined && explicit.length > 0) return explicit
  const command = [...pylonCommand(input.env), "provider", "go-online", "--json"]
  const result = await input.runner({ command, env: input.env })
  if (result.exitCode === 127) throw new PylonCliMissingError()
  if (result.exitCode !== 0) {
    throw new Error(`pylon provider go-online failed with status ${result.exitCode}: ${result.stderr.trim() || result.stdout.trim()}`)
  }
  const pylonRef = parsePylonRef(result.stdout)
  if (pylonRef === null) {
    throw new Error("pylon provider go-online did not report a pylonRef; pass --pylon-ref explicitly")
  }
  return pylonRef
}

export async function planFleetRun(options: {
  readonly env?: Record<string, string | undefined> | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly issues: ReadonlyArray<number>
  readonly maxSlots?: number | undefined
  readonly perAccount?: number | undefined
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly verify?: string | undefined
  readonly runner?: KhalaFleetRunCommandRunner | undefined
}): Promise<KhalaFleetRunPlan> {
  const env = options.env ?? process.env
  const status = await listFleetAccounts({ env })
  const readyAccounts = status.accounts.filter(account => account.readiness === "ready")
  if (readyAccounts.length === 0) {
    throw new Error("khala fleet run needs at least one ready Codex account. Run `khala fleet connect` first.")
  }
  const perAccount = positiveInteger(options.perAccount, 1, "--per-account")
  const maxSlots = positiveInteger(options.maxSlots, 10, "--max-slots")
  const desiredSlots = Math.min(maxSlots, readyAccounts.length * perAccount)
  return {
    schema: "openagents.khala.fleet_run_plan.v0.1",
    branch: cleanBranch(options.branch),
    commit: cleanCommit(options.commit),
    configPath: status.configPath,
    desiredSlots,
    issues: cleanIssueList(options.issues),
    maxSlots,
    perAccount,
    pylonHome: status.pylonHome,
    pylonRef: await resolveFleetRunPylonRef({
      env,
      explicit: options.pylonRef,
      runner: options.runner ?? defaultFleetRunCommandRunner,
    }),
    readyAccounts,
    repo: cleanRepo(options.repo),
    verify: cleanVerify(options.verify),
  }
}

async function publishFleetCapacity(input: {
  readonly env: Record<string, string | undefined>
  readonly plan: KhalaFleetRunPlan
  readonly runner: KhalaFleetRunCommandRunner
}): Promise<void> {
  const command = [...pylonCommand(input.env), "presence", "heartbeat", "--json"]
  const result = await input.runner({
    command,
    env: {
      ...input.env,
      OPENAGENTS_PYLON_CODEX_CONCURRENCY: String(input.plan.desiredSlots),
      OPENAGENTS_PYLON_CODEX_BUSY: "0",
      OPENAGENTS_PYLON_CODEX_QUEUED: "0",
    },
  })
  if (result.exitCode === 127) throw new PylonCliMissingError()
  if (result.exitCode !== 0) {
    throw new Error(`pylon presence heartbeat failed with status ${result.exitCode}: ${result.stderr.trim() || result.stdout.trim()}`)
  }
}

async function dispatchFleetIssue(input: {
  readonly accountRef: string
  readonly env: Record<string, string | undefined>
  readonly issue: number
  readonly plan: KhalaFleetRunPlan
  readonly runner: KhalaFleetRunCommandRunner
}): Promise<KhalaFleetRunIssue> {
  const prompt = `Implement public issue #${input.issue} and run the named verification.`
  const command = [
    ...pylonCommand(input.env),
    "khala",
    "request",
    "--prompt",
    prompt,
    "--workflow",
    "codex_agent_task",
    "--pylon-ref",
    input.plan.pylonRef,
    "--account-ref",
    input.accountRef,
    "--repo",
    input.plan.repo,
    "--branch",
    input.plan.branch,
    "--commit",
    input.plan.commit,
    "--verify",
    input.plan.verify,
    "--json",
  ]
  const result = await input.runner({ command, env: input.env })
  if (result.exitCode === 127) throw new PylonCliMissingError()
  if (result.exitCode !== 0) {
    return {
      accountRef: input.accountRef,
      error: result.stderr.trim() || result.stdout.trim() || `pylon exited ${result.exitCode}`,
      issue: input.issue,
      state: "failed",
    }
  }
  try {
    const parsed = JSON.parse(result.stdout) as Record<string, unknown>
    return {
      accountRef: input.accountRef,
      assignmentRef: typeof parsed.assignmentRef === "string" ? parsed.assignmentRef : undefined,
      durableRequestId: typeof parsed.durableRequestId === "string" ? parsed.durableRequestId : undefined,
      issue: input.issue,
      state: "dispatched",
    }
  } catch {
    return {
      accountRef: input.accountRef,
      error: "pylon returned non-JSON output for khala request",
      issue: input.issue,
      state: "failed",
    }
  }
}

export async function runFleetSupervisor(options: {
  readonly env?: Record<string, string | undefined> | undefined
  readonly branch?: string | undefined
  readonly commit?: string | undefined
  readonly dryRun?: boolean | undefined
  readonly issues: ReadonlyArray<number>
  readonly loop?: boolean | undefined
  readonly maxSlots?: number | undefined
  readonly perAccount?: number | undefined
  readonly pylonRef?: string | undefined
  readonly repo?: string | undefined
  readonly runner?: KhalaFleetRunCommandRunner | undefined
  readonly verify?: string | undefined
}): Promise<KhalaFleetRunResult> {
  const env = options.env ?? process.env
  const runner = options.runner ?? defaultFleetRunCommandRunner
  const plan = await planFleetRun({ ...options, env, runner })
  if (options.dryRun === true) {
    return { schema: "openagents.khala.fleet_run.v0.1", plan, dryRun: true, loop: options.loop === true, dispatched: [] }
  }
  await publishFleetCapacity({ env, plan, runner })
  const slots = Math.min(plan.desiredSlots, plan.issues.length)
  const dispatched: KhalaFleetRunIssue[] = []
  for (let slot = 0; slot < slots; slot += 1) {
    const account = plan.readyAccounts[slot % plan.readyAccounts.length]
    const issue = plan.issues[slot]
    if (account === undefined || issue === undefined) continue
    dispatched.push(await dispatchFleetIssue({
      accountRef: account.accountRef,
      env,
      issue,
      plan,
      runner,
    }))
  }
  return {
    schema: "openagents.khala.fleet_run.v0.1",
    plan,
    dryRun: false,
    loop: options.loop === true,
    dispatched,
  }
}
