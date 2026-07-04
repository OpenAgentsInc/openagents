import AppleFoundationModels from "khala-apple-foundation-models"
import PushToTalkStt from "khala-push-to-talk-stt"

export const khalaNativeModules = {
  appleFoundationModels: AppleFoundationModels,
  pushToTalkStt: PushToTalkStt
} as const

export const readNativeReadiness = async () => {
  const [speech, appleFM] = await Promise.all([
    khalaNativeModules.pushToTalkStt.getAvailabilityAsync(),
    khalaNativeModules.appleFoundationModels.getAvailabilityAsync()
  ])

  return { appleFM, speech } as const
}
