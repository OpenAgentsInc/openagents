import {
  Cause,
  Context,
  Deferred,
  Duration,
  Effect,
  Exit,
  Layer,
  Option,
  PubSub,
  Ref,
  Schema,
  Scope,
  Stream,
  SubscriptionRef
} from "effect"
import {
  BreakpointTokenSchema,
  ColorTokenSchema,
  DimensionTokenSchema,
  RadiusTokenSchema,
  SpacingTokenSchema,
  TypeScaleTokenSchema,
  breakpointTokens,
  defaultTheme,
  type BreakpointTheme,
  type BreakpointToken,
  type ColorToken,
  type DimensionToken,
  type RadiusToken,
  type SpacingToken,
  type Theme,
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

export const LegacyCatalogVersion = "effect-native/v0" as const
export const LinkCatalogVersion = "effect-native/v1" as const
export const ResponsiveCatalogVersion = "effect-native/v2" as const
export const FormCatalogVersion = "effect-native/v3" as const
export const OverlayCatalogVersion = "effect-native/v4" as const
export const CollectionCatalogVersion = "effect-native/v5" as const
export const InteractionCatalogVersion = "effect-native/v6" as const
export const HostCatalogVersion = "effect-native/v7" as const
export const IconCatalogVersion = "effect-native/v8" as const
export const DataDisplayCatalogVersion = "effect-native/v9" as const
export const AppShellCatalogVersion = "effect-native/v10" as const
export const AnchoredOverlayCatalogVersion = "effect-native/v11" as const
export const ComboboxCatalogVersion = "effect-native/v12" as const
export const TabsCatalogVersion = "effect-native/v13" as const
export const ComposerCatalogVersion = "effect-native/v14" as const
export const SettingsControlsCatalogVersion = "effect-native/v15" as const
export const FeedbackCatalogVersion = "effect-native/v16" as const
export const TranscriptCatalogVersion = "effect-native/v17" as const
export const CodeBlockCatalogVersion = "effect-native/v18" as const
export const GraphCatalogVersion = "effect-native/v19" as const
export const MarketingCatalogVersion = "effect-native/v20" as const
export const PagerCatalogVersion = "effect-native/v21" as const
export const PullToRefreshCatalogVersion = "effect-native/v22" as const
export const SwipeableListItemCatalogVersion = "effect-native/v23" as const
export const MobileSurfacesCatalogVersion = "effect-native/v24" as const
export const MobileGesturesCatalogVersion = "effect-native/v25" as const
export const MediaVideoCatalogVersion = "effect-native/v26" as const
export const PreviousCatalogVersion = MobileGesturesCatalogVersion
export const CatalogVersion = MediaVideoCatalogVersion
export const CatalogVersionSchema = Schema.Literal(CatalogVersion)
export type CatalogVersion = typeof CatalogVersion
export const compatibleCatalogVersions = [
  LegacyCatalogVersion,
  LinkCatalogVersion,
  ResponsiveCatalogVersion,
  FormCatalogVersion,
  OverlayCatalogVersion,
  CollectionCatalogVersion,
  InteractionCatalogVersion,
  HostCatalogVersion,
  IconCatalogVersion,
  DataDisplayCatalogVersion,
  AppShellCatalogVersion,
  AnchoredOverlayCatalogVersion,
  ComboboxCatalogVersion,
  TabsCatalogVersion,
  ComposerCatalogVersion,
  SettingsControlsCatalogVersion,
  FeedbackCatalogVersion,
  TranscriptCatalogVersion,
  CodeBlockCatalogVersion,
  GraphCatalogVersion,
  MarketingCatalogVersion,
  PagerCatalogVersion,
  PullToRefreshCatalogVersion,
  SwipeableListItemCatalogVersion,
  MobileSurfacesCatalogVersion,
  MobileGesturesCatalogVersion,
  MediaVideoCatalogVersion
] as const
export type CompatibleCatalogVersion = (typeof compatibleCatalogVersions)[number]
export const CompatibleCatalogVersionSchema = Schema.Literals(compatibleCatalogVersions)

export const componentTags = [
  "Stack",
  "Text",
  "Button",
  "Image",
  "TextField",
  "List",
  "SectionList",
  "Card",
  "Spacer",
  "Link",
  "Modal",
  "Sheet",
  "Host",
  "Icon",
  "Divider",
  "Badge",
  "Chip",
  "Meter",
  "StatTile",
  "Table",
  "SplitPane",
  "NavRail",
  "Workbench",
  "Popover",
  "DropdownMenu",
  "ContextMenu",
  "Tooltip",
  "Combobox",
  "CommandPalette",
  "Tabs",
  "Composer",
  "Toggle",
  "Select",
  "Checkbox",
  "RadioGroup",
  "Slider",
  "NumberField",
  "FieldRow",
  "Toast",
  "ToastRegion",
  "StatusBanner",
  "RecoveryOverlay",
  "Markdown",
  "Transcript",
  "CodeBlock",
  "DiffView",
  "GraphFigure",
  "Timeline",
  "Section",
  "Hero",
  "AnnouncementBadge",
  "CtaSection",
  "Footer",
  "NavBar",
  "Accordion",
  "PricingColumn",
  "PricingTable",
  "LogoRow",
  "StatsBand",
  "Glow",
  "MockupFrame",
  "Pager",
  "SwipeableListItem",
  "BackgroundGradient",
  "Wallpaper",
  "Spotlight",
  "Frame",
  "BlurredPopup"
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

export const BoundStringSchema = Schema.Union([Schema.String, BindingSchema])
export const BoundBooleanSchema = Schema.Union([Schema.Boolean, BindingSchema])

export const StaticPayloadSchema = Schema.TaggedStruct("StaticPayload", {
  value: JsonPayloadSchema
})
export const ComponentValueBindingSchema = Schema.TaggedStruct("ComponentValueBinding", {
  path: Schema.NonEmptyString.pipe(Schema.optionalKey)
})
export const FieldBindingSchema = Schema.Struct({
  form: Schema.NonEmptyString,
  field: Schema.NonEmptyString
})
export const FormFieldValueBindingSchema = Schema.TaggedStruct("FormFieldValueBinding", {
  form: Schema.NonEmptyString,
  field: Schema.NonEmptyString
})
export const IntentPayloadTemplateSchema = Schema.Union([
  StaticPayloadSchema,
  ComponentValueBindingSchema,
  FormFieldValueBindingSchema
])
export type StaticPayload = Schema.Schema.Type<typeof StaticPayloadSchema>
export type ComponentValueBinding = Schema.Schema.Type<typeof ComponentValueBindingSchema>
export type FieldBinding = Schema.Schema.Type<typeof FieldBindingSchema>
export type FormFieldValueBinding = Schema.Schema.Type<typeof FormFieldValueBindingSchema>
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

export const FieldBinding = (form: string, field: string): FieldBinding =>
  FieldBindingSchema.make({ form, field })

export const FormFieldValueBinding = (binding: FieldBinding): FormFieldValueBinding =>
  FormFieldValueBindingSchema.make({ _tag: "FormFieldValueBinding", ...binding })

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

  if (ref.payload._tag === "ComponentValueBinding") {
    return makeIntent(ref.name, componentValue)
  }

  return makeIntent(ref.name, {
    form: ref.payload.form,
    field: ref.payload.field,
    value: componentValue
  })
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

const DevtoolsTimestampSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteTimestamp" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "NonNegativeTimestamp" })
)

export const SerializableIntentErrorSchema = Schema.Union([
  Schema.TaggedStruct("UnknownIntentError", {
    name: Schema.String
  }),
  Schema.TaggedStruct("IntentPayloadDecodeError", {
    name: Schema.String,
    message: Schema.String
  }),
  Schema.TaggedStruct("IntentHandlerError", {
    name: Schema.String,
    message: Schema.String
  })
])
export type SerializableIntentError = Schema.Schema.Type<typeof SerializableIntentErrorSchema>

export const SerializableIntentResultSchema = Schema.Union([
  Schema.TaggedStruct("Success", {}),
  Schema.TaggedStruct("Failure", {
    error: SerializableIntentErrorSchema
  })
])
export type SerializableIntentResult = Schema.Schema.Type<typeof SerializableIntentResultSchema>

export const SerializableIntentEventSchema = Schema.Struct({
  timestamp: DevtoolsTimestampSchema,
  intent: IntentSchema,
  result: SerializableIntentResultSchema
})
export type SerializableIntentEvent = Schema.Schema.Type<typeof SerializableIntentEventSchema>

const serializeIntentError = (error: IntentError): SerializableIntentError => {
  switch (error._tag) {
    case "UnknownIntentError":
      return SerializableIntentErrorSchema.make({ _tag: "UnknownIntentError", name: error.name })
    case "IntentPayloadDecodeError":
      return SerializableIntentErrorSchema.make({
        _tag: "IntentPayloadDecodeError",
        name: error.name,
        message: error.message
      })
    case "IntentHandlerError":
      return SerializableIntentErrorSchema.make({
        _tag: "IntentHandlerError",
        name: error.name,
        message: error.message
      })
  }
}

const isIntentError = (value: unknown): value is IntentError =>
  typeof value === "object" &&
  value !== null &&
  "_tag" in value &&
  (
    value._tag === "UnknownIntentError" ||
    value._tag === "IntentPayloadDecodeError" ||
    value._tag === "IntentHandlerError"
  )

export const serializeIntentEvent = (event: IntentEvent): SerializableIntentEvent => {
  if (Exit.isSuccess(event.result)) {
    return SerializableIntentEventSchema.make({
      timestamp: event.timestamp,
      intent: event.intent,
      result: { _tag: "Success" }
    })
  }

  const error = Cause.findErrorOption(event.result.cause)
  return SerializableIntentEventSchema.make({
    timestamp: event.timestamp,
    intent: event.intent,
    result: {
      _tag: "Failure",
      error: Option.isSome(error) && isIntentError(error.value)
        ? serializeIntentError(error.value)
        : {
            _tag: "IntentHandlerError",
            name: event.intent.name,
            message: String(event.result.cause)
          }
    }
  })
}

export const DevtoolsEventSchema = Schema.Union([
  Schema.TaggedStruct("StateSnapshot", {
    timestamp: DevtoolsTimestampSchema,
    state: JsonPayloadSchema
  }),
  Schema.TaggedStruct("ViewEmitted", {
    timestamp: DevtoolsTimestampSchema,
    view: Schema.suspend(() => ViewSchema)
  }),
  Schema.TaggedStruct("IntentDispatched", {
    timestamp: DevtoolsTimestampSchema,
    event: SerializableIntentEventSchema
  })
])
export type DevtoolsEvent = Schema.Schema.Type<typeof DevtoolsEventSchema>

export interface DevtoolsSink {
  readonly emit: (event: DevtoolsEvent) => void
}

export const noopDevtoolsSink: DevtoolsSink = {
  emit: () => {
    // Intentionally empty: the default runtime path does not allocate events.
  }
}

export const DevtoolsSink = Context.Service<DevtoolsSink>("@effect-native/core/DevtoolsSink")

export const makeDevtoolsSinkLayer = (sink: DevtoolsSink) => Layer.succeed(DevtoolsSink, sink)

export interface IntentRegistry {
  readonly dispatch: (intent: Intent<string, JsonPayload>) => Effect.Effect<void, IntentError>
  readonly events: Effect.Effect<ReadonlyArray<IntentEvent>>
  readonly stream: Stream.Stream<IntentEvent>
}

export const IntentRegistry = Context.Service<IntentRegistry>("@effect-native/core/IntentRegistry")

export interface IntentRegistryOptions {
  readonly now?: () => number
  readonly redactIntent?: (intent: Intent<string, JsonPayload>) => Intent<string, JsonPayload>
  readonly devtoolsSink?: DevtoolsSink
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
    const redactIntent = options.redactIntent ?? ((intent: Intent<string, JsonPayload>) => intent)
    const devtoolsSink = options.devtoolsSink
    const definitionsByName = new Map<string, IntentDefinition>(
      definitions.map((definition) => [definition.name, definition])
    )

    const appendEvent = (event: IntentEvent) =>
      Effect.gen(function*() {
        yield* Ref.update(eventsRef, (events) => [...events, event])
        yield* PubSub.publish(eventsPubSub, event)
        if (devtoolsSink !== undefined) {
          devtoolsSink.emit({
            _tag: "IntentDispatched",
            timestamp: event.timestamp,
            event: serializeIntentEvent(event)
          })
        }
      })

    const failWith = (intent: Intent<string, JsonPayload>, error: IntentError) =>
      Effect.gen(function*() {
        const result = Exit.fail(error)
        yield* appendEvent({ timestamp: now(), intent: redactIntent(intent), result })
        return yield* Effect.fail(error)
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
            intent: redactIntent(intent),
            result: Exit.fail(error)
          })
          return yield* Effect.fail(error)
        }

        yield* appendEvent({
          timestamp: now(),
          intent: redactIntent(intent),
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
export const UrlTargetSchema = Schema.Literals(["self", "blank"] as const)
export const SheetEdgeSchema = Schema.Literals(["bottom", "side"] as const)

export type StackDirection = Schema.Schema.Type<typeof StackDirectionSchema>
export type StackAlign = Schema.Schema.Type<typeof StackAlignSchema>
export type StackJustify = Schema.Schema.Type<typeof StackJustifySchema>
export type TextWeight = Schema.Schema.Type<typeof TextWeightSchema>
export type ButtonVariant = Schema.Schema.Type<typeof ButtonVariantSchema>
export type ImageFit = Schema.Schema.Type<typeof ImageFitSchema>
export type UrlTarget = Schema.Schema.Type<typeof UrlTargetSchema>
export type SheetEdge = Schema.Schema.Type<typeof SheetEdgeSchema>

export const UriStringSchema = Schema.String.check(
  Schema.isPattern(/^[a-z][a-z0-9+.-]*:/i, {
    title: "URI"
  })
)
export const PathStringSchema = Schema.String.check(
  Schema.isPattern(/^\/(?:[^?#\s]*)?(?:\?[^#\s]*)?(?:#[^\s]*)?$/, {
    title: "AbsolutePath"
  })
)
export const AnchorIdSchema = Schema.NonEmptyString.check(
  Schema.isPattern(/^[A-Za-z][A-Za-z0-9_.:-]*$/, {
    title: "AnchorId"
  })
)
export const UrlDestinationSchema = Schema.Struct({
  kind: Schema.Literal("url"),
  href: UriStringSchema,
  target: UrlTargetSchema.pipe(Schema.optionalKey)
})
export const PathDestinationSchema = Schema.Struct({
  kind: Schema.Literal("path"),
  path: PathStringSchema,
  replace: Schema.Boolean.pipe(Schema.optionalKey)
})
export const AnchorDestinationSchema = Schema.Struct({
  kind: Schema.Literal("anchor"),
  id: AnchorIdSchema
})
export const NavigationDestinationSchema = Schema.Union([
  UrlDestinationSchema,
  PathDestinationSchema,
  AnchorDestinationSchema
])
export type UrlDestination = Schema.Schema.Type<typeof UrlDestinationSchema>
export type PathDestination = Schema.Schema.Type<typeof PathDestinationSchema>
export type AnchorDestination = Schema.Schema.Type<typeof AnchorDestinationSchema>
export type NavigationDestination = Schema.Schema.Type<typeof NavigationDestinationSchema>

export interface NavigationHandler {
  readonly navigate: (destination: NavigationDestination) => Effect.Effect<void, unknown>
}

export const NavigationHandler = Context.Service<NavigationHandler>("@effect-native/core/NavigationHandler")
export const Navigate = defineIntent("Navigate", NavigationDestinationSchema)
export const navigationIntentDefinitions = [Navigate] as const
export const makeNavigationIntentHandlers = (
  handler: NavigationHandler
): IntentHandlers<typeof navigationIntentDefinitions> => ({
  Navigate: (destination) =>
    handler.navigate(destination)
})
export const makeNavigateIntent = (destination: NavigationDestination): IntentRef =>
  IntentRef("Navigate", StaticPayload(destination))
export const makeNavigationIntentRegistryLayer = (options?: IntentRegistryOptions) =>
  Layer.effect(
    IntentRegistry,
    Effect.gen(function*() {
      const handler = yield* NavigationHandler
      return yield* makeIntentRegistry(
        navigationIntentDefinitions,
        makeNavigationIntentHandlers(handler),
        options
      )
    })
  )

export const ValidateOnSchema = Schema.Literals(["change", "blur", "submit"] as const)
export type ValidateOn = Schema.Schema.Type<typeof ValidateOnSchema>

export const FormFieldChangedPayloadSchema = Schema.Struct({
  form: Schema.NonEmptyString,
  field: Schema.NonEmptyString,
  value: JsonPayloadSchema
})
export const FormFieldBlurredPayloadSchema = Schema.Struct({
  form: Schema.NonEmptyString,
  field: Schema.NonEmptyString
})
export const FormSubmitRequestedPayloadSchema = Schema.Struct({
  form: Schema.NonEmptyString,
  via: Schema.String.pipe(Schema.optionalKey)
})
export type FormFieldChangedPayload = Schema.Schema.Type<typeof FormFieldChangedPayloadSchema>
export type FormFieldBlurredPayload = Schema.Schema.Type<typeof FormFieldBlurredPayloadSchema>
export type FormSubmitRequestedPayload = Schema.Schema.Type<typeof FormSubmitRequestedPayloadSchema>

export const FormFieldChanged = defineIntent("FormFieldChanged", FormFieldChangedPayloadSchema)
export const FormFieldBlurred = defineIntent("FormFieldBlurred", FormFieldBlurredPayloadSchema)
export const FormSubmitRequested = defineIntent("FormSubmitRequested", FormSubmitRequestedPayloadSchema)
export const formIntentDefinitions = [FormFieldChanged, FormFieldBlurred, FormSubmitRequested] as const

export const FormFieldStateSchema = Schema.Struct({
  value: JsonPayloadSchema,
  touched: Schema.Boolean,
  error: Schema.String.pipe(Schema.optionalKey),
  secure: Schema.Boolean.pipe(Schema.optionalKey)
})
export const FormStateSchema = Schema.Struct({
  id: Schema.NonEmptyString,
  fields: Schema.Record(Schema.String, FormFieldStateSchema),
  focusedField: Schema.String.pipe(Schema.optionalKey)
})
export type FormFieldState = Schema.Schema.Type<typeof FormFieldStateSchema>
export type FormState = Schema.Schema.Type<typeof FormStateSchema>

export interface FormFieldSpec<
  Name extends string = string,
  S extends Schema.ConstraintDecoder<any, never> = Schema.ConstraintDecoder<any, never>
> {
  readonly name: Name
  readonly schema: S
  readonly initialValue: JsonPayload
  readonly validateOn?: ValidateOn
  readonly invalidMessage?: string
  readonly secure?: boolean
}

export interface FormSpec<Fields extends ReadonlyArray<FormFieldSpec> = ReadonlyArray<FormFieldSpec>> {
  readonly id: string
  readonly fields: Fields
}

export type FormDecodedValue<Spec extends FormSpec> = {
  readonly [Field in Spec["fields"][number] as Field["name"]]: Schema.Schema.Type<Field["schema"]>
}

export const defineFormSpec = <const Fields extends ReadonlyArray<FormFieldSpec>>(
  spec: FormSpec<Fields>
): FormSpec<Fields> => spec

const fieldState = (
  value: JsonPayload,
  input: {
    readonly touched: boolean
    readonly error?: string | undefined
    readonly secure?: boolean | undefined
  }
): FormFieldState => FormFieldStateSchema.make({
  value,
  touched: input.touched,
  ...(input.error === undefined ? {} : { error: input.error }),
  ...(input.secure === undefined ? {} : { secure: input.secure })
})

const findFormFieldSpec = (spec: FormSpec, field: string): FormFieldSpec => {
  const found = spec.fields.find((candidate) => candidate.name === field)
  if (found === undefined) {
    throw new Error(`Unknown form field: ${field}`)
  }
  return found
}

const fieldMessage = (field: FormFieldSpec): string => field.invalidMessage ?? "Invalid value."

const validateFieldState = (
  field: FormFieldSpec,
  state: FormFieldState
): { readonly state: FormFieldState; readonly value?: JsonPayload } => {
  const decoded = Schema.decodeUnknownExit(field.schema)(state.value)
  if (Exit.isSuccess(decoded)) {
    const json = Schema.decodeUnknownExit(JsonPayloadSchema)(decoded.value)
    return {
      state: fieldState(Exit.isSuccess(json) ? json.value : state.value, {
        touched: state.touched,
        secure: state.secure
      }),
      value: Exit.isSuccess(json) ? json.value : state.value
    }
  }

  return {
    state: fieldState(state.value, {
      touched: state.touched,
      secure: state.secure,
      error: fieldMessage(field)
    })
  }
}

export const makeFormState = (spec: FormSpec): FormState => FormStateSchema.make({
  id: spec.id,
  fields: Object.fromEntries(spec.fields.map((field) => [
    field.name,
    fieldState(field.initialValue, {
      touched: false,
      secure: field.secure
    })
  ]))
})

const shouldValidateField = (field: FormFieldSpec, trigger: ValidateOn): boolean =>
  (field.validateOn ?? "submit") === trigger

const updateFormField = (
  spec: FormSpec,
  form: FormState,
  field: string,
  f: (field: FormFieldSpec, state: FormFieldState) => FormFieldState
): FormState => {
  const fieldSpec = findFormFieldSpec(spec, field)
  const current = form.fields[field] ?? fieldState(fieldSpec.initialValue, {
    touched: false,
    secure: fieldSpec.secure
  })
  const updated = f(fieldSpec, current)
  return FormStateSchema.make({
    id: form.id,
    fields: {
      ...form.fields,
      [field]: updated
    },
    ...(form.focusedField === undefined ? {} : { focusedField: form.focusedField })
  })
}

export const setFormFieldValue = (
  spec: FormSpec,
  form: FormState,
  field: string,
  value: JsonPayload
): FormState =>
  updateFormField(spec, form, field, (fieldSpec, current) => {
    const next = fieldState(value, {
      touched: true,
      secure: current.secure
    })
    return shouldValidateField(fieldSpec, "change") ? validateFieldState(fieldSpec, next).state : next
  })

export const blurFormField = (
  spec: FormSpec,
  form: FormState,
  field: string
): FormState =>
  updateFormField(spec, form, field, (fieldSpec, current) => {
    const next = fieldState(current.value, {
      touched: true,
      secure: current.secure,
      error: current.error
    })
    return shouldValidateField(fieldSpec, "blur") ? validateFieldState(fieldSpec, next).state : next
  })

export type FormSubmitResult<Spec extends FormSpec = FormSpec> =
  | {
      readonly valid: true
      readonly state: FormState
      readonly value: FormDecodedValue<Spec>
    }
  | {
      readonly valid: false
      readonly state: FormState
      readonly firstInvalid?: string
    }

export const submitForm = <Spec extends FormSpec>(
  spec: Spec,
  form: FormState
): FormSubmitResult<Spec> => {
  const fields: Record<string, FormFieldState> = { ...form.fields }
  const value: Record<string, JsonPayload> = {}
  let firstInvalid: string | undefined

  for (const field of spec.fields) {
    const current = fields[field.name] ?? fieldState(field.initialValue, {
      touched: false,
      secure: field.secure
    })
    const checked = validateFieldState(field, fieldState(current.value, {
      touched: true,
      secure: current.secure
    }))
    fields[field.name] = checked.state
    if (checked.value === undefined) {
      firstInvalid ??= field.name
    } else {
      value[field.name] = checked.value
    }
  }

  const state = FormStateSchema.make({
    id: form.id,
    fields,
    ...(firstInvalid === undefined ? {} : { focusedField: firstInvalid })
  })

  if (firstInvalid !== undefined) {
    return { valid: false, state, firstInvalid }
  }

  return {
    valid: true,
    state,
    value: value as FormDecodedValue<Spec>
  }
}

export const formFieldValue = (form: FormState, field: string): string => {
  const value = form.fields[field]?.value
  return typeof value === "string" ? value : value === undefined || value === null ? "" : String(value)
}

export const formFieldError = (form: FormState, field: string): string =>
  form.fields[field]?.error ?? ""

export const formFieldFocused = (form: FormState, field: string): boolean =>
  form.focusedField === field

export const redactedValue = "[redacted]" as const

const redactPayloadFields = (
  spec: FormSpec,
  payload: JsonPayload
): JsonPayload => {
  if (typeof payload !== "object" || payload === null || Array.isArray(payload)) {
    return payload
  }
  const secure = new Set(spec.fields.filter((field) => field.secure === true).map((field) => field.name))
  if (secure.size === 0) {
    return payload
  }
  const next: Record<string, JsonPayload> = { ...(payload as Record<string, JsonPayload>) }
  for (const field of secure) {
    if (field in next) {
      next[field] = redactedValue
    }
  }
  if (
    next.form === spec.id &&
    typeof next.field === "string" &&
    secure.has(next.field) &&
    "value" in next
  ) {
    next.value = redactedValue
  }
  return next
}

export const makeFormIntentRedactor = (
  specs: ReadonlyArray<FormSpec>
): ((intent: Intent<string, JsonPayload>) => Intent<string, JsonPayload>) => {
  const byId = new Map(specs.map((spec) => [spec.id, spec]))
  return (intent) => {
    let payload = intent.payload
    if (typeof payload === "object" && payload !== null && !Array.isArray(payload)) {
      const formId = (payload as Record<string, JsonPayload>).form
      if (typeof formId === "string") {
        const spec = byId.get(formId)
        if (spec !== undefined) {
          payload = redactPayloadFields(spec, payload)
        }
      } else {
        for (const spec of specs) {
          payload = redactPayloadFields(spec, payload)
        }
      }
    }
    return makeIntent(intent.name, payload)
  }
}

export const redactFormState = (form: FormState): FormState => FormStateSchema.make({
  id: form.id,
  fields: Object.fromEntries(Object.entries(form.fields).map(([field, state]) => [
    field,
    {
      ...state,
      value: state.secure === true ? redactedValue : state.value
    }
  ])),
  ...(form.focusedField === undefined ? {} : { focusedField: form.focusedField })
})

export const NonNegativeNumberSchema = Schema.Number.check(
  Schema.isFinite({ title: "FiniteNumber" }),
  Schema.isGreaterThanOrEqualTo(0, { title: "NonNegativeNumber" })
)
export const DimensionSchema = Schema.Union([DimensionTokenSchema, NonNegativeNumberSchema])
export type Dimension = Schema.Schema.Type<typeof DimensionSchema>

export const ViewportInputSchema = Schema.Struct({
  width: NonNegativeNumberSchema,
  height: NonNegativeNumberSchema
})
export const ViewportSchema = Schema.Struct({
  width: NonNegativeNumberSchema,
  height: NonNegativeNumberSchema,
  breakpoint: BreakpointTokenSchema
})
export type ViewportInput = Schema.Schema.Type<typeof ViewportInputSchema>
export type Viewport = Schema.Schema.Type<typeof ViewportSchema>

export const defaultViewportInput: ViewportInput = {
  width: 1024,
  height: 768
}

export const deriveActiveBreakpoint = (
  width: number,
  breakpoints: BreakpointTheme = defaultTheme.breakpoint
): BreakpointToken => {
  let active: BreakpointToken = breakpointTokens[0]
  for (const token of breakpointTokens) {
    if (width >= breakpoints[token]) {
      active = token
    }
  }
  return active
}

export const makeViewport = (
  input: ViewportInput = defaultViewportInput,
  theme: Theme = defaultTheme
): Viewport => ViewportSchema.make({
  width: input.width,
  height: input.height,
  breakpoint: deriveActiveBreakpoint(input.width, theme.breakpoint)
})

export interface ViewportService {
  readonly current: Effect.Effect<Viewport>
  readonly stream: Stream.Stream<Viewport>
  readonly set: (input: ViewportInput) => Effect.Effect<void>
}

export const ViewportService = Context.Service<ViewportService>("@effect-native/core/ViewportService")

export const makeViewportService = (
  initial: ViewportInput = defaultViewportInput,
  options: { readonly theme?: Theme } = {}
): Effect.Effect<ViewportService> =>
  Effect.gen(function*() {
    const theme = options.theme ?? defaultTheme
    const ref = yield* SubscriptionRef.make(makeViewport(initial, theme))

    return {
      current: SubscriptionRef.get(ref),
      stream: SubscriptionRef.changes(ref),
      set: (input) => SubscriptionRef.set(ref, makeViewport(input, theme))
    }
  })

export const makeViewportServiceLayer = (
  initial: ViewportInput = defaultViewportInput,
  options?: { readonly theme?: Theme }
) => Layer.effect(ViewportService, makeViewportService(initial, options))

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
export type ResponsiveBreakpoints<T> = {
  readonly base: T
} & {
  readonly [Key in BreakpointToken]?: T
}
export type ResponsiveValue<T> = T | ResponsiveBreakpoints<T>
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
export const linkStyleKeys = buttonStyleKeys
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
export type LinkStyle = StyleFor<(typeof linkStyleKeys)[number]>
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

// Exact struct: accepts exactly the declared keys and rejects any excess key.
//
// This is implemented by baking `onExcessProperty: "error"` into the struct's
// AST via `annotate`, rather than by pairing the struct with a rest
// `Record(ExtraKey, Never)`. The rest-record approach is fragile: it depends on
// `StructWithRest` routing *only* non-struct keys through the rest record. Some
// runtimes/bundles instead run the rest record's key schema against every key,
// including known ones, which made a valid known style key such as `width`
// fail with "Known key belongs to the struct" at `["style"]["width"]`
// (issue #44, blocking the openagents.com /stage1 port). Baking the excess-key
// policy into the schema itself preserves exact/unknown-key rejection while
// keeping every declared key accepted, independent of that routing behavior.
const exactStruct = <const Fields extends Schema.Struct.Fields>(fields: Fields) =>
  Schema.Struct(fields).annotate({ parseOptions: { onExcessProperty: "error" } })

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

export const makeResponsiveValueSchema = <const S extends Schema.Constraint>(
  schema: S
): Schema.Codec<ResponsiveValue<Schema.Schema.Type<S>>, ResponsiveValue<Schema.Schema.Type<S>>> =>
  Schema.Union([
    schema,
    exactStruct({
      base: schema,
      ...optionalVariantFields(breakpointTokens, schema)
    })
  ]) as unknown as Schema.Codec<ResponsiveValue<Schema.Schema.Type<S>>, ResponsiveValue<Schema.Schema.Type<S>>>

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
export const LinkStyleSchema = makeStyleSchema(linkStyleKeys)
export const ImageStyleSchema = makeStyleSchema(imageStyleKeys)
export const TextFieldStyleSchema = makeStyleSchema(textFieldStyleKeys)
export const ListStyleSchema = makeStyleSchema(listStyleKeys)
export const CardStyleSchema = makeStyleSchema(cardStyleKeys)
export const SpacerStyleSchema = makeStyleSchema(spacerStyleKeys)

export const ResponsiveStackDirectionSchema = makeResponsiveValueSchema(StackDirectionSchema)
export const ResponsiveSpacingTokenSchema = makeResponsiveValueSchema(SpacingTokenSchema)
export const ResponsiveDimensionSchema = makeResponsiveValueSchema(DimensionSchema)

// ── Interaction algebra expansion (issue #24) ────────────────────────────────
// Named, typed, closure-free interaction bindings for desktop-class surfaces.
// Every event is projected to a bounded descriptor; no raw DOM event object
// ever appears in the serializable view tree. Keyboard uses a closed key-name
// set plus modifier booleans (never a raw KeyboardEvent). Imperative view
// effects (focus, auto-pin-to-end) are expressed declaratively on the tree so
// the headless renderer records them and app code never reaches for the DOM.

export const keyNames = [
  "Enter",
  "Escape",
  "Tab",
  "Backspace",
  "Delete",
  "Space",
  "ArrowUp",
  "ArrowDown",
  "ArrowLeft",
  "ArrowRight",
  "Home",
  "End",
  "PageUp",
  "PageDown"
] as const
export const KeyNameSchema = Schema.Literals(keyNames)
export type KeyName = (typeof keyNames)[number]

export interface KeyBinding {
  readonly key: KeyName
  readonly alt?: boolean
  readonly ctrl?: boolean
  readonly meta?: boolean
  readonly shift?: boolean
  // When omitted the binding is skipped while an IME composition is active,
  // matching composer submit-vs-newline semantics. Set true to fire regardless.
  readonly whenComposing?: boolean
  readonly preventDefault?: boolean
  readonly stopPropagation?: boolean
  readonly intent: IntentRef
}
export const KeyBindingSchema: Schema.Codec<KeyBinding, KeyBinding> = exactStruct({
  key: KeyNameSchema,
  alt: Schema.Boolean.pipe(Schema.optionalKey),
  ctrl: Schema.Boolean.pipe(Schema.optionalKey),
  meta: Schema.Boolean.pipe(Schema.optionalKey),
  shift: Schema.Boolean.pipe(Schema.optionalKey),
  whenComposing: Schema.Boolean.pipe(Schema.optionalKey),
  preventDefault: Schema.Boolean.pipe(Schema.optionalKey),
  stopPropagation: Schema.Boolean.pipe(Schema.optionalKey),
  intent: IntentRefSchema
}) as unknown as Schema.Codec<KeyBinding, KeyBinding>

// Swipe direction set for mobile gesture expansion (#56).
export const swipeDirections = ["left", "right", "up", "down"] as const
export const SwipeDirectionSchema = Schema.Literals(swipeDirections)
export type SwipeDirection = (typeof swipeDirections)[number]

export interface Interactions {
  readonly onKey?: ReadonlyArray<KeyBinding>
  readonly onFocus?: IntentRef
  readonly onBlur?: IntentRef
  readonly onPointerEnter?: IntentRef
  readonly onPointerLeave?: IntentRef
  readonly onPaste?: IntentRef
  readonly onDragEnter?: IntentRef
  readonly onDragLeave?: IntentRef
  readonly onDrop?: IntentRef
  /** Mobile long-press; payload is optional pointer position as data. */
  readonly onLongPress?: IntentRef
  /** Mobile swipe commit (direction carried as runtime payload by the renderer). */
  readonly onSwipe?: IntentRef
  /** Mobile pull-to-refresh when not owned by List.onRefresh. */
  readonly onPullToRefresh?: IntentRef
}
export const InteractionsSchema: Schema.Codec<Interactions, Interactions> = exactStruct({
  onKey: Schema.Array(KeyBindingSchema).pipe(Schema.optionalKey),
  onFocus: IntentRefSchema.pipe(Schema.optionalKey),
  onBlur: IntentRefSchema.pipe(Schema.optionalKey),
  onPointerEnter: IntentRefSchema.pipe(Schema.optionalKey),
  onPointerLeave: IntentRefSchema.pipe(Schema.optionalKey),
  onPaste: IntentRefSchema.pipe(Schema.optionalKey),
  onDragEnter: IntentRefSchema.pipe(Schema.optionalKey),
  onDragLeave: IntentRefSchema.pipe(Schema.optionalKey),
  onDrop: IntentRefSchema.pipe(Schema.optionalKey),
  onLongPress: IntentRefSchema.pipe(Schema.optionalKey),
  onSwipe: IntentRefSchema.pipe(Schema.optionalKey),
  onPullToRefresh: IntentRefSchema.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<Interactions, Interactions>

// Bounded ARIA roles the renderers honor for roving-focus / combobox patterns.
export const ariaRoles = [
  "listbox",
  "option",
  "combobox",
  "menu",
  "menuitem",
  "dialog",
  "group",
  "list",
  "listitem",
  "region",
  "tablist",
  "tab",
  "tabpanel",
  "none",
  "presentation"
] as const
export const AriaRoleSchema = Schema.Literals(ariaRoles)
export type AriaRole = (typeof ariaRoles)[number]

export interface A11y {
  readonly role?: AriaRole
  readonly label?: string
  // References another node's `key`; the renderer maps it to that node's id.
  readonly activeDescendant?: string
  readonly selected?: boolean
  readonly expanded?: boolean
  readonly disabled?: boolean
  readonly hidden?: boolean
  readonly tabIndex?: -1 | 0
}
export const A11ySchema: Schema.Codec<A11y, A11y> = exactStruct({
  role: AriaRoleSchema.pipe(Schema.optionalKey),
  label: Schema.String.pipe(Schema.optionalKey),
  activeDescendant: Schema.String.pipe(Schema.optionalKey),
  selected: Schema.Boolean.pipe(Schema.optionalKey),
  expanded: Schema.Boolean.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  hidden: Schema.Boolean.pipe(Schema.optionalKey),
  tabIndex: Schema.Literals([-1, 0]).pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<A11y, A11y>

// Typed dropped-item descriptor produced by drag-and-drop drops. Only bounded
// file metadata is projected into the intent payload — never the raw
// File/DataTransfer object.
export interface DroppedItem {
  readonly name: string
  readonly kind: "file" | "string"
  readonly mimeType: string
  readonly size: number
}
export const DroppedItemSchema: Schema.Codec<DroppedItem, DroppedItem> = Schema.Struct({
  name: Schema.String,
  kind: Schema.Literals(["file", "string"]),
  mimeType: Schema.String,
  size: NonNegativeNumberSchema
}) as unknown as Schema.Codec<DroppedItem, DroppedItem>
export const DropPayloadSchema = Schema.Struct({
  items: Schema.Array(DroppedItemSchema)
})
export type DropPayload = Schema.Schema.Type<typeof DropPayloadSchema>

// Closed host-kind registry for the foreign-host escape hatch (issue #23). A
// new host kind is a reviewed catalog change with the same growth-rule bar as
// a component — never an open plugin point.
export const hostKinds = ["code-editor", "terminal", "canvas", "voice-input", "on-device-model", "media-video"] as const
export const HostKindSchema = Schema.Literals(hostKinds)
export type HostKind = (typeof hostKinds)[number]

// Closed icon-name set for the Icon catalog component (issue #31). No arbitrary
// SVG string ever enters the public contract; the name set is the stable
// contract and per-renderer registries own the concrete assets. Seeded with the
// glyphs Khala Code actually uses (fleet controls, nav, status, menus). Growing
// the set is a small, reviewed catalog change — never an escape hatch.
export const iconNames = [
  "Plus",
  "Play",
  "Pause",
  "Stop",
  "Reload",
  "Circle",
  "Check",
  "X",
  "ChevronUp",
  "ChevronDown",
  "ChevronLeft",
  "ChevronRight"
] as const
export const IconNameSchema = Schema.Literals(iconNames)
export type IconName = (typeof iconNames)[number]

export const iconSizes = ["sm", "md", "lg"] as const
export const IconSizeSchema = Schema.Literals(iconSizes)
export type IconSize = (typeof iconSizes)[number]

// Closed tone set for data-display components (issue #39), aligned to the blue
// status system.
export const tones = ["neutral", "info", "success", "warn", "danger"] as const
export const ToneSchema = Schema.Literals(tones)

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

const isResponsiveBreakpoints = <T>(value: ResponsiveValue<T>): value is ResponsiveBreakpoints<T> =>
  typeof value === "object" &&
  value !== null &&
  "base" in value

export const resolveResponsiveValue = <T>(
  value: ResponsiveValue<T>,
  viewport?: Viewport
): T => {
  if (!isResponsiveBreakpoints(value)) {
    return value
  }

  let resolved = value.base
  if (viewport === undefined) {
    return resolved
  }

  for (const token of breakpointTokens) {
    const tokenValue = value[token]
    if (tokenValue !== undefined) {
      resolved = tokenValue
    }
    if (token === viewport.breakpoint) {
      break
    }
  }

  return resolved
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
  readonly catalogVersion: CompatibleCatalogVersion
  readonly key?: NodeKey
  readonly interactions?: Interactions
  readonly a11y?: A11y
}

export interface StackView extends NodeBase {
  readonly _tag: "Stack"
  readonly direction: ResponsiveValue<StackDirection>
  readonly gap?: ResponsiveValue<SpacingToken>
  readonly align?: StackAlign
  readonly justify?: StackJustify
  readonly padding?: ResponsiveValue<SpacingToken>
  readonly style?: StackStyle
  // Scroll-region auto-pin (imperative view effect as data): when true the
  // renderer keeps the region scrolled to its end as content grows; the
  // renderer reports `onPinnedChange` with a boolean when the user scrolls
  // away from / back to the end (reproduces transcript auto-pin behavior).
  readonly pinToEnd?: boolean
  readonly onPinnedChange?: IntentRef
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
  readonly width?: ResponsiveValue<Dimension>
  readonly height?: ResponsiveValue<Dimension>
  readonly fit?: ImageFit
  readonly style?: ImageStyle
}

export interface BaseTextFieldView extends NodeBase {
  readonly _tag: "TextField"
  readonly value: string
  readonly placeholder?: string
  readonly label?: string
  readonly field?: FieldBinding
  readonly focused?: boolean
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
  readonly virtualize?: boolean
  readonly estimatedItemSize?: Dimension
  readonly onEndReached?: IntentRef
  readonly endReachedThreshold?: number
  readonly pinToEnd?: boolean
  readonly onPinnedChange?: IntentRef
  /** Pull-to-refresh state (data). When true, the list shows a refreshing control. */
  readonly refreshing?: boolean
  /** Fired when the user pulls to refresh (or activates the DOM refresh affordance). */
  readonly onRefresh?: IntentRef
  readonly items: ReadonlyArray<View & { readonly key: NodeKey }>
}

export interface SectionListSection {
  readonly key: NodeKey
  readonly header: View
  readonly items: ReadonlyArray<View & { readonly key: NodeKey }>
}

export interface SectionListView extends NodeBase {
  readonly _tag: "SectionList"
  readonly style?: ListStyle
  readonly virtualize?: boolean
  readonly estimatedItemSize?: Dimension
  readonly onEndReached?: IntentRef
  readonly endReachedThreshold?: number
  readonly stickyHeaders?: boolean
  /** Pull-to-refresh state (data). When true, the list shows a refreshing control. */
  readonly refreshing?: boolean
  /** Fired when the user pulls to refresh (or activates the DOM refresh affordance). */
  readonly onRefresh?: IntentRef
  readonly sections: ReadonlyArray<SectionListSection>
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
export type LinkChildView = TextView | ImageView | SpacerView

export interface LinkView extends NodeBase {
  readonly _tag: "Link"
  readonly destination: NavigationDestination
  readonly style?: LinkStyle
  readonly children: ReadonlyArray<LinkChildView>
}

export interface ModalView extends NodeBase {
  readonly _tag: "Modal"
  readonly title: Bound<string>
  readonly open: Bound<boolean>
  readonly dismissable: boolean
  readonly size: DimensionToken
  readonly onDismiss: IntentRef
  readonly children: ReadonlyArray<View>
}

export interface SheetView extends NodeBase {
  readonly _tag: "Sheet"
  readonly open: Bound<boolean>
  readonly dismissable: boolean
  readonly edge: SheetEdge
  readonly detents: ReadonlyArray<DimensionToken>
  readonly onDismiss: IntentRef
  readonly children: ReadonlyArray<View>
}

// Foreign-host escape hatch (issue #23). The single, catalog-blessed way to
// embed a large third-party imperative widget (Monaco, an xterm terminal, a
// raw canvas) while keeping the surrounding tree serializable. The tree carries
// only the closed host-kind tag and a serializable props payload; the widget's
// internal state is never serialized and no closures appear in the tree. The
// widget's imperative code lives only in a per-renderer host driver whose
// lifecycle is bound to an Effect Scope. Events out flow through a named typed
// intent (`onEvent`); intents in are expressed as declarative prop updates.
export interface HostView extends NodeBase {
  readonly _tag: "Host"
  readonly kind: HostKind
  readonly props: JsonPayload
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
}

// Icon catalog component (issue #31). Closed name set only; decorative vs
// meaningful is typed — a `label` present means meaningful (aria-label), absent
// means decorative (aria-hidden). Size is a token scale; color is token-driven
// and defaults to currentColor in renderers.
export interface IconView extends NodeBase {
  readonly _tag: "Icon"
  readonly name: IconName
  readonly size?: IconSize
  readonly color?: ColorToken
  readonly label?: string
  readonly style?: TextStyle
}

// Data-display catalog components (issue #39). Bounded typed building blocks
// for stat strips, tables, and status readouts, built on Text/Icon + theme
// tones. `tone` is a closed set aligned to the blue status system.
export type Tone = "neutral" | "info" | "success" | "warn" | "danger"

export interface DividerView extends NodeBase {
  readonly _tag: "Divider"
  readonly orientation?: "horizontal" | "vertical"
  readonly style?: CardStyle
}

export interface BadgeView extends NodeBase {
  readonly _tag: "Badge"
  readonly label: string
  readonly tone?: Tone
  readonly style?: CardStyle
}

export interface ChipView extends NodeBase {
  readonly _tag: "Chip"
  readonly label: string
  readonly value?: string
  readonly tone?: Tone
  readonly style?: CardStyle
}

export interface MeterView extends NodeBase {
  readonly _tag: "Meter"
  // Determinate progress in [0, 1]; omit with indeterminate: true for
  // in-flight/unknown-duration work.
  readonly value?: number
  readonly indeterminate?: boolean
  readonly label?: string
  readonly tone?: Tone
  readonly style?: CardStyle
}

export interface StatTileView extends NodeBase {
  readonly _tag: "StatTile"
  readonly label: string
  readonly value: string
  readonly tone?: Tone
  readonly style?: CardStyle
}

export interface TableColumn {
  readonly id: string
  readonly header: string
  readonly align?: "start" | "center" | "end"
  readonly width?: DimensionToken
}

export interface TableRow {
  readonly id: string
  readonly cells: ReadonlyArray<View>
}

export interface TableView extends NodeBase {
  readonly _tag: "Table"
  readonly columns: ReadonlyArray<TableColumn>
  readonly rows: ReadonlyArray<TableRow>
  readonly onRowSelect?: IntentRef
  readonly style?: CardStyle
}

// App shell catalog components (issue #27). The Khala Code Desktop top-level
// shell: a resizable SplitPane workbench beside a navigation rail. Divider drag
// and pane switching are named typed intents — no free-form drag math or
// mount/unmount closures in app code.
export interface SplitPanePane {
  readonly id: string
  readonly min?: Dimension
  readonly max?: Dimension
  readonly size?: Dimension
  readonly collapsed?: boolean
  readonly content: View
}

export interface SplitPaneView extends NodeBase {
  readonly _tag: "SplitPane"
  readonly orientation: StackDirection
  readonly panes: ReadonlyArray<SplitPanePane>
  // Divider drag reports { paneId, size } as a bounded numeric descriptor.
  readonly onResize?: IntentRef
  // Collapse/expand reports { paneId, collapsed } as typed state.
  readonly onCollapseToggle?: IntentRef
  readonly style?: CardStyle
}

export interface NavRailItem {
  readonly id: string
  readonly label: string
  readonly icon?: IconName
  readonly disabled?: boolean
}

export interface NavRailSection {
  readonly id: string
  readonly label?: string
  readonly items: ReadonlyArray<NavRailItem>
}

export interface NavRailView extends NodeBase {
  readonly _tag: "NavRail"
  readonly sections: ReadonlyArray<NavRailSection>
  readonly activeId?: string
  readonly onSelect: IntentRef
  readonly style?: CardStyle
}

export interface WorkbenchPane {
  readonly id: string
  readonly content: View
}

export interface WorkbenchView extends NodeBase {
  readonly _tag: "Workbench"
  readonly panes: ReadonlyArray<WorkbenchPane>
  readonly activePaneId: string
  // When true inactive panes stay mounted (hidden); otherwise only the active
  // pane renders. Pane switching is a typed state change (driven by NavRail /
  // Tabs onSelect), never a mount/unmount closure.
  readonly keepMounted?: boolean
  readonly style?: CardStyle
}

// Anchored overlay family (issue #28). Builds on the Modal/Sheet presence
// primitive (#13): presence is typed `open` state, dismiss is a typed intent.
// A shared placement contract (side + align) is resolved by the renderer, not
// app math; collision/flip is a DOM-renderer concern.
export type OverlaySide = "top" | "bottom" | "left" | "right"
export type OverlayAlign = "start" | "center" | "end"
export interface Placement {
  readonly side: OverlaySide
  readonly align: OverlayAlign
}

// Recursive typed menu-item model shared by DropdownMenu / ContextMenu. No
// closures — selection flows through the menu's `onSelect` with the item id.
export interface MenuItem {
  readonly id: string
  readonly label: string
  readonly icon?: IconName
  readonly disabled?: boolean
  readonly danger?: boolean
  readonly keybinding?: string
  readonly items?: ReadonlyArray<MenuItem>
}

export interface PopoverView extends NodeBase {
  readonly _tag: "Popover"
  readonly open: Bound<boolean>
  readonly placement: Placement
  // References the anchor node's `key`; the renderer positions relative to it.
  readonly anchorKey?: string
  readonly dismissable: boolean
  readonly onDismiss: IntentRef
  readonly children: ReadonlyArray<View>
}

export interface DropdownMenuView extends NodeBase {
  readonly _tag: "DropdownMenu"
  readonly open: Bound<boolean>
  readonly placement: Placement
  readonly anchorKey?: string
  readonly items: ReadonlyArray<MenuItem>
  readonly onSelect: IntentRef
  readonly onDismiss: IntentRef
  readonly style?: CardStyle
}

export interface ContextMenuView extends NodeBase {
  readonly _tag: "ContextMenu"
  readonly open: Bound<boolean>
  // Pointer-anchored position (typed, not app math).
  readonly x: number
  readonly y: number
  readonly items: ReadonlyArray<MenuItem>
  readonly onSelect: IntentRef
  readonly onDismiss: IntentRef
  readonly style?: CardStyle
}

export interface TooltipView extends NodeBase {
  readonly _tag: "Tooltip"
  readonly content: string
  readonly placement?: Placement
  readonly delayMillis?: number
  // Exactly one target the tooltip describes (aria-describedby).
  readonly children: ReadonlyArray<View>
}

// Command palette + Combobox / typeahead (issue #29). Filtering is app-supplied
// (results in as data) — no keyword/string routing in the component. Highlight
// is roving via aria-activedescendant; selection/highlight/query are named
// typed intents. CommandPalette is a modal-overlay composition of a Combobox on
// the #13 presence primitive.
export interface ComboboxOption {
  readonly id: string
  readonly label: string
  readonly subtitle?: string
  readonly icon?: IconName
  readonly group?: string
  readonly disabled?: boolean
  readonly disabledReason?: string
  readonly keybinding?: string
}

export interface ComboboxView extends NodeBase {
  readonly _tag: "Combobox"
  readonly query: string
  readonly placeholder?: string
  readonly options: ReadonlyArray<ComboboxOption>
  readonly highlightedId?: string
  readonly loading?: boolean
  readonly emptyLabel?: string
  readonly onQueryChange?: IntentRef
  readonly onHighlight?: IntentRef
  readonly onSelect: IntentRef
  readonly style?: CardStyle
}

export interface CommandPaletteView extends NodeBase {
  readonly _tag: "CommandPalette"
  readonly open: Bound<boolean>
  readonly title?: string
  readonly combobox: ComboboxView
  readonly onDismiss: IntentRef
}

// Tabs (issue #30). A typed tablist with WAI-ARIA tablist/tab/tabpanel
// semantics, roving tabindex, and arrow-key nav. Panel association is by id
// (data), never DOM position; kept-mounted vs lazy is a typed policy.
export interface TabItem {
  readonly id: string
  readonly label: string
  readonly icon?: IconName
  readonly disabled?: boolean
  readonly badge?: string
}

export interface TabPanel {
  readonly id: string
  readonly content: View
}

export interface TabsView extends NodeBase {
  readonly _tag: "Tabs"
  readonly tabs: ReadonlyArray<TabItem>
  readonly panels: ReadonlyArray<TabPanel>
  readonly selectedId: string
  readonly orientation?: "horizontal" | "vertical"
  readonly keepMounted?: boolean
  readonly onSelect: IntentRef
  readonly style?: CardStyle
}

// Rich contenteditable composer (issue #32). The app sees only a typed
// structured document (bounded inline runs + mention chips), typed attachment
// state, and named typed intents; contenteditable internals, paste
// normalization, and IME composition are owned by the renderer. Autocomplete
// triggers are typed data whose candidate list is rendered via a Combobox
// (#29) — matching/candidates are app-supplied (no keyword routing here).
export type ComposerInline =
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "mention"; readonly id: string; readonly label: string }

export interface ComposerAttachment {
  readonly id: string
  readonly name: string
  readonly mimeType: string
  readonly size: number
}

export const composerTriggers = ["slash", "mention"] as const
export type ComposerTrigger = (typeof composerTriggers)[number]

export interface ComposerAutocomplete {
  readonly trigger: ComposerTrigger
  readonly query: string
  readonly combobox: ComboboxView
}

export const composerKeyCommands = ["submit", "newline", "history-previous", "history-next"] as const
export type ComposerKeyCommand = (typeof composerKeyCommands)[number]
export type ComposerMode = "normal" | "shell"

export interface ComposerView extends NodeBase {
  readonly _tag: "Composer"
  readonly doc: ReadonlyArray<ComposerInline>
  readonly mode: ComposerMode
  readonly placeholder?: string
  readonly attachments?: ReadonlyArray<ComposerAttachment>
  readonly autocomplete?: ComposerAutocomplete
  // Fires with the normalized plaintext value of the document.
  readonly onChange?: IntentRef
  readonly onSubmit?: IntentRef
  // Fires with one of composerKeyCommands as the payload.
  readonly onKeyCommand?: IntentRef
  // Fires with bounded dropped-item metadata (DnD from the interaction algebra).
  readonly onAttachmentDrop?: IntentRef
  readonly style?: TextFieldStyle
}

// Settings form controls (issue #38). Concrete widgets the Schema-backed
// FormSpec (#12) binds to: each carries a typed value + typed onChange intent,
// disabled/invalid state, and an optional `field` binding so it drives a
// FormSpec field exactly like TextField. FieldRow is the label + control +
// description + error layout the settings panels repeat.
export interface ChoiceOption {
  readonly value: string
  readonly label: string
  readonly disabled?: boolean
}

export interface ToggleView extends NodeBase {
  readonly _tag: "Toggle"
  readonly value: boolean
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: CardStyle
}

export interface SelectView extends NodeBase {
  readonly _tag: "Select"
  readonly value: string
  readonly options: ReadonlyArray<ChoiceOption>
  readonly placeholder?: string
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: TextFieldStyle
}

export interface CheckboxView extends NodeBase {
  readonly _tag: "Checkbox"
  readonly checked: boolean
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: CardStyle
}

export interface RadioGroupView extends NodeBase {
  readonly _tag: "RadioGroup"
  readonly value: string
  readonly name: string
  readonly options: ReadonlyArray<ChoiceOption>
  readonly orientation?: "horizontal" | "vertical"
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: CardStyle
}

export interface SliderView extends NodeBase {
  readonly _tag: "Slider"
  readonly value: number
  readonly min: number
  readonly max: number
  readonly step?: number
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: CardStyle
}

export interface NumberFieldView extends NodeBase {
  readonly _tag: "NumberField"
  readonly value: number
  readonly min?: number
  readonly max?: number
  readonly step?: number
  readonly placeholder?: string
  readonly label?: string
  readonly disabled?: boolean
  readonly invalid?: boolean
  readonly field?: FieldBinding
  readonly onChange?: IntentRef
  readonly style?: TextFieldStyle
}

export interface FieldRowView extends NodeBase {
  readonly _tag: "FieldRow"
  readonly label: string
  readonly description?: string
  readonly error?: string
  // The bound control's node key, for label association.
  readonly controlKey?: string
  readonly control: View
  readonly style?: CardStyle
}

// Feedback surfaces (issue #40). Transient (Toast/ToastRegion) and persistent
// (StatusBanner) status plus a full-surface blocking RecoveryOverlay on the #13
// presence primitive. Delivery/timing is a runtime concern (enqueue via
// intent/stream); the components render the typed state they are handed.
export const toastPlacements = ["top-start", "top-end", "bottom-start", "bottom-end"] as const
export type ToastPlacement = (typeof toastPlacements)[number]

export interface NotificationModel {
  readonly id: string
  readonly tone: Tone
  readonly title: string
  readonly detail?: string
  readonly actionLabel?: string
  readonly action?: IntentRef
  readonly autoDismissMillis?: number
}

export interface ToastView extends NodeBase {
  readonly _tag: "Toast"
  readonly notification: NotificationModel
  readonly onDismiss: IntentRef
  readonly style?: CardStyle
}

export interface ToastRegionView extends NodeBase {
  readonly _tag: "ToastRegion"
  readonly notifications: ReadonlyArray<NotificationModel>
  readonly placement?: ToastPlacement
  readonly onDismiss: IntentRef
  readonly style?: CardStyle
}

export interface StatusBannerView extends NodeBase {
  readonly _tag: "StatusBanner"
  readonly tone: Tone
  readonly message: string
  readonly onRetry?: IntentRef
  readonly onDismiss?: IntentRef
  readonly style?: CardStyle
}

export interface RecoveryActionModel {
  readonly id: string
  readonly label: string
  readonly action: IntentRef
  readonly variant?: ButtonVariant
}

export interface RecoveryOverlayView extends NodeBase {
  readonly _tag: "RecoveryOverlay"
  readonly open: Bound<boolean>
  readonly title: string
  readonly message?: string
  readonly status?: string
  readonly actions: ReadonlyArray<RecoveryActionModel>
}

// Streaming transcript / markdown (issue #35). The app parses markdown to this
// typed, pre-parsed block+inline model (as Khala does today) — the catalog ships
// no parser and no arbitrary HTML enters the tree. Transcript composes a keyed,
// append-optimized list of typed message items whose bodies are ordinary
// catalog views (Markdown, Card tool-cards, CodeBlock once #36 lands).
export type MarkdownInline =
  | { readonly kind: "text"; readonly text: string }
  | { readonly kind: "code"; readonly text: string }
  | { readonly kind: "strong"; readonly children: ReadonlyArray<MarkdownInline> }
  | { readonly kind: "emphasis"; readonly children: ReadonlyArray<MarkdownInline> }
  | { readonly kind: "link"; readonly href: string; readonly children: ReadonlyArray<MarkdownInline> }

export type MarkdownBlock =
  | { readonly kind: "heading"; readonly level: 1 | 2 | 3 | 4 | 5 | 6; readonly children: ReadonlyArray<MarkdownInline> }
  | { readonly kind: "paragraph"; readonly children: ReadonlyArray<MarkdownInline> }
  | { readonly kind: "list"; readonly ordered: boolean; readonly items: ReadonlyArray<ReadonlyArray<MarkdownBlock>> }
  | { readonly kind: "blockquote"; readonly children: ReadonlyArray<MarkdownBlock> }

export interface MarkdownView extends NodeBase {
  readonly _tag: "Markdown"
  readonly blocks: ReadonlyArray<MarkdownBlock>
  readonly style?: TextStyle
}

export const transcriptRoles = ["user", "assistant", "system", "tool"] as const
export type TranscriptRole = (typeof transcriptRoles)[number]
export const transcriptStatuses = ["thinking", "streaming", "failed", "done"] as const
export type TranscriptStatus = (typeof transcriptStatuses)[number]

export interface TranscriptMessage {
  readonly key: NodeKey
  readonly role: TranscriptRole
  readonly status?: TranscriptStatus
  readonly body: ReadonlyArray<View>
}

// Syntax-highlighted CodeBlock + unified diff (issue #36). The app tokenizes
// code and parses diffs (as Khala does today via tokenizeCodeLines /
// parseUnifiedDiff); the catalog renders the pre-tokenized model — it ships no
// highlighter or diff parser, keeping the tree closed + deterministic. Token
// colors come from the blue-theme syntax tokens.
export const codeTokenKinds = ["plain", "keyword", "string", "comment", "function", "number", "operator"] as const
export type CodeTokenKind = (typeof codeTokenKinds)[number]
export interface CodeToken {
  readonly kind: CodeTokenKind
  readonly text: string
}
export interface CodeLine {
  readonly tokens: ReadonlyArray<CodeToken>
}

export interface CodeBlockView extends NodeBase {
  readonly _tag: "CodeBlock"
  readonly language?: string
  readonly lines: ReadonlyArray<CodeLine>
  readonly showLineNumbers?: boolean
  readonly startLine?: number
  readonly onCopy?: IntentRef
  readonly style?: CardStyle
}

export const diffRowKinds = ["context", "add", "remove"] as const
export type DiffRowKind = (typeof diffRowKinds)[number]
export const diffVerdicts = ["approved", "rejected", "pending"] as const
export type DiffVerdict = (typeof diffVerdicts)[number]
export interface DiffRow {
  readonly kind: DiffRowKind
  readonly tokens: ReadonlyArray<CodeToken>
  readonly oldLine?: number
  readonly newLine?: number
  // Stable id for per-line review affordances (comment/verdict).
  readonly id?: string
  readonly verdict?: DiffVerdict
  readonly comment?: string
}
export interface DiffHunk {
  readonly header: string
  readonly rows: ReadonlyArray<DiffRow>
}
export interface DiffSourceControlAction {
  readonly id: string
  readonly label: string
}
export interface DiffViewView extends NodeBase {
  readonly _tag: "DiffView"
  readonly language?: string
  readonly hunks: ReadonlyArray<DiffHunk>
  readonly layout?: "unified" | "split"
  // Review affordances as named typed intents; review state rides the rows.
  readonly onLineComment?: IntentRef
  readonly onLineVerdict?: IntentRef
  readonly onSourceControlAction?: IntentRef
  readonly actions?: ReadonlyArray<DiffSourceControlAction>
  readonly style?: CardStyle
}

// GraphFigure + Timeline (issue #37). The first catalog component targeting the
// Phase 4 canvas renderer: a typed arbiter-graph model (nodes + edges, bounded,
// no arbitrary scene data) with a typed layout policy and status→color mapping
// from theme tokens. It renders through the canvas renderer (primary) and a
// DOM/SVG fallback from the same typed model. Interactions are named typed
// intents (node select/hover, pan/zoom camera state) — no closures.
export const graphNodeKinds = ["worker", "validator", "arbiter", "task", "generic"] as const
export type GraphNodeKind = (typeof graphNodeKinds)[number]
export const graphStatuses = ["idle", "active", "success", "failed", "pending"] as const
export type GraphStatus = (typeof graphStatuses)[number]
export const graphEdgeKinds = ["flow", "dependency", "pairing"] as const
export type GraphEdgeKind = (typeof graphEdgeKinds)[number]
export const graphLayouts = ["precomputed", "force", "tree"] as const
export type GraphLayout = (typeof graphLayouts)[number]

export interface GraphNodeModel {
  readonly id: string
  readonly label: string
  readonly kind?: GraphNodeKind
  readonly status?: GraphStatus
  // Precomputed position (used when layout is "precomputed").
  readonly x?: number
  readonly y?: number
}
export interface GraphEdgeModel {
  readonly id: string
  readonly from: string
  readonly to: string
  readonly kind?: GraphEdgeKind
  readonly status?: GraphStatus
}
export interface GraphCamera {
  readonly x: number
  readonly y: number
  readonly zoom: number
}
export interface GraphFigureView extends NodeBase {
  readonly _tag: "GraphFigure"
  readonly nodes: ReadonlyArray<GraphNodeModel>
  readonly edges: ReadonlyArray<GraphEdgeModel>
  readonly layout?: GraphLayout
  readonly camera?: GraphCamera
  readonly width?: number
  readonly height?: number
  readonly onNodeSelect?: IntentRef
  readonly onNodeHover?: IntentRef
  readonly onCameraChange?: IntentRef
  readonly style?: CardStyle
}

export interface TimelineEvent {
  readonly id: string
  readonly label: string
  readonly detail?: string
  readonly time?: string
  readonly status?: GraphStatus
  // Node ids this event refers to.
  readonly refs?: ReadonlyArray<string>
}
export interface TimelineView extends NodeBase {
  readonly _tag: "Timeline"
  readonly events: ReadonlyArray<TimelineEvent>
  readonly onEventSelect?: IntentRef
  readonly style?: CardStyle
}

export interface TranscriptView extends NodeBase {
  readonly _tag: "Transcript"
  readonly messages: ReadonlyArray<TranscriptMessage>
  // Auto-pin-to-bottom while streaming; onPinnedChange fires when the user
  // scrolls away from / back to the end (the "jump to latest" affordance).
  readonly pinToEnd?: boolean
  readonly onPinnedChange?: IntentRef
  readonly virtualize?: boolean
  readonly estimatedItemSize?: Dimension
  readonly style?: ListStyle
}


// ---------------------------------------------------------------------------
// Marketing catalog (issues #46–#51, v20) — openagents.com landing demand
// ---------------------------------------------------------------------------

export type SectionWidth = "full" | "contained"
export type HeroAlign = "start" | "center"
export type AccordionMode = "single" | "multi"
export type MockupVariant = "browser" | "device" | "plain"
export type MockupTilt = "none" | "left" | "right"
export type GlowIntensity = "sm" | "md" | "lg"

export interface SectionView extends NodeBase {
  readonly _tag: "Section"
  readonly width?: SectionWidth
  readonly paddingY?: SpacingToken
  readonly background?: ColorToken
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface HeroView extends NodeBase {
  readonly _tag: "Hero"
  readonly align?: HeroAlign
  readonly headline: Bound<string>
  readonly subhead?: Bound<string>
  readonly headlineTone?: "default" | "gradient"
  readonly actions: ReadonlyArray<View>
  readonly media?: View
  readonly style?: CardStyle
}

export interface AnnouncementBadgeView extends NodeBase {
  readonly _tag: "AnnouncementBadge"
  readonly label: string
  readonly actionLabel?: string
  readonly onPress?: IntentRef
  readonly style?: CardStyle
}

export interface CtaSectionView extends NodeBase {
  readonly _tag: "CtaSection"
  readonly headline: Bound<string>
  readonly body?: Bound<string>
  readonly tone?: Tone
  readonly actions: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface FooterColumn {
  readonly id: string
  readonly title?: string
  readonly links: ReadonlyArray<View>
}

export interface FooterView extends NodeBase {
  readonly _tag: "Footer"
  readonly brand?: View
  readonly columns: ReadonlyArray<FooterColumn>
  readonly legal?: View
  readonly style?: CardStyle
}

export interface NavBarLink {
  readonly id: string
  readonly label: string
  readonly onPress: IntentRef
}

export interface NavBarView extends NodeBase {
  readonly _tag: "NavBar"
  readonly brand: View
  readonly links: ReadonlyArray<NavBarLink>
  readonly actions?: ReadonlyArray<View>
  readonly sticky?: boolean
  readonly collapsed?: boolean
  readonly onToggleMenu?: IntentRef
  readonly style?: CardStyle
}

export interface AccordionItem {
  readonly id: string
  readonly header: string
  readonly content: ReadonlyArray<View>
}

export interface AccordionView extends NodeBase {
  readonly _tag: "Accordion"
  readonly items: ReadonlyArray<AccordionItem>
  readonly mode?: AccordionMode
  readonly expandedIds: ReadonlyArray<string>
  readonly onToggle: IntentRef
  readonly style?: CardStyle
}

export interface PricingFeature {
  readonly id: string
  readonly label: string
  readonly included: boolean
}

export interface PricingColumnView extends NodeBase {
  readonly _tag: "PricingColumn"
  readonly name: string
  readonly price: string
  readonly period?: string
  readonly features: ReadonlyArray<PricingFeature>
  readonly highlighted?: boolean
  readonly ctaLabel: string
  readonly onCta: IntentRef
  readonly style?: CardStyle
}

export interface PricingTableView extends NodeBase {
  readonly _tag: "PricingTable"
  readonly columns: ReadonlyArray<PricingColumnView>
  readonly style?: CardStyle
}

export interface LogoRowItem {
  readonly id: string
  readonly source: string
  readonly alt: string
  readonly onPress?: IntentRef
}

export interface LogoRowView extends NodeBase {
  readonly _tag: "LogoRow"
  readonly logos: ReadonlyArray<LogoRowItem>
  readonly style?: CardStyle
}

export interface StatsBandItem {
  readonly id: string
  readonly label: string
  readonly value: Bound<string>
  readonly tone?: Tone
}

export interface StatsBandView extends NodeBase {
  readonly _tag: "StatsBand"
  readonly stats: ReadonlyArray<StatsBandItem>
  readonly style?: CardStyle
}

export interface GlowView extends NodeBase {
  readonly _tag: "Glow"
  readonly intensity?: GlowIntensity
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface MockupFrameView extends NodeBase {
  readonly _tag: "MockupFrame"
  readonly variant?: MockupVariant
  readonly tilt?: MockupTilt
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

// Linear onboarding stepper (issue #62) — distinct from Tabs peer selection.
export interface PagerStep {
  readonly id: string
  readonly label: string
}

export interface PagerPanel {
  readonly id: string
  readonly content: View
}

export type PagerProgress = "dots" | "bar" | "none"

export interface PagerView extends NodeBase {
  readonly _tag: "Pager"
  readonly steps: ReadonlyArray<PagerStep>
  readonly panels: ReadonlyArray<PagerPanel>
  readonly activeStepId: string
  readonly progress?: PagerProgress
  readonly canGoBack?: boolean
  readonly canAdvance?: boolean
  readonly keepMounted?: boolean
  readonly onStepChange: IntentRef
  readonly onBack?: IntentRef
  readonly onAdvance?: IntentRef
  readonly onComplete?: IntentRef
  readonly style?: CardStyle
}


// Swipe-action list row (issue #60) — composition target for List renderItem.
export interface SwipeableListAction {
  readonly id: string
  readonly label: string
  readonly icon?: IconName
  readonly tone?: Tone
  readonly destructive?: boolean
}

export interface SwipeableListItemView extends NodeBase {
  readonly _tag: "SwipeableListItem"
  readonly child: View
  readonly leadingActions?: ReadonlyArray<SwipeableListAction>
  readonly trailingActions?: ReadonlyArray<SwipeableListAction>
  readonly fullSwipeActionId?: string
  readonly onAction: IntentRef
  readonly style?: CardStyle
}


// Mobile surface treatments (issue #63) — arcade visual identity as catalog data.
export type GradientDirection = "vertical" | "horizontal" | "radial"
export type WallpaperVariant = "plain" | "city" | "mesh"
export type FrameVariant = "square" | "rounded" | "arcade"
export type SpotlightIntensity = "sm" | "md" | "lg"

export interface BackgroundGradientView extends NodeBase {
  readonly _tag: "BackgroundGradient"
  readonly direction?: GradientDirection
  readonly from?: ColorToken
  readonly to?: ColorToken
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface WallpaperView extends NodeBase {
  readonly _tag: "Wallpaper"
  readonly variant?: WallpaperVariant
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface SpotlightView extends NodeBase {
  readonly _tag: "Spotlight"
  readonly intensity?: SpotlightIntensity
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface FrameView extends NodeBase {
  readonly _tag: "Frame"
  readonly variant?: FrameVariant
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export interface BlurredPopupView extends NodeBase {
  readonly _tag: "BlurredPopup"
  readonly open: boolean
  readonly onDismiss: IntentRef
  readonly children: ReadonlyArray<View>
  readonly style?: CardStyle
}

export type View =
  | StackView
  | TextView
  | ButtonView
  | ImageView
  | TextFieldView
  | ListView
  | SectionListView
  | CardView
  | SpacerView
  | LinkView
  | ModalView
  | SheetView
  | HostView
  | IconView
  | DividerView
  | BadgeView
  | ChipView
  | MeterView
  | StatTileView
  | TableView
  | SplitPaneView
  | NavRailView
  | WorkbenchView
  | PopoverView
  | DropdownMenuView
  | ContextMenuView
  | TooltipView
  | ComboboxView
  | CommandPaletteView
  | TabsView
  | ComposerView
  | ToggleView
  | SelectView
  | CheckboxView
  | RadioGroupView
  | SliderView
  | NumberFieldView
  | FieldRowView
  | ToastView
  | ToastRegionView
  | StatusBannerView
  | RecoveryOverlayView
  | MarkdownView
  | TranscriptView
  | CodeBlockView
  | DiffViewView
  | GraphFigureView
  | TimelineView
  | SectionView
  | HeroView
  | AnnouncementBadgeView
  | CtaSectionView
  | FooterView
  | NavBarView
  | AccordionView
  | PricingColumnView
  | PricingTableView
  | LogoRowView
  | StatsBandView
  | GlowView
  | MockupFrameView
  | PagerView
  | SwipeableListItemView
  | BackgroundGradientView
  | WallpaperView
  | SpotlightView
  | FrameView
  | BlurredPopupView

export type KeyedView = View & { readonly key: NodeKey }

const childViewEntries = (
  view: View
): ReadonlyArray<{ readonly path: ReadonlyArray<string | number>; readonly view: View }> => {
  switch (view._tag) {
    case "Stack":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "List":
      return view.items.map((child, index) => ({ path: ["items", index], view: child }))
    case "SectionList":
      return view.sections.flatMap((section, sectionIndex) => [
        {
          path: ["sections", sectionIndex, "header"],
          view: section.header
        },
        ...section.items.map((child, itemIndex) => ({
          path: ["sections", sectionIndex, "items", itemIndex],
          view: child
        }))
      ])
    case "Card":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Link":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Modal":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Sheet":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Table":
      return view.rows.flatMap((row, rowIndex) =>
        row.cells.map((cell, cellIndex) => ({
          path: ["rows", rowIndex, "cells", cellIndex],
          view: cell
        })))
    case "SplitPane":
      return view.panes.map((pane, index) => ({ path: ["panes", index, "content"], view: pane.content }))
    case "Workbench":
      return view.panes.map((pane, index) => ({ path: ["panes", index, "content"], view: pane.content }))
    case "Popover":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Tooltip":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "CommandPalette":
      return [{ path: ["combobox"], view: view.combobox }]
    case "Tabs":
    case "Pager":
      return view.panels.map((panel, index) => ({ path: ["panels", index, "content"], view: panel.content }))
    case "SwipeableListItem":
      return [{ path: ["child"], view: view.child }]
    case "Composer":
      return view.autocomplete === undefined
        ? []
        : [{ path: ["autocomplete", "combobox"], view: view.autocomplete.combobox }]
    case "FieldRow":
      return [{ path: ["control"], view: view.control }]
    case "Transcript":
      return view.messages.flatMap((message, messageIndex) =>
        message.body.map((child, bodyIndex) => ({
          path: ["messages", messageIndex, "body", bodyIndex],
          view: child
        })))
    case "Section":
    case "Glow":
    case "MockupFrame":
    case "BackgroundGradient":
    case "Wallpaper":
    case "Spotlight":
    case "Frame":
    case "BlurredPopup":
      return view.children.map((child, index) => ({ path: ["children", index], view: child }))
    case "Hero":
      return [
        ...view.actions.map((child, index) => ({ path: ["actions", index], view: child })),
        ...(view.media === undefined ? [] : [{ path: ["media"] as const, view: view.media }])
      ]
    case "CtaSection":
      return view.actions.map((child, index) => ({ path: ["actions", index], view: child }))
    case "Footer":
      return [
        ...(view.brand === undefined ? [] : [{ path: ["brand"] as const, view: view.brand }]),
        ...view.columns.flatMap((column, columnIndex) =>
          column.links.map((child, linkIndex) => ({
            path: ["columns", columnIndex, "links", linkIndex] as const,
            view: child
          }))
        ),
        ...(view.legal === undefined ? [] : [{ path: ["legal"] as const, view: view.legal }])
      ]
    case "NavBar":
      return [
        { path: ["brand"], view: view.brand },
        ...(view.actions ?? []).map((child, index) => ({ path: ["actions", index], view: child }))
      ]
    case "Accordion":
      return view.items.flatMap((item, itemIndex) =>
        item.content.map((child, contentIndex) => ({
          path: ["items", itemIndex, "content", contentIndex],
          view: child
        }))
      )
    case "PricingTable":
      return view.columns.map((child, index) => ({ path: ["columns", index], view: child }))
    default:
      return []
  }
}

const findOverlayStackIssue = (
  view: View,
  path: ReadonlyArray<string | number> = [],
  insideOverlay = false,
  counts: { modal: number; sheet: number } = { modal: 0, sheet: 0 }
): { readonly path: ReadonlyArray<string | number>; readonly issue: string } | undefined => {
  const isOverlay = view._tag === "Modal" || view._tag === "Sheet"
  if (isOverlay && insideOverlay) {
    return {
      path,
      issue: "Overlay nesting beyond one modal over one sheet is not supported"
    }
  }
  if (view._tag === "Modal") {
    counts.modal += 1
  }
  if (view._tag === "Sheet") {
    counts.sheet += 1
  }
  if (counts.modal > 1 || counts.sheet > 1 || counts.modal + counts.sheet > 2) {
    return {
      path,
      issue: "At most one Modal and one Sheet may be present in a view tree"
    }
  }

  const children = childViewEntries(view)
  for (const child of children) {
    const issue = findOverlayStackIssue(
      child.view,
      [...path, ...child.path],
      insideOverlay || isOverlay,
      counts
    )
    if (issue !== undefined) {
      return issue
    }
  }

  return undefined
}

const OverlayStackFilter = Schema.makeFilter<View>((view) => findOverlayStackIssue(view))

const ViewSelf = Schema.suspend((): Schema.Codec<View, View> => ViewSchema)

const KeyedViewArraySchema = Schema.Array(ViewSelf).check(
  Schema.makeFilter<ReadonlyArray<View>>((items) => {
    const unkeyedIndex = items.findIndex((item) => item.key === undefined)
    return unkeyedIndex === -1
      ? undefined
      : { path: [unkeyedIndex, "key"], issue: "List items require explicit keys" }
  })
) as Schema.Codec<ReadonlyArray<KeyedView>, ReadonlyArray<KeyedView>>

const EndReachedThresholdSchema = NonNegativeNumberSchema

const VirtualizationFields = {
  virtualize: Schema.Boolean.pipe(Schema.optionalKey),
  estimatedItemSize: DimensionSchema.pipe(Schema.optionalKey),
  onEndReached: IntentRefSchema.pipe(Schema.optionalKey),
  endReachedThreshold: EndReachedThresholdSchema.pipe(Schema.optionalKey)
} as const

interface VirtualizationContract {
  readonly virtualize?: boolean
  readonly estimatedItemSize?: Dimension
}

const VirtualizationFilter = Schema.makeFilter<VirtualizationContract>((view) =>
  view.virtualize === true && view.estimatedItemSize === undefined
    ? {
        path: ["estimatedItemSize"],
        issue: "Virtualized collections require estimatedItemSize"
      }
    : undefined
)

const CommonFields = {
  catalogVersion: CompatibleCatalogVersionSchema,
  key: NodeKeySchema.pipe(Schema.optionalKey),
  interactions: InteractionsSchema.pipe(Schema.optionalKey),
  a11y: A11ySchema.pipe(Schema.optionalKey)
} as const

export const StackSchema: Schema.Codec<StackView, StackView> = Schema.TaggedStruct("Stack", {
  ...CommonFields,
  direction: ResponsiveStackDirectionSchema,
  gap: ResponsiveSpacingTokenSchema.pipe(Schema.optionalKey),
  align: StackAlignSchema.pipe(Schema.optionalKey),
  justify: StackJustifySchema.pipe(Schema.optionalKey),
  padding: ResponsiveSpacingTokenSchema.pipe(Schema.optionalKey),
  style: StackStyleSchema.pipe(Schema.optionalKey),
  pinToEnd: Schema.Boolean.pipe(Schema.optionalKey),
  onPinnedChange: IntentRefSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf)
})

export const TextSchema: Schema.Codec<TextView, TextView> = Schema.TaggedStruct("Text", {
  ...CommonFields,
  content: BoundStringSchema,
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
  width: ResponsiveDimensionSchema.pipe(Schema.optionalKey),
  height: ResponsiveDimensionSchema.pipe(Schema.optionalKey),
  fit: ImageFitSchema.pipe(Schema.optionalKey),
  style: ImageStyleSchema.pipe(Schema.optionalKey)
})

const BaseTextFieldFields = {
  ...CommonFields,
  value: Schema.String,
  placeholder: Schema.String.pipe(Schema.optionalKey),
  label: Schema.String.pipe(Schema.optionalKey),
  field: FieldBindingSchema.pipe(Schema.optionalKey),
  focused: Schema.Boolean.pipe(Schema.optionalKey),
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
  ...VirtualizationFields,
  pinToEnd: Schema.Boolean.pipe(Schema.optionalKey),
  onPinnedChange: IntentRefSchema.pipe(Schema.optionalKey),
  refreshing: Schema.Boolean.pipe(Schema.optionalKey),
  onRefresh: IntentRefSchema.pipe(Schema.optionalKey),
  items: KeyedViewArraySchema
}).check(VirtualizationFilter)

export const SectionListSectionSchema: Schema.Codec<SectionListSection, SectionListSection> =
  Schema.Struct({
    key: NodeKeySchema,
    header: ViewSelf,
    items: KeyedViewArraySchema
  })

export const SectionListSchema: Schema.Codec<SectionListView, SectionListView> =
  Schema.TaggedStruct("SectionList", {
    ...CommonFields,
    style: ListStyleSchema.pipe(Schema.optionalKey),
    ...VirtualizationFields,
    stickyHeaders: Schema.Boolean.pipe(Schema.optionalKey),
    refreshing: Schema.Boolean.pipe(Schema.optionalKey),
    onRefresh: IntentRefSchema.pipe(Schema.optionalKey),
    sections: Schema.Array(SectionListSectionSchema)
  }).check(VirtualizationFilter)

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

export const LinkChildSchema: Schema.Codec<LinkChildView, LinkChildView> = Schema.Union([
  TextSchema,
  ImageSchema,
  SpacerSchema
])

export const LinkSchema: Schema.Codec<LinkView, LinkView> = Schema.TaggedStruct("Link", {
  ...CommonFields,
  destination: NavigationDestinationSchema,
  style: LinkStyleSchema.pipe(Schema.optionalKey),
  children: Schema.Array(LinkChildSchema).check(
    Schema.isMinLength(1, { title: "NonEmptyLinkChildren" })
  )
})

const SheetDetentsSchema = Schema.Array(DimensionTokenSchema).check(
  Schema.makeFilter<ReadonlyArray<DimensionToken>>((detents) =>
    detents.length > 0 && detents.length <= 3
      ? undefined
      : { path: [], issue: "Sheet detents require one to three size tokens" }
  )
) as Schema.Codec<ReadonlyArray<DimensionToken>, ReadonlyArray<DimensionToken>>

export const ModalSchema: Schema.Codec<ModalView, ModalView> = Schema.TaggedStruct("Modal", {
  ...CommonFields,
  title: BoundStringSchema,
  open: BoundBooleanSchema,
  dismissable: Schema.Boolean,
  size: DimensionTokenSchema,
  onDismiss: IntentRefSchema,
  children: Schema.Array(ViewSelf)
})

export const SheetSchema: Schema.Codec<SheetView, SheetView> = Schema.TaggedStruct("Sheet", {
  ...CommonFields,
  open: BoundBooleanSchema,
  dismissable: Schema.Boolean,
  edge: SheetEdgeSchema,
  detents: SheetDetentsSchema,
  onDismiss: IntentRefSchema,
  children: Schema.Array(ViewSelf)
})

export const HostSchema: Schema.Codec<HostView, HostView> = Schema.TaggedStruct("Host", {
  ...CommonFields,
  kind: HostKindSchema,
  props: JsonPayloadSchema,
  onEvent: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const IconSchema: Schema.Codec<IconView, IconView> = Schema.TaggedStruct("Icon", {
  ...CommonFields,
  name: IconNameSchema,
  size: IconSizeSchema.pipe(Schema.optionalKey),
  color: ColorTokenSchema.pipe(Schema.optionalKey),
  label: Schema.String.pipe(Schema.optionalKey),
  style: TextStyleSchema.pipe(Schema.optionalKey)
})

export const DividerSchema: Schema.Codec<DividerView, DividerView> = Schema.TaggedStruct("Divider", {
  ...CommonFields,
  orientation: Schema.Literals(["horizontal", "vertical"]).pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const BadgeSchema: Schema.Codec<BadgeView, BadgeView> = Schema.TaggedStruct("Badge", {
  ...CommonFields,
  label: Schema.String,
  tone: ToneSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const ChipSchema: Schema.Codec<ChipView, ChipView> = Schema.TaggedStruct("Chip", {
  ...CommonFields,
  label: Schema.String,
  value: Schema.String.pipe(Schema.optionalKey),
  tone: ToneSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

const MeterValueSchema = Schema.Number.check(
  Schema.makeFilter<number>((value) =>
    value >= 0 && value <= 1 ? undefined : { path: [], issue: "Meter value must be within [0, 1]" }
  )
)

export const MeterSchema: Schema.Codec<MeterView, MeterView> = Schema.TaggedStruct("Meter", {
  ...CommonFields,
  value: MeterValueSchema.pipe(Schema.optionalKey),
  indeterminate: Schema.Boolean.pipe(Schema.optionalKey),
  label: Schema.String.pipe(Schema.optionalKey),
  tone: ToneSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const StatTileSchema: Schema.Codec<StatTileView, StatTileView> = Schema.TaggedStruct("StatTile", {
  ...CommonFields,
  label: Schema.String,
  value: Schema.String,
  tone: ToneSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const TableColumnSchema: Schema.Codec<TableColumn, TableColumn> = Schema.Struct({
  id: Schema.NonEmptyString,
  header: Schema.String,
  align: Schema.Literals(["start", "center", "end"]).pipe(Schema.optionalKey),
  width: DimensionTokenSchema.pipe(Schema.optionalKey)
})

export const TableRowSchema: Schema.Codec<TableRow, TableRow> = Schema.Struct({
  id: Schema.NonEmptyString,
  cells: Schema.Array(ViewSelf)
})

export const TableSchema: Schema.Codec<TableView, TableView> = Schema.TaggedStruct("Table", {
  ...CommonFields,
  columns: Schema.Array(TableColumnSchema),
  rows: Schema.Array(TableRowSchema),
  onRowSelect: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const SplitPanePaneSchema: Schema.Codec<SplitPanePane, SplitPanePane> = Schema.Struct({
  id: Schema.NonEmptyString,
  min: DimensionSchema.pipe(Schema.optionalKey),
  max: DimensionSchema.pipe(Schema.optionalKey),
  size: DimensionSchema.pipe(Schema.optionalKey),
  collapsed: Schema.Boolean.pipe(Schema.optionalKey),
  content: ViewSelf
})

export const SplitPaneSchema: Schema.Codec<SplitPaneView, SplitPaneView> = Schema.TaggedStruct("SplitPane", {
  ...CommonFields,
  orientation: StackDirectionSchema,
  panes: Schema.Array(SplitPanePaneSchema).check(
    Schema.isMinLength(1, { title: "NonEmptySplitPanePanes" })
  ),
  onResize: IntentRefSchema.pipe(Schema.optionalKey),
  onCollapseToggle: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const NavRailItemSchema: Schema.Codec<NavRailItem, NavRailItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  icon: IconNameSchema.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey)
})

export const NavRailSectionSchema: Schema.Codec<NavRailSection, NavRailSection> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String.pipe(Schema.optionalKey),
  items: Schema.Array(NavRailItemSchema)
})

export const NavRailSchema: Schema.Codec<NavRailView, NavRailView> = Schema.TaggedStruct("NavRail", {
  ...CommonFields,
  sections: Schema.Array(NavRailSectionSchema),
  activeId: Schema.String.pipe(Schema.optionalKey),
  onSelect: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const WorkbenchPaneSchema: Schema.Codec<WorkbenchPane, WorkbenchPane> = Schema.Struct({
  id: Schema.NonEmptyString,
  content: ViewSelf
})

export const WorkbenchSchema: Schema.Codec<WorkbenchView, WorkbenchView> = Schema.TaggedStruct("Workbench", {
  ...CommonFields,
  panes: Schema.Array(WorkbenchPaneSchema).check(
    Schema.isMinLength(1, { title: "NonEmptyWorkbenchPanes" })
  ),
  activePaneId: Schema.NonEmptyString,
  keepMounted: Schema.Boolean.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const OverlaySideSchema = Schema.Literals(["top", "bottom", "left", "right"] as const)
export const OverlayAlignSchema = Schema.Literals(["start", "center", "end"] as const)
export const PlacementSchema: Schema.Codec<Placement, Placement> = Schema.Struct({
  side: OverlaySideSchema,
  align: OverlayAlignSchema
})

const MenuItemSelf = Schema.suspend((): Schema.Codec<MenuItem, MenuItem> => MenuItemSchema)
export const MenuItemSchema: Schema.Codec<MenuItem, MenuItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  icon: IconNameSchema.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  danger: Schema.Boolean.pipe(Schema.optionalKey),
  keybinding: Schema.String.pipe(Schema.optionalKey),
  items: Schema.Array(MenuItemSelf).pipe(Schema.optionalKey)
})

export const PopoverSchema: Schema.Codec<PopoverView, PopoverView> = Schema.TaggedStruct("Popover", {
  ...CommonFields,
  open: BoundBooleanSchema,
  placement: PlacementSchema,
  anchorKey: Schema.String.pipe(Schema.optionalKey),
  dismissable: Schema.Boolean,
  onDismiss: IntentRefSchema,
  children: Schema.Array(ViewSelf)
})

export const DropdownMenuSchema: Schema.Codec<DropdownMenuView, DropdownMenuView> =
  Schema.TaggedStruct("DropdownMenu", {
    ...CommonFields,
    open: BoundBooleanSchema,
    placement: PlacementSchema,
    anchorKey: Schema.String.pipe(Schema.optionalKey),
    items: Schema.Array(MenuItemSchema),
    onSelect: IntentRefSchema,
    onDismiss: IntentRefSchema,
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })

export const ContextMenuSchema: Schema.Codec<ContextMenuView, ContextMenuView> =
  Schema.TaggedStruct("ContextMenu", {
    ...CommonFields,
    open: BoundBooleanSchema,
    x: NonNegativeNumberSchema,
    y: NonNegativeNumberSchema,
    items: Schema.Array(MenuItemSchema),
    onSelect: IntentRefSchema,
    onDismiss: IntentRefSchema,
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })

export const TooltipSchema: Schema.Codec<TooltipView, TooltipView> = Schema.TaggedStruct("Tooltip", {
  ...CommonFields,
  content: Schema.String,
  placement: PlacementSchema.pipe(Schema.optionalKey),
  delayMillis: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf).check(
    Schema.makeFilter<ReadonlyArray<View>>((children) =>
      children.length === 1 ? undefined : { path: [], issue: "Tooltip wraps exactly one target" }
    )
  )
})

export const ComboboxOptionSchema: Schema.Codec<ComboboxOption, ComboboxOption> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  subtitle: Schema.String.pipe(Schema.optionalKey),
  icon: IconNameSchema.pipe(Schema.optionalKey),
  group: Schema.String.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  disabledReason: Schema.String.pipe(Schema.optionalKey),
  keybinding: Schema.String.pipe(Schema.optionalKey)
})

export const ComboboxSchema: Schema.Codec<ComboboxView, ComboboxView> = Schema.TaggedStruct("Combobox", {
  ...CommonFields,
  query: Schema.String,
  placeholder: Schema.String.pipe(Schema.optionalKey),
  options: Schema.Array(ComboboxOptionSchema),
  highlightedId: Schema.String.pipe(Schema.optionalKey),
  loading: Schema.Boolean.pipe(Schema.optionalKey),
  emptyLabel: Schema.String.pipe(Schema.optionalKey),
  onQueryChange: IntentRefSchema.pipe(Schema.optionalKey),
  onHighlight: IntentRefSchema.pipe(Schema.optionalKey),
  onSelect: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const CommandPaletteSchema: Schema.Codec<CommandPaletteView, CommandPaletteView> =
  Schema.TaggedStruct("CommandPalette", {
    ...CommonFields,
    open: BoundBooleanSchema,
    title: Schema.String.pipe(Schema.optionalKey),
    combobox: ComboboxSchema,
    onDismiss: IntentRefSchema
  })

export const TabItemSchema: Schema.Codec<TabItem, TabItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  icon: IconNameSchema.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  badge: Schema.String.pipe(Schema.optionalKey)
})

export const TabPanelSchema: Schema.Codec<TabPanel, TabPanel> = Schema.Struct({
  id: Schema.NonEmptyString,
  content: ViewSelf
})

export const TabsSchema: Schema.Codec<TabsView, TabsView> = Schema.TaggedStruct("Tabs", {
  ...CommonFields,
  tabs: Schema.Array(TabItemSchema).check(Schema.isMinLength(1, { title: "NonEmptyTabs" })),
  panels: Schema.Array(TabPanelSchema),
  selectedId: Schema.NonEmptyString,
  orientation: Schema.Literals(["horizontal", "vertical"]).pipe(Schema.optionalKey),
  keepMounted: Schema.Boolean.pipe(Schema.optionalKey),
  onSelect: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const ComposerInlineSchema: Schema.Codec<ComposerInline, ComposerInline> = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("text"), text: Schema.String }),
  Schema.Struct({ kind: Schema.Literal("mention"), id: Schema.NonEmptyString, label: Schema.String })
]) as unknown as Schema.Codec<ComposerInline, ComposerInline>

export const ComposerAttachmentSchema: Schema.Codec<ComposerAttachment, ComposerAttachment> = Schema.Struct({
  id: Schema.NonEmptyString,
  name: Schema.String,
  mimeType: Schema.String,
  size: NonNegativeNumberSchema
})

export const ComposerAutocompleteSchema: Schema.Codec<ComposerAutocomplete, ComposerAutocomplete> = Schema.Struct({
  trigger: Schema.Literals(composerTriggers),
  query: Schema.String,
  combobox: ComboboxSchema
})

export const ComposerSchema: Schema.Codec<ComposerView, ComposerView> = Schema.TaggedStruct("Composer", {
  ...CommonFields,
  doc: Schema.Array(ComposerInlineSchema),
  mode: Schema.Literals(["normal", "shell"]),
  placeholder: Schema.String.pipe(Schema.optionalKey),
  attachments: Schema.Array(ComposerAttachmentSchema).pipe(Schema.optionalKey),
  autocomplete: ComposerAutocompleteSchema.pipe(Schema.optionalKey),
  onChange: IntentRefSchema.pipe(Schema.optionalKey),
  onSubmit: IntentRefSchema.pipe(Schema.optionalKey),
  onKeyCommand: IntentRefSchema.pipe(Schema.optionalKey),
  onAttachmentDrop: IntentRefSchema.pipe(Schema.optionalKey),
  style: TextFieldStyleSchema.pipe(Schema.optionalKey)
})

export const ChoiceOptionSchema: Schema.Codec<ChoiceOption, ChoiceOption> = Schema.Struct({
  value: Schema.String,
  label: Schema.String,
  disabled: Schema.Boolean.pipe(Schema.optionalKey)
})

const SettingsControlCommonFields = {
  ...CommonFields,
  label: Schema.String.pipe(Schema.optionalKey),
  disabled: Schema.Boolean.pipe(Schema.optionalKey),
  invalid: Schema.Boolean.pipe(Schema.optionalKey),
  field: FieldBindingSchema.pipe(Schema.optionalKey),
  onChange: IntentRefSchema.pipe(Schema.optionalKey)
} as const

export const ToggleSchema: Schema.Codec<ToggleView, ToggleView> = Schema.TaggedStruct("Toggle", {
  ...SettingsControlCommonFields,
  value: Schema.Boolean,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const SelectSchema: Schema.Codec<SelectView, SelectView> = Schema.TaggedStruct("Select", {
  ...SettingsControlCommonFields,
  value: Schema.String,
  options: Schema.Array(ChoiceOptionSchema),
  placeholder: Schema.String.pipe(Schema.optionalKey),
  style: TextFieldStyleSchema.pipe(Schema.optionalKey)
})

export const CheckboxSchema: Schema.Codec<CheckboxView, CheckboxView> = Schema.TaggedStruct("Checkbox", {
  ...SettingsControlCommonFields,
  checked: Schema.Boolean,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const RadioGroupSchema: Schema.Codec<RadioGroupView, RadioGroupView> = Schema.TaggedStruct("RadioGroup", {
  ...SettingsControlCommonFields,
  value: Schema.String,
  name: Schema.NonEmptyString,
  options: Schema.Array(ChoiceOptionSchema),
  orientation: Schema.Literals(["horizontal", "vertical"]).pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

const FiniteNumberSchema = Schema.Number.check(Schema.isFinite({ title: "FiniteNumber" }))

export const SliderSchema: Schema.Codec<SliderView, SliderView> = Schema.TaggedStruct("Slider", {
  ...SettingsControlCommonFields,
  value: FiniteNumberSchema,
  min: FiniteNumberSchema,
  max: FiniteNumberSchema,
  step: Schema.Number.check(Schema.isFinite({ title: "FiniteStep" }), Schema.isGreaterThan(0, { title: "PositiveStep" })).pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const NumberFieldSchema: Schema.Codec<NumberFieldView, NumberFieldView> = Schema.TaggedStruct("NumberField", {
  ...SettingsControlCommonFields,
  value: FiniteNumberSchema,
  min: FiniteNumberSchema.pipe(Schema.optionalKey),
  max: FiniteNumberSchema.pipe(Schema.optionalKey),
  step: Schema.Number.check(Schema.isFinite({ title: "FiniteStep" }), Schema.isGreaterThan(0, { title: "PositiveStep" })).pipe(Schema.optionalKey),
  placeholder: Schema.String.pipe(Schema.optionalKey),
  style: TextFieldStyleSchema.pipe(Schema.optionalKey)
})

export const FieldRowSchema: Schema.Codec<FieldRowView, FieldRowView> = Schema.TaggedStruct("FieldRow", {
  ...CommonFields,
  label: Schema.String,
  description: Schema.String.pipe(Schema.optionalKey),
  error: Schema.String.pipe(Schema.optionalKey),
  controlKey: Schema.String.pipe(Schema.optionalKey),
  control: ViewSelf,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const NotificationModelSchema: Schema.Codec<NotificationModel, NotificationModel> = Schema.Struct({
  id: Schema.NonEmptyString,
  tone: ToneSchema,
  title: Schema.String,
  detail: Schema.String.pipe(Schema.optionalKey),
  actionLabel: Schema.String.pipe(Schema.optionalKey),
  action: IntentRefSchema.pipe(Schema.optionalKey),
  autoDismissMillis: NonNegativeNumberSchema.pipe(Schema.optionalKey)
})

export const ToastSchema: Schema.Codec<ToastView, ToastView> = Schema.TaggedStruct("Toast", {
  ...CommonFields,
  notification: NotificationModelSchema,
  onDismiss: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const ToastRegionSchema: Schema.Codec<ToastRegionView, ToastRegionView> = Schema.TaggedStruct("ToastRegion", {
  ...CommonFields,
  notifications: Schema.Array(NotificationModelSchema),
  placement: Schema.Literals(toastPlacements).pipe(Schema.optionalKey),
  onDismiss: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const StatusBannerSchema: Schema.Codec<StatusBannerView, StatusBannerView> = Schema.TaggedStruct("StatusBanner", {
  ...CommonFields,
  tone: ToneSchema,
  message: Schema.String,
  onRetry: IntentRefSchema.pipe(Schema.optionalKey),
  onDismiss: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const RecoveryActionModelSchema: Schema.Codec<RecoveryActionModel, RecoveryActionModel> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  action: IntentRefSchema,
  variant: ButtonVariantSchema.pipe(Schema.optionalKey)
})

export const RecoveryOverlaySchema: Schema.Codec<RecoveryOverlayView, RecoveryOverlayView> =
  Schema.TaggedStruct("RecoveryOverlay", {
    ...CommonFields,
    open: BoundBooleanSchema,
    title: Schema.String,
    message: Schema.String.pipe(Schema.optionalKey),
    status: Schema.String.pipe(Schema.optionalKey),
    actions: Schema.Array(RecoveryActionModelSchema)
  })

const MarkdownInlineSelf = Schema.suspend((): Schema.Codec<MarkdownInline, MarkdownInline> => MarkdownInlineSchema)
export const MarkdownInlineSchema: Schema.Codec<MarkdownInline, MarkdownInline> = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("text"), text: Schema.String }),
  Schema.Struct({ kind: Schema.Literal("code"), text: Schema.String }),
  Schema.Struct({ kind: Schema.Literal("strong"), children: Schema.Array(MarkdownInlineSelf) }),
  Schema.Struct({ kind: Schema.Literal("emphasis"), children: Schema.Array(MarkdownInlineSelf) }),
  Schema.Struct({ kind: Schema.Literal("link"), href: UriStringSchema, children: Schema.Array(MarkdownInlineSelf) })
]) as unknown as Schema.Codec<MarkdownInline, MarkdownInline>

const MarkdownBlockSelf = Schema.suspend((): Schema.Codec<MarkdownBlock, MarkdownBlock> => MarkdownBlockSchema)
export const MarkdownBlockSchema: Schema.Codec<MarkdownBlock, MarkdownBlock> = Schema.Union([
  Schema.Struct({ kind: Schema.Literal("heading"), level: Schema.Literals([1, 2, 3, 4, 5, 6] as const), children: Schema.Array(MarkdownInlineSchema) }),
  Schema.Struct({ kind: Schema.Literal("paragraph"), children: Schema.Array(MarkdownInlineSchema) }),
  Schema.Struct({ kind: Schema.Literal("list"), ordered: Schema.Boolean, items: Schema.Array(Schema.Array(MarkdownBlockSelf)) }),
  Schema.Struct({ kind: Schema.Literal("blockquote"), children: Schema.Array(MarkdownBlockSelf) })
]) as unknown as Schema.Codec<MarkdownBlock, MarkdownBlock>

export const CodeTokenSchema: Schema.Codec<CodeToken, CodeToken> = Schema.Struct({
  kind: Schema.Literals(codeTokenKinds),
  text: Schema.String
})
export const CodeLineSchema: Schema.Codec<CodeLine, CodeLine> = Schema.Struct({
  tokens: Schema.Array(CodeTokenSchema)
})
export const CodeBlockSchema: Schema.Codec<CodeBlockView, CodeBlockView> = Schema.TaggedStruct("CodeBlock", {
  ...CommonFields,
  language: Schema.String.pipe(Schema.optionalKey),
  lines: Schema.Array(CodeLineSchema),
  showLineNumbers: Schema.Boolean.pipe(Schema.optionalKey),
  startLine: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  onCopy: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const DiffRowSchema: Schema.Codec<DiffRow, DiffRow> = Schema.Struct({
  kind: Schema.Literals(diffRowKinds),
  tokens: Schema.Array(CodeTokenSchema),
  oldLine: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  newLine: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  id: Schema.NonEmptyString.pipe(Schema.optionalKey),
  verdict: Schema.Literals(diffVerdicts).pipe(Schema.optionalKey),
  comment: Schema.String.pipe(Schema.optionalKey)
})
export const DiffHunkSchema: Schema.Codec<DiffHunk, DiffHunk> = Schema.Struct({
  header: Schema.String,
  rows: Schema.Array(DiffRowSchema)
})
export const DiffSourceControlActionSchema: Schema.Codec<DiffSourceControlAction, DiffSourceControlAction> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String
})
export const DiffViewSchema: Schema.Codec<DiffViewView, DiffViewView> = Schema.TaggedStruct("DiffView", {
  ...CommonFields,
  language: Schema.String.pipe(Schema.optionalKey),
  hunks: Schema.Array(DiffHunkSchema),
  layout: Schema.Literals(["unified", "split"]).pipe(Schema.optionalKey),
  onLineComment: IntentRefSchema.pipe(Schema.optionalKey),
  onLineVerdict: IntentRefSchema.pipe(Schema.optionalKey),
  onSourceControlAction: IntentRefSchema.pipe(Schema.optionalKey),
  actions: Schema.Array(DiffSourceControlActionSchema).pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

const GraphNumberSchema = Schema.Number.check(Schema.isFinite({ title: "FiniteGraphNumber" }))
export const GraphNodeModelSchema: Schema.Codec<GraphNodeModel, GraphNodeModel> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  kind: Schema.Literals(graphNodeKinds).pipe(Schema.optionalKey),
  status: Schema.Literals(graphStatuses).pipe(Schema.optionalKey),
  x: GraphNumberSchema.pipe(Schema.optionalKey),
  y: GraphNumberSchema.pipe(Schema.optionalKey)
})
export const GraphEdgeModelSchema: Schema.Codec<GraphEdgeModel, GraphEdgeModel> = Schema.Struct({
  id: Schema.NonEmptyString,
  from: Schema.NonEmptyString,
  to: Schema.NonEmptyString,
  kind: Schema.Literals(graphEdgeKinds).pipe(Schema.optionalKey),
  status: Schema.Literals(graphStatuses).pipe(Schema.optionalKey)
})
export const GraphCameraSchema: Schema.Codec<GraphCamera, GraphCamera> = Schema.Struct({
  x: GraphNumberSchema,
  y: GraphNumberSchema,
  zoom: Schema.Number.check(Schema.isFinite({ title: "FiniteZoom" }), Schema.isGreaterThan(0, { title: "PositiveZoom" }))
})
export const GraphFigureSchema: Schema.Codec<GraphFigureView, GraphFigureView> = Schema.TaggedStruct("GraphFigure", {
  ...CommonFields,
  nodes: Schema.Array(GraphNodeModelSchema),
  edges: Schema.Array(GraphEdgeModelSchema),
  layout: Schema.Literals(graphLayouts).pipe(Schema.optionalKey),
  camera: GraphCameraSchema.pipe(Schema.optionalKey),
  width: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  height: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  onNodeSelect: IntentRefSchema.pipe(Schema.optionalKey),
  onNodeHover: IntentRefSchema.pipe(Schema.optionalKey),
  onCameraChange: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const TimelineEventSchema: Schema.Codec<TimelineEvent, TimelineEvent> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  detail: Schema.String.pipe(Schema.optionalKey),
  time: Schema.String.pipe(Schema.optionalKey),
  status: Schema.Literals(graphStatuses).pipe(Schema.optionalKey),
  refs: Schema.Array(Schema.NonEmptyString).pipe(Schema.optionalKey)
})
export const TimelineSchema: Schema.Codec<TimelineView, TimelineView> = Schema.TaggedStruct("Timeline", {
  ...CommonFields,
  events: Schema.Array(TimelineEventSchema),
  onEventSelect: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

// Status -> theme color-token mapping shared by graph/timeline renderers.
export const graphStatusColorToken: Record<GraphStatus, ColorToken> = {
  idle: "textMuted",
  active: "info",
  success: "success",
  failed: "danger",
  pending: "warning"
}

export const MarkdownSchema: Schema.Codec<MarkdownView, MarkdownView> = Schema.TaggedStruct("Markdown", {
  ...CommonFields,
  blocks: Schema.Array(MarkdownBlockSchema),
  style: TextStyleSchema.pipe(Schema.optionalKey)
})

export const TranscriptMessageSchema: Schema.Codec<TranscriptMessage, TranscriptMessage> = Schema.Struct({
  key: NodeKeySchema,
  role: Schema.Literals(transcriptRoles),
  status: Schema.Literals(transcriptStatuses).pipe(Schema.optionalKey),
  body: Schema.Array(ViewSelf)
})

export const TranscriptSchema: Schema.Codec<TranscriptView, TranscriptView> = Schema.TaggedStruct("Transcript", {
  ...CommonFields,
  messages: Schema.Array(TranscriptMessageSchema),
  pinToEnd: Schema.Boolean.pipe(Schema.optionalKey),
  onPinnedChange: IntentRefSchema.pipe(Schema.optionalKey),
  virtualize: Schema.Boolean.pipe(Schema.optionalKey),
  estimatedItemSize: DimensionSchema.pipe(Schema.optionalKey),
  style: ListStyleSchema.pipe(Schema.optionalKey)
})


export const SectionWidthSchema = Schema.Literals(["full", "contained"] as const)
export const HeroAlignSchema = Schema.Literals(["start", "center"] as const)
export const AccordionModeSchema = Schema.Literals(["single", "multi"] as const)
export const MockupVariantSchema = Schema.Literals(["browser", "device", "plain"] as const)
export const MockupTiltSchema = Schema.Literals(["none", "left", "right"] as const)
export const GlowIntensitySchema = Schema.Literals(["sm", "md", "lg"] as const)

export const SectionSchema: Schema.Codec<SectionView, SectionView> = Schema.TaggedStruct("Section", {
  ...CommonFields,
  width: SectionWidthSchema.pipe(Schema.optionalKey),
  paddingY: SpacingTokenSchema.pipe(Schema.optionalKey),
  background: ColorTokenSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const HeroSchema: Schema.Codec<HeroView, HeroView> = Schema.TaggedStruct("Hero", {
  ...CommonFields,
  align: HeroAlignSchema.pipe(Schema.optionalKey),
  headline: BoundStringSchema,
  subhead: BoundStringSchema.pipe(Schema.optionalKey),
  headlineTone: Schema.Literals(["default", "gradient"] as const).pipe(Schema.optionalKey),
  actions: Schema.Array(ViewSelf),
  media: ViewSelf.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const AnnouncementBadgeSchema: Schema.Codec<AnnouncementBadgeView, AnnouncementBadgeView> =
  Schema.TaggedStruct("AnnouncementBadge", {
    ...CommonFields,
    label: Schema.String,
    actionLabel: Schema.String.pipe(Schema.optionalKey),
    onPress: IntentRefSchema.pipe(Schema.optionalKey),
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })

export const CtaSectionSchema: Schema.Codec<CtaSectionView, CtaSectionView> = Schema.TaggedStruct("CtaSection", {
  ...CommonFields,
  headline: BoundStringSchema,
  body: BoundStringSchema.pipe(Schema.optionalKey),
  tone: ToneSchema.pipe(Schema.optionalKey),
  actions: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const FooterColumnSchema: Schema.Codec<FooterColumn, FooterColumn> = Schema.Struct({
  id: Schema.NonEmptyString,
  title: Schema.String.pipe(Schema.optionalKey),
  links: Schema.Array(ViewSelf)
})

export const FooterSchema: Schema.Codec<FooterView, FooterView> = Schema.TaggedStruct("Footer", {
  ...CommonFields,
  brand: ViewSelf.pipe(Schema.optionalKey),
  columns: Schema.Array(FooterColumnSchema),
  legal: ViewSelf.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const NavBarLinkSchema: Schema.Codec<NavBarLink, NavBarLink> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  onPress: IntentRefSchema
})

export const NavBarSchema: Schema.Codec<NavBarView, NavBarView> = Schema.TaggedStruct("NavBar", {
  ...CommonFields,
  brand: ViewSelf,
  links: Schema.Array(NavBarLinkSchema),
  actions: Schema.Array(ViewSelf).pipe(Schema.optionalKey),
  sticky: Schema.Boolean.pipe(Schema.optionalKey),
  collapsed: Schema.Boolean.pipe(Schema.optionalKey),
  onToggleMenu: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const AccordionItemSchema: Schema.Codec<AccordionItem, AccordionItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  header: Schema.String,
  content: Schema.Array(ViewSelf)
})

export const AccordionSchema: Schema.Codec<AccordionView, AccordionView> = Schema.TaggedStruct("Accordion", {
  ...CommonFields,
  items: Schema.Array(AccordionItemSchema),
  mode: AccordionModeSchema.pipe(Schema.optionalKey),
  expandedIds: Schema.Array(Schema.NonEmptyString),
  onToggle: IntentRefSchema,
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const PricingFeatureSchema: Schema.Codec<PricingFeature, PricingFeature> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  included: Schema.Boolean
})

export const PricingColumnSchema: Schema.Codec<PricingColumnView, PricingColumnView> = Schema.TaggedStruct(
  "PricingColumn",
  {
    ...CommonFields,
    name: Schema.String,
    price: Schema.String,
    period: Schema.String.pipe(Schema.optionalKey),
    features: Schema.Array(PricingFeatureSchema),
    highlighted: Schema.Boolean.pipe(Schema.optionalKey),
    ctaLabel: Schema.String,
    onCta: IntentRefSchema,
    style: CardStyleSchema.pipe(Schema.optionalKey)
  }
)

export const PricingTableSchema: Schema.Codec<PricingTableView, PricingTableView> = Schema.TaggedStruct(
  "PricingTable",
  {
    ...CommonFields,
    columns: Schema.Array(PricingColumnSchema),
    style: CardStyleSchema.pipe(Schema.optionalKey)
  }
)

export const LogoRowItemSchema: Schema.Codec<LogoRowItem, LogoRowItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  source: UriStringSchema,
  alt: Schema.String,
  onPress: IntentRefSchema.pipe(Schema.optionalKey)
})

export const LogoRowSchema: Schema.Codec<LogoRowView, LogoRowView> = Schema.TaggedStruct("LogoRow", {
  ...CommonFields,
  logos: Schema.Array(LogoRowItemSchema),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const StatsBandItemSchema: Schema.Codec<StatsBandItem, StatsBandItem> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  value: BoundStringSchema,
  tone: ToneSchema.pipe(Schema.optionalKey)
})

export const StatsBandSchema: Schema.Codec<StatsBandView, StatsBandView> = Schema.TaggedStruct("StatsBand", {
  ...CommonFields,
  stats: Schema.Array(StatsBandItemSchema),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const GlowSchema: Schema.Codec<GlowView, GlowView> = Schema.TaggedStruct("Glow", {
  ...CommonFields,
  intensity: GlowIntensitySchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const MockupFrameSchema: Schema.Codec<MockupFrameView, MockupFrameView> = Schema.TaggedStruct(
  "MockupFrame",
  {
    ...CommonFields,
    variant: MockupVariantSchema.pipe(Schema.optionalKey),
    tilt: MockupTiltSchema.pipe(Schema.optionalKey),
    children: Schema.Array(ViewSelf),
    style: CardStyleSchema.pipe(Schema.optionalKey)
  }
)

export const PagerStepSchema: Schema.Codec<PagerStep, PagerStep> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String
})

export const PagerPanelSchema: Schema.Codec<PagerPanel, PagerPanel> = Schema.Struct({
  id: Schema.NonEmptyString,
  content: ViewSelf
})

export const PagerProgressSchema = Schema.Literals(["dots", "bar", "none"] as const)

export const PagerSchema: Schema.Codec<PagerView, PagerView> = Schema.TaggedStruct("Pager", {
  ...CommonFields,
  steps: Schema.Array(PagerStepSchema).check(Schema.isMinLength(1, { title: "NonEmptyPagerSteps" })),
  panels: Schema.Array(PagerPanelSchema),
  activeStepId: Schema.NonEmptyString,
  progress: PagerProgressSchema.pipe(Schema.optionalKey),
  canGoBack: Schema.Boolean.pipe(Schema.optionalKey),
  canAdvance: Schema.Boolean.pipe(Schema.optionalKey),
  keepMounted: Schema.Boolean.pipe(Schema.optionalKey),
  onStepChange: IntentRefSchema,
  onBack: IntentRefSchema.pipe(Schema.optionalKey),
  onAdvance: IntentRefSchema.pipe(Schema.optionalKey),
  onComplete: IntentRefSchema.pipe(Schema.optionalKey),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})


export const SwipeableListActionSchema: Schema.Codec<SwipeableListAction, SwipeableListAction> = Schema.Struct({
  id: Schema.NonEmptyString,
  label: Schema.String,
  icon: IconNameSchema.pipe(Schema.optionalKey),
  tone: ToneSchema.pipe(Schema.optionalKey),
  destructive: Schema.Boolean.pipe(Schema.optionalKey)
})

export const SwipeableListItemSchema: Schema.Codec<SwipeableListItemView, SwipeableListItemView> =
  Schema.TaggedStruct("SwipeableListItem", {
    ...CommonFields,
    child: ViewSelf,
    leadingActions: Schema.Array(SwipeableListActionSchema).pipe(Schema.optionalKey),
    trailingActions: Schema.Array(SwipeableListActionSchema).pipe(Schema.optionalKey),
    fullSwipeActionId: Schema.NonEmptyString.pipe(Schema.optionalKey),
    onAction: IntentRefSchema,
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })


export const GradientDirectionSchema = Schema.Literals(["vertical", "horizontal", "radial"] as const)
export const WallpaperVariantSchema = Schema.Literals(["plain", "city", "mesh"] as const)
export const FrameVariantSchema = Schema.Literals(["square", "rounded", "arcade"] as const)
export const SpotlightIntensitySchema = Schema.Literals(["sm", "md", "lg"] as const)

export const BackgroundGradientSchema: Schema.Codec<BackgroundGradientView, BackgroundGradientView> =
  Schema.TaggedStruct("BackgroundGradient", {
    ...CommonFields,
    direction: GradientDirectionSchema.pipe(Schema.optionalKey),
    from: ColorTokenSchema.pipe(Schema.optionalKey),
    to: ColorTokenSchema.pipe(Schema.optionalKey),
    children: Schema.Array(ViewSelf),
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })

export const WallpaperSchema: Schema.Codec<WallpaperView, WallpaperView> = Schema.TaggedStruct("Wallpaper", {
  ...CommonFields,
  variant: WallpaperVariantSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const SpotlightSchema: Schema.Codec<SpotlightView, SpotlightView> = Schema.TaggedStruct("Spotlight", {
  ...CommonFields,
  intensity: SpotlightIntensitySchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const FrameSchema: Schema.Codec<FrameView, FrameView> = Schema.TaggedStruct("Frame", {
  ...CommonFields,
  variant: FrameVariantSchema.pipe(Schema.optionalKey),
  children: Schema.Array(ViewSelf),
  style: CardStyleSchema.pipe(Schema.optionalKey)
})

export const BlurredPopupSchema: Schema.Codec<BlurredPopupView, BlurredPopupView> =
  Schema.TaggedStruct("BlurredPopup", {
    ...CommonFields,
    open: Schema.Boolean,
    onDismiss: IntentRefSchema,
    children: Schema.Array(ViewSelf),
    style: CardStyleSchema.pipe(Schema.optionalKey)
  })

export const ViewSchema: Schema.Codec<View, View> = Schema.suspend(() =>
  Schema.Union([
    StackSchema,
    TextSchema,
    ButtonSchema,
    ImageSchema,
    TextFieldSchema,
    ListSchema,
    SectionListSchema,
    CardSchema,
    SpacerSchema,
    LinkSchema,
    ModalSchema,
    SheetSchema,
    HostSchema,
    IconSchema,
    DividerSchema,
    BadgeSchema,
    ChipSchema,
    MeterSchema,
    StatTileSchema,
    TableSchema,
    SplitPaneSchema,
    NavRailSchema,
    WorkbenchSchema,
    PopoverSchema,
    DropdownMenuSchema,
    ContextMenuSchema,
    TooltipSchema,
    ComboboxSchema,
    CommandPaletteSchema,
    TabsSchema,
    ComposerSchema,
    ToggleSchema,
    SelectSchema,
    CheckboxSchema,
    RadioGroupSchema,
    SliderSchema,
    NumberFieldSchema,
    FieldRowSchema,
    ToastSchema,
    ToastRegionSchema,
    StatusBannerSchema,
    RecoveryOverlaySchema,
    MarkdownSchema,
    TranscriptSchema,
    CodeBlockSchema,
    DiffViewSchema,
    GraphFigureSchema,
    TimelineSchema,
    SectionSchema,
    HeroSchema,
    AnnouncementBadgeSchema,
    CtaSectionSchema,
    FooterSchema,
    NavBarSchema,
    AccordionSchema,
    PricingColumnSchema,
    PricingTableSchema,
    LogoRowSchema,
    StatsBandSchema,
    GlowSchema,
    MockupFrameSchema,
    PagerSchema,
    SwipeableListItemSchema,
    BackgroundGradientSchema,
    WallpaperSchema,
    SpotlightSchema,
    FrameSchema,
    BlurredPopupSchema
  ]).check(OverlayStackFilter)
)

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
  ListSchema.make({
    _tag: "List",
    catalogVersion: CatalogVersion,
    ...props,
    virtualize: props.virtualize ?? false,
    items
  })

export type SectionListProps = Omit<WithoutTagAndVersion<SectionListView>, "sections">
export const SectionList = (
  props: SectionListProps,
  sections: ReadonlyArray<SectionListSection>
): SectionListView =>
  SectionListSchema.make({
    _tag: "SectionList",
    catalogVersion: CatalogVersion,
    ...props,
    virtualize: props.virtualize ?? false,
    sections
  })

export type CardProps = Omit<WithoutTagAndVersion<CardView>, "children">
export const Card = (props: CardProps, children: ReadonlyArray<View> = []): CardView =>
  CardSchema.make({ _tag: "Card", catalogVersion: CatalogVersion, ...props, children })

export type SpacerProps = WithoutTagAndVersion<SpacerView>
export const Spacer = (props: SpacerProps): SpacerView =>
  SpacerSchema.make({ _tag: "Spacer", catalogVersion: CatalogVersion, ...props })

export type LinkProps = Omit<WithoutTagAndVersion<LinkView>, "children">
export const Link = (props: LinkProps, children: ReadonlyArray<LinkChildView>): LinkView =>
  LinkSchema.make({ _tag: "Link", catalogVersion: CatalogVersion, ...props, children })

export type ModalProps = Omit<WithoutTagAndVersion<ModalView>, "children">
export const Modal = (props: ModalProps, children: ReadonlyArray<View> = []): ModalView =>
  ModalSchema.make({ _tag: "Modal", catalogVersion: CatalogVersion, ...props, children })

export type SheetProps = Omit<WithoutTagAndVersion<SheetView>, "children">
export const Sheet = (props: SheetProps, children: ReadonlyArray<View> = []): SheetView =>
  SheetSchema.make({ _tag: "Sheet", catalogVersion: CatalogVersion, ...props, children })

export type HostProps = WithoutTagAndVersion<HostView>
export const Host = (props: HostProps): HostView =>
  HostSchema.make({ _tag: "Host", catalogVersion: CatalogVersion, ...props })

// ── CodeEditor host contract (issue #33) ─────────────────────────────────────
//
// A CodeEditor is not a new closed-catalog tag: it is a typed constructor over
// the reviewed `Host(kind: "code-editor")` escape hatch (#23), with bounded,
// serializable props and a typed event union. The concrete widget (Monaco) is
// provided by the app as a per-renderer host driver whose lifecycle is bound to
// the surface Scope; the framework owns the typed contract + driver seam, never
// a bundled editor engine. No Monaco types appear in the public contract.
export interface CodeEditorHostProps {
  readonly value: string
  readonly language: string
  readonly readOnly?: boolean
  readonly wordWrap?: boolean
  readonly minimap?: boolean
  // Font size from the token scale (never a raw pixel number).
  readonly fontScale?: TypeScaleToken
}
export const CodeEditorHostPropsSchema: Schema.Codec<CodeEditorHostProps, CodeEditorHostProps> = Schema.Struct({
  value: Schema.String,
  language: Schema.NonEmptyString,
  readOnly: Schema.Boolean.pipe(Schema.optionalKey),
  wordWrap: Schema.Boolean.pipe(Schema.optionalKey),
  minimap: Schema.Boolean.pipe(Schema.optionalKey),
  fontScale: TypeScaleTokenSchema.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<CodeEditorHostProps, CodeEditorHostProps>
export const decodeCodeEditorHostProps = Schema.decodeUnknownSync(CodeEditorHostPropsSchema)

// Typed events the code-editor driver emits outward through the Host `onEvent`
// intent (no editor types cross the boundary).
export type CodeEditorEvent =
  | { readonly type: "change"; readonly value: string }
  | { readonly type: "selection"; readonly start: number; readonly end: number }
  | { readonly type: "save"; readonly value: string }
export const CodeEditorEventSchema: Schema.Codec<CodeEditorEvent, CodeEditorEvent> = Schema.Union([
  Schema.Struct({ type: Schema.Literal("change"), value: Schema.String }),
  Schema.Struct({ type: Schema.Literal("selection"), start: NonNegativeNumberSchema, end: NonNegativeNumberSchema }),
  Schema.Struct({ type: Schema.Literal("save"), value: Schema.String })
]) as unknown as Schema.Codec<CodeEditorEvent, CodeEditorEvent>

export interface CodeEditorProps extends CodeEditorHostProps {
  readonly key?: NodeKey
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
  readonly a11y?: A11y
  readonly interactions?: Interactions
}
export const CodeEditor = (props: CodeEditorProps): HostView => {
  const { key, onEvent, style, a11y, interactions, ...hostProps } = props
  return Host({
    ...(key === undefined ? {} : { key }),
    ...(onEvent === undefined ? {} : { onEvent }),
    ...(style === undefined ? {} : { style }),
    ...(a11y === undefined ? {} : { a11y }),
    ...(interactions === undefined ? {} : { interactions }),
    kind: "code-editor",
    props: CodeEditorHostPropsSchema.make(hostProps) as unknown as JsonPayload
  })
}

// ── Terminal host contract (issue #34) ───────────────────────────────────────
//
// Like CodeEditor, a Terminal is a typed constructor over the reviewed
// `Host(kind: "terminal")` escape hatch (#23) — not a new closed-catalog tag.
// Output is delivered as a serializable `output` buffer prop (the app joins a
// byte/string Stream via the streaming data-binding runtime into this buffer);
// user input is emitted as a typed `data` event and resize as a typed `resize`
// event. No emulator (xterm) types cross the public contract; the PTY/process is
// the app's concern.
export interface TerminalHostProps {
  readonly output?: string
  readonly cols?: number
  readonly rows?: number
  readonly autoFit?: boolean
  readonly fontScale?: TypeScaleToken
  readonly scrollbackLines?: number
  readonly readOnly?: boolean
}
export const TerminalHostPropsSchema: Schema.Codec<TerminalHostProps, TerminalHostProps> = Schema.Struct({
  output: Schema.String.pipe(Schema.optionalKey),
  cols: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  rows: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  autoFit: Schema.Boolean.pipe(Schema.optionalKey),
  fontScale: TypeScaleTokenSchema.pipe(Schema.optionalKey),
  scrollbackLines: NonNegativeNumberSchema.pipe(Schema.optionalKey),
  readOnly: Schema.Boolean.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<TerminalHostProps, TerminalHostProps>
export const decodeTerminalHostProps = Schema.decodeUnknownSync(TerminalHostPropsSchema)

export type TerminalEvent =
  | { readonly type: "data"; readonly data: string }
  | { readonly type: "resize"; readonly cols: number; readonly rows: number }
export const TerminalEventSchema: Schema.Codec<TerminalEvent, TerminalEvent> = Schema.Union([
  Schema.Struct({ type: Schema.Literal("data"), data: Schema.String }),
  Schema.Struct({ type: Schema.Literal("resize"), cols: NonNegativeNumberSchema, rows: NonNegativeNumberSchema })
]) as unknown as Schema.Codec<TerminalEvent, TerminalEvent>

export interface TerminalProps extends TerminalHostProps {
  readonly key?: NodeKey
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
  readonly a11y?: A11y
  readonly interactions?: Interactions
}
export const Terminal = (props: TerminalProps): HostView => {
  const { key, onEvent, style, a11y, interactions, ...hostProps } = props
  return Host({
    ...(key === undefined ? {} : { key }),
    ...(onEvent === undefined ? {} : { onEvent }),
    ...(style === undefined ? {} : { style }),
    ...(a11y === undefined ? {} : { a11y }),
    ...(interactions === undefined ? {} : { interactions }),
    kind: "terminal",
    props: TerminalHostPropsSchema.make(hostProps) as unknown as JsonPayload
  })
}

// ── Voice / on-device model host contracts (issue #58) ────────────────────────
export interface VoiceInputHostProps {
  readonly listening?: boolean
  readonly locale?: string
  readonly partialTranscript?: string
}
export const VoiceInputHostPropsSchema: Schema.Codec<VoiceInputHostProps, VoiceInputHostProps> = Schema.Struct({
  listening: Schema.Boolean.pipe(Schema.optionalKey),
  locale: Schema.String.pipe(Schema.optionalKey),
  partialTranscript: Schema.String.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<VoiceInputHostProps, VoiceInputHostProps>
export const decodeVoiceInputHostProps = Schema.decodeUnknownSync(VoiceInputHostPropsSchema)

export type VoiceInputEvent =
  | { readonly type: "partial"; readonly text: string }
  | { readonly type: "final"; readonly text: string }
  | { readonly type: "error"; readonly message: string }
export const VoiceInputEventSchema: Schema.Codec<VoiceInputEvent, VoiceInputEvent> = Schema.Union([
  Schema.Struct({ type: Schema.Literal("partial"), text: Schema.String }),
  Schema.Struct({ type: Schema.Literal("final"), text: Schema.String }),
  Schema.Struct({ type: Schema.Literal("error"), message: Schema.String })
]) as unknown as Schema.Codec<VoiceInputEvent, VoiceInputEvent>

export interface VoiceInputProps extends VoiceInputHostProps {
  readonly key?: NodeKey
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
  readonly a11y?: A11y
  readonly interactions?: Interactions
}
export const VoiceInput = (props: VoiceInputProps): HostView => {
  const { key, onEvent, style, a11y, interactions, ...hostProps } = props
  return Host({
    ...(key === undefined ? {} : { key }),
    ...(onEvent === undefined ? {} : { onEvent }),
    ...(style === undefined ? {} : { style }),
    ...(a11y === undefined ? {} : { a11y }),
    ...(interactions === undefined ? {} : { interactions }),
    kind: "voice-input",
    props: VoiceInputHostPropsSchema.make(hostProps) as unknown as JsonPayload
  })
}

export interface OnDeviceModelHostProps {
  readonly modelId?: string
  readonly prompt?: string
  readonly status?: "idle" | "loading" | "ready" | "error"
}
export const OnDeviceModelHostPropsSchema: Schema.Codec<OnDeviceModelHostProps, OnDeviceModelHostProps> =
  Schema.Struct({
    modelId: Schema.String.pipe(Schema.optionalKey),
    prompt: Schema.String.pipe(Schema.optionalKey),
    status: Schema.Literals(["idle", "loading", "ready", "error"] as const).pipe(Schema.optionalKey)
  }) as unknown as Schema.Codec<OnDeviceModelHostProps, OnDeviceModelHostProps>
export const decodeOnDeviceModelHostProps = Schema.decodeUnknownSync(OnDeviceModelHostPropsSchema)

export type OnDeviceModelEvent =
  | { readonly type: "token"; readonly text: string }
  | { readonly type: "done"; readonly text: string }
  | { readonly type: "error"; readonly message: string }
export const OnDeviceModelEventSchema: Schema.Codec<OnDeviceModelEvent, OnDeviceModelEvent> = Schema.Union([
  Schema.Struct({ type: Schema.Literal("token"), text: Schema.String }),
  Schema.Struct({ type: Schema.Literal("done"), text: Schema.String }),
  Schema.Struct({ type: Schema.Literal("error"), message: Schema.String })
]) as unknown as Schema.Codec<OnDeviceModelEvent, OnDeviceModelEvent>

export interface OnDeviceModelProps extends OnDeviceModelHostProps {
  readonly key?: NodeKey
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
  readonly a11y?: A11y
  readonly interactions?: Interactions
}
export const OnDeviceModel = (props: OnDeviceModelProps): HostView => {
  const { key, onEvent, style, a11y, interactions, ...hostProps } = props
  return Host({
    ...(key === undefined ? {} : { key }),
    ...(onEvent === undefined ? {} : { onEvent }),
    ...(style === undefined ? {} : { style }),
    ...(a11y === undefined ? {} : { a11y }),
    ...(interactions === undefined ? {} : { interactions }),
    kind: "on-device-model",
    props: OnDeviceModelHostPropsSchema.make(hostProps) as unknown as JsonPayload
  })
}

// ── MediaVideo host contract (issue #67) ─────────────────────────────────────
//
// A live media element (WebRTC track, capture stream, or vendor-SDK attach)
// under the reviewed `Host(kind: "media-video")` escape hatch (#23) — not a new
// closed-catalog tag. The MediaStream itself is not serializable and never
// enters the tree: the app binds it to the driver-owned <video> element at the
// renderer boundary (DOM: `makeMediaVideoDriver({ onElement })`). Props stay
// bounded and serializable; playback sources (src URLs, HLS, posters) are
// explicitly out of scope — this host is a live attach target only.
export const mediaVideoFits = ["cover", "contain"] as const
export type MediaVideoFit = (typeof mediaVideoFits)[number]

export interface MediaVideoHostProps {
  readonly fit?: MediaVideoFit
  readonly muted?: boolean
  readonly mirrored?: boolean
}
export const MediaVideoHostPropsSchema: Schema.Codec<MediaVideoHostProps, MediaVideoHostProps> = Schema.Struct({
  fit: Schema.Literals(mediaVideoFits).pipe(Schema.optionalKey),
  muted: Schema.Boolean.pipe(Schema.optionalKey),
  mirrored: Schema.Boolean.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<MediaVideoHostProps, MediaVideoHostProps>
export const decodeMediaVideoHostProps = Schema.decodeUnknownSync(MediaVideoHostPropsSchema)

export type MediaVideoEvent =
  | { readonly type: "ready" }
  | { readonly type: "ended" }
  | { readonly type: "error"; readonly message: string }
export const MediaVideoEventSchema: Schema.Codec<MediaVideoEvent, MediaVideoEvent> = Schema.Union([
  Schema.Struct({ type: Schema.Literal("ready") }),
  Schema.Struct({ type: Schema.Literal("ended") }),
  Schema.Struct({ type: Schema.Literal("error"), message: Schema.String })
]) as unknown as Schema.Codec<MediaVideoEvent, MediaVideoEvent>

export interface MediaVideoProps extends MediaVideoHostProps {
  readonly key?: NodeKey
  readonly onEvent?: IntentRef
  readonly style?: CardStyle
  readonly a11y?: A11y
  readonly interactions?: Interactions
}
export const MediaVideo = (props: MediaVideoProps): HostView => {
  const { key, onEvent, style, a11y, interactions, ...hostProps } = props
  return Host({
    ...(key === undefined ? {} : { key }),
    ...(onEvent === undefined ? {} : { onEvent }),
    ...(style === undefined ? {} : { style }),
    ...(a11y === undefined ? {} : { a11y }),
    ...(interactions === undefined ? {} : { interactions }),
    kind: "media-video",
    props: MediaVideoHostPropsSchema.make(hostProps) as unknown as JsonPayload
  })
}



export type IconProps = WithoutTagAndVersion<IconView>
export const Icon = (props: IconProps): IconView =>
  IconSchema.make({ _tag: "Icon", catalogVersion: CatalogVersion, ...props })

export type DividerProps = WithoutTagAndVersion<DividerView>
export const Divider = (props: DividerProps = {}): DividerView =>
  DividerSchema.make({ _tag: "Divider", catalogVersion: CatalogVersion, ...props })

export type BadgeProps = WithoutTagAndVersion<BadgeView>
export const Badge = (props: BadgeProps): BadgeView =>
  BadgeSchema.make({ _tag: "Badge", catalogVersion: CatalogVersion, ...props })

export type ChipProps = WithoutTagAndVersion<ChipView>
export const Chip = (props: ChipProps): ChipView =>
  ChipSchema.make({ _tag: "Chip", catalogVersion: CatalogVersion, ...props })

export type MeterProps = WithoutTagAndVersion<MeterView>
export const Meter = (props: MeterProps): MeterView =>
  MeterSchema.make({ _tag: "Meter", catalogVersion: CatalogVersion, ...props })

export type StatTileProps = WithoutTagAndVersion<StatTileView>
export const StatTile = (props: StatTileProps): StatTileView =>
  StatTileSchema.make({ _tag: "StatTile", catalogVersion: CatalogVersion, ...props })

export type TableProps = WithoutTagAndVersion<TableView>
export const Table = (props: TableProps): TableView =>
  TableSchema.make({ _tag: "Table", catalogVersion: CatalogVersion, ...props })

export type SplitPaneProps = WithoutTagAndVersion<SplitPaneView>
export const SplitPane = (props: SplitPaneProps): SplitPaneView =>
  SplitPaneSchema.make({ _tag: "SplitPane", catalogVersion: CatalogVersion, ...props })

export type NavRailProps = WithoutTagAndVersion<NavRailView>
export const NavRail = (props: NavRailProps): NavRailView =>
  NavRailSchema.make({ _tag: "NavRail", catalogVersion: CatalogVersion, ...props })

export type WorkbenchProps = WithoutTagAndVersion<WorkbenchView>
export const Workbench = (props: WorkbenchProps): WorkbenchView =>
  WorkbenchSchema.make({ _tag: "Workbench", catalogVersion: CatalogVersion, ...props })

export type PopoverProps = Omit<WithoutTagAndVersion<PopoverView>, "children">
export const Popover = (props: PopoverProps, children: ReadonlyArray<View> = []): PopoverView =>
  PopoverSchema.make({ _tag: "Popover", catalogVersion: CatalogVersion, ...props, children })

export type DropdownMenuProps = WithoutTagAndVersion<DropdownMenuView>
export const DropdownMenu = (props: DropdownMenuProps): DropdownMenuView =>
  DropdownMenuSchema.make({ _tag: "DropdownMenu", catalogVersion: CatalogVersion, ...props })

export type ContextMenuProps = WithoutTagAndVersion<ContextMenuView>
export const ContextMenu = (props: ContextMenuProps): ContextMenuView =>
  ContextMenuSchema.make({ _tag: "ContextMenu", catalogVersion: CatalogVersion, ...props })

export type TooltipProps = Omit<WithoutTagAndVersion<TooltipView>, "children">
export const Tooltip = (props: TooltipProps, children: ReadonlyArray<View>): TooltipView =>
  TooltipSchema.make({ _tag: "Tooltip", catalogVersion: CatalogVersion, ...props, children })

export type ComboboxProps = WithoutTagAndVersion<ComboboxView>
export const Combobox = (props: ComboboxProps): ComboboxView =>
  ComboboxSchema.make({ _tag: "Combobox", catalogVersion: CatalogVersion, ...props })

export type CommandPaletteProps = WithoutTagAndVersion<CommandPaletteView>
export const CommandPalette = (props: CommandPaletteProps): CommandPaletteView =>
  CommandPaletteSchema.make({ _tag: "CommandPalette", catalogVersion: CatalogVersion, ...props })

export type TabsProps = WithoutTagAndVersion<TabsView>
export const Tabs = (props: TabsProps): TabsView =>
  TabsSchema.make({ _tag: "Tabs", catalogVersion: CatalogVersion, ...props })

export type ComposerProps = WithoutTagAndVersion<ComposerView>
export const Composer = (props: ComposerProps): ComposerView =>
  ComposerSchema.make({ _tag: "Composer", catalogVersion: CatalogVersion, ...props })

export type ToggleProps = WithoutTagAndVersion<ToggleView>
export const Toggle = (props: ToggleProps): ToggleView =>
  ToggleSchema.make({ _tag: "Toggle", catalogVersion: CatalogVersion, ...props })

export type SelectProps = WithoutTagAndVersion<SelectView>
export const Select = (props: SelectProps): SelectView =>
  SelectSchema.make({ _tag: "Select", catalogVersion: CatalogVersion, ...props })

export type CheckboxProps = WithoutTagAndVersion<CheckboxView>
export const Checkbox = (props: CheckboxProps): CheckboxView =>
  CheckboxSchema.make({ _tag: "Checkbox", catalogVersion: CatalogVersion, ...props })

export type RadioGroupProps = WithoutTagAndVersion<RadioGroupView>
export const RadioGroup = (props: RadioGroupProps): RadioGroupView =>
  RadioGroupSchema.make({ _tag: "RadioGroup", catalogVersion: CatalogVersion, ...props })

export type SliderProps = WithoutTagAndVersion<SliderView>
export const Slider = (props: SliderProps): SliderView =>
  SliderSchema.make({ _tag: "Slider", catalogVersion: CatalogVersion, ...props })

export type NumberFieldProps = WithoutTagAndVersion<NumberFieldView>
export const NumberField = (props: NumberFieldProps): NumberFieldView =>
  NumberFieldSchema.make({ _tag: "NumberField", catalogVersion: CatalogVersion, ...props })

export type FieldRowProps = WithoutTagAndVersion<FieldRowView>
export const FieldRow = (props: FieldRowProps): FieldRowView =>
  FieldRowSchema.make({ _tag: "FieldRow", catalogVersion: CatalogVersion, ...props })

export type ToastProps = WithoutTagAndVersion<ToastView>
export const Toast = (props: ToastProps): ToastView =>
  ToastSchema.make({ _tag: "Toast", catalogVersion: CatalogVersion, ...props })

export type ToastRegionProps = WithoutTagAndVersion<ToastRegionView>
export const ToastRegion = (props: ToastRegionProps): ToastRegionView =>
  ToastRegionSchema.make({ _tag: "ToastRegion", catalogVersion: CatalogVersion, ...props })

export type StatusBannerProps = WithoutTagAndVersion<StatusBannerView>
export const StatusBanner = (props: StatusBannerProps): StatusBannerView =>
  StatusBannerSchema.make({ _tag: "StatusBanner", catalogVersion: CatalogVersion, ...props })

export type RecoveryOverlayProps = WithoutTagAndVersion<RecoveryOverlayView>
export const RecoveryOverlay = (props: RecoveryOverlayProps): RecoveryOverlayView =>
  RecoveryOverlaySchema.make({ _tag: "RecoveryOverlay", catalogVersion: CatalogVersion, ...props })

export type MarkdownProps = WithoutTagAndVersion<MarkdownView>
export const Markdown = (props: MarkdownProps): MarkdownView =>
  MarkdownSchema.make({ _tag: "Markdown", catalogVersion: CatalogVersion, ...props })

export type TranscriptProps = WithoutTagAndVersion<TranscriptView>
export const Transcript = (props: TranscriptProps): TranscriptView =>
  TranscriptSchema.make({ _tag: "Transcript", catalogVersion: CatalogVersion, ...props })

export type CodeBlockProps = WithoutTagAndVersion<CodeBlockView>
export const CodeBlock = (props: CodeBlockProps): CodeBlockView =>
  CodeBlockSchema.make({ _tag: "CodeBlock", catalogVersion: CatalogVersion, ...props })

export type DiffViewProps = WithoutTagAndVersion<DiffViewView>
export const DiffView = (props: DiffViewProps): DiffViewView =>
  DiffViewSchema.make({ _tag: "DiffView", catalogVersion: CatalogVersion, ...props })

export type GraphFigureProps = WithoutTagAndVersion<GraphFigureView>
export const GraphFigure = (props: GraphFigureProps): GraphFigureView =>
  GraphFigureSchema.make({ _tag: "GraphFigure", catalogVersion: CatalogVersion, ...props })

export type TimelineProps = WithoutTagAndVersion<TimelineView>
export const Timeline = (props: TimelineProps): TimelineView =>
  TimelineSchema.make({ _tag: "Timeline", catalogVersion: CatalogVersion, ...props })

export type SectionProps = Omit<WithoutTagAndVersion<SectionView>, "children">
export const Section = (props: SectionProps, children: ReadonlyArray<View> = []): SectionView =>
  SectionSchema.make({ _tag: "Section", catalogVersion: CatalogVersion, ...props, children })

export type HeroProps = Omit<WithoutTagAndVersion<HeroView>, "actions" | "media">
export const Hero = (
  props: HeroProps & { readonly actions?: ReadonlyArray<View>; readonly media?: View }
): HeroView =>
  HeroSchema.make({
    _tag: "Hero",
    catalogVersion: CatalogVersion,
    actions: props.actions ?? [],
    ...props
  })

export type AnnouncementBadgeProps = WithoutTagAndVersion<AnnouncementBadgeView>
export const AnnouncementBadge = (props: AnnouncementBadgeProps): AnnouncementBadgeView =>
  AnnouncementBadgeSchema.make({ _tag: "AnnouncementBadge", catalogVersion: CatalogVersion, ...props })

export type CtaSectionProps = Omit<WithoutTagAndVersion<CtaSectionView>, "actions">
export const CtaSection = (
  props: CtaSectionProps & { readonly actions?: ReadonlyArray<View> }
): CtaSectionView =>
  CtaSectionSchema.make({
    _tag: "CtaSection",
    catalogVersion: CatalogVersion,
    actions: props.actions ?? [],
    ...props
  })

export type FooterProps = WithoutTagAndVersion<FooterView>
export const Footer = (props: FooterProps): FooterView =>
  FooterSchema.make({ _tag: "Footer", catalogVersion: CatalogVersion, ...props })

export type NavBarProps = WithoutTagAndVersion<NavBarView>
export const NavBar = (props: NavBarProps): NavBarView =>
  NavBarSchema.make({ _tag: "NavBar", catalogVersion: CatalogVersion, ...props })

export type AccordionProps = WithoutTagAndVersion<AccordionView>
export const Accordion = (props: AccordionProps): AccordionView =>
  AccordionSchema.make({ _tag: "Accordion", catalogVersion: CatalogVersion, ...props })

export type PricingColumnProps = WithoutTagAndVersion<PricingColumnView>
export const PricingColumn = (props: PricingColumnProps): PricingColumnView =>
  PricingColumnSchema.make({ _tag: "PricingColumn", catalogVersion: CatalogVersion, ...props })

export type PricingTableProps = WithoutTagAndVersion<PricingTableView>
export const PricingTable = (props: PricingTableProps): PricingTableView =>
  PricingTableSchema.make({ _tag: "PricingTable", catalogVersion: CatalogVersion, ...props })

export type LogoRowProps = WithoutTagAndVersion<LogoRowView>
export const LogoRow = (props: LogoRowProps): LogoRowView =>
  LogoRowSchema.make({ _tag: "LogoRow", catalogVersion: CatalogVersion, ...props })

export type StatsBandProps = WithoutTagAndVersion<StatsBandView>
export const StatsBand = (props: StatsBandProps): StatsBandView =>
  StatsBandSchema.make({ _tag: "StatsBand", catalogVersion: CatalogVersion, ...props })

export type GlowProps = Omit<WithoutTagAndVersion<GlowView>, "children">
export const Glow = (props: GlowProps, children: ReadonlyArray<View> = []): GlowView =>
  GlowSchema.make({ _tag: "Glow", catalogVersion: CatalogVersion, ...props, children })

export type MockupFrameProps = Omit<WithoutTagAndVersion<MockupFrameView>, "children">
export const MockupFrame = (
  props: MockupFrameProps,
  children: ReadonlyArray<View> = []
): MockupFrameView =>
  MockupFrameSchema.make({ _tag: "MockupFrame", catalogVersion: CatalogVersion, ...props, children })

export type PagerProps = WithoutTagAndVersion<PagerView>
export const Pager = (props: PagerProps): PagerView =>
  PagerSchema.make({ _tag: "Pager", catalogVersion: CatalogVersion, ...props })

export type SwipeableListItemProps = WithoutTagAndVersion<SwipeableListItemView>
export const SwipeableListItem = (props: SwipeableListItemProps): SwipeableListItemView =>
  SwipeableListItemSchema.make({ _tag: "SwipeableListItem", catalogVersion: CatalogVersion, ...props })

export type BackgroundGradientProps = Omit<WithoutTagAndVersion<BackgroundGradientView>, "children">
export const BackgroundGradient = (
  props: BackgroundGradientProps,
  children: ReadonlyArray<View> = []
): BackgroundGradientView =>
  BackgroundGradientSchema.make({ _tag: "BackgroundGradient", catalogVersion: CatalogVersion, ...props, children })

export type WallpaperProps = Omit<WithoutTagAndVersion<WallpaperView>, "children">
export const Wallpaper = (
  props: WallpaperProps,
  children: ReadonlyArray<View> = []
): WallpaperView =>
  WallpaperSchema.make({ _tag: "Wallpaper", catalogVersion: CatalogVersion, ...props, children })

export type SpotlightProps = Omit<WithoutTagAndVersion<SpotlightView>, "children">
export const Spotlight = (
  props: SpotlightProps,
  children: ReadonlyArray<View> = []
): SpotlightView =>
  SpotlightSchema.make({ _tag: "Spotlight", catalogVersion: CatalogVersion, ...props, children })

export type FrameProps = Omit<WithoutTagAndVersion<FrameView>, "children">
export const Frame = (
  props: FrameProps,
  children: ReadonlyArray<View> = []
): FrameView =>
  FrameSchema.make({ _tag: "Frame", catalogVersion: CatalogVersion, ...props, children })

export type BlurredPopupProps = Omit<WithoutTagAndVersion<BlurredPopupView>, "children">
export const BlurredPopup = (
  props: BlurredPopupProps,
  children: ReadonlyArray<View> = []
): BlurredPopupView =>
  BlurredPopupSchema.make({ _tag: "BlurredPopup", catalogVersion: CatalogVersion, ...props, children })





// Deterministic 2D layout for a graph figure: precomputed positions when given,
// otherwise a bounded named layout (a stable circle for "force", a simple
// left-to-right tree by insertion order for "tree"). Renderers and the canvas
// adapter share this so the DOM/SVG fallback and the canvas path agree.
export const layoutGraphNodes = (
  view: GraphFigureView
): ReadonlyMap<string, { readonly x: number; readonly y: number }> => {
  const layout = view.layout ?? "precomputed"
  const positions = new Map<string, { readonly x: number; readonly y: number }>()
  if (layout === "precomputed") {
    view.nodes.forEach((node, index) => {
      positions.set(node.id, { x: node.x ?? index * 100, y: node.y ?? 0 })
    })
    return positions
  }
  if (layout === "tree") {
    view.nodes.forEach((node, index) => {
      positions.set(node.id, { x: index * 120, y: (index % 2) * 80 })
    })
    return positions
  }
  // "force" -> a stable circle so the layout is deterministic + snapshot-safe.
  const count = Math.max(1, view.nodes.length)
  const radius = 120
  view.nodes.forEach((node, index) => {
    const angle = (2 * Math.PI * index) / count
    positions.set(node.id, { x: Math.round(radius * Math.cos(angle)), y: Math.round(radius * Math.sin(angle)) })
  })
  return positions
}

// Plaintext value of a pre-tokenized code block, for the copy affordance.
export const codeBlockPlainText = (lines: ReadonlyArray<CodeLine>): string =>
  lines.map((line) => line.tokens.map((token) => token.text).join("")).join("\n")

// Normalize a composer document to its plaintext value (text runs verbatim,
// mentions rendered as their label). This is the value renderers emit on change
// and the app can re-parse to a typed document.
export const composerPlainText = (doc: ReadonlyArray<ComposerInline>): string =>
  doc.map((node) => (node.kind === "text" ? node.text : node.label)).join("")

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

const resolveBoundBoolean = (value: Bound<boolean>, state: unknown): boolean => {
  if (!isBinding(value)) {
    return value
  }
  return readStatePath(state, value.path) === true
}

export interface ViewResolution {
  readonly state?: unknown
  readonly viewport?: Viewport
  readonly platform?: PlatformVariant
}

const styleResolution = (input: ViewResolution): StyleResolution => ({
  ...(input.platform === undefined ? {} : { platform: input.platform }),
  ...(input.viewport === undefined ? {} : { breakpoint: input.viewport.breakpoint })
})

export const resolveView = (view: View, input: ViewResolution = {}): View => {
  const resolution = styleResolution(input)
  switch (view._tag) {
    case "Stack":
      return {
        ...view,
        direction: resolveResponsiveValue(view.direction, input.viewport),
        ...(view.gap === undefined ? {} : { gap: resolveResponsiveValue(view.gap, input.viewport) }),
        ...(view.padding === undefined ? {} : { padding: resolveResponsiveValue(view.padding, input.viewport) }),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "Text":
      return {
        ...view,
        content: input.state === undefined ? view.content : resolveBoundText(view.content, input.state),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "Button":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "Image":
      return {
        ...view,
        ...(view.width === undefined ? {} : { width: resolveResponsiveValue(view.width, input.viewport) }),
        ...(view.height === undefined ? {} : { height: resolveResponsiveValue(view.height, input.viewport) }),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "TextField":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "List":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        items: view.items.map((item) => resolveView(item, input) as KeyedView)
      }
    case "SectionList":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        sections: view.sections.map((section) => ({
          ...section,
          header: resolveView(section.header, input),
          items: section.items.map((item) => resolveView(item, input) as KeyedView)
        }))
      }
    case "Card":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "Spacer":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "Link":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        children: view.children.map((child) => resolveView(child, input) as LinkChildView)
      }
    case "Modal":
      return {
        ...view,
        title: input.state === undefined ? view.title : resolveBoundText(view.title, input.state),
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "Sheet":
      return {
        ...view,
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "Host":
    case "Icon":
    case "Divider":
    case "Badge":
    case "Chip":
    case "Meter":
    case "StatTile":
    case "NavRail":
    case "Combobox":
    case "Toggle":
    case "Select":
    case "Checkbox":
    case "RadioGroup":
    case "Slider":
    case "NumberField":
    case "Toast":
    case "ToastRegion":
    case "StatusBanner":
    case "Markdown":
    case "CodeBlock":
    case "DiffView":
    case "GraphFigure":
    case "Timeline":
    case "AnnouncementBadge":
    case "LogoRow":
    case "PricingColumn":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "Section":
    case "Glow":
    case "MockupFrame":
    case "BackgroundGradient":
    case "Wallpaper":
    case "Spotlight":
    case "Frame":
    case "BlurredPopup":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "Hero":
      return {
        ...view,
        headline: input.state === undefined ? view.headline : resolveBoundText(view.headline, input.state),
        ...(view.subhead === undefined
          ? {}
          : {
              subhead:
                input.state === undefined
                  ? view.subhead
                  : resolveBoundText(view.subhead, input.state)
            }),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        actions: view.actions.map((child) => resolveView(child, input)),
        ...(view.media === undefined ? {} : { media: resolveView(view.media, input) })
      }
    case "CtaSection":
      return {
        ...view,
        headline: input.state === undefined ? view.headline : resolveBoundText(view.headline, input.state),
        ...(view.body === undefined
          ? {}
          : {
              body:
                input.state === undefined ? view.body : resolveBoundText(view.body, input.state)
            }),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        actions: view.actions.map((child) => resolveView(child, input))
      }
    case "Footer":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        ...(view.brand === undefined ? {} : { brand: resolveView(view.brand, input) }),
        ...(view.legal === undefined ? {} : { legal: resolveView(view.legal, input) }),
        columns: view.columns.map((column) => ({
          ...column,
          links: column.links.map((child) => resolveView(child, input))
        }))
      }
    case "NavBar":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        brand: resolveView(view.brand, input),
        ...(view.actions === undefined
          ? {}
          : { actions: view.actions.map((child) => resolveView(child, input)) })
      }
    case "Accordion":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        items: view.items.map((item) => ({
          ...item,
          content: item.content.map((child) => resolveView(child, input))
        }))
      }
    case "PricingTable":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        columns: view.columns.map((column) => resolveView(column, input) as PricingColumnView)
      }
    case "StatsBand":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        stats: view.stats.map((stat) => ({
          ...stat,
          value: input.state === undefined ? stat.value : resolveBoundText(stat.value, input.state)
        }))
      }
    case "RecoveryOverlay":
      return {
        ...view,
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state)
      }
    case "Transcript":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        messages: view.messages.map((message) => ({
          ...message,
          body: message.body.map((child) => resolveView(child, input))
        }))
      }
    case "FieldRow":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        control: resolveView(view.control, input)
      }
    case "Table":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        rows: view.rows.map((row) => ({
          ...row,
          cells: row.cells.map((cell) => resolveView(cell, input))
        }))
      }
    case "SplitPane":
    case "Workbench":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        panes: view.panes.map((pane) => ({ ...pane, content: resolveView(pane.content, input) }))
      }
    case "Popover":
      return {
        ...view,
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state),
        children: view.children.map((child) => resolveView(child, input))
      }
    case "DropdownMenu":
    case "ContextMenu":
      return {
        ...view,
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state),
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) })
      }
    case "Tooltip":
      return {
        ...view,
        children: view.children.map((child) => resolveView(child, input))
      }
    case "CommandPalette":
      return {
        ...view,
        open: input.state === undefined ? view.open : resolveBoundBoolean(view.open, input.state),
        combobox: resolveView(view.combobox, input) as ComboboxView
      }
    case "Tabs":
    case "Pager":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        panels: view.panels.map((panel) => ({ ...panel, content: resolveView(panel.content, input) }))
      }
    case "SwipeableListItem":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        child: resolveView(view.child, input)
      }
    case "Composer":
      return {
        ...view,
        ...(view.style === undefined ? {} : { style: resolveStyle(view.style, resolution) }),
        ...(view.autocomplete === undefined
          ? {}
          : { autocomplete: { ...view.autocomplete, combobox: resolveView(view.autocomplete.combobox, input) as ComboboxView } })
      }
  }
}

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
    case "Image":
    case "TextField":
    case "Spacer":
    case "Host":
    case "Icon":
    case "Divider":
    case "Badge":
    case "Chip":
    case "Meter":
    case "StatTile":
    case "NavRail":
    case "Combobox":
    case "Toggle":
    case "Select":
    case "Checkbox":
    case "RadioGroup":
    case "Slider":
    case "NumberField":
    case "Toast":
    case "ToastRegion":
    case "StatusBanner":
    case "Markdown":
    case "CodeBlock":
    case "DiffView":
    case "GraphFigure":
    case "Timeline":
    case "AnnouncementBadge":
    case "LogoRow":
    case "PricingColumn":
      return view
    case "Section":
    case "Glow":
    case "MockupFrame":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Hero":
      return {
        ...view,
        headline: resolveBoundText(view.headline, state),
        ...(view.subhead === undefined
          ? {}
          : { subhead: resolveBoundText(view.subhead, state) }),
        actions: view.actions.map((child) => resolveBindings(child, state)),
        ...(view.media === undefined ? {} : { media: resolveBindings(view.media, state) })
      }
    case "CtaSection":
      return {
        ...view,
        headline: resolveBoundText(view.headline, state),
        ...(view.body === undefined ? {} : { body: resolveBoundText(view.body, state) }),
        actions: view.actions.map((child) => resolveBindings(child, state))
      }
    case "Footer":
      return {
        ...view,
        ...(view.brand === undefined ? {} : { brand: resolveBindings(view.brand, state) }),
        ...(view.legal === undefined ? {} : { legal: resolveBindings(view.legal, state) }),
        columns: view.columns.map((column) => ({
          ...column,
          links: column.links.map((child) => resolveBindings(child, state))
        }))
      }
    case "NavBar":
      return {
        ...view,
        brand: resolveBindings(view.brand, state),
        ...(view.actions === undefined
          ? {}
          : { actions: view.actions.map((child) => resolveBindings(child, state)) })
      }
    case "Accordion":
      return {
        ...view,
        items: view.items.map((item) => ({
          ...item,
          content: item.content.map((child) => resolveBindings(child, state))
        }))
      }
    case "PricingTable":
      return {
        ...view,
        columns: view.columns.map(
          (column) => resolveBindings(column, state) as PricingColumnView
        )
      }
    case "StatsBand":
      return {
        ...view,
        stats: view.stats.map((stat) => ({
          ...stat,
          value: resolveBoundText(stat.value, state)
        }))
      }
    case "RecoveryOverlay":
      return {
        ...view,
        open: resolveBoundBoolean(view.open, state)
      }
    case "Transcript":
      return {
        ...view,
        messages: view.messages.map((message) => ({
          ...message,
          body: message.body.map((child) => resolveBindings(child, state))
        }))
      }
    case "FieldRow":
      return {
        ...view,
        control: resolveBindings(view.control, state)
      }
    case "Table":
      return {
        ...view,
        rows: view.rows.map((row) => ({
          ...row,
          cells: row.cells.map((cell) => resolveBindings(cell, state))
        }))
      }
    case "SplitPane":
    case "Workbench":
      return {
        ...view,
        panes: view.panes.map((pane) => ({ ...pane, content: resolveBindings(pane.content, state) }))
      }
    case "Popover":
      return {
        ...view,
        open: resolveBoundBoolean(view.open, state),
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "DropdownMenu":
    case "ContextMenu":
      return {
        ...view,
        open: resolveBoundBoolean(view.open, state)
      }
    case "Tooltip":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "CommandPalette":
      return {
        ...view,
        open: resolveBoundBoolean(view.open, state),
        combobox: resolveBindings(view.combobox, state) as ComboboxView
      }
    case "Tabs":
    case "Pager":
      return {
        ...view,
        panels: view.panels.map((panel) => ({ ...panel, content: resolveBindings(panel.content, state) }))
      }
    case "SwipeableListItem":
      return {
        ...view,
        child: resolveBindings(view.child, state)
      }
    case "BackgroundGradient":
    case "Wallpaper":
    case "Spotlight":
    case "Frame":
    case "BlurredPopup":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Composer":
      return view.autocomplete === undefined
        ? view
        : {
            ...view,
            autocomplete: { ...view.autocomplete, combobox: resolveBindings(view.autocomplete.combobox, state) as ComboboxView }
          }
    case "List":
      return {
        ...view,
        items: view.items.map((item) => resolveBindings(item, state) as KeyedView)
      }
    case "SectionList":
      return {
        ...view,
        sections: view.sections.map((section) => ({
          ...section,
          header: resolveBindings(section.header, state),
          items: section.items.map((item) => resolveBindings(item, state) as KeyedView)
        }))
      }
    case "Card":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Link":
      return {
        ...view,
        children: view.children.map((child) => resolveBindings(child, state) as LinkChildView)
      }
    case "Modal":
      return {
        ...view,
        title: resolveBoundText(view.title, state),
        open: resolveBoundBoolean(view.open, state),
        children: view.children.map((child) => resolveBindings(child, state))
      }
    case "Sheet":
      return {
        ...view,
        open: resolveBoundBoolean(view.open, state),
        children: view.children.map((child) => resolveBindings(child, state))
      }
  }
}

export const redactSecureView = (view: View): View => {
  switch (view._tag) {
    case "Stack":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "List":
      return {
        ...view,
        items: view.items.map((item) => redactSecureView(item) as KeyedView)
      }
    case "SectionList":
      return {
        ...view,
        sections: view.sections.map((section) => ({
          ...section,
          header: redactSecureView(section.header),
          items: section.items.map((item) => redactSecureView(item) as KeyedView)
        }))
      }
    case "Card":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "Link":
      return {
        ...view,
        children: view.children.map((child) => redactSecureView(child) as LinkChildView)
      }
    case "Modal":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "Sheet":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "TextField":
      return view.secure === true
        ? {
            ...view,
            value: redactedValue
          }
        : view
    case "Table":
      return {
        ...view,
        rows: view.rows.map((row) => ({
          ...row,
          cells: row.cells.map(redactSecureView)
        }))
      }
    case "SplitPane":
    case "Workbench":
      return {
        ...view,
        panes: view.panes.map((pane) => ({ ...pane, content: redactSecureView(pane.content) }))
      }
    case "Popover":
    case "Tooltip":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "Tabs":
    case "Pager":
      return {
        ...view,
        panels: view.panels.map((panel) => ({ ...panel, content: redactSecureView(panel.content) }))
      }
    case "SwipeableListItem":
      return {
        ...view,
        child: redactSecureView(view.child)
      }
    case "BackgroundGradient":
    case "Wallpaper":
    case "Spotlight":
    case "Frame":
    case "BlurredPopup":
      return {
        ...view,
        children: view.children.map(redactSecureView)
      }
    case "FieldRow":
      return {
        ...view,
        control: redactSecureView(view.control)
      }
    case "Transcript":
      return {
        ...view,
        messages: view.messages.map((message) => ({
          ...message,
          body: message.body.map(redactSecureView)
        }))
      }
    default:
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

export interface ViewProgramOptions<State> {
  readonly now?: () => number
  readonly devtoolsSink?: DevtoolsSink
  readonly redactState?: (state: State) => JsonPayload
}

const jsonPayloadOrNull = (value: unknown): JsonPayload => {
  const decoded = Schema.decodeUnknownExit(JsonPayloadSchema)(value)
  return Exit.isSuccess(decoded) ? decoded.value : null
}

export const makeViewProgramFromState = <State>(
  state: SubscriptionRef.SubscriptionRef<State>,
  render: (state: State) => View,
  options: ViewProgramOptions<State> = {}
): ViewProgram<State> => {
  const devtoolsSink = options.devtoolsSink
  const now = options.now ?? Date.now
  const redactState = options.redactState ?? jsonPayloadOrNull

  return {
    state,
    render,
    viewStream: SubscriptionRef.changes(state).pipe(
      Stream.map((value) => {
        const resolved = resolveBindings(render(value), value)
        if (devtoolsSink !== undefined) {
          const timestamp = now()
          devtoolsSink.emit({
            _tag: "StateSnapshot",
            timestamp,
            state: redactState(value)
          })
          devtoolsSink.emit({
            _tag: "ViewEmitted",
            timestamp,
            view: redactSecureView(resolved)
          })
        }
        return resolved
      })
    ),
    currentState: SubscriptionRef.get(state),
    setState: (value) => SubscriptionRef.set(state, value),
    updateState: (f) => SubscriptionRef.update(state, f),
    report: (ref, runtimeValue = null) => dispatchIntent(resolveIntentRef(ref, runtimeValue))
  }
}

export const makeViewProgram = <State>(
  initialState: State,
  render: (state: State) => View,
  options?: ViewProgramOptions<State>
): Effect.Effect<ViewProgram<State>> =>
  Effect.gen(function*() {
    const state = yield* SubscriptionRef.make(initialState)
    return makeViewProgramFromState(state, render, options)
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

export interface HeadlessRendererOptions {
  readonly viewport?: ViewportInput
  readonly theme?: Theme
  readonly platform?: PlatformVariant
}

export interface HeadlessSurface extends MountedSurface {
  readonly snapshots: Effect.Effect<ReadonlyArray<View>>
  readonly current: Effect.Effect<View | undefined>
  readonly currentViewport: Effect.Effect<Viewport>
  readonly setViewport: (input: ViewportInput) => Effect.Effect<void>
  readonly simulate: (ref: IntentRef, runtimeValue?: JsonPayload) => Effect.Effect<void, IntentError, IntentRegistry>
}

export const makeHeadlessRenderer = (
  options: HeadlessRendererOptions = {}
): RendererAdapter<HeadlessContainer | undefined, HeadlessSurface> => ({
  mount: (container, viewStream, report) =>
    Effect.gen(function*() {
      const parentScope = yield* Scope.Scope
      const surfaceScope = yield* Scope.fork(parentScope)

      return yield* Scope.provide(surfaceScope)(Effect.gen(function*() {
        const snapshots = yield* Ref.make<ReadonlyArray<View>>([])
        const viewport = yield* makeViewportService(
          options.viewport ?? defaultViewportInput,
          options.theme === undefined ? {} : { theme: options.theme }
        )
        const ready = yield* Deferred.make<void>()
        const resolvedViewStream = viewStream.pipe(
          Stream.zipLatestWith(viewport.stream, (view, currentViewport) =>
            resolveView(view, {
              viewport: currentViewport,
              ...(options.platform === undefined ? {} : { platform: options.platform })
            })
          )
        )

        yield* Effect.addFinalizer(() =>
          container?.onFinalize === undefined
            ? Effect.succeed(undefined)
            : container.onFinalize
        )

        yield* resolvedViewStream.pipe(
          Stream.runForEach((view) =>
            Effect.gen(function*() {
              yield* Ref.update(snapshots, (views) => [...views, redactSecureView(view)])
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
          currentViewport: viewport.current,
          setViewport: viewport.set,
          simulate: (ref: IntentRef, runtimeValue: JsonPayload = null) =>
            report(ref, runtimeValue).pipe(Effect.andThen(Effect.yieldNow))
        }
      }))
    })
})

// ── Streaming / live data binding (issue #26) ────────────────────────────────
//
// A coding-agent desktop surface is fundamentally streaming: transcript items
// append token-by-token, counters tick, fleet/gym status updates continuously.
// This runtime binds an Effect `Stream` of typed patches to a keyed region and
// applies incremental updates:
//
//   - keyed reconciliation so appends are O(new), not O(all);
//   - coalescing to frame cadence via `Stream.groupedWithin` (not ad-hoc
//     throttling), so high-frequency streams collapse to one update per frame;
//   - Scope-based interruption/cleanup — closing the region's scope interrupts
//     the source stream and releases resources;
//   - a recorded patch sequence so streaming is snapshot/replay testable.
//
// This is the update mechanism beneath the view tree, not a transport layer:
// the app supplies the streams (desktop bridge, khala-sync, polling, etc.).

export interface KeyedItem<A> {
  readonly key: string
  readonly value: A
}

export type RegionPatch<A> =
  | { readonly _tag: "Append"; readonly items: ReadonlyArray<KeyedItem<A>> }
  | { readonly _tag: "Update"; readonly key: string; readonly value: A }
  | { readonly _tag: "Remove"; readonly key: string }
  | { readonly _tag: "Replace"; readonly items: ReadonlyArray<KeyedItem<A>> }

// Apply a single patch to a keyed list. Append is O(new); Update/Remove touch
// only the matching key; Replace resets the region. Appending an existing key
// updates in place so duplicate stream deliveries stay idempotent.
export const applyRegionPatch = <A>(
  current: ReadonlyArray<KeyedItem<A>>,
  patch: RegionPatch<A>
): ReadonlyArray<KeyedItem<A>> => {
  switch (patch._tag) {
    case "Append": {
      if (patch.items.length === 0) return current
      const index = new Map(current.map((item, position) => [item.key, position] as const))
      const next = current.slice()
      for (const item of patch.items) {
        const existing = index.get(item.key)
        if (existing === undefined) {
          index.set(item.key, next.length)
          next.push(item)
        } else {
          next[existing] = item
        }
      }
      return next
    }
    case "Update":
      return current.map((item) => (item.key === patch.key ? { key: item.key, value: patch.value } : item))
    case "Remove":
      return current.filter((item) => item.key !== patch.key)
    case "Replace":
      return patch.items.slice()
  }
}

export interface StreamRegionOptions<A> {
  // Coalescing window; buffered patches within one window are applied together
  // in a single region update. Defaults to 16ms (~one animation frame).
  readonly frameMillis?: number
  // Seed items for the region before any patch arrives.
  readonly initial?: ReadonlyArray<KeyedItem<A>>
  // Optional monotonic clock for deterministic tests.
  readonly recordPatches?: boolean
}

export interface StreamRegion<A> {
  readonly items: SubscriptionRef.SubscriptionRef<ReadonlyArray<KeyedItem<A>>>
  readonly changes: Stream.Stream<ReadonlyArray<KeyedItem<A>>>
  // The recorded, applied patch sequence (in order) for snapshot/replay tests.
  readonly patches: Effect.Effect<ReadonlyArray<RegionPatch<A>>>
  // Number of coalesced frames committed (one per non-empty window).
  readonly frames: Effect.Effect<number>
}

// Bind a `Stream` of region patches to a keyed region. The consumer fiber is
// forked into the current `Scope`, so closing that scope interrupts the source
// stream and releases resources.
export const makeStreamRegion = <A, E, R>(
  source: Stream.Stream<RegionPatch<A>, E, R>,
  options: StreamRegionOptions<A> = {}
): Effect.Effect<StreamRegion<A>, never, R | Scope.Scope> =>
  Effect.gen(function*() {
    const frameMillis = options.frameMillis ?? 16
    const items = yield* SubscriptionRef.make<ReadonlyArray<KeyedItem<A>>>(options.initial ?? [])
    const recorded = yield* Ref.make<ReadonlyArray<RegionPatch<A>>>([])
    const frameCount = yield* Ref.make(0)
    const record = options.recordPatches !== false

    yield* source.pipe(
      // Coalesce every patch that arrives within the frame window into one
      // batch. `Number.MAX_SAFE_INTEGER` disables the size trigger so batching
      // is purely time-based (frame cadence).
      Stream.groupedWithin(Number.MAX_SAFE_INTEGER, Duration.millis(frameMillis)),
      Stream.runForEach((batch) =>
        batch.length === 0
          ? Effect.void
          : Effect.gen(function*() {
              yield* SubscriptionRef.update(items, (current) => {
                let next = current
                for (const patch of batch) {
                  next = applyRegionPatch(next, patch)
                }
                return next
              })
              if (record) {
                yield* Ref.update(recorded, (all) => [...all, ...batch])
              }
              yield* Ref.update(frameCount, (count) => count + 1)
            })
      ),
      Effect.forkScoped
    )

    return {
      items,
      changes: SubscriptionRef.changes(items),
      patches: Ref.get(recorded),
      frames: Ref.get(frameCount)
    }
  })

// ── Hotkey / keybinding registry + focus management (issue #41) ───────────────
//
// The app-wide counterpart to the per-node `onKey` intents from the interaction
// expansion (#24). A `Keymap` registers named commands (id, title, group,
// declarative enablement, optional keybinding) and resolves a pressed chord to a
// command within the active focus scope, then fires the command's typed intent —
// no `addEventListener` in app code. Scopes form a stack so an overlay scope can
// shadow a global binding; focus-return targets ride the same stack. Conflicts
// (two commands, same chord, same scope) are surfaced as typed diagnostics, not
// a silent last-wins. Keybinding labels are derived from the table, platform
// aware (⌘ vs Ctrl).

// A chord is not a serializable view node, so its key is the raw
// KeyboardEvent.key value (letters normalized to lower case) rather than the
// bounded navigation-only KeyName set used by node `onKey` bindings.
export interface KeyChord {
  readonly key: string
  readonly alt?: boolean
  readonly ctrl?: boolean
  readonly meta?: boolean
  readonly shift?: boolean
}

export const KeyChordSchema: Schema.Codec<KeyChord, KeyChord> = exactStruct({
  key: Schema.NonEmptyString,
  alt: Schema.Boolean.pipe(Schema.optionalKey),
  ctrl: Schema.Boolean.pipe(Schema.optionalKey),
  meta: Schema.Boolean.pipe(Schema.optionalKey),
  shift: Schema.Boolean.pipe(Schema.optionalKey)
}) as unknown as Schema.Codec<KeyChord, KeyChord>

export const defaultFocusScope = "global" as const

export interface CommandDefinition {
  readonly id: string
  readonly title: string
  readonly group?: string
  readonly intent: IntentRef
  readonly binding?: KeyChord
  // The focus scope this command belongs to; defaults to "global".
  readonly scope?: string
  // Declarative enablement: the command is enabled only when this context flag
  // is active (see Keymap.setContext). Omit for always-enabled.
  readonly when?: string
}

export interface KeymapConflict {
  readonly chord: KeyChord
  readonly scope: string
  readonly commandIds: ReadonlyArray<string>
}

export const normalizeChordKey = (key: string): string =>
  key.length === 1 ? key.toLowerCase() : key

export const chordEquals = (a: KeyChord, b: KeyChord): boolean =>
  normalizeChordKey(a.key) === normalizeChordKey(b.key) &&
  (a.alt === true) === (b.alt === true) &&
  (a.ctrl === true) === (b.ctrl === true) &&
  (a.meta === true) === (b.meta === true) &&
  (a.shift === true) === (b.shift === true)

// Platform-aware keybinding label derived from the table (never hand-authored).
export const formatChord = (chord: KeyChord, platform: PlatformVariant = "web"): string => {
  const mac = platform === "ios"
  const parts: Array<string> = []
  if (chord.ctrl === true) parts.push(mac ? "⌃" : "Ctrl")
  if (chord.alt === true) parts.push(mac ? "⌥" : "Alt")
  if (chord.shift === true) parts.push(mac ? "⇧" : "Shift")
  if (chord.meta === true) parts.push(mac ? "⌘" : "Meta")
  const key = chord.key.length === 1 ? chord.key.toUpperCase() : chord.key
  parts.push(key)
  return mac ? parts.join("") : parts.join("+")
}

const detectKeymapConflicts = (commands: ReadonlyArray<CommandDefinition>): ReadonlyArray<KeymapConflict> => {
  const byKey = new Map<string, { readonly chord: KeyChord; readonly scope: string; readonly ids: Array<string> }>()
  for (const command of commands) {
    if (command.binding === undefined) continue
    const scope = command.scope ?? defaultFocusScope
    const chord = command.binding
    const signature = `${scope}::${normalizeChordKey(chord.key)}::${chord.alt === true}::${chord.ctrl === true}::${chord.meta === true}::${chord.shift === true}`
    const existing = byKey.get(signature)
    if (existing === undefined) {
      byKey.set(signature, { chord, scope, ids: [command.id] })
    } else {
      existing.ids.push(command.id)
    }
  }
  return Array.from(byKey.values())
    .filter((entry) => entry.ids.length > 1)
    .map((entry) => ({ chord: entry.chord, scope: entry.scope, commandIds: entry.ids }))
}

export interface Keymap {
  readonly commands: ReadonlyArray<CommandDefinition>
  readonly conflicts: ReadonlyArray<KeymapConflict>
  // Resolve a chord to a command within the current scope stack + context,
  // without firing it. Higher (more recently pushed) scopes shadow lower ones.
  readonly resolve: (chord: KeyChord) => Effect.Effect<Option.Option<CommandDefinition>>
  // Resolve and fire the matched command's intent; returns the command id fired.
  readonly dispatchChord: (chord: KeyChord) => Effect.Effect<Option.Option<string>, IntentError, IntentRegistry>
  readonly activeScope: Effect.Effect<string>
  readonly scopeStack: Effect.Effect<ReadonlyArray<string>>
  // Push a focus scope (e.g. "palette-open") with an optional focus-return
  // target key restored when the scope is popped.
  readonly pushScope: (scope: string, returnFocus?: string) => Effect.Effect<void>
  // Pop the top scope; returns the focus-return target recorded for it.
  readonly popScope: Effect.Effect<Option.Option<string>>
  readonly setContext: (flags: Iterable<string>) => Effect.Effect<void>
  readonly context: Effect.Effect<ReadonlySet<string>>
  readonly keybindingLabel: (commandId: string) => Option.Option<string>
}

export interface KeymapOptions {
  readonly platform?: PlatformVariant
  readonly initialScope?: string
  readonly initialContext?: Iterable<string>
}

const commandEnabled = (command: CommandDefinition, context: ReadonlySet<string>): boolean =>
  command.when === undefined || context.has(command.when)

export const makeKeymap = (
  commands: ReadonlyArray<CommandDefinition>,
  options: KeymapOptions = {}
): Effect.Effect<Keymap> =>
  Effect.gen(function*() {
    const platform = options.platform ?? "web"
    const scopeStackRef = yield* Ref.make<ReadonlyArray<string>>([options.initialScope ?? defaultFocusScope])
    const returnFocusStackRef = yield* Ref.make<ReadonlyArray<string | undefined>>([undefined])
    const contextRef = yield* Ref.make<ReadonlySet<string>>(new Set(options.initialContext ?? []))
    const conflicts = detectKeymapConflicts(commands)
    const commandsById = new Map(commands.map((command) => [command.id, command] as const))

    const resolveIn = (
      chord: KeyChord,
      stack: ReadonlyArray<string>,
      context: ReadonlySet<string>
    ): Option.Option<CommandDefinition> => {
      for (let index = stack.length - 1; index >= 0; index -= 1) {
        const scope = stack[index]
        const match = commands.find((command) =>
          (command.scope ?? defaultFocusScope) === scope &&
          command.binding !== undefined &&
          chordEquals(command.binding, chord) &&
          commandEnabled(command, context))
        if (match !== undefined) return Option.some(match)
      }
      return Option.none()
    }

    const resolve = (chord: KeyChord): Effect.Effect<Option.Option<CommandDefinition>> =>
      Effect.gen(function*() {
        const stack = yield* Ref.get(scopeStackRef)
        const context = yield* Ref.get(contextRef)
        return resolveIn(chord, stack, context)
      })

    const dispatchChord = (chord: KeyChord): Effect.Effect<Option.Option<string>, IntentError, IntentRegistry> =>
      Effect.gen(function*() {
        const matched = yield* resolve(chord)
        if (Option.isNone(matched)) return Option.none()
        yield* dispatchIntent(resolveIntentRef(matched.value.intent))
        return Option.some(matched.value.id)
      })

    return {
      commands,
      conflicts,
      resolve,
      dispatchChord,
      activeScope: Ref.get(scopeStackRef).pipe(Effect.map((stack) => stack[stack.length - 1] ?? defaultFocusScope)),
      scopeStack: Ref.get(scopeStackRef),
      pushScope: (scope, returnFocus) =>
        Effect.gen(function*() {
          yield* Ref.update(scopeStackRef, (stack) => [...stack, scope])
          yield* Ref.update(returnFocusStackRef, (stack) => [...stack, returnFocus])
        }),
      popScope: Effect.gen(function*() {
        const stack = yield* Ref.get(scopeStackRef)
        if (stack.length <= 1) return Option.none()
        yield* Ref.set(scopeStackRef, stack.slice(0, -1))
        const focusStack = yield* Ref.get(returnFocusStackRef)
        const returned = focusStack[focusStack.length - 1]
        yield* Ref.set(returnFocusStackRef, focusStack.slice(0, -1))
        return returned === undefined ? Option.none() : Option.some(returned)
      }),
      setContext: (flags) => Ref.set(contextRef, new Set(flags)),
      context: Ref.get(contextRef),
      keybindingLabel: (commandId) => {
        const command = commandsById.get(commandId)
        return command === undefined || command.binding === undefined
          ? Option.none()
          : Option.some(formatChord(command.binding, platform))
      }
    }
  })

export const Keymap = Context.Service<Keymap>("@effect-native/core/Keymap")

export const makeKeymapLayer = (
  commands: ReadonlyArray<CommandDefinition>,
  options?: KeymapOptions
) => Layer.effect(Keymap, makeKeymap(commands, options))

// Roving-tabindex helper (issue #41): the active item gets tabIndex 0, the rest
// -1, so a group is a single tab stop with arrow-key traversal inside it.
export const rovingTabIndex = (count: number, activeIndex: number): ReadonlyArray<-1 | 0> =>
  Array.from({ length: count }, (_unused, index) => (index === activeIndex ? 0 : -1))
