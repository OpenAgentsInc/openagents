export * from "./theme.js"

export type AutopilotStatusToneTokens = Readonly<{
  success: string
  warning: string
  danger: string
  info: string
}>

export type AutopilotCoreDarkTokens = Readonly<{
  background: string
  backgroundSecondary: string
  text: string
  textSecondary: string
  outline: string
  primary: string
  tones: AutopilotStatusToneTokens
}>

export type AutopilotCoreDarkCssVar =
  | "--bg"
  | "--bg-secondary"
  | "--text"
  | "--text-secondary"
  | "--outline"
  | "--primary"
  | "--success"
  | "--warning"
  | "--danger"
  | "--info"

export type AutopilotCoreNativeTheme = Readonly<{
  colors: Readonly<{
    background: string
    backgroundSecondary: string
    text: string
    textSecondary: string
    outline: string
    primary: string
    success: string
    warning: string
    danger: string
    info: string
  }>
}>

export type AutopilotCoreProtocolDarkTokens = Readonly<{
  bg: string
  bgSecondary: string
  text: string
  textSecondary: string
  outline: string
  primary: string
  success: string
  warning: string
  danger: string
  info: string
}>

export const autopilotCoreDarkTokens = {
  background: "#000",
  backgroundSecondary: "#151515",
  text: "#d7d8e5",
  textSecondary: "#8a8c93",
  outline: "#525458",
  primary: "#fff",
  tones: {
    success: "#00c853",
    warning: "#ffb400",
    danger: "#d32f2f",
    info: "#2979ff",
  },
} as const satisfies AutopilotCoreDarkTokens

export const autopilotCoreDarkCssVars = (
  tokens: AutopilotCoreDarkTokens,
): Record<AutopilotCoreDarkCssVar, string> => ({
  "--bg": tokens.background,
  "--bg-secondary": tokens.backgroundSecondary,
  "--text": tokens.text,
  "--text-secondary": tokens.textSecondary,
  "--outline": tokens.outline,
  "--primary": tokens.primary,
  "--success": tokens.tones.success,
  "--warning": tokens.tones.warning,
  "--danger": tokens.tones.danger,
  "--info": tokens.tones.info,
})

export const autopilotCoreNativeTheme = (
  tokens: AutopilotCoreDarkTokens,
): AutopilotCoreNativeTheme => ({
  colors: {
    background: tokens.background,
    backgroundSecondary: tokens.backgroundSecondary,
    text: tokens.text,
    textSecondary: tokens.textSecondary,
    outline: tokens.outline,
    primary: tokens.primary,
    success: tokens.tones.success,
    warning: tokens.tones.warning,
    danger: tokens.tones.danger,
    info: tokens.tones.info,
  },
})

export const autopilotCoreProtocolDarkTokens = (
  tokens: AutopilotCoreDarkTokens = autopilotCoreDarkTokens,
): AutopilotCoreProtocolDarkTokens => ({
  bg: tokens.background,
  bgSecondary: tokens.backgroundSecondary,
  text: tokens.text,
  textSecondary: tokens.textSecondary,
  outline: tokens.outline,
  primary: tokens.primary,
  success: tokens.tones.success,
  warning: tokens.tones.warning,
  danger: tokens.tones.danger,
  info: tokens.tones.info,
})
