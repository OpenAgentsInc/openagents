/**
 * Account connect (custody) — moved from `apps/pylon/src/account-connect.ts`
 * (issue #8578, PY-1). Its only remaining out-of-package dependency was
 * `recordAccountLinkInPresence` from `presence.ts`, which the same session
 * moved into `presence/presence.ts`.
 *
 * One piece stays in `apps/pylon`: `defaultCodexAuthValidityProbe`, the
 * default implementation of the `codexAuthValidityProbe` injection point
 * below. It does a dynamic `import("./codex-composer.js")` to run the real
 * vendored Codex CLI binary, and `codex-composer.ts` is not (yet) in-package.
 * `runPylonAccountsConnect` here never calls it directly — it only invokes
 * `options.codexAuthValidityProbe?.(...)` when a caller supplies one (see
 * `apps/pylon/src/auth.ts`, which explicitly passes
 * `options.codexAuthValidityProbe ?? defaultCodexAuthValidityProbe`) — so
 * this module has zero static or dynamic dependency on codex-composer.ts.
 * `apps/pylon/src/account-connect.ts` (the shim) defines and re-exports
 * `defaultCodexAuthValidityProbe` alongside this module's exports so
 * existing consumers (`auth.ts`, `codex-account-auth-health.ts`) see the
 * same combined surface they always have.
 */
import { existsSync } from "node:fs"
import { chmod, mkdir, readFile, rename, writeFile } from "node:fs/promises"
import { dirname, join } from "node:path"
import {
  isolateGrokCliEnvironment,
  probeGrokReadiness,
  type GrokReadiness,
} from "@openagentsinc/grok-harness/worker-executor"

import {
  hashPylonAccountRef,
  normalizeAccountHome,
  pylonClaudeAccountHomeHasAuth,
  PYLON_CLAUDE_OAUTH_TOKEN_FILE,
  type PylonAccountProvider,
} from "./account-registry.js"
import type { BootstrapSummary } from "../shared/bootstrap.js"
import { recordAccountLinkInPresence } from "../presence/presence.js"
import { assertPublicProjectionSafe } from "../shared/state.js"
import {
  classifyCodexAccountFailure,
  type PylonCodexAccountFailure,
  type PylonCodexAccountHealthReason,
} from "./codex-account-health.js"
import { clearCodexAccountHealthFailure } from "./codex-account-health-ledger.js"

export type PylonAccountsConnectArgs = {
  provider: PylonAccountProvider
  accountRef: string
  accountLabel: string | null
  agentToken: string | null
  baseUrl: string | null
  createNewOpenAgentsAccount: boolean
  home: string | null
  forceDeviceLogin: boolean
  json: boolean
  openAgentsAttemptId: string | null
  openAgentsLink: boolean
  providerAccountRef: string | null
  /**
   * Claude setup-token (CLAUDE_CODE_OAUTH_TOKEN material). Never projected.
   * Accepted via `--token` / `--setup-token` CLI flags or env.
   */
  setupToken: string | null
  skipDeviceLogin: boolean
}

export type PylonCodexDeviceLoginRunner = (input: {
  env: Record<string, string | undefined>
  home: string
}) => Promise<{ exitCode: number }>

export type PylonGrokDeviceLoginRunner = PylonCodexDeviceLoginRunner

export type PylonGrokReadinessProbe = (input: {
  env: Record<string, string | undefined>
  home: string
  timeoutMs: number
}) => Promise<Pick<GrokReadiness, "failureClass" | "plane" | "ready">>

/**
 * Reasons a stored Codex `auth.json` can be present-but-unusable. These are
 * public-safe enum refs only; they never carry token material.
 */
export type PylonCodexAuthInvalidReason = Exclude<PylonCodexAccountHealthReason, "network" | "timeout" | "other"> | "auth_error"

/**
 * Result of probing whether a stored Codex credential is actually usable, not
 * merely present on disk. A present `auth.json` whose refresh token has been
 * revoked is INVALID even though the file exists (openagents Codex reconnect
 * bug): `existsSync` is not a validity check.
 */
export type PylonCodexAuthValidity =
  | { valid: true; reason?: string }
  | { valid: false; reason: PylonCodexAuthInvalidReason; failure?: PylonCodexAccountFailure }

/**
 * Injectable probe that classifies a stored Codex credential as valid or
 * invalid (with a reason). Production injects `defaultCodexAuthValidityProbe`
 * (defined in `apps/pylon/src/account-connect.ts`, since it depends on the
 * vendored Codex CLI resolver in `codex-composer.ts`); tests inject
 * deterministic stubs. The probe MUST be bounded and fail-safe: if it cannot
 * reach a confident verdict it returns `valid: true` so a working reconnect
 * is never blocked by probe infrastructure failure.
 */
export type PylonCodexAuthValidityProbe = (input: {
  env: Record<string, string | undefined>
  home: string
}) => Promise<PylonCodexAuthValidity>

export type PylonAccountsConnectFetcher = typeof fetch

export type PylonAccountConnectProjection = {
  schema: "pylon.accounts.connect.v1"
  provider: PylonAccountProvider
  accountRef: string
  accountRefHash: string
  homeRef: string
  homeState: "present"
  codexCredentialStore: "file" | "not_applicable"
  registry: {
    status: "created" | "updated" | "unchanged"
  }
  deviceLogin: {
    status:
      | "completed"
      | "skipped_existing_auth"
      | "skipped_by_flag"
      | "completed_recovered_invalid_auth"
      | "blocked_invalid_auth"
    reason?: string
  }
  openAgentsDeviceLogin:
    | { status: "not_requested" }
    | {
        status: "started"
        attemptId: string
        expiresAt: string
        intervalSeconds: number
        providerAccountRef: string
        userCode: string
        verificationUrl: string
        pylonLink: { owner: "openauth"; status: "linked" }
      }
    | {
        status: "polled"
        attemptId: string
        attemptStatus: string
        accountStatus: string
        providerAccountRef: string
        pylonLink: { owner: "openauth"; status: "linked" }
      }
  blockerRefs: string[]
}

type ConfigRecord = Record<string, unknown>

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/

function readRequiredValue(args: string[], index: number, option: string): string {
  const value = args[index + 1]
  if (value === undefined || value.startsWith("--") || value.trim() === "") {
    throw new Error(`${option} requires a value`)
  }
  return value
}

const ACCOUNTS_CONNECT_USAGE =
  "usage: pylon accounts connect codex|claude|grok --account <ref> [--home <path>] [--token <setup-token>] [--openagents-link|--openagents-attempt-id <id>] --json"

/**
 * Normalize CLI provider aliases to the registry provider id.
 * `claude` is the public CLI alias; `claude_agent` is the custody registry id.
 */
export function normalizeAccountsConnectProvider(raw: string | undefined): PylonAccountProvider | null {
  if (raw === "codex") return "codex"
  if (raw === "claude" || raw === "claude_agent") return "claude_agent"
  if (raw === "grok") return "grok"
  return null
}

export function parsePylonAccountsConnectArgs(args: string[]): PylonAccountsConnectArgs {
  const provider = normalizeAccountsConnectProvider(args[0])
  if (provider === null) {
    throw new Error(ACCOUNTS_CONNECT_USAGE)
  }

  const parsed: PylonAccountsConnectArgs = {
    provider,
    accountRef: "",
    accountLabel: null,
    agentToken: null,
    baseUrl: null,
    createNewOpenAgentsAccount: true,
    home: null,
    forceDeviceLogin: false,
    json: false,
    openAgentsAttemptId: null,
    openAgentsLink: false,
    providerAccountRef: null,
    setupToken: null,
    skipDeviceLogin: false,
  }

  for (let index = 1; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--account" || arg === "--account-ref") {
      parsed.accountRef = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--account-label") {
      parsed.accountLabel = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--agent-token") {
      parsed.agentToken = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--base-url") {
      parsed.baseUrl = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (
      arg === "--home" ||
      arg === "--codex-home" ||
      arg === "--claude-home" ||
      arg === "--claude-config-dir" ||
      arg === "--grok-home"
    ) {
      parsed.home = readRequiredValue(args, index, arg)
      index += 1
    } else if (arg === "--token" || arg === "--setup-token") {
      parsed.setupToken = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--openagents-link") {
      parsed.openAgentsLink = true
    } else if (arg === "--openagents-attempt-id") {
      parsed.openAgentsAttemptId = readRequiredValue(args, index, arg).trim()
      parsed.openAgentsLink = true
      index += 1
    } else if (arg === "--provider-account-ref") {
      parsed.providerAccountRef = readRequiredValue(args, index, arg).trim()
      parsed.createNewOpenAgentsAccount = false
      index += 1
    } else if (arg === "--force-device-login") {
      parsed.forceDeviceLogin = true
    } else if (arg === "--skip-device-login") {
      parsed.skipDeviceLogin = true
    } else if (arg === "--json") {
      parsed.json = true
    } else {
      throw new Error(`Unknown accounts connect option: ${arg}`)
    }
  }

  if (!accountRefPattern.test(parsed.accountRef)) {
    throw new Error("pylon accounts connect requires --account <ref> with letters, numbers, dot, dash, or underscore")
  }
  if (parsed.forceDeviceLogin && parsed.skipDeviceLogin) {
    throw new Error("Use either --force-device-login or --skip-device-login, not both")
  }
  if (parsed.provider === "claude_agent") {
    if (parsed.forceDeviceLogin || parsed.skipDeviceLogin) {
      throw new Error(
        "Claude connect does not use device-login; provide --token / --setup-token or CLAUDE_CODE_OAUTH_TOKEN (or an existing claude-oauth-token file in the account home)",
      )
    }
    if (parsed.openAgentsLink || parsed.openAgentsAttemptId !== null) {
      throw new Error(
        "Claude connect does not support --openagents-link / --openagents-attempt-id (Codex device-login only)",
      )
    }
  }
  if (parsed.provider === "grok") {
    if (parsed.home !== null) {
      throw new Error(
        "Grok connect always uses the isolated <pylon home>/accounts/grok/<ref> home; --home / --grok-home is not accepted",
      )
    }
    if (
      parsed.openAgentsLink ||
      parsed.openAgentsAttemptId !== null ||
      parsed.providerAccountRef !== null
    ) {
      throw new Error(
        "Grok connect does not support the Codex OpenAgents provider-account link flow",
      )
    }
  }
  if (parsed.provider !== "claude_agent" && parsed.setupToken !== null) {
    throw new Error("--token / --setup-token is only valid for pylon accounts connect claude")
  }

  return parsed
}

const stableHomeRef = (provider: PylonAccountProvider, home: string): string =>
  hashPylonAccountRef(provider, `home:${home}`)

function defaultAccountHome(summary: Pick<BootstrapSummary, "paths">, provider: PylonAccountProvider, ref: string): string {
  return join(summary.paths.home, "accounts", provider, ref)
}

async function readConfig(path: string): Promise<ConfigRecord> {
  try {
    const value = JSON.parse(await readFile(path, "utf8"))
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as ConfigRecord)
      : {}
  } catch {
    return {}
  }
}

async function writeConfig(path: string, config: ConfigRecord): Promise<void> {
  await mkdir(dirname(path), { recursive: true })
  const tempPath = `${path}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tempPath, `${JSON.stringify(config, null, 2)}\n`)
  await rename(tempPath, path)
}

function registryStatusFor(
  config: ConfigRecord,
  input: {
    provider: PylonAccountProvider
    accountRef: string
    home: string
    openAgentsProviderAccountRef?: string
  },
): "created" | "updated" | "unchanged" {
  const dev =
    config.dev !== null && typeof config.dev === "object" && !Array.isArray(config.dev)
      ? (config.dev as ConfigRecord)
      : {}
  const accounts = Array.isArray(dev.accounts) ? [...dev.accounts] : []
  const nextAccount = {
    ref: input.accountRef,
    provider: input.provider,
    home: input.home,
    ...(input.openAgentsProviderAccountRef === undefined
      ? {}
      : { openAgentsProviderAccountRef: input.openAgentsProviderAccountRef }),
  }
  const existingIndex = accounts.findIndex(account => {
    if (account === null || typeof account !== "object" || Array.isArray(account)) return false
    const record = account as ConfigRecord
    return record.ref === input.accountRef && record.provider === input.provider
  })

  if (existingIndex === -1) {
    accounts.push(nextAccount)
    config.dev = { ...dev, accounts }
    return "created"
  }

  const existing = accounts[existingIndex]
  if (
    existing !== null &&
    typeof existing === "object" &&
    !Array.isArray(existing) &&
    (existing as ConfigRecord).home === input.home &&
    (input.openAgentsProviderAccountRef === undefined ||
      (existing as ConfigRecord).openAgentsProviderAccountRef ===
        input.openAgentsProviderAccountRef)
  ) {
    return "unchanged"
  }

  accounts[existingIndex] = {
    ...(existing !== null && typeof existing === "object" && !Array.isArray(existing) ? existing : {}),
    ...nextAccount,
  }
  config.dev = { ...dev, accounts }
  return "updated"
}

function codexConfigWithFileCredentialStore(raw: string): string {
  const fileStoreLine = 'cli_auth_credentials_store = "file"'
  if (/^cli_auth_credentials_store\s*=\s*["'][^"']+["']\s*$/m.test(raw)) {
    return raw.replace(
      /^cli_auth_credentials_store\s*=\s*["'][^"']+["']\s*$/m,
      fileStoreLine,
    )
  }
  const trimmed = raw.trimEnd()
  return `${trimmed.length === 0 ? "" : `${trimmed}\n`}${fileStoreLine}\n`
}

async function forceCodexFileCredentialStore(home: string): Promise<void> {
  const configPath = join(home, "config.toml")
  let raw = ""
  try {
    raw = await readFile(configPath, "utf8")
  } catch {
    // A fresh account home is normal.
  }
  const next = codexConfigWithFileCredentialStore(raw)
  if (next !== raw) {
    await writeFile(configPath, next)
  }
}

/**
 * Persist a long-lived Claude setup-token into the isolated account home as
 * `claude-oauth-token` (mode 0600). The token material is never returned from
 * connect and must never appear in public projections.
 */
export async function writeClaudeOauthTokenFile(home: string, token: string): Promise<void> {
  const trimmed = token.trim()
  if (trimmed === "") {
    throw new Error("Claude setup-token must be a non-empty string")
  }
  await mkdir(home, { recursive: true })
  const path = join(home, PYLON_CLAUDE_OAUTH_TOKEN_FILE)
  const tempPath = `${path}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tempPath, `${trimmed}\n`, { mode: 0o600 })
  await chmod(tempPath, 0o600).catch(() => undefined)
  await rename(tempPath, path)
  await chmod(path, 0o600).catch(() => undefined)
}

function resolveClaudeSetupToken(
  args: PylonAccountsConnectArgs,
  env: Record<string, string | undefined>,
): string | null {
  const fromArgs = (args.setupToken ?? "").trim()
  if (fromArgs !== "") return fromArgs
  const fromEnv = (env.CLAUDE_CODE_OAUTH_TOKEN ?? "").trim()
  if (fromEnv !== "") return fromEnv
  return null
}

const defaultCodexDeviceLoginRunner: PylonCodexDeviceLoginRunner = async input => {
  const child = Bun.spawn(["codex", "login", "--device-auth"], {
    env: {
      ...process.env,
      ...input.env,
      CODEX_HOME: input.home,
    },
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })
  return { exitCode: await child.exited }
}

const defaultGrokDeviceLoginRunner: PylonGrokDeviceLoginRunner = async input => {
  const env = isolateGrokCliEnvironment({
    ...process.env,
    ...input.env,
  }, input.home)
  const child = Bun.spawn(["grok", "login", "--device-auth"], {
    env,
    stdin: "inherit",
    stdout: "inherit",
    stderr: "inherit",
  })
  return { exitCode: await child.exited }
}

const GROK_READINESS_TIMEOUT_DEFAULT_MS = 10_000

const grokReadinessTimeoutMs = (
  env: Record<string, string | undefined>,
): number => {
  const parsed = Number.parseInt(
    (env.PYLON_GROK_READINESS_TIMEOUT_MS ?? "").trim(),
    10,
  )
  return Number.isFinite(parsed)
    ? Math.max(100, Math.min(60_000, parsed))
    : GROK_READINESS_TIMEOUT_DEFAULT_MS
}

const defaultGrokReadinessProbe: PylonGrokReadinessProbe = async input =>
  probeGrokReadiness({
    env: input.env,
    timeoutMs: input.timeoutMs,
  })

/**
 * Classifies the output of a bounded Codex credential probe. Recognized
 * failure signatures map to a concrete invalid reason; an unrecognized failure
 * is treated as fail-safe `valid` (with an inconclusive reason) so an ambiguous
 * probe never forces a disruptive re-login or falsely claims a credential is
 * dead. A revoked refresh token always classifies as `credentials_revoked`.
 */
export function classifyCodexAuthProbeOutput(input: {
  exitCode: number
  stdout: string
  stderr: string
}): PylonCodexAuthValidity {
  const text = `${input.stdout}\n${input.stderr}`
  const failure = classifyCodexAccountFailure(text)
  if (failure.reason === "credentials_revoked") return { valid: false, reason: "credentials_revoked", failure }
  if (failure.reason === "usage_limited" || failure.reason === "rate_limited") return { valid: false, reason: failure.reason, failure }
  if (
    /could not be refreshed|refresh token|token (?:has )?expired|expired token|unauthorized|\b401\b|not logged in|please (?:sign in|log ?in)|sign in again|authentication (?:failed|error)|invalid (?:token|credential)/.test(
      text.toLowerCase(),
    )
  ) {
    return { valid: false, reason: "auth_error", failure }
  }
  if (input.exitCode === 0) {
    return { valid: true }
  }
  return { valid: true, reason: "probe_inconclusive" }
}

/**
 * Public-safe outcome a CLI should report for a Codex auth/connect result.
 * Drives honest messaging: a present-but-revoked credential that could not be
 * recovered MUST NOT be reported as a bare success.
 */
export type PylonCodexAuthCliOutcome =
  | { ok: true; kind: "linked" | "reauthed"; reason?: string }
  | { ok: false; kind: "blocked"; reason?: string }

export function pylonCodexAuthCliOutcome(
  deviceLoginStatus: PylonAccountConnectProjection["deviceLogin"]["status"],
  reason?: string,
): PylonCodexAuthCliOutcome {
  if (deviceLoginStatus === "completed_recovered_invalid_auth") {
    return { ok: true, kind: "reauthed", ...(reason ? { reason } : {}) }
  }
  if (deviceLoginStatus === "blocked_invalid_auth") {
    return { ok: false, kind: "blocked", ...(reason ? { reason } : {}) }
  }
  return { ok: true, kind: "linked", ...(reason ? { reason } : {}) }
}

const withoutTrailingSlash = (value: string): string => value.replace(/\/+$/, "")

function requireOpenAgentsLinkOptions(
  args: PylonAccountsConnectArgs,
  env: Record<string, string | undefined>,
): { agentToken: string; baseUrl: string } {
  const baseUrl = withoutTrailingSlash(
    args.baseUrl ?? env.PYLON_OPENAGENTS_BASE_URL ?? env.OPENAGENTS_BASE_URL ?? "https://openagents.com",
  )
  const agentToken = (args.agentToken ?? env.OPENAGENTS_AGENT_TOKEN ?? "").trim()
  if (agentToken === "") {
    throw new Error("OPENAGENTS_AGENT_TOKEN or --agent-token is required for --openagents-link")
  }
  return { agentToken, baseUrl }
}

function stringRecord(value: unknown): Record<string, unknown> {
  return value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {}
}

function requiredString(source: Record<string, unknown>, key: string): string {
  const value = source[key]
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`OpenAgents device-login response is missing ${key}`)
  }
  return value
}

function requiredNumber(source: Record<string, unknown>, key: string): number {
  const value = source[key]
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`OpenAgents device-login response is missing ${key}`)
  }
  return value
}

function linkedPylonMetadataFrom(body: Record<string, unknown>): {
  owner: "openauth"
  status: "linked"
} {
  const pylonLink = stringRecord(body.pylonLink)
  if (pylonLink.owner !== "openauth" || pylonLink.status !== "linked") {
    throw new Error("OpenAgents device-login response did not confirm a linked Pylon owner")
  }
  return { owner: "openauth", status: "linked" }
}

async function readOpenAgentsJsonResponse(response: Response): Promise<Record<string, unknown>> {
  const body = stringRecord(await response.json().catch(() => ({})))
  if (!response.ok) {
    const message =
      typeof body.message === "string"
        ? body.message
        : `OpenAgents device-login request failed with status ${response.status}`
    throw new Error(message)
  }
  return body
}

async function runOpenAgentsPylonDeviceLogin(input: {
  args: PylonAccountsConnectArgs
  env: Record<string, string | undefined>
  fetcher: PylonAccountsConnectFetcher
}): Promise<PylonAccountConnectProjection["openAgentsDeviceLogin"]> {
  if (!input.args.openAgentsLink) {
    return { status: "not_requested" }
  }
  const { agentToken, baseUrl } = requireOpenAgentsLinkOptions(input.args, input.env)
  if (input.args.openAgentsAttemptId !== null) {
    const response = await input.fetcher(
      `${baseUrl}/api/pylon/provider-accounts/chatgpt-codex/device-login/${encodeURIComponent(input.args.openAgentsAttemptId)}`,
      {
        method: "GET",
        headers: {
          authorization: `Bearer ${agentToken}`,
        },
      },
    )
    const body = await readOpenAgentsJsonResponse(response)
    const account = stringRecord(body.account)
    const attempt = stringRecord(body.attempt)
    return {
      status: "polled",
      attemptId: requiredString(attempt, "id"),
      attemptStatus: requiredString(attempt, "status"),
      accountStatus: requiredString(account, "status"),
      providerAccountRef: requiredString(account, "providerAccountRef"),
      pylonLink: linkedPylonMetadataFrom(body),
    }
  }

  const response = await input.fetcher(
    `${baseUrl}/api/pylon/provider-accounts/chatgpt-codex/device-login/start`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${agentToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        accountLabel: input.args.accountLabel ?? input.args.accountRef,
        createNew: input.args.createNewOpenAgentsAccount,
        ...(input.args.providerAccountRef === null
          ? {}
          : { providerAccountRef: input.args.providerAccountRef }),
      }),
    },
  )
  const body = await readOpenAgentsJsonResponse(response)
  const attempt = stringRecord(body.attempt)

  return {
    status: "started",
    attemptId: requiredString(attempt, "id"),
    expiresAt: requiredString(body, "expiresAt"),
    intervalSeconds: requiredNumber(body, "intervalSeconds"),
    providerAccountRef: requiredString(body, "providerAccountRef"),
    userCode: requiredString(body, "userCode"),
    verificationUrl: requiredString(body, "verificationUrl"),
    pylonLink: linkedPylonMetadataFrom(body),
  }
}

export function linkedOpenAgentsProviderAccountRef(
  result: PylonAccountConnectProjection["openAgentsDeviceLogin"],
): string | null {
  if (
    "pylonLink" in result &&
    result.pylonLink.owner === "openauth" &&
    result.pylonLink.status === "linked" &&
    "providerAccountRef" in result &&
    typeof result.providerAccountRef === "string" &&
    result.providerAccountRef.trim() !== ""
  ) {
    return result.providerAccountRef
  }
  return null
}

export async function runPylonAccountsConnect(
  summary: Pick<BootstrapSummary, "bootstrap" | "paths">,
  args: PylonAccountsConnectArgs,
  options: {
    env?: Record<string, string | undefined>
    fetcher?: PylonAccountsConnectFetcher
    runCodexDeviceLogin?: PylonCodexDeviceLoginRunner
    runGrokDeviceLogin?: PylonGrokDeviceLoginRunner
    codexAuthValidityProbe?: PylonCodexAuthValidityProbe
    grokReadinessProbe?: PylonGrokReadinessProbe
  } = {},
): Promise<PylonAccountConnectProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  if (
    args.provider === "grok" &&
    (args.home !== null ||
      args.setupToken !== null ||
      args.openAgentsLink ||
      args.openAgentsAttemptId !== null ||
      args.providerAccountRef !== null)
  ) {
    throw new Error(
      "Grok connect accepts only an isolated named device-login account",
    )
  }
  const home = normalizeAccountHome(
    args.provider === "grok"
      ? defaultAccountHome(summary, "grok", args.accountRef)
      : args.home ?? defaultAccountHome(summary, args.provider, args.accountRef),
  )
  await mkdir(home, { recursive: true })

  let deviceLoginStatus: PylonAccountConnectProjection["deviceLogin"]["status"] = "skipped_by_flag"
  let deviceLoginReason: string | undefined
  const blockerRefs: string[] = []
  if (args.provider === "codex") {
    await forceCodexFileCredentialStore(home)
    const authPath = join(home, "auth.json")
    const authAlreadyPresent = existsSync(authPath)
    if (args.skipDeviceLogin) {
      deviceLoginStatus = "skipped_by_flag"
    } else if (authAlreadyPresent && !args.forceDeviceLogin) {
      // A present `auth.json` is NOT proof the credential still works: a revoked
      // refresh token leaves a file that silently fails on first real use. When
      // a validity probe is supplied, verify the stored credential before
      // reusing it; recover automatically if it is dead. With no probe injected
      // we preserve the legacy reuse behavior for backward compatibility.
      const validity = await options.codexAuthValidityProbe?.({ env, home })
      if (validity === undefined || validity.valid || validity.reason === "usage_limited") {
        // Valid, or a usage cap (not an auth failure): reuse the existing auth
        // and surface the probe reason instead of forcing a disruptive re-login.
        deviceLoginStatus = "skipped_existing_auth"
        if (validity !== undefined && validity.reason !== undefined) {
          deviceLoginReason = validity.reason
        }
      } else {
        // credentials_revoked | auth_error: auto-run device-login to recover
        // into the SAME isolated account home (never `~/.codex`).
        deviceLoginReason = validity.reason
        const result = await (options.runCodexDeviceLogin ?? defaultCodexDeviceLoginRunner)({
          env,
          home,
        })
        if (result.exitCode === 0 && existsSync(authPath)) {
          deviceLoginStatus = "completed_recovered_invalid_auth"
        } else {
          // Could not recover (e.g. non-interactive / device-login unavailable).
          // Report the blocker honestly; never claim success for dead creds.
          deviceLoginStatus = "blocked_invalid_auth"
          blockerRefs.push("blocker.pylon.accounts_connect.codex_credentials_invalid_unrecovered")
        }
      }
    } else {
      const result = await (options.runCodexDeviceLogin ?? defaultCodexDeviceLoginRunner)({
        env,
        home,
      })
      if (result.exitCode !== 0) {
        throw new Error(`codex login --device-auth exited with status ${result.exitCode}`)
      }
      if (!existsSync(authPath)) {
        throw new Error("codex login --device-auth completed but auth.json was not written in the account home")
      }
      deviceLoginStatus = "completed"
    }
  } else if (args.provider === "grok") {
    const grokEnv = isolateGrokCliEnvironment(env, home)
    const inspect = () =>
      (options.grokReadinessProbe ?? defaultGrokReadinessProbe)({
        env: grokEnv,
        home,
        timeoutMs: grokReadinessTimeoutMs(env),
      })
    if (args.skipDeviceLogin) {
      deviceLoginStatus = "skipped_by_flag"
    } else {
      const existing = args.forceDeviceLogin ? null : await inspect()
      if (existing?.ready === true && existing.plane === "cli_session") {
        deviceLoginStatus = "skipped_existing_auth"
        deviceLoginReason = "existing_grok_cli_session"
      } else {
        const result = await (
          options.runGrokDeviceLogin ?? defaultGrokDeviceLoginRunner
        )({ env: grokEnv, home })
        if (result.exitCode !== 0) {
          throw new Error(
            `grok login --device-auth exited with status ${result.exitCode}`,
          )
        }
        const readiness = await inspect()
        if (!readiness.ready || readiness.plane !== "cli_session") {
          throw new Error(
            "grok login --device-auth completed but isolated account readiness was not confirmed",
          )
        }
        deviceLoginStatus = "completed"
      }
    }
  } else if (args.provider === "claude_agent") {
    // Claude custody is paste/setup-token file storage (not device-login). Token
    // material may come from --token/--setup-token, CLAUDE_CODE_OAUTH_TOKEN, or
    // an already-present claude-oauth-token file in the isolated home.
    const providedToken = resolveClaudeSetupToken(args, env)
    const hasExisting = await pylonClaudeAccountHomeHasAuth(home)
    if (providedToken !== null) {
      await writeClaudeOauthTokenFile(home, providedToken)
      deviceLoginStatus = hasExisting ? "completed" : "completed"
      deviceLoginReason = "setup_token"
    } else if (hasExisting) {
      deviceLoginStatus = "skipped_existing_auth"
      deviceLoginReason = "existing_claude_oauth_token"
    } else {
      throw new Error(
        "Claude connect requires --token / --setup-token, CLAUDE_CODE_OAUTH_TOKEN, or an existing claude-oauth-token file in the account home",
      )
    }
  }

  const config = await readConfig(summary.paths.config)
  let registryStatus = registryStatusFor(config, {
    provider: args.provider,
    accountRef: args.accountRef,
    home,
  })
  if (registryStatus !== "unchanged") {
    await writeConfig(summary.paths.config, config)
  }
  const openAgentsDeviceLogin = await runOpenAgentsPylonDeviceLogin({
    args,
    env,
    fetcher: options.fetcher ?? fetch,
  })
  if (
    openAgentsDeviceLogin.status === "polled" &&
    openAgentsDeviceLogin.attemptStatus === "connected" &&
    openAgentsDeviceLogin.accountStatus === "connected"
  ) {
    const linkedRegistryStatus = registryStatusFor(config, {
      provider: args.provider,
      accountRef: args.accountRef,
      home,
      openAgentsProviderAccountRef: openAgentsDeviceLogin.providerAccountRef,
    })
    if (linkedRegistryStatus !== "unchanged") {
      await writeConfig(summary.paths.config, config)
      if (registryStatus !== "created") registryStatus = "updated"
    }
  }

  const projection = {
    schema: "pylon.accounts.connect.v1",
    provider: args.provider,
    accountRef: args.accountRef,
    accountRefHash: hashPylonAccountRef(args.provider, args.accountRef),
    homeRef: stableHomeRef(args.provider, home),
    homeState: "present",
    codexCredentialStore: args.provider === "codex" ? "file" : "not_applicable",
    registry: { status: registryStatus },
    deviceLogin: {
      status: deviceLoginStatus,
      ...(deviceLoginReason !== undefined ? { reason: deviceLoginReason } : {}),
    },
    openAgentsDeviceLogin,
    blockerRefs,
  } satisfies PylonAccountConnectProjection
  assertPublicProjectionSafe(projection)
  if (
    args.provider === "codex" &&
    (deviceLoginStatus === "completed" ||
      deviceLoginStatus === "completed_recovered_invalid_auth" ||
      (deviceLoginStatus === "skipped_existing_auth" && deviceLoginReason === undefined))
  ) {
    await clearCodexAccountHealthFailure(
      summary,
      hashPylonAccountRef(args.provider, args.accountRef),
    )
  }
  const linkedProviderAccountRef = linkedOpenAgentsProviderAccountRef(openAgentsDeviceLogin)
  if (linkedProviderAccountRef !== null) {
    await recordAccountLinkInPresence(summary, {
      providerAccountRef: linkedProviderAccountRef,
    })
  }
  return projection
}
