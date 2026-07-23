import { Runtime } from "@openagentsinc/runtime-platform"
import { createHash } from "node:crypto"
import { mkdir, readFile, rename, writeFile, chmod } from "node:fs/promises"
import { dirname, join } from "node:path"

import {
  type PylonAccountsConnectArgs,
  type PylonAccountsConnectFetcher,
  type PylonCodexAuthValidityProbe,
  type PylonCodexDeviceLoginRunner,
  defaultCodexAuthValidityProbe,
  runPylonAccountsConnect,
} from "./account-connect.js"
import { type BootstrapSummary } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

export type PylonAuthTarget = "openagents" | "codex" | "claude"

export type PylonAuthArgs = {
  accountRef: string | null
  agentToken: string | null
  baseUrl: string | null
  forceDeviceLogin: boolean
  json: boolean
  /**
   * OPT-IN (owner directive, EP250): when true, `auth codex` additionally
   * links the OpenAgents Pylon (device link) and imports the provider
   * account to the OpenAgents API. Default false — the flow is LOCAL-ONLY:
   * isolated device login + local config registration, zero network calls to
   * openagents.com.
   */
  openAgentsLink: boolean
  /**
   * Claude setup-token (CLAUDE_CODE_OAUTH_TOKEN material). Only for target
   * `claude`; never projected.
   */
  setupToken: string | null
  target: PylonAuthTarget
  timeoutSeconds: number
}

type AgentTokenSource = "cli" | "env" | "stored" | "registered"

export type ResolvedOpenAgentsAgentToken = {
  source: "cli" | "env" | "stored"
  token: string
}

export type PylonAuthOpenAgentsProjection = {
  schema: "pylon.auth.openagents.v1"
  status: "linked"
  agentCredential: {
    source: AgentTokenSource
    tokenPrefix: string
  }
  deviceLogin:
    | { status: "already_linked" }
    | {
        status: "completed"
        attemptId: string
      }
  blockerRefs: string[]
}

export type PylonAuthCodexProjection = {
  schema: "pylon.auth.codex.v1"
  /**
   * `connected` — the local device login completed and the account is
   * registered in the local Pylon config. In the LOCAL-ONLY default mode
   * (owner directive, EP250) that is the whole flow: no OpenAgents network
   * calls happen and `openAgents` is absent.
   *
   * `connected_local_only` (EP250 regression) — the opt-in
   * `--openagents-link` flow completed the local connect, but a post-auth
   * server step (the OpenAgents provider-account import POST, or the config
   * providerAccountRef write) failed. Local credentials are valid and usable
   * for local fleet work; only the server-side provider-account link is
   * pending. A flow that wrote valid credentials and registered the account
   * must NEVER be reported as a bare failure.
   */
  status: "connected" | "connected_local_only" | "credentials_invalid"
  accountRef: string
  /** Present only when the opt-in `--openagents-link` flow ran. */
  openAgents?: PylonAuthOpenAgentsProjection
  localCodex: {
    deviceLoginStatus:
      | "completed"
      | "skipped_existing_auth"
      | "skipped_by_flag"
      | "completed_recovered_invalid_auth"
      | "blocked_invalid_auth"
    reason?: string
  }
  openAgentsProviderAccount: {
    accountStatus: string
    attemptId: string
    attemptStatus: string
    providerAccountRef: string
    source: "pylon_local_codex_auth"
  }
  blockerRefs: string[]
}

/**
 * Claude auth projection. The default stays local-only. The explicit
 * `--openagents-link` path imports the isolated setup-token through the typed
 * provider-account route without projecting the token.
 */
export type PylonAuthClaudeProjection = {
  schema: "pylon.auth.claude.v1"
  status: "connected" | "connected_local_only"
  accountRef: string
  provider: "claude_agent"
  openAgents?: PylonAuthOpenAgentsProjection
  localClaude: {
    setupTokenStatus: "completed" | "skipped_existing_auth"
    reason?: string
  }
  openAgentsProviderAccount: {
    accountStatus: string
    attemptId: string
    attemptStatus: string
    providerAccountRef: string
    source: "pylon_local_claude_auth"
  }
  blockerRefs: string[]
}

type DevicePromptKind = "openagents" | "codex"

type PylonAuthOptions = {
  env?: Record<string, string | undefined>
  fetcher?: PylonAccountsConnectFetcher
  onDevicePrompt?: (prompt: {
    kind: DevicePromptKind
    userCode: string
    verificationUrl: string
  }) => void
  runCodexDeviceLogin?: PylonCodexDeviceLoginRunner
  codexAuthValidityProbe?: PylonCodexAuthValidityProbe
  sleep?: (ms: number) => Promise<void>
}

type OpenAgentsAuthStartResponse =
  | {
      schema: "openagents.pylon.auth.openagents.v1"
      status: "linked"
      linkedAgent: { tokenPrefix: string }
    }
  | {
      schema: "openagents.pylon.auth.openagents.v1"
      status: "pending"
      attemptId: string
      expiresAt: string
      intervalSeconds: number
      linkedAgent: { tokenPrefix: string }
      userCode: string
      verificationUrl: string
    }

type OpenAgentsAuthStatusResponse =
  | {
      schema: "openagents.pylon.auth.openagents.v1"
      status: "linked"
      linkedAgent: { tokenPrefix: string }
    }
  | {
      schema: "openagents.pylon.auth.openagents.v1"
      status: "pending"
      attemptId: string
      expiresAt: string
      intervalSeconds: number
      linkedAgent: { tokenPrefix: string }
    }
  | {
      schema: "openagents.pylon.auth.openagents.v1"
      status: "expired"
      attemptId: string
    }

const accountRefPattern = /^[A-Za-z0-9][A-Za-z0-9._-]{0,79}$/
const defaultSleep = (ms: number): Promise<void> =>
  new Promise(resolve => setTimeout(resolve, ms))
const withoutTrailingSlash = (value: string): string => value.replace(/\/+$/, "")

function readRequiredValue(args: string[], index: number, option: string): string {
  const value = args[index + 1]
  if (value === undefined || value.startsWith("--") || value.trim() === "") {
    throw new Error(`${option} requires a value`)
  }
  return value
}

export function parsePylonAuthArgs(args: string[]): PylonAuthArgs {
  const target = args[0]
  if (target !== "openagents" && target !== "codex" && target !== "claude") {
    throw new Error("usage: pylon auth openagents|codex|claude [--account <ref>] [--token <setup-token>] [--json]")
  }

  const parsed: PylonAuthArgs = {
    accountRef: null,
    agentToken: null,
    baseUrl: null,
    forceDeviceLogin: false,
    json: false,
    openAgentsLink: false,
    setupToken: null,
    target,
    timeoutSeconds: 10 * 60,
  }

  for (let index = 1; index < args.length; index += 1) {
    const arg = args[index]
    if (arg === "--account" || arg === "--account-ref") {
      parsed.accountRef = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--agent-token") {
      parsed.agentToken = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--base-url") {
      parsed.baseUrl = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--token" || arg === "--setup-token") {
      parsed.setupToken = readRequiredValue(args, index, arg).trim()
      index += 1
    } else if (arg === "--force-device-login") {
      parsed.forceDeviceLogin = true
    } else if (arg === "--openagents-link") {
      parsed.openAgentsLink = true
    } else if (arg === "--json") {
      parsed.json = true
    } else if (arg === "--timeout-seconds") {
      const raw = readRequiredValue(args, index, arg).trim()
      const timeoutSeconds = Number.parseInt(raw, 10)
      if (!Number.isSafeInteger(timeoutSeconds) || timeoutSeconds <= 0) {
        throw new Error("--timeout-seconds must be a positive integer")
      }
      parsed.timeoutSeconds = Math.min(timeoutSeconds, 60 * 60)
      index += 1
    } else {
      throw new Error(`Unknown auth option: ${arg}`)
    }
  }

  if (parsed.accountRef !== null && !accountRefPattern.test(parsed.accountRef)) {
    throw new Error("pylon auth --account must use letters, numbers, dot, dash, or underscore")
  }
  if (parsed.target === "openagents" && parsed.accountRef !== null) {
    throw new Error("pylon auth openagents does not take --account")
  }
  if (parsed.target === "claude") {
    if (parsed.forceDeviceLogin) {
      throw new Error(
        "Claude auth does not use device-login; provide --token / --setup-token or CLAUDE_CODE_OAUTH_TOKEN (or an existing claude-oauth-token file)",
      )
    }
  } else if (parsed.setupToken !== null) {
    throw new Error("--token / --setup-token is only valid for pylon auth claude")
  }
  if (parsed.openAgentsLink && parsed.target === "openagents") {
    throw new Error("--openagents-link is only valid for pylon auth codex or claude")
  }

  return parsed
}

const baseUrlFrom = (
  args: Pick<PylonAuthArgs, "baseUrl">,
  env: Record<string, string | undefined>,
): string =>
  withoutTrailingSlash(
    [
      args.baseUrl,
      env.PYLON_OPENAGENTS_BASE_URL,
      env.OPENAGENTS_BASE_URL,
      "https://openagents.com",
    ].find((value): value is string => typeof value === "string" && value.trim() !== "") ??
      "https://openagents.com",
  )

const agentTokenPath = (summary: Pick<BootstrapSummary, "paths">): string =>
  join(summary.paths.home, "auth", "openagents-agent-token")

function requireAgentTokenShape(token: string, source: string): string {
  if (!token.startsWith("oa_agent_")) {
    throw new Error(`${source} must be an OpenAgents agent token`)
  }
  return token
}

export async function readStoredOpenAgentsAgentToken(
  summary: Pick<BootstrapSummary, "paths">,
): Promise<string | null> {
  try {
    const token = (await readFile(agentTokenPath(summary), "utf8")).trim()
    return token.startsWith("oa_agent_") ? token : null
  } catch {
    return null
  }
}

export async function resolveOpenAgentsAgentToken(input: {
  env?: Record<string, string | undefined>
  explicitAgentToken?: string | null
  summary: Pick<BootstrapSummary, "paths">
}): Promise<ResolvedOpenAgentsAgentToken | null> {
  const explicit = (input.explicitAgentToken ?? "").trim()
  if (explicit !== "") {
    return {
      source: "cli",
      token: requireAgentTokenShape(explicit, "--agent-token"),
    }
  }

  const stored = await readStoredOpenAgentsAgentToken(input.summary)
  if (stored !== null) {
    return { source: "stored", token: stored }
  }

  const envToken = (input.env?.OPENAGENTS_AGENT_TOKEN ?? "").trim()
  if (envToken !== "") {
    return {
      source: "env",
      token: requireAgentTokenShape(envToken, "OPENAGENTS_AGENT_TOKEN"),
    }
  }

  return null
}

async function writeStoredAgentToken(summary: Pick<BootstrapSummary, "paths">, token: string): Promise<void> {
  const path = agentTokenPath(summary)
  await mkdir(dirname(path), { recursive: true })
  const tempPath = `${path}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tempPath, `${token}\n`, { mode: 0o600 })
  await chmod(tempPath, 0o600).catch(() => undefined)
  await rename(tempPath, path)
  await chmod(path, 0o600).catch(() => undefined)
}

const hashRef = (value: string): string =>
  createHash("sha256").update(value).digest("hex").slice(0, 16)

class OpenAgentsAuthHttpError extends Error {
  constructor(readonly status: number, message: string) {
    super(message)
    this.name = "OpenAgentsAuthHttpError"
  }
}

const isUnauthorizedAuthError = (error: unknown): boolean =>
  error instanceof OpenAgentsAuthHttpError && error.status === 401

async function readJsonResponse(response: Response): Promise<Record<string, unknown>> {
  const body = (await response.json().catch(() => ({}))) as unknown
  const record = body !== null && typeof body === "object" && !Array.isArray(body)
    ? (body as Record<string, unknown>)
    : {}
  if (!response.ok) {
    const message = typeof record.message === "string"
      ? record.message
      : typeof record.error === "string"
        ? record.error
        : `OpenAgents request failed with status ${response.status}`
    throw new OpenAgentsAuthHttpError(response.status, message)
  }
  return record
}

function stringAt(record: Record<string, unknown>, key: string): string {
  const value = record[key]
  if (typeof value !== "string" || value.trim() === "") {
    throw new Error(`OpenAgents auth response missing ${key}`)
  }
  return value
}

function numberAt(record: Record<string, unknown>, key: string): number {
  const value = record[key]
  if (typeof value !== "number" || !Number.isFinite(value)) {
    throw new Error(`OpenAgents auth response missing ${key}`)
  }
  return value
}

async function registerAgentToken(input: {
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
}): Promise<string> {
  const response = await input.fetcher(`${input.baseUrl}/api/agents/register`, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      displayName: "Pylon CLI",
      metadata: {
        source: "pylon.auth.openagents.v1",
      },
    }),
  })
  const body = await readJsonResponse(response)
  const credential = body.credential !== null &&
    typeof body.credential === "object" &&
    !Array.isArray(body.credential)
    ? (body.credential as Record<string, unknown>)
    : {}
  return requireAgentTokenShape(stringAt(credential, "token"), "OpenAgents registration response")
}

async function ensureAgentToken(input: {
  args: PylonAuthArgs
  baseUrl: string
  env: Record<string, string | undefined>
  fetcher: PylonAccountsConnectFetcher
  summary: Pick<BootstrapSummary, "paths">
}): Promise<{ source: AgentTokenSource; token: string }> {
  const explicit = (input.args.agentToken ?? "").trim()
  if (explicit !== "") {
    const token = requireAgentTokenShape(explicit, "--agent-token")
    await writeStoredAgentToken(input.summary, token)
    return { source: "cli", token }
  }
  const stored = await readStoredOpenAgentsAgentToken(input.summary)
  if (stored !== null) {
    return { source: "stored", token: stored }
  }
  const envToken = (input.env.OPENAGENTS_AGENT_TOKEN ?? "").trim()
  if (envToken !== "") {
    const token = requireAgentTokenShape(envToken, "OPENAGENTS_AGENT_TOKEN")
    await writeStoredAgentToken(input.summary, token)
    return { source: "env", token }
  }
  const token = await registerAgentToken({
    baseUrl: input.baseUrl,
    fetcher: input.fetcher,
  })
  await writeStoredAgentToken(input.summary, token)
  return { source: "registered", token }
}

async function registerFallbackAgentToken(input: {
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
  summary: Pick<BootstrapSummary, "paths">
}): Promise<{ source: "registered"; token: string }> {
  const token = await registerAgentToken({
    baseUrl: input.baseUrl,
    fetcher: input.fetcher,
  })
  await writeStoredAgentToken(input.summary, token)
  return { source: "registered", token }
}

function parseOpenAgentsAuthStart(body: Record<string, unknown>): OpenAgentsAuthStartResponse {
  const linkedAgent = body.linkedAgent !== null &&
    typeof body.linkedAgent === "object" &&
    !Array.isArray(body.linkedAgent)
    ? (body.linkedAgent as Record<string, unknown>)
    : {}
  const status = stringAt(body, "status")
  if (status === "linked") {
    return {
      schema: "openagents.pylon.auth.openagents.v1",
      status,
      linkedAgent: { tokenPrefix: stringAt(linkedAgent, "tokenPrefix") },
    }
  }
  if (status !== "pending") {
    throw new Error(`Unexpected OpenAgents auth status: ${status}`)
  }
  return {
    schema: "openagents.pylon.auth.openagents.v1",
    status,
    attemptId: stringAt(body, "attemptId"),
    expiresAt: stringAt(body, "expiresAt"),
    intervalSeconds: numberAt(body, "intervalSeconds"),
    linkedAgent: { tokenPrefix: stringAt(linkedAgent, "tokenPrefix") },
    userCode: stringAt(body, "userCode"),
    verificationUrl: stringAt(body, "verificationUrl"),
  }
}

function parseOpenAgentsAuthStatus(body: Record<string, unknown>): OpenAgentsAuthStatusResponse {
  const status = stringAt(body, "status")
  if (status === "expired") {
    return {
      schema: "openagents.pylon.auth.openagents.v1",
      status,
      attemptId: stringAt(body, "attemptId"),
    }
  }
  const linkedAgent = body.linkedAgent !== null &&
    typeof body.linkedAgent === "object" &&
    !Array.isArray(body.linkedAgent)
    ? (body.linkedAgent as Record<string, unknown>)
    : {}
  if (status === "linked") {
    return {
      schema: "openagents.pylon.auth.openagents.v1",
      status,
      linkedAgent: { tokenPrefix: stringAt(linkedAgent, "tokenPrefix") },
    }
  }
  if (status !== "pending") {
    throw new Error(`Unexpected OpenAgents auth status: ${status}`)
  }
  return {
    schema: "openagents.pylon.auth.openagents.v1",
    status,
    attemptId: stringAt(body, "attemptId"),
    expiresAt: stringAt(body, "expiresAt"),
    intervalSeconds: numberAt(body, "intervalSeconds"),
    linkedAgent: { tokenPrefix: stringAt(linkedAgent, "tokenPrefix") },
  }
}

async function startOpenAgentsAuth(input: {
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
  token: string
}): Promise<OpenAgentsAuthStartResponse> {
  const response = await input.fetcher(`${input.baseUrl}/api/pylon/auth/openagents/device/start`, {
    method: "POST",
    headers: {
      authorization: `Bearer ${input.token}`,
    },
  })
  return parseOpenAgentsAuthStart(await readJsonResponse(response))
}

async function pollOpenAgentsAuth(input: {
  attemptId: string
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
  token: string
}): Promise<OpenAgentsAuthStatusResponse> {
  const response = await input.fetcher(
    `${input.baseUrl}/api/pylon/auth/openagents/device/${encodeURIComponent(input.attemptId)}`,
    {
      method: "GET",
      headers: {
        authorization: `Bearer ${input.token}`,
      },
    },
  )
  return parseOpenAgentsAuthStatus(await readJsonResponse(response))
}

export async function runPylonAuthOpenAgents(
  summary: Pick<BootstrapSummary, "paths">,
  args: PylonAuthArgs,
  options: PylonAuthOptions = {},
): Promise<{ agentToken: string; projection: PylonAuthOpenAgentsProjection }> {
  const env = options.env ?? (Runtime.env as Record<string, string | undefined>)
  const fetcher = options.fetcher ?? fetch
  const sleep = options.sleep ?? defaultSleep
  const baseUrl = baseUrlFrom(args, env)
  const tokenCandidates: Array<{ source: AgentTokenSource; token: string }> = []
  const primary = await ensureAgentToken({ args, baseUrl, env, fetcher, summary })
  tokenCandidates.push(primary)
  if (args.agentToken === null) {
    const stored = await readStoredOpenAgentsAgentToken(summary)
    if (
      stored !== null &&
      !tokenCandidates.some(candidate => candidate.token === stored)
    ) {
      tokenCandidates.push({ source: "stored", token: stored })
    }
    const envToken = (env.OPENAGENTS_AGENT_TOKEN ?? "").trim()
    if (
      envToken !== "" &&
      !tokenCandidates.some(candidate => candidate.token === envToken)
    ) {
      tokenCandidates.push({
        source: "env",
        token: requireAgentTokenShape(envToken, "OPENAGENTS_AGENT_TOKEN"),
      })
    }
  }

  let token = tokenCandidates[0]!
  let started: OpenAgentsAuthStartResponse | null = null
  let lastUnauthorized: unknown
  for (const candidate of tokenCandidates) {
    try {
      started = await startOpenAgentsAuth({
        baseUrl,
        fetcher,
        token: candidate.token,
      })
      token = candidate
      await writeStoredAgentToken(summary, candidate.token)
      break
    } catch (error) {
      if (!isUnauthorizedAuthError(error) || candidate.source === "cli") {
        throw error
      }
      lastUnauthorized = error
    }
  }
  if (started === null) {
    token = await registerFallbackAgentToken({ baseUrl, fetcher, summary })
    try {
      started = await startOpenAgentsAuth({
        baseUrl,
        fetcher,
        token: token.token,
      })
    } catch (error) {
      throw isUnauthorizedAuthError(error) && lastUnauthorized !== undefined
        ? lastUnauthorized
        : error
    }
  }

  if (started.status === "linked") {
    const projection = {
      schema: "pylon.auth.openagents.v1",
      status: "linked",
      agentCredential: {
        source: token.source,
        tokenPrefix: started.linkedAgent.tokenPrefix,
      },
      deviceLogin: { status: "already_linked" },
      blockerRefs: [],
    } satisfies PylonAuthOpenAgentsProjection
    assertPublicProjectionSafe(projection)
    return { agentToken: token.token, projection }
  }

  options.onDevicePrompt?.({
    kind: "openagents",
    userCode: started.userCode,
    verificationUrl: started.verificationUrl,
  })

  const deadline = Date.now() + args.timeoutSeconds * 1000
  let last: OpenAgentsAuthStartResponse | OpenAgentsAuthStatusResponse = started
  while (Date.now() <= deadline) {
    await sleep(Math.max(1, started.intervalSeconds) * 1000)
    const polled = await pollOpenAgentsAuth({
      attemptId: started.attemptId,
      baseUrl,
      fetcher,
      token: token.token,
    })
    last = polled
    if (polled.status === "linked") {
      const projection = {
        schema: "pylon.auth.openagents.v1",
        status: "linked",
        agentCredential: {
          source: token.source,
          tokenPrefix: polled.linkedAgent.tokenPrefix,
        },
        deviceLogin: {
          status: "completed",
          attemptId: started.attemptId,
        },
        blockerRefs: [],
      } satisfies PylonAuthOpenAgentsProjection
      assertPublicProjectionSafe(projection)
      return { agentToken: token.token, projection }
    }
    if (polled.status === "expired") {
      throw new Error("OpenAgents auth link expired")
    }
  }

  throw new Error(
    `OpenAgents auth timed out while ${last.status === "pending" ? "waiting for browser confirmation" : `status was ${last.status}`}`,
  )
}

async function readConfig(summary: Pick<BootstrapSummary, "paths">): Promise<Record<string, unknown>> {
  try {
    const value = JSON.parse(await readFile(summary.paths.config, "utf8"))
    return value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)
      : {}
  } catch {
    return {}
  }
}

async function writeConfig(
  summary: Pick<BootstrapSummary, "paths">,
  config: Record<string, unknown>,
): Promise<void> {
  await mkdir(dirname(summary.paths.config), { recursive: true })
  const tempPath = `${summary.paths.config}.tmp-${process.pid}-${Date.now()}`
  await writeFile(tempPath, `${JSON.stringify(config, null, 2)}\n`)
  await rename(tempPath, summary.paths.config)
}

async function nextProviderAccountRef(
  summary: Pick<BootstrapSummary, "paths">,
  provider: "codex" | "claude_agent",
  baseRef: string,
): Promise<string> {
  const config = await readConfig(summary)
  const dev = config.dev !== null && typeof config.dev === "object" && !Array.isArray(config.dev)
    ? (config.dev as Record<string, unknown>)
    : {}
  const accounts = Array.isArray(dev.accounts) ? dev.accounts : []
  const existing = new Set(
    accounts.flatMap(account => {
      if (account === null || typeof account !== "object" || Array.isArray(account)) {
        return []
      }
      const record = account as Record<string, unknown>
      return record.provider === provider && typeof record.ref === "string"
        ? [record.ref]
        : []
    }),
  )

  if (!existing.has(baseRef)) {
    return baseRef
  }
  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `${baseRef}-${index}`
    if (!existing.has(candidate)) {
      return candidate
    }
  }
  return `${baseRef}-${hashRef(String(Date.now()))}`
}

async function nextCodexAccountRef(summary: Pick<BootstrapSummary, "paths">): Promise<string> {
  return nextProviderAccountRef(summary, "codex", "codex")
}

async function nextClaudeAccountRef(summary: Pick<BootstrapSummary, "paths">): Promise<string> {
  return nextProviderAccountRef(summary, "claude_agent", "claude")
}

function codexConnectArgs(input: {
  accountRef: string
  forceDeviceLogin: boolean
  skipDeviceLogin: boolean
}): PylonAccountsConnectArgs {
  return {
    provider: "codex",
    accountRef: input.accountRef,
    accountLabel: input.accountRef,
    agentToken: null,
    baseUrl: null,
    createNewOpenAgentsAccount: true,
    home: null,
    forceDeviceLogin: input.forceDeviceLogin,
    json: true,
    openAgentsAttemptId: null,
    openAgentsLink: false,
    providerAccountRef: null,
    setupToken: null,
    skipDeviceLogin: input.skipDeviceLogin,
  }
}

function claudeConnectArgs(input: {
  accountRef: string
  home: string | null
  setupToken: string | null
}): PylonAccountsConnectArgs {
  return {
    provider: "claude_agent",
    accountRef: input.accountRef,
    accountLabel: input.accountRef,
    agentToken: null,
    baseUrl: null,
    createNewOpenAgentsAccount: true,
    home: input.home,
    forceDeviceLogin: false,
    json: true,
    openAgentsAttemptId: null,
    openAgentsLink: false,
    providerAccountRef: null,
    setupToken: input.setupToken,
    skipDeviceLogin: false,
  }
}

const defaultCodexAccountHome = (
  summary: Pick<BootstrapSummary, "paths">,
  accountRef: string,
): string => join(summary.paths.home, "accounts", "codex", accountRef)

const localCodexAuthPath = (
  summary: Pick<BootstrapSummary, "paths">,
  accountRef: string,
): string => join(defaultCodexAccountHome(summary, accountRef), "auth.json")

function recordAt(value: unknown, key: string): Record<string, unknown> {
  const child =
    value !== null && typeof value === "object" && !Array.isArray(value)
      ? (value as Record<string, unknown>)[key]
      : undefined
  return child !== null && typeof child === "object" && !Array.isArray(child)
    ? (child as Record<string, unknown>)
    : {}
}

function optionalStringAt(record: Record<string, unknown>, key: string): string | undefined {
  const value = record[key]
  return typeof value === "string" && value.trim() !== "" ? value.trim() : undefined
}

async function readLocalCodexOAuthAuth(input: {
  accountRef: string
  summary: Pick<BootstrapSummary, "paths">
}): Promise<{
  access: string
  accountId?: string
  expires: number
  idToken?: string
  refresh: string
  type: "oauth"
}> {
  const raw = await readFile(localCodexAuthPath(input.summary, input.accountRef), "utf8")
  const parsed = JSON.parse(raw) as unknown
  const tokens = recordAt(parsed, "tokens")
  const access = optionalStringAt(tokens, "access_token")
  const refresh = optionalStringAt(tokens, "refresh_token")
  if (access === undefined || refresh === undefined) {
    throw new Error("local Codex auth is missing OAuth tokens")
  }
  const top = parsed !== null && typeof parsed === "object" && !Array.isArray(parsed)
    ? (parsed as Record<string, unknown>)
    : {}
  const lastRefresh = optionalStringAt(top, "last_refresh")
  const refreshedAt = lastRefresh === undefined ? Number.NaN : Date.parse(lastRefresh)
  const expires = Number.isFinite(refreshedAt) ? refreshedAt + 1000 * 60 * 60 : Date.now() + 1000 * 60 * 60
  const accountId = optionalStringAt(tokens, "account_id")
  const idToken = optionalStringAt(tokens, "id_token")

  return {
    type: "oauth",
    access,
    refresh,
    expires,
    ...(accountId === undefined ? {} : { accountId }),
    ...(idToken === undefined ? {} : { idToken }),
  }
}

function configuredProviderAccountRef(
  config: Record<string, unknown>,
  accountRef: string,
  provider: "codex" | "claude_agent" = "codex",
): string | null {
  const dev = recordAt(config, "dev")
  const accounts = Array.isArray(dev.accounts) ? dev.accounts : []
  for (const account of accounts) {
    if (account === null || typeof account !== "object" || Array.isArray(account)) {
      continue
    }
    const record = account as Record<string, unknown>
    if (record.provider === provider && record.ref === accountRef) {
      return optionalStringAt(record, "openAgentsProviderAccountRef") ?? null
    }
  }
  return null
}

function configuredAccountHome(
  config: Record<string, unknown>,
  accountRef: string,
  provider: "codex" | "claude_agent",
): string | null {
  const dev = recordAt(config, "dev")
  const accounts = Array.isArray(dev.accounts) ? dev.accounts : []
  for (const account of accounts) {
    if (account === null || typeof account !== "object" || Array.isArray(account)) {
      continue
    }
    const record = account as Record<string, unknown>
    if (record.provider === provider && record.ref === accountRef) {
      return optionalStringAt(record, "home") ?? null
    }
  }
  return null
}

async function writeConfiguredProviderAccountRef(input: {
  accountRef: string
  provider?: "codex" | "claude_agent"
  providerAccountRef: string
  summary: Pick<BootstrapSummary, "paths">
}): Promise<void> {
  const config = await readConfig(input.summary)
  const dev = recordAt(config, "dev")
  const accounts = Array.isArray(dev.accounts) ? [...dev.accounts] : []
  const index = accounts.findIndex(account => {
    if (account === null || typeof account !== "object" || Array.isArray(account)) {
      return false
    }
    const record = account as Record<string, unknown>
    return record.provider === (input.provider ?? "codex") && record.ref === input.accountRef
  })
  if (index === -1) {
    return
  }
  const existing = accounts[index]
  accounts[index] = {
    ...(existing !== null && typeof existing === "object" && !Array.isArray(existing) ? existing : {}),
    openAgentsProviderAccountRef: input.providerAccountRef,
  }
  config.dev = { ...dev, accounts }
  await writeConfig(input.summary, config)
}

type OpenAgentsLocalCodexAuthImportResponse = {
  account: {
    providerAccountRef: string
    status: string
  }
  attempt: {
    id: string
    status: string
  }
  pylonLink: { owner: "openauth"; status: "linked" }
}

type OpenAgentsLocalClaudeAuthImportResponse = OpenAgentsLocalCodexAuthImportResponse

function parseLocalCodexAuthImportResponse(
  body: Record<string, unknown>,
): OpenAgentsLocalCodexAuthImportResponse {
  const account = recordAt(body, "account")
  const attempt = recordAt(body, "attempt")
  const pylonLink = recordAt(body, "pylonLink")
  if (pylonLink.owner !== "openauth" || pylonLink.status !== "linked") {
    throw new Error("OpenAgents local Codex auth import did not confirm a linked Pylon owner")
  }
  return {
    account: {
      providerAccountRef: stringAt(account, "providerAccountRef"),
      status: stringAt(account, "status"),
    },
    attempt: {
      id: stringAt(attempt, "id"),
      status: stringAt(attempt, "status"),
    },
    pylonLink: { owner: "openauth", status: "linked" },
  }
}

async function importLocalCodexAuth(input: {
  accountRef: string
  agentToken: string
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
  providerAccountRef: string | null
  summary: Pick<BootstrapSummary, "paths">
}): Promise<OpenAgentsLocalCodexAuthImportResponse> {
  const auth = await readLocalCodexOAuthAuth({
    accountRef: input.accountRef,
    summary: input.summary,
  })
  const response = await input.fetcher(
    `${input.baseUrl}/api/pylon/provider-accounts/chatgpt-codex/local-auth/import`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${input.agentToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        accountLabel: input.accountRef,
        createNew: input.providerAccountRef === null,
        ...(input.providerAccountRef === null
          ? {}
          : { providerAccountRef: input.providerAccountRef }),
        auth,
      }),
    },
  )
  return parseLocalCodexAuthImportResponse(await readJsonResponse(response))
}

const defaultClaudeAccountHome = (
  summary: Pick<BootstrapSummary, "paths">,
  accountRef: string,
): string => join(summary.paths.home, "accounts", "claude_agent", accountRef)

async function importLocalClaudeAuth(input: {
  accountRef: string
  agentToken: string
  authHome: string
  baseUrl: string
  fetcher: PylonAccountsConnectFetcher
  providerAccountRef: string | null
  summary: Pick<BootstrapSummary, "paths">
}): Promise<OpenAgentsLocalClaudeAuthImportResponse> {
  const authContentValue = (
    await readFile(
      join(input.authHome, "claude-oauth-token"),
      "utf8",
    )
  ).trim()
  if (authContentValue === "") {
    throw new Error("local Claude auth is empty")
  }
  const response = await input.fetcher(
    `${input.baseUrl}/api/pylon/provider-accounts/anthropic-claude/local-auth/import`,
    {
      method: "POST",
      headers: {
        authorization: `Bearer ${input.agentToken}`,
        "content-type": "application/json",
      },
      body: JSON.stringify({
        accountLabel: input.accountRef,
        createNew: input.providerAccountRef === null,
        ...(input.providerAccountRef === null
          ? {}
          : { providerAccountRef: input.providerAccountRef }),
        authContentValue,
      }),
    },
  )
  return parseLocalCodexAuthImportResponse(await readJsonResponse(response))
}

const codexDeviceUrlPattern = /https:\/\/auth\.openai\.com\/codex\/device\b/
const codexDeviceCodePattern = /\b[A-Z0-9]{4}-[A-Z0-9]{4,6}\b/

function maybeEmitCodexDevicePrompt(input: {
  buffer: string
  emitted: { current: boolean }
  onDevicePrompt: PylonAuthOptions["onDevicePrompt"]
}): void {
  if (input.emitted.current) {
    return
  }
  // Codex prints the device URL + one-time code wrapped in ANSI color escapes
  // (e.g. `\x1b[94m8260-DUG55\x1b[0m`). The leading `\x1b[94m` ends in `m` — a
  // word char — directly before the code, which kills `codexDeviceCodePattern`'s
  // leading `\b`, so the code never matches, `onDevicePrompt` never fires, and
  // the spawned `codex login --device-auth` polls forever -> the CLI hangs with
  // no output. Strip ANSI escapes before matching so the prompt is surfaced.
  const cleaned = input.buffer.replace(/\[[0-9;]*m/g, "")
  const verificationUrl = cleaned.match(codexDeviceUrlPattern)?.[0]
  const userCode = cleaned.match(codexDeviceCodePattern)?.[0]
  if (verificationUrl === undefined || userCode === undefined) {
    return
  }
  input.emitted.current = true
  input.onDevicePrompt?.({
    kind: "codex",
    userCode,
    verificationUrl,
  })
}

async function collectCodexLoginStream(input: {
  emitted: { current: boolean }
  onDevicePrompt: PylonAuthOptions["onDevicePrompt"]
  stream: ReadableStream<Uint8Array> | null
}): Promise<string> {
  if (input.stream === null) {
    return ""
  }
  const reader = input.stream.getReader()
  const decoder = new TextDecoder()
  let buffer = ""
  while (true) {
    const chunk = await reader.read()
    if (chunk.done) {
      buffer += decoder.decode()
      maybeEmitCodexDevicePrompt({ ...input, buffer })
      return buffer
    }
    buffer += decoder.decode(chunk.value, { stream: true })
    maybeEmitCodexDevicePrompt({ ...input, buffer })
  }
}

const quietCodexDeviceLoginRunner = (
  onDevicePrompt: PylonAuthOptions["onDevicePrompt"],
): PylonCodexDeviceLoginRunner => async input => {
  const child = Runtime.spawn(["codex", "login", "--device-auth"], {
    env: {
      ...process.env,
      ...input.env,
      CODEX_HOME: input.home,
    },
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
  })
  const emitted = { current: false }
  const [stdout, stderr, exitCode] = await Promise.all([
    collectCodexLoginStream({
      emitted,
      onDevicePrompt,
      stream: child.stdout,
    }),
    collectCodexLoginStream({
      emitted,
      onDevicePrompt,
      stream: child.stderr,
    }),
    child.exited,
  ])
  if (exitCode !== 0 && !emitted.current) {
    const combined = `${stdout}\n${stderr}`
    maybeEmitCodexDevicePrompt({
      buffer: combined,
      emitted,
      onDevicePrompt,
    })
  }
  return { exitCode }
}

export async function runPylonAuthCodex(
  summary: Pick<BootstrapSummary, "bootstrap" | "paths">,
  args: PylonAuthArgs,
  options: PylonAuthOptions = {},
): Promise<PylonAuthCodexProjection> {
  const env = options.env ?? (Runtime.env as Record<string, string | undefined>)
  const fetcher = options.fetcher ?? fetch
  const sleep = options.sleep ?? defaultSleep
  const baseUrl = baseUrlFrom(args, env)
  // LOCAL-ONLY by default (owner directive, EP250): the OpenAgents Pylon
  // device link + provider-account import run ONLY behind the explicit
  // --openagents-link opt-in. The default connect makes zero network calls
  // to openagents.com — isolated device login + local config registration.
  const openAgents = args.openAgentsLink
    ? await runPylonAuthOpenAgents(summary, { ...args, target: "openagents" }, options)
    : null
  const accountRef = args.accountRef ?? await nextCodexAccountRef(summary)
  const configBeforeConnect = await readConfig(summary)
  const previousProviderAccountRef = configuredProviderAccountRef(configBeforeConnect, accountRef)
  const started = await runPylonAccountsConnect(
    summary,
    codexConnectArgs({
      accountRef,
      forceDeviceLogin: args.forceDeviceLogin,
      skipDeviceLogin: false,
    }),
    {
      env,
      fetcher,
      runCodexDeviceLogin:
        options.runCodexDeviceLogin ?? quietCodexDeviceLoginRunner(options.onDevicePrompt),
      codexAuthValidityProbe: options.codexAuthValidityProbe ?? defaultCodexAuthValidityProbe,
    },
  )
  void sleep

  // The stored Codex credential was present but invalid and could not be
  // recovered (e.g. non-interactive). Do NOT import a dead credential or report
  // a connected account; return an honest credentials-invalid projection so the
  // CLI can tell the user to re-run with --force-device-login.
  if (started.deviceLogin.status === "blocked_invalid_auth") {
    const projection = {
      schema: "pylon.auth.codex.v1",
      status: "credentials_invalid",
      accountRef,
      ...(openAgents === null ? {} : { openAgents: openAgents.projection }),
      localCodex: {
        deviceLoginStatus: started.deviceLogin.status,
        ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
      },
      openAgentsProviderAccount: {
        accountStatus: "credentials_invalid",
        attemptId: "not_attempted",
        attemptStatus: "not_attempted",
        providerAccountRef: previousProviderAccountRef ?? accountRef,
        source: "pylon_local_codex_auth",
      },
      blockerRefs: started.blockerRefs,
    } satisfies PylonAuthCodexProjection
    assertPublicProjectionSafe(projection)
    return projection
  }

  // LOCAL-ONLY default: the connect is complete right here — device login
  // done, account registered locally. No server import is attempted and none
  // is pending; the OpenAgents link is a separate opt-in (--openagents-link).
  if (openAgents === null) {
    const projection = {
      schema: "pylon.auth.codex.v1",
      status: "connected",
      accountRef,
      localCodex: {
        deviceLoginStatus: started.deviceLogin.status,
        ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
      },
      openAgentsProviderAccount: {
        accountStatus: "not_attempted_local_only",
        attemptId: "not_attempted",
        attemptStatus: "not_attempted",
        providerAccountRef: previousProviderAccountRef ?? accountRef,
        source: "pylon_local_codex_auth",
      },
      blockerRefs: [],
    } satisfies PylonAuthCodexProjection
    assertPublicProjectionSafe(projection)
    return projection
  }

  // EP250 regression guard: at this point the device login already completed
  // and the account is registered in the local Pylon config — the local
  // connect substantively SUCCEEDED. The OpenAgents provider-account import
  // below is a network POST that can fail independently (server down, DNS,
  // 5xx). That failure must not surface as a bare `Pylon auth failed` exit:
  // return an honest connected_local_only projection naming the pending step.
  let imported: Awaited<ReturnType<typeof importLocalCodexAuth>>
  try {
    imported = await importLocalCodexAuth({
      accountRef,
      agentToken: openAgents.agentToken,
      baseUrl,
      fetcher,
      providerAccountRef: previousProviderAccountRef,
      summary,
    })
    await writeConfiguredProviderAccountRef({
      accountRef,
      providerAccountRef: imported.account.providerAccountRef,
      summary,
    })
  } catch {
    // No raw error text in the projection: import failures can embed URLs and
    // response bodies. The typed blocker names the step; nothing else leaves.
    const projection = {
      schema: "pylon.auth.codex.v1",
      status: "connected_local_only",
      accountRef,
      openAgents: openAgents.projection,
      localCodex: {
        deviceLoginStatus: started.deviceLogin.status,
        ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
      },
      openAgentsProviderAccount: {
        accountStatus: "import_failed",
        attemptId: "not_attempted",
        attemptStatus: "not_attempted",
        providerAccountRef: previousProviderAccountRef ?? accountRef,
        source: "pylon_local_codex_auth",
      },
      blockerRefs: ["blocker.pylon.auth.codex.openagents_provider_import_failed"],
    } satisfies PylonAuthCodexProjection
    assertPublicProjectionSafe(projection)
    return projection
  }

  const projection = {
    schema: "pylon.auth.codex.v1",
    status: "connected",
    accountRef,
    openAgents: openAgents.projection,
    localCodex: {
      deviceLoginStatus: started.deviceLogin.status,
      ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
    },
    openAgentsProviderAccount: {
      accountStatus: imported.account.status,
      attemptId: imported.attempt.id,
      attemptStatus: imported.attempt.status,
      providerAccountRef: imported.account.providerAccountRef,
      source: "pylon_local_codex_auth",
    },
    blockerRefs: [],
  } satisfies PylonAuthCodexProjection
  assertPublicProjectionSafe(projection)
  return projection
}

/**
 * Local Claude account connect via setup-token file storage. OpenAgents
 * linking is opt-in and token material is never projected.
 */
export async function runPylonAuthClaude(
  summary: Pick<BootstrapSummary, "bootstrap" | "paths">,
  args: PylonAuthArgs,
  options: PylonAuthOptions = {},
): Promise<PylonAuthClaudeProjection> {
  const env = options.env ?? (Runtime.env as Record<string, string | undefined>)
  const fetcher = options.fetcher ?? fetch
  const baseUrl = baseUrlFrom(args, env)
  const openAgents = args.openAgentsLink
    ? await runPylonAuthOpenAgents(summary, { ...args, target: "openagents" }, options)
    : null
  const accountRef = args.accountRef ?? await nextClaudeAccountRef(summary)
  const configBeforeConnect = await readConfig(summary)
  const previousProviderAccountRef = configuredProviderAccountRef(
    configBeforeConnect,
    accountRef,
    "claude_agent",
  )
  const authHome =
    configuredAccountHome(configBeforeConnect, accountRef, "claude_agent") ??
    defaultClaudeAccountHome(summary, accountRef)
  const started = await runPylonAccountsConnect(
    summary,
    claudeConnectArgs({
      accountRef,
      home: authHome,
      setupToken: args.setupToken,
    }),
    { env },
  )

  const setupTokenStatus =
    started.deviceLogin.status === "skipped_existing_auth"
      ? "skipped_existing_auth"
      : "completed"

  if (openAgents === null) {
    const projection = {
      schema: "pylon.auth.claude.v1",
      status: "connected",
      accountRef,
      provider: "claude_agent",
      localClaude: {
        setupTokenStatus,
        ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
      },
      openAgentsProviderAccount: {
        accountStatus: "not_attempted_local_only",
        attemptId: "not_attempted",
        attemptStatus: "not_attempted",
        providerAccountRef: previousProviderAccountRef ?? accountRef,
        source: "pylon_local_claude_auth",
      },
      blockerRefs: started.blockerRefs,
    } satisfies PylonAuthClaudeProjection
    assertPublicProjectionSafe(projection)
    return projection
  }

  let imported: OpenAgentsLocalClaudeAuthImportResponse
  try {
    imported = await importLocalClaudeAuth({
      accountRef,
      agentToken: openAgents.agentToken,
      authHome,
      baseUrl,
      fetcher,
      providerAccountRef: previousProviderAccountRef,
      summary,
    })
    await writeConfiguredProviderAccountRef({
      accountRef,
      provider: "claude_agent",
      providerAccountRef: imported.account.providerAccountRef,
      summary,
    })
  } catch {
    const projection = {
      schema: "pylon.auth.claude.v1",
      status: "connected_local_only",
      accountRef,
      provider: "claude_agent",
      openAgents: openAgents.projection,
      localClaude: {
        setupTokenStatus,
        ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
      },
      openAgentsProviderAccount: {
        accountStatus: "import_failed",
        attemptId: "not_attempted",
        attemptStatus: "not_attempted",
        providerAccountRef: previousProviderAccountRef ?? accountRef,
        source: "pylon_local_claude_auth",
      },
      blockerRefs: ["blocker.pylon.auth.claude.openagents_provider_import_failed"],
    } satisfies PylonAuthClaudeProjection
    assertPublicProjectionSafe(projection)
    return projection
  }

  const projection = {
    schema: "pylon.auth.claude.v1",
    status: "connected",
    accountRef,
    provider: "claude_agent",
    openAgents: openAgents.projection,
    localClaude: {
      setupTokenStatus,
      ...(started.deviceLogin.reason !== undefined ? { reason: started.deviceLogin.reason } : {}),
    },
    openAgentsProviderAccount: {
      accountStatus: imported.account.status,
      attemptId: imported.attempt.id,
      attemptStatus: imported.attempt.status,
      providerAccountRef: imported.account.providerAccountRef,
      source: "pylon_local_claude_auth",
    },
    blockerRefs: [],
  } satisfies PylonAuthClaudeProjection
  assertPublicProjectionSafe(projection)
  return projection
}
