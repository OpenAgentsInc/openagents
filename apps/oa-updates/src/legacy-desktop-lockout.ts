/**
 * Legacy desktop client lockout (CUT-26, openagents#8706).
 *
 * The deprecated Electrobun desktop clients — `khala-code-desktop` and
 * `autopilot-desktop` — are frozen migration sources with no release lane
 * (docs/DEPLOYMENT.md: "DEPRECATED/FROZEN: DO NOT RELEASE"). Publishing new
 * releases for them is already refused at the seed/publish boundary
 * (`assertDesktopReleaseProductPublishable`). This module closes the OTHER
 * half: the SERVING boundary. When the lockout is armed, every legacy
 * desktop distribution/update surface this service owns answers with one
 * typed `410 Gone` lockout document instead of feed or artifact content, so
 * the deprecated Khala Code desktop client can never fetch an update,
 * (re)install payload, or freshness signal from our infrastructure again —
 * it cannot be sustained as a live coding surface from this authority
 * post-cutover.
 *
 * Enforcement boundary (honest scope): this service is the distribution and
 * update authority for the legacy clients, and the ONLY owned server surface
 * on which those frozen clients identify themselves (their updater polls
 * product-scoped URLs). An already-installed legacy binary still executes
 * locally; the frozen clients carry no remote kill-switch and adding one
 * would be a new feature in a deprecated client, which is forbidden. The
 * CUT-27 declaration docs-flip states the support posture; this module is
 * the mechanical refusal that backs it.
 *
 * Fail-closed configuration: the lockout is ARMED by default. Only the exact
 * value `disarmed-historical-read-only` in `OA_LEGACY_DESKTOP_LOCKOUT`
 * re-enables historical read-only serving (archival inspection). Any other
 * value — typos included — stays armed.
 */

export const LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID =
  "openagents.desktop.legacy_lockout.v1" as const

/** Legacy desktop products whose serving surfaces are locked out. */
export const LEGACY_LOCKED_DESKTOP_PRODUCTS = [
  "autopilot-desktop",
  "khala-code-desktop",
] as const
export type LegacyLockedDesktopProduct =
  (typeof LEGACY_LOCKED_DESKTOP_PRODUCTS)[number]

/**
 * The flat Electrobun OTA route (`/desktop/<filename>`) carries no product
 * segment, so its lockout document names this surface marker instead of a
 * single product.
 */
export const LEGACY_DESKTOP_OTA_SURFACE = "legacy-electrobun-desktop-ota" as const

export type LegacyDesktopLockoutSubject =
  | LegacyLockedDesktopProduct
  | typeof LEGACY_DESKTOP_OTA_SURFACE

export type LegacyDesktopLockoutMode =
  | "armed"
  | "disarmed_historical_read_only"

export const LEGACY_DESKTOP_LOCKOUT_ENV = "OA_LEGACY_DESKTOP_LOCKOUT"
const DISARM_VALUE = "disarmed-historical-read-only"

/**
 * Resolve the lockout mode from the raw env value. Fail closed: everything
 * except the exact documented disarm value is ARMED.
 */
export function resolveLegacyDesktopLockoutMode(
  envValue: string | undefined,
): LegacyDesktopLockoutMode {
  return envValue?.trim() === DISARM_VALUE
    ? "disarmed_historical_read_only"
    : "armed"
}

export function isLegacyLockedDesktopProduct(
  value: string,
): value is LegacyLockedDesktopProduct {
  return (LEGACY_LOCKED_DESKTOP_PRODUCTS as readonly string[]).includes(value)
}

export type LegacyDesktopLockoutBody = {
  readonly schema: typeof LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID
  readonly subject: LegacyDesktopLockoutSubject
  readonly lockedOut: true
  readonly reason: "legacy_desktop_client_locked_out"
  readonly detail: string
  readonly replacementApp: "openagents-desktop"
  readonly replacementFeedPath: string
  readonly policyRef: "openagents#8706"
}

export function legacyDesktopLockoutBody(
  subject: LegacyDesktopLockoutSubject,
): LegacyDesktopLockoutBody {
  return {
    schema: LEGACY_DESKTOP_LOCKOUT_SCHEMA_ID,
    subject,
    lockedOut: true,
    reason: "legacy_desktop_client_locked_out",
    detail:
      "This deprecated desktop client is retired and is not a supported " +
      "coding surface. Its update/distribution feed is permanently closed. " +
      "Install OpenAgents Desktop instead.",
    replacementApp: "openagents-desktop",
    replacementFeedPath: "/desktop/openagents-desktop/stable/feed.json",
    policyRef: "openagents#8706",
  }
}

/** One typed `410 Gone` response for every locked legacy surface. */
export function legacyDesktopLockoutResponse(
  subject: LegacyDesktopLockoutSubject,
): Response {
  return Response.json(legacyDesktopLockoutBody(subject), {
    status: 410,
    headers: {
      "cache-control": "no-store",
      "content-type": "application/json",
    },
  })
}
