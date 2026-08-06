import { khalaNativeTheme } from "@openagentsinc/design-tokens/native";

/**
 * Mobile ergonomic aliases over the canonical semantic projection. The app
 * owns no color, spacing, radius, or motion values: a source change in
 * `@openagentsinc/design-tokens` reaches both this layer and the web CSS
 * projection from the same package commit.
 */
const semantic = khalaNativeTheme.color;
const matrix = khalaNativeTheme.colorMatrix;

const palette = {
  void: semantic.background,
  panel: semantic.surface,
  raised: semantic.surfaceRaised,
  sunken: semantic.background,

  hairline: semantic.border,
  hairlineQuiet: semantic.borderSubtle,
  hairlineEnergized: semantic.stateSelected,

  energy: semantic.accent,
  energyHot: semantic.accentHover,
  energyInk: semantic.focus,
  energyGlow: semantic.stateSelected,

  white: semantic.textPrimary,
  body: semantic.textBody,
  secondary: semantic.textMuted,
  faint: semantic.textFaint,

  live: semantic.attentionDone,
  liveGlow: matrix.success.soft.rest.background,
  warn: semantic.attentionApproval,
  warnGlow: matrix.warning.soft.rest.background,
  fault: semantic.attentionFailed,
  faultGlow: matrix.danger.soft.rest.background,
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

  attentionApproval: semantic.attentionApproval,
  attentionInput: semantic.attentionInput,
  attentionWorking: semantic.attentionWorking,
  attentionFailed: semantic.attentionFailed,
  attentionDone: semantic.attentionDone,
} as const;

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
