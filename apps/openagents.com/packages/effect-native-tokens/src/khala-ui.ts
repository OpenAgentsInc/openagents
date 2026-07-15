import { Effect, Schema } from "effect"

/** The complete KU-2 motif vocabulary. Adding a value is a contract change. */
export const khalaMotifIds = ["cut-corner-surface", "header-line", "signal-separator"] as const
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

export interface KhalaLiteralDimension {
  readonly _tag: "Literal"
  readonly value: number
}

export interface KhalaPercentageDimension {
  readonly _tag: "Percentage"
  readonly value: number
}

export interface KhalaBinaryDimension {
  readonly _tag: "Add" | "Subtract" | "Minimum" | "Maximum"
  readonly left: KhalaDimension
  readonly right: KhalaDimension
}

export interface KhalaScaleDimension {
  readonly _tag: "Scale"
  readonly value: KhalaDimension
  readonly factor: number
}

export interface KhalaDivideDimension {
  readonly _tag: "Divide"
  readonly value: KhalaDimension
  readonly divisor: number
}

/** Closed, renderer-neutral dimension expression language. */
export type KhalaDimension =
  | KhalaLiteralDimension
  | KhalaPercentageDimension
  | KhalaBinaryDimension
  | KhalaScaleDimension
  | KhalaDivideDimension

const KhalaDimensionSelf: Schema.Codec<KhalaDimension, KhalaDimension> = Schema.suspend(
  (): Schema.Codec<KhalaDimension, KhalaDimension> => KhalaDimensionSchema
)

export const KhalaDimensionSchema: Schema.Codec<KhalaDimension, KhalaDimension> = Schema.Union([
  Schema.TaggedStruct("Literal", { value: KhalaLengthSchema }),
  Schema.TaggedStruct("Percentage", { value: KhalaPercentageSchema }),
  Schema.TaggedStruct("Add", { left: KhalaDimensionSelf, right: KhalaDimensionSelf }),
  Schema.TaggedStruct("Subtract", { left: KhalaDimensionSelf, right: KhalaDimensionSelf }),
  Schema.TaggedStruct("Minimum", { left: KhalaDimensionSelf, right: KhalaDimensionSelf }),
  Schema.TaggedStruct("Maximum", { left: KhalaDimensionSelf, right: KhalaDimensionSelf }),
  Schema.TaggedStruct("Scale", { value: KhalaDimensionSelf, factor: KhalaScalarSchema }),
  Schema.TaggedStruct("Divide", { value: KhalaDimensionSelf, divisor: KhalaScalarSchema })
]) as Schema.Codec<KhalaDimension, KhalaDimension>

export class KhalaInvalidDimensionError extends Schema.TaggedErrorClass<KhalaInvalidDimensionError>()(
  "KhalaInvalidDimensionError",
  { reason: Schema.String }
) {}

export class KhalaDivisionByZeroError extends Schema.TaggedErrorClass<KhalaDivisionByZeroError>()(
  "KhalaDivisionByZeroError",
  { reason: Schema.String }
) {}

export class KhalaExpressionBoundsError extends Schema.TaggedErrorClass<KhalaExpressionBoundsError>()(
  "KhalaExpressionBoundsError",
  { reason: Schema.String }
) {}

export type KhalaGeometryError = KhalaInvalidDimensionError | KhalaDivisionByZeroError | KhalaExpressionBoundsError

export const khalaGeometryLimits = {
  maxLength: 16_384,
  maxExpressionDepth: 8,
  maxExpressionNodes: 64
} as const

const validateResolvedDimension = (value: number): Effect.Effect<number, KhalaInvalidDimensionError> =>
  Number.isFinite(value) && value >= 0 && value <= khalaGeometryLimits.maxLength
    ? Effect.succeed(value)
    : Effect.fail(
        new KhalaInvalidDimensionError({
          reason: `Resolved dimension must be finite and between 0 and ${khalaGeometryLimits.maxLength}; received ${String(value)}`
        })
      )

/** Evaluate a validated expression without global state, parsing, or renderer access. */
export const evaluateKhalaDimension = (expression: unknown, percentageBasis: unknown) =>
  Effect.gen(function* () {
    const decodedExpression = yield* Schema.decodeUnknownEffect(KhalaDimensionSchema)(expression)
    const decodedBasis = yield* Schema.decodeUnknownEffect(KhalaLengthSchema)(percentageBasis)
    const state = { nodes: 0 }

    const evaluate = (node: KhalaDimension, depth: number): Effect.Effect<number, KhalaGeometryError> => {
      state.nodes += 1
      if (depth > khalaGeometryLimits.maxExpressionDepth || state.nodes > khalaGeometryLimits.maxExpressionNodes) {
        return Effect.fail(
          new KhalaExpressionBoundsError({
            reason: `Expression exceeds depth ${khalaGeometryLimits.maxExpressionDepth} or ${khalaGeometryLimits.maxExpressionNodes} nodes`
          })
        )
      }

      switch (node._tag) {
        case "Literal":
          return validateResolvedDimension(node.value)
        case "Percentage":
          return validateResolvedDimension((decodedBasis * node.value) / 100)
        case "Scale":
          return Effect.flatMap(evaluate(node.value, depth + 1), (value) =>
            validateResolvedDimension(value * node.factor)
          )
        case "Divide":
          if (node.divisor === 0) {
            return Effect.fail(new KhalaDivisionByZeroError({ reason: "A Khala dimension divisor cannot be zero" }))
          }
          return Effect.flatMap(evaluate(node.value, depth + 1), (value) =>
            validateResolvedDimension(value / node.divisor)
          )
        default:
          return Effect.gen(function* () {
            const left = yield* evaluate(node.left, depth + 1)
            const right = yield* evaluate(node.right, depth + 1)
            const result =
              node._tag === "Add"
                ? left + right
                : node._tag === "Subtract"
                  ? left - right
                  : node._tag === "Minimum"
                    ? Math.min(left, right)
                    : Math.max(left, right)
            return yield* validateResolvedDimension(result)
          })
      }
    }

    return yield* evaluate(decodedExpression, 1)
  })

export const KhalaMotifInputSchema = Schema.Struct({
  motif: KhalaMotifIdSchema,
  width: KhalaPositiveLengthSchema,
  height: KhalaPositiveLengthSchema,
  zoom: KhalaZoomSchema,
  density: KhalaDensityTokenSchema,
  forcedColors: Schema.Boolean
})

export type KhalaMotifInput = Schema.Schema.Type<typeof KhalaMotifInputSchema>

export const KhalaPointSchema = Schema.Struct({ x: KhalaLengthSchema, y: KhalaLengthSchema })
export const KhalaLineSchema = Schema.Struct({
  from: KhalaPointSchema,
  to: KhalaPointSchema,
  role: KhalaLuminanceRoleSchema,
  width: KhalaLengthSchema
})

export interface KhalaPoint {
  readonly x: number
  readonly y: number
}

export interface KhalaLine {
  readonly from: KhalaPoint
  readonly to: KhalaPoint
  readonly role: KhalaLuminanceRole
  readonly width: number
}

export interface KhalaMotifGeometry {
  readonly motif: KhalaMotifId
  readonly collapse: KhalaCollapseRole
  readonly contentInset: 0
  readonly focusClearance: number
  readonly forcedColors: boolean
  readonly polygon: ReadonlyArray<KhalaPoint>
  readonly lines: ReadonlyArray<KhalaLine>
}

const point = (x: number, y: number): KhalaPoint => ({ x, y })
const line = (x1: number, y1: number, x2: number, y2: number, role: KhalaLuminanceRole, width: number): KhalaLine => ({
  from: point(x1, y1),
  to: point(x2, y2),
  role,
  width
})

const rectangle = (width: number, height: number): ReadonlyArray<KhalaPoint> => [
  point(0, 0),
  point(width, 0),
  point(width, height),
  point(0, height)
]

const collapseFor = (input: KhalaMotifInput, theme: KhalaUiTheme): KhalaCollapseRole => {
  const effectiveWidth = input.width / input.zoom
  return effectiveWidth < theme.responsiveCollapse.borderOnlyBelow
    ? "border-only"
    : effectiveWidth < theme.responsiveCollapse.simplifiedBelow
      ? "simplified"
      : "full"
}

/**
 * Resolve one of the three motifs to logical points and line segments.
 * Content inset is always zero: decoration gives way before semantic space or
 * focus clearance. Renderers consume this data in KU-3.
 */
export const resolveKhalaMotif = (input: unknown, theme: unknown) =>
  Effect.gen(function* () {
    const decodedInput = yield* Schema.decodeUnknownEffect(KhalaMotifInputSchema)(input)
    const decodedTheme = yield* Schema.decodeUnknownEffect(KhalaUiThemeSchema)(theme)
    const collapse = collapseFor(decodedInput, decodedTheme)
    const density = decodedTheme.density[decodedInput.density]
    const strokeWidth = decodedTheme.edgeWidth.structural
    const signalWidth = decodedTheme.edgeWidth.emphasis
    const quietRole: KhalaLuminanceRole = decodedInput.forcedColors ? "focus" : "structural"
    const signalRole: KhalaLuminanceRole = decodedInput.forcedColors ? "focus" : "signal"
    const accentLength = Math.min(decodedTheme.accentLength[density.accent], decodedInput.width / 2)
    const rawCut =
      collapse === "full"
        ? decodedTheme.cutSize[density.cut]
        : collapse === "simplified"
          ? decodedTheme.cutSize.small
          : 0
    const cut = Math.min(rawCut, decodedInput.width / 4, decodedInput.height / 4)

    if (decodedInput.motif === "cut-corner-surface") {
      const polygon =
        cut === 0
          ? rectangle(decodedInput.width, decodedInput.height)
          : [
              point(cut, 0),
              point(decodedInput.width - cut, 0),
              point(decodedInput.width, cut),
              point(decodedInput.width, decodedInput.height - cut),
              point(decodedInput.width - cut, decodedInput.height),
              point(cut, decodedInput.height),
              point(0, decodedInput.height - cut),
              point(0, cut)
            ]
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon,
        lines: []
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "header-line") {
      const lines =
        collapse === "border-only"
          ? [line(0, 0, decodedInput.width, 0, quietRole, strokeWidth)]
          : [
              line(0, 0, accentLength, 0, signalRole, signalWidth),
              line(accentLength, 0, decodedInput.width, 0, quietRole, strokeWidth)
            ]
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: [],
        lines
      } satisfies KhalaMotifGeometry
    }

    const center = decodedInput.width / 2
    const halfAccent = accentLength / 2
    const lines =
      collapse === "full"
        ? [
            line(0, 0, center - halfAccent - density.gap, 0, quietRole, strokeWidth),
            line(center - halfAccent, 0, center + halfAccent, 0, signalRole, signalWidth),
            line(center + halfAccent + density.gap, 0, decodedInput.width, 0, quietRole, strokeWidth)
          ]
        : collapse === "simplified"
          ? [line(center - halfAccent, 0, center + halfAccent, 0, signalRole, signalWidth)]
          : [line(0, 0, decodedInput.width, 0, quietRole, strokeWidth)]
    return {
      motif: decodedInput.motif,
      collapse,
      contentInset: 0 as const,
      focusClearance: decodedTheme.focusClearance,
      forcedColors: decodedInput.forcedColors,
      polygon: [],
      lines
    } satisfies KhalaMotifGeometry
  })
