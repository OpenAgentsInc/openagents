import CryptoKit
import Darwin
import Foundation

@_silgen_name("proc_pidpath")
private func oa_proc_pidpath(_ pid: Int32, _ buffer: UnsafeMutableRawPointer, _ buffersize: UInt32) -> Int32

// MARK: - Signed Release Manifest Types (verified by coax/lib/agent/desktop/update.coax)

/// Outer signed envelope fetched from GitHub Releases (or CODER_UPDATE_URL).
/// The Ed25519 signature is verified over the raw decoded `payload` bytes BEFORE
/// `payload` is ever passed to JSONDecoder.
struct UpdateEnvelope: Decodable {
    let schema: String?
    let key: String
    let payload: String
    let signature: String
}

/// Inner update manifest decoded only AFTER Ed25519 verification succeeds.
struct UpdateManifest: Codable, Equatable {
    let schema: String?
    let version: String
    let build: Int?
    let commit: String?
    let bundleID: String
    let teamID: String
    let url: String
    let sha256: String
    let size: Int64
    let notes: String?

    init(
        schema: String?,
        version: String,
        build: Int?,
        commit: String? = nil,
        bundleID: String,
        teamID: String,
        url: String,
        sha256: String,
        size: Int64,
        notes: String?
    ) {
        self.schema = schema
        self.version = version
        self.build = build
        self.commit = commit
        self.bundleID = bundleID
        self.teamID = teamID
        self.url = url
        self.sha256 = sha256
        self.size = size
        self.notes = notes
    }

    enum CodingKeys: String, CodingKey {
        case schema
        case version
        case build
        case commit
        case bundleID = "bundle_id"
        case teamID = "team_id"
        case url
        case sha256
        case size
        case notes
    }
}

/// Formats human-ordered desktop update identifiers combining version, consecutive build number,
/// and short commit hash (e.g. `1.0.0.34395.5277b8c`).
enum UpdateIdentifier {
    static func format(shortVersion: String, buildNumber: Int, commit: String) -> String {
        var ver = shortVersion.trimmingCharacters(in: .whitespacesAndNewlines)
        if ver.hasPrefix("v") || ver.hasPrefix("V") { ver.removeFirst() }
        if ver.isEmpty { ver = "0.0.0" }
        let shortHash = String(commit.trimmingCharacters(in: .whitespacesAndNewlines).lowercased().prefix(7))
        var parts: [String] = [ver]
        if buildNumber > 0 { parts.append(String(buildNumber)) }
        if !shortHash.isEmpty { parts.append(shortHash) }
        return parts.joined(separator: ".")
    }

    static func format(metadata: BundleMetadata) -> String {
        format(shortVersion: metadata.shortVersion, buildNumber: metadata.buildNumber, commit: metadata.commit)
    }

    static func format(manifest: UpdateManifest) -> String {
        format(shortVersion: manifest.version, buildNumber: manifest.build ?? 0, commit: manifest.commit ?? "")
    }
}

/// Semantic version helper supporting major.minor.patch[-prerelease].
struct SemVer: Comparable, Equatable, CustomStringConvertible {
    let major: Int
    let minor: Int
    let patch: Int
    let prerelease: String?

    init?(_ raw: String) {
        var s = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        if s.hasPrefix("v") || s.hasPrefix("V") { s.removeFirst() }
        let plusParts = s.split(separator: "+", maxSplits: 1, omittingEmptySubsequences: false)
        let mainAndPre = String(plusParts[0]).split(separator: "-", maxSplits: 1, omittingEmptySubsequences: false)
        let core = mainAndPre[0].split(separator: ".", omittingEmptySubsequences: false)
        guard core.count >= 2, core.count <= 3,
              let maj = Int(core[0]), maj >= 0,
              let min = Int(core[1]), min >= 0 else {
            return nil
        }
        let pat: Int
        if core.count == 3 {
            guard let p = Int(core[2]), p >= 0 else { return nil }
            pat = p
        } else {
            pat = 0
        }
        self.major = maj
        self.minor = min
        self.patch = pat
        self.prerelease = mainAndPre.count == 2 && !mainAndPre[1].isEmpty ? String(mainAndPre[1]) : nil
    }

    var description: String {
        let base = "\(major).\(minor).\(patch)"
        if let pre = prerelease { return "\(base)-\(pre)" }
        return base
    }

    static func < (lhs: SemVer, rhs: SemVer) -> Bool {
        DesktopCore.withCoaxString(lhs.description) { a in
            DesktopCore.withCoaxString(rhs.description) { b in
                oa_dc_semver_cmp(a, b) < 0
            }
        }
    }
}

enum UpdaterError: LocalizedError, Equatable {
    case invalidEnvelope(String)
    case untrustedKey(String)
    case invalidPublicKey(String)
    case invalidBase64(String)
    case invalidSignature
    case invalidManifest(String)
    case unsupportedSchema(String)
    case bundleIDMismatch(expected: String, actual: String)
    case teamIDMismatch(expected: String, actual: String)
    case versionNotNewer(current: String, candidate: String)
    case unsafeDownloadURL(String)
    case downloadFailed(String)
    case sizeMismatch(expected: Int64, actual: Int64)
    case sha256Mismatch(expected: String, actual: String)
    case unpackFailed(String)
    case appBundleNotFound
    case codesignFailed(String)
    case swapFailed(String)
    case notWritable(String)

    var errorDescription: String? {
        switch self {
        case .invalidEnvelope(let msg): return "Invalid update envelope: \(msg)"
        case .untrustedKey(let k): return "Untrusted update signing key: \(k)"
        case .invalidPublicKey(let k): return "Malformed trusted public key: \(k)"
        case .invalidBase64(let f): return "Invalid base64 in \(f)"
        case .invalidSignature: return "Update manifest signature verification failed"
        case .invalidManifest(let msg): return "Invalid update manifest: \(msg)"
        case .unsupportedSchema(let s): return "Unsupported manifest schema: \(s)"
        case .bundleIDMismatch(let e, let a): return "Bundle ID mismatch (expected \(e), got \(a))"
        case .teamIDMismatch(let e, let a): return "Team ID mismatch (expected \(e), got \(a))"
        case .versionNotNewer(let c, let n): return "Update \(n) is not newer than \(c)"
        case .unsafeDownloadURL(let u): return "Refused unsafe update URL: \(u)"
        case .downloadFailed(let msg): return "Download failed: \(msg)"
        case .sizeMismatch(let e, let a): return "Archive size mismatch (expected \(e), got \(a))"
        case .sha256Mismatch(let e, let a): return "Archive SHA-256 mismatch (expected \(e), got \(a))"
        case .unpackFailed(let msg): return "Failed to unpack update: \(msg)"
        case .appBundleNotFound: return "No .app bundle found inside update archive"
        case .codesignFailed(let msg): return "Code signature verification failed: \(msg)"
        case .swapFailed(let msg): return "Failed to install update: \(msg)"
        case .notWritable(let path): return "Cannot update read-only bundle at \(path)"
        }
    }
}

// MARK: - Updater UI State & Source

enum UpdateSource: Equatable {
    case signedRelease(UpdateManifest)
    case localStaged(appPath: String, version: String, commit: String)
    case diskAlreadyUpdated(version: String, commit: String)
}

enum UpdaterState: Equatable {
    case hidden
    case available(version: String, source: UpdateSource)
    case downloading(version: String, progress: Int)
    case ready(version: String, stagedAppPath: String?, source: UpdateSource)
    case error(message: String, retryable: Bool)

    /// Maps directly to Coax CP_UPD_* constants in page.coax.
    var coaxCode: Int32 {
        switch self {
        case .hidden: return 0
        case .available: return 1
        case .downloading: return 2
        case .ready: return 3
        case .error: return 4
        }
    }

    var pillLabel: String {
        switch self {
        case .hidden: return ""
        case .available: return "Update available"
        case .downloading(_, let pct): return pct >= 100 ? "Verifying update" : "Downloading update"
        case .ready: return "Restart to update"
        case .error: return "Update failed"
        }
    }

    var pillDetail: String {
        switch self {
        case .hidden: return ""
        case .available(let ver, _): return ver
        case .downloading(_, let pct): return pct > 0 && pct < 100 ? "\(pct)%" : ""
        case .ready(let ver, _, _): return ver
        case .error: return "Retry"
        }
    }
}

// MARK: - Manifest Verifier (Pure, Testable)

struct UpdateVerifier {
    static let pinnedBundleID = "com.openagents.desktop"
    static let pinnedTeamID = "HQWSG26L43"

    /// Compiled-in Ed25519 release signing public keys (key ID -> hex raw 32-byte public key).
    static let defaultTrustedKeys: [String: String] = [
        "openagents-desktop-2026-v1": "9d61b19deffd5a60ba844af492ec2cc44449c5697b326919703bac031cae7f60"
    ]

    let trustedKeys: [String: Data]
    let pinnedBundleID: String
    let pinnedTeamID: String

    init(
        trustedKeys: [String: Data]? = nil,
        pinnedBundleID: String = UpdateVerifier.pinnedBundleID,
        pinnedTeamID: String = UpdateVerifier.pinnedTeamID
    ) {
        if let custom = trustedKeys {
            self.trustedKeys = custom
        } else {
            var map: [String: Data] = [:]
            for (k, raw) in UpdateVerifier.defaultTrustedKeys {
                if let decoded = UpdateVerifier.decodeKeyMaterial(raw) {
                    map[k] = decoded
                }
            }
            self.trustedKeys = map
        }
        self.pinnedBundleID = pinnedBundleID
        self.pinnedTeamID = pinnedTeamID
    }

    static func decodeKeyMaterial(_ text: String) -> Data? {
        let trimmed = text.trimmingCharacters(in: .whitespacesAndNewlines)
        if trimmed.count == 64, let hex = Data(hexString: trimmed), hex.count == 32 {
            return hex
        }
        if let b64 = Data(base64Encoded: trimmed), b64.count == 32 {
            return b64
        }
        return nil
    }

    /// Verifies the signed envelope via coax/lib/agent/desktop/update.coax BEFORE returning a manifest.
    func verifyAndDecode(
        envelopeData: Data,
        currentVersion: String,
        currentBuild: Int = 0,
        allowFileURL: Bool = false
    ) throws -> UpdateManifest {
        let sortedKeys = trustedKeys.sorted { $0.key < $1.key }
        let keyIDs = sortedKeys.map { $0.key }.joined(separator: "\n")
        let keyHexes = sortedKeys.map { $0.value.map { String(format: "%02x", $0) }.joined() }.joined(separator: "\n")

        var outVersion = [UInt8](repeating: 0, count: 128)
        var outBuild: [Int64] = [0]
        var outHasBuild: [Int64] = [0]
        var outBundle = [UInt8](repeating: 0, count: 256)
        var outTeam = [UInt8](repeating: 0, count: 64)
        var outURL = [UInt8](repeating: 0, count: 2048)
        var outSHA = [UInt8](repeating: 0, count: 128)
        var outSize: [Int64] = [0]
        var outNotes = [UInt8](repeating: 0, count: 4096)
        var outErr = [UInt8](repeating: 0, count: 512)

        let ok: Int64 = DesktopCore.withCoaxData(envelopeData) { env in
            DesktopCore.withCoaxString(keyIDs) { kids in
                DesktopCore.withCoaxString(keyHexes) { khex in
                    DesktopCore.withCoaxString(currentVersion) { cur in
                        DesktopCore.withMutableBytes(&outVersion) { vb in
                            DesktopCore.withMutableS64(&outBuild) { bb in
                                DesktopCore.withMutableS64(&outHasBuild) { hbb in
                                    DesktopCore.withMutableBytes(&outBundle) { bunb in
                                        DesktopCore.withMutableBytes(&outTeam) { tb in
                                            DesktopCore.withMutableBytes(&outURL) { ub in
                                                DesktopCore.withMutableBytes(&outSHA) { shab in
                                                    DesktopCore.withMutableS64(&outSize) { szb in
                                                        DesktopCore.withMutableBytes(&outNotes) { nb in
                                                            DesktopCore.withMutableBytes(&outErr) { eb in
                                                                oa_dc_update_verify(
                                                                    env, kids, khex, cur, Int64(currentBuild),
                                                                    allowFileURL ? 1 : 0,
                                                                    vb, bb, hbb, bunb, tb, ub, shab, szb, nb, eb
                                                                )
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        guard ok == 1 else {
            let err = DesktopCore.cString(from: outErr)
            if err.contains("signature") { throw UpdaterError.invalidSignature }
            if err.contains("untrusted") || err.contains("unknown key") { throw UpdaterError.untrustedKey(err) }
            if err.contains("schema") { throw UpdaterError.unsupportedSchema(err) }
            if err.contains("downgrade") || err.contains("older") || err.contains("not newer") {
                throw UpdaterError.versionNotNewer(current: currentVersion, candidate: err)
            }
            if err.contains("url") || err.contains("https") { throw UpdaterError.unsafeDownloadURL(err) }
            throw UpdaterError.invalidManifest(err.isEmpty ? "update verification failed" : err)
        }

        let version = DesktopCore.cString(from: outVersion)
        let build: Int? = outHasBuild[0] != 0 ? Int(outBuild[0]) : nil
        let bundle = DesktopCore.cString(from: outBundle)
        let team = DesktopCore.cString(from: outTeam)
        let url = DesktopCore.cString(from: outURL)
        let sha = DesktopCore.cString(from: outSHA)
        let notes = DesktopCore.cString(from: outNotes)

        if !pinnedBundleID.isEmpty && bundle != pinnedBundleID {
            throw UpdaterError.bundleIDMismatch(expected: pinnedBundleID, actual: bundle)
        }
        if !pinnedTeamID.isEmpty && !team.isEmpty && team != pinnedTeamID {
            throw UpdaterError.teamIDMismatch(expected: pinnedTeamID, actual: team)
        }

        return UpdateManifest(
            schema: nil,
            version: version,
            build: build,
            bundleID: bundle,
            teamID: team,
            url: url,
            sha256: sha,
            size: outSize[0],
            notes: notes.isEmpty ? nil : notes
        )
    }
}

struct UpdateInstaller {
    let pinnedBundleID: String
    let pinnedTeamID: String
    let requireTeamID: Bool

    init(
        pinnedBundleID: String = UpdateVerifier.pinnedBundleID,
        pinnedTeamID: String = UpdateVerifier.pinnedTeamID,
        requireTeamID: Bool = true
    ) {
        self.pinnedBundleID = pinnedBundleID
        self.pinnedTeamID = pinnedTeamID
        self.requireTeamID = requireTeamID
    }

    /// Verifies exact byte size and SHA-256 digest of a downloaded archive via coax/lib/agent/desktop/update.coax.
    func verifyArchive(at url: URL, expectedSize: Int64, expectedSHA256: String) throws {
        var outErr = [UInt8](repeating: 0, count: 512)
        let ok: Int64 = DesktopCore.withCoaxString(url.path) { p in
            DesktopCore.withCoaxString(expectedSHA256) { sha in
                DesktopCore.withMutableBytes(&outErr) { eb in
                    oa_dc_update_archive(p, expectedSize, sha, eb)
                }
            }
        }
        guard ok == 1 else {
            let msg = DesktopCore.cString(from: outErr)
            if msg.contains("size") {
                let actual = (try? FileManager.default.attributesOfItem(atPath: url.path)[.size] as? NSNumber)?.int64Value ?? -1
                throw UpdaterError.sizeMismatch(expected: expectedSize, actual: actual)
            }
            throw UpdaterError.sha256Mismatch(expected: expectedSHA256.lowercased(), actual: msg)
        }
    }

    /// Unpacks a zip archive using `ditto -x -k` and returns the extracted `.app` bundle URL.
    func unpackZip(at zipURL: URL, into directory: URL) throws -> URL {
        let fm = FileManager.default
        try? fm.removeItem(at: directory)
        try fm.createDirectory(at: directory, withIntermediateDirectories: true)
        let res = runTool("/usr/bin/ditto", arguments: ["-x", "-k", zipURL.path, directory.path])
        guard res.exitCode == 0 else {
            throw UpdaterError.unpackFailed(res.stderr.isEmpty ? "ditto exited \(res.exitCode)" : res.stderr)
        }
        guard let appURL = findAppBundle(in: directory) else {
            throw UpdaterError.appBundleNotFound
        }
        return appURL
    }

    /// Verifies the unpacked `.app` bundle's Info.plist bundle ID and `codesign` validity.
    func verifyAppBundle(at appURL: URL, expectedVersion: String? = nil) throws {
        let plistURL = appURL.appendingPathComponent("Contents/Info.plist")
        guard let data = try? Data(contentsOf: plistURL),
              let dict = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else {
            throw UpdaterError.codesignFailed("Missing or unreadable Contents/Info.plist")
        }
        let bundleID = (dict["CFBundleIdentifier"] as? String) ?? ""
        guard bundleID == pinnedBundleID else {
            throw UpdaterError.bundleIDMismatch(expected: pinnedBundleID, actual: bundleID)
        }
        if let expectedVersion {
            let actualVer = (dict["CFBundleShortVersionString"] as? String) ?? ""
            if let exp = SemVer(expectedVersion), let act = SemVer(actualVer), exp != act {
                throw UpdaterError.invalidManifest("App bundle version \(actualVer) does not match manifest \(expectedVersion)")
            }
        }
        let verify = runTool("/usr/bin/codesign", arguments: ["--verify", "--deep", "--strict", appURL.path])
        guard verify.exitCode == 0 else {
            throw UpdaterError.codesignFailed(verify.stderr.isEmpty ? "codesign verify exited \(verify.exitCode)" : verify.stderr)
        }
        if requireTeamID {
            let info = runTool("/usr/bin/codesign", arguments: ["-dvv", appURL.path])
            let combined = info.stdout + "\n" + info.stderr
            let teamID = parseTeamIdentifier(from: combined)
            guard teamID == pinnedTeamID else {
                throw UpdaterError.teamIDMismatch(expected: pinnedTeamID, actual: teamID ?? "none")
            }
        }
    }

    /// Atomically swaps `stagedApp` into `targetApp` by staging alongside `targetApp` and renaming.
    /// Rolls back to the previous bundle if replacement fails.
    func atomicSwap(stagedApp: URL, targetApp: URL) throws {
        let fm = FileManager.default
        let parent = targetApp.deletingLastPathComponent()
        guard fm.isWritableFile(atPath: parent.path) else {
            throw UpdaterError.notWritable(parent.path)
        }
        let name = targetApp.lastPathComponent
        let stagingCopy = parent.appendingPathComponent(".\(name).new.\(UUID().uuidString.prefix(8))")
        let backup = parent.appendingPathComponent(".\(name).old.\(UUID().uuidString.prefix(8))")
        defer { try? fm.removeItem(at: stagingCopy) }

        let copyRes = runTool("/usr/bin/ditto", arguments: [stagedApp.path, stagingCopy.path])
        guard copyRes.exitCode == 0 else {
            throw UpdaterError.swapFailed(copyRes.stderr.isEmpty ? "ditto copy exited \(copyRes.exitCode)" : copyRes.stderr)
        }
        let hadExisting = fm.fileExists(atPath: targetApp.path)
        do {
            if hadExisting {
                try fm.moveItem(at: targetApp, to: backup)
            }
            do {
                try fm.moveItem(at: stagingCopy, to: targetApp)
            } catch {
                if hadExisting {
                    try? fm.moveItem(at: backup, to: targetApp)
                }
                throw UpdaterError.swapFailed(error.localizedDescription)
            }
            if hadExisting {
                try? fm.removeItem(at: backup)
            }
        } catch let e as UpdaterError {
            throw e
        } catch {
            throw UpdaterError.swapFailed(error.localizedDescription)
        }
        _ = runTool("/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister", arguments: ["-f", targetApp.path])
    }

    /// Spawns a detached helper process that waits for the current PID to exit and opens the updated app.
    func scheduleRelaunch(appURL: URL, pid: Int32 = getpid()) throws {
        let script = "while kill -0 $1 2>/dev/null; do sleep 0.1; done; /usr/bin/open -n \"$2\""
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: "/bin/sh")
        proc.arguments = ["-c", script, "_", String(pid), appURL.path]
        proc.standardInput = FileHandle.nullDevice
        proc.standardOutput = FileHandle.nullDevice
        proc.standardError = FileHandle.nullDevice
        try proc.run()
    }

    private func findAppBundle(in directory: URL) -> URL? {
        let fm = FileManager.default
        guard let items = try? fm.contentsOfDirectory(at: directory, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]) else {
            return nil
        }
        if let direct = items.first(where: { $0.pathExtension == "app" }) { return direct }
        for item in items {
            var isDir: ObjCBool = false
            if fm.fileExists(atPath: item.path, isDirectory: &isDir), isDir.boolValue {
                if let sub = try? fm.contentsOfDirectory(at: item, includingPropertiesForKeys: nil, options: [.skipsHiddenFiles]),
                   let nested = sub.first(where: { $0.pathExtension == "app" }) {
                    return nested
                }
            }
        }
        return nil
    }

    private func parseTeamIdentifier(from output: String) -> String? {
        for line in output.split(separator: "\n") {
            if line.hasPrefix("TeamIdentifier=") {
                let value = String(line.dropFirst("TeamIdentifier=".count)).trimmingCharacters(in: .whitespacesAndNewlines)
                if !value.isEmpty && value != "not set" { return value }
            }
        }
        return nil
    }

    private func runTool(_ path: String, arguments: [String]) -> (exitCode: Int32, stdout: String, stderr: String) {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: path)
        proc.arguments = arguments
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        do {
            try proc.run()
            proc.waitUntilExit()
        } catch {
            return (-1, "", error.localizedDescription)
        }
        let out = String(data: outPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let err = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        return (proc.terminationStatus, out, err)
    }
}

// MARK: - Local Dev / install.sh Staged Build Detector

struct BundleMetadata: Equatable {
    let bundleID: String
    let shortVersion: String
    let buildNumber: Int
    let commit: String

    /// Consecutive human-ordered identifier, e.g. `1.0.0.34395.5277b8c`.
    var updateIdentifier: String {
        UpdateIdentifier.format(metadata: self)
    }

    static var currentUpdateIdentifier: String {
        if let meta = read(from: Bundle.main.bundleURL) {
            return meta.updateIdentifier
        }
        let info = Bundle.main.infoDictionary
        let shortVer = (info?["CFBundleShortVersionString"] as? String) ?? "1.0.0"
        let buildNum = Int((info?["CFBundleVersion"] as? String) ?? "0") ?? 0
        let commit = (info?["OACommit"] as? String) ?? ""
        return UpdateIdentifier.format(shortVersion: shortVer, buildNumber: buildNum, commit: commit)
    }

    static func read(from appURL: URL) -> BundleMetadata? {
        let plistURL = appURL.appendingPathComponent("Contents/Info.plist")
        guard let data = try? Data(contentsOf: plistURL),
              let dict = try? PropertyListSerialization.propertyList(from: data, format: nil) as? [String: Any] else {
            return nil
        }
        let bundleID = (dict["CFBundleIdentifier"] as? String) ?? ""
        let shortVersion = (dict["CFBundleShortVersionString"] as? String) ?? "0.0.0"
        let buildStr = (dict["CFBundleVersion"] as? String) ?? "0"
        let buildNum = Int(buildStr) ?? 0
        let commit = (dict["OACommit"] as? String) ?? ""
        return BundleMetadata(bundleID: bundleID, shortVersion: shortVersion, buildNumber: buildNum, commit: commit)
    }
}

struct LocalStagedUpdateDetector {
    let releaseHome: URL
    let pinnedBundleID: String

    init(
        releaseHome: URL = LocalStagedUpdateDetector.defaultReleaseHome(),
        pinnedBundleID: String = UpdateVerifier.pinnedBundleID
    ) {
        self.releaseHome = releaseHome
        self.pinnedBundleID = pinnedBundleID
    }

    static func defaultReleaseHome() -> URL {
        if let env = ProcessInfo.processInfo.environment["CODER_RELEASE_HOME"], !env.isEmpty {
            return URL(fileURLWithPath: env)
        }
        return FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/openagents/coder-release", isDirectory: true)
    }

    /// Checks both the `install.sh` stage directory (`$CODER_RELEASE_HOME/stage/OpenAgents.app`)
    /// and whether the running `.app` on disk was already replaced with a newer build/commit.
    func detect(runningAppURL: URL, launchMetadata: BundleMetadata?) -> UpdateSource? {
        // 1. Check if the running app on disk was already swapped out from under us.
        if let launch = launchMetadata, let disk = BundleMetadata.read(from: runningAppURL),
           disk.bundleID == pinnedBundleID {
            let commitChanged = !disk.commit.isEmpty && !launch.commit.isEmpty && disk.commit != launch.commit
            let buildBumped = disk.buildNumber > launch.buildNumber && launch.buildNumber > 0
            if commitChanged || buildBumped {
                return .diskAlreadyUpdated(version: disk.updateIdentifier, commit: disk.commit)
            }
        }

        // 2. Check install.sh staged build at ~/.cache/openagents/coder-release/stage/OpenAgents.app
        let stagedApp = releaseHome.appendingPathComponent("stage/OpenAgents.app", isDirectory: true)
        guard FileManager.default.fileExists(atPath: stagedApp.path),
              let staged = BundleMetadata.read(from: stagedApp),
              staged.bundleID == pinnedBundleID else {
            return nil
        }
        if let launch = launchMetadata {
            if !staged.commit.isEmpty && staged.commit == launch.commit {
                return nil
            }
            // Refuse to downgrade when both consecutive build numbers are known and staged is older.
            if staged.buildNumber > 0 && launch.buildNumber > 0 && staged.buildNumber < launch.buildNumber {
                return nil
            }
            if staged.commit.isEmpty && staged.buildNumber <= launch.buildNumber {
                return nil
            }
        }
        return .localStaged(appPath: stagedApp.path, version: staged.updateIdentifier, commit: staged.commit)
    }
}

// MARK: - Worker & Resident Daemon Version Sync

/// Keeps `~/.openagents/bin/` worker binaries, `~/.openagents/bin/openagents-environment`, and the
/// running resident `coder-host` daemon strictly synchronized with the installed `OpenAgents.app`
/// version identifier (`{version}.{build}.{shortHash}`).
struct WorkerVersionSync {
    struct SyncReport: Equatable {
        let versionID: String
        let updatedFiles: [String]
        let daemonSynced: Bool
        let daemonPendingBusy: Bool
    }

    /// Bundled worker & runtime artifacts in `Contents/Resources/` to keep synced in `~/.openagents/bin/`.
    static let bundledArtifacts: [(name: String, executable: Bool)] = [
        ("oa-worker", true),
        ("agyworker", true),
        ("gitguard", true),
        ("coder-host", true),
        ("psionic-inkling-serve", true),
        ("inkling-kernels.metal", false),
        ("claudeworker", true),
        ("oasim", true),
        ("ple-run", true),
        ("lean-run", true),
        ("tbench-run", true),
        ("gbench-run", true),
    ]

    let home: URL
    let envLabel: String

    init(
        home: URL = FileManager.default.homeDirectoryForCurrentUser,
        envLabel: String = ProcessInfo.processInfo.environment["CODER_ENV_LABEL"] ?? "com.openagents.coax.environment"
    ) {
        self.home = home
        self.envLabel = envLabel
    }

    var binDir: URL { home.appendingPathComponent(".openagents/bin", isDirectory: true) }
    var envRootDir: URL { home.appendingPathComponent(".openagents/environment", isDirectory: true) }
    var envBinDir: URL { envRootDir.appendingPathComponent("bin", isDirectory: true) }
    var envLink: URL { binDir.appendingPathComponent("openagents-environment") }
    var workerVersionFile: URL { binDir.appendingPathComponent(".worker-version") }
    var envVersionFile: URL { envRootDir.appendingPathComponent("version") }
    var envPlist: URL { home.appendingPathComponent("Library/LaunchAgents/\(envLabel).plist") }

    static func installedWorkerVersion(home: URL = FileManager.default.homeDirectoryForCurrentUser) -> String? {
        let url = home.appendingPathComponent(".openagents/bin/.worker-version")
        guard let raw = try? String(contentsOf: url, encoding: .utf8) else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    /// Atomically syncs all bundled worker binaries and the `openagents-environment` symlink/version
    /// stamps from `appURL/Contents/Resources/` into `~/.openagents/`.
    @discardableResult
    func syncBinaries(from appURL: URL) -> (versionID: String, updatedFiles: [String]) {
        let fm = FileManager.default
        let versionID = BundleMetadata.read(from: appURL)?.updateIdentifier ?? BundleMetadata.currentUpdateIdentifier
        let resourcesDir = appURL.appendingPathComponent("Contents/Resources", isDirectory: true)
        var updated: [String] = []

        try? fm.createDirectory(at: binDir, withIntermediateDirectories: true)
        try? fm.createDirectory(at: envBinDir, withIntermediateDirectories: true)

        for artifact in Self.bundledArtifacts {
            let src = resourcesDir.appendingPathComponent(artifact.name)
            guard fm.fileExists(atPath: src.path) else { continue }
            let dst = binDir.appendingPathComponent(artifact.name)
            if !filesEqual(src, dst) {
                if atomicCopyFile(from: src, to: dst, executable: artifact.executable) {
                    updated.append(artifact.name)
                }
            }
        }

        // Sync resident daemon binary under ~/.openagents/environment/bin/<versionID>/openagents-environment
        let bundledHost = resourcesDir.appendingPathComponent("coder-host")
        if fm.fileExists(atPath: bundledHost.path) {
            let versionedDir = envBinDir.appendingPathComponent(versionID, isDirectory: true)
            let versionedHost = versionedDir.appendingPathComponent("openagents-environment")
            try? fm.createDirectory(at: versionedDir, withIntermediateDirectories: true)
            if !filesEqual(bundledHost, versionedHost) {
                if atomicCopyFile(from: bundledHost, to: versionedHost, executable: true) {
                    updated.append("openagents-environment")
                }
            }
            let currentTarget = (try? fm.destinationOfSymbolicLink(atPath: envLink.path)) ?? ""
            if currentTarget != versionedHost.path {
                let tmpLink = binDir.appendingPathComponent(".openagents-environment.new.\(getpid())")
                try? fm.removeItem(at: tmpLink)
                if (try? fm.createSymbolicLink(atPath: tmpLink.path, withDestinationPath: versionedHost.path)) != nil {
                    if rename(tmpLink.path, envLink.path) == 0 {
                        if !updated.contains("openagents-environment") {
                            updated.append("openagents-environment")
                        }
                    } else {
                        try? fm.removeItem(at: tmpLink)
                    }
                }
            }
        }

        writeVersionStamp(versionID, to: workerVersionFile)
        writeVersionStamp(versionID, to: envVersionFile)
        return (versionID, updated)
    }

    /// Ensures the running resident `coder-host` daemon matches `appURL/Contents/Resources/coder-host`.
    /// Never kills an active mid-reply daemon; returns `daemonPendingBusy = true` so the caller can
    /// retry once active worker runs go idle.
    func syncResidentDaemon(from appURL: URL, socketPath: String? = nil) -> (synced: Bool, pendingBusy: Bool) {
        let fm = FileManager.default
        let bundledHost = appURL.appendingPathComponent("Contents/Resources/coder-host")
        guard fm.isExecutableFile(atPath: bundledHost.path) else {
            return (true, false)
        }
        let sock = socketPath ?? home.appendingPathComponent(".openagents/coder/host/service.sock").path
        guard let daemonPID = socketOwnerPID(socketPath: sock) else {
            // No daemon running right now; ensureDaemon() or launchd will launch the synced binary.
            return (true, false)
        }
        let runningExe = executablePath(forPID: daemonPID)
        let stale = runningExe == nil || !filesEqual(URL(fileURLWithPath: runningExe!), bundledHost)
        guard stale else {
            return (true, false)
        }
        if hasActiveChildren(pid: daemonPID) {
            let ver = BundleMetadata.read(from: appURL)?.updateIdentifier ?? BundleMetadata.currentUpdateIdentifier
            NSLog("OpenAgents: resident daemon (pid %d) is busy running workers; deferring sync to %@", daemonPID, ver)
            return (false, true)
        }
        let uid = getuid()
        if fm.fileExists(atPath: envPlist.path) {
            _ = runTool("/bin/launchctl", arguments: ["bootout", "gui/\(uid)/\(envLabel)"])
            terminateAndWait(pid: daemonPID, maxSeconds: 4.0)
            _ = runTool("/bin/launchctl", arguments: ["bootstrap", "gui/\(uid)", envPlist.path])
        } else {
            terminateAndWait(pid: daemonPID, maxSeconds: 4.0)
        }
        let ver = BundleMetadata.read(from: appURL)?.updateIdentifier ?? BundleMetadata.currentUpdateIdentifier
        NSLog("OpenAgents: synced resident daemon to %@", ver)
        return (true, false)
    }

    @discardableResult
    static func syncAll(from appURL: URL, socketPath: String? = nil, home: URL = FileManager.default.homeDirectoryForCurrentUser) -> SyncReport {
        let syncer = WorkerVersionSync(home: home)
        let (versionID, updatedFiles) = syncer.syncBinaries(from: appURL)
        let (daemonSynced, pendingBusy) = syncer.syncResidentDaemon(from: appURL, socketPath: socketPath)
        return SyncReport(
            versionID: versionID,
            updatedFiles: updatedFiles,
            daemonSynced: daemonSynced,
            daemonPendingBusy: pendingBusy
        )
    }

    private func writeVersionStamp(_ versionID: String, to url: URL) {
        let existing = (try? String(contentsOf: url, encoding: .utf8))?.trimmingCharacters(in: .whitespacesAndNewlines)
        guard existing != versionID else { return }
        try? Data((versionID + "\n").utf8).write(to: url, options: .atomic)
    }

    private func filesEqual(_ a: URL, _ b: URL) -> Bool {
        let fm = FileManager.default
        guard fm.fileExists(atPath: a.path), fm.fileExists(atPath: b.path) else { return false }
        let aSize = (try? fm.attributesOfItem(atPath: a.path)[.size] as? NSNumber)?.int64Value
        let bSize = (try? fm.attributesOfItem(atPath: b.path)[.size] as? NSNumber)?.int64Value
        guard let aSize, let bSize, aSize == bSize else { return false }
        return fm.contentsEqual(atPath: a.path, andPath: b.path)
    }

    private func atomicCopyFile(from src: URL, to dst: URL, executable: Bool) -> Bool {
        let fm = FileManager.default
        let tmp = dst.deletingLastPathComponent().appendingPathComponent(".\(dst.lastPathComponent).new.\(getpid()).\(UUID().uuidString.prefix(6))")
        try? fm.removeItem(at: tmp)
        do {
            try fm.copyItem(at: src, to: tmp)
            try fm.setAttributes([.posixPermissions: executable ? 0o755 : 0o644], ofItemAtPath: tmp.path)
            if rename(tmp.path, dst.path) != 0 {
                try? fm.removeItem(at: tmp)
                return false
            }
            return true
        } catch {
            try? fm.removeItem(at: tmp)
            return false
        }
    }

    private func socketOwnerPID(socketPath: String) -> pid_t? {
        guard FileManager.default.fileExists(atPath: socketPath) else { return nil }
        let res = runTool("/usr/sbin/lsof", arguments: ["-U", "-F", "pn"])
        guard res.exitCode == 0 else { return nil }
        var currentPID: pid_t?
        var pids: [pid_t] = []
        for rawLine in res.stdout.split(separator: "\n") {
            guard let first = rawLine.first else { continue }
            let val = String(rawLine.dropFirst())
            if first == "p", let p = pid_t(val) {
                currentPID = p
            } else if first == "n", val == socketPath, let p = currentPID {
                if !pids.contains(p) { pids.append(p) }
            }
        }
        guard !pids.isEmpty else { return nil }
        let pidSet = Set(pids)
        for p in pids {
            let ppidRes = runTool("/bin/ps", arguments: ["-o", "ppid=", "-p", String(p)])
            let ppid = pid_t(ppidRes.stdout.trimmingCharacters(in: .whitespacesAndNewlines)) ?? 0
            if !pidSet.contains(ppid) { return p }
        }
        return pids.first
    }

    private func executablePath(forPID pid: pid_t) -> String? {
        var buf = [CChar](repeating: 0, count: Int(MAXPATHLEN) * 4)
        let n = oa_proc_pidpath(pid, &buf, UInt32(buf.count))
        guard n > 0 else { return nil }
        return String(cString: buf)
    }

    private func hasActiveChildren(pid: pid_t) -> Bool {
        let res = runTool("/usr/bin/pgrep", arguments: ["-P", String(pid)])
        return res.exitCode == 0 && !res.stdout.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty
    }

    private func terminateAndWait(pid: pid_t, maxSeconds: TimeInterval) {
        _ = kill(pid, SIGTERM)
        let deadline = Date().addingTimeInterval(maxSeconds)
        while Date() < deadline {
            if kill(pid, 0) != 0 { return }
            Thread.sleep(forTimeInterval: 0.1)
        }
    }

    private func runTool(_ path: String, arguments: [String]) -> (exitCode: Int32, stdout: String, stderr: String) {
        let proc = Process()
        proc.executableURL = URL(fileURLWithPath: path)
        proc.arguments = arguments
        let outPipe = Pipe()
        let errPipe = Pipe()
        proc.standardOutput = outPipe
        proc.standardError = errPipe
        do {
            try proc.run()
            proc.waitUntilExit()
        } catch {
            return (-1, "", error.localizedDescription)
        }
        let out = String(data: outPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        let err = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8) ?? ""
        return (proc.terminationStatus, out, err)
    }
}

// MARK: - CoderUpdater Coordinator

final class CoderUpdater: NSObject, URLSessionDownloadDelegate {
    static let defaultManifestURLString = "https://github.com/OpenAgentsInc/openagents/releases/latest/download/openagents-desktop-update.json"

    var onStateChange: ((UpdaterState) -> Void)?
    private(set) var state: UpdaterState = .hidden {
        didSet {
            guard state != oldValue else { return }
            let next = state
            DispatchQueue.main.async { [weak self] in
                self?.onStateChange?(next)
            }
        }
    }

    private let verifier: UpdateVerifier
    private let installer: UpdateInstaller
    private let localDetector: LocalStagedUpdateDetector
    private let runningAppURL: URL
    private let launchMetadata: BundleMetadata?
    private let manifestURL: URL?
    private let cacheDir: URL
    private let queue = DispatchQueue(label: "com.openagents.desktop.updater", qos: .utility)
    private var pollTimer: Timer?
    private var downloadSession: URLSession?
    private var activeManifest: UpdateManifest?
    private var isChecking = false

    override init() {
        self.verifier = UpdateVerifier()
        self.installer = UpdateInstaller()
        self.localDetector = LocalStagedUpdateDetector()
        self.runningAppURL = Bundle.main.bundleURL
        self.launchMetadata = BundleMetadata.read(from: Bundle.main.bundleURL)
        if let override = ProcessInfo.processInfo.environment["CODER_UPDATE_URL"] {
            self.manifestURL = override.isEmpty ? nil : URL(string: override)
        } else {
            self.manifestURL = URL(string: CoderUpdater.defaultManifestURLString)
        }
        self.cacheDir = FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".cache/openagents/coder-updater", isDirectory: true)
        super.init()
    }

    func startPeriodicChecks() {
        DispatchQueue.main.asyncAfter(deadline: .now() + 5.0) { [weak self] in
            self?.checkNow(userInitiated: false)
        }
        pollTimer?.invalidate()
        pollTimer = Timer.scheduledTimer(withTimeInterval: 15 * 60, repeats: true) { [weak self] _ in
            self?.checkNow(userInitiated: false)
        }
    }

    func stop() {
        pollTimer?.invalidate()
        pollTimer = nil
        downloadSession?.invalidateAndCancel()
        downloadSession = nil
    }

    /// Checks local staged builds first (instant, offline), then the signed remote manifest.
    func checkNow(userInitiated: Bool, completion: ((UpdaterState) -> Void)? = nil) {
        queue.async { [weak self] in
            guard let self else { return }
            if case .downloading = self.state {
                if let completion { DispatchQueue.main.async { completion(self.state) } }
                return
            }
            if self.isChecking {
                if let completion { DispatchQueue.main.async { completion(self.state) } }
                return
            }
            self.isChecking = true
            defer { self.isChecking = false }

            // 1. Local dev / install.sh staged build detection.
            if let local = self.localDetector.detect(runningAppURL: self.runningAppURL, launchMetadata: self.launchMetadata) {
                switch local {
                case .localStaged(let path, let ver, _):
                    self.state = .ready(version: ver, stagedAppPath: path, source: local)
                case .diskAlreadyUpdated(let ver, _):
                    self.state = .ready(version: ver, stagedAppPath: nil, source: local)
                case .signedRelease:
                    break
                }
                if let completion { DispatchQueue.main.async { completion(self.state) } }
                return
            }

            // 2. Signed GitHub Release manifest check.
            guard let url = self.manifestURL else {
                if case .ready = self.state { } else { self.state = .hidden }
                if let completion { DispatchQueue.main.async { completion(self.state) } }
                return
            }
            let allowFile = url.scheme?.lowercased() == "file"
            var req = URLRequest(url: url, cachePolicy: .reloadIgnoringLocalCacheData, timeoutInterval: 15)
            req.setValue("CoderDesktop/\(self.launchMetadata?.shortVersion ?? "0.1.0")", forHTTPHeaderField: "User-Agent")
            let sem = DispatchSemaphore(value: 0)
            var fetchedData: Data?
            var fetchErr: Error?
            URLSession.shared.dataTask(with: req) { data, resp, err in
                if let err { fetchErr = err }
                else if let http = resp as? HTTPURLResponse, !(200...299).contains(http.statusCode) {
                    fetchErr = UpdaterError.downloadFailed("HTTP \(http.statusCode)")
                } else {
                    fetchedData = data
                }
                sem.signal()
            }.resume()
            _ = sem.wait(timeout: .now() + 20)

            guard let data = fetchedData else {
                if userInitiated, let err = fetchErr {
                    self.state = .error(message: err.localizedDescription, retryable: true)
                }
                if let completion { DispatchQueue.main.async { completion(self.state) } }
                return
            }

            let currentVer = self.launchMetadata?.shortVersion ?? "0.1.0"
            let currentBuild = self.launchMetadata?.buildNumber ?? 0
            do {
                let manifest = try self.verifier.verifyAndDecode(
                    envelopeData: data,
                    currentVersion: currentVer,
                    currentBuild: currentBuild,
                    allowFileURL: allowFile
                )
                let label = UpdateIdentifier.format(manifest: manifest)
                self.state = .available(version: label, source: .signedRelease(manifest))
            } catch UpdaterError.versionNotNewer {
                self.state = .hidden
            } catch {
                if userInitiated {
                    self.state = .error(message: error.localizedDescription, retryable: true)
                }
            }
            if let completion { DispatchQueue.main.async { completion(self.state) } }
        }
    }

    /// Starts downloading, verifying, and unpacking a signed release archive.
    func beginDownload(for manifest: UpdateManifest) {
        queue.async { [weak self] in
            guard let self else { return }
            let label = UpdateIdentifier.format(manifest: manifest)
            guard let url = URL(string: manifest.url) else {
                self.state = .error(message: UpdaterError.unsafeDownloadURL(manifest.url).localizedDescription, retryable: true)
                return
            }
            self.activeManifest = manifest
            self.state = .downloading(version: label, progress: 0)
            self.downloadSession?.invalidateAndCancel()
            let cfg = URLSessionConfiguration.ephemeral
            cfg.timeoutIntervalForResource = 300
            let session = URLSession(configuration: cfg, delegate: self, delegateQueue: nil)
            self.downloadSession = session
            session.downloadTask(with: url).resume()
        }
    }

    /// Applies a staged update (atomic swap + worker/daemon sync + relaunch).
    func applyStagedAndRelaunch() -> Result<Void, Error> {
        switch state {
        case .ready(_, let stagedPath, let source):
            do {
                switch source {
                case .diskAlreadyUpdated:
                    _ = WorkerVersionSync.syncAll(from: runningAppURL)
                    try installer.scheduleRelaunch(appURL: runningAppURL)
                    return .success(())
                case .localStaged(let path, _, _):
                    let stagedURL = URL(fileURLWithPath: path)
                    let localInstaller = UpdateInstaller(
                        pinnedBundleID: verifier.pinnedBundleID,
                        pinnedTeamID: verifier.pinnedTeamID,
                        requireTeamID: false
                    )
                    try localInstaller.verifyAppBundle(at: stagedURL)
                    try localInstaller.atomicSwap(stagedApp: stagedURL, targetApp: runningAppURL)
                    _ = WorkerVersionSync.syncAll(from: runningAppURL)
                    try localInstaller.scheduleRelaunch(appURL: runningAppURL)
                    return .success(())
                case .signedRelease(let manifest):
                    guard let path = stagedPath else {
                        throw UpdaterError.appBundleNotFound
                    }
                    let stagedURL = URL(fileURLWithPath: path)
                    try installer.verifyAppBundle(at: stagedURL, expectedVersion: manifest.version)
                    try installer.atomicSwap(stagedApp: stagedURL, targetApp: runningAppURL)
                    _ = WorkerVersionSync.syncAll(from: runningAppURL)
                    try installer.scheduleRelaunch(appURL: runningAppURL)
                    return .success(())
                }
            } catch {
                state = .error(message: error.localizedDescription, retryable: true)
                return .failure(error)
            }
        default:
            return .failure(UpdaterError.appBundleNotFound)
        }
    }

    // MARK: - URLSessionDownloadDelegate

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        guard let manifest = activeManifest else { return }
        let total = totalBytesExpectedToWrite > 0 ? totalBytesExpectedToWrite : manifest.size
        let pct = total > 0 ? Int(min(99, max(0, (totalBytesWritten * 100) / total))) : 0
        let label = UpdateIdentifier.format(manifest: manifest)
        state = .downloading(version: label, progress: pct)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard let manifest = activeManifest else { return }
        let label = UpdateIdentifier.format(manifest: manifest)
        state = .downloading(version: label, progress: 100)
        do {
            let fm = FileManager.default
            try fm.createDirectory(at: cacheDir, withIntermediateDirectories: true)
            let zipURL = cacheDir.appendingPathComponent("update-\(manifest.version).zip")
            try? fm.removeItem(at: zipURL)
            try fm.moveItem(at: location, to: zipURL)
            try installer.verifyArchive(at: zipURL, expectedSize: manifest.size, expectedSHA256: manifest.sha256)
            let unpackDir = cacheDir.appendingPathComponent("unpacked-\(manifest.version)", isDirectory: true)
            let appURL = try installer.unpackZip(at: zipURL, into: unpackDir)
            try installer.verifyAppBundle(at: appURL, expectedVersion: manifest.version)
            try? fm.removeItem(at: zipURL)
            state = .ready(version: label, stagedAppPath: appURL.path, source: .signedRelease(manifest))
        } catch {
            state = .error(message: error.localizedDescription, retryable: true)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        guard let error else { return }
        if (error as NSError).code == NSURLErrorCancelled { return }
        state = .error(message: error.localizedDescription, retryable: true)
    }
}

#if OA_FULL
// MARK: - Executor Release Auto-Distribution & Pylon Dashboard (W6.3, #13928)

/// Signed `openagents.executor-release.v1` auto-distribution & local Pylon/settlement
/// snapshot backed strictly by Coax `#authority` rules (`oa_dc_pylon_dashboard`,
/// `oa_dc_exec_release_stage`, `oa_dc_exec_release_promote`, `oa_dc_exec_release_rollback`).
enum ExecutorReleaseUpdater {
    struct Snapshot {
        let pylonState: String
        let contributed: String
        let settledSats: String
        let releaseSummary: String
        let details: String
        let settledCount: Int
        let verifiedTraces: Int
        let trainingWindows: Int
        let canPromote: Bool
        let canRollback: Bool
        let activeOnDisk: Bool
    }

    static var lastMessage: String?

    static func snapshot(home: String = "") -> Snapshot {
        var pyBuf = [UInt8](repeating: 0, count: 64)
        var coBuf = [UInt8](repeating: 0, count: 128)
        var saBuf = [UInt8](repeating: 0, count: 160)
        var rlBuf = [UInt8](repeating: 0, count: 160)
        var detBuf = [UInt8](repeating: 0, count: 2048)
        var counts = [Int64](repeating: 0, count: 5)
        let rc: Int64 = DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(home) { hStr in
                DesktopCore.withByteBuf(&pyBuf) { pyV in
                    DesktopCore.withByteBuf(&coBuf) { coV in
                        DesktopCore.withByteBuf(&saBuf) { saV in
                            DesktopCore.withByteBuf(&rlBuf) { rlV in
                                DesktopCore.withByteBuf(&detBuf) { detV in
                                    DesktopCore.withS64Buf(&counts) { cntV in
                                        oa_dc_pylon_dashboard(hStr, pyV, coV, saV, rlV, detV, cntV)
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        func cstr(_ b: [UInt8]) -> String {
            let n = b.firstIndex(of: 0) ?? b.count
            return String(decoding: b[0..<n], as: UTF8.self)
        }
        return Snapshot(
            pylonState: cstr(pyBuf),
            contributed: cstr(coBuf),
            settledSats: cstr(saBuf),
            releaseSummary: cstr(rlBuf),
            details: cstr(detBuf),
            settledCount: Int(counts[0]),
            verifiedTraces: Int(counts[1]),
            trainingWindows: Int(counts[2]),
            canPromote: counts[3] == 1,
            canRollback: counts[4] == 1,
            activeOnDisk: rc == 1
        )
    }

    static func stageSignedRelease(home: String = "", manifestPath: String, payloadPath: String, trustedPubkey: String) -> String? {
        var errBuf = [UInt8](repeating: 0, count: 256)
        let rc: Int64 = DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(home) { hStr in
                DesktopCore.withCoaxString(manifestPath) { mStr in
                    DesktopCore.withCoaxString(payloadPath) { pStr in
                        DesktopCore.withCoaxString(trustedPubkey) { kStr in
                            DesktopCore.withByteBuf(&errBuf) { eV in
                                oa_dc_exec_release_stage(hStr, mStr, pStr, kStr, eV)
                            }
                        }
                    }
                }
            }
        }
        if rc == 0 { lastMessage = "Signed release verified & staged into Ch 224 shadow mode."; return nil }
        let msg = DesktopCore.cString(from: errBuf)
        lastMessage = "Refused: \(msg.isEmpty ? "code \(rc)" : msg)"
        return lastMessage
    }

    static func promoteStaged(home: String = "") -> String? {
        var errBuf = [UInt8](repeating: 0, count: 256)
        let rc: Int64 = DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(home) { hStr in
                DesktopCore.withByteBuf(&errBuf) { eV in
                    oa_dc_exec_release_promote(hStr, eV)
                }
            }
        }
        if rc == 0 { lastMessage = "Promoted shadow-verified executor checkpoint atomically."; return nil }
        let msg = DesktopCore.cString(from: errBuf)
        lastMessage = "Promote refused: \(msg.isEmpty ? "code \(rc)" : msg)"
        return lastMessage
    }

    static func rollbackActive(home: String = "") -> String? {
        var errBuf = [UInt8](repeating: 0, count: 256)
        let rc: Int64 = DesktopCore.onCoreStack {
            DesktopCore.withCoaxString(home) { hStr in
                DesktopCore.withByteBuf(&errBuf) { eV in
                    oa_dc_exec_release_rollback(hStr, eV)
                }
            }
        }
        if rc == 0 { lastMessage = "Rolled back executor checkpoint atomically to pinned rollback target."; return nil }
        let msg = DesktopCore.cString(from: errBuf)
        lastMessage = "Rollback refused: \(msg.isEmpty ? "code \(rc)" : msg)"
        return lastMessage
    }
}
#endif

// MARK: - Hex Helpers

extension Data {
    init?(hexString: String) {
        let chars = Array(hexString)
        guard chars.count % 2 == 0 else { return nil }
        var bytes = [UInt8]()
        bytes.reserveCapacity(chars.count / 2)
        var i = 0
        while i < chars.count {
            guard let hi = UInt8(String(chars[i]), radix: 16),
                  let lo = UInt8(String(chars[i + 1]), radix: 16) else {
                return nil
            }
            bytes.append((hi << 4) | lo)
            i += 2
        }
        self.init(bytes)
    }
}
