import {
  decodeRuntimeControlIntent,
  decodeRuntimeControlOutcome,
  type RuntimeControlIntent,
  type RuntimeControlOutcome,
} from "@openagentsinc/agent-runtime-schema"

/** Shared composer truth consumed by Desktop now and lowered by web/mobile later. */
export type ComposerAdmissionState =
  | "idle"
  | "active_steerable"
  | "active_nonsteerable"
  | "interrupting"
  | "repairing"
  | "queued"
  | "offline"
  | "blocked"
  | "incompatible"

export type ComposerSubmitMode = "steer" | "queue"

export type ComposerAdmission = Readonly<{
  state: ComposerAdmissionState
  activeTurnId: string | null
  reason: string | null
  queuedCount: number
}>

export type ComposerActionPresentation = Readonly<{
  mode: ComposerSubmitMode
  label: "Steer now" | "Queue next"
  submitLabel: "Steer" | "Queue"
  enabled: boolean
  consequence: string
}>

export const idleComposerAdmission = (): ComposerAdmission => ({
  state: "idle",
  activeTurnId: null,
  reason: null,
  queuedCount: 0,
})

export const composerActionPresentation = (
  admission: ComposerAdmission,
  mode: ComposerSubmitMode,
): ComposerActionPresentation => {
  if (mode === "queue") {
    const ordinal = admission.queuedCount + 1
    return {
      mode,
      label: "Queue next",
      submitLabel: "Queue",
      enabled: !["offline", "blocked", "incompatible"].includes(admission.state),
      consequence: admission.state === "idle"
        ? "Starts when submitted; no active turn is running."
        : `Saves durable follow-up #${ordinal}; starts only after the active turn settles.`,
    }
  }
  const enabled = admission.state === "active_steerable" && admission.activeTurnId !== null
  return {
    mode,
    label: "Steer now",
    submitLabel: "Steer",
    enabled,
    consequence: enabled
      ? `Sends into active turn ${admission.activeTurnId}; does not create a queue item.`
      : admission.reason ?? "Steering is unavailable until a regular active turn is confirmed.",
  }
}

type ComposerIntentBase = Readonly<{
  threadRef: string
  message: string
  intentRef: string
  clientUserMessageId: string
  control: RuntimeControlIntent
}>
export type ComposerSubmitIntent =
  | (ComposerIntentBase & Readonly<{ kind: "steer_current"; expectedTurnId: string }>)
  | (ComposerIntentBase & Readonly<{ kind: "queue_next" }>)

export type ComposerInterruptIntent = Extract<RuntimeControlIntent, { kind: "turn.interrupt" }>
export type ComposerInterruptOutcome = RuntimeControlOutcome

export const makeComposerInterruptIntent = (input: Readonly<{
  threadRef: string
  turnRef: string
  intentRef: string
  createdAt: string
  targetGeneration?: RuntimeControlIntent["targetGeneration"]
}>): ComposerInterruptIntent => {
  const expiresAt = new Date(new Date(input.createdAt).getTime() + 5 * 60_000).toISOString()
  return decodeRuntimeControlIntent({
    schema: "openagents.runtime_control_intent.v2",
    kind: "turn.interrupt",
    intentRef: input.intentRef,
    idempotencyKey: input.intentRef,
    threadRef: input.threadRef,
    turnRef: input.turnRef,
    targetGeneration: input.targetGeneration ?? {
      state: "unknown",
      reason: "not_observed",
    },
    orderingKey: `order:${input.threadRef}`,
    createdAt: input.createdAt,
    expiresAt,
    origin: { surface: "desktop", lane: "owner_local" },
  }) as ComposerInterruptIntent
}

export const makeComposerInterruptOutcome = (input: Readonly<{
  control: ComposerInterruptIntent
  observedAt: string
  admission: RuntimeControlOutcome["admission"]
  delivery: RuntimeControlOutcome["delivery"]
}>): RuntimeControlOutcome => decodeRuntimeControlOutcome({
  schema: "openagents.runtime_control_outcome.v1",
  outcomeRef: `outcome.${input.control.intentRef}`,
  intentRef: input.control.intentRef,
  idempotencyKey: input.control.idempotencyKey,
  observedAt: input.observedAt,
  admission: input.admission,
  delivery: input.delivery,
  terminal: { status: "pending" },
})

export const makeComposerSubmitIntent = (input: Readonly<{
  admission: ComposerAdmission
  mode: ComposerSubmitMode
  threadRef: string
  message: string
  intentRef: string
  clientUserMessageId: string
  createdAt: string
  targetGeneration?: RuntimeControlIntent["targetGeneration"]
}>): ComposerSubmitIntent | null => {
  const action = composerActionPresentation(input.admission, input.mode)
  if (!action.enabled || input.message.trim() === "") return null
  const expiresAt = new Date(new Date(input.createdAt).getTime() + 5 * 60_000).toISOString()
  const common = {
    schema: "openagents.runtime_control_intent.v2" as const,
    intentRef: input.intentRef,
    idempotencyKey: input.intentRef,
    threadRef: input.threadRef,
    targetGeneration: input.targetGeneration ?? {
      state: "unknown" as const,
      reason: "not_observed" as const,
    },
    orderingKey: `order:${input.threadRef}`,
    messageRef: input.clientUserMessageId,
    createdAt: input.createdAt,
    expiresAt,
    origin: { surface: "desktop" as const, lane: "owner_local" as const },
  }
  if (input.mode === "steer") {
    const expectedTurnId = input.admission.activeTurnId
    if (expectedTurnId === null) return null
    return {
      kind: "steer_current",
      threadRef: input.threadRef,
      message: input.message.trim(),
      intentRef: input.intentRef,
      clientUserMessageId: input.clientUserMessageId,
      expectedTurnId,
      control: decodeRuntimeControlIntent({
        ...common,
        kind: "turn.steer",
        turnRef: expectedTurnId,
      }),
    }
  }
  return {
    kind: "queue_next",
    threadRef: input.threadRef,
    message: input.message.trim(),
    intentRef: input.intentRef,
    clientUserMessageId: input.clientUserMessageId,
    control: decodeRuntimeControlIntent({ ...common, kind: "turn.queue" }),
  }
}
