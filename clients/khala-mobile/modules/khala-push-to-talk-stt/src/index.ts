import { requireNativeModule } from "expo-modules-core"

export type KhalaSpeechAvailability = Readonly<{
  status: "available" | "denied" | "unavailable"
  reason?: string
}>

export type KhalaSpeechRecognitionResult = Readonly<{
  transcript: string
  isFinal: boolean
  locale: string
}>

export type KhalaPushToTalkSttModule = Readonly<{
  getAvailabilityAsync: () => Promise<KhalaSpeechAvailability>
  startRecognitionAsync: (locale?: string) => Promise<KhalaSpeechRecognitionResult>
  stopRecognitionAsync: () => Promise<KhalaSpeechRecognitionResult>
}>

const module = requireNativeModule<KhalaPushToTalkSttModule>("KhalaPushToTalkStt")

export default module
