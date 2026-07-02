import { mkdir, readFile, writeFile } from "node:fs/promises"
import { homedir } from "node:os"
import { dirname, join } from "node:path"

import type { KhalaCodeDesktopRuntimeMode } from "../shared/rpc.js"

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

const DEFAULT_HARNESS_MODE: KhalaCodeDesktopRuntimeMode = "codex_harness"
const VALID_HARNESS_MODES = new Set<KhalaCodeDesktopRuntimeMode>([
  "claude_runtime",
  "codex_harness",
  "khala_native_runtime",
])

const isRuntimeMode = (value: unknown): value is KhalaCodeDesktopRuntimeMode =>
  typeof value === "string" && VALID_HARNESS_MODES.has(value as KhalaCodeDesktopRuntimeMode)

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
  await mkdir(dirname(path), { recursive: true })
  await writeFile(path, `${JSON.stringify({
    schema: "khala-code-desktop.harness-setting.v1",
    harnessMode: mode,
  }, null, 2)}\n`)
  const setting = await readKhalaCodeDesktopHarnessSetting(env)
  return {
    ...setting,
    saved: true,
  }
}
