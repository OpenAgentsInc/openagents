import { createHash } from "node:crypto"
import { mkdir, readFile, rename, writeFile, chmod } from "node:fs/promises"
import { dirname, join } from "node:path"

import {
  type PylonAccountsConnectArgs,
  type PylonAccountsConnectFetcher,
  type PylonCodexDeviceLoginRunner,
  runPylonAccountsConnect,
} from "./account-connect.js"
import { type BootstrapSummary } from "./bootstrap.js"
import { assertPublicProjectionSafe } from "./state.js"

export type PylonAuthTarget = "openagents" | "codex"

export type PylonAuthArgs = {
  accountRef: string | null
  agentToken: string | null
  baseUrl: string | null
  forceDeviceLogin: boolean
  json: boolean
  target: PylonAuthTarget
  timeoutSeconds: number
}

type AgentTokenSource = "env" | "stored" | "registered"

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
  status: "connected"
  accountRef: string
  openAgents: PylonAuthOpenAgentsProjection
  localCodex: {
    deviceLoginStatus: "completed" | "skipped_existing_auth" | "skipped_by_flag"
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
  if (target !== "openagents" && target !== "codex") {
    throw new Error("usage: pylon auth openagents|codex [--account <ref>] [--json]")
  }

  const parsed: PylonAuthArgs = {
    accountRef: null,
    agentToken: null,
    baseUrl: null,
    forceDeviceLogin: false,
    json: false,
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
    } else if (arg === "--force-device-login") {
      parsed.forceDeviceLogin = true
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
    throw new Error("pylon auth codex --account must use letters, numbers, dot, dash, or underscore")
  }
  if (parsed.target === "openagents" && parsed.accountRef !== null) {
    throw new Error("pylon auth openagents does not take --account")
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

async function readStoredAgentToken(summary: Pick<BootstrapSummary, "paths">): Promise<string | null> {
  try {
    const token = (await readFile(agentTokenPath(summary), "utf8")).trim()
    return token.startsWith("oa_agent_") ? token : null
  } catch {
    return null
  }
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
    throw new Error(message)
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
  const explicit = (input.args.agentToken ?? input.env.OPENAGENTS_AGENT_TOKEN ?? "").trim()
  if (explicit !== "") {
    const token = requireAgentTokenShape(explicit, "OPENAGENTS_AGENT_TOKEN")
    await writeStoredAgentToken(input.summary, token)
    return { source: "env", token }
  }
  const stored = await readStoredAgentToken(input.summary)
  if (stored !== null) {
    return { source: "stored", token: stored }
  }
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
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const fetcher = options.fetcher ?? fetch
  const sleep = options.sleep ?? defaultSleep
  const baseUrl = baseUrlFrom(args, env)
  const token = await ensureAgentToken({ args, baseUrl, env, fetcher, summary })
  const started = await startOpenAgentsAuth({
    baseUrl,
    fetcher,
    token: token.token,
  })

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

async function nextCodexAccountRef(summary: Pick<BootstrapSummary, "paths">): Promise<string> {
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
      return record.provider === "codex" && typeof record.ref === "string"
        ? [record.ref]
        : []
    }),
  )

  if (!existing.has("codex")) {
    return "codex"
  }
  for (let index = 2; index < 10_000; index += 1) {
    const candidate = `codex-${index}`
    if (!existing.has(candidate)) {
      return candidate
    }
  }
  return `codex-${hashRef(String(Date.now()))}`
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
    skipDeviceLogin: input.skipDeviceLogin,
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
): string | null {
  const dev = recordAt(config, "dev")
  const accounts = Array.isArray(dev.accounts) ? dev.accounts : []
  for (const account of accounts) {
    if (account === null || typeof account !== "object" || Array.isArray(account)) {
      continue
    }
    const record = account as Record<string, unknown>
    if (record.provider === "codex" && record.ref === accountRef) {
      return optionalStringAt(record, "openAgentsProviderAccountRef") ?? null
    }
  }
  return null
}

async function writeConfiguredProviderAccountRef(input: {
  accountRef: string
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
    return record.provider === "codex" && record.ref === input.accountRef
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
  const child = Bun.spawn(["codex", "login", "--device-auth"], {
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
  summary: Pick<BootstrapSummary, "paths">,
  args: PylonAuthArgs,
  options: PylonAuthOptions = {},
): Promise<PylonAuthCodexProjection> {
  const env = options.env ?? (Bun.env as Record<string, string | undefined>)
  const fetcher = options.fetcher ?? fetch
  const sleep = options.sleep ?? defaultSleep
  const baseUrl = baseUrlFrom(args, env)
  const openAgents = await runPylonAuthOpenAgents(summary, { ...args, target: "openagents" }, options)
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
    },
  )
  void sleep

  const imported = await importLocalCodexAuth({
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

  const projection = {
    schema: "pylon.auth.codex.v1",
    status: "connected",
    accountRef,
    openAgents: openAgents.projection,
    localCodex: {
      deviceLoginStatus: started.deviceLogin.status,
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
