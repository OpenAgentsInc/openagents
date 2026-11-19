import CryptoKit
import Foundation

// MARK: - Signed Release Manifest Types (openagents.desktop.update.v1)

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
    let bundleID: String
    let teamID: String
    let url: String
    let sha256: String
    let size: Int64
    let notes: String?

    enum CodingKeys: String, CodingKey {
        case schema
        case version
        case build
        case bundleID = "bundle_id"
        case teamID = "team_id"
        case url
        case sha256
        case size
        case notes
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
        if lhs.major != rhs.major { return lhs.major < rhs.major }
        if lhs.minor != rhs.minor { return lhs.minor < rhs.minor }
        if lhs.patch != rhs.patch { return lhs.patch < rhs.patch }
        switch (lhs.prerelease, rhs.prerelease) {
        case (nil, nil): return false
        case (.some, nil): return true
        case (nil, .some): return false
        case let (.some(a), .some(b)): return a < b
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
    static let schemaVersion = "openagents.desktop.update.v1"
    static let pinnedBundleID = "com.openagents.coder"
    static let pinnedTeamID = "HQWSG26L43"

    /// Compiled-in Ed25519 release signing public keys (key ID -> base64 raw 32-byte public key).
    /// Add rotated keys here alongside the active key before cutting a release.
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

    /// Verifies the Ed25519 signature over the raw payload bytes BEFORE JSON decoding the manifest.
    func verifyAndDecode(
        envelopeData: Data,
        currentVersion: String,
        currentBuild: Int = 0,
        allowFileURL: Bool = false
    ) throws -> UpdateManifest {
        let envelope: UpdateEnvelope
        do {
            envelope = try JSONDecoder().decode(UpdateEnvelope.self, from: envelopeData)
        } catch {
            throw UpdaterError.invalidEnvelope(error.localizedDescription)
        }
        if let s = envelope.schema, s != UpdateVerifier.schemaVersion {
            throw UpdaterError.unsupportedSchema(s)
        }
        guard let pubBytes = trustedKeys[envelope.key] else {
            throw UpdaterError.untrustedKey(envelope.key)
        }
        let publicKey: Curve25519.Signing.PublicKey
        do {
            publicKey = try Curve25519.Signing.PublicKey(rawRepresentation: pubBytes)
        } catch {
            throw UpdaterError.invalidPublicKey(envelope.key)
        }
        guard let payloadBytes = Data(base64Encoded: envelope.payload) else {
            throw UpdaterError.invalidBase64("payload")
        }
        guard let sigBytes = Data(base64Encoded: envelope.signature), sigBytes.count == 64 else {
            throw UpdaterError.invalidBase64("signature")
        }
        // Verify Ed25519 signature BEFORE decoding payload JSON.
        guard publicKey.isValidSignature(sigBytes, for: payloadBytes) else {
            throw UpdaterError.invalidSignature
        }

        let manifest: UpdateManifest
        do {
            manifest = try JSONDecoder().decode(UpdateManifest.self, from: payloadBytes)
        } catch {
            throw UpdaterError.invalidManifest(error.localizedDescription)
        }
        if let s = manifest.schema, s != UpdateVerifier.schemaVersion {
            throw UpdaterError.unsupportedSchema(s)
        }
        guard manifest.bundleID == pinnedBundleID else {
            throw UpdaterError.bundleIDMismatch(expected: pinnedBundleID, actual: manifest.bundleID)
        }
        guard manifest.teamID == pinnedTeamID else {
            throw UpdaterError.teamIDMismatch(expected: pinnedTeamID, actual: manifest.teamID)
        }
        guard isNewer(candidateVersion: manifest.version, candidateBuild: manifest.build, currentVersion: currentVersion, currentBuild: currentBuild) else {
            throw UpdaterError.versionNotNewer(current: currentVersion, candidate: manifest.version)
        }
        guard let url = URL(string: manifest.url) else {
            throw UpdaterError.unsafeDownloadURL(manifest.url)
        }
        let scheme = url.scheme?.lowercased() ?? ""
        if scheme != "https" && !(allowFileURL && scheme == "file") {
            throw UpdaterError.unsafeDownloadURL(manifest.url)
        }
        guard manifest.size > 0 else {
            throw UpdaterError.invalidManifest("size must be positive")
        }
        let normalizedHash = manifest.sha256.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard normalizedHash.count == 64, Data(hexString: normalizedHash) != nil else {
            throw UpdaterError.invalidManifest("sha256 must be 64 hex chars")
        }
        return manifest
    }

    func isNewer(candidateVersion: String, candidateBuild: Int?, currentVersion: String, currentBuild: Int) -> Bool {
        guard let cand = SemVer(candidateVersion), let curr = SemVer(currentVersion) else {
            return false
        }
        if cand != curr { return cand > curr }
        if let cb = candidateBuild, currentBuild > 0 { return cb > currentBuild }
        return false
    }
}

// MARK: - Archive Verification, Unpack, Codesign, Atomic Swap

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

    /// Verifies exact byte size and SHA-256 digest of a downloaded archive.
    func verifyArchive(at url: URL, expectedSize: Int64, expectedSHA256: String) throws {
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        let actualSize = (attrs[.size] as? NSNumber)?.int64Value ?? -1
        guard actualSize == expectedSize else {
            throw UpdaterError.sizeMismatch(expected: expectedSize, actual: actualSize)
        }
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }
        var hasher = SHA256()
        while true {
            let chunk = try handle.read(upToCount: 256 * 1024) ?? Data()
            if chunk.isEmpty { break }
            hasher.update(data: chunk)
        }
        let actualHash = hasher.finalize().map { String(format: "%02x", $0) }.joined()
        let expected = expectedSHA256.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        guard actualHash == expected else {
            throw UpdaterError.sha256Mismatch(expected: expected, actual: actualHash)
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

    /// Checks both the `install.sh` stage directory (`$CODER_RELEASE_HOME/stage/Coder.app`)
    /// and whether the running `.app` on disk was already replaced with a newer commit.
    func detect(runningAppURL: URL, launchMetadata: BundleMetadata?) -> UpdateSource? {
        // 1. Check if the running app on disk was already swapped out from under us.
        if let launch = launchMetadata, let disk = BundleMetadata.read(from: runningAppURL),
           disk.bundleID == pinnedBundleID {
            let commitChanged = !disk.commit.isEmpty && !launch.commit.isEmpty && disk.commit != launch.commit
            let buildBumped = disk.buildNumber > launch.buildNumber && launch.buildNumber > 0
            if commitChanged || buildBumped {
                let label = !disk.commit.isEmpty ? String(disk.commit.prefix(7)) : "v\(disk.shortVersion)"
                return .diskAlreadyUpdated(version: label, commit: disk.commit)
            }
        }

        // 2. Check install.sh staged build at ~/.cache/openagents/coder-release/stage/Coder.app
        let stagedApp = releaseHome.appendingPathComponent("stage/Coder.app", isDirectory: true)
        guard FileManager.default.fileExists(atPath: stagedApp.path),
              let staged = BundleMetadata.read(from: stagedApp),
              staged.bundleID == pinnedBundleID else {
            return nil
        }
        if let launch = launchMetadata {
            if !staged.commit.isEmpty && staged.commit == launch.commit {
                return nil
            }
            if staged.commit.isEmpty && staged.buildNumber <= launch.buildNumber {
                return nil
            }
        }
        let label = !staged.commit.isEmpty ? String(staged.commit.prefix(7)) : "v\(staged.shortVersion)"
        return .localStaged(appPath: stagedApp.path, version: label, commit: staged.commit)
    }
}

// MARK: - CoderUpdater Coordinator

final class CoderUpdater: NSObject, URLSessionDownloadDelegate {
    static let defaultManifestURLString = "https://github.com/OpenAgentsInc/openagents/releases/latest/download/coder-desktop-update.json"

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
    private let queue = DispatchQueue(label: "com.openagents.coder.updater", qos: .utility)
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
                let label = manifest.version.hasPrefix("v") ? manifest.version : "v\(manifest.version)"
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
            let label = manifest.version.hasPrefix("v") ? manifest.version : "v\(manifest.version)"
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

    /// Applies a staged update (atomic swap + relaunch). Returns true if the host should terminate now.
    func applyStagedAndRelaunch() -> Result<Void, Error> {
        switch state {
        case .ready(_, let stagedPath, let source):
            do {
                switch source {
                case .diskAlreadyUpdated:
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
                    try localInstaller.scheduleRelaunch(appURL: runningAppURL)
                    return .success(())
                case .signedRelease(let manifest):
                    guard let path = stagedPath else {
                        throw UpdaterError.appBundleNotFound
                    }
                    let stagedURL = URL(fileURLWithPath: path)
                    try installer.verifyAppBundle(at: stagedURL, expectedVersion: manifest.version)
                    try installer.atomicSwap(stagedApp: stagedURL, targetApp: runningAppURL)
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
        let label = manifest.version.hasPrefix("v") ? manifest.version : "v\(manifest.version)"
        state = .downloading(version: label, progress: pct)
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard let manifest = activeManifest else { return }
        let label = manifest.version.hasPrefix("v") ? manifest.version : "v\(manifest.version)"
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
