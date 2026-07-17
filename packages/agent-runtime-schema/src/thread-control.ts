import { Schema as S } from "effect"

export const RuntimeControlIntentSchemaLiteral = "openagents.runtime_control_intent.v2" as const
export const RuntimeControlOutcomeSchemaLiteral = "openagents.runtime_control_outcome.v1" as const

const RuntimeControlRef = S.String.check(
  S.isMinLength(1),
  S.isMaxLength(256),
  S.isPattern(/^[A-Za-z0-9][A-Za-z0-9._:-]*$/),
)
const RuntimeControlTimestamp = S.String.check(
  S.isPattern(/^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(\.\d+)?Z$/),
)
const RuntimeControlGeneration = S.Union([
  S.Struct({ state: S.Literal("known"), value: S.Number.check(S.isInt(), S.isGreaterThanOrEqualTo(0)) }),
  S.Struct({ state: S.Literal("unknown"), reason: S.Literals(["not_observed", "provider_unsupported"]) }),
])

const RuntimeControlOrigin = S.Struct({
  surface: S.Literals(["desktop", "mobile", "web", "server", "test_fixture"]),
  lane: S.Literals(["owner_local", "khala_sync", "managed_cloud", "test_fixture"]),
  deviceRef: S.optional(RuntimeControlRef),
})

const RuntimeControlIntentBase = {
  schema: S.Literal(RuntimeControlIntentSchemaLiteral),
  intentRef: RuntimeControlRef,
  idempotencyKey: RuntimeControlRef,
  threadRef: RuntimeControlRef,
  targetGeneration: RuntimeControlGeneration,
  orderingKey: RuntimeControlRef,
  createdAt: RuntimeControlTimestamp,
  expiresAt: RuntimeControlTimestamp,
  origin: RuntimeControlOrigin,
}

/**
 * Provider-neutral thread control. Queue, steer, and interrupt are deliberately
 * different discriminants: no adapter may silently translate one into another.
 * Message content stays outside this ref-only contract.
 */
export const RuntimeControlIntent = S.Union([
  S.Struct({
    ...RuntimeControlIntentBase,
    kind: S.Literal("turn.queue"),
    messageRef: RuntimeControlRef,
  }),
  S.Struct({
    ...RuntimeControlIntentBase,
    kind: S.Literal("turn.steer"),
    turnRef: RuntimeControlRef,
    messageRef: RuntimeControlRef,
  }),
  S.Struct({
    ...RuntimeControlIntentBase,
    kind: S.Literal("turn.interrupt"),
    turnRef: RuntimeControlRef,
    reasonRef: S.optional(RuntimeControlRef),
  }),
])
export type RuntimeControlIntent = typeof RuntimeControlIntent.Type

const RuntimeControlAdmission = S.Union([
  S.Struct({ status: S.Literal("pending") }),
  S.Struct({ status: S.Literal("accepted"), acceptedAt: RuntimeControlTimestamp }),
  S.Struct({ status: S.Literal("rejected"), reasonRef: RuntimeControlRef }),
  S.Struct({ status: S.Literal("expired"), expiredAt: RuntimeControlTimestamp }),
])

const RuntimeControlDelivery = S.Union([
  S.Struct({ status: S.Literal("pending") }),
  S.Struct({ status: S.Literal("queued"), queueRef: RuntimeControlRef }),
  S.Struct({ status: S.Literal("applied"), appliedAt: RuntimeControlTimestamp }),
  S.Struct({ status: S.Literal("unsupported"), reasonRef: RuntimeControlRef }),
  S.Struct({ status: S.Literal("failed"), reasonRef: RuntimeControlRef }),
  S.Struct({ status: S.Literal("skipped_stale"), observedGeneration: RuntimeControlGeneration }),
])

const RuntimeControlTerminal = S.Union([
  S.Struct({ status: S.Literal("pending") }),
  S.Struct({ status: S.Literal("observed"), eventRef: RuntimeControlRef, observedAt: RuntimeControlTimestamp }),
  S.Struct({ status: S.Literal("not_observed"), reasonRef: RuntimeControlRef }),
])

/** Admission, delivery, and terminal observation are independent evidence axes. */
export const RuntimeControlOutcome = S.Struct({
  schema: S.Literal(RuntimeControlOutcomeSchemaLiteral),
  outcomeRef: RuntimeControlRef,
  intentRef: RuntimeControlRef,
  idempotencyKey: RuntimeControlRef,
  observedAt: RuntimeControlTimestamp,
  admission: RuntimeControlAdmission,
  delivery: RuntimeControlDelivery,
  terminal: RuntimeControlTerminal,
})
export type RuntimeControlOutcome = typeof RuntimeControlOutcome.Type

const decodeRuntimeControlIntentSchema = S.decodeUnknownSync(RuntimeControlIntent)

export const decodeRuntimeControlIntent = (input: unknown): RuntimeControlIntent => {
  if (typeof input === "object" && input !== null) {
    const record = input as Readonly<Record<string, unknown>>
    for (const forbidden of ["body", "message", "prompt"] as const) {
      if (Object.hasOwn(record, forbidden)) {
        throw new Error(`Runtime control intent contains forbidden raw field: ${forbidden}`)
      }
    }
  }
  const decoded = decodeRuntimeControlIntentSchema(input)
  if (Date.parse(decoded.expiresAt) <= Date.parse(decoded.createdAt)) {
    throw new Error("Runtime control intent expiry must be after creation")
  }
  return decoded
}
export const decodeRuntimeControlOutcome = S.decodeUnknownSync(RuntimeControlOutcome)

export type RuntimeControlReplayDisposition = "new" | "exact_retry" | "conflicting_reuse"

/** Classify lost-ACK retries without treating a reused identity as a new command. */
export const classifyRuntimeControlReplay = (
  existing: RuntimeControlIntent,
  incoming: RuntimeControlIntent,
): RuntimeControlReplayDisposition => {
  const sharesIdentity = existing.intentRef === incoming.intentRef ||
    existing.idempotencyKey === incoming.idempotencyKey
  if (!sharesIdentity) return "new"
  return JSON.stringify(existing) === JSON.stringify(incoming)
    ? "exact_retry"
    : "conflicting_reuse"
}
