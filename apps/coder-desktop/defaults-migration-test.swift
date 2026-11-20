import Foundation

@main
struct DefaultsMigrationTest {
    static func assert(_ cond: Bool, _ msg: String) {
        if !cond {
            fputs("FAIL: \(msg)\n", stderr)
            exit(1)
        }
    }

    static func main() {
        let suffix = UUID().uuidString
        let srcSuite = "com.openagents.test.coder.\(suffix)"
        let dstSuite = "com.openagents.test.desktop.\(suffix)"
        defer {
            UserDefaults.standard.removePersistentDomain(forName: srcSuite)
            UserDefaults.standard.removePersistentDomain(forName: dstSuite)
        }

        guard let src = UserDefaults(suiteName: srcSuite),
              let dst = UserDefaults(suiteName: dstSuite) else {
            fputs("FAIL: could not create test UserDefaults suites\n", stderr)
            exit(1)
        }

        // Populate legacy com.openagents.coder settings, including tailnetPairing.
        src.set(false, forKey: "chatWorktrees")
        src.set(2, forKey: "foremanTier")
        src.set(true, forKey: "foremanOperatorTierRaise")
        src.set(false, forKey: "tailnetPairing")
        src.set("/tmp/my-repo", forKey: "folder")
        src.set(1, forKey: "zoom")
        src.set(Float(240.0), forKey: "sideWidth")
        src.set(Float(420.0), forKey: "paneWidth")
        src.set(true, forKey: "followUpsQueueFirst")
        src.set("anthropic/claude-sonnet-4", forKey: "openRouterMain")
        src.set("deepseek/deepseek-v4.1-flash-1017", forKey: "openRouterSub")
        src.set("customValue", forKey: "extraLegacyPref")
        src.synchronize()

        // Pre-populate one key in destination to verify it is NOT overwritten.
        dst.set("/tmp/already-set", forKey: "folder")
        dst.synchronize()

        let count = DefaultsMigration.migrateIfNeeded(sourceSuite: srcSuite, destination: dst)
        assert(count >= 11, "expected >= 11 migrated keys, got \(count)")
        assert(dst.bool(forKey: DefaultsMigration.sentinelKey) == true, "sentinel not set")
        assert(dst.object(forKey: "chatWorktrees") as? Bool == false, "chatWorktrees not migrated")
        assert(dst.integer(forKey: "foremanTier") == 2, "foremanTier not migrated")
        assert(dst.bool(forKey: "foremanOperatorTierRaise") == true, "foremanOperatorTierRaise not migrated")
        assert(dst.object(forKey: "tailnetPairing") as? Bool == false, "tailnetPairing not migrated")
        assert(dst.string(forKey: "folder") == "/tmp/already-set", "existing destination key was overwritten")
        assert(dst.integer(forKey: "zoom") == 1, "zoom not migrated")
        assert(dst.float(forKey: "sideWidth") == 240.0, "sideWidth not migrated")
        assert(dst.float(forKey: "paneWidth") == 420.0, "paneWidth not migrated")
        assert(dst.bool(forKey: "followUpsQueueFirst") == true, "followUpsQueueFirst not migrated")
        assert(dst.string(forKey: "openRouterMain") == "anthropic/claude-sonnet-4", "openRouterMain not migrated")
        assert(dst.string(forKey: "openRouterSub") == "deepseek/deepseek-v4.1-flash-1017", "openRouterSub not migrated")
        assert(dst.string(forKey: "extraLegacyPref") == "customValue", "extraLegacyPref not migrated")

        // Second run must be a no-op because sentinel is set.
        src.set("newValue", forKey: "laterKey")
        src.synchronize()
        let second = DefaultsMigration.migrateIfNeeded(sourceSuite: srcSuite, destination: dst)
        assert(second == 0, "expected 0 on second run, got \(second)")
        assert(dst.object(forKey: "laterKey") == nil, "second run migrated after sentinel")

        print("defaults-migration-test: ok")
    }
}
