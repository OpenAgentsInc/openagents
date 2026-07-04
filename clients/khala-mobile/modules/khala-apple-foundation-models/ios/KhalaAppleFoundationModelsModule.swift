import ExpoModulesCore
import Foundation

public final class KhalaAppleFoundationModelsModule: Module {
  public func definition() -> ModuleDefinition {
    Name("KhalaAppleFoundationModels")

    AsyncFunction("getAvailabilityAsync") { () -> [String: Any] in
      let baseUrl = ProcessInfo.processInfo.environment["OPENAGENTS_APPLE_FM_BASE_URL"] ??
        ProcessInfo.processInfo.environment["PROBE_APPLE_FM_BASE_URL"] ??
        "http://127.0.0.1:11435"

      return [
        "status": "blocked",
        "summary": "Apple FM bridge requires local helper health proof before mobile inference.",
        "blockerRefs": [
          "blocker.khala_mobile.apple_fm_bridge_health_unproven",
          "bridge.\(baseUrl)"
        ]
      ]
    }
  }
}
