import { StyleSheet, type TextStyle, type ViewStyle } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaScrollToLatestButtonProps = Readonly<{
  onPress: () => void
}>

/** Floating "jump to latest" affordance. Presentation uses the Khala token
 * surface directly so this tiny control does not pull the full Ignite `Screen`
 * barrel into Storybook just to render a glyph. */
export const KhalaScrollToLatestButton = ({ onPress }: KhalaScrollToLatestButtonProps) => {
  return (
    <TouchableFeedback
      accessibilityLabel="Scroll to latest"
      accessibilityRole="button"
      style={styles.button}
      onPress={onPress}
    >
      <KhalaText style={styles.glyph}>↓</KhalaText>
    </TouchableFeedback>
  )
}

const styles = StyleSheet.create({
  button: {
    alignItems: "center",
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.border,
    borderRadius: 24,
    borderWidth: 1,
    height: 48,
    justifyContent: "center",
    width: 48,
  } satisfies ViewStyle,
  glyph: {
    color: khalaMobileTheme.text,
    fontSize: 30,
    lineHeight: 32,
  } satisfies TextStyle,
})
