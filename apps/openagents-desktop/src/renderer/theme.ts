/**
 * OpenAgents Desktop theme (#8574, EP250 #8712, Autopilot UI #8858) — the one
 * OpenAgents product theme, consumed directly from `@effect-native/tokens`.
 *
 * Since 2026-07-15 the canonical product theme is `autopilotTheme` (the
 * Autopilot UI tactical-instrument palette: indigo #5262fd accent on a
 * near-black #16161e canvas, square corners, muted danger), superseding the
 * Protoss-blue `khalaTheme`, which stays exported upstream for
 * explicitly-historical surfaces only. The one-theme-many-hosts invariant is
 * unchanged: one catalog, one theme, many hosts. Desktop must not drift into
 * a second visual identity, so this module deliberately re-exports the
 * shared theme instead of constructing one.
 */
import { autopilotTheme } from "@effect-native/tokens"

export const openagentsDesktopTheme = autopilotTheme
