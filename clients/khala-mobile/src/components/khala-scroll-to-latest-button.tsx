import type { TextStyle, ViewStyle } from "react-native"

import { Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaScrollToLatestButtonProps = Readonly<{
  onPress: () => void
}>

/** Floating "jump to latest" affordance. Presentation is on the ported
 * Infinite Red Ignite `Text` primitive + theme tokens (`../ignite`); the
 * UI-thread press cross-fade stays on the arcade `TouchableFeedback`. */
export const KhalaScrollToLatestButton = ({ onPress }: KhalaScrollToLatestButtonProps) => {
  const { theme, themed } = useAppTheme()
  return (
    <TouchableFeedback
      accessibilityLabel="Scroll to latest"
      accessibilityRole="button"
      style={themed($button)}
      onPress={onPress}
    >
      <Text style={[$glyph, { color: theme.colors.text }]}>↓</Text>
    </TouchableFeedback>
  )
}

const $button: ThemedStyle<ViewStyle> = ({ colors }) => ({
  height: 48,
  width: 48,
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 24,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral200,
})

const $glyph: TextStyle = { fontSize: 30, lineHeight: 32 }
