import type { ReactNode } from "react";
import { Text as RNText, type StyleProp, type TextStyle } from "react-native";

import { colors, typography } from "./theme";

export type TextPreset =
  | "display"
  | "heading"
  | "subheading"
  | "body"
  | "bodyStrong"
  | "label"
  | "caption"
  | "mono";

/**
 * One text component with named presets, in the arcade idiom.
 *
 * A caller picks a preset rather than a font size, so a screen cannot drift
 * into a size the rest of the app does not use.
 */
export const Text = ({
  preset = "body",
  color,
  style,
  numberOfLines,
  children,
}: {
  readonly preset?: TextPreset;
  readonly color?: string;
  readonly style?: StyleProp<TextStyle>;
  readonly numberOfLines?: number;
  readonly children?: ReactNode;
}) => (
  <RNText
    numberOfLines={numberOfLines}
    style={[presets[preset], color === undefined ? null : { color }, style]}
  >
    {children}
  </RNText>
);

const base: TextStyle = {
  color: colors.textBody,
  fontFamily: typography.sans,
};

const presets: Record<TextPreset, TextStyle> = {
  display: {
    ...base,
    fontFamily: typography.monoBold,
    color: colors.text,
    fontSize: 26,
    lineHeight: 32,
    letterSpacing: -0.8,
  },
  heading: {
    ...base,
    fontFamily: typography.monoBold,
    color: colors.text,
    fontSize: 18,
    lineHeight: 24,
    letterSpacing: -0.4,
  },
  subheading: {
    ...base,
    fontFamily: typography.sansSemiBold,
    color: colors.text,
    fontSize: 16,
    lineHeight: 22,
  },
  body: { ...base, fontSize: 15, lineHeight: 23 },
  bodyStrong: {
    ...base,
    fontFamily: typography.sansSemiBold,
    color: colors.text,
    fontSize: 15,
    lineHeight: 23,
  },
  label: {
    ...base,
    fontFamily: typography.mono,
    fontSize: 11,
    lineHeight: 15,
    letterSpacing: 0.6,
    color: colors.textDim,
  },
  caption: { ...base, fontSize: 12, lineHeight: 17, color: colors.textDim },
  mono: { ...base, fontSize: 12.5, lineHeight: 19, fontFamily: typography.mono },
};
