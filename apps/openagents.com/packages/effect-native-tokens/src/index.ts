import { Context, Layer, Schema } from "effect"

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
  "surfaceRaised",
  "textPrimary",
  "textMuted",
  "accent",
  "danger",
  "border",
  "focus",
  "info",
  "success",
  "warning",
  "codeBackground",
  "diffAdd",
  "diffRemove",
  "syntaxKeyword",
  "syntaxString",
  "syntaxComment",
  "syntaxFunction",
  "syntaxNumber",
  "syntaxOperator"
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
    surfaceRaised: "#eef2f7",
    textPrimary: "#0f172a",
    textMuted: "#64748b",
    accent: "#2563eb",
    danger: "#dc2626",
    border: "#cbd5e1",
    focus: "#93c5fd",
    info: "#0ea5e9",
    success: "#16a34a",
    warning: "#d97706",
    codeBackground: "#f1f5f9",
    diffAdd: "#15803d",
    diffRemove: "#b91c1c",
    syntaxKeyword: "#1d4ed8",
    syntaxString: "#15803d",
    syntaxComment: "#64748b",
    syntaxFunction: "#7e22ce",
    syntaxNumber: "#b45309",
    syntaxOperator: "#334155"
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

/**
 * The single Khala Protoss-blue dark theme.
 *
 * Khala Code Desktop and every OpenAgents product surface share exactly one
 * uniform blue theme: deep near-black backgrounds, a blue-500/400 accent
 * family, and semantic/code/diff/syntax roles tuned to sit inside that same
 * blue system. There is intentionally no light variant and no runtime theme
 * switch — see workspace policy "uniform StarCraft blue everywhere" and
 * issue #25. Renderers and apps should treat this as the only theme value
 * they ever mount.
 */
export const khalaTheme = ThemeSchema.make({
  spacing: defaultTheme.spacing,
  color: {
    background: "#05070d",
    surface: "#0b1220",
    surfaceRaised: "#141f36",
    textPrimary: "#eef3ff",
    textMuted: "#93a4c3",
    accent: "#3b82f6",
    danger: "#f87171",
    border: "#1f2b45",
    focus: "#60a5fa",
    info: "#38bdf8",
    success: "#22c55e",
    warning: "#f59e0b",
    codeBackground: "#0a0f1c",
    diffAdd: "#4ade80",
    diffRemove: "#f87171",
    syntaxKeyword: "#60a5fa",
    syntaxString: "#4ade80",
    syntaxComment: "#5b6b8c",
    syntaxFunction: "#c084fc",
    syntaxNumber: "#fbbf24",
    syntaxOperator: "#93a4c3"
  },
  radius: {
    none: 0,
    sm: 2,
    md: 4,
    lg: 6,
    xl: 8,
    full: 9999
  },
  typeScale: {
    caption: { fontSize: 12, lineHeight: 16, fontWeight: 500 },
    body: { fontSize: 14, lineHeight: 21, fontWeight: 400 },
    label: { fontSize: 13, lineHeight: 18, fontWeight: 600 },
    title: { fontSize: 18, lineHeight: 24, fontWeight: 600 },
    heading: { fontSize: 24, lineHeight: 30, fontWeight: 600 }
  },
  breakpoint: defaultTheme.breakpoint,
  dimension: defaultTheme.dimension
})

/**
 * Effect service tag for the single active theme. There is deliberately no
 * "current mode" concept: a consumer either provides `khalaThemeLayer` (the
 * only sanctioned production layer) or a bespoke `Layer.succeed(ThemeService,
 * someTheme)` in tests/tooling. No light layer is exported.
 */
export const ThemeService = Context.Service<Theme>("@effect-native/tokens/ThemeService")

export const makeThemeLayer = (theme: Theme) => Layer.succeed(ThemeService, theme)

/** The only theme Layer wired into Khala product surfaces. */
export const khalaThemeLayer = makeThemeLayer(khalaTheme)
