import { ActivityIndicator, StyleProp, TextStyle, View, ViewStyle } from "react-native"

import { useAppTheme } from "../theme/context"
import type { ThemedStyle } from "../theme/types"

import { Button, ButtonProps } from "./Button"
import { Text, TextProps } from "./Text"

/**
 * Ported from Ignite's `EmptyState` (`boilerplate/app/components/EmptyState.tsx`),
 * adapted to run standalone in khala-mobile: relative imports, the i18n stub
 * (no `tx`), the provider-free `useAppTheme`, and the sad-face `require()` image
 * dropped (the boilerplate ships an asset this app does not). Two additions this
 * app's screens genuinely need beyond the boilerplate shape are folded in as
 * first-class props so every empty/loading/error state on a screen is one
 * Ignite component:
 *
 * - `loading` renders a themed `ActivityIndicator` above the heading and marks
 *   the container `accessibilityRole="progressbar"` (the honest "still working"
 *   signal the old `KhalaEmptyState` carried, preserved verbatim).
 * - `status="error"` tints the heading + content with the theme error color for
 *   the "…unavailable" branches, without a second bespoke component.
 */
export interface EmptyStateProps {
  /** Style override for the container. */
  style?: StyleProp<ViewStyle>
  /** The heading text. */
  heading?: TextProps["text"]
  /** Style overrides for heading text. */
  headingStyle?: StyleProp<TextStyle>
  /** Pass any additional props directly to the heading Text component. */
  HeadingTextProps?: TextProps
  /** The content text. */
  content?: TextProps["text"]
  /** Style overrides for content text. */
  contentStyle?: StyleProp<TextStyle>
  /** Pass any additional props directly to the content Text component. */
  ContentTextProps?: TextProps
  /** The button text. */
  button?: TextProps["text"]
  /** Style overrides for button. */
  buttonStyle?: ButtonProps["style"]
  /** Style overrides for button text. */
  buttonTextStyle?: ButtonProps["textStyle"]
  /** Called when the button is pressed. */
  buttonOnPress?: ButtonProps["onPress"]
  /** Pass any additional props directly to the Button component. */
  ButtonProps?: ButtonProps
  /** When true, shows a themed spinner above the heading and marks the container as a progressbar. */
  loading?: boolean
  /** Tints the heading/content for an error branch. */
  status?: "error"
  /** Optional testID for the container. */
  testID?: string
}

/**
 * A component to use when there is no data to display, still loading, or in an
 * error branch. Composed entirely from the ported Ignite `Text`/`Button`.
 */
export function EmptyState(props: EmptyStateProps) {
  const {
    theme,
    themed,
    theme: { spacing },
  } = useAppTheme()

  const {
    button,
    buttonOnPress,
    content,
    heading,
    loading = false,
    status,
    style: $containerStyleOverride,
    buttonStyle: $buttonStyleOverride,
    buttonTextStyle: $buttonTextStyleOverride,
    contentStyle: $contentStyleOverride,
    headingStyle: $headingStyleOverride,
    ButtonProps,
    ContentTextProps,
    HeadingTextProps,
    testID,
  } = props

  const isHeadingPresent = !!heading
  const isContentPresent = !!content
  const isButtonPresent = !!button

  const $errorTint: TextStyle | false = status === "error" && { color: theme.colors.error }

  const $headingStyles = [
    themed($heading),
    loading && { marginTop: spacing.md },
    (isContentPresent || isButtonPresent) && { marginBottom: spacing.xxxs },
    $errorTint,
    $headingStyleOverride,
    HeadingTextProps?.style,
  ]
  const $contentStyles = [
    themed($content),
    isHeadingPresent && { marginTop: spacing.xxxs },
    isButtonPresent && { marginBottom: spacing.xxxs },
    status === "error" ? $errorTint : { color: theme.colors.textDim },
    $contentStyleOverride,
    ContentTextProps?.style,
  ]
  const $buttonStyles = [
    (isHeadingPresent || isContentPresent) && { marginTop: spacing.xl },
    $buttonStyleOverride,
    ButtonProps?.style,
  ]

  return (
    <View
      accessibilityRole={loading ? "progressbar" : "summary"}
      style={[$container, $containerStyleOverride]}
      testID={testID}
    >
      {loading && <ActivityIndicator color={theme.colors.tint} />}

      {isHeadingPresent && (
        <Text preset="subheading" text={heading} {...HeadingTextProps} style={$headingStyles} />
      )}

      {isContentPresent && <Text text={content} {...ContentTextProps} style={$contentStyles} />}

      {isButtonPresent && (
        <Button
          preset="reversed"
          onPress={buttonOnPress}
          text={button}
          textStyle={$buttonTextStyleOverride}
          {...ButtonProps}
          style={$buttonStyles}
        />
      )}
    </View>
  )
}

const $container: ViewStyle = { alignItems: "center", justifyContent: "center" }
const $heading: ThemedStyle<TextStyle> = ({ spacing }) => ({
  textAlign: "center",
  paddingHorizontal: spacing.lg,
})
const $content: ThemedStyle<TextStyle> = ({ spacing }) => ({
  textAlign: "center",
  paddingHorizontal: spacing.lg,
})
