// Ported Infinite Red Ignite component kit + theme, adapted to run standalone
// in khala-mobile (relative imports, i18n stub, a single fixed provider-free
// dark theme, host-loaded Space Grotesk fonts). See the individual files for
// per-module port notes.

export { Button } from "./components/Button"
export type { ButtonProps, ButtonAccessoryProps } from "./components/Button"
export { Card } from "./components/Card"
export { Header } from "./components/Header"
export type { HeaderProps } from "./components/Header"
export { Icon, PressableIcon } from "./components/Icon"
export type { IconTypes } from "./components/Icon"
export { ListItem } from "./components/ListItem"
export type { ListItemProps } from "./components/ListItem"
export { Screen } from "./components/Screen"
export type { ScreenProps } from "./components/Screen"
export { Text } from "./components/Text"
export type { TextProps } from "./components/Text"

export { colors } from "./theme/colorsDark"
export { spacing } from "./theme/spacing"
export { typography } from "./theme/typography"
export { $styles } from "./theme/styles"
export { theme, useAppTheme } from "./theme/context"
export type {
  Theme,
  ThemedStyle,
  ThemedStyleArray,
  AllowedStylesT,
  ThemedFnT,
} from "./theme/types"
export { useSafeAreaInsetsStyle } from "./utils/useSafeAreaInsetsStyle"
export type { ExtendedEdge } from "./utils/useSafeAreaInsetsStyle"
