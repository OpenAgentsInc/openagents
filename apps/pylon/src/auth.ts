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
  }
  blockerRefs: string[]
}

type DevicePromptKind = "openagents" | "codex_provider"

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
  agentToken: string
  baseUrl: string
  forceDeviceLogin: boolean
  openAgentsAttemptId: string | null
  skipDeviceLogin: boolean
}): PylonAccountsConnectArgs {
  return {
    provider: "codex",
    accountRef: input.accountRef,
    accountLabel: input.accountRef,
    agentToken: input.agentToken,
    baseUrl: input.baseUrl,
    createNewOpenAgentsAccount: true,
    home: null,
    forceDeviceLogin: input.forceDeviceLogin,
    json: true,
    openAgentsAttemptId: input.openAgentsAttemptId,
    openAgentsLink: true,
    providerAccountRef: null,
    skipDeviceLogin: input.skipDeviceLogin,
  }
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
  const started = await runPylonAccountsConnect(
    summary,
    codexConnectArgs({
      accountRef,
      agentToken: openAgents.agentToken,
      baseUrl,
      forceDeviceLogin: args.forceDeviceLogin,
      openAgentsAttemptId: null,
      skipDeviceLogin: false,
    }),
    {
      env,
      fetcher,
      ...(options.runCodexDeviceLogin === undefined
        ? {}
        : { runCodexDeviceLogin: options.runCodexDeviceLogin }),
    },
  )

  if (started.openAgentsDeviceLogin.status !== "started") {
    throw new Error("OpenAgents Codex provider device login did not start")
  }

  options.onDevicePrompt?.({
    kind: "codex_provider",
    userCode: started.openAgentsDeviceLogin.userCode,
    verificationUrl: started.openAgentsDeviceLogin.verificationUrl,
  })

  const deadline = Date.now() + args.timeoutSeconds * 1000
  let attemptStatus = "pending"
  let accountStatus = "connecting"
  while (Date.now() <= deadline) {
    await sleep(Math.max(1, started.openAgentsDeviceLogin.intervalSeconds) * 1000)
    const polled = await runPylonAccountsConnect(
      summary,
      codexConnectArgs({
        accountRef,
        agentToken: openAgents.agentToken,
        baseUrl,
        forceDeviceLogin: false,
        openAgentsAttemptId: started.openAgentsDeviceLogin.attemptId,
        skipDeviceLogin: true,
      }),
      {
        env,
        fetcher,
        ...(options.runCodexDeviceLogin === undefined
          ? {}
          : { runCodexDeviceLogin: options.runCodexDeviceLogin }),
      },
    )
    if (polled.openAgentsDeviceLogin.status !== "polled") {
      throw new Error("OpenAgents Codex provider poll returned an unexpected status")
    }
    attemptStatus = polled.openAgentsDeviceLogin.attemptStatus
    accountStatus = polled.openAgentsDeviceLogin.accountStatus
    if (attemptStatus === "connected" && accountStatus === "connected") {
      const projection = {
        schema: "pylon.auth.codex.v1",
        status: "connected",
        accountRef,
        openAgents: openAgents.projection,
        localCodex: {
          deviceLoginStatus: started.deviceLogin.status,
        },
        openAgentsProviderAccount: {
          accountStatus,
          attemptId: polled.openAgentsDeviceLogin.attemptId,
          attemptStatus,
          providerAccountRef: polled.openAgentsDeviceLogin.providerAccountRef,
        },
        blockerRefs: [],
      } satisfies PylonAuthCodexProjection
      assertPublicProjectionSafe(projection)
      return projection
    }
    if (attemptStatus === "expired" || attemptStatus === "denied") {
      throw new Error(`OpenAgents Codex provider device login ${attemptStatus}`)
    }
  }

  throw new Error(
    `OpenAgents Codex provider auth timed out while account status was ${accountStatus}`,
  )
}
