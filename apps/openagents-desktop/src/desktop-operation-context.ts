import { Schema } from "@effect-native/core/effect"

const DesktopOperationRefSchema = Schema.String.check(
  Schema.isMinLength(1),
  Schema.isMaxLength(256),
  Schema.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)

export const DesktopOperationContextSchema = Schema.Struct({
  operationRef: DesktopOperationRefSchema,
  sessionRef: DesktopOperationRefSchema,
  correlationRef: DesktopOperationRefSchema,
  runRef: Schema.optional(DesktopOperationRefSchema),
})

export type DesktopOperationContext = typeof DesktopOperationContextSchema.Type

export const DesktopCorrelationStageSchema = Schema.Literals([
  "ipc.received",
  "gateway.received",
  "sync.intent",
  "ipc.returned",
])
export type DesktopCorrelationStage = typeof DesktopCorrelationStageSchema.Type

export const DesktopPublicCorrelationEventSchema = Schema.Struct({
  kind: Schema.Literal("desktop.operation.correlation"),
  stage: DesktopCorrelationStageSchema,
  context: DesktopOperationContextSchema,
})
export type DesktopPublicCorrelationEvent = typeof DesktopPublicCorrelationEventSchema.Type

const decode = Schema.decodeUnknownSync(DesktopOperationContextSchema)

export const decodeDesktopOperationContext = (value: unknown): DesktopOperationContext | null => {
  try {
    return decode(value)
  } catch {
    return null
  }
}

export const desktopOperationRef = (request: Readonly<{
  kind: "query" | "command"
  requestId?: string
  commandId?: string
}>): string => request.kind === "query" ? request.requestId ?? "invalid.query" : request.commandId ?? "invalid.command"

export const makeDesktopOperationContext = (input: Readonly<{
  operationRef: string
  sessionRef: string
  correlationRef: string
  runRef?: string
}>): DesktopOperationContext => decode(input)

export type DesktopCorrelationJournal = Readonly<{
  record: (stage: DesktopCorrelationStage, context: DesktopOperationContext) => void
  stages: (correlationRef: string) => ReadonlyArray<DesktopCorrelationStage>
  complete: (correlationRef: string) => boolean
  dispose: () => void
}>

export const makeDesktopCorrelationJournal = (
  log: (event: DesktopPublicCorrelationEvent) => void = () => undefined,
): DesktopCorrelationJournal => {
  const records = new Map<string, DesktopCorrelationStage[]>()
  return {
    record: (stage, context) => {
      const event = Schema.decodeUnknownSync(DesktopPublicCorrelationEventSchema)({
        kind: "desktop.operation.correlation",
        stage,
        context,
      })
      const previous = records.get(context.correlationRef) ?? []
      records.set(context.correlationRef, [...previous, stage].slice(-16))
      log(event)
    },
    stages: correlationRef => [...(records.get(correlationRef) ?? [])],
    complete: correlationRef => {
      const stages = records.get(correlationRef) ?? []
      return ["ipc.received", "gateway.received", "sync.intent", "ipc.returned"]
        .every(stage => stages.includes(stage as DesktopCorrelationStage))
    },
    dispose: () => { records.clear() },
  }
}
