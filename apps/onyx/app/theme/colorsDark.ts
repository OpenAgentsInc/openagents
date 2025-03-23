const palette = {
  neutral900: "#fafafa", // zinc-50
  neutral800: "#f4f4f5", // zinc-100
  neutral700: "#e4e4e7", // zinc-200
  neutral600: "#d4d4d8", // zinc-300
  neutral500: "#a1a1aa", // zinc-400
  neutral400: "#71717a", // zinc-500
  neutral300: "#52525b", // zinc-600
  neutral200: "#27272a", // zinc-800
  neutral100: "#18181b", // zinc-900
  neutral50: "#09090b", // zinc-950

  primary600: "#f4f4f5", // zinc-100
  primary500: "#e4e4e7", // zinc-200
  primary400: "#d4d4d8", // zinc-300
  primary300: "#a1a1aa", // zinc-400
  primary200: "#71717a", // zinc-500
  primary100: "#52525b", // zinc-600

  secondary500: "#f4f4f5", // zinc-100
  secondary400: "#e4e4e7", // zinc-200
  secondary300: "#d4d4d8", // zinc-300
  secondary200: "#a1a1aa", // zinc-400
  secondary100: "#71717a", // zinc-500

  accent500: "#f4f4f5", // zinc-100
  accent400: "#e4e4e7", // zinc-200
  accent300: "#d4d4d8", // zinc-300
  accent200: "#a1a1aa", // zinc-400
  accent100: "#71717a", // zinc-500

  angry100: "#e4e4e7", // zinc-200
  angry500: "red",

  overlay20: "rgba(39, 39, 42, 0.2)", // zinc-800
  overlay50: "rgba(39, 39, 42, 0.5)", // zinc-800
} as const

export const colors = {
  palette,
  transparent: "rgba(0, 0, 0, 0)",
  text: palette.neutral800,
  textDim: palette.neutral600,
  background: "#000",
  backgroundSecondary: palette.neutral100,
  border: palette.neutral200,
  tint: palette.primary500,
  tintInactive: palette.neutral300,
  separator: palette.neutral300,
  error: palette.angry500,
  errorBackground: palette.angry100,
} as const
