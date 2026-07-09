import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import type { KhalaCodeDesktopRuntimeMode } from "../shared/rpc.js"
import {
  decodeKhalaCodeModelRoleRegistry,
  defaultKhalaCodeModelRoleRegistry,
  khalaCodeModelRoleRegistryWithEntry,
  type KhalaCodeModelRole,
  type KhalaCodeModelRoleEntry,
  type KhalaCodeModelRoleRegistry,
} from "../shared/model-roles.js"

type ChatEnv = Readonly<Record<string, string | undefined>>

export type KhalaCodeDesktopHarnessSetting = {
  readonly ok: true
  readonly mode: KhalaCodeDesktopRuntimeMode
  readonly persistedMode: KhalaCodeDesktopRuntimeMode
  readonly envOverride: KhalaCodeDesktopRuntimeMode | null
  readonly path: string
}

export type KhalaCodeDesktopHarnessSettingWriteResult =
  KhalaCodeDesktopHarnessSetting & {
    readonly saved: boolean
  }

export type KhalaCodeDesktopModelRoleRegistrySetting = {
  readonly ok: true
  readonly path: string
  readonly registry: KhalaCodeModelRoleRegistry
}

export type KhalaCodeDesktopModelRoleRegistryWriteResult =
  KhalaCodeDesktopModelRoleRegistrySetting & {
    readonly saved: boolean
  }

export type KhalaCodeDesktopTraceCaptureConsentSetting = {
  readonly enabled: boolean
  readonly ok: true
  readonly path: string
}

export type KhalaCodeDesktopTraceCaptureConsentWriteResult =
  KhalaCodeDesktopTraceCaptureConsentSetting & {
    readonly saved: boolean
  }

export type KhalaCodeDesktopOpenAgentsAuthPendingAttempt = {
  readonly attemptId: string
  readonly expiresAt: string
  readonly intervalSeconds: number
  readonly pollSecret: string
  readonly userCode: string
  readonly verificationUrl: string
}

export type KhalaCodeDesktopOpenAgentsAuthSetting = {
  readonly ok: true
  readonly path: string
  readonly pendingAttempt: KhalaCodeDesktopOpenAgentsAuthPendingAttempt | null
  readonly source: "env" | "persisted" | null
  readonly state: "connected" | "missing" | "pending"
  readonly tokenPrefix: string | null
}

export type KhalaCodeDesktopOpenAgentsAuthPendingAttemptWriteResult =
  KhalaCodeDesktopOpenAgentsAuthSetting & {
    readonly saved: boolean
  }

const DEFAULT_HARNESS_MODE: KhalaCodeDesktopRuntimeMode = "codex_harness"
const VALID_HARNESS_MODES = new Set<KhalaCodeDesktopRuntimeMode>([
  "claude_runtime",
  "codex_harness",
  "grok_runtime",
  "khala_native_runtime",
])

const isRuntimeMode = (value: unknown): value is KhalaCodeDesktopRuntimeMode =>
  typeof value === "string" && VALID_HARNESS_MODES.has(value as KhalaCodeDesktopRuntimeMode)

const settingsObject = (value: unknown): Record<string, unknown> =>
  typeof value === "object" && value !== null && !Array.isArray(value)
    ? { ...(value as Record<string, unknown>) }
    : {}

const booleanValue = (value: unknown): boolean =>
  typeof value === "boolean" ? value : false

const stringValue = (value: unknown): string | null =>
  typeof value === "string" && value.trim().length > 0 ? value.trim() : null

const numberValue = (value: unknown): number | null =>
  typeof value === "number" && Number.isFinite(value) ? value : null

const openAgentsAgentTokenValue = (value: unknown): string | null => {
  const token = stringValue(value)
  return token !== null && token.startsWith("oa_agent_") ? token : null
}

const tokenPrefixFor = (token: string): string => token.slice(0, 20)

const pendingAttemptValue = (
  value: unknown,
): KhalaCodeDesktopOpenAgentsAuthPendingAttempt | null => {
  const attempt = settingsObject(value)
  const attemptId = stringValue(attempt.attemptId)
  const expiresAt = stringValue(attempt.expiresAt)
  const intervalSeconds = numberValue(attempt.intervalSeconds)
  const pollSecret = stringValue(attempt.pollSecret)
  const userCode = stringValue(attempt.userCode)
  const verificationUrl = stringValue(attempt.verificationUrl)
  return attemptId !== null &&
    expiresAt !== null &&
    intervalSeconds !== null &&
    pollSecret !== null &&
    userCode !== null &&
    verificationUrl !== null
    ? { attemptId, expiresAt, intervalSeconds, pollSecret, userCode, verificationUrl }
    : null
}

export const khalaCodeDesktopHarnessSettingPath = (env: ChatEnv): string =>
  env.KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH?.trim() ||
  join(homedir(), ".khala-code", "desktop-settings.json")

export const khalaCodeDesktopRuntimeEnvOverride = (
  env: ChatEnv,
): KhalaCodeDesktopRuntimeMode | null => {
  if (env.KHALA_CODE_DESKTOP_RUNTIME === "claude_runtime") return "claude_runtime"
  if (env.KHALA_CODE_DESKTOP_RUNTIME === "codex_harness") return "codex_harness"
  if (env.KHALA_CODE_DESKTOP_RUNTIME === "grok_runtime") return "grok_runtime"
  if (
    env.KHALA_CODE_DESKTOP_RUNTIME === "khala_native_runtime" ||
    env.KHALA_CODE_DESKTOP_LEGACY_KHALA_NATIVE_RUNTIME === "1"
  ) {
    return "khala_native_runtime"
  }
  return null
}

export async function readKhalaCodeDesktopPersistedHarnessMode(
  env: ChatEnv,
): Promise<KhalaCodeDesktopRuntimeMode> {
  try {
    const raw = await readFile(khalaCodeDesktopHarnessSettingPath(env), "utf8")
    const parsed = JSON.parse(raw) as { readonly harnessMode?: unknown }
    return isRuntimeMode(parsed.harnessMode) ? parsed.harnessMode : DEFAULT_HARNESS_MODE
  } catch {
    return DEFAULT_HARNESS_MODE
  }
}

const readKhalaCodeDesktopSettingsDocument = async (
  env: ChatEnv,
): Promise<Record<string, unknown>> => {
  try {
    return settingsObject(JSON.parse(await readFile(khalaCodeDesktopHarnessSettingPath(env), "utf8")))
  } catch {
    return {}
  }
}

export async function readKhalaCodeDesktopHarnessSetting(
  env: ChatEnv,
): Promise<KhalaCodeDesktopHarnessSetting> {
  const persistedMode = await readKhalaCodeDesktopPersistedHarnessMode(env)
  const envOverride = khalaCodeDesktopRuntimeEnvOverride(env)
  return {
    ok: true,
    mode: envOverride ?? persistedMode,
    persistedMode,
    envOverride,
    path: khalaCodeDesktopHarnessSettingPath(env),
  }
}

export async function writeKhalaCodeDesktopHarnessSetting(
  mode: KhalaCodeDesktopRuntimeMode,
  env: ChatEnv,
): Promise<KhalaCodeDesktopHarnessSettingWriteResult> {
  if (!isRuntimeMode(mode)) throw new Error(`Unsupported harness mode: ${String(mode)}`)
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const current = await readKhalaCodeDesktopSettingsDocument(env)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    ...current,
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: mode,
  }, null, 2)}\n`)
  const setting = await readKhalaCodeDesktopHarnessSetting(env)
  return {
    ...setting,
    saved: true,
  }
}

export async function readKhalaCodeDesktopModelRoleRegistry(
  env: ChatEnv,
): Promise<KhalaCodeDesktopModelRoleRegistrySetting> {
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  const registry = (() => {
    try {
      return decodeKhalaCodeModelRoleRegistry(settings.modelRoleRegistry)
    } catch {
      return defaultKhalaCodeModelRoleRegistry()
    }
  })()
  return {
    ok: true,
    path: khalaCodeDesktopHarnessSettingPath(env),
    registry,
  }
}

export async function readKhalaCodeDesktopTraceCaptureConsent(
  env: ChatEnv,
): Promise<KhalaCodeDesktopTraceCaptureConsentSetting> {
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  return {
    enabled: booleanValue(settings.traceCaptureConsentEnabled),
    ok: true,
    path: khalaCodeDesktopHarnessSettingPath(env),
  }
}

export async function writeKhalaCodeDesktopTraceCaptureConsent(
  enabled: boolean,
  env: ChatEnv,
): Promise<KhalaCodeDesktopTraceCaptureConsentWriteResult> {
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const current = await readKhalaCodeDesktopSettingsDocument(env)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    ...current,
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: isRuntimeMode(current.harnessMode) ? current.harnessMode : DEFAULT_HARNESS_MODE,
    traceCaptureConsentEnabled: enabled,
  }, null, 2)}\n`)
  const setting = await readKhalaCodeDesktopTraceCaptureConsent(env)
  return {
    ...setting,
    saved: true,
  }
}

export function khalaCodeDesktopOpenAgentsEnvToken(env: ChatEnv): string | null {
  const token = env.OPENAGENTS_AGENT_TOKEN?.trim() || env.OPENAGENTS_API_KEY?.trim()
  return token !== undefined && token.length > 0 ? token : null
}

export async function readKhalaCodeDesktopPersistedOpenAgentsAgentToken(
  env: ChatEnv,
): Promise<string | null> {
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  return openAgentsAgentTokenValue(settings.openAgentsAgentToken)
}

export async function resolveKhalaCodeDesktopOpenAgentsAgentToken(
  env: ChatEnv,
): Promise<string | null> {
  return khalaCodeDesktopOpenAgentsEnvToken(env) ??
    await readKhalaCodeDesktopPersistedOpenAgentsAgentToken(env)
}

export async function envWithKhalaCodeDesktopOpenAgentsAgentToken(
  env: ChatEnv,
): Promise<ChatEnv> {
  if (khalaCodeDesktopOpenAgentsEnvToken(env) !== null) return env
  const token = await readKhalaCodeDesktopPersistedOpenAgentsAgentToken(env)
  return token === null ? env : { ...env, OPENAGENTS_AGENT_TOKEN: token }
}

export async function readKhalaCodeDesktopOpenAgentsAuthSetting(
  env: ChatEnv,
): Promise<KhalaCodeDesktopOpenAgentsAuthSetting> {
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  const envToken = khalaCodeDesktopOpenAgentsEnvToken(env)
  if (envToken !== null) {
    return {
      ok: true,
      path,
      pendingAttempt: null,
      source: "env",
      state: "connected",
      tokenPrefix: tokenPrefixFor(envToken),
    }
  }

  const persistedToken = openAgentsAgentTokenValue(settings.openAgentsAgentToken)
  if (persistedToken !== null) {
    return {
      ok: true,
      path,
      pendingAttempt: null,
      source: "persisted",
      state: "connected",
      tokenPrefix: tokenPrefixFor(persistedToken),
    }
  }

  const pendingAttempt = pendingAttemptValue(settings.openAgentsAuthPendingAttempt)
  return {
    ok: true,
    path,
    pendingAttempt,
    source: null,
    state: pendingAttempt === null ? "missing" : "pending",
    tokenPrefix: null,
  }
}

export async function writeKhalaCodeDesktopOpenAgentsAuthPendingAttempt(
  attempt: KhalaCodeDesktopOpenAgentsAuthPendingAttempt,
  env: ChatEnv,
): Promise<KhalaCodeDesktopOpenAgentsAuthPendingAttemptWriteResult> {
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const current = await readKhalaCodeDesktopSettingsDocument(env)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    ...current,
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: isRuntimeMode(current.harnessMode) ? current.harnessMode : DEFAULT_HARNESS_MODE,
    openAgentsAuthPendingAttempt: attempt,
  }, null, 2)}\n`)
  return {
    ...await readKhalaCodeDesktopOpenAgentsAuthSetting(env),
    saved: true,
  }
}

export async function writeKhalaCodeDesktopOpenAgentsAgentToken(
  token: string,
  env: ChatEnv,
  // Khala Sync personal-scope owner user id (KS-6.2), captured from the same
  // device-link response as the agent token (`linkedAgent.userId`) so the two
  // are written together and never drift apart. Optional so existing callers
  // that only have a token (no linked-agent identity) keep working unchanged.
  ownerUserId?: string | null,
): Promise<KhalaCodeDesktopOpenAgentsAuthPendingAttemptWriteResult> {
  const trimmed = openAgentsAgentTokenValue(token)
  if (trimmed === null) throw new Error("OpenAgents agent token must start with oa_agent_.")
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const current = await readKhalaCodeDesktopSettingsDocument(env)
  const rest = { ...current }
  delete rest.openAgentsAuthPendingAttempt
  const trimmedOwnerUserId = ownerUserId?.trim()
  if (trimmedOwnerUserId !== undefined && trimmedOwnerUserId.length > 0) {
    rest.khalaSyncOwnerUserId = trimmedOwnerUserId
  }
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    ...rest,
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: isRuntimeMode(current.harnessMode) ? current.harnessMode : DEFAULT_HARNESS_MODE,
    openAgentsAgentToken: trimmed,
  }, null, 2)}\n`)
  return {
    ...await readKhalaCodeDesktopOpenAgentsAuthSetting(env),
    saved: true,
  }
}

/**
 * Khala Sync personal-scope owner user id for THIS device's linked OpenAgents
 * agent — persisted alongside the agent token (see
 * `writeKhalaCodeDesktopOpenAgentsAgentToken`) and, for local/dev setups that
 * predate a real device link, overridable via `KHALA_SYNC_CHAT_OWNER_USER_ID`.
 */
export async function readKhalaCodeDesktopKhalaSyncOwnerUserId(
  env: ChatEnv,
): Promise<string | null> {
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  const persisted = stringValue(settings.khalaSyncOwnerUserId)
  if (persisted !== null) return persisted
  const fromEnv = env.KHALA_SYNC_CHAT_OWNER_USER_ID?.trim()
  return fromEnv !== undefined && fromEnv.length > 0 ? fromEnv : null
}

export type KhalaCodeDesktopMobilePairingCredentials = Readonly<{
  ownerUserId: string
  token: string
}>

/**
 * Resolves the exact (ownerUserId, token) pair a mobile Tailnet pairing
 * handoff would hand to a phone (MC-6, docs/khala-code/2026-07-04-mobile-tailnet-handshake.md).
 * `null` whenever either half is missing — this device is not meaningfully
 * "signed in" for mobile pairing purposes yet, and callers must fail closed
 * rather than hand over a partial credential.
 */
export async function resolveKhalaCodeDesktopMobilePairingCredentials(
  env: ChatEnv,
): Promise<KhalaCodeDesktopMobilePairingCredentials | null> {
  const token = await resolveKhalaCodeDesktopOpenAgentsAgentToken(env)
  if (token === null) return null
  const ownerUserId = await readKhalaCodeDesktopKhalaSyncOwnerUserId(env)
  if (ownerUserId === null) return null
  return { ownerUserId, token }
}

export async function hasKhalaCodeDesktopPersistedModelRoleRegistry(
  env: ChatEnv,
): Promise<boolean> {
  const settings = await readKhalaCodeDesktopSettingsDocument(env)
  try {
    decodeKhalaCodeModelRoleRegistry(settings.modelRoleRegistry)
    return true
  } catch {
    return false
  }
}

export async function writeKhalaCodeDesktopModelRoleRegistry(
  registry: KhalaCodeModelRoleRegistry,
  env: ChatEnv,
): Promise<KhalaCodeDesktopModelRoleRegistryWriteResult> {
  const decoded = decodeKhalaCodeModelRoleRegistry(registry)
  const path = khalaCodeDesktopHarnessSettingPath(env)
  const current = await readKhalaCodeDesktopSettingsDocument(env)
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    ...current,
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: isRuntimeMode(current.harnessMode) ? current.harnessMode : DEFAULT_HARNESS_MODE,
    modelRoleRegistry: decoded,
  }, null, 2)}\n`)
  return {
    ok: true,
    path,
    registry: decoded,
    saved: true,
  }
}

export async function writeKhalaCodeDesktopModelRoleEntry(
  entry: KhalaCodeModelRoleEntry,
  env: ChatEnv,
): Promise<KhalaCodeDesktopModelRoleRegistryWriteResult> {
  const current = await readKhalaCodeDesktopModelRoleRegistry(env)
  const next = khalaCodeModelRoleRegistryWithEntry(current.registry, entry)
  return writeKhalaCodeDesktopModelRoleRegistry(next, env)
}

export async function resolveKhalaCodeDesktopModelRole(
  role: KhalaCodeModelRole,
  env: ChatEnv,
): Promise<KhalaCodeModelRoleEntry> {
  const { registry } = await readKhalaCodeDesktopModelRoleRegistry(env)
  return registry.roles[role]
}
