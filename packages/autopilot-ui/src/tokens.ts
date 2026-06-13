export type StatusToneTokens = Readonly<{
  success: string
  warning: string
  danger: string
  info: string
}>

export type DarkTokens = Readonly<{
  background: string
  backgroundSecondary: string
  text: string
  textSecondary: string
  outline: string
  primary: string
  tones: StatusToneTokens
}>

export type DarkTokenCssVar =
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

export type NativeTheme = Readonly<{
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

export const darkTokens = {
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
} as const satisfies DarkTokens

export const cssVars = (tokens: DarkTokens): Record<DarkTokenCssVar, string> => ({
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

export const nativeTheme = (tokens: DarkTokens): NativeTheme => ({
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
