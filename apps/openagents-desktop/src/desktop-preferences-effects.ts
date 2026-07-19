/**
 * Pure preference → presentation mapping (CUT-24 criterion 1, #8704).
 *
 * This is where density, font, and reduced-motion become a REAL effect, not an
 * inert stored value:
 *
 * - `fontScale` and `density` produce a SCALED copy of the fixed product
 *   theme (Tokyo Night colors over the shared type-scale, spacing, and control
 *   tokens for density). The renderer emits every scaled token as a `--en-*`
 *   CSS variable, so passing the scaled theme to the Effect Native DOM
 *   renderer / `setTheme` genuinely resizes the whole app through the
 *   existing token pipeline. The one Tokyo Night color identity is untouched
 *   (single-theme policy) — only sizing scales.
 * - `reducedMotion` produces a root data attribute the app CSS honors
 *   (`:root[data-en-reduce-motion="true"]`) so an explicit user override works
 *   regardless of the OS `prefers-reduced-motion` setting; `system` leaves the
 *   attribute off and defers to the OS media query.
 *
 * Pure and dependency-light so it is unit-testable headlessly (no Electron,
 * no DOM) — the boot layer is the only place that actually mounts the result.
 */
import { khalaTheme, type Theme } from "@effect-native/tokens"

import type {
  DesktopDensity,
  DesktopFontScale,
  DesktopPreferences,
  DesktopReducedMotion,
} from "./desktop-preferences-contract.ts"
import { tokyoNightDesktopThemeProjection } from "./ide/tokyo-night-theme.ts"

/** Tokyo Night is installed before the first workbench/editor paint. */
export const tokyoNightDesktopTheme: Theme = {
  ...khalaTheme,
  color: tokyoNightDesktopThemeProjection.effectNative,
}

/** Multiplier applied to type-scale font/line tokens. */
export const fontScaleFactor = (scale: DesktopFontScale): number => {
  switch (scale) {
    case "small":
      return 0.9
    case "default":
      return 1
    case "large":
      return 1.15
    case "x-large":
      return 1.3
  }
}

/** Multiplier applied to spacing + control-size tokens. */
export const densityFactor = (density: DesktopDensity): number => {
  switch (density) {
    case "comfortable":
      return 1
    case "cozy":
      return 0.9
    case "compact":
      return 0.82
  }
}

const scaleInt = (value: number, factor: number): number => Math.max(0, Math.round(value * factor))

/**
 * Build the scaled theme for the given font/density preferences. When both
 * factors are 1 the input theme is returned unchanged (identity), so the common
 * default path allocates nothing.
 */
export const applyPreferencesToTheme = (
  base: Theme,
  input: { fontScale: DesktopFontScale; density: DesktopDensity },
): Theme => {
  const font = fontScaleFactor(input.fontScale)
  const density = densityFactor(input.density)
  if (font === 1 && density === 1) return base

  const typeScale = Object.fromEntries(
    Object.entries(base.typeScale).map(([key, value]) => [
      key,
      {
        // A font scale below 1 must never invert the >= relationship or drop a
        // line below its font size; round then clamp lineHeight >= fontSize.
        fontSize: Math.max(1, scaleInt(value.fontSize, font)),
        lineHeight: Math.max(scaleInt(value.fontSize, font), scaleInt(value.lineHeight, font)),
        fontWeight: value.fontWeight,
      },
    ]),
  ) as Theme["typeScale"]

  const spacing = Object.fromEntries(
    Object.entries(base.spacing).map(([key, value]) => [key, scaleInt(value, density)]),
  ) as Theme["spacing"]

  const control = Object.fromEntries(
    Object.entries(base.control).map(([key, value]) => [
      key,
      {
        height: Math.max(1, scaleInt(value.height, density)),
        gutter: scaleInt(value.gutter, density),
        icon: Math.max(1, scaleInt(value.icon, density)),
      },
    ]),
  ) as Theme["control"]

  return { ...base, typeScale, spacing, control }
}

/** The scaled theme for a full preferences document, from fixed Tokyo Night. */
export const themeForPreferences = (preferences: DesktopPreferences): Theme =>
  applyPreferencesToTheme(tokyoNightDesktopTheme, {
    fontScale: preferences.appearance.fontScale,
    density: preferences.appearance.density,
  })

/**
 * Root data attributes derived from preferences. `data-en-reduce-motion` is the
 * one the app CSS honors; the density/font attributes are informational hooks
 * (the real sizing effect flows through the scaled theme above).
 *
 * `reduce-motion`:
 * - "true"  → force reduced motion (user override, honored by app.css)
 * - "false" → force full motion (user override, wins over OS)
 * - absent  → defer to the OS `prefers-reduced-motion` media query
 */
export const preferencesRootAttributes = (
  preferences: DesktopPreferences,
): Record<string, string> => {
  const attributes: Record<string, string> = {
    "data-en-density": preferences.appearance.density,
    "data-en-font-scale": preferences.appearance.fontScale,
  }
  const reduce = reduceMotionAttributeValue(preferences.appearance.reducedMotion)
  if (reduce !== null) attributes["data-en-reduce-motion"] = reduce
  return attributes
}

/** null → leave the attribute off (defer to OS); otherwise the explicit override. */
export const reduceMotionAttributeValue = (mode: DesktopReducedMotion): "true" | "false" | null => {
  switch (mode) {
    case "always":
      return "true"
    case "never":
      return "false"
    case "system":
      return null
  }
}
