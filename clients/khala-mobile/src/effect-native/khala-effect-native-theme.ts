import { defaultTheme, defineTheme, type Theme } from "@effect-native/tokens"

import { khalaMobileTheme } from "../theme/tokens"

/**
 * EN-3 (#8568) token bridge: the app's Protoss-blue design tokens expressed as
 * an Effect Native `Theme`. The COLOR VALUES are read straight from the app's
 * existing `khalaMobileTheme` (itself derived from the shared
 * `@openagentsinc/ui` NativeWind tokens), so NativeWind-styled RN screens and
 * Effect Native surfaces read one source of truth — no parallel palette. The
 * non-color scales (spacing/radius/type/breakpoint/dimension) inherit the
 * catalog `defaultTheme` until the app needs to diverge them.
 */
export const khalaEffectNativeTheme: Theme = defineTheme({
  ...defaultTheme,
  color: {
    background: khalaMobileTheme.background,
    surface: khalaMobileTheme.surface,
    surfaceRaised: khalaMobileTheme.surfaceRaised,
    // Chrome-language roles (tokens vNext): seeded from the canonical Khala
    // palette until a mobile chrome surface needs to diverge them.
    surfaceOverlay: "#182640",
    textFaint: "#6b7ca1",
    textInverse: "#05070d",
    textDisabled: "#55648a",
    accentHover: "#5c96f8",
    accentActive: "#2f6fe0",
    borderSubtle: "#16203a",
    borderStrong: "#2c3d63",
    stateHover: "#8fb3ff14",
    stateActive: "#8fb3ff21",
    stateSelected: "#3b82f629",
    scrim: "#02040adb",
    textPrimary: khalaMobileTheme.text,
    textMuted: khalaMobileTheme.textMuted,
    accent: khalaMobileTheme.accent,
    danger: khalaMobileTheme.danger,
    border: khalaMobileTheme.border,
    focus: khalaMobileTheme.accentSoft,
    info: khalaMobileTheme.accentSoft,
    success: khalaMobileTheme.success,
    warning: khalaMobileTheme.warning,
    // Code/diff/syntax colors (catalog v9+ CodeBlock/diff surfaces) are not yet
    // rendered on mobile; seed them from the canonical Khala palette until a
    // mobile code surface needs to diverge them.
    codeBackground: "#0a0f1c",
    diffAdd: "#4ade80",
    diffRemove: "#f87171",
    syntaxKeyword: "#60a5fa",
    syntaxString: "#4ade80",
    syntaxComment: "#5b6b8c",
    syntaxFunction: "#c084fc",
    syntaxNumber: "#fbbf24",
    syntaxOperator: "#93a4c3",
  },
})
