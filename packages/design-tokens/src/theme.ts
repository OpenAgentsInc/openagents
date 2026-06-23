/**
 * The single typed token source for the OpenAgents app surfaces.
 *
 * This module is the ONE home for theme tokens (color, spacing, radius,
 * typography, shadow, z-index, motion). It is intentionally plain typed
 * TypeScript with NO compile-time plugin: every token is a literal value that
 * can be read directly from TS/Foldkit (`oaTokens.color.text`) and is ALSO
 * projected into CSS custom properties via {@link themeCssVars} /
 * {@link themeCss} so stylesheets can reference `var(--oa-color-text)`.
 *
 * Design goals (issue #6046):
 *  - typed accessors for TS/Foldkit consumers,
 *  - a generated `:root { --oa-…: … }` stylesheet for CSS consumers,
 *  - no scattered redefinitions — desktop/web read the SAME values,
 *  - no StyleX, no runtime `window` dependency, no build plugin.
 */

// --- Color -----------------------------------------------------------------
//
// The canonical dark palette plus the supporting greys, status tones, and
// GitHub-ish diff/review accents that the desktop app previously hardcoded.
// Values are deduped: identical colors collapse to one named token.

export const colorTokens = {
  // Core surface + text (mirrors autopilotCoreDarkTokens).
  bg: "#000",
  bgSecondary: "#151515",
  text: "#d7d8e5",
  textSecondary: "#8a8c93",
  outline: "#525458",
  primary: "#fff",

  // Status tones (the canonical product palette).
  success: "#00c853",
  warning: "#ffb400",
  danger: "#d32f2f",
  info: "#2979ff",

  // Neutral text/surface ramp used across shell + pane chrome.
  textBright: "#e6e9ef",
  textMuted: "#8b93a7",
  surfaceRaised: "#101317",
  surfaceSunken: "#0c0f13",
  surfaceDeep: "#0b0d12",
  surfaceBlackish: "#050607",

  // Accent (the amber Pylon/Verse accent).
  accent: "#f5b73a",
  accentSoft: "#f5c542",
  accentWarm: "#f59e0b",

  // HUD skin palette (#5502): white-on-black HUD look.
  hudPrimary: "#ffffff",
  hudSecondary: "#f2f4f8",
  hudSuccess: "#2bd576",
  hudInfo: "#ffffff",
  hudWarning: "#f5c542",
  hudError: "#ff4d4d",
  hudNeutral: "#9aa6b2",
  hudLine: "#e6e9ef",
  hudBg: "#0b0d12",

  // GitHub-ish review/diff accents (diff-review, status surfaces).
  reviewBorder: "#30363d",
  reviewBorderStrong: "#21262d",
  reviewDanger: "#f85149",
  reviewDangerSoft: "#ffd5d5",
  reviewSuccess: "#3fb950",
  reviewSuccessSoft: "#4ade80",
  reviewSuccessTint: "#d7f7df",
  reviewWarning: "#d29922",
  reviewText: "#f0f6fc",
  reviewTextSoft: "#f0f1f4",
  reviewTextFaint: "#8b949e",
  reviewSurface: "#f1efe8",
  reviewSurfaceSoft: "#f4f4f5",
  reviewLink: "#66aaff",
  reviewLinkSoft: "#8fd0ff",
  reviewDangerStrong: "#ff7070",
  reviewWarningTint: "#fff8e6",
  reviewNeutral: "#d7dee7",
} as const

export type ColorTokens = typeof colorTokens
export type ColorToken = keyof ColorTokens

// --- Spacing ---------------------------------------------------------------
//
// A small rem scale. Desktop chrome leans on these; the names are the literal
// step so `oaTokens.space[2]` reads naturally.

export const spaceTokens = {
  0: "0",
  px: "1px",
  1: "0.25rem",
  2: "0.5rem",
  3: "0.75rem",
  4: "1rem",
  5: "1.25rem",
  6: "1.5rem",
  8: "2rem",
  10: "2.5rem",
  12: "3rem",
} as const

export type SpaceTokens = typeof spaceTokens
export type SpaceToken = keyof SpaceTokens

// --- Radius ----------------------------------------------------------------

export const radiusTokens = {
  none: "0",
  sm: "3px",
  md: "4px",
  lg: "6px",
  xl: "8px",
  "2xl": "10px",
  "3xl": "12px",
  full: "9999px",
} as const

export type RadiusTokens = typeof radiusTokens
export type RadiusToken = keyof RadiusTokens

// --- Typography ------------------------------------------------------------

export const fontTokens = {
  sans: "inherit",
  mono: "ui-monospace, monospace",
} as const

export const fontSizeTokens = {
  xs: "0.68rem",
  sm: "0.72rem",
  base: "0.78rem",
  md: "0.96rem",
  lg: "1rem",
} as const

export const lineHeightTokens = {
  tight: "1",
  snug: "1.45",
  normal: "1.5",
} as const

export const letterSpacingTokens = {
  none: "0",
  wide: "0.04em",
  wider: "0.07em",
  widest: "0.08em",
} as const

export type FontTokens = typeof fontTokens
export type FontSizeTokens = typeof fontSizeTokens
export type LineHeightTokens = typeof lineHeightTokens
export type LetterSpacingTokens = typeof letterSpacingTokens

// --- Shadow ----------------------------------------------------------------

export const shadowTokens = {
  pane: "0 24px 64px rgb(0 0 0 / 0.6)",
} as const

export type ShadowTokens = typeof shadowTokens
export type ShadowToken = keyof ShadowTokens

// --- Z-index ---------------------------------------------------------------

export const zIndexTokens = {
  paneLayer: "30",
  paneWindowBase: "100",
  returnButton: "9999",
} as const

export type ZIndexTokens = typeof zIndexTokens
export type ZIndexToken = keyof ZIndexTokens

// --- Motion ----------------------------------------------------------------

export const motionTokens = {
  fast: "0.14s",
  easing: "ease",
} as const

export type MotionTokens = typeof motionTokens
export type MotionToken = keyof MotionTokens

// --- Aggregate -------------------------------------------------------------

export const oaTokens = {
  color: colorTokens,
  space: spaceTokens,
  radius: radiusTokens,
  font: fontTokens,
  fontSize: fontSizeTokens,
  lineHeight: lineHeightTokens,
  letterSpacing: letterSpacingTokens,
  shadow: shadowTokens,
  zIndex: zIndexTokens,
  motion: motionTokens,
} as const

export type OaTokens = typeof oaTokens

// --- CSS custom-property projection ----------------------------------------

const kebab = (value: string): string =>
  value
    .replace(/([a-z0-9])([A-Z])/g, "$1-$2")
    .replace(/_/g, "-")
    .toLowerCase()

const groupVars = (group: string, record: Record<string, string>): Record<string, string> =>
  Object.fromEntries(
    Object.entries(record).map(([key, value]) => [`--oa-${group}-${kebab(key)}`, value]),
  )

/**
 * The full set of `--oa-*` custom properties as a flat record. Every value is
 * a literal token value — reading these never touches the DOM or `window`.
 */
export const themeCssVars = (): Record<string, string> => ({
  ...groupVars("color", colorTokens),
  ...groupVars("space", spaceTokens),
  ...groupVars("radius", radiusTokens),
  ...groupVars("font", fontTokens),
  ...groupVars("font-size", fontSizeTokens),
  ...groupVars("line-height", lineHeightTokens),
  ...groupVars("letter-spacing", letterSpacingTokens),
  ...groupVars("shadow", shadowTokens),
  ...groupVars("z", zIndexTokens),
  ...groupVars("motion", motionTokens),
})

/**
 * The generated `:root { … }` stylesheet block exposing every token as a CSS
 * custom property. `selector` defaults to `:root` so it can be dropped straight
 * into a stylesheet; pass a different selector to scope the tokens.
 */
export const themeCss = (selector = ":root"): string => {
  const lines = Object.entries(themeCssVars()).map(([name, value]) => `  ${name}: ${value};`)
  return `${selector} {\n${lines.join("\n")}\n}\n`
}

/**
 * Typed accessor returning the `var(--oa-color-…)` reference for a color token
 * (with the literal value as a fallback so it resolves even before the
 * stylesheet loads). Use this from Foldkit/TS where a CSS var string is wanted.
 */
export const colorVar = (token: ColorToken): string =>
  `var(--oa-color-${kebab(String(token))}, ${colorTokens[token]})`

export const spaceVar = (token: SpaceToken): string =>
  `var(--oa-space-${kebab(String(token))}, ${spaceTokens[token]})`

export const radiusVar = (token: RadiusToken): string =>
  `var(--oa-radius-${kebab(String(token))}, ${radiusTokens[token]})`
