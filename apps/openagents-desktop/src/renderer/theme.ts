/**
 * OpenAgents Desktop theme (#8574, EP250 #8712) — the one Protoss-blue
 * OpenAgents theme, consumed directly from `@effect-native/tokens`.
 *
 * The owner restored the blue Khala palette on 2026-07-16 after the temporary
 * Autopilot palette made the workroom read gray. The one-theme-many-hosts
 * invariant remains unchanged: one catalog, one theme, many hosts. Autopilot's
 * condensed/mono instrumentation and compatible color relationships fold into
 * Khala semantic roles; they never mount a competing theme. Desktop deliberately
 * re-exports the shared theme instead of constructing one.
 */
import { khalaTheme } from "@effect-native/tokens"

export const openagentsDesktopTheme = khalaTheme
