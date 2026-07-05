import type { KhalaSpeechAvailability } from "khala-push-to-talk-stt"

/** Composer mic-button phase. Pure state machine, decoupled from the native
 * module so it is fully testable under `bun test` with zero React Native
 * runtime. See `../legacy-screens/settings.tsx` for the pre-existing
 * (unwired) use of `KhalaSpeechAvailability` this ports forward — this file
 * adds the actual button-press state machine that legacy screen never had,
 * since neither legacy screen called `startRecognitionAsync`. */
export type PushToTalkPhase = "idle" | "checking" | "recording" | "denied" | "unavailable" | "error"

/** Maps the native module's one-shot availability probe to the mic button's
 * resting phase. `"denied"`/`"unavailable"` disable the button outright
 * rather than let a doomed `startRecognitionAsync()` call run — the iOS and
 * Android module implementations both currently throw unconditionally from
 * that call (see the Swift/Kotlin sources), so gating on availability first
 * avoids surfacing a confusing native exception for a case the availability
 * probe already predicted. */
export const phaseFromAvailability = (availability: KhalaSpeechAvailability): PushToTalkPhase => {
  if (availability.status === "denied") return "denied"
  if (availability.status === "unavailable") return "unavailable"
  return "idle"
}

/** Whether a tap should attempt a real `startRecognitionAsync()`/
 * `stopRecognitionAsync()` call in this phase, vs. only surface a hint. */
export const isPushToTalkPressable = (phase: PushToTalkPhase): boolean =>
  phase === "idle" || phase === "recording"

/** Short label for the mic button's accessible name, reflecting the current
 * phase so a screen reader announces state changes. */
export const pushToTalkAccessibilityLabel = (phase: PushToTalkPhase): string => {
  switch (phase) {
    case "recording":
      return "Stop dictation"
    case "checking":
      return "Checking dictation availability"
    case "denied":
      return "Dictation unavailable — microphone or speech permission denied"
    case "unavailable":
      return "Dictation unavailable on this device"
    case "error":
      return "Dictation failed — tap to try again"
    case "idle":
    default:
      return "Dictate message"
  }
}

/** Normalizes a native `startRecognitionAsync`/`stopRecognitionAsync`
 * rejection into the composer's existing user-facing error-message row
 * (`ChatComposer`'s `errorMessage` state). The current module shells always
 * reject with an `Exception` whose `message` names it as a pending native
 * capture proof (see `KhalaPushToTalkSttModule.swift`'s
 * `SpeechRuntimeUnavailableException` and the Android
 * `CodedException("android_stt_runtime_pending")`) — this keeps that reason
 * user-legible instead of dumping a raw native error string. */
export const describePushToTalkFailure = (error: unknown): string => {
  const raw = error instanceof Error ? error.message : String(error)
  const trimmed = raw.trim()
  if (trimmed.length === 0) return "Dictation is not available on this device yet."
  return `Dictation is not available on this device yet (${trimmed}).`
}

/** Merges a finished dictation transcript into the composer's current draft
 * text. Appends with a separating space when the draft already has content
 * (e.g. the user typed part of a message, then dictated the rest) rather
 * than overwriting it; a blank transcript is a no-op so a cancelled/empty
 * recognition never clobbers an in-progress draft. */
export const mergeTranscriptIntoDraft = (draft: string, transcript: string): string => {
  const trimmedTranscript = transcript.trim()
  if (trimmedTranscript.length === 0) return draft
  if (draft.trim().length === 0) return trimmedTranscript
  const needsSpace = !draft.endsWith(" ") && !draft.endsWith("\n")
  return `${draft}${needsSpace ? " " : ""}${trimmedTranscript}`
}
