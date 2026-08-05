/**
 * Local swap-surface log with a local export path (SWAP-7, #9322).
 *
 * Boltz ships a third-party support widget with a "send my logs" upload. We
 * keep the local log export and drop the widget: no third-party script loads
 * on a surface that will hold key material, and the log never leaves the
 * device unless the user downloads and shares it themselves.
 *
 * MKT-SWP §14 forbidden-material rule is binding here: entries are produced
 * only by this surface and must never contain key material, preimages, or
 * secret-store contents. Callers log event names and bounded public detail.
 */
import { buildCommit } from '@/lib/build-provenance'

import {
  loadSwapSettings,
  type SwapSettings,
  type SwapSettingsStorage,
} from './settings'

export const SWAP_LOG_STORAGE_KEY = 'openagents.swap.log.v1'
export const SWAP_LOG_SCHEMA_VERSION = 1
export const SWAP_LOG_MAX_ENTRIES = 200

export type SwapLogEntry = Readonly<{
  at: string
  event: string
  detail?: Readonly<Record<string, string | number | boolean>>
}>

export const readSwapLog = (
  storage: SwapSettingsStorage,
): ReadonlyArray<SwapLogEntry> => {
  try {
    const raw = storage.getItem(SWAP_LOG_STORAGE_KEY)
    if (raw === null) return []
    const parsed: unknown = JSON.parse(raw)
    if (typeof parsed !== 'object' || parsed === null) return []
    const record = parsed as Record<string, unknown>
    if (record['schemaVersion'] !== SWAP_LOG_SCHEMA_VERSION) return []
    const entries = record['entries']
    if (!Array.isArray(entries)) return []
    return entries.filter(
      (entry): entry is SwapLogEntry =>
        typeof entry === 'object' &&
        entry !== null &&
        typeof (entry as Record<string, unknown>)['at'] === 'string' &&
        typeof (entry as Record<string, unknown>)['event'] === 'string',
    )
  } catch {
    return []
  }
}

export const appendSwapLog = (
  storage: SwapSettingsStorage,
  event: string,
  detail?: Readonly<Record<string, string | number | boolean>>,
): void => {
  const entry: SwapLogEntry = {
    at: new Date().toISOString(),
    event,
    ...(detail === undefined ? {} : { detail }),
  }
  const entries = [...readSwapLog(storage), entry].slice(-SWAP_LOG_MAX_ENTRIES)
  try {
    storage.setItem(
      SWAP_LOG_STORAGE_KEY,
      JSON.stringify({ schemaVersion: SWAP_LOG_SCHEMA_VERSION, entries }),
    )
  } catch {
    // Storage unavailable: the log is best-effort local diagnostics only.
  }
}

export type SwapLogBundle = Readonly<{
  format: 'openagents.swap.log_export.v1'
  exportedAt: string
  buildCommit: string
  settings: SwapSettings
  entries: ReadonlyArray<SwapLogEntry>
}>

/** The complete local diagnostic bundle a user can download and inspect. */
export const buildSwapLogBundle = (
  storage: SwapSettingsStorage,
  now: () => Date = () => new Date(),
): SwapLogBundle => ({
  format: 'openagents.swap.log_export.v1',
  exportedAt: now().toISOString(),
  buildCommit: buildCommit(),
  settings: loadSwapSettings(storage),
  entries: readSwapLog(storage),
})

export const swapLogBundleJson = (bundle: SwapLogBundle): string =>
  JSON.stringify(bundle, null, 2)
