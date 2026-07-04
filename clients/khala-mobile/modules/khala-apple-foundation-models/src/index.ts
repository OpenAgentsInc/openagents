import { requireNativeModule } from "expo-modules-core"

export type KhalaAppleFoundationModelsAvailability = Readonly<{
  status: "available" | "blocked" | "unavailable"
  summary: string
  blockerRefs: ReadonlyArray<string>
}>

export type KhalaAppleFoundationModelsModule = Readonly<{
  getAvailabilityAsync: () => Promise<KhalaAppleFoundationModelsAvailability>
}>

const module =
  requireNativeModule<KhalaAppleFoundationModelsModule>("KhalaAppleFoundationModels")

export default module
