/**
 * The Omega desktop palette, as mobile tokens.
 *
 * These values are the desktop's own `Aiur dark` theme
 * (`omega/assets/themes/aiur/aiur.json`), read field for field, so a person
 * moving between the two surfaces sees one product rather than two that share
 * a name. The house language behind it is `DESIGN.md`: a blue-tinted void lit
 * by one committed blue, carried by hairlines and markers rather than fills.
 *
 * Structure follows the arcade idiom (flat consts, no provider), because the
 * owner directed this app onto arcade patterns in plain React Native on
 * 2026-07-27. That direction supersedes DESIGN.md's pointer at the Effect
 * Native token package, which this app no longer depends on.
 */

const palette = {
  void: "#05070d",
  panel: "#0b1220",
  raised: "#141f36",
  sunken: "#05070d",

  hairline: "#1f2b45",
  hairlineQuiet: "#16203a",
  hairlineEnergized: "rgba(59, 130, 246, 0.35)",

  energy: "#3b82f6",
  energyHot: "#5c96f8",
  energyInk: "#8fb3ff",
  energyGlow: "rgba(59, 130, 246, 0.16)",

  white: "#eef3ff",
  body: "#c6cfe6",
  secondary: "#a9b1d6",
  faint: "#7b85a8",

  live: "#22c55e",
  liveGlow: "rgba(34, 197, 94, 0.14)",
  warn: "#f59e0b",
  warnGlow: "rgba(245, 158, 11, 0.14)",
  fault: "#f87171",
  faultGlow: "rgba(248, 113, 113, 0.14)",
} as const;

export const colors = {
  palette,
  transparent: "rgba(0, 0, 0, 0)",

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
} as const;

export const radius = {
  small: 4,
  medium: 8,
  large: 12,
  pill: 999,
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
  quick: 140,
  regular: 220,
} as const;
