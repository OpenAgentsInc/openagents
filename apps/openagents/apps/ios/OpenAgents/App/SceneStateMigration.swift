import Foundation

/// One-time removal of window-scene state left by an earlier app on this
/// bundle ID. Runs before `UIApplicationMain`, so UIKit never reads it.
enum SceneStateMigration {
    static let doneKey = "openagents.sceneState.uikitV1"

    static let variantKey = "openagents.sceneState.variant"

    /// Replaces the old app and the modal Grid experiment with root navigation.
    static let variant = "grid-only-root"

    static func discardInheritedState(
        defaults: UserDefaults = .standard,
        fileManager: FileManager = .default,
        bundleID: String? = Bundle.main.bundleIdentifier
    ) {
        guard let bundleID else { return }
        // The Grid-only experiment shares this bundle ID. iOS reconnects a
        // saved scene with its archived delegate, so after switching between
        // it and the app the old variant's screen would come back; discard
        // the saved scene state whenever the variant changes.
        let changed = (defaults.string(forKey: variantKey) ?? "app") != variant
        guard changed || !defaults.bool(forKey: doneKey) else { return }
        if let library = fileManager.urls(for: .libraryDirectory, in: .userDomainMask).first {
            let saved = library
                .appendingPathComponent("Saved Application State", isDirectory: true)
                .appendingPathComponent("\(bundleID).savedState", isDirectory: true)
            if fileManager.fileExists(atPath: saved.path) {
                try? fileManager.removeItem(at: saved)
            }
        }
        defaults.set(true, forKey: doneKey)
        defaults.set(variant, forKey: variantKey)
    }
}
