import {
  decodeRuntimeControlIntent,
  decodeRuntimeControlOutcome,
  type RuntimeControlIntent,
  type RuntimeControlOutcome,
} from "@openagentsinc/agent-runtime-schema"

export type MobileRuntimeQueueControl = Extract<RuntimeControlIntent, { kind: "turn.queue" }>

const safeSuffix = (value: string): string =>
  value.replace(/[^A-Za-z0-9._:-]/gu, "").slice(0, 160)

export const makeMobileRuntimeQueueControl = (input: Readonly<{
  intentRef: string
  messageRef: string
  threadRef: string
  runVersion: number
  createdAt: string
  expiresAt: string
}>): MobileRuntimeQueueControl => decodeRuntimeControlIntent({
  schema: "openagents.runtime_control_intent.v2",
  kind: "turn.queue",
  intentRef: safeSuffix(input.intentRef),
  idempotencyKey: safeSuffix(`idem.${input.intentRef}`),
  threadRef: input.threadRef,
  messageRef: input.messageRef,
  targetGeneration: { state: "known", value: input.runVersion },
  orderingKey: safeSuffix(`order.${input.threadRef}`),
  createdAt: input.createdAt,
  expiresAt: input.expiresAt,
  origin: { surface: "mobile", lane: "khala_sync" },
}) as MobileRuntimeQueueControl

export const mobileRuntimeQueueAdmissionOutcome = (input: Readonly<{
  control: MobileRuntimeQueueControl
  observedAt: string
  admission: "accepted" | "pending" | "expired" | "failed"
}>): RuntimeControlOutcome => {
  const common = {
    schema: "openagents.runtime_control_outcome.v1" as const,
    outcomeRef: safeSuffix(`outcome.${input.control.intentRef}`),
    intentRef: input.control.intentRef,
    idempotencyKey: input.control.idempotencyKey,
    observedAt: input.observedAt,
    terminal: { status: "pending" as const },
  }
  switch (input.admission) {
    case "accepted":
      return decodeRuntimeControlOutcome({
        ...common,
        admission: { status: "accepted", acceptedAt: input.observedAt },
        // Pylon's queue-until-idle adapter has admitted the legacy command;
        // its later enforcement/promotion remains separately observed.
        delivery: { status: "pending" },
      })
    case "pending":
      return decodeRuntimeControlOutcome({
        ...common,
        admission: { status: "pending" },
        delivery: { status: "pending" },
      })
    case "expired":
      return decodeRuntimeControlOutcome({
        ...common,
        admission: { status: "expired", expiredAt: input.observedAt },
        delivery: { status: "failed", reasonRef: "reason.queue_expired" },
      })
    case "failed":
      return decodeRuntimeControlOutcome({
        ...common,
        admission: { status: "rejected", reasonRef: "reason.queue_admission_failed" },
        delivery: { status: "failed", reasonRef: "reason.queue_admission_failed" },
      })
  }
}

export type MobileRuntimeQueueReceipt = Readonly<{
  control: MobileRuntimeQueueControl
  outcome: RuntimeControlOutcome
  parentRunRef: string
  messageRef: string
}>
