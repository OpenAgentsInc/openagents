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
  color: colors.text,
  fontFamily: typography.primary.normal,
};

const presets: Record<TextPreset, TextStyle> = {
  display: { ...base, fontSize: 30, lineHeight: 36, fontWeight: "700", letterSpacing: -0.4 },
  heading: { ...base, fontSize: 20, lineHeight: 26, fontWeight: "700", letterSpacing: -0.2 },
  subheading: { ...base, fontSize: 17, lineHeight: 23, fontWeight: "600" },
  body: { ...base, fontSize: 15, lineHeight: 22 },
  bodyStrong: { ...base, fontSize: 15, lineHeight: 22, fontWeight: "600" },
  label: { ...base, fontSize: 13, lineHeight: 18, fontWeight: "600" },
  caption: { ...base, fontSize: 12, lineHeight: 17, color: colors.textDim },
  mono: { ...base, fontSize: 13, lineHeight: 19, fontFamily: typography.code.normal },
};
