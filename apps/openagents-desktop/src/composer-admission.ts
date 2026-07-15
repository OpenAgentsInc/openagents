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
}>
export type ComposerSubmitIntent =
  | (ComposerIntentBase & Readonly<{ kind: "steer_current"; expectedTurnId: string }>)
  | (ComposerIntentBase & Readonly<{ kind: "queue_next" }>)

export const makeComposerSubmitIntent = (input: Readonly<{
  admission: ComposerAdmission
  mode: ComposerSubmitMode
  threadRef: string
  message: string
  intentRef: string
  clientUserMessageId: string
}>): ComposerSubmitIntent | null => {
  const action = composerActionPresentation(input.admission, input.mode)
  if (!action.enabled || input.message.trim() === "") return null
  return input.mode === "steer"
    ? {
        kind: "steer_current",
        threadRef: input.threadRef,
        message: input.message.trim(),
        intentRef: input.intentRef,
        clientUserMessageId: input.clientUserMessageId,
        expectedTurnId: input.admission.activeTurnId!,
      }
    : {
        kind: "queue_next",
        threadRef: input.threadRef,
        message: input.message.trim(),
        intentRef: input.intentRef,
        clientUserMessageId: input.clientUserMessageId,
      }
}
