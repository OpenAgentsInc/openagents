/**
 * OpenAgents Desktop theme (#8574, EP250 #8712) — the one Protoss-blue
 * OpenAgents theme, consumed directly from `@effect-native/tokens`.
 *
 * The app-local palette copy this file used to carry was drift (radius
 * lg 8 / xl 12 against the canonical 6/8, body 16 against the canonical
 * 14/21, off-palette colors). Per the file's own original contract —
 * "when the shared shell theme lands, this app-local copy is deleted in
 * favor of that export" — the canonical `khalaTheme` is now the single
 * source: one catalog, one theme, many hosts. Desktop must not drift into
 * a second visual identity, so this module deliberately re-exports the
 * shared theme instead of constructing one.
 */
import { khalaTheme } from "@effect-native/tokens"

export const openagentsDesktopTheme = khalaTheme
