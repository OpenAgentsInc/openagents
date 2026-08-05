/**
 * Locally persisted swap-surface settings (SWAP-7, #9322).
 *
 * Scope follows the issue exactly: denomination, decimal separator, privacy
 * mode, relay selection, and locale. Slippage and gas top-up are token-route
 * concepts MKT-SWP v1 does not have. Relay selection has no Boltz analogue —
 * a market on relays means the user picks which relays they read and write.
 *
 * Settings live in browser localStorage only; nothing here is transmitted
 * anywhere. The store is schema-versioned from the first commit (rollout
 * plan §4.1): an unrecognized or corrupt payload falls back to defaults
 * instead of guessing.
 */
import {
  DEFAULT_LOCALE,
  SUPPORTED_LOCALES,
  type SupportedLocale,
} from '@openagentsinc/swap-i18n'

export const SWAP_SETTINGS_STORAGE_KEY = 'openagents.swap.settings.v1'
export const SWAP_SETTINGS_SCHEMA_VERSION = 1

export type SwapDenomination = 'sats' | 'btc'
export type SwapDecimalSeparator = '.' | ','

export type SwapRelayEntry = Readonly<{
  url: string
  read: boolean
  write: boolean
}>

export type SwapSettings = Readonly<{
  schemaVersion: typeof SWAP_SETTINGS_SCHEMA_VERSION
  denomination: SwapDenomination
  decimalSeparator: SwapDecimalSeparator
  /** Privacy mode hides amounts on swap surfaces that render them. */
  privacyMode: boolean
  locale: SupportedLocale
  /** The relays this browser reads market state from and writes events to. */
  relays: ReadonlyArray<SwapRelayEntry>
}>

export const DEFAULT_SWAP_RELAY_URL = 'wss://relay.openagents.com'

export const defaultSwapSettings = (): SwapSettings => ({
  schemaVersion: SWAP_SETTINGS_SCHEMA_VERSION,
  denomination: 'sats',
  decimalSeparator: '.',
  privacyMode: false,
  locale: DEFAULT_LOCALE,
  relays: [{ url: DEFAULT_SWAP_RELAY_URL, read: true, write: true }],
})

const isLoopbackHost = (host: string): boolean =>
  host === 'localhost' || host === '127.0.0.1' || host === '[::1]'

/**
 * A relay URL must be a well-formed websocket URL. Plaintext `ws:` is
 * admitted only for loopback development relays; anything reachable over a
 * network must be `wss:`.
 */
export const validateSwapRelayUrl = (
  candidate: string,
): { ok: true; url: string } | { ok: false; reason: 'invalid_url' | 'insecure_scheme' } => {
  let parsed: URL
  try {
    parsed = new URL(candidate.trim())
  } catch {
    return { ok: false, reason: 'invalid_url' }
  }
  if (parsed.protocol === 'wss:') return { ok: true, url: parsed.toString() }
  if (parsed.protocol === 'ws:' && isLoopbackHost(parsed.hostname)) {
    return { ok: true, url: parsed.toString() }
  }
  return {
    ok: false,
    reason: parsed.protocol === 'ws:' ? 'insecure_scheme' : 'invalid_url',
  }
}

const isDenomination = (value: unknown): value is SwapDenomination =>
  value === 'sats' || value === 'btc'

const isSeparator = (value: unknown): value is SwapDecimalSeparator =>
  value === '.' || value === ','

const isSupportedLocale = (value: unknown): value is SupportedLocale =>
  typeof value === 'string' &&
  (SUPPORTED_LOCALES as ReadonlyArray<string>).includes(value)

const parseRelayEntry = (value: unknown): SwapRelayEntry | undefined => {
  if (typeof value !== 'object' || value === null) return undefined
  const record = value as Record<string, unknown>
  if (typeof record['url'] !== 'string') return undefined
  const validated = validateSwapRelayUrl(record['url'])
  if (!validated.ok) return undefined
  return {
    url: validated.url,
    read: record['read'] === true,
    write: record['write'] === true,
  }
}

/** Decode a stored payload. Anything unrecognized falls back to defaults. */
export const decodeSwapSettings = (raw: string | null): SwapSettings => {
  const defaults = defaultSwapSettings()
  if (raw === null) return defaults
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return defaults
  }
  if (typeof parsed !== 'object' || parsed === null) return defaults
  const record = parsed as Record<string, unknown>
  if (record['schemaVersion'] !== SWAP_SETTINGS_SCHEMA_VERSION) return defaults
  const relays = Array.isArray(record['relays'])
    ? record['relays']
        .map(parseRelayEntry)
        .filter((entry): entry is SwapRelayEntry => entry !== undefined)
    : []
  return {
    schemaVersion: SWAP_SETTINGS_SCHEMA_VERSION,
    denomination: isDenomination(record['denomination'])
      ? record['denomination']
      : defaults.denomination,
    decimalSeparator: isSeparator(record['decimalSeparator'])
      ? record['decimalSeparator']
      : defaults.decimalSeparator,
    privacyMode: record['privacyMode'] === true,
    locale: isSupportedLocale(record['locale'])
      ? record['locale']
      : defaults.locale,
    relays: relays.length > 0 ? relays : defaults.relays,
  }
}

export type SwapSettingsStorage = Pick<Storage, 'getItem' | 'setItem'>

export const loadSwapSettings = (storage: SwapSettingsStorage): SwapSettings => {
  try {
    return decodeSwapSettings(storage.getItem(SWAP_SETTINGS_STORAGE_KEY))
  } catch {
    return defaultSwapSettings()
  }
}

export const saveSwapSettings = (
  storage: SwapSettingsStorage,
  settings: SwapSettings,
): void => {
  try {
    storage.setItem(SWAP_SETTINGS_STORAGE_KEY, JSON.stringify(settings))
  } catch {
    // Storage may be unavailable (private mode, quota). Settings remain
    // in-memory for the session; nothing else depends on the write.
  }
}
