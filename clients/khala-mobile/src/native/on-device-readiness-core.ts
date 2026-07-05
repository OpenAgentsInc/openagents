import type { KhalaAppleFoundationModelsAvailability } from "khala-apple-foundation-models"
import type { KhalaSpeechAvailability } from "khala-push-to-talk-stt"

/** Pure formatting layer between `readNativeReadiness()`'s raw availability
 * payloads and the "On-device" settings section. Ported from
 * `../legacy-screens/settings.tsx`, which showed the exact same two
 * readiness values as plain `StatLine` rows with no tone/detail — this keeps
 * the same two probes (speech + Apple FM) but formats them for the current
 * routed screen's Frame-card + tone-color convention (matching
 * `READINESS_COLOR` in `app/(drawer)/settings.tsx`'s fleet-account section). */
export type OnDeviceReadinessTone = "success" | "warning" | "danger" | "faint"

export type OnDeviceReadinessRow = Readonly<{
  key: "speech" | "appleFM"
  label: string
  status: string
  tone: OnDeviceReadinessTone
  detail?: string
}>

const speechTone = (status: KhalaSpeechAvailability["status"]): OnDeviceReadinessTone => {
  if (status === "available") return "success"
  if (status === "denied") return "danger"
  return "warning"
}

const appleFMTone = (status: KhalaAppleFoundationModelsAvailability["status"]): OnDeviceReadinessTone => {
  if (status === "available") return "success"
  if (status === "blocked") return "warning"
  return "faint"
}

export const buildOnDeviceReadinessRows = (input: {
  speech: KhalaSpeechAvailability
  appleFM: KhalaAppleFoundationModelsAvailability
}): ReadonlyArray<OnDeviceReadinessRow> => [
  {
    detail: input.speech.reason,
    key: "speech",
    label: "Speech (push-to-talk)",
    status: input.speech.status,
    tone: speechTone(input.speech.status)
  },
  {
    detail: input.appleFM.summary,
    key: "appleFM",
    label: "Apple Foundation Models",
    status: input.appleFM.status,
    tone: appleFMTone(input.appleFM.status)
  }
]
