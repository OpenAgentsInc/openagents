// Firm reason: 6 (Swift host backend for T3-parity Settings -> Providers, multi-instance Antigravity runtime,
// managed binary lifecycle with SHA-512 + live ACP verify, interactive OAuth/Enterprise/API-key/Vertex auth,
// live model & thought-level probing, quota limits probing, and skills discovery).
import AppKit
import CryptoKit
import Foundation
import Security

// MARK: - Data Model

enum ProviderDriverKind: String, Codable, CaseIterable {
    case antigravity = "antigravity"
    case claude = "claudeAgent"
    case codex = "codex"
    case cursor = "cursor"
    case acp = "acpRegistry"

    var uiIndex: Int32 {
        switch self {
        case .antigravity: return 0
        case .claude: return 1
        case .codex: return 2
        case .cursor: return 3
        case .acp: return 4
        }
    }

    static func from(uiIndex: Int32) -> ProviderDriverKind {
        switch uiIndex {
        case 0: return .antigravity
        case 1: return .claude
        case 2: return .codex
        case 3: return .cursor
        default: return .acp
        }
    }

    var defaultDisplayName: String {
        switch self {
        case .antigravity: return "Antigravity"
        case .claude: return "Claude"
        case .codex: return "Codex"
        case .cursor: return "Cursor"
        case .acp: return "ACP Agent"
        }
    }
}

enum AntigravityAuthMethod: String, Codable, CaseIterable {
    case oauthPersonal = "oauth-personal"
    case oauthBusiness = "oauth-business"
    case geminiApiKey = "gemini-api-key"
    case agentPlatform = "agent-platform"

    var uiIndex: Int32 {
        switch self {
        case .oauthPersonal: return 0
        case .oauthBusiness: return 1
        case .geminiApiKey: return 2
        case .agentPlatform: return 3
        }
    }

    static func from(uiIndex: Int32) -> AntigravityAuthMethod {
        switch uiIndex {
        case 1: return .oauthBusiness
        case 2: return .geminiApiKey
        case 3: return .agentPlatform
        default: return .oauthPersonal
        }
    }

    var label: String {
        switch self {
        case .oauthPersonal: return "Google account"
        case .oauthBusiness: return "Gemini Enterprise"
        case .geminiApiKey: return "Gemini API key"
        case .agentPlatform: return "Agent Platform (Vertex AI)"
        }
    }

    var usesBrowser: Bool {
        self == .oauthPersonal || self == .oauthBusiness
    }
}

struct ProviderEnvVarConfig: Codable, Equatable {
    var key: String
    var value: String   // Always empty on disk when secret == true (stored in ProviderSecretStore)
    var secret: Bool
}

struct ProviderModelConfig: Codable, Equatable {
    var slug: String
    var name: String
    var enabled: Bool
    var favorite: Bool
    var thinking: Bool
    var thoughtLevels: [String]
    var custom: Bool
    var isDefault: Bool
    var defaultEffort: String

    init(slug: String, name: String, enabled: Bool, favorite: Bool, thinking: Bool, thoughtLevels: [String], custom: Bool, isDefault: Bool = false, defaultEffort: String = "medium") {
        self.slug = slug
        self.name = name
        self.enabled = enabled
        self.favorite = favorite
        self.thinking = thinking
        self.thoughtLevels = thoughtLevels
        self.custom = custom
        self.isDefault = isDefault
        self.defaultEffort = defaultEffort
    }

    init(from decoder: Decoder) throws {
        let c = try decoder.container(keyedBy: CodingKeys.self)
        slug = try c.decode(String.self, forKey: .slug)
        name = try c.decode(String.self, forKey: .name)
        enabled = try c.decode(Bool.self, forKey: .enabled)
        favorite = try c.decode(Bool.self, forKey: .favorite)
        thinking = try c.decode(Bool.self, forKey: .thinking)
        thoughtLevels = try c.decode([String].self, forKey: .thoughtLevels)
        custom = try c.decode(Bool.self, forKey: .custom)
        isDefault = (try? c.decodeIfPresent(Bool.self, forKey: .isDefault)) ?? false
        defaultEffort = (try? c.decodeIfPresent(String.self, forKey: .defaultEffort)) ?? "medium"
    }
}

struct ProviderInstanceConfig: Codable, Equatable {
    var id: String
    var driver: ProviderDriverKind
    var displayName: String
    var enabled: Bool
    var isDefault: Bool
    var accent: Int
    var authMethod: AntigravityAuthMethod
    var gcpProject: String
    var gcpLocation: String
    var binaryPath: String
    var extraPath: String
    var envVars: [ProviderEnvVarConfig]
    var models: [ProviderModelConfig]
}

struct ProviderProbeSnapshot {
    var status: Int32          // 0=disabled, 1=ready, 2=warning, 3=error
    var headline: String
    var summary: String
    var version: String
    var authStatus: String     // "authenticated", "unauthenticated", "unknown"
    var hasAutoMode: Bool
    var defaultModel: String
    var checkedAt: Date
}

struct ProviderAuthSession {
    var state: Int32           // 0=idle, 1=authenticated, 2=starting, 3=waiting, 4=verifying, 5=failed
    var subtitle: String
    var detail: String
    var authUrl: String
    var canSignOut: Bool
}

// MARK: - Secret Store (0700 dir / 0600 files, never in provider-instances.json)

enum ProviderSecretStore {
    private static var rootURL: URL {
        let appSupport = FileManager.default.urls(for: .applicationSupportDirectory, in: .userDomainMask).first
            ?? FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Application Support", isDirectory: true)
        return appSupport.appendingPathComponent("Coder/secrets/providers", isDirectory: true)
    }

    static func sha256Hex(_ text: String) -> String {
        let digest = SHA256.hash(data: Data(text.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    private static func dir(for instanceId: String) -> URL {
        rootURL.appendingPathComponent(sha256Hex(instanceId), isDirectory: true)
    }

    private static func ensureDir(_ url: URL) {
        let fm = FileManager.default
        try? fm.createDirectory(at: url, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try? fm.setAttributes([.posixPermissions: 0o700], ofItemAtPath: url.path)
    }

    private static func readSecret(_ file: URL) -> String? {
        guard let data = try? Data(contentsOf: file),
              let raw = String(data: data, encoding: .utf8) else { return nil }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    private static func writeSecret(_ value: String, to file: URL) {
        let trimmed = value.trimmingCharacters(in: .whitespacesAndNewlines)
        let fm = FileManager.default
        if trimmed.isEmpty {
            try? fm.removeItem(at: file)
            return
        }
        ensureDir(file.deletingLastPathComponent())
        try? Data(trimmed.utf8).write(to: file, options: .atomic)
        try? fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
    }

    static func apiKey(for instanceId: String) -> String? {
        readSecret(dir(for: instanceId).appendingPathComponent("api-key.secret"))
    }

    static func hasApiKey(for instanceId: String) -> Bool {
        apiKey(for: instanceId) != nil
    }

    static func setApiKey(_ value: String?, for instanceId: String) {
        writeSecret(value ?? "", to: dir(for: instanceId).appendingPathComponent("api-key.secret"))
    }

    static func envSecret(for instanceId: String, key: String) -> String? {
        readSecret(dir(for: instanceId).appendingPathComponent("env-\(sha256Hex(key)).secret"))
    }

    static func hasEnvSecret(for instanceId: String, key: String) -> Bool {
        envSecret(for: instanceId, key: key) != nil
    }

    static func setEnvSecret(_ value: String?, for instanceId: String, key: String) {
        writeSecret(value ?? "", to: dir(for: instanceId).appendingPathComponent("env-\(sha256Hex(key)).secret"))
    }

    static func removeAll(for instanceId: String) {
        try? FileManager.default.removeItem(at: dir(for: instanceId))
    }
}

// MARK: - Profile Paths, settings.json Materialization & ~/.gemini Symlinks (100% T3 Parity)

enum AntigravityProfileManager {
    /// Exact 19 ambient env keys stripped by T3's AntigravityAuth.ts / AntigravityProvider.ts.
    static let removedEnvKeys: [String] = [
        "GOOGLE_APPLICATION_CREDENTIALS",
        "GOOGLE_CLOUD_ACCESS_TOKEN",
        "CLOUDSDK_AUTH_ACCESS_TOKEN",
        "GEMINI_API_KEY",
        "GOOGLE_API_KEY",
        "GOOGLE_CLOUD_PROJECT",
        "GOOGLE_CLOUD_PROJECT_ID",
        "GOOGLE_CLOUD_QUOTA_PROJECT",
        "CLOUDSDK_CORE_PROJECT",
        "GOOGLE_CLOUD_LOCATION",
        "GOOGLE_GENAI_USE_VERTEXAI",
        "GOOGLE_GENAI_USE_GCA",
        "GEMINI_CLI_HOME",
        "GEMINI_HOME",
        "XDG_CONFIG_HOME",
        "XDG_CACHE_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "SSH_CONNECTION",
    ]

    static var profilesRootURL: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".openagents/providers/antigravity", isDirectory: true)
    }

    static var runtimeTempRootURL: URL {
        profilesRootURL.appendingPathComponent("tmp", isDirectory: true)
    }

    static func profileDir(for instanceId: String) -> URL {
        if instanceId == "antigravity" {
            let legacyCustom = CoderAntigravity.profile
            if !legacyCustom.isEmpty {
                return URL(fileURLWithPath: (legacyCustom as NSString).expandingTildeInPath, isDirectory: true)
            }
        }
        return profilesRootURL.appendingPathComponent(ProviderSecretStore.sha256Hex(instanceId), isDirectory: true)
    }

    static func acpDataDir(for instanceId: String) -> URL {
        profileDir(for: instanceId).appendingPathComponent("antigravity-acp", isDirectory: true)
    }

    static func settingsJSONURL(for instanceId: String) -> URL {
        acpDataDir(for: instanceId).appendingPathComponent("settings.json")
    }

    /// Port of T3's `linkAntigravityUserConfig`: symlinks shared ~/.gemini prompt & skills dirs into isolated profile.
    static func linkUserConfig(into profile: URL) {
        let fm = FileManager.default
        let homeGemini = fm.homeDirectoryForCurrentUser.appendingPathComponent(".gemini", isDirectory: true)
        let links: [(src: URL, dst: URL)] = [
            (homeGemini.appendingPathComponent("AGENTS.md"), profile.appendingPathComponent("AGENTS.md")),
            (homeGemini.appendingPathComponent("GEMINI.md"), profile.appendingPathComponent("GEMINI.md")),
            (homeGemini.appendingPathComponent("config/skills.json"), profile.appendingPathComponent("config/skills.json")),
            (homeGemini.appendingPathComponent("config/skills"), profile.appendingPathComponent("config/skills")),
            (homeGemini.appendingPathComponent("antigravity-cli/skills"), profile.appendingPathComponent("antigravity-cli/skills")),
        ]
        for (src, dst) in links {
            guard fm.fileExists(atPath: src.path) else { continue }
            let parent = dst.deletingLastPathComponent()
            try? fm.createDirectory(at: parent, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            // Do not clobber real files/dirs already inside profile; refresh broken/stale symlinks.
            if let attrs = try? fm.attributesOfItem(atPath: dst.path),
               let type = attrs[.type] as? FileAttributeType {
                if type == .typeSymbolicLink {
                    let cur = try? fm.destinationOfSymbolicLink(atPath: dst.path)
                    if cur == src.path { continue }
                    try? fm.removeItem(at: dst)
                } else {
                    continue
                }
            }
            try? fm.createSymbolicLink(atPath: dst.path, withDestinationPath: src.path)
        }
    }

    /// Materializes `<profile>/antigravity-acp/settings.json` with exact T3 schema and permissions.
    @discardableResult
    static func prepareProfile(for inst: ProviderInstanceConfig) -> URL {
        let fm = FileManager.default
        let profile = profileDir(for: inst.id)
        let acpDir = acpDataDir(for: inst.id)
        try? fm.createDirectory(at: profile, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        try? fm.createDirectory(at: acpDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        linkUserConfig(into: profile)

        let settingsURL = settingsJSONURL(for: inst.id)
        var rootObj: [String: Any] = [:]
        if let data = try? Data(contentsOf: settingsURL),
           let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any] {
            rootObj = parsed
        }
        var authObj = (rootObj["auth"] as? [String: Any]) ?? [:]
        authObj["type"] = inst.authMethod.rawValue
        rootObj["auth"] = authObj

        let proj = inst.gcpProject.trimmingCharacters(in: .whitespacesAndNewlines)
        let loc = inst.gcpLocation.trimmingCharacters(in: .whitespacesAndNewlines)
        if !proj.isEmpty || !loc.isEmpty {
            var gcpObj = (rootObj["gcp"] as? [String: Any]) ?? [:]
            if !proj.isEmpty { gcpObj["project"] = proj } else { gcpObj.removeValue(forKey: "project") }
            if !loc.isEmpty { gcpObj["location"] = loc } else { gcpObj.removeValue(forKey: "location") }
            rootObj["gcp"] = gcpObj
        } else {
            rootObj.removeValue(forKey: "gcp")
        }

        if let out = try? JSONSerialization.data(withJSONObject: rootObj, options: [.prettyPrinted, .sortedKeys]) {
            try? out.write(to: settingsURL, options: .atomic)
            try? fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: settingsURL.path)
        }
        return profile
    }

    static func authConfigIssue(for inst: ProviderInstanceConfig) -> String? {
        let proj = inst.gcpProject.trimmingCharacters(in: .whitespacesAndNewlines)
        let hasKey = ProviderSecretStore.hasApiKey(for: inst.id)
        switch inst.authMethod {
        case .oauthPersonal:
            return nil
        case .oauthBusiness:
            return proj.isEmpty ? "Gemini Enterprise requires a GCP project." : nil
        case .geminiApiKey:
            return hasKey ? nil : "Gemini API key sign-in requires an API key."
        case .agentPlatform:
            return (hasKey || !proj.isEmpty) ? nil : "Agent Platform requires an API key or a GCP project."
        }
    }

    /// Builds `agyworker` environment dictionary for an Antigravity instance (including secrets & custom vars).
    static func workerEnv(for inst: ProviderInstanceConfig, resolvedBinary: String?) -> [String: String] {
        let profile = prepareProfile(for: inst)
        var env: [String: String] = [
            "CODER_AGY_PROFILE_DIR": profile.path,
            "CODER_AGY_AUTH_METHOD": inst.authMethod.rawValue,
            "CODER_AGY_MODE": CoderAntigravity.mode,
            "CODER_AGY_INTERACTIVE": "1",
        ]
        if let bin = resolvedBinary, !bin.isEmpty {
            env["CODER_AGY_BIN"] = bin
        } else if !inst.binaryPath.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
            env["CODER_AGY_BIN"] = (inst.binaryPath.trimmingCharacters(in: .whitespacesAndNewlines) as NSString).expandingTildeInPath
        }
        if let key = ProviderSecretStore.apiKey(for: inst.id), !key.isEmpty {
            env["CODER_AGY_API_KEY"] = key
        }
        let proj = inst.gcpProject.trimmingCharacters(in: .whitespacesAndNewlines)
        if !proj.isEmpty { env["CODER_AGY_GCP_PROJECT"] = proj }
        let loc = inst.gcpLocation.trimmingCharacters(in: .whitespacesAndNewlines)
        if !loc.isEmpty { env["CODER_AGY_GCP_LOCATION"] = loc }
        if !CoderAntigravity.model.isEmpty { env["CODER_AGY_MODEL"] = CoderAntigravity.model }
        if !CoderAntigravity.effort.isEmpty { env["CODER_AGY_EFFORT"] = CoderAntigravity.effort }

        // User-configured per-instance environment variables (non-secret + 0600 secret values).
        for item in inst.envVars {
            let k = item.key.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !k.isEmpty else { continue }
            if item.secret {
                if let sv = ProviderSecretStore.envSecret(for: inst.id, key: k) { env[k] = sv }
            } else {
                env[k] = item.value
            }
        }
        return env
    }

    static func makeProcessTempDir() -> URL? {
        let fm = FileManager.default
        let dir = runtimeTempRootURL.appendingPathComponent("run-\(UUID().uuidString.lowercased())", isDirectory: true)
        do {
            try fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            return dir
        } catch {
            return nil
        }
    }

    static func retireTempDirs() {
        try? FileManager.default.removeItem(at: runtimeTempRootURL)
    }
}

// MARK: - Managed Runtime Installation (100% Port of AntigravityInstallation.ts & antigravityCliRelease.ts)

struct AntigravityReleaseManifest: Decodable {
    let version: String
    let url: String
    let sha512: String?
    let sha256: String?
}

struct ManagedRuntimeActive: Codable {
    let version: String
    let binaryPath: String
    let installedAt: Date
}

struct AntigravityRuntimeSnapshot {
    var state: Int32         // 0=none, 1=installed(system/custom), 2=managed, 3=update, 4=installing, 5=error
    var statusText: String
    var buttonLabel: String
    var progressPct: Int32
    var canRemove: Bool
    var errorText: String
    var resolvedBinary: String?
    var version: String?
}

final class AntigravityRuntimeManager: NSObject, URLSessionDownloadDelegate {
    static let shared = AntigravityRuntimeManager()

    var onChange: (() -> Void)?

    private let fm = FileManager.default
    private let lock = NSLock()
    private var activeLeases: [String: Int] = [:]
    private var downloadTask: URLSessionDownloadTask?
    private var downloadSession: URLSession?
    private var pendingManifest: AntigravityReleaseManifest?

    private(set) var isInstalling = false
    private(set) var progressPct: Int32 = -1
    private(set) var installStage = ""
    private(set) var lastError = ""
    private(set) var latestManifest: AntigravityReleaseManifest?

    var managedRootURL: URL {
        fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/runtimes/agy", isDirectory: true)
    }
    var releasesDirURL: URL { managedRootURL.appendingPathComponent("releases", isDirectory: true) }
    var stagingDirURL: URL { managedRootURL.appendingPathComponent("staging", isDirectory: true) }
    var activeJSONURL: URL { managedRootURL.appendingPathComponent("active.json") }

    static var manifestURL: URL {
        #if arch(arm64)
        let arch = "darwin_arm64"
        #else
        let arch = "darwin_x64"
        #endif
        return URL(string: "https://antigravity-cli-auto-updater-974169037036.us-central1.run.app/manifests/\(arch).json")!
    }

    func acquireLease(binaryPath: String) {
        lock.lock(); defer { lock.unlock() }
        activeLeases[binaryPath, default: 0] += 1
    }

    func releaseLease(binaryPath: String) {
        lock.lock(); defer { lock.unlock() }
        let cur = activeLeases[binaryPath] ?? 0
        if cur <= 1 { activeLeases.removeValue(forKey: binaryPath) }
        else { activeLeases[binaryPath] = cur - 1 }
    }

    func hasActiveManagedLease() -> Bool {
        lock.lock(); defer { lock.unlock() }
        let root = managedRootURL.path
        return activeLeases.contains { $0.key.hasPrefix(root) && $0.value > 0 }
    }

    func activeManaged() -> ManagedRuntimeActive? {
        guard let data = try? Data(contentsOf: activeJSONURL),
              let rec = try? JSONDecoder().decode(ManagedRuntimeActive.self, from: data),
              fm.isExecutableFile(atPath: rec.binaryPath) else { return nil }
        return rec
    }

    func resolveBinary(for inst: ProviderInstanceConfig) -> (path: String?, source: String) {
        let custom = inst.binaryPath.trimmingCharacters(in: .whitespacesAndNewlines)
        if !custom.isEmpty {
            let expanded = (custom as NSString).expandingTildeInPath
            if fm.isExecutableFile(atPath: expanded) { return (expanded, "custom") }
            return (nil, "custom-missing")
        }
        if let managed = activeManaged() {
            return (managed.binaryPath, "managed")
        }
        if let sys = CoderAntigravity.resolvedBinaryPath() {
            return (sys, "system")
        }
        return (nil, "none")
    }

    func snapshot(for inst: ProviderInstanceConfig) -> AntigravityRuntimeSnapshot {
        let res = resolveBinary(for: inst)
        let managed = activeManaged()
        let hasManaged = managed != nil
        if isInstalling {
            let st = installStage.isEmpty ? (progressPct >= 0 ? "Downloading \(progressPct)%…" : "Installing…") : installStage
            return AntigravityRuntimeSnapshot(
                state: 4, statusText: st, buttonLabel: "Cancel installation",
                progressPct: progressPct, canRemove: false, errorText: "",
                resolvedBinary: res.path, version: managed?.version
            )
        }
        if !lastError.isEmpty {
            let btn = hasManaged ? "Reinstall Antigravity" : (res.path != nil ? "Install managed runtime" : "Install Antigravity")
            return AntigravityRuntimeSnapshot(
                state: 5, statusText: res.path != nil ? "Installed." : "Not installed.",
                buttonLabel: btn, progressPct: -1, canRemove: hasManaged, errorText: lastError,
                resolvedBinary: res.path, version: managed?.version
            )
        }
        if res.source == "managed", let m = managed {
            if let latest = latestManifest, Self.compareSemver(latest.version, m.version) > 0 {
                return AntigravityRuntimeSnapshot(
                    state: 3, statusText: "v\(m.version) · Update v\(latest.version) available.",
                    buttonLabel: "Update Antigravity", progressPct: -1, canRemove: true, errorText: "",
                    resolvedBinary: m.binaryPath, version: m.version
                )
            }
            return AntigravityRuntimeSnapshot(
                state: 2, statusText: "Managed runtime v\(m.version) installed.",
                buttonLabel: "Reinstall Antigravity", progressPct: -1, canRemove: true, errorText: "",
                resolvedBinary: m.binaryPath, version: m.version
            )
        }
        if res.path != nil {
            return AntigravityRuntimeSnapshot(
                state: 1, statusText: "Installed.",
                buttonLabel: "Install managed runtime", progressPct: -1, canRemove: hasManaged, errorText: "",
                resolvedBinary: res.path, version: CoderAntigravity.detectedVersion()
            )
        }
        return AntigravityRuntimeSnapshot(
            state: 0, statusText: "Not installed.",
            buttonLabel: "Install Antigravity", progressPct: -1, canRemove: hasManaged, errorText: "",
            resolvedBinary: nil, version: nil
        )
    }

    static func compareSemver(_ a: String, _ b: String) -> Int {
        let pa = a.split(separator: ".").compactMap { Int($0) }
        let pb = b.split(separator: ".").compactMap { Int($0) }
        for i in 0..<max(pa.count, pb.count) {
            let va = i < pa.count ? pa[i] : 0
            let vb = i < pb.count ? pb[i] : 0
            if va != vb { return va < vb ? -1 : 1 }
        }
        return 0
    }

    static func isSupportedVersion(_ v: String) -> Bool {
        let parts = v.split(separator: ".").compactMap { Int($0) }
        guard parts.count >= 2 else { return false }
        return parts[0] == 1 && parts[1] >= 5
    }

    func checkLatestRelease(completion: (() -> Void)? = nil) {
        var req = URLRequest(url: Self.manifestURL)
        req.timeoutInterval = 15
        URLSession.shared.dataTask(with: req) { [weak self] data, _, _ in
            guard let self, let data else {
                DispatchQueue.main.async { completion?() }
                return
            }
            if let manifest = try? JSONDecoder().decode(AntigravityReleaseManifest.self, from: data),
               Self.isSupportedVersion(manifest.version) {
                DispatchQueue.main.async {
                    self.latestManifest = manifest
                    self.onChange?()
                    completion?()
                }
                return
            }
            DispatchQueue.main.async { completion?() }
        }.resume()
    }

    func startInstallOrUpdate() {
        guard !isInstalling else { return }
        isInstalling = true
        progressPct = -1
        installStage = "Checking latest Antigravity release…"
        lastError = ""
        onChange?()

        var req = URLRequest(url: Self.manifestURL)
        req.timeoutInterval = 20
        URLSession.shared.dataTask(with: req) { [weak self] data, resp, err in
            guard let self else { return }
            if let err {
                self.finishInstallError("Could not fetch release manifest: \(err.localizedDescription)")
                return
            }
            guard let data,
                  let manifest = try? JSONDecoder().decode(AntigravityReleaseManifest.self, from: data),
                  let dlURL = URL(string: manifest.url) else {
                let code = (resp as? HTTPURLResponse)?.statusCode ?? 0
                self.finishInstallError("Invalid Antigravity release manifest (HTTP \(code)).")
                return
            }
            guard Self.isSupportedVersion(manifest.version) else {
                self.finishInstallError("Unsupported Antigravity CLI version \(manifest.version) (requires >= 1.5.0 < 2.0.0).")
                return
            }
            DispatchQueue.main.async {
                guard self.isInstalling else { return }
                self.latestManifest = manifest
                self.pendingManifest = manifest
                self.installStage = "Downloading v\(manifest.version)…"
                self.progressPct = 0
                self.onChange?()
                let cfg = URLSessionConfiguration.default
                cfg.timeoutIntervalForResource = 300
                let sess = URLSession(configuration: cfg, delegate: self, delegateQueue: nil)
                self.downloadSession = sess
                let task = sess.downloadTask(with: dlURL)
                self.downloadTask = task
                task.resume()
            }
        }.resume()
    }

    func cancelInstall() {
        downloadTask?.cancel()
        downloadTask = nil
        downloadSession?.invalidateAndCancel()
        downloadSession = nil
        pendingManifest = nil
        isInstalling = false
        progressPct = -1
        installStage = ""
        onChange?()
    }

    func removeManagedRuntime(instances: [ProviderInstanceConfig]) -> (ok: Bool, message: String) {
        if hasActiveManagedLease() {
            return (false, "Cannot remove managed runtime while an Antigravity chat is running.")
        }
        let root = managedRootURL.path
        for inst in instances {
            let bp = (inst.binaryPath.trimmingCharacters(in: .whitespacesAndNewlines) as NSString).expandingTildeInPath
            if !bp.isEmpty && bp.hasPrefix(root) {
                return (false, "Cannot remove managed runtime: \(inst.displayName) explicitly uses \(bp).")
            }
        }
        try? fm.removeItem(at: managedRootURL)
        lastError = ""
        CoderAntigravity.invalidateCache()
        onChange?()
        return (true, "Removed downloaded Antigravity runtime.")
    }

    // URLSessionDownloadDelegate
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) {
        let pct: Int32 = totalBytesExpectedToWrite > 0 ? Int32(min(100, max(0, (totalBytesWritten * 100) / totalBytesExpectedToWrite))) : -1
        DispatchQueue.main.async {
            guard self.isInstalling else { return }
            self.progressPct = pct
            if let v = self.pendingManifest?.version {
                self.installStage = pct >= 0 ? "Downloading v\(v) (\(pct)%)…" : "Downloading v\(v)…"
            }
            self.onChange?()
        }
    }

    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {
        guard let manifest = pendingManifest else { return }
        // Copy temp download before URLSession removes it on return.
        try? fm.createDirectory(at: stagingDirURL, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let archiveURL = stagingDirURL.appendingPathComponent("agy-\(manifest.version)-\(UUID().uuidString).tar.gz")
        do {
            try fm.copyItem(at: location, to: archiveURL)
        } catch {
            finishInstallError("Failed to stage archive: \(error.localizedDescription)")
            return
        }
        DispatchQueue.global(qos: .userInitiated).async {
            self.verifyAndActivate(archiveURL: archiveURL, manifest: manifest)
        }
    }

    func urlSession(_ session: URLSession, task: URLSessionTask, didCompleteWithError error: Error?) {
        if let err = error {
            if (err as NSError).code == NSURLErrorCancelled { return }
            finishInstallError("Download failed: \(err.localizedDescription)")
        }
    }

    private func verifyAndActivate(archiveURL: URL, manifest: AntigravityReleaseManifest) {
        defer { try? fm.removeItem(at: archiveURL) }
        DispatchQueue.main.async {
            self.installStage = "Verifying SHA-512 checksum…"
            self.progressPct = 100
            self.onChange?()
        }

        // 1. Cryptographic checksum verification (SHA-512 preferred, SHA-256 fallback).
        guard let fileData = try? Data(contentsOf: archiveURL) else {
            finishInstallError("Could not read downloaded archive.")
            return
        }
        if let expected512 = manifest.sha512?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(), !expected512.isEmpty {
            let actual = SHA512.hash(data: fileData).map { String(format: "%02x", $0) }.joined()
            if actual != expected512 {
                finishInstallError("SHA-512 checksum mismatch for Antigravity v\(manifest.version).")
                return
            }
        } else if let expected256 = manifest.sha256?.trimmingCharacters(in: .whitespacesAndNewlines).lowercased(), !expected256.isEmpty {
            let actual = SHA256.hash(data: fileData).map { String(format: "%02x", $0) }.joined()
            if actual != expected256 {
                finishInstallError("SHA-256 checksum mismatch for Antigravity v\(manifest.version).")
                return
            }
        }

        // 2. Extract tarball into staging dir.
        let extractDir = stagingDirURL.appendingPathComponent("extract-\(UUID().uuidString)", isDirectory: true)
        defer { try? fm.removeItem(at: extractDir) }
        try? fm.createDirectory(at: extractDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let tar = Process()
        tar.executableURL = URL(fileURLWithPath: "/usr/bin/tar")
        tar.arguments = ["-xzf", archiveURL.path, "-C", extractDir.path]
        tar.standardOutput = FileHandle.nullDevice
        tar.standardError = FileHandle.nullDevice
        do { try tar.run(); tar.waitUntilExit() } catch {
            finishInstallError("Extraction failed: \(error.localizedDescription)")
            return
        }
        guard tar.terminationStatus == 0 else {
            finishInstallError("Archive extraction exited with status \(tar.terminationStatus).")
            return
        }

        // Find extracted `agy` executable.
        guard let extractedBin = findAgyBinary(in: extractDir) else {
            finishInstallError("Downloaded archive did not contain an executable `agy` binary.")
            return
        }

        // 3. Live ACP initialize verification (port of AntigravityInstallation.ts verifyRuntime).
        DispatchQueue.main.async {
            self.installStage = "Verifying Antigravity ACP server…"
            self.onChange?()
        }
        if let err = verifyBinaryViaACPInitialize(binaryPath: extractedBin.path) {
            finishInstallError(err)
            return
        }

        // 4. Move into releases/<version> and lock permissions read-only (0555/0500).
        let finalDir = releasesDirURL.appendingPathComponent(manifest.version, isDirectory: true)
        try? fm.createDirectory(at: releasesDirURL, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        if fm.fileExists(atPath: finalDir.path) {
            // Make writable briefly before replacing existing version dir.
            makeWritable(finalDir)
            try? fm.removeItem(at: finalDir)
        }
        do {
            try fm.moveItem(at: extractDir, to: finalDir)
        } catch {
            finishInstallError("Could not activate release: \(error.localizedDescription)")
            return
        }
        let relSub = extractedBin.path.replacingOccurrences(of: extractDir.path, with: "")
        let finalBin = finalDir.path + relSub
        try? fm.setAttributes([.posixPermissions: 0o555], ofItemAtPath: finalBin)
        let active = ManagedRuntimeActive(version: manifest.version, binaryPath: finalBin, installedAt: Date())
        if let data = try? JSONEncoder().encode(active) {
            try? data.write(to: activeJSONURL, options: .atomic)
        }
        pruneOldReleases(keeping: manifest.version)
        CoderAntigravity.invalidateCache()

        DispatchQueue.main.async {
            self.isInstalling = false
            self.progressPct = -1
            self.installStage = ""
            self.lastError = ""
            self.downloadTask = nil
            self.downloadSession?.finishTasksAndInvalidate()
            self.downloadSession = nil
            self.pendingManifest = nil
            self.onChange?()
        }
    }

    private func findAgyBinary(in root: URL) -> URL? {
        if let en = fm.enumerator(at: root, includingPropertiesForKeys: [.isRegularFileKey]) {
            for case let u as URL in en {
                if u.lastPathComponent == "agy" && fm.isExecutableFile(atPath: u.path) { return u }
            }
        }
        return nil
    }

    /// Runs `<agy> acp` with an ACP `initialize` request to verify agentInfo name & version >= 1.5.0 < 2.0.0.
    private func verifyBinaryViaACPInitialize(binaryPath: String) -> String? {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: binaryPath)
        p.arguments = ["acp"]
        let into = Pipe(), out = Pipe()
        p.standardInput = into
        p.standardOutput = out
        p.standardError = FileHandle.nullDevice
        var env = ProcessInfo.processInfo.environment
        for k in AntigravityProfileManager.removedEnvKeys { env.removeValue(forKey: k) }
        env["AGY_CLI_DISABLE_AUTO_UPDATE"] = "1"
        env["AGY_ACP_FORCE_FILE_STORAGE"] = "1"
        p.environment = env
        do { try p.run() } catch { return "Could not start downloaded agy: \(error.localizedDescription)" }
        defer { if p.isRunning { p.terminate() } }

        let req = "{\"jsonrpc\":\"2.0\",\"id\":1,\"method\":\"initialize\",\"params\":{\"protocolVersion\":1,\"clientCapabilities\":{},\"clientInfo\":{\"name\":\"OpenAgents\",\"version\":\"1.0.0\"}}}\n"
        into.fileHandleForWriting.write(Data(req.utf8))
        try? into.fileHandleForWriting.close()

        let sem = DispatchSemaphore(value: 0)
        var line = ""
        DispatchQueue.global().async {
            let data = out.fileHandleForReading.readData(ofLength: 65536)
            line = String(decoding: data, as: UTF8.self)
            sem.signal()
        }
        if sem.wait(timeout: .now() + 10.0) == .timedOut {
            return "Timed out waiting for Antigravity ACP initialize response."
        }
        guard let firstLine = line.split(separator: "\n").first,
              let json = try? JSONSerialization.jsonObject(with: Data(firstLine.utf8)) as? [String: Any],
              let result = json["result"] as? [String: Any] else {
            return "Downloaded binary did not return a valid ACP initialize result."
        }
        if let info = result["agentInfo"] as? [String: Any] {
            let name = (info["name"] as? String ?? "").lowercased()
            let ver = info["version"] as? String ?? ""
            if !name.contains("antigravity") {
                return "Unexpected ACP agent '\(name)'."
            }
            if !ver.isEmpty && !Self.isSupportedVersion(ver) {
                return "Unsupported Antigravity ACP version \(ver)."
            }
        }
        return nil
    }

    private func makeWritable(_ url: URL) {
        if let en = fm.enumerator(at: url, includingPropertiesForKeys: nil) {
            for case let u as URL in en {
                try? fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: u.path)
            }
        }
        try? fm.setAttributes([.posixPermissions: 0o755], ofItemAtPath: url.path)
    }

    private func pruneOldReleases(keeping version: String) {
        guard let items = try? fm.contentsOfDirectory(atPath: releasesDirURL.path) else { return }
        for item in items where item != version {
            let dir = releasesDirURL.appendingPathComponent(item, isDirectory: true)
            lock.lock()
            let leased = activeLeases.contains { $0.key.hasPrefix(dir.path) && $0.value > 0 }
            lock.unlock()
            if !leased {
                makeWritable(dir)
                try? fm.removeItem(at: dir)
            }
        }
    }

    private func finishInstallError(_ msg: String) {
        DispatchQueue.main.async {
            self.isInstalling = false
            self.progressPct = -1
            self.installStage = ""
            self.lastError = msg
            self.downloadTask = nil
            self.downloadSession?.invalidateAndCancel()
            self.downloadSession = nil
            self.pendingManifest = nil
            self.onChange?()
        }
    }
}

// MARK: - Interactive Auth Manager (100% Port of AntigravityAuth.ts & antigravityCallback.ts)

final class AntigravityAuthManager {
    static let shared = AntigravityAuthManager()

    var onChange: ((String) -> Void)?
    var onAuthenticated: ((String) -> Void)?

    private struct ActiveFlow {
        var process: Process
        var codeFile: URL
        var openedBrowser: Bool
    }

    private var sessions: [String: ProviderAuthSession] = [:]
    private var flows: [String: ActiveFlow] = [:]

    func session(for inst: ProviderInstanceConfig, probe: ProviderProbeSnapshot?, hostName: String) -> ProviderAuthSession {
        if let cur = sessions[inst.id], cur.state != 0 {
            return cur
        }
        if probe?.authStatus == "authenticated" {
            let sub = inst.authMethod.usesBrowser
                ? "Signed in on \(hostName)."
                : "Connected on \(hostName) with \(inst.authMethod.label)."
            return ProviderAuthSession(state: 1, subtitle: sub, detail: "", authUrl: "", canSignOut: inst.authMethod.usesBrowser)
        }
        let sub: String
        switch inst.authMethod {
        case .oauthPersonal: sub = "Sign in with Google on \(hostName) to load models."
        case .oauthBusiness: sub = "Sign in with Gemini Enterprise on \(hostName) to load models."
        case .geminiApiKey: sub = "Connect with your Gemini API key on \(hostName) to load models."
        case .agentPlatform: sub = "Connect on \(hostName) to load models."
        }
        if let issue = AntigravityProfileManager.authConfigIssue(for: inst) {
            return ProviderAuthSession(state: 5, subtitle: sub, detail: issue, authUrl: "", canSignOut: false)
        }
        if let cur = sessions[inst.id] { return cur }
        let det = probe?.status == 3 || probe?.status == 2 ? (probe?.summary ?? "") : ""
        return ProviderAuthSession(state: det.isEmpty ? 0 : 5, subtitle: sub, detail: det, authUrl: "", canSignOut: false)
    }

    func buttonLabel(for inst: ProviderInstanceConfig, session: ProviderAuthSession) -> String {
        switch session.state {
        case 2, 3, 4: return "Cancel"
        case 1: return inst.authMethod.usesBrowser ? "Change account" : "Reconnect"
        case 5: return inst.authMethod.usesBrowser ? "Retry sign-in" : "Retry connection"
        default: return inst.authMethod.usesBrowser ? "Sign in" : "Connect"
        }
    }

    func startAuth(for inst: ProviderInstanceConfig, forceLogin: Bool, hostName: String) {
        cancelAuth(instanceId: inst.id)
        if let issue = AntigravityProfileManager.authConfigIssue(for: inst) {
            sessions[inst.id] = ProviderAuthSession(state: 5, subtitle: "Connect on \(hostName) to load models.", detail: issue, authUrl: "", canSignOut: false)
            onChange?(inst.id)
            return
        }
        guard let workerPath = Bundle.main.path(forResource: "agyworker", ofType: nil) else {
            sessions[inst.id] = ProviderAuthSession(state: 5, subtitle: "Connect on \(hostName) to load models.", detail: "Bundled agyworker binary is missing.", authUrl: "", canSignOut: false)
            onChange?(inst.id)
            return
        }
        let res = AntigravityRuntimeManager.shared.resolveBinary(for: inst)
        guard let agyBin = res.path else {
            sessions[inst.id] = ProviderAuthSession(state: 5, subtitle: "Install Antigravity first.", detail: "Antigravity CLI is not installed.", authUrl: "", canSignOut: false)
            onChange?(inst.id)
            return
        }

        let codeDir = AntigravityProfileManager.runtimeTempRootURL
        try? FileManager.default.createDirectory(at: codeDir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        let codeFile = codeDir.appendingPathComponent("auth-\(UUID().uuidString).code")
        try? FileManager.default.removeItem(at: codeFile)

        let p = Process()
        p.executableURL = URL(fileURLWithPath: workerPath)
        var args = ["--connect", "--code", codeFile.path]
        if forceLogin { args.append("--force-login") }
        p.arguments = args
        let out = Pipe()
        p.standardOutput = out
        p.standardError = out
        var env = ProcessInfo.processInfo.environment
        env["PATH"] = CoderWorker.path
        for (k, v) in AntigravityProfileManager.workerEnv(for: inst, resolvedBinary: agyBin) { env[k] = v }
        env["CODER_AGY_CODE_FILE"] = codeFile.path
        p.environment = env

        let sub = inst.authMethod.usesBrowser ? "Starting Google sign-in on \(hostName)…" : "Connecting with \(inst.authMethod.label) on \(hostName)…"
        sessions[inst.id] = ProviderAuthSession(state: 2, subtitle: sub, detail: "", authUrl: "", canSignOut: false)
        onChange?(inst.id)

        var buffer = Data()
        out.fileHandleForReading.readabilityHandler = { [weak self] handle in
            let chunk = handle.availableData
            if chunk.isEmpty { handle.readabilityHandler = nil; return }
            DispatchQueue.main.async {
                guard let self else { return }
                buffer.append(chunk)
                while let nl = buffer.firstIndex(of: 0x0a) {
                    let lineData = buffer.subdata(in: buffer.startIndex..<nl)
                    buffer.removeSubrange(buffer.startIndex...nl)
                    let line = String(decoding: lineData, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
                    if !line.isEmpty { self.handleAuthLine(line, instanceId: inst.id, inst: inst, hostName: hostName) }
                }
            }
        }
        p.terminationHandler = { [weak self] proc in
            DispatchQueue.main.async {
                guard let self else { return }
                if let flow = self.flows[inst.id], flow.process === proc {
                    try? FileManager.default.removeItem(at: flow.codeFile)
                    self.flows.removeValue(forKey: inst.id)
                    if let cur = self.sessions[inst.id], cur.state == 2 || cur.state == 3 || cur.state == 4 {
                        self.sessions[inst.id] = ProviderAuthSession(
                            state: 5,
                            subtitle: "Connect on \(hostName) to load models.",
                            detail: "Antigravity could not authenticate with the configured credentials.",
                            authUrl: "",
                            canSignOut: false
                        )
                        self.onChange?(inst.id)
                    }
                }
            }
        }
        do {
            try p.run()
            flows[inst.id] = ActiveFlow(process: p, codeFile: codeFile, openedBrowser: false)
        } catch {
            sessions[inst.id] = ProviderAuthSession(state: 5, subtitle: "Connect on \(hostName) to load models.", detail: error.localizedDescription, authUrl: "", canSignOut: false)
            onChange?(inst.id)
        }
    }

    private func handleAuthLine(_ line: String, instanceId: String, inst: ProviderInstanceConfig, hostName: String) {
        let parts = line.split(separator: "\t", maxSplits: 1).map(String.init)
        guard parts.count >= 2 else { return }
        let kind = parts[0], msg = parts[1]
        switch kind {
        case "url":
            sessions[instanceId] = ProviderAuthSession(
                state: 3,
                subtitle: "Finish sign-in in your browser on \(hostName).",
                detail: "",
                authUrl: msg,
                canSignOut: false
            )
            if var flow = flows[instanceId], !flow.openedBrowser, let url = URL(string: msg) {
                flow.openedBrowser = true
                flows[instanceId] = flow
                NSWorkspace.shared.open(url)
            }
            onChange?(instanceId)
        case "step":
            if var cur = sessions[instanceId], cur.state != 3 {
                cur.detail = msg
                sessions[instanceId] = cur
                onChange?(instanceId)
            }
        case "warn":
            if var cur = sessions[instanceId] {
                if !cur.authUrl.isEmpty { cur.state = 3 }
                cur.detail = msg
                sessions[instanceId] = cur
                onChange?(instanceId)
            }
        case "ok":
            sessions[instanceId] = ProviderAuthSession(
                state: 1,
                subtitle: "Connected on \(hostName).",
                detail: msg,
                authUrl: "",
                canSignOut: inst.authMethod.usesBrowser
            )
            onChange?(instanceId)
            onAuthenticated?(instanceId)
        case "fail":
            sessions[instanceId] = ProviderAuthSession(
                state: 5,
                subtitle: "Connect on \(hostName) to load models.",
                detail: msg.isEmpty ? "Antigravity could not authenticate with the configured credentials." : msg,
                authUrl: "",
                canSignOut: false
            )
            onChange?(instanceId)
        default:
            break
        }
    }

    /// Submits pasted authorization code or https://antigravity.google/oauth-callback?... URL.
    /// Also supports loopback redirect_uri forwarding when the auth URL targets 127.0.0.1.
    func submitCode(instanceId: String, rawInput: String) {
        let trimmed = rawInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let flow = flows[instanceId] else { return }

        // Check if auth URL used a loopback callback (http://127.0.0.1:<port>/...); if user pasted a callback URL, forward it!
        if let cur = sessions[instanceId], !cur.authUrl.isEmpty,
           let authComps = URLComponents(string: cur.authUrl),
           let redir = authComps.queryItems?.first(where: { $0.name == "redirect_uri" })?.value,
           let redirURL = URL(string: redir),
           redirURL.host == "127.0.0.1" || redirURL.host == "localhost" {
            if let pastedURL = URL(string: trimmed), let comps = URLComponents(url: pastedURL, resolvingAgainstBaseURL: false),
               let query = comps.percentEncodedQuery, !query.isEmpty,
               var target = URLComponents(url: redirURL, resolvingAgainstBaseURL: false) {
                target.percentEncodedQuery = query
                if let fwdURL = target.url {
                    var req = URLRequest(url: fwdURL)
                    req.timeoutInterval = 10
                    URLSession.shared.dataTask(with: req).resume()
                }
            }
        }

        // Write raw input (code or callback URL) to 0600 file consumed by `agyworker --connect`.
        try? Data(trimmed.utf8).write(to: flow.codeFile, options: .atomic)
        try? FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: flow.codeFile.path)
        if var cur = sessions[instanceId] {
            cur.state = 4
            cur.detail = "Verifying authorization code…"
            sessions[instanceId] = cur
            onChange?(instanceId)
        }
    }

    func cancelAuth(instanceId: String) {
        if let flow = flows.removeValue(forKey: instanceId) {
            if flow.process.isRunning { flow.process.terminate() }
            try? FileManager.default.removeItem(at: flow.codeFile)
        }
        sessions.removeValue(forKey: instanceId)
        onChange?(instanceId)
    }

    func signOut(for inst: ProviderInstanceConfig, completion: @escaping () -> Void) {
        cancelAuth(instanceId: inst.id)
        let res = AntigravityRuntimeManager.shared.resolveBinary(for: inst)
        let workerPath = Bundle.main.path(forResource: "agyworker", ofType: nil)
        let envMap = AntigravityProfileManager.workerEnv(for: inst, resolvedBinary: res.path)
        let acpDir = AntigravityProfileManager.acpDataDir(for: inst.id)

        DispatchQueue.global(qos: .userInitiated).async {
            if let wp = workerPath, res.path != nil {
                let p = Process()
                p.executableURL = URL(fileURLWithPath: wp)
                p.arguments = ["--logout"]
                var env = ProcessInfo.processInfo.environment
                env["PATH"] = CoderWorker.path
                for (k, v) in envMap { env[k] = v }
                p.environment = env
                p.standardOutput = FileHandle.nullDevice
                p.standardError = FileHandle.nullDevice
                try? p.run()
                p.waitUntilExit()
            }
            // Also clean any file-backed OAuth token cache inside the isolated profile.
            for name in ["acp_token.json", "antigravity-oauth-token", "oauth_creds.json"] {
                try? FileManager.default.removeItem(at: acpDir.appendingPathComponent(name))
            }
            DispatchQueue.main.async {
                self.sessions.removeValue(forKey: inst.id)
                self.onChange?(inst.id)
                completion()
            }
        }
    }
}

// MARK: - Usage Limits Prober (100% Port of antigravityUsageLimits.ts)

enum AntigravityUsageLimitsProber {
    private static let quotaEndpoints = [
        "https://daily-cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
        "https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuotaSummary",
    ]
    private static let modelsFallbackEndpoint = "https://cloudcode-pa.googleapis.com/v1internal:fetchAvailableModels"

    /// Reads OAuth access token from isolated profile files or macOS Keychain (`gemini:antigravity`),
    /// decoding Go keyring prefixes (`go-keyring-base64:`, `go-keyring-encoded:`). Never logs the token.
    static func readAccessToken(for instanceId: String) -> String? {
        let acpDir = AntigravityProfileManager.acpDataDir(for: instanceId)
        let candidates = [
            acpDir.appendingPathComponent("acp_token.json"),
            acpDir.appendingPathComponent("antigravity-oauth-token"),
            acpDir.appendingPathComponent("oauth_creds.json"),
        ]
        for file in candidates {
            if let data = try? Data(contentsOf: file), let tok = extractAccessToken(from: data) {
                return tok
            }
        }
        // Fallback: macOS Keychain entry written by agy CLI (`gemini:antigravity`).
        let query: [String: Any] = [
            kSecClass as String: kSecClassGenericPassword,
            kSecAttrService as String: "gemini:antigravity",
            kSecReturnData as String: true,
            kSecMatchLimit as String: kSecMatchLimitOne,
        ]
        var result: AnyObject?
        if SecItemCopyMatching(query as CFDictionary, &result) == errSecSuccess,
           let data = result as? Data,
           let tok = extractAccessToken(from: data) {
            return tok
        }
        return nil
    }

    private static func extractAccessToken(from raw: Data) -> String? {
        var text = String(decoding: raw, as: UTF8.self).trimmingCharacters(in: .whitespacesAndNewlines)
        if text.hasPrefix("go-keyring-base64:") {
            let b64 = String(text.dropFirst("go-keyring-base64:".count))
            if let dec = Data(base64Encoded: b64) { text = String(decoding: dec, as: UTF8.self) }
        } else if text.hasPrefix("go-keyring-encoded:") {
            let b64 = String(text.dropFirst("go-keyring-encoded:".count))
            if let dec = Data(base64Encoded: b64) { text = String(decoding: dec, as: UTF8.self) }
        }
        if let json = try? JSONSerialization.jsonObject(with: Data(text.utf8)) as? [String: Any] {
            if let t = json["access_token"] as? String, !t.isEmpty { return t }
            if let t = json["accessToken"] as? String, !t.isEmpty { return t }
            if let nested = json["token"] as? [String: Any],
               let t = (nested["access_token"] as? String) ?? (nested["accessToken"] as? String), !t.isEmpty {
                return t
            }
        }
        return nil
    }

    #if OA_FULL
    static func probeQuotaWindows(for inst: ProviderInstanceConfig, completion: @escaping (UsageAccountSnapshot?) -> Void) {
        guard inst.driver == .antigravity, inst.enabled, inst.authMethod == .oauthPersonal else {
            completion(nil)
            return
        }
        DispatchQueue.global(qos: .utility).async {
            guard let token = readAccessToken(for: inst.id) else {
                DispatchQueue.main.async { completion(nil) }
                return
            }
            let nowMs = Int64(Date().timeIntervalSince1970 * 1000)
            for ep in quotaEndpoints {
                if let snap = fetchQuotaSummary(endpoint: ep, token: token, profile: inst.displayName, nowMs: nowMs) {
                    DispatchQueue.main.async { completion(snap) }
                    return
                }
            }
            let fallback = fetchModelsFallback(token: token, profile: inst.displayName, nowMs: nowMs)
            DispatchQueue.main.async { completion(fallback) }
        }
    }

    private static func fetchQuotaSummary(endpoint: String, token: String, profile: String, nowMs: Int64) -> UsageAccountSnapshot? {
        guard let url = URL(string: endpoint) else { return nil }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 12
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data("{}".utf8)
        let sem = DispatchSemaphore(value: 0)
        var outData: Data?
        URLSession.shared.dataTask(with: req) { data, resp, _ in
            if let http = resp as? HTTPURLResponse, (200...299).contains(http.statusCode) { outData = data }
            sem.signal()
        }.resume()
        guard sem.wait(timeout: .now() + 13) == .success, let data = outData,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let buckets = json["buckets"] as? [[String: Any]], !buckets.isEmpty else { return nil }
        var wins: [UsageWindowSnapshot] = []
        var limited = false
        var limitReset: Int64 = 0
        let iso = ISO8601DateFormatter()
        for b in buckets {
            let modelId = (b["modelId"] as? String) ?? (b["bucketId"] as? String) ?? "Quota"
            let rem = (b["remainingFraction"] as? Double) ?? 1.0
            let usedBp = Int32(max(0, min(10000, Int(((1.0 - rem) * 10000.0).rounded()))))
            var resetS: Int64 = 0
            if let rt = b["resetTime"] as? String, let dt = iso.date(from: rt) {
                resetS = max(0, Int64(dt.timeIntervalSinceNow))
            }
            wins.append(UsageWindowSnapshot(label: modelId, usedBp: usedBp, resetS: resetS))
            if usedBp >= 10000 {
                limited = true
                if resetS > 0 && (limitReset == 0 || resetS < limitReset) { limitReset = resetS }
            }
        }
        guard !wins.isEmpty else { return nil }
        return UsageAccountSnapshot(
            provider: 2, profile: profile, plan: "Google Account",
            status: 1, why: "", gotMs: nowMs, limited: limited, limitResetS: limitReset, windows: wins
        )
    }

    private static func fetchModelsFallback(token: String, profile: String, nowMs: Int64) -> UsageAccountSnapshot? {
        guard let url = URL(string: modelsFallbackEndpoint) else { return nil }
        var req = URLRequest(url: url)
        req.httpMethod = "POST"
        req.timeoutInterval = 12
        req.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization")
        req.setValue("application/json", forHTTPHeaderField: "Content-Type")
        req.httpBody = Data("{}".utf8)
        let sem = DispatchSemaphore(value: 0)
        var outData: Data?
        URLSession.shared.dataTask(with: req) { data, resp, _ in
            if let http = resp as? HTTPURLResponse, (200...299).contains(http.statusCode) { outData = data }
            sem.signal()
        }.resume()
        guard sem.wait(timeout: .now() + 13) == .success, let data = outData,
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let models = json["models"] as? [String: Any] else { return nil }
        var wins: [UsageWindowSnapshot] = []
        let iso = ISO8601DateFormatter()
        for (k, v) in models.sorted(by: { $0.key < $1.key }) {
            guard let info = v as? [String: Any], let qi = info["quotaInfo"] as? [String: Any] else { continue }
            let rem = (qi["remainingFraction"] as? Double) ?? 1.0
            let usedBp = Int32(max(0, min(10000, Int(((1.0 - rem) * 10000.0).rounded()))))
            var resetS: Int64 = 0
            if let rt = qi["resetTime"] as? String, let dt = iso.date(from: rt) {
                resetS = max(0, Int64(dt.timeIntervalSinceNow))
            }
            wins.append(UsageWindowSnapshot(label: k, usedBp: usedBp, resetS: resetS))
        }
        guard !wins.isEmpty else { return nil }
        return UsageAccountSnapshot(
            provider: 2, profile: profile, plan: "Google Account",
            status: 1, why: "", gotMs: nowMs, limited: wins.contains { $0.usedBp >= 10000 }, limitResetS: 0, windows: wins
        )
    }
    #endif
}

// MARK: - Antigravity Skills Discovery (100% Port of AntigravitySkills.ts)

struct AntigravityDiscoveredSkill {
    let name: String
    let description: String
    let path: String
    let source: String
}

enum AntigravitySkillsDiscovery {
    private struct SkillsConfig: Decodable {
        let inherits: Bool?
        let include_only: [String]?
        let exclude: [String]?
    }

    static func discover(workspace: URL?) -> [AntigravityDiscoveredSkill] {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser
        var roots: [(URL, String)] = []
        if let ws = workspace {
            roots.append((ws.appendingPathComponent(".gemini/skills"), "project"))
            roots.append((ws.appendingPathComponent(".agents/skills"), "project"))
            roots.append((ws.appendingPathComponent(".agent/skills"), "project"))
        }
        let cfgURL = home.appendingPathComponent(".gemini/config/skills.json")
        let cfg = (try? Data(contentsOf: cfgURL)).flatMap { try? JSONDecoder().decode(SkillsConfig.self, from: $0) }
        if cfg?.inherits ?? true {
            roots.append((home.appendingPathComponent(".gemini/config/skills"), "user"))
            roots.append((home.appendingPathComponent(".gemini/antigravity-cli/skills"), "user"))
        }

        let includeOnly = cfg?.include_only.map { Set($0) }
        let exclude = Set(cfg?.exclude ?? [])
        var seen = Set<String>()
        var out: [AntigravityDiscoveredSkill] = []
        for (dir, src) in roots {
            guard let items = try? fm.contentsOfDirectory(atPath: dir.path) else { continue }
            for name in items.sorted() {
                if exclude.contains(name) { continue }
                if let inc = includeOnly, !inc.contains(name) { continue }
                if seen.contains(name) { continue }
                let skillFile = dir.appendingPathComponent(name).appendingPathComponent("SKILL.md")
                guard fm.fileExists(atPath: skillFile.path),
                      let text = try? String(contentsOf: skillFile, encoding: .utf8) else { continue }
                seen.insert(name)
                let desc = parseDescription(from: text)
                out.append(AntigravityDiscoveredSkill(name: name, description: desc, path: skillFile.path, source: src))
            }
        }
        return out
    }

    private static func parseDescription(from md: String) -> String {
        for line in md.components(separatedBy: .newlines).prefix(20) {
            let t = line.trimmingCharacters(in: .whitespaces)
            if t.lowercased().hasPrefix("description:") {
                return String(t.dropFirst("description:".count)).trimmingCharacters(in: CharacterSet(charactersIn: " \"'"))
            }
        }
        return ""
    }
}

// MARK: - Multi-Instance Provider Store & Live Prober

final class ProviderInstancesStore {
    static let shared = ProviderInstancesStore()

    var onChange: (() -> Void)?

    private let fm = FileManager.default
    private(set) var instances: [ProviderInstanceConfig] = []
    private(set) var probes: [String: ProviderProbeSnapshot] = [:]
    private(set) var selectedIndex: Int = 0
    private(set) var refreshing = false
    private(set) var lastCheckedAt: Date?

    var storeURL: URL {
        fm.homeDirectoryForCurrentUser.appendingPathComponent(".openagents/desktop/provider-instances.json")
    }

    var hostDisplayName: String {
        Host.current().localizedName ?? ProcessInfo.processInfo.hostName
    }

    init() {
        load()
        AntigravityRuntimeManager.shared.onChange = { [weak self] in
            self?.onChange?()
        }
        AntigravityAuthManager.shared.onChange = { [weak self] _ in
            self?.onChange?()
        }
        AntigravityAuthManager.shared.onAuthenticated = { [weak self] id in
            guard let self else { return }
            self.probeInstance(id: id)
        }
    }

    private static func findCliExecutable(_ names: [String]) -> String? {
        let fm = FileManager.default
        let home = fm.homeDirectoryForCurrentUser.path
        var dirs = CoderWorker.path.split(separator: ":").map(String.init)
        for extra in [home + "/.local/bin", "/opt/homebrew/bin", "/usr/local/bin", home + "/.cursor/bin"] {
            if !dirs.contains(extra) { dirs.append(extra) }
        }
        for name in names {
            for dir in dirs {
                let candidate = dir + "/" + name
                if fm.isExecutableFile(atPath: candidate) { return candidate }
            }
        }
        return nil
    }

    private static func codexCliStatus() -> (installed: Bool, auth: String) {
        let envBin = (ProcessInfo.processInfo.environment["CODER_CODEX_BIN"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let bin: String? = (!envBin.isEmpty && FileManager.default.isExecutableFile(atPath: (envBin as NSString).expandingTildeInPath))
            ? (envBin as NSString).expandingTildeInPath
            : findCliExecutable(["codex"])
        guard bin != nil else { return (false, "Codex CLI is not installed.") }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hasAuthFile = FileManager.default.fileExists(atPath: home + "/.codex/auth.json")
        let hasKey = !(ProcessInfo.processInfo.environment["OPENAI_API_KEY"] ?? "").isEmpty
        return (true, hasAuthFile ? "Signed in (~/.codex/auth.json)" : (hasKey ? "OPENAI_API_KEY set" : "Run `codex login` to sign in"))
    }

    private static func cursorCliStatus() -> (installed: Bool, auth: String) {
        let envBin = (ProcessInfo.processInfo.environment["CODER_CURSOR_BIN"] ?? "").trimmingCharacters(in: .whitespacesAndNewlines)
        let bin: String? = (!envBin.isEmpty && FileManager.default.isExecutableFile(atPath: (envBin as NSString).expandingTildeInPath))
            ? (envBin as NSString).expandingTildeInPath
            : findCliExecutable(["agent", "cursor-agent"])
        guard bin != nil else { return (false, "Cursor CLI is not installed.") }
        let home = FileManager.default.homeDirectoryForCurrentUser.path
        let hasCursorDir = FileManager.default.fileExists(atPath: home + "/.cursor")
        let hasKey = !(ProcessInfo.processInfo.environment["CURSOR_API_KEY"] ?? "").isEmpty
        return (true, hasKey ? "CURSOR_API_KEY set" : (hasCursorDir ? "Cursor profile detected (~/.cursor)" : "Run `agent login` to sign in"))
    }

    static func defaultInstances() -> [ProviderInstanceConfig] {
        [
            ProviderInstanceConfig(
                id: "antigravity", driver: .antigravity, displayName: "Antigravity",
                enabled: true,
                isDefault: true, accent: 0, authMethod: .oauthPersonal,
                gcpProject: "", gcpLocation: "us-central1",
                binaryPath: CoderAntigravity.bin, extraPath: "",
                envVars: [], models: []
            ),
        ]
    }

    /// Only expose Gemini 4 models from Antigravity (hides Gemini 3.x / Orchestrator clutter).
    static func isGemini4Model(slug: String, name: String) -> Bool {
        let s = slug.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if s.hasPrefix("gemini-4-") || s.hasPrefix("gemini-4.") || s == "gemini-4" { return true }
        if n.hasPrefix("gemini 4 ") || n == "gemini 4" { return true }
        return false
    }

    static func isGemini4Neon(slug: String, name: String) -> Bool {
        let s = slug.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        return s == "gemini-4-neon" || s.hasPrefix("gemini-4-neon-") || n == "gemini 4 neon" || n.hasPrefix("gemini 4 neon ")
    }

    private static func gemini4OrderRank(slug: String, name: String) -> Int {
        let s = slug.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let n = name.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        if s == "gemini-4-neon" || s.hasPrefix("gemini-4-neon-") || n.hasPrefix("gemini 4 neon") { return 0 }
        if s == "gemini-4-carbon" || s.hasPrefix("gemini-4-carbon-") || n.hasPrefix("gemini 4 carbon") { return 1 }
        if s == "gemini-4-hydrogen" || s.hasPrefix("gemini-4-hydrogen-") || n.hasPrefix("gemini 4 hydrogen") { return 2 }
        if s == "gemini-4-argon" || s.hasPrefix("gemini-4-argon-") || n.hasPrefix("gemini 4 argon") { return 3 }
        return 4
    }

    /// Filter Antigravity models to Gemini 4 only, order Neon first, and default to Gemini 4 Neon.
    static func normalizeAntigravityModels(_ models: [ProviderModelConfig], fallbackDefaultSlug: String = "") -> [ProviderModelConfig] {
        let filtered = models.filter { isGemini4Model(slug: $0.slug, name: $0.name) }
        var kept = filtered.enumerated().sorted { lhs, rhs in
            let ra = gemini4OrderRank(slug: lhs.element.slug, name: lhs.element.name)
            let rb = gemini4OrderRank(slug: rhs.element.slug, name: rhs.element.name)
            return ra != rb ? ra < rb : lhs.offset < rhs.offset
        }.map { $0.element }
        let hasNeon = kept.contains { isGemini4Neon(slug: $0.slug, name: $0.name) }
        for i in kept.indices {
            if hasNeon {
                kept[i].isDefault = isGemini4Neon(slug: kept[i].slug, name: kept[i].name)
            } else if !fallbackDefaultSlug.isEmpty {
                kept[i].isDefault = (kept[i].slug == fallbackDefaultSlug)
            }
        }
        return kept
    }

    func load() {
        if let data = try? Data(contentsOf: storeURL),
           let decoded = try? JSONDecoder().decode([ProviderInstanceConfig].self, from: data),
           !decoded.isEmpty {
            var filtered = decoded.filter { $0.driver == .antigravity }
            if !filtered.isEmpty {
                var changed = (filtered.count != decoded.count)
                for i in filtered.indices {
                    let norm = Self.normalizeAntigravityModels(filtered[i].models)
                    if norm != filtered[i].models {
                        filtered[i].models = norm
                        changed = true
                    }
                }
                instances = filtered
                if changed { save() }
            } else {
                instances = Self.defaultInstances()
                save()
            }
        } else {
            instances = Self.defaultInstances()
            save()
        }
        if selectedIndex >= instances.count { selectedIndex = max(0, instances.count - 1) }
        seedImmediateProbes()
    }

    func save() {
        let dir = storeURL.deletingLastPathComponent()
        try? fm.createDirectory(at: dir, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        // Never persist secret env var values in JSON.
        let sanitized = instances.map { inst -> ProviderInstanceConfig in
            var copy = inst
            copy.envVars = inst.envVars.map { ev in
                ev.secret ? ProviderEnvVarConfig(key: ev.key, value: "", secret: true) : ev
            }
            return copy
        }
        if let data = try? JSONEncoder().encode(sanitized) {
            try? data.write(to: storeURL, options: .atomic)
            try? fm.setAttributes([.posixPermissions: 0o600], ofItemAtPath: storeURL.path)
        }
    }

    var selectedInstance: ProviderInstanceConfig? {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return nil }
        return instances[selectedIndex]
    }

    func instance(id: String) -> ProviderInstanceConfig? {
        guard !id.isEmpty else { return nil }
        return instances.first { $0.id == id }
    }

    func activeAntigravityInstance() -> ProviderInstanceConfig? {
        if let sel = selectedInstance, sel.driver == .antigravity, sel.enabled { return sel }
        return instances.first { $0.driver == .antigravity && $0.enabled }
            ?? instances.first { $0.driver == .antigravity }
    }

    /// Enabled models from authenticated (or previously loaded, not explicitly unauthenticated)
    /// Antigravity instances, favorites first then user order, capped at 16 for the composer picker.
    func loadedPickerModels() -> [(instId: String, instName: String, model: ProviderModelConfig)] {
        var favs: [(instId: String, instName: String, model: ProviderModelConfig)] = []
        var rest: [(instId: String, instName: String, model: ProviderModelConfig)] = []
        for inst in instances where inst.driver == .antigravity && inst.enabled {
            if probes[inst.id]?.authStatus == "unauthenticated" { continue }
            for m in inst.models where m.enabled && !m.slug.isEmpty && Self.isGemini4Model(slug: m.slug, name: m.name) {
                let item = (inst.id, inst.displayName, m)
                if m.favorite { favs.append(item) } else { rest.append(item) }
            }
        }
        return Array((favs + rest).prefix(16))
    }

    var hasLoadedAntigravityModels: Bool {
        !loadedPickerModels().isEmpty
    }

    var hasEnabledAntigravityBinary: Bool {
        instances.contains { inst in
            inst.driver == .antigravity && inst.enabled && AntigravityRuntimeManager.shared.resolveBinary(for: inst).path != nil
        }
    }

    func select(index: Int) {
        guard index >= 0 && index < instances.count else { return }
        selectedIndex = index
        onChange?()
    }

    func setEnabled(index: Int, enabled: Bool) {
        guard index >= 0 && index < instances.count else { return }
        instances[index].enabled = enabled
        save()
        if enabled { probeInstance(id: instances[index].id) }
        else { seedImmediateProbe(for: instances[index]) }
        onChange?()
    }

    func persistSelectedConfigOnly(displayName: String, accent: Int, authMethod: AntigravityAuthMethod, newApiKey: String, gcpProject: String, gcpLocation: String, binaryPath: String, extraPath: String) {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        var inst = instances[selectedIndex]
        let cleanName = displayName.trimmingCharacters(in: .whitespacesAndNewlines)
        if !cleanName.isEmpty { inst.displayName = cleanName }
        inst.accent = max(0, min(7, accent))
        inst.authMethod = authMethod
        inst.gcpProject = gcpProject.trimmingCharacters(in: .whitespacesAndNewlines)
        inst.gcpLocation = gcpLocation.trimmingCharacters(in: .whitespacesAndNewlines)
        inst.binaryPath = binaryPath.trimmingCharacters(in: .whitespacesAndNewlines)
        inst.extraPath = extraPath.trimmingCharacters(in: .whitespacesAndNewlines)
        let trimmedKey = newApiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedKey.isEmpty {
            ProviderSecretStore.setApiKey(trimmedKey, for: inst.id)
        }
        instances[selectedIndex] = inst
        if inst.id == "antigravity" {
            CoderAntigravity.bin = inst.binaryPath
        }
        if inst.driver == .antigravity {
            _ = AntigravityProfileManager.prepareProfile(for: inst)
        }
        save()
    }

    func updateSelectedConfig(displayName: String, accent: Int, authMethod: AntigravityAuthMethod, newApiKey: String, gcpProject: String, gcpLocation: String, binaryPath: String, extraPath: String) {
        persistSelectedConfigOnly(
            displayName: displayName, accent: accent, authMethod: authMethod,
            newApiKey: newApiKey, gcpProject: gcpProject, gcpLocation: gcpLocation,
            binaryPath: binaryPath, extraPath: extraPath
        )
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        let inst = instances[selectedIndex]
        if inst.enabled { probeInstance(id: inst.id) }
        else { seedImmediateProbe(for: inst) }
        onChange?()
    }

    func markSignedOut(instanceId: String) {
        guard let idx = instances.firstIndex(where: { $0.id == instanceId }) else { return }
        instances[idx].models = instances[idx].models.filter { $0.custom }
        save()
        let inst = instances[idx]
        probes[instanceId] = ProviderProbeSnapshot(
            status: 2, headline: "Needs attention",
            summary: "Antigravity is installed. Connect with \(inst.authMethod.label) to load models.",
            version: probes[instanceId]?.version ?? (CoderAntigravity.detectedVersion().map { "v\($0)" } ?? ""),
            authStatus: "unauthenticated", hasAutoMode: false, defaultModel: "", checkedAt: Date()
        )
        onChange?()
    }

    func clearSelectedApiKey() {
        guard let inst = selectedInstance else { return }
        ProviderSecretStore.setApiKey(nil, for: inst.id)
        if inst.enabled { probeInstance(id: inst.id) }
        onChange?()
    }

    func updateSelectedEnvVars(_ vars: [(key: String, val: String, secret: Bool, hasSecret: Bool)]) {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        let instId = instances[selectedIndex].id
        var cleaned: [ProviderEnvVarConfig] = []
        for row in vars {
            let k = row.key.trimmingCharacters(in: .whitespacesAndNewlines)
            if k.isEmpty { continue }
            if row.secret {
                let v = row.val.trimmingCharacters(in: .whitespacesAndNewlines)
                if !v.isEmpty { ProviderSecretStore.setEnvSecret(v, for: instId, key: k) }
                cleaned.append(ProviderEnvVarConfig(key: k, value: "", secret: true))
            } else {
                ProviderSecretStore.setEnvSecret(nil, for: instId, key: k)
                cleaned.append(ProviderEnvVarConfig(key: k, value: row.val, secret: false))
            }
        }
        instances[selectedIndex].envVars = cleaned
        save()
        onChange?()
    }

    func updateSelectedModels(_ models: [ProviderModelConfig]) {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        instances[selectedIndex].models = models
        save()
        onChange?()
    }

    func resetSelectedDefault() {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        let cur = instances[selectedIndex]
        guard cur.isDefault else { return }
        ProviderSecretStore.removeAll(for: cur.id)
        if let def = Self.defaultInstances().first(where: { $0.id == cur.id }) {
            instances[selectedIndex] = def
        }
        if cur.driver == .antigravity {
            _ = AntigravityProfileManager.prepareProfile(for: instances[selectedIndex])
        }
        save()
        probeInstance(id: cur.id)
        onChange?()
    }

    func deleteSelectedCustom() {
        guard selectedIndex >= 0 && selectedIndex < instances.count else { return }
        let cur = instances[selectedIndex]
        guard !cur.isDefault else { return }
        AntigravityAuthManager.shared.cancelAuth(instanceId: cur.id)
        ProviderSecretStore.removeAll(for: cur.id)
        probes.removeValue(forKey: cur.id)
        instances.remove(at: selectedIndex)
        selectedIndex = min(selectedIndex, max(0, instances.count - 1))
        save()
        onChange?()
    }

    func addInstance(driver: ProviderDriverKind, label: String, id: String, accent: Int, authMethod: AntigravityAuthMethod, apiKey: String, gcpProject: String, gcpLocation: String, binaryPath: String, extraPath: String) {
        let cleanId = id.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
        let finalId = cleanId.isEmpty ? "\(driver.rawValue)-\(instances.count + 1)" : cleanId
        let cleanName = label.trimmingCharacters(in: .whitespacesAndNewlines)
        let inst = ProviderInstanceConfig(
            id: finalId, driver: driver,
            displayName: cleanName.isEmpty ? driver.defaultDisplayName : cleanName,
            enabled: true, isDefault: false, accent: max(0, min(7, accent)),
            authMethod: authMethod,
            gcpProject: gcpProject.trimmingCharacters(in: .whitespacesAndNewlines),
            gcpLocation: gcpLocation.trimmingCharacters(in: .whitespacesAndNewlines),
            binaryPath: binaryPath.trimmingCharacters(in: .whitespacesAndNewlines),
            extraPath: extraPath.trimmingCharacters(in: .whitespacesAndNewlines),
            envVars: [], models: []
        )
        let trimmedKey = apiKey.trimmingCharacters(in: .whitespacesAndNewlines)
        if !trimmedKey.isEmpty {
            ProviderSecretStore.setApiKey(trimmedKey, for: finalId)
        }
        if inst.driver == .antigravity {
            _ = AntigravityProfileManager.prepareProfile(for: inst)
        }
        instances.append(inst)
        selectedIndex = instances.count - 1
        save()
        seedImmediateProbe(for: inst)
        probeInstance(id: inst.id)
        onChange?()
    }

    func checkedAgeLabel() -> String {
        guard let t = lastCheckedAt else { return "Check now" }
        let s = max(0, Int(Date().timeIntervalSince(t)))
        if s < 45 { return "Checked just now" }
        if s < 3600 { return "Checked \(s / 60)m ago" }
        if s < 86400 { return "Checked \(s / 3600)h ago" }
        return "Checked \(s / 86400)d ago"
    }

    private func seedImmediateProbes() {
        for inst in instances { seedImmediateProbe(for: inst) }
    }

    private func seedImmediateProbe(for inst: ProviderInstanceConfig) {
        if !inst.enabled {
            probes[inst.id] = ProviderProbeSnapshot(status: 0, headline: "Disabled", summary: "", version: "", authStatus: "unknown", hasAutoMode: false, defaultModel: "", checkedAt: Date())
            return
        }
        switch inst.driver {
        case .codex:
            let st = Self.codexCliStatus()
            probes[inst.id] = ProviderProbeSnapshot(
                status: st.installed ? 1 : 2,
                headline: st.installed ? "Ready" : "Needs attention",
                summary: st.installed ? st.auth : "Codex CLI is not installed.",
                version: "", authStatus: st.installed ? "authenticated" : "unauthenticated",
                hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
        case .claude:
            let ok = CoderWorker.hasClaude
            probes[inst.id] = ProviderProbeSnapshot(
                status: ok ? 1 : 2,
                headline: ok ? "Ready" : "Needs attention",
                summary: ok ? "Claude Code CLI detected." : "Claude Code CLI is not installed.",
                version: "", authStatus: ok ? "authenticated" : "unauthenticated",
                hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
        case .cursor:
            let st = Self.cursorCliStatus()
            probes[inst.id] = ProviderProbeSnapshot(
                status: st.installed ? 1 : 2,
                headline: st.installed ? "Ready" : "Needs attention",
                summary: st.installed ? st.auth : "Cursor CLI is not installed.",
                version: "", authStatus: st.installed ? "authenticated" : "unauthenticated",
                hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
        case .acp:
            let bp = (inst.binaryPath as NSString).expandingTildeInPath
            let ok = !bp.isEmpty && fm.isExecutableFile(atPath: bp)
            probes[inst.id] = ProviderProbeSnapshot(
                status: ok ? 1 : 2,
                headline: ok ? "Ready" : "Needs attention",
                summary: ok ? bp : "Set a valid ACP binary path.",
                version: "", authStatus: ok ? "authenticated" : "unknown",
                hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
        case .antigravity:
            let res = AntigravityRuntimeManager.shared.resolveBinary(for: inst)
            if res.path == nil {
                probes[inst.id] = ProviderProbeSnapshot(
                    status: 2, headline: "Needs attention",
                    summary: "Antigravity is not installed. Install managed runtime to load models.",
                    version: "", authStatus: "unauthenticated", hasAutoMode: false, defaultModel: "", checkedAt: Date()
                )
            } else if let issue = AntigravityProfileManager.authConfigIssue(for: inst) {
                probes[inst.id] = ProviderProbeSnapshot(
                    status: 2, headline: "Needs attention", summary: issue,
                    version: CoderAntigravity.detectedVersion().map { "v\($0)" } ?? "",
                    authStatus: "unauthenticated", hasAutoMode: false, defaultModel: "", checkedAt: Date()
                )
            } else if inst.models.contains(where: { !$0.custom }) {
                probes[inst.id] = ProviderProbeSnapshot(
                    status: 1, headline: "Ready",
                    summary: "Checking Antigravity access…",
                    version: CoderAntigravity.detectedVersion().map { "v\($0)" } ?? "",
                    authStatus: "unknown", hasAutoMode: false, defaultModel: "", checkedAt: Date()
                )
            } else {
                probes[inst.id] = ProviderProbeSnapshot(
                    status: 2, headline: "Needs attention",
                    summary: "Antigravity is installed. Connect with \(inst.authMethod.label) to load models.",
                    version: CoderAntigravity.detectedVersion().map { "v\($0)" } ?? "",
                    authStatus: "unknown", hasAutoMode: false, defaultModel: "", checkedAt: Date()
                )
            }
        }
    }

    func refreshAll() {
        guard !refreshing else { return }
        refreshing = true
        onChange?()
        AntigravityRuntimeManager.shared.checkLatestRelease()

        let targets = instances
        let group = DispatchGroup()
        for inst in targets {
            if !inst.enabled || inst.driver != .antigravity {
                seedImmediateProbe(for: inst)
                continue
            }
            group.enter()
            runAntigravityProbe(inst) {
                group.leave()
            }
        }
        group.notify(queue: .main) {
            self.refreshing = false
            self.lastCheckedAt = Date()
            self.onChange?()
        }
    }

    func probeInstance(id: String) {
        guard let inst = instances.first(where: { $0.id == id }) else { return }
        if !inst.enabled || inst.driver != .antigravity {
            seedImmediateProbe(for: inst)
            lastCheckedAt = Date()
            onChange?()
            return
        }
        runAntigravityProbe(inst) {
            self.lastCheckedAt = Date()
            self.onChange?()
        }
    }

    private func runAntigravityProbe(_ inst: ProviderInstanceConfig, done: @escaping () -> Void) {
        let res = AntigravityRuntimeManager.shared.resolveBinary(for: inst)
        guard let agyBin = res.path else {
            seedImmediateProbe(for: inst)
            done()
            return
        }
        if let issue = AntigravityProfileManager.authConfigIssue(for: inst) {
            probes[inst.id] = ProviderProbeSnapshot(
                status: 2, headline: "Needs attention", summary: issue,
                version: CoderAntigravity.detectedVersion().map { "v\($0)" } ?? "",
                authStatus: "unauthenticated", hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
            done()
            return
        }
        guard let workerPath = Bundle.main.path(forResource: "agyworker", ofType: nil) else {
            seedImmediateProbe(for: inst)
            done()
            return
        }
        let envMap = AntigravityProfileManager.workerEnv(for: inst, resolvedBinary: agyBin)
        AntigravityRuntimeManager.shared.acquireLease(binaryPath: agyBin)

        DispatchQueue.global(qos: .utility).async {
            defer { AntigravityRuntimeManager.shared.releaseLease(binaryPath: agyBin) }
            let p = Process()
            p.executableURL = URL(fileURLWithPath: workerPath)
            p.arguments = ["--probe-json"]
            let pipe = Pipe()
            p.standardOutput = pipe
            p.standardError = FileHandle.nullDevice
            var env = ProcessInfo.processInfo.environment
            env["PATH"] = CoderWorker.path
            for (k, v) in envMap { env[k] = v }
            p.environment = env
            var rawOut = ""
            if (try? p.run()) != nil {
                let sem = DispatchSemaphore(value: 0)
                DispatchQueue.global().async {
                    let data = pipe.fileHandleForReading.readDataToEndOfFile()
                    rawOut = String(decoding: data, as: UTF8.self)
                    sem.signal()
                }
                if sem.wait(timeout: .now() + 35) == .timedOut {
                    if p.isRunning { p.terminate() }
                }
            }
            let jsonLine = rawOut.split(separator: "\n").last.map(String.init) ?? ""
            DispatchQueue.main.async {
                self.applyProbeJSON(jsonLine, for: inst)
                done()
            }
        }
    }

    private func applyProbeJSON(_ jsonLine: String, for inst: ProviderInstanceConfig) {
        guard !jsonLine.isEmpty,
              let obj = try? JSONSerialization.jsonObject(with: Data(jsonLine.utf8)) as? [String: Any] else {
            probes[inst.id] = ProviderProbeSnapshot(
                status: 2, headline: "Needs attention",
                summary: "Antigravity is installed. Connect with \(inst.authMethod.label) to load models.",
                version: CoderAntigravity.detectedVersion().map { "v\($0)" } ?? "",
                authStatus: "unauthenticated", hasAutoMode: false, defaultModel: "", checkedAt: Date()
            )
            return
        }
        let statusStr = (obj["status"] as? String) ?? ""
        let authSt = (obj["authStatus"] as? String) ?? (statusStr == "ready" ? "authenticated" : "unauthenticated")
        let ok = statusStr == "ready" || authSt == "authenticated" || ((obj["ok"] as? Bool) ?? false)
        let verRaw = (obj["version"] as? String) ?? ""
        let ver = verRaw.isEmpty ? "" : (verRaw.hasPrefix("v") ? verRaw : "v\(verRaw)")
        let msg = (obj["message"] as? String) ?? ""
        let hasAuto = (obj["hasAutoMode"] as? Bool) ?? false
        let defModel = (obj["defaultModel"] as? String) ?? ""
        let defModelName = (obj["defaultModelName"] as? String) ?? ""

        if ok, let rawModels = obj["models"] as? [[String: Any]] {
            var loadedCount = 0
            var chosenDefaultSlug = defModel
            var chosenDefaultName = defModelName
            if let idx = instances.firstIndex(where: { $0.id == inst.id }) {
                let existing = Dictionary(uniqueKeysWithValues: instances[idx].models.map { ($0.slug, $0) })
                var merged: [ProviderModelConfig] = []
                // Keep only Gemini 4 models, preserve enabled/favorite state, order Neon first & default to Neon.
                var seen = Set<String>()
                var bySlug: [String: ProviderModelConfig] = [:]
                for m in rawModels {
                    guard let slug = m["slug"] as? String, !slug.isEmpty else { continue }
                    let name = (m["name"] as? String) ?? slug
                    guard Self.isGemini4Model(slug: slug, name: name) else { continue }
                    var levels = (m["thoughtLevels"] as? [String]) ?? []
                    if levels.isEmpty, let detailed = m["thinkingLevels"] as? [[String: Any]] {
                        levels = detailed.compactMap { $0["value"] as? String }
                    }
                    let thinking = !levels.isEmpty || ((m["thinking"] as? Bool) ?? false)
                    let isDefault = (m["isDefault"] as? Bool) ?? (slug == defModel)
                    let defEffort = (m["thinkingDefault"] as? String) ?? "medium"
                    let prev = existing[slug]
                    bySlug[slug] = ProviderModelConfig(
                        slug: slug, name: name,
                        enabled: prev?.enabled ?? true,
                        favorite: prev?.favorite ?? false,
                        thinking: thinking,
                        thoughtLevels: levels,
                        custom: false,
                        isDefault: isDefault,
                        defaultEffort: defEffort.isEmpty ? "medium" : defEffort
                    )
                }
                for old in instances[idx].models {
                    if let fresh = bySlug[old.slug] { merged.append(fresh); seen.insert(old.slug) }
                    else if old.custom && Self.isGemini4Model(slug: old.slug, name: old.name) { merged.append(old); seen.insert(old.slug) }
                }
                for m in rawModels {
                    if let slug = m["slug"] as? String, !seen.contains(slug), let fresh = bySlug[slug] {
                        merged.append(fresh)
                        seen.insert(slug)
                    }
                }
                merged = Self.normalizeAntigravityModels(merged, fallbackDefaultSlug: defModel)
                instances[idx].models = merged
                loadedCount = merged.count
                if let defM = merged.first(where: { $0.isDefault }) ?? merged.first {
                    chosenDefaultSlug = defM.slug
                    chosenDefaultName = defM.name
                }
                save()
            }
            let summary = msg.isEmpty ? "Connected (\(loadedCount) \(loadedCount == 1 ? "model" : "models") loaded)." : msg
            probes[inst.id] = ProviderProbeSnapshot(
                status: 1, headline: "Ready", summary: summary, version: ver,
                authStatus: "authenticated", hasAutoMode: hasAuto, defaultModel: chosenDefaultSlug, checkedAt: Date()
            )
            // Keep legacy CoderProbes cache aligned for the composer chip.
            if inst.id == "antigravity" && !chosenDefaultSlug.isEmpty {
                let defName = !chosenDefaultName.isEmpty ? chosenDefaultName : chosenDefaultSlug
                CoderProbes.recordAgySeen(id: chosenDefaultSlug, name: defName)
            }
        } else {
            let needsAuth = authSt == "unauthenticated"
            // Exact T3 clearAccountMetadata parity: unauthenticated Antigravity must not keep stale models in the chat picker.
            if needsAuth, let idx = instances.firstIndex(where: { $0.id == inst.id }) {
                let keptCustom = instances[idx].models.filter { $0.custom }
                if keptCustom.count != instances[idx].models.count {
                    instances[idx].models = keptCustom
                    save()
                }
            }
            let summary = !msg.isEmpty ? msg
                : "Antigravity is installed. Connect with \(inst.authMethod.label) to load models."
            probes[inst.id] = ProviderProbeSnapshot(
                status: needsAuth ? 2 : 3,
                headline: needsAuth ? "Needs attention" : "Error",
                summary: summary,
                version: ver,
                authStatus: authSt,
                hasAutoMode: hasAuto,
                defaultModel: defModel,
                checkedAt: Date()
            )
        }
    }
}

// MARK: - CoderWindow Settings -> Providers (Core Basic Flow)

extension CoderWindow {
    func syncAntigravityModels(on targetPage: UnsafeMutableRawPointer) {
        let loaded = ProviderInstancesStore.shared.loadedPickerModels()
        cd_agy_models_clear(targetPage)
        for entry in loaded {
            let m = entry.model
            var mask: Int32 = 0
            for lvl in m.thoughtLevels {
                switch lvl.lowercased() {
                case "low": mask |= 1
                case "medium": mask |= 2
                case "high": mask |= 4
                case "xhigh": mask |= 8
                default: break
                }
            }
            if mask == 0 && m.thinking { mask = 1 | 2 | 4 }
            let defEff: Int32
            switch m.defaultEffort.lowercased() {
            case "low": defEff = 1
            case "medium": defEff = 2
            case "high": defEff = 3
            case "xhigh": defEff = 4
            default: defEff = 2
            }
            _ = cd_agy_models_add(targetPage, entry.instId, entry.instName, m.slug, m.name, mask, defEff, m.isDefault ? 1 : 0)
        }
        cd_agy(targetPage, loaded.isEmpty ? 0 : 1)
    }

    func setupProvidersSettings() {
        ProviderInstancesStore.shared.onChange = { [weak self] in
            DispatchQueue.main.async {
                guard let self else { return }
                self.syncProvidersSettingsPage()
                #if OA_FULL
                if FeatureFlags.isOn(.modelsAgy) { self.syncAntigravitySettings() }
                #endif
                self.syncAntigravityModels(on: self.page)
                for r in self.runs.values where r.page != self.page { self.syncAntigravityModels(on: r.page) }
                self.applyProbes()
                self.wake()
            }
        }
        syncProvidersSettingsPage()
        syncAntigravityModels(on: page)
        AntigravityRuntimeManager.shared.checkLatestRelease()
        // Probe enabled Antigravity instances asynchronously at launch so already-authenticated
        // accounts load real models into the composer picker automatically (never opens a browser).
        if ProviderInstancesStore.shared.hasEnabledAntigravityBinary {
            DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
                ProviderInstancesStore.shared.refreshAll()
            }
        }
    }

    func syncProvidersSettingsPage() {
        let store = ProviderInstancesStore.shared
        let host = store.hostDisplayName
        cd_ps_set_meta(page, host, "All projects", store.checkedAgeLabel(), store.refreshing ? 1 : 0)
        cd_ps_clear(page)
        for inst in store.instances {
            let probe = store.probes[inst.id]
            let status: Int32 = inst.enabled ? (probe?.status ?? 2) : 0
            let headline = inst.enabled ? (probe?.headline ?? "Needs attention") : "Disabled"
            let summary = inst.enabled ? (probe?.summary ?? "") : ""
            let version = probe?.version ?? ""
            _ = cd_ps_add_instance(
                page, inst.id, inst.driver.uiIndex, inst.displayName, version,
                inst.enabled ? 1 : 0, inst.isDefault ? 1 : 0,
                status, headline, summary, Int32(inst.accent)
            )
        }
        cd_ps_select(page, Int32(store.selectedIndex))

        guard let sel = store.selectedInstance else { return }
        let probe = store.probes[sel.id]
        cd_ps_set_detail_config(
            page, sel.displayName, Int32(sel.accent), sel.authMethod.uiIndex,
            ProviderSecretStore.hasApiKey(for: sel.id) ? 1 : 0,
            sel.gcpProject, sel.gcpLocation, sel.binaryPath, sel.extraPath
        )

        if sel.driver == .antigravity {
            let rt = AntigravityRuntimeManager.shared.snapshot(for: sel)
            cd_ps_set_detail_runtime(page, rt.state, rt.statusText, rt.buttonLabel, rt.progressPct, rt.canRemove ? 1 : 0, rt.errorText)
            let authSess = AntigravityAuthManager.shared.session(for: sel, probe: probe, hostName: host)
            let authBtn = AntigravityAuthManager.shared.buttonLabel(for: sel, session: authSess)
            cd_ps_set_detail_auth(page, authSess.state, authSess.subtitle, authSess.detail, authBtn, authSess.canSignOut ? 1 : 0, authSess.authUrl)
        } else {
            cd_ps_set_detail_runtime(page, 1, probe?.headline ?? "", "", -1, 0, "")
            let authSub = sel.enabled
                ? (probe?.summary ?? "Configured on \(host).")
                : "Enable \(sel.displayName) to configure credentials on \(host)."
            let authState: Int32 = (probe?.status == 1) ? 1 : 0
            cd_ps_set_detail_auth(page, authState, authSub, "", authState == 1 ? "Re-check" : "Connect", 0, "")
        }

        cd_ps_env_clear(page)
        for ev in sel.envVars {
            let hasSec = ev.secret && ProviderSecretStore.hasEnvSecret(for: sel.id, key: ev.key)
            _ = cd_ps_env_add(page, ev.key, ev.secret ? "" : ev.value, ev.secret ? 1 : 0, hasSec ? 1 : 0)
        }

        cd_ps_models_clear(page)
        for m in sel.models {
            _ = cd_ps_models_add(page, m.slug, m.name, m.enabled ? 1 : 0, m.favorite ? 1 : 0, m.thinking ? 1 : 0, m.custom ? 1 : 0)
        }

        if cd_settings_page(page) == 14 && store.lastCheckedAt == nil && !store.refreshing {
            store.refreshAll()
        }
    }

    private func psReadField(_ which: Int32) -> String {
        var buf = [CChar](repeating: 0, count: 1024)
        let n = cd_ps_field(page, which, &buf, Int32(buf.count))
        return n > 0 ? String(cString: buf) : ""
    }

    func providersSettingsAsked(_ what: Int32) {
        let store = ProviderInstancesStore.shared
        switch what {
        case 170: // CP_ASK_PS_REFRESH
            store.refreshAll()
        case 171: // CP_ASK_PS_SELECT
            let idx = Int(cd_ps_selected(page))
            store.select(index: idx)
        case 172: // CP_ASK_PS_TOGGLE
            let idx = Int(cd_ps_action_idx(page))
            let on = cd_ps_inst_enabled(page, Int32(idx)) != 0
            store.setEnabled(index: idx, enabled: on)
            configureWorkerEnv(run, isForeman: isForemanShown)
        case 173: // CP_ASK_PS_CONFIG
            let name = psReadField(0)
            let apiKey = psReadField(1)
            let gcpProj = psReadField(2)
            let gcpLoc = psReadField(3)
            let binPath = psReadField(4)
            let extraPath = psReadField(5)
            let accent = Int(cd_ps_int(page, 0))
            let authMethod = AntigravityAuthMethod.from(uiIndex: cd_ps_int(page, 1))
            store.updateSelectedConfig(
                displayName: name, accent: accent, authMethod: authMethod,
                newApiKey: apiKey, gcpProject: gcpProj, gcpLocation: gcpLoc,
                binaryPath: binPath, extraPath: extraPath
            )
            configureWorkerEnv(run, isForeman: isForemanShown)
        case 174: // CP_ASK_PS_API_CLEAR
            store.clearSelectedApiKey()
            configureWorkerEnv(run, isForeman: isForemanShown)
        case 175: // CP_ASK_PS_RESET
            store.resetSelectedDefault()
            configureWorkerEnv(run, isForeman: isForemanShown)
            cd_toast(page, "Reset provider to default settings.", 0)
        case 176: // CP_ASK_PS_DELETE
            let name = store.selectedInstance?.displayName ?? "provider"
            store.deleteSelectedCustom()
            cd_toast(page, "Deleted \(name).", 0)
        case 177: // CP_ASK_PS_RT_ACTION
            let mgr = AntigravityRuntimeManager.shared
            if mgr.isInstalling {
                mgr.cancelInstall()
            } else {
                mgr.startInstallOrUpdate()
            }
        case 178: // CP_ASK_PS_RT_REMOVE
            let res = AntigravityRuntimeManager.shared.removeManagedRuntime(instances: store.instances)
            cd_toast(page, res.message, res.ok ? 0 : 1)
            store.refreshAll()
        case 179: // CP_ASK_PS_AUTH_START
            // Flush any unsaved inline edits (e.g. GCP project/location/API key typed right before clicking Connect).
            store.persistSelectedConfigOnly(
                displayName: psReadField(0),
                accent: Int(cd_ps_int(page, 0)),
                authMethod: AntigravityAuthMethod.from(uiIndex: cd_ps_int(page, 1)),
                newApiKey: psReadField(1),
                gcpProject: psReadField(2),
                gcpLocation: psReadField(3),
                binaryPath: psReadField(4),
                extraPath: psReadField(5)
            )
            configureWorkerEnv(run, isForeman: isForemanShown)
            guard let sel = store.selectedInstance else { return }
            if sel.driver == .antigravity {
                let probe = store.probes[sel.id]
                let forceLogin = probe?.authStatus == "authenticated"
                AntigravityAuthManager.shared.startAuth(for: sel, forceLogin: forceLogin, hostName: store.hostDisplayName)
            } else {
                store.probeInstance(id: sel.id)
            }
        case 180: // CP_ASK_PS_AUTH_CANCEL
            if let sel = store.selectedInstance {
                AntigravityAuthManager.shared.cancelAuth(instanceId: sel.id)
            }
        case 181: // CP_ASK_PS_AUTH_SIGNOUT
            guard let sel = store.selectedInstance else { return }
            // Stop idle Antigravity worker so logout takes effect cleanly.
            if run.worker.program == "agyworker" && cd_running(page) == 0 {
                run.worker.stop()
            }
            AntigravityAuthManager.shared.signOut(for: sel) {
                store.markSignedOut(instanceId: sel.id)
                store.probeInstance(id: sel.id)
                cd_toast(self.page, "Signed out of \(sel.displayName).", 0)
            }
        case 182: // CP_ASK_PS_AUTH_OPEN_URL
            let urlStr = psReadField(7)
            if let url = URL(string: urlStr),
               let scheme = url.scheme?.lowercased(), scheme == "http" || scheme == "https" {
                NSWorkspace.shared.open(url)
            }
        case 183: // CP_ASK_PS_AUTH_COPY_URL
            let urlStr = psReadField(7)
            if !urlStr.isEmpty {
                let pb = NSPasteboard.general
                pb.clearContents()
                pb.setString(urlStr, forType: .string)
                cd_toast(page, "Sign-in link copied.", 0)
            }
        case 184: // CP_ASK_PS_AUTH_SUBMIT
            if let sel = store.selectedInstance {
                let code = psReadField(6)
                AntigravityAuthManager.shared.submitCode(instanceId: sel.id, rawInput: code)
            }
        case 185: // CP_ASK_PS_ENV_SAVE
            let n = Int(cd_ps_int(page, 5))
            var vars: [(key: String, val: String, secret: Bool, hasSecret: Bool)] = []
            var kbuf = [CChar](repeating: 0, count: 128)
            var vbuf = [CChar](repeating: 0, count: 512)
            for i in 0..<n {
                var sec: Int32 = 0, hasSec: Int32 = 0
                if cd_ps_env_get(page, Int32(i), &kbuf, Int32(kbuf.count), &vbuf, Int32(vbuf.count), &sec, &hasSec) != 0 {
                    vars.append((String(cString: kbuf), String(cString: vbuf), sec != 0, hasSec != 0))
                }
            }
            store.updateSelectedEnvVars(vars)
            configureWorkerEnv(run, isForeman: isForemanShown)
        case 186: // CP_ASK_PS_MODELS_SAVE
            let n = Int(cd_ps_int(page, 6))
            let existing = Dictionary(uniqueKeysWithValues: (store.selectedInstance?.models ?? []).map { ($0.slug, $0) })
            var models: [ProviderModelConfig] = []
            var sbuf = [CChar](repeating: 0, count: 160)
            var nbuf = [CChar](repeating: 0, count: 160)
            for i in 0..<n {
                var en: Int32 = 0, fav: Int32 = 0, th: Int32 = 0, cu: Int32 = 0
                if cd_ps_model_get(page, Int32(i), &sbuf, Int32(sbuf.count), &nbuf, Int32(nbuf.count), &en, &fav, &th, &cu) != 0 {
                    let slug = String(cString: sbuf)
                    let name = String(cString: nbuf)
                    let prev = existing[slug]
                    models.append(ProviderModelConfig(
                        slug: slug, name: name,
                        enabled: en != 0, favorite: fav != 0, thinking: th != 0,
                        thoughtLevels: prev?.thoughtLevels ?? [], custom: cu != 0,
                        isDefault: prev?.isDefault ?? false,
                        defaultEffort: prev?.defaultEffort ?? "medium"
                    ))
                }
            }
            store.updateSelectedModels(models)
        case 187: // CP_ASK_PS_WIZ_CREATE
            let drv = ProviderDriverKind.from(uiIndex: cd_ps_int(page, 2))
            let accent = Int(cd_ps_int(page, 3))
            let authMethod = AntigravityAuthMethod.from(uiIndex: cd_ps_int(page, 4))
            store.addInstance(
                driver: drv,
                label: psReadField(10),
                id: psReadField(11),
                accent: accent,
                authMethod: authMethod,
                apiKey: psReadField(12),
                gcpProject: psReadField(13),
                gcpLocation: psReadField(14),
                binaryPath: psReadField(15),
                extraPath: psReadField(16)
            )
            cd_toast(page, "Added \(store.selectedInstance?.displayName ?? "provider") instance.", 0)
        default:
            break
        }
        syncProvidersSettingsPage()
        if cd_wants_text(page) != 0 { window.makeFirstResponder(canvas) }
        wake()
    }
}
