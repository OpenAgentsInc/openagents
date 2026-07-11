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
  // One step above surfaceRaised: menus, popovers, command palettes,
  // tooltips — the floating-overlay surface (elevation = lighter).
  "surfaceOverlay",
  "textPrimary",
  "textMuted",
  // Third dim level below textMuted: placeholders, hints, chevrons, meta.
  "textFaint",
  // Text on solid accent/success/danger fills.
  "textInverse",
  // Disabled control labels.
  "textDisabled",
  "accent",
  // Solid-accent hover/active steps (dark themes lighten on hover).
  "accentHover",
  "accentActive",
  "danger",
  "border",
  // Hairline separators inside panels (weaker than border).
  "borderSubtle",
  // Hover borders and outline-variant rest borders (stronger than border).
  "borderStrong",
  "focus",
  "info",
  "success",
  "warning",
  // The alpha-overlay state engine: interactive state changes are
  // translucent overlays of one base color, never new hues. Values are
  // 8-digit hex with alpha (ColorValueSchema accepts #rrggbbaa).
  "stateHover",
  "stateActive",
  "stateSelected",
  // Modal / palette backdrop.
  "scrim",
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
/** The shared control size lattice: one metric system for every control. */
export const controlTokens = ["sm", "md", "lg", "xl"] as const

export const SpacingTokenSchema = Schema.Literals(spacingTokens)
export const ColorTokenSchema = Schema.Literals(colorTokens)
export const RadiusTokenSchema = Schema.Literals(radiusTokens)
export const TypeScaleTokenSchema = Schema.Literals(typeScaleTokens)
export const BreakpointTokenSchema = Schema.Literals(breakpointTokens)
export const DimensionTokenSchema = Schema.Literals(dimensionTokens)
export const ControlTokenSchema = Schema.Literals(controlTokens)

export type SpacingToken = (typeof spacingTokens)[number]
export type ColorToken = (typeof colorTokens)[number]
export type RadiusToken = (typeof radiusTokens)[number]
export type TypeScaleToken = (typeof typeScaleTokens)[number]
export type BreakpointToken = (typeof breakpointTokens)[number]
export type DimensionToken = (typeof dimensionTokens)[number]
export type ControlToken = (typeof controlTokens)[number]

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
/**
 * A CSS timing-function keyword or cubic-bezier expression. Renderers that
 * cannot consume CSS easing directly (e.g. React Native) map these to their
 * platform equivalents; the token value stays renderer-neutral data.
 */
export const EasingValueSchema = Schema.String.check(
  Schema.isPattern(
    /^(?:linear|ease|ease-in|ease-out|ease-in-out|cubic-bezier\(\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*,\s*-?\d*\.?\d+\s*\))$/,
    { title: "EasingValue" }
  )
)
/**
 * A shadow list ("x y blur spread color, …"). Bounded character set only —
 * no url(), no functions beyond rgba()/rgb().
 */
export const ShadowValueSchema = Schema.String.check(
  Schema.isPattern(/^[0-9a-zA-Z(),.#%\s-]+$/, { title: "ShadowValue" }),
  Schema.isMinLength(1)
)

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
/**
 * Motion vocabulary shared by every renderer: one basic transition for
 * hover/color/background state changes, and an enter/exit pair for
 * overlays. Durations are milliseconds.
 */
export const MotionThemeSchema = Schema.Struct({
  durationFastMs: NonNegativeNumberSchema,
  durationEnterMs: NonNegativeNumberSchema,
  durationExitMs: NonNegativeNumberSchema,
  easeBasic: EasingValueSchema,
  easeEnter: EasingValueSchema,
  easeExit: EasingValueSchema
})
/**
 * Elevation vocabulary: floating overlays (menus, popovers, palettes,
 * tooltips) carry `overlayShadow` plus a hairline ring of `borderSubtle`
 * at `hairlineWidth`. In-flow panels stay flat (border only, no shadow).
 */
export const ElevationThemeSchema = Schema.Struct({
  overlayShadow: ShadowValueSchema,
  hairlineWidth: NonNegativeNumberSchema
})
/** One control-lattice step: fixed height, horizontal gutter, icon size. */
export const ControlSizeValueSchema = Schema.Struct({
  height: PositiveNumberSchema,
  gutter: NonNegativeNumberSchema,
  icon: PositiveNumberSchema
})
export const ControlThemeSchema = Schema.Struct(
  tokenRecordFields(controlTokens, ControlSizeValueSchema)
)

export const ThemeSchema = Schema.Struct({
  spacing: SpacingThemeSchema,
  color: ColorThemeSchema,
  radius: RadiusThemeSchema,
  typeScale: TypeScaleThemeSchema,
  breakpoint: BreakpointThemeSchema,
  dimension: DimensionThemeSchema,
  motion: MotionThemeSchema,
  elevation: ElevationThemeSchema,
  control: ControlThemeSchema
})

export type SpacingTheme = Schema.Schema.Type<typeof SpacingThemeSchema>
export type ColorTheme = Schema.Schema.Type<typeof ColorThemeSchema>
export type RadiusTheme = Schema.Schema.Type<typeof RadiusThemeSchema>
export type TypeScaleValue = Schema.Schema.Type<typeof TypeScaleValueSchema>
export type TypeScaleTheme = Schema.Schema.Type<typeof TypeScaleThemeSchema>
export type BreakpointTheme = Schema.Schema.Type<typeof BreakpointThemeSchema>
export type DimensionTheme = Schema.Schema.Type<typeof DimensionThemeSchema>
export type MotionTheme = Schema.Schema.Type<typeof MotionThemeSchema>
export type ElevationTheme = Schema.Schema.Type<typeof ElevationThemeSchema>
export type ControlSizeValue = Schema.Schema.Type<typeof ControlSizeValueSchema>
export type ControlTheme = Schema.Schema.Type<typeof ControlThemeSchema>
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
    surfaceOverlay: "#ffffff",
    textPrimary: "#0f172a",
    textMuted: "#64748b",
    textFaint: "#94a3b8",
    textInverse: "#ffffff",
    textDisabled: "#cbd5e1",
    accent: "#2563eb",
    accentHover: "#1d4ed8",
    accentActive: "#1e40af",
    danger: "#dc2626",
    border: "#cbd5e1",
    borderSubtle: "#e2e8f0",
    borderStrong: "#94a3b8",
    focus: "#93c5fd",
    info: "#0ea5e9",
    success: "#16a34a",
    warning: "#d97706",
    stateHover: "#0f172a0a",
    stateActive: "#0f172a14",
    stateSelected: "#2563eb29",
    scrim: "#0f172a99",
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
  },
  motion: {
    durationFastMs: 150,
    durationEnterMs: 350,
    durationExitMs: 200,
    easeBasic: "ease",
    easeEnter: "cubic-bezier(0.19, 1, 0.22, 1)",
    easeExit: "cubic-bezier(0.8, 0, 0.4, 1)"
  },
  elevation: {
    overlayShadow: "0 10px 15px -3px rgba(15, 23, 42, 0.12), 0 4px 6px -4px rgba(15, 23, 42, 0.12)",
    hairlineWidth: 1
  },
  control: {
    sm: { height: 24, gutter: 8, icon: 14 },
    md: { height: 28, gutter: 10, icon: 16 },
    lg: { height: 32, gutter: 12, icon: 18 },
    xl: { height: 40, gutter: 14, icon: 20 }
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
    surfaceOverlay: "#182640",
    textPrimary: "#eef3ff",
    textMuted: "#93a4c3",
    textFaint: "#6b7ca1",
    textInverse: "#05070d",
    textDisabled: "#55648a",
    accent: "#3b82f6",
    accentHover: "#5c96f8",
    accentActive: "#2f6fe0",
    danger: "#f87171",
    border: "#1f2b45",
    borderSubtle: "#16203a",
    borderStrong: "#2c3d63",
    focus: "#60a5fa",
    info: "#38bdf8",
    success: "#22c55e",
    warning: "#f59e0b",
    stateHover: "#8fb3ff14",
    stateActive: "#8fb3ff21",
    stateSelected: "#3b82f629",
    scrim: "#02040adb",
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
  dimension: defaultTheme.dimension,
  motion: defaultTheme.motion,
  elevation: {
    overlayShadow: "0 10px 15px -3px rgba(0, 0, 0, 0.6), 0 4px 6px -4px rgba(0, 0, 0, 0.6)",
    hairlineWidth: 1
  },
  control: defaultTheme.control
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
