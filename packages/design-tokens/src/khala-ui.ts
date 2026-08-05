// Moved verbatim from the vendored Effect Native tokens package
// (`apps/openagents.com/packages/effect-native-tokens/src/khala-ui.ts`,
// upstream OpenAgentsInc/effect-native @ 467bde0) during the Effect Native
// removal, openagents#9325 packet 2. Only the `Effect` import (used solely by
// the motif-geometry resolvers, which did not move) was dropped. The token
// vocabulary itself is unchanged.
import { Schema } from "effect"

/** The complete KU-2 motif vocabulary. Adding a value is a contract change. */
export const khalaMotifIds = [
  "cut-corner-surface",
  "header-line",
  "signal-separator",
  "edge-underline",
  "corner-line-array",
  "corner-brackets",
  "octagonal-surface",
  "corner-chevron",
  "split-corner",
  "asymmetric-cut",
  "header-rail",
  "radial-dial"
] as const
export type KhalaMotifId = (typeof khalaMotifIds)[number]

export const khalaEdgeWidthTokens = ["hairline", "structural", "emphasis"] as const
export const khalaCutSizeTokens = ["none", "small", "medium", "large"] as const
export const khalaAccentLengthTokens = ["short", "medium", "long"] as const
export const khalaLuminanceRoles = ["quiet", "structural", "signal", "focus"] as const
export const khalaDensityTokens = ["compact", "comfortable", "spacious"] as const
export const khalaAmbientQualityTokens = ["off", "restrained", "enhanced"] as const
export const khalaCollapseRoles = ["border-only", "simplified", "full"] as const

export type KhalaEdgeWidthToken = (typeof khalaEdgeWidthTokens)[number]
export type KhalaCutSizeToken = (typeof khalaCutSizeTokens)[number]
export type KhalaAccentLengthToken = (typeof khalaAccentLengthTokens)[number]
export type KhalaLuminanceRole = (typeof khalaLuminanceRoles)[number]
export type KhalaDensityToken = (typeof khalaDensityTokens)[number]
export type KhalaAmbientQualityToken = (typeof khalaAmbientQualityTokens)[number]
export type KhalaCollapseRole = (typeof khalaCollapseRoles)[number]

export const KhalaMotifIdSchema = Schema.Literals(khalaMotifIds)
export const KhalaEdgeWidthTokenSchema = Schema.Literals(khalaEdgeWidthTokens)
export const KhalaCutSizeTokenSchema = Schema.Literals(khalaCutSizeTokens)
export const KhalaAccentLengthTokenSchema = Schema.Literals(khalaAccentLengthTokens)
export const KhalaLuminanceRoleSchema = Schema.Literals(khalaLuminanceRoles)
export const KhalaDensityTokenSchema = Schema.Literals(khalaDensityTokens)
export const KhalaAmbientQualityTokenSchema = Schema.Literals(khalaAmbientQualityTokens)
export const KhalaCollapseRoleSchema = Schema.Literals(khalaCollapseRoles)

const boundedNumber = (minimum: number, maximum: number, title: string) =>
  Schema.Number.check(
    Schema.isFinite({ title: `${title}Finite` }),
    Schema.isGreaterThanOrEqualTo(minimum, { title: `${title}Minimum` }),
    Schema.isLessThanOrEqualTo(maximum, { title: `${title}Maximum` })
  )

export const KhalaLengthSchema = boundedNumber(0, 16_384, "KhalaLength")
export const KhalaPositiveLengthSchema = boundedNumber(Number.EPSILON, 16_384, "KhalaPositiveLength")
export const KhalaScalarSchema = boundedNumber(-16, 16, "KhalaScalar")
export const KhalaPercentageSchema = boundedNumber(0, 100, "KhalaPercentage")
export const KhalaOpacitySchema = boundedNumber(0, 1, "KhalaOpacity")
export const KhalaDetailSchema = boundedNumber(0, 2, "KhalaDetail")
export const KhalaZoomSchema = boundedNumber(1, 4, "KhalaZoom")

const recordFields = <const Keys extends ReadonlyArray<string>, Value extends Schema.Constraint>(
  keys: Keys,
  value: Value
): { readonly [Key in Keys[number]]: Value } =>
  Object.fromEntries(keys.map((key) => [key, value])) as { readonly [Key in Keys[number]]: Value }

/**
 * Canonical inputs for static Khala geometry. Numeric values are theme data,
 * never renderer constants; luminance values point back to semantic color
 * roles instead of introducing a second palette.
 */
export const KhalaUiThemeSchema = Schema.Struct({
  edgeWidth: Schema.Struct(recordFields(khalaEdgeWidthTokens, KhalaLengthSchema)),
  cutSize: Schema.Struct(recordFields(khalaCutSizeTokens, KhalaLengthSchema)),
  accentLength: Schema.Struct(recordFields(khalaAccentLengthTokens, KhalaLengthSchema)),
  luminance: Schema.Struct({
    quiet: Schema.Literal("borderSubtle"),
    structural: Schema.Literal("borderStrong"),
    signal: Schema.Literal("accent"),
    focus: Schema.Literal("focus")
  }),
  density: Schema.Struct({
    compact: Schema.Struct({
      gap: KhalaLengthSchema,
      cut: KhalaCutSizeTokenSchema,
      accent: KhalaAccentLengthTokenSchema
    }),
    comfortable: Schema.Struct({
      gap: KhalaLengthSchema,
      cut: KhalaCutSizeTokenSchema,
      accent: KhalaAccentLengthTokenSchema
    }),
    spacious: Schema.Struct({
      gap: KhalaLengthSchema,
      cut: KhalaCutSizeTokenSchema,
      accent: KhalaAccentLengthTokenSchema
    })
  }),
  ambientQuality: Schema.Struct({
    off: Schema.Struct({ opacity: KhalaOpacitySchema, detail: KhalaDetailSchema }),
    restrained: Schema.Struct({ opacity: KhalaOpacitySchema, detail: KhalaDetailSchema }),
    enhanced: Schema.Struct({ opacity: KhalaOpacitySchema, detail: KhalaDetailSchema })
  }),
  responsiveCollapse: Schema.Struct({
    borderOnlyBelow: KhalaLengthSchema,
    simplifiedBelow: KhalaLengthSchema
  }),
  focusClearance: KhalaLengthSchema
})

export type KhalaUiTheme = Schema.Schema.Type<typeof KhalaUiThemeSchema>
