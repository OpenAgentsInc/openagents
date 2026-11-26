// Firm reason: 6 (standalone Swift verification check for Updater.swift: consecutive update
// identifiers, downgrade protection in LocalStagedUpdateDetector, and WorkerVersionSync).
import Foundation

func check(_ cond: @autoclosure () -> Bool, _ msg: String) {
    if !cond() {
        fputs("FAIL: \(msg)\n", stderr)
        exit(1)
    }
}

func makeAppBundle(
    at appURL: URL,
    bundleID: String = "com.openagents.desktop",
    shortVersion: String,
    build: Int,
    commit: String,
    workers: [String: String]
) throws {
    let fm = FileManager.default
    let resDir = appURL.appendingPathComponent("Contents/Resources", isDirectory: true)
    try fm.createDirectory(at: resDir, withIntermediateDirectories: true)
    let plist: [String: Any] = [
        "CFBundleIdentifier": bundleID,
        "CFBundleShortVersionString": shortVersion,
        "CFBundleVersion": String(build),
        "OACommit": commit,
    ]
    let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
    try data.write(to: appURL.appendingPathComponent("Contents/Info.plist"))
    for (name, body) in workers {
        let f = resDir.appendingPathComponent(name)
        try Data(body.utf8).write(to: f)
        try fm.setAttributes([.posixPermissions: name.hasSuffix(".metal") ? 0o644 : 0o755], ofItemAtPath: f.path)
    }
}

@main
struct UpdaterSyncCheck {
    static func main() throws {
        // 1. UpdateIdentifier formatting (version.build.hash7 + fallbacks)
        check(
            UpdateIdentifier.format(shortVersion: "0.1.0", buildNumber: 412, commit: "5227b8ccbde4ac43") == "0.1.0.412.5227b8c",
            "formats version.build.hash7"
        )
        check(
            UpdateIdentifier.format(shortVersion: "v1.0.0", buildNumber: 34395, commit: "ABCDEF1234567890") == "1.0.0.34395.abcdef1",
            "strips leading v and lowercases short hash"
        )
        check(
            UpdateIdentifier.format(shortVersion: "1.0.0", buildNumber: 34395, commit: "") == "1.0.0.34395",
            "omits empty commit cleanly"
        )
        check(
            UpdateIdentifier.format(shortVersion: "1.0.0", buildNumber: 0, commit: "5277b8ccbd") == "1.0.0.5277b8c",
            "omits zero build cleanly"
        )
        check(
            UpdateIdentifier.format(shortVersion: "1.0.0", buildNumber: 0, commit: "") == "1.0.0",
            "falls back to bare semver"
        )
        let manifest = UpdateManifest(
            schema: nil,
            version: "1.2.3",
            build: 412,
            commit: "5277b8ccbd",
            bundleID: "com.openagents.desktop",
            teamID: "HQWSG26L43",
            url: "https://example.com/OpenAgents.zip",
            sha256: String(repeating: "a", count: 64),
            size: 123,
            notes: nil
        )
        check(UpdateIdentifier.format(manifest: manifest) == "1.2.3.412.5277b8c", "manifest identifier includes build and hash")

        let fm = FileManager.default
        let tmp = fm.temporaryDirectory.appendingPathComponent("oa-updater-sync-\(UUID().uuidString)", isDirectory: true)
        try fm.createDirectory(at: tmp, withIntermediateDirectories: true)
        defer { try? fm.removeItem(at: tmp) }

        let releaseHome = tmp.appendingPathComponent("release", isDirectory: true)
        let runningApp = tmp.appendingPathComponent("OpenAgents.app", isDirectory: true)
        let stagedApp = releaseHome.appendingPathComponent("stage/OpenAgents.app", isDirectory: true)

        try makeAppBundle(
            at: runningApp,
            shortVersion: "1.0.0",
            build: 34395,
            commit: "b42bd4738fb465f587284b145781faae6ec231cf",
            workers: [
                "oa-worker": "oa-worker-v1",
                "agyworker": "agy-v1",
                "gitguard": "gitguard-v1",
                "coder-host": "host-v1",
                "psionic-inkling-serve": "serve-v1",
                "inkling-kernels.metal": "metal-v1",
            ]
        )
        let launchMeta = BundleMetadata.read(from: runningApp)
        check(launchMeta?.updateIdentifier == "1.0.0.34395.b42bd47", "launch metadata updateIdentifier")

        // 2. LocalStagedUpdateDetector emits consecutive version.build.hash7
        try makeAppBundle(
            at: stagedApp,
            shortVersion: "1.0.0",
            build: 34396,
            commit: "5277b8ccbde4ac43000000000000000000000000",
            workers: ["oa-worker": "oa-worker-v2", "coder-host": "host-v2"]
        )
        let detector = LocalStagedUpdateDetector(releaseHome: releaseHome)
        let detected = detector.detect(runningAppURL: runningApp, launchMetadata: launchMeta)
        if case .localStaged(_, let ver, let commit) = detected {
            check(ver == "1.0.0.34396.5277b8c", "staged update uses consecutive identifier, got \(ver)")
            check(commit.hasPrefix("5277b8c"), "staged update preserves full commit")
        } else {
            check(false, "expected localStaged update, got \(String(describing: detected))")
        }

        // 3. Refuse downgrade when staged build is older than running build
        try fm.removeItem(at: stagedApp)
        try makeAppBundle(
            at: stagedApp,
            shortVersion: "1.0.0",
            build: 34394,
            commit: "1111111111111111111111111111111111111111",
            workers: ["oa-worker": "old"]
        )
        check(
            detector.detect(runningAppURL: runningApp, launchMetadata: launchMeta) == nil,
            "refuses downgrade from older staged build"
        )

        // 4. On-disk replacement detection emits consecutive identifier
        try fm.removeItem(at: runningApp)
        try makeAppBundle(
            at: runningApp,
            shortVersion: "1.0.0",
            build: 34397,
            commit: "536b890ca3000000000000000000000000000000",
            workers: [
                "oa-worker": "oa-worker-v3",
                "agyworker": "agy-v3",
                "gitguard": "gitguard-v3",
                "coder-host": "host-v3",
                "psionic-inkling-serve": "serve-v3",
                "inkling-kernels.metal": "metal-v3",
            ]
        )
        let diskDetected = detector.detect(runningAppURL: runningApp, launchMetadata: launchMeta)
        if case .diskAlreadyUpdated(let ver, _) = diskDetected {
            check(ver == "1.0.0.34397.536b890", "disk update uses consecutive identifier, got \(ver)")
        } else {
            check(false, "expected diskAlreadyUpdated, got \(String(describing: diskDetected))")
        }

        // 5. WorkerVersionSync keeps ~/.openagents/bin workers and openagents-environment synced
        let fakeHome = tmp.appendingPathComponent("home", isDirectory: true)
        try fm.createDirectory(at: fakeHome, withIntermediateDirectories: true)
        let syncer = WorkerVersionSync(home: fakeHome)
        let (ver1, updated1) = syncer.syncBinaries(from: runningApp)
        check(ver1 == "1.0.0.34397.536b890", "sync version matches app bundle")
        check(updated1.contains("oa-worker"), "synced oa-worker")
        check(updated1.contains("coder-host"), "synced coder-host")
        check(updated1.contains("agyworker"), "synced agyworker")
        check(updated1.contains("gitguard"), "synced gitguard")
        check(updated1.contains("psionic-inkling-serve"), "synced psionic-inkling-serve")
        check(updated1.contains("inkling-kernels.metal"), "synced inkling-kernels.metal")
        check(updated1.contains("openagents-environment"), "synced openagents-environment")
        check(WorkerVersionSync.installedWorkerVersion(home: fakeHome) == "1.0.0.34397.536b890", "worker version stamp matches")
        let linkTarget = try fm.destinationOfSymbolicLink(atPath: syncer.envLink.path)
        check(linkTarget.hasSuffix("/.openagents/environment/bin/1.0.0.34397.536b890/openagents-environment"), "env symlink points to versioned dir: \(linkTarget)")
        let workerV3 = try String(contentsOf: syncer.binDir.appendingPathComponent("oa-worker"), encoding: .utf8)
        check(workerV3 == "oa-worker-v3", "oa-worker contents match")

        // Idempotent second sync updates nothing
        let (_, updatedAgain) = syncer.syncBinaries(from: runningApp)
        check(updatedAgain.isEmpty, "second sync is idempotent, got \(updatedAgain)")

        // Upgrading app bundle re-syncs changed workers and advances version stamp & symlink
        try fm.removeItem(at: runningApp)
        try makeAppBundle(
            at: runningApp,
            shortVersion: "1.0.0",
            build: 34398,
            commit: "9999999abcdef000000000000000000000000000",
            workers: [
                "oa-worker": "oa-worker-v4",
                "agyworker": "agy-v3",
                "gitguard": "gitguard-v3",
                "coder-host": "host-v4",
                "psionic-inkling-serve": "serve-v3",
                "inkling-kernels.metal": "metal-v3",
            ]
        )
        let (ver2, updated2) = syncer.syncBinaries(from: runningApp)
        check(ver2 == "1.0.0.34398.9999999", "upgraded version matches")
        check(updated2.contains("oa-worker") && updated2.contains("coder-host") && updated2.contains("openagents-environment"), "only changed workers + env updated")
        check(!updated2.contains("agyworker"), "unchanged agyworker skipped")
        check(WorkerVersionSync.installedWorkerVersion(home: fakeHome) == "1.0.0.34398.9999999", "worker version stamp advanced")
        let linkTarget2 = try fm.destinationOfSymbolicLink(atPath: syncer.envLink.path)
        check(linkTarget2.hasSuffix("/.openagents/environment/bin/1.0.0.34398.9999999/openagents-environment"), "env symlink advanced: \(linkTarget2)")
        let workerV4 = try String(contentsOf: syncer.binDir.appendingPathComponent("oa-worker"), encoding: .utf8)
        check(workerV4 == "oa-worker-v4", "oa-worker upgraded to v4")

        print("updater-sync-check: ok")
    }
}
