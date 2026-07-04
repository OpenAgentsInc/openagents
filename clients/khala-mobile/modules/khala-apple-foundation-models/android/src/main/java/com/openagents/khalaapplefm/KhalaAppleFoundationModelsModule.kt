package com.openagents.khalaapplefm

import expo.modules.kotlin.modules.Module
import expo.modules.kotlin.modules.ModuleDefinition

class KhalaAppleFoundationModelsModule : Module() {
  override fun definition() = ModuleDefinition {
    Name("KhalaAppleFoundationModels")

    AsyncFunction("getAvailabilityAsync") {
      mapOf(
        "status" to "unavailable",
        "summary" to "Apple Foundation Models are not available on Android.",
        "blockerRefs" to listOf("blocker.khala_mobile.apple_fm_android_unavailable")
      )
    }
  }
}
