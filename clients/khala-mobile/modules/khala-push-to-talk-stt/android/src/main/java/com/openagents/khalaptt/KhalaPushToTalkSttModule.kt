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

    AsyncFunction("startRecognitionAsync") { locale: String? ->
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
