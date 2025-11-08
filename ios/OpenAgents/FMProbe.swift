import Foundation
import OpenAgentsCore
#if canImport(FoundationModels)
import FoundationModels
#endif

enum FMProbe {
    static func logAvailability() {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 15.0, * ) {
            let model = SystemLanguageModel.default
            OpenAgentsLog.app.debug("FM availability=\(String(describing: model.availability))")
        } else {
            OpenAgentsLog.app.debug("FM not available: requires OS 26+")
        }
        #else
        OpenAgentsLog.app.debug("FM FoundationModels module not found in SDK")
        #endif
    }
}
