import AppleFoundationModels from "khala-apple-foundation-models"
import type { KhalaAppleFoundationModelsAvailability } from "khala-apple-foundation-models"
import PushToTalkStt from "khala-push-to-talk-stt"
import type { KhalaSpeechAvailability } from "khala-push-to-talk-stt"

export const khalaNativeModules = {
  appleFoundationModels: AppleFoundationModels,
  pushToTalkStt: PushToTalkStt
} as const

const bridgeErrorMessage = (error: unknown): string =>
  error instanceof Error ? error.message : String(error)

/**
 * `pushToTalkStt` and `appleFoundationModels` are two unrelated native
 * capability checks. The native module bodies never throw today, but the
 * Expo JS/native bridge itself can reject independently of the native
 * implementation (a real, explicitly-supported failure mode when a native
 * binary is out of sync with an OTA-updated bundle). A bare `Promise.all`
 * would let one bridge rejection blank visibility into the OTHER,
 * completely unrelated capability's real readiness — so each probe is
 * isolated here and a bridge failure reports as its own typed
 * "unavailable" outcome instead of discarding the sibling's result.
 */
export const readNativeReadiness = async () => {
  const [speechResult, appleFMResult] = await Promise.all([
    khalaNativeModules.pushToTalkStt.getAvailabilityAsync().then(
      (value): KhalaSpeechAvailability => value,
      (error: unknown): KhalaSpeechAvailability => ({
        status: "unavailable",
        reason: `Speech availability bridge call failed: ${bridgeErrorMessage(error)}`
      })
    ),
    khalaNativeModules.appleFoundationModels.getAvailabilityAsync().then(
      (value): KhalaAppleFoundationModelsAvailability => value,
      (error: unknown): KhalaAppleFoundationModelsAvailability => ({
        status: "unavailable",
        summary: `Apple Foundation Models availability bridge call failed: ${bridgeErrorMessage(error)}`,
        blockerRefs: ["blocker.native.bridge_error"]
      })
    )
  ])

  return { appleFM: appleFMResult, speech: speechResult } as const
}
