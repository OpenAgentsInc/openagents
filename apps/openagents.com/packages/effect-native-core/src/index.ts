import {
  Cause,
  Context,
  Deferred,
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
export const PreviousCatalogVersion = OverlayCatalogVersion
export const CatalogVersion = "effect-native/v5" as const
export const CatalogVersionSchema = Schema.Literal(CatalogVersion)
export type CatalogVersion = typeof CatalogVersion
export const compatibleCatalogVersions = [
  LegacyCatalogVersion,
  LinkCatalogVersion,
  ResponsiveCatalogVersion,
  FormCatalogVersion,
  PreviousCatalogVersion,
  CatalogVersion
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
  "Sheet"
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

export const makeIntentRegistry =<const Definitions extends ReadonlyArray<IntentDefinition>>(
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
}

export interface StackView extends NodeBase {
  readonly _tag: "Stack"
  readonly direction: ResponsiveValue<StackDirection>
  readonly gap?: ResponsiveValue<SpacingToken>
  readonly align?: StackAlign
  readonly justify?: StackJustify
  readonly padding?: ResponsiveValue<SpacingToken>
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
  key: NodeKeySchema.pipe(Schema.optionalKey)
} as const

export const StackSchema: Schema.Codec<StackView, StackView> = Schema.TaggedStruct("Stack", {
  ...CommonFields,
  direction: ResponsiveStackDirectionSchema,
  gap: ResponsiveSpacingTokenSchema.pipe(Schema.optionalKey),
  align: StackAlignSchema.pipe(Schema.optionalKey),
  justify: StackJustifySchema.pipe(Schema.optionalKey),
  padding: ResponsiveSpacingTokenSchema.pipe(Schema.optionalKey),
  style: StackStyleSchema.pipe(Schema.optionalKey),
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
    SheetSchema
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
      return view
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
