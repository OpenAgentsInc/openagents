import { khalaNativeTheme } from "@openagentsinc/design-tokens/native";
import { defaultTheme } from "@openagentsinc/design-tokens";
import { projectNativeTheme } from "@openagentsinc/design-tokens/native";
import { DynamicColorIOS, Platform } from "react-native";

/**
 * Mobile ergonomic aliases over the canonical semantic projection. The app
 * owns no color, spacing, radius, or motion values: a source change in
 * `@openagentsinc/design-tokens` reaches both this layer and the web CSS
 * projection from the same package commit.
 */
const semantic = khalaNativeTheme.color;
const matrix = khalaNativeTheme.colorMatrix;
const lightTheme = projectNativeTheme(defaultTheme);
const lightSemantic = lightTheme.color;
const lightMatrix = lightTheme.colorMatrix;

const adaptive = (dark: string, light: string): string =>
  Platform.OS === "ios" ? (DynamicColorIOS({ dark, light }) as unknown as string) : dark;

const palette = {
  void: adaptive(semantic.background, lightSemantic.background),
  panel: adaptive(semantic.surface, lightSemantic.surface),
  raised: adaptive(semantic.surfaceRaised, lightSemantic.surfaceRaised),
  sunken: adaptive(semantic.background, lightSemantic.background),

  hairline: adaptive(semantic.border, lightSemantic.border),
  hairlineQuiet: adaptive(semantic.borderSubtle, lightSemantic.borderSubtle),
  hairlineEnergized: adaptive(semantic.stateSelected, lightSemantic.stateSelected),

  energy: adaptive(semantic.accent, lightSemantic.accent),
  energyHot: adaptive(semantic.accentHover, lightSemantic.accentHover),
  energyInk: adaptive(semantic.focus, lightSemantic.focus),
  energyGlow: adaptive(semantic.stateSelected, lightSemantic.stateSelected),

  white: adaptive(semantic.textPrimary, lightSemantic.textPrimary),
  body: adaptive(semantic.textBody, lightSemantic.textBody),
  secondary: adaptive(semantic.textMuted, lightSemantic.textMuted),
  faint: adaptive(semantic.textFaint, lightSemantic.textFaint),

  live: adaptive(semantic.attentionDone, lightSemantic.attentionDone),
  liveGlow: adaptive(matrix.success.soft.rest.background, lightMatrix.success.soft.rest.background),
  warn: adaptive(semantic.attentionApproval, lightSemantic.attentionApproval),
  warnGlow: adaptive(matrix.warning.soft.rest.background, lightMatrix.warning.soft.rest.background),
  fault: adaptive(semantic.attentionFailed, lightSemantic.attentionFailed),
  faultGlow: adaptive(matrix.danger.soft.rest.background, lightMatrix.danger.soft.rest.background),
} as const;

export const colors = {
  palette,
  transparent: khalaNativeTheme.transparent,

  background: palette.panel,
  surface: palette.raised,
  surfaceSunken: palette.sunken,

  text: palette.white,
  textBody: palette.body,
  textDim: palette.secondary,
  textFaint: palette.faint,

  border: palette.hairline,
  borderQuiet: palette.hairlineQuiet,
  borderEnergized: palette.hairlineEnergized,

  accent: palette.energy,
  accentHot: palette.energyHot,
  accentInk: palette.energyInk,
  accentGlow: palette.energyGlow,

  live: palette.live,
  liveGlow: palette.liveGlow,
  warn: palette.warn,
  warnGlow: palette.warnGlow,
  fault: palette.fault,
  faultGlow: palette.faultGlow,

  attentionApproval: adaptive(semantic.attentionApproval, lightSemantic.attentionApproval),
  attentionInput: adaptive(semantic.attentionInput, lightSemantic.attentionInput),
  attentionWorking: adaptive(semantic.attentionWorking, lightSemantic.attentionWorking),
  attentionFailed: adaptive(semantic.attentionFailed, lightSemantic.attentionFailed),
  attentionDone: adaptive(semantic.attentionDone, lightSemantic.attentionDone),
} as const;

export const navigationColors = (scheme: string | null | undefined) => {
  const selected = scheme === "light" ? lightTheme : khalaNativeTheme;
  return {
    primary: selected.color.accent,
    background: selected.color.surface,
    card: selected.color.background,
    text: selected.color.textPrimary,
    border: selected.color.border,
    notification: selected.color.attentionApproval,
  };
};

export const spacing = {
  micro: khalaNativeTheme.spacing["0.5"],
  tiny: khalaNativeTheme.spacing["1"],
  extraSmall: khalaNativeTheme.spacing["2"],
  small: khalaNativeTheme.spacing["3"],
  medium: khalaNativeTheme.spacing["4"],
  large: khalaNativeTheme.spacing["6"],
  extraLarge: khalaNativeTheme.spacing["8"],
  huge: khalaNativeTheme.spacing["12"],
} as const;

export const radius = {
  small: khalaNativeTheme.radius.md,
  medium: khalaNativeTheme.radius.xl,
  large: khalaNativeTheme.radius["2xl"],
  pill: khalaNativeTheme.radius.full,
} as const;

/**
 * The desktop's own faces, bundled here so a message reads identically on both
 * screens.
 *
 * Omega resolves `.ZedSans` to IBM Plex Sans and `.ZedMono` to Lilex
 * (`omega/assets/settings/default.json`), and the agent panel sets prose in the
 * UI font. The same files ship in `assets/fonts/` with their licences.
 *
 * A bundled family does not synthesise weight on React Native: `fontWeight`
 * alone keeps the regular file. Ask for the weighted family by name instead.
 */
export const typography = {
  sans: "IBMPlexSans-Regular",
  sansSemiBold: "IBMPlexSans-SemiBold",
  mono: "Lilex-Regular",
  monoBold: "Lilex-Bold",
} as const;

/** Every bundled face, for the loader at the app root. */
export const fontAssets = {
  "IBMPlexSans-Regular": require("../../assets/fonts/IBMPlexSans-Regular.ttf"),
  "IBMPlexSans-Italic": require("../../assets/fonts/IBMPlexSans-Italic.ttf"),
  "IBMPlexSans-SemiBold": require("../../assets/fonts/IBMPlexSans-SemiBold.ttf"),
  "IBMPlexSans-SemiBoldItalic": require("../../assets/fonts/IBMPlexSans-SemiBoldItalic.ttf"),
  "Lilex-Regular": require("../../assets/fonts/Lilex-Regular.ttf"),
  "Lilex-Italic": require("../../assets/fonts/Lilex-Italic.ttf"),
  "Lilex-Bold": require("../../assets/fonts/Lilex-Bold.ttf"),
  "Lilex-BoldItalic": require("../../assets/fonts/Lilex-BoldItalic.ttf"),
} as const;

export const timing = {
  quick: khalaNativeTheme.motion.standard.durationFastMs,
  regular: khalaNativeTheme.motion.standard.durationExitMs,
} as const;

export const reducedTiming = {
  quick: khalaNativeTheme.motion.reduced.durationFastMs,
  regular: khalaNativeTheme.motion.reduced.durationExitMs,
} as const;
