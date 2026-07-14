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
/**
 * Shared bounded-layout lattice.
 *
 * The compact steps cover host chrome that cannot be expressed as spacing
 * (meter tracks, attachment thumbnails, and minimum composer height). 2xl
 * is the shared long-form reading measure used by Desktop surfaces. Keeping
 * these values in the closed lattice prevents applications from growing
 * per-call-site numeric dimension allowlists.
 */
export const dimensionTokens = [
  "4xs",
  "3xs",
  "2xs",
  "xs",
  "sm",
  "md",
  "lg",
  "xl",
  "2xl",
  "full"
] as const
/**
 * The shared control size lattice: one metric system for every control.
 * `2xs`/`xs` serve dense application chrome (toolbars, inline chips, status
 * rows); `sm`…`xl` are the standard control heights.
 */
export const controlTokens = ["2xs", "xs", "sm", "md", "lg", "xl"] as const

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

// ---------------------------------------------------------------------------
// Tier 1 — primitive color ramps (apps-sdk-ui harmonization P0.1, issue #74)
// ---------------------------------------------------------------------------
// Raw color ramps with no semantic meaning: the bottom tier of the
// primitive → semantic → component token architecture. Semantic themes
// (tier 2) derive their color roles from ramp steps instead of free-floating
// hex, which keeps future colors on-brand by construction. Steps are named
// on a 0…1000 lightness axis: a lower step is always lighter (higher
// relative luminance) than a higher step within the same ramp.

/** A six-digit opaque hex color — the only value form ramp steps may hold. */
export const OpaqueColorValueSchema = Schema.String.check(
  Schema.isPattern(/^#[0-9a-f]{6}$/, { title: "OpaqueHexColor" })
)
/**
 * One alpha step: a two-digit lowercase hex byte. Appended to an opaque ramp
 * step (via `withAlpha`) it forms an 8-digit `#rrggbbaa` overlay color — the
 * alpha-overlay state engine's value form. Step names are the approximate
 * opacity percentage; the byte is `round(percent × 255 / 100)` pinned as a
 * literal so derived overlay colors stay byte-exact.
 */
export const AlphaChannelValueSchema = Schema.String.check(
  Schema.isPattern(/^[0-9a-f]{2}$/, { title: "AlphaHexByte" })
)

export const blueRampSteps = [
  "25",
  "50",
  "100",
  "200",
  "300",
  "400",
  "500",
  "600",
  "700",
  "800",
  "900",
  "1000"
] as const
export const grayRampSteps = [
  "0",
  "25",
  "50",
  "100",
  "200",
  "300",
  "400",
  "450",
  "500",
  "600",
  "700",
  "750",
  "800",
  "850",
  "900",
  "925",
  "950",
  "1000"
] as const
export const statusRampSteps = ["300", "400", "500", "600"] as const
export const alphaRampSteps = [
  "4",
  "5",
  "8",
  "10",
  "13",
  "16",
  "20",
  "24",
  "30",
  "40",
  "60",
  "86"
] as const

export const BlueRampSchema = Schema.Struct(
  tokenRecordFields(blueRampSteps, OpaqueColorValueSchema)
)
export const GrayRampSchema = Schema.Struct(
  tokenRecordFields(grayRampSteps, OpaqueColorValueSchema)
)
export const StatusRampSchema = Schema.Struct(
  tokenRecordFields(statusRampSteps, OpaqueColorValueSchema)
)
export const AlphaRampSchema = Schema.Struct(
  tokenRecordFields(alphaRampSteps, AlphaChannelValueSchema)
)

/**
 * The full tier-1 primitive palette: one brand ramp (Protoss blue), one cool
 * gray ramp (blue-tinted navy neutrals), four status hue ramps plus a violet
 * syntax-accent ramp, and the alpha steps used by the overlay state engine.
 */
export const PaletteSchema = Schema.Struct({
  blue: BlueRampSchema,
  gray: GrayRampSchema,
  red: StatusRampSchema,
  green: StatusRampSchema,
  amber: StatusRampSchema,
  cyan: StatusRampSchema,
  violet: StatusRampSchema,
  alpha: AlphaRampSchema
})

export type BlueRampStep = (typeof blueRampSteps)[number]
export type GrayRampStep = (typeof grayRampSteps)[number]
export type StatusRampStep = (typeof statusRampSteps)[number]
export type AlphaRampStep = (typeof alphaRampSteps)[number]
export type Palette = Schema.Schema.Type<typeof PaletteSchema>

const decodeOpaqueColor = Schema.decodeUnknownSync(OpaqueColorValueSchema)
const decodeAlphaChannel = Schema.decodeUnknownSync(AlphaChannelValueSchema)

/**
 * Derive an 8-digit `#rrggbbaa` overlay color from an opaque ramp step and an
 * alpha step. This is the only sanctioned way to mint translucent colors from
 * the palette: interactive state changes are alpha overlays of one base hue,
 * never new hues.
 */
export const withAlpha = (color: string, alpha: string): string =>
  `${decodeOpaqueColor(color)}${decodeAlphaChannel(alpha)}`

/**
 * The Khala primitive palette (tier 1). Every khalaTheme semantic color role
 * derives from these steps — see the derivation table in `khalaTheme` and the
 * exact-hex pin in `test/khala-theme.test.ts` ("pins the palette so drift is
 * visible in review"), which guarantees the ramp refactor changed nothing
 * visually. Steps that no semantic role consumes yet exist so future colors
 * are picked from the ramp instead of hand-mixed.
 */
export const khalaPalette = PaletteSchema.make({
  // Protoss blue: the single brand hue. 500 is the canonical accent.
  blue: {
    "25": "#f2f7ff",
    "50": "#e0ecff",
    "100": "#c2d9ff",
    "200": "#8fb3ff",
    "300": "#60a5fa",
    "400": "#5c96f8",
    "500": "#3b82f6",
    "600": "#2f6fe0",
    "700": "#285dbd",
    "800": "#204a99",
    "900": "#183770",
    "1000": "#0f234a"
  },
  // Cool gray: blue-tinted navy neutrals for text, borders, and surfaces.
  gray: {
    "0": "#ffffff",
    "25": "#eef3ff",
    "50": "#d6e0f5",
    "100": "#bccbe8",
    "200": "#a7b8d6",
    "300": "#93a4c3",
    "400": "#6b7ca1",
    "450": "#5b6b8c",
    "500": "#55648a",
    "600": "#2c3d63",
    "700": "#1f2b45",
    "750": "#182640",
    "800": "#16203a",
    "850": "#141f36",
    "900": "#0b1220",
    "925": "#0a0f1c",
    "950": "#05070d",
    "1000": "#02040a"
  },
  red: {
    "300": "#fca5a5",
    "400": "#f87171",
    "500": "#ef4444",
    "600": "#dc2626"
  },
  green: {
    "300": "#86efac",
    "400": "#4ade80",
    "500": "#22c55e",
    "600": "#16a34a"
  },
  amber: {
    "300": "#fcd34d",
    "400": "#fbbf24",
    "500": "#f59e0b",
    "600": "#d97706"
  },
  cyan: {
    "300": "#7dd3fc",
    "400": "#38bdf8",
    "500": "#0ea5e9",
    "600": "#0284c7"
  },
  violet: {
    "300": "#d8b4fe",
    "400": "#c084fc",
    "500": "#a855f7",
    "600": "#9333ea"
  },
  // Alpha steps: name ≈ opacity percent; byte = round(percent × 255 / 100).
  alpha: {
    "4": "0a",
    "5": "0d",
    "8": "14",
    "10": "1a",
    "13": "21",
    "16": "29",
    "20": "33",
    "24": "3d",
    "30": "4d",
    "40": "66",
    "60": "99",
    "86": "db"
  }
})

// ---------------------------------------------------------------------------
// Tier 2 — the tone × variant × state color matrix (harmonization P0.2, #75)
// ---------------------------------------------------------------------------
// The apps-sdk-ui symmetric semantic color grid as typed schema:
// `colorMatrix[tone][variant][state]` resolves to background/border/text/ring
// roles. Interactive states resolve through the existing alpha-overlay state
// engine — on dark themes states lighten via translucent overlays of one base
// hue, never new hues. Components (Button, Badge, Alert, Chip, SelectControl)
// consume the matrix in a later catalog bump; this tier is theme data only.

export const toneTokens = [
  "accent",
  "secondary",
  "danger",
  "success",
  "warning",
  "info"
] as const
export const toneVariantTokens = ["solid", "soft", "outline", "ghost"] as const
export const toneStateTokens = ["rest", "hover", "active", "selected", "disabled"] as const

export const ToneTokenSchema = Schema.Literals(toneTokens)
export const ToneVariantTokenSchema = Schema.Literals(toneVariantTokens)
export const ToneStateTokenSchema = Schema.Literals(toneStateTokens)

export type ToneToken = (typeof toneTokens)[number]
export type ToneVariantToken = (typeof toneVariantTokens)[number]
export type ToneStateToken = (typeof toneStateTokens)[number]

/** The fully transparent color: ghost/outline resting fills and borders. */
export const transparentColor = "#00000000" as const

/** One matrix cell: the four color roles a control resolves for a state. */
export const ToneCellSchema = Schema.Struct({
  background: ColorValueSchema,
  border: ColorValueSchema,
  text: ColorValueSchema,
  ring: ColorValueSchema
})
export const ToneStateMapSchema = Schema.Struct(
  tokenRecordFields(toneStateTokens, ToneCellSchema)
)
export const ToneVariantMapSchema = Schema.Struct(
  tokenRecordFields(toneVariantTokens, ToneStateMapSchema)
)
export const ColorMatrixSchema = Schema.Struct(
  tokenRecordFields(toneTokens, ToneVariantMapSchema)
)

export type ToneCell = Schema.Schema.Type<typeof ToneCellSchema>
export type ToneStateMap = Schema.Schema.Type<typeof ToneStateMapSchema>
export type ToneVariantMap = Schema.Schema.Type<typeof ToneVariantMapSchema>
export type ColorMatrix = Schema.Schema.Type<typeof ColorMatrixSchema>

/**
 * The per-tone inputs the matrix derivation consumes. Solid states carry
 * explicit ramp steps (dark themes lighten on hover; active presses one step
 * the other way — the existing accent/accentHover/accentActive engine).
 * Non-solid states are alpha overlays of one base hue.
 */
interface ToneSpec {
  readonly foreground: string
  readonly solidBackground: string
  readonly solidBackgroundHover: string
  readonly solidBackgroundActive: string
  readonly solidText: string
  readonly solidBorder: string
  readonly hoverOverlay: string
  readonly activeOverlay: string
  readonly selectedOverlay: string
  readonly softBackground: string
  readonly softBackgroundHover: string
  readonly softBackgroundActive: string
  readonly outlineBorder: string
  readonly ring: string
  readonly disabledText: string
}

/**
 * Expand one tone spec into the full variant × state grid. Disabled cells
 * keep the resting fill with disabled text and no ring; renderers additionally
 * apply the existing opacity dim for disabled controls (unchanged engine).
 */
const toneCells = (spec: ToneSpec): ToneVariantMap => ({
  solid: {
    rest: {
      background: spec.solidBackground,
      border: spec.solidBorder,
      text: spec.solidText,
      ring: spec.ring
    },
    hover: {
      background: spec.solidBackgroundHover,
      border: spec.solidBorder,
      text: spec.solidText,
      ring: spec.ring
    },
    active: {
      background: spec.solidBackgroundActive,
      border: spec.solidBorder,
      text: spec.solidText,
      ring: spec.ring
    },
    selected: {
      background: spec.solidBackgroundActive,
      border: spec.solidBorder,
      text: spec.solidText,
      ring: spec.ring
    },
    disabled: {
      background: spec.solidBackground,
      border: spec.solidBorder,
      text: spec.disabledText,
      ring: transparentColor
    }
  },
  soft: {
    rest: {
      background: spec.softBackground,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    hover: {
      background: spec.softBackgroundHover,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    active: {
      background: spec.softBackgroundActive,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    selected: {
      background: spec.softBackgroundActive,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    disabled: {
      background: spec.softBackground,
      border: transparentColor,
      text: spec.disabledText,
      ring: transparentColor
    }
  },
  outline: {
    rest: {
      background: transparentColor,
      border: spec.outlineBorder,
      text: spec.foreground,
      ring: spec.ring
    },
    hover: {
      background: spec.hoverOverlay,
      border: spec.outlineBorder,
      text: spec.foreground,
      ring: spec.ring
    },
    active: {
      background: spec.activeOverlay,
      border: spec.outlineBorder,
      text: spec.foreground,
      ring: spec.ring
    },
    selected: {
      background: spec.selectedOverlay,
      border: spec.outlineBorder,
      text: spec.foreground,
      ring: spec.ring
    },
    disabled: {
      background: transparentColor,
      border: spec.outlineBorder,
      text: spec.disabledText,
      ring: transparentColor
    }
  },
  ghost: {
    rest: {
      background: transparentColor,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    hover: {
      background: spec.hoverOverlay,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    active: {
      background: spec.activeOverlay,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    selected: {
      background: spec.selectedOverlay,
      border: transparentColor,
      text: spec.foreground,
      ring: spec.ring
    },
    disabled: {
      background: transparentColor,
      border: transparentColor,
      text: spec.disabledText,
      ring: transparentColor
    }
  }
})

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
  /** The apps-sdk "basic" transition duration (150ms in shipped themes). */
  durationFastMs: NonNegativeNumberSchema,
  durationEnterMs: NonNegativeNumberSchema,
  durationExitMs: NonNegativeNumberSchema,
  // Continuous-loop indicator duration (Spinner/LoadingDots/ShimmerText,
  // issue #83): one named base period consuming components scale via
  // multiplier rather than minting their own duration tokens.
  durationLoopMs: NonNegativeNumberSchema,
  easeBasic: EasingValueSchema,
  // The named easing vocabulary (apps-sdk-ui harmonization C6): enter/exit
  // for overlay lifecycles, exitSnappy for less inertia delay on dismissal,
  // move for on-screen positional transitions.
  easeEnter: EasingValueSchema,
  easeExit: EasingValueSchema,
  easeExitSnappy: EasingValueSchema,
  easeMove: EasingValueSchema
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
/**
 * One control-lattice step: fixed height paired with the horizontal gutter,
 * corner radius, label font size, and icon size that coherently size a
 * control, its padding, its text, and its icon from one `size` value —
 * the apps-sdk `--control-*` family as typed sub-tokens (harmonization C3).
 */
export const ControlSizeValueSchema = Schema.Struct({
  height: PositiveNumberSchema,
  gutter: NonNegativeNumberSchema,
  radius: NonNegativeNumberSchema,
  fontSize: PositiveNumberSchema,
  icon: PositiveNumberSchema
})
export const ControlThemeSchema = Schema.Struct(
  tokenRecordFields(controlTokens, ControlSizeValueSchema)
)

export const ThemeSchema = Schema.Struct({
  spacing: SpacingThemeSchema,
  color: ColorThemeSchema,
  colorMatrix: ColorMatrixSchema,
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

/**
 * The neutral fixture theme's color matrix. Light engine: hovers darken via
 * slate overlays (`#0f172a` at the theme's stateHover/stateActive alphas);
 * status solids darken one ramp step on hover/active; selection is
 * accent-tinted (the theme's stateSelected value for accent/secondary).
 */
const defaultColorMatrix: ColorMatrix = {
  accent: toneCells({
    foreground: "#2563eb",
    solidBackground: "#2563eb",
    solidBackgroundHover: "#1d4ed8",
    solidBackgroundActive: "#1e40af",
    solidText: "#ffffff",
    solidBorder: transparentColor,
    hoverOverlay: "#0f172a0a",
    activeOverlay: "#0f172a14",
    selectedOverlay: "#2563eb29",
    softBackground: "#2563eb29",
    softBackgroundHover: "#2563eb33",
    softBackgroundActive: "#2563eb3d",
    outlineBorder: "#2563eb",
    ring: "#93c5fd",
    disabledText: "#cbd5e1"
  }),
  secondary: toneCells({
    foreground: "#0f172a",
    solidBackground: "#f8fafc",
    solidBackgroundHover: "#eef2f7",
    solidBackgroundActive: "#e2e8f0",
    solidText: "#0f172a",
    solidBorder: "#cbd5e1",
    hoverOverlay: "#0f172a0a",
    activeOverlay: "#0f172a14",
    selectedOverlay: "#2563eb29",
    softBackground: "#0f172a0a",
    softBackgroundHover: "#0f172a14",
    softBackgroundActive: "#0f172a29",
    outlineBorder: "#94a3b8",
    ring: "#93c5fd",
    disabledText: "#cbd5e1"
  }),
  danger: toneCells({
    foreground: "#dc2626",
    solidBackground: "#dc2626",
    solidBackgroundHover: "#b91c1c",
    solidBackgroundActive: "#991b1b",
    solidText: "#ffffff",
    solidBorder: transparentColor,
    hoverOverlay: "#dc26260a",
    activeOverlay: "#dc262614",
    selectedOverlay: "#dc262629",
    softBackground: "#dc262629",
    softBackgroundHover: "#dc262633",
    softBackgroundActive: "#dc26263d",
    outlineBorder: "#dc2626",
    ring: "#fca5a5",
    disabledText: "#cbd5e1"
  }),
  success: toneCells({
    foreground: "#16a34a",
    solidBackground: "#16a34a",
    solidBackgroundHover: "#15803d",
    solidBackgroundActive: "#166534",
    solidText: "#ffffff",
    solidBorder: transparentColor,
    hoverOverlay: "#16a34a0a",
    activeOverlay: "#16a34a14",
    selectedOverlay: "#16a34a29",
    softBackground: "#16a34a29",
    softBackgroundHover: "#16a34a33",
    softBackgroundActive: "#16a34a3d",
    outlineBorder: "#16a34a",
    ring: "#86efac",
    disabledText: "#cbd5e1"
  }),
  warning: toneCells({
    foreground: "#d97706",
    solidBackground: "#d97706",
    solidBackgroundHover: "#b45309",
    solidBackgroundActive: "#92400e",
    solidText: "#ffffff",
    solidBorder: transparentColor,
    hoverOverlay: "#d977060a",
    activeOverlay: "#d9770614",
    selectedOverlay: "#d9770629",
    softBackground: "#d9770629",
    softBackgroundHover: "#d9770633",
    softBackgroundActive: "#d977063d",
    outlineBorder: "#d97706",
    ring: "#fcd34d",
    disabledText: "#cbd5e1"
  }),
  info: toneCells({
    foreground: "#0ea5e9",
    solidBackground: "#0ea5e9",
    solidBackgroundHover: "#0284c7",
    solidBackgroundActive: "#0369a1",
    solidText: "#ffffff",
    solidBorder: transparentColor,
    hoverOverlay: "#0ea5e90a",
    activeOverlay: "#0ea5e914",
    selectedOverlay: "#0ea5e929",
    softBackground: "#0ea5e929",
    softBackgroundHover: "#0ea5e933",
    softBackgroundActive: "#0ea5e93d",
    outlineBorder: "#0ea5e9",
    ring: "#7dd3fc",
    disabledText: "#cbd5e1"
  })
}

/**
 * The Khala color matrix (tier 2, harmonization P0.2): every cell derives
 * from `khalaPalette` ramp steps and alpha overlays. Protoss blue stays the
 * primary tone. The accent solid/ghost cells reproduce the exact colors the
 * current Button lowering uses (accent/accentHover/accentActive fills and the
 * stateHover/stateActive/stateSelected overlays), pinned by test.
 */
const khalaColorMatrix: ColorMatrix = {
  accent: toneCells({
    foreground: khalaPalette.blue["500"],
    solidBackground: khalaPalette.blue["500"],
    solidBackgroundHover: khalaPalette.blue["400"],
    solidBackgroundActive: khalaPalette.blue["600"],
    solidText: khalaPalette.gray["25"],
    solidBorder: transparentColor,
    hoverOverlay: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["16"]),
    softBackgroundHover: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["20"]),
    softBackgroundActive: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["24"]),
    outlineBorder: khalaPalette.blue["500"],
    ring: khalaPalette.blue["300"],
    disabledText: khalaPalette.gray["500"]
  }),
  secondary: toneCells({
    foreground: khalaPalette.gray["25"],
    solidBackground: khalaPalette.gray["900"],
    solidBackgroundHover: khalaPalette.gray["850"],
    solidBackgroundActive: khalaPalette.gray["750"],
    solidText: khalaPalette.gray["25"],
    solidBorder: khalaPalette.gray["700"],
    hoverOverlay: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["8"]),
    softBackgroundHover: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["13"]),
    softBackgroundActive: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["16"]),
    outlineBorder: khalaPalette.gray["600"],
    ring: khalaPalette.blue["300"],
    disabledText: khalaPalette.gray["500"]
  }),
  danger: toneCells({
    foreground: khalaPalette.red["400"],
    solidBackground: khalaPalette.red["400"],
    solidBackgroundHover: khalaPalette.red["300"],
    solidBackgroundActive: khalaPalette.red["500"],
    solidText: khalaPalette.gray["950"],
    solidBorder: transparentColor,
    hoverOverlay: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["16"]),
    softBackgroundHover: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["20"]),
    softBackgroundActive: withAlpha(khalaPalette.red["400"], khalaPalette.alpha["24"]),
    outlineBorder: khalaPalette.red["400"],
    ring: khalaPalette.red["300"],
    disabledText: khalaPalette.gray["500"]
  }),
  success: toneCells({
    foreground: khalaPalette.green["500"],
    solidBackground: khalaPalette.green["500"],
    solidBackgroundHover: khalaPalette.green["400"],
    solidBackgroundActive: khalaPalette.green["600"],
    solidText: khalaPalette.gray["950"],
    solidBorder: transparentColor,
    hoverOverlay: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["16"]),
    softBackgroundHover: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["20"]),
    softBackgroundActive: withAlpha(khalaPalette.green["500"], khalaPalette.alpha["24"]),
    outlineBorder: khalaPalette.green["500"],
    ring: khalaPalette.green["300"],
    disabledText: khalaPalette.gray["500"]
  }),
  warning: toneCells({
    foreground: khalaPalette.amber["500"],
    solidBackground: khalaPalette.amber["500"],
    solidBackgroundHover: khalaPalette.amber["400"],
    solidBackgroundActive: khalaPalette.amber["600"],
    solidText: khalaPalette.gray["950"],
    solidBorder: transparentColor,
    hoverOverlay: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["16"]),
    softBackgroundHover: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["20"]),
    softBackgroundActive: withAlpha(khalaPalette.amber["500"], khalaPalette.alpha["24"]),
    outlineBorder: khalaPalette.amber["500"],
    ring: khalaPalette.amber["300"],
    disabledText: khalaPalette.gray["500"]
  }),
  info: toneCells({
    foreground: khalaPalette.cyan["400"],
    solidBackground: khalaPalette.cyan["400"],
    solidBackgroundHover: khalaPalette.cyan["300"],
    solidBackgroundActive: khalaPalette.cyan["500"],
    solidText: khalaPalette.gray["950"],
    solidBorder: transparentColor,
    hoverOverlay: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["8"]),
    activeOverlay: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["13"]),
    selectedOverlay: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["16"]),
    softBackground: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["16"]),
    softBackgroundHover: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["20"]),
    softBackgroundActive: withAlpha(khalaPalette.cyan["400"], khalaPalette.alpha["24"]),
    outlineBorder: khalaPalette.cyan["400"],
    ring: khalaPalette.cyan["300"],
    disabledText: khalaPalette.gray["500"]
  })
}

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
  colorMatrix: defaultColorMatrix,
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
    "4xs": 4,
    "3xs": 56,
    "2xs": 64,
    xs: 160,
    sm: 240,
    md: 320,
    lg: 480,
    xl: 640,
    "2xl": 840,
    full: "100%"
  },
  motion: {
    durationFastMs: 150,
    durationEnterMs: 350,
    durationExitMs: 200,
    durationLoopMs: 900,
    easeBasic: "ease",
    easeEnter: "cubic-bezier(0.19, 1, 0.22, 1)",
    easeExit: "cubic-bezier(0.8, 0, 0.4, 1)",
    easeExitSnappy: "cubic-bezier(0.65, 0, 0.4, 1)",
    easeMove: "cubic-bezier(0.65, 0, 0.35, 1)"
  },
  elevation: {
    overlayShadow: "0 10px 15px -3px rgba(15, 23, 42, 0.12), 0 4px 6px -4px rgba(15, 23, 42, 0.12)",
    hairlineWidth: 1
  },
  control: {
    "2xs": { height: 16, gutter: 4, radius: 2, fontSize: 11, icon: 10 },
    xs: { height: 20, gutter: 6, radius: 4, fontSize: 12, icon: 12 },
    sm: { height: 24, gutter: 8, radius: 4, fontSize: 12, icon: 14 },
    md: { height: 28, gutter: 10, radius: 6, fontSize: 14, icon: 16 },
    lg: { height: 32, gutter: 12, radius: 6, fontSize: 14, icon: 18 },
    xl: { height: 40, gutter: 14, radius: 8, fontSize: 16, icon: 20 }
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
 *
 * Every color role is a derivation from `khalaPalette` (tier 1 → tier 2);
 * no role holds free-floating hex. The derived values are pinned as exact
 * hex literals in `test/khala-theme.test.ts` so the ramp refactor is
 * mechanically proven to be zero-visual-change.
 */
export const khalaTheme = ThemeSchema.make({
  spacing: defaultTheme.spacing,
  color: {
    background: khalaPalette.gray["950"], // #05070d
    surface: khalaPalette.gray["900"], // #0b1220
    surfaceRaised: khalaPalette.gray["850"], // #141f36
    surfaceOverlay: khalaPalette.gray["750"], // #182640
    textPrimary: khalaPalette.gray["25"], // #eef3ff
    textMuted: khalaPalette.gray["300"], // #93a4c3
    textFaint: khalaPalette.gray["400"], // #6b7ca1
    textInverse: khalaPalette.gray["950"], // #05070d
    textDisabled: khalaPalette.gray["500"], // #55648a
    accent: khalaPalette.blue["500"], // #3b82f6
    accentHover: khalaPalette.blue["400"], // #5c96f8
    accentActive: khalaPalette.blue["600"], // #2f6fe0
    danger: khalaPalette.red["400"], // #f87171
    border: khalaPalette.gray["700"], // #1f2b45
    borderSubtle: khalaPalette.gray["800"], // #16203a
    borderStrong: khalaPalette.gray["600"], // #2c3d63
    focus: khalaPalette.blue["300"], // #60a5fa
    info: khalaPalette.cyan["400"], // #38bdf8
    success: khalaPalette.green["500"], // #22c55e
    warning: khalaPalette.amber["500"], // #f59e0b
    stateHover: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["8"]), // #8fb3ff14
    stateActive: withAlpha(khalaPalette.blue["200"], khalaPalette.alpha["13"]), // #8fb3ff21
    stateSelected: withAlpha(khalaPalette.blue["500"], khalaPalette.alpha["16"]), // #3b82f629
    scrim: withAlpha(khalaPalette.gray["1000"], khalaPalette.alpha["86"]), // #02040adb
    codeBackground: khalaPalette.gray["925"], // #0a0f1c
    diffAdd: khalaPalette.green["400"], // #4ade80
    diffRemove: khalaPalette.red["400"], // #f87171
    syntaxKeyword: khalaPalette.blue["300"], // #60a5fa
    syntaxString: khalaPalette.green["400"], // #4ade80
    syntaxComment: khalaPalette.gray["450"], // #5b6b8c
    syntaxFunction: khalaPalette.violet["400"], // #c084fc
    syntaxNumber: khalaPalette.amber["400"], // #fbbf24
    syntaxOperator: khalaPalette.gray["300"] // #93a4c3
  },
  colorMatrix: khalaColorMatrix,
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
  // The Khala control lattice: heights/gutters/icons unchanged from the
  // trimmed 4-step lattice (24/28/32/40 with 2xs/xs added for dense desktop
  // chrome). Radii sit on the theme's sharper radius scale (controls render
  // at radius-md = 4px today) and font sizes on the caption/label/body type
  // scale, so one `size` prop sizes a control coherently with zero drift
  // from current desktop rendering.
  control: {
    "2xs": { height: 16, gutter: 4, radius: 2, fontSize: 11, icon: 10 },
    xs: { height: 20, gutter: 6, radius: 4, fontSize: 12, icon: 12 },
    sm: { height: 24, gutter: 8, radius: 4, fontSize: 12, icon: 14 },
    md: { height: 28, gutter: 10, radius: 4, fontSize: 13, icon: 16 },
    lg: { height: 32, gutter: 12, radius: 4, fontSize: 14, icon: 18 },
    xl: { height: 40, gutter: 14, radius: 6, fontSize: 16, icon: 20 }
  }
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
