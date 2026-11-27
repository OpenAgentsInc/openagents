// One-time UserDefaults migration from com.openagents.coder to com.openagents.desktop (#13784).
import Foundation

enum DefaultsMigration {
    static let legacySuite = "com.openagents.coder"
    static let sentinelKey = "migratedFromCoderDefaultsV1"

    /// Known keys from Coder Desktop checked explicitly in addition to
    /// every key present in the legacy persistent domain.
    static let knownKeys: [String] = [
        "chatWorktrees",
        "foremanTier",
        "foremanOperatorTierRaise",
        "folder",
        "zoom",
        "sideWidth",
        "paneWidth",
        "followUpsQueueFirst",
        "openRouterMain",
        "openRouterSub",
    ]

    /// Copies every non-nil setting from `sourceSuite` into `destination` if not already set,
    /// guarded by `sentinelKey` so it runs once.
    @discardableResult
    static func migrateIfNeeded(
        sourceSuite: String = legacySuite,
        destination: UserDefaults = .standard
    ) -> Int {
        if destination.bool(forKey: sentinelKey) { return 0 }
        var migrated = 0
        if let source = UserDefaults(suiteName: sourceSuite) {
            var values: [String: Any] = [:]
            if let domain = source.persistentDomain(forName: sourceSuite) {
                for (key, value) in domain {
                    if key == sentinelKey || key.hasPrefix("NS") || key.hasPrefix("Apple") || key.hasPrefix("com.apple.") {
                        continue
                    }
                    values[key] = value
                }
            }
            for key in knownKeys where values[key] == nil {
                if let value = source.object(forKey: key) {
                    values[key] = value
                }
            }
            for (key, value) in values {
                if destination.object(forKey: key) == nil {
                    destination.set(value, forKey: key)
                    migrated += 1
                }
            }
        }
        destination.set(true, forKey: sentinelKey)
        return migrated
    }
}
