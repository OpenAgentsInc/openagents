import { View, type TextStyle, type ViewStyle } from "react-native"

import { Text, useAppTheme } from "../ignite"
import type { ThemedStyle } from "../ignite"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaThreadHeaderProps = Readonly<{
  onBack: () => void
  /** One-tap "start a fresh thread" action. Always rendered so it stays a
   * reachable escape hatch even while a turn is in flight (owner report,
   * 2026-07-06: "no way to start a new thread ... cant do anything"). When
   * `undefined` (the sync runtime hasn't opened yet) the button is shown
   * disabled rather than hidden, so the affordance never disappears. */
  onNewThread?: () => void
  subtitle: string
  title: string
}>

/** Thread view's top bar. Presentation is on the ported Infinite Red Ignite
 * `Text` primitive + theme tokens (`../ignite`); the UI-thread press
 * cross-fade stays on the arcade `TouchableFeedback`. */
export const KhalaThreadHeader = ({
  onBack,
  onNewThread,
  subtitle,
  title,
}: KhalaThreadHeaderProps) => {
  const { theme, themed } = useAppTheme()
  const newDisabled = onNewThread === undefined
  return (
    <View style={themed($container)}>
      <View style={themed($row)}>
        <TouchableFeedback
          accessibilityLabel="Back"
          accessibilityRole="button"
          style={themed($circleButton)}
          hitSlop={10}
          onPress={onBack}
        >
          <Text style={[$backGlyph, { color: theme.colors.text }]}>‹</Text>
        </TouchableFeedback>

        <View style={themed($titleColumn)}>
          <Text weight="medium" size="md" numberOfLines={1} text={title} />
          <Text size="sm" numberOfLines={1} style={themed($subtitle)} text={subtitle} />
        </View>

        <TouchableFeedback
          accessibilityLabel="New thread"
          accessibilityRole="button"
          style={[themed($newButton), newDisabled ? themed($newButtonDisabled) : themed($newButtonEnabled)]}
          disabled={newDisabled}
          hitSlop={10}
          onPress={onNewThread}
        >
          <Text style={[$newGlyph, { color: newDisabled ? theme.colors.textDim : theme.colors.tint }]}>✎</Text>
          <Text
            size="xxs"
            weight="medium"
            style={{
              color: newDisabled ? theme.colors.textDim : theme.colors.tint,
              textTransform: "uppercase",
              letterSpacing: 0.5,
            }}
            text="New"
          />
        </TouchableFeedback>
      </View>
    </View>
  )
}

const $container: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  paddingHorizontal: spacing.md,
  paddingTop: spacing.xs,
  paddingBottom: spacing.sm,
})

const $row: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.sm,
})

const $circleButton: ThemedStyle<ViewStyle> = ({ colors }) => ({
  height: 56,
  width: 56,
  alignItems: "center",
  justifyContent: "center",
  borderRadius: 28,
  borderWidth: 1,
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.palette.neutral200,
})

const $titleColumn: ThemedStyle<ViewStyle> = () => ({
  flex: 1,
  minWidth: 0,
})

const $newButton: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  height: 56,
  flexDirection: "row",
  alignItems: "center",
  gap: spacing.xs,
  borderRadius: 28,
  borderWidth: 1,
  paddingHorizontal: spacing.md,
})

const $newButtonEnabled: ThemedStyle<ViewStyle> = ({ colors }) => ({
  borderColor: colors.tint,
  backgroundColor: colors.palette.neutral200,
})

const $newButtonDisabled: ThemedStyle<ViewStyle> = ({ colors }) => ({
  borderColor: colors.palette.neutral400,
  backgroundColor: colors.background,
})

const $subtitle: ThemedStyle<TextStyle> = ({ colors }) => ({ color: colors.textDim })

const $backGlyph: TextStyle = { fontSize: 36, lineHeight: 40 }
const $newGlyph: TextStyle = { fontSize: 22, lineHeight: 24 }
