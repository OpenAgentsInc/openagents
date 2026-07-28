import type { ReactNode } from "react";
import {
  Pressable,
  type PressableStateCallbackType,
  type StyleProp,
  type TextStyle,
  type ViewStyle,
} from "react-native";

import { Text } from "./text";
import { colors, radius, spacing } from "./theme";

export type ButtonPreset = "primary" | "secondary" | "ghost" | "danger";

/**
 * A button that is the size of a button.
 *
 * The Effect Native buttons this replaces stretched to whatever the row gave
 * them, so a two-button composer row rendered two full-height columns. A fixed
 * `minHeight` with `alignSelf: "flex-start"` on the container keeps a control
 * the size of its label unless a caller asks for `fullWidth`.
 */
export const Button = ({
  label,
  onPress,
  preset = "primary",
  disabled = false,
  fullWidth = false,
  accessibilityLabel,
  style,
  textStyle,
  children,
}: {
  readonly label?: string;
  readonly onPress?: () => void;
  readonly preset?: ButtonPreset;
  readonly disabled?: boolean;
  readonly fullWidth?: boolean;
  readonly accessibilityLabel?: string;
  readonly style?: StyleProp<ViewStyle>;
  readonly textStyle?: StyleProp<TextStyle>;
  readonly children?: ReactNode;
}) => (
  <Pressable
    accessibilityRole="button"
    accessibilityLabel={accessibilityLabel ?? label}
    accessibilityState={{ disabled }}
    disabled={disabled}
    onPress={onPress}
    hitSlop={spacing.extraSmall}
    style={({ pressed }: PressableStateCallbackType) => [
      $base,
      fullWidth ? $fullWidth : null,
      $view[preset],
      pressed ? $pressed[preset] : null,
      disabled ? $disabled : null,
      style,
    ]}
  >
    {children ?? (
      <Text preset="label" style={[$text[preset], textStyle]}>
        {label}
      </Text>
    )}
  </Pressable>
);

const $base: ViewStyle = {
  minHeight: 44,
  paddingVertical: spacing.small,
  paddingHorizontal: spacing.medium,
  borderRadius: radius.medium,
  flexDirection: "row",
  alignItems: "center",
  justifyContent: "center",
  alignSelf: "flex-start",
  borderWidth: 1,
  borderColor: colors.transparent,
};

const $fullWidth: ViewStyle = { alignSelf: "stretch" };

const $disabled: ViewStyle = { opacity: 0.4 };

const $view: Record<ButtonPreset, ViewStyle> = {
  primary: { backgroundColor: colors.accent },
  secondary: { backgroundColor: colors.surface, borderColor: colors.border },
  ghost: { backgroundColor: colors.transparent },
  danger: { backgroundColor: colors.faultGlow, borderColor: colors.fault },
};

const $pressed: Record<ButtonPreset, ViewStyle> = {
  primary: { opacity: 0.85 },
  secondary: { borderColor: colors.borderEnergized },
  ghost: { backgroundColor: colors.surface },
  danger: { opacity: 0.85 },
};

const $text: Record<ButtonPreset, TextStyle> = {
  primary: { color: colors.palette.void },
  secondary: { color: colors.text },
  ghost: { color: colors.accentInk },
  danger: { color: colors.fault },
};
