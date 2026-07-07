import type { StyleProp } from "react-native"

import { colors } from "./colorsDark"
import { spacing } from "./spacing"
import { typography } from "./typography"
import type { AllowedStylesT, Theme, ThemedFnT, ThemedStyle } from "./types"

/**
 * A provider-free adaptation of Ignite's `ThemeProvider`/`useAppTheme`. The
 * upstream version reads system color scheme + an MMKV override to switch
 * between light and dark themes and pushes a React context. This port pins a
 * single fixed DARK theme (Ignite's own `colorsDark`) — the owner wants to see
 * the REAL Ignite look — so there is no provider, no MMKV, and no
 * `@react-navigation` theme. `useAppTheme()` returns a module constant, so any
 * ported component can call it without a wrapping provider.
 */

export const theme: Theme = {
  colors,
  spacing,
  typography,
  isDark: true,
}

/**
 * Resolves a `ThemedStyle` function / a plain style / an (arbitrarily nested)
 * array of either against the fixed theme, then flattens the results into a
 * single style object — mirroring Ignite's `themed` helper exactly.
 */
const themed: ThemedFnT = <T,>(styleOrStyleFn: AllowedStylesT<T>): T => {
  const flatStyles = [styleOrStyleFn].flat(3) as (ThemedStyle<T> | StyleProp<T>)[]
  const stylesArray = flatStyles.map((f) => {
    if (typeof f === "function") {
      return (f as ThemedStyle<T>)(theme)
    }
    return f
  })
  // Flatten the array of styles into a single object
  return Object.assign({}, ...stylesArray) as T
}

const appTheme = {
  theme,
  themeContext: "dark" as const,
  themed,
}

/**
 * The primary hook used to access the (fixed dark) theme in ported components.
 * Provider-free: returns a module constant.
 */
export const useAppTheme = () => appTheme
