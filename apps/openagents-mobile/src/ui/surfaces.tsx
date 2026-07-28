import type { ReactNode } from "react";
import {
  Pressable,
  type StyleProp,
  TextInput,
  View,
  type TextStyle,
  type ViewStyle,
} from "react-native";

import { Text } from "./text";
import { colors, radius, spacing } from "./theme";

/** A raised container with a hairline edge. */
export const Card = ({
  onPress,
  accessibilityLabel,
  style,
  children,
}: {
  readonly onPress?: () => void;
  readonly accessibilityLabel?: string;
  readonly style?: StyleProp<ViewStyle>;
  readonly children: ReactNode;
}) => {
  if (onPress === undefined) {
    return (
      <View accessible accessibilityLabel={accessibilityLabel} style={[$card, style]}>
        {children}
      </View>
    );
  }
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={accessibilityLabel}
      onPress={onPress}
      style={({ pressed }) => [$card, pressed ? $cardPressed : null, style]}
    >
      {children}
    </Pressable>
  );
};

export type BadgeTone = "success" | "info" | "danger" | "neutral";

/** A small state marker: the connection state, a thread's turn state. */
export const Badge = ({ label, tone }: { readonly label: string; readonly tone: BadgeTone }) => (
  <View style={[$badge, $badgeTone[tone]]}>
    <Text preset="caption" style={[$badgeText, { color: $badgeTextColor[tone] }]}>
      {label}
    </Text>
  </View>
);

/** A hairline. */
export const Divider = ({ style }: { readonly style?: StyleProp<ViewStyle> }) => (
  <View style={[$divider, style]} />
);

/** The one text input shape. */
export const Field = ({
  value,
  onChangeText,
  placeholder,
  accessibilityLabel,
  multiline = false,
  onSubmitEditing,
  style,
}: {
  readonly value: string;
  readonly onChangeText: (next: string) => void;
  readonly placeholder?: string;
  readonly accessibilityLabel?: string;
  readonly multiline?: boolean;
  readonly onSubmitEditing?: () => void;
  readonly style?: StyleProp<TextStyle>;
}) => (
  <TextInput
    accessibilityLabel={accessibilityLabel ?? placeholder}
    value={value}
    onChangeText={onChangeText}
    onSubmitEditing={onSubmitEditing}
    placeholder={placeholder}
    placeholderTextColor={colors.textDim}
    multiline={multiline}
    style={[$field, multiline ? $fieldMultiline : null, style]}
  />
);

/** What a surface says when it has nothing to show, without pretending. */
export const EmptyState = ({
  heading,
  body,
}: {
  readonly heading: string;
  readonly body: string;
}) => (
  <View style={$empty}>
    <Text preset="subheading">{heading}</Text>
    <Text preset="caption" style={$emptyBody}>
      {body}
    </Text>
  </View>
);

const $card: ViewStyle = {
  backgroundColor: colors.surface,
  borderRadius: radius.large,
  borderWidth: 1,
  borderColor: colors.separator,
  padding: spacing.medium,
};

const $cardPressed: ViewStyle = { backgroundColor: colors.surfaceRaised };

const $badge: ViewStyle = {
  paddingHorizontal: spacing.extraSmall,
  paddingVertical: spacing.micro,
  borderRadius: radius.pill,
  borderWidth: 1,
};

const $badgeText: TextStyle = { fontWeight: "600" };

const $badgeTone: Record<BadgeTone, ViewStyle> = {
  success: { backgroundColor: colors.successBackground, borderColor: colors.success },
  info: { backgroundColor: colors.warningBackground, borderColor: colors.warning },
  danger: { backgroundColor: colors.errorBackground, borderColor: colors.error },
  neutral: { backgroundColor: colors.surfaceRaised, borderColor: colors.border },
};

const $badgeTextColor: Record<BadgeTone, string> = {
  success: colors.success,
  info: colors.warning,
  danger: colors.error,
  neutral: colors.textDim,
};

const $divider: ViewStyle = { height: 1, backgroundColor: colors.separator };

const $field: TextStyle = {
  backgroundColor: colors.surfaceRaised,
  borderRadius: radius.medium,
  borderWidth: 1,
  borderColor: colors.border,
  color: colors.text,
  paddingHorizontal: spacing.small,
  paddingVertical: spacing.small,
  fontSize: 15,
  minHeight: 44,
};

const $fieldMultiline: TextStyle = { minHeight: 88, textAlignVertical: "top" };

const $empty: ViewStyle = {
  paddingVertical: spacing.extraLarge,
  paddingHorizontal: spacing.medium,
  gap: spacing.extraSmall,
};

const $emptyBody: TextStyle = { maxWidth: 320 };
