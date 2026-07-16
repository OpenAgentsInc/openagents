import { Effect, Schema } from "effect"

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

export type KhalaFrameElementGroup = "background" | "line" | "deco"

export interface KhalaFramePolygonElement {
  readonly _tag: "Polygon"
  readonly id: string
  readonly group: KhalaFrameElementGroup
  readonly role: KhalaLuminanceRole
  readonly width: number
  readonly points: ReadonlyArray<KhalaPoint>
}

export interface KhalaFrameLineElement {
  readonly _tag: "Line"
  readonly id: string
  readonly group: KhalaFrameElementGroup
  readonly role: KhalaLuminanceRole
  readonly width: number
  readonly from: KhalaPoint
  readonly to: KhalaPoint
}

export type KhalaFrameElement = KhalaFramePolygonElement | KhalaFrameLineElement

/**
 * Closed generic frame scene. `clip` and `mask` apply only to the inert paint
 * layer; semantic content is deliberately outside the compositing graph.
 */
export interface KhalaFrameScene {
  readonly motif: KhalaMotifId
  readonly geometry: KhalaMotifGeometry
  readonly elements: ReadonlyArray<KhalaFrameElement>
  readonly clip: ReadonlyArray<KhalaPoint> | null
  readonly mask: ReadonlyArray<KhalaPoint> | null
  readonly pattern: KhalaLinearPaint | null
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

const octagon = (width: number, height: number, cut: number): ReadonlyArray<KhalaPoint> => [
  point(cut, 0),
  point(width - cut, 0),
  point(width, cut),
  point(width, height - cut),
  point(width - cut, height),
  point(cut, height),
  point(0, height - cut),
  point(0, cut)
]

const cornerBrackets = (
  width: number,
  height: number,
  length: number,
  role: KhalaLuminanceRole,
  stroke: number
): ReadonlyArray<KhalaLine> => [
  line(0, 0, length, 0, role, stroke),
  line(0, 0, 0, length, role, stroke),
  line(width - length, 0, width, 0, role, stroke),
  line(width, 0, width, length, role, stroke),
  line(0, height, length, height, role, stroke),
  line(0, height - length, 0, height, role, stroke),
  line(width - length, height, width, height, role, stroke),
  line(width, height - length, width, height, role, stroke)
]

const ringLines = (
  width: number,
  height: number,
  role: KhalaLuminanceRole,
  stroke: number,
  segments = 32
): ReadonlyArray<KhalaLine> => {
  const centerX = width / 2
  const centerY = height / 2
  const radiusX = Math.max(0, width / 2 - stroke)
  const radiusY = Math.max(0, height / 2 - stroke)
  return Array.from({ length: segments }, (_, index) => {
    const start = (index / segments) * Math.PI * 2
    const end = ((index + 1) / segments) * Math.PI * 2
    return line(
      centerX + Math.cos(start) * radiusX,
      centerY + Math.sin(start) * radiusY,
      centerX + Math.cos(end) * radiusX,
      centerY + Math.sin(end) * radiusY,
      role,
      stroke
    )
  })
}

const collapseFor = (input: KhalaMotifInput, theme: KhalaUiTheme): KhalaCollapseRole => {
  const effectiveWidth = input.width / input.zoom
  return effectiveWidth < theme.responsiveCollapse.borderOnlyBelow
    ? "border-only"
    : effectiveWidth < theme.responsiveCollapse.simplifiedBelow
      ? "simplified"
      : "full"
}

/**
 * Resolve the closed static motif vocabulary to logical points and line segments.
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

    if (decodedInput.motif === "octagonal-surface") {
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: cut === 0 ? rectangle(decodedInput.width, decodedInput.height) : octagon(decodedInput.width, decodedInput.height, cut),
        lines: []
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "asymmetric-cut") {
      const asymmetric = Math.min(Math.max(cut, decodedTheme.cutSize.small), decodedInput.width / 3, decodedInput.height / 3)
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon:
          collapse === "border-only"
            ? rectangle(decodedInput.width, decodedInput.height)
            : [
                point(asymmetric * 2, 0),
                point(decodedInput.width, 0),
                point(decodedInput.width, decodedInput.height - asymmetric * 2),
                point(decodedInput.width - asymmetric * 2, decodedInput.height),
                point(0, decodedInput.height),
                point(0, asymmetric * 2)
              ],
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


    if (decodedInput.motif === "edge-underline") {
      const y = decodedInput.height
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: [],
        lines:
          collapse === "border-only"
            ? [line(0, y, decodedInput.width, y, quietRole, strokeWidth)]
            : [
                line(0, y, Math.min(accentLength, decodedInput.width), y, signalRole, signalWidth),
                line(Math.min(accentLength, decodedInput.width), y, decodedInput.width, y, quietRole, strokeWidth)
              ]
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "corner-brackets") {
      const length = collapse === "border-only" ? 0 : Math.min(accentLength, decodedInput.width / 3, decodedInput.height / 3)
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: length === 0 ? rectangle(decodedInput.width, decodedInput.height) : [],
        lines: length === 0 ? [] : cornerBrackets(decodedInput.width, decodedInput.height, length, signalRole, signalWidth)
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "corner-line-array") {
      const bracketLength = Math.min(accentLength, decodedInput.width / 3, decodedInput.height / 3)
      const base = cornerBrackets(decodedInput.width, decodedInput.height, bracketLength, quietRole, strokeWidth)
      const offset = Math.max(2, density.gap)
      const accents =
        collapse === "full"
          ? [
              line(offset, offset, bracketLength, offset, signalRole, signalWidth),
              line(decodedInput.width - bracketLength, offset, decodedInput.width - offset, offset, signalRole, signalWidth),
              line(offset, decodedInput.height - offset, bracketLength, decodedInput.height - offset, signalRole, signalWidth),
              line(decodedInput.width - bracketLength, decodedInput.height - offset, decodedInput.width - offset, decodedInput.height - offset, signalRole, signalWidth)
            ]
          : []
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: collapse === "border-only" ? rectangle(decodedInput.width, decodedInput.height) : [],
        lines: collapse === "border-only" ? [] : [...base, ...accents]
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "corner-chevron") {
      const length = Math.min(accentLength, decodedInput.width / 4, decodedInput.height / 4)
      const lines = [
        line(0, length, length, 0, signalRole, signalWidth),
        line(decodedInput.width - length, 0, decodedInput.width, length, signalRole, signalWidth),
        line(0, decodedInput.height - length, length, decodedInput.height, signalRole, signalWidth),
        line(decodedInput.width - length, decodedInput.height, decodedInput.width, decodedInput.height - length, signalRole, signalWidth)
      ]
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: collapse === "border-only" ? rectangle(decodedInput.width, decodedInput.height) : [],
        lines: collapse === "border-only" ? [] : lines
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "split-corner") {
      const length = Math.min(accentLength, decodedInput.width / 4, decodedInput.height / 4)
      const offset = Math.max(2, density.gap)
      const lines = [
        line(0, offset, length, offset, quietRole, strokeWidth),
        line(offset, 0, offset, length, signalRole, signalWidth),
        line(decodedInput.width - length, decodedInput.height - offset, decodedInput.width, decodedInput.height - offset, quietRole, strokeWidth),
        line(decodedInput.width - offset, decodedInput.height - length, decodedInput.width - offset, decodedInput.height, signalRole, signalWidth)
      ]
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: collapse === "border-only" ? rectangle(decodedInput.width, decodedInput.height) : [],
        lines: collapse === "border-only" ? [] : lines
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "header-rail") {
      const vertical = Math.min(decodedInput.height, Math.max(6, accentLength / 3))
      const rail = collapse === "border-only"
        ? [line(0, 0, decodedInput.width, 0, quietRole, strokeWidth)]
        : [
            line(0, 0, decodedInput.width, 0, quietRole, strokeWidth),
            line(0, 0, accentLength, 0, signalRole, signalWidth),
            ...(collapse === "full"
              ? [
                  line(accentLength + density.gap, 0, accentLength + density.gap, vertical, signalRole, signalWidth),
                  line(accentLength + density.gap * 2, 0, accentLength + density.gap * 2, vertical * 0.66, quietRole, strokeWidth),
                  line(accentLength + density.gap * 3, 0, accentLength + density.gap * 3, vertical * 0.33, quietRole, strokeWidth)
                ]
              : [])
          ]
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: [],
        lines: rail
      } satisfies KhalaMotifGeometry
    }

    if (decodedInput.motif === "radial-dial") {
      const ring = collapse === "border-only" ? [] : ringLines(decodedInput.width, decodedInput.height, quietRole, strokeWidth)
      const tick = Math.min(accentLength / 3, decodedInput.width / 6, decodedInput.height / 6)
      const ticks = collapse === "full"
        ? [
            line(decodedInput.width / 2, 0, decodedInput.width / 2, tick, signalRole, signalWidth),
            line(decodedInput.width / 2, decodedInput.height - tick, decodedInput.width / 2, decodedInput.height, signalRole, signalWidth),
            line(0, decodedInput.height / 2, tick, decodedInput.height / 2, signalRole, signalWidth),
            line(decodedInput.width - tick, decodedInput.height / 2, decodedInput.width, decodedInput.height / 2, signalRole, signalWidth)
          ]
        : []
      return {
        motif: decodedInput.motif,
        collapse,
        contentInset: 0 as const,
        focusClearance: decodedTheme.focusClearance,
        forcedColors: decodedInput.forcedColors,
        polygon: collapse === "border-only" ? rectangle(decodedInput.width, decodedInput.height) : [],
        lines: [...ring, ...ticks]
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

/** Resolve motif geometry into the generic grouping/compositing frame scene. */
export const resolveKhalaFrameScene = (input: unknown, theme: unknown) =>
  Effect.map(resolveKhalaMotif(input, theme), (geometry): KhalaFrameScene => {
    const outlineRole: KhalaLuminanceRole = geometry.forcedColors ? "focus" : "structural"
    const polygonElements: ReadonlyArray<KhalaFramePolygonElement> =
      geometry.polygon.length === 0
        ? []
        : [
            {
              _tag: "Polygon",
              id: `${geometry.motif}-background`,
              group: "background",
              role: outlineRole,
              width: 1,
              points: geometry.polygon
            }
          ]
    const lineElements = geometry.lines.map(
      (value, index): KhalaFrameLineElement => ({
        _tag: "Line",
        id: `${geometry.motif}-line-${index}`,
        group: value.role === "signal" || value.role === "focus" ? "deco" : "line",
        role: value.role,
        width: value.width,
        from: value.from,
        to: value.to
      })
    )
    const clips = new Set<KhalaMotifId>(["cut-corner-surface", "octagonal-surface", "asymmetric-cut"])
    return {
      motif: geometry.motif,
      geometry,
      elements: [...polygonElements, ...lineElements],
      clip: clips.has(geometry.motif) ? geometry.polygon : null,
      mask: geometry.motif === "radial-dial" && geometry.polygon.length > 0 ? geometry.polygon : null,
      pattern: geometry.motif === "corner-line-array" ? resolveKhalaStepsPaint(4) : null
    }
  })

export interface KhalaPaintStop {
  readonly offset: number
  readonly role: KhalaLuminanceRole | "transparent"
}

export interface KhalaLinearPaint {
  readonly direction: "horizontal" | "vertical"
  readonly repeating: boolean
  readonly stops: ReadonlyArray<KhalaPaintStop>
}

const boundedPaintLength = (value: number, minimum = 1, maximum = 64): number =>
  Math.min(maximum, Math.max(minimum, Math.round(value)))

/** Owned, renderer-neutral equivalent of an alternating hard-stop gradient. */
export const resolveKhalaStepsPaint = (
  length: number,
  direction: KhalaLinearPaint["direction"] = "horizontal",
  role: KhalaLuminanceRole = "signal"
): KhalaLinearPaint => {
  const count = boundedPaintLength(length, 1, 32)
  const total = count === 1 ? 1 : count * 2 - 1
  return {
    direction,
    repeating: false,
    stops: Array.from({ length: total }, (_, index) => {
      const start = index / total
      const end = (index + 1) / total
      const color = index % 2 === 0 ? role : "transparent"
      return [
        { offset: start, role: color },
        { offset: end, role: color }
      ] as const
    }).flat()
  }
}

/** Owned, bounded equivalent of a repeating multi-role strip. */
export const resolveKhalaStripPaint = (
  roles: ReadonlyArray<KhalaLuminanceRole>,
  direction: KhalaLinearPaint["direction"] = "horizontal"
): KhalaLinearPaint => {
  const safeRoles = roles.slice(0, 8)
  const resolved = safeRoles.length === 0 ? (["structural"] as const) : safeRoles
  return {
    direction,
    repeating: true,
    stops: resolved.flatMap((role, index) => [
      { offset: index / resolved.length, role },
      { offset: (index + 1) / resolved.length, role }
    ])
  }
}

/** Separator paint used by horizontal and vertical signal rails. */
export const resolveKhalaSeparatorPaint = (
  direction: "start" | "end" | "both" = "end",
  axis: KhalaLinearPaint["direction"] = "horizontal"
): KhalaLinearPaint => {
  const start: ReadonlyArray<KhalaPaintStop> =
    direction === "start" || direction === "both"
      ? [
          { offset: 0, role: "signal" },
          { offset: 0.08, role: "signal" },
          { offset: 0.08, role: "transparent" },
          { offset: 0.12, role: "transparent" },
          { offset: 0.12, role: "signal" },
          { offset: 0.2, role: "signal" }
        ]
      : [{ offset: 0, role: "structural" }]
  const end: ReadonlyArray<KhalaPaintStop> =
    direction === "end" || direction === "both"
      ? [
          { offset: 0.8, role: "structural" },
          { offset: 0.8, role: "transparent" },
          { offset: 0.88, role: "transparent" },
          { offset: 0.88, role: "signal" },
          { offset: 0.92, role: "signal" },
          { offset: 0.92, role: "transparent" },
          { offset: 0.96, role: "transparent" },
          { offset: 0.96, role: "signal" },
          { offset: 1, role: "signal" }
        ]
      : [{ offset: 1, role: "structural" }]
  return { direction: axis, repeating: false, stops: [...start, ...end] }
}
