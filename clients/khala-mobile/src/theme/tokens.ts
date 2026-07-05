import { openAgentsNativeWindTokens } from "@openagentsinc/ui/react"

const khalaMobileBackground = "#02060d"

export const khalaMobileTokens = {
  ...openAgentsNativeWindTokens,
  colors: {
    ...openAgentsNativeWindTokens.colors,
    bg: khalaMobileBackground,
  },
} as const

export const khalaMobileTheme = {
  background: khalaMobileTokens.colors.bg,
  surface: khalaMobileTokens.colors.surface,
  surfaceRaised: khalaMobileTokens.colors.surfaceRaised,
  surfaceActive: khalaMobileTokens.colors.surfaceActive,
  surfaceMuted: khalaMobileTokens.colors.surfaceMuted,
  border: khalaMobileTokens.colors.border,
  borderMuted: khalaMobileTokens.colors.borderMuted,
  borderStrong: khalaMobileTokens.colors.borderStrong,
  accent: khalaMobileTokens.colors.accent,
  accentSoft: khalaMobileTokens.colors.accentSoft,
  accentText: khalaMobileTokens.colors.accentText,
  text: khalaMobileTokens.colors.text,
  textBody: khalaMobileTokens.colors.textBody,
  textSoft: khalaMobileTokens.colors.textSoft,
  textMuted: khalaMobileTokens.colors.textMuted,
  textFaint: khalaMobileTokens.colors.textFaint,
  success: khalaMobileTokens.colors.success,
  warning: khalaMobileTokens.colors.warning,
  danger: khalaMobileTokens.colors.danger
} as const

export type KhalaMobileTheme = typeof khalaMobileTheme
