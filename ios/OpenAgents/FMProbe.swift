import Foundation
#if canImport(FoundationModels)
import FoundationModels
#endif

enum FMProbe {
    static func logAvailability() {
        #if canImport(FoundationModels)
        if #available(iOS 26.0, macOS 26.0, * ) {
            let model = SystemLanguageModel.default
            print("[FM] availability=\(String(describing: model.availability))")
        } else {
            print("[FM] not available: requires OS 26+")
        }
        #else
        print("[FM] FoundationModels module not found in SDK")
        #endif
    }
}

