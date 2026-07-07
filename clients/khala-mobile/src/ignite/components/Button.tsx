import { ComponentType } from "react"
import {
  Pressable,
  PressableProps,
  PressableStateCallbackType,
  StyleProp,
  TextStyle,
  View,
  ViewStyle,
} from "react-native"

import { useAppTheme } from "../theme/context"
import type { ThemedStyle, ThemedStyleArray } from "../theme/types"

import { Text, TextProps } from "./Text"

type Presets = "default" | "filled" | "reversed"

export interface ButtonAccessoryProps {
  style: StyleProp<any>
  pressableState: PressableStateCallbackType
  disabled?: boolean
}

export interface ButtonProps extends PressableProps {
  /**
   * Text which is looked up via i18n.
   */
  tx?: TextProps["tx"]
  /**
   * The text to display if not using `tx` or nested components.
   */
  text?: TextProps["text"]
  /**
   * Optional options to pass to i18n. Useful for interpolation.
   */
  txOptions?: TextProps["txOptions"]
  /**
   * An optional style override useful for padding & margin.
   */
  style?: StyleProp<ViewStyle>
  /**
   * An optional style override for the "pressed" state.
   */
  pressedStyle?: StyleProp<ViewStyle>
  /**
   * An optional style override for the button text.
   */
  textStyle?: StyleProp<TextStyle>
  /**
   * An optional style override for the button text when in the "pressed" state.
   */
  pressedTextStyle?: StyleProp<TextStyle>
  /**
   * An optional style override for the button text when in the "disabled" state.
   */
  disabledTextStyle?: StyleProp<TextStyle>
  /**
   * One of the different types of button presets.
   */
  preset?: Presets
  /**
   * An optional component to render on the right side of the text.
   */
  RightAccessory?: ComponentType<ButtonAccessoryProps>
  /**
   * An optional component to render on the left side of the text.
   */
  LeftAccessory?: ComponentType<ButtonAccessoryProps>
  /**
   * Children components.
   */
  children?: React.ReactNode
  /**
   * disabled prop, accessed directly for declarative styling reasons.
   */
  disabled?: boolean
  /**
   * An optional style override for the disabled state.
   */
  disabledStyle?: StyleProp<ViewStyle>
}

/**
 * A component that allows users to take actions and make choices.
 * Wraps the Text component with a Pressable component.
 * @param {ButtonProps} props - The props for the `Button` component.
 * @returns {JSX.Element} The rendered `Button` component.
 */
export function Button(props: ButtonProps) {
  const {
    tx,
    text,
    txOptions,
    style: $viewStyleOverride,
    pressedStyle: $pressedViewStyleOverride,
    textStyle: $textStyleOverride,
    pressedTextStyle: $pressedTextStyleOverride,
    disabledTextStyle: $disabledTextStyleOverride,
    children,
    RightAccessory,
    LeftAccessory,
    disabled,
    disabledStyle: $disabledViewStyleOverride,
    ...rest
  } = props

  const { themed } = useAppTheme()

  const preset: Presets = props.preset ?? "default"
  function $viewStyle({ pressed }: PressableStateCallbackType): StyleProp<ViewStyle> {
    return [
      themed($viewPresets[preset]),
      $viewStyleOverride,
      !!pressed && themed([$pressedViewPresets[preset], $pressedViewStyleOverride]),
      !!disabled && themed([$disabledViewPresets[preset], $disabledViewStyleOverride]),
    ]
  }
  function $textStyle({ pressed }: PressableStateCallbackType): StyleProp<TextStyle> {
    return [
      themed($textPresets[preset]),
      $textStyleOverride,
      !!pressed && themed([$pressedTextPresets[preset], $pressedTextStyleOverride]),
      !!disabled && themed([$disabledTextPresets[preset], $disabledTextStyleOverride]),
    ]
  }
  function $contentStyle({ pressed }: PressableStateCallbackType): StyleProp<ViewStyle> {
    return [
      themed($contentPresets[preset]),
      !!pressed && themed($pressedViewPresets[preset]),
      !!disabled && themed($disabledViewPresets[preset]),
    ]
  }

  return (
    <Pressable
      style={$viewStyle}
      accessibilityRole="button"
      accessibilityState={{ disabled: !!disabled }}
      {...rest}
      disabled={disabled}
    >
      {(state) => (
        <View pointerEvents="none" style={$contentStyle(state)}>
          {!!LeftAccessory && (
            <LeftAccessory style={$leftAccessoryStyle} pressableState={state} disabled={disabled} />
          )}

          <Text tx={tx} text={text} txOptions={txOptions} style={$textStyle(state)}>
            {children}
          </Text>

          {!!RightAccessory && (
            <RightAccessory
              style={$rightAccessoryStyle}
              pressableState={state}
              disabled={disabled}
            />
          )}
        </View>
      )}
    </Pressable>
  )
}

const $baseViewStyle: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  minHeight: 56,
  minWidth: 180,
  paddingVertical: spacing.xxs,
  paddingHorizontal: spacing.xxs,
})

const $baseContentStyle: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  minHeight: 52,
  minWidth: 176,
  borderRadius: 4,
  justifyContent: "center",
  alignItems: "center",
  flexDirection: "row",
  paddingVertical: spacing.sm,
  paddingHorizontal: spacing.lg,
})

const $baseTextStyle: ThemedStyle<TextStyle> = ({ typography }) => ({
  fontSize: 16,
  lineHeight: 20,
  fontFamily: typography.primary.medium,
  textAlign: "center",
  flexShrink: 1,
  flexGrow: 0,
  zIndex: 2,
})

const $rightAccessoryStyle: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginStart: spacing.xs,
  zIndex: 1,
})
const $leftAccessoryStyle: ThemedStyle<ViewStyle> = ({ spacing }) => ({
  marginEnd: spacing.xs,
  zIndex: 1,
})

const $viewPresets: Record<Presets, ThemedStyleArray<ViewStyle>> = {
  default: [$baseViewStyle],
  filled: [$baseViewStyle],
  reversed: [$baseViewStyle],
}

const $contentPresets: Record<Presets, ThemedStyleArray<ViewStyle>> = {
  default: [
    $baseContentStyle,
    () => ({
      borderWidth: 2,
      borderColor: "#49d7ff",
      backgroundColor: "#113242",
    }),
  ],
  filled: [
    $baseContentStyle,
    () => ({
      borderWidth: 2,
      borderColor: "#62e1ff",
      backgroundColor: "#087ea4",
    }),
  ],
  reversed: [
    $baseContentStyle,
    () => ({
      borderWidth: 2,
      borderColor: "#f4f2f1",
      backgroundColor: "#f7f3ef",
    }),
  ],
}

const $textPresets: Record<Presets, ThemedStyleArray<TextStyle>> = {
  default: [$baseTextStyle, () => ({ color: "#e8f7ff" })],
  filled: [$baseTextStyle, () => ({ color: "#f8fdff" })],
  reversed: [$baseTextStyle, () => ({ color: "#02060d" })],
}

const $pressedViewPresets: Record<Presets, ThemedStyle<ViewStyle>> = {
  default: () => ({ backgroundColor: "#17495f" }),
  filled: () => ({ backgroundColor: "#0aa1d1" }),
  reversed: () => ({ backgroundColor: "#d7cec9" }),
}

const $pressedTextPresets: Record<Presets, ThemedStyle<TextStyle>> = {
  default: () => ({ opacity: 0.9 }),
  filled: () => ({ opacity: 0.9 }),
  reversed: () => ({ opacity: 0.9 }),
}

const $disabledViewPresets: Record<Presets, ThemedStyle<ViewStyle>> = {
  default: () => ({
    borderColor: "rgba(151, 168, 184, 0.5)",
    backgroundColor: "rgba(151, 168, 184, 0.18)",
  }),
  filled: () => ({
    borderColor: "rgba(151, 168, 184, 0.55)",
    backgroundColor: "rgba(151, 168, 184, 0.28)",
  }),
  reversed: () => ({
    borderColor: "rgba(244, 242, 241, 0.28)",
    backgroundColor: "rgba(244, 242, 241, 0.28)",
  }),
}

const $disabledTextPresets: Record<Presets, ThemedStyle<TextStyle>> = {
  default: () => ({ color: "rgba(232, 247, 255, 0.48)" }),
  filled: () => ({ color: "rgba(248, 253, 255, 0.52)" }),
  reversed: () => ({ color: "rgba(2, 6, 13, 0.55)" }),
}
