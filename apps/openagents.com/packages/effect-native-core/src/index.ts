import {
  Context,
  Deferred,
  Effect,
  Exit,
  Layer,
  PubSub,
  Ref,
  Schema,
  Scope,
  Stream,
  SubscriptionRef
} from "effect"
import {
  ColorTokenSchema,
  DimensionTokenSchema,
  RadiusTokenSchema,
  SpacingTokenSchema,
  TypeScaleTokenSchema,
  breakpointTokens,
  type BreakpointToken,
  type ColorToken,
  type RadiusToken,
  type SpacingToken,
  type TypeScaleToken
} from "@effect-native/tokens"

export {
  BreakpointTokenSchema,
  ColorTokenSchema,
  DimensionTokenSchema,
  RadiusTokenSchema,
  SpacingTokenSchema,
  TypeScaleTokenSchema,
  breakpointTokens,
  colorTokens,
  defaultTheme,
  defineTheme,
  dimensionTokens,
  radiusTokens,
  spacingTokens,
  typeScaleTokens,
  type BreakpointToken,
  type ColorToken,
  type DimensionToken,
  type RadiusToken,
  type SpacingToken,
  type Theme,
  type TypeScaleToken
} from "@effect-native/tokens"

export const packageName = "@effect-native/core" as const

export const CatalogVersion = "effect-native/v0" as const
export const CatalogVersionSchema = Schema.Literal(CatalogVersion)
export type CatalogVersion = typeof CatalogVersion

export const componentTags = [
  "Stack",
  "Text",
  "Button",
  "Image",
  "TextField",
  "List",
  "Card",
  "Spacer"
] as const
export type ComponentTag = (typeof componentTags)[number]

export const NodeKeySchema = Schema.NonEmptyString
export type NodeKey = Schema.Schema.Type<typeof NodeKeySchema>

export const JsonPayloadSchema = Schema.Json
export type JsonPayload = Schema.Schema.Type<typeof JsonPayloadSchema>

export const BindingSchema = Schema.TaggedStruct("Binding", {
  path: Schema.Array(Schema.NonEmptyString).check(
    Schema.isMinLength(1, { title: "NonEmptyBindingPath" })
  )
})
export type Binding = Schema.Schema.Type<typeof BindingSchema>
export type Bound<T> = T | Binding

export const Binding = (path: readonly [string, ...Array<string>]): Binding =>
  BindingSchema.make({ _tag: "Binding", path })

export const StaticPayloadSchema = Schema.TaggedStruct("StaticPayload", {
  value: JsonPayloadSchema
})
export const ComponentValueBindingSchema = Schema.TaggedStruct("ComponentValueBinding", {
  path: Schema.NonEmptyString.pipe(Schema.optionalKey)
})
export const IntentPayloadTemplateSchema = Schema.Union([
  StaticPayloadSchema,
  ComponentValueBindingSchema
])
export type StaticPayload = Schema.Schema.Type<typeof StaticPayloadSchema>
export type ComponentValueBinding = Schema.Schema.Type<typeof ComponentValueBindingSchema>
export type IntentPayloadTemplate = Schema.Schema.Type<typeof IntentPayloadTemplateSchema>

export const IntentRefSchema = Schema.Struct({
  name: Schema.NonEmptyString,
  payload: IntentPayloadTemplateSchema.pipe(Schema.optionalKey)
})
export type IntentRef = Schema.Schema.Type<typeof IntentRefSchema>

export const StaticPayload = (value: JsonPayload): StaticPayload =>
  StaticPayloadSchema.make({ _tag: "StaticPayload", value })

export const ComponentValueBinding = (path?: string): ComponentValueBinding =>
  path === undefined
    ? ComponentValueBindingSchema.make({ _tag: "ComponentValueBinding" })
    : ComponentValueBindingSchema.make({ _tag: "ComponentValueBinding", path })

export const IntentRef = (name: string, payload?: IntentPayloadTemplate): IntentRef =>
  payload === undefined ? IntentRefSchema.make({ name }) : IntentRefSchema.make({ name, payload })

export interface Intent<Name extends string = string, Payload = JsonPayload> {
  readonly name: Name
  readonly payload: Payload
}

export const IntentSchema: Schema.Codec<Intent<string, JsonPayload>, Intent<string, JsonPayload>> =
  Schema.Struct({
    name: Schema.NonEmptyString,
    payload: JsonPayloadSchema
  })

export const makeIntent = <const Name extends string, Payload extends JsonPayload>(
  name: Name,
  payload: Payload
): Intent<Name, Payload> => IntentSchema.make({ name, payload }) as Intent<Name, Payload>

export const encodeIntent = Schema.encodeSync(IntentSchema)
export const decodeIntent = Schema.decodeUnknownSync(IntentSchema)

export const resolveIntentRef = (
  ref: IntentRef,
  componentValue: JsonPayload = null
): Intent<string, JsonPayload> => {
  if (ref.payload === undefined) {
    return makeIntent(ref.name, null)
  }

  if (ref.payload._tag === "StaticPayload") {
    return makeIntent(ref.name, ref.payload.value)
  }

  return makeIntent(ref.name, componentValue)
}

export interface IntentDefinition<
  Name extends string = string,
  PayloadSchema extends Schema.ConstraintDecoder<any, never> = Schema.ConstraintDecoder<any, never>
> {
  readonly name: Name
  readonly payloadSchema: PayloadSchema
}

export const defineIntent = <
  const Name extends string,
  const S extends Schema.ConstraintDecoder<any, never>
>(
  name: Name,
  payloadSchema: S
): { readonly name: Name; readonly payloadSchema: S } => ({ name, payloadSchema })

export type IntentPayloadOf<D extends IntentDefinition> = Schema.Schema.Type<D["payloadSchema"]>
export type IntentEncodedPayloadOf<D extends IntentDefinition> = Schema.Codec.Encoded<D["payloadSchema"]>
export type IntentFor<D extends IntentDefinition> = Intent<D["name"], IntentPayloadOf<D>>

export type IntentHandler<D extends IntentDefinition> = (
  payload: IntentPayloadOf<D>,
  intent: IntentFor<D>
) => Effect.Effect<void, unknown>

export type IntentHandlers<Definitions extends ReadonlyArray<IntentDefinition>> = {
  readonly [D in Definitions[number] as D["name"]]: IntentHandler<D>
}

export class UnknownIntentError extends Schema.TaggedErrorClass<UnknownIntentError>()(
  "UnknownIntentError",
  {
    name: Schema.String
  }
) {}

export class IntentPayloadDecodeError extends Schema.TaggedErrorClass<IntentPayloadDecodeError>()(
  "IntentPayloadDecodeError",
  {
    name: Schema.String,
    message: Schema.String
  }
) {}

export class IntentHandlerError extends Schema.TaggedErrorClass<IntentHandlerError>()(
  "IntentHandlerError",
  {
    name: Schema.String,
    message: Schema.String
  }
) {}

export type IntentError = UnknownIntentError | IntentPayloadDecodeError | IntentHandlerError

export interface IntentEvent {
  readonly timestamp: number
  readonly intent: Intent<string, JsonPayload>
  readonly result: Exit.Exit<void, IntentError>
}

export interface IntentRegistry {
  readonly dispatch: (intent: Intent<string, JsonPayload>) => Effect.Effect<void, IntentError>
  readonly events: Effect.Effect<ReadonlyArray<IntentEvent>>
  readonly stream: Stream.Stream<IntentEvent>
}

export const IntentRegistry = Context.Service<IntentRegistry>("@effect-native/core/IntentRegistry")

export interface IntentRegistryOptions {
  readonly now?: () => number
}

export const makeIntentRegistry = <const Definitions extends ReadonlyArray<IntentDefinition>>(
  definitions: Definitions,
  handlers: IntentHandlers<Definitions>,
  options: IntentRegistryOptions = {}
): Effect.Effect<IntentRegistry> =>
  Effect.gen(function*() {
    const eventsRef = yield* Ref.make<ReadonlyArray<IntentEvent>>([])
    const eventsPubSub = yield* PubSub.unbounded<IntentEvent>({ replay: 1024 })
    const now = options.now ?? Date.now
    const definitionsByName = new Map<string, IntentDefinition>(
      definitions.map((definition) => [definition.name, definition])
    )

    const appendEvent = (event: IntentEvent) =>
      Effect.gen(function*() {
        yield* Ref.update(eventsRef, (events) => [...events, event])
        yield* PubSub.publish(eventsPubSub, event)
      })

    const failWith = (intent: Intent<string, JsonPayload>, error: IntentError) =>
      Effect.gen(function*() {
        const result = Exit.fail(error)
        yield* appendEvent({ timestamp: now(), intent, result })
        return yield* error
      })

    const dispatch = (intent: Intent<string, JsonPayload>): Effect.Effect<void, IntentError> =>
      Effect.gen(function*() {
        const definition = definitionsByName.get(intent.name)
        if (definition === undefined) {
          return yield* failWith(intent, new UnknownIntentError({ name: intent.name }))
        }

        const decoded = Schema.decodeUnknownExit(definition.payloadSchema)(intent.payload)
        if (Exit.isFailure(decoded)) {
          return yield* failWith(
            intent,
            new IntentPayloadDecodeError({
              name: intent.name,
              message: String(decoded.cause)
            })
          )
        }

        const handler = handlers[intent.name as keyof typeof handlers] as IntentHandler<typeof definition>
        const typedIntent = {
          name: definition.name,
          payload: decoded.value
        } as IntentFor<typeof definition>
        const handlerExit = yield* Effect.exit(handler(decoded.value, typedIntent))

        if (Exit.isFailure(handlerExit)) {
          const error = new IntentHandlerError({
            name: intent.name,
            message: String(handlerExit.cause)
          })
          yield* appendEvent({
            timestamp: now(),
            intent,
            result: Exit.fail(error)
          })
          return yield* error
        }

        yield* appendEvent({
          timestamp: now(),
          intent,
          result: Exit.succeed(undefined)
        })
      })

    return {
      dispatch,
      events: Ref.get(eventsRef),
      stream: Stream.fromPubSub(eventsPubSub)
    }
  })

export const makeIntentRegistryLayer = <const Definitions extends ReadonlyArray<IntentDefinition>>(
  definitions: Definitions,
  handlers: IntentHandlers<Definitions>,
  options?: IntentRegistryOptions
) => Layer.effect(IntentRegistry, makeIntentRegistry(definitions, handlers, options))

export const dispatchIntent = (intent: Intent<string, JsonPayload>): Effect.Effect<void, IntentError, IntentRegistry> =>
  Effect.gen(function*() {
    const registry = yield* IntentRegistry
    yield* registry.dispatch(intent)
  })

export const getIntentEvents: Effect.Effect<ReadonlyArray<IntentEvent>, never, IntentRegistry> =
  Effect.gen(function*() {
    const registry = yield* IntentRegistry
    return yield* registry.events
  })

export const getIntentEventStream: Effect.Effect<Stream.Stream<IntentEvent>, never, IntentRegistry> =
  Effect.gen(function*() {
    const registry = yield* IntentRegistry
    return registry.stream
  })

export const StackDirectionSchema = Schema.Literals(["row", "column"] as const)
export const StackAlignSchema = Schema.Literals(["start", "center", "end", "stretch"] as const)
export const StackJustifySchema = Schema.Literals([
  "start",
  "center",
  "end",
  "between",
  "around"
] as const)
export const TextWeightSchema = Schema.Literals(["regular", "medium", "semibold", "bold"] as const)
export const ButtonVariantSchema = Schema.Literals(["primary", "secondary", "ghost"] as const)
export const ImageFitSchema = Schema.Literals(["contain", "cover", "fill"] as const)

export type StackDirection = Schema.Schema.Type<typeof StackDirectionSchema>
export type StackAlign = Schema.Schema.Type<typeof StackAlignSchema>
export type StackJustify = Schema.Schema.Type<typeof StackJustifySchema>
export type TextWeight = Schema.Schema.Type<typeof TextWeightSchema>
export type ButtonVariant = Schema.Schema.Type<typeof ButtonVariantSchema>
export type ImageFit = Schema.Schema.Type<typeof ImageFitSchema>

export const UriStringSchema = Schema.String.check(
  Schema.isPattern(/^[a-z][a-z0-9+.-]*:/i, {
    title: "URI"
  })
)
export const NonNegativeNumberSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "NonNegativeNumber" })
)
export const DimensionSchema = Schema.Union([DimensionTokenSchema, NonNegativeNumberSchema])
export type Dimension = Schema.Schema.Type<typeof DimensionSchema>

export const OpacitySchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "MinOpacity" }),
  Schema.isLessThanOrEqualTo(1, { title: "MaxOpacity" })
)
export const TextAlignSchema = Schema.Literals(["left", "center", "right"] as const)
export const AlignSelfSchema = Schema.Literals(["start", "center", "end", "stretch"] as const)
export const stateVariants = ["pressed", "focused", "disabled"] as const
export const platformVariants = ["web", "ios", "android"] as const

export type Opacity = Schema.Schema.Type<typeof OpacitySchema>
export type TextAlign = Schema.Schema.Type<typeof TextAlignSchema>
export type AlignSelf = Schema.Schema.Type<typeof AlignSelfSchema>
export type StateVariant = (typeof stateVariants)[number]
export type PlatformVariant = (typeof platformVariants)[number]

export interface StyleProperties {
  readonly margin: SpacingToken
  readonly marginTop: SpacingToken
  readonly marginRight: SpacingToken
  readonly marginBottom: SpacingToken
  readonly marginLeft: SpacingToken
  readonly padding: SpacingToken
  readonly paddingTop: SpacingToken
  readonly paddingRight: SpacingToken
  readonly paddingBottom: SpacingToken
  readonly paddingLeft: SpacingToken
  readonly gap: SpacingToken
  readonly width: Dimension
  readonly height: Dimension
  readonly minWidth: Dimension
  readonly minHeight: Dimension
  readonly maxWidth: Dimension
  readonly maxHeight: Dimension
  readonly flex: number
  readonly alignSelf: AlignSelf
  readonly opacity: Opacity
  readonly backgroundColor: ColorToken
  readonly borderColor: ColorToken
  readonly borderRadius: RadiusToken
  readonly borderWidth: number
  readonly color: ColorToken
  readonly typeScale: TypeScaleToken
  readonly fontWeight: TextWeight
  readonly textAlign: TextAlign
}

export type StyleKey = keyof StyleProperties
export type StyleVariants<StyleValue> = {
  readonly state?: { readonly [Key in StateVariant]?: StyleValue }
  readonly platform?: { readonly [Key in PlatformVariant]?: StyleValue }
  readonly breakpoint?: { readonly [Key in BreakpointToken]?: StyleValue }
}
export type StyleFor<Key extends StyleKey> = {
  readonly [Property in Key]?: StyleProperties[Property]
} & {
  readonly variants?: StyleVariants<StyleFor<Key>>
}
export type FlatStyleFor<Key extends StyleKey> = {
  readonly [Property in Key]?: StyleProperties[Property]
}
export type Style = StyleFor<StyleKey>
export type FlatStyle = FlatStyleFor<StyleKey>

export const styleKeys = [
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "padding",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "gap",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "alignSelf",
  "opacity",
  "backgroundColor",
  "borderColor",
  "borderRadius",
  "borderWidth",
  "color",
  "typeScale",
  "fontWeight",
  "textAlign"
] as const satisfies ReadonlyArray<StyleKey>

const layoutStyleKeys = [
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "padding",
  "paddingTop",
  "paddingRight",
  "paddingBottom",
  "paddingLeft",
  "gap",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "alignSelf",
  "opacity"
] as const satisfies ReadonlyArray<StyleKey>
const boxStyleKeys = [
  ...layoutStyleKeys,
  "backgroundColor",
  "borderColor",
  "borderRadius",
  "borderWidth"
] as const satisfies ReadonlyArray<StyleKey>

export const stackStyleKeys = boxStyleKeys
export const textStyleKeys = [
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "alignSelf",
  "opacity",
  "color",
  "typeScale",
  "fontWeight",
  "textAlign"
] as const satisfies ReadonlyArray<StyleKey>
export const buttonStyleKeys = [
  ...boxStyleKeys,
  "color",
  "typeScale",
  "fontWeight",
  "textAlign"
] as const satisfies ReadonlyArray<StyleKey>
export const imageStyleKeys = [
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "alignSelf",
  "opacity",
  "borderRadius"
] as const satisfies ReadonlyArray<StyleKey>
export const textFieldStyleKeys = buttonStyleKeys
export const listStyleKeys = boxStyleKeys
export const cardStyleKeys = boxStyleKeys
export const spacerStyleKeys = [
  "margin",
  "marginTop",
  "marginRight",
  "marginBottom",
  "marginLeft",
  "width",
  "height",
  "minWidth",
  "minHeight",
  "maxWidth",
  "maxHeight",
  "flex",
  "alignSelf",
  "opacity"
] as const satisfies ReadonlyArray<StyleKey>

export type StackStyle = StyleFor<(typeof stackStyleKeys)[number]>
export type TextStyle = StyleFor<(typeof textStyleKeys)[number]>
export type ButtonStyle = StyleFor<(typeof buttonStyleKeys)[number]>
export type ImageStyle = StyleFor<(typeof imageStyleKeys)[number]>
export type TextFieldStyle = StyleFor<(typeof textFieldStyleKeys)[number]>
export type ListStyle = StyleFor<(typeof listStyleKeys)[number]>
export type CardStyle = StyleFor<(typeof cardStyleKeys)[number]>
export type SpacerStyle = StyleFor<(typeof spacerStyleKeys)[number]>

const stylePropertySchemas = {
  margin: SpacingTokenSchema,
  marginTop: SpacingTokenSchema,
  marginRight: SpacingTokenSchema,
  marginBottom: SpacingTokenSchema,
  marginLeft: SpacingTokenSchema,
  padding: SpacingTokenSchema,
  paddingTop: SpacingTokenSchema,
  paddingRight: SpacingTokenSchema,
  paddingBottom: SpacingTokenSchema,
  paddingLeft: SpacingTokenSchema,
  gap: SpacingTokenSchema,
  width: DimensionSchema,
  height: DimensionSchema,
  minWidth: DimensionSchema,
  minHeight: DimensionSchema,
  maxWidth: DimensionSchema,
  maxHeight: DimensionSchema,
  flex: NonNegativeNumberSchema,
  alignSelf: AlignSelfSchema,
  opacity: OpacitySchema,
  backgroundColor: ColorTokenSchema,
  borderColor: ColorTokenSchema,
  borderRadius: RadiusTokenSchema,
  borderWidth: NonNegativeNumberSchema,
  color: ColorTokenSchema,
  typeScale: TypeScaleTokenSchema,
  fontWeight: TextWeightSchema,
  textAlign: TextAlignSchema
} as const satisfies { readonly [Key in StyleKey]: Schema.Constraint }

const exactStruct = <const Fields extends Schema.Struct.Fields>(fields: Fields) => {
  // OpenAgents EN-1 snapshot workaround: the upstream exact rest-record helper
  // rejects valid known style keys in this app runtime. Track upstream fix in
  // OpenAgentsInc/effect-native#44.
  return Schema.Struct(fields)
}

const optionalStyleFields = <const Keys extends ReadonlyArray<StyleKey>>(keys: Keys) =>
  Object.fromEntries(
    keys.map((key) => [key, stylePropertySchemas[key].pipe(Schema.optionalKey)])
  ) as Schema.Struct.Fields

const optionalVariantFields = <const Keys extends ReadonlyArray<string>, S extends Schema.Constraint>(
  keys: Keys,
  schema: S
) => {
  const pipeable = schema as S & {
    readonly pipe: (f: typeof Schema.optionalKey) => Schema.Constraint
  }
  return Object.fromEntries(keys.map((key) => [key, pipeable.pipe(Schema.optionalKey)])) as Schema.Struct.Fields
}

const makeStyleSchema = <const Keys extends ReadonlyArray<StyleKey>>(
  keys: Keys
): Schema.Codec<StyleFor<Keys[number]>, StyleFor<Keys[number]>> => {
  let styleSchema: Schema.Codec<any, any, any, any>
  const StyleSelf = Schema.suspend((): Schema.Codec<any, any, any, any> => styleSchema)
  const variantsSchema = exactStruct({
    state: exactStruct(optionalVariantFields(stateVariants, StyleSelf)).pipe(Schema.optionalKey),
    platform: exactStruct(optionalVariantFields(platformVariants, StyleSelf)).pipe(Schema.optionalKey),
    breakpoint: exactStruct(optionalVariantFields(breakpointTokens, StyleSelf)).pipe(Schema.optionalKey)
  })
  styleSchema = exactStruct({
    ...optionalStyleFields(keys),
    variants: variantsSchema.pipe(Schema.optionalKey)
  }) as unknown as Schema.Codec<any, any, any, any>
  return styleSchema as unknown as Schema.Codec<StyleFor<Keys[number]>, StyleFor<Keys[number]>>
}

export const StyleSchema = makeStyleSchema(styleKeys)
export const StackStyleSchema = makeStyleSchema(stackStyleKeys)
export const TextStyleSchema = makeStyleSchema(textStyleKeys)
export const ButtonStyleSchema = makeStyleSchema(buttonStyleKeys)
export const ImageStyleSchema = makeStyleSchema(imageStyleKeys)
export const TextFieldStyleSchema = makeStyleSchema(textFieldStyleKeys)
export const ListStyleSchema = makeStyleSchema(listStyleKeys)
export const CardStyleSchema = makeStyleSchema(cardStyleKeys)
export const SpacerStyleSchema = makeStyleSchema(spacerStyleKeys)

const copyFlatStyle = <Key extends StyleKey>(style: StyleFor<Key> | FlatStyleFor<Key>): FlatStyleFor<Key> => {
  const flat: Record<string, unknown> = {}
  for (const [key, value] of Object.entries(style)) {
    if (key !== "variants" && value !== undefined) {
      flat[key] = value
    }
  }
  return flat as FlatStyleFor<Key>
}

const mergeVariantSets = <Key extends StyleKey>(
  left: StyleVariants<StyleFor<Key>> | undefined,
  right: StyleVariants<StyleFor<Key>> | undefined
): StyleVariants<StyleFor<Key>> | undefined => {
  if (right === undefined) {
    return left
  }

  const merged: {
    state?: Record<string, StyleFor<Key>>
    platform?: Record<string, StyleFor<Key>>
    breakpoint?: Record<string, StyleFor<Key>>
  } = {
    ...(left === undefined ? {} : left)
  }

  for (const axis of ["state", "platform", "breakpoint"] as const) {
    const rightAxis = right[axis]
    if (rightAxis === undefined) {
      continue
    }
    const leftAxis = merged[axis] ?? {}
    const axisResult: Record<string, StyleFor<Key>> = { ...leftAxis }
    for (const [variant, style] of Object.entries(rightAxis)) {
      if (style !== undefined) {
        axisResult[variant] = mergeStyles(axisResult[variant], style)
      }
    }
    merged[axis] = axisResult
  }

  return merged as StyleVariants<StyleFor<Key>>
}

export const mergeStyles = <Key extends StyleKey>(
  ...styles: ReadonlyArray<StyleFor<Key> | undefined>
): StyleFor<Key> => {
  const merged: Record<string, unknown> = {}
  let variants: StyleVariants<StyleFor<Key>> | undefined

  for (const style of styles) {
    if (style === undefined) {
      continue
    }
    Object.assign(merged, copyFlatStyle(style))
    variants = mergeVariantSets(variants, style.variants)
  }

  if (variants !== undefined) {
    merged.variants = variants
  }

  return merged as StyleFor<Key>
}

const mergeFlatStyles = <Key extends StyleKey>(
  ...styles: ReadonlyArray<StyleFor<Key> | FlatStyleFor<Key> | undefined>
): FlatStyleFor<Key> => {
  const merged: Record<string, unknown> = {}
  for (const style of styles) {
    if (style !== undefined) {
      Object.assign(merged, copyFlatStyle(style))
    }
  }
  return merged as FlatStyleFor<Key>
}

export interface StyleResolution {
  readonly state?: StateVariant | ReadonlyArray<StateVariant>
  readonly platform?: PlatformVariant
  readonly breakpoint?: BreakpointToken
}

const activeStates = (state: StyleResolution["state"]): ReadonlySet<StateVariant> => {
  if (state === undefined) {
    return new Set()
  }
  return new Set(Array.isArray(state) ? state : [state])
}

export const resolveStyle = <Key extends StyleKey>(
  style: StyleFor<Key>,
  input: StyleResolution = {}
): FlatStyleFor<Key> => {
  const variants = style.variants
  const active: Array<FlatStyleFor<Key>> = []

  if (input.platform !== undefined) {
    const platformStyle = variants?.platform?.[input.platform]
    if (platformStyle !== undefined) {
      active.push(resolveStyle(platformStyle, input))
    }
  }

  if (input.breakpoint !== undefined) {
    const breakpointStyle = variants?.breakpoint?.[input.breakpoint]
    if (breakpointStyle !== undefined) {
      active.push(resolveStyle(breakpointStyle, input))
    }
  }

  const states = activeStates(input.state)
  for (const state of stateVariants) {
    if (states.has(state)) {
      const stateStyle = variants?.state?.[state]
      if (stateStyle !== undefined) {
        active.push(resolveStyle(stateStyle, input))
      }
    }
  }

  return mergeFlatStyles(copyFlatStyle(style), ...active)
}

export interface NodeBase {
  readonly catalogVersion: CatalogVersion
  readonly key?: NodeKey
}

export interface StackView extends NodeBase {
  readonly _tag: "Stack"
  readonly direction: StackDirection
  readonly gap?: SpacingToken
  readonly align?: StackAlign
  readonly justify?: StackJustify
  readonly padding?: SpacingToken
  readonly style?: StackStyle
  readonly children: ReadonlyArray<View>
}

export interface TextView extends NodeBase {
  readonly _tag: "Text"
  readonly content: Bound<string>
  readonly variant: TypeScaleToken
  readonly color?: ColorToken
  readonly weight?: TextWeight
  readonly style?: TextStyle
}

export interface ButtonView extends NodeBase {
  readonly _tag: "Button"
  readonly label: string
  readonly variant: ButtonVariant
  readonly disabled?: boolean
  readonly onPress: IntentRef
  readonly style?: ButtonStyle
}

export interface ImageView extends NodeBase {
  readonly _tag: "Image"
  readonly source: string
  readonly alt: string
  readonly width?: Dimension
  readonly height?: Dimension
  readonly fit?: ImageFit
  readonly style?: ImageStyle
}

export interface BaseTextFieldView extends NodeBase {
  readonly _tag: "TextField"
  readonly value: string
  readonly placeholder?: string
  readonly label?: string
  readonly onChange?: IntentRef
  readonly onSubmit?: IntentRef
  readonly style?: TextFieldStyle
}

export interface SecureTextFieldView extends BaseTextFieldView {
  readonly secure: true
  readonly multiline?: false
}

export interface PlainTextFieldView extends BaseTextFieldView {
  readonly secure?: false
  readonly multiline?: boolean
}

export type TextFieldView = SecureTextFieldView | PlainTextFieldView

export interface ListView extends NodeBase {
  readonly _tag: "List"
  readonly style?: ListStyle
  readonly items: ReadonlyArray<View & { readonly key: NodeKey }>
}

export interface CardView extends NodeBase {
  readonly _tag: "Card"
  readonly padding?: SpacingToken
  readonly radius?: RadiusToken
  readonly style?: CardStyle
  readonly children: ReadonlyArray<View>
}

export interface SpacerSizeView extends NodeBase {
  readonly _tag: "Spacer"
  readonly size: SpacingToken
  readonly flex?: false
  readonly style?: SpacerStyle
}

export interface SpacerFlexView extends NodeBase {
  readonly _tag: "Spacer"
  readonly flex: true
  readonly style?: SpacerStyle
}

export type SpacerView = SpacerSizeView | SpacerFlexView

export type View =
  | StackView
  | TextView
  | ButtonView
  | ImageView
  | TextFieldView
  | ListView
  | CardView
  | SpacerView

export type KeyedView = View & { readonly key: NodeKey }

const ViewSelf = Schema.suspend((): Schema.Codec<View, View> => ViewSchema)

const KeyedViewArraySchema = Schema.Array(ViewSelf).check(
  Schema.makeFilter<ReadonlyArray<View>>((items) => {
    const unkeyedIndex = items.findIndex((item) => item.key === undefined)
    return unkeyedIndex === -1
      ? undefined
      : { path: [unkeyedIndex, "key"], issue: "List items require explicit keys" }
  })
) as Schema.Codec<ReadonlyArray<KeyedView>, ReadonlyArray<KeyedView>>

const CommonFields = {
  catalogVersion: CatalogVersionSchema,
  key: NodeKeySchema.pipe(Schema.optionalKey)
} as const

export const StackSchema: Schema.Codec<StackView, StackView> = Schema.TaggedStruct("Stack", {
  ...CommonFields,
  direction: StackDirectionSchema,
  gap: SpacingTokenSchema.pipe(Schema.optionalKey),
  align: StackAlignSchema.pipe(Schema.optionalKey),
  justify: StackJustifySchema.pipe(Schema.optionalKey),
  padding: SpacingTokenSchema.pipe(Schema.optionalKey),
  style: StackStyleSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf)
})

export const TextSchema: Schema.Codec<TextView, TextView> = Schema.TaggedStruct("Text", {
  ...CommonFields,
  content: Schema.Union([Schema.String, BindingSchema]),
  variant: TypeScaleTokenSchema,
  color: ColorTokenSchema.pipe(Schema.optionalKey),
  weight: TextWeightSchema.pipe(Schema.optionalKey),
  style: TextStyleSchema.pipe(Schema.optionalKey)
})

export const ButtonSchema: Schema.Codec<ButtonView, ButtonView> = Schema.TaggedStruct("Button", {
  ...CommonFields,
  label: Schema.String,
  variant: ButtonVariantSchema,
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  onPress: IntentRefSchema,
  style: ButtonStyleSchema.pipe(Schema.optionalKey)
})

export const ImageSchema: Schema.Codec<ImageView, ImageView> = Schema.TaggedStruct("Image", {
  ...CommonFields,
  source: UriStringSchema,
  alt: Schema.String,
  width: DimensionSchema.pipe(Schema.optionalKey),
  height: DimensionSchema.pipe(Schema.optionalKey),
  fit: ImageFitSchema.pipe(Schema.optionalKey),
  style: ImageStyleSchema.pipe(Schema.optionalKey)
})

const BaseTextFieldFields = {
  ...CommonFields,
  value: Schema.String,
  placeholder: Schema.String.pipe(Schema.optionalKey),
  label: Schema.String.pipe(Schema.optionalKey),
  onChange: IntentRefSchema.pipe(Schema.optionalKey),
  onSubmit: IntentRefSchema.pipe(Schema.optionalKey),
  style: TextFieldStyleSchema.pipe(Schema.optionalKey)
} as const

export const SecureTextFieldSchema: Schema.Codec<SecureTextFieldView, SecureTextFieldView> =
  Schema.TaggedStruct("TextField", {
    ...BaseTextFieldFields,
    secure: Schema.Literal(true),
    multiline: Schema.Literal(false).pipe(Schema.optionalKey)
  })

export const PlainTextFieldSchema: Schema.Codec<PlainTextFieldView, PlainTextFieldView> =
  Schema.TaggedStruct("TextField", {
    ...BaseTextFieldFields,
    secure: Schema.Literal(false).pipe(Schema.optionalKey),
    multiline: Schema.Boolean.pipe(Schema.optionalKey)
  })

export const TextFieldSchema: Schema.Codec<TextFieldView, TextFieldView> = Schema.Union([
  SecureTextFieldSchema,
  PlainTextFieldSchema
])

export const ListSchema: Schema.Codec<ListView, ListView> = Schema.TaggedStruct("List", {
  ...CommonFields,
  style: ListStyleSchema.pipe(Schema.optionalKey),
  items: KeyedViewArraySchema
})

export const CardSchema: Schema.Codec<CardView, CardView> = Schema.TaggedStruct("Card", {
  ...CommonFields,
  padding: SpacingTokenSchema.pipe(Schema.optionalKey),
  radius: RadiusTokenSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf)
})

export const SpacerSizeSchema: Schema.Codec<SpacerSizeView, SpacerSizeView> =
  Schema.TaggedStruct("Spacer", {
    ...CommonFields,
    size: SpacingTokenSchema,
    flex: Schema.Literal(false).pipe(Schema.optionalKey),
    style: SpacerStyleSchema.pipe(Schema.optionalKey)
  })

export const SpacerFlexSchema: Schema.Codec<SpacerFlexView, SpacerFlexView> =
  Schema.TaggedStruct("Spacer", {
    ...CommonFields,
    flex: Schema.Literal(true),
    style: SpacerStyleSchema.pipe(Schema.optionalKey)
  })

export const SpacerSchema: Schema.Codec<SpacerView, SpacerView> = Schema.Union([
  SpacerSizeSchema,
  SpacerFlexSchema
])

export const ViewSchema: Schema.Codec<View, View> = Schema.suspend(() =>
  Schema.Union([
    StackSchema,
    TextSchema,
    ButtonSchema,
    ImageSchema,
    TextFieldSchema,
    ListSchema,
    CardSchema,
    SpacerSchema
  ])
)

export const compatibleCatalogVersions = [CatalogVersion] as const
export type CompatibleCatalogVersion = (typeof compatibleCatalogVersions)[number]
export const CompatibleCatalogVersionSchema = Schema.Literals(compatibleCatalogVersions)

// Compatibility policy: the current decoder accepts every catalog version in
// compatibleCatalogVersions. Future vN+1 bumps add prior-version decoders here
// before changing CatalogVersion, so vN trees continue to decode while unknown
// component tags still fail at the schema boundary.
export const CompatibleViewSchema: Schema.Codec<View, View> = ViewSchema

type DistributiveOmit<T, K extends PropertyKey> = T extends unknown ? Omit<T, K> : never
type WithoutTagAndVersion<T extends NodeBase> = DistributiveOmit<T, "_tag" | "catalogVersion">

export type StackProps = Omit<WithoutTagAndVersion<StackView>, "children">
export const Stack = (props: StackProps, children: ReadonlyArray<View> = []): StackView =>
  StackSchema.make({ _tag: "Stack", catalogVersion: CatalogVersion, ...props, children })

export type TextProps = WithoutTagAndVersion<TextView>
export const Text = (props: TextProps): TextView =>
  TextSchema.make({ _tag: "Text", catalogVersion: CatalogVersion, ...props })

export type ButtonProps = WithoutTagAndVersion<ButtonView>
export const Button = (props: ButtonProps): ButtonView =>
  ButtonSchema.make({ _tag: "Button", catalogVersion: CatalogVersion, ...props })

export type ImageProps = WithoutTagAndVersion<ImageView>
export const Image = (props: ImageProps): ImageView =>
  ImageSchema.make({ _tag: "Image", catalogVersion: CatalogVersion, ...props })

export type TextFieldProps = WithoutTagAndVersion<TextFieldView>
export const TextField = (props: TextFieldProps): TextFieldView =>
  TextFieldSchema.make({ _tag: "TextField", catalogVersion: CatalogVersion, ...props })

export type ListProps = Omit<WithoutTagAndVersion<ListView>, "items">
export const List = (props: ListProps, items: ReadonlyArray<KeyedView>): ListView =>
  ListSchema.make({ _tag: "List", catalogVersion: CatalogVersion, ...props, items })

export type CardProps = Omit<WithoutTagAndVersion<CardView>, "children">
export const Card = (props: CardProps, children: ReadonlyArray<View> = []): CardView =>
  CardSchema.make({ _tag: "Card", catalogVersion: CatalogVersion, ...props, children })

export type SpacerProps = WithoutTagAndVersion<SpacerView>
export const Spacer = (props: SpacerProps): SpacerView =>
  SpacerSchema.make({ _tag: "Spacer", catalogVersion: CatalogVersion, ...props })

export const decodeView = Schema.decodeUnknownSync(ViewSchema)
export const encodeView = Schema.encodeSync(ViewSchema)
export const decodeCompatibleView = Schema.decodeUnknownSync(CompatibleViewSchema)

export const isBinding = (value: unknown): value is Binding =>
  typeof value === "object" &&
  value !== null &&
  "_tag" in value &&
  value._tag === "Binding"

const readStatePath = (state: unknown, path: ReadonlyArray<string>): JsonPayload => {
  let current: unknown = state

  for (const segment of path) {
    if (typeof current !== "object" || current === null || !(segment in current)) {
      return null
    }
    current = (current as Record<string, unknown>)[segment]
  }

  const decoded = Schema.decodeUnknownExit(JsonPayloadSchema)(current)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

const stringifyBoundText = (value: JsonPayload): string => {
  if (value === null) {
    return ""
  }
  if (typeof value === "string") {
    return value
  }
  if (typeof value === "number" || typeof value === "boolean") {
    return String(value)
  }
  return JSON.stringify(value)
}

const resolveBoundText = (value: Bound<string>, state: unknown): string =>
  isBinding(value) ? stringifyBoundText(readStatePath(state, value.path)) : value

export const resolveBindings = <State>(view: View, state: State): View => {
  switch (view._tag) {
    case "Stack":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Text":
      return {
        ...view,
        content: resolveBoundText(view.content, state)
      }
    case "Button":
      return view
    case "Image":
      return view
    case "TextField":
      return view
    case "List":
      return {
        ...view,
        items: view.items.map((item) => resolveBindings(item, state) as KeyedView)
      }
    case "Card":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Spacer":
      return view
  }
}

export interface ViewProgram<State> {
  readonly state: SubscriptionRef.SubscriptionRef<State>
  readonly render: (state: State) => View
  readonly viewStream: Stream.Stream<View>
  readonly currentState: Effect.Effect<State>
  readonly setState: (state: State) => Effect.Effect<void>
  readonly updateState: (f: (state: State) => State) => Effect.Effect<void>
  readonly report: (ref: IntentRef, runtimeValue?: JsonPayload) => Effect.Effect<void, IntentError, IntentRegistry>
}

export const makeViewProgramFromState = <State>(
  state: SubscriptionRef.SubscriptionRef<State>,
  render: (state: State) => View
): ViewProgram<State> => ({
  state,
  render,
  viewStream: SubscriptionRef.changes(state).pipe(
    Stream.map((value) => resolveBindings(render(value), value))
  ),
  currentState: SubscriptionRef.get(state),
  setState: (value) => SubscriptionRef.set(state, value),
  updateState: (f) => SubscriptionRef.update(state, f),
  report: (ref, runtimeValue = null) => dispatchIntent(resolveIntentRef(ref, runtimeValue))
})

export const makeViewProgram = <State>(
  initialState: State,
  render: (state: State) => View
): Effect.Effect<ViewProgram<State>> =>
  Effect.gen(function*() {
    const state = yield* SubscriptionRef.make(initialState)
    return makeViewProgramFromState(state, render)
  })

export interface MountedSurface {
  readonly unmount: Effect.Effect<void>
}

export type IntentReporter = (
  ref: IntentRef,
  runtimeValue?: JsonPayload
) => Effect.Effect<void, IntentError, IntentRegistry>

export interface RendererAdapter<Container, Surface extends MountedSurface = MountedSurface> {
  readonly mount: (
    container: Container,
    viewStream: Stream.Stream<View>,
    report: IntentReporter
  ) => Effect.Effect<Surface, never, Scope.Scope>
}

export interface HeadlessContainer {
  readonly onFinalize?: Effect.Effect<void>
}

export interface HeadlessSurface extends MountedSurface {
  readonly snapshots: Effect.Effect<ReadonlyArray<View>>
  readonly current: Effect.Effect<View | undefined>
  readonly simulate: (ref: IntentRef, runtimeValue?: JsonPayload) => Effect.Effect<void, IntentError, IntentRegistry>
}

export const makeHeadlessRenderer = (): RendererAdapter<HeadlessContainer | undefined, HeadlessSurface> => ({
  mount: (container, viewStream, report) =>
    Effect.gen(function*() {
      const parentScope = yield* Scope.Scope
      const surfaceScope = yield* Scope.fork(parentScope)

      return yield* Scope.provide(surfaceScope)(Effect.gen(function*() {
        const snapshots = yield* Ref.make<ReadonlyArray<View>>([])
        const ready = yield* Deferred.make<void>()

        yield* Effect.addFinalizer(() =>
          container?.onFinalize === undefined
            ? Effect.void
            : container.onFinalize
        )

        yield* viewStream.pipe(
          Stream.runForEach((view) =>
            Effect.gen(function*() {
              yield* Ref.update(snapshots, (views) => [...views, view])
              yield* Deferred.succeed(ready, undefined)
            })
          ),
          Effect.forkScoped
        )
        yield* Deferred.await(ready)

        const current = Ref.get(snapshots).pipe(
          Effect.map((views) => views[views.length - 1])
        )

        return {
          unmount: Scope.close(surfaceScope, Exit.void),
          snapshots: Ref.get(snapshots),
          current,
          simulate: (ref: IntentRef, runtimeValue: JsonPayload = null) =>
            report(ref, runtimeValue).pipe(Effect.andThen(Effect.yieldNow))
        }
      }))
    })
})
