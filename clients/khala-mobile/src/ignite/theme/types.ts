import type { StyleProp } from "react-native"

import { colors } from "./colorsDark"
import { spacing } from "./spacing"
import { typography } from "./typography"

// This port uses a single fixed dark theme (Ignite's own `colorsDark`), so the
// Colors/Spacing/Typography types resolve to those single sources rather than a
// light-or-dark union.
export type Colors = typeof colors
export type Spacing = typeof spacing
export type Typography = typeof typography

// The overall Theme object should contain all of the data you need to style your app.
export interface Theme {
  colors: Colors
  spacing: Spacing
  typography: Typography
  isDark: boolean
}

/**
 * Represents a function that returns a styled component based on the provided theme.
 * @template T The type of the style.
 * @param theme The theme object.
 * @returns The styled component.
 *
 * @example
 * const $container: ThemedStyle<ViewStyle> = (theme) => ({
 *   flex: 1,
 *   backgroundColor: theme.colors.background,
 * })
 * // Then use in a component like so:
 * const Component = () => {
 *   const { themed } = useAppTheme()
 *   return <View style={themed($container)} />
 * }
 */
export type ThemedStyle<T> = (theme: Theme) => T
export type ThemedStyleArray<T> = (
  | ThemedStyle<T>
  | StyleProp<T>
  | (StyleProp<T> | ThemedStyle<T>)[]
)[]

export type AllowedStylesT<T> = ThemedStyle<T> | StyleProp<T> | ThemedStyleArray<T>
export type ThemedFnT = <T>(styleOrStyleFn: AllowedStylesT<T>) => T
