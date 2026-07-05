package com.openagents.khalaptt

import expo.modules.kotlin.exception.CodedException
import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class KhalaPushToTalkSttModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("KhalaPushToTalkStt")

    AsyncFunction("getAvailabilityAsync") {
      mapOf(
        "status" to "unavailable",
        "reason" to "android_stt_runtime_pending"
      )
    }

    // Explicit `<Map<String, Any>, String?>` type witness (matching
    // `stopRecognitionAsync`'s Map shape below): the lambda always throws, so
    // its own inferred return type is `Nothing`, and Kotlin does not allow
    // `Nothing` as a reified type argument
    // ("Cannot use 'Nothing' as reified type parameter") — a real clean-build
    // failure this call site had until this fix (`AsyncFunction`'s `R` is
    // `reified`). Pinning `R` explicitly sidesteps the inference entirely;
    // `Nothing` is a subtype of `Map<String, Any>` so the always-throwing
    // lambda still satisfies it at compile time.
    AsyncFunction<Map<String, Any>, String?>("startRecognitionAsync") { locale ->
      throw CodedException("android_stt_runtime_pending")
    }

    AsyncFunction("stopRecognitionAsync") {
      mapOf(
        "transcript" to "",
        "isFinal" to true,
        "locale" to "device"
      )
    }
  }
}
