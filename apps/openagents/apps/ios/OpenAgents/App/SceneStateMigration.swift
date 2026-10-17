import Foundation

/// One-time removal of window-scene state left by an earlier app on this
/// bundle ID. Runs before `UIApplicationMain`, so UIKit never reads it.
enum SceneStateMigration {
    static let doneKey = "openagents.sceneState.uikitV1"

    static func discardInheritedState(
        defaults: UserDefaults = .standard,
        fileManager: FileManager = .default,
        bundleID: String? = Bundle.main.bundleIdentifier
    ) {
        guard !defaults.bool(forKey: doneKey), let bundleID else { return }
        if let library = fileManager.urls(for: .libraryDirectory, in: .userDomainMask).first {
            let saved = library
                .appendingPathComponent("Saved Application State", isDirectory: true)
                .appendingPathComponent("\(bundleID).savedState", isDirectory: true)
            if fileManager.fileExists(atPath: saved.path) {
                try? fileManager.removeItem(at: saved)
            }
        }
        defaults.set(true, forKey: doneKey)
    }
}
