import { createHash } from "node:crypto"
import { mkdir, readFile, stat, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { join } from "node:path"
import {
  hashPylonAccountRef,
  discoverPylonSiblingAccountHomes,
  loadPylonAccountRegistry,
  pylonClaudeAccountHomeHasAuth,
  pylonAccountEnvironment,
  type PylonAccountProvider,
  type ResolvedPylonAccountSelection,
} from "./account-registry.js"
import {
  loadClaudeAgentConfig,
  probeClaudeAgentReadiness,
  type ClaudeAgentReadiness,
} from "./claude-agent.js"
import {
  detectCodexCliLogin,
  loadCodexAgentConfig,
  probeCodexAgentReadiness,
  type CodexAgentReadiness,
} from "./codex-agent.js"
import type { BootstrapSummary } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

export const PYLON_ACCOUNT_USAGE_STORE_SCHEMA = "openagents.pylon.account_usage_store.v0.3"
export const PYLON_ACCOUNTS_LIST_SCHEMA = "openagents.pylon.accounts_list.v0.3"
export const PYLON_ACCOUNTS_USAGE_SCHEMA = "openagents.pylon.accounts_usage.v0.3"
export const PYLON_ACCOUNT_USAGE_SUMMARY_SCHEMA = "openagents.pylon.account_usage_summary.v0.3"

export type PylonRateLimitWindowSnapshot = {
  usedPercent: number
  remainingPercent: number
  windowMinutes: number | null
  resetsAt: number | null
  label: string
}

export type PylonCreditsSnapshot = {
  hasCredits: boolean
  unlimited: boolean
  balance: string | null
}

export type PylonProviderRateLimitSnapshot = {
  provider: PylonAccountProvider
  limitId: string
  limitName: string | null
  primary: PylonRateLimitWindowSnapshot | null
  secondary: PylonRateLimitWindowSnapshot | null
  credits: PylonCreditsSnapshot | null
  planType: string | null
  rateLimitReachedType: string | null
}

export type PylonLocalSessionUsageSnapshot = {
  provider: PylonAccountProvider
  sessionRef: string | null
  inputTokens: number
  outputTokens: number
  totalTokens: number
  totalCostUsd?: number
}

export type PylonAccountUsageStoreEntry = {
  provider: PylonAccountProvider
  selector: "registry_ref" | "direct_home" | "default_home"
  accountRefHash: string
  providerTruth: {
    observedAt: string
    snapshots: PylonProviderRateLimitSnapshot[]
  } | null
  localSessionTruth: {
    observedAt: string
    usage: PylonLocalSessionUsageSnapshot
  } | null
  updatedAt: string
}

export type PylonAccountUsageStore = {
  schema: typeof PYLON_ACCOUNT_USAGE_STORE_SCHEMA
  accounts: Record<string, PylonAccountUsageStoreEntry>
  updatedAt: string
}

export type PylonAccountReadiness =
  | { provider: "codex"; readiness: CodexAgentReadiness }
  | { provider: "claude_agent"; readiness: ClaudeAgentReadiness }

export type PylonAccountListEntry = {
  provider: PylonAccountProvider
  selector: "registry_ref" | "default_home"
  accountRef: string | null
  accountRefHash: string
  homeRef: string
  homeState: "present" | "missing"
  readiness: PylonAccountReadiness["readiness"]
  blockerRefs: string[]
}

export type PylonAccountsListProjection = {
  schema: typeof PYLON_ACCOUNTS_LIST_SCHEMA
  observedAt: string
  accounts: PylonAccountListEntry[]
  blockerRefs: string[]
}

export type PylonAccountsUsageArgs = {
  accountRef: string | null
  provider: PylonAccountProvider | null
  all: boolean
  refresh: boolean
  json: boolean
}

export type PylonAccountsUsageProjection = {
  schema: typeof PYLON_ACCOUNTS_USAGE_SCHEMA
  observedAt: string
  refresh: {
    requested: boolean
    performed: boolean
    costStatement: string | null
    blockerRefs: string[]
  }
  accounts: Array<{
    provider: PylonAccountProvider
    accountRef: string | null
    accountRefHash: string
    readiness: PylonAccountReadiness["readiness"] | null
    truth: {
      provider: ProviderTruthProjection
      localSession: LocalSessionTruthProjection
      platform: PlatformTruthProjection
    }
    blockerRefs: string[]
  }>
  blockerRefs: string[]
}

export type PylonAccountUsageSummary = {
  schema: typeof PYLON_ACCOUNT_USAGE_SUMMARY_SCHEMA
  observedAt: string
  accountCount: number
  providerTruthAccountCount: number
  localSessionAccountCount: number
  staleProviderTruthCount: number
  accounts: Array<{
    provider: PylonAccountProvider
    accountRefHash: string
    providerTruthState: "available" | "stale" | "missing"
    localSessionState: "available" | "missing"
    latestObservedAt: string | null
  }>
}

type ProviderTruthProjection = {
  state: "available" | "stale" | "missing"
  observedAt: string | null
  ageSeconds: number | null
  snapshots: PylonProviderRateLimitSnapshot[]
  blockerRefs: string[]
}

type LocalSessionTruthProjection = {
  state: "available" | "missing"
  observedAt: string | null
  ageSeconds: number | null
  usage: PylonLocalSessionUsageSnapshot | null
  blockerRefs: string[]
}

type PlatformTruthProjection = {
  state: "available" | "unavailable"
  observedAt: string | null
  pool: {
    lowCreditAccountRefs: string[]
    cooldownAccountRefs: string[]
    leasedAccountRefs: string[]
  } | null
  blockerRefs: string[]
}

type AccountDiscoveryTarget = {
  provider: PylonAccountProvider
  selector: "registry_ref" | "default_home"
  accountRef: string | null
  accountRefHash: string
  home: string
  homeRef: string
  account: ResolvedPylonAccountSelection | null
}

export type PylonAccountUsageRefreshTarget = Pick<
  AccountDiscoveryTarget,
  "provider" | "selector" | "accountRef" | "accountRefHash" | "account"
>

type AccountUsageObservation = {
  provider: PylonAccountProvider
  account?: ResolvedPylonAccountSelection | null
  providerSnapshots?: PylonProviderRateLimitSnapshot[]
  localSessionUsage?: PylonLocalSessionUsageSnapshot
  observedAt?: Date
}

const ACCOUNT_USAGE_STALE_SECONDS = 15 * 60
const costStatement =
  "pylon accounts usage --refresh runs one minimal bounded provider inference per selected account and may consume paid provider tokens."

function providerSelectorFrom(value: string): PylonAccountProvider | null {
  const normalized = value.trim().toLowerCase().replaceAll("-", "_")
  if (normalized === "codex" || normalized === "chatgpt" || normalized === "openai") return "codex"
  if (normalized === "claude" || normalized === "claude_agent" || normalized === "anthropic") return "claude_agent"
  return null
}

function accountUsagePath(summary: Pick<BootstrapSummary, "paths">) {
  return join(summary.paths.home, "account-usage.json")
}

function stableRef(prefix: string, input: string) {
  return `${prefix}.${createHash("sha256").update(input).digest("hex").slice(0, 24)}`
}

function defaultHome(provider: PylonAccountProvider, env: Record<string, string | undefined>) {
  if (provider === "codex") {
    const configured = (env.CODEX_HOME ?? "").trim()
    return configured.length > 0 ? configured : join(homedir(), ".codex")
  }
  const configured = (env.CLAUDE_CONFIG_DIR ?? "").trim()
  return configured.length > 0 ? configured : join(homedir(), ".claude")
}

function accountIdentity(
  provider: PylonAccountProvider,
  account: ResolvedPylonAccountSelection | null | undefined,
) {
  if (account) {
    return {
      accountRefHash: account.accountRefHash,
      selector: account.selector,
    }
  }
  return {
    accountRefHash: hashPylonAccountRef(provider, "default"),
    selector: "default_home" as const,
  }
}

async function pathIsDirectory(path: string): Promise<boolean> {
  try {
    return (await stat(path)).isDirectory()
  } catch {
    return false
  }
}

async function loadAccountUsageStore(summary: Pick<BootstrapSummary, "paths">): Promise<PylonAccountUsageStore> {
  try {
    const parsed = JSON.parse(await readFile(accountUsagePath(summary), "utf8")) as PylonAccountUsageStore
    if (parsed.schema === PYLON_ACCOUNT_USAGE_STORE_SCHEMA && parsed.accounts && typeof parsed.accounts === "object") {
      return parsed
    }
  } catch {
    // Missing or malformed local telemetry starts from an empty store.
  }
  return {
    schema: PYLON_ACCOUNT_USAGE_STORE_SCHEMA,
    accounts: {},
    updatedAt: "1970-01-01T00:00:00.000Z",
  }
}

async function saveAccountUsageStore(summary: Pick<BootstrapSummary, "paths">, store: PylonAccountUsageStore) {
  await mkdir(summary.paths.home, { recursive: true })
  await writeFile(accountUsagePath(summary), `${JSON.stringify(store, null, 2)}\n`, { mode: 0o600 })
}

function numberOrNull(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value
  if (typeof value === "string" && value.trim().length > 0) {
    const parsed = Number(value)
    if (Number.isFinite(parsed)) return parsed
  }
  return null
}

function boolOrNull(value: unknown): boolean | null {
  if (typeof value === "boolean") return value
  if (typeof value === "string") {
    if (value === "1" || value.toLowerCase() === "true") return true
    if (value === "0" || value.toLowerCase() === "false") return false
  }
  return null
}

function stringOrNull(value: unknown): string | null {
  return typeof value === "string" && value.trim().length > 0 ? value.trim() : null
}

function record(value: unknown): Record<string, unknown> | null {
  return value !== null && typeof value === "object" ? value as Record<string, unknown> : null
}

function valueAt(input: Record<string, unknown>, ...keys: string[]) {
  for (const key of keys) {
    if (Object.prototype.hasOwnProperty.call(input, key)) return input[key]
  }
  return undefined
}

function isApproximateWindow(minutes: number, expectedMinutes: number) {
  return minutes >= expectedMinutes * 0.95 && minutes <= expectedMinutes * 1.05
}

export function rateLimitLabelForWindow(windowMinutes: number | null): string {
  if (windowMinutes === null) return "usage"
  const minutes = Math.max(0, windowMinutes)
  if (isApproximateWindow(minutes, 5 * 60)) return "5h"
  if (isApproximateWindow(minutes, 24 * 60)) return "daily"
  if (isApproximateWindow(minutes, 7 * 24 * 60)) return "weekly"
  if (isApproximateWindow(minutes, 30 * 24 * 60)) return "monthly"
  if (isApproximateWindow(minutes, 365 * 24 * 60)) return "annual"
  return "usage"
}

function windowFromUnknown(value: unknown): PylonRateLimitWindowSnapshot | null {
  const source = record(value)
  if (!source) return null
  const usedPercent = numberOrNull(valueAt(source, "usedPercent", "used_percent"))
  if (usedPercent === null) return null
  const windowMinutes = numberOrNull(valueAt(source, "windowMinutes", "window_minutes", "windowDurationMins", "window_duration_mins"))
  const resetsAt = numberOrNull(valueAt(source, "resetsAt", "resets_at", "resetAt", "reset_at"))
  return {
    usedPercent,
    remainingPercent: Math.max(0, Math.min(100, 100 - usedPercent)),
    windowMinutes,
    resetsAt,
    label: rateLimitLabelForWindow(windowMinutes),
  }
}

function creditsFromUnknown(value: unknown): PylonCreditsSnapshot | null {
  const source = record(value)
  if (!source) return null
  const hasCredits = boolOrNull(valueAt(source, "hasCredits", "has_credits"))
  const unlimited = boolOrNull(valueAt(source, "unlimited"))
  if (hasCredits === null || unlimited === null) return null
  return {
    hasCredits,
    unlimited,
    balance: stringOrNull(valueAt(source, "balance")),
  }
}

function snapshotFromUnknown(
  provider: PylonAccountProvider,
  value: unknown,
  fallbackLimitId = provider === "codex" ? "codex" : "claude_agent",
): PylonProviderRateLimitSnapshot | null {
  const source = record(value)
  if (!source) return null
  const rateLimits = record(valueAt(source, "rate_limits", "rateLimits"))
  const snapshotSource = rateLimits && (rateLimits.primary || rateLimits.secondary) ? rateLimits : source
  const limitId =
    stringOrNull(valueAt(source, "limitId", "limit_id", "metered_limit_name", "limit_name")) ??
    fallbackLimitId
  const snapshot = {
    provider,
    limitId,
    limitName: stringOrNull(valueAt(source, "limitName", "limit_name")),
    primary: windowFromUnknown(valueAt(snapshotSource, "primary")),
    secondary: windowFromUnknown(valueAt(snapshotSource, "secondary")),
    credits: creditsFromUnknown(valueAt(source, "credits")),
    planType: stringOrNull(valueAt(source, "planType", "plan_type")),
    rateLimitReachedType: stringOrNull(valueAt(source, "rateLimitReachedType", "rate_limit_reached_type")),
  } satisfies PylonProviderRateLimitSnapshot
  return snapshot.primary || snapshot.secondary || snapshot.credits ? snapshot : null
}

export function providerRateLimitSnapshotsFromEvent(
  provider: PylonAccountProvider,
  value: unknown,
): PylonProviderRateLimitSnapshot[] {
  const source = record(value)
  if (!source) return []
  const snapshots: PylonProviderRateLimitSnapshot[] = []
  const push = (candidate: unknown, fallbackLimitId?: string) => {
    const parsed = snapshotFromUnknown(provider, candidate, fallbackLimitId)
    if (parsed) snapshots.push(parsed)
  }

  push(value)
  push(valueAt(source, "rate_limits", "rateLimits"))
  const params = record(valueAt(source, "params"))
  if (params) {
    push(valueAt(params, "rate_limits", "rateLimits"))
    push(valueAt(params, "rateLimits", "rate_limits"))
  }

  for (const key of ["rate_limits_by_limit_id", "rateLimitsByLimitId"] as const) {
    const byLimitId = record(valueAt(source, key))
    if (!byLimitId) continue
    for (const [limitId, snapshot] of Object.entries(byLimitId)) {
      push(snapshot, limitId)
    }
  }

  return uniqueSnapshots(snapshots)
}

function uniqueSnapshots(snapshots: PylonProviderRateLimitSnapshot[]) {
  const seen = new Set<string>()
  const result: PylonProviderRateLimitSnapshot[] = []
  for (const snapshot of snapshots) {
    const key = `${snapshot.provider}:${snapshot.limitId}:${snapshot.primary?.usedPercent ?? "-"}:${snapshot.secondary?.usedPercent ?? "-"}`
    if (seen.has(key)) continue
    seen.add(key)
    result.push(snapshot)
  }
  return result
}

export function parseCodexRateLimitHeaders(headers: Record<string, unknown>): PylonProviderRateLimitSnapshot[] {
  const lower = new Map<string, unknown>()
  for (const [key, value] of Object.entries(headers)) lower.set(key.toLowerCase(), value)
  const limitIds = new Set(["codex"])
  for (const key of lower.keys()) {
    const match = key.match(/^x-(.+)-(primary|secondary)-used-percent$/)
    if (match?.[1]) limitIds.add(match[1])
  }

  const snapshots: PylonProviderRateLimitSnapshot[] = []
  for (const limitId of [...limitIds].sort()) {
    const prefix = `x-${limitId}`
    const primary = windowFromUnknown({
      usedPercent: lower.get(`${prefix}-primary-used-percent`),
      windowMinutes: lower.get(`${prefix}-primary-window-minutes`),
      resetsAt: lower.get(`${prefix}-primary-reset-at`),
    })
    const secondary = windowFromUnknown({
      usedPercent: lower.get(`${prefix}-secondary-used-percent`),
      windowMinutes: lower.get(`${prefix}-secondary-window-minutes`),
      resetsAt: lower.get(`${prefix}-secondary-reset-at`),
    })
    const credits = limitId === "codex"
      ? creditsFromUnknown({
          hasCredits: lower.get("x-codex-credits-has-credits"),
          unlimited: lower.get("x-codex-credits-unlimited"),
          balance: lower.get("x-codex-credits-balance"),
        })
      : null
    if (!primary && !secondary && !credits) continue
    snapshots.push({
      provider: "codex",
      limitId: limitId.replaceAll("-", "_"),
      limitName: stringOrNull(lower.get(`${prefix}-limit-name`)),
      primary,
      secondary,
      credits,
      planType: null,
      rateLimitReachedType: null,
    })
  }
  return snapshots
}

export async function recordPylonAccountUsageObservation(
  summary: Pick<BootstrapSummary, "paths">,
  observation: AccountUsageObservation,
) {
  const observedAt = (observation.observedAt ?? new Date()).toISOString()
  const identity = accountIdentity(observation.provider, observation.account)
  const store = await loadAccountUsageStore(summary)
  const existing = store.accounts[identity.accountRefHash]
  const updated: PylonAccountUsageStoreEntry = {
    provider: observation.provider,
    selector: identity.selector,
    accountRefHash: identity.accountRefHash,
    providerTruth:
      observation.providerSnapshots && observation.providerSnapshots.length > 0
        ? { observedAt, snapshots: observation.providerSnapshots }
        : existing?.providerTruth ?? null,
    localSessionTruth:
      observation.localSessionUsage
        ? { observedAt, usage: observation.localSessionUsage }
        : existing?.localSessionTruth ?? null,
    updatedAt: observedAt,
  }
  store.accounts[identity.accountRefHash] = updated
  store.updatedAt = observedAt
  await saveAccountUsageStore(summary, store)
}

export function parsePylonAccountsUsageArgs(args: string[]): PylonAccountsUsageArgs {
  const parsed: PylonAccountsUsageArgs = {
    accountRef: null,
    provider: null,
    all: false,
    refresh: false,
    json: false,
  }
  for (let index = 0; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--json") {
      parsed.json = true
    } else if (arg === "--refresh") {
      parsed.refresh = true
    } else if (arg === "--all") {
      parsed.all = true
    } else if (arg === "--account") {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) throw new Error("--account requires an account ref or provider selector")
      parsed.accountRef = value
      index += 1
    } else if (arg === "--provider") {
      const value = args[index + 1]
      if (!value || value.startsWith("--")) throw new Error("--provider requires codex or claude_agent")
      const provider = providerSelectorFrom(value)
      if (!provider) throw new Error("--provider must be codex or claude_agent")
      parsed.provider = provider
      index += 1
    } else {
      throw new Error(`Unknown accounts usage option: ${arg}`)
    }
  }
  const selectorCount = [parsed.accountRef, parsed.provider, parsed.all ? "all" : null].filter(Boolean).length
  if (selectorCount > 1) throw new Error("Use only one of --account, --provider, or --all")
  return parsed
}

async function readinessForTarget(
  summary: Pick<BootstrapSummary, "paths">,
  target: AccountDiscoveryTarget,
  env: Record<string, string | undefined>,
): Promise<PylonAccountReadiness> {
  const effectiveEnv = target.account ? pylonAccountEnvironment(env, target.account) : env
  if (target.provider === "codex") {
    const config = await loadCodexAgentConfig(summary)
    return {
      provider: "codex",
      readiness: await probeCodexAgentReadiness({
        config,
        codexCliLoginPresent: await detectCodexCliLogin(effectiveEnv),
        env: effectiveEnv,
      }),
    }
  }
  const config = await loadClaudeAgentConfig(summary)
  return {
    provider: "claude_agent",
    readiness: await probeClaudeAgentReadiness({
      config,
      env: effectiveEnv,
      localSessionProbe: () => target.account
        ? pylonClaudeAccountHomeHasAuth(target.home)
        : pathIsDirectory(target.home),
    }),
  }
}

async function discoverAccountTargets(
  summary: Pick<BootstrapSummary, "paths">,
  env: Record<string, string | undefined>,
): Promise<AccountDiscoveryTarget[]> {
  const targets: AccountDiscoveryTarget[] = []
  const seen = new Set<string>()
  const addTarget = (target: AccountDiscoveryTarget) => {
    const key = `${target.provider}:${target.home}`
    if (seen.has(key)) return
    seen.add(key)
    targets.push(target)
  }

  for (const entry of await loadPylonAccountRegistry(summary)) {
    addTarget({
      provider: entry.provider,
      selector: "registry_ref",
      accountRef: entry.ref,
      accountRefHash: hashPylonAccountRef(entry.provider, entry.ref),
      home: entry.home,
      homeRef: stableRef(`home.pylon.${entry.provider}`, entry.home),
      account: {
        provider: entry.provider,
        selector: "registry_ref",
        accountRef: entry.ref,
        accountRefHash: hashPylonAccountRef(entry.provider, entry.ref),
        home: entry.home,
      },
    })
  }

  for (const provider of ["codex", "claude_agent"] as const) {
    const home = defaultHome(provider, env)
    addTarget({
      provider,
      selector: "default_home",
      accountRef: null,
      accountRefHash: hashPylonAccountRef(provider, "default"),
      home,
      homeRef: stableRef(`home.pylon.${provider}`, home),
      account: null,
    })
  }

  // #4953: surface ALL account homes present on this machine, not just the two
  // default homes. Scan the home dir for sibling account homes (e.g.
  // ~/.codex-pylon-b, ~/.claude-work) so multi-account setups show every
  // account without manual registry entries. Read-only; deduped by provider:home.
  for (const sibling of await discoverPylonSiblingAccountHomes(env)) {
    addTarget({
      provider: sibling.provider,
      selector: "registry_ref",
      accountRef: sibling.ref,
      accountRefHash: hashPylonAccountRef(sibling.provider, sibling.home),
      home: sibling.home,
      homeRef: stableRef(`home.pylon.${sibling.provider}`, sibling.home),
      account: {
        provider: sibling.provider,
        selector: "registry_ref",
        accountRef: sibling.ref,
        accountRefHash: hashPylonAccountRef(sibling.provider, sibling.home),
        home: sibling.home,
      },
    })
  }

  return targets
}

export async function resolvePylonAccountUsageRefreshTargets(
  summary: Pick<BootstrapSummary, "paths">,
  args: Pick<PylonAccountsUsageArgs, "accountRef" | "provider" | "all">,
  options: { env?: Record<string, string | undefined> } = {},
): Promise<PylonAccountUsageRefreshTarget[]> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const targets = await selectAccountUsageTargets(summary, args, env)
  return targets.map((target) => ({
    provider: target.provider,
    selector: target.selector,
    accountRef: target.accountRef,
    accountRefHash: target.accountRefHash,
    account: target.account,
  }))
}

async function selectAccountUsageTargets(
  summary: Pick<BootstrapSummary, "paths">,
  args: Pick<PylonAccountsUsageArgs, "accountRef" | "provider" | "all">,
  env: Record<string, string | undefined>,
): Promise<AccountDiscoveryTarget[]> {
  const targets = (await discoverAccountTargets(summary, env)).filter((target) => {
    if (args.accountRef) return target.accountRef === args.accountRef
    if (args.provider) return target.provider === args.provider && target.selector === "default_home"
    if (args.all) return true
    return target.selector === "default_home"
  })
  if (args.accountRef && targets.length === 0) {
    const provider = providerSelectorFrom(args.accountRef)
    if (provider) {
      return (await discoverAccountTargets(summary, env)).filter(
        (target) => target.provider === provider && target.selector === "default_home",
      )
    }
    throw new Error(`unknown account ref or provider selector: ${args.accountRef}`)
  }
  return targets
}

export async function collectPylonAccountsList(
  summary: Pick<BootstrapSummary, "paths">,
  options: { env?: Record<string, string | undefined>; now?: Date } = {},
): Promise<PylonAccountsListProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const observedAt = (options.now ?? new Date()).toISOString()
  const accounts: PylonAccountListEntry[] = []
  for (const target of await discoverAccountTargets(summary, env)) {
    const probed = await readinessForTarget(summary, target, env)
    const homePresent = await pathIsDirectory(target.home)
    accounts.push({
      provider: target.provider,
      selector: target.selector,
      accountRef: target.accountRef,
      accountRefHash: target.accountRefHash,
      homeRef: target.homeRef,
      homeState: homePresent ? "present" : "missing",
      readiness: probed.readiness,
      blockerRefs: [
        ...(homePresent ? [] : [`blocker.pylon.accounts.${target.provider}_home_missing`]),
        ...probed.readiness.blockerRefs,
      ],
    })
  }
  const projection = {
    schema: PYLON_ACCOUNTS_LIST_SCHEMA,
    observedAt,
    accounts,
    blockerRefs: accounts.flatMap((account) => account.blockerRefs),
  } satisfies PylonAccountsListProjection
  assertPublicProjectionSafe(projection)
  return projection
}

// Local-only (NOT public-safe) view of connected Codex accounts for the operator
// CLI: reads each account home's auth.json for the ChatGPT email (id_token claim)
// and when it was last linked/refreshed. Deliberately kept OUT of the public-safe
// projection because it surfaces the owner's own account email (PII).
export type PylonCodexAccountLocal = {
  accountRef: string | null
  home: string
  email: string | null
  lastLinkedAt: string | null
  homeState: "present" | "missing"
}

const decodeCodexIdTokenEmail = (idToken: string): string | null => {
  try {
    const payload = idToken.split(".")[1]
    if (payload === undefined) return null
    const claims = JSON.parse(
      Buffer.from(payload, "base64url").toString("utf8"),
    ) as Record<string, unknown>
    const profile = claims["https://api.openai.com/profile"]
    const auth = claims["https://api.openai.com/auth"]
    const candidates: unknown[] = [
      claims.email,
      typeof profile === "object" && profile !== null ? (profile as Record<string, unknown>).email : undefined,
      typeof auth === "object" && auth !== null ? (auth as Record<string, unknown>).email : undefined,
    ]
    const email = candidates.find((value) => typeof value === "string" && value.includes("@"))
    return typeof email === "string" ? email : null
  } catch {
    return null
  }
}

export async function collectPylonCodexAccountsLocal(
  summary: Pick<BootstrapSummary, "paths">,
  options: { env?: Record<string, string | undefined> } = {},
): Promise<PylonCodexAccountLocal[]> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const accounts: PylonCodexAccountLocal[] = []
  for (const target of await discoverAccountTargets(summary, env)) {
    if (target.provider !== "codex") continue
    const authPath = join(target.home, "auth.json")
    let email: string | null = null
    let lastLinkedAt: string | null = null
    let present = false
    try {
      const raw = JSON.parse(await readFile(authPath, "utf8")) as Record<string, unknown>
      present = true
      const tokens = typeof raw.tokens === "object" && raw.tokens !== null ? (raw.tokens as Record<string, unknown>) : {}
      const idToken = tokens.id_token ?? raw.id_token
      if (typeof idToken === "string") email = decodeCodexIdTokenEmail(idToken)
      const lastRefresh = raw.last_refresh ?? tokens.last_refresh
      lastLinkedAt = typeof lastRefresh === "string"
        ? lastRefresh
        : (await stat(authPath)).mtime.toISOString()
    } catch {
      // missing / unreadable home -> reported as not present
    }
    accounts.push({
      accountRef: target.accountRef,
      home: target.home,
      email,
      lastLinkedAt,
      homeState: present ? "present" : "missing",
    })
  }
  return accounts
}

function ageSeconds(observedAt: string | null, now: Date): number | null {
  if (!observedAt) return null
  const parsed = Date.parse(observedAt)
  if (!Number.isFinite(parsed)) return null
  return Math.max(0, Math.round((now.getTime() - parsed) / 1000))
}

function providerTruth(entry: PylonAccountUsageStoreEntry | undefined, now: Date): ProviderTruthProjection {
  if (!entry?.providerTruth) {
    return {
      state: "missing",
      observedAt: null,
      ageSeconds: null,
      snapshots: [],
      blockerRefs: ["blocker.pylon.accounts_usage.provider_truth_missing"],
    }
  }
  const age = ageSeconds(entry.providerTruth.observedAt, now)
  const stale = age !== null && age > ACCOUNT_USAGE_STALE_SECONDS
  return {
    state: stale ? "stale" : "available",
    observedAt: entry.providerTruth.observedAt,
    ageSeconds: age,
    snapshots: entry.providerTruth.snapshots,
    blockerRefs: stale ? ["blocker.pylon.accounts_usage.provider_truth_stale"] : [],
  }
}

function localSessionTruth(entry: PylonAccountUsageStoreEntry | undefined, now: Date): LocalSessionTruthProjection {
  if (!entry?.localSessionTruth) {
    return {
      state: "missing",
      observedAt: null,
      ageSeconds: null,
      usage: null,
      blockerRefs: ["blocker.pylon.accounts_usage.local_session_truth_missing"],
    }
  }
  return {
    state: "available",
    observedAt: entry.localSessionTruth.observedAt,
    ageSeconds: ageSeconds(entry.localSessionTruth.observedAt, now),
    usage: entry.localSessionTruth.usage,
    blockerRefs: [],
  }
}

function refFromPlatformValue(value: unknown): string | null {
  if (typeof value === "string" && value.trim().length > 0) {
    return stableRef("account.pylon.platform", value.trim())
  }
  const source = record(value)
  if (!source) return null
  const raw =
    stringOrNull(valueAt(source, "accountRef", "account_ref", "providerAccountRef", "provider_account_ref", "ref", "id")) ??
    JSON.stringify(source)
  return stableRef("account.pylon.platform", raw)
}

function refListFromPool(payload: Record<string, unknown>, keys: string[]) {
  const refs: string[] = []
  for (const key of keys) {
    const value = valueAt(payload, key)
    const items = Array.isArray(value) ? value : value === undefined ? [] : [value]
    for (const item of items) {
      const ref = refFromPlatformValue(item)
      if (ref) refs.push(ref)
    }
  }
  return [...new Set(refs)].sort()
}

async function platformTruth(env: Record<string, string | undefined>, now: Date): Promise<PlatformTruthProjection> {
  const baseUrl = (env.PYLON_OPENAGENTS_BASE_URL ?? env.OPENAGENTS_BASE_URL ?? "").trim().replace(/\/$/, "")
  if (!baseUrl) {
    return {
      state: "unavailable",
      observedAt: null,
      pool: null,
      blockerRefs: ["blocker.pylon.accounts_usage.platform_truth_unconfigured"],
    }
  }
  try {
    const response = await fetch(`${baseUrl}/api/provider-accounts/pool`, {
      headers: {
        accept: "application/json",
        ...(env.OPENAGENTS_AGENT_TOKEN ? { authorization: `Bearer ${env.OPENAGENTS_AGENT_TOKEN}` } : {}),
      },
    })
    if (!response.ok) throw new Error("provider pool request failed")
    const body = record(await response.json())
    if (!body) throw new Error("provider pool response was not an object")
    const poolSource = record(valueAt(body, "pool")) ?? body
    return {
      state: "available",
      observedAt: now.toISOString(),
      pool: {
        lowCreditAccountRefs: refListFromPool(poolSource, [
          "lowCredit",
          "low_credit",
          "lowCreditAccounts",
          "low_credit_accounts",
          "lowCreditAccountRefs",
        ]),
        cooldownAccountRefs: refListFromPool(poolSource, [
          "cooldown",
          "cooldowns",
          "cooldownAccounts",
          "cooldown_accounts",
          "cooldownAccountRefs",
        ]),
        leasedAccountRefs: refListFromPool(poolSource, [
          "leased",
          "leases",
          "leasedAccounts",
          "leased_accounts",
          "leaseAccountRefs",
        ]),
      },
      blockerRefs: [],
    }
  } catch {
    return {
      state: "unavailable",
      observedAt: null,
      pool: null,
      blockerRefs: ["blocker.pylon.accounts_usage.platform_truth_unreachable"],
    }
  }
}

async function sharedPlatformTruth(env: Record<string, string | undefined>, now: Date) {
  return platformTruth(env, now)
}

function clonePlatformTruth(value: PlatformTruthProjection): PlatformTruthProjection {
  return {
    state: value.state,
    observedAt: value.observedAt,
    pool: value.pool
      ? {
          lowCreditAccountRefs: [...value.pool.lowCreditAccountRefs],
          cooldownAccountRefs: [...value.pool.cooldownAccountRefs],
          leasedAccountRefs: [...value.pool.leasedAccountRefs],
        }
      : null,
    blockerRefs: [...value.blockerRefs],
  }
}

export async function collectPylonAccountsUsage(
  summary: Pick<BootstrapSummary, "paths">,
  args: PylonAccountsUsageArgs,
  options: { env?: Record<string, string | undefined>; now?: Date } = {},
): Promise<PylonAccountsUsageProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const now = options.now ?? new Date()
  const observedAt = now.toISOString()
  const store = await loadAccountUsageStore(summary)
  const targets = await selectAccountUsageTargets(summary, args, env)
  const platform = await sharedPlatformTruth(env, now)
  const accounts: PylonAccountsUsageProjection["accounts"] = []
  for (const target of targets) {
    const readiness = (await readinessForTarget(summary, target, env)).readiness
    const entry = store.accounts[target.accountRefHash]
    const provider = providerTruth(entry, now)
    const local = localSessionTruth(entry, now)
    const platformForAccount = clonePlatformTruth(platform)
    accounts.push({
      provider: target.provider,
      accountRef: target.accountRef,
      accountRefHash: target.accountRefHash,
      readiness,
      truth: {
        provider,
        localSession: local,
        platform: platformForAccount,
      },
      blockerRefs: [
        ...readiness.blockerRefs,
        ...provider.blockerRefs,
        ...local.blockerRefs,
        ...platformForAccount.blockerRefs,
      ],
    })
  }
  const refreshBlockers = args.refresh ? [] : ["blocker.pylon.accounts_usage.refresh_not_requested"]
  const projection = {
    schema: PYLON_ACCOUNTS_USAGE_SCHEMA,
    observedAt,
    refresh: {
      requested: args.refresh,
      performed: false,
      costStatement: args.refresh ? costStatement : null,
      blockerRefs: refreshBlockers,
    },
    accounts,
    blockerRefs: accounts.flatMap((account) => account.blockerRefs),
  } satisfies PylonAccountsUsageProjection
  assertPublicProjectionSafe(projection)
  return projection
}

export async function collectPylonAccountUsageSummary(
  summary: Pick<BootstrapSummary, "paths">,
  options: { now?: Date } = {},
): Promise<PylonAccountUsageSummary | null> {
  const now = options.now ?? new Date()
  const store = await loadAccountUsageStore(summary)
  const entries = Object.values(store.accounts)
  if (entries.length === 0) return null
  const accounts = entries.map((entry) => {
    const provider = providerTruth(entry, now)
    const local = localSessionTruth(entry, now)
    const latestObservedAt = [provider.observedAt, local.observedAt]
      .filter((value): value is string => typeof value === "string")
      .sort()
      .at(-1) ?? null
    return {
      provider: entry.provider,
      accountRefHash: entry.accountRefHash,
      providerTruthState: provider.state,
      localSessionState: local.state,
      latestObservedAt,
    }
  })
  const summaryProjection = {
    schema: PYLON_ACCOUNT_USAGE_SUMMARY_SCHEMA,
    observedAt: now.toISOString(),
    accountCount: entries.length,
    providerTruthAccountCount: accounts.filter((account) => account.providerTruthState !== "missing").length,
    localSessionAccountCount: accounts.filter((account) => account.localSessionState !== "missing").length,
    staleProviderTruthCount: accounts.filter((account) => account.providerTruthState === "stale").length,
    accounts,
  } satisfies PylonAccountUsageSummary
  assertPublicProjectionSafe(summaryProjection)
  return summaryProjection
}
