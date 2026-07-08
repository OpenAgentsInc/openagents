import { Schema } from "effect"

export const packageName = "@effect-native/tokens" as const

export const spacingTokens = [
  "0",
  "0.5",
  "1",
  "1.5",
  "2",
  "2.5",
  "3",
  "3.5",
  "4",
  "5",
  "6",
  "8",
  "10",
  "12",
  "16",
  "20",
  "24",
  "32",
  "40",
  "48",
  "56",
  "64"
] as const

export const colorTokens = [
  "background",
  "surface",
  "textPrimary",
  "textMuted",
  "accent",
  "danger",
  "border",
  "focus"
] as const

export const radiusTokens = ["none", "sm", "md", "lg", "xl", "full"] as const
export const typeScaleTokens = ["caption", "body", "label", "title", "heading"] as const
export const breakpointTokens = ["sm", "md", "lg", "xl"] as const
export const dimensionTokens = ["xs", "sm", "md", "lg", "xl", "full"] as const

export const SpacingTokenSchema = Schema.Literals(spacingTokens)
export const ColorTokenSchema = Schema.Literals(colorTokens)
export const RadiusTokenSchema = Schema.Literals(radiusTokens)
export const TypeScaleTokenSchema = Schema.Literals(typeScaleTokens)
export const BreakpointTokenSchema = Schema.Literals(breakpointTokens)
export const DimensionTokenSchema = Schema.Literals(dimensionTokens)

export type SpacingToken = (typeof spacingTokens)[number]
export type ColorToken = (typeof colorTokens)[number]
export type RadiusToken = (typeof radiusTokens)[number]
export type TypeScaleToken = (typeof typeScaleTokens)[number]
export type BreakpointToken = (typeof breakpointTokens)[number]
export type DimensionToken = (typeof dimensionTokens)[number]

export const NonNegativeNumberSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "NonNegativeNumber" })
)
export const PositiveNumberSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThan(0, { title: "PositiveNumber" })
)
export const ColorValueSchema = Schema.String.check(
  Schema.isPattern(/^#(?:[0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i, {
    title: "HexColor"
  })
)
export const FontWeightValueSchema = Schema.Literals([400, 500, 600, 700] as const)

const tokenRecordFields = <const Keys extends ReadonlyArray<string>, Value extends Schema.Constraint>(
  keys: Keys,
  value: Value
): { readonly [Key in Keys[number]]: Value } =>
  Object.fromEntries(keys.map((key) => [key, value])) as { readonly [Key in Keys[number]]: Value }

export const SpacingThemeSchema = Schema.Struct(
  tokenRecordFields(spacingTokens, NonNegativeNumberSchema)
)
export const ColorThemeSchema = Schema.Struct(
  tokenRecordFields(colorTokens, ColorValueSchema)
)
export const RadiusThemeSchema = Schema.Struct(
  tokenRecordFields(radiusTokens, NonNegativeNumberSchema)
)
export const TypeScaleValueSchema = Schema.Struct({
  fontSize: PositiveNumberSchema,
  lineHeight: PositiveNumberSchema,
  fontWeight: FontWeightValueSchema
})
export const TypeScaleThemeSchema = Schema.Struct(
  tokenRecordFields(typeScaleTokens, TypeScaleValueSchema)
)
export const BreakpointThemeSchema = Schema.Struct(
  tokenRecordFields(breakpointTokens, NonNegativeNumberSchema)
)
export const DimensionThemeSchema = Schema.Struct(
  tokenRecordFields(dimensionTokens, Schema.Union([NonNegativeNumberSchema, Schema.Literal("100%")]))
)

export const ThemeSchema = Schema.Struct({
  spacing: SpacingThemeSchema,
  color: ColorThemeSchema,
  radius: RadiusThemeSchema,
  typeScale: TypeScaleThemeSchema,
  breakpoint: BreakpointThemeSchema,
  dimension: DimensionThemeSchema
})

export type SpacingTheme = Schema.Schema.Type<typeof SpacingThemeSchema>
export type ColorTheme = Schema.Schema.Type<typeof ColorThemeSchema>
export type RadiusTheme = Schema.Schema.Type<typeof RadiusThemeSchema>
export type TypeScaleValue = Schema.Schema.Type<typeof TypeScaleValueSchema>
export type TypeScaleTheme = Schema.Schema.Type<typeof TypeScaleThemeSchema>
export type BreakpointTheme = Schema.Schema.Type<typeof BreakpointThemeSchema>
export type DimensionTheme = Schema.Schema.Type<typeof DimensionThemeSchema>
export type Theme = Schema.Schema.Type<typeof ThemeSchema>

export const defaultTheme = ThemeSchema.make({
  spacing: {
    "0": 0,
    "0.5": 2,
    "1": 4,
    "1.5": 6,
    "2": 8,
    "2.5": 10,
    "3": 12,
    "3.5": 14,
    "4": 16,
    "5": 20,
    "6": 24,
    "8": 32,
    "10": 40,
    "12": 48,
    "16": 64,
    "20": 80,
    "24": 96,
    "32": 128,
    "40": 160,
    "48": 192,
    "56": 224,
    "64": 256
  },
  color: {
    background: "#ffffff",
    surface: "#f8fafc",
    textPrimary: "#0f172a",
    textMuted: "#64748b",
    accent: "#2563eb",
    danger: "#dc2626",
    border: "#cbd5e1",
    focus: "#93c5fd"
  },
  radius: {
    none: 0,
    sm: 2,
    md: 6,
    lg: 8,
    xl: 12,
    full: 9999
  },
  typeScale: {
    caption: { fontSize: 12, lineHeight: 16, fontWeight: 400 },
    body: { fontSize: 16, lineHeight: 24, fontWeight: 400 },
    label: { fontSize: 14, lineHeight: 20, fontWeight: 500 },
    title: { fontSize: 20, lineHeight: 28, fontWeight: 600 },
    heading: { fontSize: 30, lineHeight: 36, fontWeight: 700 }
  },
  breakpoint: {
    sm: 640,
    md: 768,
    lg: 1024,
    xl: 1280
  },
  dimension: {
    xs: 160,
    sm: 240,
    md: 320,
    lg: 480,
    xl: 640,
    full: "100%"
  }
})

export const defineTheme = (theme: Theme): Theme => ThemeSchema.make(theme)
export const decodeTheme = Schema.decodeUnknownSync(ThemeSchema)
export const encodeTheme = Schema.encodeSync(ThemeSchema)
