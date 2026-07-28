/**
 * The mobile design tokens, in the arcade idiom.
 *
 * Owner direction 2026-07-27: rebuild the mobile surfaces from arcade's
 * components in plain React Native and phase Effect Native out as each surface
 * is touched. These are plain values a `StyleSheet` consumes, so a component
 * needs no renderer, no view program, and no host to draw itself.
 *
 * Reference: `~/work/projects/repos/arcade/app/theme`.
 */

const palette = {
  almostBlack: "#030303",
  black: "#000000",
  white: "#FFFFFF",

  neutral100: "#FFFFFF",
  neutral200: "#F4F2F1",
  neutral300: "#D7CEC9",
  neutral400: "#B6ACA6",
  neutral500: "#978F8A",
  neutral600: "#564E4A",
  neutral700: "#3C3836",
  neutral800: "#191015",
  neutral900: "#000000",

  cyan300: "#67e8f9",
  cyan500: "#06b6d4",
  cyan700: "#0e7490",
  cyan800: "#155e75",
  cyan900: "#164e63",
  cyan950: "#083344",

  green400: "#4ADE80",
  green900: "#14532D",
  amber400: "#FBBF24",
  amber900: "#78350F",
  angry100: "#F2D6CD",
  angry500: "#C03403",

  overlay20: "rgba(69, 210, 255, 0.11)",
  overlay50: "rgba(69, 210, 255, 0.05)",
} as const;

export const colors = {
  palette,
  transparent: "rgba(0, 0, 0, 0)",
  /** Page background. */
  background: palette.almostBlack,
  /** A raised surface: cards, headers, composers. */
  surface: "#0B0F14",
  /** A surface raised above `surface`, for a message from the person. */
  surfaceRaised: "#121820",
  /** Primary reading colour. */
  text: palette.neutral100,
  /** Supporting text: timestamps, disclosure, captions. */
  textDim: "#8A9AA8",
  /** Hairlines and card edges. */
  separator: palette.cyan950,
  border: palette.cyan900,
  /** The one accent. */
  tint: "#5BC6E0",
  tintDim: palette.cyan800,
  success: palette.green400,
  successBackground: "rgba(74, 222, 128, 0.12)",
  warning: palette.amber400,
  warningBackground: "rgba(251, 191, 36, 0.12)",
  error: palette.angry500,
  errorBackground: "rgba(192, 52, 3, 0.14)",
} as const;

export const spacing = {
  micro: 2,
  tiny: 4,
  extraSmall: 8,
  small: 12,
  medium: 16,
  large: 24,
  extraLarge: 32,
  huge: 48,
  massive: 64,
} as const;

export type Spacing = keyof typeof spacing;

export const radius = {
  small: 6,
  medium: 10,
  large: 14,
  pill: 999,
} as const;

export const typography = {
  primary: {
    light: "System",
    normal: "System",
    medium: "System",
    semiBold: "System",
    bold: "System",
  },
  code: {
    normal: "Menlo",
  },
} as const;

export const timing = {
  quick: 150,
  regular: 250,
} as const;
