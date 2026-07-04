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

const DEFAULT_HARNESS_MODE: KhalaCodeDesktopRuntimeMode = "codex_harness"
const VALID_HARNESS_MODES = new Set<KhalaCodeDesktopRuntimeMode>([
  "claude_runtime",
  "codex_harness",
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

export const khalaCodeDesktopHarnessSettingPath = (env: ChatEnv): string =>
  env.KHALA_CODE_DESKTOP_HARNESS_SETTING_PATH?.trim() ||
  join(homedir(), ".khala-code", "desktop-settings.json")

export const khalaCodeDesktopRuntimeEnvOverride = (
  env: ChatEnv,
): KhalaCodeDesktopRuntimeMode | null => {
  if (env.KHALA_CODE_DESKTOP_RUNTIME === "claude_runtime") return "claude_runtime"
  if (env.KHALA_CODE_DESKTOP_RUNTIME === "codex_harness") return "codex_harness"
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
