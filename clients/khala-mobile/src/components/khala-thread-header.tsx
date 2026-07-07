import { StyleSheet, View, type TextStyle, type ViewStyle } from "react-native"

import { khalaMobileTheme } from "../theme/tokens"
import { KhalaText } from "./khala-text"
import { TouchableFeedback } from "./touchable-feedback"

export type KhalaThreadHeaderProps = Readonly<{
  /** Opens the drawer flyout menu (owner report, 2026-07-07: the chat header's
   * left button is the hamburger nav that opens the drawer — not a broken
   * back chevron). Wired to `navigation.getParent()?.openDrawer()` by the
   * thread screen; the drawer is where the nav items and credit balance live. */
  onOpenMenu: () => void
  /** One-tap "start a fresh thread" action. Always rendered so it stays a
   * reachable escape hatch even while a turn is in flight (owner report,
   * 2026-07-06: "no way to start a new thread ... cant do anything"). When
   * `undefined` (the sync runtime hasn't opened yet) the button is shown
   * disabled rather than hidden, so the affordance never disappears. */
  onNewThread?: () => void
  subtitle: string
  title: string
}>

/** Thread view's top bar. Presentation uses Khala tokens directly so the
 * header can render in Storybook without importing Ignite's native Screen
 * dependencies through the component barrel. */
export const KhalaThreadHeader = ({
  onOpenMenu,
  onNewThread,
  subtitle,
  title,
}: KhalaThreadHeaderProps) => {
  const newDisabled = onNewThread === undefined
  return (
    <View style={styles.container}>
      <View style={styles.row}>
        <TouchableFeedback
          accessibilityLabel="Open menu"
          accessibilityRole="button"
          style={styles.circleButton}
          hitSlop={10}
          onPress={onOpenMenu}
        >
          <KhalaText style={styles.menuGlyph}>☰</KhalaText>
        </TouchableFeedback>

        <View style={styles.titleColumn}>
          <KhalaText className="font-medium" numberOfLines={1} variant="body">
            {title}
          </KhalaText>
          <KhalaText numberOfLines={1} style={styles.subtitle} variant="muted">
            {subtitle}
          </KhalaText>
        </View>

        <TouchableFeedback
          accessibilityLabel="New thread"
          accessibilityRole="button"
          style={[styles.newButton, newDisabled ? styles.newButtonDisabled : styles.newButtonEnabled]}
          disabled={newDisabled}
          hitSlop={10}
          onPress={onNewThread}
        >
          <KhalaText style={[styles.newGlyph, newDisabled ? styles.newTextDisabled : styles.newTextEnabled]}>✎</KhalaText>
          <KhalaText
            className="font-medium uppercase"
            style={[styles.newLabel, newDisabled ? styles.newTextDisabled : styles.newTextEnabled]}
          >
            New
          </KhalaText>
        </TouchableFeedback>
      </View>
    </View>
  )
}

const styles = StyleSheet.create({
  circleButton: {
    alignItems: "center",
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.border,
    borderRadius: 28,
    borderWidth: 1,
    height: 56,
    justifyContent: "center",
    width: 56,
  } satisfies ViewStyle,
  container: {
    paddingBottom: 12,
    paddingHorizontal: 16,
    paddingTop: 8,
  } satisfies ViewStyle,
  menuGlyph: {
    color: khalaMobileTheme.text,
    fontSize: 26,
    lineHeight: 30,
  } satisfies TextStyle,
  newButton: {
    alignItems: "center",
    borderRadius: 28,
    borderWidth: 1,
    flexDirection: "row",
    gap: 8,
    height: 56,
    paddingHorizontal: 16,
  } satisfies ViewStyle,
  newButtonDisabled: {
    backgroundColor: khalaMobileTheme.background,
    borderColor: khalaMobileTheme.border,
  } satisfies ViewStyle,
  newButtonEnabled: {
    backgroundColor: khalaMobileTheme.surfaceRaised,
    borderColor: khalaMobileTheme.accent,
  } satisfies ViewStyle,
  newGlyph: {
    fontSize: 22,
    lineHeight: 24,
  } satisfies TextStyle,
  newLabel: {
    letterSpacing: 0.5,
  } satisfies TextStyle,
  newTextDisabled: {
    color: khalaMobileTheme.textMuted,
  } satisfies TextStyle,
  newTextEnabled: {
    color: khalaMobileTheme.accent,
  } satisfies TextStyle,
  row: {
    alignItems: "center",
    flexDirection: "row",
    gap: 12,
  } satisfies ViewStyle,
  subtitle: {
    color: khalaMobileTheme.textMuted,
  } satisfies TextStyle,
  titleColumn: {
    flex: 1,
    minWidth: 0,
  } satisfies ViewStyle,
})
