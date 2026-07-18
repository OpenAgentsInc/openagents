/**
 * Desktop update feed configuration (REL-FEED-01, #8993).
 *
 * One typed, fail-closed resolver for WHERE a packaged Desktop discovers its
 * signed ReleaseSet v2 feed and WHICH Ed25519 key it pins. The default —
 * no environment overrides — is byte-identical to the historical behavior:
 * the production `updates.openagents.com` origin with the committed
 * PRODUCTION release key pin. Nothing here weakens the trust boundary; the
 * signature (never the host/TLS) remains the only authority, and every
 * override is explicit, bounded, and refused on the slightest ambiguity.
 *
 * Staging-channel proof seam (`GUARANTEES.md` "automatic update delivery"):
 * an explicitly staged Desktop build may point at a NON-production feed host
 * (for example a Cloud Run staging deploy of `apps/oa-updates`, or a local
 * feed instance in the e2e proof) via:
 *
 *   OPENAGENTS_DESKTOP_UPDATE_FEED_BASE_URL   — feed host base (origin, with
 *     an optional path prefix). The channel path `/desktop/openagents/<ch>`
 *     is always appended by this resolver; the env var can never smuggle a
 *     different route shape, query, fragment, or credentials.
 *   OPENAGENTS_DESKTOP_UPDATE_FEED_STAGING_PIN — JSON `PinnedReleaseKey`
 *     (PUBLIC material only) used INSTEAD of the production pin, and only
 *     when the base-URL override is also present and does not point at the
 *     production feed host. A staging pin can therefore never re-key the
 *     production feed, and the production kid is reserved (a "staging" pin
 *     claiming the production kid is refused — key ids are never reused for
 *     different material, per the release-signing runbook).
 *
 * Fail-closed disposition: any invalid override yields a typed refusal. The
 * caller (Electron main) responds by disabling update checks entirely —
 * there is NO silent fallback to production with a half-applied override.
 */
import { Exit, Schema } from "effect"

import {
  PRODUCTION_RELEASE_KEY_PIN,
  PinnedReleaseKeySchema,
  type PinnedReleaseKey,
  type UpdateChannel,
} from "./update-contract.ts"

export const DESKTOP_UPDATE_FEED_BASE_URL_ENV = "OPENAGENTS_DESKTOP_UPDATE_FEED_BASE_URL" as const
export const DESKTOP_UPDATE_FEED_STAGING_PIN_ENV = "OPENAGENTS_DESKTOP_UPDATE_FEED_STAGING_PIN" as const

/** The one production feed host. A staging pin may never apply to it. */
export const PRODUCTION_UPDATE_FEED_ORIGIN = "https://updates.openagents.com" as const

const MAX_BASE_URL_LENGTH = 1024
const MAX_PIN_JSON_LENGTH = 4096

/** Loopback hosts where plain-HTTP staging (local feed instance) is allowed. */
const LOOPBACK_HOSTNAMES = new Set(["127.0.0.1", "[::1]", "localhost"])

export const desktopUpdateFeedConfigFailures = [
  "feed_base_url_invalid",
  "staging_pin_invalid",
  "staging_pin_requires_base_override",
  "staging_pin_on_production_feed",
  "staging_pin_kid_reserved",
] as const
export type DesktopUpdateFeedConfigFailure = (typeof desktopUpdateFeedConfigFailures)[number]

export type DesktopUpdateFeedResolution =
  | {
    readonly ok: true
    readonly source: "production" | "staging_override"
    /** Full channel base, e.g. `https://updates.openagents.com/desktop/openagents/rc`. */
    readonly baseUrl: string
    readonly pin: PinnedReleaseKey
  }
  | { readonly ok: false; readonly reason: DesktopUpdateFeedConfigFailure }

const readEnvValue = (
  env: Readonly<Record<string, string | undefined>>,
  name: string,
): string | null => {
  const value = env[name]
  if (value === undefined) return null
  const trimmed = value.trim()
  return trimmed.length === 0 ? null : trimmed
}

const parseFeedBaseOverride = (raw: string): URL | null => {
  if (raw.length > MAX_BASE_URL_LENGTH) return null
  let url: URL
  try {
    url = new URL(raw)
  } catch {
    return null
  }
  if (url.username !== "" || url.password !== "" || url.search !== "" || url.hash !== "") {
    return null
  }
  if (url.protocol === "https:") return url
  // Plain HTTP is admissible ONLY toward loopback — the local staging feed
  // instance in the e2e proof. A cleartext non-loopback feed is refused.
  if (url.protocol === "http:" && LOOPBACK_HOSTNAMES.has(url.hostname)) return url
  return null
}

const channelBase = (url: URL, channel: UpdateChannel): string => {
  const prefix = url.pathname.replace(/\/+$/, "")
  return `${url.origin}${prefix}/desktop/openagents/${channel}`
}

const PIN_FIELDS = new Set(["alg", "kid", "x"])

const decodeStagingPin = (raw: string): PinnedReleaseKey | null => {
  if (raw.length > MAX_PIN_JSON_LENGTH) return null
  let parsed: unknown
  try {
    parsed = JSON.parse(raw)
  } catch {
    return null
  }
  // Refuse ANY extra field explicitly — a pasted private JWK (with `d`) must
  // never be accepted-and-stripped into a working pin.
  if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return null
  if (!Object.keys(parsed).every((key) => PIN_FIELDS.has(key))) return null
  const result = Schema.decodeUnknownExit(PinnedReleaseKeySchema)(parsed)
  return Exit.isSuccess(result) ? result.value : null
}

/**
 * Resolve the update feed for one channel from the process environment.
 * Pure — the caller injects `process.env` (tests inject fixtures).
 */
export const resolveDesktopUpdateFeedConfig = (
  env: Readonly<Record<string, string | undefined>>,
  channel: UpdateChannel,
): DesktopUpdateFeedResolution => {
  const baseOverrideRaw = readEnvValue(env, DESKTOP_UPDATE_FEED_BASE_URL_ENV)
  const stagingPinRaw = readEnvValue(env, DESKTOP_UPDATE_FEED_STAGING_PIN_ENV)

  if (baseOverrideRaw === null) {
    // A staging pin without a base override could only mean "re-pin the
    // production feed" — refused unconditionally.
    if (stagingPinRaw !== null) return { ok: false, reason: "staging_pin_requires_base_override" }
    return {
      ok: true,
      source: "production",
      baseUrl: `${PRODUCTION_UPDATE_FEED_ORIGIN}/desktop/openagents/${channel}`,
      pin: PRODUCTION_RELEASE_KEY_PIN,
    }
  }

  const baseOverride = parseFeedBaseOverride(baseOverrideRaw)
  if (baseOverride === null) return { ok: false, reason: "feed_base_url_invalid" }

  if (stagingPinRaw === null) {
    // Base override with the production pin retained: a pre-promotion mirror
    // serving production-signed sets. Trust is unchanged (production key).
    return {
      ok: true,
      source: "staging_override",
      baseUrl: channelBase(baseOverride, channel),
      pin: PRODUCTION_RELEASE_KEY_PIN,
    }
  }

  if (baseOverride.origin === PRODUCTION_UPDATE_FEED_ORIGIN) {
    return { ok: false, reason: "staging_pin_on_production_feed" }
  }
  const pin = decodeStagingPin(stagingPinRaw)
  if (pin === null) return { ok: false, reason: "staging_pin_invalid" }
  if (pin.kid === PRODUCTION_RELEASE_KEY_PIN.kid) {
    return { ok: false, reason: "staging_pin_kid_reserved" }
  }
  return { ok: true, source: "staging_override", baseUrl: channelBase(baseOverride, channel), pin }
}
