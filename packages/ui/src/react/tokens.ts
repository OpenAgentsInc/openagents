import {
  colorTokens,
  fontSizeTokens,
  fontTokens,
  letterSpacingTokens,
  lineHeightTokens,
  radiusTokens,
  spaceTokens,
} from '@openagentsinc/design-tokens'

export const openAgentsReactTailwindTokens = {
  colors: {
    bg: 'var(--oa-color-bg)',
    surface: 'var(--oa-color-khala-surface)',
    surfaceRaised: 'var(--oa-color-khala-surface-raised)',
    surfaceActive: 'var(--oa-color-khala-surface-active)',
    border: 'var(--oa-color-khala-border)',
    borderStrong: 'var(--oa-color-khala-border-strong)',
    text: 'var(--oa-color-khala-text-primary)',
    textBody: 'var(--oa-color-khala-text-body)',
    textMuted: 'var(--oa-color-khala-text-muted)',
    accent: 'var(--oa-color-khala-energy-cyan)',
    accentSoft: 'var(--oa-color-khala-energy-cyan-soft)',
    success: 'var(--oa-color-khala-success-strong)',
    warning: 'var(--oa-color-khala-warning-strong)',
    danger: 'var(--oa-color-khala-danger-strong)',
  },
  fontFamily: {
    sans: 'var(--oa-font-sans)',
    mono: 'var(--oa-font-mono)',
  },
  borderRadius: {
    sm: 'var(--oa-radius-sm)',
    md: 'var(--oa-radius-md)',
    lg: 'var(--oa-radius-lg)',
    xl: 'var(--oa-radius-xl)',
  },
} as const

export const openAgentsNativeWindTokens = {
  colors: {
    bg: colorTokens.bg,
    surface: colorTokens.khalaSurface,
    surfaceRaised: colorTokens.khalaSurfaceRaised,
    surfaceActive: colorTokens.khalaSurfaceActive,
    surfaceMuted: colorTokens.khalaSurfaceMuted,
    border: colorTokens.khalaBorder,
    borderMuted: colorTokens.khalaBorderMuted,
    borderStrong: colorTokens.khalaBorderStrong,
    text: colorTokens.khalaTextPrimary,
    textBody: colorTokens.khalaTextBody,
    textSoft: colorTokens.khalaTextSoft,
    textMuted: colorTokens.khalaTextMuted,
    textFaint: colorTokens.khalaTextFaint,
    accent: colorTokens.khalaEnergyCyan,
    accentSoft: colorTokens.khalaEnergyCyanSoft,
    accentText: colorTokens.khalaEnergyTextStrong,
    success: colorTokens.khalaSuccessStrong,
    warning: colorTokens.khalaWarningStrong,
    danger: colorTokens.khalaDangerStrong,
    dangerHover: colorTokens.dangerHover,
    code: colorTokens.khalaCodePlain,
    codeMuted: colorTokens.khalaCodeComment,
  },
  fontFamily: {
    sans: fontTokens.sans,
    mono: fontTokens.mono,
    code: fontTokens.code,
  },
  fontSize: fontSizeTokens,
  lineHeight: lineHeightTokens,
  letterSpacing: letterSpacingTokens,
  borderRadius: radiusTokens,
  spacing: spaceTokens,
} as const

export type OpenAgentsNativeWindTokens = typeof openAgentsNativeWindTokens
