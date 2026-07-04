import { openAgentsNativeWindTokens } from "@openagentsinc/ui/react"

export const khalaMobileTokens = openAgentsNativeWindTokens

export const khalaMobileTheme = {
  background: khalaMobileTokens.colors.bg,
  surface: khalaMobileTokens.colors.surface,
  surfaceRaised: khalaMobileTokens.colors.surfaceRaised,
  surfaceActive: khalaMobileTokens.colors.surfaceActive,
  border: khalaMobileTokens.colors.border,
  accent: khalaMobileTokens.colors.accent,
  text: khalaMobileTokens.colors.text,
  textBody: khalaMobileTokens.colors.textBody,
  textMuted: khalaMobileTokens.colors.textMuted,
  success: khalaMobileTokens.colors.success,
  warning: khalaMobileTokens.colors.warning,
  danger: khalaMobileTokens.colors.danger
} as const

export type KhalaMobileTheme = typeof khalaMobileTheme
