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
    textPrimary: khalaMobileTheme.text,
    textMuted: khalaMobileTheme.textMuted,
    accent: khalaMobileTheme.accent,
    danger: khalaMobileTheme.danger,
    border: khalaMobileTheme.border,
    focus: khalaMobileTheme.accentSoft,
  },
})
